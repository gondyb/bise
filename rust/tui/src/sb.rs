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

/// Everything that belongs to one feed.
pub(super) struct View {
    events: Vec<Ev>,
    cache: Vec<Option<EventRows>>,
    follow: bool,
    top: usize,
    max_top: usize,
    unseen: usize,
    tail_visible: bool,
    pending: bool,
    interrupt_requested: bool,
    last_line_at: Option<std::time::Instant>,
    input: String,
    cursor: usize,
}

impl View {
    fn new() -> View {
        View {
            events: Vec::new(),
            cache: Vec::new(),
            follow: true,
            top: 0,
            max_top: 0,
            unseen: 0,
            tail_visible: true,
            pending: false,
            interrupt_requested: false,
            last_line_at: None,
            input: String::new(),
            cursor: 0,
        }
    }
}

fn swap_feed(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.events, &mut v.events);
    std::mem::swap(&mut app.cache, &mut v.cache);
    std::mem::swap(&mut app.follow, &mut v.follow);
    std::mem::swap(&mut app.top, &mut v.top);
    std::mem::swap(&mut app.max_top, &mut v.max_top);
    std::mem::swap(&mut app.unseen, &mut v.unseen);
    std::mem::swap(&mut app.tail_visible, &mut v.tail_visible);
    std::mem::swap(&mut app.pending, &mut v.pending);
    std::mem::swap(&mut app.interrupt_requested, &mut v.interrupt_requested);
    std::mem::swap(&mut app.last_line_at, &mut v.last_line_at);
}

fn swap_draft(app: &mut App, v: &mut View) {
    std::mem::swap(&mut app.input, &mut v.input);
    std::mem::swap(&mut app.cursor, &mut v.cursor);
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

    /// The card shown (or answered by Ctrl+R): the chosen one while it
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

    fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.name == name)
    }
}

pub(super) static SB_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) const SB_COMMANDS: &[Cmd] = &[
    Cmd {
        name: "/new",
        desc: "créer une tâche : /new [-w] [nom:] objectif",
        args: true,
    },
    Cmd {
        name: "/drop",
        desc: "arrêter et archiver une tâche (et son worktree)",
        args: true,
    },
    Cmd {
        name: "/restore",
        desc: "rouvrir une tâche archivée",
        args: true,
    },
    Cmd {
        name: "/isolate",
        desc: "donner un worktree git à une tâche",
        args: true,
    },
    Cmd {
        name: "/rename",
        desc: "renommer une tâche",
        args: true,
    },
    Cmd {
        name: "/answer",
        desc: "répondre à une carte : /answer N texte",
        args: true,
    },
    Cmd {
        name: "/close",
        desc: "classer une carte sans répondre : /close N",
        args: true,
    },
    Cmd {
        name: "/cancel",
        desc: "annuler le dernier routage non livré",
        args: false,
    },
    Cmd {
        name: "/tasks",
        desc: "le tableau des tâches",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrompre le tour de l'agent affiché",
        args: false,
    },
    Cmd {
        name: "/compact",
        desc: "compacter la conversation de l'agent affiché",
        args: false,
    },
    Cmd {
        name: "/help",
        desc: "commandes et touches",
        args: false,
    },
    Cmd {
        name: "/quit",
        desc: "quitter (les agents continuent)",
        args: false,
    },
];

const KEYS_HELP: &str = "touches (compositeur vide) : Ctrl+J/K choisir une tâche · ⏎ entrer · Espace aperçu · D drop · Esc revenir à main · Alt+1…9 aller à la tâche N · Alt+0 main · Ctrl+G afficher/masquer la carte (Ctrl+A aussi, composer vide) · Ctrl+N/P carte suivante/précédente · Ctrl+R répondre à la carte avec le texte du composer · Ctrl+F carte en plein écran · Ctrl+X classer la carte · Ctrl+Z annuler le dernier routage · Ctrl+O shell dans le dossier de l'agent affiché";

/// The shell asked with Ctrl+O, if any.
pub(super) fn take_shell(app: &mut App) -> Option<String> {
    app.sb.as_mut().and_then(|sb| sb.shell.take())
}

