//! Model roles (BISE-298): which model does what. One table, [`ROLES`]:
//! a new role is one row and its words. Each role has a model
//! ("provider/id") and, for a chat role, a reasoning effort; unset, it
//! follows another role or the catalog.
//!
//! config.toml (a role is a string, or a table with its settings):
//!
//! ```toml
//! [roles]
//! main = "anthropic/claude-opus-5-5"   # your team lead
//! agents = "openai/gpt-6-luna"          # the agents main starts (unset: main's)
//! small = "mistral/mistral-small-latest"  # small jobs: titles, summaries
//! voice = "mistral/voxtral-mini-latest" # listens when you talk (ctrl+r)
//!
//! [roles.main]                          # the table form, instead of the line
//! model = "anthropic/claude-opus-5-5"
//! effort = "high"
//! ```
//!
//! Each role, first found wins: its env var(s) > `[roles]` > its old key
//! (`model`, `agent_model`, `small_model`, `[voice] model`; efforts
//! `reasoning_effort`, `agent_reasoning_effort`) > its fallback. The old
//! keys keep working; bise writes the new shape ([`with_role`]) and drops
//! the old key it replaces, the rest of the file as it was.

use crate::voice::VoiceConfig;

/// What a role's model does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// a chat model (the catalog's chat entries)
    Chat,
    /// a speech-to-text model (`kind = "stt"`)
    Voice,
}

/// One role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Role {
    /// its key under `[roles]`
    pub id: &'static str,
    /// the user's word for it (screens, `/provider`'s tags, doctor)
    pub name: &'static str,
    /// what it does: always shown next to [`Role::name`]
    pub about: &'static str,
    pub kind: Kind,
    /// the env vars that set it, the first set wins
    pub env: &'static [&'static str],
    /// the key it had before roles ("" none; "voice.model": a key of a
    /// table)
    pub old_key: &'static str,
    /// the old key of its effort ("" none)
    pub old_effort: &'static str,
    /// in the screens (a declared role waits for its feature)
    pub shown: bool,
}

pub const MAIN: &str = "main";
pub const AGENTS: &str = "agents";
pub const SMALL: &str = "small";
pub const VOICE: &str = "voice";
pub const CLASSIFY: &str = "classify";

/// Every role, in the order the screens list them.
pub const ROLES: &[Role] = &[
    Role {
        id: MAIN,
        name: "main",
        about: "your team lead",
        kind: Kind::Chat,
        env: &["BISE_MODEL", "BEND_MODEL"],
        old_key: "model",
        old_effort: "reasoning_effort",
        shown: true,
    },
    Role {
        id: AGENTS,
        name: "agents",
        about: "the agents main starts",
        kind: Kind::Chat,
        env: &["BISE_AGENT_MODEL"],
        old_key: "agent_model",
        old_effort: "agent_reasoning_effort",
        shown: true,
    },
    Role {
        id: SMALL,
        name: "small jobs",
        about: "titles, summaries",
        kind: Kind::Chat,
        env: &["BISE_SMALL_MODEL"],
        old_key: "small_model",
        old_effort: "",
        shown: true,
    },
    Role {
        id: VOICE,
        name: "voice",
        about: "listens when you talk",
        kind: Kind::Voice,
        env: &["BISE_VOICE_MODEL"],
        old_key: "voice.model",
        old_effort: "",
        shown: true,
    },
    // the auto-confirm of tool calls: declared, no feature yet
    Role {
        id: CLASSIFY,
        name: "auto-confirm",
        about: "sorts tool calls",
        kind: Kind::Chat,
        env: &["BISE_CLASSIFY_MODEL"],
        old_key: "",
        old_effort: "",
        shown: false,
    },
];

pub fn role(id: &str) -> Option<&'static Role> {
    ROLES.iter().find(|r| r.id == id)
}

impl Role {
    /// `small jobs (titles, summaries)`: the name with what it does.
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.about)
    }
}

/// A chat role as config.toml's `[roles]` writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoleConfig {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// `[roles]` read: the chat roles by id, the voice role's settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RolesConfig {
    pub chat: Vec<(&'static str, RoleConfig)>,
    /// `voice = "…"` or `[roles.voice]` (model, language, vocabulary);
    /// None: not in `[roles]`
    pub voice: Option<VoiceConfig>,
}

impl RolesConfig {
    pub fn get(&self, id: &str) -> RoleConfig {
        self.chat.iter().find(|(r, _)| *r == id).map(|(_, c)| c.clone()).unwrap_or_default()
    }

