//! bend-tui — Ratatui terminal UI for the Bend Unified Harness.
//!
//! Rust port of repl-tui (Ink). Same wire protocol; the presentation is
//! modeled on the REAL OpenCode TUI (packages/tui in the opencode repo):
//! no header bar — the screen is the conversation. Blocks breathe: a
//! blank line at every content transition, one column of margin on
//! each edge of the feed, blank rows separating the history from the
//! composer, and the user block paints its panel background the full
//! column. Status speaks in glyphs, not words: ✦ reasoning, ✓ ok,
//! ✗ fail, ▲ warning, ⟳ compaction, ≡ summary, ↳ preview. User
//! messages are
//! blocks with a colored left bar and a panel background; assistant
//! markdown renders in the OpenCode markdown colors; tool calls are
//! OpenCode inline tools (braille spinner while running, muted ✓ once
//! done, red ✗ on failure); the prompt is an OpenCode prompt (left
//! border, element background, agent/model meta row); commands filter
//! in an OpenCode autocomplete popup (split border, primary selection).
//! The status row carries the spinner + ctrl+c-to-interrupt hints.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode so
//! the UI stays scriptable — the same convention as the Ink version.

use crossterm::event::{
    poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
    EnableMouseCapture, Event, KeyCode, KeyboardEnhancementFlags, KeyEventKind,
    KeyModifiers, MouseButton, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

mod theme;
use theme::*;
mod wire;
use wire::*;
mod markdown;
use markdown::*;
mod code;
mod render;
use render::*;
mod feed;
use feed::*;
mod app;
use app::*;
mod sb;
mod skills;
mod emoji;
mod editor;
mod clipboard;
mod usage;
mod term;
mod feedsel;
mod keyprobe;
mod help;
mod voice;
#[cfg(test)]
mod voice_ui_tests;
#[cfg(test)]
mod composer_wrap_tests;
pub use keyprobe::keyprobe;
pub use sb::{run_switchboard, take_reexec};

// ---- slash commands (codex-style) ----

struct Cmd {
    name: &'static str,
    desc: &'static str,
    args: bool,
}

const COMMANDS: &[Cmd] = &[
    Cmd {
        name: "/compact",
        desc: "compact the conversation (summary)",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrupt the current turn",
        args: false,
    },
    Cmd {
        name: "/reload",
        desc: "restart the harness with the latest code (session kept)",
        args: false,
    },
    Cmd {
        name: "/status",
        desc: "model, connection, compaction threshold",
        args: false,
    },
    Cmd {
        name: "/clear",
        desc: "clear the local display",
        args: false,
    },
    Cmd {
        name: "/voice",
        desc: "turn voice mode (Ctrl+R speech-to-text) on or off",
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
        desc: "quit the client (the session survives)",
        args: false,
    },
];

// BR-002/BR-003: the interrupt side-channel flag, shared by the Ctrl+C
// key and the /interrupt command
/// What the Bend REPL announces at startup (its `harness-info` line):
/// the REPL is the single source of truth, the TUI never recomputes
/// the model, the threshold or the side-channel paths.
#[derive(Clone, Debug, Default)]
pub struct HarnessInfo {
    pub model: String,
    pub threshold: String,
    pub steer_path: String,
    pub interrupt_path: String,
}

impl HarnessInfo {
    /// Parse `harness-info model=M threshold=N steer=P interrupt=Q`.
    pub fn parse(line: &str) -> Option<HarnessInfo> {
        let rest = line.trim().strip_prefix("harness-info ")?;
        let mut info = HarnessInfo::default();
        for kv in rest.split_whitespace() {
            let (k, v) = kv.split_once('=')?;
            match k {
                "model" => info.model = v.to_string(),
                "threshold" => info.threshold = v.to_string(),
                "steer" => info.steer_path = v.to_string(),
                "interrupt" => info.interrupt_path = v.to_string(),
                _ => {}
            }
        }
        if info.model.is_empty() || info.steer_path.is_empty() || info.interrupt_path.is_empty() {
            return None;
        }
        Some(info)
    }

    /// Find the line in a REPL log.
    pub fn from_log(log: &str) -> Option<HarnessInfo> {
        log.lines().find_map(HarnessInfo::parse)
    }
}

fn write_interrupt_flag(path: &str) -> bool {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .and_then(|mut f| f.write_all(b"1"))
        .is_ok()
}

fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    let list = if sb::SB_MODE.load(std::sync::atomic::Ordering::SeqCst) {
        sb::SB_COMMANDS
    } else {
        COMMANDS
    };
    list.iter().filter(|c| c.name.starts_with(input)).collect()
}

/// One entry of the composer popup: a slash command, or an agent name
/// after `@` (switchboard mode).
struct PopItem {
    label: String,
    desc: String,
    /// status glyph (mentions)
    mark: Option<(&'static str, Color)>,
    /// the composer text once picked with Tab (or Enter when `run` is
    /// None), and the cursor in it
    fill: String,
    fill_cursor: usize,
    /// Enter runs this line directly (commands without arguments)
    run: Option<String>,
    /// Esc closes the list and keeps the text (`@` and `$`); the slash
    /// popup clears the draft instead
    closable: bool,
}

fn popup_items(app: &App) -> Vec<PopItem> {
    let cmds = popup_matches(&app.ed.text);
    if !cmds.is_empty() {
        return cmds
            .into_iter()
            .map(|c| PopItem {
                label: c.name.to_string(),
                desc: c.desc.to_string(),
                mark: None,
                fill: format!("{} ", c.name),
                fill_cursor: c.name.chars().count() + 1,
                run: (!c.args).then(|| c.name.to_string()),
                closable: false,
            })
            .collect();
    }
    let versions = sb::version_items(app);
    if !versions.is_empty() {
        return versions;
    }
    let mentions = sb::mentions(app);
    if mentions.is_empty() {
        let skills = skill_items(app);
        return if skills.is_empty() { emoji_items(app) } else { skills };
    }
    mentions
        .into_iter()
        .map(|m| PopItem {
            label: format!("@{}", m.name),
            desc: if m.objective.is_empty() {
                m.status.clone()
            } else {
                format!("{} · {}", m.status, m.objective)
            },
            mark: Some(m.glyph(app.tick)),
            fill_cursor: m.completion().chars().count(),
            fill: m.completion(),
            run: None,
            closable: true,
        })
        .collect()
}

/// `$skill` anywhere in the draft: the skills of the index.
fn skill_items(app: &App) -> Vec<PopItem> {
    if app.ed.browsing() || app.popup_dismissed.as_deref() == Some(app.ed.text.as_str()) {
        return Vec::new();
    }
    let Some((start, q)) = skills::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    let all = skills::index();
    skills::filter(&all, &q)
        .into_iter()
        .map(|s| {
            let (fill, fill_cursor) = skills::complete(&app.ed.text, start, app.ed.cursor, &s.name);
            PopItem {
                label: format!("${}", s.name),
                desc: s.desc.clone(),
                mark: None,
                fill,
                fill_cursor,
                run: None,
                closable: true,
            }
        })
        .collect()
}

/// `:name` anywhere in the draft: the matching emojis (emoji.rs).
fn emoji_items(app: &App) -> Vec<PopItem> {
    if app.ed.browsing() || app.popup_dismissed.as_deref() == Some(app.ed.text.as_str()) {
        return Vec::new();
    }
    let Some((start, q)) = emoji::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    emoji::filter(&q)
        .into_iter()
        .map(|(e, name)| {
            let (fill, fill_cursor) = emoji::complete(&app.ed.text, start, app.ed.cursor, e.glyph);
            PopItem {
                label: format!(":{}:", name),
                desc: e.desc.to_string(),
                mark: Some((e.glyph, TEXT)),
                fill,
                fill_cursor,
                run: None,
                closable: true,
            }
        })
        .collect()
}

/// First visible row of a popup of `len` entries showing `rows`, so the
/// selection `sel` stays in view.
fn popup_top(sel: usize, len: usize, rows: usize) -> usize {
    if len <= rows {
        0
    } else {
        sel.min(len - 1).saturating_sub(rows - 1)
    }
}

// interprets one user line: slash command, raw protocol word, or plain
// message (implicit "say"). Returns the local events produced (echo
// included) so line mode can print them.
// Interprets one user line. The COMMAND LANGUAGE lives in the Bend REPL
// (core/commands.bend, pinned by LAWS.bend): plain text is an implicit
// "say", /commands map to protocol words, unknown ones get a server-side
// warning. This client only handles its own lifecycle and display.
fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    if v.trim() == "/voice" {
        let ev = toggle_voice(app);
        push_event(&mut app.events, &mut app.cache, ev.clone());
        return vec![ev];
    }
    if app.sb.is_some() {
        return sb::handle_input(app, v);
    }
    // the steer/say wrappers are transport, not what the user typed
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v);
    let mut out = vec![Ev::You(typed.to_string())];
    app.history.insert(0, v.to_string());
    app.popup_sel = 0;

    let first = v.split_whitespace().next().unwrap_or("");
    let first = if first == "/exit" { "/quit" } else { first };

    if first == "/quit" {
        // client lifecycle: closing here, no server round-trip
        app.should_quit = true;
    } else if first == "/clear" {
        app.events.clear();
        app.cache.clear();
        app.anchor = (0, 0);
        app.scroll = 0;
        app.follow = true;
        app.unseen = 0;
        out.push(Ev::Info("display cleared".into()));
    } else if first == "/status" {
        out.push(Ev::Info(format!(
            "model {} · {}:{} · compaction threshold {} · session {}",
            app.info.model,
            app.host,
            app.port,
            app.info.threshold,
            app.session_id
        )));
    } else if let Some(page) = help::page_of(first) {
        app.help = Some(help::Overlay::new(page));
    } else if first == "/interrupt" {
        // BR-003: the socket is only read between turns, so sending the
        // line to the harness could never interrupt anything - the
        // command goes through the same flag file as Ctrl+C while a
        // turn runs, and says so at idle
        if app.pending {
            let ok = write_interrupt_flag(&app.info.interrupt_path);
            app.pending = false;
            app.interrupt_requested = true;
            out.push(Ev::Info(if ok {
                "interrupted — the current turn stops at the next safe point".into()
            } else {
                "interrupt not written (side channel unreachable)".into()
            }));
        } else {
            out.push(Ev::Info("no turn in progress to interrupt".into()));
        }
    } else if first == "steer" && app.pending {
        // mid-turn steering goes through the FILE side-channel: the
        // harness reads the socket only between turns, but the runtime
        // drains the announced steer file at every model/tool safe
        // boundary and commits the text into the running turn (ADR 0005)
        let msg = v.strip_prefix("steer ").map(|s| s.trim()).unwrap_or(typed);
        if msg.is_empty() {
            out.push(Ev::Info("steering vide".into()));
        } else {
            let path = app.info.steer_path.clone();
            let mut line = String::with_capacity(msg.len() + 1);
            line.push_str(msg);
            line.push('\n');
            let ok = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(line.as_bytes()))
                .is_ok();
            out.push(Ev::Info(if ok {
                format!("steering queued: {}", msg)
            } else {
                "steering not written (side channel unreachable)".to_string()
            }));
        }
    } else {
        // everything else — plain text, /commands, raw protocol words —
        // goes to the harness verbatim; it interprets
        app.send(v.trim());
    }

    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

