//! The hub's durable state (RFC 0001 §12): a projection of the journal
//! events. `State::apply` is the only way the durable state changes;
//! the runtime-only fields (`Agent::run`, `Agent::waiting`) are set by
//! the core from REPL activity and are never journaled.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The name of the orchestrator agent. Never a task name.
pub const MAIN: &str = "main";
/// The human, as a message sender. Never an agent name.
pub const USER: &str = "user";
/// The hub itself, as the sender of notifications (a task crashed...).
/// Never an agent name.
pub const HUB: &str = "switchboard";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Shared,
    Worktree,
}

/// Where a task works (RFC 0002). `path` is the workspace for `shared`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub mode: Mode,
    pub path: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub base_commit: Option<String>,
    /// The worktree was removed by a drop (restore can bring it back).
    #[serde(default)]
    pub dropped: bool,
}

/// The task brief (RFC 0001 §7.1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    pub objective: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub done_when: Option<String>,
    #[serde(default)]
    pub report_format: Option<String>,
}

/// What the owner of a task decided (never automatic, RFC 0001 §9.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Active,
    Stopped,
    Archived,
    Failed,
}

/// What the agent's REPL is doing right now (runtime only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// No REPL process (not spawned yet, or stopped).
    Down,
    /// Spawned, not connected yet.
    Starting,
    Idle,
    /// A turn is running.
    Busy,
}

/// The status an agent declares about itself (RFC 0003 `setStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Declared {
    Working,
    Done,
    Blocked,
}

/// The status every view shows (RFC 0001 §9.1, RFC 0003 `ListedAgent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Working,
    Waiting,
    Idle,
    Done,
    Blocked,
    Failed,
    Stopped,
    Archived,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Starting => "starting",
            Status::Working => "working",
            Status::Waiting => "waiting",
            Status::Idle => "idle",
            Status::Done => "done",
            Status::Blocked => "blocked",
            Status::Failed => "failed",
            Status::Stopped => "stopped",
            Status::Archived => "archived",
        }
    }

    /// Can any agent's message reach this agent (RFC 0003 §6.1)? A
    /// failed task is reachable: a new message restarts it (RFC 0001
    /// §9.2), but only from the user or its parent.
    pub fn reachable(self) -> bool {
        !matches!(self, Status::Stopped | Status::Archived)
    }
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub name: String,
    /// The name at creation: its directories never move on a rename.
    pub dir: String,
    pub is_main: bool,
    /// The agent that created it: `main`, or `user` for a `/new`.
    pub parent: Option<String>,
    pub brief: Brief,
    pub created_ms: u64,
    pub ws: Workspace,
    pub lifecycle: Lifecycle,
    pub failure: Option<String>,
    pub declared: Option<(Declared, String)>,
    /// The last report: explicit (`sb report`) or the end of a turn.
    pub last_report: Option<Report>,
    pub aliases: Vec<String>,
    /// Files this task changed in the shared workspace (RFC 0001 §10.3).
    pub files: BTreeSet<String>,
    /// A drop saved work here (RFC 0002 §5.2).
    pub snapshot_ref: Option<String>,
    // ---- runtime only ----
    pub run: Run,
    /// Inside `sb wait` (its bash call is blocked on the hub).
    pub waiting: bool,
    pub turn_started_ms: Option<u64>,
    /// The last thing it did: (time, "bash `cargo test`", "wrote: ...").
    pub activity: Option<(u64, String)>,
}

impl Agent {
    pub fn status(&self) -> Status {
        match self.lifecycle {
            Lifecycle::Archived => return Status::Archived,
            Lifecycle::Stopped => return Status::Stopped,
            Lifecycle::Failed => return Status::Failed,
            Lifecycle::Active => {}
        }
        match self.run {
            Run::Down | Run::Starting => Status::Starting,
            Run::Busy if self.waiting => Status::Waiting,
            Run::Busy => Status::Working,
            Run::Idle => match self.declared {
                Some((Declared::Done, _)) => Status::Done,
                Some((Declared::Blocked, _)) => Status::Blocked,
                _ => Status::Idle,
            },
        }
    }

