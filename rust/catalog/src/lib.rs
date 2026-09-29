//! bise's own model catalog (BISE-142).
//!
//! The built-in list (`models.toml`, compiled in) merged with the user's
//! `config.toml`: `[providers.<id>]` and `[models."<provider>/<model>"]`
//! tables add or override entries key by key. No network. Any
//! `provider/model` name resolves, listed or not (its provider's
//! defaults); a name nothing knows still resolves (to an empty base URL),
//! so a model never stops bise from starting: the provider call says
//! what is wrong.
//!
//! The Bend runtime gets the merged catalog as one file
//! ([`Setup::handoff_toml`], the path in `BISE_MODELS_FILE`); the format
//! and the per-call rule are in projects/switchboard/docs/research/providers.md §7.

use std::path::{Path, PathBuf};

pub mod cli;

/// The built-in list, shipped in the binary.
pub const BUILTIN: &str = include_str!("../models.toml");

/// The wire families a provider may speak.
pub const FAMILIES: [&str; 5] = [
    "openai-chat",
    "anthropic",
    "openai-responses",
    "gemini",
    "bedrock-converse",
];

/// What a model can do, every field known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caps {
    pub context: u64,
    pub max_output: u64,
    pub vision: bool,
    pub reasoning: bool,
    pub tools: bool,
}

/// The defaults of a model nothing describes.
pub const DEFAULT_CAPS: Caps = Caps {
    context: 128_000,
    max_output: 16_384,
    vision: false,
    reasoning: false,
    tools: true,
};

/// Caps as written in a table: a missing field comes from the level below
/// (model -> provider -> [`DEFAULT_CAPS`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialCaps {
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub vision: Option<bool>,
    pub reasoning: Option<bool>,
    pub tools: Option<bool>,
}

impl PartialCaps {
    fn over(&self, base: &Caps) -> Caps {
        Caps {
            context: self.context.unwrap_or(base.context),
            max_output: self.max_output.unwrap_or(base.max_output),
            vision: self.vision.unwrap_or(base.vision),
            reasoning: self.reasoning.unwrap_or(base.reasoning),
            tools: self.tools.unwrap_or(base.tools),
        }
    }
    fn merge(&mut self, o: &PartialCaps) {
        self.context = o.context.or(self.context);
        self.max_output = o.max_output.or(self.max_output);
        self.vision = o.vision.or(self.vision);
        self.reasoning = o.reasoning.or(self.reasoning);
        self.tools = o.tools.or(self.tools);
    }
}

/// Where an entry was last set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Builtin,
    Config,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    /// the wire family, one of [`FAMILIES`]
    pub api: String,
    /// no trailing '/'; "" when unknown
    pub base_url: String,
    /// "" = no key needed
    pub key_env: String,
    /// "" = usable; else the issue that makes it usable ("BISE-149")
    pub needs: String,
    /// the defaults of its models
    pub caps: PartialCaps,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub provider: String,
    pub id: String,
    /// its own wire family when it differs from its provider's (a
    /// Responses-only model of a chat provider); None: the provider's
    pub api: Option<String>,
    pub caps: PartialCaps,
    pub source: Source,
}

impl Model {
    /// "provider/id"
    pub fn name(&self) -> String {
        format!("{}/{}", self.provider, self.id)
    }
}

/// How much the catalog knew about a resolved name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    /// the model is listed
    Listed,
    /// the provider is known, the model is not: its defaults
    Unlisted,
    /// neither: no base URL, the call will fail with a clear message
    NoProvider,
}

/// A model name resolved to everything a provider call needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// "provider/id"
    pub name: String,
    pub provider: String,
    pub id: String,
    pub api: String,
    pub base_url: String,
    pub key_env: String,
    pub needs: String,
    pub caps: Caps,
    pub known: Known,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    /// (alias, "provider/id"), in file order
    pub aliases: Vec<(String, String)>,
    /// "provider/id" when no model is configured
    pub default_model: String,
    /// what was ignored while reading, one line each
    pub warnings: Vec<String>,
}

/// Split "provider/id" at the first '/' (ids may hold '/').
pub fn split_name(name: &str) -> Option<(&str, &str)> {
    let (p, m) = name.split_once('/')?;
    (!p.is_empty() && !m.is_empty()).then_some((p, m))
}

