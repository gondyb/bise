//! bend-tui — Ratatui terminal UI for the Bend Unified Harness.
//!
//! Rust port of repl-tui (Ink). Same wire protocol, same layout spirit as
//! the Vibe CLI / OpenCode: header with model and connection state, a
//! per-turn event feed (tool lines merge start -> finish), animated
//! thinking indicator, raw-mode input box with history, status bar.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode so
//! the UI stays scriptable — the same convention as the Ink version.

use crossterm::event::{
    poll, read, EnableMouseCapture, DisableMouseCapture, Event, KeyCode,
    KeyEventKind, KeyModifiers, MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const BRAND: Color = Color::Cyan;
const DIM: Color = Color::Gray;
const YOU: Color = Color::Magenta;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;
const ERR: Color = Color::Red;

// ---- feed events ----

#[derive(Clone)]
enum ToolState {
    Run,
    Ok,
    Fail,
}

#[derive(Clone)]
enum Ev {
    You(String),
    Assistant(String),
    Tool { n: String, state: ToolState },
    Turn,
    TurnDone(String),
    Compact(String),
    Compacted(String),
    Warn(String),
    Err(String),
    Info(String),
    Idle,
    Raw(String),
}

fn parse_line(line: &str) -> Option<Ev> {
    if line.is_empty() {
        return None;
    }
    if line == "--- idle" {
        return Some(Ev::Idle);
    }
    if let Some(r) = line.strip_prefix("core rejected: ") {
        return Some(Ev::Err(r.to_string()));
    }
    let Some(o) = line.strip_prefix("  obs: ") else {
        return Some(Ev::Raw(line.to_string()));
    };
    if o == "turn_started" {
        return Some(Ev::Turn);
    }
    if let Some(t) = o.strip_prefix("assistant: ") {
        // tool-call-only replies carry no text
        return if t.is_empty() { None } else { Some(Ev::Assistant(t.to_string())) };
    }
    if o == "assistant:" {
        return None;
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        return Some(Ev::Tool { n: n.to_string(), state: ToolState::Run });
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let (n, tail) = rest.split_once(' ')?;
        let state = match tail {
            "ok" => ToolState::Ok,
            _ => ToolState::Fail,
        };
        return Some(Ev::Tool { n: n.to_string(), state });
    }
    if o.starts_with("tool_result_committed") {
        return None;
    }
    if let Some(t) = o.strip_prefix("steering_received: ") {
        return Some(Ev::Info(format!("steering reçu : {}", t)));
    }
    if let Some(t) = o.strip_prefix("steered: ") {
        return Some(Ev::Info(format!("steering transmis au modèle : {}", t)));
    }
    if let Some(t) = o.strip_prefix("notification_received: ") {
        return Some(Ev::Info(format!("notification : {}", t)));
    }
    if let Some(t) = o.strip_prefix("notification_delivered: ") {
        return Some(Ev::Info(format!("notification livrée au modèle : {}", t)));
    }
    if let Some(t) = o.strip_prefix("candidate_discarded: ") {
        return Some(Ev::Warn(format!("candidat écarté : {}", t)));
    }
    if let Some(t) = o.strip_prefix("compaction_started #") {
        return Some(Ev::Compact(t.to_string()));
    }
    if let Some(t) = o.strip_prefix("context_compaction_failed: ") {
        return Some(Ev::Err(format!("compaction échouée : {}", t)));
    }
    if let Some(t) = o.strip_prefix("session_restored: ") {
        return Some(Ev::Info(format!(
            "session restaurée · {} messages",
            t.trim_end_matches(" messages")
        )));
    }
    if let Some(t) = o.strip_prefix("compaction_done: ") {
        return Some(Ev::Compacted(t.to_string()));
    }
    if o == "null_iteration" {
        return Some(Ev::Warn("itération nulle comptée".into()));
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        let label = if t == "completed" {
            "tour terminé".to_string()
        } else if let Some(why) = t.strip_prefix("failed") {
            format!("tour échoué :{}", why)
        } else {
            "tour interrompu".to_string()
        };
        return Some(Ev::TurnDone(label));
    }
    Some(Ev::Raw(o.to_string()))
}

// structural annotations (turn separators, idle markers) are debug-only;
// messages, tool activity, compaction and errors always show
fn ev_visible(ev: &Ev, debug: bool) -> bool {
    if debug {
        return true;
    }
    !matches!(ev, Ev::Turn | Ev::TurnDone(_) | Ev::Idle | Ev::Raw(_))
}

fn push_event(events: &mut Vec<Ev>, ev: Ev) {
    // a tool finishing rewrites its running line
    if let Ev::Tool { n, state } = &ev {
        if !matches!(state, ToolState::Run) {
            for e in events.iter_mut().rev() {
                if let Ev::Tool { n: n2, state: s2 } = e {
                    if n2 == n && matches!(s2, ToolState::Run) {
                        *s2 = match state {
                            ToolState::Ok => ToolState::Ok,
                            ToolState::Fail => ToolState::Fail,
                            ToolState::Run => ToolState::Run,
                        };
                        return;
                    }
                }
            }
        }
    }
    events.push(ev);
}


// ---- markdown rendering (user + assistant messages) ----
// The wire carries newlines escaped as a literal backslash-n; the TUI
// unescapes and renders a pragmatic markdown subset: fenced code
// blocks, headers, bullet lists, blockquotes, and inline bold, italic
// and code.

fn unescape_md(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'n') {
            chars.next();
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

// inline styles: **bold**, *italic*, `code` (no nesting inside)
fn inline_spans(s: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        // `code`
        if c == '`' {
            if let Some(j) = (i + 1..cs.len()).find(|k| cs[*k] == '`') {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    Style::default().fg(Color::Green),
                ));
                i = j + 1;
                continue;
            }
        }
        // **bold** or *italic*
        if c == '*' {
            if i + 1 < cs.len() && cs[i + 1] == '*' {
                if let Some(j) = (i + 3..cs.len()).find(|k| cs[*k] == '*' && cs.get(k + 1) == Some(&'*')) {
                    if !plain.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut plain), base));
                    }
                    spans.push(Span::styled(
                        cs[i + 2..j].iter().collect::<String>(),
                        base.add_modifier(Modifier::BOLD),
                    ));
                    i = j + 2;
                    continue;
                }
            } else if let Some(j) = (i + 2..cs.len())
                .find(|k| cs[*k] == '*' && cs.get(k - 1) != Some(&'*') && cs.get(k + 1) != Some(&'*'))
            {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    base.add_modifier(Modifier::ITALIC),
                ));
                i = j + 1;
                continue;
            }
        }
        plain.push(c);
        i += 1;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, base));
    }
    spans
}

