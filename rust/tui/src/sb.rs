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

/// Events a feed keeps while it follows its tail; past it, the oldest
/// go (down to `KEEP_EVENTS`) and come back from the hub by pages when
/// the user scrolls up to them.
const MAX_EVENTS: usize = 3000;
const KEEP_EVENTS: usize = 2000;
/// Lines asked per page of older history.
const PAGE_LINES: usize = 1000;
/// A page is asked when the view gets this close to its first event.
const PAGE_AHEAD: usize = 100;

/// Which part of an agent's transcript a feed holds.
#[derive(Default)]
pub(super) struct FeedWindow {
    /// (event index, transcript position) of each line that added an
    /// event: where the feed can be cut, and where a page starts.
    marks: std::collections::VecDeque<(usize, usize)>,
    /// The position of the oldest line taken in (None: a hub without
    /// positions, nothing to page).
    first_pos: Option<usize>,
    /// A page was asked and has not arrived.
    loading: bool,
}

/// Everything that belongs to one feed.
pub(super) struct View {
    events: Vec<Ev>,
    cache: Vec<Option<EventRows>>,
    win: FeedWindow,
    follow: bool,
    anchor: (usize, usize),
    scroll: isize,
    unseen: usize,
    tail_visible: bool,
    pending: bool,
    interrupt_requested: bool,
    last_line_at: Option<std::time::Instant>,
    /// the agent's composer draft, kept while another is in focus
    ed: crate::editor::Editor,
}

impl View {
    fn new() -> View {
        View {
            events: Vec::new(),
            cache: Vec::new(),
            win: FeedWindow::default(),
            follow: true,
            anchor: (0, 0),
            scroll: 0,
            unseen: 0,
            tail_visible: true,
            pending: false,
            interrupt_requested: false,
            last_line_at: None,
            ed: crate::editor::Editor::default(),
        }
    }
}

fn swap_feed(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.events, &mut v.events);
    std::mem::swap(&mut app.cache, &mut v.cache);
    std::mem::swap(&mut app.win, &mut v.win);
    std::mem::swap(&mut app.follow, &mut v.follow);
    std::mem::swap(&mut app.anchor, &mut v.anchor);
    std::mem::swap(&mut app.scroll, &mut v.scroll);
    std::mem::swap(&mut app.unseen, &mut v.unseen);
    std::mem::swap(&mut app.tail_visible, &mut v.tail_visible);
    std::mem::swap(&mut app.pending, &mut v.pending);
    std::mem::swap(&mut app.interrupt_requested, &mut v.interrupt_requested);
    std::mem::swap(&mut app.last_line_at, &mut v.last_line_at);
}

fn swap_draft(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.ed, &mut v.ed);
}

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
}

#[derive(Clone)]
pub(super) struct Card {
    id: u64,
    kind: String,
    agent: String,
    text: String,
    /// The card's age when the snapshot arrived, and when it arrived.
    age_ms: u64,
    seen_at: std::time::Instant,
    /// The hub's remark (the asker heard from main since...).
    note: String,
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
    /// Lines that arrived in a feed out of view, since its last visit.
    activity: HashMap<String, usize>,
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
}

/// What the user sees of the attention cards: which one, shown or not,
/// full screen or not, and how far it is scrolled.
#[derive(Default)]
struct CardView {
    shown: bool,
    full: bool,
    sel: Option<u64>,
    scroll: usize,
    /// Set by the last draw: the last scroll offset and the page size.
    max_scroll: usize,
    page: usize,
}

/// One entry of the `/version` picker.
#[derive(Clone, Debug, Default)]
pub(super) struct VersionItem {
    rev: String,
    subject: String,
    marks: Vec<String>,
}

