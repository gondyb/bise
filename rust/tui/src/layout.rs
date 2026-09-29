//! The global layout of the Switchboard screen (book §8, "the reading
//! column" and "spacing, in cells"; BISE-97): outer margins, the header
//! row, the feed area and its reading column, the agents panel. One
//! place for the numbers; the header, the feed, the card box, the status
//! row, the queue, the strip, the composer block and the hints take their
//! x and width from here.

/// The reading column: 3 columns of lead (the glyph column) + 88 of text
/// (79 until BISE-101: +15%).
pub(crate) const COLUMN: u16 = 91;
/// Tables and code start at the column's x and may run this wide.
pub(crate) const WIDE: u16 = 103;
/// The feed area must be at least this wide for the column to center.
const CENTER_FROM: u16 = 95;
/// The frame (book §8 "The frame") shows from this size up; under it a
/// header row, a plain divider and margins of `BARE`.
const FRAME_W: u16 = 60;
const FRAME_H: u16 = 16;
/// Framed, text starts this many columns in: the border and 2 blank
/// columns (and ends as far from the right edge, at F − 4).
const PAD: u16 = 3;
/// Unframed: the margin, left and right.
const BARE: u16 = 1;
/// The panels by width tier: from `wide`, `w` columns with its rule
/// `rule_gap` columns left of its text; the history ends `feed_gap`
/// columns left of the rule. Unframed, the same panel `bare_gap` columns
/// from the feed, no rule.
struct Tier {
    from: u16,
    w: u16,
    name_cut: usize,
    bare_gap: u16,
}
const TIERS: [Tier; 2] = [
    Tier { from: 100, w: 28, name_cut: 16, bare_gap: 3 },
    Tier { from: 90, w: 24, name_cut: 12, bare_gap: 2 },
];
/// Framed: 1 blank column between the rule and the panel's text, 2
/// between the history's last column and the rule.
const RULE_TO_TEXT: u16 = 2;
const FEED_TO_RULE: u16 = 3;
/// The tinted rows of the raised pane: the one between the text and the
/// key bar goes under `PAD_BOTTOM_FROM` rows, the one under the divider
/// under `PAD_TOP_FROM`.
const PAD_BOTTOM_FROM: u16 = 24;
const PAD_TOP_FROM: u16 = 20;
/// The composer's text: at least `MIN_TEXT` rows (1 under
/// `PAD_TOP_FROM`), growing to `MAX_TEXT` rows or 40% of the height.
const MIN_TEXT: u16 = 2;
const MAX_TEXT: u16 = 12;
/// Unframed: the blank row under the header row goes when the screen
/// is `SHORT_BODY` rows or fewer.
const SHORT_BODY: u16 = 8;

/// The agents panel: its x, its width, where names are cut, and the x of
/// its rule (framed only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Panel {
    pub(crate) x: u16,
    pub(crate) w: u16,
    pub(crate) name_cut: usize,
    pub(crate) rule: Option<u16>,
}

/// The columns of a screen (x relative to the screen's left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cols {
    /// framed (book §8 "The frame"): the rounded border on the edge
    pub(crate) framed: bool,
    /// where text starts: 3 framed (the border + 2 blank), else 1
    pub(crate) margin: u16,
    /// the feed area: everything left of the panel, inside the margins
    pub(crate) feed_x: u16,
    pub(crate) feed_w: u16,
    /// the reading column: history and card box
    pub(crate) x0: u16,
    pub(crate) col_w: u16,
    /// how wide tables and code may run from x0 (to the feed area's edge)
    pub(crate) wide_w: u16,
    pub(crate) panel: Option<Panel>,
    /// the composer pane's text span (divider label, composer bar, key
    /// bar): from `margin` to the last column before the right margin
    pub(crate) pane_w: u16,
}

/// The screen's frame test: framed from 60 columns and 16 rows.
pub(crate) fn framed(width: u16, height: u16) -> bool {
    width >= FRAME_W && height >= FRAME_H
}

