//! The feed layout cache: events wrap to rows once (per width, per
//! mutation) and frames only clone the visible slice; plus the event
//! merge rules (push_event) and the scroll anchor arithmetic.

use crate::render::*;
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
    /// built for main's feed (render::main_feed)
    pub(crate) main: bool,
    pub(crate) rows: Vec<Line<'static>>,
    /// A running tool: where its tool line sits in `rows`, and what it
    /// needs to be redrawn alone.
    pub(crate) live: Option<LiveHead>,
}

pub(crate) struct LiveHead {
    pub(crate) at: usize,
    pub(crate) len: usize,
    pub(crate) what: Live,
}

/// What a live row redraws each frame.
pub(crate) enum Live {
    /// a running tool's line (its pulse, its elapsed)
    Tool { name: String, args: String },
    /// the fold of the last run of level-3 lines (its pulse)
    Fold { n: usize, agents: usize, open: bool },
}

pub(crate) fn event_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> EventRows {
    if is_l3(&events[i]) && ev_visible(&events[i], debug) {
        let (rows, live) = l3_rows(events, i, debug, width, tick);
        return EventRows { width: width as u16, main: main_feed(), rows, live };
    }
    let running = match &events[i] {
        Ev::Tool(td) if matches!(td.state, ToolState::Run) && ev_visible(&events[i], debug) => Some(td),
        _ => None,
    };
    let Some(td) = running else {
        return EventRows {
            width: width as u16,
            main: main_feed(),
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
        main: main_feed(),
        rows,
        live: Some(LiveHead { at, len, what: Live::Tool { name, args } }),
    }
}

/// Redraw the live row of an event (a running tool's line, the pulse of
/// the last fold), keep the rest.
pub(crate) fn refresh_live(er: &mut EventRows, ev: &Ev, tick: u32) {
    let Some(lh) = er.live.as_mut() else { return };
    let head = match (&lh.what, ev) {
        (Live::Tool { name, args }, Ev::Tool(td)) => {
            wrap_line(tool_head(td, tick, name, args), code_width(er.width as usize))
        }
        (Live::Fold { n, agents, open }, _) => vec![fold_line(*n, *agents, *open, true, tick)],
        _ => return,
    };
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
    // a row that is already the continuation of a wrapped line stays one
    // (a second wrap, wider, never loses the soft mark the copy joins on)
    let soft = feedsel::is_soft(&line);
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
    for r in rows.iter_mut().skip(usize::from(!soft)) {
        feedsel::mark_soft(r);
    }
    rows
}

// the rows of one event: an optional breathing gap, then the wrapped
// lines.
pub(crate) fn build_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> Vec<Line<'static>> {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    if !ev_visible(ev, debug) {
        return rows;
    }
    if is_l3(ev) {
        return l3_rows(events, i, debug, width, tick).0;
    }
    // the previous VISIBLE event decides the gap: a debug-only
    // annotation between two blocks must not swallow the blank line
    let prev = events[..i].iter().rev().find(|e| ev_visible(e, debug));
    if wants_gap_before(ev, prev) {
        rows.push(Line::from(""));
    }
    rows.extend(ev_rows(ev, tick, width));
    rows
}

pub(crate) fn is_message(ev: &Ev) -> bool {
    matches!(ev, Ev::You(..) | Ev::Assistant(_) | Ev::Thinking { .. } | Ev::AgentMsg { .. } | Ev::Answered { .. })
}

pub(crate) fn is_tool_block(ev: &Ev) -> bool {
    matches!(ev, Ev::Tool(_) | Ev::Sub { .. })
}

