//! Switchboard mode of the TUI (projects/switchboard, RFC 0001 §6 and
//! ux-notes.md): one feed per agent, main in focus by default, a task
//! panel on the right, checkout / Esc, preview, attention cards.
//!
//! The feeds are the same wire lines as a plain session (the hub relays
//! each agent's REPL), so every event renders with the code of lib.rs.
//! The focused feed lives in the `App` fields; the other feeds wait in
//! `Sb::views` and are swapped in on focus change.

use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::os::unix::net::UnixStream;

mod versions;
pub(super) use versions::version_items;
use versions::{parse_versions, VersionItem};
mod mention;
pub(super) use mention::mentions;
mod cards;
pub(super) use cards::{card_box_height, card_full, card_mouse, close_items, draw_card};
use cards::{answer_card, Card, CardView};
mod panel;
pub(super) use panel::{draw_panel, hint, panel_mouse, placeholder, split, status_line, workspace};
use panel::glyph;
mod feed;
pub(super) use feed::FeedWindow;
mod client;
pub use client::{run_switchboard, take_reexec};
use client::{follow_hub_exe, HUB_DOWN, HUB_UP};
#[cfg(test)]
use client::{new_sb, sb_app};
use feed::{
    clear_feed, ingest_at, prepend_page, swap_draft, swap_feed, trim_window, want_older, with_feed, View,
};

#[derive(Clone, Default)]
pub(super) struct Agent {
    name: String,
    main: bool,
    status: String,
    objective: String,
    mode: String,
    branch: Option<String>,
    path: String,
    note: String,
    queued: u64,
    turn_ms: Option<u64>,
    /// The last report, one line, and when it came (for an archived
    /// task: what it did, and about when it stopped).
    report: String,
    report_ms: Option<u64>,
    created_ms: u64,
    /// Who it waits on (`sb wait` / `sb ask`), "" when no one.
    waiting_on: String,
}

impl Agent {
    fn archived(&self) -> bool {
        self.status == "archived"
    }

    /// When it was last heard of: its last report, else its creation.
    fn last_ms(&self) -> u64 {
        self.report_ms.unwrap_or(self.created_ms)
    }

    /// In a turn (its feed shows the spinner).
    fn busy(&self) -> bool {
        self.status == "working" || self.status == "waiting"
    }
}

pub(super) struct Sb {
    /// Shared with the reader thread, which swaps in a fresh stream when
    /// it reconnects after the hub went away (a hub restart, a switch
    /// of version).
    writer: std::sync::Arc<std::sync::Mutex<UnixStream>>,
    workspace: String,
    pub(super) focus: String,
    views: HashMap<String, View>,
    agents: Vec<Agent>,
    cards: Vec<Card>,
    /// Index in `nav()` of the highlighted entry of the panel.
    selected: Option<usize>,
    preview: bool,
    confirm: Option<(u64, String)>,
    /// The feeds out of view where lines arrived since their last visit.
    activity: std::collections::HashSet<String>,
    ready: bool,
    /// Ctrl+O: a shell to open in this directory (RFC 0002 §6).
    shell: Option<String>,
    /// The version the hub runs (its VERSION id), for the status row.
    version: String,
    /// The `/version` picker (the hub's `versions` event), and when it
    /// was last asked for.
    versions: Vec<VersionItem>,
    versions_asked: std::cell::Cell<Option<std::time::Instant>>,
    /// The card box above the composer (Ctrl+G), never opened by the hub.
    card: CardView,
    /// Set by the last draw: the panel rows and their agents (clicks).
    panel_hits: std::cell::RefCell<panel::PanelHits>,
    /// The archived section of the panel is expanded (`A`, a click on
    /// its header, `/archived`).
    archived_open: bool,
}

/// The string field `k` of `v` ("" when absent).
fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