/// The meter glyph: the level while recording, the fill spinner while
/// the last words are flushed.
fn voice_glyph(v: &voice::Voice) -> char {
    match v.flushing_since() {
        Some(at) if v.state() == voice::VoiceState::Flushing => {
            voice::flush_glyph(at.elapsed().as_millis())
        }
        _ => voice::peak_glyph(v.peak()),
    }
}

/// The composer's text rows while recording: the meter before the first
/// row, the rows indented after it, the text dimmed (Vibe's `recording`
/// input class).
fn recording_lines(lines: Vec<Line<'static>>, glyph: char) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let lead = if i == 0 {
                Span::styled(
                    format!("{} ", glyph),
                    Style::default().fg(RECORDING).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![lead];
            spans.extend(l.spans.into_iter().map(|s| {
                let st = s.style.fg(DIM);
                Span::styled(s.content, st)
            }));
            Line::from(spans)
        })
        .collect()
}

fn draw(app: &mut App, frame: &mut Frame) {
    let full = app.term.draw(frame, frame.area());
    let (area, sb_panel) = sb::split(app, full);
    if let Some(p) = sb_panel {
        sb::draw_panel(app, frame, p);
    }
    // OpenCode layout: no header. Feed grows to fill, a blank row, the
    // status row, another blank row, the prompt block, a blank row,
    // then the hint row — the composer never touches the history.
    // the composer grows with its content (a pasted multi-line block),
    // capped at half the screen so the feed always survives
    // recording: the level meter takes the first 2 columns (Vibe puts
    // it in place of the prompt), the text is indented after it
    let voice_pad = if app.voice.active() { 2 } else { 0 };
    let inner_w = ((area.width as usize).saturating_sub(6 + voice_pad)).max(1);
    // rows as drawn (same width, same end-slot rule as the draw below):
    // wrapped by display width (emojis are 2 columns)
    let composer_rows = {
        let rows = editor::layout_input(&app.ed.text, inner_w);
        editor::drawn_rows(&rows, app.ed.cursor)
    };
    // the prompt block holds: 2 rows of top padding, the typed text,
    // one blank line, the meta row, 1 row of bottom padding
    let input_h = ((composer_rows + 5) as u16).min((area.height / 2).max(7));
    let card_h = sb::card_box_height(app, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3), // feed
            Constraint::Length(1), // respiration sous le feed
            Constraint::Length(1), // status row
            Constraint::Length(1), // respiration au-dessus du composeur
            Constraint::Length(card_h), // la carte affichée (Ctrl+G)
            Constraint::Length(input_h), // prompt
            Constraint::Length(1), // respiration au-dessus de l'aide
            Constraint::Length(1), // hint row
        ])
        .split(area);

    // ---- feed: cached wrapped rows, only the VISIBLE slice rendered ----
    // (a Paragraph over the whole history re-wraps everything each frame
    // and lags long sessions). The column keeps one column of margin on
    // each edge: the history never touches the screen border, and the
    // scrollbar gets its own gutter.
    let feed_w = (chunks[0].width as usize).saturating_sub(3).max(1);
    let area_w = feed_w;
    let area_h = chunks[0].height as usize;
    let text_area = Rect {
        x: chunks[0].x + 1,
        y: chunks[0].y,
        width: feed_w as u16,
        height: chunks[0].height,
    };

    let n = app.events.len();
    if app.cache.len() < n {
        app.cache.resize_with(n, || None);
    }
    let (debug, tick) = (app.debug, app.tick);
    macro_rules! rows_of {
        () => {
            &mut |i: usize| ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick)
        };
    }
    let down = app.scroll > 0;
    let mut anchor = if app.follow {
        bottom_anchor(n, area_h, rows_of!())
    } else {
        move_anchor(app.anchor, app.scroll, n, rows_of!())
    };
    app.scroll = 0;
    // the rows from the anchor down; fewer than the screen: the bottom
    let mut vis: Vec<Line> = Vec::with_capacity(area_h + 2);
    let mut vis_events: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut vis_rows: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut tail_visible = true;
    for pass in 0..2 {
        vis.clear();
        vis_events.clear();
        vis_rows.clear();
        tail_visible = true;
        let (mut i, mut skip) = anchor;
        while i < n {
            ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick);
            let rows = app.cache[i].as_ref().map(|c| &c.rows[..]).unwrap_or(&[]);
            for (ri, r) in rows.iter().enumerate().skip(skip) {
                if vis.len() >= area_h {
                    tail_visible = false;
                    break;
                }
                // the feed selection on the selection background
                match app.feed_sel.and_then(|s| s.cols(i, ri)) {
                    Some((a, b)) => vis.push(feedsel::highlight(r, a, b, SELECTION)),
                    None => vis.push(r.clone()),
                }
                vis_events.push(i);
                vis_rows.push(ri);
            }
            skip = 0;
            if !tail_visible {
                break;
            }
            i += 1;
        }
        // a full screen that shows the last row is the tail too
        if pass == 0 && vis.len() < area_h && anchor != (0, 0) {
            anchor = bottom_anchor(n, area_h, rows_of!());
            continue;
        }
        break;
    }
    if tail_visible && down && !app.follow {
        app.follow = true;
        app.unseen = 0;
    }
    // nothing above the anchor: the view shows the top of the feed
    let at_top = anchor.1 == 0
        && !app.events[..anchor.0.min(n)]
            .iter()
            .rev()
            .any(|e| ev_visible(e, app.debug));
    frame.render_widget(Paragraph::new(Text::from(vis)), text_area);

    if !(at_top && tail_visible) && n > 0 {
        // the scrollbar counts events, not rows: the rows of the whole
        // history are never summed
        let shown = vis_events.last().map_or(1, |l| l + 1 - anchor.0.min(*l));
        let pos = if tail_visible { n - 1 } else { anchor.0 };
        let mut state = ScrollbarState::new(n)
            .position(pos)
            .viewport_content_length(shown);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[0],
            &mut state,
        );
    }
    app.anchor = anchor;
    app.vis_events = vis_events;
    app.vis_rows = vis_rows;
    app.feed_x = text_area.x;
    app.area_w = area_w;
    app.area_h = area_h;
    app.tail_visible = tail_visible;

    // ---- the status row (the OpenCode prompt status row): back to
    // bottom when pinned, else spinner + cwd while idle
    if !app.tail_visible {
        app.bottom_bar_rect = Some(chunks[2]);
        let mut spans = vec![
            Span::styled(
                "  ↓ Bas ",
                Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
            ),
            Span::styled("(End)", Style::default().fg(DIM)),
        ];
        if app.unseen > 0 {
            spans.push(Span::styled(
                format!("  ·  {} new lines", app.unseen),
                Style::default().fg(WARN),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[2]);
    } else {
        app.bottom_bar_rect = None;
        let flash = app
            .flash
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(2))
            .map(|(t, _)| t.clone());
        let voice_note = app
            .voice_note
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(2))
            .map(|(t, _)| t.clone());
        let status = if let Some(t) = voice_note {
            Line::from(vec![
                Span::styled("  ● ", Style::default().fg(RECORDING)),
                Span::styled(t, Style::default().fg(TEXT)),
            ])
        } else if let Some(t) = flash {
            Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(BRAND)),
                Span::styled(t, Style::default().fg(TEXT)),
            ])
        } else if let Some(l) = sb::status_line(app) {
            l
        } else if app.pending && app.connected {
            Line::from(vec![
                Span::styled(
                    format!("  {}", spinner_frame(app.tick / 2)),
                    Style::default().fg(BRAND),
                ),
                Span::styled(
                    format!(" {} · generating…", app.info.model),
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("ctrl+c", Style::default().fg(TEXT)),
                Span::styled(" interrompre", Style::default().fg(DIM)),
            ])
        } else {
            // idle: a static standby dot — the spinner only moves
            // while a turn runs; between turns nothing animates
            Line::from(vec![
                Span::styled("  ● ", Style::default().fg(BRAND)),
                Span::styled(
                    format!(
                        " bend-harness · {}",
                        app.info.model
                    ),
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("/ commandes", Style::default().fg(TEXT)),
                Span::styled(" · End: bottom · Ctrl+C: quit", Style::default().fg(DIM)),
            ])
        };
        frame.render_widget(Paragraph::new(status), chunks[2]);
    }

    // ---- the prompt: OpenCode prompt (left border ┃, element bg, meta row)
    // multi-line: newlines break rows, long rows wrap at the inner
    // width (layout_input)
    let inner = ((chunks[5].width as usize).saturating_sub(6 + voice_pad)).max(1);
    // the text area inside the block: left border + padding 3, padding
    // 2 right, 2 top; the rows above the meta row and its blank line
    let text_rows = (chunks[5].height as usize).saturating_sub(5).max(1);
    app.composer = ComposerArea {
        x: chunks[5].x + 4 + voice_pad as u16,
        y: chunks[5].y + 2,
        w: inner,
        h: text_rows,
        top: 0,
    };
    let mut input_lines: Vec<Line> = Vec::new();
    if app.ed.is_empty() && app.voice.active() {
        input_lines.push(Line::from(""));
    } else if app.ed.is_empty() {
        input_lines.push(Line::from(Span::styled(
            sb::placeholder(app).unwrap_or_else(|| "Ask anything…".to_string()),
            Style::default().fg(DIM),
        )));
    } else {
        // rows by display width (emojis are 2 columns); the cursor is
        // the REVERSED grapheme, or a REVERSED space on a newline or at
        // the end of the text; the selection has the selection colors
        let rows = editor::layout_input(&app.ed.text, inner);
        let drawn = editor::drawn_rows(&rows, app.ed.cursor);
        let cursor = app.ed.cursor;
        let selection = app.ed.selection();
        let (cur_row, _) = editor::row_col(&rows, cursor);
        // taller than the box: scroll so the cursor row stays visible
        let top = (cur_row + 1).saturating_sub(text_rows);
        app.composer.top = top;
        let text_style = Style::default().fg(TEXT);
        let sel_style = Style::default().fg(TEXT).bg(SELECTION);
        for row in rows.iter().take(drawn).skip(top) {
            let mut spans: Vec<Span> = Vec::new();
            let mut buf = String::new();
            let mut buf_sel = false;
            for cell in row {
                let n = cell.text.chars().count().max(1);
                let is_cursor = if cell.newline {
                    cell.ci == cursor
                } else {
                    cell.ci <= cursor && cursor < cell.ci + n
                };
                let in_sel = selection.is_some_and(|(a, b)| a <= cell.ci && cell.ci < b);
                if is_cursor || buf_sel != in_sel {
                    if !buf.is_empty() {
                        let st = if buf_sel { sel_style } else { text_style };
                        spans.push(Span::styled(std::mem::take(&mut buf), st));
                    }
                    buf_sel = in_sel;
                }
                if is_cursor {
                    // a pending dead key (Option+e…): its accent, marked,
                    // before the cursor, like macOS; on a full row (no
                    // column left) it takes the cursor cell instead, so
                    // the row never overflows and the cursor stays seen
                    let marked = Style::default().fg(BRAND).add_modifier(Modifier::UNDERLINED);
                    let row_w: usize = row.iter().map(|c| c.w).sum();
                    match app.ed.pending_dead() {
                        Some(acc) if row_w + 1 > inner => spans.push(Span::styled(
                            acc.to_string(),
                            marked.add_modifier(Modifier::REVERSED),
                        )),
                        acc => {
                            if let Some(acc) = acc {
                                spans.push(Span::styled(acc.to_string(), marked));
                            }
                            spans.push(Span::styled(
                                cell.text.to_string(),
                                text_style.add_modifier(Modifier::REVERSED),
                            ));
                        }
                    }
                } else if !cell.newline {
                    buf.push_str(cell.text);
                } else if in_sel {
                    // a selected newline shows as one selected blank
                    buf.push(' ');
                }
            }
            if !buf.is_empty() {
                spans.push(Span::styled(buf, if buf_sel { sel_style } else { text_style }));
            }
            input_lines.push(Line::from(spans));
        }
    }
    if app.voice.active() {
        input_lines = recording_lines(input_lines, voice_glyph(&app.voice));
    }
    // the meta row speaks glyphs: ◆ the harness, ● connected (quiet),
    // ○ déconnecté (loud) — the normal state stays muted, only the
    // broken one raises its voice
    let meta = Line::from(vec![
        Span::styled(
            "◆ bend",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            app.info.model.clone(),
            Style::default().fg(TEXT),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            if app.ed.text.contains('\n') {
                "⏎ send · ⇧⏎ new line"
            } else {
                "⇧⏎ new line"
            },
            Style::default().fg(DIM),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        if app.connected {
            Span::styled("●", Style::default().fg(DIM))
        } else {
            Span::styled("○ disconnected", Style::default().fg(ERR))
        },
    ]);
    // one blank line between the typed text and the meta row
    input_lines.push(Line::from(""));
    input_lines.push(meta);
    let border = if app.voice.active() { RECORDING } else { BRAND };
    let prompt = Paragraph::new(input_lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_set(SPLIT)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(ELEMENT))
            .padding(Padding::new(3, 2, 2, 1)),
    );
    frame.render_widget(prompt, chunks[5]);
    if card_h > 0 {
        sb::draw_card(app, frame, chunks[4]);
    } else if sb::card_full(app) {
        sb::draw_card(app, frame, chunks[0]);
    }

    // ---- slash-command popup (OpenCode autocomplete: split border,
    // backgroundMenu, primary selection)
    let matches = popup_items(app);
    if !matches.is_empty() {
        let n = matches.len().min(8) as u16;
        let w = if matches[0].closable { 72u16 } else { 56u16 }.min(chunks[5].width);
        let sel_i = app.popup_sel.min(matches.len() - 1);
        let top = popup_top(sel_i, matches.len(), 8);
        let area = Rect {
            x: chunks[5].x,
            y: chunks[5].y.saturating_sub(n + 2),
            width: w,
            height: n + 2,
        };
        frame.render_widget(Clear, area);
        let lines: Vec<Line> = matches
            .iter()
            .enumerate()
            .skip(top)
            .take(8)
            .map(|(i, c)| {
                let sel = i == sel_i;
                let (name_style, desc_style) = if sel {
                    (
                        Style::default()
                            .bg(BRAND)
                            .fg(ON_BRAND)
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(BRAND).fg(ON_BRAND),
                    )
                } else {
                    (Style::default().fg(TEXT), Style::default().fg(DIM))
                };
                let mut spans = Vec::new();
                if let Some((g, color)) = c.mark {
                    let st = if sel {
                        Style::default().bg(BRAND).fg(ON_BRAND)
                    } else {
                        Style::default().fg(color)
                    };
                    spans.push(Span::styled(format!(" {}", g), st));
                }
                spans.push(Span::styled(format!(" {} ", c.label), name_style));
                // columns, not chars: an emoji mark is 2 columns wide
                let mark_w = c.mark.map(|(g, _)| g.width() + 1).unwrap_or(0);
                let room = (w as usize).saturating_sub(c.label.width() + mark_w + 6);
                spans.push(Span::styled(truncate_chars(&c.desc, room), desc_style));
                Line::from(spans)
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_set(SPLIT)
                    .border_style(Style::default().fg(BORDER_ACTIVE))
                    .style(Style::default().bg(ELEMENT)),
            ),
            area,
        );
    }

    // ---- hint row (the OpenCode prompt right hint row)
    let hint = if app.voice.state() == voice::VoiceState::Recording {
        "recording · any key stops · Esc/Ctrl+C cancel"
    } else if app.voice.state() == voice::VoiceState::Flushing {
        "transcribing the last words… · Esc/Ctrl+C cancel"
    } else if let Some(h) = sb::hint(app) {
        h
    } else if app.pending {
        "⏎ steer · Tab queue · Ctrl+C interrupt · / commands · End bottom"
    } else {
        "⏎ send · Shift+⏎/Ctrl+J new line · / commands · Ctrl+T reasoning · Ctrl+C quit"
    };
    let hint = if app.term.shown() { term::HINT } else { hint };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(DIM)))),
        chunks[7],
    );
    help::draw(app, frame);
}

