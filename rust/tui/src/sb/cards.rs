//! The attention cards: the hub's questions and alerts, answered from
//! the card box above the composer (ctrl+g, alt+r). A card is level 1
//! (bise book §9, §12): a bar on the left, `? {name} needs you`, the
//! question wrapped at 76, the choices, the keys dim.

use super::*;
use crate::commands::PopItem;
use crate::theme;
use ratatui::layout::Margin;
use ratatui::symbols::border;
use ratatui::widgets::{Clear, Padding, Scrollbar, ScrollbarOrientation, ScrollbarState};

#[derive(Clone)]
pub(crate) struct Card {
    pub(super) id: u64,
    pub(super) kind: String,
    pub(super) agent: String,
    pub(super) text: String,
    /// The card's age when the snapshot arrived, and when it arrived.
    pub(super) age_ms: u64,
    pub(super) seen_at: std::time::Instant,
    /// The hub's remark (the asker heard from main since...).
    pub(super) note: String,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            id: 0,
            kind: String::new(),
            agent: String::new(),
            text: String::new(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
        }
    }
}

/// What the user sees of the attention cards: which one, shown or not,
/// full screen or not, and how far it is scrolled.
#[derive(Default)]
pub(super) struct CardView {
    pub(super) shown: bool,
    pub(super) full: bool,
    pub(super) sel: Option<u64>,
    pub(super) scroll: usize,
    /// Set by the last draw: the last scroll offset and the page size.
    pub(super) max_scroll: usize,
    pub(super) page: usize,
    /// Set by the last draw: where the box is (the mouse wheel over it
    /// scrolls it).
    pub(super) area: Rect,
}

/// The widest a card line gets: prose wraps at 76 (book §11).
const READ_WIDTH: usize = 76;

/// The level-1 box: a heavy bar on the left (book §9 `┃`), light and
/// rounded elsewhere.
const LEVEL1: border::Set = border::Set {
    top_left: "┎",
    top_right: "╮",
    bottom_left: "┖",
    bottom_right: "╯",
    vertical_left: "┃",
    vertical_right: "│",
    horizontal_top: "─",
    horizontal_bottom: "─",
};

impl CardView {
    /// Scroll by `d` rows (negative: up), within the text.
    pub(super) fn scroll_by(&mut self, d: isize) {
        self.scroll = if d < 0 {
            self.scroll.saturating_sub(d.unsigned_abs())
        } else {
            self.scroll.saturating_add(d.unsigned_abs()).min(self.max_scroll)
        };
    }
}

impl Sb {
    /// The cards in reading order: what blocks a task first, then the
    /// oldest.
    fn sorted_cards(&self) -> Vec<&Card> {
        let mut v: Vec<&Card> = self.cards.iter().collect();
        v.sort_by_key(|c| (kind_look(&c.kind).0, c.id));
        v
    }

    /// The card shown (or answered by Alt+R): the chosen one while it
    /// is open, else the focused task's, else the first.
    pub(super) fn current_card(&self) -> Option<&Card> {
        let v = self.sorted_cards();
        self.card
            .sel
            .and_then(|id| v.iter().find(|c| c.id == id).copied())
            .or_else(|| v.iter().find(|c| c.agent == self.focus).copied())
            .or_else(|| v.first().copied())
    }

    pub(super) fn toggle_card(&mut self) {
        self.card.shown = !self.card.shown;
        self.card.full = false;
        if self.card.shown {
            self.card.sel = self.current_card().map(|c| c.id);
            self.card.scroll = 0;
        }
    }

    /// Ctrl+N / Ctrl+P: the next or previous card, shown.
    pub(super) fn step_card(&mut self, d: isize) {
        let ids: Vec<u64> = self.sorted_cards().iter().map(|c| c.id).collect();
        if ids.is_empty() {
            return;
        }
        let cur = self.current_card().map(|c| c.id);
        let i = cur.and_then(|id| ids.iter().position(|x| *x == id)).unwrap_or(0) as isize;
        let n = ids.len() as isize;
        let j = if self.card.shown { (i + d).rem_euclid(n) } else { i };
        self.card.sel = Some(ids[j as usize]);
        self.card.shown = true;
        self.card.scroll = 0;
    }
}

