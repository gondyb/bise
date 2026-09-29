//! API keys (BISE-143): the key store `auth.json` and where each
//! provider's key comes from.
//!
//! Resolution, per provider (its `key_env`, from the catalog):
//!   1. the environment: `key_env`, then its aliases ([`ALIASES`]);
//!   2. `auth.json` (`bise login <provider>`);
//!   3. the old `.env` files (`~/.bend-harness/.env`, `~/.vibe/.env`),
//!      `key_env` then its aliases, first file first.
//!
//! The Bend runtime reads one variable per provider (`getenv(key_env)`,
//! BISE-141), so at start the harness sets `key_env` in its own process
//! ([`Resolution::exports`]) and every REPL it starts inherits it. A key
//! is never written anywhere but `auth.json`, and no message of this
//! module holds one.
//!
//! `auth.json`, the OpenCode layout: `{"<provider>": {"type": "api",
//! "key": "..."}}`. Other entries (a future OAuth type) are kept as they
//! are. The file is 0600 in a 0700 directory, replaced atomically.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Catalog, Provider};

/// Other names a provider's key goes by in the environment and the
/// `.env` files: `key_env` -> the aliases, in order. The runtime only
/// reads `key_env`, so a key found under an alias is exported as it.
pub const ALIASES: &[(&str, &[&str])] = &[("GEMINI_API_KEY", &["GOOGLE_API_KEY"])];

/// The env names of a provider's key: `key_env`, then its aliases.
pub fn env_names(key_env: &str) -> Vec<&str> {
    let mut v = vec![key_env];
    for (k, alias) in ALIASES {
        if *k == key_env {
            v.extend(alias.iter().copied());
        }
    }
    v
}

// ---- auth.json ----

/// The key store. Reading never fails on a missing file (no keys);
/// a file that is not a JSON object is an error (never overwritten).
#[derive(Clone, Debug, Default)]
pub struct Store {
    entries: serde_json::Map<String, serde_json::Value>,
}

impl Store {
    /// Parse the text of `auth.json`. The error holds no content of the
    /// file (only the syntax position).
    pub fn parse(text: &str) -> Result<Store, String> {
        if text.trim().is_empty() {
            return Ok(Store::default());
        }
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(serde_json::Value::Object(entries)) => Ok(Store { entries }),
            Ok(_) => Err("not a JSON object".into()),
            Err(e) => Err(format!("not valid JSON (line {}, column {})", e.line(), e.column())),
        }
    }

    /// Read `path`; a missing file is an empty store.
    pub fn read(path: &Path) -> Result<Store, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Store::parse(&text).map_err(|e| format!("{}: {}", path.display(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(format!("{}: {}", path.display(), e)),
        }
    }

    /// The API key stored for `provider` (type "api", a non-empty key).
    pub fn key(&self, provider: &str) -> Option<&str> {
        let e = self.entries.get(provider)?.as_object()?;
        if e.get("type").and_then(|t| t.as_str()) != Some("api") {
            return None;
        }
        e.get("key").and_then(|k| k.as_str()).filter(|k| !k.trim().is_empty())
    }

    /// The providers with an entry, sorted.
    pub fn providers(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        v.sort();
        v
    }

    pub fn set(&mut self, provider: &str, key: &str) {
        self.entries.insert(
            provider.to_string(),
            serde_json::json!({ "type": "api", "key": key }),
        );
    }

    /// Whether there was an entry.
    pub fn remove(&mut self, provider: &str) -> bool {
        self.entries.remove(provider).is_some()
    }

    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.entries).unwrap_or_else(|_| "{}".into());
        s.push('\n');
        s
    }

    /// Write to `path`: its directory created 0700 when missing (an
    /// existing one is left as it is), the file 0600, atomic (a temp file
    /// in the same directory, renamed over it).
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        create_private_dir(dir)?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("auth.json");
        let tmp = dir.join(format!(".{}.{}.tmp", name, std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let res = (|| {
            let mut f = open_private(&tmp)?;
            f.write_all(self.to_json().as_bytes())?;
            f.sync_all()?;
            std::fs::rename(&tmp, path)
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        res
    }
}

#[cfg(unix)]
fn open_private(p: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)
}

#[cfg(not(unix))]
fn open_private(p: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(p)
}

/// Create `dir` and its missing parents; the ones created here are 0700
/// (an existing directory keeps its mode).
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_private_dir(parent)?;
    }
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    match b.create(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        r => r,
    }
}

