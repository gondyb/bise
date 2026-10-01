//! The listener's tests: the protocol's pure parts, a fake realtime
//! server on 127.0.0.1 (the session, the flush timing, Clear, the
//! reconnect, the failures), the batch path over voice's fake HTTP
//! server. One `#[ignore]` live test (by hand: a `say -o` file, no sound).

use super::*;
use crate::voice::fakes::serve_once;
use std::net::TcpListener;

type Sock = tungstenite::WebSocket<TcpStream>;

const WAIT: Duration = Duration::from_secs(3);

// ---- the pure parts ----

#[test]
fn the_url_is_the_api_base_over_websocket() {
    assert_eq!(
        realtime_url("https://api.mistral.ai/v1/", "voxtral-mini-transcribe-realtime-2602").unwrap(),
        "wss://api.mistral.ai/v1/audio/transcriptions/realtime?model=voxtral-mini-transcribe-realtime-2602"
    );
    assert_eq!(realtime_url("http://127.0.0.1:9/v1", "a b").unwrap(), "ws://127.0.0.1:9/v1/audio/transcriptions/realtime?model=a%20b");
    assert!(realtime_url("ftp://x", "m").is_err());
}

#[test]
fn the_client_messages() {
    let v: Value = serde_json::from_str(&session_update(16_000, 480)).unwrap();
    assert_eq!(v["type"], "session.update");
    assert_eq!(v["session"]["audio_format"]["encoding"], "pcm_s16le");
    assert_eq!(v["session"]["audio_format"]["sample_rate"], 16_000);
    assert_eq!(v["session"]["target_streaming_delay_ms"], 480);
    assert_eq!(serde_json::from_str::<Value>(&flush()).unwrap()["type"], "input_audio.flush");
    assert_eq!(serde_json::from_str::<Value>(&end()).unwrap()["type"], "input_audio.end");
}

#[test]
fn append_is_base64_pcm_s16le() {
    use base64::Engine;
    let v: Value = serde_json::from_str(&append(&[1, -2, 0x1234])).unwrap();
    assert_eq!(v["type"], "input_audio.append");
    let bytes = base64::engine::general_purpose::STANDARD.decode(v["audio"].as_str().unwrap()).unwrap();
    assert_eq!(bytes, vec![1, 0, 0xfe, 0xff, 0x34, 0x12]);
}

