//! `bise doctor` (BISE-167, docs/research/portable-bise.md §3.5): one line
//! per check, ✓ / ! / ✗, and how to fix what is wrong. Reads only: no
//! migration, no hub start, never a key (only where each one comes from).
//! Exit 1 when a check fails (✗); a warning (!) does not fail.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use bise_catalog::auth::{EnvFile, Keys, Store};
use switchboard::tools_env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Ok,
    Warn,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Check {
    pub(crate) mark: Mark,
    pub(crate) name: &'static str,
    pub(crate) detail: String,
    /// how to fix it (a warning or a failure)
    pub(crate) fix: Option<String>,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Check {
    Check { mark: Mark::Ok, name, detail: detail.into(), fix: None }
}

fn warn(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { mark: Mark::Warn, name, detail: detail.into(), fix: Some(fix.into()) }
}

fn fail(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { mark: Mark::Fail, name, detail: detail.into(), fix: Some(fix.into()) }
}

/// The report: one line per check.
pub(crate) fn render(checks: &[Check]) -> String {
    let mut o = String::new();
    for c in checks {
        let sym = match c.mark {
            Mark::Ok => "✓",
            Mark::Warn => "!",
            Mark::Fail => "✗",
        };
        o.push_str(&format!("{} {:<9} {}", sym, c.name, c.detail));
        if let Some(f) = &c.fix {
            o.push_str(&format!(" — fix: {}", f));
        }
        o.push('\n');
    }
    o
}

// ---- the pure checks (tested) ----

/// "14.6.1" -> (14, 6)
fn major_minor(v: &str) -> Option<(u32, u32)> {
    let mut it = v.trim().split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

pub(crate) fn macos_check(version: Option<&str>, min: &str, arch: &str, rosetta: bool) -> Check {
    let arch = if rosetta { format!("{} under Rosetta", arch) } else { arch.to_string() };
    let Some(v) = version else {
        return warn("macOS", format!("version unknown ({})", arch), "bise supports macOS only");
    };
    let detail = format!("macOS {} {}", v, arch);
    if rosetta {
        return warn("macOS", detail, "run the arm64 build of bise (this one is x86_64, emulated)");
    }
    match (major_minor(v), major_minor(min)) {
        (Some(have), Some(need)) if have < need => {
            fail("macOS", format!("{} (bise needs {} or newer)", detail, min), format!("update macOS to {} or newer", min))
        }
        _ => ok("macOS", detail),
    }
}

/// The Unix socket path limit (sun_path, macOS: 104 bytes with the NUL).
pub(crate) const SOCKET_MAX: usize = 103;

pub(crate) fn socket_check(socket: &Path) -> Check {
    let n = socket.as_os_str().len();
    if n <= SOCKET_MAX {
        ok("socket", format!("{} bytes (max {}): {}", n, SOCKET_MAX, socket.display()))
    } else {
        fail(
            "socket",
            format!("{} bytes, over the {} a Unix socket path allows: {}", n, SOCKET_MAX, socket.display()),
            "a shorter BISE_HOME (or SB_STATE_DIR)",
        )
    }
}

pub(crate) fn disk_check(where_: &Path, avail_kb: Option<u64>) -> Check {
    let Some(kb) = avail_kb else {
        return warn("disk", format!("free space unknown ({})", where_.display()), "check `df -h ~`");
    };
    let gb = kb as f64 / (1024.0 * 1024.0);
    let detail = format!("{:.1} GB free ({})", gb, where_.display());
    if gb < 1.0 {
        fail("disk", detail, "free some disk: hubs, worktrees and sessions need room")
    } else if gb < 5.0 {
        warn("disk", detail, "under 5 GB: free some disk soon")
    } else {
        ok("disk", detail)
    }
}

/// The keys line: the providers with a key and where it comes from.
pub(crate) fn keys_check(found: &[(String, String)]) -> Check {
    if found.is_empty() {
        return fail("keys", "no provider key", format!("`{} login <provider>` (or set its env variable)", bise_catalog::CLI));
    }
    let list: Vec<String> = found.iter().map(|(p, from)| format!("{} ({})", p, from)).collect();
    ok("keys", list.join(", "))
}

/// The migration line from migrated.json (None: not moved yet).
/// `legacy`: `~/.bend-harness` or `~/.local/state/switchboard` exists.
pub(crate) fn migration_check(
    marker: Option<&serde_json::Value>,
    explicit_home: bool,
    no_migrate: bool,
    legacy: bool,
) -> Check {
    let Some(v) = marker else {
        return if explicit_home {
            ok("migration", "not needed (BISE_HOME is set: never filled from the old places)")
        } else if !legacy {
            ok("migration", "nothing to move (no ~/.bend-harness nor ~/.local/state/switchboard)")
        } else if no_migrate {
            warn("migration", "not done (BISE_NO_MIGRATE is set)", "unset BISE_NO_MIGRATE and start bise once")
        } else {
            warn("migration", "not done: state still in ~/.bend-harness and ~/.local/state/switchboard", "start `bise` once: it moves them to ~/.bise")
        };
    };
    let n = |k: &str| v.get(k).and_then(|x| x.as_array()).map_or(0, |a| a.len());
    let waiting = n("hubs_waiting");
    let errors = n("errors");
    let detail = format!(
        "done: {} copied, {} hubs moved, {} waiting (running in the old place), {} errors",
        n("copied"),
        n("hubs_moved"),
        waiting,
        errors
    );
    if errors > 0 {
        warn("migration", detail, "see `errors` in ~/.bise/migrated.json")
    } else if waiting > 0 {
        warn("migration", detail, "those hubs move at their next restart (`/restart` or `bise switchboard --stop`)")
    } else {
        ok("migration", detail)
    }
}

// ---- probes of this machine ----

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn macos() -> Check {
    let version = run("/usr/bin/sw_vers", &["-productVersion"]);
    let rosetta = run("/usr/sbin/sysctl", &["-n", "sysctl.proc_translated"]).as_deref() == Some("1");
    let arch = crate::version::build_target();
    let arch = arch.split_once('-').map_or(arch.as_str(), |(_, a)| a).to_string();
    // the target of this version (VERSION macos=), else the build's (BISE-164)
    let min = root()
        .ok()
        .and_then(|(r, _)| std::fs::read_to_string(r.join("VERSION")).ok())
        .and_then(|v| v.lines().find_map(|l| l.strip_prefix("macos=").map(str::to_string)))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "14.0".into());
    macos_check(version.as_deref(), &min, &arch, rosetta)
}

fn root() -> Result<(PathBuf, crate::approot::Via), String> {
    crate::approot::locate("repl-live")
}

fn bise() -> Check {
    let exe = std::env::current_exe().ok().map(|e| std::fs::canonicalize(&e).unwrap_or(e));
    let exe = exe.map(|e| e.display().to_string()).unwrap_or_else(|| "unknown".into());
    match root() {
        Ok((r, via)) => {
            let version = std::fs::read_to_string(r.join("VERSION")).ok();
            let line = crate::version::line(
                bise_catalog::CLI,
                version.as_deref(),
                &crate::version::build_target(),
                &r.display().to_string(),
            );
            ok("bise", format!("{} · {} · files: {} ({})", line, exe, r.display(), via.describe()))
        }
        Err(e) => fail("bise", format!("{} · no app root: {}", exe, e), "reinstall bise, or set BISE_APP_ROOT"),
    }
}

fn signature() -> Check {
    let Ok(exe) = std::env::current_exe() else {
        return warn("signature", "executable unknown", "reinstall bise");
    };
    let out = Command::new("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(&exe)
        .stdin(Stdio::null())
        .output();
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stderr).to_string(),
        Err(e) => return warn("signature", format!("codesign: {}", e), "none needed to run; macOS only"),
    };
    if let Some(a) = text.lines().find_map(|l| l.strip_prefix("Authority=")) {
        ok("signature", a.to_string())
    } else if text.contains("Signature=adhoc") {
        ok("signature", "ad-hoc (enough for a curl or brew install: no quarantine flag)")
    } else {
        warn("signature", "not signed", "reinstall bise (an arm64 Mac runs no unsigned binary)")
    }
}

