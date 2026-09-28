//! `sbd`: the hub of one workspace (RFC 0001 §5), the imperative shell
//! around `core::Hub`. One thread owns the hub and executes its effects;
//! the other threads only read (sockets, REPL output) and send messages.
//!
//! - one Bend REPL per live agent, spawned from the app root, with
//!   BEND_WORKDIR / BEND_EXTRA_PROMPT / BEND_CONTEXT_FILE / SB_AGENT;
//! - `hub.sock`: clients (`{"op":"hello"}` first, then JSON lines) and
//!   the agents' `sb` CLI (`{"op":"agent", ...}`, one request, one reply);
//! - `journal.jsonl`: the durable state; `agents/<dir>/transcript.log`:
//!   every line of each feed, for the views, `sb inspect` and
//!   `sb history`.

use crate::core::{AgentReq, ClientId, Effect, Hub, Input, Token};
use crate::model::{Agent, Event, MAIN};
use crate::paths::Paths;
use crate::prompts;
use crate::transcript::{self, Anchor};
use crate::util::{clip, now_ms, wire_escape};
use crate::worktree::{Config, GitEnv};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// Lines kept in memory per feed and replayed to a new client (older
/// ones stay in the transcript: the client pages them with the
/// `history` op when the user scrolls up).
const BUFFER_LINES: usize = 1000;
/// Lines one `history` page may carry.
const PAGE_LINES: usize = 2000;

pub struct Opts {
    pub paths: Paths,
    /// Where repl-live and the runtime's relative files live.
    pub app_root: PathBuf,
    /// The bend-harness executable (the `sb` shim calls it).
    pub exe: PathBuf,
    pub repl_bin: PathBuf,
}

enum Msg {
    In(Input),
    ReplConnected {
        dir: String,
        gen: u64,
        stream: TcpStream,
        steer: String,
        interrupt: String,
        pid: u32,
        /// A REPL left running by a previous hub, reconnected (adopt).
        adopted: bool,
        /// Adopted in the middle of a turn.
        busy: bool,
    },
    /// The process exists (before its banner): it must die with the hub.
    ReplSpawned {
        dir: String,
        gen: u64,
        pid: u32,
    },
    ReplLine {
        dir: String,
        gen: u64,
        line: String,
        /// Offset in the wire log just after this line.
        offset: u64,
    },
    ReplGone {
        dir: String,
        gen: u64,
        reason: String,
    },
    ClientNew {
        id: ClientId,
        stream: UnixStream,
    },
    ClientLine {
        id: ClientId,
        v: Value,
    },
    ClientGone {
        id: ClientId,
    },
    AgentNew {
        token: Token,
        stream: UnixStream,
        v: Value,
    },
    /// `sb version …` (one request, one answer).
    Version {
        stream: UnixStream,
        v: Value,
    },
    /// A `/version` build is over.
    BuildEnded {
        rev: String,
    },
    /// A line for main's thread (the version switcher).
    Notice {
        kind: String,
        text: String,
    },
    /// `keep`: leave the REPLs running for the next hub to adopt.
    Shutdown {
        keep: bool,
    },
}

struct Repl {
    stream: TcpStream,
    steer: String,
    interrupt: String,
}

struct Shell {
    opts: Opts,
    hub: Hub,
    env: GitEnv,
    tx: Sender<Msg>,
    journal: std::fs::File,
    repls: BTreeMap<String, Repl>,
    /// The live generation of each agent's REPL: lines and exits of an
    /// older (killed) generation are ignored.
    gens: BTreeMap<String, u64>,
    next_gen: u64,
    /// Every REPL process alive, connected or not: (generation, pid).
    pids: BTreeMap<String, (u64, u32)>,
    clients: BTreeMap<ClientId, UnixStream>,
    replies: BTreeMap<Token, UnixStream>,
    /// The last lines of each feed, with their transcript positions.
    buffers: BTreeMap<String, VecDeque<(usize, String)>>,
    /// The position of the last line of each feed's transcript.
    positions: BTreeMap<String, usize>,
    /// Wire-log offsets processed since the last flush to `wire.offset`.
    offsets: BTreeMap<String, u64>,
    /// True while `Input::Boot` runs: its spawns may adopt a REPL.
    booting: bool,
    /// The binary and port of each live REPL (adopted ones may run
    /// another version's binary: they switch at their next idle).
    bins: BTreeMap<String, PathBuf>,
    ports: BTreeMap<String, u16>,
    /// REPLs switching to this hub's binary (asked to reload between
    /// turns): the writes meant for them wait here until the new process
    /// is connected, on the same port and session.
    switching: BTreeMap<String, Vec<String>>,
    /// Of those, the ones whose new process is already spawned: its exit
    /// is a crash (of the new version), not the reload.
    switch_spawned: std::collections::BTreeSet<String>,
    /// Restarted by a switch: their greeting (restored history) is not
    /// news for the feeds.
    restored: std::collections::BTreeSet<String>,
    /// Versions being built (`/version <commit>`), by revision.
    building: std::collections::BTreeSet<String>,
    /// Agents whose REPL was found dead at boot in the middle of a turn
    /// (killed by a restart, a crash): once respawned on their session,
    /// they are told to continue where they left off.
    resume_turn: std::collections::BTreeSet<String>,
}

/// What an agent whose turn was cut by a restart receives.
const RESUME_TEXT: &str = "Your turn was interrupted by a restart of Switchboard; continue where you left off.";

