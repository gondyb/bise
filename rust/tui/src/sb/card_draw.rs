//! The inbox drawn (cards.rs has the model and the keys): the strip
//! above the divider (the quick look, the inbox selected with ctrl+g),
//! the card view in the history's place (⏎ on a row), the divider's
//! label and the key bar of the view.

use super::cards::{cut, glyph_color, kind_look, shape, Card, CardHit, Part, Shape};
use super::*;
use crate::theme;
use unicode_width::UnicodeWidthStr;

/// The widest a card's text gets: prose wraps at 88, like the reading
/// column (book §8, §11).
const READ_WIDTH: usize = 88;

/// Below this screen height the strip is one row (the top card, `+ n`).
const SMALL_H: u16 = 24;

/// At most this many card rows in the strip, then `+ n more`.
const STRIP_ROWS: usize = 3;

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
/// together share a row): what ↑↓ walk while the inbox is selected.
pub(super) fn strip_ids(sb: &Sb) -> Vec<u64> {
    entries(sb).iter().map(|e| e.first().id).collect()
}

/// Strip row `id` is one item (not approvals that came together): its
/// digits answer it.
pub(super) fn strip_row_is_one(sb: &Sb, id: u64) -> bool {
    entries(sb).iter().any(|e| matches!(e, Entry::One(c) if c.id == id))
}

/// The key bar while the inbox is selected: `↑↓ choose · 1-2 answer ·
/// ⏎ open · esc back`; a row with no options `↑↓ choose · ⏎ open · esc
/// back to your message`.
pub(crate) fn inbox_pairs(app: &App) -> Vec<(&'static str, String)> {
    let sb = &app.sb;
    let es = entries(sb);
    let n = sb.card.inbox.and_then(|i| es.get(i.min(es.len().saturating_sub(1)))).map_or(0, |e| match e {
        Entry::One(c) => shape(c).options.len(),
        Entry::Group(_) => 0,
    });
    let p = |k: &'static str, l: &str| (k, l.to_string());
    if n > 0 {
        vec![p("↑↓", "choose"), p(digits(n), "answer"), p("⏎", "open"), p("esc", "back")]
    } else {
        vec![p("↑↓", "choose"), p("⏎", "open"), p("esc", "back to your message")]
    }
}

/// The inbox is selected (ctrl+g): the key bar and the composer know.
pub(crate) fn inbox_selected(app: &App) -> bool {
    app.sb.card.inbox.is_some() && !app.sb.card.open
}

/// NO_COLOR: no tint to raise a row, reverse video instead.
fn no_color() -> bool {
    theme::raised() == Color::Reset
}

/// The pointer before a selected row (`▸`, ASCII `>`).
fn pointer() -> &'static str {
    if theme::ascii_mode() {
        ">"
    } else {
        "▸"
    }
}