/// A name without a provider, as the config had before BISE-142: the
/// old table sent `claude*` to the foundry proxy and anything else to
/// Mistral.
pub fn legacy_name(name: &str) -> String {
    if name.to_ascii_lowercase().starts_with("claude") {
        format!("foundry/{}", name)
    } else {
        format!("mistral/{}", name)
    }
}

impl Catalog {
    fn empty() -> Catalog {
        Catalog {
            providers: Vec::new(),
            models: Vec::new(),
            aliases: Vec::new(),
            default_model: String::new(),
            warnings: Vec::new(),
        }
    }

    /// The built-in list alone.
    pub fn builtin() -> Catalog {
        let mut c = Catalog::empty();
        match BUILTIN.parse::<toml::Table>() {
            Ok(t) => c.apply(&t, Source::Builtin, "models.toml"),
            // a broken models.toml is a build bug: the tests catch it
            Err(e) => c.warnings.push(format!("models.toml: {}", e)),
        }
        c
    }

    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn model(&self, name: &str) -> Option<&Model> {
        let (p, m) = split_name(name)?;
        self.models.iter().find(|x| x.provider == p && x.id == m)
    }

    /// The full name a config value means: an alias, then a
    /// "provider/id", then the legacy rule for a bare name.
    pub fn canonical(&self, name: &str) -> String {
        let name = name.trim();
        if let Some((_, to)) = self.aliases.iter().find(|(a, _)| a == name) {
            return to.clone();
        }
        if split_name(name).is_some() {
            name.to_string()
        } else {
            legacy_name(name)
        }
    }

    /// The caps of a provider's unlisted models.
    pub fn provider_caps(&self, p: &Provider) -> Caps {
        p.caps.over(&DEFAULT_CAPS)
    }

    /// Resolve any name. Never fails: see [`Known`].
    pub fn resolve(&self, name: &str) -> Resolved {
        let full = self.canonical(name);
        let (pid, mid) = split_name(&full).unwrap_or(("", full.as_str()));
        let (pid, mid) = (pid.to_string(), mid.to_string());
        let Some(p) = self.provider(&pid) else {
            return Resolved {
                name: full.clone(),
                provider: pid,
                id: mid,
                api: "openai-chat".into(),
                base_url: String::new(),
                key_env: String::new(),
                needs: String::new(),
                caps: DEFAULT_CAPS,
                known: Known::NoProvider,
            };
        };
        let base = self.provider_caps(p);
        let (caps, api, known) = match self.models.iter().find(|m| m.provider == pid && m.id == mid) {
            Some(m) => (
                m.caps.over(&base),
                m.api.clone().unwrap_or_else(|| p.api.clone()),
                Known::Listed,
            ),
            None => (base, p.api.clone(), Known::Unlisted),
        };
        Resolved {
            name: full.clone(),
            provider: pid,
            id: mid,
            api,
            base_url: p.base_url.clone(),
            key_env: p.key_env.clone(),
            needs: p.needs.clone(),
            caps,
            known,
        }
    }

    /// The context window of a model, in tokens (listed or its
    /// provider's default; the default one when nothing knows it).
    pub fn context_window(&self, name: &str) -> u64 {
        self.resolve(name).caps.context
    }

