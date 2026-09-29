//! Switchboard mode of the TUI (projects/switchboard, RFC 0001 §6 and
//! ux-notes.md): one feed per agent, main in focus by default, a task
//! panel on the right, checkout / Esc, preview, attention cards.
//!
//! The feeds are the wire lines of each agent's REPL (the hub relays
//! them), so every event renders with the code of lib.rs.
//! The focused feed lives in the `App` fields; the other feeds wait in
//! `Sb::views` and are swapped in on focus change.

use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::os::unix::net::UnixStream;

mod versions;
pub(super) use versions::version_choices;
use versions::{parse_versions, VersionItem};
mod mention;
pub(super) use mention::mentions;
mod cards;
pub(super) use cards::{card_box_height, card_full, card_mouse, card_choices, draw_card};
use cards::{answer_card, Card, CardView};
mod panel;
pub(super) use panel::{draw_panel, focus_model, key_mode, panel_mouse, placeholder, split, status_state, viewed_model, viewed_who, viewed_working, workspace};
#[cfg(test)]
pub(super) use panel::status_text;
use panel::glyph;
mod feed;
pub(super) use feed::FeedWindow;
mod client;
pub(crate) mod drafts;
mod keys;
pub(super) use keys::key;
pub(crate) use keys::{scene, Scene};
#[cfg(test)]
use keys::{nav_key, Nav};
pub use client::{run_switchboard, take_reexec};
use client::{follow_hub_exe, follow_reload, HUB_DOWN, HUB_UP};
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
    /// When `turn_ms` came: the turn's age moves on between two states.
    turn_seen: Option<std::time::Instant>,
    /// The last report, one line, and when it came (for an archived
    /// task: what it did, and about when it stopped).
    report: String,
    report_ms: Option<u64>,
    /// What it is doing now, one line (BISE-126): the hub's role line, ""
    /// for main.
    role: String,
    created_ms: u64,
    /// Who it waits on (`sb wait` / `sb ask`), "" when no one.
    waiting_on: String,
    /// BISE-136: the private git worktree it works in (`gate.sh new`),
    /// "" when it works in its own workspace.
    place: String,
    /// The model it runs (the full `provider/id`) and its reasoning
    /// effort ("" = the model takes none), as the hub resolves them
    /// (BISE-135: its `/model`, `/reasoning` choice first); the efforts
    /// its model takes (the `/reasoning` list).
    model: String,
    effort: String,
    efforts: Vec<String>,
}

impl Agent {
    fn archived(&self) -> bool {
        self.status == "archived"
    }

    /// When it was last heard of: its last report, else its creation.
    fn last_ms(&self) -> u64 {
        self.report_ms.unwrap_or(self.created_ms)
    }

    /// The current turn's age now: the hub's `turn_ms` plus the time
    /// since it came.
    fn turn_age_ms(&self) -> Option<u64> {
        let since = self.turn_seen.map_or(0, |t| t.elapsed().as_millis() as u64);
        self.turn_ms.map(|ms| ms.saturating_add(since))
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
    /// `D` on an agent asks first (book §16): the agent to drop on `y`.
    drop_ask: Option<String>,
    /// The feeds out of view where lines arrived since their last visit.
    activity: std::collections::HashSet<String>,
    ready: bool,
    /// The version the hub runs (its VERSION id), for the status row.
    version: String,
    /// The `/version` picker (the hub's `versions` event), and when it
    /// was last asked for.
    versions: Vec<VersionItem>,
    versions_asked: std::cell::Cell<Option<std::time::Instant>>,
    /// The reload id of the first hub this TUI met (None before its
    /// first hello): a hub with another one was started by a reload
    /// (BISE-131), which this TUI follows by re-executing itself.
    reload_seen: Option<String>,
    /// The card box above the composer (Ctrl+G), never opened by the hub.
    card: CardView,
    /// Set by the last draw: the panel rows and their agents (clicks).
    panel_hits: std::cell::RefCell<panel::PanelHits>,
    /// The archived section of the panel is expanded (`A`, a click on
    /// its header, `/archived`).
    archived_open: bool,
    /// Things that asked for you since the start (a new card, a message
    /// to you, a confirm): a change leaves zen (BISE-121).
    calls: u64,
}

/// The string field `k` of `v` ("" when absent).
fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

impl Sb {
    /// The feed in view is main's (its replies carry `:*`, BISE-15).
    /// The agent in view (whose feed is drawn).
    pub(crate) fn focus_name(&self) -> &str {
        &self.focus
    }