// one wire line into the feed of the app (the focused view)
fn ingest_line(app: &mut App, line: String) {
    if line == "--- idle" {
        app.pending = false;
        app.interrupt_requested = false;
        // BR-002: a flag that outlived its turn must
        // not kill the next one
        let _ = std::fs::write(
            &app.info.interrupt_path,
            "",
        );
    }
    // thinking duration: the model's reply arrives one
    // batch after the previous wire line
    let now = std::time::Instant::now();
    let ms = app
        .last_line_at
        .map_or(0, |t| now.duration_since(t).as_millis());
    app.last_line_at = Some(now);
    let (line, replayed) = strip_history(&line);
    // a replayed reasoning section has no duration
    let ms = if replayed { 0 } else { ms };
    let parsed = if replayed {
        parse_history_line(line)
    } else {
        parse_line(line)
    };
    if let Some(ev) = parsed {
        // the reasoning rides inside the assistant text
        // (think markers): it becomes its own collapsed
        // section, never raw history text
        let evs: Vec<Ev> = match ev {
            Ev::Assistant(t) => match split_thinking(&t) {
                Some((think, vis)) => {
                    let mut v = vec![Ev::Thinking {
                        ms,
                        text: think.to_string(),
                        open: app.show_thinking,
                    }];
                    if !vis.trim().is_empty() {
                        v.push(Ev::Assistant(vis.to_string()));
                    }
                    v
                }
                None => vec![Ev::Assistant(t)],
            },
            other => vec![other],
        };
        for ev in evs {
            let finished = match &ev {
                Ev::Tool(td) if !matches!(td.state, ToolState::Run) => {
                    Some(td.id)
                }
                _ => None,
            };
            // the view is top-anchored: a pinned view never
            // moves, a following view re-sticks in draw
            let appended =
                push_event(&mut app.events, &mut app.cache, ev);
            if appended && !app.follow {
                app.unseen += 1;
            }
            if let (true, Some(id)) = (replayed, finished) {
                hide_replayed_elapsed(&mut app.events, &mut app.cache, id);
            }
        }
    }
}