/// A card kind: its rank in reading order (what blocks an agent first),
/// its glyph and its color. Color means attention (book §5): needs you
/// in accent, failures in error, the rest plain text.
fn kind_look(kind: &str) -> (u8, &'static str, Color) {
    match kind {
        "question" => (0, theme::G_NEEDS_YOU, theme::accent()),
        "blocked" => (1, theme::G_NEEDS_YOU, theme::accent()),
        "failed" => (2, theme::G_FAILED, theme::error()),
        "restart" => (2, theme::G_RESTART_FAILED, theme::error()),
        "drop" => (3, theme::G_STOPPED, theme::text()),
        "overlap" => (4, theme::G_OVERLAP, theme::text()),
        "done" => (5, theme::done_glyph(), theme::text()),
        _ => (5, theme::G_CARD, theme::accent()),
    }
}

/// The color of a card kind's glyph: its hue, except done's check, in
/// accent on a plain title (BISE-100, book §6).
fn glyph_color(kind: &str) -> Color {
    match kind {
        "done" => theme::accent(),
        k => kind_look(k).2,
    }
}

/// The border of a card's box: the kind's hue when it has one, else
/// faint (a done card does not call for attention).
fn border_color(kind: &str) -> Color {
    match kind {
        "drop" | "overlap" | "done" => theme::faint(),
        k => kind_look(k).2,
    }
}

/// A card's title, after its glyph (copy deck §17: `{name} needs you`).
fn kind_title(kind: &str, agent: &str) -> String {
    match kind {
        "question" => format!("{agent} needs you"),
        "blocked" => format!("{agent} is blocked"),
        "failed" => format!("{agent} failed"),
        "restart" => "restart failed".into(),
        "drop" => format!("drop {agent}?"),
        "overlap" => "overlap".into(),
        "done" => format!("{agent} is done"),
        k => format!("{agent}: {k}"),
    }
}

/// The cards answered with no words: `alt+r` on an empty composer.
fn no_words(kind: &str) -> bool {
    matches!(kind, "done" | "overlap")
}

/// The choices an agent gives at the end of its question: its last
/// lines `1. v1` / `1) v1` / `1 - v1`, numbered 1, 2, … (two to nine).
/// Returns the text without them, and the choices (none: the text as is).
fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return (text.to_string(), Vec::new());
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}

/// Alt+R: the composer's text answers the current card, shown or not
/// (Enter still talks to the agent in focus). An empty composer only
/// acknowledges the cards that need no words (done, overlap).
pub(super) fn answer_card(app: &mut App) {
    let text = app.ed.text.trim().to_string();
    let Some(sb) = app.sb.as_mut() else { return };
    let Some((id, kind, agent)) = sb
        .current_card()
        .map(|c| (c.id, c.kind.clone(), c.agent.clone()))
    else {
        return;
    };
    let text = if text.is_empty() {
        if !no_words(&kind) {
            let msg = format!("card #{} ({}): type your answer, then alt+r", id, agent);
            push_event(&mut app.events, &mut app.cache, Ev::Warn(msg));
            return;
        }
        "seen".to_string()
    } else {
        text
    };
    sb.send_input(format!("/answer {} {}", id, text));
    sb.card.scroll = 0;
    sb.card.full = false;
    if !app.ed.text.trim().is_empty() {
        app.history.insert(0, app.ed.text.clone());
    }
    app.ed.take();
}

/// The height of the card box above the composer (0: hidden, or full
/// screen over the feed instead): its whole text when it fits, else up
/// to 70% of the screen, within `room` (what the composer and the
/// fixed rows leave).
pub(crate) fn card_box_height(app: &App, area: Rect, room: u16) -> u16 {
    let Some(sb) = app.sb.as_ref() else { return 0 };
    if !sb.card.shown || sb.card.full {
        return 0;
    }
    let Some(c) = sb.current_card() else { return 0 };
    let rows = card_lines(c, text_width(area.width)).len().saturating_add(2);
    let rows = u16::try_from(rows).unwrap_or(u16::MAX);
    let cap = (area.height / 10 * 7 + area.height % 10 * 7 / 10).min(room).max(3);
    rows.min(cap)
}

