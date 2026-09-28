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
use crate::util::{clip, now_ms, wire_escape, wire_unescape};
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

/// Lines kept in memory per feed (older ones stay in the transcript).
const BUFFER_LINES: usize = 4000;

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
    },
    ReplGone {
        dir: String,
        gen: u64,
        ok_exit: bool,
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
    Shutdown,
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
    buffers: BTreeMap<String, VecDeque<String>>,
}

fn log_line(paths: &Paths, s: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(paths.log()) {
        let _ = writeln!(f, "{} {}", now_ms(), s);
    }
}

fn write_json(stream: &mut UnixStream, v: &Value) -> bool {
    let mut s = v.to_string();
    s.push('\n');
    stream.write_all(s.as_bytes()).is_ok()
}

/// The last `n` entries of a transcript (`<ms>\t<line>` per line).
fn transcript_tail(path: &Path, n: usize) -> Vec<(u64, String)> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let all: Vec<(u64, String)> = text
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(t, l)| (t.parse().unwrap_or(0), l.to_string()))
        .collect();
    let skip = all.len().saturating_sub(n);
    all.into_iter().skip(skip).collect()
}

/// What a human would call "the conversation": user messages, messages
/// in, assistant texts and tool calls, one entry each.
fn readable(line: &str) -> Option<String> {
    let t = line.trim_start();
    if let Some(r) = t.strip_prefix("obs: assistant: ") {
        let v = crate::util::strip_thinking(&wire_unescape(r));
        return (!v.is_empty()).then(|| format!("assistant: {}", v));
    }
    if let Some(r) = line.strip_prefix("sb you : ") {
        return Some(format!("user: {}", wire_unescape(r)));
    }
    if let Some(r) = line.strip_prefix("sb msg-in : ") {
        return Some(format!("message from {}", wire_unescape(r)));
    }
    if let Some(r) = line.strip_prefix("tool #") {
        let r = r.split_once(' ').map(|x| x.1).unwrap_or(r);
        return Some(format!("tool: {}", clip(&wire_unescape(r), 300)));
    }
    if let Some(r) = line.strip_prefix("sb ") {
        return Some(format!("hub: {}", wire_unescape(r)));
    }
    None
}

fn free_port() -> std::io::Result<u16> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(l.local_addr()?.port())
}