/// `sb version` from an agent: every agent may list the versions; only
/// main switches or rolls back (the user does it from the TUI).
fn version_allowed(from: &str, what: &str) -> Result<(), String> {
    match what {
        "" | "list" => Ok(()),
        _ if from == MAIN => Ok(()),
        _ => Err(format!("sb version {}: reserved for main (the parent of the tasks)", what)),
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

fn log_line(paths: &Paths, s: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.log())
    {
        let _ = writeln!(f, "{} {}", now_ms(), s);
    }
}

fn write_json(stream: &mut UnixStream, v: &Value) -> bool {
    write_line(stream, &v.to_string())
}

fn write_line(stream: &mut UnixStream, line: &str) -> bool {
    let mut s = String::with_capacity(line.len() + 1);
    s.push_str(line);
    s.push('\n');
    stream.write_all(s.as_bytes()).is_ok()
}

/// The lines of a transcript at positions [before - count, before)
/// (positions from 1, as `transcript.rs`), without keeping the rest of
/// the file in memory.
fn transcript_page(path: &Path, before: usize, count: usize) -> Vec<(usize, String)> {
    use std::io::BufRead;
    let Ok(f) = std::fs::File::open(path) else { return Vec::new() };
    let from = before.saturating_sub(count).max(1);
    let mut out = Vec::new();
    for (i, l) in std::io::BufReader::new(f).lines().enumerate() {
        let pos = i + 1;
        if pos >= before {
            break;
        }
        let Ok(l) = l else { break };
        if pos >= from {
            if let Some((_, line)) = l.split_once('\t') {
                out.push((pos, line.to_string()));
            }
        }
    }
    out
}

/// How many lines a transcript holds (the position of its last line).
fn transcript_len(path: &Path) -> usize {
    std::fs::read(path)
        .map(|b| b.iter().filter(|c| **c == b'\n').count())
        .unwrap_or(0)
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

fn free_port() -> std::io::Result<u16> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(l.local_addr()?.port())
}

impl Shell {
    /// `version` op (`/version` in the TUI, `sb version`): list the
    /// versions, switch to one (built first when needed), roll back.
    /// Answers a text for the user.
    fn version_op(&mut self, v: &Value) -> String {
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
    fn version_items(&self) -> Value {
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

    fn broadcast_versions(&mut self) {
        if !self.clients.is_empty() {
            let v = self.version_items();
            self.broadcast(&v);
        }
    }

    fn start_switch(&self, to: &Path) {
        spawn_switcher(&self.opts.paths, &self.opts.exe, to, false);
    }

    /// Save how far each wire log was processed: the next hub adopts
    /// the REPLs from there.
    fn flush_offsets(&mut self) {
        for (dir, off) in std::mem::take(&mut self.offsets) {
            let d = self.opts.paths.agent_dir(&dir);
            let _ = std::fs::write(d.join("wire.offset.tmp"), off.to_string());
            let _ = std::fs::rename(d.join("wire.offset.tmp"), d.join("wire.offset"));
        }
    }

    fn agent_by_dir(&self, dir: &str) -> Option<&Agent> {
        self.hub.st.agents.values().find(|a| a.dir == dir)
    }

    fn dir_of(&self, name: &str) -> Option<String> {
        self.hub.st.agents.get(name).map(|a| a.dir.clone())
    }

    fn transcript(&self, dir: &str) -> PathBuf {
        self.opts.paths.agent_dir(dir).join("transcript.log")
    }

    /// A line enters a feed: memory, transcript, every client.
    fn feed(&mut self, name: &str, line: &str) {
        let Some(dir) = self.dir_of(name) else { return };
        let path = self.transcript(&dir);
        let pos = match self.positions.get(name) {
            Some(p) => p + 1,
            None => transcript_len(&path) + 1,
        };
        self.positions.insert(name.to_string(), pos);
        let b = self.buffers.entry(name.to_string()).or_default();
        b.push_back((pos, line.to_string()));
        while b.len() > BUFFER_LINES {
            b.pop_front();
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{}\t{}", now_ms(), line);
        }
        self.broadcast(&json!({"ev": "line", "agent": name, "line": line, "pos": pos}));
    }

    /// One event to every client (serialized once).
    fn broadcast(&mut self, v: &Value) {
        let line = v.to_string();
        let mut dead: Vec<ClientId> = Vec::new();
        for (id, s) in self.clients.iter_mut() {
            if !write_line(s, &line) {
                dead.push(*id);
            }
        }
        for id in dead {
            self.clients.remove(&id);
            let _ = self.tx.send(Msg::ClientGone { id });
        }
    }

    fn step(&mut self, input: Input) {
        let fx = self.hub.handle(input, &mut self.env);
        for e in fx {
            self.run(e);
        }
    }

    fn run(&mut self, e: Effect) {
        match e {
            Effect::Journal(ev) => {
                if let Ok(s) = serde_json::to_string(&ev) {
                    let _ = writeln!(self.journal, "{}", s);
                    let _ = self.journal.flush();
                }
            }
            Effect::Spawn {
                agent,
                resume,
                crash_note,
            } => self.spawn_on(&agent, resume, crash_note, None),
            Effect::Kill { agent } => {
                if let Some(dir) = self.dir_of(&agent) {
                    self.gens.remove(&dir);
                    if let Some(r) = self.repls.remove(&dir) {
                        let _ = r.stream.shutdown(std::net::Shutdown::Both);
                    }
                    if let Some((_, pid)) = self.pids.remove(&dir) {
                        kill_pid(pid);
                    }
                }
            }
            Effect::Say { agent, text } => {
                let line = format!("say {}\n", wire_escape(&text));
                if !self.repl_write(&agent, &line) {
                    log_line(
                        &self.opts.paths,
                        &format!("say to {} failed: not connected", agent),
                    );
                }
            }
            Effect::Steer { agent, text } => {
                if let Some(r) = self.dir_of(&agent).and_then(|d| self.repls.get(&d)) {
                    let ok = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&r.steer)
                        .and_then(|mut f| {
                            f.write_all(format!("{}\n", wire_escape(&text)).as_bytes())
                        });
                    if ok.is_err() {
                        log_line(&self.opts.paths, &format!("steer to {} failed", agent));
                    }
                }
            }
            Effect::Passthrough { agent, line } => {
                self.repl_write(&agent, &format!("{}\n", line));
            }
            Effect::Interrupt { agent } => {
                if let Some(r) = self.dir_of(&agent).and_then(|d| self.repls.get(&d)) {
                    let _ = std::fs::write(&r.interrupt, "1");
                }
            }
            Effect::Context { agent, text } => {
                if let Some(dir) = self.dir_of(&agent) {
                    let d = self.opts.paths.agent_dir(&dir);
                    let _ = std::fs::create_dir_all(&d);
                    let tmp = d.join("context.txt.tmp");
                    if std::fs::write(&tmp, &text).is_ok() {
                        let _ = std::fs::rename(&tmp, d.join("context.txt"));
                    }
                }
            }
            Effect::Line { agent, line } => self.feed(&agent, &line),
            Effect::Reply { token, body } => {
                if let Some(mut s) = self.replies.remove(&token) {
                    write_json(&mut s, &body);
                }
            }
            Effect::ToClient { client, body } => {
                if let Some(s) = self.clients.get_mut(&client) {
                    write_json(s, &body);
                }
            }
            Effect::Renamed { old, new } => {
                if let Some(b) = self.buffers.remove(&old) {
                    self.buffers.insert(new.clone(), b);
                }
                if let Some(p) = self.positions.remove(&old) {
                    self.positions.insert(new.clone(), p);
                }
                self.broadcast(&json!({"ev": "renamed", "old": old, "new": new}));
            }
            Effect::State => {
                let snap = self.hub.snapshot(now_ms());
                self.broadcast(&snap);
            }
        }
    }

    fn repl_write(&mut self, agent: &str, line: &str) -> bool {
        let Some(dir) = self.dir_of(agent) else {
            return false;
        };
        if let Some(q) = self.switching.get_mut(&dir) {
            q.push(line.to_string());
            return true;
        }
        match self.repls.get_mut(&dir) {
            Some(r) => r.stream.write_all(line.as_bytes()).is_ok(),
            None => false,
        }
    }

    fn same_bin(a: &Path, b: &Path) -> bool {
        let c = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        c(a) == c(b)
    }

    /// Every idle REPL still on another version's binary is asked to
    /// reload (a turn boundary: it checkpoints and exits); the hub then
    /// restarts it on its own binary, same port, same session. Busy ones
    /// wait for the end of their turn: an agent never loses a turn.
    fn switch_idle_repls(&mut self) {
        let stale: Vec<(String, String)> = self
            .hub
            .st
            .agents
            .values()
            .filter(|a| a.run == crate::model::Run::Idle)
            .filter(|a| !self.switching.contains_key(&a.dir) && self.repls.contains_key(&a.dir))
            .filter(|a| {
                self.bins
                    .get(&a.dir)
                    .is_some_and(|b| !Self::same_bin(b, &self.opts.repl_bin))
            })
            .map(|a| (a.name.clone(), a.dir.clone()))
            .collect();
        for (name, dir) in stale {
            let Some(r) = self.repls.get_mut(&dir) else {
                continue;
            };
            if r.stream.write_all(b"reload\n").is_ok() {
                log_line(
                    &self.opts.paths,
                    &format!(
                        "switching the REPL of {} to {}",
                        name,
                        self.opts.repl_bin.display()
                    ),
                );
                self.switching.insert(dir, Vec::new());
            }
        }
    }

    /// Start the REPL of `name` on a supervisor thread. `port`: the port of the process it replaces (a switch keeps the
    /// port: background commands and steer files are keyed by it).
    fn spawn_on(
        &mut self,
        name: &str,
        resume: bool,
        crash_note: Option<String>,
        port: Option<u16>,
    ) {
        let Some(a) = self.hub.st.agents.get(name).cloned() else {
            return;
        };
        let dir = a.dir.clone();
        let adir = self.opts.paths.agent_dir(&dir);
        let _ = std::fs::create_dir_all(&adir);
        let role = if a.is_main {
            prompts::main_role(&self.hub.workspace)
        } else {
            prompts::task_role(&a)
        };
        let _ = std::fs::write(adir.join("role.md"), role);
        if !adir.join("context.txt").exists() {
            let _ = std::fs::write(adir.join("context.txt"), "");
        }
        let session = adir.join("session.txt");
        let gen = self.next_gen;
        self.next_gen += 1;
        self.gens.insert(dir.clone(), gen);
        if self.booting && resume {
            if let Some(r) = adoptable(&adir) {
                log_line(
                    &self.opts.paths,
                    &format!(
                        "adopting the REPL of {} (pid {}, port {})",
                        a.name, r.pid, r.port
                    ),
                );
                self.pids.insert(dir.clone(), (gen, r.pid));
                self.bins.insert(dir.clone(), r.bin.clone());
                self.ports.insert(dir.clone(), r.port);
                let tx = self.tx.clone();
                let paths = self.opts.paths.clone();
                std::thread::spawn(move || adopt(r, dir, gen, adir, tx, paths));
                return;
            }
            // not adoptable: dead. Was it in the middle of a turn?
            let wire = adir.join("wire.log");
            let len = std::fs::metadata(&wire).map(|m| m.len()).unwrap_or(0);
            if len > 0 && busy_at(&wire, len) {
                log_line(
                    &self.opts.paths,
                    &format!("{}: its REPL died mid-turn, the turn resumes", a.name),
                );
                self.resume_turn.insert(dir.clone());
            }
        }
        // a fresh process: a fresh wire log
        let _ = std::fs::write(adir.join("wire.log"), "");
        let _ = std::fs::write(adir.join("wire.offset"), "0");
        let _ = std::fs::remove_file(adir.join("repl.json"));
        let port = match port.map(Ok).unwrap_or_else(free_port) {
            Ok(p) => p,
            Err(e) => {
                let _ = self.tx.send(Msg::ReplGone {
                    dir,
                    gen,
                    reason: format!("no free port: {}", e),
                });
                return;
            }
        };
        let mut cmd = Command::new(&self.opts.repl_bin);
        cmd.current_dir(&self.opts.app_root)
            .env("BEND_REPL_PORT", port.to_string())
            .env("BEND_SESSION_FILE", &session)
            .env("BEND_EXTRA_PROMPT", adir.join("role.md"))
            .env("BEND_CONTEXT_FILE", adir.join("context.txt"))
            .env("BEND_WORKDIR", &a.ws.path)
            .env("SB_SOCKET", self.opts.paths.socket())
            .env("BEND_WIRE_LOG", adir.join("wire.log"))
            .env("SB_AGENT", &a.name)
            // RFC 0002 §9: two dev servers must not fight for one port
            .env("SB_TASK", &a.name)
            .env(
                "SB_PORT_OFFSET",
                self.hub
                    .st
                    .order
                    .iter()
                    .position(|n| *n == a.name)
                    .unwrap_or(0)
                    .to_string(),
            )
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.opts.paths.bin_dir().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env_remove("BEND_CONTINUE")
            .env_remove("BEND_CRASH_NOTE");
        if resume && session.exists() {
            cmd.env("BEND_CONTINUE", "1");
        }
        if let Some(n) = crash_note {
            cmd.env("BEND_CRASH_NOTE", n);
        }
        self.bins.insert(dir.clone(), self.opts.repl_bin.clone());
        self.ports.insert(dir.clone(), port);
        let tx = self.tx.clone();
        let paths = self.opts.paths.clone();
        std::thread::spawn(move || supervise(cmd, dir, gen, adir, port, tx, paths));
    }

    fn on_repl_line(&mut self, dir: &str, line: &str) {
        let Some(name) = self.agent_by_dir(dir).map(|a| a.name.clone()) else {
            return;
        };
        if self.switching.contains_key(dir) {
            // the reload acknowledgement of a switch: not the agent's news
            return;
        }
        if self.restored.contains(dir) {
            if line.contains("obs: session_restored") {
                self.restored.remove(dir);
            }
            if line.starts_with("history ") || line.contains("obs: session_restored") {
                return;
            }
            self.restored.remove(dir);
        }
        if line.starts_with("history ") && self.buffers.get(&name).is_some_and(|b| !b.is_empty()) {
            // a restored session replays its history: the feed has it
            return;
        }
        self.feed(&name, line);
        if line == "--- idle" {
            let leftover = self
                .repls
                .get(dir)
                .map(|r| {
                    let c = std::fs::read_to_string(&r.steer).unwrap_or_default();
                    if !c.trim().is_empty() {
                        let _ = std::fs::write(&r.steer, "");
                    }
                    !c.trim().is_empty()
                })
                .unwrap_or(false);
            self.step(Input::ReplIdle {
                agent: name,
                leftover,
            });
        } else {
            self.step(Input::ReplLine {
                agent: name,
                line: line.to_string(),
            });
        }
    }

    fn client_hello(&mut self, id: ClientId, mut stream: UnixStream) {
        let ok = write_json(
            &mut stream,
            &json!({
                "ev": "hello",
                "workspace": self.hub.workspace,
                "state_dir": self.opts.paths.state.to_string_lossy(),
                "exe": self.opts.exe.to_string_lossy(),
                "version": crate::switch::version_info(&self.opts.app_root),
            }),
        ) && write_json(&mut stream, &self.hub.snapshot(now_ms()));
        if !ok {
            return;
        }
        for name in self.hub.st.order.clone() {
            if let Some(b) = self.buffers.get(&name) {
                for (pos, l) in b {
                    if !write_json(
                        &mut stream,
                        &json!({"ev": "line", "agent": name, "line": l, "pos": pos}),
                    ) {
                        return;
                    }
                }
            }
        }
        write_json(&mut stream, &json!({"ev": "ready"}));
        write_json(&mut stream, &self.version_items());
        self.clients.insert(id, stream);
        self.step(Input::ClientHello { client: id });
    }

    fn client_line(&mut self, id: ClientId, v: Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match s("op").as_str() {
            "version" if s("do") == "items" => {
                let items = self.version_items();
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &items);
                }
            }
            "version" => {
                let text = self.version_op(&v);
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &json!({"ev": "notice", "text": text}));
                }
            }
            "input" => self.step(Input::ClientInput {
                client: id,
                focus: s("focus"),
                text: s("text"),
            }),
            // older lines of a feed, before a position (the TUI scrolled
            // to the top of what it holds)
            "history" => {
                let agent = s("agent");
                let before = v.get("before").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let count = v
                    .get("count")
                    .and_then(|x| x.as_u64())
                    .map_or(PAGE_LINES, |c| (c as usize).min(PAGE_LINES));
                let lines: Vec<Value> = match self.dir_of(&agent) {
                    Some(dir) => transcript_page(&self.transcript(&dir), before, count)
                        .into_iter()
                        .map(|(pos, line)| json!({"pos": pos, "line": line}))
                        .collect(),
                    None => Vec::new(),
                };
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(
                        c,
                        &json!({"ev": "history", "agent": agent, "before": before, "lines": lines}),
                    );
                }
            }
            "focus" => self.step(Input::ClientFocus {
                client: id,
                focus: s("focus"),
            }),
            "confirm" => self.step(Input::ClientConfirm {
                client: id,
                id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
                yes: v.get("yes").and_then(|x| x.as_bool()).unwrap_or(false),
            }),
            "interrupt" => self.step(Input::ClientInterrupt {
                client: id,
                agent: s("agent"),
            }),
            "stop_hub" => {
                let keep = v
                    .get("keep_agents")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                let _ = self.tx.send(Msg::Shutdown { keep });
            }
            other => {
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(
                        c,
                        &json!({"ev": "notice", "text": format!("unknown op: {}", other)}),
                    );
                }
            }
        }
    }

    /// `sb inspect`: a bounded page of an agent's thread, with positions
    /// and cursors, or the origin of the caller (RFC 0001 §7.5).
    fn inspect(&self, from: &str, v: &Value) -> Value {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let target = s("agent");
        let Some(name) = self.hub.st.resolve(&target) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let Some(dir) = self.dir_of(&name) else {
            return json!({"ok": false, "error": format!("no agent named {}", target)});
        };
        let raw = transcript::read(&self.transcript(&dir));
        let all = transcript::entries(&raw);
        let now = now_ms();
        if v.get("origin") == Some(&json!(true)) {
            let Some(me) = self.hub.st.agents.get(from) else {
                return json!({"ok": false, "error": "--origin: unknown calling agent"});
            };
            return match transcript::origin(&raw, &me.dir, me.created_ms) {
                Some(o) => {
                    json!({"ok": true, "text": transcript::render_origin(&name, from, &all, &o, now)})
                }
                None => {
                    json!({"ok": false, "error": format!("no creation of {} in the thread of {}", from, name)})
                }
            };
        }
        let pos = |k: &str| transcript::parse_pos(&s(k));
        let anchor = if let Some(p) = pos("at") {
            Anchor::At(p)
        } else if let Some(p) = pos("around") {
            Anchor::Around(p)
        } else if let Some(p) = pos("before") {
            Anchor::Before(p)
        } else if let Some(p) = pos("after") {
            Anchor::After(p)
        } else {
            Anchor::Tail
        };
        let limit = v
            .get("last")
            .and_then(|x| x.as_u64())
            .map(|n| n as usize)
            .unwrap_or(transcript::DEFAULT_LIMIT);
        let query = s("query");
        let words = transcript::words_of(&query);
        let page = transcript::window(&all, &words, anchor, limit, transcript::BUDGET);
        json!({"ok": true, "text": transcript::render_page(&name, &query, &page, now)})
    }

    /// `sb inspect` and `sb history` read files; the rest goes to the core.
    fn agent_request(&mut self, token: Token, mut stream: UnixStream, v: Value) {
        let from = v
            .get("from")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        match cmd {
            "inspect" => {
                let body = self.inspect(&from, &v);
                write_json(&mut stream, &body);
            }
            "history" => {
                let query = v.get("query").and_then(|x| x.as_str()).unwrap_or("");
                let dir = self.dir_of(&from).unwrap_or_else(|| MAIN.to_string());
                let raw = transcript::read(&self.transcript(&dir));
                let journal = std::fs::read_to_string(self.opts.paths.journal()).unwrap_or_default();
                let text = transcript::history(&raw, &journal, query);
                write_json(&mut stream, &json!({"ok": true, "text": text}));
            }
            _ => match AgentReq::from_json(&v) {
                Ok(req) => {
                    self.replies.insert(token, stream);
                    self.step(Input::Agent { token, from, req });
                }
                Err(e) => {
                    write_json(&mut stream, &json!({"ok": false, "error": e}));
                }
            },
        }
    }
}

