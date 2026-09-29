//! The cards v2 drawn (cards.rs has the model and the keys): the strip
//! above the divider (the quick look), the card view in the history's
//! place (ctrl+g), the divider's label and the key bar of the view.

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

/// The right side of a strip row: `+ n`, the options (`1 allow  2
/// always`, or `9 options`), `×`; each piece with its hit.
fn strip_right(e: &Entry, s: &Shape, extra: usize, max: usize) -> (Row, Vec<(usize, usize, CardHit)>) {
    let id = e.first().id;
    let one = matches!(e, Entry::One(_));
    let close = if theme::ascii_mode() { "x" } else { "×" };
    let build = |opts: u8| {
        let mut r = Row::default();
        let mut hits = Vec::new();
        if extra > 0 {
            r.push(format!("+ {}  ", extra), fg(theme::faint()));
        }
        if one && opts == 2 {
            for (i, l) in s.short.iter().enumerate() {
                if i > 0 {
                    r.push("  ", Style::default());
                }
                let (a, _) = r.push(format!("{}", i + 1), fg(theme::accent()));
                let (_, b) = r.push(format!(" {}", l), fg(theme::dim()));
                hits.push((a, b, CardHit::Pick(id, i)));
            }
            r.push("  ", Style::default());
        } else if one && opts == 1 {
            r.push(format!("{} options", s.options.len()), fg(theme::accent()));
            r.push("  ", Style::default());
        }
        if one {
            let (a, b) = r.push(close, fg(theme::faint()));
            hits.push((a, b, CardHit::Close(id)));
        }
        r.push(" ", Style::default());
        (r, hits)
    };
    let full = if s.options.is_empty() { 0 } else if s.options.len() > STRIP_ROWS { 1 } else { 2 };
    for opts in (0..=full).rev() {
        let (r, h) = build(opts);
        if r.w <= max || opts == 0 {
            return (r, h);
        }
    }
    build(0)
}

/// One strip row, `w` columns: ` ? perf · the hero image…   1 compress
/// 2 both  × `; the top row on the raised tint.
fn strip_row(e: &Entry, w: usize, top: bool, extra: usize, at: Rect, hits: &mut Vec<(Rect, CardHit)>) -> Line<'static> {
    let c = e.first();
    let s = shape(c);
    let (_, glyph, _) = kind_look(&c.kind);
    let who = match e {
        Entry::One(_) => s.who.clone(),
        Entry::Group(v) => format!("{} agents want to run", v.len()),
    };
    // the left side keeps the name and a few words
    let left_min = 3 + who.width().min(16) + 3 + 8;
    let (right, rhits) = strip_right(e, &s, extra, w.saturating_sub(left_min));
    // 2 blank columns at least between the text and the options
    let room = w.saturating_sub(right.w + 2);
    let mut left = Row::default();
    left.push(" ", Style::default());
    left.push(glyph, fg(glyph_color(&c.kind)));
    left.push(" ", Style::default());
    let who_room = room.saturating_sub(left.w);
    let who = cut(&who, who_room);
    left.push(who, fg(theme::text()));
    let text_room = room.saturating_sub(left.w + 3);
    if !s.summary.is_empty() && text_room >= 4 {
        left.push(" · ", fg(theme::dim()));
        left.push(cut(&s.summary, text_room), fg(theme::dim()));
    }
    let fill = w.saturating_sub(left.w + right.w);
    let mut spans = left.spans;
    spans.push(Span::raw(" ".repeat(fill)));
    let x0 = left.w + fill;
    spans.extend(right.spans);
    hits.push((at, CardHit::Row(c.id)));
    for (a, b, h) in rhits {
        let x = at.x + (x0 + a) as u16;
        if x < at.right() {
            hits.push((Rect { x, width: ((b - a) as u16).min(at.right() - x), ..at }, h));
        }
    }
    let line = Line::from(spans);
    if top {
        line.patch_style(Style::default().bg(theme::raised()))
    } else {
        line
    }
}

