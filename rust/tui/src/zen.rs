//! Zen while you type (BISE-121, book §9 "Zen while you type"): while
//! you write in the composer, the rest of the screen steps back.
//!
//! - Enters on a typing key that changes the composer's text (a letter,
//!   a space, backspace, delete, shift+enter; a paste), with no popup
//!   open.
//! - Leaves [`HOLD`] (8 s) after the last typing key, or at once on any
//!   other input: a mouse move, click or scroll, a key that moves the
//!   cursor or is not typing (arrows, esc, tab, enter, ctrl+…, alt+…),
//!   a popup, the terminal losing the focus, and anything that needs
//!   you (a card, a message to you, a confirm, an error).
//! - In zen, every cell's text is mixed [`DEPTH`] (45 %) toward its own
//!   background, over [`FADE`] (250 ms) in [`STEPS`] steps, out the same
//!   way. Kept as they are: the composer's text and cursor, the
//!   divider's label, the card box, and every cell in the accent or the
//!   error color (what needs you). The gust slows to half speed and each
//!   of its tones drops one step; the tick pulses hold still.
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

/// Zen lasts this long after the last typing key.
pub(crate) const HOLD: Duration = Duration::from_secs(8);
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
    /// what the last frame keeps as is (composer text, divider label,
    /// card box); set by the draw
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
    /// anything else the user does: leaves zen
    Other,
    /// not the user's doing (a key release, a resize, focus back)
    Neutral,
}

/// A key that types (a char with no modifier but shift, backspace,
/// delete, shift+enter): only these hold zen.
pub(crate) fn typing_key(k: &KeyEvent) -> bool {
    let only_shift = (k.modifiers - KeyModifiers::SHIFT).is_empty();
    match k.code {
        KeyCode::Char(_) => only_shift,
        KeyCode::Backspace | KeyCode::Delete => k.modifiers.is_empty(),
        KeyCode::Enter => k.modifiers == KeyModifiers::SHIFT,
        _ => false,
    }
}

/// A key event, told by what it did: `changed` the composer's text,
/// with a popup `open` after it.
pub(crate) fn key_input(k: &KeyEvent, changed: bool, popup_open: bool) -> Input {
    if k.kind == KeyEventKind::Release {
        Input::Neutral
    } else if typing_key(k) && changed && !popup_open {
        Input::Typing
    } else {
        Input::Other
    }
}

impl Zen {
    /// In zen at `now` (not counting the fade out).
    pub(crate) fn active(&self, now: Instant) -> bool {
        self.start.is_some() && self.broken.is_none() && self.last.is_some_and(|t| now < t + HOLD)
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
    fn typing_keys_are_chars_backspace_delete_and_shift_enter() {
        use KeyCode::*;
        let (n, s, c, a) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::CONTROL, KeyModifiers::ALT);
        for k in [key(Char('a'), n), key(Char('A'), s), key(Char(' '), n), key(Char('é'), n), key(Backspace, n), key(Delete, n), key(Enter, s)] {
            assert!(typing_key(&k), "{k:?}");
        }
        for k in [
            key(Char('w'), c),
            key(Char('b'), a),
            key(Left, n),
            key(Up, n),
            key(Home, n),
            key(End, n),
            key(Esc, n),
            key(Tab, n),
            key(Enter, n),
            key(Enter, a),
            key(Backspace, a),
            key(PageUp, n),
            key(F(1), n),
        ] {
            assert!(!typing_key(&k), "{k:?}");
        }
    }

    #[test]
    fn a_key_counts_by_what_it_did() {
        let a = key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_input(&a, true, false), Input::Typing);
        // a space that toggled a section (text unchanged), a `/` that
        // opened the popup: not typing
        assert_eq!(key_input(&a, false, false), Input::Other);
        assert_eq!(key_input(&a, true, true), Input::Other);
        let mut up = a;
        up.kind = KeyEventKind::Release;
        up.state = KeyEventState::NONE;
        assert_eq!(key_input(&up, true, false), Input::Neutral);
        assert_eq!(key_input(&key(KeyCode::Left, KeyModifiers::NONE), false, false), Input::Other);
    }

    #[test]
    fn zen_enters_on_typing_and_leaves_8s_after_the_last_key() {
        let t = Instant::now();
        let mut z = Zen::default();
        assert!(!z.active(t));
        z.input(Input::Typing, t);
        assert!(z.active(t));
        z.input(Input::Typing, ms(t, 5000));
        assert!(z.active(ms(t, 12_999)), "8 s after the LAST key");
        assert!(!z.active(ms(t, 13_000)));
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
        assert_eq!(z.level(ms(t, 3000 + 8000 - 1)), 1.0);
        assert_eq!(z.level(ms(t, 3000 + 8000 + 125)), 0.5);
        assert_eq!(z.level(ms(t, 3000 + 8000 + 250)), 0.0);
        assert!((z.depth(ms(t, 3000 + 7000)) - DEPTH).abs() < 1e-6);
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