/// The wrap width of a card's text in a box `width` columns wide (the
/// borders and the padding take 4), at most 76.
fn text_width(width: u16) -> usize {
    (width as usize).saturating_sub(4).clamp(1, READ_WIDTH)
}

pub(crate) fn card_full(app: &App) -> bool {
    app.sb
        .as_ref()
        .is_some_and(|sb| sb.card.shown && sb.card.full && !sb.cards.is_empty())
}

fn card_lines(c: &Card, width: usize) -> Vec<Line<'static>> {
    let (body, choices) = if no_words(&c.kind) {
        (c.text.clone(), Vec::new())
    } else {
        split_choices(&c.text)
    };
    let mut out: Vec<Line<'static>> = Vec::new();
    for l in body.lines() {
        out.extend(wrap_line(
            Line::from(Span::styled(l.to_string(), Style::default().fg(theme::text()))),
            width,
        ));
    }
    if !c.note.is_empty() {
        out.push(Line::from(""));
        out.extend(wrap_line(
            Line::from(Span::styled(
                c.note.clone(),
                Style::default().fg(theme::dim()).add_modifier(Modifier::ITALIC),
            )),
            width,
        ));
    }
    if !choices.is_empty() {
        // `1 v1   2 v2`: the number in accent (type it, then alt+r)
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, label) in choices.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("   "));
            }
            spans.push(Span::styled(
                format!("{}", i + 1),
                Style::default().fg(theme::accent()).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(format!(" {}", label), Style::default().fg(theme::text())));
        }
        out.push(Line::from(""));
        out.extend(wrap_line(Line::from(spans), width));
    }
    out
}