/// The strip: `3 cards … ctrl+g open`, a row per card, `+ n more`; one
/// row (the top card, `+ n`) when it has one row.
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
    if area.height == 1 {
        lines.push(strip_row(&es[0], w, true, es.len() - 1, row_at(0), &mut hits));
    } else {
        let n = sb.sorted_cards().len();
        let mut lab = Row::default();
        lab.push(format!(" {} card{}", n, if n > 1 { "s" } else { "" }), fg(theme::faint()));
        let keys = "ctrl+g open ";
        let fill = w.saturating_sub(lab.w + keys.width());
        lab.push(" ".repeat(fill), Style::default());
        lab.push("ctrl+g", fg(theme::text()));
        lab.push(" open ", fg(theme::dim()));
        lines.push(Line::from(lab.spans));
        hits.push((row_at(0), CardHit::Open));
        for (i, e) in es.iter().take(STRIP_ROWS).enumerate() {
            let y = 1 + i as u16;
            if y >= area.height {
                break;
            }
            lines.push(strip_row(e, w, i == 0, 0, row_at(y), &mut hits));
        }
        let more = es.len().saturating_sub(STRIP_ROWS);
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
/// the raised tint (`[ ]` under NO_COLOR), `ctrl+n / ctrl+p` faint on
/// the right.
fn tabs_line(sb: &Sb, cur: u64, at: Rect, hits: &mut Vec<(Rect, CardHit)>) -> Line<'static> {
    let cards = sb.sorted_cards();
    let w = at.width as usize;
    let no_color = crate::theme::raised() == Color::Reset;
    let labels: Vec<(u64, &'static str, Color, String)> =
        cards.iter().map(|c| (c.id, kind_look(&c.kind).1, glyph_color(&c.kind), c.agent.clone())).collect();
    let tab_w = |l: &str| 2 + 1 + l.width() + 2;
    let hint = "ctrl+n / ctrl+p";
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
/// raised tint, the reason dim, the options one per row; each with what
/// a click on it does.
fn body_lines(s: &Shape, id: u64, w: usize) -> Vec<(Line<'static>, Option<CardHit>)> {
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
        for (i, label) in s.options.iter().enumerate() {
            let rows = wrap_line(Line::from(Span::styled(label.clone(), fg(theme::text()))), w.saturating_sub(2).max(1));
            for (j, row) in rows.into_iter().enumerate() {
                let lead = if j == 0 { Span::styled(format!("{:<2}", i + 1), fg(theme::accent())) } else { Span::raw("  ") };
                let mut v = vec![lead];
                v.extend(row.spans);
                out.push((Line::from(v), Some(CardHit::Pick(id, i))));
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
    let mut m = meta(sb, &c, pos, ids.len());
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
    let lines = body_lines(&s, c.id, tw);
    let body_y = inner.y + 2.min(inner.height);
    let body_h = inner.bottom().saturating_sub(body_y) as usize;
    let (visible, max_scroll) = if lines.len() > body_h {
        let v = body_h.saturating_sub(1).max(1);
        (v, lines.len() - v)
    } else {
        (body_h, 0)
    };
    let scroll = app.sb.card.scroll.min(max_scroll);
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
    cv.scroll = scroll;
    cv.max_scroll = max_scroll;
    cv.page = visible.saturating_sub(1).max(1);
    cv.area = area;
    cv.hits.borrow_mut().extend(hits);
}

/// The divider's label in the card view: `you → ? perf's card · your
/// answer`.
pub(crate) fn divider_label(app: &App) -> Option<Vec<Span<'static>>> {
    if !card_view_open(app) {
        return None;
    }
    let c = app.sb.current_card()?;
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    Some(vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), fg(theme::dim())),
        Span::styled(format!("{} {}'s card", kind_look(&c.kind).1, c.agent), fg(theme::accent())),
        Span::styled(" · your answer", fg(theme::dim())),
        Span::raw(" "),
    ])
}

/// The key bar of the card view: `1-2 pick · ⏎ answer · ctrl+x close ·
/// ctrl+n next · esc back` (an approval: `⏎ deny with a note`); typing,
/// no pick, and `esc back, draft kept`.
pub(crate) fn key_pairs(app: &App) -> Vec<(&'static str, &'static str)> {
    const PICK: [&str; 9] = ["1", "1-2", "1-3", "1-4", "1-5", "1-6", "1-7", "1-8", "1-9"];
    let sb = &app.sb;
    let Some(c) = sb.current_card() else { return Vec::new() };
    let s = shape(c);
    let typing = !app.ed.text.is_empty();
    let mut p = Vec::new();
    if !typing && !s.options.is_empty() {
        p.push((PICK[s.options.len().min(9) - 1], "pick"));
    }
    p.push(match s.enter {
        super::cards::Enter::Deny => ("⏎", "deny with a note"),
        super::cards::Enter::Ack if !typing => ("⏎", "got it"),
        _ => ("⏎", "answer"),
    });
    p.push(("ctrl+x", "close"));
    let ids: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
    if ids.len() > 1 {
        let last = ids.last() == Some(&c.id);
        p.push(if last { ("ctrl+p", "previous") } else { ("ctrl+n", "next") });
    }
    p.push(("esc", if typing { "back, draft kept" } else { "back" }));
    p
}
