//! The bise theme (book §5-6, contract C1): roles, not colors.
//!
//! Two palettes, dark and light, switched by [`set_mode`] (default dark).
//! BISE-92: bise paints its own background ([`bg`], the palette's ground)
//! on every cell, so the text reads whatever the terminal's colors or a
//! wrong theme pick: [`paint`] turns every cell left at `Color::Reset`
//! into the theme's ground and text, once per frame. The tints on that
//! ground are [`selection_bg`], [`card_tint`] and [`raised`] (the
//! composer pane, BISE-102).
//!
//! Color means attention: only "needs you" (accent) and errors get a hue;
//! everything else is text, dim or faint. `faint` is never for text you
//! must read.
//!
//! The old OpenCode constants (`BRAND`, `DIM`, …) were `#[deprecated]`
//! aliases of the dark palette until BISE-83; new code calls the roles.

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
    /// the composer pane, raised a little above the ground (BISE-102)
    pub raised: Color,
    /// the ground every cell is painted with (BISE-92)
    pub bg: Color,
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
    raised: rgb(0x1f1c1a),
    bg: rgb(0x141211),
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
    // on the painted ground (a lighter cream than #f7f4ee, so both tints
    // show on it and every role still reads on them): a pink selection,
    // a sand card
    selection_bg: rgb(0xfdeef2),
    card_tint: rgb(0xf1eee6),
    raised: rgb(0xf4f0e8),
    bg: rgb(0xfdfbf7),
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
/// The ground: painted on every cell (BISE-92).
pub(crate) fn bg() -> Color {
    palette().bg
}

/// The frame pass (BISE-92): every cell left at the terminal's default
/// (`Color::Reset`) gets the theme's ground, or text color for the
/// foreground. Runs after everything is drawn, before `asciify`.
pub(crate) fn paint(buf: &mut ratatui::buffer::Buffer) {
    let (ground, ink) = (bg(), text());
    for cell in buf.content.iter_mut() {
        if cell.bg == Color::Reset {
            cell.bg = ground;
        }
        if cell.fg == Color::Reset {
            cell.fg = ink;
        }
    }
}
/// The light tint under selected text.
pub(crate) fn selection_bg() -> Color {
    palette().selection_bg
}
/// The light tint of the card box.
pub(crate) fn card_tint() -> Color {
    palette().card_tint
}
/// The composer pane's tint (book §5 `raised`, §13): everything under the
/// divider. Under `NO_COLOR`, none (`Reset`: the ground).
pub(crate) fn raised() -> Color {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        Color::Reset
    } else {
        palette().raised
    }
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
// After the glyph audit (BISE-03, BISE-84): only glyphs a fallback font
// draws at width 1; the breakers are replaced (✉ → @, ⟳ → ≡ pulsing,
// ⧗ → Δ, ⎇ → ψ, ↪ → »). Under `BISE_ASCII=1` every glyph has a plain
// ASCII form: call [`glyph`] (the constants are the Unicode forms), and
// [`asciify`] catches what is drawn without it.

// entities
pub(crate) const G_YOU: &str = "›"; // you, and the composer prompt
pub(crate) const G_MAIN: &str = ":*"; // main (accent)
pub(crate) const G_BRIEF: &str = "◇"; // an agent's brief
pub(crate) const G_THINK: &str = "∴"; // thinking (dim)
pub(crate) const G_BASH: &str = "$"; // a bash call
pub(crate) const G_TS: &str = "λ"; // a TypeScript call
pub(crate) const G_SUBCALL: &str = "↳"; // a sub-call inside a TypeScript run
pub(crate) const G_PATCH: &str = "±"; // a file edit
pub(crate) const G_MSG: &str = "@"; // a message between agents, or to you (was ✉)
pub(crate) const G_IMAGE: &str = "▣"; // an image (accent chip)
pub(crate) const G_CARD: &str = "?"; // a card: a decision that needs you (accent)
pub(crate) const G_COMPACTING: &str = "≡"; // compaction running (dim, pulsing; was ⟳)
pub(crate) const G_SUMMARY: &str = "≡"; // compaction summary (dim, still)
pub(crate) const G_INTERRUPTED: &str = "▲"; // turn interrupted (dim)
pub(crate) const G_WRAP: &str = "»"; // a wrapped code row continues (faint; was ↪)

// agent status
pub(crate) const G_STARTING: &str = "·"; // dim, pulsing
pub(crate) const G_WORKING: &str = "∿"; // pulsing: a breeze
pub(crate) const G_WAITING: &str = "…"; // waiting on another agent
pub(crate) const G_NEEDS_YOU: &str = "?"; // accent
pub(crate) const G_DONE: &str = "✓"; // accent (BISE-100, was ♡); draw it with [`done_glyph`]
pub(crate) const G_FAILED: &str = "✗"; // error
pub(crate) const G_IDLE: &str = "○"; // dim
pub(crate) const G_STOPPED: &str = "–"; // dim

