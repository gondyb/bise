//! The inbox drawn (cards.rs has the model and the keys; designer's
//! round 2, variant A): its own box above the divider, a rounded border
//! with its title in it, a row per item numbered for ctrl+1-9; the item
//! open in place where its row was (raised, the accent bar), the fold of
//! the last answer on top; the item full screen (ctrl+o) in the
//! history's place; the divider's label and the key bars.

use super::cards::{cut, glyph_color, kind_look, shape, Card, CardHit, Enter, Fold, Part, Shape};
use super::*;
use crate::theme;
use unicode_width::UnicodeWidthStr;

/// The widest a card's text gets: prose wraps at 88, like the reading
/// column (book §8, §11).
const READ_WIDTH: usize = 88;

/// Below this screen height the box at rest is one row (the top item,
/// `+ n`).
const SMALL_H: u16 = 24;

/// At most this many item rows in the box, then `+ n more`.
const STRIP_ROWS: usize = 3;

/// The fewest rows an open item takes in place (its blank rows, head,
/// one line, the options and the hint): below, the other rows step out.
const ITEM_MIN: usize = 7;

/// Approvals that open this close together share one strip row.
const TOGETHER_MS: u64 = 5_000;

/// A strip row: one card, or approvals that came together.
enum Entry<'a> {
    One(&'a Card),
    Group(Vec<&'a Card>),
}

impl Entry<'_> {
    fn first(&self) -> &Card {
        match self {
            Entry::One(c) => c,
            Entry::Group(v) => v[0],
        }
    }

    fn holds(&self, id: u64) -> bool {
        match self {
            Entry::One(c) => c.id == id,
            Entry::Group(v) => v.iter().any(|c| c.id == id),
        }
    }
}

fn entries(sb: &Sb) -> Vec<Entry<'_>> {
    let mut out: Vec<Entry> = Vec::new();
    for c in sb.sorted_cards() {
        if c.kind == "approval" {
            if let Some(last) = out.last_mut() {
                let f = last.first();
                if f.kind == "approval" && f.age_now().abs_diff(c.age_now()) <= TOGETHER_MS {
                    match last {
                        Entry::One(a) => *last = Entry::Group(vec![*a, c]),
                        Entry::Group(v) => v.push(c),
                    }
                    continue;
                }
            }
        }
        out.push(Entry::One(c));
    }
    out
}

/// The strip's rows, by the first card of each (approvals that came
/// together share a row): row N is what ctrl+N opens (BISE-302).
pub(super) fn strip_ids(sb: &Sb) -> Vec<u64> {
    entries(sb).iter().map(|e| e.first().id).collect()
}

/// The strip's number of each card (1 = the most blocking; approvals
/// that came together share their row's), for the panel's inbox rows.
pub(super) fn row_numbers(sb: &Sb) -> Vec<(u64, usize)> {
    let mut out = Vec::new();
    for (i, e) in entries(sb).iter().enumerate() {
        match e {
            Entry::One(c) => out.push((c.id, i + 1)),
            Entry::Group(v) => out.extend(v.iter().map(|c| (c.id, i + 1))),
        }
    }
    out
}

/// A row's number (BISE-302, designer): faint; ctrl held (ctrlhint.rs),
/// in accent (bold under NO_COLOR) where ctrl+N opens it. Nothing moves.
pub(super) fn number_style(app: &App, base: Color) -> Style {
    let lit = app.ctrl_digits && crate::ctrlhint::held(app) == Some(crate::ctrlhint::Held::Ctrl);
    match (lit, no_color()) {
        (true, true) => Style::default().add_modifier(Modifier::BOLD),
        (true, false) => fg(theme::accent()),
        (false, _) => fg(base),
    }
}

/// NO_COLOR: no tint to raise a row, reverse video instead.
fn no_color() -> bool {
    theme::raised() == Color::Reset
}

/// A new frame: the hits of the last one go (the box or the full-screen
/// item adds its own when drawn).
pub(crate) fn card_frame(app: &App) {
    app.sb.card.hits.borrow_mut().clear();
}

/// An item is open, in its box or full screen.
pub(crate) fn item_open(app: &App) -> bool {
    app.sb.card.open && !app.sb.sorted_cards().is_empty()
}

/// The full-screen item is up (ctrl+o): it takes the history's place.
pub(crate) fn card_view_open(app: &App) -> bool {
    item_open(app) && app.sb.card.full
}

/// Spans and their widths, built left to right; `mark` notes where a
/// hit starts and ends.
#[derive(Default)]
struct Row {
    spans: Vec<Span<'static>>,
    w: usize,
}

impl Row {
    fn push(&mut self, s: impl Into<String>, st: Style) -> (usize, usize) {
        let s: String = s.into();
        let x = self.w;
        self.w += s.width();
        self.spans.push(Span::styled(s, st));
        (x, self.w)
    }
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

/// The digits that answer `n` options: `1`, `1-2` … `1-9`.
pub(super) fn digits(n: usize) -> &'static str {
    const PICK: [&str; 9] = ["1", "1-2", "1-3", "1-4", "1-5", "1-6", "1-7", "1-8", "1-9"];
    PICK[n.clamp(1, 9) - 1]
}