impl Sb {
    fn send(&mut self, v: Value) {
        let mut s = v.to_string();
        s.push('\n');
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(s.as_bytes());
        }
    }

    /// What the panel navigates: main, then the live tasks, then (the
    /// section expanded) the archived ones, newest first.
    fn nav(&self) -> Vec<&Agent> {
        let mut out: Vec<&Agent> = self.agents.iter().filter(|a| !a.archived()).collect();
        if self.archived_open {
            out.extend(self.archived());
        }
        out
    }

    /// The archived tasks, the most recently active first.
    fn archived(&self) -> Vec<&Agent> {
        let mut out: Vec<&Agent> = self.agents.iter().filter(|a| a.archived()).collect();
        out.sort_by_key(|a| std::cmp::Reverse(a.last_ms()));
        out
    }

    /// Expand or collapse the archived section; the selection stays on
    /// the same entry (or leaves a row that is folded away).
    fn toggle_archived(&mut self) {
        let sel = self.selected_agent().map(|a| a.name.clone());
        self.archived_open = !self.archived_open;
        self.selected = sel.and_then(|n| self.nav().iter().position(|a| a.name == n));
        if self.selected.is_none() {
            self.preview = false;
        }
    }

    /// The agent in focus is archived: its feed is read-only.
    fn focus_archived(&self) -> bool {
        self.agent(&self.focus).is_some_and(|a| a.archived())
    }

    /// The context usage of an agent's feed (the focused one lives in
    /// the `App` fields).
    fn usage_of<'a>(&'a self, app: &'a App, name: &str) -> Option<&'a crate::usage::Usage> {
        if self.focus == name {
            return crate::usage::current(&app.events);
        }
        crate::usage::current(&self.views.get(name)?.events)
    }

    fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.name == name)
    }

    /// The entry of the panel highlighted by Ctrl+K/J.
    fn selected_agent(&self) -> Option<&Agent> {
        self.nav().get(self.selected?).copied()
    }

    /// Tell the hub which feed is in focus.
    fn send_focus(&mut self) {
        let focus = self.focus.clone();
        self.send(json!({"op": "focus", "focus": focus}));
    }

    /// A line typed to the agent in focus (the hub interprets it).
    fn send_input(&mut self, text: String) {
        let focus = self.focus.clone();
        self.send(json!({"op": "input", "focus": focus, "text": text}));
    }
}

pub(super) static SB_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) const SB_COMMANDS: &[Cmd] = &[
    Cmd {
        name: "/voice",
        desc: "turn voice mode (Ctrl+R speech-to-text) on or off",
        args: false,
    },
    Cmd {
        name: "/restart",
        desc: "rebuild + restart the hub on the latest commit (agents kept): /restart [current|<commit>]",
        args: true,
    },
    Cmd {
        name: "/version",
        desc: "Switchboard versions: /version [<commit>|tree|back]",
        args: true,
    },
    Cmd {
        name: "/new",
        desc: "create a task: /new [-w] [name:] objective",
        args: true,
    },
    Cmd {
        name: "/drop",
        desc: "stop and archive a task (and its worktree)",
        args: true,
    },
    Cmd {
        name: "/restore",
        desc: "reopen an archived task",
        args: true,
    },
    Cmd {
        name: "/archived",
        desc: "show or hide the archived tasks in the panel",
        args: false,
    },
    Cmd {
        name: "/isolate",
        desc: "give a task its own git worktree",
        args: true,
    },
    Cmd {
        name: "/rename",
        desc: "rename a task",
        args: true,
    },
    Cmd {
        name: "/answer",
        desc: "answer a card: /answer N text",
        args: true,
    },
    Cmd {
        name: "/close",
        desc: "close a card without answering: /close N",
        args: true,
    },
    Cmd {
        name: "/plugins",
        desc: "the workspace's agent plugins (enable|disable NAME)",
        args: true,
    },
    Cmd {
        name: "/tasks",
        desc: "the task board",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrupt the turn of the agent in view",
        args: false,
    },
    Cmd {
        name: "/compact",
        desc: "compact the conversation of the agent in view",
        args: false,
    },
    Cmd {
        name: "/help",
        desc: "the commands and the essential keys",
        args: false,
    },
    Cmd {
        name: "/shortcuts",
        desc: "every keyboard shortcut (also /keys)",
        args: false,
    },
    Cmd {
        name: "/quit",
        desc: "quit (the agents keep running)",
        args: false,
    },
];

/// Ctrl+L in the switchboard: the feed in focus is cleared, its lines
/// stay reachable by scrolling up (`feed::clear_feed`).
pub(super) fn clear_display(app: &mut App) {
    clear_feed(app);
}

/// Startup timing: the hub's `ready` arrived (its replay is taken in).
pub(super) fn is_ready(app: &App) -> bool {
    app.sb.as_ref().is_some_and(|sb| sb.ready)
}

/// Startup timing: the events of the feeds not in focus.
pub(super) fn background_events(app: &App) -> usize {
    app.sb.as_ref().map_or(0, |sb| sb.views.values().map(|v| v.events.len()).sum())
}

/// The shell asked with Ctrl+O, if any.
pub(super) fn take_shell(app: &mut App) -> Option<String> {
    app.sb.as_mut().and_then(|sb| sb.shell.take())
}

