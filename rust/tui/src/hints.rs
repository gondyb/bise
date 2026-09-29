//! One-time hints (BISE-61, book §15 step 6, contract C4).
//!
//! No tour: each hint shows once per user, next to the thing, the first
//! time it happens: the first agent (left of the panel), the first
//! message between agents in view (under it), the first card (above it).
//! A hint is a small accent-bordered note; it goes away when used (the
//! agent looked into, the fold opened or gone, the card answered) or
//! after the next user message.
//!
//! `hints::once(app, Hint::X)` asks for one (the event handling in
//! `sb.rs` calls it); it is marked seen in `hints.json` (`{ "first_agent":
//! true, … }`, next to the onboarding's flag in the Switchboard state
//! root) the first time it is really drawn. `SB_ONBOARDING=off` turns the
//! hints off too (the tmux tests). Under `cargo test` they are off unless
//! a test gives a store ([`use_store`]).
//!
//! Where a hint goes is read from the drawn frame (the panel's row `1`,
//! the last fold / level-3 row, the last card title), so the layout code
//! of the other tracks stays as it is.

use crate::theme;
use crate::App;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hint {
    FirstAgent,
    FirstLevel3,
    FirstCard,
    /// book §15 (⚠ proposed): for the steering marks (BISE-15, track F)
    FirstSteer,
}

impl Hint {
    /// Its key in `hints.json`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Hint::FirstAgent => "first_agent",
            Hint::FirstLevel3 => "first_level3",
            Hint::FirstCard => "first_card",
            Hint::FirstSteer => "first_steer",
        }
    }

    /// The text (book §15); `{…}` is a key, in accent.
    pub(crate) fn text(self) -> &'static str {
        match self {
            Hint::FirstAgent => {
                "new: your agents. they work in the background. {⌥ 1} to look inside, {esc} to come back. →"
            }
            Hint::FirstLevel3 => "agents talk to each other. it stays dim: you can ignore it, or {▸} to read.",
            Hint::FirstCard => "a card: someone needs you. type your answer, {alt+r} sends it. ↓",
            Hint::FirstSteer => "{✓} the agent got it · {✓✓} it read it.",
        }
    }
}

// ---- the store ----

/// `hints.json` next to the onboarding flag.
pub(crate) fn store_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        STORE.with(|s| s.borrow().clone())
    }
    #[cfg(not(test))]
    {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        if env(crate::onboarding::ENV).is_some_and(|v| matches!(v.trim(), "off" | "0" | "no")) {
            return None;
        }
        Some(crate::onboarding::flag_path(&env).with_file_name("hints.json"))
    }
}

/// The keys marked in a store text.
pub(crate) fn seen_in(text: &str) -> BTreeMap<String, bool> {
    serde_json::from_str::<BTreeMap<String, serde_json::Value>>(text)
        .map(|m| m.into_iter().map(|(k, v)| (k, v.as_bool().unwrap_or(false))).collect())
        .unwrap_or_default()
}

/// Mark `key` in the store file (other keys kept).
pub(crate) fn mark_in(path: &std::path::Path, key: &str) -> std::io::Result<()> {
    let mut m = std::fs::read_to_string(path).map(|t| seen_in(&t)).unwrap_or_default();
    m.insert(key.to_string(), true);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&m).unwrap_or_default() + "\n")
}

// ---- the live hints (UI thread only) ----

#[derive(Default)]
struct State {
    /// the store, read once
    seen: Option<BTreeMap<String, bool>>,
    /// asked for, waiting for their thing to be on screen
    pending: Vec<Hint>,
    /// the one drawn now (marked seen when it came up)
    active: Option<Hint>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    #[cfg(test)]
    static STORE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Tests: hints on, with this store (per thread).
#[cfg(test)]
pub(crate) fn use_store(p: Option<PathBuf>) {
    STORE.with(|s| *s.borrow_mut() = p);
    STATE.with(|s| *s.borrow_mut() = State::default());
}

fn is_seen(st: &mut State, path: &std::path::Path, h: Hint) -> bool {
    let seen = st
        .seen
        .get_or_insert_with(|| std::fs::read_to_string(path).map(|t| seen_in(&t)).unwrap_or_default());
    seen.get(h.key()).copied().unwrap_or(false)
}

/// Ask for hint `h` (contract C4). A hint never seen waits until its
/// thing is on screen and no other hint is up, then shows (and counts as
/// seen). True when it is waiting or up.
pub(crate) fn once(_app: &App, h: Hint) -> bool {
    request(h)
}

fn request(h: Hint) -> bool {
    let Some(path) = store_path() else { return false };
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if st.active == Some(h) || st.pending.contains(&h) {
            return true;
        }
        if is_seen(&mut st, &path, h) {
            return false;
        }
        st.pending.push(h);
        true
    })
}

