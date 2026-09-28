//! The bise theme (book §5-6, contract C1): roles, not colors.
//!
//! Two palettes, dark and light, switched by [`set_mode`] (default dark).
//! We never paint the background: every role is a foreground color, the
//! background is `Color::Reset` (the user's terminal shows through). The
//! only tints are [`selection_bg`] and [`card_tint`].
//!
//! Color means attention: only "needs you" (accent) and errors get a hue;
//! everything else is text, dim or faint. `faint` is never for text you
//! must read.
//!
//! The old OpenCode constants (`BRAND`, `DIM`, …) stay as `#[deprecated]`
//! aliases of the dark palette until BISE-83; new code calls the roles.

// the roles and §6 glyphs land before their users (wave 1 migrates the
// feed, the panel, the cards): no dead-code noise until then
#![allow(dead_code)]

use ratatui::style::Color;
use ratatui::symbols::border;

// ---- mode ----

/// Which palette the roles read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Dark,
    Light,
}

#[cfg(not(test))]
mod mode_cell {
    //! The process-wide mode: detection (BISE-02) may run on any thread.
    use super::Mode;
    use std::sync::atomic::{AtomicBool, Ordering};

    static LIGHT: AtomicBool = AtomicBool::new(false);

    pub(super) fn set(m: Mode) {
        LIGHT.store(m == Mode::Light, Ordering::Relaxed);
    }
    pub(super) fn get() -> Mode {
        if LIGHT.load(Ordering::Relaxed) {
            Mode::Light
        } else {
            Mode::Dark
        }
    }
}

#[cfg(test)]
mod mode_cell {
    //! Under `cargo test` the mode is per thread (each test runs on its own
    //! thread), so a test that switches to light never repaints a render
    //! test running next to it.
    use super::Mode;
    use std::cell::Cell;

    thread_local!(static MODE: Cell<Mode> = const { Cell::new(Mode::Dark) });

    pub(super) fn set(m: Mode) {
        MODE.with(|c| c.set(m));
    }
    pub(super) fn get() -> Mode {
        MODE.with(|c| c.get())
    }
}

/// Switch every role to the dark or the light palette.
pub(crate) fn set_mode(m: Mode) {
    mode_cell::set(m);
}

/// The current mode (default dark).
pub(crate) fn mode() -> Mode {
    mode_cell::get()
}

// ---- palettes ----

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// One palette: the value of every role in one mode.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Palette {
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub error: Color,
    pub ok: Color,
    /// text drawn on an accent background (a chip, the popup selection)
    pub on_accent: Color,
    pub selection_bg: Color,
    pub card_tint: Color,
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_comment: Color,
    pub syntax_number: Color,
    pub syntax_call: Color,
    pub syntax_type: Color,
}

/// Dark: for a dark terminal (checked on black, `#141211`, `#282c34`).
pub(crate) const DARK: Palette = Palette {
    text: rgb(0xece6da),
    dim: rgb(0xa39c90),
    faint: rgb(0x4a4540),
    accent: rgb(0xf4a6b0), // pale pink
    error: rgb(0xff5a52),
    ok: rgb(0xb9d99a),
    on_accent: rgb(0x1b1917),
    selection_bg: rgb(0x33292c),
    card_tint: rgb(0x211d1b),
    syntax_keyword: rgb(0xd7a6f0),
    syntax_string: rgb(0xb9d99a),
    // the book's #857e74 is 3.5:1 on #282c34: lifted to pass 4.5:1
    syntax_comment: rgb(0x99928a),
    syntax_number: rgb(0xf0b27a),
    syntax_call: rgb(0x8fc4f0),
    syntax_type: rgb(0xe8cf9a),
};