/// The feed position under the screen cell, from the last frame (the
/// feed starts at the top of the terminal: the screen row IS the feed
/// row). `clamp`: a row below the feed is its last row, at the end.
fn feed_pos(app: &App, x: u16, y: u16, clamp: bool) -> Option<feedsel::FeedPos> {
    let col = x.saturating_sub(app.feed_x) as usize;
    let (row, col) = if (y as usize) < app.vis_events.len() {
        (y as usize, col)
    } else if clamp && !app.vis_events.is_empty() {
        (app.vis_events.len() - 1, usize::MAX / 2)
    } else {
        return None;
    };
    Some((app.vis_events[row], *app.vis_rows.get(row)?, col))
}

/// The text of the feed selection (the rows of every event it spans).
fn feed_selection_text(app: &mut App) -> Option<String> {
    let sel = app.feed_sel?;
    let ((e0, r0, c0), (e1, r1, c1)) = sel.range();
    let (debug, w, tick) = (app.debug, app.area_w, app.tick);
    let mut rows: Vec<Line<'static>> = Vec::new();
    for i in e0..=e1.min(app.events.len().saturating_sub(1)) {
        ensure_rows(&app.events, &mut app.cache, i, debug, w, tick);
        let Some(er) = app.cache.get(i).and_then(|c| c.as_ref()) else { continue };
        let from = if i == e0 { r0 } else { 0 };
        let to = if i == e1 { (r1 + 1).min(er.rows.len()) } else { er.rows.len() };
        rows.extend(er.rows.get(from..to).unwrap_or(&[]).iter().cloned());
    }
    Some(feedsel::selection_text(&rows, c0, c1.saturating_add(1)))
}

/// Copies to the system clipboard and says so in the status row.
fn copy_text(app: &mut App, text: &str) {
    let n = text.chars().count();
    let note = if clipboard::copy(text) {
        format!("copied {} char{}", n, if n == 1 { "" } else { "s" })
    } else {
        "copy failed (no pbcopy, and the terminal refused OSC 52)".to_string()
    };
    app.flash = Some((note, std::time::Instant::now()));
}

/// Speech-to-text keys (Vibe's text_area._handle_voice_key): Ctrl+R
/// starts; while recording any key stops, Ctrl+C / Esc cancel; nothing
/// else sees those keys. `true` when the key was the voice's. `api_key`
/// finds MISTRAL_API_KEY (read only when a recording starts).
fn voice_key(
    app: &mut App,
    k: &crossterm::event::KeyEvent,
    api_key: impl FnOnce() -> Option<String>,
) -> bool {
    use voice::KeyAction;
    let now = std::time::Instant::now();
    match voice::key_action(app.voice.state(), app.voice.enabled, k.code, k.modifiers) {
        KeyAction::Pass => return false,
        KeyAction::Start => {
            if let Err(m) = app.voice.start(api_key(), now) {
                push_event(&mut app.events, &mut app.cache, Ev::Warn(m));
            }
        }
        KeyAction::Stop => app.voice.stop(now),
        KeyAction::Cancel => app.voice.cancel(),
        KeyAction::Swallow => {}
        KeyAction::OffHint => app.voice_note = Some((voice::OFF_HINT.into(), now)),
    }
    true
}

/// The transcription events of this tick: the text lands at the
/// composer cursor as it arrives.
fn pump_voice(app: &mut App) {
    let now = std::time::Instant::now();
    for out in app.voice.poll(now) {
        apply_voice(app, out, now);
    }
}

fn apply_voice(app: &mut App, out: voice::VoiceOutput, now: std::time::Instant) {
    match out {
        voice::VoiceOutput::Insert(t) => {
            app.ed.insert_voice(&t);
            app.popup_sel = 0;
        }
        voice::VoiceOutput::Utterance => app.ed.break_undo(),
        voice::VoiceOutput::Error(m) => {
            push_event(&mut app.events, &mut app.cache, Ev::Err(m));
        }
        voice::VoiceOutput::Notice(m) => app.voice_note = Some((m, now)),
    }
}

/// /voice: voice mode on or off, saved in ~/.bend-harness/tui.json.
fn toggle_voice(app: &mut App) -> Ev {
    let on = !app.voice.enabled;
    app.voice.enabled = on;
    if !on {
        app.voice.cancel();
    }
    match voice::save_voice_enabled(on) {
        Err(e) => Ev::Warn(format!(
            "{} (not saved: {})",
            if on { voice::ENABLED_MESSAGE } else { voice::DISABLED_MESSAGE },
            e
        )),
        Ok(()) => Ev::Info(if on { voice::ENABLED_MESSAGE } else { voice::DISABLED_MESSAGE }.into()),
    }
}

