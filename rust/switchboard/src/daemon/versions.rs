//! The versions of Switchboard itself, as the hub serves them: `/version`
//! and `sb version` (list, switch, roll back, restart), the picker items,
//! and the detached switcher that replaces this hub.

use super::{log_line, Msg, Shell};
use crate::model::MAIN;
use crate::paths::Paths;
use crate::util::{clip, wire_escape};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `sb version` from an agent: every agent may list the versions; only
/// main switches or rolls back (the user does it from the TUI).
pub(super) fn version_allowed(from: &str, what: &str) -> Result<(), String> {
    match what {
        "" | "list" => Ok(()),
        _ if from == MAIN => Ok(()),
        _ => Err(format!("sb version {}: reserved for main (the parent of the agents)", what)),
    }
}

/// What `/restart [<arg>]` restarts the hub on, in bise's source tree
/// (dev mode: unchanged by BISE-131).
#[derive(Debug, PartialEq)]
enum RestartTarget {
    /// `current`: the running version, nothing rebuilt.
    Current,
    /// no argument, `latest` or `head`: HEAD, built if needed.
    Latest,
    /// `<commit>`: that commit, built if needed.
    Rev(String),
}

/// What `/restart [<arg>]` does (BISE-131).
#[derive(Debug, PartialEq)]
enum RestartPlan {
    /// bise's source tree (dev mode): exactly as before BISE-131, build
    /// that target then switch (probation), or restart the hub on the
    /// running version (`current`, or the target already running).
    Dev(RestartTarget),
    /// any other workspace, an installed bise: reload the running
    /// version (hub, REPLs, TUIs), nothing built.
    Reload,
    /// any other workspace with a commit: refused, `/version` switches.
    Refuse,
}

fn restart_plan(dev: bool, arg: &str) -> RestartPlan {
    match (dev, arg.trim()) {
        (true, a) => RestartPlan::Dev(restart_target(a)),
        (false, "" | "current") => RestartPlan::Reload,
        (false, _) => RestartPlan::Refuse,
    }
}

fn restart_target(arg: &str) -> RestartTarget {
    match arg.trim() {
        "current" => RestartTarget::Current,
        "" | "latest" | "head" | "HEAD" => RestartTarget::Latest,
        r => RestartTarget::Rev(r.to_string()),
    }
}

