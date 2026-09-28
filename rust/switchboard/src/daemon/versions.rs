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

/// What `/restart [<arg>]` restarts the hub on.
#[derive(Debug, PartialEq)]
enum RestartTarget {
    /// `current`: the running version, nothing rebuilt.
    Current,
    /// no argument, `latest` or `head`: HEAD, built if needed.
    Latest,
    /// `<commit>`: that commit, built if needed.
    Rev(String),
}

fn restart_target(arg: &str) -> RestartTarget {
    match arg.trim() {
        "current" => RestartTarget::Current,
        "" | "latest" | "head" | "HEAD" => RestartTarget::Latest,
        r => RestartTarget::Rev(r.to_string()),
    }
}

/// `git log -<n>` of the repository versions are built from: one
/// `<short hash> <subject>` per line (empty when git fails).
fn recent_commits(repo: &Path, n: usize) -> String {
    Command::new("git")
        .args(["log", &format!("-{}", n), "--format=%h %s"])
        .current_dir(repo)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
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
                let (repo, versions_dir) = self.version_ctx();
                let rev = match restart_target(&s("to")) {
                    RestartTarget::Current => String::new(),
                    RestartTarget::Latest => Command::new("git")
                        .args(["rev-parse", "--short", "HEAD"])
                        .current_dir(&repo)
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default(),
                    RestartTarget::Rev(r) => r,
                };
                if rev.is_empty() && restart_target(&s("to")) == RestartTarget::Latest {
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
                    spawn_switcher(&paths, &self.opts.exe, &cur, true);
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
                let target = if dir.join("bend-harness").exists() {
                    Some(dir)
                } else if versions_dir.join(&to).join("bend-harness").exists() {
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
                    let known = Command::new("git")
                        .args(["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", to)])
                        .current_dir(&repo)
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
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
                    let out = Command::new(&script)
                        .args(["build", &rev])
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
                            spawn_switcher(&paths, &exe, Path::new(&dir), false);
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
        .unwrap_or_else(|| {
            let base = std::env::var("XDG_STATE_HOME")
                .ok()
                .filter(|d| !d.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state")
                });
            base.join("switchboard/versions")
        });
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
        spawn_switcher(&self.opts.paths, &self.opts.exe, to, false);
    }
}

/// `exe sbswitch --to <dir>`: detached, from THIS (known good) binary;
/// it outlives this hub, which it replaces.
pub(super) fn spawn_switcher(paths: &Paths, exe: &Path, to: &Path, restart: bool) {
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
        .args(if restart { &["--restart"][..] } else { &[] })
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
