//! Speech-to-text in the composer, the Vibe CLI way (vibe/cli/voice_manager,
//! vibe/cli/transcribe, vibe/cli/audio_recorder).
//!
//! Ctrl+R records from the default microphone and streams 16 kHz mono
//! PCM to Mistral's realtime transcription websocket; the text deltas
//! land in the composer as they arrive. While recording, any key stops
//! (the last words are flushed), Ctrl+C or Esc cancels. Off by default:
//! `/voice` toggles it, saved in ~/.bend-harness/tui.json.
//!
//! Layout: pure parts first (key decisions, resampling, the wire
//! protocol, settings), then the controller [`Voice`] driven through
//! two ports ([`Recorder`], [`Transcriber`]) so the tests run without a
//! microphone or a network, then the real adapters (cpal, tungstenite).

use crossterm::event::{KeyCode, KeyModifiers};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---- configuration (Vibe's defaults: vibe/core/config/vibe_schema.py) ----

pub const API_KEY_ENV: &str = "MISTRAL_API_KEY";
const API_BASE: &str = "wss://api.mistral.ai";
const MODEL: &str = "voxtral-mini-transcribe-realtime-2602";
pub const SAMPLE_RATE: u32 = 16_000;
const TARGET_STREAMING_DELAY_MS: u32 = 500;
/// Audio sent per websocket message (Vibe's capture buffer is 200 ms).
const SEND_BLOCK: usize = SAMPLE_RATE as usize / 5;
const FLUSH_TIMEOUT: Duration = Duration::from_secs(10);
/// The flush timed out after some text arrived.
pub const LATE_DONE_NOTICE: &str = "the last words may be missing (the transcription did not finish in time).";
const MAX_DURATION: Duration = Duration::from_secs(300);
/// Shorter than this, a recording has no audio blocks yet: silence then
/// means "stopped too early", not "the microphone is muted".
const MIN_SIGNAL_DURATION: Duration = Duration::from_millis(500);
/// A denied or muted microphone gives pure silence: any peak above this
/// floor means a real signal reached us.
const SILENCE_PEAK: f32 = 0.001;
/// The server rejects the end-of-stream flush of a recording without
/// audio: a benign empty recording, not a failure.
const EMPTY_RECORDING_MARKER: &str = "before sending any audio bytes";

/// The level meter shown in place of the prompt while recording.
pub const PEAK_BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// The spinner shown while the last words are flushed.
pub const FILL_BLOCKS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

pub fn peak_glyph(peak: f32) -> char {
    let i = (peak.clamp(0.0, 1.0) * PEAK_BLOCKS.len() as f32) as usize;
    PEAK_BLOCKS[i.min(PEAK_BLOCKS.len() - 1)]
}

/// `ms` since the flush started → the spinner frame (100 ms a frame).
pub fn flush_glyph(ms: u128) -> char {
    FILL_BLOCKS[(ms / 100) as usize % FILL_BLOCKS.len()]
}

fn mic_access_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        " grant access in System Settings → Privacy & Security → Microphone."
    } else if cfg!(target_os = "windows") {
        " grant access in Settings → Privacy & security → Microphone."
    } else {
        ""
    }
}

fn no_audio_detected_message() -> String {
    format!(
        "no audio detected from the microphone — check your terminal has mic access.{}",
        mic_access_hint()
    )
}

pub const ENABLED_MESSAGE: &str = "voice mode on. press ctrl+r to start recording.";
pub const DISABLED_MESSAGE: &str = "voice mode off.";
pub const OFF_HINT: &str = "voice mode is off: /voice turns it on";

// ---- keys ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceState {
    Idle,
    Recording,
    Flushing,
}

/// What a key does to the voice input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// not a voice key: the composer handles it
    Pass,
    Start,
    Stop,
    Cancel,
    /// eaten while the last words are flushed
    Swallow,
    /// Ctrl+R with voice mode off
    OffHint,
}