fn parse_versions(v: &Value) -> Vec<VersionItem> {
    let s = |x: &Value, k: &str| x.get(k).and_then(|y| y.as_str()).unwrap_or("").to_string();
    v.get("items")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|x| VersionItem {
                    rev: s(x, "rev"),
                    subject: s(x, "subject"),
                    marks: x
                        .get("marks")
                        .and_then(|m| m.as_array())
                        .map(|m| m.iter().filter_map(|y| y.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The text after `/version ` when the composer holds a version query.
fn version_query(input: &str) -> Option<&str> {
    input.strip_prefix("/version ").filter(|q| !q.contains('\n'))
}

/// The items matching `q` (in the revision or the subject).
fn filter_versions<'a>(items: &'a [VersionItem], q: &str) -> Vec<&'a VersionItem> {
    let q = q.trim().to_lowercase();
    items
        .iter()
        .filter(|i| {
            q.is_empty() || i.rev.to_lowercase().contains(&q) || i.subject.to_lowercase().contains(&q)
        })
        .collect()
}

/// The marks as words, and the glyph of the most telling one.
fn version_marks(marks: &[String]) -> (String, (&'static str, Color)) {
    let has = |m: &str| marks.iter().any(|x| x == m);
    let glyph = if has("building") {
        ("…", WARN)
    } else if has("failed") && !has("current") {
        ("✗", ERR)
    } else if has("current") {
        ("◉", BRAND)
    } else if has("good") {
        ("✓", INFO)
    } else if has("built") {
        ("●", TEXT)
    } else {
        ("○", DIM)
    };
    let words: Vec<&str> = marks
        .iter()
        .map(|m| match m.as_str() {
            "current" => "current",
            "trial" => "on trial",
            "good" => "last good",
            "built" => "built",
            "building" => "building…",
            "failed" => "failed",
            other => other,
        })
        .collect();
    (words.join(", "), glyph)
}

/// `/version <query>`: the picker of versions (commits, tree, back).
/// Enter builds if needed, then switches; Tab fills the composer.
pub(super) fn version_items(app: &App) -> Vec<PopItem> {
    let Some(sb) = app.sb.as_ref() else {
        return Vec::new();
    };
    if app.ed.browsing() || app.popup_dismissed.as_deref() == Some(app.ed.text.as_str()) {
        return Vec::new();
    }
    let Some(q) = version_query(&app.ed.text) else {
        return Vec::new();
    };
    // ask the hub for a fresh list (at most every 3 s while it is open)
    let stale = sb
        .versions_asked
        .get()
        .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(3));
    if stale {
        sb.versions_asked.set(Some(std::time::Instant::now()));
        if let Ok(mut w) = sb.writer.lock() {
            let _ = w.write_all(b"{\"op\":\"version\",\"do\":\"items\"}\n");
        }
    }
    if sb.versions.is_empty() {
        return vec![PopItem {
            label: "…".into(),
            desc: "loading the versions".into(),
            mark: None,
            fill: app.ed.text.clone(),
            fill_cursor: app.ed.cursor,
            run: None,
            closable: true,
        }];
    }
    filter_versions(&sb.versions, q)
        .into_iter()
        .map(|i| {
            let (words, glyph) = version_marks(&i.marks);
            let line = format!("/version {}", i.rev);
            PopItem {
                label: i.rev.clone(),
                desc: if words.is_empty() {
                    i.subject.clone()
                } else {
                    format!("[{}] {}", words, i.subject)
                },
                mark: Some(glyph),
                fill_cursor: line.chars().count(),
                fill: line.clone(),
                run: Some(line),
                closable: true,
            }
        })
        .collect()
}

impl Sb {
    fn send(&mut self, v: Value) {
        let mut s = v.to_string();
        s.push('\n');
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(s.as_bytes());
        }
    }

    /// The cards in reading order: what blocks a task first, then the
    /// oldest.
    fn sorted_cards(&self) -> Vec<&Card> {
        let rank = |k: &str| match k {
            "question" => 0,
            "blocked" => 1,
            "failed" | "restart" => 2,
            "drop" => 3,
            "overlap" => 4,
            _ => 5,
        };
        let mut v: Vec<&Card> = self.cards.iter().collect();
        v.sort_by_key(|c| (rank(&c.kind), c.id));
        v
    }

    /// The card shown (or answered by Alt+R): the chosen one while it
    /// is open, else the focused task's, else the first.
    fn current_card(&self) -> Option<&Card> {
        let v = self.sorted_cards();
        self.card
            .sel
            .and_then(|id| v.iter().find(|c| c.id == id).copied())
            .or_else(|| v.iter().find(|c| c.agent == self.focus).copied())
            .or_else(|| v.first().copied())
    }

    fn toggle_card(&mut self) {
        self.card.shown = !self.card.shown;
        self.card.full = false;
        if self.card.shown {
            self.card.sel = self.current_card().map(|c| c.id);
            self.card.scroll = 0;
        }
    }

    /// Ctrl+N / Ctrl+P: the next or previous card, shown.
    fn step_card(&mut self, d: isize) {
        let ids: Vec<u64> = self.sorted_cards().iter().map(|c| c.id).collect();
        if ids.is_empty() {
            return;
        }
        let cur = self.current_card().map(|c| c.id);
        let i = cur.and_then(|id| ids.iter().position(|x| *x == id)).unwrap_or(0) as isize;
        let n = ids.len() as isize;
        let j = if self.card.shown { (i + d).rem_euclid(n) } else { i };
        self.card.sel = Some(ids[j as usize]);
        self.card.shown = true;
        self.card.scroll = 0;
    }

    /// What the panel navigates: main, then the live tasks.
    fn nav(&self) -> Vec<&Agent> {
        self.agents
            .iter()
            .filter(|a| a.status != "archived")
            .collect()
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
        desc: "restart the hub safely (agents kept): /restart [<commit>|latest]",
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
        name: "/cancel",
        desc: "cancel the last undelivered route",
        args: false,
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
        desc: "commands and keys",
        args: false,
    },
    Cmd {
        name: "/quit",
        desc: "quit (the agents keep running)",
        args: false,
    },
];

const KEYS_HELP: &str = "keys (empty composer): Ctrl+K/J next/previous task · ⏎ enter · Space preview · D drop · Esc back to main · Alt+1…9 go to task N · Alt+0 main · Ctrl+G show/hide the card (also Ctrl+A, empty composer) · Ctrl+N/P next/previous card · Alt+R answer the card with the composer text · Ctrl+R voice input (/voice) · Ctrl+F card full screen · Ctrl+X close the card · Ctrl+Z cancel the last route · Ctrl+O shell in the folder of the agent in view";

/// The shell asked with Ctrl+O, if any.
pub(super) fn take_shell(app: &mut App) -> Option<String> {
    app.sb.as_mut().and_then(|sb| sb.shell.take())
}

/// Route one hub event.
/// The executable this TUI should re-exec as (the hub switched to
/// another version): taken by the caller once the terminal is restored.
static REEXEC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The hub runs `exe`: another binary than ours (and one that exists)
/// means another version, which this TUI must follow. Only in a
/// terminal: the line mode has nothing to keep.
fn follow_hub_exe(exe: &str) -> bool {
    let canon = |p: &std::path::Path| p.canonicalize().ok();
    let theirs = canon(std::path::Path::new(exe));
    let ours = std::env::current_exe().ok().and_then(|p| canon(&p));
    let differs = theirs.is_some() && theirs != ours;
    if differs && io::stdout().is_terminal() {
        if let Ok(mut r) = REEXEC.lock() {
            *r = Some(exe.to_string());
        }
        return true;
    }
    false
}

/// After `run_switchboard` returned: the binary to exec to follow the
/// hub's version, if it asked for one.
pub fn take_reexec() -> Option<String> {
    REEXEC.lock().ok().and_then(|mut r| r.take())
}

/// Markers of the reader thread (not JSON): the hub went away / is back.
const HUB_DOWN: &str = "\u{0}hub-down";
const HUB_UP: &str = "\u{0}hub-up";

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
    let focus = sb.focus.clone();
    sb.send(json!({"op": "focus", "focus": focus}));
}

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
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
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
            *sb.activity.entry(agent.to_string()).or_insert(0) += 1;
        }
    }
    with_feed(app, agent, |app| {
        ingest_at(app, line, pos);
        trim_window(app);
    });
}

