//! Zen while you type (BISE-121, book §9 "Zen while you type"): while
//! you write in the composer, the rest of the screen steps back.
//!
//! - Enters on a composer key that changes the composer (its text or a
//!   pending dead key: a character, with any modifier, that types; an
//!   Option accent; backspace, delete, a new line (shift/alt+⏎,
//!   ctrl+j); a paste), with no popup open.
//! - Holds (the timer starts again) on every key that edits or moves
//!   inside the composer (BISE-124): the above, and the arrows,
//!   home/end, the word moves (⌥/ctrl+arrows), undo, select all. A key
//!   the app has no use for (a lone modifier, caps lock) changes nothing.
//! - Leaves [`HOLD`] (5 s, BISE-128) after the last typing key, or at
//!   once on: ⏎ send (BISE-128), a mouse move, click or scroll, the
//!   terminal losing the focus, esc, tab, page up/down, every key an app
//!   shortcut takes before the composer (⌥0-9 and the panel keys, the
//!   card keys, ctrl+g, ctrl+o and the other app shortcuts, end back to
//!   the bottom, a key while the help or the terminal pane is up), a
//!   `/` `@` `$` popup, and anything that needs you (a card, a message
//!   to you, a confirm, an error).
//! - In zen, the chrome's text is mixed [`DEPTH`] (45 %) toward its own
//!   background, over [`FADE`] (250 ms) in [`STEPS`] steps, out the same
//!   way: the header (frame title, counts), the frame lines, the agents
//!   panel (agents, cards list), the divider's rule and right side, the
//!   queue and attachments, the key bar. Kept as they are: the history
//!   you read (BISE-132: the feed area, from its first row to the
//!   divider; its new lines still come), the composer's text and cursor,
//!   the divider's label, the card box, and every cell in the accent or
//!   the error color (what needs you). The other agents' gusts (panel,
//!   header) stand still (BISE-132); the gust of the agent in view (the
//!   divider's label) slows to half speed, each tone one step down; the
//!   tick pulses hold still.
//! - `BISE_REDUCE_MOTION`: no ramp (one step in, one out). `NO_COLOR`: no
//!   mixed colors: the terminal's dim attribute on the faded cells.
//!   `BISE_ASCII` changes nothing here.
//!
//! Pure state and a buffer pass: the loop feeds it the events and the
//! clock; it draws nothing and asks for no frame (the loop draws every
//! 80 ms anyway; a steady zen writes no cell).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::time::{Duration, Instant};

/// Zen lasts this long after the last composer key (BISE-128: was 8 s).
pub(crate) const HOLD: Duration = Duration::from_secs(5);
/// The fade in (and out).
pub(crate) const FADE: Duration = Duration::from_millis(250);
/// The fade's steps: at most this many repaints each way.
pub(crate) const STEPS: u32 = 4;
/// How far a faded cell's text goes toward its background (0 = none, 1 =
/// gone).
pub(crate) const DEPTH: f32 = 0.45;

/// The zen state: when it started, the last typing key, a break.
#[derive(Clone, Debug)]
pub(crate) struct Zen {
    start: Option<Instant>,
    last: Option<Instant>,
    broken: Option<Instant>,
    /// the fade's length (`Duration::ZERO` under `BISE_REDUCE_MOTION`)
    pub(crate) fade: Duration,
    /// `NO_COLOR`: dim attribute instead of mixed colors
    pub(crate) no_color: bool,
    /// the "needs you" count last seen (a change breaks zen)
    calls: u64,
    /// what the last frame keeps as is (the history, composer text,
    /// divider label, card box); set by the draw
    pub(crate) keep: Vec<Rect>,
}

impl Default for Zen {
    fn default() -> Zen {
        Zen { start: None, last: None, broken: None, fade: FADE, no_color: false, calls: 0, keep: Vec::new() }
    }
}

/// How an input counts for zen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Input {
    /// typing into the composer: enters (or holds) zen
    Typing,
    /// a key that stays in the composer without changing it (an arrow, a
    /// word move, home/end): holds zen, never starts it
    Hold,
    /// anything else the user does: leaves zen
    Other,
    /// not the user's doing (a key release, a resize, focus back), or a
    /// key the app has no use for (a lone modifier, caps lock)
    Neutral,
}

