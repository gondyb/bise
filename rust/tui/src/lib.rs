//! bend-tui — Ratatui terminal UI for the Bend Unified Harness.
//!
//! Rust port of repl-tui (Ink). Same wire protocol; the presentation is
//! modeled on the REAL OpenCode TUI (packages/tui in the opencode repo):
//! no header bar — the screen is the conversation. User messages are
//! blocks with a colored left bar and a panel background; assistant
//! markdown renders in the OpenCode markdown colors; tool calls are
//! OpenCode inline tools (braille spinner while running, muted ✓ once
//! done, red ✗ on failure); the prompt is an OpenCode prompt (left
//! border, element background, agent/model meta row); commands filter
//! in an OpenCode autocomplete popup (split border, primary selection).
//! The status row carries the spinner + esc-to-interrupt hints.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode so
//! the UI stays scriptable — the same convention as the Ink version.

use crossterm::event::{
    poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
    KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
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
use unicode_width::UnicodeWidthChar;

// ---- the OpenCode theme (opencode.json, dark) ----
// primary #fab283 (the OpenCode orange) is the agent color: user blocks,
// prompt border, spinners, back-to-bottom. Markdown follows the
// markdown* keys; tools follow the inline-tool rules (muted once
// complete, error red).
const BRAND: Color = Color::Rgb(0xfa, 0xb2, 0x83); // primary
const ACCENT: Color = Color::Rgb(0x9d, 0x7c, 0xd8); // markdownHeading
const HEAD: Color = Color::Rgb(0xe5, 0xc0, 0x7b); // markdownEmph / syntaxType
const INFO: Color = Color::Rgb(0x56, 0xb6, 0xc2); // info / markdownListEnumeration
const TEXT: Color = Color::Rgb(0xee, 0xee, 0xee); // text
const DIM: Color = Color::Rgb(0x80, 0x80, 0x80); // textMuted
                                                 // complete tools sit at textMuted (OpenCode: fg textMuted when complete)
const TOOL: Color = Color::Rgb(0x80, 0x80, 0x80);
const OK: Color = Color::Rgb(0x7f, 0xd8, 0x8f); // success / markdownCode
const WARN: Color = Color::Rgb(0xf5, 0xa7, 0x42); // warning / markdownStrong
const ERR: Color = Color::Rgb(0xe0, 0x6c, 0x75); // error
const PANEL: Color = Color::Rgb(0x14, 0x14, 0x14); // backgroundPanel
const ELEMENT: Color = Color::Rgb(0x1e, 0x1e, 0x1e); // backgroundElement
const BORDER_ACTIVE: Color = Color::Rgb(0x60, 0x60, 0x60); // borderActive

// the OpenCode prompt/autocomplete borders: only a colored vertical bar
const SPLIT: border::Set = border::Set {
    top_left: "",
    top_right: "",
    bottom_left: "",
    bottom_right: "",
    vertical_left: "┃",
    vertical_right: "┃",
    horizontal_top: " ",
    horizontal_bottom: " ",
};

// the OpenCode spinner (component/spinner.tsx): braille dots
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner_frame(tick: u32) -> &'static str {
    SPINNER[(tick as usize) % SPINNER.len()]
}

// ---- feed events ----

#[derive(Clone)]
enum ToolState {
    Run,
    Ok,
    Fail,
}

// one tool call: enriched by the runtime annotations (name/args/result)
#[derive(Clone)]
struct ToolData {
    id: u32,
    name: Option<String>,
    args: Option<String>,
    state: ToolState,
    result: Option<(bool, String)>,
    started: std::time::Instant,
    // frozen at finish; None while running (elapsed ticks live)
    elapsed: Option<String>,
}

