//! `bise config get|set <key> [value]` (BISE-273): config.toml's top-level
//! choices from a script or an install prompt, without an editor. Only
//! the keys below; the rest of the file is kept as it was (comments and
//! tables included).

use crate::auth::tilde;
use crate::auth_cli::Paths;
use crate::{toml_string, with_key, Known, Setup, CLI};
use bise_home::style::Style;

/// The keys `config` reads and writes, and what they hold. The model
/// roles (BISE-298) live in `[roles]`; their UI word comes first.
pub const KEYS: [(&str, &str); 5] = [
    ("main", "your team lead"),
    ("agents", "the agents main starts. unset: same as main"),
    ("small", "small jobs: titles and summaries. unset: auto"),
    ("voice", "listens when you talk (ctrl+r)"),
    ("project_doc_fallback_filenames", "files read when a folder has no AGENTS.md, comma-separated (e.g. CLAUDE.md)"),
];

/// The keys before roles (BISE-298): still taken, they read and write
/// their role.
pub const OLD_KEYS: [(&str, &str); 3] = [("model", "main"), ("agent_model", "agents"), ("small_model", "small")];

/// A key as `config` takes it: a role id for a model key (old names
/// too), else the key.
fn role_key(key: &str) -> &str {
    OLD_KEYS.iter().find(|(o, _)| *o == key).map_or(key, |(_, r)| r)
}

fn is_role(key: &str) -> bool {
    crate::roles::role(role_key(key)).is_some_and(|r| r.shown)
}

fn usage() -> String {
    let mut s = format!(
        "{cli} config: config.toml's top-level choices, without an editor\n\n  {cli} config get <key>\n  {cli} config set <key> <value>\n\nkeys (models are \"provider/model\")\n",
        cli = CLI
    );
    // the roles' column; a longer key takes its own room (designer)
    let w = KEYS.iter().map(|(k, _)| k.len()).filter(|n| *n < 12).max().unwrap_or(0) + 3;
    for (k, what) in KEYS {
        let w = w.max(k.len() + 3);
        s.push_str(&format!("  {:<w$}{}\n", k, what, w = w));
    }
    s.push_str(&format!("\nsee also: {} models (the models)", CLI));
    s
}

/// The TOML value to write for `key`, or why `value` can't be one.
pub fn value_of(setup: &Setup, key: &str, value: &str) -> Result<(String, Vec<String>), String> {
    let v = value.trim();
    match role_key(key) {
        k if is_role(k) => {
            if v.is_empty() || v.chars().any(|c| c.is_whitespace() || c.is_control()) || !v.contains('/') {
                return Err(format!("'{}' is not a model: \"provider/model\", e.g. anthropic/claude-sonnet-4-5", v));
            }
            let voice = k == crate::roles::VOICE;
            let r = if voice {
                let s = setup.catalog.resolve_stt(v);
                crate::Resolved { name: s.name, provider: s.provider, known: s.known, needs: s.needs, ..setup.catalog.resolve(v) }
            } else {
                setup.catalog.resolve(v)
            };
            let mut notes = Vec::new();
            match r.known {
                Known::NoProvider => {
                    return Err(format!(
                        "unknown provider '{}': '{} models' lists them; a custom one goes in config.toml as [providers.{}]",
                        r.provider, CLI, r.provider
                    ))
                }
                Known::Unlisted => notes.push(format!("{} is not in bise's list: {}'s defaults apply", v, r.provider)),
                Known::Listed => {}
            }
            if !r.needs.is_empty() {
                notes.push(format!("{} is not usable yet ({})", r.provider, r.needs));
            }
            Ok((toml_string(v), notes))
        }
        "project_doc_fallback_filenames" => {
            let names: Vec<&str> = v.split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
            if let Some(bad) = names.iter().find(|n| n.contains('/') || n.contains('\\') || **n == "..") {
                return Err(format!("'{}' is not a file name (no folders)", bad));
            }
            let list: Vec<String> = names.iter().map(|n| toml_string(n)).collect();
            Ok((format!("[{}]", list.join(", ")), Vec::new()))
        }
        k => Err(format!("unknown key '{}'\n{}", k, usage())),
    }
}

