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
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ---- the OpenCode theme (opencode.json, dark) ----
// primary #fab283 (the OpenCode orange) is the agent color: user blocks,
// prompt border, spinners, back-to-bottom. Markdown follows the
// markdown* keys; tools follow the inline-tool rules (muted once
// complete, error red).
mod sb;
pub use sb::run_switchboard;

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
const FAINT: Color = Color::Rgb(0x4a, 0x4a, 0x4a); // rails & turn marks — dimmer than textMuted
// the syntax palette of the run_typescript block (OpenCode dark
// syntax colors: purple keywords, green strings, faint comments,
// orange numbers, blue calls; types reuse syntaxType)
const SYNTAX_KEYWORD: Color = Color::Rgb(0xc6, 0x78, 0xdd);
const SYNTAX_STRING: Color = Color::Rgb(0x98, 0xc3, 0x79);
const SYNTAX_COMMENT: Color = Color::Rgb(0x5c, 0x63, 0x70);
const SYNTAX_NUMBER: Color = Color::Rgb(0xd1, 0x9a, 0x66);
const SYNTAX_FUNC: Color = Color::Rgb(0x61, 0xaf, 0xef);
// the apply_patch diff bands (dark tints under the ok / error colors)
const DIFF_ADD_BG: Color = Color::Rgb(0x16, 0x2e, 0x1c);
const DIFF_DEL_BG: Color = Color::Rgb(0x3a, 0x18, 0x1b);

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

// ---- the feed glyph vocabulary: status is symbols, not words ----
const GLYPH_THINK: &str = "✦"; // reasoning section (duration when collapsed)
const GLYPH_OK: &str = "✓"; // success (tool, sub-call, turn)
const GLYPH_ERR: &str = "✗"; // failure
const GLYPH_WARN: &str = "▲"; // warning (interrupt, discarded candidate)
const GLYPH_INFO: &str = "·"; // neutral notice
const GLYPH_COMPACT: &str = "⟳"; // compaction running
const GLYPH_SUMMARY: &str = "≡"; // compaction summary
const GLYPH_BRANCH: &str = "↳"; // preview / sub-result line
const GLYPH_RAIL: &str = "│"; // rail of an expanded reasoning section

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
    // the source of a code tool (run_typescript args JSON, bash raw
    // command), wire-encoded (tool_code annotation); None otherwise
    code: Option<String>,
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
    // the model's reasoning for the message that follows: rendered
    // collapsed as "thought for Ns"; ctrl+t expands every section, a
    // click on the section toggles just that one
    Thinking {
        ms: u128,
        text: String,
        open: bool,
    },
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
    // the source of a code tool (tool_code annotation: run_typescript,
    // bash): the FULL args, wire-encoded, merged into the matching Tool
    // by id
    ToolCode {
        id: u32,
        code: String,
    },
    Turn,
    TurnDone,
    Compact(String),
    Compacted(String),
    Warn(String),
    Err(String),
    Info(String),
    Idle,
    Raw(String),
    // switchboard: a message from another agent (or the user's answer)
    AgentMsg {
        head: String,
        text: String,
    },
    // switchboard: an attention card
    Card(String),
}

// the wire carries the model's reasoning wrapped in think markers inside
// the assistant text (the transport the API re-send depends on); the TUI
// never shows the markers: it splits them into a Thinking section
const THINK_START: &str = "<think>";
const THINK_END: &str = "</think>";

fn split_thinking(s: &str) -> Option<(String, String)> {
    // one reply can carry several thinking blocks: every span joins the
    // section, the text around them stays visible
    let mut think: Vec<&str> = Vec::new();
    let mut visible = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(THINK_START) {
        let after = &rest[start + THINK_START.len()..];
        let Some(end) = after.find(THINK_END) else {
            break;
        };
        visible.push_str(&rest[..start]);
        think.push(&after[..end]);
        rest = &after[end + THINK_END.len()..];
        // the wire carries newlines as a literal backslash-n escape
        if let Some(r) = rest.strip_prefix("\\n") {
            rest = r;
        }
    }
    if think.is_empty() {
        return None;
    }
    visible.push_str(rest);
    Some((think.join("\\n"), visible))
}

// --resume / reload: the REPL replays the restored history as the live
// wire lines, each prefixed "history " (runtime/main.bend replay). Two
// lines exist only there: "you : <text>" (a user message: live, the
// client echoes what it sends) and "injected : <text>" (steering and
// notifications the Core committed). Everything else is a live line.
fn strip_history(line: &str) -> (&str, bool) {
    match line.strip_prefix("history ") {
        Some(rest) => (rest, true),
        None => (line, false),
    }
}

fn parse_history_line(line: &str) -> Option<Ev> {
    if let Some(t) = line.strip_prefix("you : ") {
        return Some(Ev::You(unescape_md(t)));
    }
    if let Some(t) = line.strip_prefix("injected : ") {
        let flat = unescape_md(t).replace('\n', " ");
        return Some(Ev::Info(format!("injecté · {}", truncate_chars(flat.trim(), 110))));
    }
    parse_line(line)
}

// a replayed tool has no meaningful duration (the timing is the replay's)
fn hide_replayed_elapsed(events: &mut [Ev], cache: &mut [Option<EventRows>], id: u32) {
    for (i, e) in events.iter_mut().enumerate().rev() {
        if let Ev::Tool(td) = e {
            if td.id == id {
                td.elapsed = Some(String::new());
                cache[i] = None;
                return;
            }
        }
    }
}

// "2/10 · provider 529 (transient) · retry in 4s" -> the warning the
// user reads while the call waits
fn provider_retry_text(t: &str) -> String {
    let parts: Vec<&str> = t.split(" · ").collect();
    match parts.as_slice() {
        [n, why, wait] => format!(
            "API du modèle en erreur (essai {}) : {} — nouvel essai dans {}",
            n,
            why,
            wait.trim_start_matches("retry in ")
        ),
        _ => format!("API du modèle en erreur : {}", t),
    }
}