// marks
pub(crate) const G_SENDING: &str = "·"; // your message: sending
pub(crate) const G_RECEIVED: &str = "✓"; // the agent got it
pub(crate) const G_READ: &str = "✓✓"; // the model read it (accent)
pub(crate) const G_UNREAD: &str = "•"; // unread activity (accent)
pub(crate) const G_WORKTREE: &str = "ψ"; // the agent has its own worktree (was ⎇)
pub(crate) const G_OVERLAP: &str = "⇄"; // two agents changed the same file
pub(crate) const G_RESTART_FAILED: &str = "↻"; // error
pub(crate) const G_BUILDING: &str = "Δ"; // a version building or on trial (was ⧗)
pub(crate) const G_CLOSED: &str = "▸"; // progressive disclosure: closed
pub(crate) const G_OPEN: &str = "▾"; // progressive disclosure: open

/// Every `G_*` glyph with its ASCII form, then the other glyphs the TUI
/// draws today (chrome, hints, old feed glyphs, the braille spinner): the
/// table [`glyph`] and [`asciify`] read. One cell each, except `✓✓`.
/// Box drawing (`│ ┃ ─ ╮ …`) and block elements (`▁ █ ▏ …`) stay: they
/// draw everywhere. User text is never in it (no letters, accents, CJK,
/// emoji, quotes).
pub(crate) const ASCII: &[(&str, &str)] = &[
    // §6 glyphs
    ("›", ">"),
    ("◇", "&"),
    ("∴", ":"),
    ("λ", "\\"),
    ("↳", "L"),
    ("±", "%"),
    ("▣", "#"),
    ("≡", "="),
    ("▲", "^"),
    ("»", "}"),
    ("·", "."),
    ("∿", "~"),
    ("…", ";"),
    ("✗", "x"),
    ("○", "o"),
    ("–", "_"),
    ("✓✓", "vv"),
    ("✓", "v"),
    ("•", "!"),
    ("ψ", "Y"),
    ("⇄", "/"),
    ("↻", "("),
    ("Δ", "A"),
    ("▸", "+"),
    ("▾", "-"),
    // the replaced ones, while old code still draws them
    ("✉", "@"),
    ("⟳", "="),
    ("⧗", "A"),
    ("⎇", "Y"),
    ("↪", "}"),
    ("♡", "*"),
    // chrome and hints drawn outside the G_* constants (BISE-83 moves them)
    ("✦", "*"),
    ("◀", "<"),
    ("▶", ">"),
    ("●", "*"),
    ("◉", "@"),
    ("◆", "*"),
    ("✚", "+"),
    ("▪", "*"),
    ("×", "x"),
    ("⏎", "<"),
    ("→", ">"),
    ("←", "<"),
    ("↑", "^"),
    ("↓", "v"),
    ("⇧", "S"),
    ("⌥", "M"),
    ("—", "-"),
    ("−", "-"),
    // the old braille spinner
    ("⠋", "~"),
    ("⠙", "~"),
    ("⠹", "~"),
    ("⠸", "~"),
    ("⠼", "~"),
    ("⠴", "~"),
    ("⠦", "~"),
    ("⠧", "~"),
    ("⠇", "~"),
    ("⠏", "~"),
];

#[cfg(not(test))]
mod ascii_cell {
    //! `BISE_ASCII=1` (or `true`), read once.
    use std::sync::OnceLock;

    static ASCII: OnceLock<bool> = OnceLock::new();

    pub(super) fn get() -> bool {
        *ASCII.get_or_init(|| {
            std::env::var("BISE_ASCII").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
        })
    }
}

#[cfg(test)]
mod ascii_cell {
    //! Per thread under `cargo test`, like the mode; off by default.
    use std::cell::Cell;

    thread_local!(static ASCII: Cell<bool> = const { Cell::new(false) });

    pub(super) fn get() -> bool {
        ASCII.with(|c| c.get())
    }
    pub(super) fn set(on: bool) {
        ASCII.with(|c| c.set(on));
    }
}

/// True under `BISE_ASCII=1`: every glyph is drawn in plain ASCII.
pub(crate) fn ascii_mode() -> bool {
    ascii_cell::get()
}

/// Tests of other modules switch ASCII mode for their thread.
#[cfg(test)]
pub(crate) fn set_ascii_for_tests(on: bool) {
    ascii_cell::set(on);
}