/// Run `f` on the feed of `agent`, swapped into the `App` fields when it
/// is not the one in focus.
fn with_feed(app: &mut App, agent: &str, f: impl FnOnce(&mut App)) {
    let Some(sb) = app.sb.as_mut() else { return };
    if sb.focus == agent {
        f(app);
        return;
    }
    let mut view = sb.views.remove(agent).unwrap_or_else(View::new);
    swap_feed(app, &mut view);
    f(app);
    swap_feed(app, &mut view);
    if let Some(sb) = app.sb.as_mut() {
        sb.views.insert(agent.to_string(), view);
    }
}

/// One line of the feed, at transcript position `pos`.
fn ingest_at(app: &mut App, line: String, pos: Option<usize>) {
    let n0 = app.events.len();
    ingest_line(app, line);
    if let Some(p) = pos {
        app.win.first_pos.get_or_insert(p);
        if app.events.len() > n0 {
            app.win.marks.push_back((n0, p));
        }
    }
}

/// A feed that follows its tail keeps its last events only: the oldest
/// go, cut at a line boundary (they come back by pages).
fn trim_window(app: &mut App) {
    if !app.follow || app.events.len() <= MAX_EVENTS {
        return;
    }
    let want = app.events.len() - KEEP_EVENTS;
    let Some(&(k, pos)) = app.win.marks.iter().find(|(i, _)| *i >= want) else {
        return;
    };
    app.events.drain(..k);
    let c = k.min(app.cache.len());
    app.cache.drain(..c);
    if let Some(first) = app.cache.first_mut() {
        // its breathing gap depended on the event before it
        *first = None;
    }
    while app.win.marks.front().is_some_and(|(i, _)| *i < k) {
        app.win.marks.pop_front();
    }
    for m in app.win.marks.iter_mut() {
        m.0 -= k;
    }
    app.win.first_pos = Some(pos);
    app.anchor.0 = app.anchor.0.saturating_sub(k);
}

/// The view came close to the first event it holds: ask the hub for the
/// lines before it (one page at a time).
fn want_older(app: &mut App) {
    if app.follow || app.win.loading || app.anchor.0 >= PAGE_AHEAD {
        return;
    }
    let Some(before) = app.win.first_pos.filter(|p| *p > 1) else { return };
    let Some(sb) = app.sb.as_mut() else { return };
    let agent = sb.focus.clone();
    sb.send(json!({"op": "history", "agent": agent, "before": before, "count": PAGE_LINES}));
    app.win.loading = true;
}

/// A page of older lines arrived: its events go in front of the feed,
/// the view stays on the rows it shows.
fn prepend_page(app: &mut App, before: usize, lines: Vec<(usize, String)>) {
    if app.win.first_pos != Some(before) {
        // the feed changed since the ask (cut, reconnection): stale
        app.win.loading = false;
        return;
    }
    let first = lines.first().map_or(1, |l| l.0);
    // the page renders through the same path as live lines, on an empty
    // feed; what the live feed holds is set aside meanwhile
    let events = std::mem::take(&mut app.events);
    let cache = std::mem::take(&mut app.cache);
    let marks = std::mem::take(&mut app.win.marks);
    let kept = (
        app.follow,
        app.unseen,
        app.pending,
        app.interrupt_requested,
        app.last_line_at,
    );
    app.follow = true;
    for (pos, line) in lines {
        ingest_at(app, line, Some(pos));
    }
    let k = app.events.len();
    app.cache.resize_with(k, || None);
    app.events.extend(events);
    app.cache.extend(cache);
    // the old first event may gain its breathing gap: the view keeps
    // showing the same rows
    let mut shift = 0;
    if let Some(Some(old)) = app.cache.get(k) {
        let rows = event_rows(&app.events, k, app.debug, old.width as usize, app.tick);
        shift = rows.rows.len().saturating_sub(old.rows.len());
        app.cache[k] = Some(rows);
    }
    app.win.marks.extend(marks.into_iter().map(|(i, p)| (i + k, p)));
    (
        app.follow,
        app.unseen,
        app.pending,
        app.interrupt_requested,
        app.last_line_at,
    ) = kept;
    if app.anchor.0 == 0 {
        app.anchor.1 += shift;
    }
    app.anchor.0 += k;
    app.win.first_pos = Some(first);
    app.win.loading = false;
}

fn apply_state(app: &mut App, v: &Value) {
    let Some(sb) = app.sb.as_mut() else { return };
    let s = |x: &Value, k: &str| x.get(k).and_then(|y| y.as_str()).unwrap_or("").to_string();
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
    let busy: HashMap<String, bool> = sb
        .agents
        .iter()
        .map(|a| {
            (
                a.name.clone(),
                a.status == "working" || a.status == "waiting",
            )
        })
        .collect();
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
    sb.send(json!({"op": "focus", "focus": name}));
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
        "/clear" => {
            app.events.clear();
            app.cache.clear();
            app.anchor = (0, 0);
    app.scroll = 0;
            app.follow = true;
            out.push(Ev::Info("display cleared".into()));
        }
        "/help" => {
            for c in SB_COMMANDS {
                out.push(Ev::Info(format!("{:<10} — {}", c.name, c.desc)));
            }
            out.push(Ev::Info(
                "@task text — direct message to a task (@main from a task)".into(),
            ));
            out.push(Ev::Info(KEYS_HELP.into()));
            out.push(Ev::Info(crate::editor::EDIT_HELP.into()));
            out.push(Ev::Info(crate::editor::GHOSTTY_TIPS.into()));
            out.push(Ev::Info(crate::term::HELP.into()));
        }
        _ => {
            let focus = sb.focus.clone();
            sb.send(json!({"op": "input", "focus": focus, "text": typed}));
        }
    }
    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