/// The permission bits of `path` when others than its owner may read or
/// write it (a warning); None when private or missing.
pub fn loose_mode(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
        (m & 0o077 != 0).then_some(m)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

// ---- the old .env files ----

/// One `.env` file: its path and its `KEY=VALUE` lines.
#[derive(Clone, Debug, Default)]
pub struct EnvFile {
    pub path: PathBuf,
    pub vars: BTreeMap<String, String>,
}

impl EnvFile {
    /// `KEY=VALUE` lines: `#` comments, `export `, quotes; the first
    /// line of a key wins.
    pub fn parse(path: PathBuf, text: &str) -> EnvFile {
        let mut vars = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if !k.is_empty() {
                vars.entry(k.to_string()).or_insert_with(|| v.to_string());
            }
        }
        EnvFile { path, vars }
    }

    /// The files that exist among `paths`, in order.
    pub fn read_all(paths: &[PathBuf]) -> Vec<EnvFile> {
        paths
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok().map(|t| EnvFile::parse(p.clone(), &t)))
            .collect()
    }
}

// ---- resolution ----

/// Where a provider's key comes from (never the key itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum From {
    /// the environment, under this name
    Env(String),
    /// auth.json
    AuthFile,
    /// an old .env file, under this name
    EnvFile(PathBuf, String),
}

impl From {
    /// "env ANTHROPIC_API_KEY", "auth.json", "~/.vibe/.env (ANTHROPIC_API_KEY)"
    pub fn describe(&self, home: Option<&Path>) -> String {
        match self {
            From::Env(n) => format!("env {}", n),
            From::AuthFile => "auth.json".into(),
            From::EnvFile(p, n) => format!("{} ({})", tilde(p, home), n),
        }
    }
}

/// `path` with the home directory written `~`.
pub fn tilde(p: &Path, home: Option<&Path>) -> String {
    if let Some(rest) = home.and_then(|h| p.strip_prefix(h).ok()) {
        return format!("~/{}", rest.display());
    }
    p.display().to_string()
}

/// A provider's key, resolved. Debug never shows the key.
#[derive(Clone)]
pub struct Found {
    pub from: From,
    pub key: String,
}

impl std::fmt::Debug for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Found {{ from: {:?}, key: <{} bytes> }}", self.from, self.key.len())
    }
}

/// What a key lookup sees: the environment (the real one, before any
/// .env file is loaded into it), auth.json, the old .env files.
pub struct Keys<'a> {
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub store: &'a Store,
    pub files: &'a [EnvFile],
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl Keys<'_> {
    /// The key of the provider `id` whose key variable is `key_env`
    /// (None: no key found, or the provider needs none).
    pub fn find(&self, id: &str, key_env: &str) -> Option<Found> {
        if key_env.is_empty() {
            return None;
        }
        let names = env_names(key_env);
        for n in &names {
            if let Some(key) = non_empty((self.env)(n)) {
                return Some(Found { from: From::Env(n.to_string()), key });
            }
        }
        if let Some(key) = self.store.key(id) {
            return Some(Found { from: From::AuthFile, key: key.trim().to_string() });
        }
        for f in self.files {
            for n in &names {
                if let Some(key) = non_empty(f.vars.get(*n).cloned()) {
                    return Some(Found { from: From::EnvFile(f.path.clone(), n.to_string()), key });
                }
            }
        }
        None
    }

    pub fn for_provider(&self, p: &Provider) -> Option<Found> {
        self.find(&p.id, &p.key_env)
    }

    /// Every provider's key, in catalog order.
    pub fn resolve(&self, c: &Catalog) -> Resolution {
        let mut r = Resolution::default();
        for p in &c.providers {
            if let Some(f) = self.for_provider(p) {
                r.found.push((p.id.clone(), p.key_env.clone(), f));
            }
        }
        r
    }
}

/// The keys of all the providers.
#[derive(Clone, Debug, Default)]
pub struct Resolution {
    /// (provider id, key_env, found)
    pub found: Vec<(String, String, Found)>,
}

impl Resolution {
    /// The variables to set so the runtime's `getenv(key_env)` sees each
    /// key: every key_env whose key does not already come from the
    /// environment under that very name. Two providers sharing a
    /// key_env: the first one wins.
    pub fn exports(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (_, key_env, f) in &self.found {
            if f.from == From::Env(key_env.clone()) || out.iter().any(|(k, _)| k == key_env) {
                continue;
            }
            out.push((key_env.clone(), f.key.clone()));
        }
        out
    }
}
