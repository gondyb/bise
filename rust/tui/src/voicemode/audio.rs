//! The mic and the speaker of voice mode (owner: voice-audio; plan §2,
//! §4.1, §4.2).
//!
//! In: [`CpalMic`], the default input open for the whole voice mode
//! (voice.rs's cpal stream and resampler), cut into [`BLOCK`]s of
//! [`MIC_RATE`] PCM with their capture time ([`Blocker`]); [`ScriptedMic`]
//! sends a recording instead (the fakes, `BISE_VOICE_FAKE`).
//!
//! Out: [`Playback`] is the speaker without a device (pure): the queue
//! of utterances, the clock from the samples handed to the device, the
//! fade of a stop, the level. [`QueueSpeaker`] is the [`Speaker`] over
//! it (resampling [`TTS_RATE`] to the device's rate); its device is
//! cpal ([`open_speaker`]) or a thread that pulls the samples in real
//! time and plays nothing ([`silent_speaker`]: the tests, the e2e).
//!
//! The recorded fixtures (testdata/, made by testdata/make.sh) are read
//! by [`read_wav`]; the tests get them with [`fixture`].

use super::{Mic, MicBlock, MicStream, Speaker, UttId, MIC_RATE, TTS_RATE};
use crate::voice::{loudness, peak, Resampler};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The mic's blocks (plan §4.1: 20-100 ms).
pub const BLOCK: Duration = Duration::from_millis(20);
const BLOCK_LEN: usize = (MIC_RATE as usize) * 20 / 1000;
/// A stop fades the voice out over this (no click), then it is silent.
pub const FADE: Duration = Duration::from_millis(10);

fn samples_len(n: usize, rate: u32) -> Duration {
    Duration::from_secs_f64(n as f64 / rate.max(1) as f64)
}

/// A level shared with the audio thread (f32 bits).
#[derive(Default)]
pub struct SharedLevel(AtomicU32);