fn home_check(home: &bise_home::Home) -> Check {
    let root = home.root();
    let how = if std::env::var_os(bise_home::BISE_HOME).is_some_and(|v| !v.is_empty()) {
        "BISE_HOME"
    } else {
        "default"
    };
    match home.layout() {
        // a fresh HOME: nothing in the old places either (qa C)
        bise_home::Layout::Legacy if !legacy_found(home) => warn(
            "home",
            format!("{} does not exist yet", home.user_home().join(".bise").display()),
            "start `bise` once",
        ),
        bise_home::Layout::Legacy => warn(
            "home",
            format!("old layout: {} + ~/.local/state/switchboard", root.display()),
            "start `bise` once: it moves the state to ~/.bise",
        ),
        bise_home::Layout::Bise if !root.is_dir() => {
            warn("home", format!("{} ({}) does not exist yet", root.display(), how), "start `bise` once")
        }
        bise_home::Layout::Bise => {
            let has = |p: PathBuf| if p.exists() { "✓" } else { "-" };
            ok(
                "home",
                format!(
                    "{} ({}): config.toml {}, auth.json {}, hubs/ {}, sessions/ {}",
                    root.display(),
                    how,
                    has(home.config_file()),
                    has(home.auth_file()),
                    has(home.hubs_dir()),
                    has(home.sessions_dir())
                ),
            )
        }
    }
}