/// The hub is back (a new connection, `hello` sent): it replays every
/// feed, so the feeds start empty again. The focus and the drafts stay.
fn hub_reconnected(app: &mut App) {
    app.connected = true;
    app.events.clear();
    app.cache.clear();
    app.win = FeedWindow::default();
    app.pending = false;
    app.interrupt_requested = false;
    app.follow = true;
    app.anchor = (0, 0);
    app.scroll = 0;
    app.unseen = 0;
    let Some(sb) = app.sb.as_mut() else { return };
    for v in sb.views.values_mut() {
        let draft = std::mem::take(&mut v.ed);
        *v = View::new();
        v.ed = draft;
    }
    sb.activity.clear();
    sb.ready = false;
    sb.confirm = None;
    sb.send_focus();
}

/// Route one hub event.
pub(super) fn dispatch(app: &mut App, raw: &str) {
    if raw == HUB_DOWN {
        app.connected = false;
        return;
    }
    if raw == HUB_UP {
        hub_reconnected(app);
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    let s = |k: &str| str_of(&v, k);
    match s("ev").as_str() {
        "line" => {
            let pos = v.get("pos").and_then(|x| x.as_u64()).map(|p| p as usize);
            ingest_for(app, &s("agent"), s("line"), pos)
        }
        "history" => {
            let before = v.get("before").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let lines: Vec<(usize, String)> = v
                .get("lines")
                .and_then(|l| l.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let pos = x.get("pos")?.as_u64()? as usize;
                            Some((pos, x.get("line")?.as_str()?.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            with_feed(app, &s("agent"), |app| prepend_page(app, before, lines));
        }
        "state" => apply_state(app, &v),
        "notice" => {
            for l in s("text").lines() {
                push_event(&mut app.events, &mut app.cache, Ev::Info(l.to_string()));
            }
        }
        "confirm" => {
            let id = v.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
            let text = s("text");
            push_event(&mut app.events, &mut app.cache, Ev::Warn(text.clone()));
            push_event(
                &mut app.events,
                &mut app.cache,
                Ev::Info("answer y (yes) or n (no), then ⏎".into()),
            );
            if let Some(sb) = app.sb.as_mut() {
                sb.confirm = Some((id, text));
            }
        }
        "focus" => focus(app, &s("focus")),
        "renamed" => {
            let (old, new) = (s("old"), s("new"));
            if let Some(sb) = app.sb.as_mut() {
                if let Some(view) = sb.views.remove(&old) {
                    sb.views.insert(new.clone(), view);
                }
                if sb.focus == old {
                    sb.focus = new;
                }
            }
        }
        "versions" => {
            if let Some(sb) = app.sb.as_mut() {
                sb.versions = parse_versions(&v);
            }
        }
        "hello" => {
            if let Some(sb) = app.sb.as_mut() {
                sb.workspace = s("workspace");
                sb.version = v
                    .pointer("/version/id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            // the hub runs another version: this TUI follows it
            let exe = s("exe");
            if !exe.is_empty() && follow_hub_exe(&exe) {
                app.should_quit = true;
            }
        }
        "ready" => {
            if let Some(sb) = app.sb.as_mut() {
                sb.ready = true;
            }
        }
        _ => {}
    }
}

fn ingest_for(app: &mut App, agent: &str, line: String, pos: Option<usize>) {
    let Some(sb) = app.sb.as_mut() else { return };
    if sb.focus != agent {
        let visible = line.contains("obs: assistant:") || line.starts_with("sb ");
        if visible && sb.ready {
            sb.activity.insert(agent.to_string());
        }
    }
    with_feed(app, agent, |app| {
        ingest_at(app, line, pos);
        trim_window(app);
    });
}

fn apply_state(app: &mut App, v: &Value) {
    let Some(sb) = app.sb.as_mut() else { return };
    let s = str_of;
    sb.agents = v
        .get("agents")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| Agent {
                    name: s(x, "name"),
                    main: x.get("main").and_then(|m| m.as_bool()).unwrap_or(false),
                    status: s(x, "status"),
                    objective: s(x, "objective"),
                    mode: s(x, "mode"),
                    branch: x.get("branch").and_then(|b| b.as_str()).map(String::from),
                    path: s(x, "path"),
                    note: s(x, "note"),
                    queued: x.get("queued").and_then(|q| q.as_u64()).unwrap_or(0),
                    turn_ms: x.get("turn_ms").and_then(|q| q.as_u64()),
                    report: s(x, "report"),
                    report_ms: x.get("report_ms").and_then(|q| q.as_u64()),
                    created_ms: x.get("created_ms").and_then(|q| q.as_u64()).unwrap_or(0),
                    waiting_on: s(x, "waiting_on"),
                })
                .collect()
        })
        .unwrap_or_default();
    sb.cards = v
        .get("cards")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| Card {
                    id: x.get("id").and_then(|i| i.as_u64()).unwrap_or(0),
                    kind: s(x, "kind"),
                    agent: s(x, "agent"),
                    text: s(x, "text"),
                    age_ms: x.get("age_ms").and_then(|i| i.as_u64()).unwrap_or(0),
                    seen_at: std::time::Instant::now(),
                    note: s(x, "note"),
                })
                .collect()
        })
        .unwrap_or_default();
    if sb.cards.is_empty() {
        sb.card.shown = false;
        sb.card.full = false;
    }
    // the spinner of every feed follows the agent, whoever started the turn
    let busy: HashMap<String, bool> = sb.agents.iter().map(|a| (a.name.clone(), a.busy())).collect();
    for (name, view) in sb.views.iter_mut() {
        view.pending = busy.get(name).copied().unwrap_or(false);
    }
    let focus_busy = busy.get(&sb.focus).copied().unwrap_or(false);
    if let Some(sel) = sb.selected {
        if sel >= sb.nav().len() {
            sb.selected = None;
            sb.preview = false;
        }
    }
    app.pending = focus_busy;
    if !focus_busy {
        app.interrupt_requested = false;
    }
}

