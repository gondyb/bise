//! audio.rs's tests: the speaker over a fake output callback (no
//! device, no sound), the mic's blocks, the WAV reader, the fixtures.

use super::*;
use crate::voicemode::{STOP_WITHIN, TTS_RATE};
use std::sync::mpsc;

/// the queue's tests: a device at the TTS's rate (no resampling, exact
/// sample counts); the resampling has its own test
const RATE: u32 = TTS_RATE;
/// a device buffer: 10 ms
const BUF: usize = 240;

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// `n` ms of a constant at `rate`.
fn tone(ms: usize, rate: u32, x: f32) -> Vec<f32> {
    vec![x; ms * rate as usize / 1000]
}

/// The fake device: pulls `bufs` buffers, returns what it got.
fn pull(p: &Arc<Mutex<Playback>>, bufs: usize) -> Vec<f32> {
    let mut all = Vec::new();
    for _ in 0..bufs {
        let mut b = vec![9.0f32; BUF];
        p.lock().unwrap().fill(&mut b);
        all.extend(b);
    }
    all
}

fn speaker() -> (QueueSpeaker, Arc<Mutex<Playback>>) {
    let play = Arc::new(Mutex::new(Playback::new(RATE)));
    (QueueSpeaker::new(play.clone(), None), play)
}

#[test]
fn utterances_play_back_to_back_in_push_order() {
    let (mut s, p) = speaker();
    s.push(1, &tone(30, TTS_RATE, 0.5));
    s.push(2, &tone(20, TTS_RATE, -0.25));
    s.end(1);
    s.end(2);
    // 30 ms of 1, then 20 ms of 2, then silence
    let out = pull(&p, 6);
    assert!(out[..720].iter().all(|&x| (x - 0.5).abs() < 1e-6), "utt 1");
    assert!(out[720..1200].iter().all(|&x| (x + 0.25).abs() < 1e-6), "utt 2");
    assert!(out[1200..].iter().all(|&x| x == 0.0), "silence after");
    assert!(s.done(1) && s.done(2));
    assert_eq!(s.clock(), None);
}

#[test]
fn the_clock_counts_the_samples_handed_to_the_device() {
    let (mut s, p) = speaker();
    assert_eq!(s.clock(), None);
    s.push(7, &tone(100, TTS_RATE, 0.1));
    assert_eq!(s.clock(), Some((7, Duration::ZERO)), "queued, nothing played");
    pull(&p, 3);
    let (id, at) = s.clock().unwrap();
    assert_eq!((id, ms(at)), (7, 30));
    s.end(7);
    pull(&p, 7);
    // its last sample went out: done, the clock moves on
    assert!(s.done(7));
    assert_eq!(s.clock(), None);
}

#[test]
fn an_utterance_waiting_for_its_audio_holds_the_next_one_back() {
    let (mut s, p) = speaker();
    s.push(1, &tone(10, TTS_RATE, 0.5));
    s.push(2, &tone(10, TTS_RATE, 0.3));
    s.end(2);
    // 1 is not ended: after its 10 ms, silence, and 2 waits
    let out = pull(&p, 3);
    assert!(out[BUF..].iter().all(|&x| x == 0.0));
    assert_eq!(s.clock().map(|(id, at)| (id, ms(at))), Some((1, 10)));
    assert!(!s.done(1) && !s.done(2));
    // the rest of 1 streams in, then it ends: 2 plays after it
    s.push(1, &tone(10, TTS_RATE, 0.5));
    s.end(1);
    let out = pull(&p, 2);
    assert!(out[..BUF].iter().all(|&x| (x - 0.5).abs() < 1e-6));
    assert!(out[BUF..].iter().all(|&x| (x - 0.3).abs() < 1e-6));
    assert!(s.done(1) && s.done(2));
}

#[test]
fn an_utterance_is_not_done_before_its_audio_comes() {
    let (mut s, p) = speaker();
    // the TTS has not answered yet: the controller asks
    assert!(!s.done(3));
    s.push(3, &[]);
    pull(&p, 2);
    assert!(!s.done(3), "no audio and not ended: it waits");
    // an utterance ended with no audio at all: done when its turn comes
    s.end(3);
    pull(&p, 1);
    assert!(s.done(3));
}

