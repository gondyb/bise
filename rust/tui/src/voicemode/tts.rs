//! Text to speech (owner: voice-tts; plan §4.5): Voxtral TTS, streamed.
//! `POST {base}/audio/speech` with `stream: true` and `response_format:
//! "pcm"` answers Server-Sent Events: `speech.audio.delta` events carry
//! base64 little-endian f32 PCM at [`TTS_RATE`], `speech.audio.done`
//! ends it (Mistral's OpenAPI spec, `SpeechRequest`/`SpeechStreamEvents`).
//! The API has no speed field: `SayJob::speed` paces the word timings
//! only ([`super::timing`]).

use super::{SayJob, Synth, Synthesizer};
use crate::voice::http;
use base64::Engine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// The default voice until the brand voice is picked by ear: Mistral's
/// preset neutral English voice.
pub const DEFAULT_VOICE: &str = "en_paul_neutral";

/// The model when the settings name none.
pub const DEFAULT_MODEL: &str = "voxtral-mini-tts-2603";

/// The connect, and the longest silence of the stream.
const TIMEOUT: Duration = Duration::from_secs(15);

pub struct VoxtralTts;

impl Synthesizer for VoxtralTts {
    fn start(&self, job: SayJob, text: String, events: Sender<Synth>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            let req = request(&job, &text);
            synthesize(&job, &req, &events, &cancel, &|req, cancel, on_body| {
                http::stream(req, "text/event-stream", TIMEOUT, cancel, on_body)
            });
        });
    }
}

/// The streamed request: one sentence, PCM, the job's voice.
pub fn request(job: &SayJob, text: &str) -> http::Request {
    let model = if job.api.model.is_empty() { DEFAULT_MODEL } else { job.api.model.as_str() };
    let voice = if job.voice.trim().is_empty() { DEFAULT_VOICE } else { job.voice.trim() };
    let body = serde_json::json!({
        "model": model,
        "input": text,
        "voice_id": voice,
        "response_format": "pcm",
        "stream": true,
    });
    http::Request {
        url: format!("{}/audio/speech", job.api.base_url.trim_end_matches('/')),
        headers: vec![
            ("Authorization".into(), format!("Bearer {}", job.api.key)),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: body.to_string().into_bytes(),
    }
}

/// The streamed call (`http::stream`, or a fake in the tests).
pub type StreamFn<'a> =
    &'a dyn Fn(&http::Request, &AtomicBool, &mut dyn FnMut(&[u8]) -> bool) -> Result<http::Streamed, String>;

/// One synthesis: `Synth::Audio` as the chunks come, then `Done` or
/// `Failed`; nothing more once `cancel` is set.
pub fn synthesize(job: &SayJob, req: &http::Request, events: &Sender<Synth>, cancel: &AtomicBool, call: StreamFn) {
    let mut sse = Sse::default();
    let mut pcm = Pcm::default();
    let mut end: Option<Synth> = None;
    let result = call(req, cancel, &mut |bytes| {
        for ev in sse.feed(bytes) {
            match step(&ev, &mut pcm) {
                Step::Audio(a) => {
                    if !a.is_empty() && events.send(Synth::Audio(a)).is_err() {
                        return false;
                    }
                }
                Step::Done => {
                    end = Some(Synth::Done);
                    return false;
                }
                Step::Failed(why) => {
                    end = Some(Synth::Failed(why));
                    return false;
                }
                Step::Nothing => {}
            }
        }
        true
    });
    if cancel.load(Ordering::SeqCst) {
        return;
    }
    let provider = job.api.provider_name.as_str();
    let last = match (result, end) {
        (_, Some(e)) => e,
        // the body ended without speech.audio.done: the audio is in
        (Ok(http::Streamed::Done), None) => Synth::Done,
        (Ok(http::Streamed::Stopped), None) => return,
        (Ok(http::Streamed::Refused(r)), None) => {
            Synth::Failed(fail_line(&crate::voice::Failure::of_answer(r.status, &r.body, &job.api.key), provider, &job.api.model))
        }
        (Err(e), None) => Synth::Failed(fail_line(
            &crate::voice::Failure { kind: crate::voice::FailKind::Down, said: crate::voice::stt::one_line(&e) },
            provider,
            &job.api.model,
        )),
    };
    let _ = events.send(last);
}

/// A failed synthesis in one line (`voice::fail_lines`' wording, said
/// for the voice).
pub fn fail_line(f: &crate::voice::Failure, provider: &str, model: &str) -> String {
    use crate::voice::FailKind;
    let head = match f.kind {
        FailKind::WrongKey => format!("{} says the voice key is wrong. /provider fixes it.", provider),
        FailKind::NoCredit => format!("your {} account has no credit yet.", provider),
        FailKind::Model => format!("{} can't speak with {}. /voice picks another voice.", provider, model),
        FailKind::Down => format!("i couldn't reach {} to speak. the answer is on screen.", provider),
        FailKind::Other => format!("{} answered something i can't play. the answer is on screen.", provider),
    };
    match f.kind {
        FailKind::Model | FailKind::Other | FailKind::Down if !f.said.is_empty() => format!("{} ({})", head, f.said),
        _ => head,
    }
}

// ---- Server-Sent Events ----

/// One event: its `event:` name ("" when none) and its `data:` lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

/// An SSE stream parsed as its bytes come (lines may split anywhere).
#[derive(Default)]
pub struct Sse {
    buf: Vec<u8>,
    cur: SseEvent,
    has_data: bool,
}

impl Sse {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        let mut at = 0;
        while let Some(nl) = self.buf[at..].iter().position(|b| *b == b'\n') {
            let line = String::from_utf8_lossy(&self.buf[at..at + nl]).trim_end_matches('\r').to_string();
            at += nl + 1;
            if line.is_empty() {
                if self.has_data || !self.cur.event.is_empty() {
                    out.push(std::mem::take(&mut self.cur));
                }
                self.has_data = false;
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
                None => (line.as_str(), ""),
            };
            match field {
                "event" => self.cur.event = value.to_string(),
                "data" => {
                    if self.has_data {
                        self.cur.data.push('\n');
                    }
                    self.cur.data.push_str(value);
                    self.has_data = true;
                }
                _ => {}
            }
        }
        self.buf.drain(..at);
        out
    }
}

