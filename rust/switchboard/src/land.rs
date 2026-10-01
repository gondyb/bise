//! `sb land` (dev-flow §5): what this repo did by hand (a private
//! `GIT_INDEX_FILE` from the current tip, `commit-tree`, a compare-and-
//! swap `update-ref`, then the place's index synced), as one checked step
//! the hub runs for an agent, off its loop (the daemon's thread).
//!
//! - `--here`: the agent's own files (the hub's list, RFC 0001 §10.3) are
//!   committed on its place's branch: the shared folder's (main), or its
//!   worktree's. Another agent's half-done files are never in it; a file
//!   another agent of the place also changed is refused (main decides).
//! - plain, from a worktree: its own files first (with the message), then
//!   the branch is rebased on main (the worktree must be clean: every
//!   agent landed its files), the repo's check runs if main had moved,
//!   main moves to the branch (fast-forward); in trunk flow with `push`,
//!   main is pushed. From the shared folder: `--here`, then the push.
//! - One land at a time per target ref ([`Queue`]): the others wait in
//!   line, and the views say so (`waits to land · 2nd`).

use crate::flow::{FlowConfig, FlowMode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

/// What the hub knows when an agent asks to land (core.rs builds it from
/// the state; the daemon adds the repo's flow).
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub agent: String,
    pub here: bool,
    pub message: String,
    /// The place: its id, its folder, and whether it is a worktree.
    pub place: String,
    pub dir: PathBuf,
    pub worktree: bool,
    /// The shared folder (main's checkout).
    pub shared: PathBuf,
    /// The agent's files and the other agents' of the place (not
    /// archived), as the hub tracked them (absolute or relative).
    pub files: Vec<String>,
    pub others: Vec<(String, Vec<String>)>,
    pub flow: FlowConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The ref that moved (`main`, `sb/x`), its new tip (short), how many
    /// commits it gained.
    pub target: String,
    pub sha: String,
    pub commits: usize,
    /// None: no push tried; Some(false): tried, failed (`push_error`).
    pub pushed: Option<bool>,
    pub push_error: Option<String>,
}

// ---- the queue: one land at a time per target ref ----

#[derive(Default)]
struct Line {
    /// (ticket, target ref, place id), the first one landing.
    waiting: Vec<(u64, String, String)>,
    next: u64,
}

/// The land queue, shared by the daemon's land threads.
#[derive(Clone, Default)]
pub struct Queue {
    inner: Arc<(Mutex<Line>, Condvar)>,
}

/// A place in line; dropping it leaves the line.
pub struct Turn {
    q: Queue,
    ticket: u64,
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (m, cv) = &*self.q.inner;
        let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
        l.waiting.retain(|(t, _, _)| *t != self.ticket);
        cv.notify_all();
    }
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{}{}", n, suffix)
}

impl Queue {
    /// Join the line for `target` and wait until first. `joined`: called
    /// once in line (the views refresh: the lid says it waits).
    pub fn wait_turn(&self, target: &str, place: &str, joined: &mut dyn FnMut()) -> Turn {
        let (m, cv) = &*self.inner;
        let ticket = {
            let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
            l.next += 1;
            let t = l.next;
            l.waiting.push((t, target.to_string(), place.to_string()));
            t
        };
        joined();
        let mut l = m.lock().unwrap_or_else(|e| e.into_inner());
        while l.waiting.iter().find(|(_, r, _)| r == target).map(|(t, _, _)| *t) != Some(ticket) {
            l = cv.wait(l).unwrap_or_else(|e| e.into_inner());
        }
        Turn {
            q: self.clone(),
            ticket,
        }
    }

    /// The held line of each place in line (pr-design §4.1, the lid):
    /// `landing` for the first, `waits to land · 2nd` for the next.
    pub fn lids(&self) -> BTreeMap<String, String> {
        let (m, _) = &*self.inner;
        let l = m.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = BTreeMap::new();
        let mut rank: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, r, p) in &l.waiting {
            let n = rank.entry(r.as_str()).or_insert(0);
            let lid = if *n == 0 { "landing".to_string() } else { format!("waits to land · {}", ordinal(*n + 1)) };
            out.entry(p.clone()).or_insert(lid);
            *n += 1;
        }
        out
    }
}