/// `exe sbswitch --to <dir>`: detached, from THIS (known good) binary;
/// it outlives this hub, which it replaces.
fn spawn_switcher(paths: &Paths, exe: &Path, to: &Path, restart: bool) {
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

/// What a hub needs to reconnect to a running REPL (`repl.json`).
struct ReplInfo {
    pid: u32,
    port: u16,
    bin: PathBuf,
    steer: String,
    interrupt: String,
}

fn kill_pid(pid: u32) {
    let _ = Command::new("kill").arg(pid.to_string()).status();
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_repl(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("repl-"))
        .unwrap_or(false)
}

/// A REPL a previous hub left running, when it can be adopted: its
/// `repl.json` names a live REPL process, and it writes a wire log.
fn adoptable(adir: &Path) -> Option<ReplInfo> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(adir.join("repl.json")).ok()?).ok()?;
    let r = ReplInfo {
        pid: v.get("pid")?.as_u64()? as u32,
        port: v.get("port")?.as_u64()? as u16,
        bin: PathBuf::from(v.get("bin").and_then(|b| b.as_str()).unwrap_or("")),
        steer: v.get("steer")?.as_str()?.to_string(),
        interrupt: v.get("interrupt")?.as_str()?.to_string(),
    };
    (adir.join("wire.log").exists() && pid_alive(r.pid) && is_repl(r.pid)).then_some(r)
}

