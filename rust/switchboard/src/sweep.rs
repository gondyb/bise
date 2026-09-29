//! Task worktrees live in one folder per task, under the project's
//! `Paths::worktrees` (`~/.bise/worktrees/<project-id>/<task>/`, like
//! Codex's `~/.codex/worktrees`):
//!
//! ```text
//! <task>/<repo>     the git worktree (`sb spawn --worktree`, gate.sh new)
//! <task>/target     gate.sh's cargo target (this repo only), a cache
//! <task>/owner      the task it belongs to (else the folder's name)
//! ```
//!
//! The hub removes the folder of a task that is archived (at its /drop)
//! or unknown (at the hub's start), when no worktree in it has
//! uncommitted changes or commits that no branch or remote has. It never
//! deletes such work: it keeps the folder and says so. `migrate` moves
//! the worktrees of the old place (`<state>/worktrees/<task>`) here.

use crate::core::Loss;
use crate::worktree::git;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The file naming a task folder's owner (gate.sh writes `$SB_AGENT`).
pub const OWNER: &str = "owner";

/// A folder of an unknown task younger than this is left alone: a
/// `gate.sh new` of a task this hub does not know yet may be filling it.
pub const GRACE_MS: u64 = 10 * 60 * 1000;

/// Who may own a task folder, from the hub's state.
#[derive(Clone, Debug, Default)]
pub struct Owners {
    /// The names, dirs and old names of the tasks that are not archived.
    pub live: BTreeSet<String>,
    /// The same for the archived ones.
    pub archived: BTreeSet<String>,
    /// The worktrees live tasks work in (their workspace, `sb worktree`).
    pub paths: BTreeSet<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Removed(PathBuf),
    /// Kept, and why (the work a removal would lose).
    Kept(PathBuf, String),
}

/// The task a folder belongs to: its `owner` file, else its name.
pub fn owner_of(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(OWNER))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// A linked git worktree has a `.git` file (a clone a `.git` folder).
fn is_linked(p: &Path) -> bool {
    p.join(".git").is_file()
}

