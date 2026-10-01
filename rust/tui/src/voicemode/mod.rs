//! Voice mode (docs/voice-to-voice-design.md §0, E; the build plan and
//! these contracts: docs/voice-mode-plan.md). ctrl+r twice enters, esc
//! leaves: the composer grows to half the screen with the big talking
//! kiss, you talk, the agent in view answers aloud.
//!
//! The cascade: the mic ([`Mic`]) → the turn controller ([`turn`]: VAD,
//! end of turn, barge-in) → the listener ([`Listener`]: Voxtral Realtime,
//! else the batch voice model per turn) → the agent in view (`sb` input)
//! → its message → [`speak::speakable`] → the synthesizer ([`Synthesizer`]:
//! Voxtral TTS, streamed) → the speaker ([`Speaker`]), whose clock lights
//! the said words in the pane and in the thread ([`Lit`]).
//!
//! This file is the contract between the pieces (owners in the plan §2):
//! change a type here through the lead (`voice-mode`), never alone.

// the pieces land in parallel (plan §3); the integration removes this
#![allow(dead_code)]

use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub mod ack; // voice-tts
pub mod answers; // voice-settings
pub mod audio; // voice-audio
pub mod config; // voice-settings
pub mod kiss; // voice-tui
pub mod listen; // voice-stt
pub mod pane; // voice-tui
pub mod route; // voice-audio
pub mod settings; // voice-settings
pub mod speak; // voice-tts
pub mod timing; // voice-tts
pub mod tts; // voice-tts
pub mod turn; // voice-mode (lead)
pub mod vad; // voice-audio

#[cfg(test)]
pub mod fakes; // voice-mode (lead); each piece may keep its own in its file

// ---- rates and timings (plan §4.4) ----

/// The mic, the VAD and the listener: 16 kHz mono i16 (voice.rs's rate).
pub const MIC_RATE: u32 = crate::voice::SAMPLE_RATE;
/// Voxtral TTS's PCM: 24 kHz mono f32.
pub const TTS_RATE: u32 = 24_000;
/// After your last word, `about to answer ●●●··` fills in this long, then
/// the turn is sent (talking again empties it; space sends at once).
pub const END_OF_TURN: Duration = Duration::from_millis(1200);
/// Silence that starts the fill (a breath inside a sentence is shorter).
pub const PAUSE: Duration = Duration::from_millis(300);
/// Real words over the agent's voice for this long cut it off (headphones).
pub const BARGE_IN: Duration = Duration::from_millis(400);
/// Over this long, even backchannels cut in (a long "mmmm…" is a turn).
pub const BARGE_IN_ANYWAY: Duration = Duration::from_millis(1200);
/// ctrl+r then ctrl+r within this: voice mode (one ctrl+r: dictation).
pub const DOUBLE_CTRL_R: Duration = Duration::from_millis(400);
/// The playback stops this fast after a cut (Speaker::stop).
pub const STOP_WITHIN: Duration = Duration::from_millis(50);
/// The "on it" line waits this long for the small-jobs model, else a
/// canned one is said.
pub const ACK_DEADLINE: Duration = Duration::from_millis(700);

/// The lanes' waves: one level per step, this many kept.
pub const WAVE_STEP: Duration = Duration::from_millis(100);
pub const WAVE_LEN: usize = 52;

/// Words that never cut the agent off and never allow anything
/// (design §0, §5). Lowercase, without punctuation; English and French.
pub const BACKCHANNELS: &[&str] = &[
    "mm", "mmm", "mhm", "hm", "hmm", "uh", "uh-huh", "ok", "okay", "right", "yeah", "yes", "yep", "sure", "oui",
    "ouais", "d'accord", "ah", "oh", "euh", "bon",
];

// ---- audio in (voice-audio) ----

/// ~20-100 ms of mic audio at [`MIC_RATE`].
#[derive(Clone, Debug, PartialEq)]
pub struct MicBlock {
    pub pcm: Vec<i16>,
    /// when the block's first sample was captured
    pub at: Instant,
}

/// The microphone, opened for the whole voice mode.
pub trait Mic {
    /// Start sending blocks; dropping the stream closes the mic (the
    /// macOS mic indicator goes off).
    fn open(&mut self, blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String>;
}

pub trait MicStream {
    /// The last block's loudness, 0..1 (`voice::loudness` of its peak).
    fn level(&self) -> f32;
}

// ---- audio out (voice-audio) ----

/// One sentence said aloud, numbered by the controller.
pub type UttId = u64;

/// The speaker: utterances play in push order, back to back.
pub trait Speaker {
    /// More audio of `utt` at [`TTS_RATE`]; a new utt queues after the
    /// ones before it.
    fn push(&mut self, utt: UttId, pcm: &[f32]);
    /// `utt` has all its audio: it ends when its last sample played.
    fn end(&mut self, utt: UttId);
    /// Cut: silent within [`STOP_WITHIN`] (a ~10 ms fade), every queued
    /// utterance dropped.
    fn stop(&mut self);
    /// The utterance playing and how much of it has played (the clock
    /// of the lit words); None: nothing plays.
    fn clock(&self) -> Option<(UttId, Duration)>;
    /// `utt` ended (its last sample played) or was dropped.
    fn done(&self, utt: UttId) -> bool;
    /// The output's loudness now, 0..1: the mouth and its arcs.
    fn level(&self) -> f32;
}

/// Where the voice comes out: decides barge-in (headphones) or hold
/// space while the agent talks (speakers: it would hear itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Headphones,
    Speakers,
    /// not known (not macOS, an odd device): asked once, then saved
    Unknown,
}

// ---- speech to text (voice-stt) ----