/// Was a turn running at `offset` of the wire log? (the last
/// `turn_started` comes after the last `--- idle`)
fn busy_at(wire: &Path, offset: u64) -> bool {
    let Ok(bytes) = std::fs::read(wire) else {
        return false;
    };
    let text = String::from_utf8_lossy(&bytes[..(offset as usize).min(bytes.len())]).to_string();
    let mut busy = false;
    for l in text.lines() {
        if l == "--- idle" {
            busy = false;
        } else if l.trim_start().starts_with("obs: turn_started") {
            busy = true;
        }
    }
    busy
}

fn err_tail(err_path: &Path) -> String {
    std::fs::read_to_string(err_path)
        .ok()
        .and_then(|t| {
            t.lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| clip(l.trim(), 200))
        })
        .unwrap_or_default()
}

/// The REPL's output, from its wire log (the REPL appends every batch
/// there before sending it): complete lines from `offset` on, until the
/// connection closes (the REPL died, or was killed). The socket is only
/// drained, so a full buffer never blocks the REPL.
fn pump_wire(stream: &TcpStream, wire: &Path, offset: u64, dir: &str, gen: u64, tx: &Sender<Msg>) {
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let closed = Arc::new(AtomicBool::new(false));
    if let Ok(mut s) = stream.try_clone() {
        let closed = closed.clone();
        std::thread::spawn(move || {
            let mut sink = [0u8; 8192];
            while matches!(s.read(&mut sink), Ok(n) if n > 0) {}
            closed.store(true, Ordering::SeqCst);
        });
    }
    let mut f = match std::fs::File::open(wire) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut offset = offset;
    let _ = f.seek(SeekFrom::Start(offset));
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = Vec::new();
    loop {
        // the flag BEFORE the read: the last read then sees every byte
        let done = closed.load(Ordering::SeqCst);
        chunk.clear();
        let _ = f.read_to_end(&mut chunk);
        pending.extend_from_slice(&chunk);
        while let Some(n) = pending.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = pending.drain(..=n).collect();
            offset += raw.len() as u64;
            let line = String::from_utf8_lossy(&raw)
                .trim_end_matches(['\n', '\r'])
                .to_string();
            let m = Msg::ReplLine {
                dir: dir.to_string(),
                gen,
                line,
                offset,
            };
            if tx.send(m).is_err() {
                return;
            }
        }
        if done {
            return;
        }
        if chunk.is_empty() {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Reconnect to a REPL a previous hub left running. It accepts the
/// connection when the old one is over (at once when idle, at the end of
/// its turn otherwise); its output meanwhile is in the wire log.
fn adopt(r: ReplInfo, dir: String, gen: u64, adir: PathBuf, tx: Sender<Msg>, paths: Paths) {
    let gone = |reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            reason,
        });
    };
    let wire = adir.join("wire.log");
    let offset: u64 = std::fs::read_to_string(adir.join("wire.offset"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let stream = match TcpStream::connect(("127.0.0.1", r.port)) {
        Ok(s) => s,
        Err(e) => return gone(format!("reconnecting to the REPL: {}", e)),
    };
    let _ = stream.set_nodelay(true);
    let Ok(writer) = stream.try_clone() else {
        return gone("socket".into());
    };
    let _ = tx.send(Msg::ReplConnected {
        dir: dir.clone(),
        gen,
        stream: writer,
        steer: r.steer.clone(),
        interrupt: r.interrupt.clone(),
        pid: r.pid,
        adopted: true,
        busy: busy_at(&wire, offset),
    });
    pump_wire(&stream, &wire, offset, &dir, gen, &tx);
    // not our child: no exit status, only its death
    for _ in 0..50 {
        if !pid_alive(r.pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let tail = err_tail(&adir.join("repl.err"));
    let reason = if tail.is_empty() {
        "the REPL stopped".to_string()
    } else {
        format!("the REPL stopped · {}", tail)
    };
    log_line(
        &paths,
        &format!("repl {} (adopted) exited: {}", dir, reason),
    );
    gone(reason);
}

/// Spawn one REPL, wait for its banner, connect, stream its lines.
fn supervise(
    mut cmd: Command,
    dir: String,
    gen: u64,
    adir: PathBuf,
    port: u16,
    tx: Sender<Msg>,
    paths: Paths,
) {
    let log_path = adir.join("repl.log");
    let err_path = adir.join("repl.err");
    let bin = PathBuf::from(cmd.get_program());
    let gone = |reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            reason,
        });
    };
    let log = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => return gone(format!("log : {}", e)),
    };
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&err_path);
    cmd.stdin(Stdio::null()).stdout(Stdio::from(log));
    match err {
        Ok(f) => cmd.stderr(Stdio::from(f)),
        Err(_) => cmd.stderr(Stdio::null()),
    };
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return gone(format!("spawn : {}", e)),
    };
    let _ = tx.send(Msg::ReplSpawned {
        dir: dir.clone(),
        gen,
        pid: child.id(),
    });
    let start = Instant::now();
    let info = loop {
        let content = std::fs::read_to_string(&log_path).unwrap_or_default();
        if content.contains("REPL on") {
            break content;
        }
        if let Ok(Some(st)) = child.try_wait() {
            return gone(format!("the REPL died at startup ({})", st));
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return gone("the REPL did not start within 30 s".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let field = |k: &str| -> String {
        info.lines()
            .find(|l| l.starts_with("harness-info "))
            .and_then(|l| {
                l.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(&format!("{}=", k)).map(|v| v.to_string()))
            })
            .unwrap_or_default()
    };
    let steer = field("steer");
    let interrupt = field("interrupt");
    let stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return gone(format!("connecting to the REPL: {}", e));
        }
    };
    let _ = stream.set_nodelay(true);
    let writer = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => return gone(e.to_string()),
    };
    // what the next hub needs to adopt this REPL
    let _ = std::fs::write(
        adir.join("repl.json"),
        json!({"pid": child.id(), "port": port, "steer": steer, "interrupt": interrupt,
               "bin": bin.to_string_lossy()})
        .to_string(),
    );
    let _ = tx.send(Msg::ReplConnected {
        dir: dir.clone(),
        gen,
        stream: writer,
        steer,
        interrupt,
        pid: child.id(),
        adopted: false,
        busy: false,
    });
    pump_wire(&stream, &adir.join("wire.log"), 0, &dir, gen, &tx);
    let status = child.wait();
    let reason = match &status {
        Ok(s) => {
            let tail = err_tail(&err_path);
            format!(
                "{}{}",
                s,
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", tail)
                }
            )
        }
        Err(e) => e.to_string(),
    };
    log_line(&paths, &format!("repl {} exited: {}", dir, reason));
    gone(reason);
}

