//! The one-time move to `~/.bise` (BISE-161, portable-bise.md §3.3).
//!
//! Run by the harness at its start (TUI, hub, headless; never by `sb`),
//! under a lock, when `$BISE_HOME` is not set and `BISE_NO_MIGRATE` is not:
//!
//! 1. **Once** (no `~/.bise/migrated.json` yet): the user files are
//!    **copied**, never moved: `~/.bend-harness/*` → `~/.bise/` (the
//!    indexes into `cache/`, `run/` left out), `tui.json` + the old
//!    `hints.json`, `tip`, `onboarded` → `prefs.json`, `drafts/` copied.
//!    `~/.bise/dev/{versions,build}` are links to the old folders: the
//!    running hubs and `switch.json` name those paths, and the build cache
//!    is shared with running gates. The old folders stay whole: an older
//!    version still reads them.
//! 2. **Every start**: each hub folder still in
//!    `~/.local/state/switchboard/<name>-<hash>` moves to
//!    `~/.bise/hubs/<name>-<hash>` when it is idle (no live socket, no live
//!    pid, no switch on probation: the caller's `busy`). The old path
//!    becomes a symlink to the new one, so an older binary (a rollback, an
//!    old checkout's run.sh) computing the old path opens the SAME hub;
//!    `git worktree repair` then points the repositories at the moved
//!    worktrees. A running hub stays where it is ([`crate::Home::hub_dir`]
//!    keeps using it) and moves at the first start after it stopped.
//! 3. `migrated.json` records what was copied and moved; written last, it
//!    turns the bise layout on.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{Lookup, MIGRATED};

/// Set to 1 (or anything): never migrate.
pub const NO_MIGRATE: &str = "BISE_NO_MIGRATE";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// This run was the first one (user files copied, marker written).
    pub first: bool,
    /// Entries copied from the old places (relative names).
    pub copied: Vec<String>,
    /// Hub ids moved to `hubs/` by this run.
    pub hubs_moved: Vec<String>,
    /// Hub ids left in the old place because they run.
    pub hubs_kept: Vec<String>,
    /// What failed (the rest went on).
    pub errors: Vec<String>,
}

/// The migration applies to this environment: a HOME, no `$BISE_HOME`
/// (an explicit home is never filled from the old places), no `BISE_NO_MIGRATE`.
pub fn wanted(env: Lookup) -> bool {
    let get = |k: &str| env(k).filter(|v| !v.is_empty());
    get("HOME").is_some() && get(crate::BISE_HOME).is_none() && get(NO_MIGRATE).is_none()
}

/// A hub folder's name: `<name>-<8 hex>` (`switchboard::paths::workspace_id`).
pub fn is_hub_id(name: &str) -> bool {
    match name.rsplit_once('-') {
        Some((base, h)) => {
            !base.is_empty()
                && base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                && h.len() == 8
                && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        None => false,
    }
}

/// Migrate the old places of `user` (a HOME) into `user/.bise`. `busy(dir)`:
/// the hub in `dir` runs (it stays in place).
pub fn migrate(user: &Path, busy: &dyn Fn(&Path) -> bool) -> io::Result<Report> {
    let bise = user.join(".bise");
    let old_root = user.join(".bend-harness");
    let old_state = user.join(".local").join("state").join("switchboard");
    create_private_dir(&bise)?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(bise.join(".migrate.lock"))?;
    lock.lock()?;
    let marker = bise.join(MIGRATED);
    let mut rep = Report { first: !marker.exists(), ..Report::default() };
    if rep.first {
        copy_user_files(&old_root, &old_state, &bise, &mut rep);
    }
    move_hubs(&old_state, &bise.join("hubs"), busy, &mut rep);
    if rep.first || !rep.hubs_moved.is_empty() || !rep.errors.is_empty() {
        record(&marker, &rep)?;
    }
    Ok(rep)
}

fn create_private_dir(d: &Path) -> io::Result<()> {
    if d.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(d)?;
    set_mode(d, 0o700)
}

fn set_mode(p: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))
}

// ---- 1. the user files (once) ----

fn copy_user_files(old_root: &Path, old_state: &Path, bise: &Path, rep: &mut Report) {
    let note = |r: io::Result<()>, what: String, rep: &mut Report| match r {
        Ok(()) => rep.copied.push(what),
        Err(e) => rep.errors.push(format!("copy {what}: {e}")),
    };
    if let Ok(rd) = std::fs::read_dir(old_root) {
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names {
            let src = old_root.join(&name);
            let dest = match name.as_str() {
                // per-session side channels: nothing to keep
                "run" => continue,
                // merged into prefs.json below
                "tui.json" => continue,
                "mcp-index.txt" | "skills-index.txt" => bise.join("cache").join(&name),
                _ => bise.join(&name),
            };
            note(copy_tree(&src, &dest), format!("~/.bend-harness/{name}"), rep);
        }
    }
    let drafts = old_state.join("drafts");
    if drafts.is_dir() {
        note(copy_tree(&drafts, &bise.join("drafts")), "~/.local/state/switchboard/drafts".into(), rep);
    }
    match write_prefs(old_root, old_state, &bise.join("prefs.json")) {
        Ok(true) => rep.copied.push("prefs".into()),
        Ok(false) => {}
        Err(e) => rep.errors.push(format!("prefs: {e}")),
    }
    // the dev versions and build cache stay where they are: links to them
    for d in ["versions", "build"] {
        let (src, dest) = (old_state.join(d), bise.join("dev").join(d));
        if src.is_dir() && std::fs::symlink_metadata(&dest).is_err() {
            let r = std::fs::create_dir_all(bise.join("dev")).and_then(|_| std::os::unix::fs::symlink(&src, &dest));
            note(r, format!("~/.local/state/switchboard/{d} (link)"), rep);
        }
    }
}