impl SharedLevel {
    pub fn set(&self, x: f32) {
        self.0.store(x.to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

// ---- in ----

/// Cuts the device's buffers (any size; ~10 ms on macOS) into [`BLOCK`]s,
/// each with the capture time of its first sample.
#[derive(Debug)]
pub struct Blocker {
    len: usize,
    buf: Vec<i16>,
    at: Option<Instant>,
}

impl Default for Blocker {
    fn default() -> Self {
        Blocker::new(BLOCK_LEN)
    }
}

impl Blocker {
    pub fn new(len: usize) -> Blocker {
        Blocker { len: len.max(1), buf: Vec::with_capacity(len), at: None }
    }

    /// `pcm` captured from `at` (its first sample): the whole blocks it
    /// completes; the rest waits for the next buffer.
    pub fn push(&mut self, pcm: &[i16], at: Instant) -> Vec<MicBlock> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < pcm.len() {
            if self.buf.is_empty() {
                self.at = Some(at + samples_len(i, MIC_RATE));
            }
            let take = (self.len - self.buf.len()).min(pcm.len() - i);
            self.buf.extend_from_slice(&pcm[i..i + take]);
            i += take;
            if self.buf.len() == self.len {
                let pcm = std::mem::replace(&mut self.buf, Vec::with_capacity(self.len));
                out.push(MicBlock { pcm, at: self.at.take().unwrap_or(at) });
            }
        }
        out
    }
}

/// The default input device, open for the whole voice mode. Never
/// opened by a test.
#[derive(Default)]
pub struct CpalMic;

struct CpalMicStream {
    #[cfg(not(target_os = "linux"))]
    _stream: cpal::Stream,
    level: Arc<SharedLevel>,
}

impl MicStream for CpalMicStream {
    fn level(&self) -> f32 {
        self.level.get()
    }
}

#[cfg(not(target_os = "linux"))]
impl Mic for CpalMic {
    fn open(&mut self, blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        let level = Arc::new(SharedLevel::default());
        let l = level.clone();
        let mut blocker = Blocker::default();
        let stream = crate::voice::open_input(MIC_RATE, move |pcm| {
            // the buffer was captured just before its callback
            let now = Instant::now();
            let at = now.checked_sub(samples_len(pcm.len(), MIC_RATE)).unwrap_or(now);
            for b in blocker.push(&pcm, at) {
                l.set(loudness(peak(&b.pcm)));
                let _ = blocks.send(b);
            }
        })
        .map_err(crate::voice::start_error_line)?;
        Ok(Box::new(CpalMicStream { _stream: stream, level }))
    }
}

#[cfg(target_os = "linux")]
impl Mic for CpalMic {
    fn open(&mut self, _blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        let _ = CpalMicStream { level: Arc::default() };
        Err("voice mode is not available on Linux yet".into())
    }
}

/// A mic that says a recording: its blocks, then silence until the
/// stream is dropped. `pace`: one block every [`BLOCK`] with the capture
/// time now (the e2e's mic); else every block at once, no silence after
/// (the controller's tests).
#[derive(Clone, Debug, Default)]
pub struct ScriptedMic {
    pub pcm: Vec<i16>,
    pub pace: bool,
}

impl ScriptedMic {
    /// A 16 kHz mono 16-bit WAV file (testdata/), paced.
    pub fn from_wav_file(path: &std::path::Path) -> Result<ScriptedMic, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let (rate, pcm) = read_wav(&bytes)?;
        if rate != MIC_RATE {
            return Err(format!("{}: {} Hz, the mic is {} Hz", path.display(), rate, MIC_RATE));
        }
        Ok(ScriptedMic { pcm, pace: true })
    }
}

struct ScriptedStream {
    stop: Arc<AtomicBool>,
    level: Arc<SharedLevel>,
}

impl Drop for ScriptedStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl MicStream for ScriptedStream {
    fn level(&self) -> f32 {
        self.level.get()
    }
}

impl Mic for ScriptedMic {
    fn open(&mut self, blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let level = Arc::new(SharedLevel::default());
        let send = {
            let level = level.clone();
            move |pcm: Vec<i16>, at: Instant| {
                level.set(loudness(peak(&pcm)));
                blocks.send(MicBlock { pcm, at }).is_ok()
            }
        };
        if !self.pace {
            let start = Instant::now();
            for (i, b) in self.pcm.chunks(BLOCK_LEN).enumerate() {
                send(b.to_vec(), start + BLOCK * i as u32);
            }
            return Ok(Box::new(ScriptedStream { stop, level }));
        }
        let (pcm, s) = (self.pcm.clone(), stop.clone());
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut script = pcm.chunks(BLOCK_LEN);
            for i in 0u32.. {
                if s.load(Ordering::SeqCst) {
                    return;
                }
                let block = script.next().map(|b| b.to_vec()).unwrap_or_else(|| vec![0; BLOCK_LEN]);
                if !send(block, Instant::now()) {
                    return;
                }
                let next = start + BLOCK * (i + 1);
                std::thread::sleep(next.saturating_duration_since(Instant::now()));
            }
        });
        Ok(Box::new(ScriptedStream { stop, level }))
    }
}

// ---- out ----

#[derive(Debug)]
struct Utt {
    id: UttId,
    /// at the device's rate, not played yet
    pcm: VecDeque<f32>,
    /// all its audio is in
    ended: bool,
    /// samples handed to the device
    played: u64,
}

/// The speaker without its device: the device's callback calls
/// [`Playback::fill`]; the controller's calls (through [`QueueSpeaker`])
/// push, end, stop and read the clock. Mono, at the device's rate.
#[derive(Debug)]
pub struct Playback {
    rate: u32,
    queue: VecDeque<Utt>,
    /// a stop's fade-out, played before anything else
    fade: VecDeque<f32>,
    /// ended (last sample handed to the device) or dropped by a stop
    closed: HashSet<UttId>,
    level: f32,
}