fn accept_loop(listener: UnixListener, tx: Sender<Msg>) {
    let mut next: u64 = 1;
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let id = next;
        next += 1;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let Ok(read_half) = stream.try_clone() else {
                return;
            };
            let mut r = BufReader::new(read_half);
            let mut first = String::new();
            if r.read_line(&mut first).unwrap_or(0) == 0 {
                return;
            }
            let Ok(v) = serde_json::from_str::<Value>(first.trim()) else {
                return;
            };
            match v.get("op").and_then(|x| x.as_str()) {
                Some("hello") => {
                    let _ = tx.send(Msg::ClientNew { id, stream });
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match r.read_line(&mut line) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                                    let _ = tx.send(Msg::ClientLine { id, v });
                                }
                            }
                        }
                    }
                    let _ = tx.send(Msg::ClientGone { id });
                }
                Some("agent") => {
                    let _ = tx.send(Msg::AgentNew {
                        token: id,
                        stream,
                        v,
                    });
                }
                Some("version") => {
                    let _ = tx.send(Msg::Version { stream, v });
                }
                Some("notice") => {
                    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let _ = tx.send(Msg::Notice {
                        kind: s("kind"),
                        text: s("text"),
                    });
                }
                Some("ping") => {
                    let mut s = stream;
                    let _ = write_json(&mut s, &json!({"ok": true, "pid": std::process::id()}));
                }
                _ => {}
            }
        });
    }
}