/// The panel's age of a report (`12 s`, `3 min`, `2 h`); the panel
/// stops using it with BISE-20 (its own `short_age`).
#[allow(dead_code)]
pub(super) fn ago(ms: u64) -> String {
    let s = ms / 1000;
    if s < 60 {
        format!("{} s", s)
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else {
        format!("{} h", s / 3600)
    }
}

/// A card's age, short (`12s`, `2m`, `3h`, `2d`).
fn short_age(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{}s", s),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// The keys of the card box (copy deck §17), most useful first; keeps
/// those that fit in `width` columns.
fn keys_hint(kind: &str, many: bool, full: bool, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let mut parts: Vec<&str> = vec![
        if no_words(kind) { "alt+r got it" } else { "alt+r answer with text" },
        "ctrl+x later",
    ];
    if !full {
        parts.push("ctrl+f full screen");
    }
    if many {
        parts.push("ctrl+n next");
    }
    let mut out = String::new();
    for p in parts {
        let next = if out.is_empty() { p.to_string() } else { format!("{} · {}", out, p) };
        if next.width() + 2 > width {
            break;
        }
        out = next;
    }
    if out.is_empty() {
        out
    } else {
        format!(" {} ", out)
    }
}

/// Where the text is scrolled (none when it fits): the lines below the
/// view (`▾ 12 more lines · pgdn`), or the way back up at the end.
fn scroll_hint(scroll: usize, max_scroll: usize) -> String {
    if max_scroll == 0 {
        return String::new();
    }
    let left = max_scroll.saturating_sub(scroll);
    match left {
        0 => " end · pgup ".to_string(),
        1 => format!(" {} 1 more line · pgdn ", theme::G_OPEN),
        n => format!(" {} {} more lines · pgdn ", theme::G_OPEN, n),
    }
}

/// The scroll hint when the keys leave little room: ` ▾ 12 ` / ` end `.
fn scroll_hint_short(scroll: usize, max_scroll: usize) -> String {
    match max_scroll.saturating_sub(scroll) {
        _ if max_scroll == 0 => String::new(),
        0 => " end ".to_string(),
        n => format!(" {} {} ", theme::G_OPEN, n),
    }
}

/// The title: the glyph and `{name} needs you` bold in the kind's
/// color, then dim where it is (`1 of 3`, its age, or `full screen ·
/// ctrl+f back`); what does not fit in `width` is cut.
fn title_line(c: &Card, pos: usize, count: usize, full: bool, width: usize) -> Line<'static> {
    use unicode_width::UnicodeWidthStr;
    let (_, glyph, color) = kind_look(&c.kind);
    let mark = Span::styled(format!(" {} ", glyph), Style::default().fg(glyph_color(&c.kind)).add_modifier(Modifier::BOLD));
    let head = kind_title(&c.kind, &c.agent);
    let mut meta: Vec<String> = Vec::new();
    if count > 1 {
        meta.push(format!("{} of {}", pos, count));
    }
    if full {
        meta.push("full screen · ctrl+f back".into());
    } else {
        meta.push(short_age(c.age_ms.saturating_add(c.seen_at.elapsed().as_millis() as u64)));
    }
    let meta = format!(" · {} ", meta.join(" · "));
    let bold = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let mark_w = mark.content.width();
    if mark_w + head.width() + meta.width() <= width {
        Line::from(vec![mark, Span::styled(head, bold), Span::styled(meta, Style::default().fg(theme::dim()))])
    } else {
        Line::from(vec![
            mark,
            Span::styled(format!("{} ", truncate_chars(&head, width.saturating_sub(2 + mark_w))), bold),
        ])
    }
}

/// The card box: the whole text, wrapped, scrolled by PgUp/PgDn, the
/// arrows (empty composer) or the mouse wheel.
pub(crate) fn draw_card(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(sb) = app.sb.as_mut() else { return };
    // never outside the frame (ratatui panics outside its buffer)
    let area = area.intersection(frame.area());
    if area.height < 3 || area.width < 5 {
        return;
    }
    let order: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
    let Some(c) = sb.current_card() else { return };
    let lines = card_lines(c, text_width(area.width));
    let visible = area.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    let pos = order.iter().position(|x| *x == c.id).unwrap_or(0) + 1;
    let color = border_color(&c.kind);
    let inner_w = (area.width as usize).saturating_sub(2);
    let title = title_line(c, pos, order.len(), sb.card.full, inner_w);
    let scroll = sb.card.scroll.min(max_scroll);
    // the keys first (copy deck), then where the text is if it fits
    // a scrolled card keeps room for its short scroll hint (` ▾ 12 `):
    // in the reading column (≤ 79, BISE-97) all the keys would take it
    let keep = {
        use unicode_width::UnicodeWidthStr;
        scroll_hint_short(scroll, max_scroll).width()
    };
    let keys = keys_hint(&c.kind, order.len() > 1, sb.card.full, inner_w.saturating_sub(keep + usize::from(keep > 0)));
    let hint = {
        use unicode_width::UnicodeWidthStr;
        let room = inner_w.saturating_sub(keys.width() + 1);
        let long = scroll_hint(scroll, max_scroll);
        let short = scroll_hint_short(scroll, max_scroll);
        if long.width() <= room {
            long
        } else if short.width() <= room {
            short
        } else {
            String::new()
        }
    };
    sb.card.scroll = scroll;
    sb.card.max_scroll = max_scroll;
    sb.card.page = visible.saturating_sub(1).max(1);
    sb.card.area = area;
    let total = lines.len();
    let dim = Style::default().fg(theme::dim());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(LEVEL1)
        .border_style(Style::default().fg(color))
        .title(title)
        .title_bottom(Line::from(Span::styled(keys, dim)).left_aligned())
        .title_bottom(Line::from(Span::styled(hint, dim)).right_aligned())
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll as u16, 0)), area);
    if max_scroll > 0 {
        // on the right border: how much there is, and where
        let mut state = ScrollbarState::new(max_scroll + 1)
            .position(scroll)
            .viewport_content_length(visible.min(total));
        let bar = area.inner(Margin { vertical: 1, horizontal: 0 });
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::default().fg(color)),
            bar,
            &mut state,
        );
    }
}