impl Playback {
    pub fn new(rate: u32) -> Playback {
        Playback { rate: rate.max(1), queue: VecDeque::new(), fade: VecDeque::new(), closed: HashSet::new(), level: 0.0 }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    fn utt(&mut self, id: UttId) -> Option<&mut Utt> {
        if self.closed.contains(&id) {
            return None;
        }
        if let Some(i) = self.queue.iter().position(|u| u.id == id) {
            return self.queue.get_mut(i);
        }
        self.queue.push_back(Utt { id, pcm: VecDeque::new(), ended: false, played: 0 });
        self.queue.back_mut()
    }

    /// More audio of `id`, at the device's rate; a new id queues last.
    /// Audio of a closed utterance (a late chunk after a stop) is dropped.
    pub fn push(&mut self, id: UttId, pcm: &[f32]) {
        if let Some(u) = self.utt(id) {
            u.pcm.extend(pcm.iter().copied());
        }
    }

    /// `id` has all its audio (an id never pushed: an empty utterance,
    /// done when its turn comes).
    pub fn end(&mut self, id: UttId) {
        if let Some(u) = self.utt(id) {
            u.ended = true;
        }
    }

    /// Cut: the next [`FADE`] of the playing utterance fades to zero,
    /// then silence; every utterance is dropped (done) at once.
    pub fn stop(&mut self) {
        let n = (self.rate as usize * FADE.as_millis() as usize) / 1000;
        if let Some(head) = self.queue.front_mut() {
            let mut fade: VecDeque<f32> = head.pcm.drain(..n.min(head.pcm.len())).collect();
            // the fade-out of an earlier stop still plays first
            let base = self.fade.len();
            for (i, x) in fade.iter_mut().enumerate() {
                *x *= 1.0 - (base + i + 1) as f32 / (base + n) as f32;
            }
            self.fade.extend(fade);
        }
        self.closed.extend(self.queue.drain(..).map(|u| u.id));
    }

    pub fn clock(&self) -> Option<(UttId, Duration)> {
        self.queue.front().map(|u| (u.id, samples_len(u.played as usize, self.rate)))
    }

    pub fn done(&self, id: UttId) -> bool {
        self.closed.contains(&id)
    }

    /// The last buffer's loudness, 0..1.
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Nothing queued and no fade left.
    pub fn idle(&self) -> bool {
        self.queue.is_empty() && self.fade.is_empty()
    }

    /// The device wants `out.len()` samples: the fade of a stop, then
    /// the utterances in order; silence where the playing one waits for
    /// its audio (the next never jumps ahead of it).
    pub fn fill(&mut self, out: &mut [f32]) {
        let mut top = 0.0f32;
        for slot in out.iter_mut() {
            *slot = self.next_sample();
            top = top.max(slot.abs());
        }
        self.close_ended();
        self.level = loudness(top);
    }

    fn next_sample(&mut self) -> f32 {
        if let Some(x) = self.fade.pop_front() {
            return x;
        }
        loop {
            let Some(head) = self.queue.front_mut() else { return 0.0 };
            if let Some(x) = head.pcm.pop_front() {
                head.played += 1;
                return x;
            }
            if !head.ended {
                return 0.0;
            }
            self.close_head();
        }
    }

    fn close_ended(&mut self) {
        while self.queue.front().is_some_and(|u| u.ended && u.pcm.is_empty()) {
            self.close_head();
        }
    }

    fn close_head(&mut self) {
        if let Some(u) = self.queue.pop_front() {
            self.closed.insert(u.id);
        }
    }
}

/// The [`Speaker`] over a [`Playback`] that a device pulls from:
/// resamples [`TTS_RATE`] to the device's rate, one resampler per
/// utterance. Dropping it closes the device.
pub struct QueueSpeaker {
    play: Arc<Mutex<Playback>>,
    resampler: Option<(UttId, Resampler)>,
    _device: Option<Box<dyn std::any::Any>>,
}

impl QueueSpeaker {
    /// `play`'s device is `device` (kept until the speaker is dropped);
    /// None: the caller pulls ([`Playback::fill`]) itself (the tests).
    pub fn new(play: Arc<Mutex<Playback>>, device: Option<Box<dyn std::any::Any>>) -> QueueSpeaker {
        QueueSpeaker { play, resampler: None, _device: device }
    }