/// A line of the box or of an item, and what a click on its cells does
/// (from which column, how wide).
#[derive(Clone)]
struct Drawn {
    line: Line<'static>,
    hits: Vec<(usize, usize, CardHit)>,
    /// a line of the open item: on its tint, after its accent bar
    item: bool,
}

impl Drawn {
    fn plain(line: Line<'static>) -> Drawn {
        Drawn { line, hits: Vec::new(), item: false }
    }

    fn blank() -> Drawn {
        Drawn::plain(Line::from(""))
    }

    fn width(&self) -> usize {
        self.line.spans.iter().map(|s| s.content.width()).sum()
    }
}

/// The box's lines: its corners, edges and the open item's bar (ASCII:
/// `+ - | |`).
struct Lines {
    tl: &'static str,
    tr: &'static str,
    bl: &'static str,
    br: &'static str,
    h: &'static str,
    v: &'static str,
    bar: &'static str,
}

fn lines() -> Lines {
    if theme::ascii_mode() {
        Lines { tl: "+", tr: "+", bl: "+", br: "+", h: "-", v: "|", bar: "|" }
    } else {
        Lines { tl: "╭", tr: "╮", bl: "╰", br: "╯", h: "─", v: "│", bar: "┃" }
    }
}

/// An item's age, short (`12s`, `2m`, `3h`, `2d`; `now` under 10 s).
fn short_age(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=9 => "now".into(),
        10..=59 => format!("{}s", s),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// What a row says the item is about: an approval's agent, a question's
/// agent; the TUI's own items and the rest their own words.
fn row_who(c: &Card, s: &Shape) -> String {
    match c.kind.as_str() {
        "approval" | "confirm" | "question" => c.agent.clone(),
        _ => s.who.clone(),
    }
}

/// One row of the box, `w` columns: `1 ? t3 · $ npm publish   2m`, the
/// number faint (accent while ctrl is held), the glyph in its color, the
/// age faint on the right, `+ n` before it when the box has one row. A
/// click opens it in place.
fn box_row(app: &App, e: &Entry, n: usize, w: usize, extra: usize) -> Drawn {
    let c = e.first();
    let s = shape(c);
    let (_, glyph, _) = kind_look(&c.kind);
    let who = match e {
        Entry::One(_) => row_who(c, &s),
        Entry::Group(v) => format!("{} agents", v.len()),
    };
    let mut right = Row::default();
    if extra > 0 {
        right.push(format!("+ {}  ", extra), fg(theme::faint()));
    }
    right.push(short_age(c.age_now()), fg(theme::faint()));
    // 2 blank columns at least between the text and the right side
    let room = w.saturating_sub(right.w + 2);
    let mut left = Row::default();
    left.push(format!("{n} "), number_style(app, theme::faint()));
    left.push(glyph, fg(glyph_color(&c.kind)));
    left.push(" ", Style::default());
    let who = cut(&who, room.saturating_sub(left.w));
    left.push(who, fg(theme::text()));
    let dollar = if s.cmd { 2 } else { 0 };
    let text_room = room.saturating_sub(left.w + 3 + dollar);
    if !s.summary.is_empty() && text_room >= 4 {
        left.push(" · ", fg(theme::dim()));
        if s.cmd {
            left.push("$ ", fg(theme::accent()));
        }
        left.push(cut(&s.summary, text_room), fg(theme::text()));
        // its faint end, when it fits whole (`new file · 14 lines`)
        let note_room = room.saturating_sub(left.w + 1);
        if !s.note.is_empty() && s.note.width() <= note_room {
            left.push(format!(" {}", s.note), fg(theme::faint()));
        }
    }
    let fill = w.saturating_sub(left.w + right.w);
    let mut spans = left.spans;
    spans.push(Span::raw(" ".repeat(fill)));
    spans.extend(right.spans);
    Drawn { line: Line::from(spans), hits: vec![(0, w, CardHit::Row(c.id))], item: false }
}

/// `+ n more · ? release · npm publish`, faint: a click opens the first
/// of them.
fn more_row(es: &[Entry], hidden: &[usize], w: usize) -> Drawn {
    let first = es[hidden[0]].first();
    let s = shape(first);
    let (_, glyph, _) = kind_look(&first.kind);
    let mut t = format!("+ {} more · {} {}", hidden.len(), glyph, row_who(first, &s));
    if !s.summary.is_empty() {
        t.push_str(&format!(" · {}{}", if s.cmd { "$ " } else { "" }, s.summary));
    }
    let line = Line::from(Span::styled(cut(&t, w), fg(theme::faint())));
    Drawn { line, hits: vec![(0, w, CardHit::Row(first.id))], item: false }
}

/// The last answer, folded: `✓ you allowed t3: npm publish` (✓ accent),
/// `✗ you said no to …: … · "note"` (all dim).
fn fold_row(f: &Fold, w: usize) -> Drawn {
    let (mark, st) = if f.ok { (theme::glyph(theme::G_RECEIVED), fg(theme::accent())) } else { (theme::glyph(theme::G_FAILED), fg(theme::dim())) };
    // the box's fold stays one line: a typed answer after the sentence,
    // a no's note in quotes (the history has them whole, BISE-307)
    let t = match (f.note.is_empty(), f.ok) {
        (true, _) => f.text.clone(),
        (false, true) => format!("{}: {}", f.text, f.note.split_whitespace().collect::<Vec<_>>().join(" ")),
        (false, false) => format!("{} · \"{}\"", f.text, f.note.split_whitespace().collect::<Vec<_>>().join(" ")),
    };
    let room = w.saturating_sub(mark.width() + 1);
    Drawn::plain(Line::from(vec![Span::styled(mark, st), Span::raw(" "), Span::styled(cut(&t, room), fg(theme::dim()))]))
}

/// Where the agent is: `t3 is shipping 2.5.0 · its last step: ✓ npm run
/// build · 12s` (its role, its last finished tool row while its feed is
/// loaded); none when neither is known.
fn where_line(app: &App, agent: &str) -> Option<String> {
    let sb = &app.sb;
    // main's question comes from its own `sb card`: its last step is the
    // item itself (designer)
    if sb.agents.iter().any(|a| a.name == agent && a.main) || agent == "main" {
        return None;
    }
    let role = sb.agents.iter().find(|a| a.name == agent).map(|a| a.role.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let step = last_step(app, agent);
    match (role.is_empty(), step) {
        (true, None) => None,
        (false, None) => Some(format!("{agent} is {role}")),
        (true, Some(s)) => Some(format!("{agent}'s last step: {s}")),
        (false, Some(s)) => Some(format!("{agent} is {role} · its last step: {s}")),
    }
}

/// The last tool row `agent` finished, in the feed this TUI holds:
/// `✓ npm run build · 12s`.
fn last_step(app: &App, agent: &str) -> Option<String> {
    use crate::wire::ToolState;
    let sb = &app.sb;
    let events = if sb.focus == agent { &app.events } else { &sb.views.get(agent)?.events };
    let td = events.iter().rev().find_map(|e| match e {
        Ev::Tool(td) if matches!(td.state, ToolState::Ok | ToolState::Fail) => Some(td),
        _ => None,
    })?;
    let (name, args, code) = crate::render::tool_meta(td);
    let what = match code {
        Some((_, src)) => src.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string(),
        None if args.is_empty() => name,
        None => format!("{name} {args}"),
    };
    let mark = if matches!(td.state, ToolState::Ok) { theme::glyph(theme::G_RECEIVED) } else { theme::glyph(theme::G_FAILED) };
    let mut t = format!("{mark} {}", cut(&what, 48));
    if let Some(e) = td.elapsed.as_deref().filter(|e| !e.is_empty()) {
        t.push_str(&format!(" · {e}"));
    }
    Some(t)
}

/// An item's head: `? t3 wants to run`, the glyph in its color, the
/// title bold, `1 of 4 · 2m` dim on the right (an item of the TUI's own:
/// its meta).
fn head_line(sb: &Sb, c: &Card, s: &Shape, w: usize) -> Drawn {
    let (_, glyph, _) = kind_look(&c.kind);
    let meta = match &s.meta {
        Some(own) => own.clone(),
        None => {
            let ids: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
            let pos = ids.iter().position(|x| *x == c.id).unwrap_or(0) + 1;
            format!("{} of {} · {}", pos, ids.len(), short_age(c.age_now()))
        }
    };
    let head_w = glyph.width() + 1;
    let meta = if head_w + s.title.width().min(12) + 2 + meta.width() <= w { meta } else { String::new() };
    let title = cut(&s.title, w.saturating_sub(head_w + if meta.is_empty() { 0 } else { meta.width() + 2 }));
    let fill = w.saturating_sub(head_w + title.width() + meta.width());
    Drawn::plain(Line::from(vec![
        Span::styled(glyph, fg(glyph_color(&c.kind)).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(title, fg(theme::text()).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(fill)),
        Span::styled(meta, fg(theme::dim())),
    ]))
}

/// The middle of an item, `w` columns: what it asks (its text, its
/// command or patch in the code colors; what was already allowed dim
/// above it), then a blank line and, dim, why it asks, the hub's remark
/// and where the agent is.
fn middle_lines(app: &App, c: &Card, s: &Shape, w: usize) -> Vec<Drawn> {
    let mut main: Vec<Drawn> = Vec::new();
    let mut side: Vec<Drawn> = Vec::new();
    let plain = |out: &mut Vec<Drawn>, text: &str, st: Style| {
        for l in text.lines() {
            // BISE-290: bise's own `[label](url)` is a link
            for row in wrap_line(Line::from(crate::textlayer::copy_spans(l, st)), w) {
                out.push(Drawn::plain(row));
            }
        }
    };
    let mut asked = false;
    for p in &s.parts {
        match p {
            Part::Text(t) => {
                if asked {
                    main.push(Drawn::blank());
                }
                plain(&mut main, t.trim_end(), fg(theme::text()));
                asked = true;
            }
            Part::Code(rows) => {
                for spans in rows {
                    for row in wrap_line(Line::from(spans.clone()), w.max(1)) {
                        main.push(Drawn::plain(row));
                    }
                }
                asked = true;
            }
            // what was allowed already: dim, above the command
            Part::Reason(t) if !asked => plain(&mut main, t, fg(theme::dim())),
            Part::Reason(t) => plain(&mut side, t, fg(theme::dim())),
            Part::Note(t) => plain(&mut side, t, fg(theme::dim()).add_modifier(Modifier::ITALIC)),
        }
    }
    if let Some(at) = where_line(app, &c.agent).filter(|_| !super::setup::is_local(c.id)) {
        plain(&mut side, &at, fg(theme::dim()));
    }
    if !side.is_empty() {
        main.push(Drawn::blank());
        main.extend(side);
    }
    main
}

/// A highlighted option: accent on the ground (inverse), reversed under
/// NO_COLOR.
fn picked() -> Style {
    if no_color() {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().fg(theme::on_accent()).bg(theme::accent())
    }
}

/// The options, `w` columns: on one line (`1 allow   2 always allow …
/// here   3 no`, digits accent, labels dim) or one per line when they
/// don't fit; the one highlighted (`hi`) accent on the ground. `typing`:
/// the composer answers, the digits dim and no highlight. Then the hint,
/// faint: `or type why not, ⏎ says no`.
fn option_lines(s: &Shape, id: u64, w: usize, hi: Option<usize>, typing: bool) -> Vec<Drawn> {
    let mut out = Vec::new();
    let num_st = fg(if typing { theme::dim() } else { theme::accent() });
    let label_st = fg(theme::dim());
    let texts: Vec<String> = s.options.iter().enumerate().map(|(i, l)| format!("{} {}", s.num(i), l)).collect();
    let one = texts.iter().map(|t| t.width()).sum::<usize>() + 3 * texts.len().saturating_sub(1);
    if !texts.is_empty() && one <= w {
        let mut r = Row::default();
        let mut hits = Vec::new();
        for (i, label) in s.options.iter().enumerate() {
            if i > 0 {
                r.push("   ", Style::default());
            }
            let x = r.w;
            if !typing && hi == Some(i) {
                r.push(texts[i].clone(), picked());
            } else {
                r.push(format!("{}", s.num(i)), num_st);
                r.push(format!(" {label}"), label_st);
            }
            hits.push((x, r.w - x, CardHit::Pick(id, i)));
        }
        out.push(Drawn { line: Line::from(r.spans), hits, item: false });
    } else {
        for (i, label) in s.options.iter().enumerate() {
            let on = !typing && hi == Some(i);
            let rows = wrap_line(Line::from(label.clone()), w.saturating_sub(2).max(1));
            for (j, row) in rows.into_iter().enumerate() {
                let text: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
                let lead = if j == 0 { format!("{} ", s.num(i)) } else { "  ".to_string() };
                let line = if on {
                    Line::from(Span::styled(format!("{lead}{text}"), picked()))
                } else {
                    Line::from(vec![Span::styled(lead, num_st), Span::styled(text, label_st)])
                };
                let d = Drawn { line, hits: Vec::new(), item: false };
                let wd = d.width();
                out.push(Drawn { hits: vec![(0, wd, CardHit::Pick(id, i))], ..d });
            }
        }
    }
    if let Some(h) = hint(s) {
        out.push(Drawn::plain(Line::from(Span::styled(cut(h, w), fg(theme::faint())))));
    }
    out
}

/// The hint under the options: how words answer.
fn hint(s: &Shape) -> Option<&'static str> {
    if !s.keys.is_empty() {
        return None;
    }
    match s.enter {
        Enter::Deny => Some("or type why not, ⏎ says no"),
        Enter::Answer if s.words && !s.options.is_empty() => Some("or type your answer, ⏎ sends it"),
        Enter::Answer if s.words => Some("type your answer, ⏎ sends it"),
        Enter::Ack => Some("⏎ got it"),
        Enter::Answer => None,
    }
}

/// The open item in place, `w` columns (its bar and gap and its right
/// padding included), in
/// at most `cap` rows: a blank row, the head, the middle, a blank row,
/// the options and the hint, a blank row; a middle too long for `cap`
/// keeps its first lines, then `… n more lines · ctrl+o full screen`.
fn item_block(app: &App, c: &Card, w: usize, cap: usize) -> Vec<Drawn> {
    let sb = &app.sb;
    let s = shape(c);
    // the bar and its gap on the left, one tinted column on the right:
    // `1 of 4 · 2m` never touches the tint's edge
    let tw = w.saturating_sub(3).max(1);
    let typing = !app.ed.text.is_empty();
    let mut middle = middle_lines(app, c, &s, tw);
    let opts = option_lines(&s, c.id, tw, sb.card.opt, typing);
    let fixed = 4 + opts.len();
    let room = cap.saturating_sub(fixed);
    if middle.len() > room {
        let keep = room.saturating_sub(1);
        let more = middle.len() - keep;
        middle.truncate(keep);
        // a blank line never ends what is kept
        while middle.last().is_some_and(|d| d.width() == 0) {
            middle.pop();
        }
        let more = more + (keep - middle.len());
        let f = fg(theme::faint());
        middle.push(Drawn::plain(Line::from(vec![
            Span::styled(format!("{} {} more line{} · ", theme::ellipsis(), more, if more == 1 { "" } else { "s" }), f),
            Span::styled("ctrl+o", fg(theme::text())),
            Span::styled(" full screen", f),
        ])));
    }
    let mut out = vec![Drawn::blank(), head_line(sb, c, &s, tw)];
    out.extend(middle);
    if !opts.is_empty() {
        out.push(Drawn::blank());
        out.extend(opts);
    }
    out.push(Drawn::blank());
    out.truncate(cap.max(1));
    for d in &mut out {
        d.item = true;
    }
    out
}

/// The box's inner lines, `w` columns, at most `max`; the open item at
/// most `half` rows. At rest: a row per item (3 at most), `+ n more`;
/// one row (`+ n` on its right) when `max` is short. An item open: the
/// fold of the last answer on top (2 s), the rows, the open one grown
/// where it stood (past the third: in the third's place).
fn box_lines(app: &App, w: usize, max: usize, half: usize) -> Vec<Drawn> {
    let sb = &app.sb;
    let es = entries(sb);
    if es.is_empty() || max == 0 {
        return Vec::new();
    }
    let open = sb.card.sel.filter(|_| sb.card.open && !sb.card.full);
    let at = open.and_then(|id| es.iter().position(|e| e.holds(id)));
    let mut shown: Vec<usize> = (0..es.len().min(STRIP_ROWS)).collect();
    if let Some(i) = at.filter(|i| *i >= STRIP_ROWS) {
        shown.pop();
        shown.push(i);
    }
    let fold = sb.card.fresh_fold().filter(|_| at.is_some());
    let Some((at, id)) = at.zip(open) else {
        // at rest
        let need = shown.len() + usize::from(es.len() > shown.len());
        if need > max {
            return vec![box_row(app, &es[0], 1, w, es.len() - 1)];
        }
        let mut out: Vec<Drawn> = shown.iter().map(|&i| box_row(app, &es[i], i + 1, w, 0)).collect();
        let hidden: Vec<usize> = (shown.len()..es.len()).collect();
        if !hidden.is_empty() {
            out.push(more_row(&es, &hidden, w));
        }
        return out;
    };
    let Some(c) = sb.card_by_id(id) else { return Vec::new() };
    // the other rows step out when the item would be cramped
    let others = |shown: &[usize]| shown.len() - 1 + usize::from(es.len() > shown.len()) + usize::from(fold.is_some());
    if max.saturating_sub(others(&shown)) < ITEM_MIN {
        shown = vec![at];
    }
    let cap = half.max(ITEM_MIN).min(max.saturating_sub(others(&shown)));
    let mut out = Vec::new();
    if let Some(f) = fold.filter(|_| max > ITEM_MIN) {
        out.push(fold_row(f, w));
    }
    for &i in &shown {
        if i == at {
            out.extend(item_block(app, c, w, cap));
        } else {
            out.push(box_row(app, &es[i], i + 1, w, 0));
        }
    }
    let hidden: Vec<usize> = (0..es.len()).filter(|i| !shown.contains(i)).collect();
    if !hidden.is_empty() && out.len() < max {
        out.push(more_row(&es, &hidden, w));
    }
    out.truncate(max);
    out
}

/// What the box may take: the screen's height, the rows free (`room`),
/// the open item's most (half the feed area).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxFit {
    pub(crate) screen_h: u16,
    pub(crate) room: u16,
    pub(crate) half: u16,
}

impl BoxFit {
    /// The most inner lines: one at rest under 24 rows.
    fn max(self, app: &App) -> usize {
        if !app.sb.card.open && self.screen_h < SMALL_H {
            1
        } else {
            self.room.saturating_sub(2) as usize
        }
    }
}

/// The box's height in a `w`-column box: its borders and its lines;
/// none while no item waits, or full screen (the item has the history's
/// place).
pub(crate) fn box_height(app: &App, fit: BoxFit, w: u16) -> u16 {
    if app.sb.card.open && app.sb.card.full {
        return 0;
    }
    if w < 12 || fit.room < 3 {
        return 0;
    }
    let n = box_lines(app, w as usize - 4, fit.max(app), fit.half as usize).len();
    if n == 0 {
        0
    } else {
        n as u16 + 2
    }
}

/// The title's right side (BISE-302, designer): what opens a row, never
/// a key that doesn't work: `ctrl+1-3 open` (the rows shown), else
/// `click to open`, else `/inbox opens it`. The key in text color, the
/// words dim.
fn opener_label(app: &App, rows: usize) -> (String, &'static str) {
    match crate::reach::opener(app) {
        crate::reach::Opener::CtrlDigit if rows <= 1 => ("ctrl+1".into(), "open"),
        crate::reach::Opener::CtrlDigit => (format!("ctrl+1-{}", rows.min(STRIP_ROWS)), "open"),
        crate::reach::Opener::Click => ("click".into(), "to open"),
        crate::reach::Opener::Command => ("/inbox".into(), "opens it"),
    }
}

/// The top border, `w` columns: `╭─ inbox · 4 waiting for you ───
/// ctrl+1-3 open ─╮`, the title faint (`inbox · 4 waiting` when short),
/// the key in text color, its words dim; a click opens the top item.
fn top_border(app: &App, w: usize) -> Line<'static> {
    let l = lines();
    let line_st = fg(theme::rule());
    let n = app.sb.sorted_cards().len();
    let (key, words) = opener_label(app, entries(&app.sb).len());
    let right = format!(" {key} {words} ");
    // the corners and the `─ ` around the title and the right side
    let frame_w = 2 + 1 + 2;
    let mut title = format!("inbox · {n} waiting for you");
    for t in [format!("inbox · {n} waiting for you"), format!("inbox · {n} waiting"), "inbox".to_string()] {
        title = t;
        if frame_w + title.width() + 1 + right.width() <= w {
            break;
        }
    }
    let show_right = frame_w + title.width() + 1 + right.width() <= w;
    let title = cut(&title, w.saturating_sub(frame_w + 1));
    let used = frame_w + title.width() + if show_right { right.width() } else { 1 };
    let mut spans = vec![
        Span::styled(format!("{}{} ", l.tl, l.h), line_st),
        Span::styled(title, fg(theme::faint())),
        Span::styled(format!(" {}", l.h.repeat(w.saturating_sub(used + 1))), line_st),
    ];
    if show_right {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(key, fg(theme::text())));
        spans.push(Span::styled(format!(" {words} "), fg(theme::dim())));
    } else {
        spans.push(Span::styled(l.h.to_string(), line_st));
    }
    spans.push(Span::styled(format!("{}{}", l.h, l.tr), line_st));
    Line::from(spans)
}

