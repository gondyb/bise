//! Working = a gust blowing by (book §6, §9, BISE-107): the animated mark
//! of a working agent. The motif is data (frames × cells, the glyphs'
//! tones and ASCII forms, the frame time), so another pick of the user
//! (site/book/working.html) is a table swap: [`MOTIF`].
//!
//! - header and divider: a strip of 5 cells, 3 when short on room, or the
//!   1-cell breath ([`Size`], book §9 "Short on room (the gust)");
//! - the panel's status cell: always the breath.
//!
//! The draw loop keeps the [`Motion`] in `App::motion`: a frame number
//! from the clock ([`clock`]), or [`Motion::Still`] (the terminal lost the
//! focus, a draw took longer than the budget, `BISE_REDUCE_MOTION` is set;
//! and in tests): then every form is one static `∿` in the text color.
//! While you type (zen, BISE-121) it is [`Motion::Calm`]: half the speed,
//! each tone one step down (text → dim → faint).
//! Ratatui only rewrites changed cells: with 110 ms a frame, at most ~9
//! cell updates a second, and none when no agent works (no gust drawn).

use crate::theme::{self, dim, faint, text};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::time::Duration;

/// Moving (a frame number) or still.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Still,
    Frame(u64),
    /// zen (BISE-121): the frame at half speed, the tones one step down
    Calm(u64),
}

/// The gust's form: the whole strip, the short one, or the breath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Size {
    Five,
    Three,
    One,
}

/// How bright a cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Text,
    Dim,
    Faint,
}

/// A working motif.
pub(crate) struct Motif {
    /// how long a frame stays (≥ 100 ms: at most 10 frames a second)
    pub(crate) frame_ms: u64,
    /// header and divider: one string per frame, one char per cell
    pub(crate) strip: &'static [&'static str],
    /// the same, short on room
    pub(crate) short: &'static [&'static str],
    /// the panel's status cell (and the smallest form): one per frame
    pub(crate) breath: &'static [&'static str],
    /// no motion: this one glyph, in the text color, for every form
    pub(crate) still: (&'static str, &'static str),
    /// every glyph the frames use: (glyph, tone, ASCII form)
    pub(crate) glyphs: &'static [(&'static str, Tone, &'static str)],
}

/// W1, "gust" (user pick, working.html variant I): a puff crosses 5 cells
/// left to right, `≈` head (text), `∿` (text) `~` (dim) `·` (faint) tail,
/// then a calm; cell k at frame i = ramp[(i − k) mod 9], ramp = `≈∿~·` + 5
/// spaces; short: 3 cells, ramp `≈∿~·` + 3 spaces (cycle 7). The breath:
/// `· ~ ∿ ≈ ∿ ~`. ASCII `= ~ - .`. Still: `∿`.
pub(crate) const W1: Motif = Motif {
    frame_ms: 110,
    strip: &["≈    ", "∿≈   ", "~∿≈  ", "·~∿≈ ", " ·~∿≈", "  ·~∿", "   ·~", "    ·", "     "],
    short: &["≈  ", "∿≈ ", "~∿≈", "·~∿", " ·~", "  ·", "   "],
    breath: &["·", "~", "∿", "≈", "∿", "~"],
    still: ("∿", "~"),
    glyphs: &[("≈", Tone::Text, "="), ("∿", Tone::Text, "~"), ("~", Tone::Dim, "-"), ("·", Tone::Faint, ".")],
};

/// The motif the TUI draws.
pub(crate) const MOTIF: Motif = W1;

/// The header's gust on a screen `width` wide (book §9): 5 cells from 90
/// columns, 3 from 70, else the breath.
pub(crate) fn header_size(width: u16) -> Size {
    match width {
        90.. => Size::Five,
        70..=89 => Size::Three,
        _ => Size::One,
    }
}

/// The frame number `elapsed` after the start.
pub(crate) fn frame(elapsed: Duration) -> u64 {
    elapsed.as_millis() as u64 / MOTIF.frame_ms.max(1)
}