fn is_repo(p: &Path) -> bool {
    p.join(".git").is_dir()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// The worktrees of a task folder: the folder itself (the old layout),
/// else its sub-folders that are one.
fn checkouts(dir: &Path) -> Vec<PathBuf> {
    if is_linked(dir) {
        return vec![dir.to_path_buf()];
    }
    children(dir).into_iter().filter(|p| is_linked(p)).collect()
}

/// What removing a worktree would lose: its uncommitted changes
/// (untracked files included, ignored ones not), and the commits of its
/// HEAD that no other branch and no remote has.
pub fn loss(checkout: &Path) -> Result<Loss, String> {
    let dirty = git(checkout, &["status", "--porcelain"])?.lines().filter(|l| !l.trim().is_empty()).count();
    let branch = git(checkout, &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
    let mut args = vec!["rev-list", "HEAD", "--not", "--remotes"];
    let exclude = format!("--exclude={}", branch);
    if !branch.is_empty() {
        args.push(&exclude);
    }
    args.push("--branches");
    let unpushed = git(checkout, &args)?.lines().filter(|l| !l.trim().is_empty()).count();
    Ok(Loss { dirty, unpushed })
}

pub fn describe(l: &Loss) -> String {
    let mut parts = Vec::new();
    if l.dirty > 0 {
        parts.push(format!("{} uncommitted change{}", l.dirty, if l.dirty == 1 { "" } else { "s" }));
    }
    if l.unpushed > 0 {
        parts.push(format!("{} commit{} on no other branch", l.unpushed, if l.unpushed == 1 { "" } else { "s" }));
    }
    parts.join(", ")
}

/// The repository a linked worktree belongs to (its common git dir).
fn common_dir(checkout: &Path) -> Result<PathBuf, String> {
    git(checkout, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).map(PathBuf::from)
}

/// Remove a worktree (checked by the caller) from its repository; its
/// branch goes too when the hub made it (`prefix`, e.g. `sb/`).
fn remove_checkout(checkout: &Path, prefix: &str) -> Result<(), String> {
    let common = common_dir(checkout)?;
    let branch = git(checkout, &["symbolic-ref", "-q", "--short", "HEAD"]).unwrap_or_default();
    git(&common, &["worktree", "remove", "--force", &checkout.to_string_lossy()])?;
    let _ = git(&common, &["worktree", "prune"]);
    if !prefix.is_empty() && branch.starts_with(prefix) {
        let _ = git(&common, &["branch", "-D", &branch]);
    }
    Ok(())
}

fn age_ms(dir: &Path, now: u64) -> u64 {
    let ms = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    };
    now.saturating_sub(ms(dir).max(ms(&dir.join(OWNER))))
}

/// Remove a task folder whose worktrees lose nothing: the worktrees from
/// their repository, then the rest (the target, the owner file). Kept,
/// with the reason, when one would lose work, cannot be measured (its
/// repository is gone) or when the folder holds a whole repository.
pub fn remove_task_dir(dir: &Path, prefix: &str) -> Outcome {
    let kept = |why: String| Outcome::Kept(dir.to_path_buf(), why);
    let cs = checkouts(dir);
    for c in &cs {
        match loss(c) {
            Ok(l) if l.any() => return kept(describe(&l)),
            Ok(_) => {}
            Err(e) => return kept(format!("git cannot read {}: {}", c.display(), e)),
        }
    }
    if let Some(r) = children(dir).into_iter().find(|p| is_repo(p)) {
        return kept(format!("{} is a repository, not a worktree", r.display()));
    }
    for c in &cs {
        if let Err(e) = remove_checkout(c, prefix) {
            return kept(e);
        }
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Outcome::Removed(dir.to_path_buf()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Outcome::Removed(dir.to_path_buf()),
        Err(e) => kept(format!("rm: {}", e)),
    }
}

/// Clean `root` (one project's task folders): each folder of an archived
/// or unknown task is removed when it loses nothing (`remove_task_dir`).
/// `only`: the folders of these tasks only (a /drop), else every one
/// (the hub's start). A dangling link (`migrate` left it) goes too.
pub fn sweep(root: &Path, owners: &Owners, only: Option<&BTreeSet<String>>, now: u64, prefix: &str) -> Vec<Outcome> {
    let mut out = Vec::new();
    for d in children(root) {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&d) else { continue };
        if meta.file_type().is_symlink() {
            if !d.exists() && std::fs::remove_file(&d).is_ok() {
                out.push(Outcome::Removed(d));
            }
            continue;
        }
        if !meta.is_dir() {
            continue;
        }
        let names = [owner_of(&d), name];
        if only.is_some_and(|o| !names.iter().any(|n| o.contains(n))) {
            continue;
        }
        if names.iter().any(|n| owners.live.contains(n)) || owners.paths.iter().any(|p| p.starts_with(&d)) {
            continue;
        }
        let known = names.iter().any(|n| owners.archived.contains(n));
        if !known && age_ms(&d, now) < GRACE_MS {
            continue;
        }
        out.push(remove_task_dir(&d, prefix));
    }
    out
}

/// A free task folder `root/<name>` (`<name>-2`, ... when taken).
pub fn free_task_dir(root: &Path, name: &str) -> PathBuf {
    let first = root.join(name);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    (2..)
        .map(|i| root.join(format!("{}-{}", name, i)))
        .find(|p| std::fs::symlink_metadata(p).is_err())
        .unwrap()
}

/// The name a project's worktrees get in their task folder.
pub fn repo_name(workspace: &Path) -> String {
    workspace
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty() && n != "target" && n != OWNER)
        .unwrap_or_else(|| "repo".into())
}

/// Move the worktrees of the old place (`legacy`: `<state>/worktrees/<task>`)
/// to `root/<task>/<repo>` (`git worktree move`: a rename, so a process
/// working in one follows it), leaving a link at the old path for the
/// agents that still have it. Answers (old, new) per move, and the errors
/// (the worktree stays where it is).
pub fn migrate(legacy: &Path, root: &Path, repo: &str) -> (Vec<(PathBuf, PathBuf)>, Vec<String>) {
    let (mut moved, mut errors) = (Vec::new(), Vec::new());
    if legacy == root {
        return (moved, errors);
    }
    for old in children(legacy) {
        let Ok(meta) = std::fs::symlink_metadata(&old) else { continue };
        if meta.file_type().is_symlink() || !is_linked(&old) {
            continue;
        }
        let name = old.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dir = free_task_dir(root, &name);
        // the folder keeps the worktree's age: an old orphan is not a new
        // folder the sweep's grace would spare
        let since = meta.modified().ok();
        let new = dir.join(repo);
        let r = (|| {
            std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {}", dir.display(), e))?;
            let common = common_dir(&old)?;
            git(&common, &["worktree", "move", &old.to_string_lossy(), &new.to_string_lossy()])?;
            let _ = std::fs::write(dir.join(OWNER), format!("{}\n", name));
            std::os::unix::fs::symlink(&new, &old).map_err(|e| format!("link {}: {}", old.display(), e))?;
            if let Some(t) = since {
                for p in [dir.join(OWNER), dir.clone()] {
                    let _ = std::fs::File::open(&p).and_then(|f| f.set_modified(t));
                }
            }
            Ok::<(), String>(())
        })();
        match r {
            Ok(()) => moved.push((old, new)),
            Err(e) => {
                let _ = std::fs::remove_dir(&dir);
                errors.push(format!("{}: {}", old.display(), e));
            }
        }
    }
    (moved, errors)
}