#[derive(Clone)]
enum Ev {
    You(String),
    Assistant(String),
    Tool(ToolData),
    // a sub-call made inside a run_typescript program
    Sub {
        name: String,
        ok: bool,
        preview: String,
    },
    // runtime annotations, merged into the matching Tool by id
    ToolInfo {
        id: u32,
        name: String,
        args: String,
    },
    ToolResult {
        id: u32,
        ok: bool,
        preview: String,
    },
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
    // runtime annotations: tool #<id> <name> : <args>
    if let Some(r) = line.strip_prefix("tool #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolInfo {
            id,
            name: name.trim().to_string(),
            args: args.to_string(),
        });
    }
    // tool_result #<id> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("tool_result #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolResult {
            id,
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
    }
    // subtool <name> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("subtool ") {
        let (name, rest) = r.split_once(' ')?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::Sub {
            name: name.to_string(),
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
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
        return if t.is_empty() {
            None
        } else {
            Some(Ev::Assistant(t.to_string()))
        };
    }
    if o == "assistant:" {
        return None;
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        let id: u32 = n.parse().ok()?;
        return Some(Ev::Tool(ToolData {
            id,
            name: None,
            args: None,
            state: ToolState::Run,
            result: None,
            started: std::time::Instant::now(),
            elapsed: None,
        }));
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let (id_s, tail) = rest.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let state = match tail {
            "ok" => ToolState::Ok,
            _ => ToolState::Fail,
        };
        let started = std::time::Instant::now();
        return Some(Ev::Tool(ToolData {
            id,
            name: None,
            args: None,
            state,
            result: None,
            started,
            elapsed: Some(fmt_elapsed(started)),
        }));
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
        return Some(Ev::Warn(
            "réponse vide du modèle — nouvelle tentative".into(),
        ));
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        // a completed turn needs no annotation; a failure or an
        // interrupt must never disappear — the turn just stops
        if t == "completed" {
            return Some(Ev::TurnDone("tour terminé".to_string()));
        }
        if let Some(why) = t.strip_prefix("failed: ") {
            return Some(Ev::Err(format!("tour échoué : {}", why)));
        }
        if t == "interrupted" {
            return Some(Ev::Warn("tour interrompu".into()));
        }
        return Some(Ev::Err(format!("tour arrêté : {}", t)));
    }
    // the runtime ran out of execution budget mid-turn (never silent)
    if let Some(t) = o.strip_prefix("turn_stalled: ") {
        return Some(Ev::Err(format!("tour interrompu : {}", t)));
    }
    Some(Ev::Raw(o.to_string()))
}

// ---- the codex-style layout cache ----
// Events render to wrapped rows ONCE (per width / per mutation); every
// frame only the visible slice is cloned into the paragraph. A running
// tool re-renders each frame because its elapsed ticks live.

struct EventRows {
    width: u16,
    rows: Vec<Line<'static>>,
}

fn line_from(cells: Vec<(char, Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cur_style: Option<Style> = None;
    let mut buf = String::new();
    for (c, st) in cells {
        match cur_style {
            Some(s) if s == st => buf.push(c),
            _ => {
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), cur_style.unwrap()));
                }
                cur_style = Some(st);
                buf.push(c);
            }
        }
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur_style.unwrap()));
    }
    Line::from(spans)
}

// span-aware greedy word wrap; words wider than the row hard-split
fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut cells: Vec<(char, Style)> = Vec::new();
    for sp in line.spans {
        for c in sp.content.chars() {
            cells.push((c, sp.style));
        }
    }
    if cells.is_empty() {
        return vec![Line::from("")];
    }
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut row_w = 0usize;
    let mut i = 0usize;
    while i < cells.len() {
        // the next word (non-space run)
        let mut word: Vec<(char, Style)> = Vec::new();
        let mut word_w = 0usize;
        while i < cells.len() && cells[i].0 != ' ' {
            word_w += cells[i].0.width().unwrap_or(1).max(1);
            word.push(cells[i]);
            i += 1;
        }
        // wrap before the word if it does not fit
        if row_w > 0 && row_w + word_w > width {
            rows.push(line_from(std::mem::take(&mut row)));
            row_w = 0;
        }
        // hard-split words wider than a full row
        if row_w == 0 && word_w > width {
            let mut chunk: Vec<(char, Style)> = Vec::new();
            let mut cw = 0usize;
            for (c, st) in word {
                let cc = c.width().unwrap_or(1).max(1);
                if cw + cc > width {
                    rows.push(line_from(std::mem::take(&mut chunk)));
                    cw = 0;
                }
                chunk.push((c, st));
                cw += cc;
            }
            row = chunk;
            row_w = cw;
        } else {
            row.extend(word);
            row_w += word_w;
        }
        // the spaces that follow the word stay on the row
        while i < cells.len() && cells[i].0 == ' ' {
            row.push(cells[i]);
            row_w += 1;
            i += 1;
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(line_from(row));
    }
    rows
}

// the rows of one event: the block separator, then the wrapped lines
fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> Vec<Line<'static>> {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    if !ev_visible(ev, debug) {
        return rows;
    }
    // the previous VISIBLE event decides the separator: a debug-only
    // annotation between two blocks must not swallow the blank line
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    let message = matches!(ev, Ev::You(_) | Ev::Assistant(_));
    let tool_block = matches!(ev, Ev::Tool(_) | Ev::Sub { .. });
    let prev_message = prev.map_or(false, |p| matches!(p, Ev::You(_) | Ev::Assistant(_)));
    let prev_tool_block = prev.map_or(false, |p| matches!(p, Ev::Tool(_) | Ev::Sub { .. }));
    // messages and tool blocks breathe: a blank line at every transition
    if (message && prev_tool_block) || (tool_block && prev_message) {
        rows.push(Line::from(""));
    }
    for l in ev_lines_t(ev, tick) {
        rows.extend(wrap_line(l, width));
    }
    rows
}