/// Light: for a light terminal (checked on white and our cream `#f7f4ee`).
pub(crate) const LIGHT: Palette = Palette {
    text: rgb(0x1b1917),
    dim: rgb(0x6b645a),
    faint: rgb(0xcfc8bd),
    accent: rgb(0xb8416b), // raspberry
    error: rgb(0xb3261e),
    ok: rgb(0x3f7a2a),
    on_accent: rgb(0xffffff),
    selection_bg: rgb(0xfaeef0),
    card_tint: rgb(0xf3eee6),
    syntax_keyword: rgb(0x8a3fb0),
    syntax_string: rgb(0x44782a),
    syntax_comment: rgb(0x726b60),
    syntax_number: rgb(0x9a4a0c),
    syntax_call: rgb(0x1f63a8),
    syntax_type: rgb(0x7a5c00),
};

/// The palette of a mode.
pub(crate) const fn palette_of(m: Mode) -> &'static Palette {
    match m {
        Mode::Dark => &DARK,
        Mode::Light => &LIGHT,
    }
}

/// The palette of the current mode.
pub(crate) fn palette() -> &'static Palette {
    palette_of(mode())
}

// ---- roles ----

/// Everything you read.
pub(crate) fn text() -> Color {
    palette().text
}
/// Secondary text, level 3, durations.
pub(crate) fn dim() -> Color {
    palette().dim
}
/// Rails, borders, numbers. Never for text you must read.
pub(crate) fn faint() -> Color {
    palette().faint
}
/// The `:*`, "needs you", the agent you talk to, `✓✓` read.
pub(crate) fn accent() -> Color {
    palette().accent
}
/// Failures only.
pub(crate) fn error() -> Color {
    palette().error
}
/// Diff additions only.
pub(crate) fn ok() -> Color {
    palette().ok
}
/// Text on an accent background (a chip, a selected popup row).
pub(crate) fn on_accent() -> Color {
    palette().on_accent
}
/// The background: never painted, the terminal's own shows through.
pub(crate) fn bg() -> Color {
    Color::Reset
}
/// The light tint under selected text.
pub(crate) fn selection_bg() -> Color {
    palette().selection_bg
}
/// The light tint of the card box.
pub(crate) fn card_tint() -> Color {
    palette().card_tint
}
pub(crate) fn syntax_keyword() -> Color {
    palette().syntax_keyword
}
pub(crate) fn syntax_string() -> Color {
    palette().syntax_string
}
pub(crate) fn syntax_comment() -> Color {
    palette().syntax_comment
}
pub(crate) fn syntax_number() -> Color {
    palette().syntax_number
}
/// Function and method calls.
pub(crate) fn syntax_call() -> Color {
    palette().syntax_call
}
pub(crate) fn syntax_type() -> Color {
    palette().syntax_type
}

// ---- glyphs (book §6): one glyph per entity and per status ----

// entities
pub(crate) const G_YOU: &str = "›"; // you, and the composer prompt
pub(crate) const G_MAIN: &str = ":*"; // main (accent)
pub(crate) const G_BRIEF: &str = "◇"; // an agent's brief
pub(crate) const G_THINK: &str = "∴"; // thinking (dim)
pub(crate) const G_BASH: &str = "$"; // a bash call
pub(crate) const G_TS: &str = "λ"; // a TypeScript call
pub(crate) const G_SUBCALL: &str = "↳"; // a sub-call inside a TypeScript run
pub(crate) const G_PATCH: &str = "±"; // a file edit
pub(crate) const G_MSG: &str = "✉"; // a message between agents, or to you
pub(crate) const G_IMAGE: &str = "▣"; // an image (accent chip)
pub(crate) const G_CARD: &str = "?"; // a card: a decision that needs you (accent)
pub(crate) const G_COMPACTING: &str = "⟳"; // compaction running (dim)
pub(crate) const G_SUMMARY: &str = "≡"; // compaction summary (dim)
pub(crate) const G_INTERRUPTED: &str = "▲"; // turn interrupted (dim)