// ---- git ----

fn git_in(dir: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut cmd = crate::tools_env::git_command()?;
    cmd.arg("-C").arg(dir).args(args);
    if let Some(i) = index {
        cmd.env("GIT_INDEX_FILE", i);
    }
    let out = cmd.output().map_err(|e| format!("git could not start: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_in(dir, args, None)
}

/// A tracked file path, relative to the place's folder: absolute paths
/// outside it are not the place's (a private worktree of `gate.sh new`).
pub fn relative(dir: &Path, f: &str) -> Option<String> {
    let p = Path::new(f);
    let rel = if p.is_absolute() {
        let d = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let pc = p.parent().and_then(|x| x.canonicalize().ok()).map(|x| x.join(p.file_name().unwrap_or_default()));
        let p = pc.as_deref().unwrap_or(p);
        p.strip_prefix(&d).ok().or_else(|| p.strip_prefix(dir).ok())?.to_string_lossy().to_string()
    } else {
        f.trim_start_matches("./").to_string()
    };
    (!rel.is_empty() && !rel.starts_with("..")).then_some(rel)
}

/// Of `files`, those that differ from the checkout's HEAD (changed,
/// added, deleted), in order.
fn changed(dir: &Path, files: &[String]) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all", "--"];
    args.extend(files.iter().map(String::as_str));
    let out = git(dir, &args)?;
    let mut seen: Vec<String> = Vec::new();
    let mut parts = out.split('\0').filter(|s| !s.is_empty());
    while let Some(e) = parts.next() {
        let (xy, path) = e.split_at(e.len().min(3));
        seen.push(path.to_string());
        if xy.starts_with('R') || xy.starts_with('C') {
            if let Some(old) = parts.next() {
                seen.push(old.to_string());
            }
        }
    }
    Ok(files.iter().filter(|f| seen.contains(f)).cloned().collect())
}

/// The ref the checkout at `dir` is on (`refs/heads/main`).
fn head_ref(dir: &Path) -> Result<String, String> {
    git(dir, &["symbolic-ref", "-q", "HEAD"]).map_err(|_| format!("{} is not on a branch (detached HEAD)", dir.display()))
}

fn short(r: &str) -> &str {
    r.strip_prefix("refs/heads/").unwrap_or(r)
}

fn tmp_index(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sb-land-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()))
}

/// Commit `files` (relative to `dir`, as they are in it) on `target`
/// through a private index built from its tip, then move it with a
/// compare-and-swap: a tip moved meanwhile (someone else committed)
/// rebuilds from the new one, so nothing of theirs is lost. `after_read`:
/// tests only (a commit lands between the read and the swap).
pub fn commit_files(
    dir: &Path,
    target: &str,
    files: &[String],
    message: &str,
    after_read: &mut dyn FnMut(),
) -> Result<String, String> {
    for _ in 0..5 {
        let old = git(dir, &["rev-parse", "--verify", target])?;
        let idx = tmp_index("idx");
        let res = (|| {
            git_in(dir, &["read-tree", &old], Some(&idx))?;
            let mut args = vec!["update-index", "--add", "--remove", "--"];
            args.extend(files.iter().map(String::as_str));
            git_in(dir, &args, Some(&idx))?;
            let tree = git_in(dir, &["write-tree"], Some(&idx))?;
            if tree == git(dir, &["rev-parse", &format!("{}^{{tree}}", old)])? {
                return Err("nothing to land: your files are already committed".to_string());
            }
            git(dir, &["commit-tree", "-p", &old, "-m", message, &tree])
        })();
        let _ = std::fs::remove_file(&idx);
        let new = res?;
        after_read();
        if git(dir, &["update-ref", target, &new, &old]).is_ok() {
            sync_index(dir, target, files);
            return Ok(new);
        }
    }
    Err(format!("{} kept moving: try again", short(target)))
}