fn md_to_lines(text: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    for raw in text.split('\n') {
        let line = raw.trim_end();
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::Green),
            )));
            continue;
        }
        if line.is_empty() {
            out.push(Line::from(""));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim_start();
        let base = Style::default().fg(Color::White);
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            let head = t[level..].trim_start();
            out.push(Line::from(Span::styled(
                head.to_string(),
                base.add_modifier(Modifier::BOLD).fg(BRAND),
            )));
            continue;
        }
        if indent == 0 && (t.starts_with("- ") || t.starts_with("* ")) {
            out.push(Line::from_iter(std::iter::once(Span::styled(
                "  - ",
                Style::default().fg(DIM),
            )).chain(inline_spans(&t[2..], base))));
            continue;
        }
        if indent == 0 && t.starts_with("> ") {
            out.push(Line::from_iter(std::iter::once(Span::styled(
                "  | ",
                Style::default().fg(DIM),
            )).chain(inline_spans(&t[2..], Style::default().fg(DIM)))));
            continue;
        }
        out.push(Line::from(inline_spans(line, base)));
    }
    out
}

// one event renders as one or many lines (markdown expands messages)
fn ev_lines(ev: &Ev) -> Vec<Line<'static>> {
    match ev {
        Ev::You(t) => {
            let mut ls = md_to_lines(t);
            if let Some(first) = ls.first_mut() {
                let mut spans = vec![Span::styled(
                    "> ",
                    Style::default().fg(YOU).add_modifier(Modifier::BOLD),
                )];
                spans.append(&mut first.spans);
                *first = Line::from(spans);
            }
            ls
        }
        Ev::Assistant(t) => {
            let mut ls = md_to_lines(&unescape_md(t));
            if let Some(first) = ls.first_mut() {
                let mut spans = vec![Span::styled(
                    "  assistant ",
                    Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
                )];
                spans.append(&mut first.spans);
                *first = Line::from(spans);
            }
            for l in ls.iter_mut().skip(1) {
                let mut spans = vec![Span::raw("             ")];
                spans.append(&mut l.spans);
                *l = Line::from(spans);
            }
            ls
        }
        Ev::Compacted(t) => vec![Line::from(vec![
            Span::styled("  résumé ", Style::default().fg(OK).add_modifier(Modifier::BOLD)),
            Span::styled(unescape_md(t), Style::default().fg(DIM)),
        ])],
        Ev::Idle => vec![Line::from(Span::styled(
            "  inactif — session préservée",
            Style::default().fg(DIM),
        ))],
        _ => vec![ev_line(ev)],
    }
}