/// The old places of this HOME exist (what the migration would read).
fn legacy_found(home: &bise_home::Home) -> bool {
    let u = home.user_home();
    u.join(".bend-harness").exists() || u.join(".local/state/switchboard").exists()
}

fn migration(home: &bise_home::Home) -> Check {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let marker = std::fs::read_to_string(home.user_home().join(".bise").join(bise_home::MIGRATED))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    migration_check(
        marker.as_ref(),
        env(bise_home::BISE_HOME).is_some(),
        env(bise_home::migrate::NO_MIGRATE).is_some(),
        legacy_found(home),
    )
}

/// The PATH the agents' tools are looked up on (the hub's, plus the usual
/// dirs; the hub also adds the login shell's, BISE-166).
fn tools_path() -> String {
    tools_env::join_path(&[&std::env::var("PATH").unwrap_or_default(), &tools_env::STD_DIRS.join(":")])
}

fn git() -> Check {
    match tools_env::git() {
        tools_env::Git::Works { bin, version } => ok("git", format!("{} ({})", version, bin.display())),
        g => fail("git", "no working git (needed for -w worktrees and /version)", g.problem().unwrap_or_default()),
    }
}

fn rg() -> Check {
    match tools_env::which("rg", &tools_path()) {
        Some(p) => ok("rg", p.display().to_string()),
        None => warn("rg", "not found: the agents search with grep", "`brew install ripgrep` (faster searches)"),
    }
}

fn on_path() -> Check {
    let me = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok());
    let path = std::env::var("PATH").unwrap_or_default();
    let Some(p) = tools_env::which(bise_catalog::CLI, &path) else {
        return warn("PATH", "`bise` is not on PATH", "add ~/.local/bin to PATH (the installer does it; open a new terminal)");
    };
    let real = std::fs::canonicalize(&p).ok();
    // a launcher (install.sh, install.sh --dev) names the version it runs
    let launched = real.as_deref().filter(|r| is_launcher(r)).and_then(|r| {
        Command::new(r)
            .arg("--launcher-root")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
    });
    let mine = root().ok().map(|(r, _)| std::fs::canonicalize(&r).unwrap_or(r));
    path_check(&p, real.as_deref(), me.as_deref(), launched.as_deref(), mine.as_deref())
}

/// A shell script written by install.sh (a launcher), not a binary.
fn is_launcher(p: &Path) -> bool {
    let mut head = [0u8; 256];
    let n = std::fs::File::open(p).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    let head = String::from_utf8_lossy(&head[..n]);
    head.starts_with("#!") && head.contains("launcher")
}