/// Change the feed in focus (checkout / return).
pub(super) fn focus(app: &mut App, name: &str) {
    let Some(sb) = app.sb.as_mut() else { return };
    sb.selected = None;
    sb.preview = false;
    if sb.focus == name {
        return;
    }
    let old = std::mem::replace(&mut sb.focus, name.to_string());
    sb.activity.remove(name);
    let mut incoming = sb.views.remove(name).unwrap_or_else(View::new);
    sb.send_focus();
    swap_feed(app, &mut incoming);
    swap_draft(app, &mut incoming);
    // a feed selection belongs to the feed we left
    app.feed_sel = None;
    // `incoming` now holds the feed we left
    if let Some(sb) = app.sb.as_mut() {
        sb.views.insert(old, incoming);
    }
    app.follow = true;
    app.unseen = 0;
}

/// One line typed by the user (the hub interprets it).
pub(super) fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v)
        .trim()
        .to_string();
    app.history.insert(0, typed.clone());
    app.popup_sel = 0;
    let mut out: Vec<Ev> = Vec::new();
    let Some(sb) = app.sb.as_mut() else {
        return out;
    };
    if let Some((id, _)) = sb.confirm.clone() {
        let t = typed.to_lowercase();
        let yes = matches!(t.as_str(), "y" | "yes" | "o" | "oui");
        let no = matches!(t.as_str(), "n" | "no" | "non");
        if yes || no {
            sb.confirm = None;
            sb.send(json!({"op": "confirm", "id": id, "yes": yes}));
            return out;
        }
    }
    let first = typed.split_whitespace().next().unwrap_or("");
    match first {
        "/quit" | "/exit" => app.should_quit = true,
        "/restart" => {
            let arg = typed.split_whitespace().nth(1).unwrap_or("");
            sb.send(json!({"op": "version", "do": "restart", "to": arg}));
        }
        "/version" => {
            let arg = typed.split_whitespace().nth(1).unwrap_or("");
            let req = match arg {
                "" | "list" => json!({"op": "version", "do": "list"}),
                "back" | "rollback" => json!({"op": "version", "do": "rollback"}),
                to => json!({"op": "version", "do": "switch", "to": to}),
            };
            sb.send(req);
        }
        "/plugins" => {
            let ws = std::path::PathBuf::from(&sb.workspace);
            out.push(Ev::Info(crate::plugins::command(&typed, &ws, None)));
        }
        "/clear" => {
            clear_feed(app);
            out.push(Ev::Info("display cleared — scroll up to see the earlier lines again".into()));
        }
        "/help" | "/shortcuts" | "/shortcut" | "/keys" => {
            app.help = crate::help::page_of(first).map(crate::help::Overlay::new);
        }
        "/archived" => sb.toggle_archived(),
        // an archived task reads nothing: its feed is history only
        _ if sb.focus_archived() && !typed.starts_with('/') => {
            out.push(Ev::Warn(format!(
                "@{} is archived: its history is read-only · /restore brings it back · Esc → main",
                sb.focus
            )));
        }
        _ => {
            sb.send_input(typed);
        }
    }
    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

/// Agent navigation from the keyboard.
#[derive(Debug, PartialEq, Eq)]
enum Nav {
    Next,
    Prev,
    /// agent number N of the list (0 = main)
    Goto(usize),
}