/// Vibe's text_area._handle_voice_key: while not idle every key is
/// taken (Ctrl+C / Esc cancel, any other key stops a recording); at
/// idle, Ctrl+R starts.
pub fn key_action(state: VoiceState, enabled: bool, code: KeyCode, mods: KeyModifiers) -> KeyAction {
    let ctrl_c = code == KeyCode::Char('c') && mods == KeyModifiers::CONTROL;
    match state {
        VoiceState::Recording | VoiceState::Flushing if ctrl_c || code == KeyCode::Esc => {
            KeyAction::Cancel
        }
        VoiceState::Recording => KeyAction::Stop,
        VoiceState::Flushing => KeyAction::Swallow,
        VoiceState::Idle if code == KeyCode::Char('r') && mods == KeyModifiers::CONTROL => {
            if enabled {
                KeyAction::Start
            } else {
                KeyAction::OffHint
            }
        }
        VoiceState::Idle => KeyAction::Pass,
    }
}

// ---- audio processing (pure) ----

/// Interleaved frames → mono (the channel average).
pub fn to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect()
}

/// A streaming linear resampler: feed blocks of any size, the phase and
/// the last sample carry over from one block to the next.
#[derive(Debug)]
pub struct Resampler {
    /// input samples per output sample
    step: f64,
    /// the position of the next output sample, in input samples,
    /// counted from the previous block's last sample (index -1): the
    /// first output is the first input sample (pos 1)
    pos: f64,
    last: Option<f32>,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Resampler {
            step: from_rate.max(1) as f64 / to_rate.max(1) as f64,
            pos: 1.0,
            last: None,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<i16> {
        if input.is_empty() {
            return Vec::new();
        }
        // x[-1] = the previous block's last sample (or the first one)
        let prev = self.last.unwrap_or(input[0]);
        let at = |i: isize| if i < 0 { prev } else { input[i as usize] };
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 1);
        // pos is relative to index -1: sample k sits at pos - 1
        while self.pos - 1.0 <= (input.len() - 1) as f64 {
            let p = self.pos - 1.0;
            let i0 = p.floor() as isize;
            let frac = (p - i0 as f64) as f32;
            let a = at(i0);
            let b = if i0 < (input.len() - 1) as isize { at(i0 + 1) } else { a };
            out.push(to_i16(a + (b - a) * frac));
            self.pos += self.step;
        }
        self.pos -= input.len() as f64;
        self.last = input.last().copied();
        out
    }
}

pub fn to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// The block peak, in [0, 1].
pub fn peak(samples: &[i16]) -> f32 {
    let m = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    (m as f32 / i16::MAX as f32).min(1.0)
}

fn pcm_bytes(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

// ---- the realtime protocol (pure) ----
// mistralai/extra/realtime: session.created, then session.update, then
// input_audio.append blocks, input_audio.flush + input_audio.end.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscribeEvent {
    SessionCreated,
    Delta(String),
    Done,
    Error(String),
}

/// One server message → an event (None: an event we do not use).
pub fn parse_server_event(text: &str) -> Option<TranscribeEvent> {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return None,
    };
    match v.get("type").and_then(|t| t.as_str())? {
        "session.created" => Some(TranscribeEvent::SessionCreated),
        "transcription.text.delta" => Some(TranscribeEvent::Delta(
            v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        )),
        "transcription.done" => Some(TranscribeEvent::Done),
        "error" => {
            let msg = error_message(&v);
            if msg.contains(EMPTY_RECORDING_MARKER) {
                Some(TranscribeEvent::Done)
            } else {
                Some(TranscribeEvent::Error(msg))
            }
        }
        _ => None,
    }
}

fn error_message(v: &Value) -> String {
    let e = v.get("error").unwrap_or(v);
    match e.get("message") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => e.to_string(),
    }
}

pub fn session_update_message(sample_rate: u32, delay_ms: u32) -> String {
    json!({
        "type": "session.update",
        "session": {
            "audio_format": {"encoding": "pcm_s16le", "sample_rate": sample_rate},
            "target_streaming_delay_ms": delay_ms,
        }
    })
    .to_string()
}