fn parse_line(line: &str) -> Option<Ev> {
    if line.is_empty() {
        return None;
    }
    // switchboard: the hub's own lines in a feed
    if let Some(rest) = line.strip_prefix("sb ") {
        return sb::parse_hub_line(rest);
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
    // tool_code #<id> : <full args, wire-encoded> (run_typescript only)
    if let Some(r) = line.strip_prefix("tool_code #") {
        let (id_s, rest) = r.split_once(" : ")?;
        let id: u32 = id_s.trim().parse().ok()?;
        return Some(Ev::ToolCode {
            id,
            code: rest.to_string(),
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
        // BR-003: right after an interrupt the in-flight completion has
        // no turn to land on - expected plumbing, not an error
        if r == "no pending completion" || r == "no pending tool result" {
            return Some(Ev::Info(
                "réponse en vol ignorée (tour interrompu)".into(),
            ));
        }
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
            code: None,
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
            code: None,
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
    if let Some(t) = o.strip_prefix("provider_retry: ") {
        return Some(Ev::Warn(provider_retry_text(t)));
    }
    if let Some(t) = o.strip_prefix("harness_restarted: ") {
        return Some(Ev::Err(format!(
            "le harness a planté ({}) et a redémarré — le tour en cours est interrompu, l'historique est restauré jusqu'au dernier appel au modèle",
            t
        )));
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
            return Some(Ev::TurnDone);
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

// the rows of one event: an optional breathing gap, then the wrapped
// lines. The user block is padded to the full width so its panel
// background reads as a solid block.
fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> Vec<Line<'static>> {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    if !ev_visible(ev, debug) {
        return rows;
    }
    // the previous VISIBLE event decides the gap: a debug-only
    // annotation between two blocks must not swallow the blank line
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    if wants_gap_before(ev, prev) {
        rows.push(Line::from(""));
    }
    let user = matches!(ev, Ev::You(_));
    for l in ev_lines_t(ev, tick, width) {
        for mut r in wrap_line(l, width) {
            if user {
                pad_line_bg(&mut r, width);
            }
            rows.push(r);
        }
    }
    rows
}

// paint the row with the panel background — the line style is the base
// every span patches, so the bar, the text and the padding all sit on
// the panel — then fill the rest of the column, so the user block
// reads as a solid panel the full width (OpenCode style)
fn pad_line_bg(line: &mut Line<'static>, width: usize) {
    line.style = Style::default().bg(PANEL);
    let used: usize = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars())
        .map(|c| c.width().unwrap_or(0))
        .sum();
    if used < width {
        line.spans
            .push(Span::styled(" ".repeat(width - used), Style::default().bg(PANEL)));
    }
}

fn is_message(ev: &Ev) -> bool {
    matches!(ev, Ev::You(_) | Ev::Assistant(_) | Ev::Thinking { .. } | Ev::AgentMsg { .. })
}

fn is_tool_block(ev: &Ev) -> bool {
    matches!(ev, Ev::Tool(_) | Ev::Sub { .. })
}

fn is_notice(ev: &Ev) -> bool {
    matches!(
        ev,
        Ev::Warn(_)
            | Ev::Card(_)
            | Ev::Err(_)
            | Ev::Info(_)
            | Ev::Compact(_)
            | Ev::Compacted(_)
            | Ev::TurnDone
    )
}

// the breathing rules: one blank line when the content kind switches
// (message / tool block / notice). Two exceptions: a reply never
// detaches from its thinking section, and the first event of the feed
// starts flush at the top.
fn wants_gap_before(ev: &Ev, prev: Option<&Ev>) -> bool {
    let Some(p) = prev else {
        return false;
    };
    let prev_message = is_message(p);
    let prev_tool = is_tool_block(p);
    let prev_notice = is_notice(p);
    match ev {
        Ev::Assistant(_) => {
            !matches!(p, Ev::Thinking { .. })
                && (prev_message || prev_tool || prev_notice)
        }
        Ev::Thinking { .. } => prev_message || prev_tool || prev_notice,
        _ if is_tool_block(ev) => prev_message || prev_notice,
        _ if is_notice(ev) => prev_message || prev_tool,
        _ => prev_message || prev_tool || prev_notice,
    }
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
            | Ev::TurnDone
            | Ev::Idle
            | Ev::Raw(_)
            | Ev::ToolInfo { .. }
            | Ev::ToolResult { .. }
            | Ev::ToolCode { .. }
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
        Ev::ToolCode { id, code } => {
            for (i, e) in events.iter_mut().enumerate().rev() {
                if let Ev::Tool(td) = e {
                    if td.id == *id {
                        td.code = Some(code.clone());
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
        Ev::TurnDone | Ev::Idle => {
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
// the first non-empty line of a source, "…" when more lines follow
fn first_line_preview(code: &str) -> String {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut p = truncate_chars(first, 64);
    if code.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
        p.push_str(" …");
    }
    p
}

fn args_preview(name: &str, args: &str) -> String {
    if name == "run_typescript" {
        if let Some(code) = json_str_field(args, "code") {
            return first_line_preview(&code);
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
fn ev_lines_t(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    // the feed twin: identical shape, but the tool spinner animates
    match ev {
        Ev::Tool(td) => tool_lines(td, tick, width),
        other => ev_lines(other, width),
    }
}

// a thinking section: collapsed it is one dim glyph + duration;
// expanded (ctrl+t, or a click) the reasoning shows under a faint rail
fn thinking_lines(ms: u128, text: &str, open: bool) -> Vec<Line<'static>> {
    let head = Line::from(vec![
        Span::styled(format!(" {} ", GLYPH_THINK), Style::default().fg(DIM)),
        Span::styled(
            fmt_think_ms(ms),
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ),
    ]);
    if !open || text.trim().is_empty() {
        return vec![head];
    }
    let mut rows = vec![head];
    for l in unescape_md(text).split('\n') {
        // the BENDSIG line carries the provider signature (the signed
        // thinking transport), never part of the reasoning itself
        if l.starts_with("BENDSIG::") {
            continue;
        }
        rows.push(Line::from(vec![
            Span::styled(format!(" {} ", GLYPH_RAIL), Style::default().fg(FAINT)),
            Span::styled(
                l.to_string(),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ]));
    }
    rows
}

// 800ms -> "0.8s"; 4200ms -> "4.2s"; 12_300ms -> "12s"; 90_000 -> "1m30s"
fn fmt_think_ms(ms: u128) -> String {
    if ms == 0 {
        // no measured duration: a replayed (--resume) section, or lines
        // that arrived in the same batch
        "raisonnement".to_string()
    } else if ms < 10_000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

fn ev_lines(ev: &Ev, width: usize) -> Vec<Line<'static>> {
    match ev {
        Ev::You(t) => user_block_lines(t),
        Ev::Assistant(t) => md_to_lines(&unescape_md(t)),
        Ev::Thinking { ms, text, open } => thinking_lines(*ms, text, *open),
        Ev::Tool(td) => tool_lines(td, 0, width),
        Ev::Idle => vec![Line::from("")],
        Ev::Sub { name, ok, preview } => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_BRANCH), Style::default().fg(FAINT)),
            Span::styled(name.clone(), Style::default().fg(DIM)),
            Span::styled(
                if *ok {
                    format!(" {} ", GLYPH_OK)
                } else {
                    format!(" {} ", GLYPH_ERR)
                },
                Style::default().fg(if *ok { DIM } else { ERR }),
            ),
            Span::styled(
                truncate_chars(preview.trim(), 100),
                Style::default().fg(DIM),
            ),
        ])],
        Ev::Turn => vec![Line::from(vec![
            Span::styled(" ── tour ", Style::default().fg(FAINT)),
            Span::styled("─".repeat(24), Style::default().fg(FAINT)),
        ])],
        Ev::TurnDone => vec![Line::from(vec![
            Span::styled(" └─ ", Style::default().fg(FAINT)),
            Span::styled(GLYPH_OK, Style::default().fg(DIM)),
        ])],
        Ev::Compact(t) => vec![Line::from(vec![
            Span::styled(
                format!("  {} ", GLYPH_COMPACT),
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "compaction ",
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Compacted(t) => vec![Line::from(vec![
            Span::styled(
                format!("  {} ", GLYPH_SUMMARY),
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "résumé ",
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                unescape_md(t),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::Warn(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_WARN), Style::default().fg(WARN)),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Err(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_ERR), Style::default().fg(ERR)),
            Span::styled(t.clone(), Style::default().fg(ERR)),
        ])],
        Ev::Info(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_INFO), Style::default().fg(FAINT)),
            Span::styled(
                t.clone(),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } | Ev::ToolCode { .. } => vec![],
        Ev::Raw(t) => vec![Line::from(Span::styled(
            format!("  {}", t),
            Style::default().fg(DIM),
        ))],
        Ev::AgentMsg { head, text } => {
            let mut rows = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" ◀ ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled(head.clone(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                ]),
            ];
            for l in md_to_lines(text) {
                let mut spans = vec![Span::styled(" │ ", Style::default().fg(ACCENT))];
                spans.extend(l.spans);
                rows.push(Line::from(spans));
            }
            rows
        }
        Ev::Card(t) => vec![Line::from(vec![
            Span::styled("  ◆ ", Style::default().fg(WARN).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("carte {}", t),
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
        ])],
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

// ---- the run_typescript code block ----

// decode the tool_code wire encoding: "\N" newline, "\R" CR, backslash
// doubled (the same reversible encoding the provider wire uses)
fn wire_decode(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < cs.len() {
        if cs[i] == '\\' && i + 1 < cs.len() {
            match cs[i + 1] {
                'N' => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                'R' => {
                    out.push('\r');
                    i += 2;
                    continue;
                }
                '\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

const TS_KEYWORDS: &[&str] = &[
    "abstract", "any", "as", "asserts", "async", "await", "boolean", "break", "case", "catch",
    "class", "const", "continue", "debugger", "declare", "default", "delete", "do", "else",
    "enum", "export", "extends", "false", "finally", "for", "from", "function", "if",
    "implements", "import", "in", "infer", "instanceof", "interface", "is", "keyof", "let",
    "namespace", "never", "new", "null", "number", "object", "of", "private", "protected",
    "public", "readonly", "return", "satisfies", "static", "string", "super", "switch",
    "symbol", "this", "throw", "true", "try", "type", "typeof", "undefined", "unknown", "var",
    "void", "while", "with", "yield",
];

fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

// append one token to the per-line span list, splitting on newlines and
// merging adjacent same-style spans (fewer spans render faster)
fn push_tok(lines: &mut Vec<Vec<Span<'static>>>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    let mut first = true;
    for part in text.split('\n') {
        if !first {
            lines.push(Vec::new());
        }
        first = false;
        if part.is_empty() {
            continue;
        }
        let line = lines.last_mut().expect("push_tok: a line exists");
        if let Some(last) = line.last_mut() {
            if last.style == style {
                last.content.to_mut().push_str(part);
                continue;
            }
        }
        line.push(Span::styled(part.to_string(), style));
    }
}

// tokenize TypeScript into highlighted per-line spans, one pass, char by
// char: keywords, strings and template literals, comments (line and
// block, block may span lines), numbers, call sites, capitalized types
fn highlight_ts(src: &str) -> Vec<Vec<Span<'static>>> {
    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let comment = Style::default().fg(SYNTAX_COMMENT).add_modifier(Modifier::ITALIC);
    let string = Style::default().fg(SYNTAX_STRING);
    let number = Style::default().fg(SYNTAX_NUMBER);
    let keyword = Style::default().fg(SYNTAX_KEYWORD).add_modifier(Modifier::BOLD);
    let func = Style::default().fg(SYNTAX_FUNC);
    let typ = Style::default().fg(HEAD);
    let plain = Style::default().fg(TEXT);
    let punct = Style::default().fg(DIM);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut i = 0usize;
    while i < n {
        let c = cs[i];
        // line comment
        if c == '/' && cs.get(i + 1) == Some(&'/') {
            let mut t = String::new();
            while i < n && cs[i] != '\n' {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // block comment (may span lines; push_tok splits them)
        if c == '/' && cs.get(i + 1) == Some(&'*') {
            let mut t = String::from("/*");
            i += 2;
            while i < n {
                if cs[i] == '*' && cs.get(i + 1) == Some(&'/') {
                    t.push_str("*/");
                    i += 2;
                    break;
                }
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // strings and template literals
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let mut t = String::new();
            t.push(quote);
            i += 1;
            while i < n {
                if cs[i] == '\\' && i + 1 < n {
                    t.push(cs[i]);
                    t.push(cs[i + 1]);
                    i += 2;
                    continue;
                }
                t.push(cs[i]);
                if cs[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            push_tok(&mut lines, &t, string);
            continue;
        }
        // number
        if c.is_ascii_digit() && !cs.get(i.wrapping_sub(1)).is_some_and(|&p| is_id_char(p)) {
            let mut t = String::new();
            while i < n && (cs[i].is_ascii_alphanumeric() || matches!(cs[i], '.' | '_')) {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, number);
            continue;
        }
        // identifier: keyword, call, type or plain name
        if is_id_start(c) {
            let mut t = String::new();
            while i < n && is_id_char(cs[i]) {
                t.push(cs[i]);
                i += 1;
            }
            let mut k = i;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let style = if TS_KEYWORDS.contains(&t.as_str()) {
                keyword
            } else if cs.get(k) == Some(&'(') {
                func
            } else if t.starts_with(|ch: char| ch.is_uppercase()) {
                typ
            } else {
                plain
            };
            push_tok(&mut lines, &t, style);
            continue;
        }
        // any other char is punctuation
        push_tok(&mut lines, &c.to_string(), punct);
        i += 1;
    }
    lines
}

// ---- the bash code block ----

const BASH_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until",
    "do", "done", "in", "function", "time", "!", "[[", "]]", "coproc",
];

// keywords after which the next word is a command again
const BASH_CMD_AFTER: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "time", "!", "coproc",
];

fn is_bash_word_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '|' | '&' | ';' | '<' | '>' | '(' | ')' | '"' | '\'' | '`' | '$')
}

// one $-expansion starting at cs[i] == '$': ${...}, $name, $1, $?, ...
// "$(" is not an expansion (the caller treats it as a command start).
// Returns the token and the index after it.
fn bash_var(cs: &[char], i: usize) -> Option<(String, usize)> {
    let n = cs.len();
    match cs.get(i + 1) {
        Some('{') => {
            let mut j = i + 2;
            while j < n && cs[j] != '}' && cs[j] != '\n' {
                j += 1;
            }
            let end = if j < n && cs[j] == '}' { j + 1 } else { j };
            Some((cs[i..end].iter().collect(), end))
        }
        Some(&c) if c.is_ascii_alphabetic() || c == '_' => {
            let mut j = i + 1;
            while j < n && (cs[j].is_ascii_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            Some((cs[i..j].iter().collect(), j))
        }
        Some(&c) if c.is_ascii_digit() || matches!(c, '?' | '@' | '#' | '$' | '!' | '*' | '-') => {
            Some((cs[i..i + 2].iter().collect(), i + 2))
        }
        _ => None,
    }
}

// tokenize bash into highlighted per-line spans, one pass: comments,
// strings (with $-expansions inside double quotes), variables,
// keywords, the command word of each simple command, options, numbers
// and operators (pipes, lists, redirections)
fn highlight_bash(src: &str) -> Vec<Vec<Span<'static>>> {
    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let comment = Style::default().fg(SYNTAX_COMMENT).add_modifier(Modifier::ITALIC);
    let string = Style::default().fg(SYNTAX_STRING);
    let number = Style::default().fg(SYNTAX_NUMBER);
    let var = Style::default().fg(SYNTAX_NUMBER);
    let keyword = Style::default().fg(SYNTAX_KEYWORD).add_modifier(Modifier::BOLD);
    let op = Style::default().fg(SYNTAX_KEYWORD);
    let func = Style::default().fg(SYNTAX_FUNC);
    let plain = Style::default().fg(TEXT);
    let option = Style::default().fg(HEAD);
    let punct = Style::default().fg(DIM);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    // the next word is a command name (start, after ; | && || ( $( ...)
    let mut cmd_pos = true;
    // "for x in", "case x in": the "in" after the name is a keyword
    let mut want_in = false;
    let mut i = 0usize;
    while i < n {
        let c = cs[i];
        if c == '\n' {
            push_tok(&mut lines, "\n", plain);
            cmd_pos = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            push_tok(&mut lines, &c.to_string(), plain);
            i += 1;
            continue;
        }
        // comment: '#' at the start of a word
        if c == '#' && (i == 0 || cs[i - 1].is_whitespace() || matches!(cs[i - 1], ';' | '(' | '|' | '&')) {
            let mut t = String::new();
            while i < n && cs[i] != '\n' {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // single quotes: literal to the closing quote
        if c == '\'' {
            let mut t = String::from("'");
            i += 1;
            while i < n {
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '\'' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            cmd_pos = false;
            continue;
        }
        // double quotes: $-expansions keep their own color
        if c == '"' {
            let mut t = String::from("\"");
            i += 1;
            while i < n {
                if cs[i] == '\\' && i + 1 < n {
                    t.push(cs[i]);
                    t.push(cs[i + 1]);
                    i += 2;
                    continue;
                }
                if cs[i] == '$' {
                    if let Some((v, j)) = bash_var(&cs, i) {
                        push_tok(&mut lines, &t, string);
                        t.clear();
                        push_tok(&mut lines, &v, var);
                        i = j;
                        continue;
                    }
                }
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '"' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            cmd_pos = false;
            continue;
        }
        // backquotes: a command substitution, shown as a string
        if c == '`' {
            let mut t = String::from("`");
            i += 1;
            while i < n {
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '`' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            continue;
        }
        if c == '$' {
            // $( and $(( open a new command / arithmetic
            if cs.get(i + 1) == Some(&'(') {
                push_tok(&mut lines, "$(", op);
                i += 2;
                cmd_pos = true;
                continue;
            }
            if let Some((v, j)) = bash_var(&cs, i) {
                push_tok(&mut lines, &v, var);
                i = j;
                cmd_pos = false;
                continue;
            }
            push_tok(&mut lines, "$", plain);
            i += 1;
            continue;
        }
        // operators: lists, pipes, subshells, redirections
        if matches!(c, '|' | '&' | ';' | '(' | ')' | '<' | '>') {
            let mut t = String::new();
            t.push(c);
            i += 1;
            while i < n && matches!(cs[i], '|' | '&' | ';' | '<' | '>') && t.len() < 3 {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, op);
            // a redirection target is an argument; everything else
            // starts a new command
            cmd_pos = !(t.contains('<') || t.contains('>') || t == ")");
            continue;
        }
        // a word
        let mut t = String::new();
        while i < n && is_bash_word_char(cs[i]) {
            if cs[i] == '\\' && i + 1 < n {
                t.push(cs[i]);
                t.push(cs[i + 1]);
                i += 2;
                continue;
            }
            t.push(cs[i]);
            i += 1;
        }
        if t.is_empty() {
            // a lone special char the branches above did not take
            push_tok(&mut lines, &c.to_string(), punct);
            i += 1;
            continue;
        }
        let w = t.as_str();
        if want_in && w == "in" {
            push_tok(&mut lines, w, keyword);
            want_in = false;
            cmd_pos = false;
        } else if (cmd_pos && BASH_KEYWORDS.contains(&w)) || matches!(w, "]]" | "{" | "}") {
            push_tok(&mut lines, w, keyword);
            want_in = matches!(w, "for" | "case" | "select");
            cmd_pos = BASH_CMD_AFTER.contains(&w) || w == "{";
        } else if cmd_pos
            && w.find('=').is_some_and(|k| {
                k > 0 && w[..k].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            })
        {
            // NAME=value before the command: still in command position
            let k = w.find('=').unwrap_or(0);
            push_tok(&mut lines, &w[..k], var);
            push_tok(&mut lines, "=", punct);
            push_tok(&mut lines, &w[k + 1..], plain);
        } else if cmd_pos {
            push_tok(&mut lines, w, func);
            cmd_pos = false;
        } else if w.starts_with('-') && w.len() > 1 {
            push_tok(&mut lines, w, option);
        } else if w.chars().all(|ch| ch.is_ascii_digit()) {
            push_tok(&mut lines, w, number);
        } else {
            push_tok(&mut lines, w, plain);
        }
    }
    lines
}

// ---- the apply_patch diff block ----

// the V4A patch as a diff: file headers, hunk markers, added lines on a
// green band, removed lines on a red band, context dimmed. The
// Begin/End Patch envelope is noise and never shows.
fn highlight_patch(src: &str) -> Vec<Vec<Span<'static>>> {
    let file = |glyph: &str, path: &str, color: Color, note: &str| {
        let mut v = vec![
            Span::styled(format!("{} ", glyph), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(path.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ];
        if !note.is_empty() {
            v.push(Span::styled(format!(" · {}", note), Style::default().fg(DIM)));
        }
        v
    };
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    for l in src.split('\n') {
        let header = if let Some(p) = l.strip_prefix("*** Update File: ") {
            Some(file("~", p, HEAD, ""))
        } else if let Some(p) = l.strip_prefix("*** Add File: ") {
            Some(file("+", p, OK, "nouveau"))
        } else { l.strip_prefix("*** Delete File: ").map(|p| file("−", p, ERR, "supprimé")) };
        if let Some(h) = header {
            // a blank row between two files
            if !lines.is_empty() {
                lines.push(Vec::new());
            }
            lines.push(h);
            continue;
        }
        if let Some(p) = l.strip_prefix("*** Move to: ") {
            lines.push(file("→", p, HEAD, "renommé"));
            continue;
        }
        if l.starts_with("*** ") || (l.is_empty() && lines.is_empty()) {
            // Begin Patch, End Patch, End of File
            continue;
        }
        let span = if l.starts_with("@@") {
            Span::styled(l.to_string(), Style::default().fg(SYNTAX_FUNC))
        } else if l.starts_with('+') {
            Span::styled(l.to_string(), Style::default().fg(OK).bg(DIFF_ADD_BG))
        } else if l.starts_with('-') {
            Span::styled(l.to_string(), Style::default().fg(ERR).bg(DIFF_DEL_BG))
        } else {
            Span::styled(l.to_string(), Style::default().fg(DIM))
        };
        lines.push(vec![span]);
    }
    // a trailing blank (the text's final newline) is not a row
    while lines.last().is_some_and(|l| l.iter().all(|s| s.content.trim().is_empty())) {
        lines.pop();
    }
    lines
}

// the tool-line preview of a patch: each file with its line counts,
// "core/obs.bend +3 −1, LAWS.bend +12"
fn patch_summary(src: &str) -> String {
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for l in src.split('\n') {
        let path = l
            .strip_prefix("*** Update File: ")
            .or_else(|| l.strip_prefix("*** Add File: "))
            .or_else(|| l.strip_prefix("*** Delete File: "));
        if let Some(p) = path {
            files.push((p.trim().to_string(), 0, 0));
            continue;
        }
        if let Some(p) = l.strip_prefix("*** Move to: ") {
            if let Some(f) = files.last_mut() {
                f.0 = format!("{} → {}", f.0, p.trim());
            }
            continue;
        }
        if let Some(f) = files.last_mut() {
            if l.starts_with('+') {
                f.1 += 1;
            } else if l.starts_with('-') {
                f.2 += 1;
            }
        }
    }
    let parts: Vec<String> = files
        .into_iter()
        .map(|(p, add, del)| {
            let mut s = p;
            if add > 0 {
                s.push_str(&format!(" +{}", add));
            }
            if del > 0 {
                s.push_str(&format!(" −{}", del));
            }
            s
        })
        .collect();
    truncate_chars(&parts.join(", "), 80)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CodeLang {
    TypeScript,
    Bash,
    Patch,
}

// the code tools: which ones render a source block, in which language
fn code_lang(tool: &str) -> Option<CodeLang> {
    match tool {
        "run_typescript" => Some(CodeLang::TypeScript),
        "bash" => Some(CodeLang::Bash),
        "apply_patch" => Some(CodeLang::Patch),
        _ => None,
    }
}

// the source a code tool ran, from its tool_code args: the "code" field
// of the run_typescript JSON, the raw command for bash
fn tool_source(lang: CodeLang, decoded: String) -> String {
    match lang {
        CodeLang::TypeScript => json_str_field(&decoded, "code").unwrap_or(decoded),
        CodeLang::Bash | CodeLang::Patch => decoded,
    }
}

// wrap one line of spans into rows of at most `w` display columns, so
// the block shows the whole source. A row breaks after its last
// whitespace when that keeps at least half the row; otherwise (a long
// token) it breaks hard at the width. Returns each row with its width.
fn wrap_code_line(spans: &[Span<'static>], w: usize) -> Vec<(Vec<Span<'static>>, usize)> {
    let w = w.max(1);
    let cells: Vec<(char, Style, usize)> = spans
        .iter()
        .flat_map(|sp| {
            sp.content
                .chars()
                .map(move |ch| (ch, sp.style, ch.width().unwrap_or(1).max(1)))
        })
        .collect();
    let mut rows = Vec::new();
    let mut start = 0usize;
    while start < cells.len() {
        // the longest run that fits
        let mut end = start;
        let mut used = 0usize;
        while end < cells.len() && used + cells[end].2 <= w {
            used += cells[end].2;
            end += 1;
        }
        if end == start {
            // a single cell wider than the row: take it anyway
            end = start + 1;
        } else if end < cells.len() {
            // prefer breaking after the last whitespace in the row
            if let Some(k) = (start..end).rev().find(|&k| cells[k].0.is_whitespace()) {
                let before: usize = cells[start..=k].iter().map(|c| c.2).sum();
                if before * 2 >= w {
                    end = k + 1;
                }
            }
        }
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut row_w = 0usize;
        for &(ch, style, cw) in &cells[start..end] {
            row_w += cw;
            match row.last_mut() {
                Some(last) if last.style == style => last.content.to_mut().push(ch),
                _ => row.push(Span::styled(ch.to_string(), style)),
            }
        }
        rows.push((row, row_w));
        start = end;
    }
    if rows.is_empty() {
        // an empty source line still gets its row
        rows.push((Vec::new(), 0));
    }
    rows
}

// the code block: a rounded border (orange while the tool runs, dim once
// done, red on failure), a header, a line-number gutter, and the whole
// source: a long line wraps inside the box, its continuation rows with
// an empty gutter (the numbers mark where each source line starts)
fn code_block_lines(
    code: &str,
    lang: CodeLang,
    state: &ToolState,
    width: usize,
) -> Vec<Line<'static>> {
    let hl = match lang {
        CodeLang::TypeScript => highlight_ts(code),
        CodeLang::Bash => highlight_bash(code),
        CodeLang::Patch => highlight_patch(code),
    };
    // a diff has no meaningful line numbers: no gutter
    let numbered = lang != CodeLang::Patch;
    if hl.is_empty() {
        return Vec::new();
    }
    let border = match state {
        ToolState::Run => BRAND,
        ToolState::Ok => BORDER_ACTIVE,
        ToolState::Fail => ERR,
    };
    let bstyle = Style::default().fg(border);
    // 2 feed margin, "│ " and "│" borders, 1 right pad, and when
    // numbered the gutter with its " │ " separator
    let gutter_w = hl.len().to_string().len();
    let overhead = if numbered { gutter_w + 9 } else { 6 };
    let cw = width.saturating_sub(overhead).max(8);
    let inner = if numbered { gutter_w + cw + 5 } else { cw + 2 };
    let mut rows: Vec<Line<'static>> = Vec::new();
    // top: "  ╭─ typescript ───…───╮" (or "─ bash ")
    let header = match lang {
        CodeLang::TypeScript => "─ typescript ",
        CodeLang::Bash => "─ bash ",
        CodeLang::Patch => "─ diff ",
    };
    let fill = inner.saturating_sub(header.width()).max(1);
    rows.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("╭", bstyle),
        Span::styled(header.to_string(), bstyle),
        Span::styled("─".repeat(fill), bstyle),
        Span::styled("╮", bstyle),
    ]));
    for (i, spans) in hl.iter().enumerate() {
        // a background band (diff lines) runs to the right border
        let band = spans
            .first()
            .and_then(|sp| sp.style.bg)
            .map_or(Style::default(), |bg| Style::default().bg(bg));
        for (r, (content, used)) in wrap_code_line(spans, cw).into_iter().enumerate() {
            let mut ls = vec![
                Span::styled("  ", Style::default()),
                Span::styled("│ ", bstyle),
            ];
            if numbered {
                let gutter = if r == 0 {
                    format!("{:>gw$}", i + 1, gw = gutter_w)
                } else {
                    " ".repeat(gutter_w)
                };
                ls.push(Span::styled(gutter, Style::default().fg(FAINT)));
                ls.push(Span::styled(" │ ", bstyle));
            }
            ls.extend(content);
            // pad to the row width, plus the 1-column right pad
            ls.push(Span::styled(" ".repeat(cw - used.min(cw) + 1), band));
            ls.push(Span::styled("│", bstyle));
            rows.push(Line::from(ls));
        }
    }
    rows.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("╰", bstyle),
        Span::styled("─".repeat(inner), bstyle),
        Span::styled("╯", bstyle),
    ]));
    rows
}

// the tool line, OpenCode inline-tool style: 2-col icon, name,
// elapsed, args preview; result preview on the next line. The state is
// a glyph: braille spinner while running, green ✓ once ok, red ✗ on
// failure.
// " 1.2s" after the tool name; nothing for a replayed tool (no duration)
fn elapsed_label(elapsed: &Option<String>) -> String {
    match elapsed.as_deref() {
        Some(e) if !e.is_empty() => format!(" {}", e),
        _ => String::new(),
    }
}

fn tool_lines(td: &ToolData, tick: u32, width: usize) -> Vec<Line<'static>> {
    let name = td.name.clone().unwrap_or_else(|| format!("#{}", td.id));
    // the source of a code tool (run_typescript, bash, apply_patch), when
    // the runtime sent it: rendered in full, highlighted, under the line
    let code = match (&td.code, code_lang(&name)) {
        (Some(raw), Some(lang)) => {
            Some((lang, tool_source(lang, wire_decode(raw)))).filter(|(_, c)| !c.trim().is_empty())
        }
        _ => None,
    };
    let args = match &code {
        // the block shows the whole source: a gray copy on the tool line
        // would only repeat it
        Some((CodeLang::Bash | CodeLang::TypeScript, _)) => String::new(),
        // a patch keeps its one-line summary (the files it touches)
        Some((CodeLang::Patch, src)) => patch_summary(src),
        None => td
            .args
            .as_deref()
            .map(|a| args_preview(&name, a))
            .unwrap_or_default(),
    };
    let elapsed = fmt_elapsed(td.started);
    let args_span = |st: Style| {
        Span::styled(
            if args.is_empty() {
                String::new()
            } else {
                format!(" · {}", args)
            },
            st,
        )
    };
    let mut ls = Vec::new();
    match td.state {
        ToolState::Run => ls.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(spinner_frame(tick / 2), Style::default().fg(BRAND)),
            Span::styled(format!(" {}", name), Style::default().fg(TEXT)),
            Span::styled(format!(" {}", elapsed), Style::default().fg(DIM)),
            args_span(Style::default().fg(DIM)),
        ])),
        ToolState::Ok => ls.push(Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_OK), Style::default().fg(OK)),
            Span::styled(name.clone(), Style::default().fg(TOOL)),
            Span::styled(elapsed_label(&td.elapsed), Style::default().fg(TOOL)),
            args_span(Style::default().fg(TOOL)),
        ])),
        ToolState::Fail => ls.push(Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_ERR), Style::default().fg(ERR)),
            Span::styled(name.clone(), Style::default().fg(ERR)),
            Span::styled(elapsed_label(&td.elapsed), Style::default().fg(ERR)),
            args_span(Style::default().fg(ERR)),
        ])),
    }
    if let Some((ok, preview)) = &td.result {
        if !preview.trim().is_empty() {
            ls.push(Line::from(vec![
                Span::styled(format!("    {} ", GLYPH_BRANCH), Style::default().fg(TOOL)),
                Span::styled(
                    truncate_chars(preview.trim(), 110),
                    Style::default().fg(if *ok { TOOL } else { ERR }),
                ),
            ]));
        }
    }
    // the source block: the FULL code, highlighted, in a bordered box
    if let Some((lang, src)) = &code {
        ls.extend(code_block_lines(src, *lang, &td.state, width));
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
    // when the last wire line arrived (thinking duration = the delta to
    // the assistant line) and the ctrl+t thinking-section toggle
    last_line_at: Option<std::time::Instant>,
    show_thinking: bool,
    // a Ctrl+C interrupt is in flight (until the dying turn's idle):
    // a second Ctrl+C quits instead of interrupting again
    interrupt_requested: bool,
    pending: bool,
    input: String,
    cursor: usize, // char index into input
    popup_sel: usize,
    history: Vec<String>,
    hist_idx: Option<usize>,
    tick: u32,
    info: HarnessInfo,
    host: String,
    port: u16,
    session_id: String,
    stream: Option<TcpStream>,
    rx: Receiver<String>,
    should_quit: bool,
    /// Switchboard mode (projects/switchboard): the hub connection and
    /// the feeds out of focus.
    sb: Option<sb::Sb>,
}

impl App {
    fn send(&mut self, line: &str) {
        self.pending = true;
        // the socket protocol is line-oriented: real newlines in the
        // composer escape to a literal backslash-n (the REPL unescapes
        // the say/steer text; the message carries the real newlines)
        let wire = line.replace('\n', "\\n");
        if let Some(s) = self.stream.as_mut() {
            let _ = s.write_all(format!("{}\n", wire).as_bytes());
        }
    }

    // line mode: one printed line per finished tool, carrying the merged
    // annotations; sub-calls print live as they complete
    fn feed_line(&mut self, line: &str) {
        if line == "--- idle" {
            self.pending = false;
        }
        // every wire line moves the timing reference: a thinking
        // duration is the delta from the previous line's arrival
        let now = std::time::Instant::now();
        let ms = self
            .last_line_at
            .map_or(0, |t0| now.duration_since(t0).as_millis());
        self.last_line_at = Some(now);
        let (line, replayed) = strip_history(line);
        let parsed = if replayed {
            parse_history_line(line)
        } else {
            parse_line(line)
        };
        let Some(ev) = parsed else { return };
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
            Ev::ToolCode { id, code } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.code = Some(code);
                }
            }
            Ev::Tool(td) => {
                if let Some(mut held) = self.line_tools.remove(&td.id) {
                    held.state = td.state;
                    if held.elapsed.is_none() {
                        held.elapsed = Some(fmt_elapsed(held.started));
                    }
                    print_ev_of(&Ev::Tool(held), self.debug, self.area_w);
                } else {
                    print_ev_of(&Ev::Tool(td), self.debug, self.area_w);
                }
            }
            ev2 @ (Ev::TurnDone | Ev::Idle) => {
                // abandoned running tools get one final line
                let mut held: Vec<ToolData> = self.line_tools.drain().map(|(_, td)| td).collect();
                held.sort_by_key(|td| td.id);
                for mut td in held {
                    td.state = ToolState::Fail;
                    td.elapsed = Some(fmt_elapsed(td.started));
                    if td.result.is_none() {
                        td.result = Some((false, "interrompu".to_string()));
                    }
                    print_ev_of(&Ev::Tool(td), self.debug, self.area_w);
                }
                print_ev_of(&ev2, self.debug, self.area_w);
            }
            Ev::Assistant(t) => {
                // the line mode shows the same collapsed section: the
                // raw reasoning never prints (unless --debug)
                match split_thinking(&t) {
                    Some((think, vis)) => {
                        print_ev_of(
                            &Ev::Thinking {
                                ms,
                                text: think.to_string(),
                                open: self.debug,
                            },
                            self.debug,
                            self.area_w,
                        );
                        if !vis.trim().is_empty() {
                            print_ev_of(&Ev::Assistant(vis.to_string()), self.debug, self.area_w);
                        }
                    }
                    None => print_ev_of(&Ev::Assistant(t), self.debug, self.area_w),
                }
            }
            other => print_ev_of(&other, self.debug, self.area_w),
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
            "modèle {} · {}:{} · seuil de compaction {} · session {}",
            app.info.model,
            app.host,
            app.port,
            app.info.threshold,
            app.session_id
        )));
    } else if first == "/help" {
        for c in COMMANDS {
            out.push(Ev::Info(format!("{:<11} — {}", c.name, c.desc)));
        }
        out.push(Ev::Info(
            "texte simple : nouveau message · pendant un tour, ⏎ dirige le modèle et Tab met en file".into(),
        ));
        out.push(Ev::Info(
            "glyphes : ✦ raisonnement · ✓ ok · ✗ échec · ▲ alerte · ⟳ compaction · ≡ résumé · ↳ aperçu".into(),
        ));
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
                "interrompu — le tour en cours s'arrête au prochain point sûr".into()
            } else {
                "interruption non écrite (side-channel inaccessible)".into()
            }));
        } else {
            out.push(Ev::Info("aucun tour en cours à interrompre".into()));
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
    let (area, sb_panel) = sb::split(app, frame.area());
    if let Some(p) = sb_panel {
        sb::draw_panel(app, frame, p);
    }
    // OpenCode layout: no header. Feed grows to fill, a blank row, the
    // status row, another blank row, the prompt block, a blank row,
    // then the hint row — the composer never touches the history.
    // the composer grows with its content (a pasted multi-line block),
    // capped at half the screen so the feed always survives
    let inner_w = ((area.width as usize).saturating_sub(6)).max(1);
    let composer_rows = {
        let mut rows = 1usize;
        let mut col = 0usize;
        for c in app.input.chars() {
            if c == '\n' {
                rows += 1;
                col = 0;
            } else {
                col += 1;
                if col > inner_w {
                    rows += 1;
                    col = 1;
                }
            }
        }
        rows
    };
    // the prompt block holds: 2 rows of top padding, the typed text,
    // one blank line, the meta row, 1 row of bottom padding
    let input_h = ((composer_rows + 5) as u16).min((area.height / 2).max(7));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3), // feed
            Constraint::Length(1), // respiration sous le feed
            Constraint::Length(1), // status row
            Constraint::Length(1), // respiration au-dessus du composeur
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
    let mut starts: Vec<usize> = Vec::with_capacity(n);
    let mut total_rows = 0usize;
    for i in 0..n {
        let live = matches!(&app.events[i], Ev::Tool(td) if matches!(td.state, ToolState::Run));
        let stale = app.cache[i]
            .as_ref()
            .is_none_or(|c| c.width != area_w as u16 || live);
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
                format!("  ·  {} nouvelles lignes", app.unseen),
                Style::default().fg(WARN),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[2]);
    } else {
        app.bottom_bar_rect = None;
        let status = if let Some(l) = sb::status_line(app) {
            l
        } else if app.pending && app.connected {
            Line::from(vec![
                Span::styled(
                    format!("  {}", spinner_frame(app.tick / 2)),
                    Style::default().fg(BRAND),
                ),
                Span::styled(
                    format!(" {} · génération…", app.info.model),
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
                Span::styled(" · End : bas · Ctrl+C : quitter", Style::default().fg(DIM)),
            ])
        };
        frame.render_widget(Paragraph::new(status), chunks[2]);
    }

    // ---- the prompt: OpenCode prompt (left border ┃, element bg, meta row)
    // multi-line: newlines break rows, long rows wrap at the inner
    // width, the cursor is the REVERSED char (or a REVERSED space at
    // the end of the input)
    let chars: Vec<char> = app.input.chars().collect();
    let inner = ((chunks[4].width as usize).saturating_sub(6)).max(1);
    let total = chars.len();
    let mut input_lines: Vec<Line> = Vec::new();
    if total == 0 {
        input_lines.push(Line::from(Span::styled(
            sb::placeholder(app).unwrap_or_else(|| "Ask anything…".to_string()),
            Style::default().fg(DIM),
        )));
    } else {
        let mut spans: Vec<Span> = Vec::new();
        let mut buf = String::new();
        let mut col = 0usize;
        let flush_plain = |spans: &mut Vec<Span>, buf: &mut String| {
            if !buf.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(buf),
                    Style::default().fg(TEXT),
                ));
            }
        };
        for (i, c) in chars.iter().enumerate() {
            if *c == '\n' {
                flush_plain(&mut spans, &mut buf);
                if i == app.cursor {
                    spans.push(Span::styled(
                        " ".to_string(),
                        Style::default().fg(TEXT).add_modifier(Modifier::REVERSED),
                    ));
                }
                input_lines.push(Line::from(std::mem::take(&mut spans)));
                spans = Vec::new();
                col = 0;
                continue;
            }
            // the cursor char counts toward the row width like any
            // other char: a row may overflow by one otherwise
            let is_cursor = i == app.cursor;
            if is_cursor {
                flush_plain(&mut spans, &mut buf);
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(TEXT).add_modifier(Modifier::REVERSED),
                ));
            } else {
                buf.push(*c);
            }
            col += 1;
            if col >= inner {
                flush_plain(&mut spans, &mut buf);
                input_lines.push(Line::from(std::mem::take(&mut spans)));
                spans = Vec::new();
                col = 0;
            }
        }
        if app.cursor >= total {
            flush_plain(&mut spans, &mut buf);
            spans.push(Span::styled(
                " ".to_string(),
                Style::default().fg(TEXT).add_modifier(Modifier::REVERSED),
            ));
        } else {
            flush_plain(&mut spans, &mut buf);
        }
        if !spans.is_empty() {
            input_lines.push(Line::from(spans));
        }
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
            if app.input.contains('\n') {
                "⏎ envoyer · ⇧⏎ ligne"
            } else {
                "⇧⏎ ligne"
            },
            Style::default().fg(DIM),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        if app.connected {
            Span::styled("●", Style::default().fg(DIM))
        } else {
            Span::styled("○ déconnecté", Style::default().fg(ERR))
        },
    ]);
    // one blank line between the typed text and the meta row
    input_lines.push(Line::from(""));
    input_lines.push(meta);
    let prompt = Paragraph::new(input_lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_set(SPLIT)
            .border_style(Style::default().fg(BRAND))
            .style(Style::default().bg(ELEMENT))
            .padding(Padding::new(3, 2, 2, 1)),
    );
    frame.render_widget(prompt, chunks[4]);

    // ---- slash-command popup (OpenCode autocomplete: split border,
    // backgroundMenu, primary selection)
    let matches = popup_matches(&app.input);
    if !matches.is_empty() {
        let n = matches.len().min(8) as u16;
        let w = 56u16.min(chunks[4].width);
        let area = Rect {
            x: chunks[4].x,
            y: chunks[4].y.saturating_sub(n + 2),
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
    let hint = if let Some(h) = sb::hint(app) {
        h
    } else if app.pending {
        "⏎ diriger · Tab file · Ctrl+C interrompre · / commandes · End bas"
    } else {
        "⏎ envoyer · Maj+⏎/Ctrl+J nouvelle ligne · / commandes · Ctrl+T raisonnement · Ctrl+C quitter"
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(DIM)))),
        chunks[6],
    );
}