/// The hint up now.
#[cfg(test)]
pub(crate) fn active() -> Option<Hint> {
    STATE.with(|s| s.borrow().active)
}

/// `h` was used, or its thing is gone: it goes away (up or waiting).
pub(crate) fn used(h: Hint) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.pending.retain(|p| *p != h);
        if st.active == Some(h) {
            st.active = None;
        }
    });
}

/// The user sent a message: the hint up goes away.
pub(crate) fn user_message() {
    STATE.with(|s| s.borrow_mut().active = None);
}

/// `h` comes up: out of the queue, marked seen in the store.
fn bring_up(h: Hint) {
    let Some(path) = store_path() else { return };
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.pending.retain(|p| *p != h);
        st.active = Some(h);
        st.seen.get_or_insert_with(BTreeMap::new).insert(h.key().to_string(), true);
        let _ = mark_in(&path, h.key());
    });
}

// ---- drawing ----

/// Inner text width of a hint box (the mockup's 36ch).
const TEXT_W: usize = 36;

/// The words of `text`, `{…}` in accent (a key never splits), wrapped at
/// `w` columns.
pub(crate) fn wrap(text: &str, w: usize) -> Vec<Line<'static>> {
    let mut words: Vec<(String, bool)> = Vec::new();
    for (i, part) in text.split(['{', '}']).enumerate() {
        if i % 2 == 1 {
            words.push((part.to_string(), true));
        } else {
            words.extend(part.split(' ').filter(|w| !w.is_empty()).map(|w| (w.to_string(), false)));
        }
    }
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut col = 0usize;
    for (wd, k) in words {
        let ww = wd.width();
        if col > 0 && col + 1 + ww > w {
            lines.push(Vec::new());
            col = 0;
        }
        let line = lines.last_mut().expect("one line");
        if col > 0 {
            line.push(Span::raw(" "));
            col += 1;
        }
        let c = if k { theme::accent() } else { theme::text() };
        line.push(Span::styled(wd, Style::default().fg(c)));
        col += ww;
    }
    lines.into_iter().map(Line::from).collect()
}

/// The text of row `y` of `buf` between columns `x0..x1`.
fn row_text(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    (x0..x1).map(|x| buf[(x, y)].symbol()).collect()
}

/// Where hint `h` points at in the drawn frame: the anchor row, if any.
fn anchor(buf: &Buffer, h: Hint, feed: Rect, panel: Option<Rect>) -> Option<u16> {
    let rows = |r: Rect| (r.y..r.bottom()).map(move |y| (y, row_text(buf, y, r.x, r.right())));
    match h {
        Hint::FirstAgent => {
            let p = panel?;
            rows(p).find_map(|(y, t)| t.trim_start_matches(['│', ' ']).starts_with("1 ").then_some(y))
        }
        Hint::FirstLevel3 => {
            // a level-3 chip (BISE-106): its envelope then its arrow
            let (env, arrow) = (crate::render::envelope(), theme::glyph("→"));
            let chip = |t: &str| t.find(env).is_some_and(|at| t[at..].contains(arrow));
            rows(feed)
                .filter(|(_, t)| t.contains("messages between") || chip(t))
                .map(|(y, _)| y)
                .next_back()
        }
        Hint::FirstCard => {
            let title = format!("┃ {} ", theme::glyph(theme::G_CARD));
            rows(feed).filter(|(_, t)| t.contains(&title) && t.contains("needs you")).map(|(y, _)| y).next_back()
        }
        Hint::FirstSteer => {
            let read = theme::glyph(theme::G_READ);
            rows(feed).filter(|(_, t)| t.trim_start().starts_with(theme::glyph(theme::G_YOU)) && t.contains(read)).map(|(y, _)| y).next_back()
        }
    }
}

