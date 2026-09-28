//! voice.rs: no microphone, no network — fake ports.

use super::fakes::*;
use super::*;
use std::sync::Mutex;

fn voice(rec: &FakeRecorder, tr: &FakeTranscriber) -> Voice {
    Voice::new(true, Box::new(rec.clone()), Box::new(tr.clone()))
}

fn key() -> Option<String> {
    Some("sk-test".into())
}

// ---- keys ----

#[test]
fn ctrl_r_starts_when_enabled_and_hints_when_off() {
    let ctrl = KeyModifiers::CONTROL;
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('r'), ctrl), KeyAction::Start);
    assert_eq!(key_action(VoiceState::Idle, false, KeyCode::Char('r'), ctrl), KeyAction::OffHint);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('r'), KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('c'), ctrl), KeyAction::Pass);
}

#[test]
fn while_recording_any_key_stops_and_ctrl_c_or_esc_cancel() {
    let r = VoiceState::Recording;
    assert_eq!(key_action(r, true, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Enter, KeyModifiers::NONE), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Char('r'), KeyModifiers::CONTROL), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Char('c'), KeyModifiers::CONTROL), KeyAction::Cancel);
    assert_eq!(key_action(r, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Cancel);
    let f = VoiceState::Flushing;
    assert_eq!(key_action(f, true, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Swallow);
    assert_eq!(key_action(f, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Cancel);
    // voice mode switched off mid-recording: the keys still end it
    assert_eq!(key_action(r, false, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Stop);
}

// ---- audio ----

#[test]
fn mono_averages_the_channels() {
    assert_eq!(to_mono(&[0.2, 0.4, -1.0, 1.0], 2), vec![0.3f32, 0.0]);
    assert_eq!(to_mono(&[0.1, 0.2], 1), vec![0.1, 0.2]);
}

#[test]
fn resampler_48k_to_16k_keeps_one_sample_in_three_across_blocks() {
    let mut r = Resampler::new(48_000, 16_000);
    let input: Vec<f32> = (0..960).map(|i| (i as f32) / 1000.0).collect();
    let mut out = Vec::new();
    // odd block sizes: the phase carries over
    for block in input.chunks(97) {
        out.extend(r.process(block));
    }
    assert_eq!(out.len(), 320);
    for (k, s) in out.iter().enumerate() {
        assert_eq!(*s, to_i16(input[k * 3]), "sample {}", k);
    }
}

#[test]
fn resampler_upsamples_by_interpolating() {
    let mut r = Resampler::new(8_000, 16_000);
    let out = r.process(&[0.0, 0.5]);
    assert_eq!(out, vec![0, to_i16(0.25), to_i16(0.5)]);
    // the next block interpolates from the previous last sample
    let out = r.process(&[1.0]);
    assert_eq!(out, vec![to_i16(0.75), to_i16(1.0)]);
}

#[test]
fn resampler_44100_output_count_is_stable() {
    let mut r = Resampler::new(44_100, 16_000);
    let n: usize = (0..100).map(|_| r.process(&[0.0; 441]).len()).sum();
    assert!((15_999..=16_001).contains(&n), "{}", n);
}

#[test]
fn samples_clip_and_peak_is_normalized() {
    assert_eq!(to_i16(2.0), i16::MAX);
    assert_eq!(to_i16(-2.0), -i16::MAX);
    assert_eq!(peak(&[0, -16384, 100]), 16384.0 / 32767.0);
    assert_eq!(peak(&[i16::MIN]), 1.0);
    assert_eq!(peak(&[]), 0.0);
    assert_eq!(peak_glyph(0.0), '▁');
    assert_eq!(peak_glyph(1.0), '█');
    assert_eq!(peak_glyph(0.5), '▅');
    assert_eq!(flush_glyph(0), '▏');
    assert_eq!(flush_glyph(250), '▍');
    assert_eq!(flush_glyph(800), '▏');
}

// ---- protocol ----

#[test]
fn server_events_parse() {
    assert_eq!(
        parse_server_event(r#"{"type":"session.created","session":{"request_id":"r"}}"#),
        Some(TranscribeEvent::SessionCreated)
    );
    assert_eq!(
        parse_server_event(r#"{"type":"transcription.text.delta","text":" hello"}"#),
        Some(TranscribeEvent::Delta(" hello".into()))
    );
    assert_eq!(parse_server_event(r#"{"type":"transcription.done","text":"x"}"#), Some(TranscribeEvent::Done));
    assert_eq!(
        parse_server_event(r#"{"type":"error","error":{"message":"bad key","code":401}}"#),
        Some(TranscribeEvent::Error("bad key".into()))
    );
    // a message object is shown as JSON, not dropped
    assert_eq!(
        parse_server_event(r#"{"type":"error","error":{"message":{"detail":"x"}}}"#),
        Some(TranscribeEvent::Error(r#"{"detail":"x"}"#.into()))
    );
    // the empty-recording error is a normal end
    assert_eq!(
        parse_server_event(r#"{"type":"error","error":{"message":"flush before sending any audio bytes"}}"#),
        Some(TranscribeEvent::Done)
    );
    assert_eq!(parse_server_event(r#"{"type":"transcription.language","audio_language":"en"}"#), None);
    assert_eq!(parse_server_event(r#"{"type":"session.updated"}"#), None);
    assert_eq!(parse_server_event("not json"), None);
}

#[test]
fn client_messages_match_the_sdk() {
    let v: Value = serde_json::from_str(&session_update_message(16_000, 500)).unwrap();
    assert_eq!(v["type"], "session.update");
    assert_eq!(v["session"]["audio_format"]["encoding"], "pcm_s16le");
    assert_eq!(v["session"]["audio_format"]["sample_rate"], 16_000);
    assert_eq!(v["session"]["target_streaming_delay_ms"], 500);
    let v: Value = serde_json::from_str(&append_message(&[1, -2])).unwrap();
    assert_eq!(v["type"], "input_audio.append");
    // little-endian s16: 01 00 fe ff
    assert_eq!(v["audio"], "AQD+/w==");
    assert_eq!(flush_message(), r#"{"type":"input_audio.flush"}"#);
    assert_eq!(end_message(), r#"{"type":"input_audio.end"}"#);
    assert_eq!(
        realtime_url("wss://api.mistral.ai/", "voxtral-mini-transcribe-realtime-2602"),
        "wss://api.mistral.ai/v1/audio/transcriptions/realtime?model=voxtral-mini-transcribe-realtime-2602"
    );
}

// ---- settings ----

#[test]
fn voice_mode_is_off_by_default_and_the_env_overrides() {
    assert!(!voice_enabled_from(None, None));
    assert!(!voice_enabled_from(Some("garbage"), None));
    assert!(voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), None));
    assert!(!voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), Some("0")));
    assert!(voice_enabled_from(None, Some("1")));
    assert!(voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), Some("")));
}

#[test]
fn saving_keeps_the_other_settings() {
    let t = with_voice_enabled(Some(r#"{"theme": "dark"}"#), true);
    let v: Value = serde_json::from_str(&t).unwrap();
    assert_eq!(v["theme"], "dark");
    assert_eq!(v["voice_mode_enabled"], true);
    let v: Value = serde_json::from_str(&with_voice_enabled(Some("[1]"), false)).unwrap();
    assert_eq!(v, json!({"voice_mode_enabled": false}));
}

#[test]
fn env_file_values() {
    let t = "# c\nexport MISTRAL_API_KEY=\"abc\"\nOTHER=1\n";
    assert_eq!(env_file_value(t, "MISTRAL_API_KEY"), Some("abc".into()));
    assert_eq!(env_file_value("MISTRAL_API_KEY=\n", "MISTRAL_API_KEY"), None);
    assert_eq!(env_file_value(t, "MISSING"), None);
}

// ---- the controller ----

#[test]
fn start_needs_an_api_key() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let e = v.start(None, Instant::now()).unwrap_err();
    assert_eq!(e, "voice transcription needs an API key: set MISTRAL_API_KEY");
    assert_eq!(v.start(Some("  ".into()), Instant::now()).unwrap_err(), e);
    assert_eq!(v.state(), VoiceState::Idle);
}

#[test]
fn start_errors_say_what_to_do() {
    let tr = FakeTranscriber::default();
    let mut rec = FakeRecorder::ok(true);
    rec.result = Err(StartError::NoInputDevice);
    let e = voice(&rec, &tr).start(key(), Instant::now()).unwrap_err();
    assert!(e.starts_with("no audio input device found."), "{}", e);
    rec.result = Err(StartError::Backend("boom".into()));
    let e = voice(&rec, &tr).start(key(), Instant::now()).unwrap_err();
    assert_eq!(e, "audio backend is unavailable: boom");
}

#[test]
fn deltas_are_inserted_live_then_stop_flushes_and_done_ends() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    assert_eq!(v.state(), VoiceState::Recording);
    assert_eq!(v.peak(), 0.5);
    assert_eq!(tr.session.lock().unwrap().as_ref().unwrap().3, "sk-test");
    tr.send(TranscribeEvent::SessionCreated);
    tr.send(TranscribeEvent::Delta("Hello".into()));
    tr.send(TranscribeEvent::Delta(" world".into()));
    assert_eq!(
        v.poll(t0),
        vec![VoiceOutput::Insert("Hello".into()), VoiceOutput::Insert(" world".into())]
    );
    assert_eq!(v.state(), VoiceState::Recording);
    let t1 = t0 + Duration::from_secs(2);
    v.stop(t1);
    assert_eq!(v.state(), VoiceState::Flushing);
    assert!(*rec.stopped.borrow(), "the microphone is released at stop");
    assert_eq!(v.flushing_since(), Some(t1));
    assert_eq!(tr.audio(), vec![AudioMsg::End]);
    tr.send(TranscribeEvent::Delta("!".into()));
    tr.send(TranscribeEvent::Done);
    assert_eq!(v.poll(t1), vec![VoiceOutput::Insert("!".into()), VoiceOutput::Utterance]);
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(v.poll(t1).is_empty());
}

#[test]
fn cancel_releases_everything_and_keeps_quiet() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    v.cancel();
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(*rec.stopped.borrow());
    assert!(tr.cancelled());
    // the cancelled session's events go nowhere
    let s = tr.session.lock().unwrap();
    assert!(s.as_ref().unwrap().1.send(TranscribeEvent::Delta("late".into())).is_err());
    drop(s);
    assert!(v.poll(Instant::now()).is_empty());
}

#[test]
fn a_server_error_stops_the_recording() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    tr.send(TranscribeEvent::Error("HTTP 401 Unauthorized".into()));
    assert_eq!(
        v.poll(Instant::now()),
        vec![VoiceOutput::Error("voice transcription failed: HTTP 401 Unauthorized".into())]
    );
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(*rec.stopped.borrow());
    assert!(tr.cancelled());
}

#[test]
fn no_text_and_silence_blames_the_microphone() {
    let (rec, tr) = (FakeRecorder::ok(false), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    v.stop(t0 + Duration::from_millis(800));
    tr.send(TranscribeEvent::Done);
    let out = v.poll(t0 + Duration::from_millis(900));
    assert_eq!(out.len(), 1);
    match &out[0] {
        VoiceOutput::Error(m) => assert!(
            m.starts_with("voice transcription failed: no audio detected from the microphone"),
            "{}",
            m
        ),
        other => panic!("{:?}", other),
    }
}

#[test]
fn no_text_but_a_signal_or_a_short_press_is_no_speech() {
    for (signal, ms) in [(true, 2000), (false, 200)] {
        let (rec, tr) = (FakeRecorder::ok(signal), FakeTranscriber::default());
        let mut v = voice(&rec, &tr);
        let t0 = Instant::now();
        v.start(key(), t0).unwrap();
        v.stop(t0 + Duration::from_millis(ms));
        tr.close();
        assert_eq!(
            v.poll(t0 + Duration::from_millis(ms + 10)),
            vec![VoiceOutput::Notice("no speech detected".into())]
        );
        assert_eq!(v.state(), VoiceState::Idle);
    }
}

#[test]
fn the_flush_times_out_after_ten_seconds() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    v.stop(t0);
    assert!(v.poll(t0 + Duration::from_secs(9)).is_empty());
    assert_eq!(
        v.poll(t0 + Duration::from_secs(10)),
        vec![VoiceOutput::Error("voice transcription failed: the transcription timed out".into())]
    );
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(tr.cancelled());
}

#[test]
fn a_flush_timeout_after_text_keeps_the_text_without_a_failure() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    tr.send(TranscribeEvent::Delta("Hello".into()));
    assert_eq!(v.poll(t0), vec![VoiceOutput::Insert("Hello".into())]);
    v.stop(t0);
    assert_eq!(
        v.poll(t0 + Duration::from_secs(10)),
        vec![VoiceOutput::Utterance, VoiceOutput::Notice(LATE_DONE_NOTICE.into())]
    );
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(tr.cancelled());
}

// ---- the session loop (fake socket) ----

fn run_fake(socket: &mut FakeSocket, msgs: Vec<AudioMsg>) -> (Result<(), String>, Vec<TranscribeEvent>) {
    let (atx, arx) = mpsc::channel();
    // every message queued before the loop runs: End sits right behind
    // the chunks, and the sender stays alive (as in Voice's Run)
    for m in msgs {
        atx.send(m).unwrap();
    }
    let (etx, erx) = mpsc::channel();
    let r = stream_session(socket, &arx, &etx, &AtomicBool::new(false));
    drop(atx);
    (r, erx.try_iter().collect())
}

fn done_json() -> Value {
    json!({"type": "transcription.done", "text": "hi there"})
}

#[test]
fn end_queued_behind_chunks_still_flushes_and_ends_once() {
    let mut ws = FakeSocket::default();
    ws.after_end.push_back(server_text(json!({"type": "transcription.text.delta", "text": " there"})));
    ws.after_end.push_back(server_text(done_json()));
    let (r, events) = run_fake(
        &mut ws,
        vec![AudioMsg::Chunk(vec![1; 100]), AudioMsg::Chunk(vec![2; 100]), AudioMsg::End],
    );
    assert_eq!(r, Ok(()));
    assert_eq!(
        ws.sent_types(),
        ["session.update", "input_audio.append", "input_audio.flush", "input_audio.end"]
    );
    assert_eq!(events, vec![TranscribeEvent::Delta(" there".into()), TranscribeEvent::Done]);
    assert!(ws.closed);
}

#[test]
fn flush_and_end_are_sent_once_while_waiting_for_done() {
    let mut ws = FakeSocket::default();
    // the server takes a while: many idle reads before done
    for _ in 0..20 {
        ws.after_end.push_back(SocketRead::Idle);
    }
    ws.after_end.push_back(server_text(done_json()));
    let (r, events) = run_fake(&mut ws, vec![AudioMsg::Chunk(vec![1; SEND_BLOCK + 10]), AudioMsg::End]);
    assert_eq!(r, Ok(()));
    let types = ws.sent_types();
    assert_eq!(types.iter().filter(|t| *t == "input_audio.flush").count(), 1);
    assert_eq!(types.iter().filter(|t| *t == "input_audio.end").count(), 1);
    // the whole block then the tail, all before the flush
    assert_eq!(
        types,
        ["session.update", "input_audio.append", "input_audio.flush", "input_audio.end"]
    );
    assert_eq!(events, vec![TranscribeEvent::Done]);
}

#[test]
fn a_closed_socket_after_the_end_is_done_before_it_a_failure() {
    let mut ws = FakeSocket::default();
    ws.after_end.push_back(SocketRead::Closed);
    let (r, events) = run_fake(&mut ws, vec![AudioMsg::End]);
    assert_eq!(r, Ok(()));
    assert_eq!(events, vec![TranscribeEvent::Done]);

    let mut ws = FakeSocket::default();
    ws.script.push_back(SocketRead::Closed);
    let (r, _) = run_fake(&mut ws, vec![AudioMsg::Chunk(vec![1; 10])]);
    assert_eq!(r, Err("the connection closed before the recording finished".into()));
}

#[test]
fn a_dropped_audio_sender_ends_the_stream() {
    let mut ws = FakeSocket::default();
    ws.after_end.push_back(server_text(done_json()));
    let (atx, arx) = mpsc::channel();
    atx.send(AudioMsg::Chunk(vec![3; 10])).unwrap();
    drop(atx);
    let (etx, erx) = mpsc::channel();
    assert_eq!(stream_session(&mut ws, &arx, &etx, &AtomicBool::new(false)), Ok(()));
    assert_eq!(
        ws.sent_types(),
        ["session.update", "input_audio.append", "input_audio.flush", "input_audio.end"]
    );
    assert_eq!(erx.try_iter().collect::<Vec<_>>(), vec![TranscribeEvent::Done]);
}

#[test]
fn a_server_error_fails_and_cancel_closes() {
    let mut ws = FakeSocket::default();
    ws.after_end.push_back(server_text(json!({"type": "error", "error": {"message": "boom"}})));
    let (r, _) = run_fake(&mut ws, vec![AudioMsg::Chunk(vec![1; 10]), AudioMsg::End]);
    assert_eq!(r, Err("boom".into()));

    let mut ws = FakeSocket::default();
    let (_atx, arx) = mpsc::channel::<AudioMsg>();
    let (etx, _erx) = mpsc::channel();
    assert_eq!(stream_session(&mut ws, &arx, &etx, &AtomicBool::new(true)), Ok(()));
    assert!(ws.closed);
    assert_eq!(ws.sent_types(), ["session.update"]);
}

#[test]
fn recording_stops_itself_after_five_minutes() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    assert!(v.poll(t0 + Duration::from_secs(299)).is_empty());
    assert_eq!(v.state(), VoiceState::Recording);
    v.poll(t0 + Duration::from_secs(300));
    assert_eq!(v.state(), VoiceState::Flushing);
    assert_eq!(tr.audio(), vec![AudioMsg::End]);
}

#[test]
fn start_while_active_is_a_no_op() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    assert_eq!(v.start(None, Instant::now()), Ok(()));
    assert_eq!(v.state(), VoiceState::Recording);
}

/// The real API, by hand only (network + key): a 16 kHz mono s16 WAV
/// (`say -o x.aiff "…" && afconvert -f WAVE -d LEI16@16000 -c 1 x.aiff x.wav`)
/// streamed in realtime blocks. `SB_STT_WAV=x.wav cargo test -p bend-tui
/// real_api -- --ignored --nocapture`
fn wav_samples() -> Vec<i16> {
    let path = std::env::var("SB_STT_WAV").expect("SB_STT_WAV");
    let bytes = std::fs::read(path).unwrap();
    // skip the RIFF header: the "data" chunk
    let data = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
    bytes[data..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()
}

/// The real socket, recording the type of each message sent.
struct Traced {
    ws: WsSocket,
    sent: Arc<Mutex<Vec<String>>>,
}

impl RealtimeSocket for Traced {
    fn send_text(&mut self, text: String) -> Result<(), String> {
        let v: Value = serde_json::from_str(&text).unwrap();
        self.sent.lock().unwrap().push(v["type"].as_str().unwrap().to_string());
        self.ws.send_text(text)
    }
    fn read(&mut self, wait: Duration) -> Result<SocketRead, String> {
        self.ws.read(wait)
    }
    fn close(&mut self) {
        self.ws.close()
    }
}

/// Streams the WAV at realtime pace; `burst_tail` blocks at the end go
/// at once with End right behind them (as when the microphone stops
/// with audio still queued). Returns the text, the time from End to
/// done, and the message types sent.
fn stream_wav(burst_tail: usize) -> (String, Duration, Vec<String>) {
    let samples = wav_samples();
    let (atx, arx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let url = realtime_url(API_BASE, MODEL);
    let key = resolve_api_key().expect("key");
    let trace = sent.clone();
    std::thread::spawn(move || {
        let r = WsSocket::connect(&url, &key).and_then(|ws| {
            let mut t = Traced { ws, sent: trace };
            stream_session(&mut t, &arx, &etx, &AtomicBool::new(false))
        });
        if let Err(e) = r {
            let _ = etx.send(TranscribeEvent::Error(e));
        }
    });
    let mut text = String::new();
    let blocks: Vec<&[i16]> = samples.chunks(320).collect();
    let n = blocks.len();
    for (i, block) in blocks.into_iter().enumerate() {
        atx.send(AudioMsg::Chunk(block.to_vec())).unwrap();
        if i + burst_tail < n {
            std::thread::sleep(Duration::from_millis(20));
        }
        while let Ok(ev) = erx.try_recv() {
            if let TranscribeEvent::Delta(t) = ev {
                text.push_str(&t);
            }
        }
    }
    // the sender stays alive, as in Voice's Run: only End ends the stream
    atx.send(AudioMsg::End).unwrap();
    let stopped = Instant::now();
    loop {
        match erx.recv_timeout(FLUSH_TIMEOUT) {
            Ok(TranscribeEvent::Delta(t)) => text.push_str(&t),
            Ok(TranscribeEvent::Done) => break,
            Ok(TranscribeEvent::Error(e)) => panic!("error: {}", e),
            Ok(TranscribeEvent::SessionCreated) => {}
            Err(e) => panic!("no transcription.done within {:?}: {:?}", FLUSH_TIMEOUT, e),
        }
    }
    let end_latency = stopped.elapsed();
    drop(atx);
    let sent = sent.lock().unwrap().clone();
    eprintln!("end → done {:?} · text {:?}", end_latency, text);
    (text, end_latency, sent)
}

fn count(sent: &[String], ty: &str) -> usize {
    sent.iter().filter(|t| *t == ty).count()
}

#[test]
#[ignore]
fn real_api_transcribes_a_wav() {
    let (text, latency, sent) = stream_wav(0);
    assert!(!text.trim().is_empty());
    assert!(latency < Duration::from_secs(2), "done took {:?}", latency);
    assert_eq!((count(&sent, "input_audio.flush"), count(&sent, "input_audio.end")), (1, 1));
}

/// The bug: End queued right behind chunks was dropped, flush/end never
/// went out, done never came, and the flush timed out after 10 s.
#[test]
#[ignore]
fn real_api_done_arrives_quickly_when_end_follows_queued_audio() {
    let (text, latency, sent) = stream_wav(10);
    assert!(!text.trim().is_empty());
    assert!(latency < Duration::from_secs(2), "done took {:?}", latency);
    assert_eq!((count(&sent, "input_audio.flush"), count(&sent, "input_audio.end")), (1, 1));
    assert_eq!(sent.last().map(String::as_str), Some("input_audio.end"));
}