    pub(crate) fn is_main_focus(&self) -> bool {
        match self.agents.iter().find(|a| a.name == self.focus) {
            Some(a) => a.main,
            None => self.focus == "main",
        }
    }

    /// How many things asked for you so far (zen, BISE-121).
    pub(crate) fn calls(&self) -> u64 {
        self.calls
    }

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

    /// The model of the agent in focus (BISE-150): the one its last
    /// usage line names, else the one its role starts with (main's or
    /// the sub-agents', from the catalog's setup).
    fn focus_model(&self, app: &App) -> String {
        // the hub says what it runs now (BISE-135: a /model switch)
        if let Some(m) = self.agent(&self.focus).map(|a| a.model.clone()).filter(|m| !m.is_empty()) {
            return m;
        }
        let used = app.events.iter().rev().find_map(|e| match e {
            crate::Ev::Usage(u) if !u.model.is_empty() => Some(u.model.clone()),
            _ => None,
        });
        used.unwrap_or_else(|| {
            let main = self.agent(&self.focus).is_none_or(|a| a.main);
            crate::models::model_for(main)
        })
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
        self.send_input_to(&focus, text);
    }

    /// What the user typed, for `agent` (in view or not).
    fn send_input_to(&mut self, agent: &str, text: String) {
        self.send(json!({"op": "input", "focus": agent, "text": text}));
    }
}

/// What `ctrl+z` and a typed `/cancel` say (book §13, §17).
pub(super) const NO_UNDO: &str =
    "no undo: an agent may already have acted. say the change to main instead (\"no, v1 for docs\").";

/// `/theme [auto|light|dark]`: switch the palette now and keep it for
/// the next launches (`choose`: `theme_detect::choose`, a fake in tests
/// so they never write the real home). No argument: say which one is in
/// use.
fn theme_command(
    arg: Option<&str>,
    choose: impl Fn(crate::theme_detect::Choice) -> (crate::theme::Mode, Result<(), String>),
) -> Ev {
    use crate::theme_detect::Choice;
    let name = |m| if m == crate::theme::Mode::Light { "light" } else { "dark" };
    match arg.map(Choice::parse) {
        None => Ev::Info(format!("theme: {}. /theme auto, light or dark to change it.", name(crate::theme::mode()))),
        Some(Some(c)) => match choose(c) {
            (m, Ok(())) => Ev::Info(format!("theme: {}.", name(m))),
            (m, Err(e)) => Ev::Warn(format!("theme: {}, for now: i couldn't save it ({}).", name(m), e)),
        },
        Some(None) => Ev::Warn("/theme takes auto, light or dark.".into()),
    }
}


/// Ctrl+L in the switchboard: the feed in focus is cleared, its lines
/// stay reachable by scrolling up (`feed::clear_feed`).
pub(super) fn clear_display(app: &mut App) {
    clear_feed(app);
}

/// The agents matching `q` (name or objective): the live tasks (not
/// main), or the archived ones, newest first (BISE-117).
pub(super) fn agent_choices(app: &App, archived: bool, q: &str) -> Vec<Choice> {
    let sb = &app.sb;
    let list: Vec<&Agent> = if archived {
        sb.archived()
    } else {
        sb.agents.iter().filter(|a| !a.main && !a.archived()).collect()
    };
    list.into_iter()
        .filter(|a| crate::commands::matches(q, &[&a.name, &a.objective]))
        .map(|a| Choice {
            value: a.name.clone(),
            label: a.name.clone(),
            desc: format!("{} · {}", a.status, truncate_chars(&a.objective, 80)),
            mark: None,
        })
        .collect()
}

/// Startup timing: the hub's `ready` arrived (its replay is taken in).
pub(super) fn is_ready(app: &App) -> bool {
    app.sb.ready
}

/// Startup timing: the events of the feeds not in focus.
pub(super) fn background_events(app: &App) -> usize {
    app.sb.views.values().map(|v| v.events.len()).sum()
}

/// The hub is back (a new connection, `hello` sent): it replays every
/// feed, so the feeds start empty again. The focus and the drafts stay.
fn hub_reconnected(app: &mut App) {
    app.connected = true;
    feed::empty_feed(app);
    app.win = FeedWindow::default();
    app.pending = false;
    app.interrupt_requested = false;
    let sb = &mut app.sb;
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
            let lines = crate::wire::parse_history(&v);
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
            let sb = &mut app.sb;
            sb.confirm = Some((id, text));
            sb.calls += 1;
        }
        "focus" => focus(app, &s("focus")),
        "renamed" => {
            let (old, new) = (s("old"), s("new"));
            let sb = &mut app.sb;
            if let Some(view) = sb.views.remove(&old) {
                sb.views.insert(new.clone(), view);
            }
            if sb.focus == old {
                sb.focus = new;
            }
        }
        "versions" => {
            let sb = &mut app.sb;
            sb.versions = parse_versions(&v);
        }
        "hello" => {
            let sb = &mut app.sb;
            sb.workspace = s("workspace");
            sb.version = v
                .pointer("/version/id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            // the hub runs another version: this TUI follows it
            let exe = s("exe");
            let reload = s("reload");
            let first = sb.reload_seen.is_none();
            let reloaded = !first && !reload.is_empty() && sb.reload_seen.as_deref() != Some(reload.as_str());
            if first {
                sb.reload_seen = Some(reload);
            }
            if !exe.is_empty() && follow_hub_exe(&exe) {
                app.should_quit = true;
            } else if reloaded && follow_reload() {
                // a reload (BISE-131): the same binary starts again; the
                // drafts and queues are written when the UI ends
                app.should_quit = true;
            }
        }
        "ready" => {
            let sb = &mut app.sb;
            sb.ready = true;
            // the queues saved by the TUI before this one (a reload, a
            // restart): back now that the feeds say who is busy
            drafts::requeue(app);
        }
        _ => {}
    }
}

