use super::*;
use crate::voicemode::{Endpoint, TTS_RATE};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Instant;

fn job(base: &str) -> SayJob {
    SayJob {
        api: Endpoint {
            name: "mistral/voxtral-mini-tts-2603".into(),
            provider_name: "Mistral".into(),
            base_url: base.into(),
            model: "voxtral-mini-tts-2603".into(),
            key: "sk-secret".into(),
        },
        voice: String::new(),
        speed: 1.0,
    }
}

fn b64(samples: &[f32]) -> String {
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn delta(samples: &[f32]) -> String {
    format!("event: speech.audio.delta\ndata: {{\"type\":\"speech.audio.delta\",\"audio_data\":\"{}\"}}\n\n", b64(samples))
}

const DONE: &str = "event: speech.audio.done\ndata: {\"type\":\"speech.audio.done\",\"usage\":{\"prompt_tokens\":3}}\n\n";

#[test]
fn the_request_streams_pcm_with_the_default_voice() {
    let r = request(&job("https://api.mistral.ai/v1/"), "on it.");
    assert_eq!(r.url, "https://api.mistral.ai/v1/audio/speech");
    let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(b["model"], "voxtral-mini-tts-2603");
    assert_eq!(b["input"], "on it.");
    assert_eq!(b["voice_id"], DEFAULT_VOICE);
    assert_eq!(b["response_format"], "pcm");
    assert_eq!(b["stream"], true);
    assert!(r.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-secret"));
    assert!(!format!("{r:?}").contains("sk-secret"));
    let mut j = job("https://x/v1");
    j.voice = "fr_marie_neutral".into();
    let b: serde_json::Value = serde_json::from_slice(&request(&j, "x").body).unwrap();
    assert_eq!(b["voice_id"], "fr_marie_neutral");
}

#[test]
fn sse_events_parse_across_any_split() {
    let stream = format!(": keepalive\n\n{}{}", delta(&[0.5, -0.25]), DONE).replace('\n', "\r\n");
    let bytes = stream.as_bytes();
    // every split point gives the same events
    for cut in 0..bytes.len() {
        let mut sse = Sse::default();
        let mut evs = sse.feed(&bytes[..cut]);
        evs.extend(sse.feed(&bytes[cut..]));
        assert_eq!(evs.len(), 2, "cut at {cut}");
        assert_eq!(evs[0].event, "speech.audio.delta");
        assert_eq!(evs[1].event, "speech.audio.done");
    }
    let mut sse = Sse::default();
    let evs = sse.feed(b"data: {\"a\":1}\ndata: {\"b\":2}\n\n");
    assert_eq!(evs, [SseEvent { event: String::new(), data: "{\"a\":1}\n{\"b\":2}".into() }]);
}

#[test]
fn deltas_decode_to_samples_even_split_mid_sample() {
    let mut pcm = Pcm::default();
    let ev = SseEvent { event: "speech.audio.delta".into(), data: format!("{{\"audio_data\":\"{}\"}}", b64(&[0.5, -0.25, 2.0])) };
    assert_eq!(step(&ev, &mut pcm), Step::Audio(vec![0.5, -0.25, 1.0]));
    let bytes: Vec<u8> = [0.125f32, 0.75].iter().flat_map(|s| s.to_le_bytes()).collect();
    assert_eq!(pcm.push(&bytes[..3]), Vec::<f32>::new());
    assert_eq!(pcm.push(&bytes[3..6]), vec![0.125]);
    assert_eq!(pcm.push(&bytes[6..]), vec![0.75]);
    // the type in the data, no event line
    let ev = SseEvent { event: String::new(), data: "{\"type\":\"speech.audio.done\",\"usage\":{}}".into() };
    assert_eq!(step(&ev, &mut pcm), Step::Done);
    let ev = SseEvent { event: "error".into(), data: "{\"message\":\"voice not found\"}".into() };
    assert_eq!(step(&ev, &mut pcm), Step::Failed("the voice stopped: voice not found".into()));
}

#[test]
fn chunked_bodies_dechunk_across_any_split() {
    let body = b"5\r\nhello\r\n6;ext=1\r\n world\r\n0\r\n\r\n";
    for cut in 0..body.len() {
        let mut d = http::Dechunker::default();
        let mut out = Vec::new();
        let a = d.feed(&body[..cut], &mut out).unwrap();
        let b = d.feed(&body[cut..], &mut out).unwrap();
        // done once the last chunk's size line is in (the trailer's CRLF is not needed)
        assert!(!a || cut >= body.len() - 2, "cut at {cut}");
        assert!(b);
        assert_eq!(out, b"hello world", "cut at {cut}");
    }
}

/// A fake TTS server: answers one request with `head` then the `parts`,
/// each as a chunk, `gap` apart; returns the request it got.
fn serve(head: &'static str, parts: Vec<String>, gap: Duration) -> (String, mpsc::Receiver<String>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut req = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = s.read(&mut buf) {
            req.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&req).to_string();
            if let Some(i) = text.find("\r\n\r\n") {
                let len: usize = text
                    .lines()
                    .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                    .unwrap_or(0);
                if req.len() >= i + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&req).to_string());
        let _ = s.write_all(head.as_bytes());
        for p in parts {
            std::thread::sleep(gap);
            if s.write_all(format!("{:x}\r\n{}\r\n", p.len(), p).as_bytes()).is_err() {
                let _ = tx.send("closed".into());
                return;
            }
        }
        let _ = s.write_all(b"0\r\n\r\n");
    });
    (base, rx)
}

