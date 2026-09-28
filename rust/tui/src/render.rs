//! Feed events as styled lines: messages, reasoning sections, notices
//! and the OpenCode inline tool lines.

use crate::code::*;
use crate::markdown::*;
use crate::theme::*;
use crate::wire::*;
use crate::wrap_line;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

// ---- tool-call rendering helpers ----

pub(crate) fn fmt_elapsed(started: std::time::Instant) -> String {
    let s = started.elapsed().as_secs_f64();
    if s < 10.0 {
        format!("{:.1}s", s)
    } else if s < 60.0 {
        format!("{:.0}s", s)
    } else {
        format!("{:.0}m{:02.0}s", (s / 60.0).floor(), s % 60.0)
    }
}

pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{}{}", head, ellipsis())
    }
}

/// `s` in at most `room` chars, the `…` (`...` in ASCII mode) included
/// when it is cut.
pub(crate) fn fit_chars(s: &str, room: usize) -> String {
    if s.chars().count() <= room {
        return s.to_string();
    }
    let e = ellipsis();
    let n = e.chars().count();
    if room < n {
        return s.chars().take(room).collect();
    }
    let head: String = s.chars().take(room - n).collect();
    format!("{}{}", head, e)
}

// naive "field":"value" extractor for JSON-ish args (no parser needed:
// the runtime caps the payload and the shape is known)
pub(crate) fn json_str_field(s: &str, field: &str) -> Option<String> {
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
pub(crate) fn first_line_preview(code: &str) -> String {
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

pub(crate) fn args_preview(name: &str, args: &str) -> String {
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
pub(crate) fn ev_lines_t(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    // the feed twin: identical shape, but the tool spinner animates
    match ev {
        Ev::Tool(td) => tool_lines(td, tick, width),
        other => ev_lines(other, width),
    }
}

// ---- the measure (book §11) ----

/// Prose (messages, reports, notices) wraps at this many columns.
pub(crate) const PROSE_MAX: usize = 79;
/// Code (scripts, diffs, outputs) runs up to this many columns.
pub(crate) const CODE_MAX: usize = 103;

/// The prose measure in a feed column of `width`.
pub(crate) fn prose_width(width: usize) -> usize {
    width.clamp(1, PROSE_MAX)
}

/// The code measure in a feed column of `width`.
pub(crate) fn code_width(width: usize) -> usize {
    width.clamp(1, CODE_MAX)
}

/// The rows of one event in a feed column of `width`: prose wrapped at
/// its measure, a tool (its line, its code, its output) at the code
/// measure. The extra width stays empty: rows never stretch.
pub(crate) fn ev_rows(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    let w = match ev {
        Ev::Tool(_) => code_width(width),
        // a reply wraps its prose at the prose measure itself; its tables
        // may run to the code measure (BISE-87)
        Ev::Assistant(_) => code_width(width),
        // a level-3 line is a row of a list, not prose: the code measure
        Ev::AgentMsg { level: 3, text, .. } if !is_brief(text) && report_parts(text).is_none() => code_width(width),
        _ => prose_width(width),
    };
    // a reply's rows are final (md_lines wrapped them): no second pass
    if matches!(ev, Ev::Assistant(_)) && !main_feed() {
        return ev_lines_t(ev, tick, w);
    }
    let mut rows = Vec::new();
    for l in ev_lines_t(ev, tick, w) {
        rows.extend(wrap_line(l, w));
    }
    rows
}

// a thinking section: collapsed it is one dim glyph + duration;
// expanded (ctrl+o, or a click) the reasoning shows under a faint rail
pub(crate) fn thinking_lines(ms: u128, text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let label = match fmt_think_ms(ms) {
        d if d.is_empty() => "thought".to_string(),
        d => format!("thought for {}", d),
    };
    let head = Line::from(vec![
        Span::styled(format!(" {} ", G_THINK), dim_st),
        Span::styled(label, dim_st),
        Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), dim_st),
    ]);
    if !open || text.trim().is_empty() {
        return vec![head];
    }
    let mut rows = vec![head];
    // the BENDSIG line carries the provider signature (the signed
    // thinking transport), never part of the reasoning itself
    let body = unescape_md(text);
    let lines = body
        .split('\n')
        .filter(|l| !l.starts_with("BENDSIG::"))
        .map(|l| Line::from(Span::styled(l.to_string(), dim_st)));
    let bar = Span::styled(RAIL, Style::default().fg(faint()));
    rows.extend(barred_rows(&bar, lines, width));
    rows
}

// 800ms -> "0.8s"; 4200ms -> "4.2s"; 12_300ms -> "12s"; 90_000 -> "1m30s";
// 0 (no measured duration: a replayed section, or lines that arrived in
// the same batch) -> ""
pub(crate) fn fmt_think_ms(ms: u128) -> String {
    if ms == 0 {
        String::new()
    } else if ms < 10_000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

thread_local! {
    /// the feed drawn is main's (ui.rs sets it before each frame)
    static MAIN_FEED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whose feed is drawn: main's, or an agent's (its replies carry no
/// `:*`). The rows built under one owner rebuild under the other.
pub(crate) fn set_main_feed(on: bool) {
    MAIN_FEED.with(|c| c.set(on));
}

pub(crate) fn main_feed() -> bool {
    MAIN_FEED.with(|c| c.get())
}

thread_local! {
    static FEED_OWNER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The agent whose feed is drawn: a message it received (no `to` on the
/// wire) names it in the `to` column. The rows built under one owner
/// rebuild under the other (the feed cache is per view).
pub(crate) fn set_feed_owner(name: &str) {
    FEED_OWNER.with(|c| {
        if *c.borrow() != name {
            *c.borrow_mut() = name.to_string();
        }
    });
}

fn feed_owner() -> String {
    FEED_OWNER.with(|c| c.borrow().clone())
}

/// The faint rail in front of disclosed text (reasoning, a report, a
/// brief, a message body).
const RAIL: &str = " │ ";

/// A notice with no §6 glyph (an info line). Not in the book: see the
/// BISE-13 notes.
const G_NOTE: &str = "·";

/// One line in the feed's glyph column: the glyph at column 1, the text
/// from column 3 (under the names of the tool lines).
// a glyph in the glyph column, the text from column 4; its wrapped rows
// hang under the text, never at column 1 (BISE-90)
fn glyph_line(glyph: &str, glyph_st: Style, text: String, text_st: Style, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let first = Span::styled(format!(" {} ", glyph), glyph_st);
    let pad = Span::raw(" ".repeat(first.content.width()));
    hung_rows(&first, &pad, [Line::from(Span::styled(text, text_st))], width)
}

pub(crate) fn ev_lines(ev: &Ev, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let text_st = Style::default().fg(text());
    let err_st = Style::default().fg(error());
    match ev {
        // an image marker is an accent chip `▣ login.png` (book §14)
        Ev::You(t, mark) => user_block_lines(t, *mark, width),
        Ev::MarkYou { .. } => vec![],
        // BISE-86 (book §13, §17): `✗ not delivered: {name} stopped.`, and
        // while it waits for an answer `⏎ send again · esc drop`
        Ev::Undelivered { name, open, .. } => {
            let mut l = glyph_line(G_FAILED, err_st, format!("not delivered: {} stopped.", name), text_st, width);
            if *open {
                l[0].spans.push(Span::styled(" ⏎ send again · esc drop", dim_st));
            }
            l
        }
        // in main's feed, main's reply carries `:*` (book §6), its text at
        // column 3; inside an agent, the reply is the view's own voice
        Ev::Assistant(t) if main_feed() => {
            let mark = Span::styled(format!(" {} ", G_MAIN), Style::default().fg(accent()));
            // wrapped once, at the text's own width: the rows under the
            // first one line up with its text (BISE-97: at 80 columns a
            // row 1 cell too wide wrapped again, leaving one-word rows)
            use unicode_width::UnicodeWidthStr;
            let lead = mark.content.width();
            let rows = md_lines(&unescape_md(t), prose_width(width).saturating_sub(lead), width.saturating_sub(lead));
            hung_rows(&mark, &Span::raw(" ".repeat(lead)), rows, width)
        }
        // inside an agent: the reply starts at the glyph column, like the
        // mockup "inside an agent" (BISE-90; it was one column left of it)
        Ev::Assistant(t) => {
            let lead = Span::raw(" ");
            let rows = md_lines(&unescape_md(t), prose_width(width).saturating_sub(1), width.saturating_sub(1));
            hung_rows(&lead, &lead, rows, width)
        }
        Ev::Thinking { ms, text, open } => thinking_lines(*ms, text, *open, width),
        Ev::Tool(td) => tool_lines(td, 0, width),
        Ev::Idle => vec![Line::from("")],
        // a sub-call inside a TypeScript run: `↳ github.search_issues ✓`;
        // a failed one says why, in the error color
        Ev::Sub { name, ok, preview } => vec![Line::from(vec![
            Span::styled(format!("   {} ", G_SUBCALL), dim_st),
            Span::styled(name.clone(), dim_st),
            if *ok {
                Span::styled(format!(" {}", G_RECEIVED), dim_st)
            } else {
                let why = fit_chars(preview.trim(), 80);
                Span::styled(format!(" {} {}", G_FAILED, why).trim_end().to_string(), err_st)
            },
        ])],
        Ev::Turn => vec![Line::from(vec![
            Span::styled(" ── turn ", Style::default().fg(faint())),
            Span::styled("─".repeat(24), Style::default().fg(faint())),
        ])],
        Ev::TurnDone => vec![Line::from(vec![
            Span::styled(" └─ ", Style::default().fg(faint())),
            Span::styled(G_RECEIVED, dim_st),
        ])],
        // book §6, §17 (BISE-90): `≡ compacting` (the glyph pulses while
        // it runs: feed.rs Live::Compacting), then `≡ summary ▸`, the
        // summary under the rail once opened
        Ev::Compact => compacting_line(0, true),
        Ev::Compacted { text, open } => summary_lines(text, *open, width),
        // an interrupted turn is dim; any other warning reads as text
        Ev::Warn(t) if t == "turn interrupted" => glyph_line(G_INTERRUPTED, dim_st, t.clone(), dim_st, width),
        Ev::Warn(t) => glyph_line(G_INTERRUPTED, dim_st, t.clone(), text_st, width),
        // a model without vision refused an image: say so, and the way out
        Ev::Err(t) => match crate::attach::no_vision(t) {
            Some(mut spans) => {
                if let Some(first) = spans.first_mut() {
                    first.content = format!(" {} ", G_FAILED).into();
                }
                vec![Line::from(spans)]
            }
            None => glyph_line(G_FAILED, err_st, t.clone(), err_st, width),
        },
        Ev::Info(t) => glyph_line(G_NOTE, Style::default().fg(faint()), bend_images::display(t), dim_st, width),
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } | Ev::ToolCode { .. } => vec![],
        Ev::Usage(u) => vec![Line::from(Span::styled(
            format!("  usage: {} (in {} · out {})", u.label(), u.input, u.output),
            dim_st,
        ))],
        Ev::Raw(t) => vec![Line::from(Span::styled(format!("  {}", t), dim_st))],
        Ev::AgentMsg { text, open, .. } if is_brief(text) => brief_lines(text, *open, width),
        Ev::AgentMsg { from, text, open, .. } if report_parts(text).is_some() => {
            let (kind, body) = report_parts(text).unwrap_or_default();
            report_lines(from, kind, body, *open, width)
        }
        Ev::AgentMsg { from, to, text, level: 3, id, open, .. } => l3_lines(from, to, id, text, *open, width),
        Ev::AgentMsg { from, text, .. } => l2_lines(from, text, width),
        Ev::Answered { agent, question, answer, why, open } => answered_lines(agent, question, answer, why, *open, width),
        Ev::TimeMark(t) => vec![Line::from(Span::styled(format!(" {} {} {}", G_NOTE, t, G_NOTE), Style::default().fg(faint())))],
        Ev::Card { text, closed } => card_lines(text, closed, width),
        Ev::CardClosed { .. } => vec![],
    }
}

// ---- the three levels (book §9) ----

/// The columns a level-3 line gives each name.
const NAME_COLS: usize = 10;
/// Where the text of a level-3 line starts: rail, glyph, two names.
const L3_HEAD: usize = 3 + 2 + NAME_COLS + 2 + NAME_COLS;

/// A name in its fixed column: cut to leave a space, then padded.
fn name_col(name: &str) -> String {
    format!("{:<w$}", fit_chars(name, NAME_COLS - 1), w = NAME_COLS)
}

/// A level-3 text that may not fit its line: it opens (`▸`).
pub(crate) fn l3_long(text: &str) -> bool {
    text.trim().contains('\n') || text.trim().chars().count() > CODE_MAX - L3_HEAD
}

/// A message between agents (level 3): dim, under a faint rail, one line
/// `@ from      → to        text`, `▸` when cut; open, the whole text
/// under the rail. No `to` (what this feed's owner received): the owner's
/// name (`main` in main's feed), else the message id (BISE-90).
pub(crate) fn l3_lines(from: &str, to: &str, id: &str, text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let faint_st = Style::default().fg(faint());
    let flat = text.trim().replace('\n', " ");
    let long = l3_long(text);
    let cut = flat.chars().count() > width.saturating_sub(L3_HEAD) || text.trim().contains('\n');
    // room for the text, and for ` ▸` when it is cut and opens
    let room = width.saturating_sub(L3_HEAD + if cut && long { 2 } else { 0 }).max(8);
    let owner = if main_feed() { "main".to_string() } else { feed_owner() };
    let (arrow, target) = match (to.is_empty(), owner.is_empty()) {
        (false, _) => ("→ ", to.to_string()),
        (true, false) => ("→ ", owner),
        (true, true) => ("  ", id.to_string()),
    };
    let mut row = vec![
        Span::styled(RAIL, faint_st),
        Span::styled(format!("{} ", G_MSG), dim_st),
        Span::styled(name_col(from), dim_st),
        Span::styled(arrow, faint_st),
        Span::styled(name_col(&target), faint_st),
        Span::styled(fit_chars(&flat, room), dim_st),
    ];
    if cut && long {
        row.push(Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), dim_st));
    }
    let mut ls = vec![Line::from(row)];
    if open && cut && long {
        let bar = Span::styled(format!("{}  ", RAIL), faint_st);
        let body = text.trim().split('\n').map(|l| Line::from(Span::styled(l.to_string(), dim_st)));
        ls.extend(barred_rows(&bar, body, width.min(PROSE_MAX)));
    }
    ls
}

/// An agent writing to you (level 2): normal text, `@ name to you: …`
/// (from main: `:* …`).
fn l2_lines(from: &str, body: &str, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let (glyph, glyph_st, lead) = if from == "main" {
        (G_MAIN, Style::default().fg(accent()), String::new())
    } else {
        (G_MSG, text_st, format!("{} to you: ", from))
    };
    let mut lines = md_lines(&unescape_md(body.trim()), width.saturating_sub(3), width.saturating_sub(3));
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines[0].spans.insert(0, Span::styled(lead, text_st));
    let mark = Span::styled(format!(" {} ", glyph), glyph_st);
    hung_rows(&mark, &Span::raw("   "), lines, width)
}

/// Main answered an agent for you (level 2): `:* docs asked: v1 or v2?
/// i answered: v2. ▸ why`; open, the why under the rail.
fn answered_lines(agent: &str, question: &str, answer: &str, why: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let q = question.trim().replace('\n', " ");
    let sep = if q.ends_with(['?', '.', '!', ':']) { " " } else { "; " };
    let mut line = vec![Span::styled(
        format!("{} asked: {}{}i answered: {}", agent, q, sep, answer.trim().replace('\n', " ")),
        text_st,
    )];
    if !why.trim().is_empty() {
        line.push(Span::styled(format!(" {} why", if open { G_OPEN } else { G_CLOSED }), Style::default().fg(dim())));
    }
    let mark = Span::styled(format!(" {} ", G_MAIN), Style::default().fg(accent()));
    let mut ls = hung_rows(&mark, &Span::raw("   "), [Line::from(line)], width);
    if open && !why.trim().is_empty() {
        let bar = Span::styled(RAIL, Style::default().fg(faint()));
        ls.extend(barred_rows(&bar, md_lines(why.trim(), width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

/// The fold of a run of level-3 lines (book §10): `▸ 47 messages
/// between 30 agents`, dim under the faint rail; the last run, still
/// growing, carries the working pulse.
pub(crate) fn fold_line(n: usize, agents: usize, open: bool, live: bool, tick: u32) -> Line<'static> {
    let mut row = vec![
        Span::styled(RAIL, Style::default().fg(faint())),
        Span::styled(
            format!(
                "{} {} messages between {} agent{}",
                if open { G_OPEN } else { G_CLOSED },
                n,
                agents,
                if agents == 1 { "" } else { "s" }
            ),
            Style::default().fg(dim()),
        ),
    ];
    if live {
        let (g, c) = working_frame(tick);
        row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
    }
    Line::from(row)
}

/// A card line of the hub (`#3 question @docs : v1 or v2?`): its kind,
/// the agent, the text.
pub(crate) fn card_parts(t: &str) -> Option<(&str, &str, &str)> {
    let rest = t.strip_prefix('#')?;
    let (_, rest) = rest.split_once(' ')?;
    let (kind, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('@')?;
    let (name, text) = rest.split_once(" : ").unwrap_or((rest, ""));
    Some((kind, name, text))
}

// a card in the history (book §9): a question or a blocker is level 1,
// an accent bar `┃`, the bold accent title `? {name} needs you`, the
// body in text; a done or failed card is one line for you (`♡`, `✗`).
// Answered (`closed`: the hub's word, book §10, §12), a level-1 card
// fades in place: dim bar, dim title with ` · answered`, dim body; the
// answer follows as its own line.
fn card_lines(t: &str, closed: &str, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let (kind, name, body) = card_parts(t).unwrap_or(("question", "", t));
    match kind {
        "done" => return glyph_line(G_DONE, text_st, format!("{} is done: {}", name, body), text_st, width),
        k if k.contains("fail") => {
            return glyph_line(G_FAILED, Style::default().fg(error()), format!("{} failed: {}", name, body), text_st, width)
        }
        _ => {}
    }
    let mut title = if name.is_empty() { "needs you".to_string() } else { format!("{} needs you", name) };
    let (bar_st, title_st, body_st) = if closed.is_empty() {
        let accent_st = Style::default().fg(accent()).add_modifier(Modifier::BOLD);
        (Style::default().fg(accent()), accent_st, text_st)
    } else {
        title.push_str(&format!(" · {}", closed_word(closed)));
        let dim_st = Style::default().fg(dim());
        (dim_st, dim_st, dim_st)
    };
    let bar = Span::styled(" ┃ ", bar_st);
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{} ", G_CARD), title_st),
        Span::styled(title, title_st),
    ])];
    lines.extend(body.split('\n').map(|l| Line::from(Span::styled(l.to_string(), body_st))));
    barred_rows(&bar, lines, width)
}

/// How a card was closed, in the user's words (the hub's `card-closed`
/// result: `answered`, `answered via @x`, `closed`, `accepted`, …).
pub(crate) fn closed_word(res: &str) -> String {
    let res = res.trim();
    if let Some(who) = res.strip_prefix("answered via @") {
        return format!("answered by {}", who);
    }
    match res {
        "closed" => "closed".into(),
        "accepted" => "dropped".into(),
        "refused" => "kept".into(),
        "vue" => "seen".into(),
        "reprise" => "resumed".into(),
        "task stopped" => "agent stopped".into(),
        "" => "answered".into(),
        r => r.to_string(),
    }
}

// your message (book §6, mockups): `›` dim in the glyph column, the text
// from column 3, each of its lines (Shift+Enter, paste) on its own rows.
// No bar, no background: the terminal's own shows through.
pub(crate) fn user_block_lines(msg: &str, mark: Mark, width: usize) -> Vec<Line<'static>> {
    let style = Style::default().fg(text());
    let mut lines: Vec<Line<'static>> = msg
        .split('\n')
        .map(|l| Line::from(crate::attach::chip_spans(l.trim_end_matches('\r'), style)))
        .collect();
    // its mark at the end (C3): `·` sent, `✓` got, `✓✓` read (accent)
    if let (Some(last), Some(m)) = (lines.last_mut(), mark_span(mark)) {
        last.spans.push(m);
    }
    // BISE-90 (user decision, marketing's look): a thin accent bar at
    // column 0 on every row of your message, the text from column 3 (`›`
    // stays the composer's prompt); the heavy `┃` is a card's
    let bar = Span::styled(format!("{}  ", user_bar()), Style::default().fg(accent()));
    let mut rows = hung_rows(&bar, &bar, lines, width);
    // the sizes of its images, dim, under it (still behind the bar)
    if let Some(sizes) = crate::attach::sizes_line(msg) {
        rows.extend(hung_rows(&bar, &bar, [Line::from(Span::styled(sizes, Style::default().fg(dim())))], width));
    }
    rows
}

/// The bar in front of your messages: `│`, `|` under `BISE_ASCII=1`.
fn user_bar() -> &'static str {
    if crate::theme::ascii_mode() {
        "|"
    } else {
        "│"
    }
}

/// The mark after your message (C3).
fn mark_span(mark: Mark) -> Option<Span<'static>> {
    let (g, c) = match mark {
        Mark::Sent => (G_SENDING, dim()),
        Mark::Received => (G_RECEIVED, faint()),
        Mark::Read => (G_READ, accent()),
        Mark::Failed => (G_FAILED, error()),
    };
    Some(Span::styled(format!(" {}", glyph(g)), Style::default().fg(c)))
}

// each line wrapped to the width left after the bar, every row (the
// wrapped continuations too) behind the same bar, so the text stays
// aligned; a continuation keeps its soft mark (the copy joins it)
fn barred_rows(
    bar: &Span<'static>,
    lines: impl IntoIterator<Item = Line<'static>>,
    width: usize,
) -> Vec<Line<'static>> {
    hung_rows(bar, bar, lines, width)
}

// the same with a different prefix on the very first row (a glyph) and
// on all the others (its blank indent); both the same width
fn hung_rows(
    first: &Span<'static>,
    rest: &Span<'static>,
    lines: impl IntoIterator<Item = Line<'static>>,
    width: usize,
) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let inner = width.saturating_sub(first.content.width()).max(1);
    let mut rows = Vec::new();
    for l in lines {
        for r in wrap_line(l, inner) {
            let mut spans = vec![if rows.is_empty() { first.clone() } else { rest.clone() }];
            spans.extend(r.spans);
            let mut row = Line::from(spans);
            row.alignment = r.alignment;
            rows.push(row);
        }
    }
    rows
}