fn ev_line(ev: &Ev) -> Line<'static> {
    match ev {
        Ev::Idle => Line::from(""),
        Ev::You(t) => Line::from(Span::styled(
            t.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Ev::Assistant(t) => Line::from(Span::styled(
            t.clone(),
            Style::default().fg(Color::White),
        )),
        Ev::Tool { n, state } => {
            let (label, color) = match state {
                ToolState::Run => ("…", WARN),
                ToolState::Ok => ("ok", OK),
                ToolState::Fail => ("ECHEC", ERR),
            };
            Line::from(vec![
                Span::styled(format!("    outil #{} ", n), Style::default().fg(DIM)),
                Span::styled(label, Style::default().fg(color)),
            ])
        }
        Ev::Turn => Line::from(Span::styled(
            "── tour ".to_string() + &"─".repeat(30),
            Style::default().fg(DIM),
        )),
        Ev::TurnDone(t) => Line::from(vec![
            Span::styled(" └ ", Style::default().fg(DIM)),
            Span::styled(t.clone(), Style::default().fg(DIM).add_modifier(Modifier::ITALIC)),
        ]),
        Ev::Compact(t) => Line::from(vec![
            Span::styled("  compaction ", Style::default().fg(WARN).add_modifier(Modifier::BOLD)),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ]),
        Ev::Compacted(t) => Line::from(vec![
            Span::styled("  résumé accepté : ", Style::default().fg(OK)),
            Span::styled(t.clone(), Style::default().fg(DIM).add_modifier(Modifier::ITALIC)),
        ]),
        Ev::Warn(t) => Line::from(vec![
            Span::styled("  ! ", Style::default().fg(WARN)),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ]),
        Ev::Err(t) => Line::from(vec![
            Span::styled("  x ", Style::default().fg(ERR)),
            Span::styled(t.clone(), Style::default().fg(ERR)),
        ]),
        Ev::Info(t) => Line::from(Span::styled(format!("  {}", t), Style::default().fg(DIM).add_modifier(Modifier::ITALIC))),
        Ev::Raw(t) => Line::from(Span::styled(format!("  {}", t), Style::default().fg(DIM))),
    }
}

// ---- app state ----

struct App {
    connected: bool,
    debug: bool,
    // feed scrollback: rows from the bottom (0 = follow the newest line)
    scroll: usize,
    area_w: usize,
    area_h: usize,
    events: Vec<Ev>,
    pending: bool,
    input: String,
    cursor: usize, // char index into input
    popup_sel: usize,
    history: Vec<String>,
    hist_idx: Option<usize>,
    tick: u32,
    is_live: bool,
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    rx: Receiver<String>,
    should_quit: bool,
}

impl App {
    fn send(&mut self, line: &str) {
        self.pending = true;
        if let Some(s) = self.stream.as_mut() {
            let _ = s.write_all(format!("{}\n", line).as_bytes());
        }
    }
}

// ---- slash commands (codex-style) ----

struct Cmd {
    name: &'static str,
    desc: &'static str,
    args: bool,
}

const COMMANDS: &[Cmd] = &[
    Cmd { name: "/compact", desc: "compacter la conversation (résumé)", args: false },
    Cmd { name: "/interrupt", desc: "interrompre le tour en cours", args: false },
    Cmd { name: "/steer", desc: "diriger le modèle pendant le tour", args: true },
    Cmd { name: "/notify", desc: "injecter une notification au modèle", args: true },
    Cmd { name: "/status", desc: "modèle, connexion, seuil de compaction", args: false },
    Cmd { name: "/clear", desc: "vider l'affichage local", args: false },
    Cmd { name: "/help", desc: "liste des commandes", args: false },
    Cmd { name: "/quit", desc: "quitter le client (la session survit)", args: false },
];

fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    COMMANDS.iter().filter(|c| c.name.starts_with(input)).collect()
}