// agent status
pub(crate) const G_STARTING: &str = "·"; // dim, pulsing
pub(crate) const G_WORKING: &str = "∿"; // pulsing: a breeze
pub(crate) const G_WAITING: &str = "…"; // waiting on another agent
pub(crate) const G_NEEDS_YOU: &str = "?"; // accent
pub(crate) const G_DONE: &str = "♡";
pub(crate) const G_FAILED: &str = "✗"; // error
pub(crate) const G_IDLE: &str = "○"; // dim
pub(crate) const G_STOPPED: &str = "–"; // dim

// marks
pub(crate) const G_SENDING: &str = "·"; // your message: sending
pub(crate) const G_RECEIVED: &str = "✓"; // the agent got it
pub(crate) const G_READ: &str = "✓✓"; // the model read it (accent)
pub(crate) const G_UNREAD: &str = "•"; // unread activity (accent)
pub(crate) const G_WORKTREE: &str = "⎇"; // the agent has its own worktree
pub(crate) const G_OVERLAP: &str = "⇄"; // two agents changed the same file
pub(crate) const G_RESTART_FAILED: &str = "↻"; // error
pub(crate) const G_BUILDING: &str = "⧗"; // a version building or on trial
pub(crate) const G_CLOSED: &str = "▸"; // progressive disclosure: closed
pub(crate) const G_OPEN: &str = "▾"; // progressive disclosure: open

/// The working pulse: `∿` in text, then dim, then text… (one phase every
/// 4 ticks). Replaces the braille spinner.
pub(crate) fn working_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { text() } else { dim() };
    (G_WORKING, color)
}

/// The starting pulse: `·`, dim then faint, same rhythm as [`working_frame`].
pub(crate) fn starting_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { dim() } else { faint() };
    (G_STARTING, color)
}

// the prompt/autocomplete borders: only a vertical bar
pub(crate) const SPLIT: border::Set = border::Set {
    top_left: "",
    top_right: "",
    bottom_left: "",
    bottom_right: "",
    vertical_left: "┃",
    vertical_right: "┃",
    horizontal_top: " ",
    horizontal_bottom: " ",
};

// ---- deprecated aliases (until BISE-83) ----
// Fixed to the dark palette: they do not follow `set_mode`. Each names
// the role to use instead.