// the tool line (book §6, mockup "inside an agent"): the tool's glyph,
// its name, its state (the working pulse and the elapsed time while it
// runs, `✓` once ok, `✗` in the error color on failure), the args.
// " 1.2s" after the tool name; nothing for a replayed tool (no duration)
pub(crate) fn elapsed_label(elapsed: &Option<String>) -> String {
    match elapsed.as_deref() {
        Some(e) if !e.is_empty() => format!(" {}", e),
        _ => String::new(),
    }
}

// the name and the one-line args of a tool, and its decoded source
pub(crate) fn tool_meta(td: &ToolData) -> (String, String, Option<(CodeLang, String)>) {
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
    (name, args, code)
}

// the tool line itself: the only part of a running tool that changes
// from one frame to the next (spinner, elapsed)
pub(crate) fn tool_head(td: &ToolData, tick: u32, name: &str, args: &str) -> Line<'static> {
    if name == "apply_patch" {
        if let Some(src) = td.code.as_deref().map(|raw| tool_source(CodeLang::Patch, wire_decode(raw))) {
            return edit_head(td, tick, &src);
        }
    }
    // book §6: `$` bash, `λ` TypeScript; other tools keep an empty
    // glyph column
    let (glyph, label) = match name {
        "bash" => (G_BASH, "bash"),
        "run_typescript" => (G_TS, "typescript"),
        other => (" ", other),
    };
    let dim_st = Style::default().fg(dim());
    // §9 Emphasis: a one-line tool call is the agent's own work, dim
    let mut row = vec![
        Span::styled(format!(" {} ", glyph), dim_st),
        Span::styled(label.to_string(), dim_st),
    ];
    match td.state {
        ToolState::Run => {
            let (g, c) = working_frame(tick);
            row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
            row.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            row.push(Span::styled(format!(" {}{}", G_RECEIVED, elapsed_label(&td.elapsed)), dim_st));
        }
        ToolState::Fail => {
            row.push(Span::styled(
                format!(" {}{}", G_FAILED, elapsed_label(&td.elapsed)),
                Style::default().fg(error()),
            ));
        }
    }
    if !args.is_empty() {
        row.push(Span::styled(format!(" · {}", args), dim_st));
    }
    Line::from(row)
}