    /// One line: what this agent is for.
    pub fn description(&self) -> String {
        if self.is_main {
            "orchestrator: routes the user's requests to the tasks".to_string()
        } else {
            crate::util::one_line(&self.brief.objective)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub at_ms: u64,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub decisions: Vec<String>,
    /// Built by the hub at the end of a turn, not sent by the agent.
    #[serde(default)]
    pub auto: bool,
}

/// A message between two parties of the group (RFC 0003). The sender or
/// the recipient may be `user`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Msg {
    pub id: u64,
    pub thread: u64,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub reply_to: Option<u64>,
    #[serde(default)]
    pub expect_reply: bool,
    #[serde(default)]
    pub auto: bool,
    pub text: String,
    pub created_ms: u64,
    /// A user message typed to the task (checkout or `@name`): delivered
    /// as a plain user message, without the agent_message tag.
    #[serde(default)]
    pub plain: bool,
    /// `sb send --mode queued`: never steered into a running turn,
    /// delivered only as a new turn (RFC 0003 §6.1).
    #[serde(default)]
    pub queued: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum MsgState {
    Queued { reason: String },
    Delivered,
    Rejected { error: String },
    Cancelled,
}

/// An attention card (RFC 0001 §11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    /// question | failed | blocked | overlap | done | restart |
    /// drop
    pub kind: String,
    pub agent: String,
    pub text: String,
    /// A question card: the user's answer replies to this message.
    #[serde(default)]
    pub for_msg: Option<u64>,
    pub created_ms: u64,
}

/// Every durable change, one JSON object per journal line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TaskCreated {
        name: String,
        parent: String,
        brief: Brief,
        ws: Workspace,
        at_ms: u64,
    },
    Lifecycle {
        name: String,
        lifecycle: Lifecycle,
        #[serde(default)]
        reason: Option<String>,
    },
    WorkspaceChanged {
        name: String,
        ws: Workspace,
    },
    Snapshot {
        name: String,
        #[serde(default)]
        snapshot_ref: Option<String>,
    },
    Declared {
        name: String,
        #[serde(default)]
        status: Option<Declared>,
        #[serde(default)]
        note: String,
    },
    Reported {
        name: String,
        report: Report,
    },
    FileTouched {
        name: String,
        path: String,
    },
    Renamed {
        name: String,
        new_name: String,
    },
    MessageSent {
        msg: Msg,
    },
    MessageState {
        id: u64,
        #[serde(flatten)]
        state: MsgState,
    },
    /// The message got its first reply, or will never get an automatic
    /// one (main escalated it to the user).
    MessageSettled {
        id: u64,
    },
    CardOpened {
        card: Card,
    },
    CardClosed {
        id: u64,
        #[serde(default)]
        resolution: String,
    },
    /// What main sees about the user's direct exchanges and routes.
    MainNote {
        text: String,
        at_ms: u64,
    },
    MainNotesFlushed,
}

/// The durable projection of the journal.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub agents: BTreeMap<String, Agent>,
    pub order: Vec<String>,
    pub msgs: BTreeMap<u64, Msg>,
    pub msg_state: BTreeMap<u64, MsgState>,
    pub settled: BTreeSet<u64>,
    pub cards: BTreeMap<u64, Card>,
    pub main_notes: Vec<String>,
    pub next_msg: u64,
    pub next_card: u64,
}

impl State {
    /// A state with only main, working in `workspace`.
    pub fn new(workspace: &str) -> State {
        let mut st = State {
            next_msg: 1,
            next_card: 1,
            ..State::default()
        };
        st.agents.insert(
            MAIN.to_string(),
            Agent {
                name: MAIN.to_string(),
                dir: MAIN.to_string(),
                is_main: true,
                parent: None,
                brief: Brief::default(),
                created_ms: 0,
                ws: Workspace {
                    mode: Mode::Shared,
                    path: workspace.to_string(),
                    branch: None,
                    base_commit: None,
                    dropped: false,
                },
                lifecycle: Lifecycle::Active,
                failure: None,
                declared: None,
                last_report: None,
                aliases: Vec::new(),
                files: BTreeSet::new(),
                snapshot_ref: None,
                run: Run::Down,
                waiting: false,
                turn_started_ms: None,
                activity: None,
            },
        );
        st.order.push(MAIN.to_string());
        st
    }