// ---- multi-line composer navigation ----
// The cursor is a char index over the whole input; Up/Down move it to
// the same column on the previous/next line (clamped to that line).

fn cursor_pos(input: &str, cursor: usize) -> (usize, usize) {
    // (line, column) of the cursor
    let mut line = 0usize;
    let mut col = 0usize;
    for c in input.chars().take(cursor) {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn line_bounds(input: &str) -> Vec<(usize, usize)> {
    // (start, len-without-newline) of every line
    let chars: Vec<char> = input.chars().collect();
    let mut out = vec![(0usize, 0usize)];
    let mut start = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            out.last_mut().unwrap().1 = i - start;
            start = i + 1;
            out.push((start, 0));
        }
    }
    out.last_mut().unwrap().1 = chars.len() - start;
    out
}

fn cursor_line_up(input: &str, cursor: &mut usize) {
    let (_, col) = cursor_pos(input, *cursor);
    let bounds = line_bounds(input);
    // find the current line index
    let mut cur = 0usize;
    for (i, (st, _)) in bounds.iter().enumerate() {
        if *st <= *cursor {
            cur = i;
        }
    }
    if cur == 0 {
        return;
    }
    let (prev_start, prev_len) = bounds[cur - 1];
    *cursor = prev_start + col.min(prev_len);
}