// everything under the tool line (book §11, progressive disclosure): a
// bash or TypeScript script always in full (never folded, whatever its
// length); an edit's diff only when opened (its line says the rest);
// the output one line, `▸ output`, until opened
pub(crate) fn tool_body(td: &ToolData, code: &Option<(CodeLang, String)>, width: usize) -> Vec<Line<'static>> {
    let mut ls = Vec::new();
    match code {
        Some((CodeLang::Patch, src)) => {
            if td.expanded {
                ls.extend(code_block_lines(src, CodeLang::Patch, &td.state, width));
            }
            // an edit's result is on its line (✓ +3 −1, or why it failed)
            return ls;
        }
        Some((lang, src)) => ls.extend(code_block_lines(src, *lang, &td.state, width)),
        None => {}
    }
    ls.extend(output_lines(td, width));
    ls
}

/// The output of a tool (the runtime's one-line preview): closed,
/// `▸ output` (` · 3 failed` when the text says so); a failure shows its
/// reason in the error color instead, `▸` when cut. Open, the whole
/// text under the rail.
pub(crate) fn output_lines(td: &ToolData, width: usize) -> Vec<Line<'static>> {
    let Some((ok, preview)) = &td.result else { return Vec::new() };
    // images in the result: `result · ▣ shot.png 390×844` (book §14)
    let images = crate::attach::result_spans(preview);
    let shown = if images.is_some() { crate::attach::without_markers(preview) } else { preview.clone() };
    let text = shown.trim();
    let faint_st = Style::default().fg(dim());
    if let Some(spans) = images.filter(|_| *ok) {
        let mut row = vec![Span::raw("   ")];
        if !text.is_empty() {
            row.push(Span::styled(format!("{} ", if td.expanded { G_OPEN } else { G_CLOSED }), faint_st));
        }
        row.extend(spans);
        let mut ls = vec![Line::from(row)];
        if td.expanded && !text.is_empty() {
            let hl: Vec<Vec<Span<'static>>> = text.split('\n').map(|l| vec![Span::styled(l.to_string(), faint_st)]).collect();
            ls.extend(rail_rows(&hl, width));
        }
        return ls;
    }
    if text.is_empty() {
        return Vec::new();
    }
    let text_st = Style::default().fg(if *ok { dim() } else { error() });
    let mut ls = Vec::new();
    if td.expanded {
        ls.push(Line::from(Span::styled(format!("   {} output", G_OPEN), faint_st)));
        let hl: Vec<Vec<Span<'static>>> = text
            .split('\n')
            .map(|l| vec![Span::styled(l.to_string(), text_st)])
            .collect();
        ls.extend(rail_rows(&hl, width));
        return ls;
    }
    if *ok {
        let mut label = format!("   {} output", G_CLOSED);
        if let Some(k) = failed_count(text) {
            label.push_str(&format!(" · {} failed", k));
        }
        ls.push(Line::from(Span::styled(label, faint_st)));
        return ls;
    }
    // a failure: one line, its reason first; `▸` when there is more
    let room = width.saturating_sub(3 + 2).max(8);
    let first = text.lines().next().unwrap_or("");
    let cut = first.chars().count() > room || text.lines().nth(1).is_some();
    let mut row = vec![Span::styled(format!("   {}", fit_chars(first, room)), text_st)];
    if cut {
        row.push(Span::styled(format!(" {}", G_CLOSED), faint_st));
    }
    ls.push(Line::from(row));
    ls
}

