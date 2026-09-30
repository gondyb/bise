//! The Seatbelt sandbox on macOS (design §6, brief 1e).
//!
//! In `auto`, every bash call runs under a per-agent profile: writes only
//! in the roots (design §7), network only on loopback and unix sockets
//! (the hub's socket), unless the command names a network program the
//! gate allowed. The hub writes two profiles next to the agent's gate file
//! (`run/sandbox.sb`, `run/sandbox-net.sb`, [`ensure`]); its allow line
//! tells the runtime which one to use ([`run_flags`]).
//!
//! A command the sandbox stopped comes back to the gate once more, with
//! its output: [`Denial::of`] reads what it tried, the checker and then a
//! card decide a rerun without the sandbox ([`Denial::reason`]).
//!
//! Pure but for [`ensure`], [`git_common_dir`] and [`available`].

use std::path::{Path, PathBuf};

use super::paths::{fold, Fs, Roots};
use super::{parse, tiers, Call, CacheKey};

/// The profile with the network closed (loopback and unix sockets kept).
pub const PROFILE: &str = "sandbox.sb";
/// The same with the network open: a part that names a network program.
pub const PROFILE_NET: &str = "sandbox-net.sb";

/// The allow flags: run under `sandbox.sb`, or `sandbox-net.sb`.
pub const FLAG_SANDBOX: &str = "sandbox";
pub const FLAG_SANDBOX_NET: &str = "sandbox net";

/// Caches real work writes outside the roots (design §6.2, §6.4): no
/// secrets there, allowed by default. Relative to `~`.
pub const CACHES: &[&str] = &[
    ".cargo/registry",
    ".cargo/git",
    ".npm",
    "Library/pnpm",
    ".cache",
    "Library/Caches",
];

/// Shell files under `~` a sandboxed command never writes, even when the
/// agent's folder is the home (design §7, `Roots::protected`).
const HOME_PROTECTED: &[&str] = &[
    ".ssh",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".bashrc",
    ".bash_profile",
    ".profile",
    ".config/git",
    ".gitconfig",
    "Library/LaunchAgents",
];

/// What one agent's profile allows: its roots, canonical (Seatbelt
/// matches the real path: `/var/folders/…` is `/private/var/folders/…`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The agent's folder (workspace or worktree).
    pub cwd: PathBuf,
    /// The repo's git common dir (a worktree's objects and refs live
    /// there): writable but its `hooks/` and `config`.
    pub git: Option<PathBuf>,
    /// `~/.bise`: writable but `hubs/`, `approvals.toml`, `auth.json`.
    pub bise: PathBuf,
    /// The agent's temp folder, the one writable place in `hubs/`.
    pub tmp: PathBuf,
    pub home: PathBuf,
    /// macOS's per-user temp folder (`getconf DARWIN_USER_TEMP_DIR`):
    /// `mktemp` writes there whatever `$TMPDIR` says (macOS 26), so it is
    /// allowed like a cache; `None` elsewhere.
    pub user_tmp: Option<PathBuf>,
}

impl Spec {
    /// The spec of a gated call's agent, its paths resolved by `fs`, the
    /// git common dir found from its folder.
    pub fn of(call: &Call, fs: &dyn Fs) -> Spec {
        let cwd = fs.real(&call.cwd);
        Spec {
            git: git_common_dir(&cwd).map(|g| fs.real(&g)),
            cwd,
            bise: fs.real(&call.bise),
            tmp: fs.real(&call.tmp),
            home: fs.real(&call.home),
            user_tmp: darwin_user_temp().map(|t| fs.real(&t)),
        }
    }
}

/// `getconf DARWIN_USER_TEMP_DIR`, once per hub (macOS only).
pub fn darwin_user_temp() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let o = std::process::Command::new("/usr/bin/getconf")
            .arg("DARWIN_USER_TEMP_DIR")
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&o.stdout).trim().trim_end_matches('/').to_string();
        (o.status.success() && s.starts_with('/')).then(|| {
            std::fs::canonicalize(&s).unwrap_or_else(|_| PathBuf::from(s))
        })
    })
    .clone()
}