/// The frame number now (the clock starts at the first call).
pub(crate) fn clock() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    frame(START.get_or_init(std::time::Instant::now).elapsed())
}

/// A draw longer than this stops the motion (the redraw budget).
pub(crate) const DRAW_BUDGET: Duration = Duration::from_millis(60);

/// `BISE_REDUCE_MOTION` set (not empty, not `0`): no motion.
pub(crate) fn reduce_motion() -> bool {
    std::env::var("BISE_REDUCE_MOTION").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The motion for the next frame: moving unless the terminal lost the
/// focus, the last draw took longer than [`DRAW_BUDGET`] or the user
/// asked for less motion; calm while you type (`zen`).
pub(crate) fn motion(focus_lost: bool, last_draw: Duration, reduce: bool, zen: bool) -> Motion {
    if focus_lost || reduce || last_draw > DRAW_BUDGET {
        Motion::Still
    } else if zen {
        Motion::Calm(clock() / 2)
    } else {
        Motion::Frame(clock())
    }
}

/// One tone down (zen): text → dim → faint.
fn calmer(t: Tone) -> Tone {
    match t {
        Tone::Text => Tone::Dim,
        Tone::Dim | Tone::Faint => Tone::Faint,
    }
}

/// A glyph of the motif: its tone and the form to draw (ASCII or not); a
/// space (or anything unknown) is a blank cell.
fn look(m: &Motif, g: &str, ascii: bool) -> (&'static str, Tone) {
    match m.glyphs.iter().find(|(x, _, _)| *x == g) {
        Some((x, tone, a)) => (if ascii { a } else { x }, *tone),
        None => (" ", Tone::Faint),
    }
}

/// The cells of the `size` form at `motion`.
pub(crate) fn cells(m: &Motif, motion: Motion, size: Size, ascii: bool) -> Vec<(&'static str, Tone)> {
    let (i, calm) = match motion {
        Motion::Frame(i) => (i, false),
        Motion::Calm(i) => (i, true),
        Motion::Still => return vec![(if ascii { m.still.1 } else { m.still.0 }, Tone::Text)],
    };
    let frames = match size {
        Size::Five => m.strip,
        Size::Three => m.short,
        Size::One => m.breath,
    };
    if frames.is_empty() {
        return Vec::new();
    }
    let f = frames[(i % frames.len() as u64) as usize];
    let mut buf = [0u8; 4];
    f.chars()
        .map(|c| look(m, c.encode_utf8(&mut buf), ascii))
        .map(|(g, t)| (g, if calm { calmer(t) } else { t }))
        .collect()
}

fn color(t: Tone) -> Color {
    match t {
        Tone::Text => text(),
        Tone::Dim => dim(),
        Tone::Faint => faint(),
    }
}

/// The gust in `size` at `motion`, one span a cell, in the theme's colors.
pub(crate) fn mark(motion: Motion, size: Size) -> Vec<Span<'static>> {
    cells(&MOTIF, motion, size, theme::ascii_mode())
        .into_iter()
        .map(|(g, t)| Span::styled(g, Style::default().fg(color(t))))
        .collect()
}