/// The mouse wheel over the card box (or anywhere while it is full
/// screen) scrolls it; `true` when handled.
pub(crate) fn card_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let Some(sb) = app.sb.as_mut() else { return false };
    if !sb.card.shown || sb.cards.is_empty() {
        return false;
    }
    let a = sb.card.area;
    let over = m.column >= a.x
        && m.column < a.x.saturating_add(a.width)
        && m.row >= a.y
        && m.row < a.y.saturating_add(a.height);
    if !over && !sb.card.full {
        return false;
    }
    match m.kind {
        MouseEventKind::ScrollUp => sb.card.scroll_by(-3),
        MouseEventKind::ScrollDown => sb.card.scroll_by(3),
        _ => return false,
    }
    true
}

/// `/close <query>`: the open cards whose id, kind, agent or text match;
/// picked, the composer holds `/close <id> ` (Enter sends it).
pub(crate) fn close_items(app: &App) -> Vec<PopItem> {
    let Some(sb) = app.sb.as_ref() else { return Vec::new() };
    if !crate::commands::popup_open(app) {
        return Vec::new();
    }
    let Some(q) = app.ed.text.strip_prefix("/close ") else {
        return Vec::new();
    };
    // the id is typed (a space after it: the note follows)
    if q.contains(' ') || q.contains('\n') {
        return Vec::new();
    }
    let q = q.to_lowercase();
    sb.sorted_cards()
        .into_iter()
        .filter(|c| {
            q.is_empty()
                || c.id.to_string().starts_with(&q)
                || c.kind.to_lowercase().contains(&q)
                || c.agent.to_lowercase().contains(&q)
                || c.text.to_lowercase().contains(&q)
        })
        .map(|c| {
            let (_, icon, _) = kind_look(&c.kind);
            let color = glyph_color(&c.kind);
            let fill = format!("/close {} ", c.id);
            PopItem {
                label: format!("#{}", c.id),
                desc: format!("{} @{} · {}", c.kind, c.agent, truncate_chars(&one_line(&c.text), 80)),
                mark: Some((icon, color)),
                fill_cursor: fill.chars().count(),
                fill,
                run: None,
                closable: true,
                path: None,
                folder: false,
            }
        })
        .collect()
}

