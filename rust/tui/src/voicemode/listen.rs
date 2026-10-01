//! Speech to text for voice mode (owner: voice-stt; plan §2, §4.3):
//! Voxtral Realtime over a websocket, else the voice role's batch model,
//! one request per turn.
//!
//! The realtime protocol (Mistral's SDK, mistralai/extra/realtime, and
//! 0659158's voice.rs): connect to `{base}/audio/transcriptions/realtime
//! ?model=…` (wss), read `session.created`, send `session.update` (our
//! format and delay), then `input_audio.append` (base64 PCM s16le); the
//! server sends `transcription.text.delta` as you talk. `input_audio.flush`
//! then `input_audio.end` close the stream: the last deltas come, then
//! `transcription.done`. Mistral has no "flushed" event for a flush alone,
//! so a session lasts one turn: at [`ListenMsg::Flush`] it is ended,
//! [`Heard::Flushed`] goes at `transcription.done` ([`FLUSH_WAIT`] at
//! most), and the next session opens at once (the handshake is hidden
//! while the agent works). [`ListenMsg::Clear`] drops the session.
//!
//! Failures: [`Heard::Failed`] with `voice::fail_lines`' wording ends the
//! turn (no `Flushed` for it; the audio is dropped until the next Flush
//! or Clear). A websocket that drops reconnects once, quietly. A handshake
//! that says the model is not there for this key (404, a model error)
//! switches to the batch model for the rest of voice mode, quietly.

use super::{Endpoint, Heard, ListenJob, ListenMsg, Listener, MIC_RATE};
use crate::voice::{self, http, stt, FailKind, Failure, VoiceJob};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The realtime model's delay behind your voice (a multiple of 80 ms).
pub const DELAY_MS: u32 = 480;
/// After a Flush, the last words come within this, or `Flushed` goes
/// without them.
pub const FLUSH_WAIT: Duration = Duration::from_secs(2);
/// Audio sent per websocket message.
const SEND_BLOCK: usize = MIC_RATE as usize / 10;
/// The TCP connect, the handshake and `session.created`, each.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// One batch request.
const BATCH_TIMEOUT: Duration = Duration::from_secs(60);
/// The batch clip keeps the last this many samples (5 min, ~9.6 MB):
/// the mic sends audio while nobody talks too.
const MAX_CLIP: usize = 5 * 60 * MIC_RATE as usize;
/// A session that carried this much audio after a reconnect earns the
/// next quiet reconnect.
const STEADY: usize = 10 * MIC_RATE as usize;
/// The server rejects the end of a stream without audio: nothing heard,
/// not a failure.
const EMPTY_STREAM: &str = "before sending any audio bytes";

/// The listener for `job`: realtime when it has an endpoint, else batch.
pub fn listener_for(job: &ListenJob) -> Box<dyn Listener> {
    if job.realtime.is_some() {
        Box::new(RealtimeListener)
    } else {
        Box::new(BatchListener)
    }
}

/// Voxtral Realtime (falls back to batch when the job has no realtime
/// endpoint, or the endpoint refuses realtime).
pub struct RealtimeListener;

impl Listener for RealtimeListener {
    fn start(&self, job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let mut inbox = Inbox::new(audio);
            let clip = match &job.realtime {
                Some(ep) => {
                    let billing = if ep.provider_name == job.batch.provider_name { job.batch.billing_url.as_str() } else { "" };
                    match run_realtime(ep, billing, &mut inbox, &events, &cancel) {
                        Some(clip) => clip,
                        None => return,
                    }
                }
                None => Vec::new(),
            };
            run_batch(&job.batch, clip, &mut inbox, &events, &cancel, &|req| http::send(req, BATCH_TIMEOUT));
        });
    }
}

/// One batch request per turn (voice::transcribe_clip), at Flush.
pub struct BatchListener;

impl Listener for BatchListener {
    fn start(&self, job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let mut inbox = Inbox::new(audio);
            run_batch(&job.batch, Vec::new(), &mut inbox, &events, &cancel, &|req| http::send(req, BATCH_TIMEOUT));
        });
    }
}