/// A provider call; the key is never printed (Debug hides it).
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// "provider/id", for messages
    pub name: String,
    pub provider_name: String,
    /// no trailing '/'
    pub base_url: String,
    pub model: String,
    pub key: String,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Endpoint {{ name: {:?}, base_url: {:?}, model: {:?}, key: <{} bytes> }}", self.name, self.base_url, self.model, self.key.len())
    }
}

#[derive(Clone, Debug)]
pub struct ListenJob {
    /// Voxtral Realtime when the voice role's provider has it (Mistral)
    pub realtime: Option<Endpoint>,
    /// else the voice role's batch model, one request per turn
    pub batch: crate::voice::VoiceJob,
}

/// What the controller feeds the listener.
#[derive(Debug, PartialEq)]
pub enum ListenMsg {
    /// [`MIC_RATE`] PCM, only while the mic is live (not muted, not
    /// paused, not gated on speakers)
    Audio(Vec<i16>),
    /// the turn ends: transcribe every word sent so far, then [`Heard::Flushed`]
    Flush,
    /// drop what was heard since the last flush (esc on a half turn)
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Heard {
    /// new words, appended to the turn's text as is (they carry their
    /// own leading space); a realtime listener sends them as you talk
    Text(String),
    /// every word of the audio sent before the Flush is in
    Flushed,
    /// the one-line reason (voice::fail_lines' wording)
    Failed(String),
}

pub trait Listener {
    /// Runs on its own thread until `audio` closes or `cancel`.
    fn start(&self, job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>);
}

// ---- what is said (voice-tts) ----

/// One sentence to say, with where its words are in the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sentence {
    /// the text sent to the TTS (numbers in words, no code/paths/URLs)
    pub say: String,
    pub words: Vec<Word>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    /// byte range in [`Sentence::say`]
    pub say: Range<usize>,
    /// byte range in the message as shown in the thread; None for a word
    /// the message does not have ("three things:", "the rest is on screen")
    pub src: Option<Range<usize>>,
}

/// A message as said: the first sentence or two (~12 s), the rest shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spoken {
    pub sentences: Vec<Sentence>,
    /// there is more than what is said ("the rest is on screen" is then
    /// the last sentence)
    pub more: bool,
}

#[derive(Clone, Debug)]
pub struct SayJob {
    pub api: Endpoint,
    /// the voice id (one brand voice for now: a default Voxtral voice)
    pub voice: String,
    /// 0.8-1.6
    pub speed: f32,
}

#[derive(Debug, PartialEq)]
pub enum Synth {
    /// [`TTS_RATE`] mono f32, as it streams
    Audio(Vec<f32>),
    Done,
    Failed(String),
}

pub trait Synthesizer {
    /// Says `text`, streamed; its own thread; stops at `cancel`.
    fn start(&self, job: SayJob, text: String, events: Sender<Synth>, cancel: Arc<AtomicBool>);
}

/// The small-jobs model, for the spoken "on it" while the agent starts.
#[derive(Clone, Debug)]
pub struct AckJob {
    pub api: Endpoint,
    /// its chat wire family (bise_catalog: "openai", "anthropic", …)
    pub family: String,
}

pub trait Acker {
    /// Sends at most one short line (≤ 5 words) for what you said; may
    /// send nothing (the controller says a canned one at [`ACK_DEADLINE`]).
    fn start(&self, job: AckJob, heard: String, events: Sender<String>, cancel: Arc<AtomicBool>);
}

// ---- the pane and the thread (voice-tui reads, the controller writes) ----

/// What the turn is at: the kiss, the status row and the keys follow it.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// mic open, nobody talks (the `*` breathes)
    Listening,
    /// you talk
    Hearing,
    /// you stopped: `about to answer ●●●··`, `fill` 0..1
    AboutToAnswer { fill: f32 },
    /// space held: the floor is yours, no fill
    Holding,
    /// sent; the agent works (its tool lines come into the thread above)
    Working,
    /// the agent talks
    Speaking,
    /// you cut in: the mouth shuts to a line, `you cut in`
    CutIn,
    /// m: the mic is off
    Muted,
    /// tab: you type, the mic waits
    Typing,
    /// speakers and the agent talks: hold space to talk
    HoldToTalk,
    /// the one-line reason; voice mode stays open
    Failed(String),
}

/// Who a caption line is from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    You,
    /// the agent in view ("main", "cookies")
    Agent(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordState {
    /// you: heard, final
    Heard,
    /// you: still being transcribed (dim)
    Partial,
    /// the agent: said already
    Said,
    /// the agent: not said yet
    ToSay,
    /// the agent: not said, you cut in (faint)
    Cut,
}

/// Everything the pane draws, built by the controller each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneView {
    pub phase: Phase,
    /// the agent in view
    pub agent: String,
    /// who the captions are from, and their words (the pane wraps them
    /// at ≤ 28 cells and keeps the last lines that fit)
    pub who: Who,
    pub words: Vec<(String, WordState)>,
    /// your level and the agent's, 0..1, now
    pub you_level: f32,
    pub agent_level: f32,
    /// the recent levels for the lanes' waves, oldest first, one per
    /// [`WAVE_STEP`], at most [`WAVE_LEN`]
    pub you_wave: Vec<f32>,
    pub agent_wave: Vec<f32>,
    /// since voice mode opened (the header's `● voice mode 2:14`)
    pub elapsed: Duration,
    pub route: Route,
    /// a question or an approval was heard: `heard "the first one" → 1 smaller` (1.5 s)
    pub heard_answer: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LitState {
    Said,
    ToSay,
    Cut,
}

/// The agent's message being said, lit in the thread (the same words as
/// beside the kiss). The feed finds the last assistant message of
/// `agent` whose text is `text` and styles these byte ranges of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lit {
    pub agent: String,
    pub text: String,
    pub spans: Vec<(Range<usize>, LitState)>,
}