/// A key the composer takes (BISE-124): every key the editor maps to an
/// edit or a move (a character with any modifier that types, an Option
/// accent or dead key, backspace/delete and their word forms, the
/// arrows, home/end, ctrl+a/e/…, undo, select all, cut), the newline
/// keys (shift/alt+⏎, ctrl+j). Not: ⏎ send (BISE-128), copy, ⌥0-9
/// (switches agents), esc, tab, page up/down, ctrl+o and the other app
/// shortcuts. Whether it really reached the composer is the caller's to
/// say (`App::key_in_composer`).
pub(crate) fn composer_key(k: &KeyEvent) -> bool {
    use crate::editor::Action;
    match (k.code, k.modifiers) {
        (KeyCode::Enter, KeyModifiers::SHIFT | KeyModifiers::ALT) | (KeyCode::Char('j'), KeyModifiers::CONTROL) => true,
        (KeyCode::Enter, _) => false,
        (KeyCode::Char(c), KeyModifiers::ALT) if c.is_ascii_digit() => false,
        _ => crate::editor::action(k).is_some_and(|a| !matches!(a, Action::Copy)),
    }
}

/// A key that does nothing in the app: no input at all for zen (a lone
/// modifier or a lock key some terminals report).
fn idle_key(k: &KeyEvent) -> bool {
    matches!(
        k.code,
        KeyCode::Null
            | KeyCode::Modifier(_)
            | KeyCode::CapsLock
            | KeyCode::NumLock
            | KeyCode::ScrollLock
            | KeyCode::Media(_)
            | KeyCode::KeypadBegin
    )
}

/// A key event, told by where it went and what it did: `composer` = it
/// reached the composer (nothing outside it changed, no help or
/// terminal pane in front), `changed` = the composer's text or pending
/// dead key changed, `popup` = a popup was open before or after it.
pub(crate) fn key_input(k: &KeyEvent, composer: bool, changed: bool, popup: bool) -> Input {
    if k.kind == KeyEventKind::Release || idle_key(k) {
        Input::Neutral
    } else if !composer_key(k) || !composer || popup {
        Input::Other
    } else if changed {
        Input::Typing
    } else {
        Input::Hold
    }
}

impl Zen {
    /// In zen at `now` (not counting the fade out).
    pub(crate) fn active(&self, now: Instant) -> bool {
        self.start.is_some() && self.broken.is_none() && self.last.is_some_and(|t| now < t + HOLD)
    }

    /// When the last zen ends (or ended): a break, else [`HOLD`] after
    /// the last key; the pulses' hold stops there (anim.rs, BISE-204).
    pub(crate) fn end(&self) -> Option<Instant> {
        self.last.map(|l| self.broken.unwrap_or(l + HOLD))
    }

    /// The fade at `now`, 0 (none) to 1 (all the way), before steps.
    fn raw(&self, now: Instant) -> f32 {
        let (Some(start), Some(last)) = (self.start, self.last) else { return 0.0 };
        let up = |d: Duration| if self.fade.is_zero() { 1.0 } else { (d.as_secs_f32() / self.fade.as_secs_f32()).min(1.0) };
        let end = self.broken.unwrap_or(last + HOLD);
        if now < end {
            return up(now.saturating_duration_since(start));
        }
        let at_end = up(end.saturating_duration_since(start));
        let down = if self.fade.is_zero() { 1.0 } else { now.saturating_duration_since(end).as_secs_f32() / self.fade.as_secs_f32() };
        (at_end - down).max(0.0)
    }

    /// The fade at `now` in [`STEPS`] steps: 0, 1/4 … 1 (0 or 1 under
    /// `NO_COLOR`: the dim attribute has no steps).
    pub(crate) fn level(&self, now: Instant) -> f32 {
        let r = self.raw(now);
        if self.no_color {
            return if r >= 0.5 { 1.0 } else { 0.0 };
        }
        (r * STEPS as f32).round() / STEPS as f32
    }

    /// How far a faded cell goes at `now` (`DEPTH` × the level).
    pub(crate) fn depth(&self, now: Instant) -> f32 {
        DEPTH * self.level(now)
    }