/// A key for the composer's editor (after the popups and the app keys):
/// moves, selection, deletes, undo/redo, typing, copy/cut; Up/Down move
/// between the visual rows, then through the history from the first and
/// last rows.
fn composer_key(app: &mut App, k: &crossterm::event::KeyEvent) {
    use editor::{Action, Motion};
    let Some(a) = editor::action(k) else { return };
    let w = app.composer.w.max(1);
    match a {
        Action::Up(sel) => {
            if !app.ed.row_up(w, sel) && (sel || !app.ed.history_up(&app.history)) {
                app.ed.move_cursor(Motion::TextStart, sel);
            }
        }
        Action::Down(sel) => {
            if !app.ed.row_down(w, sel) && (sel || !app.ed.history_down(&app.history)) {
                app.ed.move_cursor(Motion::TextEnd, sel);
            }
        }
        // the composer's selection, else the feed's
        Action::Copy => {
            if let Some(t) = app.ed.selected_text().or_else(|| feed_selection_text(app)) {
                copy_text(app, &t);
            }
        }
        Action::Cut => {
            if let Some(t) = app.ed.cut() {
                copy_text(app, &t);
                app.popup_sel = 0;
            }
        }
        Action::Insert(t) => {
            app.ed.insert(&t);
            app.popup_sel = 0;
            // a typed (never a pasted) `:name:` becomes its emoji
            if t == ":" {
                if let Some((text, cur)) = emoji::replace_typed(&app.ed.text, app.ed.cursor) {
                    app.ed.set(&text, cur);
                }
            }
        }
        other => {
            let edits = !matches!(other, Action::Move(..));
            app.ed.apply(&other);
            if edits {
                app.popup_sel = 0;
            }
        }
    }
}