#[deprecated(note = "BISE-01: use theme::accent()")]
pub(crate) const BRAND: Color = DARK.accent;
#[deprecated(note = "BISE-01: headings are theme::text() + bold")]
pub(crate) const ACCENT: Color = DARK.text;
#[deprecated(note = "BISE-01: use theme::text() (emphasis) or theme::syntax_type() (code)")]
pub(crate) const HEAD: Color = DARK.text;
#[deprecated(note = "BISE-01: use theme::dim()")]
pub(crate) const INFO: Color = DARK.dim;
#[deprecated(note = "BISE-01: use theme::text()")]
pub(crate) const TEXT: Color = DARK.text;
#[deprecated(note = "BISE-01: use theme::dim()")]
pub(crate) const DIM: Color = DARK.dim;
#[deprecated(note = "BISE-01: use theme::dim()")]
pub(crate) const TOOL: Color = DARK.dim;
#[deprecated(note = "BISE-01: use theme::ok()")]
pub(crate) const OK: Color = DARK.ok;
#[deprecated(note = "BISE-01: use theme::accent() (needs you) or theme::dim()")]
pub(crate) const WARN: Color = DARK.accent;
#[deprecated(note = "BISE-01: use theme::error()")]
pub(crate) const ERR: Color = DARK.error;
#[deprecated(note = "BISE-01: never paint the background (theme::bg())")]
pub(crate) const PANEL: Color = Color::Reset;
#[deprecated(note = "BISE-01: use theme::on_accent()")]
pub(crate) const ON_BRAND: Color = DARK.on_accent;
#[deprecated(note = "BISE-01: never paint the background (theme::bg())")]
pub(crate) const ELEMENT: Color = Color::Reset;
#[deprecated(note = "BISE-01: use theme::accent()")]
pub(crate) const RECORDING: Color = DARK.accent;
#[deprecated(note = "BISE-01: use theme::selection_bg()")]
pub(crate) const SELECTION: Color = DARK.selection_bg;
#[deprecated(note = "BISE-01: use theme::faint()")]
pub(crate) const BORDER_ACTIVE: Color = DARK.faint;
#[deprecated(note = "BISE-01: use theme::faint()")]
pub(crate) const FAINT: Color = DARK.faint;
#[deprecated(note = "BISE-01: use theme::syntax_keyword()")]
pub(crate) const SYNTAX_KEYWORD: Color = DARK.syntax_keyword;
#[deprecated(note = "BISE-01: use theme::syntax_string()")]
pub(crate) const SYNTAX_STRING: Color = DARK.syntax_string;
#[deprecated(note = "BISE-01: use theme::syntax_comment()")]
pub(crate) const SYNTAX_COMMENT: Color = DARK.syntax_comment;
#[deprecated(note = "BISE-01: use theme::syntax_number()")]
pub(crate) const SYNTAX_NUMBER: Color = DARK.syntax_number;
#[deprecated(note = "BISE-01: use theme::syntax_call()")]
pub(crate) const SYNTAX_FUNC: Color = DARK.syntax_call;
#[deprecated(note = "BISE-01: never paint the background; additions are theme::ok()")]
pub(crate) const DIFF_ADD_BG: Color = Color::Reset;
#[deprecated(note = "BISE-01: never paint the background; deletions are theme::error()")]
pub(crate) const DIFF_DEL_BG: Color = Color::Reset;

#[deprecated(note = "BISE-01: use theme::working_frame()")]
pub(crate) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[deprecated(note = "BISE-01: use theme::working_frame()")]
#[allow(deprecated)]
pub(crate) fn spinner_frame(tick: u32) -> &'static str {
    SPINNER[(tick as usize) % SPINNER.len()]
}

