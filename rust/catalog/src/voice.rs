//! Speech to text (BISE-130): which model transcribes the voice input,
//! and everything its call needs.
//!
//! The providers and models are catalog entries like the chat ones: a
//! provider that transcribes has `stt = "<family>"` (its wire format,
//! one of [`STT_FAMILIES`]) and shares `base_url` / `key_env` with its
//! chat models; `kind = "stt"` marks a provider or a model that only
//! transcribes. Adding an OpenAI-compatible one is data:
//!
//! ```toml
//! [providers.fireworks]
//! stt = "openai"
//! [models."fireworks/whisper-v3"]
//! kind = "stt"
//! ```
//!
//! The user's choice, in config.toml (env `BISE_VOICE_MODEL` wins):
//!
//! ```toml
//! [voice]
//! model = "mistral/voxtral-mini-latest"
//! language = "fr"                       # optional; unset: detected
//! vocabulary = ["bise", "config.toml"]  # optional: words to spell right
//! ```

use crate::auth::Keys;
use crate::{split_name, Catalog, Known, Setup, CLI};

/// The speech-to-text wire families:
///   mistral    POST {base}/audio/transcriptions, multipart, context_bias
///   openai     POST {base}/audio/transcriptions, multipart, prompt
///              (OpenAI, Groq and the other compatible servers)
///   elevenlabs POST {base}/speech-to-text, multipart, keyterms
///   deepgram   POST {base}/listen?model=..., the WAV as the body, keyterm
pub const STT_FAMILIES: [&str; 4] = ["mistral", "openai", "elevenlabs", "deepgram"];

/// `[voice]` as written in config.toml.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceConfig {
    pub model: Option<String>,
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

impl VoiceConfig {
    /// Read `[voice]`; what is wrong becomes a warning.
    pub fn read(t: &toml::Table, warnings: &mut Vec<String>) -> VoiceConfig {
        let mut c = VoiceConfig::default();
        let Some(v) = t.get("voice") else { return c };
        let Some(v) = v.as_table() else {
            warnings.push("config.toml: voice: not a table ([voice] then model = ...)".into());
            return c;
        };
        let mut warn = |w: String| warnings.push(format!("config.toml: voice.{}", w));
        for (k, x) in v {
            let s = x.as_str().map(|s| s.trim().to_string());
            match k.as_str() {
                "model" => match s {
                    Some(m) if !m.is_empty() => c.model = Some(m),
                    _ => warn("model: a \"provider/model\" name".into()),
                },
                "language" => match s {
                    // "" or "auto": detected by the model
                    Some(l) if l.is_empty() || l.eq_ignore_ascii_case("auto") => c.language = None,
                    Some(l) => c.language = Some(l),
                    None => warn("language: a language code (\"fr\", \"en\") or \"auto\"".into()),
                },
                "vocabulary" => match vocabulary(x) {
                    Some(words) => c.vocabulary = words,
                    None => warn("vocabulary: a list of words, [\"bise\", \"config.toml\"]".into()),
                },
                other => warn(format!("{}: unknown key (model, language, vocabulary)", other)),
            }
        }
        c
    }
}

/// A list of strings, or one comma-separated string.
fn vocabulary(v: &toml::Value) -> Option<Vec<String>> {
    let words: Vec<String> = match v {
        toml::Value::String(s) => s.split(',').map(|w| w.trim().to_string()).collect(),
        toml::Value::Array(a) => a
            .iter()
            .map(|w| w.as_str().map(|s| s.trim().to_string()))
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    Some(words.into_iter().filter(|w| !w.is_empty()).collect())
}

/// The voice input's choices, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceSetup {
    /// canonical "provider/id"
    pub model: String,
    /// "BISE_VOICE_MODEL", "config" or "default"
    pub from: &'static str,
    /// None: the model detects it
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

impl VoiceSetup {
    /// model = BISE_VOICE_MODEL > `[voice] model` > default_voice_model
    pub fn of(c: &Catalog, cfg: VoiceConfig, env: &dyn Fn(&str) -> Option<String>) -> VoiceSetup {
        let (model, from) = match (env("BISE_VOICE_MODEL"), cfg.model) {
            (Some(m), _) => (m, "BISE_VOICE_MODEL"),
            (None, Some(m)) => (m, "config"),
            (None, None) => (c.default_voice_model.clone(), "default"),
        };
        VoiceSetup { model: c.canonical_stt(&model), from, language: cfg.language, vocabulary: cfg.vocabulary }
    }
}

/// A speech-to-text model name resolved (never the key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SttResolved {
    /// "provider/id"
    pub name: String,
    pub provider: String,
    pub id: String,
    /// the provider's STT family; "" = it does not transcribe
    pub api: String,
    pub base_url: String,
    pub key_env: String,
    pub needs: String,
    pub known: Known,
}