/// A failure as voice mode says it: `voice::fail_lines`' first line.
fn fail_line(f: &Failure, provider: &str, model: &str, billing: &str) -> String {
    voice::fail_lines(f, provider, model, billing, false).head
}

// ---- the controller's messages, in order ----

/// The controller's messages; a flush's wait reads ahead, what it read
/// waits here for after it.
struct Inbox {
    rx: Receiver<ListenMsg>,
    ahead: VecDeque<ListenMsg>,
    closed: bool,
}

impl Inbox {
    fn new(rx: Receiver<ListenMsg>) -> Inbox {
        Inbox { rx, ahead: VecDeque::new(), closed: false }
    }

    /// The next message, waiting up to `wait`; None: nothing yet, or the
    /// controller is gone ([`Inbox::done`]).
    fn next(&mut self, wait: Duration) -> Option<ListenMsg> {
        if let Some(m) = self.ahead.pop_front() {
            return Some(m);
        }
        if self.closed {
            return None;
        }
        match self.rx.recv_timeout(wait) {
            Ok(m) => Some(m),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.closed = true;
                None
            }
        }
    }

    /// Everything already sent, kept in order for later.
    fn read_ahead(&mut self) {
        while !self.closed {
            match self.rx.try_recv() {
                Ok(m) => self.ahead.push_back(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => self.closed = true,
            }
        }
    }

    fn done(&self) -> bool {
        self.closed && self.ahead.is_empty()
    }
}

// ---- the batch path ----

/// One `voice::transcribe_clip` per Flush over the audio since the last
/// one (`clip`: audio already collected). `send`: the HTTP call.
fn run_batch(
    job: &VoiceJob,
    mut clip: Vec<i16>,
    inbox: &mut Inbox,
    events: &Sender<Heard>,
    cancel: &AtomicBool,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) {
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        match inbox.next(Duration::from_millis(50)) {
            Some(ListenMsg::Audio(pcm)) => {
                clip.extend(pcm);
                if clip.len() > MAX_CLIP + MAX_CLIP / 4 {
                    clip.drain(..clip.len() - MAX_CLIP);
                }
            }
            Some(ListenMsg::Clear) => clip.clear(),
            Some(ListenMsg::Flush) => {
                let result = voice::transcribe_clip(job, &clip, cancel, send);
                clip.clear();
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let _ = match result {
                    Ok(text) => {
                        if !text.is_empty() {
                            let _ = events.send(Heard::Text(format!(" {}", text)));
                        }
                        events.send(Heard::Flushed)
                    }
                    Err(f) => events.send(Heard::Failed(fail_line(&f, &job.provider_name, &job.model, &job.billing_url))),
                };
            }
            None if inbox.done() => return,
            None => {}
        }
    }
}

// ---- the realtime protocol (pure) ----

/// The websocket URL of `base` (an http(s) API base, ".../v1").
pub fn realtime_url(base: &str, model: &str) -> Result<String, String> {
    let base = base.trim_end_matches('/');
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        return Err(format!("not an http(s) URL: {}", base));
    };
    Ok(format!("{}/audio/transcriptions/realtime?model={}", ws, stt::url_encode(model)))
}

pub fn session_update(sample_rate: u32, delay_ms: u32) -> String {
    json!({
        "type": "session.update",
        "session": {
            "audio_format": {"encoding": "pcm_s16le", "sample_rate": sample_rate},
            "target_streaming_delay_ms": delay_ms,
        }
    })
    .to_string()
}

/// `samples` as base64 PCM s16le.
pub fn append(samples: &[i16]) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    json!({"type": "input_audio.append", "audio": base64::engine::general_purpose::STANDARD.encode(bytes)}).to_string()
}

pub fn flush() -> String {
    json!({"type": "input_audio.flush"}).to_string()
}

pub fn end() -> String {
    json!({"type": "input_audio.end"}).to_string()
}

/// What the server says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Server {
    Created,
    /// new words, with their leading space
    Delta(String),
    /// the stream ended; the whole session's text
    Done(String),
    /// one line
    Error(String),
    /// an event voice mode does not use (session.updated, the language, …)
    Other,
}

