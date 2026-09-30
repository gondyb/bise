//! The agents' tools (BISE-166, docs/research/portable-bise.md §3.4):
//! the PATH their shells get, whether `git` really works, whether `rg`
//! is there, and the note that tells the model so once per session.
//!
//! - The PATH is built on purpose: the hub's `bin` dir (`sb`), the
//!   hub's own PATH (the TUI that started it), the user's login-shell
//!   PATH, then the standard dirs; deduplicated. A hub started from a
//!   thin environment (an IDE task, launchd, `env -i`) still finds
//!   Homebrew tools.
//! - `git` on a fresh Mac is `/usr/bin/git`, a shim that opens the
//!   "install the Command Line Tools" dialog and fails. It is never run
//!   while `xcode-select -p` names no developer dir: the hub would pop
//!   the dialog at every git call.
//! - `rg` is not shipped: agents use it when it is on PATH, else grep.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Appended to every agent PATH (after the user's own dirs).
pub const STD_DIRS: [&str; 6] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// The macOS git shim (xcrun): real git only with a developer dir.
pub const MACOS_GIT_SHIM: &str = "/usr/bin/git";

/// Join PATH strings in order: empty entries dropped, the first copy
/// of each dir kept.
pub fn join_path(parts: &[&str]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for d in parts.iter().flat_map(|p| p.split(':')) {
        if !d.is_empty() && !out.contains(&d) {
            out.push(d);
        }
    }
    out.join(":")
}

/// The agents' PATH: `bin_dir` (sb) : the hub's PATH : the login
/// shell's PATH : the standard dirs.
pub fn agent_path(bin_dir: &Path, inherited: &str, login: Option<&str>) -> String {
    let bin = bin_dir.to_string_lossy();
    let std_dirs = STD_DIRS.join(":");
    join_path(&[&bin, inherited, login.unwrap_or(""), &std_dirs])
}

/// The agents' PATH for this hub process (the login shell is asked
/// once per process).
pub fn hub_agent_path(bin_dir: &Path) -> String {
    agent_path(bin_dir, &std::env::var("PATH").unwrap_or_default(), login_path())
}

/// The PATH the hub itself finds git on: its own PATH + the standard
/// dirs (no login shell: git lives in a standard dir).
fn hub_tools_path() -> String {
    join_path(&[&std::env::var("PATH").unwrap_or_default(), &STD_DIRS.join(":")])
}

/// The user's login-shell PATH, asked once per process ($SHELL, else
/// /bin/zsh; 3 s at most); None when the shell fails or is too slow.
pub fn login_path() -> Option<&'static str> {
    static LOGIN: OnceLock<Option<String>> = OnceLock::new();
    LOGIN
        .get_or_init(|| {
            let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty());
            shell_path(shell.as_deref().unwrap_or("/bin/zsh"), Duration::from_secs(3))
        })
        .as_deref()
}

const MARK: &str = "__BISE_PATH__";

extern "C" {
    fn setsid() -> i32;
}

/// PATH as `shell -i -l` sets it (-l: .zprofile/.bash_profile, -i:
/// .zshrc/.bashrc, where nvm & co add their dirs). The shell runs in
/// its own session, so an interactive shell cannot reach the terminal
/// of the TUI that started the hub; stdin is /dev/null. The PATH is
/// the line after a marker (rc files may print banners). Works for
/// sh, bash, zsh and fish (printenv prints the joined form).
pub fn shell_path(shell: &str, timeout: Duration) -> Option<String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(shell);
    cmd.args(["-i", "-l", "-c", &format!("echo {}; /usr/bin/printenv PATH", MARK)])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe; nothing else runs between
    // fork and exec
    unsafe {
        cmd.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        let _ = tx.send(s);
    });
    let got = rx.recv_timeout(timeout).ok();
    if got.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    parse_marked(&got?)
}

fn parse_marked(out: &str) -> Option<String> {
    let mut lines = out.lines().skip_while(|l| l.trim() != MARK);
    lines.next()?;
    let p = lines.next()?.trim();
    (!p.is_empty()).then(|| p.to_string())
}