/// The value of `key` as the file has it (the TOML text), None when unset.
/// A role (BISE-298): `[roles]`'s line or table, else its old key.
pub fn get(text: &str, key: &str) -> Option<String> {
    let doc: toml::Table = text.parse().ok()?;
    if is_role(key) {
        let id = role_key(key);
        let roles = doc.get("roles").and_then(|r| r.get(id));
        let own = roles.and_then(|v| v.as_str().or_else(|| v.get("model").and_then(|m| m.as_str())));
        let r = crate::roles::role(id)?;
        let old = match r.old_key.split_once('.') {
            Some((t, k)) => doc.get(t).and_then(|x| x.get(k)).and_then(|x| x.as_str()),
            None => doc.get(r.old_key).and_then(|x| x.as_str()),
        };
        return own.or(old).map(str::to_string);
    }
    Some(match doc.get(key)? {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(","),
        v => v.to_string(),
    })
}

/// The note for a model whose provider has no key yet (the environment,
/// auth.json, the old .env files): how to set it up. None when it has
/// one or needs none.
pub fn no_key_note(setup: &Setup, model: &str, paths: &Paths) -> Option<String> {
    use crate::auth::{EnvFile, Keys, Store};
    let r = setup.catalog.resolve(model.trim());
    let p = setup.catalog.provider(&r.provider)?;
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let files = EnvFile::read_all(&paths.env_files);
    let env = |k: &str| std::env::var(k).ok();
    let keys = Keys { env: &env, store: &store, files: &files };
    (p.needs.is_empty() && !keys.ready(p))
        .then(|| format!("no {} key yet: '{} login {}' sets it up (in bise: /provider)", p.name, CLI, p.id))
}

/// A key as config.toml has it: `[roles] main` for a role.
fn shown_key(key: &str) -> String {
    if is_role(key) {
        format!("[roles] {}", role_key(key))
    } else {
        key.to_string()
    }
}