/// The columns of a `width` × `height` screen. Framed: text from column
/// 3 to F − 4; F ≥ 100 a 28-column panel (text F − 31 .. F − 4, names cut
/// at 16) behind a rule at F − 33, the history ending at F − 36; 90–99 a
/// 24-column panel (rule at F − 29, names cut at 12); < 90 none.
/// Unframed: margins of 1, the same panels 3 (2) columns from the feed,
/// no rule. The column is 91 wide at most, centered in the feed area when
/// that is at least 95 wide, else at its left.
pub(crate) fn cols(width: u16, height: u16) -> Cols {
    let framed = framed(width, height);
    let margin = if framed { PAD } else { BARE };
    let inner = width.saturating_sub(2 * margin);
    let tier = TIERS.iter().find(|t| width >= t.from);
    let (feed_w, panel) = match tier {
        None => (inner.max(1), None),
        Some(t) if framed => {
            // the panel text ends at the right margin; its rule 2 columns
            // left of it; the history 3 left of the rule
            let x = width - margin - t.w;
            let rule = x - RULE_TO_TEXT;
            let feed_w = (rule - FEED_TO_RULE + 1).saturating_sub(margin).max(1);
            (feed_w, Some(Panel { x, w: t.w, name_cut: t.name_cut, rule: Some(rule) }))
        }
        Some(t) => {
            let feed_w = inner.saturating_sub(t.w + t.bare_gap).max(1);
            (feed_w, Some(Panel { x: margin + feed_w + t.bare_gap, w: t.w, name_cut: t.name_cut, rule: None }))
        }
    };
    let feed_x = margin;
    let col_w = feed_w.min(COLUMN);
    let x0 = if feed_w >= CENTER_FROM { feed_x + (feed_w - col_w) / 2 } else { feed_x };
    let wide_w = (feed_x + feed_w - x0).min(WIDE);
    Cols { framed, margin, feed_x, feed_w, x0, col_w, wide_w, panel, pane_w: inner.max(1) }
}

/// The rows of a screen `height` tall (y relative to its top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rows {
    /// the header: the frame's top border (framed) or its own row 0
    pub(crate) header: u16,
    /// the history's first row: 1 blank row under the header
    pub(crate) body: u16,
    /// the key bar's row: above the frame's bottom border, or the last;
    /// `height` when it goes into the divider (`keys_in_divider`)
    pub(crate) keybar: u16,
    /// under `KEYS_OWN_ROW_FROM` rows the key bar takes the divider's
    /// right side instead of the state
    pub(crate) keys_in_divider: bool,
    /// the tinted rows of the raised pane (book §13): one right under the
    /// divider, one between the text and the key bar
    pub(crate) pad_top: u16,
    pub(crate) pad_bottom: u16,
    /// the composer's text rows: at least `min_text`, at most `max_text`
    pub(crate) min_text: u16,
    pub(crate) max_text: u16,
}

/// Under this height the key bar has no row of its own.
const KEYS_OWN_ROW_FROM: u16 = 14;