pub fn parse_server(text: &str) -> Server {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Server::Other };
    let s = |p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
    match v.get("type").and_then(Value::as_str) {
        Some("session.created") => Server::Created,
        Some("transcription.text.delta") => Server::Delta(s("/text")),
        Some("transcription.done") => Server::Done(s("/text")),
        Some("error") => {
            let msg = server_error(&v);
            if msg.contains(EMPTY_STREAM) {
                Server::Done(String::new())
            } else {
                Server::Error(msg)
            }
        }
        _ => Server::Other,
    }
}

/// {"error": {"message": "…" | {"detail": "…"}}}, {"error": "…"}.
fn server_error(v: &Value) -> String {
    let str_of = |x: &Value| x.as_str().map(str::to_string);
    let e = v.get("error");
    let msg = e
        .and_then(|e| e.get("message"))
        .and_then(|m| str_of(m).or_else(|| m.get("detail").and_then(str_of)))
        .or_else(|| e.and_then(str_of))
        .or_else(|| v.get("message").and_then(str_of))
        .unwrap_or_else(|| "realtime transcription error".into());
    stt::one_line(&msg)
}

// ---- the websocket ----

type Ws = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>;

struct Conn {
    ws: Ws,
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("rustls: the default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

fn down(e: impl ToString) -> Failure {
    Failure { kind: FailKind::Down, said: stt::one_line(&e.to_string()) }
}

/// Opens a session: the handshake, `session.created`, our format.
fn dial(ep: &Endpoint) -> Result<Conn, Failure> {
    use tungstenite::client::IntoClientRequest;
    use tungstenite::handshake::HandshakeError;
    let other = |e: String| Failure { kind: FailKind::Other, said: stt::one_line(&e) };
    let url = realtime_url(&ep.base_url, &ep.model).map_err(other)?;
    let (tls, host, port, _) = http::split_url(&ep.base_url).map_err(other)?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(down)?
        .next()
        .ok_or_else(|| down(format!("no address for {}", host)))?;
    let tcp = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(down)?;
    let _ = tcp.set_nodelay(true);
    let _ = tcp.set_read_timeout(Some(CONNECT_TIMEOUT));
    let _ = tcp.set_write_timeout(Some(CONNECT_TIMEOUT));
    let mut req = url.into_client_request().map_err(|e| other(e.to_string()))?;
    let auth = format!("Bearer {}", ep.key)
        .parse()
        .map_err(|_| Failure { kind: FailKind::WrongKey, said: String::new() })?;
    req.headers_mut().insert("Authorization", auth);
    req.headers_mut().insert("User-Agent", tungstenite::http::HeaderValue::from_static("bise"));
    let connector = if tls { tungstenite::Connector::Rustls(tls_config()) } else { tungstenite::Connector::Plain };
    let (ws, _) = tungstenite::client_tls_with_config(req, tcp, None, Some(connector)).map_err(|e| match e {
        HandshakeError::Failure(tungstenite::Error::Http(resp)) => {
            Failure::of_answer(resp.status().as_u16(), resp.body().as_deref().unwrap_or(&[]), &ep.key)
        }
        HandshakeError::Failure(e) => down(e),
        HandshakeError::Interrupted(_) => down("the handshake timed out"),
    })?;
    let mut conn = Conn { ws };
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(down("no session.created"));
        }
        match conn.read(left) {
            Ok(Some(Server::Created)) => break,
            Ok(Some(Server::Error(m))) => return Err(Failure { kind: FailKind::Other, said: m }),
            Ok(_) => {}
            Err(e) => return Err(down(e)),
        }
    }
    conn.send(session_update(MIC_RATE, DELAY_MS)).map_err(down)?;
    Ok(conn)
}

impl Conn {
    fn send(&mut self, text: String) -> Result<(), String> {
        self.ws.send(tungstenite::Message::text(text)).map_err(|e| e.to_string())
    }