/// A hub that died abruptly (killed, crashed) left its REPLs running:
/// they still hold their sessions. Their pids are in `repl.pid`.
fn kill_stale_repls(sh: &Shell) {
    for a in sh.hub.st.agents.values() {
        if sh.pids.contains_key(&a.dir) {
            // adopted at boot, or just spawned
            continue;
        }
        let f = sh.opts.paths.agent_dir(&a.dir).join("repl.pid");
        let Ok(pid) = std::fs::read_to_string(&f) else {
            continue;
        };
        let pid = pid.trim().to_string();
        let cmdline = Command::new("ps")
            .args(["-p", &pid, "-o", "command="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if cmdline.contains("repl-live") {
            log_line(
                &sh.opts.paths,
                &format!("killing a stale REPL of {} (pid {})", a.name, pid),
            );
            let _ = Command::new("kill").arg(&pid).status();
        }
        let _ = std::fs::remove_file(&f);
    }
}

/// The `sb` shim: agents call `sb …` from their bash tool.
fn write_shim(paths: &Paths, exe: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(paths.bin_dir())?;
    let p = paths.bin_dir().join("sb");
    let q = exe.to_string_lossy().replace('\'', "'\\''");
    std::fs::write(&p, format!("#!/bin/sh\nexec '{}' sb \"$@\"\n", q))?;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))
}