/// The glyph to draw for `g` (a `G_*` constant, or any glyph of the
/// table): its ASCII form under `BISE_ASCII=1`, else `g` itself.
pub(crate) fn glyph(g: &'static str) -> &'static str {
    if !ascii_mode() {
        return g;
    }
    ASCII.iter().find(|(u, _)| *u == g).map_or(g, |(_, a)| *a)
}

/// The done glyph: `✓` (accent), `*` under `BISE_ASCII=1`. Not
/// `glyph(G_DONE)`: the table's `✓` is your read mark (`v`), and done
/// keeps its own ASCII form (BISE-100; book §6).
pub(crate) fn done_glyph() -> &'static str {
    if ascii_mode() {
        "*"
    } else {
        G_DONE
    }
}

/// The mark of a cut text: `…`, or `...` under `BISE_ASCII=1` (QA 12: the
/// one-cell `;` of the table is for the waiting status, not for prose).
pub(crate) fn ellipsis() -> &'static str {
    if ascii_mode() {
        "..."
    } else {
        "…"
    }
}

/// Under `BISE_ASCII=1`, rewrite every cell of `buf` holding a glyph of the
/// table to its ASCII form (one cell to one cell, layout unchanged).
/// Nothing else is touched: letters, accents, CJK, emoji, box drawing.
/// Called after each draw; free when the mode is off.
pub(crate) fn asciify(buf: &mut ratatui::buffer::Buffer) {
    if !ascii_mode() {
        return;
    }
    for cell in buf.content.iter_mut() {
        let sym = cell.symbol();
        if sym.is_ascii() {
            continue;
        }
        if let Some((_, a)) = ASCII.iter().find(|(u, a)| *u == sym && a.len() == 1) {
            cell.set_symbol(a);
        }
    }
}

/// The working pulse: `∿` in text, then dim, then text… (one phase every
/// 4 ticks). Replaces the braille spinner.
pub(crate) fn working_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { text() } else { dim() };
    (glyph(G_WORKING), color)
}