    /// The next server event within `wait`; Ok(None): nothing yet; Err:
    /// the socket closed or broke.
    fn read(&mut self, wait: Duration) -> Result<Option<Server>, String> {
        use tungstenite::stream::MaybeTlsStream;
        let wait = Some(wait.max(Duration::from_millis(1)));
        let _ = match self.ws.get_mut() {
            MaybeTlsStream::Plain(s) => s.set_read_timeout(wait),
            MaybeTlsStream::Rustls(s) => s.get_mut().set_read_timeout(wait),
            _ => Ok(()),
        };
        match self.ws.read() {
            Ok(tungstenite::Message::Text(t)) => Ok(Some(parse_server(t.as_str()))),
            Ok(tungstenite::Message::Close(_)) => Err("the server closed the session".into()),
            Ok(_) => Ok(Some(Server::Other)),
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                Ok(None)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn close(mut self) {
        let _ = self.ws.close(None);
        let _ = self.ws.flush();
    }
}

// ---- the realtime path ----

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Go,
    /// the endpoint refuses realtime: the batch model takes over
    Batch,
}

/// One realtime turn at a time.
struct Realtime<'a> {
    ep: &'a Endpoint,
    billing: &'a str,
    events: &'a Sender<Heard>,
    conn: Option<Conn>,
    /// audio not sent yet (a block builds up, or the session opens)
    pending: Vec<i16>,
    /// samples sent in this session
    sent: usize,
    /// the turn's text so far (sent as Text)
    text: String,
    /// this turn reconnected once already
    retried: bool,
    /// Failed was sent: the turn's audio is dropped until Flush or Clear
    failed: bool,
}