/// Copy `src` (a file, a symlink or a folder) to `dest`; what exists at
/// `dest` is kept (a second run, a file the new version already wrote).
/// Files keep their mode; on APFS a copy is a clone (no space used).
pub fn copy_tree(src: &Path, dest: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(src)?;
    if std::fs::symlink_metadata(dest).is_ok() && !meta.is_dir() {
        return Ok(());
    }
    if meta.file_type().is_symlink() {
        return std::os::unix::fs::symlink(std::fs::read_link(src)?, dest);
    }
    if meta.is_dir() {
        if !dest.is_dir() {
            std::fs::create_dir_all(dest)?;
            std::fs::set_permissions(dest, meta.permissions())?;
        }
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_tree(&e.path(), &dest.join(e.file_name()))?;
        }
        return Ok(());
    }
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::copy(src, dest).map(|_| ())
}

/// prefs.json from `tui.json` (voice, theme, any other key) and the old
/// `hints.json`, `tip`, `onboarded`. Keys already in prefs.json win.
/// False: nothing to write.
fn write_prefs(old_root: &Path, old_state: &Path, dest: &Path) -> io::Result<bool> {
    let read_json = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let mut prefs = serde_json::Map::new();
    if let Some(Value::Object(tui)) = read_json(old_root.join("tui.json")) {
        prefs.extend(tui);
    }
    if let Some(h @ Value::Object(_)) = read_json(old_state.join("hints.json")) {
        prefs.insert("hints".into(), h);
    }
    if let Some(t @ Value::Number(_)) = read_json(old_state.join("tip")) {
        prefs.insert("tip".into(), t);
    }
    if old_state.join("onboarded").exists() {
        prefs.insert("onboarded".into(), Value::Bool(true));
    }
    if prefs.is_empty() {
        return Ok(false);
    }
    if let Some(Value::Object(mine)) = read_json(dest.to_path_buf()) {
        prefs.extend(mine);
    }
    crate::prefs::write_atomic(dest, &(serde_json::to_string_pretty(&Value::Object(prefs))? + "\n"))?;
    Ok(true)
}

// ---- 2. the hubs (every start) ----

fn move_hubs(old_state: &Path, hubs: &Path, busy: &dyn Fn(&Path) -> bool, rep: &mut Report) {
    let Ok(rd) = std::fs::read_dir(old_state) else { return };
    let mut ids: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) // a symlink: moved already
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_hub_id(n))
        .collect();
    ids.sort();
    for id in ids {
        let (old, new) = (old_state.join(&id), hubs.join(&id));
        if busy(&old) {
            rep.hubs_kept.push(id);
            continue;
        }
        if std::fs::symlink_metadata(&new).is_ok() {
            rep.errors.push(format!("hub {id}: {} exists, the old one stays", new.display()));
            continue;
        }
        match move_hub(&old, &new) {
            Ok(()) => rep.hubs_moved.push(id),
            Err(e) => rep.errors.push(format!("hub {id}: {e}")),
        }
    }
    if !rep.hubs_moved.is_empty() {
        let _ = std::fs::write(
            old_state.join("MOVED"),
            format!(
                "The hubs of bise moved to {} (BISE-161). Each <name>-<hash> here is a link to its new folder.\n",
                hubs.display()
            ),
        );
    }
}

/// `old` → `new` (a rename: same disk, instant, open files keep working),
/// a link at `old`, then the worktrees repaired.
fn move_hub(old: &Path, new: &Path) -> io::Result<()> {
    std::fs::create_dir_all(new.parent().unwrap_or(Path::new("/")))?;
    std::fs::rename(old, new)?;
    if let Err(e) = std::os::unix::fs::symlink(new, old) {
        // no link: put it back, the old place keeps working
        let _ = std::fs::rename(new, old);
        return Err(e);
    }
    repair_worktrees(new);
    Ok(())
}

/// `git worktree repair` in each moved worktree: its repository's
/// `.git/worktrees/<n>/gitdir` names the new path (the old one still
/// works through the link, so a failure here breaks nothing).
fn repair_worktrees(hub: &Path) {
    let Ok(rd) = std::fs::read_dir(hub.join("worktrees")) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.join(".git").is_file() {
            let _ = std::process::Command::new("git")
                .arg("-C")
                .arg(&p)
                .args(["worktree", "repair"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}

// ---- 3. the record ----

fn record(marker: &Path, rep: &Report) -> io::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let mut v = std::fs::read_to_string(marker)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({"version": 1, "at": now, "copied": [], "hubs_moved": [], "errors": []}));
    let push = |v: &mut Value, k: &str, items: &[String]| {
        if let Some(a) = v.get_mut(k).and_then(|a| a.as_array_mut()) {
            a.extend(items.iter().map(|s| Value::String(s.clone())));
        } else {
            v[k] = json!(items);
        }
    };
    push(&mut v, "copied", &rep.copied);
    let moved: Vec<String> = rep.hubs_moved.iter().map(|id| format!("{id} ({now})")).collect();
    push(&mut v, "hubs_moved", &moved);
    push(&mut v, "errors", &rep.errors);
    v["hubs_waiting"] = json!(rep.hubs_kept);
    v["updated"] = json!(now);
    crate::prefs::write_atomic(marker, &(serde_json::to_string_pretty(&v)? + "\n"))
}