fn cursor_line_down(input: &str, cursor: &mut usize) {
    let (_, col) = cursor_pos(input, *cursor);
    let bounds = line_bounds(input);
    let mut cur = 0usize;
    for (i, (st, _)) in bounds.iter().enumerate() {
        if *st <= *cursor {
            cur = i;
        }
    }
    if cur + 1 >= bounds.len() {
        return;
    }
    let (next_start, next_len) = bounds[cur + 1];
    *cursor = next_start + col.min(next_len);
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
        loop {
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
        terminal.draw(|f| {
            if app.sb.is_some() {
                sb::draw_sb(app, f)
            } else {
                draw(app, f)
            }
        })?;
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
                    // click the back-to-bottom bar to return to the tail;
                    // click a thinking section to expand/collapse it
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
                        // the feed starts at the top of the terminal:
                        // the clicked terminal row IS the feed row
                        if m.row as usize >= app.area_h {
                            continue;
                        }
                        // the same math as draw: content row = the
                        // scroll offset + the feed row, then the event
                        // whose row range contains it
                        let top = app.top.min(app.max_top);
                        let row = top + m.row as usize;
                        let mut acc = 0usize;
                        for i in 0..app.events.len() {
                            let len = app
                                .cache
                                .get(i)
                                .and_then(|c| c.as_ref())
                                .map_or(0, |c| c.rows.len());
                            if row < acc + len {
                                if let Ev::Thinking { open, .. } = &mut app.events[i] {
                                    *open = !*open;
                                    if let Some(c) = app.cache.get_mut(i) {
                                        *c = None;
                                    }
                                }
                                break;
                            }
                            acc += len;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if let Event::Paste(text) = ev {
                // normalize CRLF/CR so a terminal paste behaves like the
                // typed newline, then insert at the cursor
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                let b = byte_at_char(&app.input, app.cursor);
                app.input.insert_str(b, &text);
                app.cursor += text.chars().count();
                app.hist_idx = None;
                app.popup_sel = 0;
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
                                    "interrompu — le tour en cours s'arrête au prochain point sûr · Ctrl+C à nouveau pour quitter".to_string()
                                } else {
                                    "interruption non écrite (side-channel inaccessible) — Ctrl+C à nouveau pour quitter".to_string()
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
                        app.top = 0;
                        app.follow = true;
                        app.unseen = 0;
                    }
                    // esc: close the popup only — it never interrupts
                    // (Ctrl+C does, through the flag side-channel)
                    (KeyCode::Esc, _) => {
                        if popup_open {
                            app.input.clear();
                            app.cursor = 0;
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
                    // a newline in the composer: Shift+Enter (needs
                    // the kitty keyboard protocol), Ctrl+J (LF, the one
                    // binding EVERY terminal transmits), or alt+enter;
                    // plain Enter sends
                    (KeyCode::Enter, KeyModifiers::SHIFT)
                    | (KeyCode::Char('j'), KeyModifiers::CONTROL)
                    | (KeyCode::Enter, KeyModifiers::ALT) => {
                        let b = byte_at_char(&app.input, app.cursor);
                        app.input.insert(b, '\n');
                        app.cursor += 1;
                        app.hist_idx = None;
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
                    (KeyCode::Up, _) => {
                        if popup_open {
                            app.popup_sel = (app.popup_sel + matches.len() - 1) % matches.len();
                        } else if app.input.contains('\n') {
                            cursor_line_up(&app.input, &mut app.cursor);
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
                        } else if app.input.contains('\n') {
                            cursor_line_down(&app.input, &mut app.cursor);
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
            eprintln!("lance le harness avec ./run.sh");
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
        input: String::new(),
        cursor: 0,
        popup_sel: 0,
        history: Vec::new(),
        hist_idx: None,
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
                "API du modèle en erreur (essai 2/10) : provider 529 (transient) — nouvel essai dans 4s"
            ),
            _ => panic!("provider_retry must render as a warning"),
        }
        match parse_line("  obs: harness_restarted: exit status: 1 · bend: out of memory") {
            Some(Ev::Err(t)) => assert!(t.contains("bend: out of memory") && t.contains("redémarré")),
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
                "info injecté · [notification] bg 0 done".to_string(),
                "info session restaurée · 7 messages".to_string(),
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
        assert!(rows.iter().any(|r| r.contains("│ + notes.md · nouveau")));
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