/// The first executable file named `name` in a PATH string.
pub fn which(name: &str, path: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join(name)).find(|p| {
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

/// What `git` is on a PATH.
#[derive(Clone, Debug, PartialEq)]
pub enum Git {
    Works { bin: PathBuf, version: String },
    /// no git on the PATH
    Missing,
    /// the macOS shim without Command Line Tools (never run)
    Stub,
    /// `git --version` failed (its first error line)
    Broken(String),
}

const INSTALL: &str = "run `xcode-select --install` (macOS Command Line Tools) or install git, then restart the hub";

impl Git {
    /// Why git cannot be used, in a sentence for the user; None when it
    /// works.
    pub fn problem(&self) -> Option<String> {
        match self {
            Git::Works { .. } => None,
            Git::Missing => Some(format!("git is not installed (not found on PATH): {}", INSTALL)),
            Git::Stub => Some(format!(
                "git is not installed: {} is only the macOS installer stub (no Command Line Tools); {}",
                MACOS_GIT_SHIM, INSTALL
            )),
            Git::Broken(e) => Some(format!("git does not work (`git --version`: {})", e)),
        }
    }

    /// One line for the hub log.
    pub fn describe(&self) -> String {
        match self {
            Git::Works { bin, version } => format!("{} ({})", version, bin.display()),
            _ => self.problem().unwrap_or_default(),
        }
    }
}

/// Probe git on `path`. `shim`: the path that is only real git when
/// `developer_dir_ok()` (macOS: `/usr/bin/git` and `xcode-select -p`);
/// that one is never run otherwise. Else `git --version` must succeed.
pub fn probe_git_in(path: &str, shim: Option<&Path>, developer_dir_ok: &dyn Fn() -> bool) -> Git {
    let Some(bin) = which("git", path) else {
        return Git::Missing;
    };
    if shim.is_some_and(|s| bin == s) && !developer_dir_ok() {
        return Git::Stub;
    }
    match Command::new(&bin).arg("--version").stdin(Stdio::null()).output() {
        Ok(o) if o.status.success() => Git::Works {
            bin,
            version: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        Ok(o) => Git::Broken(first_line(&String::from_utf8_lossy(&o.stderr), &o.status.to_string())),
        Err(e) => Git::Broken(e.to_string()),
    }
}

fn first_line(s: &str, default: &str) -> String {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(default).to_string()
}

/// `xcode-select -p` names an existing developer dir (Command Line
/// Tools or Xcode). It never opens a dialog.
pub fn developer_dir_ok() -> bool {
    Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| Path::new(String::from_utf8_lossy(&o.stdout).trim()).is_dir())
}

fn macos_shim() -> Option<&'static Path> {
    cfg!(target_os = "macos").then(|| Path::new(MACOS_GIT_SHIM))
}

/// git for this process: a working git is kept for good; a missing or
/// stub one is probed again at most every 10 s (the user may install
/// the tools meanwhile; the probe never runs the stub).
pub fn git() -> Git {
    static CACHE: Mutex<Option<(Instant, Git)>> = Mutex::new(None);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match &*c {
        Some((_, g @ Git::Works { .. })) => return g.clone(),
        Some((t, g)) if t.elapsed() < Duration::from_secs(10) => return g.clone(),
        _ => {}
    }
    let g = probe_git_in(&hub_tools_path(), macos_shim(), &developer_dir_ok);
    *c = Some((Instant::now(), g.clone()));
    g
}

/// A `git` command on the probed binary, or why git cannot be used.
pub fn git_command() -> Result<Command, String> {
    match git() {
        Git::Works { bin, .. } => Ok(Command::new(bin)),
        g => Err(g.problem().unwrap_or_default()),
    }
}

/// The other CLIs the note names when they are on PATH: the ones an
/// agent otherwise probes with `which` (or guesses) before using them.
/// Probed with `which` only (a PATH scan, no process started).
pub const OTHER_CLIS: [&str; 13] = [
    "gh", "node", "npm", "pnpm", "bun", "python3", "uv", "cargo", "go", "jq", "tmux", "docker", "make",
];

/// The note's last line: which of OTHER_CLIS are on PATH (None: none).
pub fn other_clis_line(found: &[&str]) -> Option<String> {
    (!found.is_empty()).then(|| {
        let names: Vec<String> = found.iter().map(|n| format!("`{}`", n)).collect();
        format!("- Also installed: {}.", names.join(", "))
    })
}

/// What the model is told once per session (the end of its system
/// prompt, through BEND_TOOLS_NOTE): whether `rg` is there, whether
/// `git` works.
pub fn tools_note(rg: Option<&Path>, git: &Git) -> String {
    let rg_line = match rg {
        Some(_) => "- `rg` (ripgrep) is installed: use it to search files and file contents.".to_string(),
        None => "- `rg` (ripgrep) is NOT installed: search with `grep -rn` (and `find`) instead; do not call `rg`.".to_string(),
    };
    let git_line = match git {
        Git::Works { .. } => "- `git` works.".to_string(),
        Git::Stub => format!(
            "- `git` is NOT installed: {} is the macOS installer stub and each call opens a dialog on the user's screen. Never run `git`; when a task needs it, ask the user to run `xcode-select --install`.",
            MACOS_GIT_SHIM
        ),
        Git::Missing => "- `git` is NOT installed. When a task needs it, ask the user to install it (macOS: `xcode-select --install`).".to_string(),
        Git::Broken(e) => format!("- `git` does not work here (`git --version`: {}).", e),
    };
    format!("## Shell tools on this machine\n\n{}\n{}", rg_line, git_line)
}

/// The note for a shell whose PATH is `path`: rg, git, then the other
/// CLIs found on it.
pub fn tools_note_for(path: &str) -> String {
    let found: Vec<&str> = OTHER_CLIS.iter().copied().filter(|n| which(n, path).is_some()).collect();
    let note = tools_note(which("rg", path).as_deref(), &git());
    match other_clis_line(&found) {
        Some(line) => format!("{}\n{}", note, line),
        None => note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-tools-env-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{}\n", body)).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn agent_path_has_sb_first_then_hub_login_and_std_dirs_once() {
        let p = agent_path(
            Path::new("/state/bin"),
            "/usr/bin:/custom/bin::/bin",
            Some("/Users/u/.nvm/bin:/opt/homebrew/bin:/custom/bin"),
        );
        assert_eq!(
            p,
            "/state/bin:/usr/bin:/custom/bin:/bin:/Users/u/.nvm/bin:/opt/homebrew/bin:/usr/local/bin:/usr/sbin:/sbin"
        );
        // a thin environment (env -i, launchd): sb + the standard dirs
        assert_eq!(
            agent_path(Path::new("/s/bin"), "", None),
            "/s/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        );
    }

    #[test]
    fn login_shell_path_is_read_after_the_marker() {
        let d = tmp("shell");
        // an rc file that prints a banner, then the shell's -c command
        let sh = script(&d, "fakesh", "echo 'Welcome!'\nshift 3\nPATH=/from/login:/usr/bin; export PATH\neval \"$1\"");
        assert_eq!(
            shell_path(sh.to_str().unwrap(), Duration::from_secs(5)).as_deref(),
            Some("/from/login:/usr/bin")
        );
        // too slow: None, in about the timeout
        let slow = script(&d, "slowsh", "sleep 5");
        let t = Instant::now();
        assert_eq!(shell_path(slow.to_str().unwrap(), Duration::from_millis(300)), None);
        assert!(t.elapsed() < Duration::from_secs(3));
        // no shell, no output
        assert_eq!(shell_path("/nonexistent/sh", Duration::from_secs(1)), None);
        assert_eq!(parse_marked("x\n__BISE_PATH__\n\n"), None);
        assert!(shell_path("/bin/sh", Duration::from_secs(5)).is_some());
    }

    #[test]
    fn which_wants_an_executable_file() {
        let d = tmp("which");
        let a = d.join("a");
        let b = d.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("rg"), "not executable").unwrap();
        std::fs::create_dir_all(a.join("tool")).unwrap();
        let rg = script(&b, "rg", "true");
        let path = format!("{}:{}", a.display(), b.display());
        assert_eq!(which("rg", &path), Some(rg));
        assert_eq!(which("tool", &path), None);
        assert_eq!(which("rg", ""), None);
    }

    #[test]
    fn missing_rg_tells_the_model_to_use_grep() {
        let d = tmp("rg");
        let works = Git::Works { bin: "/usr/bin/git".into(), version: "git version 2.39.5".into() };
        let note = tools_note(which("rg", d.to_str().unwrap()).as_deref(), &works);
        assert!(note.starts_with("## Shell tools on this machine\n\n"), "{}", note);
        assert!(note.contains("`rg` (ripgrep) is NOT installed: search with `grep -rn`"), "{}", note);
        assert!(note.contains("- `git` works."));
        let rg = script(&d, "rg", "true");
        let note = tools_note(which("rg", d.to_str().unwrap()).as_deref(), &works);
        assert!(note.contains("`rg` (ripgrep) is installed: use it"), "{}", note);
        assert!(!note.contains("grep -rn"));
        assert_eq!(tools_note(Some(&rg), &works), note);
    }

    #[test]
    fn the_other_clis_found_are_named_in_one_line() {
        assert_eq!(other_clis_line(&[]), None);
        assert_eq!(other_clis_line(&["gh", "jq"]).as_deref(), Some("- Also installed: `gh`, `jq`."));
        let d = tmp("clis");
        script(&d, "jq", "true");
        script(&d, "rg", "true");
        let note = tools_note_for(d.to_str().unwrap());
        assert!(note.ends_with("\n- Also installed: `jq`."), "{}", note);
        let empty = tmp("clis-none");
        assert!(!tools_note_for(empty.to_str().unwrap()).contains("Also installed"));
    }

    #[test]
    fn git_missing_is_said_in_english_with_the_fix() {
        let d = tmp("nogit");
        let g = probe_git_in(d.to_str().unwrap(), None, &|| true);
        assert_eq!(g, Git::Missing);
        let p = g.problem().unwrap();
        assert!(p.starts_with("git is not installed"), "{}", p);
        assert!(p.contains("xcode-select --install"), "{}", p);
        assert!(tools_note(None, &g).contains("`git` is NOT installed"));
    }

    #[test]
    fn the_macos_stub_is_never_run_without_developer_tools() {
        let d = tmp("stub");
        let ran = d.join("ran");
        // the stub: it would pop the dialog (here: leave a trace)
        let shim = script(&d, "git", &format!("touch '{}'; echo 'xcrun: error: invalid active developer path' >&2; exit 1", ran.display()));
        let path = d.to_str().unwrap();
        let asked = std::cell::Cell::new(0);
        let g = probe_git_in(path, Some(&shim), &|| {
            asked.set(asked.get() + 1);
            false
        });
        assert_eq!(g, Git::Stub);
        assert_eq!(asked.get(), 1);
        assert!(!ran.exists(), "the stub ran");
        let p = g.problem().unwrap();
        assert!(p.contains("installer stub") && p.contains("xcode-select --install"), "{}", p);
        let note = tools_note(None, &g);
        assert!(note.contains("Never run `git`"), "{}", note);
        // with the developer tools, the same path is real git: run it
        let g = probe_git_in(path, Some(&shim), &|| true);
        assert!(ran.exists());
        assert_eq!(g, Git::Broken("xcrun: error: invalid active developer path".into()));
        assert!(g.problem().unwrap().starts_with("git does not work"));
    }

    #[test]
    fn a_real_git_answers_its_version() {
        let d = tmp("git");
        let bin = script(&d, "git", "[ \"$1\" = --version ] && echo 'git version 9.9.9'");
        // a git that is not the shim never asks for the developer dir
        let g = probe_git_in(d.to_str().unwrap(), Some(Path::new(MACOS_GIT_SHIM)), &|| panic!("asked"));
        assert_eq!(g, Git::Works { bin: bin.clone(), version: "git version 9.9.9".into() });
        assert_eq!(g.problem(), None);
        assert_eq!(g.describe(), format!("git version 9.9.9 ({})", bin.display()));
    }
}