impl Shell {
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
        let b = self.buffers.entry(name.to_string()).or_default();
        b.push_back(line.to_string());
        while b.len() > BUFFER_LINES {
            b.pop_front();
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.transcript(&dir)) {
            let _ = writeln!(f, "{}\t{}", now_ms(), line);
        }
        self.broadcast(&json!({"ev": "line", "agent": name, "line": line}));
    }

    fn broadcast(&mut self, v: &Value) {
        let mut dead: Vec<ClientId> = Vec::new();
        for (id, s) in self.clients.iter_mut() {
            if !write_json(s, v) {
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
            Effect::Spawn { agent, resume, crash_note } => self.spawn(&agent, resume, crash_note),
            Effect::Kill { agent } => {
                if let Some(dir) = self.dir_of(&agent) {
                    self.gens.remove(&dir);
                    if let Some(r) = self.repls.remove(&dir) {
                        let _ = r.stream.shutdown(std::net::Shutdown::Both);
                    }
                    if let Some((_, pid)) = self.pids.remove(&dir) {
                        let _ = Command::new("kill").arg(pid.to_string()).status();
                    }
                }
            }
            Effect::Say { agent, text } => {
                let line = format!("say {}\n", wire_escape(&text));
                if !self.repl_write(&agent, &line) {
                    log_line(&self.opts.paths, &format!("say to {} failed: not connected", agent));
                }
            }
            Effect::Steer { agent, text } => {
                if let Some(r) = self.dir_of(&agent).and_then(|d| self.repls.get(&d)) {
                    let ok = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&r.steer)
                        .and_then(|mut f| f.write_all(format!("{}\n", wire_escape(&text)).as_bytes()));
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
                self.broadcast(&json!({"ev": "renamed", "old": old, "new": new}));
            }
            Effect::State => {
                let snap = self.hub.snapshot(now_ms());
                self.broadcast(&snap);
            }
        }
    }

    fn repl_write(&mut self, agent: &str, line: &str) -> bool {
        let Some(dir) = self.dir_of(agent) else { return false };
        match self.repls.get_mut(&dir) {
            Some(r) => r.stream.write_all(line.as_bytes()).is_ok(),
            None => false,
        }
    }

    /// Start the REPL of `name` on a supervisor thread.
    fn spawn(&mut self, name: &str, resume: bool, crash_note: Option<String>) {
        let Some(a) = self.hub.st.agents.get(name).cloned() else { return };
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
        let port = match free_port() {
            Ok(p) => p,
            Err(e) => {
                let _ = self.tx.send(Msg::ReplGone {
                    dir,
                    gen,
                    ok_exit: false,
                    reason: format!("pas de port libre : {}", e),
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
            .env("SB_AGENT", &a.name)
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
        let log_path = adir.join("repl.log");
        let err_path = adir.join("repl.err");
        let tx = self.tx.clone();
        let paths = self.opts.paths.clone();
        std::thread::spawn(move || supervise(cmd, dir, gen, log_path, err_path, port, tx, paths));
    }

    fn on_repl_line(&mut self, dir: &str, line: &str) {
        let Some(name) = self.agent_by_dir(dir).map(|a| a.name.clone()) else { return };
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
            self.step(Input::ReplIdle { agent: name, leftover });
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
            }),
        ) && write_json(&mut stream, &self.hub.snapshot(now_ms()));
        if !ok {
            return;
        }
        for name in self.hub.st.order.clone() {
            if let Some(b) = self.buffers.get(&name) {
                for l in b {
                    if !write_json(&mut stream, &json!({"ev": "line", "agent": name, "line": l})) {
                        return;
                    }
                }
            }
        }
        write_json(&mut stream, &json!({"ev": "ready"}));
        self.clients.insert(id, stream);
        self.step(Input::ClientHello { client: id });
    }

    fn client_line(&mut self, id: ClientId, v: Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match s("op").as_str() {
            "input" => self.step(Input::ClientInput {
                client: id,
                focus: s("focus"),
                text: s("text"),
            }),
            "focus" => self.step(Input::ClientFocus { client: id, focus: s("focus") }),
            "confirm" => self.step(Input::ClientConfirm {
                client: id,
                id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
                yes: v.get("yes").and_then(|x| x.as_bool()).unwrap_or(false),
            }),
            "interrupt" => self.step(Input::ClientInterrupt { client: id, agent: s("agent") }),
            "stop_hub" => {
                let _ = self.tx.send(Msg::Shutdown);
            }
            other => {
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &json!({"ev": "notice", "text": format!("op inconnue : {}", other)}));
                }
            }
        }
    }

    /// `sb inspect` and `sb history` read files; the rest goes to the core.
    fn agent_request(&mut self, token: Token, mut stream: UnixStream, v: Value) {
        let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        match cmd {
            "inspect" => {
                let target = v.get("agent").and_then(|x| x.as_str()).unwrap_or("");
                let last = v.get("last").and_then(|x| x.as_u64()).unwrap_or(20) as usize;
                let query = v.get("query").and_then(|x| x.as_str()).unwrap_or("").to_lowercase();
                let body = match self.hub.st.resolve(target).and_then(|n| self.dir_of(&n)) {
                    None => json!({"ok": false, "error": format!("aucun agent nommé {}", target)}),
                    Some(dir) => {
                        let entries: Vec<String> = transcript_tail(&self.transcript(&dir), 20_000)
                            .into_iter()
                            .filter_map(|(_, l)| readable(&l))
                            .filter(|l| query.is_empty() || l.to_lowercase().contains(&query))
                            .collect();
                        let skip = entries.len().saturating_sub(last.min(200));
                        let text: Vec<String> = entries.into_iter().skip(skip).map(|e| clip(&e, 1500)).collect();
                        let mut out = text.join("\n");
                        if out.chars().count() > 4000 {
                            out = crate::util::clip_tail(&out, 4000);
                        }
                        json!({"ok": true, "text": out})
                    }
                };
                write_json(&mut stream, &body);
            }
            "history" => {
                let query = v.get("query").and_then(|x| x.as_str()).unwrap_or("").to_lowercase();
                let words: Vec<&str> = query.split_whitespace().collect();
                let dir = self.dir_of(&from).unwrap_or_else(|| MAIN.to_string());
                let mut hits: Vec<String> = transcript_tail(&self.transcript(&dir), usize::MAX)
                    .into_iter()
                    .filter_map(|(t, l)| readable(&l).map(|r| (t, r)))
                    .filter(|(_, r)| {
                        let low = r.to_lowercase();
                        !words.is_empty() && words.iter().all(|w| low.contains(w))
                    })
                    .map(|(t, r)| format!("[{}] {}", t, clip(&r, 600)))
                    .collect();
                if let Ok(j) = std::fs::read_to_string(self.opts.paths.journal()) {
                    for l in j.lines() {
                        let low = l.to_lowercase();
                        if !words.is_empty() && words.iter().all(|w| low.contains(w)) {
                            hits.push(format!("[journal] {}", clip(l, 600)));
                        }
                    }
                }
                let skip = hits.len().saturating_sub(30);
                let text = if hits.is_empty() {
                    "no match".to_string()
                } else {
                    hits.into_iter().skip(skip).collect::<Vec<_>>().join("\n")
                };
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

/// Spawn one REPL, wait for its banner, connect, stream its lines.
#[allow(clippy::too_many_arguments)]
fn supervise(mut cmd: Command, dir: String, gen: u64, log_path: PathBuf, err_path: PathBuf, port: u16, tx: Sender<Msg>, paths: Paths) {
    let gone = |ok_exit: bool, reason: String| {
        let _ = tx.send(Msg::ReplGone {
            dir: dir.clone(),
            gen,
            ok_exit,
            reason,
        });
    };
    let log = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => return gone(false, format!("log : {}", e)),
    };
    let err = std::fs::OpenOptions::new().create(true).append(true).open(&err_path);
    cmd.stdin(Stdio::null()).stdout(Stdio::from(log));
    match err {
        Ok(f) => cmd.stderr(Stdio::from(f)),
        Err(_) => cmd.stderr(Stdio::null()),
    };
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return gone(false, format!("spawn : {}", e)),
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
            return gone(false, format!("la REPL est morte au démarrage ({})", st));
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return gone(false, "la REPL n'a pas démarré en 30 s".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let field = |k: &str| -> String {
        info.lines()
            .find(|l| l.starts_with("harness-info "))
            .and_then(|l| l.split_whitespace().find_map(|kv| kv.strip_prefix(&format!("{}=", k)).map(|v| v.to_string())))
            .unwrap_or_default()
    };
    let steer = field("steer");
    let interrupt = field("interrupt");
    let stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return gone(false, format!("connexion à la REPL : {}", e));
        }
    };
    let _ = stream.set_nodelay(true);
    let reader = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => return gone(false, e.to_string()),
    };
    let _ = tx.send(Msg::ReplConnected {
        dir: dir.clone(),
        gen,
        stream,
        steer,
        interrupt,
        pid: child.id(),
    });
    let mut r = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf).trim_end_matches(['\n', '\r']).to_string();
                if tx.send(Msg::ReplLine { dir: dir.clone(), gen, line }).is_err() {
                    break;
                }
            }
        }
    }
    let status = child.wait();
    let reason = match &status {
        Ok(s) => {
            let tail = std::fs::read_to_string(&err_path)
                .ok()
                .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(|l| clip(l.trim(), 200)))
                .unwrap_or_default();
            format!("{}{}", s, if tail.is_empty() { String::new() } else { format!(" · {}", tail) })
        }
        Err(e) => e.to_string(),
    };
    log_line(&paths, &format!("repl {} exited: {}", dir, reason));
    gone(status.map(|s| s.success()).unwrap_or(false), reason);
}