/// The PATH line: `found` is `bise` on PATH (`real`: its target); `me` this
/// executable; `launched`: the app root the launcher runs, when `found` is
/// one; `mine`: this process's app root.
pub(crate) fn path_check(found: &Path, real: Option<&Path>, me: Option<&Path>, launched: Option<&Path>, mine: Option<&Path>) -> Check {
    if real.is_some() && real == me {
        return ok("PATH", format!("{} is this bise", found.display()));
    }
    match launched {
        Some(l) if Some(l) == mine => ok("PATH", format!("{} is a launcher that runs this bise ({})", found.display(), l.display())),
        Some(l) => warn(
            "PATH",
            format!("{} is a launcher that runs {}, not this bise", found.display(), l.display()),
            "fine for a version you run by hand; `bise doctor` checks the one the launcher runs",
        ),
        None => warn(
            "PATH",
            format!("{} is another bise ({})", found.display(), real.map(|r| r.display().to_string()).unwrap_or_default()),
            "fine in the dev tree; else put ~/.local/bin first in PATH",
        ),
    }
}

/// The voice input's model (`[voice]`, BISE-130): optional, so a problem
/// is a warning (ctrl+r fails, nothing else does). `key`: its provider's
/// key is set (None: the provider needs none).
fn voice_check(v: &bise_catalog::voice::VoiceSetup, r: &bise_catalog::voice::SttResolved, key: Option<bool>) -> Check {
    let what = format!("{} ({})", r.name, v.from);
    let fix_model = format!("set [voice] model to a listed one (`{} models voice`)", bise_catalog::CLI);
    if r.known == bise_catalog::Known::NoProvider {
        return warn("voice", format!("{}: unknown provider '{}'", what, r.provider), fix_model);
    }
    if r.api.is_empty() {
        return warn("voice", format!("{}: {} does not transcribe", what, r.provider), fix_model);
    }
    if !r.needs.is_empty() {
        return warn("voice", format!("{}: not usable yet ({})", what, r.needs), fix_model);
    }
    if key == Some(false) {
        return warn(
            "voice",
            format!("{}: no {} key", what, r.provider),
            format!("`{} login {}` (or set {})", bise_catalog::CLI, r.provider, r.key_env),
        );
    }
    ok("voice", format!("{} · language {}", what, v.language.as_deref().unwrap_or("auto")))
}

/// config.toml's warnings (what `bise models` prints under its list).
fn config_check(path: &Path, warnings: &[String]) -> Check {
    match warnings {
        [] => ok("config", if path.exists() { path.display().to_string() } else { format!("{} (none: the defaults)", path.display()) }),
        ws => warn("config", ws.join(" · "), format!("fix {} (`{} models` shows the same)", path.display(), bise_catalog::CLI)),
    }
}

/// The fix of a model whose provider has no key (BISE-266): a provider
/// with a key and a default model → that model; a hidden provider (a
/// private proxy: nobody else can log in to it) → pick one from the
/// start; else its login.
fn no_key_fix(setup: &bise_catalog::Setup, r: &bise_catalog::Resolved, found: &[(String, String)]) -> String {
    let cli = bise_catalog::CLI;
    let keyed = setup
        .catalog
        .providers
        .iter()
        .find(|p| !p.model.is_empty() && !p.stt_only && p.needs.is_empty() && found.iter().any(|(id, _)| *id == p.id));
    if let Some(p) = keyed {
        return format!("you have a {} key: set model = \"{}/{}\" in config.toml", p.id, p.id, p.model);
    }
    if setup.catalog.provider(&r.provider).is_some_and(|p| p.hidden) {
        return format!("run `{cli}` and pick a provider, or `{cli} login anthropic` (any provider: `{cli} models`) and set model in config.toml");
    }
    format!("`{} login {}` (or set {})", cli, r.provider, r.key_env)
}