pub fn append_message(samples: &[i16]) -> String {
    use base64::Engine;
    let audio = base64::engine::general_purpose::STANDARD.encode(pcm_bytes(samples));
    json!({"type": "input_audio.append", "audio": audio}).to_string()
}

pub fn flush_message() -> String {
    json!({"type": "input_audio.flush"}).to_string()
}

pub fn end_message() -> String {
    json!({"type": "input_audio.end"}).to_string()
}

pub fn realtime_url(api_base: &str, model: &str) -> String {
    format!(
        "{}/v1/audio/transcriptions/realtime?model={}",
        api_base.trim_end_matches('/'),
        model
    )
}

// ---- settings (~/.bend-harness/tui.json) ----

fn settings_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::Path::new(&home).join(".bend-harness").join("tui.json"))
}

/// Voice mode from the settings text and the SB_VOICE override
/// (1/on/true forces on, 0/off/false forces off).
pub fn voice_enabled_from(settings: Option<&str>, env: Option<&str>) -> bool {
    match env.map(|s| s.trim().to_lowercase()) {
        Some(s) if matches!(s.as_str(), "1" | "on" | "true" | "yes") => return true,
        Some(s) if matches!(s.as_str(), "0" | "off" | "false" | "no") => return false,
        _ => {}
    }
    settings
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .and_then(|v| v.get("voice_mode_enabled").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

pub fn load_voice_enabled() -> bool {
    let text = settings_path().and_then(|p| std::fs::read_to_string(p).ok());
    let env = std::env::var("SB_VOICE").ok();
    voice_enabled_from(text.as_deref(), env.as_deref())
}

/// The settings text with voice_mode_enabled set, other keys kept.
pub fn with_voice_enabled(settings: Option<&str>, enabled: bool) -> String {
    let mut v = settings
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}));
    v["voice_mode_enabled"] = Value::Bool(enabled);
    serde_json::to_string_pretty(&v).unwrap_or_default() + "\n"
}

pub fn save_voice_enabled(enabled: bool) -> Result<(), String> {
    let path = settings_path().ok_or("HOME is not set")?;
    let old = std::fs::read_to_string(&path).ok();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, with_voice_enabled(old.as_deref(), enabled)).map_err(|e| e.to_string())
}

/// KEY=VALUE lines of a .env text (the harness's load_env_files rules).
pub fn env_file_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (k, v) = line.split_once('=')?;
        let v = v.trim().trim_matches('"').trim_matches('\'');
        (k.trim() == key && !v.is_empty()).then(|| v.to_string())
    })
}

/// MISTRAL_API_KEY from the environment, else ~/.bend-harness/.env then
/// ~/.vibe/.env: the keys the harness already uses (the switchboard
/// client does not load the .env files itself).
pub fn resolve_api_key() -> Option<String> {
    if let Some(v) = std::env::var(API_KEY_ENV).ok().filter(|v| !v.trim().is_empty()) {
        return Some(v);
    }
    let home = std::env::var("HOME").ok()?;
    [".bend-harness/.env", ".vibe/.env"].iter().find_map(|f| {
        let text = std::fs::read_to_string(format!("{}/{}", home, f)).ok()?;
        env_file_value(&text, API_KEY_ENV)
    })
}

// ---- ports ----

/// What the recorder sends the transcriber.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioMsg {
    /// mono PCM at [`SAMPLE_RATE`]
    Chunk(Vec<i16>),
    /// the recording stopped: flush and end the stream
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    NoInputDevice,
    Backend(String),
}

/// A running capture; dropping it stops the microphone.
pub trait Capture {
    fn peak(&self) -> f32;
    fn has_signal(&self) -> bool;
}

pub trait Recorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError>;
}

/// Starts a transcription session on its own thread: reads `audio` until
/// [`AudioMsg::End`], sends the events to `events`, stops when `cancel`
/// is set.
pub trait Transcriber {
    fn start(
        &self,
        api_key: String,
        audio: Receiver<AudioMsg>,
        events: Sender<TranscribeEvent>,
        cancel: Arc<AtomicBool>,
    );
}