/// Alt+R: the composer's text answers the current card, shown or not
/// (Enter still talks to the agent in focus). An empty composer only
/// acknowledges the cards that need no words (done, overlap).
fn answer_card(app: &mut App) {
    let text = app.ed.text.trim().to_string();
    let Some(sb) = app.sb.as_mut() else { return };
    let Some((id, kind, agent)) = sb
        .current_card()
        .map(|c| (c.id, c.kind.clone(), c.agent.clone()))
    else {
        return;
    };
    let text = if text.is_empty() {
        if !matches!(kind.as_str(), "done" | "overlap") {
            let msg = format!("card #{} (@{}): type your answer, then Alt+R", id, agent);
            push_event(&mut app.events, &mut app.cache, Ev::Warn(msg));
            return;
        }
        "seen".to_string()
    } else {
        text
    };
    let f = sb.focus.clone();
    sb.send(json!({"op": "input", "focus": f, "text": format!("/answer {} {}", id, text)}));
    sb.card.scroll = 0;
    sb.card.full = false;
    if !app.ed.text.trim().is_empty() {
        app.history.insert(0, app.ed.text.clone());
    }
    app.ed.take();
}

/// The height of the card box above the composer (0: hidden, or full
/// screen over the feed instead).
pub(super) fn card_box_height(app: &App, area: Rect) -> u16 {
    let Some(sb) = app.sb.as_ref() else { return 0 };
    if !sb.card.shown || sb.card.full {
        return 0;
    }
    let Some(c) = sb.current_card() else { return 0 };
    let w = (area.width as usize).saturating_sub(4).max(1);
    let rows = card_lines(c, w).len() as u16 + 2;
    let cap = (area.height * 35 / 100).max(5);
    rows.min(cap)
}

pub(super) fn card_full(app: &App) -> bool {
    app.sb
        .as_ref()
        .is_some_and(|sb| sb.card.shown && sb.card.full && !sb.cards.is_empty())
}

fn card_lines(c: &Card, width: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for l in c.text.lines() {
        out.extend(wrap_line(
            Line::from(Span::styled(l.to_string(), Style::default().fg(TEXT))),
            width,
        ));
    }
    if !c.note.is_empty() {
        out.push(Line::from(""));
        out.extend(wrap_line(
            Line::from(Span::styled(
                format!("ⓘ {}", c.note),
                Style::default().fg(INFO).add_modifier(Modifier::ITALIC),
            )),
            width,
        ));
    }
    out
}

