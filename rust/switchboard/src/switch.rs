//! Version switch of a workspace's hub, with a probation period and an
//! automatic rollback (the `sbswitch` subcommand).
//!
//! A version is an app root built by `versions.sh` (bise,
//! repl-live, sb-core, VERSION). A switch never stops an agent: the old
//! hub exits with `keep_agents`, the new one adopts the running REPLs
//! (each moves to the new binary at its next idle, same session). The
//! switcher runs from the OLD (known good) binary, detached from both
//! hubs, and watches the new one for the probation period:
//! - the hub process died, or does not answer `ping` for ~10 s;
//! - the hub reported a failure (`probation-fail`: a REPL of the new
//!   version crashed, or could not start).
//!
//! Either one rolls back to the version it came from (same mechanism,
//! backwards) and leaves a warning in main's thread. A probation without
//! failure marks the version good. `versions.json` in the state dir holds
//! `current`, `good` and `previous` (version dirs).

use crate::client;
use crate::paths::Paths;
use crate::util::now_ms;
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const PROBATION: Duration = Duration::from_secs(120);

pub fn state_file(paths: &Paths) -> PathBuf {
    paths.state.join("versions.json")
}

pub fn fail_file(paths: &Paths) -> PathBuf {
    paths.state.join("probation-fail")
}

pub fn read_state(paths: &Paths) -> Value {
    state_of(std::fs::read_to_string(state_file(paths)).ok().as_deref())
}

/// The state file's text as a JSON object: `st["current"] = ...` panics
/// on an array or a number, so a hand-edited, truncated or foreign file
/// reads as empty.
fn state_of(text: Option<&str>) -> Value {
    text.and_then(|s| serde_json::from_str(s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn write_state(paths: &Paths, v: &Value) {
    let tmp = paths.state.join("versions.json.tmp");
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, state_file(paths));
    }
}

/// True while a switch is on probation (a hub then reports its
/// failures in `probation-fail`).
pub fn on_probation(paths: &Paths) -> bool {
    read_state(paths)
        .get("probation_until")
        .and_then(|x| x.as_u64())
        .is_some_and(|t| t > now_ms())
}

/// A hub on probation reports a failure (the switcher rolls back).
pub fn report_failure(paths: &Paths, reason: &str) {
    if on_probation(paths) && !fail_file(paths).exists() {
        let _ = std::fs::write(fail_file(paths), reason);
    }
}

/// The version an app root is (its `VERSION` file), as `key=value`.
pub fn version_info(root: &Path) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("dir".into(), json!(root.to_string_lossy()));
    if let Ok(t) = std::fs::read_to_string(root.join("VERSION")) {
        for l in t.lines() {
            if let Some((k, v)) = l.split_once('=') {
                m.insert(k.to_string(), json!(v));
            }
        }
    }
    Value::Object(m)
}

fn log(paths: &Paths, s: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
    {
        let _ = writeln!(f, "{} switch: {}", now_ms(), s);
    }
}