    /// Merge one TOML layer (the built-in list, then the config).
    fn apply(&mut self, t: &toml::Table, src: Source, file: &str) {
        let mut warnings: Vec<String> = Vec::new();
        let mut warn = |w: String| warnings.push(format!("{}: {}", file, w));
        let mut providers: Vec<(String, Provider)> = Vec::new();
        let mut models: Vec<Model> = Vec::new();
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut default_model = None;
        for (k, v) in t {
            match k.as_str() {
                "default_model" => match v.as_str() {
                    Some(s) if !s.trim().is_empty() => default_model = Some(s.trim().to_string()),
                    _ => warn("default_model: not a model name".into()),
                },
                "aliases" => match v.as_table() {
                    Some(a) => {
                        for (from, to) in a {
                            match to.as_str() {
                                Some(to) if split_name(to).is_some() => {
                                    aliases.push((from.clone(), to.to_string()))
                                }
                                _ => warn(format!("aliases.\"{}\": not a \"provider/model\" name", from)),
                            }
                        }
                    }
                    None => warn("aliases: not a table".into()),
                },
                "providers" => match v.as_table() {
                    Some(ps) => {
                        for (id, pv) in ps {
                            let Some(pt) = pv.as_table() else {
                                warn(format!("providers.{}: not a table", id));
                                continue;
                            };
                            let where_ = format!("providers.{}", id);
                            let mut p = self.provider(id).cloned().unwrap_or(Provider {
                                id: id.clone(),
                                name: id.clone(),
                                api: "openai-chat".into(),
                                base_url: String::new(),
                                key_env: String::new(),
                                needs: String::new(),
                                caps: PartialCaps::default(),
                                source: src,
                            });
                            p.source = src;
                            let mut caps = PartialCaps::default();
                            for (fk, fv) in pt {
                                let s = || fv.as_str().map(|x| x.trim().to_string());
                                match fk.as_str() {
                                    "name" => set_str(&mut p.name, s(), &where_, fk, &mut warn),
                                    "api" => match s() {
                                        Some(a) if FAMILIES.contains(&a.as_str()) => p.api = a,
                                        _ => warn(format!(
                                            "{}.api: one of {}",
                                            where_,
                                            FAMILIES.join(", ")
                                        )),
                                    },
                                    "base_url" => set_str(
                                        &mut p.base_url,
                                        s().map(|u| u.trim_end_matches('/').to_string()),
                                        &where_,
                                        fk,
                                        &mut warn,
                                    ),
                                    "key_env" => set_str(&mut p.key_env, s(), &where_, fk, &mut warn),
                                    "needs" => set_str(&mut p.needs, s(), &where_, fk, &mut warn),
                                    _ => cap_field(&mut caps, fk, fv, &where_, &mut warn),
                                }
                            }
                            p.caps.merge(&caps);
                            match providers.iter_mut().find(|(i, _)| i == id) {
                                Some(slot) => slot.1 = p,
                                None => providers.push((id.clone(), p)),
                            }
                        }
                    }
                    None => warn("providers: not a table".into()),
                },
                "models" => match v.as_table() {
                    Some(ms) => {
                        for (name, mv) in ms {
                            let where_ = format!("models.\"{}\"", name);
                            let Some((pid, mid)) = split_name(name) else {
                                warn(format!("{}: the name must be \"provider/model\"", where_));
                                continue;
                            };
                            let Some(mt) = mv.as_table() else {
                                warn(format!("{}: not a table", where_));
                                continue;
                            };
                            let mut caps = PartialCaps::default();
                            let mut api = None;
                            for (fk, fv) in mt {
                                if fk == "api" {
                                    match fv.as_str().map(str::trim) {
                                        Some(a) if FAMILIES.contains(&a) => api = Some(a.to_string()),
                                        _ => warn(format!("{}.api: one of {}", where_, FAMILIES.join(", "))),
                                    }
                                } else {
                                    cap_field(&mut caps, fk, fv, &where_, &mut warn);
                                }
                            }
                            let mut m = self.model(name).cloned().unwrap_or(Model {
                                provider: pid.to_string(),
                                id: mid.to_string(),
                                api: None,
                                caps: PartialCaps::default(),
                                source: src,
                            });
                            m.source = src;
                            m.api = api.or(m.api);
                            m.caps.merge(&caps);
                            models.push(m);
                        }
                    }
                    None => warn("models: not a table".into()),
                },
                "provider" if src == Source::Config => {
                    warn("[provider.<id>] is not read: write [providers.<id>]".into())
                }
                "model" if src == Source::Builtin => {}
                _ => {} // the config's other keys (threshold, bg_after, ...)
            }
        }
        for (id, p) in providers {
            match self.providers.iter_mut().find(|x| x.id == id) {
                Some(slot) => *slot = p,
                None => self.providers.push(p),
            }
        }
        for m in models {
            match self
                .models
                .iter_mut()
                .find(|x| x.provider == m.provider && x.id == m.id)
            {
                Some(slot) => *slot = m,
                None => self.models.push(m),
            }
        }
        for (a, to) in aliases {
            self.aliases.retain(|(x, _)| *x != a);
            self.aliases.push((a, to));
        }
        if let Some(d) = default_model {
            self.default_model = d;
        }
        self.warnings.extend(warnings);
    }
}