/// The inbox's box in `area` (from the gutter to the panel), in `fit`: its border in the line color, its title in
/// it, the rows on the ground, the open item on its tint behind the
/// accent bar. A click on a row opens it (in place, or jumps to it), on
/// an option picks it, on the title the top item.
pub(crate) fn draw_box(app: &mut App, frame: &mut Frame, area: Rect, fit: BoxFit) {
    let area = area.intersection(frame.area());
    if area.height < 3 || area.width < 12 {
        return;
    }
    let l = lines();
    let line_st = fg(theme::rule());
    let w = area.width as usize - 4;
    // the lines the height was measured with, cut to the area
    let mut rows = box_lines(app, w, fit.max(app), fit.half as usize);
    rows.truncate(area.height as usize - 2);
    if rows.is_empty() {
        return;
    }
    let mut hits: Vec<(Rect, CardHit)> = Vec::new();
    let at = |y: u16, x: u16, wd: u16| Rect { x: area.x + x, y: area.y + y, width: wd, height: 1 }.intersection(area);
    frame.render_widget(Paragraph::new(top_border(app, area.width as usize)), at(0, 0, area.width));
    hits.push((at(0, 0, area.width), CardHit::Open));
    let inner_h = area.height - 2;
    for y in 1..=inner_h {
        let edge = Span::styled(l.v, line_st);
        frame.render_widget(Paragraph::new(Line::from(edge.clone())), at(y, 0, 1));
        frame.render_widget(Paragraph::new(Line::from(edge)), at(y, area.width - 1, 1));
    }
    let bottom = format!("{}{}{}", l.bl, l.h.repeat(area.width as usize - 2), l.br);
    frame.render_widget(Paragraph::new(Span::styled(bottom, line_st)), at(area.height - 1, 0, area.width));
    for (i, d) in rows.iter().enumerate().take(inner_h as usize) {
        let y = 1 + i as u16;
        let x = if d.item {
            // the tint from the bar to the inner edge, the bar in accent
            let tint = at(y, 2, w as u16);
            frame.buffer_mut().set_style(tint, Style::default().bg(theme::item_tint()));
            frame.buffer_mut().set_string(area.x + 2, area.y + y, l.bar, fg(theme::accent()));
            4
        } else {
            2
        };
        let wd = (w + 2).saturating_sub(x as usize) as u16;
        frame.render_widget(Paragraph::new(d.line.clone()), at(y, x, wd));
        for &(from, hw, h) in &d.hits {
            let r = at(y, x + from as u16, (hw as u16).min(wd.saturating_sub(from as u16)));
            if !r.is_empty() {
                hits.push((r, h));
            }
        }
    }
    // BISE-290: its text selects, copies and has links
    crate::textlayer::text(Rect { x: area.x + 2, y: area.y + 1, width: w as u16, height: inner_h });
    let cv = &mut app.sb.card;
    cv.strip = area;
    // BISE-272: the hand over what a click opens or picks
    for (r, _) in &hits {
        crate::pointer::region(*r, crate::pointer::Shape::Pointer);
    }
    cv.hits.borrow_mut().extend(hits);
}