/// `git log -<n>` of the repository versions are built from: one
/// `<short hash> <subject>` per line (empty when git fails), each line
/// cut to SUBJECT_MAX chars: the TUI never shows more, and a hello
/// with 40 subjects of 2 KB filled a client's socket buffer (the hub
/// blocked on a client that did not read yet).
fn recent_commits(repo: &Path, n: usize) -> String {
    crate::tools_env::git_command()
        .ok()
        .and_then(|mut c| c.args(["log", &format!("-{}", n), "--format=%h %s"]).current_dir(repo).output().ok())
        .map(|o| clip_lines(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

const SUBJECT_MAX: usize = 100;

fn clip_lines(log: &str) -> String {
    log.lines().map(|l| crate::util::clip(l, SUBJECT_MAX) + "\n").collect()
}

impl Shell {
    /// `version` op (`/version` in the TUI, `sb version`): list the
    /// versions, switch to one (built first when needed), roll back.
    /// Answers a text for the user.
    pub(super) fn version_op(&mut self, v: &Value) -> String {
        use crate::switch;
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let paths = self.opts.paths.clone();
        let st = switch::read_state(&paths);
        let root = &self.opts.app_root;
        let me = switch::version_info(root);
        let id_of = |p: &str| switch::id_of(Path::new(p));
        let (repo, versions_dir) = self.version_ctx();
        match s("do").as_str() {
            "" | "list" => {
                let cur = me.get("id").and_then(|x| x.as_str()).unwrap_or("dev tree");
                let mut out = vec![format!(
                    "current version: {} — {}",
                    cur,
                    me.get("subject").and_then(|x| x.as_str()).unwrap_or(&root.to_string_lossy())
                )];
                for (k, label) in [("good", "last good"), ("previous", "previous")] {
                    if let Some(p) = st.get(k).and_then(|x| x.as_str()) {
                        out.push(format!("{}: {}", label, id_of(p)));
                    }
                }
                if let Some(f) = st.get("failed") {
                    out.push(format!(
                        "last failure: {} ({})",
                        id_of(f.get("version").and_then(|x| x.as_str()).unwrap_or("")),
                        f.get("reason").and_then(|x| x.as_str()).unwrap_or("")
                    ));
                }
                let log = recent_commits(&repo, 12);
                out.push(format!("commits ({}) — ● built:", repo.display()));
                for l in log.lines() {
                    let h = l.split(' ').next().unwrap_or("");
                    let built = versions_dir.join(h).join("VERSION").exists();
                    out.push(format!("  {} {}", if built { "●" } else { "○" }, clip(l, 100)));
                }
                out.push(
                    "/version <commit>: switch to this commit (built if needed) · /version tree: the working tree · /version back: roll back"
                        .into(),
                );
                out.join("\n")
            }
            "rollback" | "back" if switch::switch_running(&paths) => {
                // on probation: the switcher itself goes back
                switch::abort_probation(&paths);
                "rolling back to the previous version (probation stopped)".into()
            }
            "restart" if switch::switch_running(&paths) => {
                "a version switch is in progress (probation): wait for it to end, or /version back".into()
            }
            "restart" => {
                // not bise's source tree: a reload, like VS Code's
                // "Reload Window" (BISE-131); nothing to build
                let target = match restart_plan(switch::dev_workspace(&self.opts.paths.workspace), &s("to")) {
                    RestartPlan::Reload => return self.reload(),
                    RestartPlan::Refuse => {
                        return "/restart reloads bise on the version running now (this workspace is not bise's source tree): nothing to build; /version switches versions".into()
                    }
                    RestartPlan::Dev(t) => t,
                };
                let latest = target == RestartTarget::Latest;
                let (repo, versions_dir) = self.version_ctx();
                let rev = match target {
                    RestartTarget::Current => String::new(),
                    RestartTarget::Latest => crate::tools_env::git_command()
                        .ok()
                        .and_then(|mut c| c.args(["rev-parse", "--short", "HEAD"]).current_dir(&repo).output().ok())
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default(),
                    RestartTarget::Rev(r) => r,
                };
                if rev.is_empty() && latest {
                    return format!(
                        "no latest commit found in {}: /restart current restarts on the running version",
                        repo.display()
                    );
                }
                let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
                let same = rev.is_empty()
                    || versions_dir
                        .join(&rev)
                        .canonicalize()
                        .map(|d| d == cur)
                        .unwrap_or(false);
                if same {
                    spawn_switcher(&paths, &self.opts.exe, &cur, Switcher::Restart);
                    return format!(
                        "restarting the hub on the current version {} — the agents keep running",
                        me.get("id").and_then(|x| x.as_str()).unwrap_or("(dev tree)")
                    );
                }
                self.version_op(&json!({"do": "switch", "to": rev}))
            }
            "switch" if switch::switch_running(&paths) => {
                "a version switch is in progress (probation): wait for it to end, or /version back".into()
            }
            "rollback" | "back" => {
                let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
                let target = ["good", "previous"].iter().find_map(|k| {
                    st.get(*k)
                        .and_then(|x| x.as_str())
                        .map(PathBuf::from)
                        .filter(|p| p.canonicalize().map(|c| c != cur).unwrap_or(false))
                });
                match target {
                    Some(t) => {
                        self.start_switch(&t);
                        format!("rolling back to version {}", id_of(&t.to_string_lossy()))
                    }
                    None => "no other version to roll back to".into(),
                }
            }
            "switch" => {
                let to = s("to");
                if to.is_empty() {
                    return "which version? (a commit, an id, a folder, or tree)".into();
                }
                // a version dir, a built id, else a git revision to build
                let dir = PathBuf::from(&to);
                let target = if switch::exe_of(&dir).is_some() {
                    Some(dir)
                } else if switch::exe_of(&versions_dir.join(&to)).is_some() {
                    Some(versions_dir.join(&to))
                } else {
                    None
                };
                if let Some(t) = target {
                    self.start_switch(&t);
                    return format!("switching to version {}", id_of(&t.to_string_lossy()));
                }
                let script = repo.join("versions.sh");
                if !script.exists() {
                    return format!("versions.sh not found in {}", repo.display());
                }
                let rev = if to == "tree" { "--tree".to_string() } else { to.clone() };
                if to != "tree" {
                    let known = crate::tools_env::git_command()
                        .ok()
                        .and_then(|mut c| {
                            c.args(["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", to)])
                                .current_dir(&repo)
                                .output()
                                .ok()
                        })
                        .is_some_and(|o| o.status.success());
                    if !known {
                        return format!(
                            "unknown commit {} in {} — type /version and pick one in the list",
                            to,
                            repo.display()
                        );
                    }
                }
                if !self.building.insert(to.clone()) {
                    return format!("{} is already being built", to);
                }
                self.feed(
                    MAIN,
                    &format!("sb info : {}", wire_escape(&format!("version {}: building…", to))),
                );
                self.broadcast_versions();
                let exe = self.opts.exe.clone();
                let tx = self.tx.clone();
                let answer = format!(
                    "building {} (nothing is interrupted), then switching to this version",
                    to
                );
                std::thread::spawn(move || {
                    // the build goes where version_ctx looks (bise_home), not
                    // where the script's own default would put it
                    let home = bise_home::Home::from_env();
                    let out = Command::new(&script)
                        .args(["build", &rev])
                        .env("SB_VERSIONS_DIR", &versions_dir)
                        .env("SB_BUILD_DIR", home.build_dir())
                        .current_dir(&repo)
                        .stdin(Stdio::null())
                        .output();
                    let note = |kind: &str, text: String| {
                        let _ = tx.send(Msg::Notice {
                            kind: kind.into(),
                            text,
                        });
                    };
                    let _ = tx.send(Msg::BuildEnded { rev: to.clone() });
                    match out {
                        Ok(o) if o.status.success() => {
                            let dir = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            note("info", format!("version {}: built, switching…", to));
                            spawn_switcher(&paths, &exe, Path::new(&dir), Switcher::Switch);
                        }
                        Ok(o) => {
                            let err = String::from_utf8_lossy(&o.stderr).to_string();
                            let tail: Vec<&str> = err.lines().rev().take(4).collect();
                            note(
                                "warn",
                                format!(
                                    "build of {} failed: {}",
                                    to,
                                    tail.into_iter().rev().collect::<Vec<_>>().join(" ⏎ ")
                                ),
                            );
                        }
                        Err(e) => note("warn", format!("build of {}: {}", to, e)),
                    }
                });
                answer
            }
            other => format!("version: unknown action {}", other),
        }
    }

    /// The repository versions are built from, and the versions dir.
    fn version_ctx(&self) -> (PathBuf, PathBuf) {
        let root = &self.opts.app_root;
        let repo = crate::switch::version_info(root)
            .get("repo")
            .and_then(|x| x.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| root.clone());
        let versions_dir = if root.join("VERSION").exists() {
            root.parent().map(|p| p.to_path_buf())
        } else {
            None
        }
        .unwrap_or_else(|| bise_home::Home::from_env().versions_dir());
        (repo, versions_dir)
    }

    /// The `/version` picker: `back`, `tree`, then the recent commits,
    /// each with its marks (current, good, built, building, failed, trial).
    pub(super) fn version_items(&self) -> Value {
        use crate::switch;
        let (repo, versions_dir) = self.version_ctx();
        let st = switch::read_state(&self.opts.paths);
        let me = switch::version_info(&self.opts.app_root);
        let cur_id = me.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id_at = |k: &str| -> String {
            st.get(k)
                .and_then(|x| x.as_str())
                .filter(|p| !p.is_empty())
                .and_then(|p| switch::version_id(Path::new(p)))
                .unwrap_or_default()
        };
        let good = id_at("good");
        let failed = st
            .pointer("/failed/version")
            .and_then(|x| x.as_str())
            .and_then(|p| Path::new(p).file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let trial = switch::on_probation(&self.opts.paths);
        let mut items: Vec<Value> = Vec::new();
        let back = [id_at("good"), id_at("previous")]
            .into_iter()
            .find(|i| !i.is_empty() && *i != cur_id);
        if let Some(b) = back {
            items.push(json!({"rev": "back", "subject": format!("roll back to {}", b), "marks": []}));
        }
        let mut tree_marks = vec![];
        if cur_id.is_empty() {
            tree_marks.push("current");
        }
        if self.building.contains("tree") {
            tree_marks.push("building");
        }
        items.push(json!({"rev": "tree", "subject": "the working tree, uncommitted changes included", "marks": tree_marks}));
        let log = recent_commits(&repo, 40);
        for l in log.lines() {
            let (h, subject) = l.split_once(' ').unwrap_or((l, ""));
            let mut marks = vec![];
            if cur_id == h || cur_id.starts_with(&format!("{}-", h)) {
                marks.push("current");
                if trial {
                    marks.push("trial");
                }
            }
            if good == h {
                marks.push("good");
            }
            if versions_dir.join(h).join("VERSION").exists() {
                marks.push("built");
            }
            if self.building.contains(h) {
                marks.push("building");
            }
            if failed == h {
                marks.push("failed");
            }
            items.push(json!({"rev": h, "subject": subject, "marks": marks}));
        }
        json!({"ev": "versions", "current": cur_id, "items": items})
    }

    pub(super) fn broadcast_versions(&mut self) {
        if !self.clients.is_empty() {
            let v = self.version_items();
            self.broadcast(&v);
        }
    }

    fn start_switch(&self, to: &Path) {
        spawn_switcher(&self.opts.paths, &self.opts.exe, to, Switcher::Switch);
    }

    /// Reload bise on the version running now (BISE-131): the switcher
    /// restarts the hub on it (same probation), the new hub relaunches
    /// every agent's REPL at its next idle (same session, same port) and
    /// tells the TUIs to re-exec. Nothing is built.
    fn reload(&self) -> String {
        let root = &self.opts.app_root;
        let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
        spawn_switcher(&self.opts.paths, &self.opts.exe, &cur, Switcher::Reload);
        format!(
            "reloading bise on the running version {}: the hub, every agent and the TUI restart on it, nothing lost (an agent in a turn reloads when its turn ends)",
            crate::switch::version_id(root).unwrap_or_else(|| "(dev tree)".into())
        )
    }
}

/// What the switcher does.
#[derive(Clone, Copy)]
pub(super) enum Switcher {
    /// to another version
    Switch,
    /// the hub again on `to` (maybe the running version), agents kept
    Restart,
    /// the running version again: hub, every REPL and TUI (BISE-131)
    Reload,
}

/// `exe sbswitch --to <dir>`: detached, from THIS (known good) binary;
/// it outlives this hub, which it replaces.
pub(super) fn spawn_switcher(paths: &Paths, exe: &Path, to: &Path, how: Switcher) {
    use std::os::unix::process::CommandExt;
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"));
    let mut cmd = Command::new(exe);
    cmd.arg("sbswitch")
        .arg("--workspace")
        .arg(&paths.workspace)
        .arg("--to")
        .arg(to)
        .args(match how {
            Switcher::Switch => &[][..],
            Switcher::Restart => &["--restart"][..],
            Switcher::Reload => &["--restart", "--reload"][..],
        })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0);
    if let Ok(f) = err {
        cmd.stderr(Stdio::from(f));
    }
    if let Err(e) = cmd.spawn() {
        log_line(paths, &format!("sbswitch: {}", e));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn commit_lines_are_cut_to_100_chars() {
        let long = format!("abc1234 {}", "é".repeat(3000));
        let out = super::clip_lines(&format!("{}\ndef5678 short\n", long));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].chars().count(), 100);
        assert!(lines[0].starts_with("abc1234 é") && lines[0].ends_with('…'));
        assert_eq!(lines[1], "def5678 short");
    }

    use super::version_allowed;

    #[test]
    fn only_main_switches_versions() {
        for who in ["main", "docs", ""] {
            assert!(version_allowed(who, "list").is_ok());
            assert!(version_allowed(who, "").is_ok());
        }
        assert!(version_allowed("main", "switch").is_ok());
        assert!(version_allowed("main", "rollback").is_ok());
        for what in ["switch", "rollback"] {
            let e = version_allowed("docs", what).unwrap_err();
            assert!(e.contains("reserved for main"), "{}", e);
            assert!(version_allowed("", what).is_err());
        }
    }

    #[test]
    fn restart_is_unchanged_in_dev_and_a_reload_elsewhere() {
        use super::{restart_plan, RestartPlan::*, RestartTarget::*};
        // bise's source tree: exactly as before (build + switch, or the hub
        // again on the running version); never a reload
        assert_eq!(restart_plan(true, ""), Dev(Latest));
        assert_eq!(restart_plan(true, "latest"), Dev(Latest));
        assert_eq!(restart_plan(true, "current"), Dev(Current));
        assert_eq!(restart_plan(true, "021b8a1"), Dev(Rev("021b8a1".into())));
        // anywhere else: a reload, nothing built
        assert_eq!(restart_plan(false, ""), Reload);
        assert_eq!(restart_plan(false, " current "), Reload);
        assert_eq!(restart_plan(false, "latest"), Refuse);
        assert_eq!(restart_plan(false, "021b8a1"), Refuse);
    }

    #[test]
    fn restart_defaults_to_latest() {
        use super::{restart_target, RestartTarget::*};
        assert_eq!(restart_target(""), Latest);
        assert_eq!(restart_target("  "), Latest);
        assert_eq!(restart_target("latest"), Latest);
        assert_eq!(restart_target("head"), Latest);
        assert_eq!(restart_target("HEAD"), Latest);
        assert_eq!(restart_target("current"), Current);
        assert_eq!(restart_target("021b8a1"), Rev("021b8a1".into()));
    }
}