impl Catalog {
    /// A bare voice model name goes to the default voice model's provider.
    pub fn canonical_stt(&self, name: &str) -> String {
        let name = name.trim();
        if split_name(name).is_some() {
            return name.to_string();
        }
        let p = split_name(&self.default_voice_model).map(|(p, _)| p).unwrap_or("mistral");
        format!("{}/{}", p, name)
    }

    pub fn resolve_stt(&self, name: &str) -> SttResolved {
        let full = self.canonical_stt(name);
        let (pid, mid) = split_name(&full).unwrap_or(("", full.as_str()));
        let mut r = SttResolved {
            name: full.clone(),
            provider: pid.to_string(),
            id: mid.to_string(),
            api: String::new(),
            base_url: String::new(),
            key_env: String::new(),
            needs: String::new(),
            known: Known::NoProvider,
        };
        if let Some(p) = self.provider(pid) {
            r.api = p.stt.clone();
            r.base_url = p.base_url.clone();
            r.key_env = p.key_env.clone();
            r.needs = p.needs.clone();
            r.known = match self.model(&full) {
                Some(m) if m.stt => Known::Listed,
                _ => Known::Unlisted,
            };
        }
        r
    }

    /// The providers that transcribe, in catalog order.
    pub fn stt_providers(&self) -> impl Iterator<Item = &crate::Provider> {
        self.providers.iter().filter(|p| !p.stt.is_empty())
    }
}

/// Everything one transcription call needs. Debug never shows the key.
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceJob {
    /// "provider/id", for messages
    pub name: String,
    /// the provider's name ("Mistral") and where credit is added ("" =
    /// none): the lines of a failed transcription (BISE-298)
    pub provider_name: String,
    pub billing_url: String,
    /// one of [`STT_FAMILIES`]
    pub api: String,
    /// no trailing '/'
    pub base_url: String,
    /// the model id the API takes
    pub model: String,
    pub key: String,
    pub language: Option<String>,
    pub vocabulary: Vec<String>,
}

impl std::fmt::Debug for VoiceJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VoiceJob {{ name: {:?}, provider_name: {:?}, billing_url: {:?}, api: {:?}, base_url: {:?}, model: {:?}, key: <{} bytes>, language: {:?}, vocabulary: {:?} }}",
            self.name,
            self.provider_name,
            self.billing_url,
            self.api,
            self.base_url,
            self.model,
            self.key.len(),
            self.language,
            self.vocabulary
        )
    }
}

impl Setup {
    /// The voice model's call, with its key (the chat keys' resolution:
    /// env > auth.json > the old .env files). Err: one line saying what
    /// to do.
    pub fn voice_job(&self, keys: &Keys) -> Result<VoiceJob, String> {
        let r = self.catalog.resolve_stt(&self.voice.model);
        if r.known == Known::NoProvider {
            return Err(format!(
                "voice model {}: unknown provider '{}' ('{} models' lists the voice ones)",
                r.name, r.provider, CLI
            ));
        }
        if r.api.is_empty() {
            return Err(format!(
                "voice model {}: {} does not transcribe ('{} models' lists the voice ones)",
                r.name, r.provider, CLI
            ));
        }
        if !r.needs.is_empty() {
            return Err(format!("voice model {}: {} is not usable yet ({})", r.name, r.provider, r.needs));
        }
        if r.base_url.is_empty() {
            return Err(format!("voice model {}: [providers.{}] has no base_url", r.name, r.provider));
        }
        let key = if r.key_env.is_empty() {
            String::new()
        } else {
            match keys.find(&r.provider, &r.key_env) {
                Some(f) => f.key,
                None => {
                    return Err(format!(
                        "voice transcription needs an API key: set {} or run '{} login {}'",
                        r.key_env, CLI, r.provider
                    ))
                }
            }
        };
        let (provider_name, billing_url) = self
            .catalog
            .provider(&r.provider)
            .map(|p| (p.name.clone(), p.billing_url.clone()))
            .unwrap_or_default();
        Ok(VoiceJob {
            name: r.name,
            provider_name,
            billing_url,
            api: r.api,
            base_url: r.base_url,
            model: r.id,
            key,
            language: self.voice.language.clone(),
            vocabulary: self.voice.vocabulary.clone(),
        })
    }
}