    /// Feed an input at `now`.
    pub(crate) fn input(&mut self, i: Input, now: Instant) {
        match i {
            Input::Typing => {
                if !self.active(now) {
                    // from where the fade out is (no jump back to 0)
                    let back = self.fade.mul_f32(self.raw(now));
                    self.start = Some(now.checked_sub(back).unwrap_or(now));
                    self.broken = None;
                }
                self.last = Some(now);
            }
            Input::Hold => {
                if self.active(now) {
                    self.last = Some(now);
                }
            }
            Input::Other => self.leave(now),
            Input::Neutral => {}
        }
    }

    /// Leave zen at `now` (the fade out starts).
    pub(crate) fn leave(&mut self, now: Instant) {
        if self.active(now) {
            self.broken = Some(now);
        }
    }

    /// The count of things that need you (cards, messages to you,
    /// errors) at `now`: a change leaves zen.
    pub(crate) fn calls(&mut self, n: u64, now: Instant) {
        if n != self.calls {
            self.calls = n;
            self.leave(now);
        }
    }
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

/// `fg` mixed `t` toward `bg` (both RGB; else `None`).
pub(crate) fn toward(fg: Color, bg: Color, t: f32) -> Option<Color> {
    match (fg, bg) {
        (Color::Rgb(r, g, b), Color::Rgb(r2, g2, b2)) => Some(Color::Rgb(mix(r, r2, t), mix(g, g2, t), mix(b, b2, t))),
        _ => None,
    }
}

/// The zen pass (after the theme's paint): every cell's text mixed
/// `depth` toward its background, except the cells in `keep` and the
/// ones that need you (text or background in `attention`: the accent,
/// the error color). `no_color`: the dim attribute instead. Free at 0.
pub(crate) fn fade(buf: &mut Buffer, depth: f32, keep: &[Rect], attention: &[Color], no_color: bool) {
    if depth <= 0.0 {
        return;
    }
    let area = buf.area;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if keep.iter().any(|r| x >= r.x && x < r.right() && y >= r.y && y < r.bottom()) {
                continue;
            }
            let cell = &mut buf[(x, y)];
            if attention.contains(&cell.fg) || attention.contains(&cell.bg) {
                continue;
            }
            match toward(cell.fg, cell.bg, depth).filter(|_| !no_color) {
                Some(c) => cell.fg = c,
                None => cell.modifier.insert(Modifier::DIM),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn key(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    fn ms(t: Instant, n: u64) -> Instant {
        t + Duration::from_millis(n)
    }

    #[test]
    fn composer_keys_are_every_edit_and_move_with_any_modifier() {
        use KeyCode::*;
        let (n, s, c, a, cmd) =
            (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::CONTROL, KeyModifiers::ALT, KeyModifiers::SUPER);
        let (sa, ca) = (s | a, c | a);
        for k in [
            // characters, and the ones the Option layer types (Ghostty on
            // a U.S. layout sends Option as Alt: ⌥e is Char('e') + ALT)
            key(Char('a'), n),
            key(Char('A'), s),
            key(Char(' '), n),
            key(Char('é'), n),
            key(Char('`'), a),
            key(Char('e'), a),
            key(Char('c'), a),
            key(Char('E'), sa),
            // edits and newlines
            key(Backspace, n),
            key(Backspace, a),
            key(Backspace, cmd),
            key(Delete, n),
            key(Delete, c),
            key(Char('w'), c),
            key(Char('u'), c),
            key(Enter, s),
            key(Enter, a),
            key(Char('j'), c),
            // moves: arrows, word moves, home/end, emacs keys
            key(Left, n),
            key(Right, s),
            key(Left, a),
            key(Right, c),
            key(Left, ca),
            key(Right, cmd),
            key(Up, n),
            key(Down, s),
            key(Home, n),
            key(End, n),
            key(Char('a'), c),
            key(Char('e'), c),
            key(Char('b'), a),
            // undo, select all
            key(Char('z'), cmd),
            key(Char('a'), cmd),
        ] {
            assert!(composer_key(&k), "{k:?}");
        }
        for k in [
            // ⏎ sends (BISE-128)
            key(Enter, n),
            key(Enter, c),
            key(Enter, cmd),
            key(Char('1'), a),
            key(Char('0'), a),
            key(Up, a),
            key(Down, a),
            key(Char('o'), c),
            key(Char('c'), c),
            key(Char('l'), c),
            key(Char('g'), c),
            key(Char('v'), c),
            key(Char('c'), cmd),
            key(Esc, n),
            key(Tab, n),
            key(BackTab, s),
            key(PageUp, n),
            key(PageDown, n),
            key(F(1), n),
        ] {
            assert!(!composer_key(&k), "{k:?}");
        }
    }

    #[test]
    fn a_key_counts_by_where_it_went_and_what_it_did() {
        let a = key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_input(&a, true, true, false), Input::Typing);
        // taken outside the composer (a space that toggled a section, a
        // panel key), a `/` that opened the popup: not typing
        assert_eq!(key_input(&a, false, true, false), Input::Other);
        assert_eq!(key_input(&a, true, true, true), Input::Other);
        let mut up = a;
        up.kind = KeyEventKind::Release;
        up.state = KeyEventState::NONE;
        assert_eq!(key_input(&up, true, true, false), Input::Neutral);
        // a move holds; the dead key ⌥` changes the composer: typing
        let left = key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(key_input(&left, true, false, false), Input::Hold);
        assert_eq!(key_input(&key(KeyCode::Char('`'), KeyModifiers::ALT), true, true, false), Input::Typing);
        // an app shortcut leaves, wherever it went
        assert_eq!(key_input(&key(KeyCode::Char('o'), KeyModifiers::CONTROL), true, false, false), Input::Other);
        assert_eq!(key_input(&key(KeyCode::Esc, KeyModifiers::NONE), true, false, false), Input::Other);
        // a lone modifier, caps lock: nothing
        let shift = KeyCode::Modifier(crossterm::event::ModifierKeyCode::LeftShift);
        assert_eq!(key_input(&key(shift, KeyModifiers::SHIFT), false, false, false), Input::Neutral);
        assert_eq!(key_input(&key(KeyCode::CapsLock, KeyModifiers::NONE), false, false, false), Input::Neutral);
    }

    #[test]
    fn a_move_holds_zen_but_never_starts_it() {
        let t = Instant::now();
        let mut z = Zen::default();
        z.input(Input::Hold, t);
        assert!(!z.active(t), "an arrow alone is not typing");
        z.input(Input::Typing, t);
        z.input(Input::Hold, ms(t, 4000));
        assert!(z.active(ms(t, 8_999)), "the timer starts again");
        assert!(!z.active(ms(t, 9_000)));
        // after the time-out, a move does not bring it back
        z.input(Input::Hold, ms(t, 10_000));
        assert!(!z.active(ms(t, 10_000)));
    }

    #[test]
    fn zen_enters_on_typing_and_leaves_5s_after_the_last_key() {
        let t = Instant::now();
        let mut z = Zen::default();
        assert!(!z.active(t));
        z.input(Input::Typing, t);
        assert!(z.active(t));
        z.input(Input::Typing, ms(t, 3000));
        assert!(z.active(ms(t, 7_999)), "5 s after the LAST key");
        assert!(!z.active(ms(t, 8_000)));
        // neutral input changes nothing
        z.input(Input::Neutral, ms(t, 6000));
        assert!(z.active(ms(t, 6000)));
    }

    #[test]
    fn any_other_input_or_a_call_leaves_at_once() {
        let t = Instant::now();
        let mut z = Zen::default();
        z.input(Input::Typing, t);
        z.input(Input::Other, ms(t, 1000));
        assert!(!z.active(ms(t, 1000)));
        // typing again comes back
        z.input(Input::Typing, ms(t, 2000));
        assert!(z.active(ms(t, 2000)));
        // a card, a message to you: the count changes
        z.calls(1, ms(t, 3000));
        assert!(!z.active(ms(t, 3000)));
        // the same count again does not break a new zen
        z.input(Input::Typing, ms(t, 4000));
        z.calls(1, ms(t, 4100));
        assert!(z.active(ms(t, 4100)));
        z.leave(ms(t, 4200));
        assert!(!z.active(ms(t, 4200)));
    }

    #[test]
    fn the_fade_takes_250ms_in_four_steps_and_the_same_out() {
        let t = Instant::now();
        let mut z = Zen::default();
        assert_eq!(z.level(t), 0.0);
        z.input(Input::Typing, t);
        let lv: Vec<f32> = [0, 63, 125, 188, 250, 1000].iter().map(|&n| z.level(ms(t, n))).collect();
        assert_eq!(lv, vec![0.0, 0.25, 0.5, 0.75, 1.0, 1.0]);
        z.input(Input::Other, ms(t, 1000));
        let lv: Vec<f32> = [1000, 1063, 1125, 1188, 1250, 2000].iter().map(|&n| z.level(ms(t, n))).collect();
        assert_eq!(lv, vec![1.0, 0.75, 0.5, 0.25, 0.0, 0.0]);
        // the time-out fades out the same way
        z.input(Input::Typing, ms(t, 3000));
        assert_eq!(z.level(ms(t, 3000 + 5000 - 1)), 1.0);
        assert_eq!(z.level(ms(t, 3000 + 5000 + 125)), 0.5);
        assert_eq!(z.level(ms(t, 3000 + 5000 + 250)), 0.0);
        assert!((z.depth(ms(t, 3000 + 4000)) - DEPTH).abs() < 1e-6);
    }

    #[test]
    fn typing_during_the_fade_out_turns_it_back_without_a_jump() {
        let t = Instant::now();
        let mut z = Zen::default();
        z.input(Input::Typing, t);
        z.input(Input::Other, ms(t, 500));
        // half-way out
        assert_eq!(z.level(ms(t, 625)), 0.5);
        z.input(Input::Typing, ms(t, 625));
        assert_eq!(z.level(ms(t, 625)), 0.5);
        assert_eq!(z.level(ms(t, 750)), 1.0);
    }

    #[test]
    fn reduce_motion_and_no_color_have_no_ramp() {
        let t = Instant::now();
        let mut z = Zen { fade: Duration::ZERO, ..Zen::default() };
        z.input(Input::Typing, t);
        assert_eq!(z.level(t), 1.0);
        z.leave(ms(t, 10));
        assert_eq!(z.level(ms(t, 10)), 0.0);
        let mut z = Zen { no_color: true, ..Zen::default() };
        z.input(Input::Typing, t);
        assert_eq!([z.level(ms(t, 60)), z.level(ms(t, 130)), z.level(ms(t, 260))], [0.0, 1.0, 1.0]);
    }

    #[test]
    fn the_pass_mixes_text_toward_its_ground_and_keeps_what_needs_you() {
        let (ground, ink, accent) = (Color::Rgb(0x14, 0x12, 0x11), Color::Rgb(0xec, 0xe6, 0xda), Color::Rgb(0xf4, 0xa6, 0xb0));
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        for c in buf.content.iter_mut() {
            c.fg = ink;
            c.bg = ground;
        }
        buf[(1, 0)].fg = accent;
        buf[(2, 0)].fg = Color::Gray;
        let keep = [Rect::new(0, 1, 2, 1)];
        fade(&mut buf, 0.0, &keep, &[accent], false);
        assert!(buf.content.iter().all(|c| c.fg != Color::Rgb(0x8a, 0x84, 0x7c)), "free at 0");
        fade(&mut buf, DEPTH, &keep, &[accent], false);
        // #ece6da 45 % toward #141211: #8b8780
        let faded = Color::Rgb(0x8b, 0x87, 0x80);
        assert_eq!(toward(ink, ground, DEPTH), Some(faded));
        assert_eq!(buf[(0, 0)].fg, faded);
        assert_eq!(buf[(3, 1)].fg, faded);
        assert_eq!(buf[(1, 0)].fg, accent, "accent: needs you");
        assert_eq!((buf[(0, 1)].fg, buf[(1, 1)].fg), (ink, ink), "kept rect");
        assert_eq!(buf[(2, 0)].fg, Color::Gray);
        assert!(buf[(2, 0)].modifier.contains(Modifier::DIM), "not rgb: dim");
        assert_eq!(buf[(0, 0)].bg, ground, "the ground never moves");
        // NO_COLOR: the dim attribute, colors untouched
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        buf[(0, 0)].fg = ink;
        buf[(0, 0)].bg = ground;
        fade(&mut buf, DEPTH, &[], &[], true);
        assert_eq!(buf[(0, 0)].fg, ink);
        assert!(buf[(0, 0)].modifier.contains(Modifier::DIM));
    }
}