// byte offset of the n-th char (char-boundary-safe cursor helpers)
fn byte_at_char(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

// interprets one user line: slash command, raw protocol word, or plain
// message (implicit "say"). Returns the local events produced (echo
// included) so line mode can print them.
// Interprets one user line. The COMMAND LANGUAGE lives in the Bend REPL
// (core/commands.bend, pinned by LAWS.bend): plain text is an implicit
// "say", /commands map to protocol words, unknown ones get a server-side
// warning. This client only handles its own lifecycle and display.
fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    let mut out = vec![Ev::You(v.to_string())];
    app.history.insert(0, v.to_string());
    app.hist_idx = None;
    app.popup_sel = 0;

    let first = v.split_whitespace().next().unwrap_or("");
    let first = if first == "/exit" { "/quit" } else { first };

    if first == "/quit" {
        // client lifecycle: closing here, no server round-trip
        app.should_quit = true;
    } else if first == "/clear" {
        app.events.clear();
        out.push(Ev::Info("affichage vidé".into()));
    } else if first == "/status" {
        out.push(Ev::Info(format!(
            "modèle {} · {}:{} · seuil de compaction 800000 · session persistante",
            if app.is_live { "zai-glm-5-3" } else { "scripté" },
            app.host,
            app.port
        )));
    } else if first == "/help" {
        for c in COMMANDS {
            out.push(Ev::Info(format!("{:<11} — {}", c.name, c.desc)));
        }
        out.push(Ev::Info("texte simple : say implicite (interprété par le harness)".into()));
    } else {
        // everything else — plain text, /commands, raw protocol words —
        // goes to the harness verbatim; it interprets
        app.send(v.trim());
    }

    for ev in &out {
        push_event(&mut app.events, ev.clone());
    }
    out
}

// wrapped-row accounting for the scrollback
fn line_rows(l: &Line, inner_w: usize) -> usize {
    let w = l.width();
    if w == 0 {
        1
    } else {
        ((w as usize) + inner_w - 1) / inner_w
    }
    .max(1)
}

fn ev_rows(ev: &Ev, inner_w: usize) -> usize {
    let mut n = 0;
    for l in ev_lines(ev) {
        n += line_rows(&l, inner_w);
    }
    n
}

