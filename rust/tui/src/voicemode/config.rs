//! Voice mode's settings (owner: voice-settings; design §6, plan §2):
//! `[voice]` in ~/.bise/config.toml through bise_catalog, and the jobs
//! (endpoints and keys) the pieces call. Stub: filled by voice-settings.

use super::{AckJob, ListenJob, SayJob};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenMode {
    /// hands-free with headphones, hold space on speakers
    Auto,
    HandsFree,
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadAloud {
    /// what needs you + what you asked
    Needs,
    /// everything the agent would write
    All,
    Nothing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceModeConfig {
    pub listen: ListenMode,
    /// "provider/model" of the TTS
    pub tts_model: String,
    pub voice: String,
    pub speed: f32,
    pub read_aloud: ReadAloud,
    pub sounds: bool,
    pub language: Option<String>,
    /// the first voice mode showed who hears you
    pub seen_privacy: bool,
}

impl Default for VoiceModeConfig {
    fn default() -> Self {
        VoiceModeConfig {
            listen: ListenMode::Auto,
            tts_model: "mistral/voxtral-mini-tts-2603".into(),
            voice: super::tts::DEFAULT_VOICE.into(),
            speed: 1.0,
            read_aloud: ReadAloud::Needs,
            sounds: true,
            language: None,
            seen_privacy: false,
        }
    }
}

pub fn load() -> VoiceModeConfig {
    VoiceModeConfig::default()
}

pub fn listen_job() -> Result<ListenJob, String> {
    crate::voice::resolve_job().map(|batch| ListenJob { realtime: None, batch })
}

pub fn say_job(_cfg: &VoiceModeConfig) -> Result<SayJob, String> {
    Err("voice mode's voice is not set up yet".into())
}

pub fn ack_job() -> Result<AckJob, String> {
    Err("voice mode's small-jobs model is not set up yet".into())
}