/// "3 failed" in a test runner's output: the first count above zero.
pub(crate) fn failed_count(text: &str) -> Option<u64> {
    let mut rest = text;
    while let Some(k) = rest.find(" failed") {
        let before = rest[..k].trim_end_matches(',');
        let n: String = before.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let n: String = n.chars().rev().collect();
        if let Ok(v) = n.parse::<u64>() {
            if v > 0 && (before.len() == n.len() || !before[..before.len() - n.len()].ends_with(|c: char| c.is_alphanumeric())) {
                return Some(v);
            }
        }
        rest = &rest[k + " failed".len()..];
    }
    None
}

/// An edit's line (book §11): `± edit {path} ✓ +{a} −{d} ▸`; several
/// files read `{n} files`; a failure gives its reason in the error color.
pub(crate) fn edit_head(td: &ToolData, tick: u32, src: &str) -> Line<'static> {
    let files = patch_files(src);
    let target = match files.as_slice() {
        [(p, _, _)] => p.clone(),
        fs => format!("{} files", fs.len()),
    };
    let (adds, dels) = files.iter().fold((0, 0), |(a, d), f| (a + f.1, d + f.2));
    let dim_st = Style::default().fg(dim());
    let mut row = vec![
        Span::styled(format!(" {} ", G_PATCH), Style::default().fg(text())),
        Span::styled(format!("edit {}", target), Style::default().fg(text())),
    ];
    let mark = format!(" {}", if td.expanded { G_OPEN } else { G_CLOSED });
    match td.state {
        ToolState::Run => {
            let (g, c) = working_frame(tick);
            row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
            row.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            let mut counts = format!(" {}", G_RECEIVED);
            if adds > 0 {
                counts.push_str(&format!(" +{}", adds));
            }
            if dels > 0 {
                counts.push_str(&format!(" −{}", dels));
            }
            counts.push_str(&mark);
            row.push(Span::styled(counts, dim_st));
        }
        ToolState::Fail => {
            let reason = td
                .result
                .as_ref()
                .map(|(_, r)| r.trim().to_string())
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| "failed".into());
            row.push(Span::styled(format!(" {} {}", G_FAILED, reason), Style::default().fg(error())));
            row.push(Span::styled(mark, dim_st));
        }
    }
    Line::from(row)
}