#[test]
fn stop_is_silent_within_50_ms_with_a_10_ms_fade() {
    let (mut s, p) = speaker();
    s.push(1, &tone(2000, TTS_RATE, 0.8));
    s.push(2, &tone(500, TTS_RATE, 0.8));
    pull(&p, 10);
    s.stop();
    // dropped at once
    assert!(s.done(1) && s.done(2));
    assert_eq!(s.clock(), None);
    let out = pull(&p, 6);
    let fade = (RATE / 100) as usize;
    // the fade: down from the voice to zero, monotonic, no click
    assert!(out[0] > 0.7 && out[0] <= 0.8, "{}", out[0]);
    assert!(out[..fade].windows(2).all(|w| w[1] <= w[0]));
    assert!(out[fade - 1].abs() < 0.01, "{}", out[fade - 1]);
    // then silence: well within STOP_WITHIN
    let silent_from = out.iter().rposition(|&x| x != 0.0).map_or(0, |i| i + 1);
    assert!(silent_from as u128 * 1000 / RATE as u128 <= ms(STOP_WITHIN));
    assert!(out[fade..].iter().all(|&x| x == 0.0));
    assert_eq!(s.level(), 0.0);
}

#[test]
fn late_audio_of_a_stopped_utterance_is_dropped_and_the_next_one_plays() {
    let (mut s, p) = speaker();
    s.push(1, &tone(100, TTS_RATE, 0.5));
    s.stop();
    // the synth's chunks still in the channel
    s.push(1, &tone(100, TTS_RATE, 0.5));
    s.end(1);
    pull(&p, 2);
    assert_eq!(s.clock(), None);
    // the next turn's sentence
    s.push(2, &tone(20, TTS_RATE, 0.2));
    s.end(2);
    let out = pull(&p, 3);
    assert!(out[..480].iter().all(|&x| (x - 0.2).abs() < 1e-6));
    assert!(s.done(2));
}

#[test]
fn a_stop_with_nothing_playing_does_nothing() {
    let (mut s, p) = speaker();
    s.stop();
    assert!(pull(&p, 1).iter().all(|&x| x == 0.0));
    assert!(p.lock().unwrap().idle());
}

#[test]
fn the_level_follows_the_output() {
    let (mut s, p) = speaker();
    assert_eq!(s.level(), 0.0);
    s.push(1, &tone(20, TTS_RATE, 0.5));
    s.end(1);
    pull(&p, 1);
    // half of full scale: -6 dBFS on the meter's -50 dB scale
    assert!((s.level() - loudness(0.5)).abs() < 1e-6 && s.level() > 0.85, "{}", s.level());
    pull(&p, 2);
    assert_eq!(s.level(), 0.0);
}

#[test]
fn the_tts_rate_is_resampled_to_the_devices() {
    // a 24 kHz sine at 440 Hz: 48 kHz out, twice the samples, the same tone
    let sine: Vec<f32> = (0..2400).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / TTS_RATE as f32).sin() * 0.5).collect();
    let p = Arc::new(Mutex::new(Playback::new(48_000)));
    let mut s = QueueSpeaker::new(p.clone(), None);
    s.push(1, &sine[..1200]);
    s.push(1, &sine[1200..]);
    s.end(1);
    let mut out = vec![0.0f32; 4800 + 480];
    p.lock().unwrap().fill(&mut out);
    let n = out.iter().rposition(|&x| x != 0.0).unwrap() + 1;
    assert!(n.abs_diff(4800) <= 2, "{} samples", n);
    assert!(s.done(1));
    for (i, &x) in out[..n].iter().enumerate().step_by(37) {
        let want = (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5;
        assert!((x - want).abs() < 0.01, "sample {}: {} vs {}", i, x, want);
    }
    // a device at the TTS's rate: as is
    let play = Arc::new(Mutex::new(Playback::new(TTS_RATE)));
    let mut s = QueueSpeaker::new(play.clone(), None);
    s.push(1, &sine);
    let mut b = vec![0.0; 2400];
    play.lock().unwrap().fill(&mut b);
    assert_eq!(b, sine);
}

#[test]
fn the_silent_speaker_runs_its_clock_in_real_time() {
    let mut s = silent_speaker();
    s.push(1, &tone(60, TTS_RATE, 0.5));
    s.end(1);
    let start = Instant::now();
    while !s.done(1) && start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(s.done(1));
    let took = start.elapsed();
    assert!(took >= Duration::from_millis(40), "{:?}", took);
}