    /// The canonical name of an agent: itself, or the task an alias
    /// (an old name) points to.
    pub fn resolve(&self, name: &str) -> Option<String> {
        if self.agents.contains_key(name) {
            return Some(name.to_string());
        }
        self.agents
            .values()
            .find(|a| a.aliases.iter().any(|x| x == name))
            .map(|a| a.name.clone())
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Agent> {
        self.order
            .iter()
            .filter_map(|n| self.agents.get(n))
            .filter(|a| !a.is_main)
    }

    pub fn open_cards(&self) -> impl Iterator<Item = &Card> {
        self.cards.values()
    }

    /// Is `name` taken, as a name or as an alias (RFC 0001 §9.3)?
    pub fn name_taken(&self, name: &str) -> bool {
        name == MAIN || name == USER || name == HUB || self.resolve(name).is_some()
    }

    pub fn apply(&mut self, ev: &Event) {
        match ev {
            Event::TaskCreated {
                name,
                parent,
                brief,
                ws,
                at_ms,
            } => {
                self.agents.insert(
                    name.clone(),
                    Agent {
                        name: name.clone(),
                        dir: name.clone(),
                        is_main: false,
                        parent: Some(parent.clone()),
                        brief: brief.clone(),
                        created_ms: *at_ms,
                        ws: ws.clone(),
                        lifecycle: Lifecycle::Active,
                        failure: None,
                        declared: None,
                        last_report: None,
                        aliases: Vec::new(),
                        files: BTreeSet::new(),
                        snapshot_ref: None,
                        run: Run::Down,
                        waiting: false,
                        turn_started_ms: None,
                        activity: None,
                    },
                );
                if !self.order.contains(name) {
                    self.order.push(name.clone());
                }
            }
            Event::Lifecycle {
                name,
                lifecycle,
                reason,
            } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.lifecycle = lifecycle.clone();
                    a.failure = reason.clone();
                    if *lifecycle == Lifecycle::Active {
                        a.declared = None;
                    }
                }
            }
            Event::WorkspaceChanged { name, ws } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.ws = ws.clone();
                }
            }
            Event::Snapshot { name, snapshot_ref } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.snapshot_ref = snapshot_ref.clone();
                }
            }
            Event::Declared { name, status, note } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.declared = status.map(|s| (s, note.clone()));
                }
            }
            Event::Reported { name, report } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.last_report = Some(report.clone());
                }
            }
            Event::FileTouched { name, path } => {
                if let Some(a) = self.agents.get_mut(name) {
                    a.files.insert(path.clone());
                }
            }
            Event::Renamed { name, new_name } => {
                if let Some(mut a) = self.agents.remove(name) {
                    a.aliases.push(name.clone());
                    a.name = new_name.clone();
                    self.agents.insert(new_name.clone(), a);
                    for n in self.order.iter_mut() {
                        if n == name {
                            *n = new_name.clone();
                        }
                    }
                }
            }
            Event::MessageSent { msg } => {
                self.next_msg = self.next_msg.max(msg.id + 1).max(msg.thread + 1);
                self.msgs.insert(msg.id, msg.clone());
                self.msg_state.insert(
                    msg.id,
                    MsgState::Queued {
                        reason: "new".to_string(),
                    },
                );
                if let Some(r) = msg.reply_to {
                    let replier_is_recipient = self
                        .msgs
                        .get(&r)
                        .is_some_and(|orig| orig.to == msg.from || msg.from == USER);
                    if replier_is_recipient {
                        self.settled.insert(r);
                    }
                }
            }
            Event::MessageState { id, state } => {
                self.msg_state.insert(*id, state.clone());
            }
            Event::MessageSettled { id } => {
                self.settled.insert(*id);
            }
            Event::CardOpened { card } => {
                self.next_card = self.next_card.max(card.id + 1);
                self.cards.insert(card.id, card.clone());
            }
            Event::CardClosed { id, .. } => {
                self.cards.remove(id);
            }
            Event::MainNote { text, .. } => {
                self.main_notes.push(text.clone());
            }
            Event::MainNotesFlushed => {
                self.main_notes.clear();
            }
        }
    }

    /// Messages waiting for delivery to `name`, oldest first.
    pub fn queued_for(&self, name: &str) -> Vec<&Msg> {
        self.msgs
            .values()
            .filter(|m| m.to == name)
            .filter(|m| matches!(self.msg_state.get(&m.id), Some(MsgState::Queued { .. })))
            .collect()
    }

    /// Delivered messages to `name` that expect a reply it has not given.
    pub fn unanswered_for(&self, name: &str) -> Vec<&Msg> {
        self.msgs
            .values()
            .filter(|m| m.to == name && m.expect_reply && !self.settled.contains(&m.id))
            .filter(|m| matches!(self.msg_state.get(&m.id), Some(MsgState::Delivered)))
            .collect()
    }

    /// Messages of a thread, in order.
    pub fn thread(&self, thread: u64) -> Vec<&Msg> {
        self.msgs.values().filter(|m| m.thread == thread).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn created(name: &str) -> Event {
        Event::TaskCreated {
            name: name.to_string(),
            parent: MAIN.to_string(),
            brief: Brief {
                objective: "do it".into(),
                ..Brief::default()
            },
            ws: Workspace {
                mode: Mode::Shared,
                path: "/w".into(),
                branch: None,
                base_commit: None,
                dropped: false,
            },
            at_ms: 5,
        }
    }

    #[test]
    fn events_roundtrip_through_json() {
        let evs = vec![
            created("a"),
            Event::MessageState {
                id: 3,
                state: MsgState::Queued {
                    reason: "busy".into(),
                },
            },
            Event::MainNotesFlushed,
        ];
        for ev in evs {
            let line = serde_json::to_string(&ev).unwrap();
            let back: Event = serde_json::from_str(&line).unwrap();
            assert_eq!(back, ev, "{}", line);
        }
    }

    #[test]
    fn status_follows_lifecycle_then_run_then_declared() {
        let mut st = State::new("/w");
        st.apply(&created("a"));
        let a = st.agents.get_mut("a").unwrap();
        assert_eq!(a.status(), Status::Starting);
        a.run = Run::Busy;
        assert_eq!(a.status(), Status::Working);
        a.waiting = true;
        assert_eq!(a.status(), Status::Waiting);
        a.waiting = false;
        a.run = Run::Idle;
        a.declared = Some((Declared::Done, String::new()));
        assert_eq!(a.status(), Status::Done);
        a.lifecycle = Lifecycle::Archived;
        assert_eq!(a.status(), Status::Archived);
    }

    #[test]
    fn rename_keeps_the_old_name_as_alias() {
        let mut st = State::new("/w");
        st.apply(&created("a"));
        st.apply(&Event::Renamed {
            name: "a".into(),
            new_name: "b".into(),
        });
        assert_eq!(st.resolve("a").as_deref(), Some("b"));
        assert!(st.name_taken("a"));
        assert_eq!(st.order, vec!["main".to_string(), "b".to_string()]);
    }

    #[test]
    fn a_reply_settles_the_question() {
        let mut st = State::new("/w");
        st.apply(&created("a"));
        let q = Msg {
            id: 1,
            thread: 1,
            from: "a".into(),
            to: MAIN.into(),
            reply_to: None,
            expect_reply: true,
            auto: false,
            text: "?".into(),
            created_ms: 1,
            plain: false,
            queued: false,
        };
        st.apply(&Event::MessageSent { msg: q.clone() });
        st.apply(&Event::MessageState {
            id: 1,
            state: MsgState::Delivered,
        });
        assert_eq!(st.unanswered_for(MAIN).len(), 1);
        let r = Msg {
            id: 2,
            from: MAIN.into(),
            to: "a".into(),
            reply_to: Some(1),
            expect_reply: false,
            text: "!".into(),
            ..q
        };
        st.apply(&Event::MessageSent { msg: r });
        assert!(st.unanswered_for(MAIN).is_empty());
        assert_eq!(st.next_msg, 3);
    }
}