/// Ctrl+K / Alt+↓ next, Ctrl+J / Alt+↑ previous, Alt+N agent N (0 = main).
fn nav_key(k: &crossterm::event::KeyEvent) -> Option<Nav> {
    match (k.code, k.modifiers) {
        (KeyCode::Char('k'), KeyModifiers::CONTROL) | (KeyCode::Down, KeyModifiers::ALT) => {
            Some(Nav::Next)
        }
        (KeyCode::Char('j'), KeyModifiers::CONTROL) | (KeyCode::Up, KeyModifiers::ALT) => {
            Some(Nav::Prev)
        }
        (KeyCode::Char(c), KeyModifiers::ALT) if c.is_ascii_digit() => {
            c.to_digit(10).map(|d| Nav::Goto(d as usize))
        }
        _ => None,
    }
}

/// Keys of the switchboard mode; `true` when handled.
pub(super) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    let empty = app.ed.text.is_empty();
    let pending = app.pending;
    let interrupt_requested = app.interrupt_requested;
    let Some(sb) = app.sb.as_mut() else {
        return false;
    };
    let n = sb.nav().len();
    let nav = nav_key(k);
    match (k.code, k.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) if pending && !interrupt_requested => {
            let f = sb.focus.clone();
            sb.send(json!({"op": "interrupt", "agent": f}));
            app.interrupt_requested = true;
            push_event(
                &mut app.events,
                &mut app.cache,
                Ev::Info("interrupted — the turn stops at the next safe point · Ctrl+C again to quit".into()),
            );
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Next) => {
            sb.selected = Some(match sb.selected {
                None => 0,
                Some(i) => (i + 1) % n,
            });
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Prev) => {
            sb.selected = Some(match sb.selected {
                None | Some(0) => n - 1,
                Some(i) => i - 1,
            });
            true
        }
        (KeyCode::Enter, KeyModifiers::NONE) if empty && sb.selected.is_some() => {
            if let Some(name) = sb.selected_agent().map(|a| a.name.clone()) {
                focus(app, &name);
            }
            true
        }
        (KeyCode::Char(' '), _) if empty && sb.selected.is_some() => {
            sb.preview = !sb.preview;
            true
        }
        (KeyCode::Char('A'), _) if empty && sb.selected.is_some() => {
            sb.toggle_archived();
            true
        }
        (KeyCode::Char('D'), _) if empty && sb.selected.is_some() => {
            let live = |a: &&Agent| !a.main && !a.archived();
            if let Some(name) = sb.selected_agent().filter(live).map(|a| a.name.clone()) {
                sb.send_input(format!("/drop {}", name));
            }
            true
        }
        (KeyCode::Esc, _) if !popup_open => {
            if sb.card.full {
                sb.card.full = false;
                return true;
            }
            if let Some((id, _)) = sb.confirm.take() {
                sb.send(json!({"op": "confirm", "id": id, "yes": false}));
                return true;
            }
            if sb.selected.is_some() || sb.preview {
                sb.selected = None;
                sb.preview = false;
                return true;
            }
            if sb.card.shown && empty {
                sb.card.shown = false;
                return true;
            }
            if !empty {
                // the draft goes to the history (Up brings it back)
                let d = app.ed.take();
                app.history.insert(0, d);
                return true;
            }
            if sb.focus != "main" {
                focus(app, "main");
                return true;
            }
            false
        }
        _ if matches!(nav, Some(Nav::Goto(_))) => {
            // the number shown in the panel (it stays while the agent lives)
            if let Some(t) = nav.and_then(|n| if let Nav::Goto(i) = n { sb.agent_numbered(i) } else { None }) {
                focus(app, &t);
            }
            true
        }
        (KeyCode::Char('g'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.toggle_card();
            true
        }
        // on an empty composer, Ctrl+A (line start) has nothing to do
        (KeyCode::Char('a'), KeyModifiers::CONTROL) if empty && !sb.cards.is_empty() => {
            sb.toggle_card();
            true
        }
        (KeyCode::Char('f'), KeyModifiers::CONTROL) if sb.card.shown => {
            sb.card.full = !sb.card.full;
            true
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.step_card(1);
            true
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.step_card(-1);
            true
        }
        (KeyCode::PageUp, _) if sb.card.shown && !popup_open => {
            let page = sb.card.page.max(1) as isize;
            sb.card.scroll_by(-page);
            true
        }
        (KeyCode::PageDown, _) if sb.card.shown && !popup_open => {
            let page = sb.card.page.max(1) as isize;
            sb.card.scroll_by(page);
            true
        }
        // an empty composer: the arrows scroll a card longer than its box
        (KeyCode::Up, KeyModifiers::NONE)
            if empty && sb.card.shown && sb.card.max_scroll > 0 && !popup_open =>
        {
            sb.card.scroll_by(-1);
            true
        }
        (KeyCode::Down, KeyModifiers::NONE)
            if empty && sb.card.shown && sb.card.max_scroll > 0 && !popup_open =>
        {
            sb.card.scroll_by(1);
            true
        }
        (KeyCode::Char('x'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            if let Some(id) = sb.current_card().map(|c| c.id) {
                sb.send_input(format!("/close {}", id));
                sb.card.scroll = 0;
            }
            true
        }
        // Alt+R; '®' is Option+R on a macOS terminal that does not send
        // Option as Alt
        (KeyCode::Char('r'), KeyModifiers::ALT) | (KeyCode::Char('®'), KeyModifiers::NONE)
            if !sb.cards.is_empty() =>
        {
            answer_card(app);
            true
        }
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
            let dir = sb
                .agent(&sb.focus)
                .map(|a| a.path.clone())
                .filter(|p| !p.is_empty());
            let dir = dir.unwrap_or_else(|| sb.workspace.clone());
            sb.shell = Some(dir);
            true
        }
        _ => false,
    }
}