/// The keys, model and voice lines.
fn keys_and_model(home: &bise_home::Home) -> (Check, Check, Check) {
    let store = match Store::read(&home.auth_file()) {
        Ok(s) => s,
        Err(e) => {
            let f = fail("keys", format!("auth.json unreadable: {}", e), format!("fix or remove {}", home.auth_file().display()));
            return (f.clone(), fail("model", "keys unknown", "fix auth.json first"), warn("voice", "keys unknown", "fix auth.json first"));
        }
    };
    let files = EnvFile::read_all(&home.env_files());
    let setup = bise_catalog::Setup::load(&home.config_file());
    let env = |k: &str| std::env::var(k).ok();
    let keys = Keys { env: &env, store: &store, files: &files };
    let user = Some(home.user_home());
    let found: Vec<(String, String)> = setup
        .catalog
        .providers
        .iter()
        .filter_map(|p| keys.source(p, user).map(|from| (p.id.clone(), from)))
        .collect();
    let keys_line = keys_check(&found);
    let model = |label: &str, name: &str, from: &str| -> Result<String, (String, String)> {
        // BISE-266: no built-in model: none until a key is checked
        if name.trim().is_empty() {
            return Err((
                format!("{}: no model yet", label),
                format!("run `{}` and pick a provider (it checks the key), or set model in config.toml", bise_catalog::CLI),
            ));
        }
        let r = setup.catalog.resolve(name);
        let what = format!("{} {} ({})", label, r.name, from);
        if r.known == bise_catalog::Known::NoProvider {
            return Err((
                format!("{}: unknown provider '{}'", what, r.provider),
                format!("add [providers.{}] to config.toml, or pick a listed model (`{} models`)", r.provider, bise_catalog::CLI),
            ));
        }
        if !r.needs.is_empty() {
            return Err((format!("{}: not usable yet ({})", what, r.needs), format!("pick another model (`{} models`)", bise_catalog::CLI)));
        }
        if !r.key_env.is_empty() && keys.find(&r.provider, &r.key_env).is_none() {
            return Err((format!("{}: no {} key", what, r.provider), no_key_fix(&setup, &r, &found)));
        }
        Ok(what)
    };
    let agent_from = if setup.agent_model_from == "model" { "same as model" } else { setup.agent_model_from };
    let lines = [
        model("main", &setup.model, setup.model_from),
        model("agents", &setup.agent_model, agent_from),
    ];
    let mut bad = lines.iter().filter_map(|l| l.as_ref().err());
    let model_line = match bad.next() {
        Some((detail, fix)) => fail("model", detail.clone(), fix.clone()),
        None => ok("model", lines.iter().filter_map(|l| l.as_ref().ok().cloned()).collect::<Vec<_>>().join(" · ")),
    };
    let stt = setup.catalog.resolve_stt(&setup.voice.model);
    let stt_key = (!stt.key_env.is_empty()).then(|| keys.find(&stt.provider, &stt.key_env).is_some());
    (keys_line, model_line, voice_check(&setup.voice, &stt, stt_key))
}

fn hubs(home: &bise_home::Home) -> Check {
    let ws = crate::sb_workspace(&[]);
    let paths = switchboard::paths::Paths::for_workspace(&ws);
    let here = if switchboard::switch::hub_busy(&paths.state) {
        format!("running for {} ({})", paths.workspace.display(), paths.state.display())
    } else {
        format!("none for {} (`{}` starts one)", paths.workspace.display(), bise_catalog::CLI)
    };
    let count = |dir: &Path| -> usize {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| bise_home::migrate::is_hub_id(&e.file_name().to_string_lossy()))
                    .filter(|e| e.path().is_dir() && switchboard::switch::hub_busy(&e.path()))
                    .count()
            })
            .unwrap_or(0)
    };
    let new = count(&home.hubs_dir());
    let old_dir = home.user_home().join(".local/state/switchboard");
    let old = if old_dir == home.hubs_dir() { 0 } else { count(&old_dir) };
    let detail = format!("{}; {} running in {}, {} in the old place", here, new, home.hubs_dir().display(), old);
    let socket = socket_check(&paths.socket());
    if socket.mark == Mark::Fail {
        return socket;
    }
    if old > 0 && home.layout() == bise_home::Layout::Bise {
        warn("hubs", detail, "they move to ~/.bise at their next restart")
    } else {
        ok("hubs", detail)
    }
}

