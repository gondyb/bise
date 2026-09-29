//! The voice chip (BISE-222): while you record and while the clip is
//! transcribed, one atomic chip in the composer text at the cursor; the
//! transcript replaces it in place. Same pill as the quote and image
//! chips (BISE-205): one padding cell each side on the `pill` tint, or
//! brackets without a tint (`NO_COLOR`, `BISE_ASCII=1`).
//!
//! - recording ` ● ▂▅▃▆▂▃ 0:07 `: the `●` blinks accent/dim every
//!   [`BLINK_MS`] (bold on/off under `NO_COLOR`), the bars are the last
//!   [`BARS`] live levels (one per [`METER_BLOCK`] of audio, the newest on
//!   the right) in accent, the timer in the text color;
//! - transcribing ` ∿ ▃▅▆▅▃▂ 0:07 `: same width; the bars roll the
//!   [`WAVE`] one cell every [`WAVE_MS`], dim; the timer holds the clip's
//!   length;
//! - ASCII: `[* _.-=#- 0:07]`, `[~ .-=#=- 0:07]`;
//! - narrow (less than [`FULL_FROM`] columns of text): no timer, then
//!   (less than [`BARS_FROM`]) 3 bars.
//!
//! The frames come from the animation clock's pulse time (anim.rs,
//! BISE-204): they hold while you type (zen) and stand still under
//! `BISE_REDUCE_MOTION`. The chip's text in the composer is [`LABEL`].

use super::{peak_glyph, PEAK_BLOCKS, SAMPLE_RATE};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// The chip in the composer text (attach::chips finds it by [`OPEN`]).
pub(crate) const LABEL: &str = "[Voice #1]";
pub(crate) const OPEN: &str = "[Voice #";
/// The bars of the chip.
pub(crate) const BARS: usize = 6;
/// One level of the meter per 100 ms of audio.
pub(crate) const METER_BLOCK: usize = SAMPLE_RATE as usize / 10;
/// The `●` blinks: on, then off, this long each.
pub(crate) const BLINK_MS: u64 = 600;
/// The transcribing wave moves one cell this often.
pub(crate) const WAVE_MS: u64 = 120;
/// The rolling wave (indices in [`PEAK_BLOCKS`]): `▂▃▅▆▅▃`.
pub(crate) const WAVE: [usize; BARS] = [1, 2, 4, 5, 4, 2];
/// The text columns from which the chip shows its timer, its 6 bars.
pub(crate) const FULL_FROM: usize = 40;
pub(crate) const BARS_FROM: usize = 20;
/// A [`PEAK_BLOCKS`] level in ASCII (`_ . - = #`).
const ASCII_LEVELS: [char; 8] = ['_', '.', '-', '-', '=', '#', '#', '#'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Recording,
    Transcribing,
}

/// What the chip shows in `inner` columns of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    Full,
    NoTimer,
    ThreeBars,
}

pub(crate) fn fit(inner: usize) -> Fit {
    if inner >= FULL_FROM {
        Fit::Full
    } else if inner >= BARS_FROM {
        Fit::NoTimer
    } else {
        Fit::ThreeBars
    }
}

/// The chip's width: the same while recording and transcribing, in
/// every form (no layout shift).
pub(crate) fn width(inner: usize) -> usize {
    match fit(inner) {
        Fit::Full => 15,
        Fit::NoTimer => 10,
        Fit::ThreeBars => 7,
    }
}

/// One frame of the chip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) phase: Phase,
    /// the live levels, oldest first (recording)
    pub(crate) levels: [f32; BARS],
    /// the recording's length so far, or the clip's
    pub(crate) secs: u64,
    /// the clock's pulse time (ms): the blink and the wave
    pub(crate) ms: u64,
    /// no motion (`BISE_REDUCE_MOTION`, a slow draw, no focus)
    pub(crate) still: bool,
}

/// How the chip is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Form {
    /// the pill tint (else brackets)
    pub(crate) tinted: bool,
    pub(crate) ascii: bool,
    /// `NO_COLOR`: the blink is bold on/off
    pub(crate) no_color: bool,
}

impl Form {
    pub(crate) fn now() -> Form {
        Form {
            tinted: crate::render::chip_form() == crate::render::ChipForm::Tinted,
            ascii: crate::theme::ascii_mode(),
            no_color: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
        }
    }
}

/// The `●` is lit at this frame.
pub(crate) fn lit(look: &Look) -> bool {
    look.still || (look.ms / BLINK_MS).is_multiple_of(2)
}

fn level_char(i: usize, ascii: bool) -> char {
    let i = i.min(PEAK_BLOCKS.len() - 1);
    if ascii {
        ASCII_LEVELS[i]
    } else {
        PEAK_BLOCKS[i]
    }
}