/// Draw, with the previewed feed swapped in when a preview is open.
pub(super) fn draw_sb(app: &mut App, frame: &mut Frame) {
    let target = app.sb.as_ref().and_then(|sb| {
        if !sb.preview {
            return None;
        }
        sb.selected_agent()
            .map(|a| a.name.clone())
            .filter(|n| *n != sb.focus)
    });
    match target {
        Some(name) => with_feed(app, &name, |app| draw(app, frame)),
        None => {
            draw(app, frame);
            want_older(app);
        }
    }
}

/// The synthetic lines of the hub (`sb <kind> : <text>`) as feed events.
/// A hub line `sb <kind> : <text>` (hub line protocol, contract C2).
/// v1 kinds keep working (an old transcript still renders); v2 adds:
/// - `msg : {from} → {to} : {text}`: between agents (level 3);
/// - `msg-you : {from} : {text}`: an agent writing to the user (level 2);
/// - `answered : {agent} : {question} : {answer} : {why}`: main answered
///   an agent for the user (level 2). Inside a field the hub escapes
///   `" : "` as `" \: "` (core.rs `field_escape`); undone here.
pub(super) fn parse_hub_line(rest: &str) -> Option<Ev> {
    let (kind, raw) = rest.split_once(" : ").unwrap_or((rest, ""));
    let text = unescape_md(raw);
    let field = |s: &str| unescape_md(&s.replace(" \\: ", " : "));
    Some(match kind {
        "you" => Ev::You(text),
        // v1: what this feed's owner received: `{from} m_<n> : {text}`
        // (`@{from} : {text}` for an old direct reply to the user)
        "msg-in" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            let (from, id) = head.split_once(' ').unwrap_or((head, ""));
            match from.strip_prefix('@') {
                Some(f) => Ev::AgentMsg {
                    from: f.to_string(),
                    to: "you".into(),
                    text: body.to_string(),
                    level: 2,
                    id: id.to_string(),
                    open: false,
                },
                None => Ev::AgentMsg {
                    from: from.to_string(),
                    to: String::new(),
                    text: body.to_string(),
                    level: 3,
                    id: id.to_string(),
                    open: false,
                },
            }
        }
        "msg" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            let (from, to) = head.split_once(" → ").unwrap_or((head, ""));
            Ev::AgentMsg {
                from: from.to_string(),
                to: to.to_string(),
                text: body.to_string(),
                level: 3,
                id: String::new(),
                open: false,
            }
        }
        "msg-you" => {
            let (from, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            Ev::AgentMsg {
                from: from.to_string(),
                to: "you".into(),
                text: body.to_string(),
                level: 2,
                id: String::new(),
                open: false,
            }
        }
        "answered" => {
            let mut f = raw.splitn(4, " : ").map(field);
            let mut next = || f.next().unwrap_or_default();
            Ev::Answered {
                agent: next(),
                question: next(),
                answer: next(),
                why: next(),
            }
        }
        "card" => Ev::Card(text),
        "card-closed" => Ev::Info(format!("card {} ", text)),
        "route" => Ev::Info(format!("→ {}", text)),
        "spawn" => Ev::Info(format!("✚ {}", text)),
        "direct" => Ev::Info(format!("⇄ {}", text)),
        "warn" => Ev::Warn(text),
        _ => Ev::Info(text),
    })
}