pub fn run(opts: Opts) -> std::io::Result<()> {
    let paths = opts.paths.clone();
    std::fs::create_dir_all(&paths.state)?;
    // one hub per workspace
    if UnixStream::connect(paths.socket()).is_ok() {
        eprintln!("a hub is already running for {}", paths.workspace.display());
        return Ok(());
    }
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    std::fs::write(paths.pid_file(), std::process::id().to_string())?;
    write_shim(&paths, &opts.exe)?;
    // the switcher reads where this hub runs from (to come back to it)
    let _ = std::fs::write(paths.state.join("hub.root"), opts.app_root.to_string_lossy().as_bytes());
    log_line(
        &paths,
        &format!(
            "hub start pid={} workspace={}",
            std::process::id(),
            paths.workspace.display()
        ),
    );

    let workspace = paths.workspace.to_string_lossy().to_string();
    let mut hub = Hub::new(&workspace);
    let events: Vec<Event> = std::fs::read_to_string(paths.journal())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    hub.replay(&events);
    let journal = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.journal())?;

    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let log_paths = paths.clone();
    let env = GitEnv {
        paths: paths.clone(),
        config: Config::load(&paths),
        log: Box::new(move |s| log_line(&log_paths, s)),
    };
    let mut sh = Shell {
        opts,
        hub,
        env,
        tx: tx.clone(),
        journal,
        repls: BTreeMap::new(),
        gens: BTreeMap::new(),
        next_gen: 1,
        pids: BTreeMap::new(),
        clients: BTreeMap::new(),
        replies: BTreeMap::new(),
        buffers: BTreeMap::new(),
        positions: BTreeMap::new(),
        offsets: BTreeMap::new(),
        booting: false,
        bins: BTreeMap::new(),
        ports: BTreeMap::new(),
        switching: BTreeMap::new(),
        switch_spawned: std::collections::BTreeSet::new(),
        restored: std::collections::BTreeSet::new(),
        building: std::collections::BTreeSet::new(),
        resume_turn: std::collections::BTreeSet::new(),
    };
    // the feeds survive a hub restart through their transcripts
    for a in sh.hub.st.agents.values() {
        let all = transcript::read(&sh.transcript(&a.dir));
        sh.positions.insert(a.name.clone(), all.last().map_or(0, |r| r.0));
        let skip = all.len().saturating_sub(BUFFER_LINES);
        let tail: VecDeque<(usize, String)> =
            all.into_iter().skip(skip).map(|(p, _, l)| (p, l)).collect();
        if !tail.is_empty() {
            sh.buffers.insert(a.name.clone(), tail);
        }
    }

    {
        let tx = tx.clone();
        std::thread::spawn(move || accept_loop(listener, tx));
    }
    {
        let tx = tx.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            if tx.send(Msg::In(Input::Tick)).is_err() {
                break;
            }
        });
    }
    // a rollback's warning, left by the switcher for this hub
    let notice = paths.state.join("switch-notice");
    if let Ok(t) = std::fs::read_to_string(&notice) {
        let _ = std::fs::remove_file(&notice);
        sh.feed(MAIN, &format!("sb warn : {}", wire_escape(&t)));
    }
    sh.booting = true;
    sh.step(Input::Boot);
    sh.booting = false;
    kill_stale_repls(&sh);

    let mut keep_agents = false;
    while let Ok(m) = rx.recv() {
        match m {
            Msg::In(i) => {
                let tick = matches!(i, Input::Tick);
                if tick {
                    sh.flush_offsets();
                }
                sh.step(i);
                if tick {
                    sh.switch_idle_repls();
                }
            }
            Msg::ReplConnected {
                dir,
                gen,
                stream,
                steer,
                interrupt,
                pid,
                adopted,
                busy,
            } => {
                if sh.gens.get(&dir) != Some(&gen) {
                    // killed while it was starting
                    kill_pid(pid);
                    continue;
                }
                if !adopted {
                    let _ = std::fs::write(&steer, "");
                    let _ = std::fs::write(&interrupt, "");
                }
                sh.repls.insert(
                    dir.clone(),
                    Repl {
                        stream,
                        steer,
                        interrupt,
                    },
                );
                sh.switch_spawned.remove(&dir);
                if let Some(q) = sh.switching.remove(&dir) {
                    // a switched REPL: same session, the core never saw
                    // it go; the writes it missed go now
                    log_line(&sh.opts.paths, &format!("switched the REPL of {}", dir));
                    if let Some(r) = sh.repls.get_mut(&dir) {
                        for l in &q {
                            let _ = r.stream.write_all(l.as_bytes());
                        }
                    }
                    if !q.is_empty() {
                        continue;
                    }
                }
                if !adopted && sh.resume_turn.remove(&dir) {
                    // its turn was cut: the first turn of the new process
                    // continues it (queued writes of the core come after)
                    if let Some(r) = sh.repls.get_mut(&dir) {
                        let _ = r
                            .stream
                            .write_all(format!("say {}\n", wire_escape(RESUME_TEXT)).as_bytes());
                    }
                    if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        sh.feed(
                            &name,
                            "sb info : its turn was interrupted by a restart — it continues where it left off",
                        );
                    }
                }
                if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    if busy {
                        // adopted mid-turn: busy until its `--- idle`
                        sh.step(Input::ReplLine {
                            agent: name,
                            line: "  obs: turn_started".into(),
                        });
                    } else {
                        sh.step(Input::ReplReady { agent: name });
                    }
                }
            }
            Msg::ReplSpawned { dir, gen, pid } => {
                let _ = std::fs::write(
                    sh.opts.paths.agent_dir(&dir).join("repl.pid"),
                    pid.to_string(),
                );
                if sh.gens.get(&dir) == Some(&gen) {
                    sh.pids.insert(dir, (gen, pid));
                } else {
                    kill_pid(pid);
                }
            }
            Msg::ReplLine {
                dir,
                gen,
                line,
                offset,
            } => {
                if sh.gens.get(&dir) == Some(&gen) {
                    sh.on_repl_line(&dir, &line);
                    sh.offsets.insert(dir, offset);
                }
            }
            Msg::ReplGone {
                dir,
                gen,
                reason,
            } => {
                // a killed generation is not live anymore: its exit is
                // expected; any exit of the live one is a crash (the hub
                // never asks a REPL to quit)
                if sh.gens.get(&dir) != Some(&gen) {
                    continue;
                }
                sh.gens.remove(&dir);
                sh.repls.remove(&dir);
                sh.pids.remove(&dir);
                if sh.switching.contains_key(&dir) && !sh.switch_spawned.contains(&dir) {
                    // the reload a switch asked for: the same session on
                    // this hub's binary, the same port
                    let port = sh.ports.get(&dir).copied();
                    if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        sh.restored.insert(dir.clone());
                        sh.switch_spawned.insert(dir.clone());
                        sh.spawn_on(&name, true, None, port);
                        continue;
                    }
                }
                // the new process of a switch died: a crash like any other
                sh.switching.remove(&dir);
                sh.switch_spawned.remove(&dir);
                sh.restored.remove(&dir);
                // a new version on probation: a REPL that dies is a
                // reason to roll back
                crate::switch::report_failure(
                    &sh.opts.paths,
                    &format!("the REPL of {} stopped: {}", dir, reason),
                );
                if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    sh.step(Input::ReplExited {
                        agent: name,
                        crashed: true,
                        reason,
                    });
                }
            }
            Msg::ClientNew { id, stream } => sh.client_hello(id, stream),
            Msg::ClientLine { id, v } => sh.client_line(id, v),
            Msg::ClientGone { id } => {
                if sh.clients.remove(&id).is_some() {
                    sh.step(Input::ClientGone { client: id });
                }
            }
            Msg::AgentNew { token, stream, v } => sh.agent_request(token, stream, v),
            Msg::Version { mut stream, v } => {
                let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("");
                let what = v.get("do").and_then(|x| x.as_str()).unwrap_or("");
                match version_allowed(from, what) {
                    Ok(()) => {
                        let text = sh.version_op(&v);
                        write_json(&mut stream, &json!({"ok": true, "text": text}));
                    }
                    Err(e) => {
                        write_json(&mut stream, &json!({"ok": false, "error": e}));
                    }
                }
            }
            Msg::Notice { kind, text } => {
                let kind = if kind == "warn" { "warn" } else { "info" };
                sh.feed(MAIN, &format!("sb {} : {}", kind, wire_escape(&text)));
                sh.broadcast_versions();
            }
            Msg::BuildEnded { rev } => {
                sh.building.remove(&rev);
                sh.broadcast_versions();
            }
            Msg::Shutdown { keep } => {
                keep_agents = keep;
                break;
            }
        }
    }
    sh.flush_offsets();
    if keep_agents {
        log_line(&paths, "hub stop (REPLs kept for the next hub)");
    } else {
        log_line(&paths, "hub stop");
        for (_, pid) in sh.pids.values() {
            kill_pid(*pid);
        }
    }
    let _ = std::fs::remove_file(paths.socket());
    let _ = std::fs::remove_file(paths.pid_file());
    Ok(())
}

#[cfg(test)]
mod version_tests {
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
