//! The attention cards: the hub's questions and alerts, answered from
//! the card box above the composer (Ctrl+G, Alt+R).

use super::*;
use crate::commands::PopItem;
use ratatui::layout::Margin;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

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

/// The widest a card line gets: longer lines are hard to follow.
const READ_WIDTH: usize = 100;

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

/// Alt+R: the composer's text answers the current card, shown or not
/// (Enter still talks to the agent in focus). An empty composer only
/// acknowledges the cards that need no words (done, overlap).
/// A card kind: its rank in reading order (what blocks a task first),
/// its icon and its color.
fn kind_look(kind: &str) -> (u8, &'static str, Color) {
    match kind {
        "question" => (0, "?", WARN),
        "blocked" => (1, GLYPH_WARN, WARN),
        "failed" | "restart" => (2, GLYPH_ERR, ERR),
        "drop" => (3, "⇣", WARN),
        "overlap" => (4, "⚠", WARN),
        "done" => (5, GLYPH_OK, OK),
        _ => (5, "◆", WARN),
    }
}

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
        if !matches!(kind.as_str(), "done" | "overlap") {
            let msg = format!("card #{} (@{}): type your answer, then Alt+R", id, agent);
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
/// borders and the padding take 4).
fn text_width(width: u16) -> usize {
    (width as usize).saturating_sub(4).clamp(1, READ_WIDTH)
}

pub(crate) fn card_full(app: &App) -> bool {
    app.sb
        .as_ref()
        .is_some_and(|sb| sb.card.shown && sb.card.full && !sb.cards.is_empty())
}

fn card_lines(c: &Card, width: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for l in c.text.lines() {
        out.extend(wrap_line(
            Line::from(Span::styled(l.to_string(), Style::default().fg(TEXT))),
            width,
        ));
    }
    if !c.note.is_empty() {
        out.push(Line::from(""));
        out.extend(wrap_line(
            Line::from(Span::styled(
                format!("ⓘ {}", c.note),
                Style::default().fg(INFO).add_modifier(Modifier::ITALIC),
            )),
            width,
        ));
    }
    out
}

fn ago(ms: u64) -> String {
    let s = ms / 1000;
    if s < 60 {
        format!("{} s", s)
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else {
        format!("{} h", s / 3600)
    }
}

/// The keys of the card box footer, most useful first; the footer keeps
/// those that fit in `width` columns.
fn footer(kind: &str, pos: Option<String>, many: bool, full: bool, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let answer = if matches!(kind, "done" | "overlap") {
        "alt+r ok"
    } else {
        "alt+r answer"
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = pos {
        parts.push(format!("{} ↑↓ PgUp/PgDn", p));
    }
    parts.push(answer.into());
    if many {
        parts.push("ctrl+n/p next".into());
    }
    parts.push(if full { "ctrl+f box" } else { "ctrl+f full" }.into());
    parts.push("ctrl+x close".into());
    parts.push("esc hide".into());
    let mut out = String::new();
    for p in parts {
        let next = if out.is_empty() { p } else { format!("{} · {}", out, p) };
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
    let (_, icon, color) = kind_look(&c.kind);
    let age = ago(c.age_ms.saturating_add(c.seen_at.elapsed().as_millis() as u64));
    let title = format!(
        " ◆ card {}/{} · {} #{} {} @{} · {} ago ",
        pos,
        order.len(),
        icon,
        c.id,
        c.kind,
        c.agent,
        age
    );
    let scroll = sb.card.scroll.min(max_scroll);
    let shown = (max_scroll > 0).then(|| crate::help::rows_shown(scroll, visible, lines.len()));
    let foot = footer(
        &c.kind,
        shown,
        order.len() > 1,
        sb.card.full,
        (area.width as usize).saturating_sub(2),
    );
    sb.card.scroll = scroll;
    sb.card.max_scroll = max_scroll;
    sb.card.page = visible.saturating_sub(1).max(1);
    sb.card.area = area;
    let title = Line::from(Span::styled(
        truncate_chars(&title, (area.width as usize).saturating_sub(4)),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    let total = lines.len();
    crate::help::scroll_box(frame, area, color, title, foot, lines, scroll);
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
            let (_, icon, color) = kind_look(&c.kind);
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
        let (w, h) = (100u16, 40u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.sb.as_mut().unwrap().cards = vec![card(3, "question", long_text()), card(4, "done", "ok".into())];
        app.sb.as_mut().unwrap().toggle_card();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        let s = screen(&term);
        let area = app.sb.as_ref().unwrap().card.area;
        assert!(area.height >= h * 6 / 10, "box height {} of {}", area.height, h);
        assert!(s.contains("line 01 of the card"));
        assert!(s.contains("1–"), "{}", s);
        for k in ["↑↓ PgUp/PgDn", "alt+r answer", "ctrl+n/p next", "ctrl+f full"] {
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
            assert!(a.y.saturating_add(a.height) <= h.saturating_sub(7) || a.height == 0, "{}x{}: {:?}", w, h, a);
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
        // 300 chars wrapped at 100 columns: 3-4 rows, plus the borders
        assert!((5..=6).contains(&h), "height {}", h);
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
