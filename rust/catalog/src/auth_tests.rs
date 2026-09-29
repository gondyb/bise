//! BISE-143: the key store, the resolution order, the commands' output.
//! No test prints a key: the asserts check that none shows up.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::auth::{EnvFile, From, Keys, Store};
use crate::auth_cli::{self, Paths};
use crate::{Catalog, Setup};

const SECRET: &str = "sk-test-SECRET-0123456789";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-auth-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn paths(dir: &Path) -> Paths {
    Paths {
        auth_file: dir.join("home/.bise/auth.json"),
        config: dir.join("none.toml"),
        env_files: vec![dir.join("home/.bend-harness/.env"), dir.join("home/.vibe/.env")],
        home: Some(dir.join("home")),
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn login_writes_a_0600_file_in_a_0700_dir_and_logout_removes_the_key() {
    let dir = tmp("login");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    let p = c.provider("openai").unwrap();
    let out = auth_cli::login(&ps, p, &format!("  {}\n", SECRET), &no_env).unwrap();
    assert!(out[0].contains("saved the OpenAI key in ~/.bise/auth.json"), "{out:?}");
    assert!(out.iter().all(|l| !l.contains(SECRET)), "{out:?}");
    let f = &ps.auth_file;
    assert_eq!(mode(f), 0o600);
    assert_eq!(mode(f.parent().unwrap()), 0o700);
    assert_eq!(mode(&dir.join("home")), 0o700, "a parent created here is private too");
    let store = Store::read(f).unwrap();
    assert_eq!(store.key("openai"), Some(SECRET), "trimmed");
    // a second provider keeps the first; no temp file left behind
    auth_cli::login(&ps, c.provider("groq").unwrap(), "gsk-2", &no_env).unwrap();
    let store = Store::read(f).unwrap();
    assert_eq!(store.providers(), vec!["groq", "openai"]);
    let left: Vec<_> = std::fs::read_dir(f.parent().unwrap()).unwrap().collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // logout
    let out = auth_cli::logout(&ps, "openai", &no_env, &c).unwrap();
    assert_eq!(out, vec!["removed the openai key".to_string()]);
    assert_eq!(Store::read(f).unwrap().key("openai"), None);
    assert_eq!(mode(f), 0o600);
    let e = auth_cli::logout(&ps, "openai", &no_env, &c).unwrap_err();
    assert!(e.contains("no key stored for 'openai'"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_dir_keeps_its_mode_and_a_loose_file_becomes_0600() {
    let dir = tmp("existing");
    let ps = paths(&dir);
    let d = ps.auth_file.parent().unwrap();
    std::fs::create_dir_all(d).unwrap();
    std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&ps.auth_file, "{\"x\": {\"type\": \"oauth\", \"refresh\": \"r\"}}").unwrap();
    std::fs::set_permissions(&ps.auth_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let c = Catalog::builtin();
    let keys = Keys { env: &no_env, store: &Store::read(&ps.auth_file).unwrap(), files: &[] };
    let list = auth_cli::render_list(&c, &keys, &ps);
    assert!(list.contains("readable by others (mode 644)"), "{list}");
    auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &no_env).unwrap();
    assert_eq!(mode(d), 0o755, "an existing dir is not chmod'ed");
    assert_eq!(mode(&ps.auth_file), 0o600);
    // an entry of another type is kept as it is
    let text = std::fs::read_to_string(&ps.auth_file).unwrap();
    assert!(text.contains("\"oauth\"") && text.contains("\"refresh\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_store_is_an_error_without_its_content_and_is_never_overwritten() {
    let dir = tmp("broken");
    let ps = paths(&dir);
    std::fs::create_dir_all(ps.auth_file.parent().unwrap()).unwrap();
    let text = format!("{{\"openai\": {{\"type\": \"api\", \"key\": \"{}\"", SECRET);
    std::fs::write(&ps.auth_file, &text).unwrap();
    let e = Store::read(&ps.auth_file).unwrap_err();
    assert!(e.contains("not valid JSON (line 1"), "{e}");
    assert!(!e.contains(SECRET), "{e}");
    let c = Catalog::builtin();
    let e = auth_cli::login(&ps, c.provider("openai").unwrap(), "new", &no_env).unwrap_err();
    assert!(!e.contains(SECRET), "{e}");
    assert_eq!(std::fs::read_to_string(&ps.auth_file).unwrap(), text);
    assert!(Store::parse("[1]").unwrap_err().contains("not a JSON object"));
    assert_eq!(Store::parse("").unwrap().providers().len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_keys_and_providers_are_refused_without_echoing_the_key() {
    let c = Catalog::builtin();
    assert!(auth_cli::clean_key("   \n").unwrap_err().contains("no key given"));
    let e = auth_cli::clean_key("sk-a b").unwrap_err();
    assert!(e.contains("space") && !e.contains("sk-a"), "{e}");
    assert!(auth_cli::clean_key("sk-\u{1b}[A").is_err());
    assert!(auth_cli::check_provider(&c, "nope").unwrap_err().contains("unknown provider 'nope'"));
    assert!(auth_cli::check_provider(&c, "ollama").unwrap_err().contains("needs no key"));
    assert!(auth_cli::check_provider(&c, "anthropic").is_ok());
    // a custom provider from config.toml takes a key too
    let s = Setup::from_text(Some("[providers.work]\nbase_url = \"http://w\"\nkey_env = \"CORP_KEY\"\n"), &no_env);
    assert_eq!(auth_cli::check_provider(&s.catalog, "work").unwrap().key_env, "CORP_KEY");
}

#[test]
fn resolution_order_env_then_alias_then_auth_json_then_old_env_files() {
    let c = Catalog::builtin();
    let mut store = Store::default();
    store.set("openai", "from-store");
    store.set("google", "g-store");
    store.set("groq", "q-store");
    let files = vec![
        EnvFile::parse("/h/.bend-harness/.env".into(), "OPENAI_API_KEY=file1\nexport MISTRAL_API_KEY='m-file1'\n"),
        EnvFile::parse("/h/.vibe/.env".into(), "MISTRAL_API_KEY=m-file2\nGOOGLE_API_KEY=g-file\nXAI_API_KEY=\"x-file\"\n"),
    ];
    let env = |k: &str| match k {
        "OPENAI_API_KEY" => Some("from-env".to_string()),
        "GOOGLE_API_KEY" => Some("g-env-alias".to_string()),
        "GROQ_API_KEY" => Some("  ".to_string()), // empty = unset
        _ => None,
    };
    let keys = Keys { env: &env, store: &store, files: &files };
    let f = |id: &str| keys.for_provider(c.provider(id).unwrap());
    assert_eq!(f("openai").unwrap().from, From::Env("OPENAI_API_KEY".into()));
    assert_eq!(f("openai").unwrap().key, "from-env");
    // the alias is still the environment: it wins over auth.json
    assert_eq!(f("google").unwrap().from, From::Env("GOOGLE_API_KEY".into()));
    assert_eq!(f("groq").unwrap().from, From::AuthFile);
    assert_eq!(f("groq").unwrap().key, "q-store");
    // the first .env file wins; quotes and `export` are read
    let m = f("mistral").unwrap();
    assert_eq!((m.from.clone(), m.key.as_str()), (From::EnvFile("/h/.bend-harness/.env".into(), "MISTRAL_API_KEY".into()), "m-file1"));
    assert_eq!(f("xai").unwrap().key, "x-file");
    assert!(f("deepseek").is_none());
    assert!(f("ollama").is_none(), "no key needed");
    assert_eq!(m.from.describe(Some(Path::new("/h"))), "~/.bend-harness/.env (MISTRAL_API_KEY)");
    // Debug never shows a key
    assert!(!format!("{:?}", m).contains("m-file1"));

    // the exports: the runtime reads getenv(key_env) only
    let ex = keys.resolve(&c).exports();
    let get = |k: &str| ex.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    assert_eq!(get("OPENAI_API_KEY"), None, "already in the env under its name");
    assert_eq!(get("GEMINI_API_KEY"), Some("g-env-alias"), "an alias is exported as key_env");
    assert_eq!(get("GROQ_API_KEY"), Some("q-store"));
    assert_eq!(get("MISTRAL_API_KEY"), Some("m-file1"));
    assert_eq!(get("DEEPSEEK_API_KEY"), None);
}

#[test]
fn auth_list_says_where_each_key_comes_from_and_never_the_key() {
    let dir = tmp("list");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    let mut store = Store::default();
    store.set("groq", SECRET);
    store.set("mystery", SECRET);
    let files = vec![EnvFile::parse(dir.join("home/.vibe/.env"), &format!("MISTRAL_API_KEY={}\n", SECRET))];
    let env = |k: &str| (k == "OPENAI_API_KEY").then(|| SECRET.to_string());
    let keys = Keys { env: &env, store: &store, files: &files };
    let out = auth_cli::render_list(&c, &keys, &ps);
    assert!(!out.contains(SECRET), "{out}");
    let line = |id: &str| out.lines().find(|l| l.starts_with(&format!("{} ", id))).unwrap_or("").to_string();
    assert!(line("openai").ends_with("env OPENAI_API_KEY"), "{out}");
    assert!(line("groq").ends_with("auth.json"), "{out}");
    assert!(line("mistral").ends_with("~/.vibe/.env (MISTRAL_API_KEY)"), "{out}");
    assert!(line("deepseek").ends_with(" -"), "{out}");
    assert!(line("ollama").is_empty(), "no key needed: not listed\n{out}");
    assert!(out.contains("auth.json has 'mystery', a provider bise does not know"), "{out}");
    assert!(out.contains("keys: ~/.bise/auth.json"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn logout_says_when_another_source_still_has_a_key() {
    let dir = tmp("still");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &no_env).unwrap();
    let env = |k: &str| (k == "OPENAI_API_KEY").then(|| "e".to_string());
    let out = auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &env).unwrap();
    assert!(out.iter().any(|l| l.contains("OPENAI_API_KEY is set in the environment and wins")), "{out:?}");
    let out = auth_cli::logout(&ps, "openai", &env, &c).unwrap();
    assert!(out.iter().any(|l| l == "openai still has a key: env OPENAI_API_KEY"), "{out:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_spawn_sees_a_login_or_logout_made_after_the_hub_started() {
    let c = Catalog::builtin();
    // at start: OPENAI_API_KEY came from auth.json (set by load_keys, so
    // "ours"), MISTRAL_API_KEY from the user's real environment
    let ours = vec!["OPENAI_API_KEY".to_string(), "SOME_DOTENV_VAR".to_string()];
    let real = |k: &str| match k {
        "MISTRAL_API_KEY" => Some("m-env".to_string()),
        _ => None,
    };
    // later: `login groq`, `logout openai`
    let mut store = Store::default();
    store.set("groq", "q-new");
    let keys = Keys { env: &real, store: &store, files: &[] };
    let spawn = keys.resolve(&c).spawn_env(&c, &ours);
    let get = |k: &str| spawn.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(get("GROQ_API_KEY"), Some(Some("q-new".into())), "a login reaches the next REPL");
    assert_eq!(get("OPENAI_API_KEY"), Some(None), "a logout unsets what the hub set");
    assert_eq!(get("MISTRAL_API_KEY"), None, "the real env is inherited, untouched");
    assert_eq!(get("SOME_DOTENV_VAR"), None, "not a provider key: left as it is");
    assert_eq!(get("DEEPSEEK_API_KEY"), None);
    // a key replaced in auth.json: the new one
    store.set("openai", "o-new");
    let keys = Keys { env: &real, store: &store, files: &[] };
    let spawn = keys.resolve(&c).spawn_env(&c, &ours);
    assert!(spawn.contains(&("OPENAI_API_KEY".into(), Some("o-new".into()))));
}
