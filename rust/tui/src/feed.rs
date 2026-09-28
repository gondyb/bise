//! The feed layout cache: events wrap to rows once (per width, per
//! mutation) and frames only clone the visible slice; plus the event
//! merge rules (push_event) and the scroll anchor arithmetic.

use crate::render::*;
use crate::theme::*;
use crate::wire::*;
use crate::feedsel;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// a replayed tool has no meaningful duration (the timing is the replay's)
pub(crate) fn hide_replayed_elapsed(events: &mut [Ev], cache: &mut [Option<EventRows>], id: u32) {
    if let Some((i, td)) = last_tool_mut(events, |td| td.id == id) {
        td.elapsed = Some(String::new());
        cache[i] = None;
    }
}

/// The newest tool event that matches, with its index.
fn last_tool_mut(events: &mut [Ev], pred: impl Fn(&ToolData) -> bool) -> Option<(usize, &mut ToolData)> {
    events.iter_mut().enumerate().rev().find_map(|(i, e)| match e {
        Ev::Tool(td) if pred(td) => Some((i, td)),
        _ => None,
    })
}

// ---- the codex-style layout cache ----
// Events render to wrapped rows ONCE (per width / per mutation); every
// frame only the visible slice is cloned into the paragraph. A running
// tool re-renders only its tool line each frame (spinner, elapsed): its
// body (a source block can be thousands of rows) stays cached.

pub(crate) struct EventRows {
    pub(crate) width: u16,
    pub(crate) rows: Vec<Line<'static>>,
    /// A running tool: where its tool line sits in `rows`, and what it
    /// needs to be redrawn alone.
    pub(crate) live: Option<LiveHead>,
}

pub(crate) struct LiveHead {
    pub(crate) at: usize,
    pub(crate) len: usize,
    pub(crate) name: String,
    pub(crate) args: String,
}

pub(crate) fn event_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> EventRows {
    let running = match &events[i] {
        Ev::Tool(td) if matches!(td.state, ToolState::Run) && ev_visible(&events[i], debug) => Some(td),
        _ => None,
    };
    let Some(td) = running else {
        return EventRows {
            width: width as u16,
            rows: build_rows(events, i, debug, width, tick),
            live: None,
        };
    };
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    let mut rows: Vec<Line<'static>> = Vec::new();
    if wants_gap_before(&events[i], prev) {
        rows.push(Line::from(""));
    }
    let (name, args, code) = tool_meta(td);
    // a tool is code: its rows follow the code measure
    let cw = code_width(width);
    let at = rows.len();
    rows.extend(wrap_line(tool_head(td, tick, &name, &args), cw));
    let len = rows.len() - at;
    for l in tool_body(td, &code, cw) {
        rows.extend(wrap_line(l, cw));
    }
    EventRows {
        width: width as u16,
        rows,
        live: Some(LiveHead { at, len, name, args }),
    }
}

/// Redraw the tool line of a running tool, keep the rest.
pub(crate) fn refresh_live(er: &mut EventRows, ev: &Ev, tick: u32) {
    let (Some(lh), Ev::Tool(td)) = (er.live.as_mut(), ev) else { return };
    let head = wrap_line(tool_head(td, tick, &lh.name, &lh.args), code_width(er.width as usize));
    let n = head.len();
    er.rows.splice(lh.at..lh.at + lh.len, head);
    lh.len = n;
}

pub(crate) fn line_from(cells: Vec<(char, Style)>) -> Line<'static> {
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

/// The display width of each char of `chars`, by grapheme: the first
/// char of a grapheme carries its width (at least 1), the rest 0, so an
/// emoji ZWJ sequence or a char with a variation selector counts as
/// ratatui draws it, and a row never breaks inside one.
pub(crate) fn cell_widths(chars: impl Iterator<Item = char>) -> Vec<usize> {
    let s: String = chars.collect();
    let mut out = Vec::with_capacity(s.len());
    for g in s.graphemes(true) {
        let mut first = true;
        for _ in g.chars() {
            out.push(if first { g.width().max(1) } else { 0 });
            first = false;
        }
    }
    out
}