fn ingest_for(app: &mut App, agent: &str, line: String, pos: Option<usize>) {
    let sb = &mut app.sb;
    // BISE-61: the first live message between agents in view
    let level3 = sb.ready
        && sb.focus == agent
        && (line.starts_with("sb msg : ") || line.starts_with("sb msg-in : "));
    // BISE-15: the first steering the model read (its line turns ✓✓)
    let steered = sb.ready && sb.focus == agent && line.contains("obs: steered: ");
    // zen (BISE-121): a live card or message to you, in any feed
    if sb.ready && (line.starts_with("sb card : ") || line.starts_with("sb msg-you : ") || line.starts_with("sb msg-in : @")) {
        sb.calls += 1;
    }
    if sb.focus != agent {
        let visible = line.contains("obs: assistant:") || line.starts_with("sb ");
        if visible && sb.ready {
            sb.activity.insert(agent.to_string());
        }
    }
    let mut queued = None;
    with_feed(app, agent, |app| {
        ingest_at(app, line, pos);
        trim_window(app);
        // BISE-89: its turn ended, the oldest queued message goes (and
        // the next one waits for the turn it starts)
        queued = crate::queue::next(app);
        if queued.is_some() {
            app.pending = true;
        }
    });
    if let Some(m) = queued {
        app.sb.send_input_to(agent, m);
    }
    if level3 {
        crate::hints::once(app, crate::hints::Hint::FirstLevel3);
    }
    if steered {
        crate::hints::once(app, crate::hints::Hint::FirstSteer);
    }
}