// ---- the controller (vibe/cli/voice_manager/voice_manager.py) ----

/// What the UI does after a controller step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceOutput {
    /// insert at the composer cursor
    Insert(String),
    /// the end of an utterance (one undo step)
    Utterance,
    /// a failure (Vibe's error toast)
    Error(String),
    /// a short notice (Vibe's inline notice)
    Notice(String),
}

struct Run {
    capture: Option<Box<dyn Capture>>,
    audio: Sender<AudioMsg>,
    events: Receiver<TranscribeEvent>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    stopped: Option<Instant>,
    has_signal: bool,
    text_len: usize,
}

pub struct Voice {
    pub enabled: bool,
    recorder: Box<dyn Recorder>,
    transcriber: Box<dyn Transcriber>,
    state: VoiceState,
    run: Option<Run>,
}

impl Voice {
    pub fn new(enabled: bool, recorder: Box<dyn Recorder>, transcriber: Box<dyn Transcriber>) -> Self {
        Voice { enabled, recorder, transcriber, state: VoiceState::Idle, run: None }
    }

    /// The real microphone and the Mistral realtime API.
    pub fn live(enabled: bool) -> Self {
        Voice::new(enabled, Box::new(CpalRecorder), Box::new(MistralRealtime::default()))
    }

    pub fn state(&self) -> VoiceState {
        self.state
    }

    pub fn active(&self) -> bool {
        self.state != VoiceState::Idle
    }

    pub fn peak(&self) -> f32 {
        self.run
            .as_ref()
            .and_then(|r| r.capture.as_ref())
            .map(|c| c.peak())
            .unwrap_or(0.0)
    }

    /// When the flush started (the spinner's time base).
    pub fn flushing_since(&self) -> Option<Instant> {
        self.run.as_ref().and_then(|r| r.stopped)
    }