fn set_str(slot: &mut String, v: Option<String>, where_: &str, k: &str, warn: &mut dyn FnMut(String)) {
    match v {
        Some(v) => *slot = v,
        None => warn(format!("{}.{}: not a string", where_, k)),
    }
}

fn cap_field(caps: &mut PartialCaps, k: &str, v: &toml::Value, where_: &str, warn: &mut dyn FnMut(String)) {
    let int = || v.as_integer().filter(|n| *n > 0).map(|n| n as u64);
    let bool_ = || v.as_bool();
    let bad = |what: &str| format!("{}.{}: {}", where_, k, what);
    match k {
        "context" => match int() {
            Some(n) => caps.context = Some(n),
            None => warn(bad("a number of tokens > 0")),
        },
        "max_output" => match int() {
            Some(n) => caps.max_output = Some(n),
            None => warn(bad("a number of tokens > 0")),
        },
        "vision" | "reasoning" | "tools" => match bool_() {
            Some(b) => match k {
                "vision" => caps.vision = Some(b),
                "reasoning" => caps.reasoning = Some(b),
                _ => caps.tools = Some(b),
            },
            None => warn(bad("true or false")),
        },
        _ => warn(format!("{}: unknown key {}", where_, k)),
    }
}

// ---- the config: the catalog + the models the agents use ----

/// The catalog merged with the config, and the two model choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub catalog: Catalog,
    /// the main agent's model, canonical "provider/id"
    pub model: String,
    /// the sub-agents' model, canonical; = model when unset
    pub agent_model: String,
    /// where each choice came from ("BISE_MODEL", "config", "default", ...)
    pub model_from: &'static str,
    pub agent_model_from: &'static str,
}

/// A string key of the config, even when the file is not valid TOML (the
/// Bend reader accepts bare words): `key = value # comment`.
fn loose_key(text: &str, key: &str) -> Option<String> {
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            return None; // top-level keys only
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        let v = match v.strip_prefix('"') {
            Some(rest) => rest.split('"').next().unwrap_or(""),
            None => v.split('#').next().unwrap_or("").trim(),
        };
        return (!v.is_empty()).then(|| v.to_string());
    }
    None
}