/// A selected row: raised, reversed under NO_COLOR.
fn raise(line: Line<'static>) -> Line<'static> {
    let st = if no_color() { Style::default().add_modifier(Modifier::REVERSED) } else { Style::default().bg(theme::raised()) };
    line.patch_style(st)
}

/// A new frame: the hits of the last one go (the strip or the view adds
/// its own when drawn).
pub(crate) fn card_frame(app: &App) {
    app.sb.card.hits.borrow_mut().clear();
}

/// The card view is up (the history's place is the card's).
pub(crate) fn card_view_open(app: &App) -> bool {
    app.sb.card.open && !app.sb.sorted_cards().is_empty()
}

/// The strip's height: its label row, a row per card (3 at most), `+ n
/// more`; one row under 24 rows or when `room` is short; none while the
/// view is open or no card is.
pub(crate) fn strip_height(app: &App, screen_h: u16, room: u16) -> u16 {
    let sb = &app.sb;
    if sb.card.open {
        return 0;
    }
    let n = entries(sb).len();
    if n == 0 {
        return 0;
    }
    let full = (1 + n.min(STRIP_ROWS) + usize::from(n > STRIP_ROWS)) as u16;
    if screen_h >= SMALL_H && full <= room {
        full
    } else {
        room.min(1)
    }
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

/// The right side of a strip row: `+ n`, then only on the row selected
/// (ctrl+g, `sel`) its keys: the options with their digits (`1 allow  2
/// always`, more than 3: `1-9 answer`), `⏎ open` (or the item's own
/// action, `⏎ paste it`), `×`; each piece with its hit. The other rows
/// and the strip out of the inbox show no keys: nothing but ctrl+g (and
/// the mouse) acts on the inbox.
fn strip_right(e: &Entry, s: &Shape, extra: usize, max: usize, sel: bool) -> (Row, Vec<(usize, usize, CardHit)>) {
    let id = e.first().id;
    let one = matches!(e, Entry::One(_));
    let n = if one { s.options.len() } else { 0 };
    let close = if theme::ascii_mode() { "x" } else { "×" };
    // level 3: the options whole; 2: `1-n answer`; 1: `⏎ open` and `×`;
    // 0: `+ n` only
    let build = |level: u8| {
        let mut r = Row::default();
        let mut hits = Vec::new();
        if extra > 0 {
            r.push(format!("+ {}  ", extra), fg(theme::faint()));
        }
        if sel && level >= 3 && n > 0 && n <= STRIP_ROWS && s.short.len() == n {
            for (i, l) in s.short.iter().enumerate() {
                if i > 0 {
                    r.push("  ", Style::default());
                }
                let (a, _) = r.push(format!("{}", i + 1), fg(theme::accent()));
                let (_, b) = r.push(format!(" {}", l), fg(theme::dim()));
                hits.push((a, b, CardHit::Pick(id, i)));
            }
            r.push("   ", Style::default());
        } else if sel && level >= 2 && n > 0 {
            r.push(digits(n), fg(theme::accent()));
            r.push(" answer   ", fg(theme::dim()));
        }
        if sel && level >= 1 {
            let (k, l) = match s.right {
                Some(kl) if one && n == 0 => kl,
                _ => ("⏎", "open"),
            };
            let (a, _) = r.push(k, fg(theme::text()));
            let (_, b) = r.push(format!(" {l}"), fg(theme::dim()));
            hits.push((a, b, CardHit::Row(id)));
            if one {
                r.push("  ", Style::default());
                let (a, b) = r.push(close, fg(theme::faint()));
                hits.push((a, b, CardHit::Close(id)));
            }
        }
        r.push(" ", Style::default());
        (r, hits)
    };
    for level in (1..=3).rev() {
        let (r, h) = build(level);
        if r.w <= max {
            return (r, h);
        }
    }
    build(0)
}

/// The digits that answer `n` options: `1`, `1-2` … `1-9`.
pub(super) fn digits(n: usize) -> &'static str {
    const PICK: [&str; 9] = ["1", "1-2", "1-3", "1-4", "1-5", "1-6", "1-7", "1-8", "1-9"];
    PICK[n.clamp(1, 9) - 1]
}

/// One strip row, `w` columns: ` ? perf · the hero image…   1 compress
/// 2 both  × `; `on`: raised (the top row, or the one selected). While
/// the inbox is selected (`mark`), `▸ ` before the selected row's glyph
/// and 2 spaces before the others'.
fn strip_row(
    e: &Entry,
    w: usize,
    on: bool,
    mark: bool,
    extra: usize,
    at: Rect,
    hits: &mut Vec<(Rect, CardHit)>,
) -> Line<'static> {
    let c = e.first();
    let s = shape(c);
    let (_, glyph, _) = kind_look(&c.kind);
    let who = match e {
        Entry::One(_) => s.who.clone(),
        Entry::Group(v) => format!("{} agents want to run", v.len()),
    };
    // the left side keeps the name and a few words
    let left_min = 3 + who.width().min(16) + 3 + 8;
    let (right, rhits) = strip_right(e, &s, extra, w.saturating_sub(left_min), mark && on);
    // 2 blank columns at least between the text and the options
    let room = w.saturating_sub(right.w + 2);
    let mut left = Row::default();
    left.push(" ", Style::default());
    if mark && on {
        left.push(format!("{} ", pointer()), fg(theme::accent()));
    } else if mark {
        left.push("  ", Style::default());
    }
    left.push(glyph, fg(glyph_color(&c.kind)));
    left.push(" ", Style::default());
    let who_room = room.saturating_sub(left.w);
    let who = cut(&who, who_room);
    left.push(who, fg(theme::text()));
    let text_room = room.saturating_sub(left.w + 3);
    if !s.summary.is_empty() && text_room >= 4 {
        left.push(" · ", fg(theme::dim()));
        left.push(cut(&s.summary, text_room), fg(theme::dim()));
        // its faint end, when it fits whole (`new file · 14 lines`)
        let note_room = room.saturating_sub(left.w + 1);
        if !s.note.is_empty() && s.note.width() <= note_room {
            left.push(format!(" {}", s.note), fg(theme::faint()));
        }
    }
    let fill = w.saturating_sub(left.w + right.w);
    let mut spans = left.spans;
    spans.push(Span::raw(" ".repeat(fill)));
    let x0 = left.w + fill;
    spans.extend(right.spans);
    hits.push((at, CardHit::Select(c.id)));
    for (a, b, h) in rhits {
        let x = at.x + (x0 + a) as u16;
        if x < at.right() {
            hits.push((Rect { x, width: ((b - a) as u16).min(at.right() - x), ..at }, h));
        }
    }
    let line = Line::from(spans);
    match (on, mark) {
        (true, true) => raise(line),
        // the top row out of the inbox selected: the tint only
        (true, false) => line.patch_style(Style::default().bg(theme::raised())),
        _ => line,
    }
}