fn ago(ms: u64) -> String {
    let s = ms / 1000;
    if s < 60 {
        format!("{} s", s)
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else {
        format!("{} h", s / 3600)
    }
}

/// The card box: the whole text, wrapped, scrolled by PgUp/PgDn.
pub(super) fn draw_card(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(sb) = app.sb.as_mut() else { return };
    if area.height < 3 {
        return;
    }
    let order: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
    let Some(c) = sb.current_card() else { return };
    let w = (area.width as usize).saturating_sub(4).max(1);
    let lines = card_lines(c, w);
    let visible = area.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    let pos = order.iter().position(|x| *x == c.id).unwrap_or(0) + 1;
    let (icon, color) = match c.kind.as_str() {
        "question" => ("?", WARN),
        "blocked" => (GLYPH_WARN, WARN),
        "failed" | "restart" => (GLYPH_ERR, ERR),
        "drop" => ("⇣", WARN),
        "overlap" => ("⚠", WARN),
        "done" => (GLYPH_OK, OK),
        _ => ("◆", WARN),
    };
    let age = ago(c.age_ms + c.seen_at.elapsed().as_millis() as u64);
    let title = format!(
        " ◆ card {}/{} · {} #{} {} @{} · {} ago ",
        pos,
        order.len(),
        icon,
        c.id,
        c.kind,
        c.agent,
        age
    );
    let scroll = sb.card.scroll.min(max_scroll);
    let more = if max_scroll > 0 {
        format!(
            " {}–{}/{} · PgUp/PgDn ",
            scroll + 1,
            (scroll + visible).min(lines.len()),
            lines.len()
        )
    } else {
        String::new()
    };
    sb.card.scroll = scroll;
    sb.card.max_scroll = max_scroll;
    sb.card.page = (visible / 2).max(1);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            truncate_chars(&title, (area.width as usize).saturating_sub(4)),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(more, Style::default().fg(DIM))).right_aligned())
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((scroll as u16, 0)),
        area,
    );
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
        _ if empty && n > 0 && nav_key(k) == Some(Nav::Next) => {
            sb.selected = Some(match sb.selected {
                None => 0,
                Some(i) => (i + 1) % n,
            });
            true
        }
        _ if empty && n > 0 && nav_key(k) == Some(Nav::Prev) => {
            sb.selected = Some(match sb.selected {
                None | Some(0) => n - 1,
                Some(i) => i - 1,
            });
            true
        }
        (KeyCode::Enter, KeyModifiers::NONE) if empty && sb.selected.is_some() => {
            let name = sb.nav()[sb.selected.unwrap().min(n - 1)].name.clone();
            focus(app, &name);
            true
        }
        (KeyCode::Char(' '), _) if empty && sb.selected.is_some() => {
            sb.preview = !sb.preview;
            true
        }
        (KeyCode::Char('D'), _) if empty && sb.selected.is_some() => {
            let a = sb.nav()[sb.selected.unwrap().min(n - 1)].clone();
            if !a.main {
                let f = sb.focus.clone();
                sb.send(json!({"op": "input", "focus": f, "text": format!("/drop {}", a.name)}));
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
        _ if matches!(nav_key(k), Some(Nav::Goto(_))) => {
            let Some(Nav::Goto(i)) = nav_key(k) else { return false };
            let target = sb.nav().get(i).map(|a| a.name.clone());
            if let Some(t) = target {
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
            sb.card.scroll = sb.card.scroll.saturating_sub(sb.card.page.max(1));
            true
        }
        (KeyCode::PageDown, _) if sb.card.shown && !popup_open => {
            sb.card.scroll = (sb.card.scroll + sb.card.page.max(1)).min(sb.card.max_scroll);
            true
        }
        (KeyCode::Char('x'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            if let Some(id) = sb.current_card().map(|c| c.id) {
                let f = sb.focus.clone();
                sb.send(json!({"op": "input", "focus": f, "text": format!("/close {}", id)}));
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
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            let f = sb.focus.clone();
            sb.send(json!({"op": "input", "focus": f, "text": "/cancel"}));
            true
        }
        _ => false,
    }
}

fn glyph(status: &str, tick: u32) -> (&'static str, Color) {
    match status {
        "working" => (spinner_frame(tick / 2), BRAND),
        "waiting" => ("◌", INFO),
        "starting" => ("…", DIM),
        "idle" => ("○", DIM),
        "done" => (GLYPH_OK, OK),
        "blocked" => (GLYPH_WARN, WARN),
        "failed" => (GLYPH_ERR, ERR),
        "stopped" => ("■", DIM),
        _ => ("·", FAINT),
    }
}

/// The feed and composer area, and the panel on the right when it fits.
/// The workspace folder (the embedded terminal starts there).
pub(super) fn workspace(app: &App) -> Option<String> {
    app.sb.as_ref().map(|sb| sb.workspace.clone()).filter(|w| !w.is_empty())
}

pub(super) fn split(app: &App, full: Rect) -> (Rect, Option<Rect>) {
    if app.sb.is_none() || full.width < 70 {
        return (full, None);
    }
    let w = (full.width / 4).clamp(28, 40);
    (
        Rect {
            width: full.width - w,
            ..full
        },
        Some(Rect {
            x: full.x + full.width - w,
            width: w,
            ..full
        }),
    )
}

pub(super) fn draw_panel(app: &App, frame: &mut Frame, area: Rect) {
    let Some(sb) = app.sb.as_ref() else { return };
    let w = area.width.saturating_sub(3) as usize;
    let mut lines: Vec<Line> = Vec::new();
    let ws = std::path::Path::new(&sb.workspace)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled(
            " Switchboard ",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            truncate_chars(&ws, w.saturating_sub(13)),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(""));
    for (i, a) in sb.nav().iter().enumerate() {
        let (g, gc) = glyph(&a.status, app.tick);
        let focused = a.name == sb.focus;
        let selected = sb.selected == Some(i);
        let mut name_style = Style::default().fg(if focused { BRAND } else { TEXT });
        if focused {
            name_style = name_style.add_modifier(Modifier::BOLD);
        }
        if selected {
            name_style = name_style.add_modifier(Modifier::REVERSED);
        }
        let label = if a.main {
            "main".to_string()
        } else {
            format!("{} {}", i, a.name)
        };
        let act = sb.activity.get(&a.name).copied().unwrap_or(0);
        let mut spans = vec![
            Span::styled(format!(" {} ", g), Style::default().fg(gc)),
            Span::styled(truncate_chars(&label, w.saturating_sub(12)), name_style),
            Span::styled(format!(" {}", a.status), Style::default().fg(DIM)),
        ];
        if let Some(u) = sb.usage_of(app, &a.name) {
            spans.push(Span::styled(format!(" {}", u.short()), Style::default().fg(FAINT)));
        }
        if act > 0 && !focused {
            spans.push(Span::styled(" •", Style::default().fg(INFO)));
        }
        if a.queued > 0 {
            spans.push(Span::styled(
                format!(" ✉{}", a.queued),
                Style::default().fg(WARN),
            ));
        }
        lines.push(Line::from(spans));
        if !a.main {
            let mut sub = truncate_chars(&a.objective, w.saturating_sub(3));
            if let Some(b) = &a.branch {
                sub = truncate_chars(&format!("⎇ {} · {}", b, a.objective), w.saturating_sub(3));
            }
            lines.push(Line::from(Span::styled(
                format!("   {}", sub),
                Style::default().fg(FAINT),
            )));
            if !a.note.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("   {}", truncate_chars(&a.note, w.saturating_sub(3))),
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )));
            }
        }
        if a.main && sb.nav().len() > 1 {
            lines.push(Line::from(Span::styled(
                format!(" {}", "─".repeat(w.saturating_sub(1))),
                Style::default().fg(FAINT),
            )));
        }
    }
    let archived = sb.agents.iter().filter(|a| a.status == "archived").count();
    if archived > 0 {
        lines.push(Line::from(Span::styled(
            format!(
                " {} archived",
                archived
            ),
            Style::default().fg(FAINT),
        )));
    }
    if !sb.cards.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" ◆ cards ({}) · Ctrl+G", sb.cards.len()),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        )));
        for c in &sb.cards {
            lines.push(Line::from(vec![
                Span::styled(format!(" #{} ", c.id), Style::default().fg(WARN)),
                Span::styled(
                    truncate_chars(&format!("{} @{}", c.kind, c.agent), w.saturating_sub(5)),
                    Style::default().fg(TEXT),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                format!("   {}", truncate_chars(&c.text, w.saturating_sub(3))),
                Style::default().fg(DIM),
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(FAINT)),
        ),
        area,
    );
}

/// The status row in switchboard mode (None: the plain one).
pub(super) fn status_line(app: &App) -> Option<Line<'static>> {
    let sb = app.sb.as_ref()?;
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let mut spans: Vec<Span<'static>> = Vec::new();
    if !sb.version.is_empty() {
        spans.push(Span::styled(
            format!("  v {}", sb.version.chars().take(24).collect::<String>()),
            Style::default().fg(DIM),
        ));
    }
    for i in &sb.versions {
        if i.marks.iter().any(|m| m == "building") {
            spans.push(Span::styled(format!("  ⧗ building {}", i.rev), Style::default().fg(WARN)));
        }
        if i.marks.iter().any(|m| m == "trial") {
            spans.push(Span::styled(format!("  ⧗ {} on trial", i.rev), Style::default().fg(WARN)));
        }
    }
    if app.pending {
        spans.push(Span::styled(
            format!("  {} ", spinner_frame(app.tick / 2)),
            Style::default().fg(BRAND),
        ));
    } else {
        spans.push(Span::styled("  ● ", Style::default().fg(BRAND)));
    }
    if sb.focus == "main" {
        spans.push(Span::styled(
            "main".to_string(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            format!("@{}", sb.focus),
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        format!(" · {}", a.status),
        Style::default().fg(DIM),
    ));
    if let Some(b) = &a.branch {
        spans.push(Span::styled(
            format!(" · ⎇ {}", b),
            Style::default().fg(DIM),
        ));
    } else if !a.main && !a.path.is_empty() && a.mode == "shared" {
        spans.push(Span::styled(
            " · shared folder".to_string(),
            Style::default().fg(DIM),
        ));
    }
    if let Some(u) = crate::usage::current(&app.events) {
        spans.push(Span::styled(format!(" · {}", u.label()), Style::default().fg(DIM)));
    }
    if let Some(ms) = a.turn_ms.filter(|_| app.pending) {
        spans.push(Span::styled(
            format!(" · {}s", ms / 1000),
            Style::default().fg(DIM),
        ));
    }
    if sb.focus != "main" {
        spans.push(Span::styled(
            " · you talk to the task directly · Esc → main".to_string(),
            Style::default().fg(INFO),
        ));
    }
    if sb.preview {
        if let Some(sel) = sb
            .selected
            .and_then(|i| sb.nav().get(i).map(|a| a.name.clone()))
        {
            spans.push(Span::styled(
                format!(" · preview of @{} (⏎ enter, Esc close)", sel),
                Style::default().fg(WARN),
            ));
        }
    }
    if !sb.cards.is_empty() && !sb.card.shown {
        spans.push(Span::styled(
            format!(
                " · ◆ {} card{} · Ctrl+G",
                sb.cards.len(),
                if sb.cards.len() > 1 { "s" } else { "" }
            ),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ));
    }
    if !app.connected {
        spans.push(Span::styled(
            " · ○ hub disconnected".to_string(),
            Style::default().fg(ERR),
        ));
    }
    Some(Line::from(spans))
}

pub(super) fn hint(app: &App) -> Option<&'static str> {
    let sb = app.sb.as_ref()?;
    Some(if sb.confirm.is_some() {
        "y yes · n no · Esc cancel"
    } else if sb.card.full {
        "Alt+R answer · PgUp/PgDn scroll · Ctrl+N/P card · Ctrl+X close · Ctrl+F/Esc shrink · Ctrl+G hide"
    } else if sb.card.shown {
        "Alt+R answer (⏎ still goes to main) · PgUp/PgDn scroll · Ctrl+N/P card · Ctrl+F full screen · Ctrl+X close · Ctrl+G hide"
    } else if sb.selected.is_some() {
        "⏎ enter · Space preview · D drop · Ctrl+K/J select · Esc close"
    } else if app.pending {
        "⏎ steer · Ctrl+C interrupt · Ctrl+K/J tasks · Alt+N° task N · Esc main · /help"
    } else if sb.focus != "main" {
        "⏎ send to the task · @main … for main · Esc back to main · Ctrl+K/J tasks · Alt+N° task N · /help"
    } else {
        "⏎ send to main · @task … direct · Ctrl+K/J tasks · Alt+N° task N · Ctrl+G card · /help"
    })
}

pub(super) fn placeholder(app: &App) -> Option<String> {
    let sb = app.sb.as_ref()?;
    Some(if sb.focus == "main" {
        "Message to main…".to_string()
    } else {
        format!("Direct message to @{}…", sb.focus)
    })
}

/// Draw, with the previewed feed swapped in when a preview is open.
pub(super) fn draw_sb(app: &mut App, frame: &mut Frame) {
    let target = app.sb.as_ref().and_then(|sb| {
        if !sb.preview {
            return None;
        }
        sb.selected
            .and_then(|i| sb.nav().get(i).map(|a| a.name.clone()))
            .filter(|n| *n != sb.focus)
    });
    match target {
        Some(name) => {
            let mut view = app
                .sb
                .as_mut()
                .and_then(|sb| sb.views.remove(&name))
                .unwrap_or_else(View::new);
            swap_feed(app, &mut view);
            draw(app, frame);
            swap_feed(app, &mut view);
            if let Some(sb) = app.sb.as_mut() {
                sb.views.insert(name, view);
            }
        }
        None => {
            draw(app, frame);
            want_older(app);
        }
    }
}

/// The `@` popup: the name being typed when the composer holds `@prefix`
/// and nothing else yet (routing only reads `@name` at line start).
pub(super) fn mention_query(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('@')?;
    if rest.contains(char::is_whitespace) {
        return None;
    }
    Some(rest)
}

/// The live agents a `@query` can name, the one in focus excluded:
/// case-insensitive prefix matches first, then substring matches, each
/// group in panel order.
fn filter_mentions<'a>(agents: &'a [Agent], focus: &str, query: &str) -> Vec<&'a Agent> {
    let q = query.to_lowercase();
    let live = agents
        .iter()
        .filter(|a| a.status != "archived" && a.name != focus && !a.name.is_empty());
    let (mut prefix, mut inner) = (Vec::new(), Vec::new());
    for a in live {
        let n = a.name.to_lowercase();
        if n.starts_with(&q) {
            prefix.push(a);
        } else if n.contains(&q) {
            inner.push(a);
        }
    }
    prefix.extend(inner);
    prefix
}

