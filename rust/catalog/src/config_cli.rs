//! `bise config get|set <key> [value]` (BISE-273): config.toml's top-level
//! choices from a script or an install prompt, without an editor. Only
//! the keys below; the rest of the file is kept as it was (comments and
//! tables included).

use crate::auth::tilde;
use crate::auth_cli::Paths;
use crate::{toml_string, with_key, Known, Setup, CLI};
use bise_home::style::Style;

/// The keys `config` reads and writes, and what they hold.
pub const KEYS: [(&str, &str); 4] = [
    ("model", "the model main and new agents use: \"provider/model\""),
    ("agent_model", "the tasks' model when not main's: \"provider/model\""),
    ("small_model", "titles and summaries: \"provider/model\""),
    ("project_doc_fallback_filenames", "files read when a folder has no AGENTS.md, comma-separated (e.g. CLAUDE.md)"),
];

fn usage() -> String {
    let mut s = format!(
        "{cli} config: config.toml's top-level choices, without an editor\n\n  {cli} config get <key>\n  {cli} config set <key> <value>\n\nkeys\n",
        cli = CLI
    );
    for (k, what) in KEYS {
        s.push_str(&format!("  {:<32} {}\n", k, what));
    }
    s.push_str(&format!("\nsee also: {} models (the models)", CLI));
    s
}

/// The TOML value to write for `key`, or why `value` can't be one.
pub fn value_of(setup: &Setup, key: &str, value: &str) -> Result<(String, Vec<String>), String> {
    let v = value.trim();
    match key {
        "model" | "agent_model" | "small_model" => {
            if v.is_empty() || v.chars().any(|c| c.is_whitespace() || c.is_control()) || !v.contains('/') {
                return Err(format!("'{}' is not a model: \"provider/model\", e.g. anthropic/claude-sonnet-4-5", v));
            }
            let r = setup.catalog.resolve(v);
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
pub fn get(text: &str, key: &str) -> Option<String> {
    let doc: toml::Table = text.parse().ok()?;
    Some(match doc.get(key)? {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(","),
        v => v.to_string(),
    })
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
        ["get", key] if KEYS.iter().any(|(k, _)| k == key) => match get(&text, key) {
            Some(v) => {
                println!("{}", v);
                0
            }
            None => {
                eprintln!("{}", err.dim(&format!("{} is not set in {}", key, shown)));
                1
            }
        },
        ["set", key, value] => {
            let setup = Setup::from_text(Some(&text), &|_| None);
            let (v, notes) = match value_of(&setup, key, value) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("{}", err.fail(&e));
                    return 1;
                }
            };
            let new = with_key(&text, key, &v);
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
                println!("{}", out.ok(&format!("{} = {} in {}", key, v, shown)));
            } else {
                println!("{}", out.ok(&format!("{} = {} in {} already", key, v, shown)));
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
        assert_eq!(t.matches("model = \"anthropic/claude-sonnet-4-5\"").count(), 1, "{t}");
        assert!(t.contains("# mine") && t.contains("[voice]\nmodel = \"mistral/v\""), "{t}");
        let s = Setup::from_text(Some(&t), &|_| None);
        assert_eq!((s.model.as_str(), s.voice.model.as_str()), ("anthropic/claude-sonnet-4-5", "mistral/v"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