fn accept_loop(listener: UnixListener, tx: Sender<Msg>) {
    let mut next: u64 = 1;
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let id = next;
        next += 1;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let Ok(read_half) = stream.try_clone() else { return };
            let mut r = BufReader::new(read_half);
            let mut first = String::new();
            if r.read_line(&mut first).unwrap_or(0) == 0 {
                return;
            }
            let Ok(v) = serde_json::from_str::<Value>(first.trim()) else { return };
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
                    let _ = tx.send(Msg::AgentNew { token: id, stream, v });
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
        let f = sh.opts.paths.agent_dir(&a.dir).join("repl.pid");
        let Ok(pid) = std::fs::read_to_string(&f) else { continue };
        let pid = pid.trim().to_string();
        let cmdline = Command::new("ps")
            .args(["-p", &pid, "-o", "command="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if cmdline.contains("repl-live") {
            log_line(&sh.opts.paths, &format!("killing a stale REPL of {} (pid {})", a.name, pid));
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
        eprintln!("un hub tourne déjà pour {}", paths.workspace.display());
        return Ok(());
    }
    let _ = std::fs::remove_file(paths.socket());
    let listener = UnixListener::bind(paths.socket())?;
    std::fs::write(paths.pid_file(), std::process::id().to_string())?;
    write_shim(&paths, &opts.exe)?;
    log_line(&paths, &format!("hub start pid={} workspace={}", std::process::id(), paths.workspace.display()));

    let workspace = paths.workspace.to_string_lossy().to_string();
    let mut hub = Hub::new(&workspace);
    let events: Vec<Event> = std::fs::read_to_string(paths.journal())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    hub.replay(&events);
    let journal = std::fs::OpenOptions::new().create(true).append(true).open(paths.journal())?;

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
    };
    // the feeds survive a hub restart through their transcripts
    for a in sh.hub.st.agents.values() {
        let tail: VecDeque<String> = transcript_tail(&sh.transcript(&a.dir), BUFFER_LINES)
            .into_iter()
            .map(|(_, l)| l)
            .collect();
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
    kill_stale_repls(&sh);
    sh.step(Input::Boot);

    while let Ok(m) = rx.recv() {
        match m {
            Msg::In(i) => sh.step(i),
            Msg::ReplConnected {
                dir,
                gen,
                stream,
                steer,
                interrupt,
                pid,
            } => {
                if sh.gens.get(&dir) != Some(&gen) {
                    // killed while it was starting
                    let _ = Command::new("kill").arg(pid.to_string()).status();
                    continue;
                }
                let _ = std::fs::write(&steer, "");
                let _ = std::fs::write(&interrupt, "");
                let _ = pid;
                sh.repls.insert(dir.clone(), Repl { stream, steer, interrupt });
                if let Some(name) = sh.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    sh.step(Input::ReplReady { agent: name });
                }
            }
            Msg::ReplSpawned { dir, gen, pid } => {
                let _ = std::fs::write(sh.opts.paths.agent_dir(&dir).join("repl.pid"), pid.to_string());
                if sh.gens.get(&dir) == Some(&gen) {
                    sh.pids.insert(dir, (gen, pid));
                } else {
                    let _ = Command::new("kill").arg(pid.to_string()).status();
                }
            }
            Msg::ReplLine { dir, gen, line } => {
                if sh.gens.get(&dir) == Some(&gen) {
                    sh.on_repl_line(&dir, &line);
                }
            }
            Msg::ReplGone { dir, gen, ok_exit, reason } => {
                // a killed generation is not live anymore: its exit is
                // expected; any exit of the live one is a crash (the hub
                // never asks a REPL to quit)
                if sh.gens.get(&dir) != Some(&gen) {
                    continue;
                }
                sh.gens.remove(&dir);
                sh.repls.remove(&dir);
                sh.pids.remove(&dir);
                let _ = ok_exit;
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
            Msg::Shutdown => break,
        }
    }
    log_line(&paths, "hub stop");
    for (_, pid) in sh.pids.values() {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
    let _ = std::fs::remove_file(paths.socket());
    let _ = std::fs::remove_file(paths.pid_file());
    Ok(())
}