/// One entry of the `@` popup.
pub(super) struct Mention {
    pub(super) name: String,
    pub(super) status: String,
    pub(super) objective: String,
}

impl Mention {
    /// What the composer holds once the entry is picked.
    pub(super) fn completion(&self) -> String {
        format!("@{} ", self.name)
    }

    pub(super) fn glyph(&self, tick: u32) -> (&'static str, Color) {
        glyph(&self.status, tick)
    }
}

/// The `@` popup entries for the composer's current text (empty: closed).
pub(super) fn mentions(app: &App) -> Vec<Mention> {
    let Some(sb) = app.sb.as_ref() else {
        return Vec::new();
    };
    // a recalled history line is not a completion request
    if app.ed.browsing() || app.popup_dismissed.as_deref() == Some(app.ed.text.as_str()) {
        return Vec::new();
    }
    let Some(q) = mention_query(&app.ed.text) else {
        return Vec::new();
    };
    filter_mentions(&sb.agents, &sb.focus, q)
        .into_iter()
        .map(|a| Mention {
            name: a.name.clone(),
            status: a.status.clone(),
            objective: a.objective.clone(),
        })
        .collect()
}

/// The synthetic lines of the hub (`sb <kind> : <text>`) as feed events.
pub(super) fn parse_hub_line(rest: &str) -> Option<Ev> {
    let (kind, text) = rest.split_once(" : ").unwrap_or((rest, ""));
    let text = unescape_md(text);
    Some(match kind {
        "you" => Ev::You(text),
        "msg-in" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", text.as_str()));
            Ev::AgentMsg {
                head: head.to_string(),
                text: body.to_string(),
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

/// `bend-harness switchboard`: the client of a workspace's hub.
/// Read the hub's lines; when the hub goes away, say so and reconnect
/// (the socket path stays the same across hub restarts and version
/// switches), then swap the fresh stream into `writer`.
fn hub_reader(
    stream: UnixStream,
    socket: std::path::PathBuf,
    writer: std::sync::Arc<std::sync::Mutex<UnixStream>>,
    tx: mpsc::Sender<String>,
) {
    let mut stream = stream;
    loop {
        let mut r = io::BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match io::BufRead::read_line(&mut r, &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(line.trim_end().to_string()).is_err() {
                        return;
                    }
                }
            }
        }
        if tx.send(HUB_DOWN.to_string()).is_err() {
            return;
        }
        stream = loop {
            thread::sleep(std::time::Duration::from_millis(250));
            let Ok(mut s) = UnixStream::connect(&socket) else { continue };
            let Ok(w) = s.try_clone() else { continue };
            if s.write_all(b"{\"op\":\"hello\"}\n").is_err() {
                continue;
            }
            if let Ok(mut slot) = writer.lock() {
                *slot = w;
            }
            break s;
        };
        if tx.send(HUB_UP.to_string()).is_err() {
            return;
        }
    }
}

/// A fresh switchboard state over the hub connection `writer`.
fn new_sb(writer: std::sync::Arc<std::sync::Mutex<UnixStream>>, workspace: String) -> Sb {
    Sb {
        writer,
        workspace,
        focus: "main".to_string(),
        views: HashMap::new(),
        agents: Vec::new(),
        cards: Vec::new(),
        card: CardView::default(),
        selected: None,
        preview: false,
        confirm: None,
        activity: HashMap::new(),
        ready: false,
        shell: None,
        version: String::new(),
        versions: Vec::new(),
        versions_asked: std::cell::Cell::new(None),
    }
}

pub fn run_switchboard(
    stream: UnixStream,
    socket: std::path::PathBuf,
    workspace: String,
    debug: bool,
) -> io::Result<()> {
    SB_MODE.store(true, std::sync::atomic::Ordering::SeqCst);
    let reader = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<String>();
    let writer = std::sync::Arc::new(std::sync::Mutex::new(stream));
    {
        let writer = writer.clone();
        thread::spawn(move || hub_reader(reader, socket, writer, tx));
    }
    let sb = new_sb(writer, workspace.clone());
    let mut app = App {
        connected: true,
        term: crate::term::Term::default(),
        debug,
        line_tools: std::collections::HashMap::new(),
        follow: true,
        anchor: (0, 0),
        scroll: 0,
        vis_events: Vec::new(),
        vis_rows: Vec::new(),
        feed_x: 0,
        feed_sel: None,
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
        win: Default::default(),
        area_w: crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(100)
            .max(40),
        area_h: 24,
        events: Vec::new(),
        last_line_at: None,
        show_thinking: false,
        interrupt_requested: false,
        pending: false,
        ed: crate::editor::Editor::default(),
        composer: crate::ComposerArea::default(),
        flash: None,
        voice: super::voice::Voice::live(super::voice::load_voice_enabled()),
        voice_note: None,
        mouse: crate::MouseState::default(),
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        tick: 0,
        info: HarnessInfo {
            model: "switchboard".into(),
            threshold: String::new(),
            steer_path: String::new(),
            interrupt_path: String::new(),
        },
        host: String::new(),
        port: 0,
        session_id: workspace,
        stream: None,
        rx,
        should_quit: false,
        sb: Some(sb),
    };
    let interactive = io::stdout().is_terminal() && io::stdin().is_terminal();
    if interactive {
        run_tui(&mut app)
    } else {
        line_mode(&mut app)
    }
}

/// Without a terminal: stdin lines go to the agent in focus (`:focus
/// <agent>` changes it), every feed prints as `[agent] …`. Ends when
/// stdin is closed and every agent is idle.
fn line_mode(app: &mut App) -> io::Result<()> {
    let (itx, irx) = mpsc::channel::<Option<String>>();
    thread::spawn(move || {
        let stdin = io::stdin();
        for l in io::BufRead::lines(stdin.lock()).map_while(Result::ok) {
            if itx.send(Some(l)).is_err() {
                return;
            }
        }
        let _ = itx.send(None);
    });
    let mut stdin_open = true;
    let mut quiet_since: Option<std::time::Instant> = None;
    loop {
        while let Ok(raw) = app.rx.try_recv() {
            print_hub_event(&raw);
            dispatch(app, &raw);
        }
        while let Ok(l) = irx.try_recv() {
            match l {
                Some(l) => {
                    if let Some(f) = l.strip_prefix(":focus ") {
                        focus(app, f.trim());
                    } else if !l.trim().is_empty() {
                        handle_input(app, &l);
                    }
                    quiet_since = None;
                }
                None => stdin_open = false,
            }
        }
        let busy = app
            .sb
            .as_ref()
            .map(|sb| {
                sb.agents.iter().any(|a| {
                    a.status == "working" || a.status == "waiting" || a.status == "starting"
                })
            })
            .unwrap_or(false);
        if !stdin_open && !busy {
            let t = *quiet_since.get_or_insert_with(std::time::Instant::now);
            if t.elapsed() > Duration::from_secs(3) {
                return Ok(());
            }
        } else {
            quiet_since = None;
        }
        if app.should_quit {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn print_hub_event(raw: &str) {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    match s("ev").as_str() {
        "line" => {
            let line = s("line");
            let shown = if let Some(r) = line.strip_prefix("sb ") {
                Some(format!(
                    "[{}] {}",
                    s("agent"),
                    unescape_md(r).replace('\n', " ⏎ ")
                ))
            } else if let Some(r) = line.trim_start().strip_prefix("obs: assistant: ") {
                let (_, vis) = split_thinking(r).unwrap_or((String::new(), r.to_string()));
                Some(format!(
                    "[{}] assistant: {}",
                    s("agent"),
                    unescape_md(&vis).replace('\n', " ⏎ ")
                ))
            } else {
                line.strip_prefix("tool #")
                    .map(|r| format!("[{}] tool {}", s("agent"), truncate_chars(r, 200)))
            };
            if let Some(t) = shown {
                println!("{}", t);
            }
        }
        "notice" | "confirm" => println!("[hub] {}", s("text")),
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod bench;

#[cfg(test)]
mod version_picker_tests {
    use super::*;

    fn item(rev: &str, subject: &str, marks: &[&str]) -> VersionItem {
        VersionItem {
            rev: rev.into(),
            subject: subject.into(),
            marks: marks.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn the_query_is_the_text_after_version() {
        assert_eq!(version_query("/version "), Some(""));
        assert_eq!(version_query("/version ab1"), Some("ab1"));
        assert_eq!(version_query("/version"), None);
        assert_eq!(version_query("/versions x"), None);
    }

    #[test]
    fn the_filter_matches_the_revision_or_the_subject() {
        let items = vec![
            item("back", "roll back to 1234567", &[]),
            item("tree", "the working tree", &["current"]),
            item("abc1234", "tui: faster feed", &["built"]),
            item("def5678", "hub: journal", &["good", "built"]),
        ];
        let revs = |q: &str| -> Vec<String> {
            filter_versions(&items, q).iter().map(|i| i.rev.clone()).collect()
        };
        assert_eq!(revs("").len(), 4);
        assert_eq!(revs("abc"), vec!["abc1234"]);
        assert_eq!(revs("JOURNAL"), vec!["def5678"]);
        assert_eq!(revs("back"), vec!["back"]);
    }

    #[test]
    fn marks_read_as_words_and_one_glyph() {
        let (w, g) = version_marks(&["current".into(), "trial".into()]);
        assert_eq!(w, "current, on trial");
        assert_eq!(g.0, "◉");
        assert_eq!(version_marks(&["building".into()]).1 .0, "…");
        assert_eq!(version_marks(&["failed".into()]).1 .0, "✗");
        assert_eq!(version_marks(&[]).1 .0, "○");
    }

    #[test]
    fn the_hub_event_parses() {
        let v = json!({"ev": "versions", "items": [
            {"rev": "abc1234", "subject": "s", "marks": ["built", "good"]}]});
        let items = parse_versions(&v);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].marks, vec!["built", "good"]);
    }
}

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

#[cfg(test)]
mod mention_tests {
    use super::*;

    fn agent(name: &str, status: &str) -> Agent {
        Agent {
            name: name.into(),
            status: status.into(),
            ..Agent::default()
        }
    }

    fn names(v: Vec<&Agent>) -> Vec<&str> {
        v.into_iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn query_only_while_the_name_is_typed() {
        assert_eq!(mention_query("@"), Some(""));
        assert_eq!(mention_query("@be"), Some("be"));
        assert_eq!(mention_query("@bend-hub salut"), None);
        assert_eq!(mention_query("salut @be"), None);
        assert_eq!(mention_query("/new"), None);
        assert_eq!(mention_query(""), None);
    }

    #[test]
    fn filters_live_agents_prefix_first() {
        let agents = vec![
            agent("main", "idle"),
            agent("bend-hub", "working"),
            agent("parent-history", "working"),
            agent("at-complete", "working"),
            agent("old-bend", "archived"),
        ];
        // everything live but the focus
        assert_eq!(
            names(filter_mentions(&agents, "main", "")),
            ["bend-hub", "parent-history", "at-complete"]
        );
        // from a task, main is a candidate
        assert_eq!(
            names(filter_mentions(&agents, "bend-hub", "")),
            ["main", "parent-history", "at-complete"]
        );
        // prefix before substring, archived out, case-insensitive
        assert_eq!(names(filter_mentions(&agents, "main", "B")), ["bend-hub"]);
        assert_eq!(
            names(filter_mentions(&agents, "main", "a")),
            ["at-complete", "parent-history"]
        );
        assert_eq!(names(filter_mentions(&agents, "main", "his")), ["parent-history"]);
        assert!(filter_mentions(&agents, "main", "zzz").is_empty());
    }

    #[test]
    fn completion_inserts_the_name_and_a_space() {
        let m = Mention {
            name: "bend-hub".into(),
            status: "working".into(),
            objective: String::new(),
        };
        assert_eq!(m.completion(), "@bend-hub ");
        assert_eq!(mention_query(&m.completion()), None); // popup closes
    }
}