/// The journal's workspaces at a path `migrate` moved (a link in the
/// old place, maybe through the link of a hub folder the home migration
/// moved, BISE-161) name their new place under `root`, so the hub
/// (sb-core) knows it.
pub fn follow_moves(events: &mut [Value], root: &Path) -> usize {
    let Ok(real_root) = root.canonicalize() else { return 0 };
    let mut n = 0;
    for ev in events.iter_mut() {
        let Some(p) = ev["ws"]["path"].as_str().map(PathBuf::from) else { continue };
        if p.starts_with(root) {
            continue;
        }
        let Some(rest) = p.canonicalize().ok().and_then(|c| c.strip_prefix(&real_root).ok().map(Path::to_path_buf)) else {
            continue;
        };
        ev["ws"]["path"] = Value::String(root.join(rest).to_string_lossy().into_owned());
        n += 1;
    }
    n
}

/// After a sweep: the old place, once empty, goes.
pub fn drop_empty(dir: &Path) {
    let _ = std::fs::remove_dir(dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, script: &str) {
        let ok = Command::new("/bin/sh").arg("-c").arg(script).current_dir(dir).status().unwrap().success();
        assert!(ok, "{}", script);
    }

    /// A repo `repo/` and a project root `wt/` in a temp dir.
    fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let t = std::env::temp_dir().join(format!("sb-sweep-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()));
        let repo = t.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        sh(&repo, "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo a > f && git add f && git commit -qm init");
        let t = t.canonicalize().unwrap();
        let root = t.join("wt");
        std::fs::create_dir_all(&root).unwrap();
        (t.clone(), t.join("repo"), root)
    }

    /// A task folder like gate.sh new makes: a detached worktree and a target.
    fn task(repo: &Path, root: &Path, name: &str, owner: Option<&str>) -> PathBuf {
        let d = root.join(name);
        std::fs::create_dir_all(d.join("target/debug")).unwrap();
        std::fs::write(d.join("target/debug/big"), "x").unwrap();
        if let Some(o) = owner {
            std::fs::write(d.join(OWNER), o).unwrap();
        }
        sh(repo, &format!("git worktree add -q --detach {} HEAD", d.join("repo").display()));
        d
    }

    fn owners(live: &[&str], archived: &[&str]) -> Owners {
        Owners {
            live: live.iter().map(|s| s.to_string()).collect(),
            archived: archived.iter().map(|s| s.to_string()).collect(),
            paths: BTreeSet::new(),
        }
    }

    fn listed(repo: &Path, p: &Path) -> bool {
        git(repo, &["worktree", "list", "--porcelain"]).unwrap().contains(&*p.to_string_lossy())
    }

    /// The start's sweep: an archived task's clean folder goes (worktree,
    /// target), a live one's stays, a dirty or unique-commit one stays
    /// with the reason, an unknown young one waits, an unknown old one goes.
    #[test]
    fn the_sweep_removes_clean_orphans_only() {
        let (t, repo, root) = setup("orphans");
        let clean = task(&repo, &root, "clean", Some("gone"));
        let live = task(&repo, &root, "live", None);
        let dirty = task(&repo, &root, "dirty", None);
        sh(&dirty.join("repo"), "echo b >> f && echo new > n");
        let ahead = task(&repo, &root, "ahead", None);
        sh(&ahead.join("repo"), "echo c >> f && git commit -qam mine");
        let young = task(&repo, &root, "young", None);
        let o = owners(&["live"], &["gone", "dirty", "ahead"]);
        let now = crate::util::now_ms();
        let out = sweep(&root, &o, None, now, "sb/");
        assert!(out.contains(&Outcome::Removed(clean.clone())), "{:?}", out);
        assert!(!clean.exists() && !listed(&repo, &clean.join("repo")));
        assert!(live.join("repo/f").exists() && young.exists());
        assert!(out.contains(&Outcome::Kept(dirty.clone(), "2 uncommitted changes".into())), "{:?}", out);
        assert!(out.contains(&Outcome::Kept(ahead.clone(), "1 commit on no other branch".into())), "{:?}", out);
        assert!(dirty.join("repo/n").exists() && ahead.join("target/debug/big").exists());
        // later, the unknown folder is an orphan
        let out = sweep(&root, &o, None, now + GRACE_MS + 60_000, "sb/");
        assert!(out.contains(&Outcome::Removed(young.clone())), "{:?}", out);
        // the commit reaches a branch: nothing is lost any more
        sh(&repo, &format!("git branch keep {}", git(&ahead.join("repo"), &["rev-parse", "HEAD"]).unwrap()));
        assert_eq!(remove_task_dir(&ahead, "sb/"), Outcome::Removed(ahead.clone()));
        let _ = std::fs::remove_dir_all(t);
    }

    /// A /drop: only the dropped task's folders, found by its owner file.
    #[test]
    fn a_drop_sweeps_its_own_folders() {
        let (t, repo, root) = setup("drop");
        let mine = task(&repo, &root, "gate-name", Some("fix"));
        let other = task(&repo, &root, "other", Some("gone"));
        let only: BTreeSet<String> = ["fix".to_string()].into();
        let out = sweep(&root, &owners(&[], &["fix", "gone"]), Some(&only), crate::util::now_ms(), "sb/");
        assert_eq!(out, vec![Outcome::Removed(mine.clone())]);
        assert!(!mine.exists() && other.exists());
        // a live task's worktree path protects its folder
        let mut o = owners(&[], &["gone"]);
        o.paths.insert(other.join("repo"));
        assert!(sweep(&root, &o, None, crate::util::now_ms(), "sb/").is_empty());
        let _ = std::fs::remove_dir_all(t);
    }

    /// The old place: worktrees move (a link stays), the journal follows,
    /// a dirty one moves with its changes.
    #[test]
    fn migrate_moves_old_worktrees_and_the_journal_follows() {
        let (t, repo, root) = setup("migrate");
        let legacy = t.join("state/worktrees");
        std::fs::create_dir_all(&legacy).unwrap();
        sh(&repo, &format!("git worktree add -q -b sb/a {}", legacy.join("a").display()));
        sh(&legacy.join("a"), "echo dirty >> f");
        // `a` is taken in the new place: the next free name
        std::fs::create_dir_all(root.join("a")).unwrap();
        let (moved, errors) = migrate(&legacy, &root, "repo");
        assert!(errors.is_empty(), "{:?}", errors);
        let new = root.join("a-2/repo");
        assert_eq!(moved, vec![(legacy.join("a"), new.clone())]);
        assert_eq!(std::fs::read_to_string(new.join("f")).unwrap(), "a\ndirty\n");
        assert!(listed(&repo, &new));
        assert_eq!(owner_of(&root.join("a-2")), "a");
        assert_eq!(std::fs::read_link(legacy.join("a")).unwrap(), new);
        let mut evs = vec![
            serde_json::json!({"type": "task_created", "ws": {"path": legacy.join("a").to_string_lossy()}}),
            serde_json::json!({"type": "workspace_changed", "ws": {"path": "/elsewhere"}}),
        ];
        assert_eq!(follow_moves(&mut evs, &root), 1);
        assert_eq!(evs[0]["ws"]["path"], new.to_string_lossy().as_ref());
        // twice is a no-op
        assert!(migrate(&legacy, &root, "repo").0.is_empty());
        // the task is archived: kept (dirty); clean, it goes, and the link
        // left in the old place, now dangling, with it
        let o = owners(&[], &["a"]);
        let out = sweep(&root, &o, None, crate::util::now_ms(), "sb/");
        assert_eq!(
            out,
            vec![Outcome::Removed(root.join("a")), Outcome::Kept(root.join("a-2"), "1 uncommitted change".into())],
            "the empty `a` goes, the dirty one stays"
        );
        sh(&new, "git checkout -q -- f");
        let out = sweep(&root, &o, None, crate::util::now_ms(), "sb/");
        assert!(out.contains(&Outcome::Removed(root.join("a-2"))), "{:?}", out);
        assert!(git(&repo, &["show-ref", "--verify", "--quiet", "refs/heads/sb/a"]).is_err(), "the hub's branch goes");
        assert_eq!(sweep(&legacy, &o, None, crate::util::now_ms(), "sb/"), vec![Outcome::Removed(legacy.join("a"))]);
        drop_empty(&legacy);
        assert!(!legacy.exists());
        let _ = std::fs::remove_dir_all(t);
    }
}