// structural annotations (turn separators, idle markers) are debug-only;
// messages, tool activity, compaction and errors always show
fn ev_visible(ev: &Ev, debug: bool) -> bool {
    if debug {
        return true;
    }
    !matches!(
        ev,
        Ev::Turn
            | Ev::TurnDone(_)
            | Ev::Idle
            | Ev::Raw(_)
            | Ev::ToolInfo { .. }
            | Ev::ToolResult { .. }
    )
}

fn push_event(events: &mut Vec<Ev>, cache: &mut Vec<Option<EventRows>>, ev: Ev) -> bool {
    // annotations enrich the matching tool event instead of stacking
    match &ev {
        Ev::ToolInfo { id, name, args } => {
            for (i, e) in events.iter_mut().enumerate().rev() {
                if let Ev::Tool(td) = e {
                    if td.id == *id {
                        td.name = Some(name.clone());
                        td.args = Some(args.clone());
                        cache[i] = None;
                        return false;
                    }
                }
            }
            return false;
        }
        Ev::ToolResult { id, ok, preview } => {
            for (i, e) in events.iter_mut().enumerate().rev() {
                if let Ev::Tool(td) = e {
                    if td.id == *id {
                        td.result = Some((*ok, preview.clone()));
                        cache[i] = None;
                        return false;
                    }
                }
            }
            return false;
        }
        Ev::Tool(td) if !matches!(td.state, ToolState::Run) => {
            // a tool finishing rewrites its running line
            for (i, e) in events.iter_mut().enumerate().rev() {
                if let Ev::Tool(td2) = e {
                    if td2.id == td.id && matches!(td2.state, ToolState::Run) {
                        td2.state = td.state.clone();
                        td2.elapsed = Some(fmt_elapsed(td2.started));
                        if td2.result.is_none() {
                            td2.result = td.result.clone();
                        }
                        cache[i] = None;
                        return false;
                    }
                }
            }
            events.push(ev);
            cache.push(None);
            return true;
        }
        // the turn ended: a tool still shown as running was abandoned
        // (interrupt or failed turn) — freeze it so the elapsed stops
        Ev::TurnDone(_) | Ev::Idle => {
            for (i, e) in events.iter_mut().enumerate() {
                if let Ev::Tool(td) = e {
                    if matches!(td.state, ToolState::Run) {
                        td.state = ToolState::Fail;
                        td.elapsed = Some(fmt_elapsed(td.started));
                        if td.result.is_none() {
                            td.result = Some((false, "interrompu".to_string()));
                        }
                        cache[i] = None;
                    }
                }
            }
        }
        _ => {}
    }
    events.push(ev);
    cache.push(None);
    true
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

// inline styles in the OpenCode markdown colors: **strong** is
// markdownStrong (orange), *emph* is markdownEmph (yellow), `code` is
// markdownCode (green)
fn inline_spans(s: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '`' {
            if let Some(j) = (i + 1..cs.len()).find(|k| cs[*k] == '`') {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    Style::default().fg(OK).add_modifier(Modifier::BOLD),
                ));
                i = j + 1;
                continue;
            }
        }
        if c == '*' {
            if i + 1 < cs.len() && cs[i + 1] == '*' {
                if let Some(j) =
                    (i + 3..cs.len()).find(|k| cs[*k] == '*' && cs.get(k + 1) == Some(&'*'))
                {
                    if !plain.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut plain), base));
                    }
                    spans.push(Span::styled(
                        cs[i + 2..j].iter().collect::<String>(),
                        base.add_modifier(Modifier::BOLD).fg(WARN),
                    ));
                    i = j + 2;
                    continue;
                }
            } else if let Some(j) = (i + 2..cs.len()).find(|k| {
                cs[*k] == '*' && cs.get(k - 1) != Some(&'*') && cs.get(k + 1) != Some(&'*')
            }) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    base.add_modifier(Modifier::ITALIC).fg(HEAD),
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
            out.push(Line::from(Span::styled("  ", Style::default().bg(ELEMENT))));
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(TEXT).bg(ELEMENT),
            )));
            continue;
        }
        if line.is_empty() {
            out.push(Line::from(""));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim_start();
        let base = Style::default().fg(TEXT);
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            let head = t[level..].trim_start();
            out.push(Line::from(Span::styled(
                head.to_string(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        // OpenCode markdownListEnumeration: "N. item" in the info color
        let dot = t
            .char_indices()
            .find(|(i, c)| *i > 0 && *c == '.' && t[i + 1..].starts_with(' '));
        if let Some((i, _)) = dot {
            if t[..i].chars().all(|c| c.is_ascii_digit()) {
                out.push(Line::from_iter(
                    std::iter::once(Span::styled(
                        format!("  {} ", &t[..i + 1]),
                        Style::default().fg(INFO),
                    ))
                    .chain(inline_spans(t[i + 1..].trim_start(), base)),
                ));
                continue;
            }
        }
        if indent == 0 && (t.starts_with("- ") || t.starts_with("* ")) {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  - ", Style::default().fg(BRAND)))
                    .chain(inline_spans(&t[2..], base)),
            ));
            continue;
        }
        if indent == 0 && t.starts_with("> ") {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  | ", Style::default().fg(HEAD)))
                    .chain(inline_spans(&t[2..], Style::default().fg(HEAD))),
            ));
            continue;
        }
        out.push(Line::from(inline_spans(line, base)));
    }
    out
}