fn draw(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    // header
    let conn_style = if app.connected { Style::default().fg(OK) } else { Style::default().fg(ERR) };
    let conn_text = if app.connected { "connecté" } else if app.is_live { "déconnecté" } else { "déconnecté" };
    let header = Paragraph::new(Line::from(vec![
        Span::styled("BEND HARNESS", Style::default().fg(BRAND).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            format!("{} · live · seuil 800k · {}:{}", if app.is_live { "zai-glm-5-3" } else { "scripté" }, app.host, app.port),
            Style::default().fg(DIM),
        ),
        Span::raw("  "),
        Span::styled(conn_text, conn_style),
    ]))
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(if app.connected { BRAND } else { DIM })));
    frame.render_widget(header, chunks[0]);

    // feed: all visible lines, wrapped-row scrollback + scrollbar.
    // The paragraph column is one char narrower than the feed area so
    // the scrollbar never covers text.
    let area_w = (chunks[1].width as usize).saturating_sub(1).max(1);
    let area_h = chunks[1].height as usize;
    let text_area = Rect {
        x: chunks[1].x,
        y: chunks[1].y,
        width: chunks[1].width.saturating_sub(1).max(1),
        height: chunks[1].height,
    };
    let mut lines: Vec<Line> = Vec::new();
    for ev in app.events.iter() {
        if !ev_visible(ev, app.debug) {
            continue;
        }
        lines.extend(ev_lines(ev));
    }
    let mut total_rows = 0usize;
    for l in lines.iter() {
        total_rows += line_rows(l, area_w);
    }
    let max_scroll = total_rows.saturating_sub(area_h);
    let scroll = app.scroll.min(max_scroll);
    // 0 = follow the bottom; otherwise the window ends scroll rows earlier
    let start_row = max_scroll - scroll;
    let feed = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((start_row as u16, 0));
    frame.render_widget(feed, text_area);

    // scrollbar on the right edge of the feed
    if total_rows > area_h {
        let mut state = ScrollbarState::new(total_rows)
            .position(start_row)
            .viewport_content_length(area_h);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[1].inner(ratatui::layout::Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut state,
        );
    }
    // published for the input loop (paging, keep-in-place on new rows)
    app.area_w = area_w;
    app.area_h = area_h;

    // thinking indicator
    let think = if app.pending && app.connected {
        let dots = match (app.tick / 3) % 4 {
            0 => ".",
            1 => "..",
            2 => "...",
            _ => "  ",
        };
        Line::from(Span::styled(
            format!("  {} réfléchit{}", if app.is_live { "zai-glm-5-3" } else { "réponse" }, dots),
            Style::default().fg(BRAND),
        ))
    } else {
        Line::from("")
    };
    frame.render_widget(Paragraph::new(think), chunks[2]);

    // input box with a visible cursor at app.cursor (char index).
    // long inputs scroll horizontally: the window slides to keep the
    // cursor visible instead of clipping the right edge
    let chars: Vec<char> = app.input.chars().collect();
    let inner = ((chunks[3].width as usize).saturating_sub(4)).max(1); // borders + "> "
    let total = chars.len();
    let view = if total <= inner {
        0
    } else {
        (app.cursor + 1).saturating_sub(inner).min(total - inner)
    };
    let before: String = chars[view..app.cursor.min(total)].iter().collect();
    let at: String = chars.get(app.cursor).map(|c| c.to_string()).unwrap_or_default();
    let after: String = chars[(app.cursor + 1).min(total)..].iter().collect();
    let input = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(BRAND).add_modifier(Modifier::BOLD)),
        Span::raw(before),
        Span::styled(
            if at.is_empty() { " ".to_string() } else { at },
            Style::default().add_modifier(Modifier::REVERSED).fg(BRAND),
        ),
        Span::raw(after),
    ]))
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BRAND)));
    frame.render_widget(input, chunks[3]);

    // slash-command popup (filtered, codex-style) above the input box
    let matches = popup_matches(&app.input);
    if !matches.is_empty() {
        let n = matches.len().min(8) as u16;
        let w = 56u16.min(chunks[3].width);
        let area = Rect {
            x: chunks[3].x,
            y: chunks[3].y.saturating_sub(n + 2),
            width: w,
            height: n + 2,
        };
        frame.render_widget(Clear, area);
        let lines: Vec<Line> = matches
            .iter()
            .take(8)
            .enumerate()
            .map(|(i, c)| {
                let sel = i == app.popup_sel.min(matches.len() - 1);
                let style = if sel {
                    Style::default().fg(BRAND).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(BRAND)
                };
                Line::from(vec![
                    Span::styled(format!(" {:<10}", c.name), style),
                    Span::styled(c.desc, Style::default().fg(DIM)),
                ])
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BRAND)),
            ),
            area,
        );
    }

    // status bar
    let status = Line::from(vec![
        Span::styled(
            "Entrée : envoyer · / : commandes · Pg↑↓/molette : défiler · End : bas · Échap : interrompre · Ctrl+C : quitter",
            Style::default().fg(DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(status), chunks[4]);
}

fn run_tui(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
    loop {
        while let Ok(line) = app.rx.try_recv() {
            if line == "--- idle" {
                app.pending = false;
            }
            if let Some(ev) = parse_line(&line) {
                // scrolled up: pin the view — grow the offset by the
                // rows the new event adds at the bottom
                if app.scroll > 0 && ev_visible(&ev, app.debug) {
                    app.scroll += ev_rows(&ev, app.area_w);
                }
                push_event(&mut app.events, ev);
            }
        }
        if app.should_quit {
            break;
        }
        terminal.draw(|f| draw(app, f))?;
        if poll(Duration::from_millis(80))? {
            let ev = read()?;
            if let Event::Mouse(m) = ev {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        app.scroll += 3;
                    }
                    MouseEventKind::ScrollDown => {
                        app.scroll = app.scroll.saturating_sub(3);
                    }
                    _ => {}
                }
                continue;
            }
            if let Event::Key(k) = ev {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                let matches = popup_matches(&app.input);
                let popup_open = !matches.is_empty();
                let sel = if popup_open {
                    Some(matches[app.popup_sel.min(matches.len() - 1)])
                } else {
                    None
                };
                match (k.code, k.modifiers) {
                    // ctrl+c: clear input first, quit when already empty
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        if app.input.is_empty() {
                            break;
                        }
                        app.input.clear();
                        app.cursor = 0;
                        app.hist_idx = None;
                    }
                    // ctrl+l: clear the local feed
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        app.events.clear();
                    }
                    // esc: close popup, else interrupt the running turn
                    (KeyCode::Esc, _) => {
                        if popup_open {
                            app.input.clear();
                            app.cursor = 0;
                        } else if app.pending {
                            app.send("interrupt");
                        }
                    }
                    // scrollback: PgUp/PgDn page, End follows the bottom
                    (KeyCode::PageUp, _) => {
                        let page = (app.area_h / 2).max(1);
                        app.scroll += page;
                    }
                    (KeyCode::PageDown, _) => {
                        let page = (app.area_h / 2).max(1);
                        app.scroll = app.scroll.saturating_sub(page);
                    }
                    (KeyCode::End, _) => {
                        if app.scroll > 0 {
                            app.scroll = 0;
                        } else {
                            app.cursor = app.input.chars().count();
                        }
                    }
                    (KeyCode::Tab, _) => {
                        if let Some(c) = sel {
                            app.input = format!("{} ", c.name);
                            app.cursor = app.input.chars().count();
                            app.popup_sel = 0;
                        }
                    }
                    (KeyCode::Enter, _) => {
                        if let Some(c) = sel {
                            if c.args {
                                app.input = format!("{} ", c.name);
                                app.cursor = app.input.chars().count();
                                app.popup_sel = 0;
                            } else {
                                let v = c.name.to_string();
                                app.input.clear();
                                app.cursor = 0;
                                handle_input(app, &v);
                            }
                        } else {
                            let v = app.input.trim().to_string();
                            app.input.clear();
                            app.cursor = 0;
                            app.scroll = 0;
                            if !v.is_empty() {
                                handle_input(app, &v);
                            }
                        }
                    }
                    (KeyCode::Up, _) => {
                        if popup_open {
                            app.popup_sel =
                                (app.popup_sel + matches.len() - 1) % matches.len();
                        } else {
                            let next = match app.hist_idx {
                                None if !app.history.is_empty() => Some(0),
                                Some(i) if i + 1 < app.history.len() => Some(i + 1),
                                other => other,
                            };
                            if let Some(i) = next {
                                app.hist_idx = next;
                                app.input = app.history[i].clone();
                                app.cursor = app.input.chars().count();
                            }
                        }
                    }
                    (KeyCode::Down, _) => {
                        if popup_open {
                            app.popup_sel = (app.popup_sel + 1) % matches.len();
                        } else {
                            let next = match app.hist_idx {
                                Some(0) | None => None,
                                Some(i) => Some(i - 1),
                            };
                            app.hist_idx = next;
                            app.input = match next {
                                Some(i) => app.history[i].clone(),
                                None => String::new(),
                            };
                            app.cursor = app.input.chars().count();
                        }
                    }
                    // readline-style cursor movement
                    (KeyCode::Left, _) => {
                        app.cursor = app.cursor.saturating_sub(1);
                    }
                    (KeyCode::Right, _) => {
                        if app.cursor < app.input.chars().count() {
                            app.cursor += 1;
                        }
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                        app.cursor = 0;
                    }
                    (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                        app.cursor = app.input.chars().count();
                    }
                    (KeyCode::Backspace, _) => {
                        if app.cursor > 0 {
                            let b = byte_at_char(&app.input, app.cursor - 1);
                            let e = byte_at_char(&app.input, app.cursor);
                            app.input.replace_range(b..e, "");
                            app.cursor -= 1;
                        }
                    }
                    (KeyCode::Delete, _) => {
                        if app.cursor < app.input.chars().count() {
                            let b = byte_at_char(&app.input, app.cursor);
                            let e = byte_at_char(&app.input, app.cursor + 1);
                            app.input.replace_range(b..e, "");
                        }
                    }
                    // ctrl+w: delete the word before the cursor
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                        let e = byte_at_char(&app.input, app.cursor);
                        let head = app.input[..e].trim_end();
                        let b = head
                            .rfind(char::is_whitespace)
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        app.input.replace_range(b..e, "");
                        app.cursor = app.input[..b].chars().count();
                    }
                    (KeyCode::Char(c), m)
                        if m.is_empty() || m == KeyModifiers::SHIFT =>
                    {
                        let b = byte_at_char(&app.input, app.cursor);
                        app.input.insert(b, c);
                        app.cursor += 1;
                    }
                    _ => {}
                }
            }
        }
        app.tick = app.tick.wrapping_add(1);
    }
    let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
}