/// The bars of `look` for `fit`.
pub(crate) fn bars(look: &Look, fit: Fit, ascii: bool) -> String {
    let all: Vec<char> = match look.phase {
        Phase::Recording => look
            .levels
            .iter()
            .map(|&p| {
                let g = peak_glyph(p);
                level_char(PEAK_BLOCKS.iter().position(|&b| b == g).unwrap_or(0), ascii)
            })
            .collect(),
        Phase::Transcribing => {
            let k = if look.still { 0 } else { (look.ms / WAVE_MS) as usize };
            (0..BARS).map(|i| level_char(WAVE[(i + k) % BARS], ascii)).collect()
        }
    };
    match (fit, look.phase) {
        (Fit::ThreeBars, Phase::Recording) => all[BARS - 3..].iter().collect(),
        (Fit::ThreeBars, Phase::Transcribing) => all[..3].iter().collect(),
        _ => all.into_iter().collect(),
    }
}

fn timer(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// What replaces the chip between `before` and `after` (the chars
/// around it): the transcript `t` (trimmed), a space before it when it
/// would touch a word or a `.`/`:` (not an opening bracket or quote), one after it when a word follows; and the
/// cursor in it (the transcript's end).
pub(crate) fn landing(before: Option<char>, t: &str, after: Option<char>) -> (String, usize) {
    let t = t.trim();
    if t.is_empty() {
        return (String::new(), 0);
    }
    // after a word or a `.`/`:`, not after an opening `(` or quote
    let pre = before.is_some_and(|c| !c.is_whitespace() && !"([{<\"'«“‘".contains(c))
        && t.starts_with(char::is_alphanumeric);
    let post = after.is_some_and(char::is_alphanumeric);
    let with = format!("{}{t}{}", if pre { " " } else { "" }, if post { " " } else { "" });
    (with, usize::from(pre) + t.chars().count())
}

fn glyph(phase: Phase, ascii: bool) -> &'static str {
    match (phase, ascii) {
        (Phase::Recording, false) => "●",
        (Phase::Recording, true) => "*",
        (Phase::Transcribing, false) => "∿",
        (Phase::Transcribing, true) => "~",
    }
}

/// The chip's spans: `over` is patched on every one (the selection's
/// background, the cursor's REVERSED), so the whole pill takes it.
pub(crate) fn spans(look: &Look, fit: Fit, form: Form, over: Style) -> Vec<Span<'static>> {
    use crate::theme::{accent, dim, pill_bg, text};
    let (open, close) = if form.tinted { (" ", " ") } else { ("[", "]") };
    let st = |fg| {
        let s = Style::default().fg(fg);
        if form.tinted { s.bg(pill_bg()) } else { s }.patch(over)
    };
    let on = lit(look);
    let g_style = match look.phase {
        Phase::Recording if form.no_color => {
            if on {
                st(accent()).add_modifier(Modifier::BOLD)
            } else {
                st(accent())
            }
        }
        Phase::Recording => st(if on { accent() } else { dim() }),
        Phase::Transcribing => st(accent()),
    };
    let bar_fg = if look.phase == Phase::Recording { accent() } else { dim() };
    let mut out = vec![
        Span::styled(open, st(dim())),
        Span::styled(glyph(look.phase, form.ascii), g_style),
        Span::styled(" ", st(text())),
        Span::styled(bars(look, fit, form.ascii), st(bar_fg)),
    ];
    if fit == Fit::Full {
        out.push(Span::styled(format!(" {}", timer(look.secs)), st(text())));
    }
    out.push(Span::styled(close, st(dim())));
    out
}

/// The chip as text (the tests).
#[cfg(test)]
pub(crate) fn text(look: &Look, fit: Fit, form: Form) -> String {
    spans(look, fit, form, Style::default()).iter().map(|s| s.content.as_ref()).collect()
}

/// The live levels: the loudest sample of each [`METER_BLOCK`] of audio,
/// the last [`BARS`] of them (oldest first); they scroll left one cell
/// per block.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Meter {
    levels: [f32; BARS],
    block_max: f32,
    block_n: usize,
}

impl Meter {
    pub(crate) fn push(&mut self, samples: &[i16]) {
        for &s in samples {
            self.block_max = self.block_max.max((s as f32 / i16::MAX as f32).abs());
            self.block_n += 1;
            if self.block_n >= METER_BLOCK {
                self.levels.rotate_left(1);
                self.levels[BARS - 1] = self.block_max.min(1.0);
                self.block_max = 0.0;
                self.block_n = 0;
            }
        }
    }