fn apply_state(app: &mut App, v: &Value) {
    let sb = &mut app.sb;
    let s = str_of;
    let known: Vec<u64> = sb.cards.iter().map(|c| c.id).collect();
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
                    turn_seen: Some(std::time::Instant::now()),
                    report: s(x, "report"),
                    role: s(x, "role"),
                    report_ms: x.get("report_ms").and_then(|q| q.as_u64()),
                    created_ms: x.get("created_ms").and_then(|q| q.as_u64()).unwrap_or(0),
                    waiting_on: s(x, "waiting_on"),
                    place: s(x, "place"),
                    model: s(x, "model"),
                    effort: s(x, "effort"),
                    efforts: x
                        .get("efforts")
                        .and_then(|e| e.as_array())
                        .map(|e| e.iter().filter_map(|w| w.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
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
    // zen (BISE-121): a card that was not there
    if sb.cards.iter().any(|c| !known.contains(&c.id)) {
        sb.calls += 1;
    }
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
    // BISE-61: the first agent, the first card (one-time hints)
    let sb = &app.sb;
    let (agent, card) = (sb.agents.iter().any(|a| !a.main && !a.archived()), !sb.cards.is_empty());
    if agent {
        crate::hints::once(app, crate::hints::Hint::FirstAgent);
    }
    if card {
        crate::hints::once(app, crate::hints::Hint::FirstCard);
    } else {
        crate::hints::used(crate::hints::Hint::FirstCard);
    }
}

/// Change the feed in focus (checkout / return).
pub(super) fn focus(app: &mut App, name: &str) {
    let sb = &mut app.sb;
    // BISE-61: looking inside an agent is what the first-agent hint asks
    if sb.agents.iter().any(|a| a.name == name && !a.main) {
        crate::hints::used(crate::hints::Hint::FirstAgent);
    }
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
    let sb = &mut app.sb;
    sb.views.insert(old, incoming);
    app.follow = true;
    app.unseen = 0;
}

/// One line typed by the user: the client's own commands (/voice,
/// /quit, /clear, /help, /theme…) here, the rest goes to the hub.
pub(crate) fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    if v.trim() == "/voice" {
        let ev = crate::input::toggle_voice(app);
        push_event(&mut app.events, &mut app.cache, ev.clone());
        return vec![ev];
    }
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v)
        .trim()
        .to_string();
    app.history.insert(0, typed.clone());
    app.history.truncate(drafts::HISTORY_MAX);
    // BISE-120a: the sent draft leaves the file at once
    drafts::save_now(app);
    app.popup_sel = 0;
    let mut out: Vec<Ev> = Vec::new();
    let sb = &mut app.sb;
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
    let recolor = first == "/theme";
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
            out.push(Ev::Info(crate::plugins::command(&typed, &ws)));
        }
        "/clear" => {
            clear_feed(app);
            out.push(Ev::Info("display cleared — scroll up to see the earlier lines again".into()));
        }
        "/help" | "/shortcuts" | "/shortcut" | "/keys" => {
            app.help = crate::help::page_of(first).map(crate::help::Overlay::new);
        }
        "/archived" => sb.toggle_archived(),
        "/theme" => out.push(theme_command(typed.split_whitespace().nth(1), crate::theme_detect::choose)),
        "/welcome" => crate::onboarding::run(app),
        "/cancel" => out.push(Ev::Info(NO_UNDO.into())),
        // an archived task reads nothing: its feed is history only
        _ if sb.focus_archived() && !typed.starts_with('/') => {
            out.push(Ev::Warn(format!(
                "@{} is archived: its history is read-only · /restore brings it back · esc → main",
                sb.focus
            )));
        }
        _ => {
            sb.send_input(typed);
            // BISE-61: a hint goes away after the next user message
            crate::hints::user_message();
        }
    }
    if recolor {
        // the feed's rows carry their colors: build them again
        app.cache.clear();
    }
    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

/// Draw, with the previewed feed swapped in when a preview is open.
pub(super) fn draw_sb(app: &mut App, frame: &mut Frame) {
    let sb = &app.sb;
    let target = sb
        .selected_agent()
        .filter(|_| sb.preview)
        .map(|a| a.name.clone())
        .filter(|n| *n != sb.focus);
    match target {
        Some(name) => with_feed(app, &name, |app| draw(app, frame)),
        None => {
            draw(app, frame);
            want_older(app);
        }
    }
}