    fn with<R>(&self, f: impl FnOnce(&mut Playback) -> R) -> Option<R> {
        self.play.lock().ok().map(|mut p| f(&mut p))
    }
}

impl Speaker for QueueSpeaker {
    fn push(&mut self, utt: UttId, pcm: &[f32]) {
        let Some(rate) = self.with(|p| p.rate()) else { return };
        if self.resampler.as_ref().is_none_or(|(id, _)| *id != utt) {
            self.resampler = Some((utt, Resampler::new(TTS_RATE, rate)));
        }
        let out = match self.resampler.as_mut() {
            Some((_, r)) if rate != TTS_RATE => r.process_f32(pcm),
            _ => pcm.to_vec(),
        };
        self.with(|p| p.push(utt, &out));
    }
    fn end(&mut self, utt: UttId) {
        self.with(|p| p.end(utt));
    }
    fn stop(&mut self) {
        self.resampler = None;
        self.with(|p| p.stop());
    }
    fn clock(&self) -> Option<(UttId, Duration)> {
        self.with(|p| p.clock()).flatten()
    }
    fn done(&self, utt: UttId) -> bool {
        self.with(|p| p.done(utt)).unwrap_or(true)
    }
    fn level(&self) -> f32 {
        self.with(|p| p.level()).unwrap_or(0.0)
    }
}

/// The voice-processing unit (macOS): the mic and the speaker in one,
/// the speaker's echo removed from the mic (voice-echo; voicemode/aec.rs).
#[path = "aec.rs"]
pub mod aec;

/// The mic and the speaker together (plan §8 #6): on macOS one
/// VoiceProcessingIO unit, the speaker's echo cancelled in the mic,
/// `aec: true`; when it cannot open (another OS, a unit or device
/// error) or `BISE_VOICE_AEC=0`, the plain devices, `aec: false`.
/// Never opened by a test.
pub fn open_voice_io() -> Result<super::VoiceIo, String> {
    let env = std::env::var("BISE_VOICE_AEC").ok();
    if aec::wanted(env.as_deref()) {
        match aec::open() {
            Ok((mic, speaker)) => {
                super::debug::log(|| "echo cancelling on · the voice-processing unit opened and started".into());
                return Ok(super::VoiceIo { mic, speaker, aec: true });
            }
            // voice-echo3: the reason was dropped; the debug log says it
            Err(e) => super::debug::log(|| format!("echo cancelling off · {} · plain mic and speaker", e)),
        }
    } else {
        super::debug::log(|| format!("echo cancelling off · BISE_VOICE_AEC={} · plain mic and speaker", env.unwrap_or_default()));
    }
    let speaker = open_speaker();
    if let Err(e) = &speaker {
        super::debug::log(|| format!("no speaker · {}", e));
    }
    Ok(super::VoiceIo { mic: Box::new(CpalMic), speaker: speaker?, aec: false })
}

/// The default output device (cpal), f32 resampled from TTS_RATE. Never
/// opened by a test.
#[cfg(not(target_os = "linux"))]
pub fn open_speaker() -> Result<Box<dyn Speaker>, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no audio output device found.")?;
    let config = device.default_output_config().map_err(|e| format!("audio output is unavailable: {}", e))?;
    let stream_config: cpal::StreamConfig = config.config();
    let channels = stream_config.channels as usize;
    let play = Arc::new(Mutex::new(Playback::new(stream_config.sample_rate.0)));
    let p = play.clone();
    let stream = match config.sample_format() {
        SampleFormat::F32 => output_stream::<f32>(&device, &stream_config, channels, p),
        SampleFormat::I16 => output_stream::<i16>(&device, &stream_config, channels, p),
        SampleFormat::U16 => output_stream::<u16>(&device, &stream_config, channels, p),
        SampleFormat::I32 => output_stream::<i32>(&device, &stream_config, channels, p),
        other => Err(format!("unsupported sample format {}", other)),
    }?;
    stream.play().map_err(|e| format!("audio output is unavailable: {}", e))?;
    Ok(Box::new(QueueSpeaker::new(play, Some(Box::new(stream)))))
}

#[cfg(not(target_os = "linux"))]
fn output_stream<T>(device: &cpal::Device, config: &cpal::StreamConfig, channels: usize, play: Arc<Mutex<Playback>>) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;
    let channels = channels.max(1);
    let mut mono: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                mono.resize(data.len() / channels, 0.0);
                match play.lock() {
                    Ok(mut p) => p.fill(&mut mono),
                    Err(_) => mono.fill(0.0),
                }
                for (frame, x) in data.chunks_mut(channels).zip(&mono) {
                    for s in frame {
                        *s = T::from_sample(*x);
                    }
                }
            },
            |_err| {},
            None,
        )
        .map_err(|e| format!("audio output is unavailable: {}", e))
}

#[cfg(target_os = "linux")]
pub fn open_speaker() -> Result<Box<dyn Speaker>, String> {
    Err("voice mode is not available on Linux yet".into())
}

