//! Text as the terminal will lay it out. ratatui gives each grapheme the
//! width `unicode-width` says, and writes a TAB, a CR or an ESC into a cell
//! like a letter; the terminal then jumps to the next tab stop, goes back
//! to column 0 or starts an escape sequence. Its cursor and ratatui's
//! buffer disagree from that cell on, and the cells the diff believes are
//! already right stay stale on screen: text ghosts to the right of the
//! feed that only a resize clears (tool outputs of `du`, `sort`, `ls -l`...).
//!
//! Two layers: [`clean`] where verbatim text enters (tool results, code,
//! markdown) expands tabs and drops escape sequences and invisible
//! controls, so the columns stay right; [`cells`], after each frame, is the
//! net for every other path: no cell ever holds a control character.

use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

/// Tab stops of verbatim terminal output (what a terminal shows).
pub(crate) const TAB_OUTPUT: usize = 8;
/// Tab stops of code and prose (the model's own text).
pub(crate) const TAB_CODE: usize = 4;

/// A character with no place in a cell: C0 and C1 controls, DEL, and the
/// invisible format characters a terminal may draw, skip or apply
/// (bidi overrides and marks, zero-width space, word joiner, BOM). ZWJ,
/// ZWNJ and the variation selectors stay: emoji sequences need them.
pub(crate) fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}' | '\u{200B}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
        )
}

/// `s` with its tabs expanded to stops every `tab` columns (from the start
/// of each line), ANSI escape sequences dropped, and every other unsafe
/// character ([`is_unsafe`]) dropped, CR included (a progress bar's states
/// then follow each other: the runtime flattens the newlines of a result
/// preview, so a CR cannot say which text it rewrote). Newlines stay.
/// Borrowed when clean.
pub(crate) fn clean(s: &str, tab: usize) -> Cow<'_, str> {
    if !s.chars().any(|c| c != '\n' && is_unsafe(c)) {
        return Cow::Borrowed(s);
    }
    let tab = tab.max(1);
    let mut out = String::with_capacity(s.len() + 16);
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let start = out.len();
        let mut cs = line.chars().peekable();
        while let Some(c) = cs.next() {
            match c {
                '\t' => {
                    // the column as the terminal counts it (an emoji sequence is one glyph)
                    let n = tab - out[start..].width() % tab;
                    out.extend(std::iter::repeat_n(' ', n));
                }
                '\u{1b}' => skip_escape(&mut cs),
                c if is_unsafe(c) => {}
                c => out.push(c),
            }
        }
    }
    Cow::Owned(out)
}

/// Skip the rest of an escape sequence (the ESC is read): CSI up to its
/// final byte, OSC/DCS/APC/PM/SOS up to BEL or ST, else one character.
fn skip_escape(cs: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match cs.next() {
        Some('[') => {
            for c in cs.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    break;
                }
            }
        }
        Some(']' | 'P' | '_' | '^' | 'X') => {
            while let Some(c) = cs.next() {
                if c == '\u{7}' || c == '\u{9c}' {
                    break;
                }
                if c == '\u{1b}' {
                    if cs.peek() == Some(&'\\') {
                        cs.next();
                    }
                    break;
                }
            }
        }
        _ => {}
    }
}

/// The net, after each frame: a cell holding an unsafe character keeps
/// the rest of its symbol, or a space (a TAB or an ESC took one column in
/// the layout: it stays one column).
pub(crate) fn cells(buf: &mut ratatui::buffer::Buffer) {
    for cell in buf.content.iter_mut() {
        let sym = cell.symbol();
        let safe = match sym.as_bytes() {
            [b] => (0x20..0x7f).contains(b),
            _ => !sym.chars().any(is_unsafe),
        };
        if safe {
            continue;
        }
        let kept: String = sym.chars().filter(|&c| !is_unsafe(c)).collect();
        cell.set_symbol(if kept.is_empty() { " " } else { &kept });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, text::Line, widgets::Widget};

    #[test]
    fn tabs_expand_to_stops_from_the_line_start() {
        assert_eq!(clean("524\telectron\n12G\t/tmp", 8), "524     electron\n12G     /tmp");
        assert_eq!(clean("\tx\n12345678\ty", 4), "    x\n12345678    y");
        // wide characters count their columns
        assert_eq!(clean("日本\tx", 8), "日本    x");
    }

    #[test]
    fn controls_escapes_and_invisibles_go() {
        assert_eq!(clean("a\r\nb\r\n", 8), "a\nb\n");
        assert_eq!(clean("10%\r100%", 8), "10%100%");
        assert_eq!(clean("\x1b[31mred\x1b[0m ok", 8), "red ok");
        assert_eq!(clean("\x1b]8;;https://x\x1b\\link\x1b]8;;\x07!", 8), "link!");
        assert_eq!(clean("a\u{7}b\u{0}c\u{7f}d\u{85}e", 8), "abcde");
        assert_eq!(clean("a\u{202e}b\u{200b}c\u{feff}d\u{2066}e", 8), "abcde");
        // emoji sequences keep their joiners and selectors
        assert_eq!(clean("👩\u{200d}💻 ❤\u{fe0f}\t.", 8), "👩\u{200d}💻 ❤\u{fe0f}   ."); // 2 + 1 + 2 columns, then the stop at 8
        assert!(matches!(clean("plain\ntext", 8), Cow::Borrowed(_)));
    }

    /// Every cell a Line drew from raw text holds a symbol the terminal
    /// prints in the columns the buffer gave it.
    #[test]
    fn the_net_leaves_no_control_in_a_cell() {
        let mut b = Buffer::empty(Rect::new(0, 0, 24, 1));
        Line::from("524\telectron\r\x1b[2Ka\u{202e}b\u{7}").render(b.area, &mut b);
        cells(&mut b);
        let row: String = (0..24).map(|x| b[(x, 0)].symbol().to_string()).collect();
        assert!(!row.chars().any(is_unsafe), "{row:?}");
        assert_eq!(row, "524 electron  [2Kab     ");
    }
}