/// The rows by height (book §8 "The frame", §13 "The composer pane"): the
/// header on row 0, 1 blank row, the history; the divider, then the raised
/// pane: 1 tinted row (from 20 rows), the queue and the strip, the text (2
/// rows, 1 under 20, up to min(12, 40%)), 1 tinted row (from 24 rows), the
/// key bar (its own row from 14 rows), the frame's bottom edge (framed).
pub(crate) fn rows(width: u16, height: u16) -> Rows {
    let framed = framed(width, height);
    let gap = u16::from(framed || height > SHORT_BODY);
    let min_text = if height >= PAD_TOP_FROM { MIN_TEXT } else { 1 };
    let keys_in_divider = height < KEYS_OWN_ROW_FROM;
    Rows {
        header: 0,
        body: 1 + gap,
        keybar: if keys_in_divider { height } else { height.saturating_sub(1 + u16::from(framed)) },
        keys_in_divider,
        pad_top: u16::from(height >= PAD_TOP_FROM),
        pad_bottom: u16::from(height >= PAD_BOTTOM_FROM),
        min_text,
        max_text: (height * 2 / 5).min(MAX_TEXT).max(min_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_tiers() {
        // 160 framed: the panel text F-31..F-4, its rule at F-33, the
        // history ending at F-36, the column centered in the feed area
        let c = cols(160, 40);
        assert!(c.framed);
        assert_eq!((c.margin, c.feed_x, c.pane_w), (3, 3, 154));
        assert_eq!(c.panel, Some(Panel { x: 129, w: 28, name_cut: 16, rule: Some(127) }));
        assert_eq!(c.panel.unwrap().x + 28 - 1, 160 - 4);
        assert_eq!(c.feed_x + c.feed_w - 1, 160 - 36);
        assert_eq!((c.feed_w, c.col_w, c.x0), (122, 91, 3 + (122 - 91) / 2));
        // tables and code: to the feed area's edge, 103 at most
        assert_eq!(c.wide_w, (3 + 122 - c.x0).min(WIDE));
        // 130: feed area 92, too narrow to center (BISE-101: from 95)
        let c = cols(130, 40);
        assert_eq!((c.feed_w, c.col_w, c.x0), (92, 91, 3));
        let c = cols(133, 40);
        assert_eq!((c.feed_w, c.col_w, c.x0), (95, 91, 3 + 2));
        // 100: feed area 62, not centered
        let c = cols(100, 40);
        assert_eq!((c.feed_w, c.x0, c.col_w), (62, 3, 62));
        assert_eq!(c.panel.unwrap().rule, Some(67));
        // 95: panel 24, rule at F-29, names cut at 12
        let c = cols(95, 40);
        assert_eq!(c.panel, Some(Panel { x: 68, w: 24, name_cut: 12, rule: Some(66) }));
        assert_eq!(c.feed_x + c.feed_w - 1, 95 - 32);
        // 80: no panel, column 74 (3..76)
        let c = cols(80, 40);
        assert_eq!((c.panel, c.x0, c.col_w), (None, 3, 74));
        // 60 framed: column 54
        let c = cols(60, 40);
        assert_eq!((c.framed, c.x0, c.col_w), (true, 3, 54));
    }

    #[test]
    fn small_screens_go_bare() {
        // under 60 columns or 16 rows: no frame, margins 1, no rule
        let c = cols(59, 40);
        assert_eq!((c.framed, c.margin, c.x0, c.col_w), (false, 1, 1, 57));
        let c = cols(120, 15);
        assert!(!c.framed);
        assert_eq!(c.panel, Some(Panel { x: 91, w: 28, name_cut: 16, rule: None }));
        assert_eq!(c.panel.unwrap().x + 28 + 1, 120);
        // tiny: never zero
        assert!(cols(3, 3).col_w >= 1);
    }

    #[test]
    fn the_rows() {
        // framed, tall: header on the border, 1 blank row, key bar above
        // the bottom border, 1 blank row each side of the text
        let r = rows(100, 40);
        assert_eq!((r.header, r.body, r.keybar, r.pad_top, r.pad_bottom), (0, 2, 38, 1, 1));
        assert_eq!((r.min_text, r.max_text), (2, 12));
        // < 24 rows: no blank row under the text; < 20: none above, 1 row
        assert_eq!((rows(100, 23).pad_top, rows(100, 23).pad_bottom), (1, 0));
        let r = rows(100, 19);
        assert_eq!((r.pad_top, r.pad_bottom, r.min_text), (0, 0, 1));
        // unframed: the key bar on the last row; under 14 rows, in the divider
        assert_eq!(rows(100, 15).keybar, 14);
        assert!(!rows(100, 14).keys_in_divider);
        let r = rows(100, 13);
        assert!(r.keys_in_divider);
        assert_eq!(r.keybar, 13);
        assert_eq!(rows(100, 8).body, 1);
    }
}