/// Route one hub event.
/// Markers of the reader thread (not JSON): the hub went away / is back.
const HUB_DOWN: &str = "\u{0}hub-down";
const HUB_UP: &str = "\u{0}hub-up";

/// The hub is back (a new connection, `hello` sent): it replays every
/// feed, so the feeds start empty again. The focus and the drafts stay.
fn hub_reconnected(app: &mut App) {
    app.connected = true;
    app.events.clear();
    app.cache.clear();
    app.pending = false;
    app.interrupt_requested = false;
    app.follow = true;
    app.top = 0;
    app.unseen = 0;
    let Some(sb) = app.sb.as_mut() else { return };
    for v in sb.views.values_mut() {
        let draft = (std::mem::take(&mut v.input), v.cursor);
        *v = View::new();
        (v.input, v.cursor) = draft;
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
        "line" => ingest_for(app, &s("agent"), s("line")),
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
                Ev::Info("réponds y (oui) ou n (non), puis ⏎".into()),
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
        "hello" => {
            if let Some(sb) = app.sb.as_mut() {
                sb.workspace = s("workspace");
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

fn ingest_for(app: &mut App, agent: &str, line: String) {
    let Some(sb) = app.sb.as_mut() else { return };
    if sb.focus == agent {
        ingest_line(app, line);
        return;
    }
    let visible = line.contains("obs: assistant:") || line.starts_with("sb ");
    if visible && sb.ready {
        *sb.activity.entry(agent.to_string()).or_insert(0) += 1;
    }
    let mut view = sb.views.remove(agent).unwrap_or_else(View::new);
    swap_feed(app, &mut view);
    ingest_line(app, line);
    swap_feed(app, &mut view);
    if let Some(sb) = app.sb.as_mut() {
        sb.views.insert(agent.to_string(), view);
    }
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
    app.hist_idx = None;
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
        "/clear" => {
            app.events.clear();
            app.cache.clear();
            app.top = 0;
            app.follow = true;
            out.push(Ev::Info("affichage vidé".into()));
        }
        "/help" => {
            for c in SB_COMMANDS {
                out.push(Ev::Info(format!("{:<10} — {}", c.name, c.desc)));
            }
            out.push(Ev::Info(
                "@tâche texte — message direct à une tâche (@main depuis une tâche)".into(),
            ));
            out.push(Ev::Info(KEYS_HELP.into()));
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

/// Ctrl+R: the composer's text answers the current card, shown or not
/// (Enter still talks to the agent in focus). An empty composer only
/// acknowledges the cards that need no words (done, overlap).
fn answer_card(app: &mut App) {
    let text = app.input.trim().to_string();
    let Some(sb) = app.sb.as_mut() else { return };
    let Some((id, kind, agent)) = sb
        .current_card()
        .map(|c| (c.id, c.kind.clone(), c.agent.clone()))
    else {
        return;
    };
    let text = if text.is_empty() {
        if !matches!(kind.as_str(), "done" | "overlap") {
            let msg = format!("carte #{} (@{}) : tape ta réponse puis Ctrl+R", id, agent);
            push_event(&mut app.events, &mut app.cache, Ev::Warn(msg));
            return;
        }
        "vu".to_string()
    } else {
        text
    };
    let f = sb.focus.clone();
    sb.send(json!({"op": "input", "focus": f, "text": format!("/answer {} {}", id, text)}));
    sb.card.scroll = 0;
    sb.card.full = false;
    if !app.input.trim().is_empty() {
        app.history.insert(0, app.input.clone());
    }
    app.input.clear();
    app.cursor = 0;
    app.hist_idx = None;
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
        " ◆ carte {}/{} · {} #{} {} @{} · il y a {} ",
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

/// Keys of the switchboard mode; `true` when handled.
pub(super) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    let empty = app.input.is_empty();
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
                Ev::Info("interrompu — le tour s'arrête au prochain point sûr · Ctrl+C à nouveau pour quitter".into()),
            );
            true
        }
        (KeyCode::Char('j'), KeyModifiers::CONTROL) | (KeyCode::Down, KeyModifiers::ALT)
            if empty && n > 0 =>
        {
            sb.selected = Some(match sb.selected {
                None => 0,
                Some(i) => (i + 1) % n,
            });
            true
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) | (KeyCode::Up, KeyModifiers::ALT)
            if empty && n > 0 =>
        {
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
                let d = std::mem::take(&mut app.input);
                app.history.insert(0, d);
                app.cursor = 0;
                return true;
            }
            if sb.focus != "main" {
                focus(app, "main");
                return true;
            }
            false
        }
        (KeyCode::Char(c), KeyModifiers::ALT) if c.is_ascii_digit() => {
            let i = c.to_digit(10).unwrap_or(0) as usize;
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
        (KeyCode::Char('r'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
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
                " {} archivée{}",
                archived,
                if archived > 1 { "s" } else { "" }
            ),
            Style::default().fg(FAINT),
        )));
    }
    if !sb.cards.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" ◆ cartes ({}) · Ctrl+G", sb.cards.len()),
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
            " · dossier partagé".to_string(),
            Style::default().fg(DIM),
        ));
    }
    if let Some(ms) = a.turn_ms.filter(|_| app.pending) {
        spans.push(Span::styled(
            format!(" · {}s", ms / 1000),
            Style::default().fg(DIM),
        ));
    }
    if sb.focus != "main" {
        spans.push(Span::styled(
            " · tu parles directement à la tâche · Esc → main".to_string(),
            Style::default().fg(INFO),
        ));
    }
    if sb.preview {
        if let Some(sel) = sb
            .selected
            .and_then(|i| sb.nav().get(i).map(|a| a.name.clone()))
        {
            spans.push(Span::styled(
                format!(" · aperçu de @{} (⏎ entrer, Esc fermer)", sel),
                Style::default().fg(WARN),
            ));
        }
    }
    if !sb.cards.is_empty() && !sb.card.shown {
        spans.push(Span::styled(
            format!(
                " · ◆ {} carte{} · Ctrl+G",
                sb.cards.len(),
                if sb.cards.len() > 1 { "s" } else { "" }
            ),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ));
    }
    if !app.connected {
        spans.push(Span::styled(
            " · ○ hub déconnecté".to_string(),
            Style::default().fg(ERR),
        ));
    }
    Some(Line::from(spans))
}