fn hub_pid(paths: &Paths) -> Option<u32> {
    std::fs::read_to_string(paths.pid_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

use crate::procs::alive;

fn ping(paths: &Paths) -> bool {
    client::request(
        &paths.socket(),
        &json!({"op": "ping"}),
        Duration::from_secs(3),
    )
    .is_ok()
}

/// A line in main's thread (and every client), through the hub.
pub fn notice(paths: &Paths, kind: &str, text: &str) {
    if let Ok(mut s) = UnixStream::connect(paths.socket()) {
        let v = json!({"op": "notice", "kind": kind, "text": text});
        let _ = s.write_all(format!("{}\n", v).as_bytes());
    }
}

/// The app root the running hub was started from (its executable's dir).
pub fn running_root(paths: &Paths) -> Option<PathBuf> {
    let pid = hub_pid(paths)?;
    // the hub writes its app root at start (`ps` may truncate the path)
    if alive(pid) {
        if let Ok(r) = std::fs::read_to_string(paths.state.join("hub.root")) {
            let r = PathBuf::from(r.trim());
            if r.join("repl-live").exists() {
                return Some(r);
            }
        }
    }
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    let exe = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let root = Path::new(&exe).parent()?.to_path_buf();
    if !root.is_absolute() {
        return None;
    }
    // the dev tree runs rust/target/debug/bise from the repo
    if root.join("repl-live").exists() {
        Some(root)
    } else {
        root.ancestors()
            .skip(1)
            .take(3)
            .find(|d| d.join("repl-live").exists())
            .map(|d| d.to_path_buf())
    }
}

/// The command's file in a version dir (BISE-165; was `bend-harness`).
pub const EXE: &str = "bise";

/// The executable of an app root: a version dir's `bise` (or the
/// `bend-harness` of a version built before the rename), the dev tree's
/// debug build.
pub fn exe_of(root: &Path) -> Option<PathBuf> {
    [EXE, "bend-harness", "rust/target/debug/bise", "rust/target/debug/bend-harness"]
        .iter()
        .map(|f| root.join(f))
        .find(|p| p.exists())
}

/// `root/bise sbd`, detached (its own process group), like
/// `client::start_hub`; the child handle tells a hub that died at once.
/// `same`: the version running now (a restart, a reload): when `root`
/// has no binary (a dev tree built in a CARGO_TARGET_DIR), the running
/// hub's own, this switcher's; else the root's, as before (a dev tree's
/// fresh `rust/target/debug/bise`).
fn start_hub(paths: &Paths, root: &Path, same: bool) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"))?;
    let mine = || std::env::current_exe().ok().filter(|_| same);
    let exe = exe_of(root).or_else(mine).unwrap_or_else(|| root.join(EXE));
    Command::new(exe)
        .arg("sbd")
        .arg("--workspace")
        .arg(&paths.workspace)
        // the new version's own files, not the old hub's inherited root
        .env("BISE_APP_ROOT", root)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .process_group(0)
        .spawn()
}

/// Stop the hub, agents kept; then start the hub of `root`. Err: why
/// the new hub is not up.
fn replace_hub(paths: &Paths, root: &Path, same: bool) -> Result<(), String> {
    let old = hub_pid(paths);
    let _ = client::stop(paths, true);
    if let Some(p) = old {
        let t0 = Instant::now();
        while alive(p) && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if alive(p) {
            // not answering: it cannot stop cleanly; its REPLs survive it
            crate::procs::kill_now(p);
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let _ = std::fs::remove_file(paths.socket());
    if !same {
        // an older version's hub would write its sb script through the link
        crate::daemon::drop_sb_link(&paths.bin_dir());
    }
    let mut child = start_hub(paths, root, same).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) {
        if ping(paths) {
            return Ok(());
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let err = std::fs::read_to_string(paths.state.join("hub.err")).unwrap_or_default();
    let tail = err
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    Err(format!(
        "the hub did not start{}",
        if tail.is_empty() {
            String::new()
        } else {
            format!(" ({})", crate::util::clip(tail, 200))
        }
    ))
}

/// The id of the version at `root` (from its `VERSION` file); None for
/// a dev tree.
pub fn version_id(root: &Path) -> Option<String> {
    version_info(root)
        .get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
}

/// The version id, else the path (for messages).
pub fn id_of(root: &Path) -> String {
    version_id(root).unwrap_or_else(|| root.to_string_lossy().to_string())
}

/// Watch the new hub for the probation period. Err: the reason to roll back.
fn probation(paths: &Paths, period: Duration) -> Result<(), String> {
    let t0 = Instant::now();
    let mut misses = 0;
    while t0.elapsed() < period {
        std::thread::sleep(Duration::from_secs(1));
        if let Ok(r) = std::fs::read_to_string(fail_file(paths)) {
            return Err(r);
        }
        match hub_pid(paths) {
            Some(p) if alive(p) => {}
            _ => return Err("the hub stopped (crash)".into()),
        }
        if ping(paths) {
            misses = 0;
        } else {
            misses += 1;
            if misses >= 4 {
                return Err("the hub no longer answers".into());
            }
        }
    }
    Ok(())
}

/// The hub whose state is `state` runs (a live socket or pid) or is being
/// switched: the migration to ~/.bise leaves it in place (BISE-161).
pub fn hub_busy(state: &Path) -> bool {
    let paths = Paths { workspace: PathBuf::new(), state: state.to_path_buf(), worktrees: PathBuf::new() };
    UnixStream::connect(paths.socket()).is_ok() || hub_pid(&paths).is_some_and(alive) || switch_running(&paths)
}

/// A switcher is running (a switch on probation).
pub fn switch_running(paths: &Paths) -> bool {
    std::fs::read_to_string(paths.state.join("switch.pid"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .is_some_and(alive)
}

/// Roll back the switch on probation now (the user asked).
pub fn abort_probation(paths: &Paths) {
    let _ = std::fs::write(fail_file(paths), "rollback requested");
}

/// `sbswitch --to <version dir> [--probation <s>]`: the whole switch.
/// Copy the hub state (journal, sessions, transcripts) aside before a
/// switch or a restart: `/tmp/sb-backup-<state dir name>-<ms>`.
fn backup(paths: &Paths) -> Option<PathBuf> {
    let name = paths.state.file_name()?.to_string_lossy().to_string();
    let dest = PathBuf::from(format!("/tmp/sb-backup-{}-{}", name, now_ms()));
    let ok = std::process::Command::new("rsync")
        .args(["-a", "--exclude", "hub.sock"])
        .arg(format!("{}/", paths.state.display()))
        .arg(format!("{}/", dest.display()))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(dest)
}

/// The workspace is bise's own source tree (dev mode, BISE-131): there
/// `/restart` builds the latest commit, then switches to it; anywhere
/// else it reloads the running version (nothing built).
pub fn dev_workspace(ws: &Path) -> bool {
    versions_script(ws).is_some() && ws.join("rust/switchboard/Cargo.toml").is_file()
}

/// The script that builds a version of the source tree `repo`:
/// scripts/versions.sh, or versions.sh at the top of a tree from before
/// the root cleanup.
pub fn versions_script(repo: &Path) -> Option<PathBuf> {
    ["scripts/versions.sh", "versions.sh"].iter().map(|p| repo.join(p)).find(|p| p.is_file())
}

/// Left by a reload's switcher for the hub it starts (BISE-131): that hub
/// reloads every REPL it adopts and tells the TUIs to re-exec. It holds
/// the reload's id (ms), the hub takes it at boot.
pub fn reload_file(paths: &Paths) -> PathBuf {
    paths.state.join("reload")
}

/// At boot: the id of the reload that started this hub, if one did.
pub fn take_reload(paths: &Paths) -> Option<String> {
    let f = reload_file(paths);
    let id = std::fs::read_to_string(&f).ok()?.trim().to_string();
    let _ = std::fs::remove_file(&f);
    (!id.is_empty()).then_some(id)
}

/// `restart`: `to` may be the running version (a plain restart of the
/// hub, e.g. a stuck one); the agents are kept the same way. `reload`
/// (BISE-131, implies `restart`): the new hub also relaunches every
/// agent's REPL (at its next idle, same session) and the TUIs.
pub fn run(paths: &Paths, to: &Path, period: Duration, restart: bool, reload: bool) -> i32 {
    let lock = paths.state.join("switch.pid");
    if let Some(p) = std::fs::read_to_string(&lock)
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        if alive(p) && p != std::process::id() {
            notice(paths, "warn", "a version switch is already in progress");
            return 1;
        }
    }
    let _ = std::fs::write(&lock, std::process::id().to_string());
    let code = run_locked(paths, to, period, restart || reload, reload);
    let _ = std::fs::remove_file(&lock);
    code
}

fn run_locked(paths: &Paths, to: &Path, period: Duration, restart: bool, reload: bool) -> i32 {
    let to = to.canonicalize().unwrap_or_else(|_| to.to_path_buf());
    if !to.join("repl-live").exists() {
        notice(
            paths,
            "warn",
            &format!("version not found: {}", to.display()),
        );
        return 1;
    }
    let mut st = read_state(paths);
    let from = running_root(paths).or_else(|| {
        st.get("current")
            .and_then(|x| x.as_str())
            .map(PathBuf::from)
    });
    let Some(from) = from.map(|p| p.canonicalize().unwrap_or(p)) else {
        notice(
            paths,
            "warn",
            "current version unknown: no rollback possible, switch cancelled",
        );
        return 1;
    };
    if from == to && !restart {
        notice(
            paths,
            "info",
            &format!("already on version {}", id_of(&to)),
        );
        return 0;
    }
    let (from_id, to_id) = (id_of(&from), id_of(&to));
    log(paths, &format!("{} -> {}", from.display(), to.display()));
    let saved = backup(paths)
        .map(|d| format!(" · state backed up: {}", d.display()))
        .unwrap_or_default();
    let what = if reload {
        format!("reloading bise on version {}: the hub, every agent and the TUI restart, nothing lost", to_id)
    } else if from == to {
        format!("restarting the hub on version {}", to_id)
    } else if restart {
        format!("restarting the hub on version {} (from {})", to_id, from_id)
    } else {
        format!("switching to version {} (from {})", to_id, from_id)
    };
    notice(
        paths,
        "info",
        &format!("{}{}{}", what, if reload { "" } else { " — the agents keep running" }, saved),
    );
    let _ = std::fs::remove_file(fail_file(paths));
    if from != to {
        st["previous"] = json!(from.to_string_lossy());
    }
    st["current"] = json!(to.to_string_lossy());
    if st.get("good").is_none() {
        st["good"] = json!(from.to_string_lossy());
    }
    st["probation_until"] = json!(now_ms() + period.as_millis() as u64);
    write_state(paths, &st);
    if reload {
        let _ = std::fs::write(reload_file(paths), now_ms().to_string());
    }

    let outcome = replace_hub(paths, &to, from == to).and_then(|_| probation(paths, period));
    let mut st = read_state(paths);
    st["probation_until"] = json!(0);
    match outcome {
        Ok(()) => {
            st["good"] = json!(to.to_string_lossy());
            write_state(paths, &st);
            log(paths, &format!("{} good", to_id));
            notice(
                paths,
                "info",
                &if reload {
                    format!("bise reloaded on version {} (probation passed)", to_id)
                } else if from == to {
                    format!("hub restarted on version {} (probation passed)", to_id)
                } else {
                    format!("version {} validated (probation passed)", to_id)
                },
            );
            0
        }
        Err(reason) => {
            log(
                paths,
                &format!("{} failed: {} — rollback to {}", to_id, reason, from_id),
            );
            // a restart on the same version goes back to the last good
            // one when there is another, else it tries that version again
            let from = if from == to {
                st.get("good")
                    .and_then(|x| x.as_str())
                    .map(PathBuf::from)
                    .filter(|g| g.canonicalize().map(|c| c != to).unwrap_or(false))
                    .unwrap_or_else(|| from.clone())
            } else {
                from
            };
            let from_id = id_of(&from);
            st["current"] = json!(from.to_string_lossy());
            if from != to {
                st["previous"] = json!(to.to_string_lossy());
            }
            st["failed"] =
                json!({"version": to.to_string_lossy(), "reason": reason, "at": now_ms()});
            write_state(paths, &st);
            let _ = std::fs::remove_file(fail_file(paths));
            let back = replace_hub(paths, &from, from == to);
            let text = format!(
                "version {} failed ({}) — rollback to version {}{}",
                to_id,
                reason,
                from_id,
                match &back {
                    Ok(()) => String::new(),
                    Err(e) => format!(": ROLLBACK FAILED, {}", e),
                }
            );
            notice(paths, "warn", &text);
            // the hub may still be booting: the notice is also left for it
            let _ = std::fs::write(paths.state.join("switch-notice"), &text);
            2
        }
    }
}

/// A `bise` launch (BISE-255): the hub of the workspace runs `hub`, the
/// launched bise is `me`. True when the hub should move to `me`: `me` is
/// the installed `current` (what `bise update` or the daily check
/// installed), `hub` an older build of the same install, and a switch to
/// `me` did not fail in this workspace (its probation rolled back: the
/// hub stays where it is, `/restart` tries again).
pub fn should_follow_install(me: &Path, hub: &Path, failed: Option<&Path>) -> bool {
    use bise_home::release::{read_version, Install};
    let canon = |p: &Path| p.canonicalize().ok();
    let (Some(me), Some(hub)) = (canon(me), canon(hub)) else { return false };
    if me == hub || failed.and_then(canon).as_ref() == Some(&me) {
        return false;
    }
    let (Some(mine), Some(theirs)) = (Install::of_root(&me), Install::of_root(&hub)) else {
        return false;
    };
    if mine.prefix != theirs.prefix || mine.current().as_ref() != Some(&me) {
        return false;
    }
    let built = |r: &Path| read_version(r).get("built").cloned().unwrap_or_default();
    let (b_me, b_hub) = (built(&me), built(&hub));
    !b_me.is_empty() && b_me > b_hub
}

/// Before a `bise` TUI attaches to the hub of its workspace (BISE-255):
/// a hub on an older installed version moves to the launched one (the
/// hub's own `/version <dir>`: the switcher, probation, agents kept, an
/// agent in a turn moves at its next idle), and this waits for the new
/// hub (<= 30 s) so the TUI attaches to it instead of following the old
/// one. Every hub since BISE-131 takes the request. `say`: one line for
/// the user each (the terminal is not the TUI's yet).
pub fn follow_install(paths: &Paths, me: &Path, say: &dyn Fn(&str)) {
    use std::io::{BufRead, BufReader};
    if switch_running(paths) {
        return;
    }
    let Some(hub) = running_root(paths) else { return };
    let failed = read_state(paths)
        .pointer("/failed/version")
        .and_then(|x| x.as_str())
        .map(PathBuf::from);
    if !should_follow_install(me, &hub, failed.as_deref()) {
        return;
    }
    let me = me.canonicalize().unwrap_or_else(|_| me.to_path_buf());
    let (from_id, to_id) = (id_of(&hub), id_of(&me));
    say(&format!("this folder's hub runs bise {}: moving it to {} (the agents keep running)…", from_id, to_id));
    let answer = (|| -> std::io::Result<String> {
        let mut s = UnixStream::connect(paths.socket())?;
        let req = json!({"op": "version", "do": "switch", "to": me.to_string_lossy()});
        s.write_all(format!("{{\"op\":\"hello\"}}\n{}\n", req).as_bytes())?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        // the hub's hello first (tens of KB), then the answer: a notice
        for line in BufReader::new(s).lines() {
            let v: Value = serde_json::from_str(&line?).unwrap_or(Value::Null);
            if v.get("ev").and_then(|x| x.as_str()) == Some("notice") {
                return Ok(v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string());
            }
        }
        Ok(String::new())
    })()
    .unwrap_or_default();
    if !answer.starts_with("switching to version") {
        say(&format!("the hub did not switch ({}): /restart in it switches to {}", clip_answer(&answer), to_id));
        return;
    }
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(200));
        let there = running_root(paths).and_then(|r| r.canonicalize().ok()).as_ref() == Some(&me);
        if there && ping(paths) {
            say(&format!("the hub runs bise {} now", to_id));
            return;
        }
    }
    say(&format!("the hub is still switching to {} (probation): the TUI follows it once it is up", to_id));
}

fn clip_answer(a: &str) -> String {
    if a.trim().is_empty() {
        "no answer".into()
    } else {
        crate::util::clip(a.trim(), 160)
    }
}

#[cfg(test)]
mod state_tests {
    #[test]
    fn a_launch_moves_the_hub_to_a_newer_installed_current_only() {
        use super::should_follow_install as follow;
        use std::path::Path;
        let t = std::env::temp_dir().join(format!("sb-follow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let mk = |prefix: &Path, id: &str, extra: &str| {
            let d = prefix.join("versions").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("VERSION"), format!("id={}\n{}", id, extra)).unwrap();
            d
        };
        let p = t.join("prefix");
        let old = mk(&p, "old", "built=2026-09-29T14:00:00Z\n");
        let new = mk(&p, "new", "built=2026-09-30T01:00:00Z\n");
        std::os::unix::fs::symlink("versions/new", p.join("current")).unwrap();
        assert!(follow(&new, &old, None), "the installed current, newer: the hub moves");
        assert!(!follow(&new, &new, None), "already there");
        assert!(!follow(&old, &new, None), "an older bise launched never moves the hub back");
        assert!(!follow(&new, &old, Some(&new)), "a switch to it failed here: stay");
        assert!(follow(&new, &old, Some(&old)), "another version failed: no matter");
        // not the installed current (a hand-run versions/<id>/bise): no move
        let newer = mk(&p, "newer", "built=2026-10-01T00:00:00Z\n");
        assert!(!follow(&newer, &old, None));
        // a dev hub (repl=) or another install: never touched
        let dev = mk(&p, "dev", "built=2026-01-01T00:00:00Z\nrepo=/src\n");
        assert!(!follow(&new, &dev, None));
        let q = t.join("other");
        let elsewhere = mk(&q, "x", "built=2026-01-01T00:00:00Z\n");
        std::os::unix::fs::symlink("versions/x", q.join("current")).unwrap();
        assert!(!follow(&new, &elsewhere, None));
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn dev_mode_is_bise_source_tree() {
        let d = std::env::temp_dir().join(format!("sb-devws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("rust/switchboard")).unwrap();
        assert!(!super::dev_workspace(&d), "any workspace: a reload");
        std::fs::create_dir_all(d.join("scripts")).unwrap();
        std::fs::write(d.join("scripts/versions.sh"), "").unwrap();
        assert!(!super::dev_workspace(&d));
        std::fs::write(d.join("rust/switchboard/Cargo.toml"), "").unwrap();
        assert!(super::dev_workspace(&d), "bise's sources: build then switch");
        // a tree from before the root cleanup: versions.sh at the top
        std::fs::remove_dir_all(d.join("scripts")).unwrap();
        assert!(!super::dev_workspace(&d));
        std::fs::write(d.join("versions.sh"), "").unwrap();
        assert!(super::dev_workspace(&d), "old layout");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_reload_file_is_taken_once() {
        let d = std::env::temp_dir().join(format!("sb-reload-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let paths = crate::paths::Paths { workspace: d.clone(), state: d.clone(), worktrees: d.join("worktrees") };
        assert_eq!(super::take_reload(&paths), None);
        std::fs::write(super::reload_file(&paths), "123\n").unwrap();
        assert_eq!(super::take_reload(&paths).as_deref(), Some("123"));
        assert_eq!(super::take_reload(&paths), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_non_object_state_file_reads_as_empty() {
        for t in [None, Some(""), Some("[1]"), Some("3"), Some("null"), Some("{\"good\": \"x\"")] {
            let mut st = super::state_of(t);
            st["current"] = serde_json::json!("v"); // never panics
            assert_eq!(st["current"], "v");
        }
        assert_eq!(super::state_of(Some("{\"good\": \"x\"}"))["good"], "x");
    }
}