pub(crate) fn tool_lines(td: &ToolData, tick: u32, width: usize) -> Vec<Line<'static>> {
    let (name, args, code) = tool_meta(td);
    if crate::toolbox::is_boxed(td) {
        return crate::toolbox::box_lines(td, &code, &[], tick, code_width(width));
    }
    let mut ls = vec![tool_head(td, tick, &name, &args)];
    ls.extend(tool_body(td, &code, width));
    ls
}

// ---- compaction (book §6: `≡` dim, pulsing while it runs) ----

/// `≡ compacting`, the glyph pulsing (dim / faint) while `running`.
pub(crate) fn compacting_line(tick: u32, running: bool) -> Vec<Line<'static>> {
    let glyph_c = if running && !(tick / 4).is_multiple_of(2) { faint() } else { dim() };
    vec![Line::from(vec![
        Span::styled(format!(" {} ", G_COMPACTING), Style::default().fg(glyph_c)),
        Span::styled("compacting", Style::default().fg(dim())),
    ])]
}

/// `≡ summary ▸`; open, the summary under the rail.
fn summary_lines(summary: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let body = unescape_md(summary);
    let has = !body.trim().is_empty();
    let mut head = vec![
        Span::styled(format!(" {} ", G_SUMMARY), dim_st),
        Span::styled("summary", dim_st),
    ];
    if has {
        head.push(Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), dim_st));
    }
    let mut ls = vec![Line::from(head)];
    if open && has {
        let bar = Span::styled(RAIL, Style::default().fg(faint()));
        ls.extend(barred_rows(&bar, md_lines(&body, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

// ---- folded messages: reports in main, the brief inside an agent ----

/// A report (`[report: done] summary…`): its kind and its text.
pub(crate) fn report_parts(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[report: ")?;
    let (kind, body) = rest.split_once(']')?;
    Some((kind.trim(), body.trim_start()))
}

/// The brief an agent got from main (`# Task \`name\`` …).
pub(crate) fn is_brief(text: &str) -> bool {
    text.starts_with("# Task `")
}

/// A report is one line, `♡ bench: the summary ▸ report`; open, the rest
/// of it under the rail. The glyph says the kind: `♡` done, `✗` failed,
/// `?` blocked, `·` progress.
fn report_lines(from: &str, kind: &str, body: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let (glyph, color, st) = match kind {
        "done" => (G_DONE, text(), text()),
        k if k.contains("fail") => (G_FAILED, error(), text()),
        "blocked" => (G_NEEDS_YOU, accent(), text()),
        _ => (G_STARTING, dim(), dim()),
    };
    let first = body.lines().next().unwrap_or("").trim();
    let rest = body.split_once('\n').map(|(_, r)| r.trim_matches('\n')).unwrap_or("");
    let head = format!("{}: ", from);
    let label = format!(" {} report", if open { G_OPEN } else { G_CLOSED });
    let room = width.saturating_sub(3 + head.chars().count() + label.chars().count()).max(8);
    let more = !rest.trim().is_empty() || first.chars().count() > room;
    let shown = if open { first.to_string() } else { fit_chars(first, room) };
    let mut row = vec![Span::styled(head, Style::default().fg(st)), Span::styled(shown, Style::default().fg(st))];
    if more {
        row.push(Span::styled(label, Style::default().fg(dim())));
    }
    // open, its first line may be longer than the row: its wrapped rows
    // hang under the text, never at column 1 (BISE-90)
    let mark = Span::styled(format!(" {} ", glyph), Style::default().fg(color));
    let mut ls = hung_rows(&mark, &Span::raw("   "), [Line::from(row)], width);
    if open && !rest.trim().is_empty() {
        let bar = Span::styled(" │ ", Style::default().fg(faint()));
        ls.extend(barred_rows(&bar, md_lines(rest, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

/// The brief inside an agent: `◇ brief ▸`; open, the brief under the
/// rail (without its `# Task` title: the agent is the view).
fn brief_lines(brief: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let mut ls = vec![Line::from(vec![
        Span::styled(format!(" {} ", G_BRIEF), Style::default().fg(text())),
        Span::styled("brief", Style::default().fg(text())),
        Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), Style::default().fg(dim())),
    ])];
    if open {
        let body = brief.split_once('\n').map(|(_, r)| r.trim_matches('\n')).unwrap_or("");
        let bar = Span::styled(" │ ", Style::default().fg(faint()));
        ls.extend(barred_rows(&bar, md_lines(body, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

// a multi-line user message (Shift+Enter, paste) and an agent message:
// one row per line, a long line wrapped, every row behind the bar
#[cfg(test)]
mod multiline_tests {
    use crate::feed::build_rows;
    use crate::wire::Ev;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    fn screen(ev: Ev, width: u16) -> Vec<String> {
        let rows = build_rows(&[ev], 0, false, width as usize, 0);
        let h = rows.len() as u16;
        let mut term = Terminal::new(TestBackend::new(width, h)).unwrap();
        term.draw(|f| f.render_widget(Paragraph::new(rows), f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
            .collect()
    }

    #[test]
    fn user_message_keeps_its_line_breaks() {
        let long = "word ".repeat(12);
        let text = format!("first line\nsecond line\n{}end", long);
        let s = screen(Ev::You(text, crate::wire::Mark::Read), 30);
        // the bar on every row (BISE-90), every row's text at column 3
        assert_eq!(s[0].as_str(), "│  first line", "{s:#?}");
        assert_eq!(s[1].as_str(), "│  second line", "{s:#?}");
        // the long line wraps into several rows, all at the same column
        assert!(s.len() >= 5, "{s:#?}");
        for r in &s[2..] {
            assert!(r.starts_with("│  word") || r.starts_with("│  end") || r.trim_start_matches('│').trim() == "✓✓", "{s:#?}");
            assert!(r.chars().count() <= 30);
        }
        assert!(s.last().unwrap().ends_with("end ✓✓"), "{s:#?}");
        assert!(!s.iter().any(|r| r.contains('\n')));
    }

    #[test]
    fn agent_message_wraps_behind_its_bar() {
        // a level-3 line is one line; open, its whole text under the rail
        let text = format!("one{}two {}", '\n', "x ".repeat(30));
        let s = screen(Ev::AgentMsg { from: "main".into(), to: String::new(), text, level: 3, id: String::new(), open: true, fold: false }, 40);
        let head = format!(" │ {} main", crate::theme::G_MSG);
        assert!(s[0].starts_with(&head), "{s:#?}");
        let body = &s[1..];
        assert_eq!(body[0].as_str(), " │   one", "{s:#?}");
        assert!(body[1].starts_with(" │   two x"), "{s:#?}");
        assert!(body.len() >= 3, "{s:#?}");
        assert!(body.iter().all(|r| r.starts_with(" │   ")), "{s:#?}");
    }
}