/// What one event means.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Audio(Vec<f32>),
    Done,
    Failed(String),
    Nothing,
}

pub fn step(ev: &SseEvent, pcm: &mut Pcm) -> Step {
    if ev.data.trim() == "[DONE]" {
        return Step::Done;
    }
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap_or(serde_json::Value::Null);
    let kind = if ev.event.is_empty() { v.get("type").and_then(|t| t.as_str()).unwrap_or("") } else { ev.event.as_str() };
    match kind {
        "speech.audio.delta" => match v.get("audio_data").and_then(|a| a.as_str()) {
            Some(b64) => match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                Ok(bytes) => Step::Audio(pcm.push(&bytes)),
                Err(_) => Step::Failed("the voice sent audio i can't read.".into()),
            },
            None => Step::Nothing,
        },
        "speech.audio.done" => Step::Done,
        "error" => Step::Failed(error_line(&v, &ev.data)),
        _ if v.get("error").is_some() => Step::Failed(error_line(&v, &ev.data)),
        _ => Step::Nothing,
    }
}

fn error_line(v: &serde_json::Value, raw: &str) -> String {
    let said = v
        .pointer("/error/message")
        .or_else(|| v.get("message"))
        .or_else(|| v.get("error"))
        .and_then(|m| m.as_str())
        .unwrap_or(raw);
    format!("the voice stopped: {}", crate::voice::stt::one_line(said))
}

/// Little-endian f32 samples from byte chunks that may split a sample.
#[derive(Default)]
pub struct Pcm {
    carry: Vec<u8>,
}

impl Pcm {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<f32> {
        self.carry.extend_from_slice(bytes);
        let whole = self.carry.len() / 4 * 4;
        let out = self.carry[..whole]
            .chunks_exact(4)
            .map(|b| {
                let x = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if x.is_finite() {
                    x.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect();
        self.carry.drain(..whole);
        out
    }
}

#[cfg(test)]
#[path = "tts_tests.rs"]
mod tests;