/// The panel's cell (the breath) at `motion` and its color.
pub(crate) fn cell(motion: Motion) -> (&'static str, Color) {
    match cells(&MOTIF, motion, Size::One, theme::ascii_mode()).first() {
        Some((g, t)) => (g, color(*t)),
        None => (" ", faint()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn text_of(cells: &[(&str, Tone)]) -> String {
        cells.iter().map(|(g, _)| *g).collect()
    }

    fn at(i: u64, size: Size, ascii: bool) -> String {
        text_of(&cells(&W1, Motion::Frame(i), size, ascii))
    }

    #[test]
    fn w1_is_the_ramp_crossing_five_then_three_cells() {
        // book §9: cell k at frame i = ramp[(i − k) mod n]
        let ramp = |n: usize, j: i64| -> &str { ["≈", "∿", "~", "·"].get(j.rem_euclid(n as i64) as usize).copied().unwrap_or(" ") };
        for i in 0..18u64 {
            let five: String = (0..5).map(|k| ramp(9, i as i64 - k)).collect();
            assert_eq!(at(i, Size::Five, false), five, "frame {i}");
            let three: String = (0..3).map(|k| ramp(7, i as i64 - k)).collect();
            assert_eq!(at(i, Size::Three, false), three, "frame {i}");
        }
        assert_eq!((W1.strip.len(), W1.short.len(), W1.breath.len()), (9, 7, 6));
    }

    #[test]
    fn every_frame_has_its_size() {
        for i in 0..63u64 {
            for ascii in [false, true] {
                assert_eq!(at(i, Size::Five, ascii).width(), 5, "frame {i} ascii {ascii}");
                assert_eq!(at(i, Size::Three, ascii).width(), 3, "frame {i} ascii {ascii}");
                assert_eq!(at(i, Size::One, ascii).width(), 1, "frame {i} ascii {ascii}");
            }
        }
    }

    #[test]
    fn tones_ascii_and_breath() {
        let full = cells(&W1, Motion::Frame(4), Size::Five, false);
        assert_eq!(text_of(&full), " ·~∿≈");
        let tones: Vec<Tone> = full.iter().map(|c| c.1).collect();
        assert_eq!(tones[1..], [Tone::Faint, Tone::Dim, Tone::Text, Tone::Text]);
        assert_eq!(at(4, Size::Five, true), " .-~=");
        let breath: String = (0..6).map(|i| at(i, Size::One, false)).collect();
        assert_eq!(breath, "·~∿≈∿~");
        let breath: String = (0..6).map(|i| at(i, Size::One, true)).collect();
        assert_eq!(breath, ".-~=~-");
    }

    #[test]
    fn still_is_one_static_wave_in_every_size() {
        for size in [Size::Five, Size::Three, Size::One] {
            assert_eq!(cells(&W1, Motion::Still, size, false), vec![("∿", Tone::Text)]);
            assert_eq!(cells(&W1, Motion::Still, size, true), vec![("~", Tone::Text)]);
        }
    }

    #[test]
    fn motion_stops_on_focus_loss_slow_draws_and_the_env() {
        let fast = Duration::from_millis(5);
        assert!(matches!(motion(false, fast, false, false), Motion::Frame(_)));
        assert_eq!(motion(true, fast, false, false), Motion::Still);
        assert_eq!(motion(false, fast, true, false), Motion::Still);
        assert_eq!(motion(false, DRAW_BUDGET + Duration::from_millis(1), false, false), Motion::Still);
        // zen (BISE-121): calm, and still wins over it
        assert!(matches!(motion(false, fast, false, true), Motion::Calm(_)));
        assert_eq!(motion(false, fast, true, true), Motion::Still);
    }

    #[test]
    fn calm_is_the_same_frames_one_tone_down() {
        for i in 0..18u64 {
            for size in [Size::Five, Size::Three, Size::One] {
                let (f, c) = (cells(&W1, Motion::Frame(i), size, false), cells(&W1, Motion::Calm(i), size, false));
                assert_eq!(text_of(&f), text_of(&c));
                for ((_, a), (_, b)) in f.iter().zip(&c) {
                    assert_eq!(*b, calmer(*a));
                    assert_ne!(*b, Tone::Text, "no full-text cell in zen");
                }
            }
        }
    }

    #[test]
    fn the_header_gust_narrows_with_the_screen() {
        // book §9: F ≥ 90 5 cells, 70–89 3 cells, < 70 the breath
        assert_eq!([header_size(120), header_size(90)], [Size::Five, Size::Five]);
        assert_eq!([header_size(89), header_size(70)], [Size::Three, Size::Three]);
        assert_eq!([header_size(69), header_size(40)], [Size::One, Size::One]);
    }

    #[test]
    fn at_most_ten_frames_a_second() {
        const { assert!(MOTIF.frame_ms >= 100) };
        assert_eq!(frame(Duration::from_millis(109)), 0);
        assert_eq!(frame(Duration::from_millis(110)), 1);
        assert_eq!(frame(Duration::from_secs(1)), 9);
    }
}