/// The box of hint `h` next to its anchor row `y` (None: no room).
pub(crate) fn place(h: Hint, y: u16, lines: u16, area: Rect, feed: Rect, panel: Option<Rect>) -> Option<Rect> {
    let bw = (TEXT_W as u16 + 4).min(feed.width.saturating_sub(4));
    let bh = lines + 2;
    if bw < 16 || bh > area.height {
        return None;
    }
    let (x, y) = match h {
        // left of the panel, level with agent 1, the arrow pointing at it
        Hint::FirstAgent => {
            let p = panel?;
            (p.x.checked_sub(bw + 1)?, y.saturating_sub(1).max(area.y))
        }
        // under the line, else above it
        Hint::FirstLevel3 | Hint::FirstSteer => {
            let limit = area.bottom().saturating_sub(4);
            let y = if y + 1 + bh <= limit { y + 1 } else { y.checked_sub(bh)? };
            (feed.x + 4, y)
        }
        // above the card, the arrow pointing down at it
        Hint::FirstCard => (feed.x + 4, y.checked_sub(bh)?.max(area.y)),
    };
    Some(Rect { x, y, width: bw, height: bh }.intersection(area))
}

/// Draw the hint up, else the first waiting one whose thing is on screen
/// (after the frame is drawn: the anchor is read from it). Coming up marks
/// it seen; a hint up whose thing left the screen goes away.
pub(crate) fn draw(app: &App, f: &mut Frame) {
    let (active, pending) = STATE.with(|s| {
        let st = s.borrow();
        (st.active, st.pending.clone())
    });
    if active.is_none() && pending.is_empty() {
        return;
    }
    let area = f.area();
    let (feed, panel) = crate::sb::split(app, area);
    let found = match active {
        Some(h) => match anchor(f.buffer_mut(), h, feed, panel) {
            Some(y) => Some((h, y)),
            None => {
                used(h);
                None
            }
        },
        None => pending.into_iter().find_map(|h| anchor(f.buffer_mut(), h, feed, panel).map(|y| (h, y))),
    };
    let Some((h, y)) = found else { return };
    let lines = wrap(h.text(), TEXT_W);
    let Some(r) = place(h, y, lines.len() as u16, area, feed, panel) else { return };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .style(Style::default().bg(theme::card_tint()))
        .padding(Padding::horizontal(1));
    f.render_widget(Clear, r);
    f.render_widget(Paragraph::new(lines).block(block), r);
    if active.is_none() {
        bring_up(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-hints-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("hints.json")
    }

    fn text_of(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_store_keeps_other_keys() {
        let p = tmp("store");
        assert!(seen_in("junk").is_empty());
        std::fs::write(&p, "{\"other\": true}").unwrap();
        mark_in(&p, "first_card").unwrap();
        let m = seen_in(&std::fs::read_to_string(&p).unwrap());
        assert_eq!(m.get("other"), Some(&true));
        assert_eq!(m.get("first_card"), Some(&true));
    }

    #[test]
    fn off_without_a_store() {
        use_store(None);
        assert!(!request(Hint::FirstCard));
        assert_eq!(active(), None);
    }

    #[test]
    fn once_then_seen_when_it_comes_up_across_restarts() {
        let p = tmp("once");
        use_store(Some(p.clone()));
        assert!(request(Hint::FirstCard));
        assert!(request(Hint::FirstAgent));
        // waiting, not up: nothing is seen yet
        assert_eq!(active(), None);
        assert!(!p.exists());
        bring_up(Hint::FirstCard);
        assert_eq!(active(), Some(Hint::FirstCard));
        assert!(seen_in(&std::fs::read_to_string(&p).unwrap())["first_card"]);
        assert!(request(Hint::FirstCard));
        user_message();
        assert_eq!(active(), None);
        assert!(!request(Hint::FirstCard));
        // a restart reads the store again; the one never shown can come
        use_store(Some(p.clone()));
        assert!(!request(Hint::FirstCard));
        assert!(request(Hint::FirstAgent));
        used(Hint::FirstAgent);
        assert!(STATE.with(|s| s.borrow().pending.is_empty()));
    }

    #[test]
    fn it_comes_up_when_its_thing_is_drawn_and_goes_with_it() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let p = tmp("draw");
        use_store(Some(p.clone()));
        let app = crate::sb::bench::test_app();
        request(Hint::FirstLevel3);
        request(Hint::FirstCard);
        let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let screen = |t: &Terminal<TestBackend>| {
            let b = t.backend().buffer();
            (0..30).map(|y| row_text(b, y, 0, 120)).collect::<Vec<_>>().join("\n")
        };
        // nothing to point at: nothing shows
        t.draw(|f| draw(&app, f)).unwrap();
        assert_eq!(active(), None);
        // a card title in the feed: its hint comes up above it
        let card = format!("  ┃ {} t1 needs you", theme::glyph(theme::G_CARD));
        t.draw(|f| {
            f.render_widget(Paragraph::new(card.as_str()), Rect::new(3, 20, 80, 1));
            draw(&app, f)
        })
        .unwrap();
        assert_eq!(active(), Some(Hint::FirstCard));
        let sc = screen(&t);
        assert!(sc.contains("a card: someone needs you."), "{sc}");
        assert!(seen_in(&std::fs::read_to_string(&p).unwrap())["first_card"]);
        // the card is gone: so is the hint; the level-3 one waits its turn
        t.draw(|f| draw(&app, f)).unwrap();
        assert_eq!(active(), None);
        let l3 = format!("   {} t1 → t2  v1 or v2?", crate::render::envelope());
        t.draw(|f| {
            f.render_widget(Paragraph::new(l3.as_str()), Rect::new(3, 5, 80, 1));
            draw(&app, f)
        })
        .unwrap();
        assert_eq!(active(), Some(Hint::FirstLevel3));
        assert!(screen(&t).contains("agents talk to each other."));
    }

    #[test]
    fn keys_are_accent_and_lines_fit() {
        let ls = wrap(Hint::FirstAgent.text(), TEXT_W);
        let all: Vec<String> = ls.iter().map(text_of).collect();
        assert_eq!(
            all.join(" "),
            "new: your agents. they work in the background. ⌥ 1 to look inside, esc to come back. →"
        );
        assert!(all.iter().all(|l| l.width() <= TEXT_W), "{all:?}");
        let accent: Vec<&str> = ls
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.fg == Some(theme::accent()))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(accent, vec!["⌥ 1", "esc"]);
        let l3: Vec<String> = wrap(Hint::FirstLevel3.text(), TEXT_W).iter().map(text_of).collect();
        assert_eq!(l3.join(" "), "agents talk to each other. it stays dim: you can ignore it, or ▸ to read.");
        let card: Vec<String> = wrap(Hint::FirstCard.text(), TEXT_W).iter().map(text_of).collect();
        assert_eq!(card.join(" "), "a card: someone needs you. type your answer, alt+r sends it. ↓");
    }

    #[test]
    fn boxes_sit_next_to_their_thing() {
        let area = Rect::new(0, 0, 120, 40);
        let feed = Rect::new(0, 0, 90, 40);
        let panel = Some(Rect::new(90, 0, 30, 40));
        // left of the panel, level with its row
        let r = place(Hint::FirstAgent, 3, 3, area, feed, panel).unwrap();
        assert_eq!((r.right(), r.y, r.height), (89, 2, 5));
        assert_eq!(place(Hint::FirstAgent, 3, 3, area, feed, None), None);
        // under a level-3 row, above it near the bottom
        assert_eq!(place(Hint::FirstLevel3, 10, 3, area, feed, panel).unwrap().y, 11);
        assert_eq!(place(Hint::FirstLevel3, 34, 3, area, feed, panel).unwrap().y, 29);
        // above a card
        assert_eq!(place(Hint::FirstCard, 30, 3, area, feed, panel).unwrap().bottom(), 30);
    }
}