// the old feed glyphs: kept as they are (BISE-13 moves the feed to §6)
#[deprecated(note = "BISE-01: use theme::G_THINK")]
pub(crate) const GLYPH_THINK: &str = "✦";
#[deprecated(note = "BISE-01: use theme::G_RECEIVED / G_DONE")]
pub(crate) const GLYPH_OK: &str = "✓";
#[deprecated(note = "BISE-01: use theme::G_FAILED")]
pub(crate) const GLYPH_ERR: &str = "✗";
#[deprecated(note = "BISE-01: use theme::G_INTERRUPTED")]
pub(crate) const GLYPH_WARN: &str = "▲";
#[deprecated(note = "BISE-01: use a §6 glyph")]
pub(crate) const GLYPH_INFO: &str = "·";
#[deprecated(note = "BISE-01: use theme::G_COMPACTING")]
pub(crate) const GLYPH_COMPACT: &str = "⟳";
#[deprecated(note = "BISE-01: use theme::G_SUMMARY")]
pub(crate) const GLYPH_SUMMARY: &str = "≡";
#[deprecated(note = "BISE-01: use theme::G_SUBCALL")]
pub(crate) const GLYPH_BRANCH: &str = "↳";
#[deprecated(note = "BISE-01: use theme::faint() + '│'")]
pub(crate) const GLYPH_RAIL: &str = "│";

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Color) -> f64 {
        match c {
            Color::Rgb(r, g, b) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b),
            other => panic!("not an rgb color: {other:?}"),
        }
    }

    /// WCAG 2 contrast ratio.
    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Every role you must be able to read.
    fn readable(p: &Palette) -> [(&'static str, Color); 11] {
        [
            ("text", p.text),
            ("dim", p.dim),
            ("accent", p.accent),
            ("error", p.error),
            ("ok", p.ok),
            ("syntax_keyword", p.syntax_keyword),
            ("syntax_string", p.syntax_string),
            ("syntax_comment", p.syntax_comment),
            ("syntax_number", p.syntax_number),
            ("syntax_call", p.syntax_call),
            ("syntax_type", p.syntax_type),
        ]
    }

    fn check(p: &Palette, backgrounds: &[(&str, Color)]) -> Vec<String> {
        let mut fails = vec![];
        for (bg_name, bg) in backgrounds {
            for (name, fg) in readable(p) {
                let r = contrast(fg, *bg);
                if r < 4.5 {
                    fails.push(format!("{name} on {bg_name}: {r:.2}"));
                }
            }
        }
        fails
    }

    #[test]
    fn dark_roles_read_on_dark_backgrounds() {
        let bgs = [("black", rgb(0x000000)), ("#141211", rgb(0x141211)), ("#282c34", rgb(0x282c34))];
        let fails = check(&DARK, &bgs);
        assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
    }

    #[test]
    fn light_roles_read_on_light_backgrounds() {
        let bgs = [("white", rgb(0xffffff)), ("#f7f4ee", rgb(0xf7f4ee))];
        let fails = check(&LIGHT, &bgs);
        assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
    }

    #[test]
    fn text_reads_on_the_tints() {
        for p in [&DARK, &LIGHT] {
            for (tint_name, tint) in [("selection", p.selection_bg), ("card", p.card_tint)] {
                for (name, fg) in [("text", p.text), ("dim", p.dim), ("accent", p.accent)] {
                    let r = contrast(fg, tint);
                    assert!(r >= 4.5, "{name} on {tint_name} tint: {r:.2}");
                }
            }
            let r = contrast(p.on_accent, p.accent);
            assert!(r >= 4.5, "on_accent on accent: {r:.2}");
        }
    }

    #[test]
    fn faint_is_quieter_than_dim() {
        // faint sits between dim and the background, in both modes
        let dark_bg = rgb(0x141211);
        assert!(contrast(DARK.faint, dark_bg) < contrast(DARK.dim, dark_bg));
        let light_bg = rgb(0xf7f4ee);
        assert!(contrast(LIGHT.faint, light_bg) < contrast(LIGHT.dim, light_bg));
    }

    fn roles() -> Vec<Color> {
        vec![
            text(),
            dim(),
            faint(),
            accent(),
            error(),
            ok(),
            on_accent(),
            selection_bg(),
            card_tint(),
            syntax_keyword(),
            syntax_string(),
            syntax_comment(),
            syntax_number(),
            syntax_call(),
            syntax_type(),
        ]
    }

    #[test]
    fn set_mode_switches_every_role() {
        assert_eq!(mode(), Mode::Dark, "default is dark");
        let dark = roles();
        set_mode(Mode::Light);
        assert_eq!(mode(), Mode::Light);
        let light = roles();
        set_mode(Mode::Dark);
        assert_eq!(roles(), dark);
        for (i, (d, l)) in dark.iter().zip(&light).enumerate() {
            assert_ne!(d, l, "role #{i} is the same in both modes");
        }
        assert_eq!(bg(), Color::Reset);
    }

    #[test]
    fn working_pulses_between_text_and_dim() {
        let frames: Vec<_> = (0..8).map(working_frame).collect();
        assert!(frames.iter().all(|(g, _)| *g == G_WORKING));
        assert_eq!(frames[0].1, text());
        assert_eq!(frames[4].1, dim());
    }

    #[test]
    #[allow(deprecated)]
    fn aliases_follow_the_dark_roles() {
        assert_eq!(BRAND, DARK.accent);
        assert_eq!(WARN, DARK.accent);
        assert_eq!(ERR, DARK.error);
        assert_eq!(DIM, DARK.dim);
        assert_eq!(PANEL, Color::Reset);
        assert_eq!(ELEMENT, Color::Reset);
    }
}