    /// Read `[roles]`; what is wrong becomes a warning.
    pub fn read(t: &toml::Table, warnings: &mut Vec<String>) -> RolesConfig {
        let mut c = RolesConfig::default();
        let Some(v) = t.get("roles") else { return c };
        let Some(v) = v.as_table() else {
            warnings.push("config.toml: roles: not a table ([roles] then main = \"provider/model\")".into());
            return c;
        };
        let ids = || ROLES.iter().map(|r| r.id).collect::<Vec<_>>().join(", ");
        for (k, x) in v {
            let Some(r) = role(k) else {
                warnings.push(format!("config.toml: roles.{}: unknown role ({})", k, ids()));
                continue;
            };
            if r.kind == Kind::Voice {
                let mut t = toml::Table::new();
                let v = match x {
                    toml::Value::String(s) => {
                        let mut m = toml::Table::new();
                        m.insert("model".into(), toml::Value::String(s.clone()));
                        toml::Value::Table(m)
                    }
                    other => other.clone(),
                };
                t.insert("voice".into(), v);
                let mut w = Vec::new();
                let vc = VoiceConfig::read(&t, &mut w);
                warnings.extend(w.into_iter().map(|w| w.replacen("config.toml: voice", "config.toml: roles.voice", 1)));
                c.voice = Some(vc);
                continue;
            }
            let mut rc = RoleConfig::default();
            let s = |x: &toml::Value| x.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            match x {
                toml::Value::String(_) => match s(x) {
                    Some(m) => rc.model = Some(m),
                    None => warnings.push(format!("config.toml: roles.{}: a \"provider/model\" name", k)),
                },
                toml::Value::Table(tb) => {
                    for (fk, fv) in tb {
                        match (fk.as_str(), s(fv)) {
                            ("model", Some(m)) => rc.model = Some(m),
                            ("effort", Some(e)) => rc.effort = Some(e),
                            ("model" | "effort", None) => {
                                warnings.push(format!("config.toml: roles.{}.{}: a non-empty string", k, fk))
                            }
                            _ => warnings.push(format!("config.toml: roles.{}.{}: unknown key (model, effort)", k, fk)),
                        }
                    }
                }
                _ => warnings.push(format!("config.toml: roles.{}: a \"provider/model\" name or a table", k)),
            }
            c.chat.push((r.id, rc));
        }
        c
    }
}

// ---- writing config.toml ----

/// The table a header line opens (`[roles]` → "roles", `[roles.voice] #
/// x` → "roles.voice"); None: not a header (an array of tables too).
fn header(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('[') || t.starts_with("[[") {
        return None;
    }
    let end = t.find(']')?;
    Some(t[1..end].split('.').map(|p| p.trim().trim_matches('"')).collect::<Vec<_>>().join("."))
}

/// The key a line sets (`model = "x"` → "model"), comments aside.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim().trim_matches('"'))
}

/// `text` without the line setting `key` in `table` ("" = top level).
pub fn without_key(text: &str, table: &str, key: &str) -> String {
    let mut cur = String::new();
    let mut out: Vec<&str> = Vec::new();
    for l in text.lines() {
        if let Some(h) = header(l) {
            cur = h;
        } else if cur == table && key_of(l) == Some(key) {
            continue;
        }
        out.push(l);
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// `text` with `key = value` (`value` already TOML) in `table`: its line
/// replaced where it is, else added at the end of the table, else a new
/// table at the end of the file.
pub fn with_table_key(text: &str, table: &str, key: &str, value: &str) -> String {
    let line = format!("{} = {}", key, value);
    let lines: Vec<&str> = text.lines().collect();
    let mut cur = String::new();
    // the table's last non-blank line, and the line of the key
    let (mut last, mut at) = (None, None);
    for (i, l) in lines.iter().enumerate() {
        if let Some(h) = header(l) {
            cur = h;
            if cur == table {
                last = Some(i);
            }
            continue;
        }
        if cur == table {
            if key_of(l) == Some(key) && at.is_none() {
                at = Some(i);
            }
            if !l.trim().is_empty() {
                last = Some(i);
            }
        }
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match (at, last) {
        (Some(i), _) => out[i] = line,
        (None, Some(i)) => out.insert(i + 1, line),
        (None, None) => {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{}]", table));
            out.push(line);
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// config.toml's text with role `id`'s model set to `model` (BISE-298):
/// in `[roles.<id>]` when that table is there, else `<id> = "…"` in
/// `[roles]`; the old key it replaces goes. The rest of the file as it
/// was.
pub fn with_role(text: &str, id: &str, model: &str) -> String {
    let value = crate::toml_string(model);
    let mut t = text.to_string();
    if let Some(r) = role(id) {
        match r.old_key.split_once('.') {
            Some((table, key)) => t = without_key(&t, table, key),
            None if !r.old_key.is_empty() => t = without_key(&t, "", r.old_key),
            None => {}
        }
    }
    let own = format!("roles.{}", id);
    if t.lines().any(|l| header(l).as_deref() == Some(own.as_str())) {
        with_table_key(&t, &own, "model", &value)
    } else {
        with_table_key(&t, "roles", id, &value)
    }
}

/// Where a role's model comes from, in the user's words, for the
/// screens: picked (in config.toml or an env var) or following another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// set in config.toml
    Picked,
    /// an env var sets it (its name)
    Env(&'static str),
    /// unset: the model of that role ("main", "agents")
    SameAs(&'static str),
    /// unset: chosen by bise (a provider's small model, the default
    /// voice model)
    Auto,
    /// none at all
    None,
}

impl Source {
    /// A `Setup` "from" label as a [`Source`].
    pub fn of(from: &'static str) -> Source {
        match from {
            "config" => Source::Picked,
            "model" => Source::SameAs(MAIN),
            "agent_model" => Source::SameAs(AGENTS),
            "small_model" => Source::SameAs(SMALL),
            "provider" | "default" => Source::Auto,
            "none" | "" => Source::None,
            env => Source::Env(env),
        }
    }
}