fn disk(home: &bise_home::Home) -> Check {
    let at = if home.root().exists() { home.root().to_path_buf() } else { home.user_home().to_path_buf() };
    let kb = Command::new("/bin/df")
        .arg("-Pk")
        .arg(&at)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .nth(1)
                .and_then(|l| l.split_whitespace().nth(3).and_then(|v| v.parse().ok()))
        });
    disk_check(&at, kb)
}

/// `bise doctor`: the report on stdout; 1 when a check failed.
pub(crate) fn main() -> i32 {
    let home = bise_home::Home::from_env();
    let (keys, model, voice) = keys_and_model(&home);
    let config = config_check(&home.config_file(), &bise_catalog::Setup::load(&home.config_file()).catalog.warnings);
    let checks = vec![
        macos(),
        bise(),
        signature(),
        on_path(),
        home_check(&home),
        migration(&home),
        git(),
        rg(),
        config,
        keys,
        model,
        voice,
        hubs(&home),
        disk(&home),
    ];
    print!("{}", render(&checks));
    if checks.iter().any(|c| c.mark == Mark::Fail) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_versions() {
        assert_eq!(macos_check(Some("26.6"), "14.0", "arm64", false).mark, Mark::Ok);
        assert_eq!(macos_check(Some("14.0"), "14.0", "arm64", false).mark, Mark::Ok);
        let old = macos_check(Some("13.6.1"), "14.0", "x86_64", false);
        assert_eq!(old.mark, Mark::Fail);
        assert!(old.fix.unwrap().contains("14.0"));
        assert_eq!(macos_check(Some("15.1"), "14.0", "x86_64", true).mark, Mark::Warn);
        assert_eq!(macos_check(None, "14.0", "arm64", false).mark, Mark::Warn);
    }

    #[test]
    fn socket_length() {
        let short = format!("/Users/me/.bise/hubs/{}-0123abcd/hub.sock", "a".repeat(32));
        assert_eq!(socket_check(Path::new(&short)).mark, Mark::Ok);
        let long = format!("/Users/me/{}/hubs/x-0123abcd/hub.sock", "d".repeat(80));
        let c = socket_check(Path::new(&long));
        assert_eq!(c.mark, Mark::Fail);
        assert!(c.fix.unwrap().contains("BISE_HOME"));
    }

    #[test]
    fn disk_thresholds() {
        let gb = 1024 * 1024;
        assert_eq!(disk_check(Path::new("/"), Some(20 * gb)).mark, Mark::Ok);
        assert_eq!(disk_check(Path::new("/"), Some(3 * gb)).mark, Mark::Warn);
        assert_eq!(disk_check(Path::new("/"), Some(gb / 2)).mark, Mark::Fail);
        assert_eq!(disk_check(Path::new("/"), None).mark, Mark::Warn);
    }

    #[test]
    fn keys_never_print_a_key_only_the_source() {
        let c = keys_check(&[("mistral".into(), "auth.json".into()), ("openai".into(), "env OPENAI_API_KEY".into())]);
        assert_eq!(c.mark, Mark::Ok);
        assert_eq!(c.detail, "mistral (auth.json), openai (env OPENAI_API_KEY)");
        let none = keys_check(&[]);
        assert_eq!(none.mark, Mark::Fail);
        assert!(none.fix.unwrap().contains("bise login"));
    }

    /// qa G: doctor reads `[voice]` and shows config.toml's warnings.
    #[test]
    fn voice_and_config_lines() {
        let d = std::env::temp_dir().join(format!("doctor-voice-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let cfg = d.join("config.toml");
        std::fs::write(&cfg, "[voice]\nmodel = \"nosuch/whisper\"\nlanguage = 42\nbogus = true\n").unwrap();
        let s = bise_catalog::Setup::load(&cfg);
        let r = s.catalog.resolve_stt(&s.voice.model);
        let v = voice_check(&s.voice, &r, None);
        assert_eq!(v.mark, Mark::Warn);
        assert!(v.detail.contains("unknown provider 'nosuch'"), "{v:?}");
        let c = config_check(&cfg, &s.catalog.warnings);
        assert_eq!(c.mark, Mark::Warn);
        assert!(c.detail.contains("voice."), "{c:?}");
        std::fs::write(&cfg, "").unwrap();
        let s = bise_catalog::Setup::load(&cfg);
        let r = s.catalog.resolve_stt(&s.voice.model);
        assert_eq!(voice_check(&s.voice, &r, Some(true)).mark, Mark::Ok);
        assert_eq!(voice_check(&s.voice, &r, Some(false)).mark, Mark::Warn, "no key");
        assert_eq!(config_check(&cfg, &s.catalog.warnings).mark, Mark::Ok);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// qa C: a fresh HOME is not an "old layout".
    #[test]
    fn a_fresh_home_is_not_an_old_layout() {
        let d = std::env::temp_dir().join(format!("doctor-fresh-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let hs = d.to_string_lossy().into_owned();
        let home = bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| hs.clone()));
        let c = home_check(&home);
        assert!(c.detail.contains(".bise does not exist yet") && !c.detail.contains("old layout"), "{c:?}");
        std::fs::create_dir_all(d.join(".bend-harness")).unwrap();
        assert!(home_check(&home).detail.contains("old layout"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn migration_states() {
        let done = serde_json::json!({"copied": ["a", "b"], "hubs_moved": ["h"], "hubs_waiting": [], "errors": []});
        let c = migration_check(Some(&done), false, false, true);
        assert_eq!(c.mark, Mark::Ok);
        assert!(c.detail.contains("2 copied, 1 hubs moved, 0 waiting"), "{}", c.detail);
        let waiting = serde_json::json!({"copied": [], "hubs_moved": [], "hubs_waiting": ["x"], "errors": []});
        assert_eq!(migration_check(Some(&waiting), false, false, true).mark, Mark::Warn);
        assert_eq!(migration_check(None, true, false, true).mark, Mark::Ok);
        assert_eq!(migration_check(None, false, false, true).mark, Mark::Warn);
        // qa C: a fresh HOME has nothing to move
        let fresh = migration_check(None, false, false, false);
        assert_eq!(fresh.mark, Mark::Ok);
        assert!(fresh.detail.contains("nothing to move"), "{}", fresh.detail);
    }

    #[test]
    fn a_launcher_on_path_that_runs_this_bise_is_fine() {
        let (found, launcher) = (Path::new("/h/.local/bin/bise"), Path::new("/h/.bise/dev/bin/bise"));
        let (exe, root) = (Path::new("/v/abc/bise"), Path::new("/v/abc"));
        // the binary itself on PATH
        assert_eq!(path_check(found, Some(exe), Some(exe), None, Some(root)).mark, Mark::Ok);
        // a launcher that runs this version (the dev channel, an install)
        let c = path_check(found, Some(launcher), Some(exe), Some(root), Some(root));
        assert_eq!(c.mark, Mark::Ok, "{c:?}");
        assert!(c.detail.contains("launcher that runs this bise"), "{c:?}");
        // a launcher that runs another version: a warning, never a failure
        let c = path_check(found, Some(launcher), Some(exe), Some(Path::new("/v/old")), Some(root));
        assert_eq!(c.mark, Mark::Warn);
        assert!(c.detail.contains("/v/old"), "{c:?}");
        // another binary
        assert_eq!(path_check(found, Some(Path::new("/x/bise")), Some(exe), None, Some(root)).mark, Mark::Warn);
    }

    #[test]
    fn one_line_per_check_with_its_fix() {
        let out = render(&[ok("git", "git version 2.50"), fail("keys", "no provider key", "`bise login <provider>`")]);
        assert_eq!(out, "✓ git       git version 2.50\n✗ keys      no provider key — fix: `bise login <provider>`\n");
    }
}
