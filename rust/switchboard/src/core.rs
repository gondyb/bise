//! The hub's decisions now run in Bend (`hub/*.bend`, the `sb-core`
//! process): this module is the Rust side of the link. `Hub::handle`
//! turns one input into a JSON line for sb-core, answers its git queries
//! through `Env`, and turns the effects it returns into `Effect`s. It
//! keeps a read-only mirror of the durable state (for the views: board,
//! snapshot, contexts, prompts), fed only by the journal events and the
//! runtime changes sb-core emits.

use crate::board;
use crate::model::*;
use crate::prompts;
use crate::router::{self, UserCmd};
use crate::util::{clip, clip_tail, one_line, wire_escape};
use crate::wire::{self, Wire};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdout, Command, Stdio};


pub type ClientId = u64;
pub type Token = u64;

/// What a worktree drop would lose (RFC 0002 §5.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loss {
    pub dirty: usize,
    pub unpushed: usize,
}

impl Loss {
    pub fn any(&self) -> bool {
        self.dirty > 0 || self.unpushed > 0
    }
}

/// The side of the world the core may query synchronously.
pub trait Env {
    fn now(&self) -> u64;
    fn is_git(&self) -> bool;
    /// RFC 0002 §4.1: a worktree on `sb/<name>` for this task.
    fn worktree_create(&mut self, name: &str, with_changes: bool) -> Result<Workspace, String>;
    fn worktree_loss(&mut self, ws: &Workspace) -> Loss;
    /// RFC 0002 §5.2-5.3: save if needed, then remove. Answers the ref.
    fn worktree_drop(
        &mut self,
        name: &str,
        ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String>;
    /// RFC 0002 §5.6.
    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        snapshot: Option<&str>,
    ) -> Result<Workspace, String>;
}

/// A request of the `sb` CLI (RFC 0003 §4, RFC 0001 §7.2, §7.5).
#[derive(Clone, Debug, PartialEq)]
pub enum AgentReq {
    List,
    /// `sb tasks`: the detailed state of every task.
    Tasks,
    Send {
        to: String,
        text: String,
        expect_reply: bool,
        reply_to: Option<u64>,
        /// `--mode queued`: delivered only as a new turn.
        queued: bool,
        /// `--why`: main's reason when it answers a task for the user
        /// (shown in the `answered` line of main's feed).
        why: String,
    },
    Wait {
        msg: u64,
        timeout_s: u64,
    },
    Ask {
        to: String,
        text: String,
        timeout_s: u64,
    },
    Status {
        status: Declared,
        note: String,
    },
    Report {
        kind: String,
        summary: String,
        decisions: Vec<String>,
    },
    Spawn {
        name: String,
        brief: Brief,
        worktree: bool,
        with_changes: bool,
    },
    Interrupt {
        agent: String,
    },
    Stop {
        agent: String,
        reason: String,
    },
    Drop {
        agent: String,
    },
    Card {
        text: String,
        for_msg: Option<u64>,
    },
    /// `sb close N ["note"]`: close attention card N with a short note.
    Close {
        card: u64,
        note: String,
    },
    /// `sb rename <task> <new-name>`: the same rules as `/rename`.
    Rename {
        agent: String,
        new_name: String,
    },
    /// `sb restore <task>`: only on the user's explicit request.
    Restore {
        agent: String,
    },
    /// `sb isolate <task>`: only on the user's explicit request.
    Isolate {
        agent: String,
    },
}

/// `m_12` or `12`.
pub fn parse_msg_id(s: &str) -> Option<u64> {
    s.trim().trim_start_matches("m_").parse().ok()
}