/// A message id: `m_<digits>`.
pub(crate) fn is_msg_id(s: &str) -> bool {
    s.strip_prefix("m_").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
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
        "you" => Ev::You(text, Mark::Sent),
        // BISE-86: `undelivered : {name} : {text}` (fields escaped)
        "undelivered" => {
            let (name, t) = raw.split_once(" : ").unwrap_or((raw, ""));
            Ev::Undelivered { name: field(name), text: field(t), open: true }
        }
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
                    fold: false,
                },
                None => Ev::AgentMsg {
                    from: from.to_string(),
                    to: String::new(),
                    text: body.to_string(),
                    level: 3,
                    id: id.to_string(),
                    open: false,
                    fold: false,
                },
            }
        }
        // `{from} → {to} m_<n> : {text}` (the id since BISE-110; an older
        // line has none)
        "msg" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            let (from, to) = head.split_once(" → ").unwrap_or((head, ""));
            let (to, id) = match to.rsplit_once(' ') {
                Some((t, id)) if is_msg_id(id) => (t, id),
                _ => (to, ""),
            };
            Ev::AgentMsg {
                from: from.to_string(),
                to: to.to_string(),
                text: body.to_string(),
                level: 3,
                id: id.to_string(),
                open: false,
                fold: false,
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
                fold: false,
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
                open: false,
            }
        }
        "card" => Ev::Card { text, closed: String::new() },
        "card-closed" => match text.strip_prefix('#').and_then(|t| t.split_once(' ')) {
            Some((id, res)) if id.parse::<u64>().is_ok() => Ev::CardClosed {
                id: id.parse().unwrap_or(0),
                res: res.trim().to_string(),
            },
            _ => Ev::Info(format!("card {} ", text)),
        },
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
            Ev::Answered { agent, question, answer, why, .. } => format!("answered {agent}|{question}|{answer}|{why}"),
            Ev::You(t, _) => format!("you {t}"),
            Ev::Card { text, .. } => format!("card {text}"),
            Ev::CardClosed { id, res } => format!("card-closed {id}|{res}"),
            Ev::Info(t) => format!("info {t}"),
            Ev::Warn(t) => format!("warn {t}"),
            _ => "other".into(),
        })
    }

    #[test]
    fn v2_kinds() {
        assert_eq!(p("msg : a → b : hi : there"), Some(msg("a", "b", "hi : there", 3, "")));
        // BISE-110: the id after the receiver
        assert_eq!(p("msg : main → docs m_12 : use v2"), Some(msg("main", "docs", "use v2", 3, "m_12")));
        assert_eq!(p("msg : a → b m_x : hi"), Some(msg("a", "b m_x", "hi", 3, "")));
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
        assert_eq!(p("card-closed : #1 answered"), Some("card-closed 1|answered".into()));
        assert_eq!(p("card-closed : #12 answered via @main"), Some("card-closed 12|answered via @main".into()));
        assert_eq!(p("card-closed : weird"), Some("info card weird ".into()));
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
        // level 3: a chip, the id as the receiver (BISE-106)
        let (m3, to_you) = (format!("{} docs → m_3", crate::render::G_ENVELOPE), format!("{G_MSG} docs to you"));
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
        let ab = format!("{} a → b", crate::render::G_ENVELOPE);
        let why = format!("{} why", crate::theme::G_CLOSED);
        for want in [ab.as_str(), "hello b", "docs asked: v1 or v2? i answered: v2", why.as_str()] {
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

    /// BISE-150: a message with an image to a model the catalog lists
    /// without vision is not sent: the no-vision line, the text stays.
    #[test]
    fn images_to_a_model_without_vision_are_refused_before_sending() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main")];
        let usage = |m: &str| Ev::Usage(crate::usage::Usage { model: m.into(), input: 10, ..Default::default() });
        app.events.push(usage("mistral/codestral-latest"));
        app.cache.push(None);
        app.attachments.push(crate::attach::Attachment {
            label: "[Image #1]".into(),
            marker: "<image name=\"[Image #1]\" b64=\"/x.b64\">".into(),
            info: Default::default(),
        });
        app.ed.insert("look at [Image #1]");
        let n = app.events.len();
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.ed.text, "look at [Image #1]", "kept in the composer");
        assert_eq!(app.attachments.len(), 1);
        match app.events.get(n) {
            Some(Ev::Err(e)) => {
                assert!(crate::attach::is_no_vision(e), "{e}");
                assert!(e.contains("mistral/codestral-latest"), "{e}");
            }
            _ => panic!("no no-vision line"),
        }
        // a model that reads images: sent as before
        app.events.push(usage("mistral/mistral-medium-latest"));
        app.cache.push(None);
        crate::input::on_key(&mut app, &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.ed.text, "");
        assert!(app.attachments.is_empty());
    }

    /// The panel path of tui_tmux.py: from no selection, Ctrl+K selects
    /// main then the first task; Enter (empty composer) enters its view.
    #[test]
    fn ctrl_k_then_enter_enters_the_selected_task() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main"), agent("t1")];
        assert!(press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(app.sb.selected, Some(0));
        assert!(press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(app.sb.selected, Some(1));
        assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        let sb = &app.sb;
        assert_eq!(sb.focus, "t1");
        assert_eq!(sb.selected, None);
    }

    /// Ctrl+J goes backwards: from no selection, the last agent first.
    #[test]
    fn ctrl_j_from_nothing_selects_the_last_agent() {
        let mut app = bench::test_app();
        app.sb.agents = vec![agent("main"), agent("t1"), agent("t2")];
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.sb.selected, Some(2));
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(app.sb.selected, Some(1));
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.focus, "t1");
    }