/// An SBPL string literal.
fn lit(p: &Path) -> String {
    let s = p.to_string_lossy();
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The profile text (pure). In SBPL the last matching rule wins: deny
/// every write, allow the roots, deny the protected paths inside them,
/// allow the agent's `tmp/` again.
pub fn profile(s: &Spec, net: bool) -> String {
    let mut o = String::new();
    o.push_str("(version 1)\n");
    o.push_str("; bise: written by the hub for one agent (docs/approvals-design.md §6). do not edit.\n");
    o.push_str("(allow default)\n");
    o.push_str("(deny file-write*)\n");
    o.push_str("(allow file-write*\n");
    let mut roots: Vec<PathBuf> = vec![s.cwd.clone()];
    roots.extend(s.git.clone());
    roots.push(s.bise.clone());
    roots.extend(CACHES.iter().map(|c| s.home.join(c)));
    roots.extend(s.user_tmp.clone());
    for r in &roots {
        o.push_str(&format!("  (subpath {})\n", lit(r)));
    }
    o.push_str("  (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/stdout\") (literal \"/dev/stderr\")\n");
    o.push_str("  (literal \"/dev/dtracehelper\") (literal \"/dev/ptmx\") (subpath \"/dev/fd\") (regex #\"^/dev/tty\"))\n");
    o.push_str("(deny file-write*\n");
    let mut denied: Vec<String> = vec![];
    if let Some(g) = &s.git {
        denied.push(format!("(subpath {})", lit(&g.join("hooks"))));
        denied.push(format!("(literal {})", lit(&g.join("config"))));
    }
    denied.push(format!("(subpath {})", lit(&s.bise.join("hubs"))));
    denied.push(format!("(literal {})", lit(&s.bise.join("approvals.toml"))));
    denied.push(format!("(literal {})", lit(&s.bise.join("auth.json"))));
    denied.push(format!("(literal {})", lit(&s.cwd.join(".envrc"))));
    for h in HOME_PROTECTED {
        denied.push(format!("(subpath {})", lit(&s.home.join(h))));
    }
    for d in denied {
        o.push_str(&format!("  {d}\n"));
    }
    o.push_str(")\n");
    o.push_str(&format!("(allow file-write* (subpath {}))\n", lit(&s.tmp)));
    if !net {
        o.push_str("(deny network*)\n");
        o.push_str("(allow network* (local unix-socket) (remote unix-socket))\n");
        o.push_str("(allow network-bind network-inbound (local ip \"localhost:*\"))\n");
        o.push_str("(allow network-outbound (remote ip \"localhost:*\"))\n");
    }
    o
}

/// Write the agent's two profiles into its `run/` folder when they
/// changed (a root moved): the old text is read and compared, the new one
/// replaces it atomically. Called by the hub before each sandboxed allow.
pub fn ensure(run_dir: &Path, s: &Spec) -> std::io::Result<()> {
    std::fs::create_dir_all(run_dir)?;
    for (name, net) in [(PROFILE, false), (PROFILE_NET, true)] {
        let path = run_dir.join(name);
        let text = profile(s, net);
        if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
            continue;
        }
        let part = run_dir.join(format!(".{name}.{}", std::process::id()));
        std::fs::write(&part, &text)?;
        std::fs::rename(&part, &path)?;
    }
    Ok(())
}

/// The git common dir of the repo holding `cwd`: its `.git` folder, or
/// for a worktree (`.git` is a file `gitdir: …`) the folder its
/// `commondir` names. `None` outside a repo.
pub fn git_common_dir(cwd: &Path) -> Option<PathBuf> {
    let mut cur = Some(cwd);
    while let Some(d) = cur {
        let dot = d.join(".git");
        if dot.is_dir() {
            return Some(dot);
        }
        if let Ok(text) = std::fs::read_to_string(&dot) {
            let gitdir = text.trim().strip_prefix("gitdir:")?.trim().to_string();
            let gitdir = fold(&d.join(gitdir));
            return Some(match std::fs::read_to_string(gitdir.join("commondir")) {
                Ok(c) => fold(&gitdir.join(c.trim())),
                Err(_) => gitdir,
            });
        }
        cur = d.parent();
    }
    None
}

/// Whether this hub sandboxes: macOS, `sandbox-exec` there, and not
/// turned off (`BISE_SANDBOX=0`).
pub fn available() -> Availability {
    static EXISTS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let exists = *EXISTS.get_or_init(|| Path::new(SANDBOX_EXEC).exists());
    availability(
        cfg!(target_os = "macos"),
        exists,
        std::env::var("BISE_SANDBOX").ok().as_deref(),
    )
}

pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    On,
    /// Not macOS (Linux: `sb/ports`), or turned off: the parser path.
    Off,
    /// macOS without `sandbox-exec`: the parser path, said once in main's
    /// feed ([`MISSING_NOTICE`]).
    Missing,
}

impl Availability {
    pub fn on(self) -> bool {
        self == Availability::On
    }
}

pub fn availability(macos: bool, exists: bool, env: Option<&str>) -> Availability {
    match (macos, env) {
        (false, _) | (true, Some("0" | "off" | "false")) => Availability::Off,
        (true, _) if !exists => Availability::Missing,
        _ => Availability::On,
    }
}