pub(super) fn hint(app: &App) -> Option<&'static str> {
    let sb = app.sb.as_ref()?;
    Some(if sb.confirm.is_some() {
        "y oui · n non · Esc annuler"
    } else if sb.card.full {
        "Ctrl+R répondre · PgUp/PgDn défiler · Ctrl+N/P carte · Ctrl+X classer · Ctrl+F/Esc réduire · Ctrl+G masquer"
    } else if sb.card.shown {
        "Ctrl+R répondre (⏎ reste pour main) · PgUp/PgDn défiler · Ctrl+N/P carte · Ctrl+F plein écran · Ctrl+X classer · Ctrl+G masquer"
    } else if sb.selected.is_some() {
        "⏎ entrer · Espace aperçu · D drop · Ctrl+J/K choisir · Esc fermer"
    } else if app.pending {
        "⏎ diriger · Ctrl+C interrompre · Ctrl+J/K tâches · Esc main · /help"
    } else if sb.focus != "main" {
        "⏎ envoyer à la tâche · @main … pour main · Esc revenir à main · Ctrl+J/K tâches · /help"
    } else {
        "⏎ envoyer à main · @tâche … direct · Ctrl+J/K tâches · Ctrl+G carte · /help"
    })
}

pub(super) fn placeholder(app: &App) -> Option<String> {
    let sb = app.sb.as_ref()?;
    Some(if sb.focus == "main" {
        "Message à main…".to_string()
    } else {
        format!("Message direct à @{}…", sb.focus)
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
        None => draw(app, frame),
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
    if app.hist_idx.is_some() || app.popup_dismissed.as_deref() == Some(app.input.as_str()) {
        return Vec::new();
    }
    let Some(q) = mention_query(&app.input) else {
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
        "card-closed" => Ev::Info(format!("carte {} ", text)),
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
    let sb = Sb {
        writer,
        workspace: workspace.clone(),
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
    };
    let mut app = App {
        connected: true,
        debug,
        line_tools: std::collections::HashMap::new(),
        follow: true,
        top: 0,
        max_top: 0,
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
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
        input: String::new(),
        cursor: 0,
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        hist_idx: None,
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