/// `bise config …`; returns the exit code.
pub fn main(args: &[String], paths: &Paths) -> i32 {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let text = std::fs::read_to_string(&paths.config).unwrap_or_default();
    let shown = tilde(&paths.config, paths.home.as_deref());
    let (out, err) = (Style::stdout(), Style::stderr());
    match a.as_slice() {
        ["-h" | "--help" | "help"] | [] => {
            println!("{}", usage());
            i32::from(a.is_empty()) * 2
        }
        ["get", key] if KEYS.iter().any(|(k, _)| k == key) || is_role(key) => match get(&text, key) {
            Some(v) => {
                println!("{}", v);
                0
            }
            // BISE-298: an unset role says what runs instead
            None if is_role(key) => {
                let id = role_key(key);
                let setup = Setup::from_text(Some(&text), &|k| std::env::var(k).ok());
                let (m, from) = setup.role_model(id);
                let rule = match from {
                    crate::roles::Source::SameAs(of) => format!("same as {}", crate::roles::role(of).map_or(of, |r| r.name)),
                    crate::roles::Source::Env(n) => format!("{} sets it", n),
                    crate::roles::Source::None => "none yet".into(),
                    _ => "auto".into(),
                };
                let name = crate::roles::role(id).map_or(id, |r| r.name);
                let who = if name == id { id.to_string() } else { format!("{} ({})", id, name) };
                let what = if m.is_empty() || from == crate::roles::Source::None { rule } else { format!("{} · {}", rule, m) };
                eprintln!("{}", err.dim(&format!("{} is not set: {}", who, what)));
                1
            }
            None => {
                eprintln!("{}", err.dim(&format!("{} is not set in {}", key, shown)));
                1
            }
        },
        ["set", key, value] => {
            let setup = Setup::from_text(Some(&text), &|_| None);
            let (v, mut notes) = match value_of(&setup, key, value) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("{}", err.fail(&e));
                    return 1;
                }
            };
            // BISE-294: a model whose provider has no key can't run yet
            if is_role(key) {
                if let Some(n) = no_key_note(&setup, value, paths) {
                    notes.push(n);
                }
            }
            // BISE-298: a role goes in [roles], its old key out
            let new = if is_role(key) { crate::roles::with_role(&text, role_key(key), value.trim()) } else { with_key(&text, key, &v) };
            if new != text {
                if let Some(d) = paths.config.parent() {
                    if let Err(e) = std::fs::create_dir_all(d) {
                        eprintln!("{}", err.fail(&format!("cannot create {}: {}", d.display(), e)));
                        return 1;
                    }
                }
                if new.parse::<toml::Table>().is_err() && text.parse::<toml::Table>().is_ok() {
                    eprintln!("{}", err.fail(&format!("the change would break {}: nothing written", shown)));
                    return 1;
                }
                if let Err(e) = std::fs::write(&paths.config, &new) {
                    eprintln!("{}", err.fail(&format!("cannot write {}: {}", shown, e)));
                    return 1;
                }
                println!("{}", out.ok(&format!("{} = {} in {}", shown_key(key), v, shown)));
            } else {
                println!("{}", out.ok(&format!("{} = {} in {} already", shown_key(key), v, shown)));
            }
            for n in notes {
                println!("{}", out.ask(&n));
            }
            0
        }
        _ => {
            eprintln!("{}", usage());
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Setup {
        Setup::from_text(None, &|_| None)
    }

    #[test]
    fn models_are_checked_against_the_catalog() {
        let s = setup();
        let (v, notes) = value_of(&s, "model", "anthropic/claude-sonnet-4-5").unwrap();
        assert_eq!(v, "\"anthropic/claude-sonnet-4-5\"");
        assert!(notes.iter().all(|n| !n.contains("not usable")), "{notes:?}");
        assert!(value_of(&s, "model", "nope/x").unwrap_err().contains("unknown provider 'nope'"));
        assert!(value_of(&s, "model", "gpt").is_err());
        assert!(value_of(&s, "model", "a b/c").is_err());
        assert!(value_of(&s, "colour", "x").unwrap_err().contains("unknown key"));
    }

    #[test]
    fn fallback_names_become_a_toml_list() {
        let s = setup();
        assert_eq!(value_of(&s, "project_doc_fallback_filenames", "CLAUDE.md, .cursorrules").unwrap().0, "[\"CLAUDE.md\", \".cursorrules\"]");
        assert!(value_of(&s, "project_doc_fallback_filenames", "../x").is_err());
        let t = with_key("model = \"a/b\"\n", "project_doc_fallback_filenames", "[\"CLAUDE.md\"]");
        assert_eq!(get(&t, "project_doc_fallback_filenames").as_deref(), Some("CLAUDE.md"));
        assert_eq!(get(&t, "model").as_deref(), Some("a/b"));
        assert_eq!(get(&t, "main").as_deref(), Some("a/b"), "a role reads its old key");
        assert_eq!(get(&t, "small_model"), None);
    }

    #[test]
    fn set_writes_once_and_keeps_the_rest() {
        let d = std::env::temp_dir().join(format!("bise-config-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let paths = Paths { auth_file: d.join("auth.json"), config: d.join("config.toml"), env_files: vec![], home: None };
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(&paths.config, "# mine\n[voice]\nmodel = \"mistral/v\"\n").unwrap();
        let set = |k: &str, v: &str| main(&["set".into(), k.into(), v.into()], &paths);
        assert_eq!(set("model", "anthropic/claude-sonnet-4-5"), 0);
        assert_eq!(set("model", "anthropic/claude-sonnet-4-5"), 0);
        assert_eq!(set("model", "nope/x"), 1);
        let t = std::fs::read_to_string(&paths.config).unwrap();
        // BISE-298: the old key writes its role
        assert_eq!(t.matches("main = \"anthropic/claude-sonnet-4-5\"").count(), 1, "{t}");
        assert!(t.contains("# mine") && t.contains("[voice]\nmodel = \"mistral/v\""), "{t}");
        let s = Setup::from_text(Some(&t), &|_| None);
        assert_eq!((s.model.as_str(), s.voice.model.as_str()), ("anthropic/claude-sonnet-4-5", "mistral/v"));
        // the roles by their names; voice takes a voice model
        assert_eq!(set("voice", "openai/gpt-transcribe"), 0);
        assert_eq!(set("agents", "openai/gpt-6-luna"), 0);
        let t = std::fs::read_to_string(&paths.config).unwrap();
        assert!(t.contains("voice = \"openai/gpt-transcribe\"") && !t.contains("model = \"mistral/v\""), "{t}");
        assert_eq!(get(&t, "agents").as_deref(), Some("openai/gpt-6-luna"));
        assert_eq!(get(&t, "voice").as_deref(), Some("openai/gpt-transcribe"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