/// D asks first (book §16, BISE-43): the status row says
    /// `drop {name}? …`; n and esc keep the agent; y sends the /drop.
    #[test]
    fn d_asks_before_dropping() {
        use std::io::Read;
        let (a, mut hub) = UnixStream::pair().unwrap();
        hub.set_nonblocking(true).unwrap();
        let (_tx, rx) = mpsc::channel::<String>();
        let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "ws".into());
        let mut app = sb_app(sb, rx, false, 100, crate::voice::Voice::live(false), "ws".into());
        let mut sent = move || {
            let mut buf = vec![0u8; 4096];
            match hub.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).to_string(),
                Err(_) => String::new(),
            }
        };
        app.sb.agents = vec![agent("main"), agent("docs")];
        app.sb.selected = Some(1);
        let status = |app: &App| status_text(app);
        // D: the question, nothing sent
        assert!(press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT));
        assert_eq!(status(&app).trim(), "drop docs? its history stays in archived. y / n");
        assert_eq!(key_mode(&app), crate::keybar::Mode::DropAsk);
        assert_eq!(sent(), "");
        // n keeps it, the composer stays empty
        assert!(press(&mut app, KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!status(&app).contains("drop docs?"));
        assert_eq!(sent(), "");
        // esc keeps it too
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert!(press(&mut app, KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.sb.drop_ask, None);
        assert_eq!(sent(), "");
        // y drops it
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert!(press(&mut app, KeyCode::Char('y'), KeyModifiers::NONE));
        let out = sent();
        assert!(out.contains(r#""op":"input""#) && out.contains("/drop docs"), "{out}");
        assert_eq!(app.ed.text, "");
        // main is never asked about
        app.sb.selected = Some(0);
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert_eq!(app.sb.drop_ask, None);
    }

    /// BISE-86 (C2 `undelivered`, book §13, §17): the hub could not
    /// deliver your message: your line ends with `✗`, a line says
    /// `✗ not delivered: {name} stopped. ⏎ send again · esc drop`; ⏎ on an
    /// empty composer sends it again (`@name` from another view), esc
    /// drops it; the question then goes away.
    #[test]
    fn a_message_not_delivered_is_marked_and_asks() {
        use std::io::Read;
        let (a, mut hub) = UnixStream::pair().unwrap();
        hub.set_nonblocking(true).unwrap();
        let (_tx, rx) = mpsc::channel::<String>();
        let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "ws".into());
        let mut app = sb_app(sb, rx, false, 100, crate::voice::Voice::live(false), "ws".into());
        let mut sent = move || {
            let mut buf = vec![0u8; 4096];
            match hub.read(&mut buf) {
                Ok(n) => String::from_utf8_lossy(&buf[..n]).to_string(),
                Err(_) => String::new(),
            }
        };
        let ev = parse_hub_line("undelivered : fix : d'abord \\: les tests").unwrap();
        assert!(matches!(&ev, Ev::Undelivered { name, text, open: true } if name == "fix" && text == "d'abord : les tests"));
        push_event(&mut app.events, &mut app.cache, Ev::You("d'abord : les tests".into(), Mark::Sent));
        push_event(&mut app.events, &mut app.cache, ev.clone());
        assert!(matches!(&app.events[0], Ev::You(_, Mark::Failed)));
        let rows = |app: &App| -> Vec<String> {
            app.events
                .iter()
                .flat_map(|e| crate::render::ev_lines(e, 80))
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
                .collect()
        };
        let r = rows(&app);
        assert!(r[0].trim_end().ends_with("d'abord : les tests ✗"), "{:?}", r);
        assert_eq!(r[1].trim(), "✗ not delivered: fix stopped. ⏎ send again · esc drop");
        // esc drops it: nothing sent, the question goes
        assert!(press(&mut app, KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(sent(), "");
        assert_eq!(rows(&app)[1].trim(), "✗ not delivered: fix stopped.");
        // a second one, from main's view: ⏎ sends it again to @fix
        push_event(&mut app.events, &mut app.cache, Ev::You("encore".into(), Mark::Sent));
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : encore").unwrap());
        assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        let out = sent();
        assert!(out.contains(r#""op":"input""#) && out.contains("@fix encore"), "{out}");
        assert!(matches!(app.events.last(), Some(Ev::You(t, Mark::Sent)) if t == "@fix encore"));
        // `@fix encore` is the line the user wrote in main's view: marked
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : encore").unwrap());
        assert!(matches!(app.events.iter().rev().nth(1), Some(Ev::You(t, Mark::Failed)) if t == "@fix encore"));
        // a message the feed does not show (an `@fix` line from another
        // view): it comes back, marked, before the question
        let n = app.events.len();
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : où ?").unwrap());
        assert!(matches!(&app.events[n], Ev::You(t, Mark::Failed) if t == "@fix où ?"));
        assert!(matches!(&app.events[n + 1], Ev::Undelivered { open: true, .. }));
        // not with a draft: ⏎ sends the draft as usual
        push_event(&mut app.events, &mut app.cache, parse_hub_line("undelivered : fix : x").unwrap());
        app.ed.text = "draft".into();
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!sent().contains(r#""text":"@fix x""#));
        assert!(app.events.iter().any(|e| matches!(e, Ev::Undelivered { text, open: true, .. } if text == "x")));
    }

    fn infos(app: &App) -> Vec<String> {
        app.events.iter().filter_map(|e| match e { Ev::Info(t) => Some(t.clone()), _ => None }).collect()
    }

    /// No undo (book §13, §17): ctrl+z and a typed /cancel only say so;
    /// the draft stays, nothing goes to the hub, the composer's own undo
    /// (cmd+z, ctrl+/) does not answer to ctrl+z.
    #[test]
    fn ctrl_z_and_cancel_say_no_undo() {
        let mut app = bench::test_app();
        app.ed.text = "draft".into();
        assert!(press(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL));
        let k = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert!(crate::editor::action(&k).is_none());
        assert_eq!(app.ed.text, "draft");
        assert_eq!(infos(&app), vec![NO_UNDO.to_string()]);
        assert!(NO_UNDO.starts_with("no undo: an agent may already have acted."));
        let out = handle_input(&mut app, "/cancel");
        assert!(matches!(&out[..], [Ev::Info(t)] if t == NO_UNDO));
        assert!(!crate::commands::COMMANDS.iter().any(|c| c.name == "/cancel"));
    }

    /// /theme switches the palette; /welcome and /theme are listed; the
    /// descriptions are lowercase and say "agent" (book §4).
    #[test]
    fn theme_and_welcome_commands() {
        use crate::theme_detect::{apply, Choice};
        let saved = |c: Choice| (apply(c), Ok(()));
        let ev = theme_command(Some("light"), saved);
        assert!(matches!(&ev, Ev::Info(t) if t == "theme: light."));
        assert_eq!(crate::theme::mode(), crate::theme::Mode::Light);
        let ev = theme_command(Some("dark"), saved);
        assert!(matches!(&ev, Ev::Info(t) if t == "theme: dark."));
        let ev = theme_command(Some("light"), |c| (apply(c), Err("disk full".into())));
        assert!(matches!(&ev, Ev::Warn(t) if t.contains("couldn't save it (disk full)")));
        assert!(matches!(theme_command(Some("blue"), saved), Ev::Warn(_)));
        assert!(matches!(theme_command(None, saved), Ev::Info(t) if t.starts_with("theme: light.")));
        for name in ["/theme", "/welcome"] {
            assert!(crate::commands::COMMANDS.iter().any(|c| c.name == name), "{name}");
        }
        for c in crate::commands::COMMANDS {
            assert!(!c.desc.chars().next().unwrap().is_uppercase(), "{}", c.desc);
            assert!(!c.desc.contains("task"), "{}", c.desc);
        }
    }

    /// Ctrl+R belongs to voice input: Alt+R (or '®', Option+R on a
    /// macOS terminal) answers the card with the composer text.
    #[test]
    fn alt_r_answers_the_card_and_ctrl_r_is_not_the_cards() {
        let mut app = bench::test_app();
        app.sb.cards = vec![Card {
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
        app.sb.agents = vec![agent("main"), agent("t1")];
        app.sb.selected = Some(1);
        app.ed.text = "hello".into();
        assert!(!press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.sb.focus, "main");
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