/// The starting pulse: `·`, dim then faint, same rhythm as [`working_frame`].
pub(crate) fn starting_frame(tick: u32) -> (&'static str, Color) {
    let color = if (tick / 4).is_multiple_of(2) { dim() } else { faint() };
    (glyph(G_STARTING), color)
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
            for (tint_name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised)] {
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
    fn roles_read_on_the_painted_ground_and_the_tints_show_on_it() {
        for p in [&DARK, &LIGHT] {
            let fails = check(p, &[("ground", p.bg)]);
            assert!(fails.is_empty(), "below 4.5:1 on the ground: {fails:?}");
            // a tint must be seen on the ground, and apart from the other one
            for (name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised)] {
                let r = contrast(tint, p.bg);
                assert!(r >= 1.08, "{name} tint vs ground: {r:.3}");
            }
            assert!(contrast(p.selection_bg, p.card_tint) >= 1.03);
        }
    }

    #[test]
    fn paint_leaves_no_reset_cell() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::Style;
        let mut b = Buffer::empty(Rect::new(0, 0, 4, 1));
        b[(1, 0)].set_style(Style::default().bg(card_tint()).fg(accent()));
        paint(&mut b);
        assert!(b.content.iter().all(|c| c.bg != Color::Reset && c.fg != Color::Reset));
        assert_eq!((b[(0, 0)].bg, b[(0, 0)].fg), (bg(), text()));
        assert_eq!((b[(1, 0)].bg, b[(1, 0)].fg), (card_tint(), accent()), "set colors stay");
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
            bg(),
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
        assert_eq!(bg(), DARK.bg);
    }

    #[test]
    fn working_pulses_between_text_and_dim() {
        let frames: Vec<_> = (0..8).map(working_frame).collect();
        assert!(frames.iter().all(|(g, _)| *g == G_WORKING));
        assert_eq!(frames[0].1, text());
        assert_eq!(frames[4].1, dim());
    }

    const ALL_GLYPHS: &[&str] = &[
        G_YOU, G_MAIN, G_BRIEF, G_THINK, G_BASH, G_TS, G_SUBCALL, G_PATCH, G_MSG, G_IMAGE,
        G_CARD, G_COMPACTING, G_SUMMARY, G_INTERRUPTED, G_WRAP, G_STARTING, G_WORKING,
        G_WAITING, G_NEEDS_YOU, G_DONE, G_FAILED, G_IDLE, G_STOPPED, G_SENDING, G_RECEIVED,
        G_READ, G_UNREAD, G_WORKTREE, G_OVERLAP, G_RESTART_FAILED, G_BUILDING, G_CLOSED, G_OPEN,
    ];

    /// The documented widths: `:*` and `✓✓` are two cells, the rest one.
    fn documented_width(g: &str) -> usize {
        if g == G_MAIN || g == G_READ {
            2
        } else {
            1
        }
    }

    #[test]
    fn every_glyph_is_one_cell_in_both_modes() {
        use unicode_width::UnicodeWidthStr;
        for ascii in [false, true] {
            ascii_cell::set(ascii);
            for g in ALL_GLYPHS {
                let shown = glyph(g);
                assert_eq!(shown.width(), documented_width(g), "{g:?} → {shown:?} (ascii {ascii})");
                // no ambiguous-width surprise: the CJK width agrees for ASCII forms
                if ascii {
                    assert!(shown.is_ascii(), "{g:?} → {shown:?} is not ASCII");
                    assert_eq!(shown.width_cjk(), shown.width());
                }
            }
            let (w, _) = working_frame(0);
            assert_eq!(w, if ascii { "~" } else { "∿" });
        }
        ascii_cell::set(false);
    }

    #[test]
    fn the_ascii_table_is_one_cell_to_one_cell() {
        use unicode_width::UnicodeWidthStr;
        let mut seen = std::collections::HashSet::new();
        for (u, a) in ASCII {
            assert!(seen.insert(*u), "{u:?} twice in the table");
            assert!(a.is_ascii() && !a.is_empty(), "{u:?} → {a:?}");
            assert_eq!(u.width(), a.width(), "{u:?} → {a:?} changes the width");
            assert!(u.chars().all(|c| !c.is_alphanumeric() || c == 'λ' || c == 'ψ' || c == 'Δ'),
                "{u:?}: letters are user text");
        }
        // every G_* glyph that is not ASCII has its form
        for g in ALL_GLYPHS.iter().filter(|g| !g.is_ascii()) {
            assert!(ASCII.iter().any(|(u, _)| u == g), "{g:?} has no ASCII form");
        }
        // the replaced glyphs are gone from the constants
        for gone in ["✉", "⟳", "⧗", "⎇", "↪"] {
            assert!(!ALL_GLYPHS.contains(&gone), "{gone} is back");
        }
    }

    /// QA 12: two different glyphs never share an ASCII form, and the
    /// cut-text mark is `...` in ASCII mode.
    #[test]
    fn every_entity_has_its_own_ascii_form() {
        ascii_cell::set(true);
        let mut by_ascii: std::collections::HashMap<&str, &str> = Default::default();
        for g in ALL_GLYPHS {
            if let Some(other) = by_ascii.insert(glyph(g), g) {
                assert_eq!(other, *g, "{other:?} and {g:?} both read {:?}", glyph(g));
            }
        }
        // done is the read mark's `✓`, drawn `*` in ASCII (BISE-100)
        assert_eq!(done_glyph(), "*");
        assert!(ALL_GLYPHS.iter().all(|g| glyph(g) != "*"), "* is done's");
        assert_eq!(ellipsis(), "...");
        ascii_cell::set(false);
        assert_eq!(done_glyph(), "✓");
        assert_eq!(ellipsis(), "…");
    }

    #[test]
    fn glyph_is_the_identity_when_off() {
        assert!(!ascii_mode());
        for g in ALL_GLYPHS {
            assert_eq!(glyph(g), *g);
        }
    }

    fn buffer_of(text: &str) -> ratatui::buffer::Buffer {
        use ratatui::layout::Rect;
        use unicode_width::UnicodeWidthStr;
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, text.width() as u16, 1));
        buf.set_string(0, 0, text, ratatui::style::Style::default());
        buf
    }

    fn row(buf: &ratatui::buffer::Buffer) -> String {
        buf.content.iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn asciify_maps_only_the_table() {
        let text = "› ∿ ♡ ✓ ψ Δ … · ─│┃ é ñ ü 漢字 👍 « » “q” ✦ ⠋";
        let mut buf = buffer_of(text);
        let before = row(&buf);
        asciify(&mut buf);
        assert_eq!(row(&buf), before, "off: nothing changes");
        ascii_cell::set(true);
        asciify(&mut buf);
        ascii_cell::set(false);
        let after = row(&buf);
        // (wide characters keep their continuation cell: compare buffers)
        assert_eq!(after, row(&buffer_of("> ~ * v Y A ; . ─│┃ é ñ ü 漢字 👍 « } “q” * ~")));
        let non_ascii: String = after
            .chars()
            .filter(|c| !c.is_ascii() && !('\u{2500}'..='\u{257f}').contains(c))
            .collect();
        assert_eq!(non_ascii, "éñü漢字👍«“”", "only user text and box drawing survive");
    }
}