impl Setup {
    /// Precedence on each key: env > config > default.
    ///   model       = BISE_MODEL > BEND_MODEL > config `model` > default_model
    ///   agent_model = BISE_AGENT_MODEL > config `agent_model` > model
    /// `config` is the text of config.toml (None: no file). A broken file
    /// is a warning: its tables are ignored, `model`/`agent_model` lines
    /// are still read.
    pub fn from_text(config: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Setup {
        let mut catalog = Catalog::builtin();
        let (mut model_cfg, mut agent_cfg) = (None, None);
        if let Some(text) = config {
            match text.parse::<toml::Table>() {
                Ok(t) => {
                    catalog.apply(&t, Source::Config, "config.toml");
                    let s = |k: &str| {
                        t.get(k)
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    };
                    model_cfg = s("model");
                    agent_cfg = s("agent_model");
                }
                Err(e) => {
                    let first = e.to_string().lines().next().unwrap_or("").to_string();
                    catalog.warnings.push(format!(
                        "config.toml is not valid TOML ({}): its [providers] and [models] tables are ignored",
                        first
                    ));
                    model_cfg = loose_key(text, "model");
                    agent_cfg = loose_key(text, "agent_model");
                }
            }
        }
        let envv = |k: &str| env(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let (model, model_from) = if let Some(m) = envv("BISE_MODEL") {
            (m, "BISE_MODEL")
        } else if let Some(m) = envv("BEND_MODEL") {
            (m, "BEND_MODEL")
        } else if let Some(m) = model_cfg {
            (m, "config")
        } else {
            (catalog.default_model.clone(), "default")
        };
        let model = catalog.canonical(&model);
        let (agent_model, agent_model_from) = if let Some(m) = envv("BISE_AGENT_MODEL") {
            (catalog.canonical(&m), "BISE_AGENT_MODEL")
        } else if let Some(m) = agent_cfg {
            (catalog.canonical(&m), "config")
        } else {
            (model.clone(), "model")
        };
        Setup {
            catalog,
            model,
            agent_model,
            model_from,
            agent_model_from,
        }
    }

    /// Read `config` (a missing file is no config) with the real env.
    pub fn load(config: &Path) -> Setup {
        let text = std::fs::read_to_string(config).ok();
        Setup::from_text(text.as_deref(), &|k| std::env::var(k).ok())
    }

    /// The model of a role: "main" or "agent".
    pub fn model_for(&self, role: &str) -> Resolved {
        match role {
            "agent" => self.catalog.resolve(&self.agent_model),
            _ => self.catalog.resolve(&self.model),
        }
    }

    /// The hand-off to the Bend runtime: the merged catalog in the
    /// config's own format, so core/config.bend reads it (a key's path
    /// keeps the quotes: `models."openai/gpt-5".context`). A provider has
    /// every key; a model only the keys that differ from its provider's
    /// (the lookup is per key: the model's, else its provider's). The
    /// model choices are not in it: the runtime reads `model` and
    /// `agent_model` from config.toml on each call (providers.md §7).
    pub fn handoff_toml(&self) -> String {
        let c = &self.catalog;
        let mut o = String::new();
        o.push_str("# bise's model catalog, merged with config.toml. Written by bise at start\n");
        o.push_str("# for the Bend runtime (BISE_MODELS_FILE); do not edit: edit config.toml.\n");
        o.push_str("version = 1\n");
        o.push_str(&format!("default_model = {}\n", q(&c.default_model)));
        o.push_str("\n[aliases]\n");
        for (a, to) in &c.aliases {
            o.push_str(&format!("{} = {}\n", q(a), q(to)));
        }
        for p in &c.providers {
            o.push_str(&format!("\n[providers.{}]\n", key(&p.id)));
            o.push_str(&format!("name = {}\n", q(&p.name)));
            o.push_str(&format!("api = {}\n", q(&p.api)));
            o.push_str(&format!("base_url = {}\n", q(&p.base_url)));
            o.push_str(&format!("key_env = {}\n", q(&p.key_env)));
            o.push_str(&format!("needs = {}\n", q(&p.needs)));
            caps_lines(&mut o, &c.provider_caps(p));
        }
        for m in &c.models {
            let r = c.resolve(&m.name());
            o.push_str(&format!("\n[models.{}]\n", q(&m.name())));
            let Some(p) = c.provider(&m.provider) else {
                caps_lines(&mut o, &r.caps); // no provider to fall back to
                continue;
            };
            if r.api != p.api {
                o.push_str(&format!("api = {}\n", q(&r.api)));
            }
            let base = c.provider_caps(p);
            let (a, b) = (&r.caps, &base);
            if a.context != b.context {
                o.push_str(&format!("context = {}\n", a.context));
            }
            if a.max_output != b.max_output {
                o.push_str(&format!("max_output = {}\n", a.max_output));
            }
            for (k, x, y) in [
                ("vision", a.vision, b.vision),
                ("reasoning", a.reasoning, b.reasoning),
                ("tools", a.tools, b.tools),
            ] {
                if x != y {
                    o.push_str(&format!("{} = {}\n", k, x));
                }
            }
        }
        o
    }

    /// Write the hand-off file atomically (temp file + rename).
    pub fn write_handoff(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp: PathBuf = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        std::fs::write(&tmp, self.handoff_toml())?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

fn caps_lines(o: &mut String, c: &Caps) {
    o.push_str(&format!(
        "context = {}\nmax_output = {}\nvision = {}\nreasoning = {}\ntools = {}\n",
        c.context, c.max_output, c.vision, c.reasoning, c.tools
    ));
}

/// A TOML basic string.
fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A bare key when it can be one, else quoted.
fn key(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        s.to_string()
    } else {
        q(s)
    }
}

/// Write the hand-off where the runtime will find it and return its
/// path: `<cache_dir>/models.toml`, else the temp dir. None when neither
/// is writable (the runtime then keeps its old table).
pub fn export_handoff(config: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let setup = Setup::load(config);
    let fallback = std::env::temp_dir().join(format!("bise-models-{}.toml", std::process::id()));
    [cache_dir.join("models.toml"), fallback]
        .into_iter()
        .find(|p| setup.write_handoff(p).is_ok())
}

#[cfg(test)]
mod tests;