fn run_tui(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
    // a multi-line paste arrives as ONE Event::Paste instead of a
    // keystroke storm where every Enter would send
    let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
    // the kitty keyboard protocol reports Shift+Enter distinctly (the
    // plain terminal encodings cannot); terminals without support just
    // ignore the push, and Ctrl+J remains the universal fallback
    let _ = crossterm::execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    loop {
        // the hub replays whole feeds on connect: the lines are taken in
        // slices of a few ms, a frame in between, so the UI never waits
        // for a replay to end
        let slice = std::time::Instant::now();
        let mut backlog = false;
        loop {
            if slice.elapsed() >= Duration::from_millis(12) {
                backlog = true;
                break;
            }
            match app.rx.try_recv() {
                Ok(line) => {
                    if app.sb.is_some() {
                        sb::dispatch(app, &line);
                    } else {
                        ingest_line(app, line);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // the REPL process is gone: a /reload exits it (the
                    // parent respawns and reconnects), a crash does not.
                    // Either way this UI is dead — stop, let the parent
                    // decide.
                    app.connected = false;
                    app.should_quit = true;
                    break;
                }
            }
        }
        if app.should_quit {
            break;
        }
        // switchboard Ctrl+O: a shell in the agent's directory; the TUI
        // gives the terminal back when it exits
        if let Some(dir) = sb::take_shell(app) {
            let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
            ratatui::restore();
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            println!("shell in {} — exit to return to Switchboard", dir);
            let _ = std::process::Command::new(shell).current_dir(&dir).status();
            terminal = ratatui::init();
            let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
            let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
            let _ = terminal.clear();
        }
        pump_voice(app);
        terminal.draw(|f| {
            if app.sb.is_some() {
                sb::draw_sb(app, f)
            } else {
                draw(app, f)
            }
        })?;
        // the level meter moves every 50 ms while recording (Vibe's poll)
        let wait = if backlog {
            Duration::ZERO
        } else if app.voice.active() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(80)
        };
        if poll(wait)? {
            let ev = read()?;
            if let Event::Mouse(m) = ev {
                if help::mouse(app, &m) {
                    continue;
                }
                if app.term.mouse(&m, terminal.size().map(|s| s.height).unwrap_or(24)) {
                    continue;
                }
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        app.follow = false;
                        app.scroll -= 3;
                    }
                    MouseEventKind::ScrollDown => {
                        if !app.follow {
                            app.scroll += 3;
                        }
                    }
                    // click the back-to-bottom bar to return to the tail;
                    // click a thinking section to expand/collapse it
                    // the composer: a press places the cursor (Shift
                    // extends), a drag selects, a double click selects
                    // the word, a triple click the whole text; the
                    // release copies the selection
                    MouseEventKind::Down(MouseButton::Left)
                        if app.composer.hit(&app.ed.text, m.column, m.row, false).is_some() =>
                    {
                        let ci = app.composer.hit(&app.ed.text, m.column, m.row, false).unwrap_or(0);
                        let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
                        match clicks {
                            2 => {
                                let (a, b) = editor::word_at(&app.ed.text, ci);
                                app.ed.select_range(a, b);
                            }
                            3 => app.ed.select_all(),
                            _ => app.ed.click(ci, m.modifiers.contains(KeyModifiers::SHIFT)),
                        }
                        app.mouse.drag = Some(DragIn::Composer);
                    }
                    MouseEventKind::Drag(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
                        if let Some(ci) = app.composer.hit(&app.ed.text, m.column, m.row, true) {
                            app.ed.click(ci, true);
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
                        app.mouse.drag = None;
                        if let Some(t) = app.ed.selected_text() {
                            copy_text(app, &t);
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(r) = app.bottom_bar_rect {
                            let inside = m.column >= r.x
                                && m.column < r.x + r.width
                                && m.row >= r.y
                                && m.row < r.y + r.height;
                            if inside {
                                app.follow = true;
                                app.unseen = 0;
                                continue;
                            }
                        }
                        // the feed: a press starts a selection (a double
                        // click selects the word, a triple the row); the
                        // release copies it, or toggles the section when
                        // the mouse did not move
                        let Some(pos) = feed_pos(app, m.column, m.row, false) else {
                            app.feed_sel = None;
                            continue;
                        };
                        let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
                        let row_text = app
                            .cache
                            .get(pos.0)
                            .and_then(|c| c.as_ref())
                            .and_then(|c| c.rows.get(pos.1))
                            .map(feedsel::line_text)
                            .unwrap_or_default();
                        let (a, b) = match clicks {
                            2 => feedsel::word_cols(&row_text, pos.2),
                            3 => (0, row_text.width().saturating_sub(1)),
                            _ => (pos.2, pos.2),
                        };
                        app.feed_sel = Some(feedsel::FeedSel { anchor: (pos.0, pos.1, a), head: (pos.0, pos.1, b) });
                        app.mouse.drag = Some(DragIn::Feed { moved: clicks > 1 });
                    }
                    MouseEventKind::Drag(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
                        // dragging on the top row or below the feed scrolls
                        if m.row == 0 {
                            app.follow = false;
                            app.scroll -= 1;
                        } else if m.row as usize >= app.area_h && !app.follow {
                            app.scroll += 1;
                        }
                        if let (Some(pos), Some(sel)) = (feed_pos(app, m.column, m.row, true), app.feed_sel.as_mut()) {
                            if sel.head != pos {
                                sel.head = pos;
                                app.mouse.drag = Some(DragIn::Feed { moved: true });
                            }
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
                        let moved = matches!(app.mouse.drag, Some(DragIn::Feed { moved: true }));
                        app.mouse.drag = None;
                        if moved {
                            if let Some(t) = feed_selection_text(app).filter(|t| !t.is_empty()) {
                                copy_text(app, &t);
                            }
                            continue;
                        }
                        // a plain click: expand/collapse the section
                        let Some(i) = app.feed_sel.take().map(|s| s.anchor.0) else { continue };
                        let toggled = match app.events.get_mut(i) {
                            Some(Ev::Thinking { open, .. }) => {
                                *open = !*open;
                                true
                            }
                            Some(Ev::Tool(td)) if td.code.is_some() => {
                                td.expanded = !td.expanded;
                                true
                            }
                            _ => false,
                        };
                        if toggled {
                            if let Some(c) = app.cache.get_mut(i) {
                                *c = None;
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if let Event::Paste(text) = &ev {
                if app.term.paste(text) {
                    continue;
                }
            }
            if let Event::Paste(text) = ev {
                // normalize CRLF/CR so a terminal paste behaves like the
                // typed newline, then insert at the cursor
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                app.ed.paste(&text);
                app.popup_sel = 0;
                continue;
            }
            if let Event::Key(k) = ev {
                if help::on_key(app, &k) {
                    continue;
                }
                if term::on_key(app, &k) {
                    continue;
                }
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if voice_key(app, &k, voice::resolve_api_key) {
                    continue;
                }
                if app.popup_dismissed.as_deref() != Some(app.ed.text.as_str()) {
                    app.popup_dismissed = None;
                }
                let matches = popup_items(app);
                let popup_open = !matches.is_empty();
                let sel = if popup_open {
                    Some(&matches[app.popup_sel.min(matches.len() - 1)])
                } else {
                    None
                };
                if app.sb.is_some() && sb::key(app, &k, popup_open) {
                    continue;
                }
                match (k.code, k.modifiers) {
                    // ctrl+c: clear input first, quit when already empty
                    // ctrl+c: INTERRUPT the running turn — the UI is
                    // free immediately (local abort); the runtime kills
                    // the turn at the next safe boundary (the blocking
                    // model/tool call in flight cannot be cancelled),
                    // and the dying turn's wire lines still render.
                    // The SECOND press quits the CLI; at idle, one press
                    // quits.
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        if app.interrupt_requested {
                            break;
                        } else if app.pending {
                            let ok = write_interrupt_flag(&app.info.interrupt_path);
                            // the local abort: the composer frees and the
                            // user can type right away; the flag clears
                            // when the interrupted turn's idle arrives
                            app.pending = false;
                            app.interrupt_requested = true;
                            push_event(
                                &mut app.events,
                                &mut app.cache,
                                Ev::Info(if ok {
                                    "interrupted — the current turn stops at the next safe point · Ctrl+C again to quit".to_string()
                                } else {
                                    "interrupt not written (side channel unreachable) — Ctrl+C again to quit".to_string()
                                }),
                            );
                        } else {
                            break;
                        }
                    }
                    // ctrl+t: expand/collapse every thinking section
                    (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                        app.show_thinking = !app.show_thinking;
                        for e in app.events.iter_mut() {
                            if let Ev::Thinking { open, .. } = e {
                                *open = app.show_thinking;
                            }
                        }
                        app.cache.clear();
                    }
                    // ctrl+l: clear the local feed
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        app.events.clear();
                        app.cache.clear();
                        app.anchor = (0, 0);
        app.scroll = 0;
                        app.follow = true;
                        app.unseen = 0;
                    }
                    // esc: close the popup, else drop the selection — it
                    // never interrupts (Ctrl+C does, through the flag
                    // side-channel)
                    (KeyCode::Esc, _) => {
                        if sel.is_some_and(|c| c.closable) {
                            // close the list, keep the text
                            app.popup_dismissed = Some(app.ed.text.clone());
                            app.popup_sel = 0;
                        } else if popup_open {
                            app.ed.clear();
                        } else {
                            app.ed.anchor = None;
                            app.feed_sel = None;
                        }
                    }
                    // scrollback: PgUp/PgDn page, End follows the bottom
                    (KeyCode::PageUp, _) => {
                        let page = (app.area_h / 2).max(1);
                        app.follow = false;
                        app.scroll -= page as isize;
                    }
                    (KeyCode::PageDown, _) => {
                        let page = (app.area_h / 2).max(1);
                        if !app.follow {
                            app.scroll += page as isize;
                        }
                    }
                    // End back to the tail when scrolled up, else the
                    // line end (the editor)
                    (KeyCode::End, KeyModifiers::NONE) if !app.follow => {
                        app.follow = true;
                        app.unseen = 0;
                    }
                    (KeyCode::Tab, _) => {
                        if let Some(c) = sel {
                            // popup completion
                            app.ed.set(&c.fill, c.fill_cursor);
                            app.popup_sel = 0;
                        } else if app.pending {
                            // codex queue_keys: queue the draft for after
                            // the turn ("say" forces the message reading
                            // even if the text starts with a protocol word)
                            let v = app.ed.text.trim().to_string();
                            if !v.is_empty() && !v.starts_with('/') {
                                app.ed.take();
                                handle_input(app, &format!("say {}", v));
                            }
                        }
                    }
                    // a newline in the composer: Shift+Enter (needs
                    // the kitty keyboard protocol), Ctrl+J (LF, the one
                    // binding EVERY terminal transmits), or alt+enter;
                    // plain Enter sends
                    (KeyCode::Enter, KeyModifiers::SHIFT)
                    | (KeyCode::Char('j'), KeyModifiers::CONTROL)
                    | (KeyCode::Enter, KeyModifiers::ALT) => {
                        app.ed.insert("\n");
                    }
                    (KeyCode::Enter, _) => {
                        if let Some(c) = sel {
                            if let Some(v) = c.run.clone() {
                                app.ed.take();
                                handle_input(app, &v);
                            } else {
                                app.ed.set(&c.fill, c.fill_cursor);
                                app.popup_sel = 0;
                            }
                        } else {
                            let v = app.ed.take().trim().to_string();
                            app.follow = true;
                            app.unseen = 0;
                            if !v.is_empty() {
                                // codex semantics: while the agent works,
                                // Enter STEERS the running turn; at idle it
                                // starts one. Commands pass through. The
                                // explicit "say" keeps text that starts
                                // with a protocol word ("reload ce
                                // fichier", "compact la fonction") a
                                // message, never a command.
                                let line = if v.starts_with('/') {
                                    v
                                } else if app.pending {
                                    format!("steer {}", v)
                                } else {
                                    format!("say {}", v)
                                };
                                handle_input(app, &line);
                            }
                        }
                    }
                    // the popup takes the plain arrows
                    (KeyCode::Up, KeyModifiers::NONE) if popup_open => {
                        app.popup_sel = (app.popup_sel + matches.len() - 1) % matches.len();
                    }
                    (KeyCode::Down, KeyModifiers::NONE) if popup_open => {
                        app.popup_sel = (app.popup_sel + 1) % matches.len();
                    }
                    _ => composer_key(app, &k),
                }
            }
        }
        app.tick = app.tick.wrapping_add(1);
    }
    // no orphan shell
    app.term.shutdown();
    let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
    let _ = crossterm::execute!(io::stdout(), DisableBracketedPaste);
    let _ = crossterm::execute!(io::stdout(), PopKeyboardEnhancementFlags);
    ratatui::restore();
    Ok(())
}

// ---- line mode (non-interactive stdin) ----

fn print_ev_of(ev: &Ev, debug: bool, width: usize) {
    if !ev_visible(ev, debug) {
        return;
    }
    let mut out = io::stdout();
    for l in ev_lines(ev, width) {
        for span in l.spans.iter() {
            let _ = write!(out, "{}", span.content);
        }
        let _ = writeln!(out);
    }
}

// waits for the turn to finish ("--- idle") or the channel to close,
// printing every event as it arrives; gives up after `max`
fn wait_idle(app: &mut App, max: Duration) {
    let start = std::time::Instant::now();
    loop {
        match app.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                app.feed_line(&line);
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
            app.feed_line(&l);
        }
        let line = line?;
        let v = line.trim().to_string();
        if v.is_empty() {
            continue;
        }
        let local = handle_input(app, &v);
        for ev in &local {
            print_ev_of(ev, app.debug, app.area_w);
        }
        if app.should_quit {
            break;
        }
        // a sent command runs a turn: wait for it to finish
        if app.pending {
            wait_idle(app, Duration::from_secs(120));
        } else {
            while let Ok(line) = app.rx.try_recv() {
                app.feed_line(&line);
            }
        }
    }
    Ok(())
}

/// Connect to the REPL and run the UI (interactive ratatui when stdin and
/// stdout are TTYs, line mode otherwise). `info` is what the REPL
/// announced (model, threshold, side-channel paths).
pub fn run(host: String, port: u16, info: HarnessInfo, debug: bool, session_id: String) -> io::Result<()> {
    let stream = TcpStream::connect((host.as_str(), port));
    let connected = true;
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            eprintln!("connexion impossible : {}", e);
            eprintln!("start the harness with ./run.sh");
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
        term: term::Term::default(),
        help: None,
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
        // line mode renders without a frame: the terminal width (or a
        // sane default) sizes the code blocks; interactive mode
        // overwrites this every frame
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
        ed: editor::Editor::default(),
        composer: ComposerArea::default(),
        flash: None,
        voice: voice::Voice::live(voice::load_voice_enabled()),
        voice_note: None,
        mouse: MouseState::default(),
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        tick: 0,
        info,
        host,
        port,
        session_id,
        stream: Some(stream),
        rx,
        should_quit: false,
        sb: None,
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
    while app.rx.try_recv().is_ok() {}
    let _ = connected;
    Ok(())
}

// ---- tests ----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_retry_and_restart_render() {
        match parse_line("  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s") {
            Some(Ev::Warn(t)) => assert_eq!(
                t,
                "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s"
            ),
            _ => panic!("provider_retry must render as a warning"),
        }
        match parse_line("  obs: harness_restarted: exit status: 1 · bend: out of memory") {
            Some(Ev::Err(t)) => assert!(t.contains("bend: out of memory") && t.contains("restarted")),
            _ => panic!("harness_restarted must render as an error"),
        }
    }

    #[test]
    fn split_thinking_takes_every_span() {
        let t = "<think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok";
        let (think, vis) = split_thinking(t).unwrap();
        assert_eq!(think, "a\\nBENDSIG::s1\\nb\\nBENDSIG::s2");
        assert_eq!(vis, "ok");
        assert!(split_thinking("no markers").is_none());
        assert!(split_thinking("<think>open").is_none());
    }

    // the runtime's wire encoding, as emit_code_ann produces it
    fn wire_encode(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('\r', "\\R")
            .replace('\n', "\\N")
    }

    // proper JSON args, as the model emits them: quotes and backslashes
    // escaped inside the code string, newlines as \n
    fn json_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    // the feed receives tool_started, the tool annotations, then
    // tool_finished: the block must render under the merged line
    #[test]
    fn tool_code_block_renders_under_the_tool_line() {
        let code = "// orchestre les appels
async function main(): Promise<unknown> {
  const xs = [1, 2, 3];
  const s = \"une chaîne\";
  return xs.length + s.length;
}";
        let args = format!("{{\"code\": \"{}\"}}", json_escape(code));
        let wire_lines = vec![
            "  obs: tool_started #3".to_string(),
            format!("tool #3 run_typescript : {}", args),
            format!("tool_code #3 : {}", wire_encode(&args)),
            "  obs: tool_finished #3 ok".to_string(),
        ];
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in &wire_lines {
            let ev = parse_line(l).expect("parse");
            push_event(&mut events, &mut cache, ev);
        }
        let tool = events
            .iter()
            .find_map(|e| match e {
                Ev::Tool(td) => Some(td.clone()),
                _ => None,
            })
            .expect("the merged tool");
        assert_eq!(tool.id, 3);
        assert_eq!(tool.name.as_deref(), Some("run_typescript"));
        assert!(tool.code.is_some());
        let rows = ev_lines(&Ev::Tool(tool), 80);
        let joined: Vec<String> = rows
            .iter()
            .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        for (i, l) in joined.iter().enumerate() {
            println!("{:2} | {}", i, l);
        }
        assert!(joined[0].contains("run_typescript") && !joined[0].contains("orchestre"));
        assert!(joined.iter().any(|l| l.contains("╭─ typescript")));
        assert!(joined.iter().any(|l| l.contains("async function main")));
        assert!(joined.iter().any(|l| l.contains("une chaîne")));
        // one bordered row per source line, top and bottom included
        assert_eq!(joined.iter().filter(|l| l.contains('│')).count(), 6);
    }

    fn merged_tool(wire_lines: &[String]) -> ToolData {
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in wire_lines {
            let ev = parse_line(l).expect("parse");
            push_event(&mut events, &mut cache, ev);
        }
        events
            .iter()
            .find_map(|e| match e {
                Ev::Tool(td) => Some(td.clone()),
                _ => None,
            })
            .expect("the merged tool")
    }

    fn rows_text(rows: &[Line<'static>]) -> Vec<String> {
        rows.iter()
            .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect()
    }

    // bash gets the same block as run_typescript: the raw command (no
    // JSON), bash header, one row per line, no preview on the tool line
    #[test]
    fn a_long_source_block_folds_until_clicked() {
        let cmd: String = (1..=200).map(|i| format!("echo {}", i)).collect::<Vec<_>>().join("\n");
        let wire_lines = vec![
            "  obs: tool_started #4".to_string(),
            "tool #4 bash : echo".to_string(),
            format!("tool_code #4 : {}", wire_encode(&cmd)),
            "  obs: tool_finished #4 ok".to_string(),
        ];
        let mut tool = merged_tool(&wire_lines);
        let folded = rows_text(&ev_lines(&Ev::Tool(tool.clone()), 80));
        assert_eq!(folded.iter().filter(|l| l.contains("│ echo")).count(), CODE_FOLD_SHOW);
        assert!(folded.iter().any(|l| l.contains("160 more lines")));
        tool.expanded = true;
        let whole = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        assert_eq!(whole.iter().filter(|l| l.contains("│ echo")).count(), 200);
        assert!(!whole.iter().any(|l| l.contains("more lines")));
    }

    #[test]
    fn bash_code_block_renders_under_the_tool_line() {
        let cmd = "# compte les fichiers
for f in *.rs; do
  echo \"$f: $(wc -l < \"$f\")\"
done | sort -n";
        let wire_lines = vec![
            "  obs: tool_started #4".to_string(),
            format!("tool #4 bash : {}", cmd.replace('\n', " ")),
            format!("tool_code #4 : {}", wire_encode(cmd)),
            "  obs: tool_finished #4 ok".to_string(),
        ];
        let tool = merged_tool(&wire_lines);
        assert_eq!(tool.name.as_deref(), Some("bash"));
        let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        for (i, l) in joined.iter().enumerate() {
            println!("{:2} | {}", i, l);
        }
        // the tool line names the tool; the source shows only in the block
        assert!(joined[0].contains("bash") && !joined[0].contains("compte"));
        assert!(joined.iter().any(|l| l.contains("╭─ bash")));
        assert!(joined.iter().any(|l| l.contains("for f in *.rs; do")));
        assert!(joined.iter().any(|l| l.contains("done | sort -n")));
        // one bordered row per source line (4 lines)
        assert_eq!(joined.iter().filter(|l| l.contains('│')).count(), 4);
    }

    // a bash tool line without tool_code (an older runtime) keeps the
    // flattened args preview and renders no block
    #[test]
    fn bash_without_code_has_no_block() {
        let tool = merged_tool(&[
            "  obs: tool_started #5".to_string(),
            "tool #5 bash : ls -la".to_string(),
            "  obs: tool_finished #5 ok".to_string(),
        ]);
        let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        assert!(joined[0].contains("ls -la"));
        assert!(!joined.iter().any(|l| l.contains('╭')));
    }

    fn style_of(lines: &[Vec<Span<'static>>], tok: &str) -> Style {
        lines
            .iter()
            .flatten()
            // adjacent same-style spans merge (whitespace included)
            .find(|s| s.content.trim() == tok)
            .map(|s| s.style)
            .unwrap_or_else(|| panic!("no span {:?}", tok))
    }

    // the lines LAWS.resume_replays_history pins on the Bend side: the
    // client rebuilds the conversation, tools merged, no fake duration
    #[test]
    fn resume_history_rebuilds_the_feed() {
        let wire = [
            "history   obs: compaction_done: court...",
            "history you : salut\\nça va",
            "history   obs: tool_started #7",
            "history tool #7 bash : ls -la",
            "history tool_code #7 : ls -la",
            "history tool_result #7 fail : boom",
            "history   obs: tool_finished #7 failed",
            "history   obs: assistant: fini",
            "history   obs: tool_started #9",
            "history tool #9 bash : pwd",
            "history tool_code #9 : pwd",
            "history   obs: tool_finished #9 failed",
            "history injected : [notification] bg 0 done",
            "  obs: session_restored: 7 messages",
        ];
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in wire {
            let (line, replayed) = strip_history(l);
            let ev = if replayed { parse_history_line(line) } else { parse_line(line) };
            let Some(ev) = ev else { continue };
            let finished = match &ev {
                Ev::Tool(td) if !matches!(td.state, ToolState::Run) => Some(td.id),
                _ => None,
            };
            push_event(&mut events, &mut cache, ev);
            if let (true, Some(id)) = (replayed, finished) {
                hide_replayed_elapsed(&mut events, &mut cache, id);
            }
        }
        let kinds: Vec<String> = events
            .iter()
            .map(|e| match e {
                Ev::Compacted(t) => format!("compacted {}", t),
                Ev::You(t) => format!("you {}", t),
                Ev::Assistant(t) => format!("assistant {}", t),
                Ev::Tool(td) => format!(
                    "tool {} {} ok={} code={} elapsed={:?}",
                    td.id,
                    td.name.clone().unwrap_or_default(),
                    matches!(td.state, ToolState::Ok),
                    td.code.is_some(),
                    td.elapsed
                ),
                Ev::Info(t) => format!("info {}", t),
                _ => "other".to_string(),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "compacted court...".to_string(),
                "you salut\nça va".to_string(),
                "tool 7 bash ok=false code=true elapsed=Some(\"\")".to_string(),
                "assistant fini".to_string(),
                "tool 9 bash ok=false code=true elapsed=Some(\"\")".to_string(),
                "info injected · [notification] bg 0 done".to_string(),
                "info session restored · 7 messages".to_string(),
            ]
        );
    }

    // a long line wraps inside the box: nothing is lost, every row has
    // the box width, continuation rows have an empty gutter
    #[test]
    fn long_code_lines_wrap_inside_the_box() {
        let cmd = "cd /Users/someone/lab/project && cargo test -p some-crate --release 2>&1 | rg 'test result|FAILED|panicked' | head -20
echo ok";
        let rows = rows_text(&code_block_lines(cmd, CodeLang::Bash, &ToolState::Ok, 50));
        for r in &rows {
            println!("{}", r);
        }
        let widths: Vec<usize> = rows.iter().map(|r| r.width()).collect();
        assert!(widths.iter().all(|&w| w == widths[0]), "aligned box: {:?}", widths);
        // top + 2 source lines, the first on several rows + bottom
        assert!(rows.len() > 4);
        // the text between the gutter bars reassembles the source
        let body: String = rows[1..rows.len() - 1]
            .iter()
            .map(|r| {
                // between the gutter's closing bar and the right border
                let first = r.find('│').unwrap();
                let second = first + 3 + r[first + 3..].find('│').unwrap();
                let last = r.rfind('│').unwrap();
                r[second + 3..last].to_string()
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(body.replace(' ', ""), cmd.replace(['\n', ' '], ""));
        // line 2 starts on a numbered row; continuation rows are blank
        assert!(rows[1].contains(" 1 │ cd "));
        assert!(rows[2].starts_with("  │   │ "));
        assert!(rows.iter().any(|r| r.contains(" 2 │ echo ok")));
    }

    // a token longer than the row breaks hard, never overflows
    #[test]
    fn wrap_breaks_long_tokens() {
        let spans = vec![Span::raw("x".repeat(25))];
        let rows = wrap_code_line(&spans, 10);
        let ws: Vec<usize> = rows.iter().map(|(_, w)| *w).collect();
        assert_eq!(ws, vec![10, 10, 5]);
        assert_eq!(wrap_code_line(&[], 10).len(), 1);
    }

    // apply_patch renders as a diff: summary in the tool line, the
    // envelope dropped, file headers, colored bands to the border
    #[test]
    fn apply_patch_renders_a_diff_block() {
        let patch = "*** Begin Patch
*** Update File: core/obs.bend
@@ def show_obs
   case T.Assistant{text}:
-    old line
+    new line
+    another line
*** Add File: notes.md
+# Notes
*** End Patch
";
        let tool = merged_tool(&[
            "  obs: tool_started #6".to_string(),
            format!("tool #6 apply_patch : {}", patch.replace('\n', " ")),
            format!("tool_code #6 : {}", wire_encode(patch)),
            "tool_result #6 ok : Done!".to_string(),
            "  obs: tool_finished #6 ok".to_string(),
        ]);
        let lines = ev_lines(&Ev::Tool(tool), 70);
        let rows = rows_text(&lines);
        for r in &rows {
            println!("{}", r);
        }
        assert!(rows[0].contains("apply_patch") && rows[0].ends_with("· core/obs.bend +2 −1, notes.md +1"));
        assert!(rows.iter().any(|r| r.contains("╭─ diff")));
        assert!(!rows.iter().any(|r| r.contains("Begin Patch") || r.contains("End Patch")));
        assert!(rows.iter().any(|r| r.contains("│ ~ core/obs.bend")));
        assert!(rows.iter().any(|r| r.contains("│ + notes.md · new")));
        // no line-number gutter in a diff
        assert!(rows.iter().any(|r| r.contains("│ -    old line")));
        // box aligned
        let boxed: Vec<usize> = rows.iter().filter(|r| r.contains('│') || r.contains('╭') || r.contains('╰')).map(|r| r.width()).collect();
        assert!(boxed.iter().all(|&w| w == boxed[0]), "{:?}", boxed);
        // the added line's band reaches the right border (padding included)
        let add_row = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("+    new line")))
            .expect("the added row");
        let n = add_row.spans.len();
        assert_eq!(add_row.spans[n - 2].style.bg, Some(DIFF_ADD_BG));
        let del_row = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("-    old line")))
            .expect("the removed row");
        assert!(del_row.spans.iter().any(|s| s.style.bg == Some(DIFF_DEL_BG)));
    }

    #[test]
    fn bash_highlighting_classifies_tokens() {
        let hl = highlight_bash(
            "# note
X=1 grep -n \"$HOME\" f.txt | wc -l && for i in 1 2; do echo $i; done",
        );
        let fg = |tok: &str| style_of(&hl, tok).fg;
        assert_eq!(fg("# note"), Some(SYNTAX_COMMENT));
        assert_eq!(fg("X"), Some(SYNTAX_NUMBER)); // assignment name
        assert_eq!(fg("grep"), Some(SYNTAX_FUNC)); // command word
        assert_eq!(fg("-n"), Some(HEAD)); // option
        assert_eq!(fg("$HOME"), Some(SYNTAX_NUMBER)); // expansion in quotes
        assert_eq!(fg("f.txt"), Some(TEXT)); // argument
        assert_eq!(fg("wc"), Some(SYNTAX_FUNC)); // command after a pipe
        assert_eq!(fg("for"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("in"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("do"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("echo"), Some(SYNTAX_FUNC)); // command after "do"
        assert_eq!(fg("done"), Some(SYNTAX_KEYWORD));
        assert_eq!(hl.len(), 2); // one span list per source line
    }
}

#[cfg(test)]
mod harness_info_tests {
    use super::HarnessInfo;

    // the exact string LAWS.bend pins for Rt.info_line (law
    // info_line_format): the two sides of the contract agree
    const BEND_LINE: &str = "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt";

    #[test]
    fn parses_the_line_bend_prints() {
        let info = HarnessInfo::parse(BEND_LINE).expect("parses");
        assert_eq!(info.model, "claude-opus-5-5");
        assert_eq!(info.threshold, "800000");
        assert_eq!(info.steer_path, "/tmp/bend-steer-7.txt");
        assert_eq!(info.interrupt_path, "/tmp/bend-interrupt-7.txt");
    }

    #[test]
    fn finds_the_line_in_a_repl_log() {
        let log = format!("{}\nbend-harness LIVE REPL on 127.0.0.1:7 ...\n[mcp] connector index written\n", BEND_LINE);
        assert_eq!(HarnessInfo::from_log(&log).expect("found").model, "claude-opus-5-5");
    }

    #[test]
    fn rejects_an_incomplete_line() {
        assert!(HarnessInfo::parse("harness-info model=m threshold=1").is_none());
        assert!(HarnessInfo::parse("bend-harness LIVE REPL on 127.0.0.1:7").is_none());
    }
}

#[cfg(test)]
mod popup_tests {
    use super::popup_top;

    #[test]
    fn the_selection_stays_in_view() {
        assert_eq!(popup_top(0, 5, 8), 0);
        assert_eq!(popup_top(4, 5, 8), 0);
        assert_eq!(popup_top(7, 12, 8), 0);
        assert_eq!(popup_top(8, 12, 8), 1);
        assert_eq!(popup_top(11, 12, 8), 4);
        assert_eq!(popup_top(99, 12, 8), 4); // clamped like the selection
    }
}

#[cfg(test)]
mod emoji_width_tests {
    use super::*;
    use crate::editor::layout_input;

    fn row_widths(input: &str, inner: usize) -> Vec<usize> {
        layout_input(input, inner)
            .iter()
            .map(|r| r.iter().filter(|c| !c.newline).map(|c| c.w).sum())
            .collect()
    }

    #[test]
    fn composer_rows_count_emojis_as_two_columns() {
        // 5 emojis at 6 columns: 3 per row, the 4th never splits a row
        assert_eq!(row_widths("👏👏👏👏👏", 6), vec![6, 4]);
        // an emoji that does not fit the row end moves to the next row
        assert_eq!(row_widths("abcde👏", 6), vec![5, 2]);
        for inner in 2..12 {
            for input in ["a👏b👍🏽c❤️d👨‍👩‍👧e🇫🇷", "👏👏👏👏👏👏👏", "x y 👏👏 z\nq👏"] {
                assert!(row_widths(input, inner).iter().all(|&w| w <= inner), "{input} @ {inner}");
            }
        }
    }

    #[test]
    fn composer_cells_are_graphemes_with_char_indices() {
        let rows = layout_input("a👍🏽❤️👨‍👩‍👧b", 40);
        let cells: Vec<(usize, &str, usize)> = rows[0].iter().map(|c| (c.ci, c.text, c.w)).collect();
        assert_eq!(
            cells,
            vec![(0, "a", 1), (1, "👍🏽", 2), (3, "❤️", 2), (5, "👨‍👩‍👧", 2), (10, "b", 1), (11, " ", 1)]
        );
        // the end cursor slot sits after the last char
        assert!(rows[0].last().unwrap().newline);
    }

    #[test]
    fn feed_wrap_counts_graphemes_as_drawn() {
        let w = |l: &Line| l.spans.iter().map(|s| s.content.width()).sum::<usize>();
        let rows = wrap_line(Line::from("👨‍👩‍👧 👨‍👩‍👧 👨‍👩‍👧"), 8);
        assert_eq!(rows.len(), 1, "3 × 2 cols + 2 spaces fit 8");
        let rows = wrap_line(Line::from("👏👏👏👏👏"), 4);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| w(r) <= 4));
        let rows = wrap_code_line(&[Span::raw("❤️❤️❤️")], 4);
        assert_eq!(rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec![4, 2]);
    }
}