/// The strip: `inbox · 3 waiting for you … ctrl+g select`, a row per
/// card, `+ n more`; one row (the top card, `+ n`) when it has one row.
/// The inbox selected: the rows scroll to keep the selected one shown.
pub(crate) fn draw_strip(app: &mut App, frame: &mut Frame, area: Rect) {
    let area = area.intersection(frame.area());
    if area.height == 0 || area.width < 10 {
        return;
    }
    let sb = &app.sb;
    let es = entries(sb);
    if es.is_empty() {
        return;
    }
    let w = area.width as usize;
    let mut hits = Vec::new();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let row_at = |i: u16| Rect { y: area.y + i, height: 1, ..area };
    let sel = sb.card.inbox.filter(|_| !sb.card.open).map(|i| i.min(es.len() - 1));
    let mark = sel.is_some();
    if area.height == 1 {
        let i = sel.unwrap_or(0);
        lines.push(strip_row(&es[i], w, true, mark, es.len() - 1, row_at(0), &mut hits));
    } else {
        let n = sb.sorted_cards().len();
        let mut lab = Row::default();
        lab.push(format!(" inbox · {} waiting for you", n), fg(theme::faint()));
        let keys = "ctrl+g select ";
        let fill = w.saturating_sub(lab.w + keys.width());
        lab.push(" ".repeat(fill), Style::default());
        lab.push("ctrl+g", fg(theme::text()));
        lab.push(" select ", fg(theme::dim()));
        lines.push(Line::from(lab.spans));
        hits.push((row_at(0), CardHit::Open));
        let rows = (area.height as usize - 1).min(STRIP_ROWS);
        let start = sel.map_or(0, |i| (i + 1).saturating_sub(rows));
        for (i, e) in es.iter().enumerate().skip(start).take(rows) {
            let y = 1 + (i - start) as u16;
            let on = sel.map_or(i == 0, |s| s == i);
            lines.push(strip_row(e, w, on, mark, 0, row_at(y), &mut hits));
        }
        let more = es.len().saturating_sub(start + STRIP_ROWS);
        let y = lines.len() as u16;
        if more > 0 && y < area.height {
            lines.push(Line::from(Span::styled(format!(" + {} more", more), fg(theme::faint()))));
            hits.push((row_at(y), CardHit::Open));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
    let cv = &mut app.sb.card;
    cv.strip = area;
    cv.hits.borrow_mut().extend(hits);
}

/// The meta line of a card: `2 of 3 · 6m · ⌥1 perf`.
fn meta(sb: &Sb, c: &Card, pos: usize, n: usize) -> Vec<String> {
    let mut m = Vec::new();
    if n > 1 {
        m.push(format!("{} of {}", pos, n));
    }
    m.push(short_age(c.age_now()));
    if let Some(k) = sb.number_of(&c.agent) {
        m.push(format!("⌥{} {}", k, c.agent));
    }
    m
}

/// A card's age, short (`12s`, `2m`, `3h`, `2d`; `now` under 10 s).
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

/// The tabs row: `? release  ? perf  ? dark-mode`, the current one on
/// the raised tint (`[ ]` under NO_COLOR), `←→` faint on the right.
fn tabs_line(sb: &Sb, cur: u64, at: Rect, hits: &mut Vec<(Rect, CardHit)>) -> Line<'static> {
    let cards = sb.sorted_cards();
    let w = at.width as usize;
    let no_color = crate::theme::raised() == Color::Reset;
    let labels: Vec<(u64, &'static str, Color, String)> =
        cards.iter().map(|c| (c.id, kind_look(&c.kind).1, glyph_color(&c.kind), shape(c).tab.unwrap_or_else(|| c.agent.clone()))).collect();
    let tab_w = |l: &str| 2 + 1 + l.width() + 2;
    let hint = if theme::ascii_mode() { "left/right" } else { "←→" };
    let with_hint = labels.len() > 1;
    let avail = |hint_on: bool| if hint_on { w.saturating_sub(hint.width() + 3) } else { w };
    let ci = labels.iter().position(|l| l.0 == cur).unwrap_or(0);
    // the first tab shown: the current one always fits
    let span_w = |from: usize, to: usize| (from..=to).map(|i| tab_w(&labels[i].3) + 1).sum::<usize>();
    let mut start = 0;
    while start < ci && span_w(start, ci) > avail(with_hint) {
        start += 1;
    }
    let mut r = Row::default();
    for (i, (id, g, gc, name)) in labels.iter().enumerate().skip(start) {
        let tw = tab_w(name);
        if r.w + tw > avail(with_hint) && i != ci {
            break;
        }
        let x = r.w;
        let on = *id == cur;
        if on {
            let bg = Style::default().bg(theme::raised());
            let (l, rr) = if no_color { ("[", "]") } else { (" ", " ") };
            r.push(l, bg.fg(theme::text()));
            r.push(*g, bg.fg(*gc));
            r.push(format!(" {}", name), bg.fg(theme::text()));
            r.push(rr, bg.fg(theme::text()));
        } else {
            r.push(" ", Style::default());
            r.push(*g, fg(*gc));
            r.push(format!(" {} ", name), fg(theme::dim()));
        }
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

/// A card's lines in the view, `w` columns: the text, the code on the
/// raised tint, the reason dim, the options one per row after a 2-column
/// gutter (`▸` on the one highlighted, `hi`, raised); each with what a
/// click on it does. `typing`: the composer answers, the options dim and
/// no highlight.
fn body_lines(s: &Shape, id: u64, w: usize, hi: Option<usize>, typing: bool) -> Vec<(Line<'static>, Option<CardHit>)> {
    let mut out: Vec<(Line<'static>, Option<CardHit>)> = Vec::new();
    let plain = |out: &mut Vec<(Line<'static>, Option<CardHit>)>, text: &str, st: Style| {
        for l in text.lines() {
            for row in wrap_line(Line::from(Span::styled(l.to_string(), st)), w) {
                out.push((row, None));
            }
        }
    };
    for (i, p) in s.parts.iter().enumerate() {
        if i > 0 {
            out.push((Line::from(""), None));
        }
        match p {
            Part::Text(t) => plain(&mut out, t.trim_end(), fg(theme::text())),
            Part::Reason(t) => plain(&mut out, t, fg(theme::dim())),
            Part::Note(t) => plain(&mut out, t, fg(theme::dim()).add_modifier(Modifier::ITALIC)),
            Part::Code(rows) => {
                let bg = Style::default().bg(theme::raised());
                for spans in rows {
                    for row in wrap_line(Line::from(spans.clone()), w.saturating_sub(2).max(1)) {
                        let used: usize = row.spans.iter().map(|s| s.content.width()).sum();
                        let mut v = vec![Span::raw(" ")];
                        v.extend(row.spans);
                        v.push(Span::raw(" ".repeat(w.saturating_sub(used + 1))));
                        out.push((Line::from(v).patch_style(bg), None));
                    }
                }
            }
        }
    }
    if !s.options.is_empty() {
        out.push((Line::from(""), None));
        let (num, text) = if typing { (theme::dim(), theme::dim()) } else { (theme::accent(), theme::text()) };
        for (i, label) in s.options.iter().enumerate() {
            let on = !typing && hi == Some(i);
            let rows = wrap_line(Line::from(Span::styled(label.clone(), fg(text))), w.saturating_sub(4).max(1));
            for (j, row) in rows.into_iter().enumerate() {
                let gutter = if on && j == 0 { format!("{} ", pointer()) } else { "  ".to_string() };
                let lead = if j == 0 { Span::styled(format!("{:<2}", i + 1), fg(num)) } else { Span::raw("  ") };
                let mut v = vec![Span::styled(gutter, fg(theme::accent())), lead];
                v.extend(row.spans);
                let mut line = Line::from(v);
                if on {
                    let used: usize = line.spans.iter().map(|s| s.content.width()).sum();
                    line.spans.push(Span::raw(" ".repeat(w.saturating_sub(used))));
                    line = raise(line);
                }
                out.push((line, Some(CardHit::Pick(id, i))));
            }
        }
    }
    out
}

/// Where the card is scrolled (none when it fits).
fn scroll_hint(scroll: usize, max_scroll: usize) -> String {
    match max_scroll.saturating_sub(scroll) {
        0 => "end · pgup".to_string(),
        1 => format!("{} 1 more line · pgdn", theme::G_OPEN),
        n => format!("{} {} more lines · pgdn", theme::G_OPEN, n),
    }
}

/// The card view in `area` (the history's place): the tabs, then the
/// card: the accent bar, `? perf needs you` bold with its meta dim on
/// the right, the whole text at the reading width, the options; it
/// scrolls when longer than the area.
pub(crate) fn draw_view(app: &mut App, frame: &mut Frame, area: Rect) {
    let area = area.intersection(frame.area());
    if area.height < 3 || area.width < 12 {
        return;
    }
    let sb = &app.sb;
    let ids: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
    let Some(c) = sb.current_card().cloned() else { return };
    let pos = ids.iter().position(|x| *x == c.id).unwrap_or(0) + 1;
    let s = shape(&c);
    let mut hits = Vec::new();
    // the tabs, 1 blank row under them when there is room
    let tabs = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(tabs_line(sb, c.id, tabs, &mut hits)), tabs);
    let gap = u16::from(area.height >= 8);
    let card = Rect { y: area.y + 1 + gap, height: area.height - 1 - gap, ..area };
    let tw = ((card.width as usize).saturating_sub(2)).min(READ_WIDTH);
    let inner = Rect { x: card.x + 2, width: tw as u16, ..card };
    // the title row: glyph and title bold in the kind's color, meta dim
    let (_, glyph, color) = kind_look(&c.kind);
    let bold = fg(color).add_modifier(Modifier::BOLD);
    let mut m = match &s.meta {
        Some(own) => vec![own.clone()],
        None => meta(sb, &c, pos, ids.len()),
    };
    let head_w = glyph.width() + 1 + s.title.width();
    let meta_s = loop {
        let t = m.join(" · ");
        if m.is_empty() || head_w + 2 + t.width() <= tw {
            break t;
        }
        m.pop();
    };
    let title = cut(&s.title, tw.saturating_sub(glyph.width() + 1 + if meta_s.is_empty() { 0 } else { meta_s.width() + 2 }));
    let fill = tw.saturating_sub(glyph.width() + 1 + title.width() + meta_s.width());
    let title_line = Line::from(vec![
        Span::styled(glyph, fg(glyph_color(&c.kind)).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(title, bold),
        Span::raw(" ".repeat(fill)),
        Span::styled(meta_s, fg(theme::dim())),
    ]);
    frame.render_widget(Paragraph::new(title_line), Rect { height: 1, ..inner });
    // the body: from 2 rows under the title, scrolled
    let typing = !app.ed.text.is_empty();
    let lines = body_lines(&s, c.id, tw, sb.card.opt, typing);
    let body_y = inner.y + 2.min(inner.height);
    let body_h = inner.bottom().saturating_sub(body_y) as usize;
    let (visible, max_scroll) = if lines.len() > body_h {
        let v = body_h.saturating_sub(1).max(1);
        (v, lines.len() - v)
    } else {
        (body_h, 0)
    };
    let mut scroll = app.sb.card.scroll.min(max_scroll);
    // ↑↓ moved the highlight: its rows come into view
    if let (true, Some(i)) = (app.sb.card.reveal, app.sb.card.opt) {
        let rows: Vec<usize> =
            lines.iter().enumerate().filter(|(_, (_, h))| *h == Some(CardHit::Pick(c.id, i))).map(|(r, _)| r).collect();
        if let (Some(&first), Some(&last)) = (rows.first(), rows.last()) {
            if last >= scroll + visible {
                scroll = (last + 1 - visible).min(max_scroll);
            }
            if first < scroll {
                scroll = first;
            }
        }
    }
    for (i, (line, hit)) in lines.iter().skip(scroll).take(visible).enumerate() {
        let r = Rect { y: body_y + i as u16, height: 1, ..inner };
        frame.render_widget(Paragraph::new(line.clone()), r);
        if let Some(h) = hit {
            hits.push((r, *h));
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
    // the accent bar, 2 columns with its gap, the card's height
    let bar_bottom = (body_y + shown + u16::from(max_scroll > 0)).min(card.bottom());
    let buf = frame.buffer_mut();
    for y in card.y..bar_bottom {
        buf[(card.x, y)].set_symbol("┃").set_style(fg(theme::accent()));
    }
    let cv = &mut app.sb.card;
    cv.reveal = false;
    cv.scroll = scroll;
    cv.max_scroll = max_scroll;
    cv.page = visible.saturating_sub(1).max(1);
    cv.area = area;
    cv.hits.borrow_mut().extend(hits);
}

/// The divider's label in the card view: `you → ? perf · your answer`.
pub(crate) fn divider_label(app: &App) -> Option<Vec<Span<'static>>> {
    if !card_view_open(app) {
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

/// The key bar of the card view (book §12): nothing highlighted `↑↓
/// choose · 1-2 pick · ←→ other cards · type to answer in your words ·
/// esc back` (an approval: `type a note to deny`); an option highlighted
/// `↑↓ choose · ⏎ pick “both” · ←→ other cards · esc back`; text typed
/// `⏎ send as your answer · ctrl+n next card · esc back, draft kept`.
/// No options: `⏎ got it` (done, overlap) or `type to answer`.
pub(crate) fn key_pairs(app: &App) -> Vec<(&'static str, String)> {
    use super::cards::Enter;
    let sb = &app.sb;
    let Some(c) = sb.current_card() else { return Vec::new() };
    let s = shape(c);
    let n = s.options.len();
    let more = sb.sorted_cards().len() > 1;
    let p = |k: &'static str, l: &str| (k, l.to_string());
    // an item with its own keys (the connectors key: `paste your key ·
    // ⏎ save · ctrl+x not now · esc back`)
    if !s.keys.is_empty() {
        return s.keys.iter().map(|(k, l)| p(k, l)).collect();
    }
    let mut v = Vec::new();
    if !app.ed.text.is_empty() {
        v.push(p("⏎", if s.enter == Enter::Deny { "deny with your note" } else { "send as your answer" }));
        if more {
            let last = sb.sorted_cards().last().map(|l| l.id) == Some(c.id);
            v.push(if last { p("ctrl+p", "previous item") } else { p("ctrl+n", "next item") });
        }
        v.push(p("esc", "back, draft kept"));
        return v;
    }
    if n > 0 {
        v.push(p("↑↓", "choose"));
    }
    match sb.card.opt.filter(|i| *i < n) {
        Some(i) => {
            let (l, r) = if theme::ascii_mode() { ("\"", "\"") } else { ("“", "”") };
            v.push(("⏎", format!("pick {l}{}{r}", cut(&s.options[i], 32))));
        }
        None if n > 0 => v.push(p(digits(n), "pick")),
        None if s.enter == Enter::Ack => v.push(p("⏎", "got it")),
        None => {}
    }
    let typed = match s.enter {
        Enter::Deny if sb.card.opt.is_none() => Some(p("type", "a note to deny")),
        Enter::Answer if sb.card.opt.is_none() && s.words => Some(p("type", if n > 0 { "to answer in your words" } else { "to answer" })),
        _ => None,
    };
    // an approval says its note before the other cards
    if s.enter == Enter::Deny {
        v.extend(typed.clone());
    }
    if more {
        v.push(p("←→", "other items"));
    }
    if s.enter != Enter::Deny {
        v.extend(typed);
    }
    v.push(p("esc", "back"));
    v
}

/// The pairs of the card view's key bar that fit in `width`: the others
/// drop from the right, `↑↓ choose` and `⏎ …` last.
pub(crate) fn fit_pairs(mut v: Vec<(&'static str, String)>, width: usize) -> Vec<(&'static str, String)> {
    let w = |v: &[(&'static str, String)]| {
        v.iter().map(|(k, l)| k.width() + l.width() + 1).sum::<usize>() + 3 * v.len().saturating_sub(1)
    };
    while v.len() > 1 && w(&v) > width {
        match v.iter().rposition(|(k, _)| *k != "↑↓" && *k != "⏎") {
            Some(i) => v.remove(i),
            None => v.pop().unwrap_or(("", String::new())),
        };
    }
    v
}