#[test]
fn the_blocker_cuts_device_buffers_into_20_ms_blocks_with_their_time() {
    let t0 = Instant::now();
    let mut b = Blocker::default();
    // 10 ms buffers (160 samples): a block every second buffer
    let buf = |k: i16| vec![k; 160];
    assert!(b.push(&buf(1), t0).is_empty());
    let out = b.push(&buf(2), t0 + Duration::from_millis(10));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].pcm.len(), 320);
    assert_eq!(out[0].at, t0);
    assert_eq!((out[0].pcm[0], out[0].pcm[319]), (1, 2));
    // a 50 ms buffer: two blocks and a half; the times follow the samples
    let t1 = t0 + Duration::from_millis(20);
    let out = b.push(&vec![3; 800], t1);
    assert_eq!(out.iter().map(|m| m.at).collect::<Vec<_>>(), vec![t1, t1 + Duration::from_millis(20)]);
    let out = b.push(&buf(4), t1 + Duration::from_millis(50));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].at, t1 + Duration::from_millis(40));
    assert_eq!((out[0].pcm[0], out[0].pcm[319]), (3, 4));
}

#[test]
fn the_scripted_mic_sends_its_recording_in_blocks() {
    let pcm = fixture("ok");
    let (tx, rx) = mpsc::channel();
    let stream = ScriptedMic { pcm: pcm.clone(), pace: false }.open(tx).unwrap();
    let blocks: Vec<MicBlock> = rx.try_iter().collect();
    assert_eq!(blocks.iter().map(|b| b.pcm.len()).sum::<usize>(), pcm.len());
    assert!(blocks.iter().all(|b| b.pcm.len() <= 320));
    assert!(blocks.windows(2).all(|w| w[1].at - w[0].at == BLOCK));
    assert_eq!(blocks.concat_pcm(), pcm);
    // the last block's level
    assert_eq!(stream.level(), loudness(peak(&blocks.last().unwrap().pcm)));
}

#[test]
fn the_paced_scripted_mic_sends_silence_after_and_stops_when_dropped() {
    let (tx, rx) = mpsc::channel();
    let stream = ScriptedMic { pcm: vec![1000; 320 * 2], pace: true }.open(tx).unwrap();
    let first: Vec<MicBlock> = (0..4).map(|_| rx.recv_timeout(Duration::from_secs(1)).unwrap()).collect();
    assert_eq!(first[0].pcm, vec![1000; 320]);
    assert_eq!(first[3].pcm, vec![0; 320]);
    assert!(first[3].at - first[0].at >= Duration::from_millis(50));
    drop(stream);
    // the thread ends: the channel closes
    let end = Instant::now();
    while rx.recv_timeout(Duration::from_millis(100)).is_ok() {
        assert!(end.elapsed() < Duration::from_secs(1));
    }
}

trait ConcatPcm {
    fn concat_pcm(&self) -> Vec<i16>;
}

impl ConcatPcm for Vec<MicBlock> {
    fn concat_pcm(&self) -> Vec<i16> {
        self.iter().flat_map(|b| b.pcm.iter().copied()).collect()
    }
}

#[test]
fn read_wav_reads_the_fixtures_and_skips_other_chunks() {
    // voice.rs's writer, with a padding chunk before the data (afconvert's FLLR)
    let pcm: Vec<i16> = (0..100).map(|i| i * 3 - 150).collect();
    let plain = crate::voice::wav_bytes(&pcm, 16_000);
    assert_eq!(read_wav(&plain).unwrap(), (16_000, pcm.clone()));
    let mut padded = plain[..36].to_vec();
    padded.extend(b"FLLR");
    padded.extend(5u32.to_le_bytes());
    padded.extend([0u8; 6]); // 5 bytes + the pad byte
    padded.extend(&plain[36..]);
    assert_eq!(read_wav(&padded).unwrap(), (16_000, pcm));
    assert!(read_wav(b"nope").is_err());
    let stereo = {
        let mut w = plain.clone();
        w[22] = 2;
        w
    };
    assert!(read_wav(&stereo).unwrap_err().contains("mono"));
}

#[test]
fn the_fixtures_are_16_khz_mono_and_small() {
    for name in ["sentence", "sentence_room", "mm", "ok", "silence", "room", "keyboard"] {
        let pcm = fixture(name);
        assert!(!pcm.is_empty(), "{}", name);
        // < 150 KB each (plan §7)
        assert!(pcm.len() * 2 < 150_000, "{}: {} bytes", name, pcm.len() * 2);
    }
    assert!(peak(&fixture("silence")) == 0.0);
    assert!(peak(&fixture("sentence")) > 0.1);
}