    /// Start recording; Err is the warning to show (Vibe's
    /// RecordingStartError messages).
    pub fn start(&mut self, api_key: Option<String>, now: Instant) -> Result<(), String> {
        if self.state != VoiceState::Idle {
            return Ok(());
        }
        let Some(key) = api_key.filter(|k| !k.trim().is_empty()) else {
            return Err(format!("voice transcription needs an API key: set {}", API_KEY_ENV));
        };
        let (audio_tx, audio_rx) = mpsc::channel();
        let capture = self.recorder.start(SAMPLE_RATE, audio_tx.clone()).map_err(|e| match e {
            StartError::NoInputDevice => format!("no audio input device found.{}", mic_access_hint()),
            StartError::Backend(m) => format!("audio backend is unavailable: {}", m),
        })?;
        let (ev_tx, ev_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.transcriber.start(key, audio_rx, ev_tx, cancel.clone());
        self.run = Some(Run {
            capture: Some(capture),
            audio: audio_tx,
            events: ev_rx,
            cancel,
            started: now,
            stopped: None,
            has_signal: false,
            text_len: 0,
        });
        self.state = VoiceState::Recording;
        Ok(())
    }

    /// Stop the microphone; the last words are flushed.
    pub fn stop(&mut self, now: Instant) {
        if self.state != VoiceState::Recording {
            return;
        }
        if let Some(run) = self.run.as_mut() {
            stop_capture(run, now);
            let _ = run.audio.send(AudioMsg::End);
        }
        self.state = VoiceState::Flushing;
    }

    /// Drop the recording (the text already inserted stays).
    pub fn cancel(&mut self) {
        if let Some(run) = self.run.take() {
            run.cancel.store(true, Ordering::SeqCst);
            // dropping the capture stops the microphone
        }
        self.state = VoiceState::Idle;
    }

    /// Drain the transcription events; checks the flush timeout and the
    /// maximum duration. Call it on every UI tick.
    pub fn poll(&mut self, now: Instant) -> Vec<VoiceOutput> {
        let mut out = Vec::new();
        while let Some(run) = self.run.as_mut() {
            match run.events.try_recv() {
                Ok(TranscribeEvent::Delta(t)) => {
                    run.text_len += t.chars().count();
                    if !t.is_empty() {
                        out.push(VoiceOutput::Insert(t));
                    }
                }
                Ok(TranscribeEvent::SessionCreated) => {}
                Ok(TranscribeEvent::Error(m)) => {
                    self.cancel();
                    out.push(VoiceOutput::Error(format!("voice transcription failed: {}", m)));
                }
                Ok(TranscribeEvent::Done) | Err(TryRecvError::Disconnected) => {
                    out.extend(self.finish(now));
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match (self.state, self.run.as_ref()) {
            (VoiceState::Recording, Some(r)) if now.duration_since(r.started) >= MAX_DURATION => {
                self.stop(now)
            }
            (VoiceState::Flushing, Some(r))
                if r.stopped.is_some_and(|s| now.duration_since(s) >= FLUSH_TIMEOUT) =>
            {
                // text already landed: keep it, only the last words may
                // be missing — a notice, not a failure
                let had_text = r.text_len > 0;
                self.cancel();
                if had_text {
                    out.push(VoiceOutput::Utterance);
                    out.push(VoiceOutput::Notice(LATE_DONE_NOTICE.into()));
                } else {
                    out.push(VoiceOutput::Error(
                        "voice transcription failed: the transcription timed out".into(),
                    ));
                }
            }
            _ => {}
        }
        out
    }

    fn finish(&mut self, now: Instant) -> Vec<VoiceOutput> {
        let Some(mut run) = self.run.take() else { return Vec::new() };
        stop_capture(&mut run, now);
        run.cancel.store(true, Ordering::SeqCst);
        self.state = VoiceState::Idle;
        let duration = run.stopped.unwrap_or(now).duration_since(run.started);
        if run.text_len > 0 {
            vec![VoiceOutput::Utterance]
        } else if !run.has_signal && duration >= MIN_SIGNAL_DURATION {
            vec![VoiceOutput::Error(format!(
                "voice transcription failed: {}",
                no_audio_detected_message()
            ))]
        } else {
            vec![VoiceOutput::Notice("no speech detected".into())]
        }
    }
}

fn stop_capture(run: &mut Run, now: Instant) {
    if let Some(c) = run.capture.take() {
        run.has_signal = c.has_signal();
        run.stopped = Some(now);
    }
}

// ---- the microphone (cpal: CoreAudio on macOS) ----

/// The capture level, shared with the audio callback.
#[derive(Default)]
struct Level {
    peak_bits: AtomicU32,
    signal: AtomicBool,
}

impl Level {
    fn record(&self, samples: &[i16]) {
        let p = peak(samples);
        self.peak_bits.store(p.to_bits(), Ordering::Relaxed);
        if p > SILENCE_PEAK {
            self.signal.store(true, Ordering::Relaxed);
        }
    }
}

struct CpalCapture {
    _stream: cpal::Stream,
    level: Arc<Level>,
}

impl Capture for CpalCapture {
    fn peak(&self) -> f32 {
        f32::from_bits(self.level.peak_bits.load(Ordering::Relaxed))
    }
    fn has_signal(&self) -> bool {
        self.level.signal.load(Ordering::Relaxed)
    }
}

pub struct CpalRecorder;

impl Recorder for CpalRecorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use cpal::SampleFormat;
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(StartError::NoInputDevice)?;
        let config = device
            .default_input_config()
            .map_err(|_| StartError::NoInputDevice)?;
        let level = Arc::new(Level::default());
        let stream_config: cpal::StreamConfig = config.config();
        let channels = stream_config.channels as usize;
        let rate = stream_config.sample_rate.0;
        let stream = match config.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, channels, rate, sample_rate, audio, level.clone()),
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, channels, rate, sample_rate, audio, level.clone()),
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, channels, rate, sample_rate, audio, level.clone()),
            SampleFormat::I32 => build_stream::<i32>(&device, &stream_config, channels, rate, sample_rate, audio, level.clone()),
            other => Err(StartError::Backend(format!("unsupported sample format {}", other))),
        }?;
        stream.play().map_err(|e| StartError::Backend(e.to_string()))?;
        Ok(Box::new(CpalCapture { _stream: stream, level }))
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    from_rate: u32,
    to_rate: u32,
    audio: Sender<AudioMsg>,
    level: Arc<Level>,
) -> Result<cpal::Stream, StartError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;
    let mut resampler = Resampler::new(from_rate, to_rate);
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let floats: Vec<f32> = data.iter().map(|s| cpal::Sample::to_sample::<f32>(*s)).collect();
                let out = resampler.process(&to_mono(&floats, channels));
                level.record(&out);
                let _ = audio.send(AudioMsg::Chunk(out));
            },
            |_err| {},
            None,
        )
        .map_err(|e| StartError::Backend(e.to_string()))
}

