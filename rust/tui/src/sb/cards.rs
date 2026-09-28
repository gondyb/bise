//! The attention cards: the hub's questions and alerts, answered from
//! the card box above the composer (Ctrl+G, Alt+R).

use super::*;

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
/// screen over the feed instead).
pub(crate) fn card_box_height(app: &App, area: Rect) -> u16 {
    let Some(sb) = app.sb.as_ref() else { return 0 };
    if !sb.card.shown || sb.card.full {
        return 0;
    }
    let Some(c) = sb.current_card() else { return 0 };
    let w = (area.width as usize).saturating_sub(4).max(1);
    let rows = card_lines(c, w).len() as u16 + 2;
    let cap = (area.height * 35 / 100).max(5);
    rows.min(cap)
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

/// The card box: the whole text, wrapped, scrolled by PgUp/PgDn.
pub(crate) fn draw_card(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(sb) = app.sb.as_mut() else { return };
    if area.height < 3 {
        return;
    }
    let order: Vec<u64> = sb.sorted_cards().iter().map(|c| c.id).collect();
    let Some(c) = sb.current_card() else { return };
    let w = (area.width as usize).saturating_sub(4).max(1);
    let lines = card_lines(c, w);
    let visible = area.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    let pos = order.iter().position(|x| *x == c.id).unwrap_or(0) + 1;
    let (_, icon, color) = kind_look(&c.kind);
    let age = ago(c.age_ms + c.seen_at.elapsed().as_millis() as u64);
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
    let more = if max_scroll > 0 {
        format!(" {} · PgUp/PgDn ", crate::help::rows_shown(scroll, visible, lines.len()))
    } else {
        String::new()
    };
    sb.card.scroll = scroll;
    sb.card.max_scroll = max_scroll;
    sb.card.page = (visible / 2).max(1);
    let title = Line::from(Span::styled(
        truncate_chars(&title, (area.width as usize).saturating_sub(4)),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    crate::help::scroll_box(frame, area, color, title, more, lines, scroll);
}