/// BISE-04: the hub line protocol, v1 and v2 kinds (contract C2).
#[cfg(test)]
mod hub_line_tests {
    use super::*;

    fn msg(from: &str, to: &str, text: &str, level: u8, id: &str) -> String {
        format!("msg {from}|{to}|{text}|{level}|{id}")
    }

    /// The feed rows of `evs`, as text.
    fn draw(evs: &[Ev]) -> String {
        (0..evs.len())
            .flat_map(|i| crate::feed::build_rows(evs, i, false, 60, 0))
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>() + "\n")
            .collect()
    }

    /// A comparable form of the events these tests look at (`Ev` has no
    /// `PartialEq`).
    fn p(line: &str) -> Option<String> {
        Some(match parse_hub_line(line)? {
            Ev::AgentMsg { from, to, text, level, id, .. } => format!("msg {from}|{to}|{text}|{level}|{id}"),
            Ev::Answered { agent, question, answer, why } => format!("answered {agent}|{question}|{answer}|{why}"),
            Ev::You(t) => format!("you {t}"),
            Ev::Card(t) => format!("card {t}"),
            Ev::Info(t) => format!("info {t}"),
            Ev::Warn(t) => format!("warn {t}"),
            _ => "other".into(),
        })
    }

    #[test]
    fn v2_kinds() {
        assert_eq!(p("msg : a → b : hi : there"), Some(msg("a", "b", "hi : there", 3, "")));
        assert_eq!(p("msg : a → b : one\\ntwo"), Some(msg("a", "b", "one\ntwo", 3, "")));
        assert_eq!(p("msg-you : docs : la v2"), Some(msg("docs", "you", "la v2", 2, "")));
        assert_eq!(
            p("answered : docs : v1 \\: v2? : v2 : the brief says v2").as_deref(),
            Some("answered docs|v1 : v2?|v2|the brief says v2")
        );
        assert_eq!(p("answered : docs : q : a : ").as_deref(), Some("answered docs|q|a|"));
    }

    /// An old (v1) transcript still parses to what it drew before.
    #[test]
    fn v1_kinds_still_parse() {
        assert_eq!(p("msg-in : docs m_3 : done : ok"), Some(msg("docs", "", "done : ok", 3, "m_3")));
        assert_eq!(p("msg-in : @docs : la v2"), Some(msg("docs", "you", "la v2", 2, "")));
        assert_eq!(p("you : bonjour"), Some("you bonjour".into()));
        assert_eq!(p("card : #1 question @docs"), Some("card #1 question @docs".into()));
        assert_eq!(p("spawn : main → new task @t : x"), Some("info ✚ main → new task @t : x".into()));
        assert_eq!(p("warn : w"), Some("warn w".into()));
        // and draws: an old main feed with every v1 kind
        let evs: Vec<Ev> = [
            "you : bonjour",
            "msg-in : docs m_3 : done",
            "msg-in : @docs : la v2",
            "card : #1 question @docs",
            "card-closed : #1 answered",
            "route : you → @docs : v2",
            "direct : x",
            "warn : w",
        ]
        .iter()
        .filter_map(|l| parse_hub_line(l))
        .collect();
        let text = draw(&evs);
        let (m3, to_you) = (format!("{G_MSG} docs m_3"), format!("{G_MSG} docs to you"));
        for want in [m3.as_str(), "done", to_you.as_str(), "la v2", "docs needs you", "→ you → @docs"] {
            assert!(text.contains(want), "{want:?} missing in:\n{text}");
        }
    }

    #[test]
    fn peer_and_answered_draw() {
        let evs = vec![
            parse_hub_line("msg : a → b : hello b").unwrap(),
            parse_hub_line("answered : docs : v1 or v2? : v2 : the brief").unwrap(),
        ];
        let text = draw(&evs);
        let ab = format!("{G_MSG} a → b");
        for want in [ab.as_str(), "hello b", "main answered @docs", "docs asked: v1 or v2?", "main answered: v2", "why: the brief"] {
            assert!(text.contains(want), "{want:?} missing in:\n{text}");
        }
    }
}

#[cfg(test)]
pub(crate) mod bench;