fn jstr(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn jstrs(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

impl AgentReq {
    pub fn from_json(v: &Value) -> Result<AgentReq, String> {
        let cmd = jstr(v, "cmd");
        let timeout = v.get("timeout_s").and_then(|x| x.as_u64()).unwrap_or(20);
        let req = match cmd.as_str() {
            "list" => AgentReq::List,
            "tasks" => AgentReq::Tasks,
            "send" => AgentReq::Send {
                to: jstr(v, "to"),
                text: jstr(v, "text"),
                expect_reply: v
                    .get("expect_reply")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
                reply_to: v
                    .get("reply_to")
                    .and_then(|x| x.as_str())
                    .and_then(parse_msg_id),
                queued: match jstr(v, "mode").as_str() {
                    "" | "steer" => false,
                    "queued" => true,
                    m => return Err(format!("unknown mode: {} (steer|queued)", m)),
                },
                why: jstr(v, "why"),
            },
            "wait" => AgentReq::Wait {
                msg: parse_msg_id(&jstr(v, "msg")).ok_or("invalid message id")?,
                timeout_s: timeout,
            },
            "ask" => AgentReq::Ask {
                to: jstr(v, "to"),
                text: jstr(v, "text"),
                timeout_s: timeout,
            },
            "status" => AgentReq::Status {
                status: match jstr(v, "status").as_str() {
                    "working" => Declared::Working,
                    "done" => Declared::Done,
                    "blocked" => Declared::Blocked,
                    s => return Err(format!("unknown status: {} (working|done|blocked)", s)),
                },
                note: jstr(v, "note"),
            },
            "report" => {
                let kind = jstr(v, "kind");
                if !["progress", "done", "failed", "blocked"].contains(&kind.as_str()) {
                    return Err(format!(
                        "unknown report kind: {} (progress|done|failed|blocked)",
                        kind
                    ));
                }
                AgentReq::Report {
                    kind,
                    summary: jstr(v, "summary"),
                    decisions: jstrs(v, "decisions"),
                }
            }
            "spawn" => AgentReq::Spawn {
                name: jstr(v, "name"),
                brief: Brief {
                    objective: jstr(v, "objective"),
                    context: jstr(v, "context"),
                    constraints: jstrs(v, "constraints"),
                    done_when: Some(jstr(v, "done_when")).filter(|s| !s.is_empty()),
                    report_format: Some(jstr(v, "report_format")).filter(|s| !s.is_empty()),
                },
                worktree: v.get("worktree").and_then(|x| x.as_bool()).unwrap_or(false),
                with_changes: v
                    .get("with_changes")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            },
            "interrupt" => AgentReq::Interrupt {
                agent: jstr(v, "agent"),
            },
            "stop" => AgentReq::Stop {
                agent: jstr(v, "agent"),
                reason: jstr(v, "reason"),
            },
            "drop" => AgentReq::Drop {
                agent: jstr(v, "agent"),
            },
            "card" => AgentReq::Card {
                text: jstr(v, "text"),
                for_msg: v.get("for").and_then(|x| x.as_str()).and_then(parse_msg_id),
            },
            "close" => AgentReq::Close {
                card: v
                    .get("card")
                    .and_then(|x| x.as_u64())
                    .ok_or("usage: sb close <card> [\"<note>\"]")?,
                note: jstr(v, "note"),
            },
            "rename" => AgentReq::Rename {
                agent: jstr(v, "agent"),
                new_name: jstr(v, "new_name"),
            },
            "restore" => AgentReq::Restore {
                agent: jstr(v, "agent"),
            },
            "isolate" => AgentReq::Isolate {
                agent: jstr(v, "agent"),
            },
            other => return Err(format!("unknown command: {}", other)),
        };
        Ok(req)
    }
}

#[derive(Clone, Debug)]
pub enum Input {
    /// The daemon started: spawn every live agent.
    Boot,
    /// The REPL of `agent` is connected and idle.
    ReplReady {
        agent: String,
    },
    ReplLine {
        agent: String,
        line: String,
    },
    /// `--- idle`. `leftover`: steering written during the turn was still
    /// in the file (never read by the runtime).
    ReplIdle {
        agent: String,
        leftover: bool,
    },
    ReplExited {
        agent: String,
        crashed: bool,
        reason: String,
    },
    ClientHello {
        client: ClientId,
    },
    ClientInput {
        client: ClientId,
        focus: String,
        text: String,
    },
    ClientFocus {
        client: ClientId,
        focus: String,
    },
    ClientGone {
        client: ClientId,
    },
    ClientConfirm {
        client: ClientId,
        id: u64,
        yes: bool,
    },
    ClientInterrupt {
        client: ClientId,
        agent: String,
    },
    Agent {
        token: Token,
        from: String,
        req: AgentReq,
    },
    Tick,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // short-lived, one list per input
pub enum Effect {
    /// Append to the journal (already applied to the state).
    Journal(Event),
    Spawn {
        agent: String,
        resume: bool,
        crash_note: Option<String>,
    },
    Kill {
        agent: String,
    },
    /// A new turn: `say <text>` on the REPL socket (the agent is idle).
    Say {
        agent: String,
        text: String,
    },
    /// Steering: appended to the steering file (the agent is busy).
    Steer {
        agent: String,
        text: String,
    },
    /// A raw REPL command line (`/compact`), the agent is idle.
    Passthrough {
        agent: String,
        line: String,
    },
    Interrupt {
        agent: String,
    },
    /// Rewrite the agent's BEND_CONTEXT_FILE.
    Context {
        agent: String,
        text: String,
    },
    /// A synthetic line in the agent's feed (`sb <kind> : <text>`).
    Line {
        agent: String,
        line: String,
    },
    /// The answer to an `sb` request.
    Reply {
        token: Token,
        body: Value,
    },
    ToClient {
        client: ClientId,
        body: Value,
    },
    Renamed {
        old: String,
        new: String,
    },
    /// The state changed: broadcast a snapshot.
    State,
}

/// The sb-core executable: `SB_CORE_BIN`, else `sb-core` at the root of
/// the repository this crate was built from.
pub fn core_bin() -> std::path::PathBuf {
    match std::env::var("SB_CORE_BIN") {
        Ok(p) if !p.is_empty() => p.into(),
        _ => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sb-core"),
    }
}

/// One sb-core process and its connection.
pub struct CoreLink {
    child: Child,
    _out: BufReader<ChildStdout>,
    w: TcpStream,
    r: BufReader<TcpStream>,
}

impl CoreLink {
    /// Start sb-core on a free port. A port taken meanwhile (parallel
    /// hubs) is retried on another one.
    pub fn start() -> std::io::Result<CoreLink> {
        let mut last = None;
        for _ in 0..5 {
            match CoreLink::try_start() {
                Ok(l) => return Ok(l),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap())
    }

    fn try_start() -> std::io::Result<CoreLink> {
        let port = std::net::TcpListener::bind("127.0.0.1:0")?
            .local_addr()?
            .port();
        let mut child = Command::new(core_bin())
            .env("SB_CORE_PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut out = BufReader::new(child.stdout.take().expect("stdout"));
        let mut banner = String::new();
        out.read_line(&mut banner)?;
        let conn = if banner.starts_with("sb-core on") {
            TcpStream::connect(("127.0.0.1", port))
        } else {
            Err(std::io::Error::other(format!("sb-core did not start: {:?}", banner)))
        };
        let w = match conn {
            Ok(w) => w,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        w.set_nodelay(true)?;
        let r = BufReader::new(w.try_clone()?);
        Ok(CoreLink {
            child,
            _out: out,
            w,
            r,
        })
    }

    /// One input line, one answer line.
    pub fn call(&mut self, v: &Value) -> Value {
        let mut line = v.to_string();
        line.push('\n');
        self.w.write_all(line.as_bytes()).expect("sb-core: write");
        let mut back = String::new();
        self.r.read_line(&mut back).expect("sb-core: read");
        serde_json::from_str(&back)
            .unwrap_or_else(|e| panic!("sb-core: bad answer {:?}: {}", back, e))
    }
}

impl Drop for CoreLink {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Debug)]
struct ClientView {
    focus: String,
    since_ms: u64,
    sent: Vec<String>,
    last_route: Option<u64>,
}

pub struct Hub {
    /// The mirror of sb-core's durable state (plus the runtime fields it
    /// reports), for the views.
    pub st: State,
    pub workspace: String,
    clients: BTreeMap<ClientId, ClientView>,
    /// Drop confirmations pending on a client: id -> (client, task).
    confirms: BTreeMap<u64, (ClientId, String)>,
    next_confirm: u64,
    /// (time, text) of recent assistant messages, for direct-exchange
    /// excerpts.
    recent: BTreeMap<String, VecDeque<(u64, String)>>,
    contexts: BTreeMap<String, String>,
    /// The last thing each agent did (from its REPL lines, for the views).
    activity: BTreeMap<String, (u64, String)>,
    dirty: bool,
    link: CoreLink,
}

type Fx = Vec<Effect>;

fn line(agent: &str, kind: &str, text: &str) -> Effect {
    Effect::Line {
        agent: agent.to_string(),
        line: format!("sb {} : {}", kind, wire_escape(text)),
    }
}

/// The separator of the fields of a hub line (C2: `answered`).
pub const FIELD_SEP: &str = " : ";

/// A field of a multi-field hub line: a `" : "` inside it becomes
/// `" \\: "`, so the reader splits on the real separators only (the TUI's
/// `parse_hub_line` undoes it).
pub fn field_escape(s: &str) -> String {
    s.replace(FIELD_SEP, " \\: ")
}

/// The text of a hub line made of fields (C2 `answered`: agent,
/// question, answer, why).
pub fn join_fields(fields: &[String]) -> String {
    fields.iter().map(|f| field_escape(f)).collect::<Vec<_>>().join(FIELD_SEP)
}

fn notice(client: ClientId, text: &str) -> Effect {
    Effect::ToClient {
        client,
        body: json!({"ev": "notice", "text": text}),
    }
}

fn parse<T: serde::de::DeserializeOwned>(x: &Value) -> Option<T> {
    T::deserialize(x).ok()
}

fn run_of(s: &str) -> Run {
    match s {
        "starting" => Run::Starting,
        "idle" => Run::Idle,
        "busy" => Run::Busy,
        _ => Run::Down,
    }
}

impl Hub {
    pub fn new(workspace: &str) -> Hub {
        let mut link = CoreLink::start().unwrap_or_else(|e| {
            panic!("sb-core introuvable ({}): {}", core_bin().display(), e)
        });
        link.call(&json!({"t": "init", "workspace": workspace}));
        let mut hub = Hub {
            st: State::new(workspace),
            workspace: workspace.to_string(),
            clients: BTreeMap::new(),
            confirms: BTreeMap::new(),
            next_confirm: 1,
            recent: BTreeMap::new(),
            contexts: BTreeMap::new(),
            activity: BTreeMap::new(),
            dirty: false,
            link,
        };
        hub.view_all();
        hub
    }

    /// Tests only: put an agent's REPL in a given state (never sent by
    /// the daemon).
    #[cfg(test)]
    pub fn force_run(&mut self, agent: &str, run: Run) {
        let r = match run {
            Run::Down => "down",
            Run::Starting => "starting",
            Run::Idle => "idle",
            Run::Busy => "busy",
        };
        let out = self.link.call(&json!({"t": "force_run", "agent": agent, "run": r}));
        self.load_view(&out["view"]);
    }

    /// Rebuild the durable state from the journal: sb-core replays it,
    /// then sends the whole state.
    pub fn replay(&mut self, events: &[Event]) {
        for ev in events {
            self.link.call(&json!({"t": "replay", "ev": ev}));
        }
        self.view_all();
    }

    fn view_all(&mut self) {
        let out = self.link.call(&json!({"t": "view_all"}));
        self.load_view(&out["view"]);
    }

    /// Store the state sb-core sent (it is the only source of truth).
    fn load_view(&mut self, v: &Value) {
        let mut agents = BTreeMap::new();
        for a in v["agents"].as_array().into_iter().flatten() {
            let name = jstr(a, "name");
            let declared = a["declared"]
                .as_object()
                .and_then(|d| parse(&d["status"]).map(|st| (st, jstr(&a["declared"], "note"))));
            let agent = Agent {
                name: name.clone(),
                dir: jstr(a, "dir"),
                is_main: a["is_main"].as_bool().unwrap_or(false),
                parent: a["parent"].as_str().map(|x| x.to_string()),
                brief: parse(&a["brief"]).unwrap_or_default(),
                created_ms: a["created_ms"].as_u64().unwrap_or(0),
                ws: parse(&a["ws"]).unwrap_or_else(|| panic!("sb-core: bad ws {}", a["ws"])),
                lifecycle: parse(&a["lifecycle"]).unwrap_or(Lifecycle::Active),
                failure: a["failure"].as_str().map(|x| x.to_string()),
                declared,
                last_report: parse(&a["last_report"]),
                aliases: parse(&a["aliases"]).unwrap_or_default(),
                files: parse(&a["files"]).unwrap_or_default(),
                snapshot_ref: a["snapshot_ref"].as_str().map(|x| x.to_string()),
                run: run_of(&jstr(a, "run")),
                waiting: a["waiting"].as_bool().unwrap_or(false),
                turn_started_ms: a["turn_ms"].as_u64(),
                activity: self.activity.get(&name).cloned(),
            };
            agents.insert(name, agent);
        }
        self.st.agents = agents;
        self.st.order = parse(&v["order"]).unwrap_or_default();
        if v["all_msgs"].as_bool() == Some(true) {
            self.st.msgs.clear();
            self.st.msg_state.clear();
            self.st.settled.clear();
        }
        for r in v["msgs"].as_array().into_iter().flatten() {
            let Some(m) = parse(&r["msg"]) else { continue };
            let m: Msg = m;
            if let Some(state) = parse(&r["state"]) {
                self.st.msg_state.insert(m.id, state);
            }
            if r["settled"].as_bool() == Some(true) {
                self.st.settled.insert(m.id);
            } else {
                self.st.settled.remove(&m.id);
            }
            self.st.msgs.insert(m.id, m);
        }
        self.st.cards = v["cards"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| parse(c).map(|c: Card| (c.id, c)))
            .collect();
        self.st.main_notes = parse(&v["notes"]).unwrap_or_default();
        self.st.next_msg = v["next_msg"].as_u64().unwrap_or(1);
        self.st.next_card = v["next_card"].as_u64().unwrap_or(1);
    }

    /// The agent's last activity, for the views (not a decision).
    fn set_activity(&mut self, agent: &str, now: u64, what: String) {
        if let Some(a) = self.st.agents.get_mut(agent) {
            a.activity = Some((now, what.clone()));
            self.activity.insert(agent.to_string(), (now, what));
        }
    }
    /// The client snapshot (agents, cards) for the views.
    pub fn snapshot(&self, now: u64) -> Value {
        let agents: Vec<Value> = self
            .st
            .order
            .iter()
            .filter_map(|n| self.st.agents.get(n))
            .map(|a| {
                json!({
                    "name": a.name,
                    "main": a.is_main,
                    "status": a.status().as_str(),
                    "objective": a.description(),
                    "parent": a.parent,
                    "mode": match a.ws.mode { Mode::Worktree => "worktree", Mode::Shared => "shared" },
                    "path": a.ws.path,
                    "branch": a.ws.branch,
                    "dropped": a.ws.dropped,
                    "created_ms": a.created_ms,
                    "note": a.declared.as_ref().map(|(_, n)| n.clone()).unwrap_or_default(),
                    "report": a.last_report.as_ref().map(|r| clip(&one_line(&r.summary), 200)),
                    // when it last reported (an archived task: about when it stopped)
                    "report_ms": a.last_report.as_ref().map(|r| r.at_ms),
                    "queued": board::queued_count(&self.st, &a.name),
                    "turn_ms": a.turn_started_ms.map(|t| now.saturating_sub(t)),
                })
            })
            .collect();
        let cards: Vec<Value> = self
            .st
            .open_cards()
            .map(|c| {
                json!({
                    "id": c.id,
                    "kind": c.kind,
                    "agent": c.agent,
                    "text": c.text,
                    "for_msg": c.for_msg,
                    "age_ms": now.saturating_sub(c.created_ms),
                    "note": self.card_note(c),
                })
            })
            .collect();
        json!({"ev": "state", "agents": agents, "cards": cards})
    }

    /// A question card whose asker heard from main since, without a
    /// reply to the question: maybe answered another way (the card
    /// stays open, the view says so).
    fn card_note(&self, c: &Card) -> Option<String> {
        if c.kind != "question" || c.agent == MAIN {
            return None;
        }
        let m = self
            .st
            .msgs
            .values()
            .rev()
            .take_while(|m| m.created_ms >= c.created_ms)
            .find(|m| m.from == MAIN && m.to == c.agent && m.reply_to != c.for_msg)?;
        Some(format!(
            "@main wrote to @{} since then (m_{}): {}",
            c.agent,
            m.id,
            clip(&one_line(&m.text), 120)
        ))
    }


    pub fn handle(&mut self, input: Input, env: &mut dyn Env) -> Fx {
        let mut fx = Fx::new();
        match input {
            Input::Boot => self.core(&mut fx, env, None, json!({"t": "boot"})),
            Input::ReplReady { agent } => {
                self.core(&mut fx, env, None, json!({"t": "ready", "agent": agent}))
            }
            Input::ReplLine { agent, line } => self.repl_line(&mut fx, env, &agent, &line),
            Input::ReplIdle { agent, leftover } => self.core(
                &mut fx,
                env,
                None,
                json!({"t": "idle", "agent": agent, "leftover": leftover}),
            ),
            Input::ReplExited {
                agent,
                crashed,
                reason,
            } => self.core(
                &mut fx,
                env,
                None,
                json!({"t": "exited", "agent": agent, "crashed": crashed, "reason": reason}),
            ),
            Input::ClientHello { client } => {
                self.clients.insert(
                    client,
                    ClientView {
                        focus: MAIN.to_string(),
                        since_ms: env.now(),
                        sent: Vec::new(),
                        last_route: None,
                    },
                );
            }
            Input::ClientInput {
                client,
                focus,
                text,
            } => self.user_input(&mut fx, env, client, &focus, &text),
            Input::ClientFocus { client, focus } => self.set_focus(&mut fx, env, client, &focus),
            Input::ClientGone { client } => {
                self.set_focus(&mut fx, env, client, MAIN);
                self.clients.remove(&client);
            }
            Input::ClientConfirm { client, id, yes } => self.confirm(&mut fx, env, client, id, yes),
            Input::ClientInterrupt { client, agent } => self.core(
                &mut fx,
                env,
                Some(client),
                json!({"t": "interrupt", "agent": agent}),
            ),
            Input::Agent { token, from, req } => self.agent_req(&mut fx, env, token, &from, req),
            Input::Tick => self.core(&mut fx, env, None, json!({"t": "tick"})),
        }
        self.refresh_contexts(&mut fx, env.now());
        if self.dirty {
            fx.push(Effect::State);
            self.dirty = false;
        }
        fx
    }

    /// Run one input in sb-core: answer its git queries (the input is
    /// replayed with the answers), then apply the effects it returns.
    fn core(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, input: Value) {
        let mut input = input;
        input["now"] = json!(env.now());
        input["git"] = json!(env.is_git());
        let mut ans: Vec<Value> = Vec::new();
        loop {
            input["ans"] = Value::Array(ans.clone());
            let out = self.link.call(&input);
            if let Some(q) = out.get("need") {
                let a = self.query(env, q);
                ans.push(a);
                continue;
            }
            if let Some(e) = out.get("error") {
                panic!("sb-core: {}", e);
            }
            if out.get("dirty").and_then(|d| d.as_bool()) == Some(true) {
                self.dirty = true;
            }
            // the state after the step first: the deliveries render from it
            self.load_view(&out["view"]);
            for f in out.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
                self.effect(fx, env, client, f);
            }
            return;
        }
    }

    /// A git query of sb-core (RFC 0002), on the workspace the mirror
    /// knows for the task.
    fn query(&mut self, env: &mut dyn Env, q: &Value) -> Value {
        let name = jstr(q, "name");
        let ws: Option<Workspace> = serde_json::from_value(q["ws"].clone()).ok();
        let snap = q["snapshot_ref"].as_str().map(|x| x.to_string());
        let res = |r: Result<Value, String>| match r {
            Ok(v) => json!({"ok": v}),
            Err(e) => json!({"err": e}),
        };
        match jstr(q, "q").as_str() {
            "worktree_create" => {
                let wc = q.get("with_changes").and_then(|x| x.as_bool()).unwrap_or(false);
                res(env.worktree_create(&name, wc).map(|w| json!(w)))
            }
            "worktree_loss" => {
                let l = ws.map(|w| env.worktree_loss(&w)).unwrap_or_default();
                json!({"dirty": l.dirty, "unpushed": l.unpushed})
            }
            "worktree_drop" => {
                let n = |k: &str| q.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let loss = Loss {
                    dirty: n("dirty"),
                    unpushed: n("unpushed"),
                };
                match ws {
                    Some(w) => res(env.worktree_drop(&name, &w, &loss).map(|r| json!(r))),
                    None => json!({"err": "no task"}),
                }
            }
            "worktree_restore" => match ws {
                Some(w) => res(env
                    .worktree_restore(&name, &w, snap.as_deref())
                    .map(|w| json!(w))),
                None => json!({"err": "no task"}),
            },
            other => json!({"err": format!("unknown query: {}", other)}),
        }
    }

    /// One effect of sb-core.
    fn effect(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, f: &Value) {
        let agent = jstr(f, "agent");
        match jstr(f, "fx").as_str() {
            "journal" => {
                let ev: Event = serde_json::from_value(f["ev"].clone())
                    .unwrap_or_else(|e| panic!("sb-core: bad event {}: {}", f["ev"], e));
                fx.push(Effect::Journal(ev));
            }
            // the runtime state comes with the view
            "rt" => {}
            "spawn" => fx.push(Effect::Spawn {
                agent,
                resume: f["resume"].as_bool().unwrap_or(false),
                crash_note: f["crash_note"].as_str().map(|x| x.to_string()),
            }),
            "kill" => fx.push(Effect::Kill { agent }),
            "say" => fx.push(Effect::Say {
                agent,
                text: jstr(f, "text"),
            }),
            "passthrough" => fx.push(Effect::Passthrough {
                agent,
                line: jstr(f, "line"),
            }),
            "interrupt" => fx.push(Effect::Interrupt { agent }),
            "line" => {
                // C2: a multi-field line (`answered`) comes as `fields`
                let text = match f.get("fields") {
                    Some(_) => join_fields(&jstrs(f, "fields")),
                    None => jstr(f, "text"),
                };
                fx.push(line(&agent, &jstr(f, "kind"), &text))
            }
            "reply" => fx.push(Effect::Reply {
                token: f["token"].as_u64().unwrap_or(0),
                body: f["body"].clone(),
            }),
            "deliver" => self.deliver(fx, env, f),
            other => self.client_effect(fx, client, other, f),
        }
    }

    /// The effects on the client that typed the input.
    fn client_effect(&mut self, fx: &mut Fx, client: Option<ClientId>, kind: &str, f: &Value) {
        match kind {
            "notice" => {
                if let Some(c) = client {
                    fx.push(notice(c, &jstr(f, "text")));
                }
            }
            "confirm" => {
                if let Some(c) = client {
                    let id = self.next_confirm;
                    self.next_confirm += 1;
                    self.confirms.insert(id, (c, jstr(f, "name")));
                    fx.push(Effect::ToClient {
                        client: c,
                        body: json!({"ev": "confirm", "id": id, "text": jstr(f, "text")}),
                    });
                }
            }
            "focus_main" => {
                let name = jstr(f, "name");
                for (c, v) in self.clients.iter() {
                    if v.focus == name {
                        fx.push(Effect::ToClient {
                            client: *c,
                            body: json!({"ev": "focus", "focus": MAIN}),
                        });
                    }
                }
            }
            "user_sent" => {
                if let Some(v) = client.and_then(|c| self.clients.get_mut(&c)) {
                    v.sent.push(jstr(f, "text"));
                }
            }
            "routed" => {
                if let Some(v) = client.and_then(|c| self.clients.get_mut(&c)) {
                    v.last_route = f["id"].as_u64();
                }
            }
            "renamed" => {
                let (old, new) = (jstr(f, "old"), jstr(f, "new"));
                for v in self.clients.values_mut().filter(|v| v.focus == old) {
                    v.focus = new.clone();
                }
                fx.push(Effect::Renamed { old, new });
            }
            // a newer sb-core (a version switch in flight) may know
            // effects this hub does not: skipped and logged, never a crash
            other => eprintln!("sb-core: unknown effect {} (skipped): {}", other, f),
        }
    }

    /// The text a delivery puts in the agent's context: main's notes, the
    /// task status block, then each message as the recipient reads it.
    fn deliver(&mut self, fx: &mut Fx, env: &mut dyn Env, f: &Value) {
        let agent = jstr(f, "agent");
        let mut parts: Vec<String> = Vec::new();
        let notes: Vec<String> = f["notes"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|x| x.to_string())).collect())
            .unwrap_or_default();
        if !notes.is_empty() {
            let mut t = String::from("<switchboard_notes>\n");
            for n in &notes {
                t.push_str(&format!("- {}\n", n));
            }
            t.push_str("</switchboard_notes>");
            parts.push(t);
        }
        if f["status"].as_bool() == Some(true) {
            let status = board::status_block(&self.st, env.now());
            if !status.is_empty() {
                parts.push(status);
            }
        }
        for m in f["msgs"].as_array().into_iter().flatten() {
            let id = m["id"].as_u64().unwrap_or(0);
            if let Some(msg) = self.st.msgs.get(&id) {
                parts.push(prompts::tagged(msg, &jstr(m, "rel")));
            }
        }
        let text = parts.join("\n\n");
        if jstr(f, "mode") == "steer" {
            fx.push(Effect::Steer { agent, text });
        } else {
            fx.push(Effect::Say { agent, text });
        }
    }

    fn repl_line(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, raw: &str) {
        let now = env.now();
        let t = |kind: &str| json!({"t": kind, "agent": agent});
        match wire::parse(raw) {
            Wire::TurnStarted => self.core(fx, env, None, t("turn_started")),
            Wire::SteeringReceived => self.core(fx, env, None, t("steer_rx")),
            Wire::Steered => self.core(fx, env, None, t("steered")),
            Wire::Assistant(text) if !text.is_empty() => {
                self.set_activity(agent, now, format!("wrote: {}", clip(&one_line(&text), 160)));
                let r = self.recent.entry(agent.to_string()).or_default();
                r.push_back((now, text.clone()));
                if r.len() > 20 {
                    r.pop_front();
                }
                self.core(
                    fx,
                    env,
                    None,
                    json!({"t": "assistant", "agent": agent, "text": text}),
                );
            }
            Wire::Tool { name, args } if name != "apply_patch" => {
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, format!("{} `{}`", name, clip(&one_line(&args), 120)));
                    self.dirty = true;
                }
            }
            Wire::Tool { args, .. } => {
                let files = wire::patch_files(&args);
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, format!("apply_patch {}", files.join(", ")));
                    self.dirty = true;
                }
                for path in files {
                    self.core(
                        fx,
                        env,
                        None,
                        json!({"t": "touch", "agent": agent, "path": path}),
                    );
                }
            }
            // BR-007: a task whose turn failed (network down, provider
            // error...) must not go quiet: the failure reaches its parent
            // as a report (board + message), like a report the task wrote.
            // Main's own failures show in main's view (its turn_done line);
            // a stop the user asked for (Ctrl+C) is not news to anyone.
            Wire::TurnDone(t) => {
                if let Some(summary) = failed_turn_report(agent, &t) {
                    if self.st.agents.contains_key(agent) {
                        let q = json!({"cmd": "report", "kind": "turn_failed",
                            "summary": summary, "decisions": []});
                        // token 0: no connection waits for this reply
                        // (connection tokens start at 1)
                        self.core(
                            fx,
                            env,
                            None,
                            json!({"t": "req", "token": 0, "from": agent, "req": q}),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn user_input(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        client: ClientId,
        focus: &str,
        text: &str,
    ) {
        let focus = self.st.resolve(focus).unwrap_or_else(|| MAIN.to_string());
        if let Some(v) = self.clients.get_mut(&client) {
            if v.focus != focus {
                v.focus = focus.clone();
            }
        }
        let c = Some(client);
        match router::parse(text, &focus) {
            UserCmd::Say(t) => {
                if !t.is_empty() {
                    self.core(fx, env, c, json!({"t": "say", "focus": focus, "text": t}));
                }
            }
            UserCmd::To { target, text } => self.core(
                fx,
                env,
                c,
                json!({"t": "route", "target": target, "text": text, "focus": focus}),
            ),
            UserCmd::New {
                name,
                brief,
                worktree,
                with_changes,
            } => {
                let b = Brief {
                    objective: brief,
                    ..Brief::default()
                };
                match new_task(name.as_deref(), &b, worktree, with_changes) {
                    Ok(mut v) => {
                        v["t"] = json!("new");
                        self.core(fx, env, c, v)
                    }
                    Err(e) => fx.push(notice(client, &e)),
                }
            }
            UserCmd::Drop { name, force } => match name {
                None => fx.push(notice(
                    client,
                    "usage: /drop <task> (or /drop from the task's view)",
                )),
                Some(name) => self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "drop", "name": name, "force": force}),
                ),
            },
            UserCmd::Restore { name } => {
                self.core(fx, env, c, json!({"t": "restore", "name": name}))
            }
            UserCmd::Isolate { name } => {
                self.core(fx, env, c, json!({"t": "isolate", "name": name}))
            }
            UserCmd::Rename { name, new_name } => {
                let valid = router::valid_name(&new_name);
                self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "rename", "name": name, "new_name": new_name, "valid": valid}),
                )
            }
            UserCmd::Answer { card, text } => {
                self.core(fx, env, c, json!({"t": "answer", "card": card, "text": text}))
            }
            UserCmd::Close { card } => {
                self.core(fx, env, c, json!({"t": "close", "card": card}))
            }
            UserCmd::Cancel => {
                let last = self.clients.get(&client).and_then(|v| v.last_route);
                self.core(fx, env, c, json!({"t": "cancel", "last": last}))
            }
            UserCmd::Tasks => fx.push(notice(client, &board::user_board(&self.st, env.now()))),
            UserCmd::Interrupt => {
                self.core(fx, env, c, json!({"t": "interrupt", "agent": focus}))
            }
            UserCmd::Passthrough(l) => {
                let first = l.split_whitespace().next().unwrap_or("");
                if first != "/compact" {
                    fx.push(notice(
                        client,
                        &format!("unknown command: {} (see /help)", first),
                    ));
                    return;
                }
                self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "passthrough", "focus": focus, "line": l}),
                )
            }
            UserCmd::Help => fx.push(notice(client, HELP)),
            UserCmd::Invalid(e) => fx.push(notice(client, &e)),
        }
    }

    fn set_focus(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, focus: &str) {
        let now = env.now();
        let Some(view) = self.clients.get_mut(&client) else {
            return;
        };
        let prev = std::mem::replace(
            view,
            ClientView {
                focus: focus.to_string(),
                since_ms: now,
                sent: Vec::new(),
                last_route: view.last_route,
            },
        );
        let mut input = json!({"t": "focus", "focus": focus, "note": null, "direct": ""});
        if prev.focus != MAIN && prev.focus != focus && !prev.sent.is_empty() {
            // RFC 0001 §7.4: main learns what the user decided directly
            let reply: String = self
                .recent
                .get(&prev.focus)
                .map(|r| {
                    r.iter()
                        .filter(|(t, _)| *t >= prev.since_ms)
                        .map(|(_, s)| s.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let (note, direct) = direct_exchange(&prev.focus, &prev.sent, &reply);
            input["note"] = json!(note);
            input["direct"] = json!(direct);
        }
        self.core(fx, env, Some(client), input);
    }

    fn confirm(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, id: u64, yes: bool) {
        let Some((_, name)) = self.confirms.remove(&id) else {
            return;
        };
        if !yes {
            fx.push(notice(client, &format!("drop of @{} cancelled", name)));
            return;
        }
        self.core(fx, env, Some(client), json!({"t": "confirm_drop", "name": name}));
    }

    fn refresh_contexts(&mut self, fx: &mut Fx, now: u64) {
        let names: Vec<String> = self
            .st
            .agents
            .values()
            .filter(|a| a.lifecycle == Lifecycle::Active)
            .map(|a| a.name.clone())
            .collect();
        for name in names {
            let text = if name == MAIN {
                board::main_context(&self.st, now)
            } else {
                board::task_context(&self.st, &name, now)
            };
            if self.contexts.get(&name) != Some(&text) {
                self.contexts.insert(name.clone(), text.clone());
                fx.push(Effect::Context { agent: name, text });
            }
        }
    }

    fn agent_req(&mut self, fx: &mut Fx, env: &mut dyn Env, token: Token, from: &str, req: AgentReq) {
        let reply = |fx: &mut Fx, body: Value| fx.push(Effect::Reply { token, body });
        let q = match req {
            AgentReq::List | AgentReq::Tasks => {
                let Some(from) = self.st.resolve(from) else {
                    reply(fx, json!({"ok": false, "error": format!("unknown agent: {}", from)}));
                    return;
                };
                let text = if req == AgentReq::List {
                    board::roster(&self.st, &from, env.now()).join("\n")
                } else {
                    board::tasks_detail(&self.st, env.now())
                };
                reply(fx, json!({"ok": true, "text": text}));
                return;
            }
            AgentReq::Send {
                to,
                text,
                expect_reply,
                reply_to,
                queued,
                why,
            } => json!({"cmd": "send", "to": to, "text": text, "expect_reply": expect_reply,
                        "reply_to": reply_to, "queued": queued, "why": why}),
            AgentReq::Wait { msg, timeout_s } => {
                json!({"cmd": "wait", "msg": msg, "timeout_s": timeout_s})
            }
            AgentReq::Ask {
                to,
                text,
                timeout_s,
            } => json!({"cmd": "ask", "to": to, "text": text, "timeout_s": timeout_s}),
            AgentReq::Status { status, note } => {
                json!({"cmd": "status", "status": status, "note": note})
            }
            AgentReq::Report {
                kind,
                summary,
                decisions,
            } => json!({"cmd": "report", "kind": kind, "summary": summary, "decisions": decisions}),
            AgentReq::Spawn {
                name,
                brief,
                worktree,
                with_changes,
            } => {
                let name = Some(name.as_str()).filter(|n| !n.is_empty());
                match new_task(name, &brief, worktree, with_changes) {
                    Ok(mut v) => {
                        v["cmd"] = json!("spawn");
                        v
                    }
                    Err(e) => json!({"cmd": "spawn", "pre_err": e}),
                }
            }
            AgentReq::Interrupt { agent } => json!({"cmd": "interrupt", "agent": agent}),
            AgentReq::Stop { agent, reason } => {
                json!({"cmd": "stop", "agent": agent, "reason": reason})
            }
            AgentReq::Drop { agent } => json!({"cmd": "drop", "agent": agent}),
            AgentReq::Card { text, for_msg } => {
                json!({"cmd": "card", "text": text, "for": for_msg})
            }
            AgentReq::Close { card, note } => json!({"cmd": "close", "card": card, "note": note}),
            AgentReq::Rename { agent, new_name } => {
                let valid = router::valid_name(&new_name);
                json!({"cmd": "rename", "agent": agent, "new_name": new_name, "valid": valid})
            }
            AgentReq::Restore { agent } => json!({"cmd": "restore", "agent": agent}),
            AgentReq::Isolate { agent } => json!({"cmd": "isolate", "agent": agent}),
        };
        self.core(
            fx,
            env,
            None,
            json!({"t": "req", "token": token, "from": from, "req": q}),
        );
    }
}

/// RFC 0001 §7.4: what main (`note`) and the user's own feed (`direct`)
/// read after the user talked directly to `task`: the messages sent,
/// and the end of the task's reply since.
fn direct_exchange(task: &str, sent: &[String], reply: &str) -> (String, String) {
    let quoted: Vec<String> = sent.iter().map(|m| format!("\"{}\"", one_line(m))).collect();
    let n = sent.len();
    let s = if n > 1 { "s" } else { "" };
    let note = format!(
        "The user talked directly to @{} ({} message{}): {}. Last reply of @{}: \"{}\"",
        task,
        n,
        s,
        quoted.join(", "),
        task,
        clip_tail(&one_line(reply), 2000)
    );
    (note, format!("You talked to @{} ({} message{})", task, n, s))
}

/// BR-007: the report a failed turn of a task sends to its parent, or
/// None when there is nothing to report: a completed or interrupted
/// turn, main (its own view shows the failure), or a retry loop the user
/// stopped on purpose.
fn failed_turn_report(agent: &str, turn_done: &str) -> Option<String> {
    let why = turn_done.strip_prefix("failed: ")?;
    if agent == "main" || why.starts_with("stopped retrying (interrupted by the user)") {
        return None;
    }
    Some(format!(
        "my turn failed: {} — a new message retries it",
        why
    ))
}

/// The checks and texts of a new task the daemon prepares for sb-core
/// (RFC 0001 §7.1): a valid name or the slug of the objective, the brief
/// as the task reads it (sb-core adds the `# Task` header with the final
/// name).
fn new_task(name: Option<&str>, brief: &Brief, worktree: bool, with_changes: bool) -> Result<Value, String> {
    if brief.objective.trim().is_empty() {
        return Err("empty objective".into());
    }
    let base = match name {
        Some(n) if !n.is_empty() => {
            if !router::valid_name(n) {
                return Err(format!("invalid name: {} ([a-z0-9-], 24 characters max)", n));
            }
            n.to_string()
        }
        _ => router::slug(&brief.objective),
    };
    Ok(json!({"base": base, "brief": brief, "brief_text": prompts::brief_body(brief), "objective": brief.objective,
              "worktree": worktree, "with_changes": with_changes}))
}


pub const HELP: &str = "\
plain text        message to the agent in view (main by default)
@task text        direct message to a task, without main (@main from a task)
/new [-w] [name:] objective   create a task (-w: isolated git worktree, --with-changes: with your changes)
/drop [task] [--force]        stop and archive a task (and delete its worktree)
/restore task     reopen an archived task (and its saved worktree)
/isolate task     give a worktree to a task that has not changed anything yet
/rename a b       rename a task (the old name still works)
/answer N text    answer attention card N
/cancel           cancel the last route if it is not delivered yet
/tasks            the task board
/interrupt        interrupt the turn of the agent in view
/compact          compact the conversation of the agent in view";

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