/// The tabs of the full-screen item: `1 ? t3   2 ? api-v2   3 ? sad-404`
/// (the box's numbers), the current one accent (`[ ]` under NO_COLOR),
/// the others dim, `↑↓` faint on the right.
fn tabs_line(sb: &Sb, cur: u64, at: Rect, hits: &mut Vec<(Rect, CardHit)>) -> Line<'static> {
    let cards = sb.sorted_cards();
    let nums = row_numbers(sb);
    let w = at.width as usize;
    let labels: Vec<(u64, String, &'static str, Color, String)> = cards
        .iter()
        .map(|c| {
            let n = nums.iter().find(|(id, _)| *id == c.id).map_or(String::new(), |(_, n)| format!("{n} "));
            (c.id, n, kind_look(&c.kind).1, glyph_color(&c.kind), shape(c).tab.unwrap_or_else(|| c.agent.clone()))
        })
        .collect();
    let tab_w = |n: &str, l: &str| 1 + n.width() + 2 + l.width() + 1;
    let hint = if theme::ascii_mode() { "up/down" } else { "↑↓" };
    let with_hint = labels.len() > 1;
    let avail = if with_hint { w.saturating_sub(hint.width() + 3) } else { w };
    let ci = labels.iter().position(|l| l.0 == cur).unwrap_or(0);
    // the first tab shown: the current one always fits
    let span_w = |from: usize, to: usize| (from..=to).map(|i| tab_w(&labels[i].1, &labels[i].4) + 2).sum::<usize>();
    let mut start = 0;
    while start < ci && span_w(start, ci) > avail {
        start += 1;
    }
    let mut r = Row::default();
    for (i, (id, n, g, gc, name)) in labels.iter().enumerate().skip(start) {
        let tw = tab_w(n, name);
        if r.w + tw > avail && i != ci {
            break;
        }
        let x = r.w;
        let on = *id == cur;
        let (l, rr) = if on && no_color() { ("[", "]") } else { (" ", " ") };
        let st = if on { fg(theme::accent()) } else { fg(theme::dim()) };
        r.push(l, st);
        r.push(n.clone(), if on { st } else { fg(theme::faint()) });
        r.push(*g, if on { st } else { fg(*gc) });
        r.push(format!(" {}", name), st);
        r.push(rr, st);
        r.push("  ", Style::default());
        let xs = at.x + x as u16;
        if xs < at.right() {
            hits.push((Rect { x: xs, width: (tw as u16).min(at.right() - xs), ..at }, CardHit::Tab(*id)));
        }
    }
    if with_hint && r.w + hint.width() < w {
        let fill = w - r.w - hint.width();
        r.push(" ".repeat(fill), Style::default());
        r.push(hint, fg(theme::faint()));
    }
    Line::from(r.spans)
}