#[test]
fn the_server_events() {
    assert_eq!(parse_server(r#"{"type":"session.created","session":{}}"#), Server::Created);
    assert_eq!(parse_server(r#"{"type":"transcription.text.delta","text":" hello"}"#), Server::Delta(" hello".into()));
    assert_eq!(parse_server(r#"{"type":"transcription.done","text":" hello there","model":"m"}"#), Server::Done(" hello there".into()));
    assert_eq!(parse_server(r#"{"type":"error","error":{"message":"bad   thing\nhere"}}"#), Server::Error("bad thing here".into()));
    assert_eq!(parse_server(r#"{"type":"error","error":{"message":{"detail":"no"}}}"#), Server::Error("no".into()));
    assert_eq!(parse_server(r#"{"type":"error","error":"plain"}"#), Server::Error("plain".into()));
    assert_eq!(parse_server(r#"{"type":"error"}"#), Server::Error("realtime transcription error".into()));
    // ending a stream that had no audio: nothing heard, not a failure
    assert_eq!(
        parse_server(r#"{"type":"error","error":{"message":"input_audio.end received before sending any audio bytes"}}"#),
        Server::Done(String::new())
    );
    assert_eq!(parse_server(r#"{"type":"session.updated"}"#), Server::Other);
    assert_eq!(parse_server(r#"{"type":"transcription.language","audio_language":"fr"}"#), Server::Other);
    assert_eq!(parse_server("not json"), Server::Other);
}

// ---- a fake realtime server ----

/// What one connection of the fake server does.
#[derive(Clone, Copy)]
enum Script {
    /// a delta " w<n>" per append; at the end, done with the deltas' text
    /// and " tail" (a word the deltas missed)
    Echo,
    /// reads, never answers (the flush timing)
    Mute,
    /// answers the first `n` appends, then drops the socket
    DropAfter(usize),
    /// an error event at the first append
    ErrorAtFirst,
    /// refuses the handshake with this status
    Reject(u16),
}

/// The server: the scripts in order, one per connection. The log: one
/// line per thing it saw ("connect <path> <auth>", the client messages'
/// types, "closed", "gone", "dropped").
fn fake(scripts: Vec<Script>) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (log, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for script in scripts {
            let Ok((tcp, _)) = listener.accept() else { return };
            tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            let log = log.clone();
            // one thread per connection: a dropped one may still be open
            std::thread::spawn(move || serve(tcp, script, &log));
        }
    });
    (base, rx)
}

// tungstenite's handshake callback returns its ErrorResponse by value
#[allow(clippy::result_large_err)]
fn serve(tcp: TcpStream, script: Script, log: &Sender<String>) {
    use tungstenite::handshake::server::{ErrorResponse, Request, Response};
    let seen = log.clone();
    let callback = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let auth = req.headers().get("authorization").map(|v| v.to_str().unwrap().to_string()).unwrap_or_default();
        let _ = seen.send(format!("connect {} {}", req.uri(), auth));
        match script {
            Script::Reject(status) => {
                let body = Some(format!("{{\"message\":\"refused with {}\"}}", status));
                Err(tungstenite::http::Response::builder().status(status).body(body).unwrap())
            }
            _ => Ok(resp),
        }
    };
    let Ok(mut ws) = tungstenite::accept_hdr(tcp, callback) else { return };
    let say = |ws: &mut Sock, v: Value| ws.send(tungstenite::Message::text(v.to_string())).unwrap();
    say(&mut ws, json!({"type": "session.created", "session": {"request_id": "r1"}}));
    let mut appends = 0;
    let mut text = String::new();
    loop {
        match ws.read() {
            Ok(tungstenite::Message::Text(t)) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                let kind = v["type"].as_str().unwrap().to_string();
                let _ = log.send(kind.clone());
                match (kind.as_str(), script) {
                    ("input_audio.append", Script::DropAfter(n)) if appends == n => {
                        let _ = log.send("dropped".into());
                        return;
                    }
                    ("input_audio.append", Script::ErrorAtFirst) => {
                        say(&mut ws, json!({"type": "error", "error": {"message": "the session broke"}}));
                    }
                    ("input_audio.append", Script::Echo | Script::DropAfter(_)) => {
                        appends += 1;
                        let w = format!(" w{}", appends);
                        text.push_str(&w);
                        say(&mut ws, json!({"type": "transcription.text.delta", "text": w}));
                    }
                    ("input_audio.end", Script::Echo) => {
                        let all = format!("{} tail", text);
                        say(&mut ws, json!({"type": "transcription.done", "text": all, "model": "m", "language": null}));
                    }
                    _ => {}
                }
            }
            Ok(tungstenite::Message::Close(_)) => {
                let _ = log.send("closed".into());
                return;
            }
            Ok(_) => {}
            Err(_) => {
                let _ = log.send("gone".into());
                return;
            }
        }
    }
}

fn endpoint(base: &str) -> Endpoint {
    Endpoint {
        name: "mistral/voxtral-mini-transcribe-realtime-2602".into(),
        provider_name: "Mistral".into(),
        base_url: base.into(),
        model: "voxtral-mini-transcribe-realtime-2602".into(),
        key: "sk-test".into(),
    }
}

struct Run {
    audio: Sender<ListenMsg>,
    heard: Receiver<Heard>,
    cancel: Arc<AtomicBool>,
}

impl Run {
    fn block(&self) {
        self.audio.send(ListenMsg::Audio(vec![3000; SEND_BLOCK])).unwrap();
    }
    fn next(&self) -> Heard {
        self.heard.recv_timeout(WAIT).expect("the listener said nothing")
    }
    fn quiet(&self, d: Duration) -> Option<Heard> {
        self.heard.recv_timeout(d).ok()
    }
}

fn start(listener: &dyn Listener, realtime: Option<Endpoint>, batch: VoiceJob) -> Run {
    let (audio, rx) = mpsc::channel();
    let (tx, heard) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    listener.start(ListenJob { realtime, batch }, rx, tx, cancel.clone());
    Run { audio, heard, cancel }
}

fn realtime(base: &str) -> Run {
    start(&RealtimeListener, Some(endpoint(base)), crate::voice::fakes::job())
}

/// The log's lines until `line` (included); panics after [`WAIT`].
fn log_until(log: &Receiver<String>, line: &str) -> Vec<String> {
    let mut seen = Vec::new();
    loop {
        let l = log.recv_timeout(WAIT).unwrap_or_else(|_| panic!("no {:?} in {:?}", line, seen));
        let done = l.starts_with(line);
        seen.push(l);
        if done {
            return seen;
        }
    }
}

#[test]
fn listener_for_picks_realtime_when_there_is_an_endpoint() {
    // no network: a batch job with no realtime endpoint and a closed channel
    let job = ListenJob { realtime: None, batch: crate::voice::fakes::job() };
    let _ = listener_for(&job);
    let job = ListenJob { realtime: Some(endpoint("http://127.0.0.1:9/v1")), ..job };
    let _ = listener_for(&job);
}

#[test]
fn words_come_as_you_talk_and_flushed_after_the_last_ones() {
    let (base, log) = fake(vec![Script::Echo, Script::Echo]);
    let run = realtime(&base);
    for _ in 0..3 {
        run.block();
    }
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    assert_eq!(run.next(), Heard::Text(" w2".into()));
    assert_eq!(run.next(), Heard::Text(" w3".into()));
    run.audio.send(ListenMsg::Flush).unwrap();
    // the word the deltas missed, then Flushed
    assert_eq!(run.next(), Heard::Text(" tail".into()));
    assert_eq!(run.next(), Heard::Flushed);
    let seen = log_until(&log, "closed");
    assert_eq!(seen[0], "connect /v1/audio/transcriptions/realtime?model=voxtral-mini-transcribe-realtime-2602 Bearer sk-test");
    assert_eq!(seen[1], "session.update");
    assert_eq!(&seen[2..], ["input_audio.append"; 3].iter().chain(&["input_audio.flush", "input_audio.end", "closed"]).copied().collect::<Vec<_>>());
    // the next session opens at once, before any audio (the handshake hidden)
    assert!(log_until(&log, "connect")[0].starts_with("connect"));
    run.block();
    assert_eq!(run.next(), Heard::Text(" w1".into()));
}

#[test]
fn audio_sent_while_a_flush_waits_belongs_to_the_next_turn() {
    let (base, _log) = fake(vec![Script::Mute, Script::Echo]);
    let run = realtime(&base);
    run.block();
    run.audio.send(ListenMsg::Flush).unwrap();
    run.block();
    run.block();
    assert_eq!(run.next(), Heard::Flushed);
    assert_eq!(run.next(), Heard::Text(" w1".into()));
}

#[test]
fn flushed_comes_within_two_seconds_when_the_server_says_nothing() {
    let (base, log) = fake(vec![Script::Mute]);
    let run = realtime(&base);
    run.block();
    log_until(&log, "input_audio.append");
    let t = Instant::now();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    let took = t.elapsed();
    assert!(took >= FLUSH_WAIT - Duration::from_millis(50) && took < FLUSH_WAIT + Duration::from_millis(700), "{:?}", took);
}

#[test]
fn a_flush_with_no_audio_is_flushed_at_once() {
    let (base, _log) = fake(vec![Script::Echo]);
    let run = realtime(&base);
    let t = Instant::now();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    assert!(t.elapsed() < Duration::from_millis(500));
}

#[test]
fn clear_drops_the_session_and_its_words() {
    let (base, log) = fake(vec![Script::Echo, Script::Echo]);
    let run = realtime(&base);
    run.block();
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    run.audio.send(ListenMsg::Clear).unwrap();
    let seen = log_until(&log, "closed");
    assert!(!seen.contains(&"input_audio.end".to_string()), "{:?}", seen);
    run.block();
    // a new session: its own words
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Text(" tail".into()));
    assert_eq!(run.next(), Heard::Flushed);
}

#[test]
fn a_dropped_socket_reconnects_once_quietly() {
    let (base, _log) = fake(vec![Script::DropAfter(1), Script::Echo]);
    let run = realtime(&base);
    run.block();
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    // the drop shows at the next send or read; the next blocks reconnect
    for _ in 0..3 {
        run.block();
        std::thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    run.audio.send(ListenMsg::Flush).unwrap();
    loop {
        match run.next() {
            Heard::Text(_) => {}
            h => {
                assert_eq!(h, Heard::Flushed);
                break;
            }
        }
    }
}

#[test]
fn a_second_drop_in_a_turn_fails_and_the_turn_has_no_flushed() {
    let (base, _log) = fake(vec![Script::DropAfter(1), Script::DropAfter(1)]);
    let run = realtime(&base);
    let mut failed = None;
    for _ in 0..40 {
        run.block();
        match run.quiet(Duration::from_millis(50)) {
            Some(Heard::Failed(l)) => {
                failed = Some(l);
                break;
            }
            Some(Heard::Text(_)) | None => {}
            Some(h) => panic!("{:?}", h),
        }
    }
    assert_eq!(failed.as_deref(), Some("i couldn't reach Mistral to transcribe. try again, or /voice setup for another provider."));
    run.block();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.quiet(Duration::from_millis(400)), None);
}

#[test]
fn a_refused_key_fails_with_voice_wording() {
    let (base, _log) = fake(vec![Script::Reject(401)]);
    let run = realtime(&base);
    run.block();
    assert_eq!(run.next(), Heard::Failed("Mistral says the voice key is wrong. /provider fixes it.".into()));
}

#[test]
fn an_error_event_fails_the_turn() {
    let (base, _log) = fake(vec![Script::ErrorAtFirst]);
    let run = realtime(&base);
    run.block();
    match run.next() {
        Heard::Failed(l) => assert!(l.starts_with("Mistral answered something i can't read"), "{}", l),
        h => panic!("{:?}", h),
    }
}

#[test]
fn a_model_without_realtime_falls_back_to_batch_quietly() {
    let (base, _log) = fake(vec![Script::Reject(404)]);
    let (http_base, request) = serve_once(200, r#"{"text":"hello there"}"#);
    let batch = VoiceJob { base_url: http_base, ..crate::voice::fakes::job() };
    let run = start(&RealtimeListener, Some(endpoint(&base)), batch);
    for _ in 0..3 {
        run.block();
    }
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Text(" hello there".into()));
    assert_eq!(run.next(), Heard::Flushed);
    let req = String::from_utf8_lossy(&request.recv_timeout(WAIT).unwrap()).to_string();
    assert!(req.starts_with("POST /v1/audio/transcriptions "), "{}", &req[..60]);
}

#[test]
fn the_session_closes_when_the_controller_goes() {
    let (base, log) = fake(vec![Script::Echo]);
    let run = realtime(&base);
    run.block();
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    drop(run.audio);
    log_until(&log, "closed");
}

#[test]
fn cancel_closes_the_session() {
    let (base, log) = fake(vec![Script::Echo]);
    let run = realtime(&base);
    run.block();
    assert_eq!(run.next(), Heard::Text(" w1".into()));
    run.cancel.store(true, Ordering::SeqCst);
    log_until(&log, "closed");
}

// ---- the batch path ----

fn batch(base: &str) -> Run {
    start(&BatchListener, None, VoiceJob { base_url: base.into(), ..crate::voice::fakes::job() })
}

#[test]
fn batch_transcribes_the_turn_at_flush() {
    let (base, request) = serve_once(200, r#"{"text":"  fix the tests "}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    assert_eq!(run.quiet(Duration::from_millis(200)), None, "nothing before the flush");
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Text(" fix the tests".into()));
    assert_eq!(run.next(), Heard::Flushed);
    let req = request.recv_timeout(WAIT).unwrap();
    // 4 blocks of 1600 samples, 16-bit, in a WAV: the whole turn
    assert!(req.len() > 4 * SEND_BLOCK * 2);
}

#[test]
fn batch_under_min_clip_is_flushed_at_once_without_a_request() {
    let (base, request) = serve_once(200, r#"{"text":"never"}"#);
    let run = batch(&base);
    run.audio.send(ListenMsg::Audio(vec![3000; 800])).unwrap();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    assert!(request.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn batch_clear_drops_the_half_turn() {
    let (base, request) = serve_once(200, r#"{"text":"never"}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    run.audio.send(ListenMsg::Clear).unwrap();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    assert!(request.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn batch_failure_says_it_in_voice_wording() {
    let (base, _request) = serve_once(401, r#"{"message":"Unauthorized"}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Failed("Mistral says the voice key is wrong. /provider fixes it.".into()));
}

// ---- live (by hand, no sound) ----

/// The 16 kHz mono PCM of a WAV file (its "data" chunk).
fn wav_pcm(bytes: &[u8]) -> Vec<i16> {
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        if &bytes[i..i + 4] == b"data" {
            let data = &bytes[i + 8..(i + 8 + len).min(bytes.len())];
            return data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk")
}

/// Realtime STT of a `say -o` sentence with the voice role's Mistral key
/// (`cargo test -p bise-tui live_realtime -- --ignored --nocapture`).
#[test]
#[ignore]
fn live_realtime_transcribes_a_spoken_sentence() {
    let batch = crate::voice::resolve_job().expect("a voice key");
    assert_eq!(batch.api, "mistral", "the voice role's provider is not Mistral");
    let dir = std::env::temp_dir().join(format!("bise-listen-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (aiff, wav) = (dir.join("s.aiff"), dir.join("s.wav"));
    let ok = std::process::Command::new("say").arg("-o").arg(&aiff).arg("please run the tests and fix the failing ones").status().unwrap();
    assert!(ok.success());
    let ok = std::process::Command::new("afconvert").args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"]).arg(&aiff).arg(&wav).status().unwrap();
    assert!(ok.success());
    let mut pcm = wav_pcm(&std::fs::read(&wav).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    pcm.extend(vec![0; MIC_RATE as usize / 2]);
    let ep = Endpoint {
        name: "mistral/voxtral-mini-transcribe-realtime-2602".into(),
        provider_name: batch.provider_name.clone(),
        base_url: batch.base_url.clone(),
        model: "voxtral-mini-transcribe-realtime-2602".into(),
        key: batch.key.clone(),
    };
    let run = start(&RealtimeListener, Some(ep), batch);
    let t = Instant::now();
    // at twice the real pace, in 100 ms blocks
    for b in pcm.chunks(SEND_BLOCK) {
        run.audio.send(ListenMsg::Audio(b.to_vec())).unwrap();
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut text = String::new();
    let mut while_talking = 0;
    while let Ok(Heard::Text(t)) = run.heard.try_recv() {
        text.push_str(&t);
        while_talking += 1;
    }
    let flush_at = Instant::now();
    run.audio.send(ListenMsg::Flush).unwrap();
    loop {
        match run.heard.recv_timeout(Duration::from_secs(10)).expect("no Flushed") {
            Heard::Text(t) => text.push_str(&t),
            Heard::Flushed => break,
            Heard::Failed(l) => panic!("{}", l),
        }
    }
    eprintln!(
        "heard {:?} ({} deltas while talking; flushed {:?} after the flush, {:?} in all)",
        text,
        while_talking,
        flush_at.elapsed(),
        t.elapsed()
    );
    let lower = text.to_lowercase();
    assert!(lower.contains("tests") && lower.contains("fix"), "{}", text);
    assert!(flush_at.elapsed() <= FLUSH_WAIT + Duration::from_millis(500));
}