// ---- the Mistral realtime websocket (tungstenite, one thread) ----

pub struct MistralRealtime {
    pub api_base: String,
    pub model: String,
}

impl Default for MistralRealtime {
    fn default() -> Self {
        MistralRealtime { api_base: API_BASE.into(), model: MODEL.into() }
    }
}

impl Transcriber for MistralRealtime {
    fn start(
        &self,
        api_key: String,
        audio: Receiver<AudioMsg>,
        events: Sender<TranscribeEvent>,
        cancel: Arc<AtomicBool>,
    ) {
        let url = realtime_url(&self.api_base, &self.model);
        std::thread::spawn(move || {
            let result = WsSocket::connect(&url, &api_key)
                .and_then(|mut ws| stream_session(&mut ws, &audio, &events, &cancel));
            if let Err(e) = result {
                if !cancel.load(Ordering::SeqCst) {
                    let _ = events.send(TranscribeEvent::Error(e));
                }
            }
        });
    }
}

/// Waits of the session loop: while recording it alternates between
/// the audio channel and the socket; once the audio ended only the
/// socket is left.
const AUDIO_WAIT: Duration = Duration::from_millis(20);
const RECORDING_READ_WAIT: Duration = Duration::from_millis(20);
const ENDED_READ_WAIT: Duration = Duration::from_millis(100);

/// One read from the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SocketRead {
    Text(String),
    /// nothing arrived within the wait
    Idle,
    /// the server closed the connection
    Closed,
}

/// The websocket as the session loop sees it, so the loop runs against
/// a fake socket in the tests.
pub trait RealtimeSocket {
    fn send_text(&mut self, text: String) -> Result<(), String>;
    fn read(&mut self, wait: Duration) -> Result<SocketRead, String>;
    fn close(&mut self);
}

/// The audio channel drained without blocking past the first wait:
/// every queued chunk, and whether the recording ended. `End` can sit
/// right behind the last chunks (the capture stops, then End is sent):
/// it must never be dropped with them, or flush/end are never sent and
/// the server never answers `transcription.done`.
fn drain_audio(audio: &Receiver<AudioMsg>, wait: Duration, pending: &mut Vec<i16>) -> bool {
    let mut next = match audio.recv_timeout(wait) {
        Ok(m) => Some(m),
        Err(mpsc::RecvTimeoutError::Timeout) => None,
        Err(mpsc::RecvTimeoutError::Disconnected) => return true,
    };
    while let Some(msg) = next {
        match msg {
            AudioMsg::Chunk(c) => pending.extend(c),
            AudioMsg::End => return true,
        }
        next = match audio.try_recv() {
            Ok(m) => Some(m),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => return true,
        };
    }
    false
}