/// Main's feed, once per hub, when `auto` is on and `sandbox-exec` is
/// missing.
pub const MISSING_NOTICE: &str =
    "sandbox-exec isn't on this Mac, so auto reads each command instead of sandboxing it.";

/// How an allowed bash call runs under the sandbox: with the network
/// open when one of its parts names a network program (design §6.3: the
/// gate let it through, by a saved rule, the cache or the checker).
pub fn run_flags(cmd: &str) -> &'static str {
    let parsed = parse::parse(cmd);
    if parsed
        .parts
        .iter()
        .any(|p| tiers::risks(p).contains(&tiers::Risk::Network))
    {
        FLAG_SANDBOX_NET
    } else {
        FLAG_SANDBOX
    }
}

/// The cache key of "this command may run without the sandbox": the
/// checker (or the user) allowed its rerun; the next identical call skips
/// the sandbox instead of running twice.
pub fn rerun_key(cmd: &str) -> CacheKey {
    CacheKey::Exact(format!("unsandboxed {}", cmd.trim()))
}

/// What the sandbox stopped, read from the command's output (Codex's
/// heuristic, `sandboxing/src/denial.rs`: there is no sure way to tell).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Denial {
    /// A write: the path when the output names one.
    Write(Option<String>),
    /// The network (closed for this command).
    Network,
}

/// Output that says a write was refused by the OS.
const WRITE_SIGNS: &[&str] = &["operation not permitted"];
/// Output that says a network call failed; only a sign under the closed
/// profile.
const NETWORK_SIGNS: &[&str] = &[
    "could not resolve host",
    "couldn't connect to server",
    "could not connect to server",
    "nodename nor servname provided",
    "name or service not known",
    "temporary failure in name resolution",
    "failed to lookup address",
    "network is unreachable",
    "no route to host",
];

impl Denial {
    /// The denial a failed sandboxed run's output shows, if any.
    pub fn of(output: &str) -> Option<Denial> {
        let lower = output.to_lowercase();
        if WRITE_SIGNS.iter().any(|s| lower.contains(s)) {
            return Some(Denial::Write(denied_path(output)));
        }
        if NETWORK_SIGNS.iter().any(|s| lower.contains(s)) {
            return Some(Denial::Network);
        }
        None
    }

    /// The card's reason (design §6.3, brief 1e: the rerun runs the
    /// command a second time).
    pub fn reason(&self, roots: &Roots) -> String {
        let what = match self {
            Denial::Write(Some(p)) => {
                let shown = roots
                    .resolve(Some(&roots.cwd), p, &super::LexicalFs)
                    .map(|a| roots.show(&a))
                    .unwrap_or_else(|| p.clone());
                format!("it needs to write outside the repo: {shown}.")
            }
            Denial::Write(None) => "it needs to write outside the repo.".to_string(),
            Denial::Network => "it needs the network.".to_string(),
        };
        format!("{what} yes runs it a second time, outside the sandbox.")
    }

    /// What the checker is told on top of the command.
    pub fn state(&self) -> String {
        let what = match self {
            Denial::Write(Some(p)) => format!("a write to {p}, outside the repo"),
            Denial::Write(None) => "a write outside the repo".to_string(),
            Denial::Network => "a network call".to_string(),
        };
        format!(
            "the sandbox stopped this command: {what}. if allowed, it runs again, without the sandbox."
        )
    }

    /// The agent's result when the rerun is refused.
    pub fn result(&self, why: &str) -> String {
        let what = match self {
            Denial::Write(Some(p)) => {
                format!("it tried to write to {p}, outside the repo, ~/.bise and $TMPDIR")
            }
            Denial::Write(None) => {
                "it tried to write outside the repo, ~/.bise and $TMPDIR".to_string()
            }
            Denial::Network => "it needs the network, which is closed for this command".to_string(),
        };
        let why = why.trim();
        if why.is_empty() {
            format!("stopped by the sandbox: {what}. not run again.")
        } else {
            format!("stopped by the sandbox: {what}. not run again: {why}")
        }
    }
}

/// The path an "Operation not permitted" line names: the last `: `
/// segment that is a path (`/bin/sh: /x/y: Operation…`, `touch: /x:
/// Operation…`, python's `…Operation not permitted: '/x'`).
pub fn denied_path(output: &str) -> Option<String> {
    output
        .lines()
        .filter(|l| l.to_lowercase().contains("operation not permitted"))
        .find_map(|l| {
            l.split(": ")
                .map(|s| s.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`'))
                .filter(|s| s.starts_with('/') || s.starts_with("~/"))
                .last()
                .map(str::to_string)
        })
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod sandbox_tests;
