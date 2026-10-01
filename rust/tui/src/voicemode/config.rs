//! Voice mode's settings (owner: voice-settings; design §6, plan §4.7):
//! `[voice]` in ~/.bise/config.toml through bise_catalog (its keys:
//! `bise_catalog::voice::VoiceModeKeys`), and the jobs (endpoints and
//! keys) the pieces call, with the chat keys' resolution (env > auth.json
//! > the old .env files), as `voice::resolve_job` does.

use super::{AckJob, Endpoint, ListenJob, SayJob};
use bise_catalog::auth::{EnvFile, Keys, Store};
use bise_catalog::roles::{with_table_key, without_key};
use bise_catalog::voice::{ModeCall, VoiceModeKeys};
use bise_catalog::Setup;

pub use bise_catalog::voice::{ListenMode, ReadAloud, SPEED_MAX, SPEED_MIN};

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceModeConfig {
    pub listen: ListenMode,
    /// "provider/model" of the TTS; "" when the voice role's provider
    /// does not speak and none is picked ([`say_job`] says why)
    pub tts_model: String,
    pub voice: String,
    /// [`SPEED_MIN`]..=[`SPEED_MAX`]
    pub speed: f32,
    pub read_aloud: ReadAloud,
    pub sounds: bool,
    /// shared with dictation (`[voice] language`); None: detected
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

impl VoiceModeConfig {
    /// From config.toml's text (None: no file) and the env.
    pub fn of_text(text: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> VoiceModeConfig {
        let setup = Setup::from_text(text, env);
        VoiceModeConfig::of(&setup, &VoiceModeKeys::from_text(text))
    }

    fn of(setup: &Setup, k: &VoiceModeKeys) -> VoiceModeConfig {
        VoiceModeConfig {
            listen: k.listen,
            tts_model: setup.tts_name(k).unwrap_or_default(),
            voice: k.tts_voice.clone().unwrap_or_else(|| super::tts::DEFAULT_VOICE.into()),
            speed: k.speed,
            read_aloud: k.read_aloud,
            sounds: k.sounds,
            language: setup.voice.language.clone(),
            seen_privacy: k.seen_privacy,
        }
    }

    /// As config.toml's keys: the TTS model and the voice only when they
    /// are not what unset gives.
    fn keys(&self, setup: &Setup) -> VoiceModeKeys {
        let auto = setup.tts_name(&VoiceModeKeys::default()).unwrap_or_default();
        VoiceModeKeys {
            listen: self.listen,
            tts_model: (!self.tts_model.is_empty() && self.tts_model != auto).then(|| self.tts_model.clone()),
            tts_voice: (!self.voice.is_empty() && self.voice != super::tts::DEFAULT_VOICE).then(|| self.voice.clone()),
            speed: self.speed,
            read_aloud: self.read_aloud,
            sounds: self.sounds,
            seen_privacy: self.seen_privacy,
        }
    }
}

fn home() -> bise_home::Home {
    bise_home::Home::from_env()
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

/// Read at each use, so a config edit applies at once.
pub fn load() -> VoiceModeConfig {
    let text = std::fs::read_to_string(home().config_file()).ok();
    VoiceModeConfig::of_text(text.as_deref(), &env)
}

/// Write what changed since config.toml's text (its other lines as they
/// were); the keys left as they were stay unwritten.
pub fn save(cfg: &VoiceModeConfig) -> Result<(), String> {
    let path = home().config_file();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let new = with_config(&text, cfg, &env);
    if new == text {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, new).map_err(|e| format!("{}: {}", path.display(), e))
}

/// config.toml's `text` with `cfg` written in `[voice]`: only the keys
/// whose value changed; the rest of the file as it was.
pub fn with_config(text: &str, cfg: &VoiceModeConfig, env: &dyn Fn(&str) -> Option<String>) -> String {
    let setup = Setup::from_text(Some(text), env);
    let before = VoiceModeKeys::from_text(Some(text));
    let after = cfg.keys(&setup);
    let mut t = text.to_string();
    for ((key, old), (_, new)) in before.written().into_iter().zip(after.written()) {
        if old == new {
            continue;
        }
        t = match new {
            Some(v) => with_table_key(&t, "voice", key, &v),
            None => without_key(&t, "voice", key),
        };
    }
    if cfg.language != setup.voice.language {
        t = match &cfg.language {
            Some(l) => with_table_key(&t, "voice", "language", &bise_catalog::toml_string(l)),
            None => without_key(&t, "voice", "language"),
        };
    }
    t
}

// ---- the jobs ----

/// The setup and the keys as the TUI reads them.
fn with_keys<T>(f: impl FnOnce(&Setup, &Keys) -> T) -> T {
    let home = home();
    let setup = Setup::load(&home.config_file());
    let store = Store::read(&home.auth_file()).unwrap_or_default();
    let files = EnvFile::read_all(&home.env_files());
    let keys = Keys { env: &env, store: &store, files: &files };
    f(&setup, &keys)
}

fn endpoint(c: ModeCall) -> Endpoint {
    Endpoint { name: c.name, provider_name: c.provider_name, base_url: c.base_url, model: c.model, key: c.key }
}

pub fn listen_job() -> Result<ListenJob, String> {
    with_keys(listen_job_of)
}

/// Realtime when the voice role's provider has it (Mistral), the voice
/// role's batch model always (the realtime fallback, and the only one
/// elsewhere).
pub fn listen_job_of(setup: &Setup, keys: &Keys) -> Result<ListenJob, String> {
    let batch = setup.voice_job(keys).map_err(|e| {
        if crate::voice::needs_key(setup, keys) {
            crate::voice::NEEDS_KEY.to_string()
        } else {
            e
        }
    })?;
    let realtime = setup.realtime_call(keys).ok().flatten().map(endpoint);
    Ok(ListenJob { realtime, batch })
}

pub fn say_job(cfg: &VoiceModeConfig) -> Result<SayJob, String> {
    with_keys(|s, k| say_job_of(s, k, cfg))
}

/// The TTS on `cfg.tts_model` (unset: the voice role's provider's).
pub fn say_job_of(setup: &Setup, keys: &Keys, cfg: &VoiceModeConfig) -> Result<SayJob, String> {
    let mode = cfg.keys(setup);
    let api = setup.tts_call(&mode, keys).map(endpoint)?;
    Ok(SayJob { api, voice: cfg.voice.clone(), speed: cfg.speed.clamp(SPEED_MIN, SPEED_MAX) })
}

pub fn ack_job() -> Result<AckJob, String> {
    with_keys(ack_job_of)
}

/// The small-jobs model (titles, summaries: `[roles] small`).
pub fn ack_job_of(setup: &Setup, keys: &Keys) -> Result<AckJob, String> {
    let (call, family) = setup.small_call(keys)?;
    Ok(AckJob { api: endpoint(call), family })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