pub(crate) fn is_notice(ev: &Ev) -> bool {
    matches!(
        ev,
        Ev::Warn(_)
            | Ev::Card { .. }
            | Ev::Err(_)
            | Ev::Info(_)
            | Ev::Compact(_)
            | Ev::Compacted(_)
            | Ev::TurnDone
            | Ev::TimeMark(_)
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
    // the lines of a run of level 3 sit together; a time mark stands
    // apart from whatever came before
    if is_l3(ev) && is_l3(p) {
        return false;
    }
    if matches!(ev, Ev::TimeMark(_)) {
        return true;
    }
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
            after_append(events, cache);
            return true;
        }
        // BISE-86: the hub could not deliver your message: its line gets
        // `✗`; only the newest `not delivered` line keeps its question
        Ev::Undelivered { name, text, .. } => {
            // your line is the text, or `@name text` from another view
            let yours = |t: &str| {
                t == text
                    || t.strip_prefix('@').and_then(|r| r.split_once(' ')).is_some_and(|(n, r)| n == name && r.trim() == text)
            };
            let mut found = false;
            for (i, e) in events.iter_mut().enumerate().rev() {
                match e {
                    Ev::Undelivered { open, .. } if *open => {
                        *open = false;
                        cache[i] = None;
                    }
                    Ev::You(t, m) if !found && yours(t) && *m != Mark::Failed => {
                        *m = Mark::Failed;
                        cache[i] = None;
                        found = true;
                    }
                    _ => {}
                }
            }
            // not in this feed (an `@name` line from another view shows
            // only once delivered): your line comes back, marked
            if !found {
                let t = format!("@{} {}", name, text);
                push_event(events, cache, Ev::You(t, Mark::Failed));
            }
        }
        // C3: a steering line moves the mark of your message
        Ev::MarkYou { text, mark, or } => {
            if !mark_you(events, cache, text, *mark) {
                if let Some(ev) = or {
                    return push_event(events, cache, (**ev).clone());
                }
            }
            return false;
        }
        // BISE-31: a closed card fades in place (its last line in this
        // feed); not in the feed (an older page): an info line as before
        Ev::CardClosed { id, res } => {
            let head = format!("#{} ", id);
            let found = events.iter_mut().enumerate().rev().find_map(|(i, e)| match e {
                Ev::Card { text, closed } if text.starts_with(&head) => Some((i, closed)),
                _ => None,
            });
            match found {
                Some((i, closed)) => {
                    *closed = res.clone();
                    if let Some(c) = cache.get_mut(i) {
                        *c = None;
                    }
                    return false;
                }
                None => return push_event(events, cache, Ev::Info(format!("card #{} {}", id, res))),
            }
        }
        // a turn starts: the model reads what you sent since the last one
        // (a message at idle goes straight to ✓✓)
        Ev::Turn => {
            for (i, e) in events.iter_mut().enumerate().rev() {
                match e {
                    Ev::Turn => break,
                    Ev::You(_, m) if matches!(*m, Mark::Sent | Mark::Received) => {
                        *m = Mark::Read;
                        if let Some(c) = cache.get_mut(i) {
                            *c = None;
                        }
                    }
                    _ => {}
                }
            }
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
                            td.result = Some((false, "interrupted".to_string()));
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
    after_append(events, cache);
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
        Some(c) if c.width == width as u16 && c.main == main_feed() => {
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
        Ev::AgentMsg { text, level: 3, .. } if !is_brief(text) && report_parts(text).is_none() => l3_long(text),
        Ev::AgentMsg { text, .. } => is_brief(text) || report_parts(text).is_some(),
        Ev::Answered { why, .. } => !why.trim().is_empty(),
        _ => false,
    }
}

/// Open or close event `i` in place (its rows rebuild). False when it
/// has nothing to disclose.
pub(crate) fn toggle_event(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    // the first line of a folded run: the fold opens or closes
    if events.get(i).is_some_and(is_l3) && folded_run(events, i, false) == Some(i) {
        return toggle_fold(events, cache, i);
    }
    toggle_own(events, cache, i)
}

/// Open or close the fold starting at `start` (its lines rebuild).
fn toggle_fold(events: &mut [Ev], cache: &mut [Option<EventRows>], start: usize) -> bool {
    let end = run_from(events, start, false).end;
    if let Some(Ev::AgentMsg { fold, .. }) = events.get_mut(start) {
        *fold = !*fold;
    }
    forget(cache, start..=end);
    true
}

/// Open or close event `i` in place from row `row` of its rows: on the
/// first line of an open fold, its fold row toggles the fold and its
/// message row the message; elsewhere as [`toggle_event`].
pub(crate) fn toggle_at(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize, row: usize) -> bool {
    if events.get(i).is_some_and(|e| is_l3(e) && fold_open(e)) && folded_run(events, i, false) == Some(i) {
        let prev = prev_visible(events, i, false).map(|p| &events[p]);
        let fold_row = usize::from(wants_gap_before(&events[i], prev));
        if row > fold_row {
            return toggle_own(events, cache, i);
        }
    }
    toggle_event(events, cache, i)
}

/// Open or close event `i` itself (not a fold).
fn toggle_own(events: &mut [Ev], cache: &mut [Option<EventRows>], i: usize) -> bool {
    let Some(ev) = events.get_mut(i).filter(|e| discloses(e)) else {
        return false;
    };
    match ev {
        Ev::Thinking { open, .. } | Ev::AgentMsg { open, .. } | Ev::Answered { open, .. } => *open = !*open,
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

// ---- the history: runs of level 3, folds, time marks (book §10, BISE-14) ----

/// A run of level-3 lines longer than this folds into one line.
pub(crate) const FOLD_AFTER: usize = 3;
/// A pause this long without a line gets a time mark.
pub(crate) const PAUSE_MS: u128 = 5 * 60 * 1000;

/// A message between agents (level 3), not a brief or a report (they
/// have their own lines).
pub(crate) fn is_l3(ev: &Ev) -> bool {
    matches!(ev, Ev::AgentMsg { level: 3, text, .. } if !is_brief(text) && report_parts(text).is_none())
}

/// The visible event before `i`.
fn prev_visible(events: &[Ev], i: usize, debug: bool) -> Option<usize> {
    (0..i).rev().find(|&j| ev_visible(&events[j], debug))
}

/// The first line of the run that holds level-3 line `i`, and how many
/// of its lines come up to `i` (included).
fn run_back(events: &[Ev], i: usize, debug: bool) -> (usize, usize) {
    let (mut start, mut n) = (i, 1);
    let mut j = i;
    while let Some(p) = prev_visible(events, j, debug) {
        if !is_l3(&events[p]) {
            break;
        }
        (start, n, j) = (p, n + 1, p);
    }
    (start, n)
}

/// A run of level-3 lines: consecutive among the visible events. Any
/// other visible line ends it (a level-1 or level-2 line, a tool, a time
/// mark), so a closed run never changes: only the last one grows.
pub(crate) struct Run {
    /// its last line
    pub(crate) end: usize,
    pub(crate) n: usize,
    /// the agents it names (senders and receivers)
    pub(crate) agents: usize,
    /// nothing visible after it yet: it may still grow
    pub(crate) live: bool,
}

/// The run that starts at level-3 line `start`, counted forward.
fn run_from(events: &[Ev], start: usize, debug: bool) -> Run {
    let mut names: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let (mut n, mut end, mut live) = (0, start, true);
    for (j, e) in events.iter().enumerate().skip(start) {
        if !ev_visible(e, debug) {
            continue;
        }
        let Ev::AgentMsg { from, to, .. } = e else {
            live = false;
            break;
        };
        if !is_l3(e) {
            live = false;
            break;
        }
        names.insert(from);
        if !to.is_empty() {
            names.insert(to);
        }
        n += 1;
        end = j;
    }
    Run { end, n, agents: names.len(), live }
}

/// Whether the run of level-3 line `i` folds, and where it starts:
/// counted back to its start, then forward only as far as needed.
fn folded_run(events: &[Ev], i: usize, debug: bool) -> Option<usize> {
    let (start, mut n) = run_back(events, i, debug);
    let mut j = i + 1;
    while n <= FOLD_AFTER && j < events.len() {
        let e = &events[j];
        if ev_visible(e, debug) {
            if !is_l3(e) {
                break;
            }
            n += 1;
        }
        j += 1;
    }
    (n > FOLD_AFTER).then_some(start)
}

fn fold_open(ev: &Ev) -> bool {
    matches!(ev, Ev::AgentMsg { fold: true, .. })
}

/// The rows of level-3 line `i`: its own line in a short run; in a
/// folded one the first line carries the fold (`▸ 12 messages between 5
/// agents`), the others show only when it is open, in place, in order.
fn l3_rows(events: &[Ev], i: usize, debug: bool, width: usize, tick: u32) -> (Vec<Line<'static>>, Option<LiveHead>) {
    let ev = &events[i];
    let mut rows: Vec<Line<'static>> = Vec::new();
    let Some(start) = folded_run(events, i, debug) else {
        let prev = prev_visible(events, i, debug).map(|p| &events[p]);
        if wants_gap_before(ev, prev) {
            rows.push(Line::from(""));
        }
        rows.extend(ev_rows(ev, tick, width));
        return (rows, None);
    };
    let open = fold_open(&events[start]);
    if i != start {
        if open {
            rows.extend(ev_rows(ev, tick, width));
        }
        return (rows, None);
    }
    let prev = prev_visible(events, i, debug).map(|p| &events[p]);
    if wants_gap_before(ev, prev) {
        rows.push(Line::from(""));
    }
    let run = run_from(events, start, debug);
    let at = rows.len();
    rows.push(fold_line(run.n, run.agents, open, run.live, tick));
    if open {
        rows.extend(ev_rows(ev, tick, width));
    }
    let live = run.live.then_some(LiveHead {
        at,
        len: 1,
        what: Live::Fold { n: run.n, agents: run.agents, open },
    });
    (rows, live)
}

fn forget(cache: &mut [Option<EventRows>], range: std::ops::RangeInclusive<usize>) {
    for c in cache.iter_mut().take(range.end() + 1).skip(*range.start()) {
        *c = None;
    }
}

/// A line was appended: the run before it changes (its count, the lines
/// that fold at the fourth, its pulse when it closes). Nothing else does.
fn after_append(events: &[Ev], cache: &mut [Option<EventRows>]) {
    let e = events.len() - 1;
    if !ev_visible(&events[e], false) {
        return;
    }
    let Some(p) = prev_visible(events, e, false).filter(|&p| is_l3(&events[p])) else {
        return;
    };
    let (start, n) = run_back(events, p, false);
    if is_l3(&events[e]) && n + 1 == FOLD_AFTER + 1 {
        forget(cache, start..=e);
    } else if n + usize::from(is_l3(&events[e])) > FOLD_AFTER {
        forget(cache, start..=start);
    }
}

/// A live line after a pause of `gap_ms`: first a time mark `· 14:31 ·`
/// (`now` gives the time). Not at the top of a feed, not twice.
pub(crate) fn pause_mark(
    events: &mut Vec<Ev>,
    cache: &mut Vec<Option<EventRows>>,
    gap_ms: u128,
    now: impl FnOnce() -> String,
) -> bool {
    if gap_ms < PAUSE_MS {
        return false;
    }
    match events.iter().rev().find(|e| ev_visible(e, false)) {
        None | Some(Ev::TimeMark(_)) => false,
        Some(_) => push_event(events, cache, Ev::TimeMark(now())),
    }
}

/// The local time, `14:31` (`date` knows the zone; the standard library
/// does not). UTC when `date` fails.
pub(crate) fn local_hhmm() -> String {
    let local = std::process::Command::new("date")
        .arg("+%H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 5);
    local.unwrap_or_else(|| {
        let s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!("{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60)
    })
}

// ---- everything at once (ctrl+o, input.rs) ----

/// Whether event `ev` itself is open; None when it has nothing to
/// disclose.
fn own_open(ev: &Ev) -> Option<bool> {
    if !discloses(ev) {
        return None;
    }
    match ev {
        Ev::Thinking { open, .. } | Ev::AgentMsg { open, .. } | Ev::Answered { open, .. } => Some(*open),
        Ev::Tool(td) => Some(td.expanded),
        _ => None,
    }
}

/// Whether event `i` is closed and can open: a thinking section, an
/// output, a report, a brief, a long level-3 line, a `▸ why`, a fold.
pub(crate) fn is_closed_at(events: &[Ev], i: usize) -> bool {
    let ev = &events[i];
    own_open(ev) == Some(false) || (is_l3(ev) && !fold_open(ev) && folded_run(events, i, false) == Some(i))
}

/// Anything closed in the feed.
pub(crate) fn anything_closed(events: &[Ev]) -> bool {
    (0..events.len()).any(|i| is_closed_at(events, i))
}

/// Open (or close) everything that discloses, folds included.
pub(crate) fn set_everything(events: &mut [Ev], cache: &mut [Option<EventRows>], open: bool) {
    for i in 0..events.len() {
        if is_l3(&events[i]) && fold_open(&events[i]) != open && folded_run(events, i, false) == Some(i) {
            toggle_fold(events, cache, i);
        }
        if own_open(&events[i]).is_some_and(|o| o != open) {
            toggle_own(events, cache, i);
        }
    }
}

// ---- message marks (C3, book §13, BISE-15) ----

/// How far back a steering line looks for your message.
const MARK_LOOKBACK: usize = 500;

/// The words of a text, for matching a steering line to your message
/// (the wire flattens its line breaks).
fn words(t: &str) -> impl Iterator<Item = &str> {
    t.split_whitespace()
}

/// Move the mark of your last message with `text` up to `mark` (never
/// down). False when there is no such message.
pub(crate) fn mark_you(events: &mut [Ev], cache: &mut [Option<EventRows>], text: &str, mark: Mark) -> bool {
    let plain = crate::markdown::unescape_md(text);
    let from = events.len().saturating_sub(MARK_LOOKBACK);
    for i in (from..events.len()).rev() {
        if let Ev::You(t, m) = &mut events[i] {
            if words(t).eq(words(&plain)) {
                if mark > *m {
                    *m = mark;
                    if let Some(c) = cache.get_mut(i) {
                        *c = None;
                    }
                }
                return true;
            }
        }
    }
    false
}