// span-aware greedy word wrap; words wider than the row hard-split
pub(crate) fn wrap_line(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
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
    let widths = cell_widths(cells.iter().map(|c| c.0));
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut row_w = 0usize;
    let mut i = 0usize;
    while i < cells.len() {
        // the next word (non-space run)
        let mut word: Vec<(char, Style)> = Vec::new();
        let mut word_w = 0usize;
        while i < cells.len() && cells[i].0 != ' ' {
            word_w += widths[i];
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
            let word_start = i - word.len();
            for (k, (c, st)) in word.into_iter().enumerate() {
                let cc = widths[word_start + k];
                if cc > 0 && cw > 0 && cw + cc > width {
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
    // the rows after the first continue the line (the copy joins them)
    for r in rows.iter_mut().skip(1) {
        feedsel::mark_soft(r);
    }
    rows
}

// the rows of one event: an optional breathing gap, then the wrapped
// lines. The user block is padded to the full width so its panel
// background reads as a solid block.
pub(crate) fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> Vec<Line<'static>> {
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
    for mut r in ev_rows(ev, tick, width) {
        if user {
            pad_line_bg(&mut r, prose_width(width));
        }
        rows.push(r);
    }
    rows
}

// paint the row with the panel background — the line style is the base
// every span patches, so the bar, the text and the padding all sit on
// the panel — then fill the rest of the column, so the user block
// reads as a solid panel the full width (OpenCode style)
pub(crate) fn pad_line_bg(line: &mut Line<'static>, width: usize) {
    line.style = Style::default().bg(PANEL);
    let used: usize = line.spans.iter().map(|s| s.content.width()).sum();
    if used < width {
        line.spans
            .push(Span::styled(" ".repeat(width - used), Style::default().bg(PANEL)));
    }
}

pub(crate) fn is_message(ev: &Ev) -> bool {
    matches!(ev, Ev::You(_) | Ev::Assistant(_) | Ev::Thinking { .. } | Ev::AgentMsg { .. })
}

pub(crate) fn is_tool_block(ev: &Ev) -> bool {
    matches!(ev, Ev::Tool(_) | Ev::Sub { .. })
}

pub(crate) fn is_notice(ev: &Ev) -> bool {
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
pub(crate) fn wants_gap_before(ev: &Ev, prev: Option<&Ev>) -> bool {
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
pub(crate) fn ev_visible(ev: &Ev, debug: bool) -> bool {
    if debug {
        return true;
    }
    !matches!(
        ev,
        Ev::Turn
            | Ev::TurnDone
            | Ev::Idle
            | Ev::Raw(_)
            | Ev::Usage(_)
            | Ev::ToolInfo { .. }
            | Ev::ToolResult { .. }
            | Ev::ToolCode { .. }
    )
}

pub(crate) fn push_event(events: &mut Vec<Ev>, cache: &mut Vec<Option<EventRows>>, ev: Ev) -> bool {
    // annotations enrich the matching tool event instead of stacking
    match &ev {
        Ev::ToolInfo { id, name, args } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.name = Some(name.clone());
                td.args = Some(args.clone());
                cache[i] = None;
            }
            return false;
        }
        Ev::ToolResult { id, ok, preview } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.result = Some((*ok, preview.clone()));
                cache[i] = None;
            }
            return false;
        }
        Ev::ToolCode { id, code } => {
            if let Some((i, td)) = last_tool_mut(events, |td| td.id == *id) {
                td.code = Some(code.clone());
                cache[i] = None;
            }
            return false;
        }
        Ev::Tool(done) if !matches!(done.state, ToolState::Run) => {
            // a tool finishing rewrites its running line
            let running = |td: &ToolData| td.id == done.id && matches!(td.state, ToolState::Run);
            if let Some((i, td)) = last_tool_mut(events, running) {
                td.state = done.state.clone();
                td.elapsed = Some(fmt_elapsed(td.started));
                if td.result.is_none() {
                    td.result = done.result.clone();
                }
                cache[i] = None;
                return false;
            }
            events.push(ev);
            cache.push(None);
            return true;
        }
        // the turn ended: a tool still shown as running was abandoned
        // (interrupt or failed turn) — freeze it so the elapsed stops
        Ev::TurnDone | Ev::Idle => {
            // back to the previous end of turn: the tools before it were
            // frozen then (O(turn), not O(history))
            for (i, e) in events.iter_mut().enumerate().rev() {
                if matches!(e, Ev::TurnDone | Ev::Idle) {
                    break;
                }
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

/// The rows of event `i` at this width, built when missing (a running
/// tool redraws its tool line). Returns how many rows it has.
pub(crate) fn ensure_rows(
    events: &[Ev],
    cache: &mut Vec<Option<EventRows>>,
    i: usize,
    debug: bool,
    width: usize,
    tick: u32,
) -> usize {
    // total: no event there, no rows; a cache shorter than the events
    // (a feed swapped or trimmed since the last frame) grows first
    if i >= events.len() {
        return 0;
    }
    if cache.len() < events.len() {
        cache.resize_with(events.len(), || None);
    }
    match cache[i].as_mut() {
        Some(c) if c.width == width as u16 => {
            refresh_live(c, &events[i], tick);
        }
        _ => cache[i] = Some(event_rows(events, i, debug, width, tick)),
    }
    cache[i].as_ref().map_or(0, |c| c.rows.len())
}

/// The anchor that shows the last `h` rows.
pub(crate) fn bottom_anchor(n: usize, h: usize, rows_of: &mut dyn FnMut(usize) -> usize) -> (usize, usize) {
    let mut need = h;
    let mut i = n;
    while i > 0 {
        i -= 1;
        let len = rows_of(i);
        if len >= need {
            return (i, len - need);
        }
        need -= len;
    }
    (0, 0)
}

/// Move a (event, row) anchor by `d` rows (negative: up), building only
/// the rows it walks over.
pub(crate) fn move_anchor(
    anchor: (usize, usize),
    d: isize,
    n: usize,
    rows_of: &mut dyn FnMut(usize) -> usize,
) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }
    let (mut i, mut r) = anchor;
    if i >= n {
        i = n - 1;
        r = usize::MAX;
    }
    r = r.min(rows_of(i).saturating_sub(1));
    let mut k = d.unsigned_abs();
    if d < 0 {
        while k > 0 {
            if r >= k {
                r -= k;
                break;
            }
            k -= r;
            r = 0;
            // one row up: the last row of the previous event with rows
            let mut j = i;
            let mut moved = false;
            while j > 0 {
                j -= 1;
                let len = rows_of(j);
                if len > 0 {
                    (i, r) = (j, len - 1);
                    k -= 1;
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
        }
    } else {
        while k > 0 {
            let len = rows_of(i);
            if r + k < len {
                r += k;
                break;
            }
            // to the first row of the next event with rows
            k -= len.saturating_sub(r);
            let mut j = i + 1;
            while j < n && rows_of(j) == 0 {
                j += 1;
            }
            if j >= n {
                r = len.saturating_sub(1);
                break;
            }
            (i, r) = (j, 0);
        }
    }
    (i, r)
}

// ---- progressive disclosure (book §11, BISE-12) ----

/// A tool has something behind its `▸`: an output, or an edit's diff.
fn tool_discloses(td: &ToolData) -> bool {
    td.result.as_ref().is_some_and(|(_, r)| !r.trim().is_empty())
        || (td.name.as_deref() == Some("apply_patch") && td.code.is_some())
}

/// Whether event `ev` opens and closes: a thinking section, a tool with
/// an output or a diff, a report, a brief.
pub(crate) fn discloses(ev: &Ev) -> bool {
    match ev {
        Ev::Thinking { .. } => true,
        Ev::Tool(td) => tool_discloses(td),
        Ev::AgentMsg { text, .. } => is_brief(text) || report_parts(text).is_some(),
        _ => false,
    }
}

/// Open or close event `i` in place (its rows rebuild). False when it
/// has nothing to disclose.
pub(crate) fn toggle_event(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    let Some(ev) = events.get_mut(i).filter(|e| discloses(e)) else {
        return false;
    };
    match ev {
        Ev::Thinking { open, .. } | Ev::AgentMsg { open, .. } => *open = !*open,
        Ev::Tool(td) => td.expanded = !td.expanded,
        _ => return false,
    }
    if let Some(c) = cache.get_mut(i) {
        *c = None;
    }
    true
}

/// Toggle the item the feed selection is on (`space`; the key is bound
/// in BISE-42). False when there is no selection or nothing to toggle.
#[cfg_attr(not(test), allow(dead_code))] // bound by BISE-42
pub(crate) fn toggle_selected(app: &mut crate::app::App) -> bool {
    let Some(i) = app.feed_sel.map(|s| s.head.0) else {
        return false;
    };
    toggle_event(&mut app.events, &mut app.cache, i)
}

/// Open every tool output and edit diff when one is closed, else close
/// them all. Returns whether they are open now.
#[cfg_attr(not(test), allow(dead_code))] // bound by BISE-42
pub(crate) fn toggle_all_outputs(app: &mut crate::app::App) -> bool {
    set_all_outputs(&mut app.events, &mut app.cache)
}

#[cfg_attr(not(test), allow(dead_code))] // bound by BISE-42
pub(crate) fn set_all_outputs(events: &mut [Ev], cache: &mut [Option<EventRows>]) -> bool {
    let open = events.iter().any(|e| matches!(e, Ev::Tool(td) if tool_discloses(td) && !td.expanded));
    for (i, e) in events.iter_mut().enumerate() {
        if let Ev::Tool(td) = e {
            if tool_discloses(td) && td.expanded != open {
                td.expanded = open;
                if let Some(c) = cache.get_mut(i) {
                    *c = None;
                }
            }
        }
    }
    open
}