/// A speaker with a real-time clock and no device: a thread pulls
/// [`SILENT_PULL`] of samples every [`SILENT_PULL`] and plays nothing
/// (the e2e's speaker, `BISE_VOICE_FAKE`; the controller's tests that
/// want the clock to run). The thread ends with the speaker.
pub fn silent_speaker() -> Box<dyn Speaker> {
    const RATE: u32 = 48_000;
    let play = Arc::new(Mutex::new(Playback::new(RATE)));
    let stop = Arc::new(AtomicBool::new(false));
    let (p, s) = (play.clone(), stop.clone());
    std::thread::spawn(move || {
        let n = (RATE as u128 * SILENT_PULL.as_millis() / 1000) as usize;
        let mut buf = vec![0.0f32; n];
        let start = Instant::now();
        for i in 1u32.. {
            if s.load(Ordering::SeqCst) {
                return;
            }
            match p.lock() {
                Ok(mut p) => p.fill(&mut buf),
                Err(_) => return,
            }
            std::thread::sleep((start + SILENT_PULL * i).saturating_duration_since(Instant::now()));
        }
    });
    Box::new(QueueSpeaker::new(play, Some(Box::new(StopOnDrop(stop)))))
}

/// How often [`silent_speaker`]'s thread pulls (a device's buffer).
pub const SILENT_PULL: Duration = Duration::from_millis(10);

struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A speaker that plays nothing and has nothing to play (no output
/// device): every utterance is done at once.
#[derive(Default)]
pub struct NoSpeaker;

impl Speaker for NoSpeaker {
    fn push(&mut self, _utt: UttId, _pcm: &[f32]) {}
    fn end(&mut self, _utt: UttId) {}
    fn stop(&mut self) {}
    fn clock(&self) -> Option<(UttId, Duration)> {
        None
    }
    fn done(&self, _utt: UttId) -> bool {
        true
    }
    fn level(&self) -> f32 {
        0.0
    }
}

// ---- recordings ----

/// A PCM 16-bit mono WAV: its rate and samples. The chunks other than
/// `fmt ` and `data` (afconvert's `FLLR` padding) are skipped.
pub fn read_wav(bytes: &[u8]) -> Result<(u32, Vec<i16>), String> {
    let u16_at = |i: usize| bytes.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err("not a WAV file".into());
    }
    let mut rate = None;
    let mut i = 12;
    while let (Some(id), Some(len)) = (bytes.get(i..i + 4), u32_at(i + 4)) {
        let body = i + 8;
        let len = len as usize;
        match id {
            b"fmt " => {
                let (format, channels, bits) = (u16_at(body), u16_at(body + 2), u16_at(body + 14));
                if format != Some(1) || channels != Some(1) || bits != Some(16) {
                    return Err(format!("not PCM 16-bit mono (format {:?}, {:?} channels, {:?} bits)", format, channels, bits));
                }
                rate = u32_at(body + 4);
            }
            b"data" => {
                let rate = rate.ok_or("the data comes before the format")?;
                let data = bytes.get(body..(body + len).min(bytes.len())).unwrap_or_default();
                return Ok((rate, data.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()));
            }
            _ => {}
        }
        i = body + len + (len & 1);
    }
    Err("no data in the WAV file".into())
}

/// A recorded fixture by name (testdata/<name>.wav, 16 kHz mono):
/// "sentence", "sentence_room" (the sentence after 0.5 s of a room,
/// 0.7 s of it after), "mm", "ok", "silence", "room", "keyboard".
#[cfg(test)]
pub fn fixture(name: &str) -> Vec<i16> {
    let bytes: &[u8] = match name {
        "sentence" => include_bytes!("testdata/sentence.wav"),
        "sentence_room" => include_bytes!("testdata/sentence_room.wav"),
        "mm" => include_bytes!("testdata/mm.wav"),
        "ok" => include_bytes!("testdata/ok.wav"),
        "silence" => include_bytes!("testdata/silence.wav"),
        "room" => include_bytes!("testdata/room.wav"),
        "keyboard" => include_bytes!("testdata/keyboard.wav"),
        other => panic!("no fixture {:?}", other),
    };
    let (rate, pcm) = read_wav(bytes).unwrap_or_else(|e| panic!("{}: {}", name, e));
    assert_eq!(rate, MIC_RATE, "{}", name);
    pcm
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