// ---- tool-call rendering helpers ----

fn fmt_elapsed(started: std::time::Instant) -> String {
    let s = started.elapsed().as_secs_f64();
    if s < 10.0 {
        format!("{:.1}s", s)
    } else if s < 60.0 {
        format!("{:.0}s", s)
    } else {
        format!("{:.0}m{:02.0}s", (s / 60.0).floor(), s % 60.0)
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{}…", head)
    }
}

// naive "field":"value" extractor for JSON-ish args (no parser needed:
// the runtime caps the payload and the shape is known)
fn json_str_field(s: &str, field: &str) -> Option<String> {
    let pat = format!("\"{}\"", field);
    let i = s.find(&pat)?;
    let rest = s[i + pat.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut ch = rest.chars();
    while let Some(c) = ch.next() {
        match c {
            '\\' => match ch.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                // truncated payload: keep the partial value
                None => out.push('\\'),
            },
            '"' => return Some(out),
            _ => out.push(c),
        }
    }
    // the runtime caps the wire payload: a cut string is still useful
    Some(out)
}

// the args preview: what the engineer reads at a glance
//   run_typescript        the first line of main() (the signature)
//   search_tool_functions mode and the query
//   bash / mcp            the raw args
fn args_preview(name: &str, args: &str) -> String {
    if name == "run_typescript" {
        if let Some(code) = json_str_field(args, "code") {
            let first = code
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("");
            let mut p = truncate_chars(first, 64);
            if code.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
                p.push_str(" …");
            }
            return p;
        }
    }
    if name == "search_tool_functions" {
        let mode = json_str_field(args, "mode").unwrap_or_else(|| "best_match".into());
        if let Some(q) = json_str_field(args, "query") {
            return format!("{} \"{}\"", mode, truncate_chars(&q, 48));
        }
        return mode;
    }
    truncate_chars(args.trim(), 80)
}

// one event renders as one or many lines (markdown expands messages).
// The shape follows the OpenCode message parts: user messages are blocks
// with a colored left bar and panel background; assistant text is
// markdown in the OpenCode colors; tools are inline tools.
fn ev_lines_t(ev: &Ev, tick: u32) -> Vec<Line<'static>> {
    // the feed twin: identical shape, but the tool spinner animates
    match ev {
        Ev::Tool(td) => tool_lines(td, tick),
        other => ev_lines(other),
    }
}