/// Where the item is scrolled (none when it fits).
fn scroll_hint(scroll: usize, max_scroll: usize) -> String {
    match max_scroll.saturating_sub(scroll) {
        0 => "end · pgup".to_string(),
        1 => format!("{} 1 more line · pgdn", theme::G_OPEN),
        n => format!("{} {} more lines · pgdn", theme::G_OPEN, n),
    }
}

/// The full-screen item in `area` (ctrl+o, the history's place): the
/// tabs, then the item: the accent bar, its head, the whole text at the
/// reading width, the options and the hint; it scrolls (pgup pgdn, the
/// wheel) when longer than the area.
pub(crate) fn draw_view(app: &mut App, frame: &mut Frame, area: Rect) {
    let area = area.intersection(frame.area());
    if area.height < 3 || area.width < 12 {
        return;
    }
    let sb = &app.sb;
    let Some(c) = sb.current_card().cloned() else { return };
    let s = shape(&c);
    let mut hits = Vec::new();
    // the tabs, 1 blank row under them when there is room
    let tabs = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(tabs_line(sb, c.id, tabs, &mut hits)), tabs);
    let gap = u16::from(area.height >= 8);
    let card = Rect { y: area.y + 1 + gap, height: area.height - 1 - gap, ..area };
    let tw = ((card.width as usize).saturating_sub(2)).min(READ_WIDTH);
    let inner = Rect { x: card.x + 2, width: tw as u16, ..card };
    frame.render_widget(Paragraph::new(head_line(sb, &c, &s, tw).line), Rect { height: 1, ..inner });
    // the body: from 2 rows under the head, scrolled
    let typing = !app.ed.text.is_empty();
    let mut lines = middle_lines(app, &c, &s, tw);
    let opts = option_lines(&s, c.id, tw, sb.card.opt, typing);
    if !opts.is_empty() {
        lines.push(Drawn::blank());
        lines.extend(opts);
    }
    let body_y = inner.y + 2.min(inner.height);
    let body_h = inner.bottom().saturating_sub(body_y) as usize;
    let (visible, max_scroll) = if lines.len() > body_h {
        let v = body_h.saturating_sub(1).max(1);
        (v, lines.len() - v)
    } else {
        (body_h, 0)
    };
    let mut scroll = app.sb.card.scroll.min(max_scroll);
    // ←→ moved the highlight: its rows come into view
    if let (true, Some(i)) = (app.sb.card.reveal, app.sb.card.opt) {
        let rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, d)| d.hits.iter().any(|h| h.2 == CardHit::Pick(c.id, i)))
            .map(|(r, _)| r)
            .collect();
        if let (Some(&first), Some(&last)) = (rows.first(), rows.last()) {
            if last >= scroll + visible {
                scroll = (last + 1 - visible).min(max_scroll);
            }
            if first < scroll {
                scroll = first;
            }
        }
    }
    for (i, d) in lines.iter().skip(scroll).take(visible).enumerate() {
        let r = Rect { y: body_y + i as u16, height: 1, ..inner };
        frame.render_widget(Paragraph::new(d.line.clone()), r);
        for &(from, hw, h) in &d.hits {
            let x = r.x + from as u16;
            if x < r.right() {
                hits.push((Rect { x, width: (hw as u16).min(r.right() - x), ..r }, h));
            }
        }
    }
    let shown = lines.len().min(visible) as u16;
    if max_scroll > 0 {
        let t = scroll_hint(scroll, max_scroll);
        let y = body_y + visible as u16;
        if y < inner.bottom() {
            let x = inner.right().saturating_sub(t.width() as u16);
            frame.render_widget(Paragraph::new(Span::styled(t, fg(theme::dim()))), Rect { x, y, width: inner.right() - x, height: 1 });
        }
    }
    // the accent bar, 2 columns with its gap, the item's height
    let bar_bottom = (body_y + shown + u16::from(max_scroll > 0)).min(card.bottom());
    let bar = lines_bar();
    let buf = frame.buffer_mut();
    for y in card.y..bar_bottom {
        buf[(card.x, y)].set_symbol(bar).set_style(fg(theme::accent()));
    }
    // BISE-290: its text selects, copies and has links
    crate::textlayer::text(area);
    let cv = &mut app.sb.card;
    cv.reveal = false;
    cv.scroll = scroll;
    cv.max_scroll = max_scroll;
    cv.page = visible.saturating_sub(1).max(1);
    cv.area = area;
    // BISE-272: the hand over what a click opens or picks
    for (r, _) in &hits {
        crate::pointer::region(*r, crate::pointer::Shape::Pointer);
    }
    cv.hits.borrow_mut().extend(hits);
}