impl Realtime<'_> {
    fn heard(&mut self, words: String) {
        if !words.is_empty() {
            self.text.push_str(&words);
            let _ = self.events.send(Heard::Text(words));
        }
    }

    fn fail(&mut self, f: &Failure) {
        let _ = self.events.send(Heard::Failed(fail_line(f, &self.ep.provider_name, &self.ep.model, self.billing)));
        if let Some(c) = self.conn.take() {
            c.close();
        }
        self.pending.clear();
        self.failed = true;
    }

    /// A new turn: the session closed, its words forgotten.
    fn new_turn(&mut self) {
        if let Some(c) = self.conn.take() {
            c.close();
        }
        self.sent = 0;
        self.text.clear();
        self.retried = false;
        self.failed = false;
    }

    /// The socket dropped: the next send opens a new session, once a
    /// turn (a session that ran [`STEADY`] since earns another).
    fn dropped(&mut self, why: String) {
        self.conn = None;
        if self.retried {
            self.fail(&down(why));
        } else {
            self.retried = true;
            self.sent = 0;
        }
    }

    /// Opens the session when there is none.
    fn open(&mut self) -> Result<(), Failure> {
        if self.conn.is_none() {
            self.conn = Some(dial(self.ep)?);
        }
        Ok(())
    }

    /// Opens the next session ahead of its audio (hides the handshake);
    /// a failure here is quiet: the first audio tries again and says it.
    fn prewarm(&mut self) {
        if !self.failed {
            let _ = self.open();
        }
    }

    /// Sends the pending audio, opening the session if needed.
    fn send_pending(&mut self) -> Step {
        if self.failed {
            self.pending.clear();
            return Step::Go;
        }
        if self.pending.is_empty() {
            return Step::Go;
        }
        if let Err(f) = self.open() {
            if f.kind == FailKind::Model && self.sent == 0 && self.text.is_empty() {
                return Step::Batch;
            }
            self.fail(&f);
            return Step::Go;
        }
        let msg = append(&self.pending);
        match self.conn.as_mut().map(|c| c.send(msg)) {
            Some(Ok(())) => {
                self.sent += self.pending.len();
                self.pending.clear();
                if self.retried && self.sent >= STEADY {
                    self.retried = false;
                }
            }
            Some(Err(e)) => self.dropped(e),
            None => {}
        }
        Step::Go
    }

    /// Reads what the server sent within `wait`.
    fn pump(&mut self, wait: Duration) {
        let mut wait = wait;
        while let Some(conn) = self.conn.as_mut() {
            match conn.read(wait) {
                Ok(None) => break,
                Ok(Some(Server::Delta(t))) => self.heard(t),
                Ok(Some(Server::Error(m))) => self.fail(&Failure { kind: FailKind::Other, said: m }),
                Ok(Some(Server::Done(_))) => self.dropped("the server ended the session".into()),
                Ok(Some(_)) => {}
                Err(e) => self.dropped(e),
            }
            wait = Duration::from_millis(1);
        }
    }

    /// The turn ends: every word of its audio, then Flushed (or Failed).
    fn flush(&mut self, inbox: &mut Inbox, cancel: &AtomicBool) -> Step {
        // a dropped send reconnects once: two tries
        for _ in 0..2 {
            if self.pending.is_empty() || self.failed {
                break;
            }
            if self.send_pending() == Step::Batch {
                return Step::Batch;
            }
        }
        if self.failed {
            self.new_turn();
            return Step::Go;
        }
        let sent = self.sent;
        let Some(conn) = self.conn.as_mut().filter(|_| sent > 0) else {
            self.new_turn();
            let _ = self.events.send(Heard::Flushed);
            return Step::Go;
        };
        if conn.send(flush()).and_then(|_| conn.send(end())).is_ok() {
            let deadline = Instant::now() + FLUSH_WAIT;
            loop {
                if cancel.load(Ordering::SeqCst) {
                    return Step::Go;
                }
                inbox.read_ahead();
                let left = deadline.saturating_duration_since(Instant::now());
                let Some(conn) = self.conn.as_mut().filter(|_| !left.is_zero()) else { break };
                match conn.read(left.min(Duration::from_millis(20))) {
                    Ok(Some(Server::Delta(t))) => self.heard(t),
                    Ok(Some(Server::Done(all))) => {
                        // words the deltas missed, if any
                        if all.len() > self.text.len() && all.starts_with(&self.text) {
                            let rest = all[self.text.len()..].to_string();
                            self.heard(rest);
                        }
                        break;
                    }
                    Ok(Some(Server::Error(m))) => {
                        self.fail(&Failure { kind: FailKind::Other, said: m });
                        self.new_turn();
                        return Step::Go;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
        self.new_turn();
        let _ = self.events.send(Heard::Flushed);
        Step::Go
    }
}

/// The realtime session until the controller goes or cancels; Some(the
/// audio not transcribed yet): the endpoint refused realtime, batch takes
/// over.
fn run_realtime(ep: &Endpoint, billing: &str, inbox: &mut Inbox, events: &Sender<Heard>, cancel: &AtomicBool) -> Option<Vec<i16>> {
    let mut rt = Realtime {
        ep,
        billing,
        events,
        conn: None,
        pending: Vec::new(),
        sent: 0,
        text: String::new(),
        retried: false,
        failed: false,
    };
    loop {
        if cancel.load(Ordering::SeqCst) {
            rt.new_turn();
            return None;
        }
        let wait = Duration::from_millis(if rt.conn.is_some() { 10 } else { 50 });
        let step = match inbox.next(wait) {
            Some(ListenMsg::Audio(pcm)) => {
                if !rt.failed {
                    rt.pending.extend(pcm);
                }
                Step::Go
            }
            Some(ListenMsg::Flush) => {
                let step = rt.flush(inbox, cancel);
                if step == Step::Go && !inbox.done() && !cancel.load(Ordering::SeqCst) {
                    rt.prewarm();
                }
                step
            }
            Some(ListenMsg::Clear) => {
                rt.new_turn();
                rt.pending.clear();
                rt.prewarm();
                Step::Go
            }
            None if inbox.done() => {
                rt.new_turn();
                return None;
            }
            None => Step::Go,
        };
        if step == Step::Batch || (rt.pending.len() >= SEND_BLOCK && rt.send_pending() == Step::Batch) {
            rt.new_turn();
            return Some(std::mem::take(&mut rt.pending));
        }
        rt.pump(Duration::from_millis(5));
    }
}

#[cfg(test)]
#[path = "listen_tests.rs"]
mod tests;