/// The checkout's index follows the new tip for those paths, so `git
/// status` stays clean there (the "D / ??" lag of 18b7443). Only when
/// the checkout is on `target`; a locked index is retried.
fn sync_index(dir: &Path, target: &str, files: &[String]) {
    if head_ref(dir).as_deref() != Ok(target) || files.is_empty() {
        return;
    }
    let mut args = vec!["reset", "-q", "--"];
    args.extend(files.iter().map(String::as_str));
    for _ in 0..10 {
        if git(dir, &args).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The agent's files of the place that differ from its tip, refused when
/// another agent of the place changed one too.
fn own_changes(job: &Job) -> Result<Vec<String>, String> {
    let mine: Vec<String> = job.files.iter().filter_map(|f| relative(&job.dir, f)).collect();
    let mine = changed(&job.dir, &mine)?;
    for f in &mine {
        let who: Vec<&str> = job
            .others
            .iter()
            .filter(|(_, fs)| fs.iter().filter_map(|x| relative(&job.dir, x)).any(|x| &x == f))
            .map(|(n, _)| n.as_str())
            .collect();
        if !who.is_empty() {
            return Err(format!(
                "{} is also changed by @{}: not landed, main decides who lands it",
                f,
                who.join(", @")
            ));
        }
    }
    Ok(mine)
}

fn shorten(dir: &Path, sha: &str) -> String {
    git(dir, &["rev-parse", "--short", sha]).unwrap_or_else(|_| sha.chars().take(7).collect())
}

/// Run `check` in `dir` (`sh -c`); its output's tail when it fails.
fn run_check(dir: &Path, check: &str) -> Result<(), String> {
    let out = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(check)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("the check `{}` could not start: {}", check, e))?;
    if out.status.success() {
        return Ok(());
    }
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Err(format!("the check `{}` failed: {}", check, crate::util::clip_tail(text.trim(), 600)))
}

/// Push `target` to its remote (trunk flow, `push = true`): a refused
/// push fetches; main behind the remote is rebased (only in a clean
/// shared folder), then one more try. Err: why it is not pushed.
fn push(shared: &Path, target: &str) -> Result<(), String> {
    let remote = git(shared, &["remote"])?.lines().next().map(str::to_string);
    let Some(remote) = remote else {
        return Err("no remote".into());
    };
    let b = short(target);
    let spec = format!("{}:{}", target, target);
    if git(shared, &["push", "-q", &remote, &spec]).is_ok() {
        return Ok(());
    }
    git(shared, &["fetch", "-q", &remote, b])?;
    let theirs = format!("{}/{}", remote, b);
    let behind = git(shared, &["merge-base", "--is-ancestor", &theirs, target]).is_err();
    if behind {
        let clean = git(shared, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
        if !clean || head_ref(shared).as_deref() != Ok(target) {
            return Err(format!("{} moved on {}; not rebased: the shared folder has changes", b, remote));
        }
        git(shared, &["rebase", "-q", &theirs]).map_err(|e| {
            let _ = git(shared, &["rebase", "--abort"]);
            format!("{} moved on {} and the rebase conflicts: {}", b, remote, e)
        })?;
    }
    git(shared, &["push", "-q", &remote, &spec]).map(|_| ())
}

fn pushed(job: &Job, target: &str, main: &str) -> (Option<bool>, Option<String>) {
    if job.flow.mode != Some(FlowMode::Trunk) || !job.flow.push || target != main {
        return (None, None);
    }
    match push(&job.shared, target) {
        Ok(()) => (Some(true), None),
        Err(e) if e == "no remote" => (None, None),
        Err(e) => (Some(false), Some(e)),
    }
}

/// `sb land` for `job`, in line on `queue`. `joined`: the views refresh.
pub fn run(job: &Job, queue: &Queue, joined: &mut dyn FnMut()) -> Result<Outcome, String> {
    let main = head_ref(&job.shared)?;
    if !job.here && job.worktree && job.flow.mode == Some(FlowMode::Pr) {
        return Err("this repo ships through pull requests: commit with `sb land --here`, then open a PR".into());
    }
    let mine = own_changes(job)?;
    if !job.here && job.worktree {
        return land_branch(job, queue, joined, &main, &mine);
    }
    if mine.is_empty() {
        return Err("nothing of yours to land: the files you changed match the branch".into());
    }
    if job.message.trim().is_empty() {
        return Err("sb land needs a commit message: sb land [--here] \"<message>\"".into());
    }
    let target = head_ref(&job.dir)?;
    let _turn = queue.wait_turn(&target, &job.place, joined);
    let new = commit_files(&job.dir, &target, &mine, &job.message, &mut || {})?;
    let (pushed, push_error) = pushed(job, &target, &main);
    Ok(Outcome {
        target: short(&target).to_string(),
        sha: shorten(&job.dir, &new),
        commits: 1,
        pushed,
        push_error,
    })
}

/// A worktree's branch onto main: its agent's files first, then rebase,
/// check, fast-forward.
fn land_branch(job: &Job, queue: &Queue, joined: &mut dyn FnMut(), main: &str, mine: &[String]) -> Result<Outcome, String> {
    let branch = head_ref(&job.dir)?;
    if !mine.is_empty() {
        if job.message.trim().is_empty() {
            return Err("you have changes not committed: give a message (sb land \"<message>\"), or commit them with sb land --here first".into());
        }
        commit_files(&job.dir, &branch, mine, &job.message, &mut || {})?;
    }
    let dirty = git(&job.dir, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        let files: Vec<&str> = dirty.lines().map(|l| l.get(3..).unwrap_or(l)).take(5).collect();
        return Err(format!(
            "the worktree has changes not committed ({}): each agent lands its own with sb land --here first",
            files.join(", ")
        ));
    }
    let _turn = queue.wait_turn(main, &job.place, joined);
    let base = git(&job.shared, &["rev-parse", "--verify", main])?;
    let tip = git(&job.dir, &["rev-parse", "HEAD"])?;
    if git(&job.dir, &["merge-base", "--is-ancestor", &tip, &base]).is_ok() {
        return Err(format!("nothing to land: {} has no commit that {} lacks", short(&branch), short(main)));
    }
    let moved = git(&job.dir, &["merge-base", "--is-ancestor", &base, &tip]).is_err();
    if moved {
        if let Err(e) = git(&job.dir, &["rebase", "-q", &base]) {
            let conflicts = git(&job.dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            let _ = git(&job.dir, &["rebase", "--abort"]);
            let files: Vec<&str> = conflicts.lines().collect();
            return Err(if files.is_empty() {
                format!("the rebase on {} failed: {}", short(main), e)
            } else {
                format!(
                    "{} changed on {} too: the rebase conflicts. rebase {} on {} yourself, then sb land again",
                    files.join(", "),
                    short(main),
                    short(&branch),
                    short(main)
                )
            });
        }
        if let Some(check) = &job.flow.check {
            run_check(&job.dir, check)?;
        }
    }
    let tip = git(&job.dir, &["rev-parse", "HEAD"])?;
    let commits = git(&job.dir, &["rev-list", "--count", &format!("{}..{}", base, tip)])?
        .parse::<usize>()
        .unwrap_or(0);
    if head_ref(&job.shared).as_deref() == Ok(main) {
        // main is checked out in the shared folder: a fast-forward there
        // moves its files too (refused if someone's changes are in the way)
        git(&job.shared, &["merge", "--ff-only", "-q", &tip]).map_err(|e| {
            format!("{} could not move: {} (the shared folder has changes on the same files, or {} moved)", short(main), e, short(main))
        })?;
    } else {
        git(&job.shared, &["update-ref", main, &tip, &base]).map_err(|_| format!("{} moved meanwhile: sb land again", short(main)))?;
    }
    let (pushed, push_error) = pushed(job, main, main);
    Ok(Outcome {
        target: short(main).to_string(),
        sha: shorten(&job.shared, &tip),
        commits,
        pushed,
        push_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, script: &str) {
        let ok = Command::new("/bin/sh").arg("-c").arg(script).current_dir(dir).status().unwrap().success();
        assert!(ok, "{}", script);
    }

    fn repo(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sb-land-test-{}-{}-{}", tag, std::process::id(), crate::util::now_ms()));
        let ws = root.join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        sh(
            &ws,
            "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo a > a && echo b > b && echo c > c && git add . && git commit -qm init",
        );
        ws.canonicalize().unwrap()
    }

    fn job(agent: &str, dir: &Path, shared: &Path, files: &[&str], others: &[(&str, &[&str])]) -> Job {
        Job {
            agent: agent.into(),
            here: true,
            message: format!("{}'s work", agent),
            place: "shared".into(),
            dir: dir.to_path_buf(),
            worktree: dir != shared,
            shared: shared.to_path_buf(),
            files: files.iter().map(|s| s.to_string()).collect(),
            others: others
                .iter()
                .map(|(n, fs)| (n.to_string(), fs.iter().map(|s| s.to_string()).collect()))
                .collect(),
            flow: FlowConfig::default(),
        }
    }

    fn lines(xs: &[&str]) -> String {
        xs.join("\u{a}")
    }

    fn log(dir: &Path) -> String {
        git(dir, &["log", "--format=%s", "main"]).unwrap()
    }

    #[test]
    fn land_here_commits_only_my_files_and_keeps_the_status_clean() {
        let ws = repo("here");
        // mine: a (changed), n (new); another agent's: b; the user's: c
        sh(&ws, "echo a2 > a && echo new > n && echo b2 > b && echo c2 > c");
        let abs_a = ws.join("a").to_string_lossy().to_string();
        let j = job("x", &ws, &ws, &[&abs_a, "n", "/elsewhere/z"], &[("y", &["b"])]);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!((o.target.as_str(), o.commits, o.pushed), ("main", 1, None));
        assert_eq!(log(&ws), lines(&["x's work", "init"]));
        let files = git(&ws, &["show", "--name-only", "--format=", "main"]).unwrap();
        assert_eq!(files.lines().collect::<Vec<_>>(), ["a", "n"]);
        // the shared index follows: a and n clean, b and c still the others'
        let st = git(&ws, &["status", "--porcelain"]).unwrap();
        assert_eq!(st.lines().collect::<Vec<_>>(), [" M b", " M c"]);
        // again: nothing of mine left
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("nothing of yours"), "{}", e);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn an_overlap_is_refused() {
        let ws = repo("overlap");
        sh(&ws, "echo a2 > a");
        let j = job("x", &ws, &ws, &["a"], &[("y", &["a", "b"])]);
        let e = run(&j, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("a is also changed by @y") && e.contains("main decides"), "{}", e);
        assert_eq!(log(&ws), "init", "nothing landed");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_cas_race_keeps_the_other_commit() {
        let ws = repo("cas");
        sh(&ws, "echo a2 > a");
        let mut once = true;
        let mut race = || {
            if std::mem::take(&mut once) {
                // someone commits b on main between our read and our swap
                let w = ws.clone();
                sh(&w, "git worktree add -q ../other main 2>/dev/null || git worktree add -q --detach ../other main; cd ../other && echo b2 > b && git -c commit.gpgsign=false commit -qam other && git update-ref refs/heads/main HEAD");
            }
        };
        let new = commit_files(&ws, "refs/heads/main", &["a".into()], "mine", &mut race).unwrap();
        assert_eq!(git(&ws, &["rev-parse", "main"]).unwrap(), new);
        assert_eq!(log(&ws), lines(&["mine", "other", "init"]), "rebuilt on the moved tip");
        assert_eq!(git(&ws, &["show", "main:b"]).unwrap(), "b2", "their commit is kept");
        assert_eq!(git(&ws, &["show", "main:a"]).unwrap(), "a2");
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn two_agents_in_one_worktree_then_the_branch_lands_on_main() {
        let ws = repo("wt");
        let wt = ws.parent().unwrap().join("wt");
        sh(&ws, &format!("git worktree add -q -b sb/x {} main", wt.display()));
        let wt = wt.canonicalize().unwrap();
        // x and y share the worktree; each lands only its own file
        sh(&wt, "echo a2 > a && echo b2 > b");
        let mut jx = job("x", &wt, &ws, &["a"], &[("y", &["b"])]);
        jx.place = "wt:x".into();
        run(&jx, &Queue::default(), &mut || {}).unwrap();
        let st = git(&wt, &["status", "--porcelain"]).unwrap();
        assert_eq!(st.trim(), "M b", "y's file is untouched");
        // the plain land refuses while y's file is not committed
        jx.here = false;
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("not committed (b)"), "{}", e);
        let mut jy = job("y", &wt, &ws, &["b"], &[("x", &["a"])]);
        jy.place = "wt:x".into();
        run(&jy, &Queue::default(), &mut || {}).unwrap();
        // main moved meanwhile (c): the branch is rebased, checked, then main
        // fast-forwards, the shared folder's files with it
        sh(&ws, "echo c2 > c && git commit -qam c-on-main");
        jx.flow.check = Some("test -f a".into());
        let o = run(&jx, &Queue::default(), &mut || {}).unwrap();
        assert_eq!((o.target.as_str(), o.commits), ("main", 2));
        assert_eq!(log(&ws), lines(&["y's work", "x's work", "c-on-main", "init"]));
        assert_eq!(std::fs::read_to_string(ws.join("a")).unwrap().trim_end(), "a2");
        assert!(git(&ws, &["status", "--porcelain"]).unwrap().is_empty());
        // a failing check after a rebase: main does not move
        sh(&ws, "echo c3 > c && git commit -qam c3");
        sh(&wt, "echo a3 > a");
        jx.flow.check = Some("echo broken; exit 1".into());
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("the check `echo broken; exit 1` failed: broken"), "{}", e);
        assert_eq!(log(&ws).lines().next(), Some("c3"));
        // PR flow: no landing on main
        jx.flow.mode = Some(FlowMode::Pr);
        let e = run(&jx, &Queue::default(), &mut || {}).unwrap_err();
        assert!(e.contains("pull requests"), "{}", e);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }

    #[test]
    fn the_queue_lands_one_at_a_time_per_ref_and_says_who_waits() {
        let q = Queue::default();
        let first = q.wait_turn("refs/heads/main", "wt:a", &mut || {});
        let q2 = q.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let h = std::thread::spawn(move || {
            let _t = q2.wait_turn("refs/heads/main", "wt:b", &mut || {});
            tx.send(()).unwrap();
        });
        // another ref is not in that line
        let other = q.wait_turn("refs/heads/sb/c", "wt:c", &mut || {});
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(rx.try_recv().is_err(), "b waits while a lands");
        let lids = q.lids();
        assert_eq!(lids["wt:a"], "landing");
        assert_eq!(lids["wt:b"], "waits to land · 2nd");
        assert_eq!(lids["wt:c"], "landing");
        drop(first);
        rx.recv_timeout(std::time::Duration::from_secs(5)).expect("b's turn");
        h.join().unwrap();
        drop(other);
        assert!(q.lids().is_empty());
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(11), "11th");
    }

    #[test]
    fn trunk_pushes_main_after_a_land() {
        let ws = repo("push");
        let remote = ws.parent().unwrap().join("remote.git");
        sh(&ws, &format!("git init -q --bare {} && git remote add origin {} && git push -q origin main", remote.display(), remote.display()));
        sh(&ws, "echo a2 > a");
        let mut j = job("x", &ws, &ws, &["a"], &[]);
        j.flow.mode = Some(FlowMode::Trunk);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(o.pushed, Some(true));
        assert_eq!(git(&remote, &["log", "--format=%s", "-1", "main"]).unwrap(), "x's work");
        // the remote moved: fetch, rebase (the shared folder is clean), push
        let other = ws.parent().unwrap().join("other");
        sh(&ws, &format!("git clone -q {} {} && cd {} && git config user.email o@o && git config user.name o && echo c2 > c && git -c commit.gpgsign=false commit -qam theirs && git push -q origin main", remote.display(), other.display(), other.display()));
        sh(&ws, "echo b2 > b");
        let mut j = job("x", &ws, &ws, &["b"], &[]);
        j.flow.mode = Some(FlowMode::Trunk);
        let o = run(&j, &Queue::default(), &mut || {}).unwrap();
        assert_eq!(o.pushed, Some(true), "{:?}", o.push_error);
        assert_eq!(git(&remote, &["log", "--format=%s", "main"]).unwrap(), lines(&["x's work", "theirs", "x's work", "init"]));
        // push = false: lands stay local
        sh(&ws, "echo a3 > a");
        let mut j = job("x", &ws, &ws, &["a"], &[]);
        j.flow = FlowConfig { mode: Some(FlowMode::Trunk), check: None, push: false };
        assert_eq!(run(&j, &Queue::default(), &mut || {}).unwrap().pushed, None);
        let _ = std::fs::remove_dir_all(ws.parent().unwrap());
    }
}