fn lines_bar() -> &'static str {
    lines().bar
}

/// The divider's label while an item is open: `you → ? t3 · your
/// answer`.
pub(crate) fn divider_label(app: &App) -> Option<Vec<Span<'static>>> {
    if !item_open(app) {
        return None;
    }
    let c = app.sb.current_card()?;
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    Some(vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), fg(theme::dim())),
        Span::styled(format!("{} {}", kind_look(&c.kind).1, c.agent), fg(theme::accent())),
        Span::styled(" · your answer", fg(theme::dim())),
        Span::raw(" "),
    ])
}

/// The key bar while an item is open (designer's round 2): in place
/// `1-3 answer   ←→ choose   ↑↓ other items   ctrl+o full screen   esc
/// back to your message` (a hard rule `1 3 answer`; an option
/// highlighted `⏎ always allow` first); full screen `1-3 answer   ↑↓
/// other items   pgup pgdn scroll   ctrl+o back in place   esc back to
/// your message`; text typed `⏎ says no, with your note` (a question
/// `⏎ sends your answer`). No options: `⏎ got it` (done, overlap).
pub(crate) fn key_pairs(app: &App) -> Vec<(&'static str, String)> {
    let sb = &app.sb;
    let Some(c) = sb.current_card() else { return Vec::new() };
    let s = shape(c);
    let n = s.options.len();
    let more = sb.sorted_cards().len() > 1;
    let full = sb.card.full;
    let p = |k: &'static str, l: &str| (k, l.to_string());
    // an item with its own keys (the connectors key: `paste your key ·
    // ⏎ save · ctrl+x not now · esc back`)
    if !s.keys.is_empty() {
        return s.keys.iter().map(|(k, l)| p(k, l)).collect();
    }
    let mut v = Vec::new();
    let back = p("esc", "back to your message");
    if !app.ed.text.is_empty() {
        v.push(p("⏎", if s.enter == Enter::Deny { "says no, with your note" } else { "sends your answer" }));
        v.push(if full { p("ctrl+o", "back in place") } else { p("ctrl+o", "full screen") });
        v.push(back);
        return v;
    }
    match sb.card.opt.filter(|i| *i < n) {
        Some(i) => v.push(("⏎", cut(&s.options[i], 32))),
        // a hard rule's card: 1 allow, 3 no (designer)
        None if n > 0 && s.nums == [1, 3] => v.push(p("1 3", "answer")),
        None if n > 0 => v.push(p(digits(n), "answer")),
        None if s.enter == Enter::Ack => v.push(p("⏎", "got it")),
        None => {}
    }
    if n > 0 && !full {
        v.push(p("←→", "choose"));
    }
    if more {
        v.push(p("↑↓", "other items"));
    }
    if full {
        v.push(p("pgup pgdn", "scroll"));
        v.push(p("ctrl+o", "back in place"));
    } else {
        v.push(p("ctrl+o", "full screen"));
    }
    v.push(back);
    v
}