fn ev_lines(ev: &Ev) -> Vec<Line<'static>> {
    match ev {
        Ev::You(t) => user_block_lines(t),
        Ev::Assistant(t) => md_to_lines(&unescape_md(t)),
        Ev::Tool(td) => tool_lines(td, 0),
        Ev::Idle => vec![Line::from("")],
        Ev::Sub { name, ok, preview } => vec![Line::from(vec![
            Span::styled("    ↳ ", Style::default().fg(DIM)),
            Span::styled(name.clone(), Style::default().fg(DIM)),
            Span::styled(
                if *ok { " ok " } else { " échec " },
                Style::default().fg(if *ok { DIM } else { ERR }),
            ),
            Span::styled(
                truncate_chars(preview.trim(), 100),
                Style::default().fg(DIM),
            ),
        ])],
        Ev::Turn => vec![Line::from(Span::styled(
            "── tour ".to_string() + &"─".repeat(30),
            Style::default().fg(DIM),
        ))],
        Ev::TurnDone(t) => vec![Line::from(vec![
            Span::styled(" └ ", Style::default().fg(DIM)),
            Span::styled(
                t.clone(),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::Compact(t) => vec![Line::from(vec![
            Span::styled(
                "  compaction ",
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Compacted(t) => vec![Line::from(vec![
            Span::styled(
                "  résumé ",
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                unescape_md(t),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::Warn(t) => vec![Line::from(vec![
            Span::styled("  ! ", Style::default().fg(WARN)),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Err(t) => vec![Line::from(vec![
            Span::styled("  ✗ ", Style::default().fg(ERR)),
            Span::styled(t.clone(), Style::default().fg(ERR)),
        ])],
        Ev::Info(t) => vec![Line::from(Span::styled(
            format!("  {}", t),
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))],
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } => vec![],
        Ev::Raw(t) => vec![Line::from(Span::styled(
            format!("  {}", t),
            Style::default().fg(DIM),
        ))],
    }
}

// the OpenCode user message block: colored left bar (┃ primary), panel
// background, one blank line above and below
fn user_block_lines(text: &str) -> Vec<Line<'static>> {
    let mut rows = vec![Line::from("")];
    for l in wrap_line(
        Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        4000,
    ) {
        let mut spans = vec![Span::styled(" ┃ ", Style::default().fg(BRAND))];
        spans.extend(l.spans);
        rows.push(Line::from(spans));
    }
    rows.push(Line::from(Span::styled("   ", Style::default().bg(PANEL))));
    rows
}

// the tool line, OpenCode inline-tool style: 2-col icon, name, args
// preview; result preview on the next line. Running: braille spinner +
// text. Complete: ✓ + textMuted. Failed: ✗ + error red.
fn tool_lines(td: &ToolData, tick: u32) -> Vec<Line<'static>> {
    let name = td.name.clone().unwrap_or_else(|| format!("#{}", td.id));
    let args = td
        .args
        .as_deref()
        .map(|a| args_preview(&name, a))
        .unwrap_or_default();
    let elapsed = fmt_elapsed(td.started);
    let mut ls = Vec::new();
    match td.state {
        ToolState::Run => ls.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(spinner_frame(tick / 2), Style::default().fg(BRAND)),
            Span::styled(format!(" {}", name), Style::default().fg(TEXT)),
            Span::styled(format!(" {}", elapsed), Style::default().fg(DIM)),
            Span::styled(
                if args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", args)
                },
                Style::default().fg(DIM),
            ),
        ])),
        ToolState::Ok => ls.push(Line::from(vec![
            Span::styled("  ✓ ", Style::default().fg(DIM)),
            Span::styled(name.clone(), Style::default().fg(TOOL)),
            Span::styled(
                format!(" ok {}", td.elapsed.clone().unwrap_or_default()),
                Style::default().fg(TOOL),
            ),
            Span::styled(
                if args.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", args)
                },
                Style::default().fg(TOOL),
            ),
        ])),
        ToolState::Fail => ls.push(Line::from(vec![
            Span::styled("  ✗ ", Style::default().fg(ERR)),
            Span::styled(name.clone(), Style::default().fg(ERR)),
            Span::styled(
                format!(" échec {}", td.elapsed.clone().unwrap_or_default()),
                Style::default().fg(ERR),
            ),
            Span::styled(
                if args.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", args)
                },
                Style::default().fg(ERR),
            ),
        ])),
    }
    if let Some((ok, preview)) = &td.result {
        if !preview.trim().is_empty() {
            ls.push(Line::from(vec![
                Span::styled("    ↳ ", Style::default().fg(TOOL)),
                Span::styled(
                    truncate_chars(preview.trim(), 110),
                    Style::default().fg(if *ok { TOOL } else { ERR }),
                ),
            ]));
        }
    }
    ls
}

// ---- app state ----

struct App {
    connected: bool,
    debug: bool,
    // line mode holds running tools until they finish so the printed
    // line carries the merged annotations (name, args, result)
    line_tools: std::collections::HashMap<u32, ToolData>,
    // feed scrollback: top is an offset from the FIRST row, follow means
    // stick to the bottom (any scroll up turns it off, End/enter turn it
    // back on). Top-anchored, so new content never moves a pinned view.
    follow: bool,
    top: usize,
    max_top: usize,
    // activity that arrived while pinned (shown by the back-to-bottom bar)
    unseen: usize,
    tail_visible: bool,
    bottom_bar_rect: Option<ratatui::layout::Rect>,
    // wrapped rows per event, keyed by event index (the codex layout
    // cache: rebuild on mutation, width change, or live-elapsed tools)
    cache: Vec<Option<EventRows>>,
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

    // line mode: one printed line per finished tool, carrying the merged
    // annotations; sub-calls print live as they complete
    fn feed_line(&mut self, line: &str) {
        if line == "--- idle" {
            self.pending = false;
        }
        let Some(ev) = parse_line(line) else { return };
        match ev {
            Ev::Tool(td) if matches!(td.state, ToolState::Run) => {
                self.line_tools.insert(td.id, td);
            }
            Ev::ToolInfo { id, name, args } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.name = Some(name);
                    td.args = Some(args);
                }
            }
            Ev::ToolResult { id, ok, preview } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.result = Some((ok, preview));
                }
            }
            Ev::Tool(td) => {
                if let Some(mut held) = self.line_tools.remove(&td.id) {
                    held.state = td.state;
                    if held.elapsed.is_none() {
                        held.elapsed = Some(fmt_elapsed(held.started));
                    }
                    print_ev_of(&Ev::Tool(held), self.debug);
                } else {
                    print_ev_of(&Ev::Tool(td), self.debug);
                }
            }
            ev2 @ (Ev::TurnDone(_) | Ev::Idle) => {
                // abandoned running tools get one final line
                let mut held: Vec<ToolData> = self.line_tools.drain().map(|(_, td)| td).collect();
                held.sort_by_key(|td| td.id);
                for mut td in held {
                    td.state = ToolState::Fail;
                    td.elapsed = Some(fmt_elapsed(td.started));
                    if td.result.is_none() {
                        td.result = Some((false, "interrompu".to_string()));
                    }
                    print_ev_of(&Ev::Tool(td), self.debug);
                }
                print_ev_of(&ev2, self.debug);
            }
            other => print_ev_of(&other, self.debug),
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
    Cmd {
        name: "/compact",
        desc: "compacter la conversation (résumé)",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrompre le tour en cours",
        args: false,
    },
    Cmd {
        name: "/reload",
        desc: "relancer le harness avec le dernier code (session conservée)",
        args: false,
    },
    Cmd {
        name: "/steer",
        desc: "diriger le modèle pendant le tour",
        args: true,
    },
    Cmd {
        name: "/notify",
        desc: "injecter une notification au modèle",
        args: true,
    },
    Cmd {
        name: "/status",
        desc: "modèle, connexion, seuil de compaction",
        args: false,
    },
    Cmd {
        name: "/clear",
        desc: "vider l'affichage local",
        args: false,
    },
    Cmd {
        name: "/help",
        desc: "liste des commandes",
        args: false,
    },
    Cmd {
        name: "/quit",
        desc: "quitter le client (la session survit)",
        args: false,
    },
];

fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    COMMANDS
        .iter()
        .filter(|c| c.name.starts_with(input))
        .collect()
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
    // the steer/say wrappers are transport, not what the user typed
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v);
    let mut out = vec![Ev::You(typed.to_string())];
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
        app.cache.clear();
        app.top = 0;
        app.follow = true;
        app.unseen = 0;
        out.push(Ev::Info("affichage vidé".into()));
    } else if first == "/status" {
        out.push(Ev::Info(format!(
            "modèle {} · {}:{} · seuil de compaction 800000 · session persistante",
            if app.is_live {
                "zai-glm-5-3"
            } else {
                "scripté"
            },
            app.host,
            app.port
        )));
    } else if first == "/help" {
        for c in COMMANDS {
            out.push(Ev::Info(format!("{:<11} — {}", c.name, c.desc)));
        }
        out.push(Ev::Info(
            "texte simple : say implicite (interprété par le harness)".into(),
        ));
    } else if (first == "steer" || first == "/steer") && app.pending {
        // mid-turn steering goes through the FILE side-channel: the
        // harness reads the socket only between turns, but the runtime
        // drains /tmp/bend-steer-<port>.txt at every model/tool safe
        // boundary and commits the text into the running turn (ADR 0005)
        let msg = v
            .strip_prefix("steer ")
            .or_else(|| v.strip_prefix("/steer "))
            .map(|s| s.trim())
            .unwrap_or(typed);
        if msg.is_empty() {
            out.push(Ev::Info("steering vide".into()));
        } else {
            let path = format!("/tmp/bend-steer-{}.txt", app.port);
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
                format!("steering mis en attente : {}", msg)
            } else {
                "steering non écrit (side-channel inaccessible)".to_string()
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

fn draw(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    // OpenCode layout: no header. Feed grows to fill, then one status row,
    // then the prompt block, then one hint row.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    // ---- feed: cached wrapped rows, only the VISIBLE slice rendered ----
    // (a Paragraph over the whole history re-wraps everything each frame
    // and lags long sessions). The column is one char narrower so the
    // scrollbar never covers text.
    let area_w = (chunks[0].width as usize).saturating_sub(1).max(1);
    let area_h = chunks[0].height as usize;
    let text_area = Rect {
        x: chunks[0].x,
        y: chunks[0].y,
        width: chunks[0].width.saturating_sub(1).max(1),
        height: chunks[0].height,
    };

    let n = app.events.len();
    if app.cache.len() < n {
        app.cache.resize_with(n, || None);
    }
    let mut starts: Vec<usize> = Vec::with_capacity(n);
    let mut total_rows = 0usize;
    for i in 0..n {
        let live = matches!(&app.events[i], Ev::Tool(td) if matches!(td.state, ToolState::Run));
        let stale = app.cache[i]
            .as_ref()
            .map_or(true, |c| c.width != area_w as u16 || live);
        if stale {
            let rows = build_rows(&app.events, i, app.debug, area_w, app.tick);
            app.cache[i] = Some(EventRows {
                width: area_w as u16,
                rows,
            });
        }
        starts.push(total_rows);
        total_rows += app.cache[i].as_ref().map(|c| c.rows.len()).unwrap_or(0);
    }

    let max_top = total_rows.saturating_sub(area_h);
    if app.follow {
        app.top = max_top;
    }
    let top = app.top.min(max_top);
    let tail_visible = top + area_h >= total_rows;

    let mut vis: Vec<Line> = Vec::with_capacity(area_h + 2);
    if total_rows > 0 {
        let mut i0 = 0usize;
        for (i, st) in starts.iter().enumerate() {
            if *st <= top {
                i0 = i;
            } else {
                break;
            }
        }
        let mut skip = top - starts[i0];
        for i in i0..n {
            let empty = &Vec::new();
            let rows = app.cache[i].as_ref().map(|c| &c.rows).unwrap_or(empty);
            if skip >= rows.len() {
                skip -= rows.len();
                continue;
            }
            for r in rows.iter().skip(skip) {
                if vis.len() >= area_h {
                    break;
                }
                vis.push(r.clone());
            }
            skip = 0;
            if vis.len() >= area_h {
                break;
            }
        }
    }
    frame.render_widget(Paragraph::new(Text::from(vis)), text_area);

    if total_rows > area_h {
        let mut state = ScrollbarState::new(total_rows)
            .position(top)
            .viewport_content_length(area_h);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[0],
            &mut state,
        );
    }
    app.area_w = area_w;
    app.area_h = area_h;
    app.max_top = max_top;
    app.tail_visible = tail_visible;

    // ---- the status row (the OpenCode prompt status row): back to
    // bottom when pinned, else spinner + cwd while idle
    if !app.tail_visible {
        app.bottom_bar_rect = Some(chunks[1]);
        let mut spans = vec![
            Span::styled(
                "  ↓ Bas ",
                Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
            ),
            Span::styled("(End)", Style::default().fg(DIM)),
        ];
        if app.unseen > 0 {
            spans.push(Span::styled(
                format!("  ·  {} nouvelles lignes", app.unseen),
                Style::default().fg(WARN),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);
    } else {
        app.bottom_bar_rect = None;
        let status = if app.pending && app.connected {
            Line::from(vec![
                Span::styled(
                    format!("  {}", spinner_frame(app.tick / 2)),
                    Style::default().fg(BRAND),
                ),
                Span::styled(
                    if app.is_live {
                        " zai-glm-5-3 · génération…"
                    } else {
                        " réponse…"
                    },
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("esc", Style::default().fg(TEXT)),
                Span::styled(" interrupt", Style::default().fg(DIM)),
            ])
        } else {
            // idle: a static standby dot — the spinner only moves
            // while a turn runs; between turns nothing animates
            Line::from(vec![
                Span::styled("  ● ", Style::default().fg(BRAND)),
                Span::styled(
                    format!(
                        " bend-harness · {}",
                        if app.is_live {
                            "zai-glm-5-3"
                        } else {
                            "scripté"
                        }
                    ),
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("/ commandes", Style::default().fg(TEXT)),
                Span::styled(" · End : bas · Ctrl+C : quitter", Style::default().fg(DIM)),
            ])
        };
        frame.render_widget(Paragraph::new(status), chunks[1]);
    }

    // ---- the prompt: OpenCode prompt (left border ┃, element bg, meta row)
    let chars: Vec<char> = app.input.chars().collect();
    let inner = ((chunks[2].width as usize).saturating_sub(4)).max(1);
    let total = chars.len();
    let view = if total <= inner {
        0
    } else {
        (app.cursor + 1).saturating_sub(inner).min(total - inner)
    };
    let before: String = chars[view..app.cursor.min(total)].iter().collect();
    let at: String = chars
        .get(app.cursor)
        .map(|c| c.to_string())
        .unwrap_or_default();
    let after: String = chars[(app.cursor + 1).min(total)..].iter().collect();
    let mut input_lines = vec![Line::from(vec![
        Span::styled(before, Style::default().fg(TEXT)),
        Span::styled(
            if at.is_empty() { " ".to_string() } else { at },
            Style::default().fg(TEXT).add_modifier(Modifier::REVERSED),
        ),
        Span::styled(after, Style::default().fg(TEXT)),
    ])];
    if total == 0 {
        input_lines.push(Line::from(Span::styled(
            "Ask anything…",
            Style::default().fg(DIM),
        )));
    }
    let meta = Line::from(vec![
        Span::styled("Bend", Style::default().fg(BRAND)),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            if app.is_live {
                "zai-glm-5-3"
            } else {
                "scripté"
            },
            Style::default().fg(TEXT),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            if app.connected {
                "connecté"
            } else {
                "déconnecté"
            },
            Style::default().fg(if app.connected { DIM } else { ERR }),
        ),
    ]);
    input_lines.push(meta);
    let prompt = Paragraph::new(input_lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_set(SPLIT)
            .border_style(Style::default().fg(BRAND))
            .style(Style::default().bg(ELEMENT))
            .padding(Padding::new(2, 2, 1, 1)),
    );
    frame.render_widget(prompt, chunks[2]);

    // ---- slash-command popup (OpenCode autocomplete: split border,
    // backgroundMenu, primary selection)
    let matches = popup_matches(&app.input);
    if !matches.is_empty() {
        let n = matches.len().min(8) as u16;
        let w = 56u16.min(chunks[2].width);
        let area = Rect {
            x: chunks[2].x,
            y: chunks[2].y.saturating_sub(n + 2),
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
                let (name_style, desc_style) = if sel {
                    (
                        Style::default()
                            .bg(BRAND)
                            .fg(TEXT)
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(BRAND).fg(TEXT),
                    )
                } else {
                    (Style::default().fg(TEXT), Style::default().fg(DIM))
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", c.name), name_style),
                    Span::styled(c.desc, desc_style),
                ])
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
    let hint = if app.pending {
        "Entrée : diriger · Tab : mettre en file · / : commandes · Pg↑↓ : défiler · End : bas"
    } else {
        "Entrée : envoyer · / : commandes · Pg↑↓/molette : défiler · End : bas · Ctrl+C : quitter"
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(DIM)))),
        chunks[3],
    );
}

fn run_tui(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
    loop {
        loop {
            match app.rx.try_recv() {
                Ok(line) => {
                    if line == "--- idle" {
                        app.pending = false;
                    }
                    if let Some(ev) = parse_line(&line) {
                        // the view is top-anchored: a pinned view never moves,
                        // a following view re-sticks in draw
                        let appended = push_event(&mut app.events, &mut app.cache, ev);
                        if appended && !app.follow {
                            app.unseen += 1;
                        }
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
        terminal.draw(|f| draw(app, f))?;
        if poll(Duration::from_millis(80))? {
            let ev = read()?;
            if let Event::Mouse(m) = ev {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        app.follow = false;
                        app.top = app.top.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        app.top += 3;
                        if app.top >= app.max_top {
                            app.follow = true;
                            app.unseen = 0;
                        }
                    }
                    // click the back-to-bottom bar to return to the tail
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(r) = app.bottom_bar_rect {
                            let inside = m.column >= r.x
                                && m.column < r.x + r.width
                                && m.row >= r.y
                                && m.row < r.y + r.height;
                            if inside {
                                app.follow = true;
                                app.unseen = 0;
                            }
                        }
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
                        app.cache.clear();
                        app.top = 0;
                        app.follow = true;
                        app.unseen = 0;
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
                        app.follow = false;
                        app.top = app.top.saturating_sub(page);
                    }
                    (KeyCode::PageDown, _) => {
                        let page = (app.area_h / 2).max(1);
                        app.top += page;
                        if app.top >= app.max_top {
                            app.follow = true;
                            app.unseen = 0;
                        }
                    }
                    (KeyCode::End, _) => {
                        if !app.follow {
                            app.follow = true;
                            app.unseen = 0;
                        } else {
                            app.cursor = app.input.chars().count();
                        }
                    }
                    (KeyCode::Tab, _) => {
                        if let Some(c) = sel {
                            // popup completion
                            app.input = format!("{} ", c.name);
                            app.cursor = app.input.chars().count();
                            app.popup_sel = 0;
                        } else if app.pending {
                            // codex queue_keys: queue the draft for after
                            // the turn ("say" forces the message reading
                            // even if the text starts with a protocol word)
                            let v = app.input.trim().to_string();
                            if !v.is_empty() && !v.starts_with('/') {
                                app.input.clear();
                                app.cursor = 0;
                                handle_input(app, &format!("say {}", v));
                            }
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
                            app.follow = true;
                            app.unseen = 0;
                            if !v.is_empty() {
                                // codex semantics: while the agent works,
                                // Enter STEERS the running turn; at idle it
                                // starts one. Commands pass through.
                                let line = if app.pending && !v.starts_with('/') {
                                    format!("steer {}", v)
                                } else {
                                    v
                                };
                                handle_input(app, &line);
                            }
                        }
                    }
                    (KeyCode::Up, _) => {
                        if popup_open {
                            app.popup_sel = (app.popup_sel + matches.len() - 1) % matches.len();
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
                        let b = head.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
                        app.input.replace_range(b..e, "");
                        app.cursor = app.input[..b].chars().count();
                    }
                    (KeyCode::Char(c), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
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
                app.feed_line(&line);
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
                if is_live {
                    "bend runtime/repl-live.bend (et le bridge)"
                } else {
                    "bend runtime/repl.bend"
                }
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
        line_tools: std::collections::HashMap::new(),
        follow: true,
        top: 0,
        max_top: 0,
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
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