/// A session over an open socket (mistralai/extra/realtime
/// transcribe_stream): session.update, the audio in blocks, then
/// input_audio.flush + input_audio.end exactly once, then read until
/// transcription.done (or the server closing the socket).
pub fn stream_session(
    ws: &mut dyn RealtimeSocket,
    audio: &Receiver<AudioMsg>,
    events: &Sender<TranscribeEvent>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    ws.send_text(session_update_message(SAMPLE_RATE, TARGET_STREAMING_DELAY_MS))?;
    let mut pending: Vec<i16> = Vec::new();
    let mut ended = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            ws.close();
            return Ok(());
        }
        // send side: the captured audio, in blocks
        if !ended {
            ended = drain_audio(audio, AUDIO_WAIT, &mut pending);
            if pending.len() >= SEND_BLOCK || (ended && !pending.is_empty()) {
                ws.send_text(append_message(&pending))?;
                pending.clear();
            }
            if ended {
                ws.send_text(flush_message())?;
                ws.send_text(end_message())?;
            }
        }
        // read side: every message already there
        let wait = if ended { ENDED_READ_WAIT } else { RECORDING_READ_WAIT };
        loop {
            match ws.read(wait)? {
                SocketRead::Text(t) => match parse_server_event(&t) {
                    Some(TranscribeEvent::Done) => {
                        let _ = events.send(TranscribeEvent::Done);
                        ws.close();
                        return Ok(());
                    }
                    Some(TranscribeEvent::Error(m)) => return Err(m),
                    Some(ev) => {
                        let _ = events.send(ev);
                    }
                    None => {}
                },
                SocketRead::Closed if ended => {
                    let _ = events.send(TranscribeEvent::Done);
                    return Ok(());
                }
                SocketRead::Closed => {
                    return Err("the connection closed before the recording finished".into())
                }
                SocketRead::Idle => break,
            }
        }
    }
}

type Ws = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// The real socket (tungstenite, blocking, a read timeout per wait).
pub struct WsSocket {
    ws: Ws,
    wait: Option<Duration>,
}

impl WsSocket {
    pub fn connect(url: &str, api_key: &str) -> Result<WsSocket, String> {
        use tungstenite::client::IntoClientRequest;
        let mut req = url.into_client_request().map_err(|e| e.to_string())?;
        let auth = format!("Bearer {}", api_key).parse().map_err(|_| "invalid API key".to_string())?;
        req.headers_mut().insert("Authorization", auth);
        req.headers_mut().insert(
            "User-Agent",
            tungstenite::http::HeaderValue::from_static("bend-harness-tui"),
        );
        let (ws, _) = tungstenite::connect(req).map_err(ws_error)?;
        Ok(WsSocket { ws, wait: None })
    }

    fn set_read_timeout(&mut self, t: Duration) {
        use tungstenite::stream::MaybeTlsStream;
        if self.wait == Some(t) {
            return;
        }
        self.wait = Some(t);
        let _ = match self.ws.get_mut() {
            MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(t)),
            MaybeTlsStream::Rustls(s) => s.get_mut().set_read_timeout(Some(t)),
            _ => Ok(()),
        };
    }
}

impl RealtimeSocket for WsSocket {
    fn send_text(&mut self, text: String) -> Result<(), String> {
        self.ws.send(tungstenite::Message::text(text)).map_err(ws_error)
    }

    fn read(&mut self, wait: Duration) -> Result<SocketRead, String> {
        use tungstenite::{Error, Message};
        self.set_read_timeout(wait);
        loop {
            return match self.ws.read() {
                Ok(Message::Text(t)) => Ok(SocketRead::Text(t.as_str().to_string())),
                Ok(Message::Close(_)) | Err(Error::ConnectionClosed | Error::AlreadyClosed) => {
                    Ok(SocketRead::Closed)
                }
                // pings are answered by tungstenite; binary frames are unused
                Ok(_) => continue,
                Err(Error::Io(e))
                    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
                {
                    Ok(SocketRead::Idle)
                }
                Err(e) => Err(ws_error(e)),
            };
        }
    }

    fn close(&mut self) {
        let _ = self.ws.close(None);
        let _ = self.ws.flush();
    }
}

fn ws_error(e: tungstenite::Error) -> String {
    match e {
        tungstenite::Error::Http(r) => {
            let body = r
                .body()
                .as_ref()
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_default();
            let status = r.status();
            if body.is_empty() {
                format!("HTTP {}", status)
            } else {
                format!("HTTP {}: {}", status, body.trim())
            }
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod fakes;
#[cfg(test)]
mod tests;