#[cfg(test)]
mod nav_key_tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn nav(code: KeyCode, m: KeyModifiers) -> Option<Nav> {
        nav_key(&KeyEvent::new(code, m))
    }

    #[test]
    fn ctrl_k_next_ctrl_j_previous() {
        assert_eq!(nav(KeyCode::Char('k'), KeyModifiers::CONTROL), Some(Nav::Next));
        assert_eq!(nav(KeyCode::Char('j'), KeyModifiers::CONTROL), Some(Nav::Prev));
        assert_eq!(nav(KeyCode::Down, KeyModifiers::ALT), Some(Nav::Next));
        assert_eq!(nav(KeyCode::Up, KeyModifiers::ALT), Some(Nav::Prev));
    }

    fn agent(name: &str) -> Agent {
        Agent {
            name: name.into(),
            main: name == "main",
            status: "idle".into(),
            objective: String::new(),
            mode: String::new(),
            branch: None,
            path: String::new(),
            note: String::new(),
            queued: 0,
            turn_ms: None,
            ..Agent::default()
        }
    }

    fn press(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
        key(app, &KeyEvent::new(code, m), false)
    }

    /// The panel path of tui_tmux.py: from no selection, Ctrl+K selects
    /// main then the first task; Enter (empty composer) enters its view.
    #[test]
    fn ctrl_k_then_enter_enters_the_selected_task() {
        let mut app = bench::test_app();
        app.sb.as_mut().unwrap().agents = vec![agent("main"), agent("t1")];
        assert!(press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(app.sb.as_ref().unwrap().selected, Some(0));
        assert!(press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(app.sb.as_ref().unwrap().selected, Some(1));
        assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        let sb = app.sb.as_ref().unwrap();
        assert_eq!(sb.focus, "t1");
        assert_eq!(sb.selected, None);
    }

    /// Ctrl+J goes backwards: from no selection, the last agent first.
    #[test]
    fn ctrl_j_from_nothing_selects_the_last_agent() {
        let mut app = bench::test_app();
        app.sb.as_mut().unwrap().agents = vec![agent("main"), agent("t1"), agent("t2")];
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.sb.as_ref().unwrap().selected, Some(2));
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.sb.as_ref().unwrap().selected, Some(1));
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.as_ref().unwrap().focus, "t1");
    }

    /// No undo (book §13): Ctrl+Z is not a switchboard key, and the
    /// composer's own undo (Cmd+Z, Ctrl+/) does not answer to it either.
    #[test]
    fn ctrl_z_does_nothing() {
        let mut app = bench::test_app();
        app.ed.text = "draft".into();
        assert!(!press(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL));
        let k = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert!(crate::editor::action(&k).is_none());
        assert_eq!(app.ed.text, "draft");
        assert!(!SB_COMMANDS.iter().any(|c| c.name == "/cancel"));
    }

    /// Ctrl+R belongs to voice input: Alt+R (or '®', Option+R on a
    /// macOS terminal) answers the card with the composer text.
    #[test]
    fn alt_r_answers_the_card_and_ctrl_r_is_not_the_cards() {
        let mut app = bench::test_app();
        app.sb.as_mut().unwrap().cards = vec![Card {
            id: 7,
            kind: "question".into(),
            agent: "t1".into(),
            text: "which one?".into(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
        }];
        app.ed.text = "the first".into();
        assert!(!press(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(app.ed.text, "the first");
        assert!(press(&mut app, KeyCode::Char('r'), KeyModifiers::ALT));
        assert_eq!(app.ed.text, "", "the answer left the composer");
        app.ed.text = "again".into();
        assert!(press(&mut app, KeyCode::Char('®'), KeyModifiers::NONE));
        assert_eq!(app.ed.text, "");
    }

    /// A non-empty composer keeps Enter for sending: no view change.
    #[test]
    fn enter_with_text_does_not_enter_the_selection() {
        let mut app = bench::test_app();
        app.sb.as_mut().unwrap().agents = vec![agent("main"), agent("t1")];
        app.sb.as_mut().unwrap().selected = Some(1);
        app.ed.text = "hello".into();
        assert!(!press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.sb.as_ref().unwrap().focus, "main");
    }

    #[test]
    fn alt_digits_go_to_agent_n() {
        for d in 0..=9u32 {
            let c = char::from_digit(d, 10).unwrap();
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::ALT), Some(Nav::Goto(d as usize)));
            // Ctrl+digit is not bound (most terminals cannot send it)
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::CONTROL), None);
        }
    }

    #[test]
    fn plain_keys_and_card_keys_are_not_navigation() {
        assert_eq!(nav(KeyCode::Char('1'), KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Char('k'), KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Enter, KeyModifiers::NONE), None);
        assert_eq!(nav(KeyCode::Esc, KeyModifiers::NONE), None);
        for c in ['g', 'f', 'n', 'p', 'r', 'x', 'a', 'c', 'o', 'z'] {
            assert_eq!(nav(KeyCode::Char(c), KeyModifiers::CONTROL), None);
        }
    }
}