/// The pairs of an open item's key bar that fit in `width`: `ctrl+o`
/// goes first, then `←→ choose` and `pgup pgdn`, then esc says `back`
/// (80 columns: `1-3 answer   ↑↓ other items   esc back`); then the
/// others drop from the right, the first pair last.
pub(crate) fn fit_pairs(mut v: Vec<(&'static str, String)>, width: usize) -> Vec<(&'static str, String)> {
    let w = |v: &[(&'static str, String)]| {
        v.iter().map(|(k, l)| k.width() + l.width() + 1).sum::<usize>() + 3 * v.len().saturating_sub(1)
    };
    // the long form needs room to breathe: under 100 columns, the short one
    let roomy = |v: &[(&'static str, String)]| w(v) <= width && width >= 100;
    for k in ["ctrl+o", "←→", "pgup pgdn"] {
        if roomy(&v) {
            return v;
        }
        v.retain(|(key, _)| *key != k);
    }
    if w(&v) > width || width < 100 {
        for (k, l) in v.iter_mut() {
            if *k == "esc" {
                *l = "back".into();
            }
        }
    }
    // then from the right, the first pair (what answers) last
    while v.len() > 1 && w(&v) > width {
        if v.len() > 2 {
            v.remove(v.len() - 2);
        } else {
            v.pop();
        }
    }
    v
}