const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";

#[test]
fn the_stream_sends_audio_as_it_comes_then_done() {
    let first = delta(&[0.1; 480]);
    // an event split across two chunks
    let second = delta(&[0.2; 480]);
    let (a, b) = second.split_at(17);
    let (base, got) = serve(SSE_HEAD, vec![first, a.into(), b.into(), DONE.into()], Duration::from_millis(5));
    let (tx, rx) = mpsc::channel();
    VoxtralTts.start(job(&base), "on it.".into(), tx, Arc::new(AtomicBool::new(false)));
    let req = got.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(req.starts_with("POST /v1/audio/speech HTTP/1.1"));
    assert!(req.contains("Accept: text/event-stream"));
    assert!(req.contains("\"stream\":true"));
    let evs: Vec<Synth> = rx.iter().collect();
    assert_eq!(evs, [Synth::Audio(vec![0.1; 480]), Synth::Audio(vec![0.2; 480]), Synth::Done]);
}

#[test]
fn cancel_closes_the_socket_and_says_nothing_more() {
    let parts: Vec<String> = (0..200).map(|_| delta(&[0.3; 240])).collect();
    let (base, got) = serve(SSE_HEAD, parts, Duration::from_millis(20));
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    VoxtralTts.start(job(&base), "a long sentence.".into(), tx, cancel.clone());
    let _req = got.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Synth::Audio(_)));
    let at = Instant::now();
    cancel.store(true, Ordering::SeqCst);
    // the thread ends (the sender drops) without Done or Failed
    loop {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Synth::Audio(_)) => {}
            Ok(other) => panic!("after cancel: {other:?}"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    assert!(at.elapsed() < Duration::from_millis(500), "{:?}", at.elapsed());
    // the server sees the socket closed
    assert_eq!(got.recv_timeout(Duration::from_secs(5)).unwrap(), "closed");
}

#[test]
fn a_refusal_is_one_line() {
    let (base, _got) = serve(
        "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 26\r\n\r\n{\"message\":\"Unauthorized\"}",
        vec![],
        Duration::ZERO,
    );
    let (tx, rx) = mpsc::channel();
    VoxtralTts.start(job(&base), "on it.".into(), tx, Arc::new(AtomicBool::new(false)));
    let evs: Vec<Synth> = rx.iter().collect();
    assert_eq!(evs, [Synth::Failed("Mistral says the voice key is wrong. /provider fixes it.".into())]);
}

#[test]
fn nobody_there_is_one_line() {
    // a port nobody listens on
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    drop(l);
    let (tx, rx) = mpsc::channel();
    VoxtralTts.start(job(&base), "on it.".into(), tx, Arc::new(AtomicBool::new(false)));
    match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
        Synth::Failed(line) => {
            assert!(line.starts_with("i couldn't reach Mistral to speak."), "{line}");
            assert!(!line.contains('\n'));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_error_event_mid_stream_fails() {
    let err = "event: error\ndata: {\"message\":\"rate limited\"}\n\n".to_string();
    let (base, _got) = serve(SSE_HEAD, vec![delta(&[0.1; 10]), err], Duration::from_millis(2));
    let (tx, rx) = mpsc::channel();
    VoxtralTts.start(job(&base), "x".into(), tx, Arc::new(AtomicBool::new(false)));
    let evs: Vec<Synth> = rx.iter().collect();
    assert_eq!(evs, [Synth::Audio(vec![0.1; 10]), Synth::Failed("the voice stopped: rate limited".into())]);
}

/// By hand, with the voice role's Mistral key: one sentence, decoded and
/// measured, never played. `cargo test -p bise-tui live_tts -- --ignored`
#[test]
#[ignore]
fn live_tts_one_sentence() {
    let v = crate::voice::resolve_job().expect("the voice role's job");
    let j = SayJob {
        api: Endpoint {
            name: "mistral/voxtral-mini-tts-2603".into(),
            provider_name: v.provider_name.clone(),
            base_url: v.base_url.clone(),
            model: DEFAULT_MODEL.into(),
            key: v.key.clone(),
        },
        voice: DEFAULT_VOICE.into(),
        speed: 1.0,
    };
    let (tx, rx) = mpsc::channel();
    let at = Instant::now();
    VoxtralTts.start(j, "Done: the login test waits for the event now.".into(), tx, Arc::new(AtomicBool::new(false)));
    let mut samples = Vec::new();
    let mut first = None;
    let mut chunks = 0;
    for ev in rx.iter() {
        match ev {
            Synth::Audio(a) => {
                first.get_or_insert(at.elapsed());
                chunks += 1;
                samples.extend(a);
            }
            Synth::Done => break,
            Synth::Failed(l) => panic!("{l}"),
        }
    }
    let secs = samples.len() as f32 / TTS_RATE as f32;
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    eprintln!("live tts: {chunks} chunks, {secs:.2} s of audio, first audio after {first:?}, peak {peak:.2}");
    assert!((1.5..8.0).contains(&secs), "{secs} s");
    assert!(peak > 0.05 && peak <= 1.0, "peak {peak}");
}