// ---- line mode (non-interactive stdin) ----

fn print_ev_of(ev: &Ev, debug: bool) {
    if !ev_visible(ev, debug) {
        return;
    }
    let mut out = io::stdout();
    for l in ev_lines(ev) {
        for span in l.spans.iter() {
            let _ = write!(out, "{}", span.content);
        }
        let _ = writeln!(out);
    }
}

fn print_ev(line: &str, debug: bool) {
    if let Some(ev) = parse_line(line) {
        print_ev_of(&ev, debug);
    }
}

// waits for the turn to finish ("--- idle") or the channel to close,
// printing every event as it arrives; gives up after `max`
fn wait_idle(app: &mut App, max: Duration) {
    let start = std::time::Instant::now();
    loop {
        match app.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if line == "--- idle" {
                    app.pending = false;
                }
                print_ev(&line, app.debug);
                if line == "--- idle" {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > max {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                app.connected = false;
                return;
            }
        }
    }
}

fn run_line_mode(app: &mut App) -> io::Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        // show anything the harness sent since the last command
        // (the --continue greeting arrives at connect)
        while let Ok(l) = app.rx.try_recv() {
            if l == "--- idle" {
                app.pending = false;
            }
            print_ev(&l, app.debug);
        }
        let line = line?;
        let v = line.trim().to_string();
        if v.is_empty() {
            continue;
        }
        let local = handle_input(app, &v);
        for ev in &local {
            print_ev_of(ev, app.debug);
        }
        if app.should_quit {
            break;
        }
        // a sent command runs a turn: wait for it to finish
        if app.pending {
            wait_idle(app, Duration::from_secs(120));
        } else {
            while let Ok(line) = app.rx.try_recv() {
                if line == "--- idle" {
                    app.pending = false;
                }
                print_ev(&line, app.debug);
            }
        }
    }
    Ok(())
}

