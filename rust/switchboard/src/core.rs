//! The hub's decisions: a pure state machine. `Hub::handle` turns one
//! input (a REPL line, a client command, an agent CLI request, a tick)
//! into effects the daemon executes. Git is behind the `Env` trait so the
//! tests run without processes or repositories.

use crate::board;
use crate::model::*;
use crate::prompts;
use crate::router::{self, UserCmd};
use crate::util::{clip, clip_tail, one_line, wire_escape};
use crate::wire::{self, Wire};
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
                    m => return Err(format!("mode inconnu : {} (steer|queued)", m)),
                },
            },
            "wait" => AgentReq::Wait {
                msg: parse_msg_id(&jstr(v, "msg")).ok_or("identifiant de message invalide")?,
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
                    s => return Err(format!("statut inconnu : {} (working|done|blocked)", s)),
                },
                note: jstr(v, "note"),
            },
            "report" => {
                let kind = jstr(v, "kind");
                if !["progress", "done", "failed", "blocked"].contains(&kind.as_str()) {
                    return Err(format!(
                        "type de rapport inconnu : {} (progress|done|failed|blocked)",
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
            other => return Err(format!("commande inconnue : {}", other)),
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

#[derive(Clone, Debug)]
struct ClientView {
    focus: String,
    since_ms: u64,
    sent: Vec<String>,
    last_route: Option<u64>,
}

#[derive(Clone, Debug)]
struct Waiter {
    token: Token,
    agent: String,
    msg: u64,
    deadline_ms: u64,
}

#[derive(Clone, Debug)]
enum Pending {
    Drop { name: String },
}

/// Technical bounds only: no cap on messages, threads or parallel
/// tasks (decision of 2026-09-28).
#[derive(Clone, Debug)]
pub struct Limits {
    /// `sb wait` never blocks longer (the bash tool hands a command off
    /// to the background after BEND_BG_AFTER seconds).
    pub wait_cap_s: u64,
    pub max_crashes: u32,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            wait_cap_s: 25,
            max_crashes: 5,
        }
    }
}

pub struct Hub {
    pub st: State,
    pub workspace: String,
    pub limits: Limits,
    clients: BTreeMap<ClientId, ClientView>,
    waiters: Vec<Waiter>,
    /// Messages written to the steering file in the current turn.
    steered: BTreeMap<String, Vec<u64>>,
    /// (received, injected) steering entries of the current turn: an
    /// entry received while the final answer was being written is
    /// committed to history without the model ever reading it (ADR 0005
    /// finish_turn).
    steer_counts: BTreeMap<String, (usize, usize)>,
    last_assistant: BTreeMap<String, String>,
    /// (time, text) of recent assistant messages, for direct-exchange
    /// excerpts.
    recent: BTreeMap<String, Vec<(u64, String)>>,
    crashes: BTreeMap<String, u32>,
    confirms: BTreeMap<u64, (ClientId, Pending)>,
    next_confirm: u64,
    contexts: BTreeMap<String, String>,
    dirty: bool,
}

type Fx = Vec<Effect>;

fn ok(v: Value) -> Value {
    let mut v = v;
    v["ok"] = json!(true);
    v
}

fn err(e: impl Into<String>) -> Value {
    json!({"ok": false, "error": e.into()})
}

fn line(agent: &str, kind: &str, text: &str) -> Effect {
    Effect::Line {
        agent: agent.to_string(),
        line: format!("sb {} : {}", kind, wire_escape(text)),
    }
}

impl Hub {
    pub fn new(workspace: &str) -> Hub {
        Hub {
            st: State::new(workspace),
            workspace: workspace.to_string(),
            limits: Limits::default(),
            clients: BTreeMap::new(),
            waiters: Vec::new(),
            steered: BTreeMap::new(),
            steer_counts: BTreeMap::new(),
            last_assistant: BTreeMap::new(),
            recent: BTreeMap::new(),
            crashes: BTreeMap::new(),
            confirms: BTreeMap::new(),
            next_confirm: 1,
            contexts: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Rebuild the durable state from the journal.
    pub fn replay(&mut self, events: &[Event]) {
        for ev in events {
            self.st.apply(ev);
        }
    }

    fn emit(&mut self, fx: &mut Fx, ev: Event) {
        self.st.apply(&ev);
        fx.push(Effect::Journal(ev));
        self.dirty = true;
    }

    fn agent(&self, name: &str) -> Option<&Agent> {
        self.st.resolve(name).and_then(|n| self.st.agents.get(&n))
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
                    "queued": board::queued_count(&self.st, &a.name),
                    "turn_ms": a.turn_started_ms.map(|t| now.saturating_sub(t)),
                })
            })
            .collect();
        let cards: Vec<Value> = self
            .st
            .open_cards()
            .map(|c| json!({"id": c.id, "kind": c.kind, "agent": c.agent, "text": c.text, "for_msg": c.for_msg}))
            .collect();
        json!({"ev": "state", "agents": agents, "cards": cards})
    }

    pub fn handle(&mut self, input: Input, env: &mut dyn Env) -> Fx {
        let mut fx = Fx::new();
        match input {
            Input::Boot => self.boot(&mut fx),
            Input::ReplReady { agent } => self.repl_ready(&mut fx, env, &agent),
            Input::ReplLine { agent, line } => self.repl_line(&mut fx, env, &agent, &line),
            Input::ReplIdle { agent, leftover } => self.repl_idle(&mut fx, env, &agent, leftover),
            Input::ReplExited {
                agent,
                crashed,
                reason,
            } => self.repl_exited(&mut fx, env, &agent, crashed, &reason),
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
            Input::ClientInterrupt { client, agent } => {
                match self.agent(&agent).map(|a| (a.name.clone(), a.run)) {
                    Some((name, Run::Busy)) => fx.push(Effect::Interrupt { agent: name }),
                    _ => fx.push(self.notice(client, "aucun tour en cours à interrompre")),
                }
            }
            Input::Agent { token, from, req } => self.agent_req(&mut fx, env, token, &from, req),
            Input::Tick => self.tick(&mut fx, env),
        }
        self.refresh_contexts(&mut fx, env.now());
        if self.dirty {
            fx.push(Effect::State);
            self.dirty = false;
        }
        fx
    }

    fn notice(&self, client: ClientId, text: &str) -> Effect {
        Effect::ToClient {
            client,
            body: json!({"ev": "notice", "text": text}),
        }
    }

    // ---- lifecycle of the REPLs ----

    fn boot(&mut self, fx: &mut Fx) {
        let live: Vec<String> = self
            .st
            .order
            .iter()
            .filter(|n| {
                self.st
                    .agents
                    .get(*n)
                    .is_some_and(|a| a.lifecycle == Lifecycle::Active)
            })
            .cloned()
            .collect();
        for name in live {
            self.st.agents.get_mut(&name).unwrap().run = Run::Starting;
            fx.push(Effect::Spawn {
                agent: name,
                resume: true,
                crash_note: None,
            });
        }
        self.dirty = true;
    }

    fn spawn(&mut self, fx: &mut Fx, name: &str, resume: bool) {
        if let Some(a) = self.st.agents.get_mut(name) {
            if matches!(a.run, Run::Down) {
                a.run = Run::Starting;
                fx.push(Effect::Spawn {
                    agent: name.to_string(),
                    resume,
                    crash_note: None,
                });
                self.dirty = true;
            }
        }
    }

    fn repl_ready(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str) {
        if let Some(a) = self.st.agents.get_mut(agent) {
            a.run = Run::Idle;
            a.waiting = false;
            self.dirty = true;
        }
        self.pump(fx, env, agent);
    }

    fn repl_exited(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        agent: &str,
        crashed: bool,
        reason: &str,
    ) {
        self.fail_waiters(fx, agent, "interrupted_by_restart");
        self.steered.remove(agent);
        let Some(a) = self.st.agents.get_mut(agent) else {
            return;
        };
        a.run = Run::Down;
        a.turn_started_ms = None;
        self.dirty = true;
        if !crashed || a.lifecycle != Lifecycle::Active {
            return;
        }
        let n = self.crashes.entry(agent.to_string()).or_insert(0);
        *n += 1;
        if *n <= self.limits.max_crashes {
            let n = *n;
            a.run = Run::Starting;
            fx.push(Effect::Spawn {
                agent: agent.to_string(),
                resume: true,
                crash_note: Some(reason.to_string()),
            });
            fx.push(line(
                MAIN,
                "warn",
                &format!(
                    "la session de @{} a planté ({}) — redémarrage {}/{}",
                    agent, reason, n, self.limits.max_crashes
                ),
            ));
            if agent != MAIN {
                let text = format!(
                    "Task @{} crashed ({}). Its session restarts on its saved checkpoint (attempt {}/{}); the turn it was running is interrupted and it may need a new message to continue.",
                    agent, reason, n, self.limits.max_crashes
                );
                self.notify_main(fx, env, &text);
            }
        } else {
            let reason = format!("la REPL a planté {} fois : {}", n, reason);
            self.emit(
                fx,
                Event::Lifecycle {
                    name: agent.to_string(),
                    lifecycle: Lifecycle::Failed,
                    reason: Some(reason.clone()),
                },
            );
            self.open_card(fx, env, "failed", agent, &reason, None);
            if agent != MAIN {
                let text = format!(
                    "Task @{} failed: {}. It is not restarted anymore. A new message from you or the user restarts it; otherwise tell the user.",
                    agent, reason
                );
                self.notify_main(fx, env, &text);
            }
        }
    }

    /// A notification of the hub to main: it wakes main (or steers its
    /// running turn) like any message.
    fn notify_main(&mut self, fx: &mut Fx, env: &mut dyn Env, text: &str) {
        let _ = self.send(fx, env, HUB, MAIN, text, false, None, false, false);
    }

    fn repl_line(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, raw: &str) {
        let now = env.now();
        match wire::parse(raw) {
            Wire::TurnStarted => {
                let declared = self.st.agents.get(agent).and_then(|a| a.declared.clone());
                if let Some(a) = self.st.agents.get_mut(agent) {
                    a.run = Run::Busy;
                    a.turn_started_ms = Some(now);
                    self.dirty = true;
                }
                if matches!(declared, Some((Declared::Done | Declared::Blocked, _))) {
                    self.emit(
                        fx,
                        Event::Declared {
                            name: agent.to_string(),
                            status: None,
                            note: String::new(),
                        },
                    );
                    self.close_cards(
                        fx,
                        |c| c.agent == agent && (c.kind == "blocked" || c.kind == "done"),
                        "reprise",
                    );
                }
            }
            Wire::SteeringReceived => {
                self.steer_counts.entry(agent.to_string()).or_default().0 += 1
            }
            Wire::Steered => self.steer_counts.entry(agent.to_string()).or_default().1 += 1,
            Wire::Assistant(t) if !t.is_empty() => {
                if let Some(a) = self.st.agents.get_mut(agent) {
                    a.activity = Some((now, format!("wrote: {}", clip(&one_line(&t), 160))));
                }
                self.last_assistant.insert(agent.to_string(), t.clone());
                let r = self.recent.entry(agent.to_string()).or_default();
                r.push((now, t));
                if r.len() > 20 {
                    r.remove(0);
                }
            }
            Wire::Tool { name, args } if name != "apply_patch" => {
                if let Some(a) = self.st.agents.get_mut(agent) {
                    a.activity = Some((now, format!("{} `{}`", name, clip(&one_line(&args), 120))));
                    self.dirty = true;
                }
            }
            Wire::Tool { name, args } => {
                let _ = name;
                let files = wire::patch_files(&args);
                if let Some(a) = self.st.agents.get_mut(agent) {
                    a.activity = Some((now, format!("apply_patch {}", files.join(", "))));
                    self.dirty = true;
                }
                let shared = self
                    .st
                    .agents
                    .get(agent)
                    .is_some_and(|a| !a.is_main && a.ws.mode == Mode::Shared);
                if shared {
                    for path in wire::patch_files(&args) {
                        self.touch(fx, env, agent, &path);
                    }
                }
            }
            Wire::Idle => {
                // the daemon turns "--- idle" into ReplIdle (it reads the
                // steering file first)
            }
            _ => {}
        }
    }

    /// RFC 0001 §10.3: a file changed by two live shared tasks.
    fn touch(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, path: &str) {
        let known = self
            .st
            .agents
            .get(agent)
            .is_some_and(|a| a.files.contains(path));
        if known {
            return;
        }
        self.emit(
            fx,
            Event::FileTouched {
                name: agent.to_string(),
                path: path.to_string(),
            },
        );
        let others: Vec<String> = self
            .st
            .tasks()
            .filter(|a| {
                a.name != agent && a.ws.mode == Mode::Shared && a.lifecycle == Lifecycle::Active
            })
            .filter(|a| a.files.contains(path))
            .map(|a| a.name.clone())
            .collect();
        if !others.is_empty() {
            let text = format!(
                "{} est modifié par @{} et par @{}",
                path,
                agent,
                others.join(", @")
            );
            self.open_card(fx, env, "overlap", agent, &text, None);
            self.note_main(fx, env, &format!("file overlap: {}", text));
        }
    }

    fn repl_idle(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, leftover: bool) {
        let now = env.now();
        self.fail_waiters(fx, agent, "turn_ended");
        let steered = self.steered.remove(agent).unwrap_or_default();
        let (received, injected) = self.steer_counts.remove(agent).unwrap_or_default();
        // steering committed after the last model call: the model never
        // read it; a new turn makes it answer
        let unread = !leftover && received > injected && !steered.is_empty();
        if leftover {
            // the runtime never read them: they go again, as a new turn
            for id in &steered {
                self.emit(
                    fx,
                    Event::MessageState {
                        id: *id,
                        state: MsgState::Queued {
                            reason: "steer_leftover".into(),
                        },
                    },
                );
            }
        }
        let Some(a) = self.st.agents.get_mut(agent) else {
            return;
        };
        a.run = Run::Idle;
        a.waiting = false;
        a.turn_started_ms = None;
        let is_main = a.is_main;
        self.dirty = true;
        let text = self.last_assistant.remove(agent).unwrap_or_default();
        // RFC 0003 §7: a question gets an answer, the end of the turn
        let unanswered: Vec<Msg> = self
            .st
            .unanswered_for(agent)
            .into_iter()
            .filter(|m| !((leftover || unread) && steered.contains(&m.id)))
            .cloned()
            .collect();
        for m in unanswered {
            let body = if text.trim().is_empty() {
                "(pas de réponse écrite à la fin du tour)".to_string()
            } else {
                text.clone()
            };
            let _ = self.send(
                fx,
                env,
                agent,
                &m.from,
                &body,
                false,
                Some(m.id),
                false,
                true,
            );
        }
        // RFC 0001 §7.2: the automatic report (board only)
        if !is_main && !text.trim().is_empty() {
            self.emit(
                fx,
                Event::Reported {
                    name: agent.to_string(),
                    report: Report {
                        at_ms: now,
                        kind: "turn".into(),
                        summary: clip(&text, 1000),
                        decisions: Vec::new(),
                        auto: true,
                    },
                },
            );
        }
        if unread && self.st.queued_for(agent).is_empty() {
            if let Some(a) = self.st.agents.get_mut(agent) {
                a.run = Run::Busy;
                a.turn_started_ms = Some(now);
            }
            fx.push(Effect::Say {
                agent: agent.to_string(),
                text: "[switchboard] The message(s) just above arrived while you were writing your last answer: you have not answered them yet. Answer them now.".to_string(),
            });
        }
        self.pump(fx, env, agent);
    }

    // ---- messages (RFC 0003) ----

    /// Send a message (steer mode); answers it, or why it was refused.
    #[allow(clippy::too_many_arguments)]
    fn send(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        from: &str,
        to: &str,
        text: &str,
        expect_reply: bool,
        reply_to: Option<u64>,
        plain: bool,
        auto: bool,
    ) -> Result<Msg, String> {
        self.send_mode(
            fx,
            env,
            from,
            to,
            text,
            expect_reply,
            reply_to,
            plain,
            auto,
            false,
        )
    }

    /// Send a message; `queued`: never steered into a running turn nor
    /// handed to a `sb wait`, delivered only as a new turn (RFC 0003 §6.1).
    #[allow(clippy::too_many_arguments)]
    fn send_mode(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        from: &str,
        to: &str,
        text: &str,
        expect_reply: bool,
        reply_to: Option<u64>,
        plain: bool,
        auto: bool,
        queued: bool,
    ) -> Result<Msg, String> {
        let to = if to == USER {
            return Err(
                "invalid_recipient: pour parler à l'utilisateur, passe par main (sb card)".into(),
            );
        } else {
            self.st
                .resolve(to)
                .ok_or_else(|| format!("recipient_unknown: aucun agent nommé {}", to))?
        };
        if to == from {
            return Err("invalid_recipient: un agent ne s'écrit pas à lui-même".into());
        }
        if text.trim().is_empty() {
            return Err("message vide".into());
        }
        let (status, lifecycle, dropped_wt, parent) = {
            let a = &self.st.agents[&to];
            (
                a.status(),
                a.lifecycle.clone(),
                a.ws.mode == Mode::Worktree && a.ws.dropped,
                a.parent.clone(),
            )
        };
        let may_revive = from == USER || parent.as_deref() == Some(from);
        if lifecycle != Lifecycle::Active {
            let revivable = match lifecycle {
                Lifecycle::Failed => may_revive,
                Lifecycle::Stopped | Lifecycle::Archived => from == USER && !dropped_wt,
                Lifecycle::Active => true,
            };
            if !revivable {
                let hint = if dropped_wt {
                    " (son worktree a été supprimé : /restore)"
                } else {
                    ""
                };
                return Err(format!(
                    "recipient_unavailable: @{} est {}{}",
                    to,
                    status.as_str(),
                    hint
                ));
            }
            self.emit(
                fx,
                Event::Lifecycle {
                    name: to.clone(),
                    lifecycle: Lifecycle::Active,
                    reason: None,
                },
            );
            self.crashes.remove(&to);
            self.spawn(fx, &to, true);
        }
        let thread = match reply_to {
            Some(r) => match self.st.msgs.get(&r) {
                Some(orig) => orig.thread,
                None => return Err(format!("reply_to inconnu : m_{}", r)),
            },
            None => self.st.next_msg,
        };
        let msg = Msg {
            id: self.st.next_msg,
            thread,
            from: from.to_string(),
            to: to.clone(),
            reply_to,
            expect_reply,
            auto,
            text: text.to_string(),
            created_ms: env.now(),
            plain,
            queued,
        };
        self.emit(fx, Event::MessageSent { msg: msg.clone() });
        // a waiting recipient gets it as the result of its `sb wait`
        if let Some(i) = self.waiters.iter().position(|w| {
            !queued && w.agent == to && (Some(w.msg) == reply_to || (expect_reply && !plain))
        }) {
            let w = self.waiters.remove(i);
            let kind = if Some(w.msg) == reply_to {
                "reply"
            } else {
                "incoming_request"
            };
            self.deliver_to_waiter(fx, w, &msg, kind);
            return Ok(msg);
        }
        let _ = status;
        self.pump(fx, env, &to);
        Ok(msg)
    }

    fn deliver_to_waiter(&mut self, fx: &mut Fx, w: Waiter, msg: &Msg, kind: &str) {
        self.emit(
            fx,
            Event::MessageState {
                id: msg.id,
                state: MsgState::Delivered,
            },
        );
        fx.push(Effect::Reply {
            token: w.token,
            body: ok(json!({
                "type": kind,
                "from": msg.from,
                "message_id": format!("m_{}", msg.id),
                "thread": format!("t_{}", msg.thread),
                "expects_reply": msg.expect_reply,
                "auto": msg.auto,
                "message": msg.text,
            })),
        });
        fx.push(line(
            &w.agent,
            "msg-in",
            &format!("{} m_{} : {}", msg.from, msg.id, msg.text),
        ));
        let still = self.waiters.iter().any(|x| x.agent == w.agent);
        if let Some(a) = self.st.agents.get_mut(&w.agent) {
            a.waiting = still;
        }
        self.dirty = true;
    }

    fn fail_waiters(&mut self, fx: &mut Fx, agent: &str, why: &str) {
        let (gone, keep): (Vec<Waiter>, Vec<Waiter>) =
            self.waiters.drain(..).partition(|w| w.agent == agent);
        self.waiters = keep;
        for w in gone {
            fx.push(Effect::Reply {
                token: w.token,
                body: err(why),
            });
        }
        if let Some(a) = self.st.agents.get_mut(agent) {
            a.waiting = false;
        }
    }

    /// Deliver what is queued for `name`, when its state allows it
    /// (RFC 0003 §6.1, RFC 0001 §10.1).
    fn pump(&mut self, fx: &mut Fx, env: &mut dyn Env, name: &str) {
        let now = env.now();
        let Some(a) = self.st.agents.get(name) else {
            return;
        };
        if a.lifecycle != Lifecycle::Active {
            return;
        }
        let run = a.run;
        let is_main = a.is_main;
        let queued: Vec<Msg> = self.st.queued_for(name).into_iter().cloned().collect();
        let notes = if is_main {
            self.st.main_notes.clone()
        } else {
            Vec::new()
        };
        if queued.is_empty() {
            return;
        }
        let batch: Vec<Msg> = match run {
            Run::Down | Run::Starting => return,
            // a queued-mode message waits for the end of the turn
            Run::Busy => queued.into_iter().filter(|m| !m.queued).collect(),
            Run::Idle => queued,
        };
        if batch.is_empty() {
            return;
        }
        let mut parts: Vec<String> = Vec::new();
        if is_main && !notes.is_empty() {
            let mut s = String::from("<switchboard_notes>\n");
            for n in &notes {
                s.push_str(&format!("- {}\n", n));
            }
            s.push_str("</switchboard_notes>");
            parts.push(s);
            self.emit(fx, Event::MainNotesFlushed);
        }
        if is_main && batch.iter().any(|m| m.from == USER) {
            let status = board::status_block(&self.st, now);
            if !status.is_empty() {
                parts.push(status);
            }
        }
        for m in &batch {
            let (fp, tp) = (
                self.st.agents.get(&m.from).and_then(|a| a.parent.clone()),
                self.st.agents.get(&m.to).and_then(|a| a.parent.clone()),
            );
            let rel = prompts::relation(&m.from, &m.to, fp.as_deref(), tp.as_deref());
            parts.push(prompts::tagged(m, rel));
            self.emit(
                fx,
                Event::MessageState {
                    id: m.id,
                    state: MsgState::Delivered,
                },
            );
            if m.plain {
                fx.push(line(name, "you", &m.text));
            } else {
                fx.push(line(
                    name,
                    "msg-in",
                    &format!("{} m_{} : {}", m.from, m.id, m.text),
                ));
            }
        }
        let text = parts.join("\n\n");
        match run {
            Run::Busy => {
                self.steered
                    .entry(name.to_string())
                    .or_default()
                    .extend(batch.iter().map(|m| m.id));
                fx.push(Effect::Steer {
                    agent: name.to_string(),
                    text,
                });
            }
            _ => {
                // the turn starts now: nothing else may `say` meanwhile
                if let Some(a) = self.st.agents.get_mut(name) {
                    a.run = Run::Busy;
                    a.turn_started_ms = Some(now);
                }
                fx.push(Effect::Say {
                    agent: name.to_string(),
                    text,
                });
            }
        }
        self.dirty = true;
    }

    fn note_main(&mut self, fx: &mut Fx, env: &mut dyn Env, text: &str) {
        self.emit(
            fx,
            Event::MainNote {
                text: text.to_string(),
                at_ms: env.now(),
            },
        );
    }

    fn open_card(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        kind: &str,
        agent: &str,
        text: &str,
        for_msg: Option<u64>,
    ) -> u64 {
        let id = self.st.next_card;
        self.emit(
            fx,
            Event::CardOpened {
                card: Card {
                    id,
                    kind: kind.to_string(),
                    agent: agent.to_string(),
                    text: text.to_string(),
                    for_msg,
                    created_ms: env.now(),
                },
            },
        );
        fx.push(line(
            MAIN,
            "card",
            &format!("#{} {} @{} : {}", id, kind, agent, text),
        ));
        id
    }

    fn close_cards(&mut self, fx: &mut Fx, pred: impl Fn(&Card) -> bool, resolution: &str) {
        let ids: Vec<u64> = self
            .st
            .open_cards()
            .filter(|c| pred(c))
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.emit(
                fx,
                Event::CardClosed {
                    id,
                    resolution: resolution.to_string(),
                },
            );
            fx.push(line(
                MAIN,
                "card-closed",
                &format!("#{} {}", id, resolution),
            ));
        }
    }

    // ---- tasks ----

    fn unique_name(&self, base: &str) -> String {
        if !self.st.name_taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| {
                let suffix = format!("-{}", i);
                let keep = 24usize.saturating_sub(suffix.len());
                format!(
                    "{}{}",
                    base[..base.len().min(keep)].trim_end_matches('-'),
                    suffix
                )
            })
            .find(|n| !self.st.name_taken(n))
            .unwrap()
    }

    /// RFC 0001 §7.1 and RFC 0002 §3-4. Answers the task name.
    #[allow(clippy::too_many_arguments)]
    fn create_task(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        parent: &str,
        name: Option<&str>,
        brief: Brief,
        worktree: bool,
        with_changes: bool,
    ) -> Result<String, String> {
        if brief.objective.trim().is_empty() {
            return Err("objectif vide".into());
        }
        let base = match name {
            Some(n) if !n.is_empty() => {
                if !router::valid_name(n) {
                    return Err(format!(
                        "nom invalide : {} ([a-z0-9-], 24 caractères max)",
                        n
                    ));
                }
                n.to_string()
            }
            _ => router::slug(&brief.objective),
        };
        let name = self.unique_name(&base);
        let ws = if worktree {
            if !env.is_git() {
                return Err(
                    "le workspace n'est pas un dépôt git : pas de worktree possible".into(),
                );
            }
            env.worktree_create(&name, with_changes)?
        } else {
            Workspace {
                mode: Mode::Shared,
                path: self.workspace.clone(),
                branch: None,
                base_commit: None,
                dropped: false,
            }
        };
        let label = match &ws.branch {
            Some(b) => format!(" (worktree {})", b),
            None => String::new(),
        };
        self.emit(
            fx,
            Event::TaskCreated {
                name: name.clone(),
                parent: parent.to_string(),
                brief: brief.clone(),
                ws,
                at_ms: env.now(),
            },
        );
        let who = if parent == USER { "toi" } else { parent };
        fx.push(line(
            MAIN,
            "spawn",
            &format!(
                "{} → nouvelle tâche @{}{} : {}",
                who,
                name,
                label,
                one_line(&brief.objective)
            ),
        ));
        if parent == USER {
            self.note_main(
                fx,
                env,
                &format!(
                    "the user created the task @{}{}: {}",
                    name,
                    label,
                    one_line(&brief.objective)
                ),
            );
        }
        self.spawn(fx, &name, false);
        let text = prompts::brief_text(&name, &brief);
        let expect = parent == MAIN;
        self.send(
            fx,
            env,
            parent,
            &name,
            &text,
            expect,
            None,
            parent == USER,
            false,
        )?;
        Ok(name)
    }

    fn stop_task(&mut self, fx: &mut Fx, name: &str, lifecycle: Lifecycle, reason: &str) {
        fx.push(Effect::Kill {
            agent: name.to_string(),
        });
        self.fail_waiters(fx, name, "stopped");
        self.steered.remove(name);
        if let Some(a) = self.st.agents.get_mut(name) {
            a.run = Run::Down;
            a.turn_started_ms = None;
        }
        self.emit(
            fx,
            Event::Lifecycle {
                name: name.to_string(),
                lifecycle,
                reason: Some(reason.to_string()).filter(|r| !r.is_empty()),
            },
        );
        let queued: Vec<u64> = self.st.queued_for(name).iter().map(|m| m.id).collect();
        for id in queued {
            self.emit(
                fx,
                Event::MessageState {
                    id,
                    state: MsgState::Rejected {
                        error: "recipient_unavailable".into(),
                    },
                },
            );
        }
        self.close_cards(fx, |c| c.agent == name, "tâche arrêtée");
    }

    /// RFC 0001 §9.4 + RFC 0002 §5. `Err` carries a reason; `Ok(None)`:
    /// done; `Ok(Some(text))`: needs a confirmation first.
    fn drop_task(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        name: &str,
        force: bool,
    ) -> Result<Option<String>, String> {
        let a = self
            .agent(name)
            .ok_or_else(|| format!("aucune tâche nommée {}", name))?
            .clone();
        if a.is_main {
            return Err("main ne se droppe pas".into());
        }
        if a.lifecycle == Lifecycle::Archived {
            return Err(format!("@{} est déjà archivée", a.name));
        }
        let has_wt = a.ws.mode == Mode::Worktree && !a.ws.dropped;
        let loss = if has_wt {
            env.worktree_loss(&a.ws)
        } else {
            Loss::default()
        };
        let busy = a.run == Run::Busy;
        if !force && (busy || loss.any()) {
            let mut parts: Vec<String> = Vec::new();
            if busy {
                parts.push("la tâche est en cours".into());
            }
            if loss.any() {
                parts.push(format!(
                    "{} fichier{} modifié{} et {} commit{} non poussé{} seront sauvegardés (/restore)",
                    loss.dirty,
                    if loss.dirty > 1 { "s" } else { "" },
                    if loss.dirty > 1 { "s" } else { "" },
                    loss.unpushed,
                    if loss.unpushed > 1 { "s" } else { "" },
                    if loss.unpushed > 1 { "s" } else { "" },
                ));
            }
            return Ok(Some(format!(
                "Drop @{} ? {}. [y/N]",
                a.name,
                parts.join(" ; ")
            )));
        }
        self.stop_task(fx, &a.name, Lifecycle::Archived, "drop");
        let mut label = String::new();
        if has_wt {
            match env.worktree_drop(&a.name, &a.ws, &loss) {
                Ok(snap) => {
                    let mut ws = a.ws.clone();
                    ws.dropped = true;
                    self.emit(
                        fx,
                        Event::WorkspaceChanged {
                            name: a.name.clone(),
                            ws,
                        },
                    );
                    if snap.is_some() {
                        label = " — travail sauvegardé (/restore)".into();
                    } else {
                        label = " — worktree supprimé".into();
                    }
                    self.emit(
                        fx,
                        Event::Snapshot {
                            name: a.name.clone(),
                            snapshot_ref: snap,
                        },
                    );
                }
                Err(e) => label = format!(" — worktree NON supprimé : {}", e),
            }
        }
        fx.push(line(
            MAIN,
            "info",
            &format!("@{} archivée{}", a.name, label),
        ));
        // a client looking at it goes back to main
        let watchers: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, v)| v.focus == a.name)
            .map(|(c, _)| *c)
            .collect();
        for c in watchers {
            fx.push(Effect::ToClient {
                client: c,
                body: json!({"ev": "focus", "focus": MAIN}),
            });
        }
        Ok(None)
    }

    fn restore_task(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        name: &str,
    ) -> Result<String, String> {
        let a = self
            .agent(name)
            .ok_or_else(|| format!("aucune tâche nommée {}", name))?
            .clone();
        if a.lifecycle == Lifecycle::Active {
            return Err(format!("@{} est active", a.name));
        }
        if a.ws.mode == Mode::Worktree && a.ws.dropped {
            let ws = env.worktree_restore(&a.name, &a.ws, a.snapshot_ref.as_deref())?;
            self.emit(
                fx,
                Event::WorkspaceChanged {
                    name: a.name.clone(),
                    ws,
                },
            );
            self.emit(
                fx,
                Event::Snapshot {
                    name: a.name.clone(),
                    snapshot_ref: None,
                },
            );
        }
        self.emit(
            fx,
            Event::Lifecycle {
                name: a.name.clone(),
                lifecycle: Lifecycle::Active,
                reason: None,
            },
        );
        self.crashes.remove(&a.name);
        self.spawn(fx, &a.name, true);
        fx.push(line(MAIN, "info", &format!("@{} restaurée", a.name)));
        Ok(a.name)
    }

    fn isolate_task(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        name: &str,
    ) -> Result<String, String> {
        let a = self
            .agent(name)
            .ok_or_else(|| format!("aucune tâche nommée {}", name))?
            .clone();
        if a.is_main || a.ws.mode == Mode::Worktree {
            return Err(format!("@{} a déjà son propre dossier", a.name));
        }
        if !a.files.is_empty() {
            return Err(format!(
                "@{} a déjà modifié des fichiers du workspace ({}) : impossible de séparer son travail du tien",
                a.name,
                a.files.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        if a.run == Run::Busy {
            return Err(format!("@{} est en plein tour : attends la fin", a.name));
        }
        if !env.is_git() {
            return Err("le workspace n'est pas un dépôt git".into());
        }
        let ws = env.worktree_create(&a.name, false)?;
        let path = ws.path.clone();
        let branch = ws.branch.clone().unwrap_or_default();
        self.emit(
            fx,
            Event::WorkspaceChanged {
                name: a.name.clone(),
                ws,
            },
        );
        // the REPL takes its new BEND_WORKDIR at the next spawn
        fx.push(Effect::Kill {
            agent: a.name.clone(),
        });
        if let Some(x) = self.st.agents.get_mut(&a.name) {
            x.run = Run::Down;
        }
        self.spawn(fx, &a.name, true);
        let text = format!(
            "[switchboard] Your working directory is now the isolated git worktree `{}` (branch `{}`). Work only there from now on.",
            path, branch
        );
        self.send(fx, env, USER, &a.name, &text, false, None, true, false)?;
        fx.push(line(
            MAIN,
            "info",
            &format!(
                "@{} travaille maintenant dans le worktree {}",
                a.name, branch
            ),
        ));
        Ok(a.name)
    }

    // ---- the user ----

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
            let quoted: Vec<String> = prev
                .sent
                .iter()
                .map(|m| format!("\"{}\"", one_line(m)))
                .collect();
            let note = format!(
                "The user talked directly to @{} ({} message{}): {}. Last reply of @{}: \"{}\"",
                prev.focus,
                prev.sent.len(),
                if prev.sent.len() > 1 { "s" } else { "" },
                quoted.join(", "),
                prev.focus,
                clip_tail(&one_line(&reply), 2000)
            );
            self.note_main(fx, env, &note);
            fx.push(line(
                MAIN,
                "direct",
                &format!(
                    "Tu as parlé à @{} ({} message{})",
                    prev.focus,
                    prev.sent.len(),
                    if prev.sent.len() > 1 { "s" } else { "" }
                ),
            ));
        }
        if focus != MAIN {
            self.close_cards(fx, |c| c.agent == focus && c.kind == "done", "vue");
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
        match router::parse(text, &focus) {
            UserCmd::Say(t) => {
                if t.is_empty() {
                    return;
                }
                self.user_says(fx, env, client, &focus, &t);
            }
            UserCmd::To { target, text } => {
                if target == MAIN {
                    self.user_says(fx, env, client, MAIN, &text);
                    return;
                }
                let Some(name) = self.st.resolve(&target) else {
                    let close: Vec<String> = self
                        .st
                        .tasks()
                        .filter(|a| {
                            a.name.starts_with(target.chars().next().unwrap_or('?'))
                                || a.name.contains(&target)
                        })
                        .map(|a| format!("@{}", a.name))
                        .collect();
                    let hint = if close.is_empty() {
                        String::new()
                    } else {
                        format!(" — tu voulais dire {} ?", close.join(", "))
                    };
                    fx.push(
                        self.notice(client, &format!("aucune tâche nommée @{}{}", target, hint)),
                    );
                    return;
                };
                match self.send(fx, env, USER, &name, &text, false, None, true, false) {
                    Ok(m) => {
                        fx.push(line(MAIN, "route", &format!("toi → @{} : {}", name, text)));
                        if focus != MAIN || name != focus {
                            self.note_main(
                                fx,
                                env,
                                &format!(
                                    "the user wrote directly to @{}: \"{}\"",
                                    name,
                                    one_line(&text)
                                ),
                            );
                        }
                        if let Some(v) = self.clients.get_mut(&client) {
                            v.last_route = Some(m.id);
                        }
                    }
                    Err(e) => fx.push(self.notice(client, &e)),
                }
            }
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
                if let Err(e) =
                    self.create_task(fx, env, USER, name.as_deref(), b, worktree, with_changes)
                {
                    fx.push(self.notice(client, &e));
                }
            }
            UserCmd::Drop { name, force } => {
                let Some(name) = name else {
                    fx.push(self.notice(
                        client,
                        "usage : /drop <tâche> (ou /drop depuis la vue de la tâche)",
                    ));
                    return;
                };
                match self.drop_task(fx, env, &name, force) {
                    Ok(None) => {
                        self.note_main(fx, env, &format!("the user dropped the task @{}", name))
                    }
                    Ok(Some(q)) => {
                        let id = self.next_confirm;
                        self.next_confirm += 1;
                        let canon = self.st.resolve(&name).unwrap_or(name);
                        self.confirms
                            .insert(id, (client, Pending::Drop { name: canon }));
                        fx.push(Effect::ToClient {
                            client,
                            body: json!({"ev": "confirm", "id": id, "text": q}),
                        });
                    }
                    Err(e) => fx.push(self.notice(client, &e)),
                }
            }
            UserCmd::Restore { name } => match self.restore_task(fx, env, &name) {
                Ok(n) => self.note_main(fx, env, &format!("the user restored the task @{}", n)),
                Err(e) => fx.push(self.notice(client, &e)),
            },
            UserCmd::Isolate { name } => match self.isolate_task(fx, env, &name) {
                Ok(n) => self.note_main(
                    fx,
                    env,
                    &format!("the user moved @{} into its own git worktree", n),
                ),
                Err(e) => fx.push(self.notice(client, &e)),
            },
            UserCmd::Rename { name, new_name } => {
                let Some(old) = self.st.resolve(&name).filter(|n| n != MAIN) else {
                    fx.push(self.notice(client, &format!("aucune tâche nommée @{}", name)));
                    return;
                };
                if !router::valid_name(&new_name) || self.st.name_taken(&new_name) {
                    fx.push(
                        self.notice(client, &format!("nom invalide ou déjà pris : {}", new_name)),
                    );
                    return;
                }
                self.emit(
                    fx,
                    Event::Renamed {
                        name: old.clone(),
                        new_name: new_name.clone(),
                    },
                );
                for m in [&mut self.steered] {
                    if let Some(v) = m.remove(&old) {
                        m.insert(new_name.clone(), v);
                    }
                }
                for w in self.waiters.iter_mut().filter(|w| w.agent == old) {
                    w.agent = new_name.clone();
                }
                for v in self.clients.values_mut().filter(|v| v.focus == old) {
                    v.focus = new_name.clone();
                }
                fx.push(Effect::Renamed {
                    old: old.clone(),
                    new: new_name.clone(),
                });
                fx.push(line(
                    MAIN,
                    "info",
                    &format!("@{} s'appelle maintenant @{}", old, new_name),
                ));
                self.note_main(
                    fx,
                    env,
                    &format!(
                        "the user renamed @{} to @{} (the old name still works)",
                        old, new_name
                    ),
                );
            }
            UserCmd::Answer { card, text } => self.answer_card(fx, env, client, card, &text),
            UserCmd::Cancel => {
                let last = self.clients.get(&client).and_then(|v| v.last_route);
                match last {
                    Some(id)
                        if matches!(self.st.msg_state.get(&id), Some(MsgState::Queued { .. })) =>
                    {
                        self.emit(
                            fx,
                            Event::MessageState {
                                id,
                                state: MsgState::Cancelled,
                            },
                        );
                        let to = self
                            .st
                            .msgs
                            .get(&id)
                            .map(|m| m.to.clone())
                            .unwrap_or_default();
                        fx.push(line(MAIN, "info", &format!("routage vers @{} annulé", to)));
                        self.note_main(
                            fx,
                            env,
                            &format!(
                                "the user cancelled the message m_{} to @{} before delivery",
                                id, to
                            ),
                        );
                    }
                    Some(_) => fx.push(self.notice(
                        client,
                        "trop tard : le message est déjà livré, envoie une correction",
                    )),
                    None => fx.push(self.notice(client, "aucun routage à annuler")),
                }
            }
            UserCmd::Tasks => {
                let now = env.now();
                let mut lines: Vec<String> =
                    self.st.tasks().map(|a| board::task_line(a, now)).collect();
                if lines.is_empty() {
                    lines.push("aucune tâche".into());
                }
                for c in self.st.open_cards() {
                    lines.push(format!(
                        "carte #{} {} @{} : {}",
                        c.id,
                        c.kind,
                        c.agent,
                        clip(&one_line(&c.text), 100)
                    ));
                }
                fx.push(Effect::ToClient {
                    client,
                    body: json!({"ev": "notice", "text": lines.join("\n")}),
                });
            }
            UserCmd::Interrupt => match self.st.agents.get(&focus).map(|a| a.run) {
                Some(Run::Busy) => fx.push(Effect::Interrupt {
                    agent: focus.clone(),
                }),
                _ => fx.push(self.notice(client, "aucun tour en cours à interrompre")),
            },
            UserCmd::Passthrough(l) => {
                let first = l.split_whitespace().next().unwrap_or("");
                if first != "/compact" {
                    fx.push(self.notice(
                        client,
                        &format!("commande inconnue : {} (voir /help)", first),
                    ));
                    return;
                }
                match self.st.agents.get_mut(&focus) {
                    Some(a) if a.run == Run::Idle => {
                        a.run = Run::Busy;
                        self.dirty = true;
                        fx.push(Effect::Passthrough {
                            agent: focus.clone(),
                            line: l,
                        });
                    }
                    _ => fx.push(self.notice(
                        client,
                        "l'agent n'est pas inactif : réessaie à la fin du tour",
                    )),
                }
            }
            UserCmd::Help => fx.push(Effect::ToClient {
                client,
                body: json!({"ev": "notice", "text": HELP}),
            }),
            UserCmd::Invalid(e) => fx.push(self.notice(client, &e)),
        }
    }

    /// Plain text from the user to the agent in focus.
    fn user_says(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        client: ClientId,
        to: &str,
        text: &str,
    ) {
        // RFC 0001 §7.3: answering a task that waits on the user, from
        // its own view, answers its question
        if to != MAIN {
            let card = self
                .st
                .open_cards()
                .find(|c| {
                    c.kind == "question"
                        && c.for_msg
                            .is_some_and(|m| self.st.msgs.get(&m).is_some_and(|x| x.from == to))
                })
                .map(|c| c.id);
            if let Some(id) = card {
                if let Some(v) = self.clients.get_mut(&client) {
                    v.sent.push(text.to_string());
                }
                self.answer_card(fx, env, client, id, text);
                return;
            }
        }
        match self.send(fx, env, USER, to, text, false, None, true, false) {
            Ok(_) => {
                if to != MAIN {
                    if let Some(v) = self.clients.get_mut(&client) {
                        v.sent.push(text.to_string());
                    }
                }
            }
            Err(e) => fx.push(self.notice(client, &e)),
        }
    }

    fn answer_card(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        client: ClientId,
        id: u64,
        text: &str,
    ) {
        let Some(card) = self.st.cards.get(&id).cloned() else {
            fx.push(self.notice(client, &format!("aucune carte ouverte #{}", id)));
            return;
        };
        let yes = matches!(
            text.trim().to_lowercase().as_str(),
            "y" | "yes" | "o" | "oui" | "ok"
        );
        let resolution = match card.kind.as_str() {
            "question" => {
                let target = card
                    .for_msg
                    .and_then(|m| self.st.msgs.get(&m).map(|x| (m, x.from.clone())));
                match target {
                    Some((m, asker)) => {
                        match self.send(fx, env, USER, &asker, text, false, Some(m), false, false) {
                            Ok(_) => {
                                self.note_main(
                                    fx,
                                    env,
                                    &format!(
                                        "the user answered card #{} (@{} asked: \"{}\"): \"{}\"",
                                        id,
                                        asker,
                                        one_line(&card.text),
                                        one_line(text)
                                    ),
                                );
                                fx.push(line(
                                    MAIN,
                                    "route",
                                    &format!(
                                        "toi → @{} (réponse à la carte #{}) : {}",
                                        asker, id, text
                                    ),
                                ));
                                "répondue".to_string()
                            }
                            Err(e) => {
                                fx.push(self.notice(client, &e));
                                return;
                            }
                        }
                    }
                    None => {
                        self.note_main(
                            fx,
                            env,
                            &format!(
                                "the user answered card #{} (\"{}\"): \"{}\"",
                                id,
                                one_line(&card.text),
                                one_line(text)
                            ),
                        );
                        "répondue".to_string()
                    }
                }
            }
            "drop" => {
                if yes {
                    match self.drop_task(fx, env, &card.agent, true) {
                        Ok(_) => self.note_main(
                            fx,
                            env,
                            &format!("the user accepted to drop @{}", card.agent),
                        ),
                        Err(e) => fx.push(self.notice(client, &e)),
                    }
                    "acceptée".to_string()
                } else {
                    self.note_main(
                        fx,
                        env,
                        &format!("the user refused to drop @{}", card.agent),
                    );
                    "refusée".to_string()
                }
            }
            "blocked" | "failed" | "restart" => {
                if let Err(e) =
                    self.send(fx, env, USER, &card.agent, text, false, None, true, false)
                {
                    fx.push(self.notice(client, &e));
                    return;
                }
                "répondue".to_string()
            }
            _ => "vue".to_string(),
        };
        self.emit(
            fx,
            Event::CardClosed {
                id,
                resolution: resolution.clone(),
            },
        );
        fx.push(line(
            MAIN,
            "card-closed",
            &format!("#{} {}", id, resolution),
        ));
    }

    fn confirm(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, id: u64, yes: bool) {
        let Some((_, pending)) = self.confirms.remove(&id) else {
            return;
        };
        match pending {
            Pending::Drop { name } => {
                if !yes {
                    fx.push(self.notice(client, &format!("drop de @{} annulé", name)));
                    return;
                }
                match self.drop_task(fx, env, &name, true) {
                    Ok(_) => {
                        self.note_main(fx, env, &format!("the user dropped the task @{}", name))
                    }
                    Err(e) => fx.push(self.notice(client, &e)),
                }
            }
        }
    }

    // ---- the agents' `sb` requests ----

    fn agent_req(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        token: Token,
        from: &str,
        req: AgentReq,
    ) {
        let Some(from) = self.st.resolve(from) else {
            fx.push(Effect::Reply {
                token,
                body: err(format!("agent inconnu : {}", from)),
            });
            return;
        };
        let is_main = from == MAIN;
        let main_only = |fx: &mut Fx| {
            fx.push(Effect::Reply {
                token,
                body: err("réservé à main (le parent des tâches)"),
            });
        };
        let reply = |fx: &mut Fx, body: Value| fx.push(Effect::Reply { token, body });
        match req {
            AgentReq::List => {
                let lines = board::roster(&self.st, &from, env.now());
                reply(fx, ok(json!({"text": lines.join("\n")})));
            }
            AgentReq::Tasks => {
                reply(
                    fx,
                    ok(json!({"text": board::tasks_detail(&self.st, env.now())})),
                );
            }
            AgentReq::Send {
                to,
                text,
                expect_reply,
                reply_to,
                queued,
            } => {
                // a plain message to an agent that asked us something
                // answers it
                let reply_to = reply_to.or_else(|| {
                    if expect_reply {
                        None
                    } else {
                        self.open_question(&to, &from)
                    }
                });
                let sent = self.send_mode(
                    fx,
                    env,
                    &from,
                    &to,
                    &text,
                    expect_reply,
                    reply_to,
                    false,
                    false,
                    queued,
                );
                match sent {
                    Ok(m) => {
                        let delivered =
                            matches!(self.st.msg_state.get(&m.id), Some(MsgState::Delivered));
                        reply(
                            fx,
                            ok(json!({
                                "message_id": format!("m_{}", m.id),
                                "thread": format!("t_{}", m.thread),
                                "to": m.to,
                                "delivery": if delivered { "delivered" } else { "queued" },
                            })),
                        );
                    }
                    Err(e) => reply(fx, err(e)),
                }
            }
            AgentReq::Wait { msg, timeout_s } => self.wait(fx, env, token, &from, msg, timeout_s),
            AgentReq::Ask {
                to,
                text,
                timeout_s,
            } => match self.send(fx, env, &from, &to, &text, true, None, false, false) {
                Ok(m) => self.wait(fx, env, token, &from, m.id, timeout_s),
                Err(e) => reply(fx, err(e)),
            },
            AgentReq::Status { status, note } => {
                self.emit(
                    fx,
                    Event::Declared {
                        name: from.clone(),
                        status: Some(status),
                        note: note.clone(),
                    },
                );
                if status == Declared::Blocked && !is_main {
                    self.open_card(
                        fx,
                        env,
                        "blocked",
                        &from,
                        if note.is_empty() { "bloquée" } else { &note },
                        None,
                    );
                }
                reply(fx, ok(json!({})));
            }
            AgentReq::Report {
                kind,
                summary,
                decisions,
            } => {
                if is_main {
                    reply(fx, err("main ne se rapporte à personne"));
                    return;
                }
                self.emit(
                    fx,
                    Event::Reported {
                        name: from.clone(),
                        report: Report {
                            at_ms: env.now(),
                            kind: kind.clone(),
                            summary: summary.clone(),
                            decisions: decisions.clone(),
                            auto: false,
                        },
                    },
                );
                match kind.as_str() {
                    "done" => {
                        self.emit(
                            fx,
                            Event::Declared {
                                name: from.clone(),
                                status: Some(Declared::Done),
                                note: String::new(),
                            },
                        );
                        self.open_card(
                            fx,
                            env,
                            "done",
                            &from,
                            &clip(&one_line(&summary), 200),
                            None,
                        );
                    }
                    "blocked" => {
                        self.emit(
                            fx,
                            Event::Declared {
                                name: from.clone(),
                                status: Some(Declared::Blocked),
                                note: summary.clone(),
                            },
                        );
                        self.open_card(fx, env, "blocked", &from, &summary, None);
                    }
                    "failed" => {
                        self.emit(
                            fx,
                            Event::Lifecycle {
                                name: from.clone(),
                                lifecycle: Lifecycle::Failed,
                                reason: Some(summary.clone()),
                            },
                        );
                        self.open_card(fx, env, "failed", &from, &summary, None);
                    }
                    _ => {}
                }
                let mut text = format!("[report: {}] {}", kind, summary.trim());
                if !decisions.is_empty() {
                    text.push_str("\nDecisions:");
                    for d in &decisions {
                        text.push_str(&format!("\n- {}", d));
                    }
                }
                let parent = self
                    .st
                    .agents
                    .get(&from)
                    .and_then(|a| a.parent.clone())
                    .unwrap_or_else(|| MAIN.into());
                let to = if parent == USER {
                    MAIN.to_string()
                } else {
                    parent
                };
                // the report answers the parent's open request, if any:
                // no automatic reply repeats it at the end of the turn
                let reply_to = self.open_question(&to, &from);
                let r = self.send(fx, env, &from, &to, &text, false, reply_to, false, false);
                match r {
                    Ok(m) => reply(fx, ok(json!({"message_id": format!("m_{}", m.id)}))),
                    Err(e) => reply(fx, err(e)),
                }
            }
            AgentReq::Spawn {
                name,
                brief,
                worktree,
                with_changes,
            } => {
                if !is_main {
                    main_only(fx);
                    return;
                }
                match self.create_task(
                    fx,
                    env,
                    MAIN,
                    Some(&name).filter(|n| !n.is_empty()).map(|s| s.as_str()),
                    brief,
                    worktree,
                    with_changes,
                ) {
                    Ok(n) => {
                        let a = &self.st.agents[&n];
                        reply(
                            fx,
                            ok(json!({"name": n, "path": a.ws.path, "branch": a.ws.branch})),
                        );
                    }
                    Err(e) => reply(fx, err(e)),
                }
            }
            AgentReq::Interrupt { agent } => {
                if !is_main {
                    main_only(fx);
                    return;
                }
                match self.agent(&agent).map(|a| (a.name.clone(), a.run)) {
                    Some((n, Run::Busy)) => {
                        fx.push(Effect::Interrupt { agent: n });
                        reply(fx, ok(json!({})));
                    }
                    Some(_) => reply(fx, err("pas de tour en cours")),
                    None => reply(fx, err(format!("aucune tâche nommée {}", agent))),
                }
            }
            AgentReq::Stop { agent, reason } => {
                if !is_main {
                    main_only(fx);
                    return;
                }
                match self.st.resolve(&agent).filter(|n| n != MAIN) {
                    Some(n) => {
                        self.stop_task(fx, &n, Lifecycle::Stopped, &reason);
                        fx.push(line(
                            MAIN,
                            "info",
                            &format!("main → @{} arrêtée : {}", n, reason),
                        ));
                        reply(fx, ok(json!({})));
                    }
                    None => reply(fx, err(format!("aucune tâche nommée {}", agent))),
                }
            }
            AgentReq::Drop { agent } => {
                if !is_main {
                    main_only(fx);
                    return;
                }
                match self.drop_task(fx, env, &agent, false) {
                    Ok(None) => reply(fx, ok(json!({"dropped": true}))),
                    Ok(Some(q)) => {
                        let n = self.st.resolve(&agent).unwrap_or(agent.clone());
                        let text = q.trim_end_matches(" [y/N]").to_string();
                        let id = self.open_card(
                            fx,
                            env,
                            "drop",
                            &n,
                            &format!("main propose : {} (réponds oui ou non)", text),
                            None,
                        );
                        reply(
                            fx,
                            ok(
                                json!({"dropped": false, "card": id, "reason": "the user must confirm: an attention card is open"}),
                            ),
                        );
                    }
                    Err(e) => reply(fx, err(e)),
                }
            }
            AgentReq::Card { text, for_msg } => {
                if !is_main {
                    main_only(fx);
                    return;
                }
                let agent = match for_msg {
                    Some(m) => match self.st.msgs.get(&m) {
                        Some(x) => x.from.clone(),
                        None => {
                            reply(fx, err(format!("message inconnu : m_{}", m)));
                            return;
                        }
                    },
                    None => MAIN.to_string(),
                };
                let id = self.open_card(fx, env, "question", &agent, &text, for_msg);
                if let Some(m) = for_msg {
                    // main hands the question over: no automatic reply
                    self.emit(fx, Event::MessageSettled { id: m });
                }
                reply(fx, ok(json!({"card": id})));
            }
        }
    }

    /// The oldest delivered message from `asker` to `me` that still
    /// expects `me`'s reply.
    fn open_question(&self, asker: &str, me: &str) -> Option<u64> {
        let asker = self.st.resolve(asker)?;
        self.st
            .unanswered_for(me)
            .into_iter()
            .find(|m| m.from == asker)
            .map(|m| m.id)
    }

    fn wait(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        token: Token,
        from: &str,
        msg: u64,
        timeout_s: u64,
    ) {
        let Some(orig) = self.st.msgs.get(&msg).cloned() else {
            fx.push(Effect::Reply {
                token,
                body: err(format!("message inconnu : m_{}", msg)),
            });
            return;
        };
        if orig.from != from {
            fx.push(Effect::Reply {
                token,
                body: err("on n'attend que la réponse à ses propres messages"),
            });
            return;
        }
        // the reply may already be there
        let existing = self
            .st
            .msgs
            .values()
            .find(|m| m.reply_to == Some(msg) && (m.from == orig.to || m.from == USER))
            .cloned();
        let w = Waiter {
            token,
            agent: from.to_string(),
            msg,
            deadline_ms: env.now() + 1000 * timeout_s.min(self.limits.wait_cap_s),
        };
        if let Some(r) = existing {
            self.deliver_to_waiter(fx, w, &r, "reply");
            return;
        }
        self.waiters.push(w);
        if let Some(a) = self.st.agents.get_mut(from) {
            a.waiting = true;
        }
        self.dirty = true;
    }

    fn tick(&mut self, fx: &mut Fx, env: &mut dyn Env) {
        let now = env.now();
        let (late, keep): (Vec<Waiter>, Vec<Waiter>) =
            self.waiters.drain(..).partition(|w| w.deadline_ms <= now);
        self.waiters = keep;
        for w in late {
            fx.push(Effect::Reply {
                token: w.token,
                body: json!({
                    "ok": false,
                    "error": "timeout",
                    "hint": format!("no reply to m_{} yet. It stays valid: end your turn (the reply will wake you up) or continue with an explicit assumption.", w.msg),
                }),
            });
            let still = self.waiters.iter().any(|x| x.agent == w.agent);
            if let Some(a) = self.st.agents.get_mut(&w.agent) {
                a.waiting = still;
            }
            self.dirty = true;
            let agent = w.agent.clone();
            self.pump(fx, env, &agent);
        }
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
}

pub const HELP: &str = "\
texte simple      message à l'agent affiché (main par défaut)
@tâche texte      message direct à une tâche, sans passer par main (@main depuis une tâche)
/new [-w] [nom:] objectif   créer une tâche (-w : worktree git isolé, --with-changes : avec tes modifs)
/drop [tâche] [--force]     arrêter et archiver une tâche (et supprimer son worktree)
/restore tâche    rouvrir une tâche archivée (et son worktree sauvegardé)
/isolate tâche    donner un worktree à une tâche qui n'a encore rien modifié
/rename a b       renommer une tâche (l'ancien nom reste valable)
/answer N texte   répondre à la carte d'attention N
/cancel           annuler le dernier routage s'il n'est pas encore livré
/tasks            le tableau des tâches
/interrupt        interrompre le tour de l'agent affiché
/compact          compacter la conversation de l'agent affiché";

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
