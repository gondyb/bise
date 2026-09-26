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
    KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use unicode_width::UnicodeWidthChar;
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const BRAND: Color = Color::Cyan;
const DIM: Color = Color::Gray;
// tool calls sit one step below DIM: DarkGray (ANSI 90) recedes on dark
// terminals, where Gray (ANSI 37) still glows; status marks stay colored
const TOOL: Color = Color::DarkGray;
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
    Sub { name: String, ok: bool, preview: String },
    // runtime annotations, merged into the matching Tool by id
    ToolInfo { id: u32, name: String, args: String },
    ToolResult { id: u32, ok: bool, preview: String },
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
        return if t.is_empty() { None } else { Some(Ev::Assistant(t.to_string())) };
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
fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize) -> Vec<Line<'static>> {
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
    for l in ev_lines(ev) {
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
    let rest = s[i + pat.len()..].trim_start().strip_prefix(':')?.trim_start();
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
        Ev::Tool(td) => tool_lines(td),
        Ev::Sub { name, ok, preview } => {
            let mut ls = vec![Line::from(vec![
                Span::styled("    ↳ ", Style::default().fg(TOOL)),
                Span::styled(name.clone(), Style::default().fg(TOOL)),
                Span::styled(
                    if *ok { " ok " } else { " ECHEC " },
                    Style::default().fg(if *ok { OK } else { ERR }),
                ),
                Span::styled(truncate_chars(preview.trim(), 100), Style::default().fg(TOOL)),
            ])];
            if ls.first().map(|l| l.width()).unwrap_or(0) == 0 {
                ls.clear();
            }
            ls
        }
        _ => vec![ev_line(ev)],
    }
}

// the tool line: mark + name + elapsed + args preview, then the
// result preview (errors in red — the first thing an engineer looks for)
fn tool_lines(td: &ToolData) -> Vec<Line<'static>> {
    let name = td
        .name
        .clone()
        .unwrap_or_else(|| format!("#{}", td.id));
    let args = td
        .args
        .as_deref()
        .map(|a| args_preview(&name, a))
        .unwrap_or_default();
    let elapsed = fmt_elapsed(td.started);
    let mut ls = Vec::new();
    match td.state {
        ToolState::Run => {
            let mut spans = vec![
                Span::styled("  ● ", Style::default().fg(WARN)),
                Span::styled(name.clone(), Style::default().fg(TOOL)),
                Span::styled(format!(" {}", elapsed), Style::default().fg(TOOL)),
            ];
            if !args.is_empty() {
                spans.push(Span::styled(" — ", Style::default().fg(TOOL)));
                spans.push(Span::styled(args, Style::default().fg(TOOL)));
            }
            ls.push(Line::from(spans));
        }
        ToolState::Ok => {
            let mut spans = vec![
                Span::styled("  ✓ ", Style::default().fg(OK)),
                Span::styled(name.clone(), Style::default().fg(TOOL)),
                Span::styled(
                    format!(" ok {}", td.elapsed.clone().unwrap_or_default()),
                    Style::default().fg(OK),
                ),
            ];
            if !args.is_empty() {
                spans.push(Span::styled(" — ", Style::default().fg(TOOL)));
                spans.push(Span::styled(args, Style::default().fg(TOOL)));
            }
            ls.push(Line::from(spans));
        }
        ToolState::Fail => {
            let mut spans = vec![
                Span::styled("  ✗ ", Style::default().fg(ERR)),
                Span::styled(name.clone(), Style::default().fg(TOOL)),
                Span::styled(
                    format!(" ECHEC {}", td.elapsed.clone().unwrap_or_default()),
                    Style::default().fg(ERR),
                ),
            ];
            if !args.is_empty() {
                spans.push(Span::styled(" — ", Style::default().fg(TOOL)));
                spans.push(Span::styled(args, Style::default().fg(TOOL)));
            }
            ls.push(Line::from(spans));
        }
    }
    if let Some((ok, preview)) = &td.result {
        if !preview.trim().is_empty() {
            ls.push(Line::from(vec![
                Span::styled("    → ", Style::default().fg(TOOL)),
                Span::styled(
                    truncate_chars(preview.trim(), 110),
                    Style::default().fg(if *ok { TOOL } else { ERR }),
                ),
            ]));
        }
    }
    ls
}

fn ev_line(ev: &Ev) -> Line<'static> {
    match ev {
        Ev::Idle => Line::from(""),
        // handled by ev_lines / merged in push_event; safe fallbacks
        Ev::Tool(_) | Ev::Sub { .. } => Line::from(""),
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } => Line::from(""),
        Ev::You(t) => Line::from(Span::styled(
            t.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Ev::Assistant(t) => Line::from(Span::styled(
            t.clone(),
            Style::default().fg(Color::White),
        )),
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
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
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

    // feed: cached wrapped rows, only the VISIBLE slice rendered (the
    // codex transcript approach — a Paragraph over the whole history
    // re-wraps everything each frame and lags long sessions).
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

    // refresh the cache: rebuild on width change, mutation, or for
    // running tools whose elapsed ticks every frame
    let n = app.events.len();
    if app.cache.len() < n {
        app.cache.resize_with(n, || None);
    }
    let mut starts: Vec<usize> = Vec::with_capacity(n);
    let mut total_rows = 0usize;
    for i in 0..n {
        let live = matches!(&app.events[i],
            Ev::Tool(td) if matches!(td.state, ToolState::Run));
        let stale = app.cache[i]
            .as_ref()
            .map_or(true, |c| c.width != area_w as u16 || live);
        if stale {
            let rows = build_rows(&app.events, i, app.debug, area_w);
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

    // the visible slice: the events covering rows [top, top+area_h)
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

    // scrollbar on the right edge of the feed
    if total_rows > area_h {
        let mut state = ScrollbarState::new(total_rows)
            .position(top)
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
    // published for the input loop (paging, bottom detection)
    app.area_w = area_w;
    app.area_h = area_h;
    app.max_top = max_top;
    app.tail_visible = tail_visible;

    // the indicator row doubles as the codex-style back-to-bottom bar:
    // when the tail is scrolled out of view it offers the way back and
    // reports activity that arrived while pinned (clickable)
    if !app.tail_visible {
        app.bottom_bar_rect = Some(chunks[2]);
        let mut spans = vec![
            Span::styled("  ↓ Bas ", Style::default().fg(BRAND).add_modifier(Modifier::BOLD)),
            Span::styled("(End)", Style::default().fg(DIM)),
        ];
        if app.unseen > 0 {
            spans.push(Span::styled(
                format!("  ·  {} nouvelles lignes", app.unseen),
                Style::default().fg(WARN),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[2]);
    } else {
        app.bottom_bar_rect = None;
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
    }

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

    // status bar: the Enter hint follows the turn state (codex semantics)
    let status = if app.pending {
        Line::from(vec![
            Span::styled(
                "Entrée : diriger le tour · Tab : mettre en file · / : commandes · Pg↑↓ : défiler · End : bas · Échap : interrompre",
                Style::default().fg(DIM),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "Entrée : envoyer · / : commandes · Pg↑↓/molette : défiler · End : bas · Ctrl+C : quitter",
                Style::default().fg(DIM),
            ),
        ])
    };
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
                // the view is top-anchored: a pinned view never moves,
                // a following view re-sticks in draw
                let appended = push_event(&mut app.events, &mut app.cache, ev);
                if appended && !app.follow {
                    app.unseen += 1;
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