    pub(crate) fn levels(&self) -> [f32; BARS] {
        self.levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    const COLOR: Form = Form { tinted: true, ascii: false, no_color: false };
    const NO_COLOR: Form = Form { tinted: false, ascii: false, no_color: true };
    const ASCII: Form = Form { tinted: false, ascii: true, no_color: false };

    fn rec(ms: u64) -> Look {
        // ▂▅▃▆▂▃
        Look { phase: Phase::Recording, levels: [0.2, 0.5, 0.3, 0.7, 0.2, 0.3], secs: 7, ms, still: false }
    }

    fn tr(ms: u64) -> Look {
        Look { phase: Phase::Transcribing, ms, ..rec(0) }
    }

    #[test]
    fn the_forms_of_the_chip() {
        assert_eq!(text(&rec(0), Fit::Full, COLOR), " ● ▂▅▃▆▂▃ 0:07 ");
        assert_eq!(text(&tr(0), Fit::Full, COLOR), " ∿ ▂▃▅▆▅▃ 0:07 ");
        assert_eq!(text(&rec(0), Fit::Full, NO_COLOR), "[● ▂▅▃▆▂▃ 0:07]");
        assert_eq!(text(&rec(0), Fit::Full, ASCII), "[* .=-#.- 0:07]");
        assert_eq!(text(&tr(0), Fit::Full, ASCII), "[~ .-=#=- 0:07]");
        assert_eq!(text(&rec(0), Fit::NoTimer, COLOR), " ● ▂▅▃▆▂▃ ");
        assert_eq!(text(&rec(0), Fit::ThreeBars, COLOR), " ● ▆▂▃ ");
        assert_eq!(text(&tr(0), Fit::ThreeBars, ASCII), "[~ .-=]");
        // the same width while recording and transcribing, in every form
        for (inner, f) in [(80, Fit::Full), (39, Fit::NoTimer), (19, Fit::ThreeBars)] {
            assert_eq!(fit(inner), f);
            for form in [COLOR, NO_COLOR, ASCII] {
                for look in [rec(0), tr(0), rec(700), tr(360)] {
                    assert_eq!(text(&look, f, form).width(), width(inner), "{look:?} {form:?}");
                }
            }
        }
        // a long recording: 5:00 at most, still 4 cells
        assert_eq!(text(&Look { secs: 300, ..rec(0) }, Fit::Full, COLOR), " ● ▂▅▃▆▂▃ 5:00 ");
    }

    #[test]
    fn the_frames_come_from_the_clock_time() {
        use crate::theme::{accent, dim};
        // the dot: on for 600 ms, off for 600 ms
        let dot = |ms, form| spans(&rec(ms), Fit::Full, form, Style::default())[1].style;
        assert_eq!(dot(0, COLOR).fg, Some(accent()));
        assert_eq!(dot(599, COLOR).fg, Some(accent()));
        assert_eq!(dot(600, COLOR).fg, Some(dim()));
        assert_eq!(dot(1200, COLOR).fg, Some(accent()));
        // NO_COLOR: bold on/off
        assert!(dot(0, NO_COLOR).add_modifier.contains(Modifier::BOLD));
        assert!(!dot(600, NO_COLOR).add_modifier.contains(Modifier::BOLD));
        // the wave rolls one cell every 120 ms
        assert_eq!(bars(&tr(0), Fit::Full, false), "▂▃▅▆▅▃");
        assert_eq!(bars(&tr(119), Fit::Full, false), "▂▃▅▆▅▃");
        assert_eq!(bars(&tr(120), Fit::Full, false), "▃▅▆▅▃▂");
        assert_eq!(bars(&tr(720), Fit::Full, false), "▂▃▅▆▅▃");
        // still (BISE_REDUCE_MOTION): no blink, no roll; the meter lives
        let still = |l: Look| Look { still: true, ..l };
        assert!(lit(&still(rec(600))));
        assert_eq!(bars(&still(tr(120)), Fit::Full, false), "▂▃▅▆▅▃");
        assert_eq!(bars(&still(rec(600)), Fit::Full, false), "▂▅▃▆▂▃");
    }

    #[test]
    fn the_transcript_takes_one_space_where_it_touches_a_word() {
        let l = |b, t, a| landing(b, t, a);
        assert_eq!(l(None, " Hello world. ", None), ("Hello world.".into(), 12));
        assert_eq!(l(Some('e'), "Hi", Some('t')), (" Hi ".into(), 3));
        assert_eq!(l(Some(' '), "Hi", Some(' ')), ("Hi".into(), 2));
        // punctuation after the word before, before a comma: no space
        assert_eq!(l(Some('e'), ".", Some(',')), (".".into(), 1));
        assert_eq!(l(Some('('), "Hi", Some(')')), ("Hi".into(), 2));
        assert_eq!(l(Some('a'), "   ", Some('b')), (String::new(), 0));
    }

    #[test]
    fn the_meter_scrolls_left_one_level_per_block() {
        let mut m = Meter::default();
        assert_eq!(m.levels(), [0.0; BARS]);
        // half a block: nothing yet
        m.push(&vec![i16::MAX / 2; METER_BLOCK / 2]);
        assert_eq!(m.levels(), [0.0; BARS]);
        // the block's loudest sample lands on the right
        m.push(&vec![i16::MAX; METER_BLOCK / 2]);
        assert_eq!(m.levels()[BARS - 1], 1.0);
        m.push(&vec![0; METER_BLOCK]);
        let l = m.levels();
        assert_eq!((l[BARS - 2], l[BARS - 1]), (1.0, 0.0));
        // many blocks in one push (a late callback): each one counts
        m.push(&vec![i16::MAX / 4; METER_BLOCK * 3]);
        assert!(m.levels()[BARS - 5] == 1.0 && m.levels()[BARS - 1] > 0.24);
    }
}