/// Connect to the REPL and run the UI (interactive ratatui when stdin and
/// stdout are TTYs, line mode otherwise). `is_live` only affects the header.
pub fn run(host: String, port: u16, is_live: bool, debug: bool) -> io::Result<()> {

    let stream = TcpStream::connect((host.as_str(), port));
    let connected = true;
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            eprintln!("connexion impossible : {}", e);
            eprintln!(
                "lance d'abord le harness :  {}",
                if is_live { "bend runtime/repl-live.bend (et le bridge)" } else { "bend runtime/repl.bend" }
            );
            return Ok(());
        }
    };
    let reader = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut r = reader;
        let mut bytes: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match r.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    bytes.push(byte[0]);
                    while let Some(i) = bytes.iter().position(|b| *b == b'\n') {
                        let line = String::from_utf8_lossy(&bytes[..i]).trim_end().to_string();
                        bytes.drain(..=i);
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    let _ = stream.set_nodelay(true);

    let mut app = App {
        connected,
        debug,
        scroll: 0,
        area_w: 80,
        area_h: 24,
        events: Vec::new(),
        pending: false,
        input: String::new(),
        cursor: 0,
        popup_sel: 0,
        history: Vec::new(),
        hist_idx: None,
        tick: 0,
        is_live,
        host,
        port,
        stream: Some(stream),
        rx,
        should_quit: false,
    };

    // "connected" updates when the reader ends: reflect it via a probe
    // (the channel closes when the socket closes)
    {
        let tx_probe = app.stream.as_ref().map(|_| ());
        let _ = tx_probe;
    }

    let interactive = io::stdout().is_terminal() && io::stdin().is_terminal();
    if interactive {
        run_tui(&mut app)
    } else {
        run_line_mode(&mut app)
    }?;

    // final drain of the channel in line mode: detect close
    while let Ok(_) = app.rx.try_recv() {}
    let _ = connected;
    Ok(())
}