/// The text on one line: runs of whitespace become one space.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn card(id: u64, kind: &str, text: String) -> Card {
        Card {
            id,
            kind: kind.into(),
            agent: "t1".into(),
            text,
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
        }
    }

    fn long_text() -> String {
        (1..=70).map(|i| format!("line {:02} of the card", i)).collect::<Vec<_>>().join("\n")
    }

    fn screen(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let w = buf.area.width as usize;
        buf.content
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn press(app: &mut App, code: KeyCode) -> bool {
        key(app, &KeyEvent::new(code, KeyModifiers::NONE), false)
    }

    /// A 70-line card: the box takes most of the screen, the footer says
    /// how to scroll, and PgDn, ↓ and the wheel reach every line.
    #[test]
    fn a_long_card_is_big_and_fully_scrollable() {
        let mut app = bench::test_app();
        let (w, h) = (140u16, 40u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.sb.as_mut().unwrap().cards = vec![card(3, "question", long_text()), card(4, "done", "ok".into())];
        app.sb.as_mut().unwrap().toggle_card();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        let s = screen(&term);
        let area = app.sb.as_ref().unwrap().card.area;
        assert!(area.height >= h * 6 / 10, "box height {} of {}", area.height, h);
        assert!(s.contains("line 01 of the card"));
        // the box takes the reading column (≤ 79, book §8): with every key
        // shown, the scroll hint may be its short form (`▾ 58`)
        assert!(s.contains("more lines · pgdn") || s.contains(&format!(" {} ", crate::theme::G_OPEN)), "a scroll hint in
{}", s);
        // the most useful keys first; the last ones give way to the
        // scroll hint when the column is narrow
        for k in [
            "alt+r answer with text",
            "ctrl+x later",
            "? t1 needs you · 1 of 2",
        ] {
            assert!(s.contains(k), "{} in\n{}", k, s);
        }
        // every line shows up while scrolling to the end
        let mut seen = std::collections::BTreeSet::new();
        for step in 0..200 {
            term.draw(|f| draw_sb(&mut app, f)).unwrap();
            let s = screen(&term);
            for i in 1..=70 {
                if s.contains(&format!("line {:02} of the card", i)) {
                    seen.insert(i);
                }
            }
            let sb = app.sb.as_ref().unwrap();
            if sb.card.scroll == sb.card.max_scroll && step > 0 {
                break;
            }
            match step % 3 {
                0 => assert!(press(&mut app, KeyCode::PageDown)),
                1 => assert!(press(&mut app, KeyCode::Down)),
                _ => assert!(card_mouse(
                    &mut app,
                    &MouseEvent {
                        kind: MouseEventKind::ScrollDown,
                        column: area.x + 2,
                        row: area.y + 2,
                        modifiers: KeyModifiers::NONE,
                    }
                )),
            }
        }
        assert_eq!(seen.len(), 70, "missing lines: {:?}", seen);
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        assert!(screen(&term).contains("end · pgup"));
        // back up with ↑ and PgUp; Esc hides the box
        assert!(press(&mut app, KeyCode::PageUp));
        assert!(press(&mut app, KeyCode::Up));
        assert!(press(&mut app, KeyCode::Esc));
        assert!(!app.sb.as_ref().unwrap().card.shown);
    }

    /// The box never hides the composer, even on a short screen, and
    /// never draws outside the frame.
    #[test]
    fn the_box_leaves_the_composer_on_small_screens() {
        for (w, h) in [(20u16, 8u16), (40, 12), (80, 24), (200, 60), (4, 3)] {
            let mut app = bench::test_app();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            app.sb.as_mut().unwrap().cards = vec![card(1, "blocked", long_text())];
            app.sb.as_mut().unwrap().toggle_card();
            term.draw(|f| draw_sb(&mut app, f)).unwrap();
            let a = app.sb.as_ref().unwrap().card.area;
            // at least the status row and one composer row stay under it
            assert!(a.y.saturating_add(a.height) <= h.saturating_sub(2) || a.height == 0, "{}x{}: {:?}", w, h, a);
            app.sb.as_mut().unwrap().card.full = true;
            term.draw(|f| draw_sb(&mut app, f)).unwrap();
        }
    }

    /// A short card: the box fits its text, the lines keep a readable width.
    #[test]
    fn a_short_card_fits_and_wraps_at_a_readable_width() {
        let mut app = bench::test_app();
        let long_line = "word ".repeat(60);
        app.sb.as_mut().unwrap().cards = vec![card(1, "done", long_line)];
        app.sb.as_mut().unwrap().toggle_card();
        let h = card_box_height(&app, Rect::new(0, 0, 200, 50), 40);
        // 300 chars wrapped at 76 columns: 4 rows, plus the borders
        assert_eq!(h, 6, "height {}", h);
    }

    /// Draws `cards` with the box shown (the first one in reading order)
    /// on a 100x30 screen; returns the screen and the box's area.
    fn draw_cards(app: &mut App, cards: Vec<Card>) -> (String, Rect, Terminal<TestBackend>) {
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        app.sb.as_mut().unwrap().cards = cards;
        app.sb.as_mut().unwrap().toggle_card();
        term.draw(|f| draw_sb(app, f)).unwrap();
        let s = screen(&term);
        let a = app.sb.as_ref().unwrap().card.area;
        (s, a, term)
    }

    /// The box's rows, as text.
    fn box_rows(s: &str, a: Rect) -> Vec<String> {
        s.lines()
            .skip(a.y as usize)
            .take(a.height as usize)
            .map(|l| l.chars().skip(a.x as usize).take(a.width as usize).collect())
            .collect()
    }

    /// "cards: a question": level 1 (accent bar on the left, bold accent
    /// `? docs needs you`), the question, the choices, the keys dim.
    #[test]
    fn a_question_is_level_one_with_its_choices_and_keys() {
        let mut app = bench::test_app();
        let mut q = card(
            1,
            "question",
            "the brief says \"keep old clients working\", but v2 removes /users. do we document v1 or v2?\n1. v1\n2. v2".into(),
        );
        q.agent = "docs".into();
        q.age_ms = 120_000;
        let (s, a, term) = draw_cards(&mut app, vec![q]);
        let rows = box_rows(&s, a);
        assert!(rows[0].starts_with("┎ ? docs needs you · 2m "), "{}", rows[0]);
        assert!(rows[1].starts_with("┃ the brief says"), "{}", rows[1]);
        assert!(rows.iter().any(|r| r.contains("1 v1   2 v2")), "{}", s);
        assert!(!rows.iter().any(|r| r.contains("1. v1")), "the choices are not in the text: {}", s);
        let last = rows.last().unwrap();
        assert!(
            last.contains("alt+r answer with text · ctrl+x later · ctrl+f full screen"),
            "{}",
            last
        );
        assert!(last.starts_with("┖"), "{}", last);
        // every text row within 76 columns (the box is 100 wide)
        for r in &rows[1..rows.len() - 1] {
            let text = r.trim_start_matches('┃').trim_end_matches('│').trim_end();
            assert!(text.chars().count() <= 77, "{:?}", r);
        }
        // colors: bar, glyph and title in accent; the keys dim; the choice number accent
        let buf = term.backend().buffer();
        let cell = |x: u16, y: u16| buf[(x, y)].clone();
        assert_eq!(cell(a.x, a.y + 1).fg, theme::accent(), "bar");
        assert_eq!(cell(a.x + 2, a.y).fg, theme::accent(), "glyph");
        assert!(cell(a.x + 4, a.y).modifier.contains(Modifier::BOLD), "title bold");
        assert_eq!(cell(a.x + 2, a.y + a.height - 1).fg, theme::dim(), "keys");
        let crow = rows.iter().position(|r| r.contains("1 v1")).unwrap() as u16;
        let cx = rows[crow as usize].chars().position(|c| c == '1').unwrap() as u16;
        assert_eq!(cell(a.x + cx, a.y + crow).fg, theme::accent(), "choice number");
    }

    /// "cards: every kind": one glyph and one title per kind, the hue
    /// only for attention (needs you, failures), sorted by what blocks an
    /// agent first.
    #[test]
    fn every_kind_has_its_glyph_title_and_color() {
        let cases = [
            ("question", "? t1 needs you", Some(theme::accent())),
            ("blocked", "? t1 is blocked", Some(theme::accent())),
            ("failed", "✗ t1 failed", Some(theme::error())),
            ("restart", "↻ restart failed", Some(theme::error())),
            ("drop", "– drop t1?", None),
            ("overlap", "⇄ overlap", None),
            ("done", "✓ t1 is done", None),
        ];
        for (kind, title, hue) in cases {
            let mut app = bench::test_app();
            let (s, a, term) = draw_cards(&mut app, vec![card(1, kind, "some text".into())]);
            let rows = box_rows(&s, a);
            assert!(rows[0].starts_with(&format!("┎ {} · ", title)), "{}: {}", kind, rows[0]);
            let buf = term.backend().buffer();
            let border = buf[(a.x, a.y + 1)].fg;
            let glyph = buf[(a.x + 2, a.y)].fg;
            match hue {
                Some(h) => {
                    assert_eq!(border, h, "{} border", kind);
                    assert_eq!(glyph, h, "{} glyph", kind);
                }
                None => {
                    assert_eq!(border, theme::faint(), "{} border", kind);
                    // done's check is accent on a plain title (BISE-100)
                    let g = if kind == "done" { theme::accent() } else { theme::text() };
                    assert_eq!(glyph, g, "{} glyph", kind);
                }
            }
            let answer = if matches!(kind, "done" | "overlap") { "alt+r got it" } else { "alt+r answer with text" };
            assert!(rows.last().unwrap().contains(answer), "{}: {}", kind, s);
            // the /close list uses the same glyph
            let mark = kind_look(kind).1;
            assert_eq!(mark, title.split(' ').next().unwrap(), "{}", kind);
        }
        // reading order: what blocks first, then the oldest
        let mut app = bench::test_app();
        let kinds = ["done", "overlap", "drop", "restart", "failed", "blocked", "question"];
        app.sb.as_mut().unwrap().cards =
            kinds.iter().enumerate().map(|(i, k)| card(i as u64 + 1, k, "x".into())).collect();
        let order: Vec<String> =
            app.sb.as_ref().unwrap().sorted_cards().iter().map(|c| c.kind.clone()).collect();
        assert_eq!(order, vec!["question", "blocked", "restart", "failed", "drop", "overlap", "done"]);
    }

    /// "a card, full screen": ctrl+f opens it over the feed (title
    /// `full screen · ctrl+f back`, still wrapped at 76, pgdn scrolls);
    /// ctrl+f or esc brings the box back.
    #[test]
    fn ctrl_f_opens_the_card_full_screen_and_back() {
        let mut app = bench::test_app();
        let mut c = card(5, "question", long_text());
        c.agent = "release".into();
        let (_, _, mut term) = draw_cards(&mut app, vec![c]);
        let ctrl = |app: &mut App, ch: char| key(app, &KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL), false);
        assert!(ctrl(&mut app, 'f'));
        assert!(card_full(&app));
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        let s = screen(&term);
        let a = app.sb.as_ref().unwrap().card.area;
        assert!(a.height > 15, "full screen: {:?}", a);
        assert!(s.contains("? release needs you · full screen · ctrl+f back"), "{}", s);
        assert!(s.contains("more lines · pgdn"), "{}", s);
        assert!(!s.contains("ctrl+f full screen"), "{}", s);
        assert!(press(&mut app, KeyCode::PageDown));
        assert!(app.sb.as_ref().unwrap().card.scroll > 0);
        assert!(ctrl(&mut app, 'f'));
        assert!(!card_full(&app) && app.sb.as_ref().unwrap().card.shown);
        assert!(ctrl(&mut app, 'f'));
        assert!(press(&mut app, KeyCode::Esc));
        assert!(!card_full(&app) && app.sb.as_ref().unwrap().card.shown);
    }

    /// Choices: only a numbered run (1, 2, …) at the end of the text.
    #[test]
    fn choices_are_a_numbered_run_at_the_end() {
        let (b, c) = split_choices("pick one:\n1) sqlite\n2) postgres\n3 - both\n");
        assert_eq!(b, "pick one:");
        assert_eq!(c, vec!["sqlite", "postgres", "both"]);
        for t in ["just text", "1. only one", "steps:\n2. b\n3. c", "a\n1. x\n3. y", "1. x\nthen more"] {
            assert_eq!(split_choices(t), (t.to_string(), Vec::new()), "{:?}", t);
        }
        // a done card keeps its whole summary (numbered lists included)
        let d = card(1, "done", "did:\n1. a\n2. b".into());
        assert_eq!(card_lines(&d, 76).len(), 3);
    }

    /// `/close ` lists the open cards; a query filters by id, kind, agent
    /// or text; the pick fills `/close <id> ` and the list closes.
    #[test]
    fn close_completes_the_open_cards() {
        let mut app = bench::test_app();
        app.sb.as_mut().unwrap().cards = vec![
            card(3, "question", "which db?".into()),
            card(12, "done", "shipped the parser".into()),
        ];
        let labels = |app: &App| -> Vec<String> {
            crate::commands::popup_items(app).into_iter().map(|i| i.label).collect()
        };
        app.ed.text = "/close ".into();
        app.ed.cursor = app.ed.text.chars().count();
        assert_eq!(labels(&app), vec!["#3", "#12"]);
        app.ed.text = "/close 12".into();
        assert_eq!(labels(&app), vec!["#12"]);
        app.ed.text = "/close parser".into();
        let items = crate::commands::popup_items(&app);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].fill, "/close 12 ");
        assert!(items[0].desc.contains("done @t1") && items[0].desc.contains("shipped"));
        assert!(items[0].run.is_none());
        app.ed.text = "/close 12 ".into();
        assert!(labels(&app).is_empty(), "the id is typed: Enter sends");
        app.sb.as_mut().unwrap().cards.clear();
        app.ed.text = "/close ".into();
        assert!(labels(&app).is_empty());
    }
}
