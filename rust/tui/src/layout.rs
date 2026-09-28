//! The global layout of the Switchboard screen (book §8, "the reading
//! column" and "spacing, in cells"; BISE-97): outer margins, the header
//! row, the feed area and its reading column, the agents panel. One
//! place for the numbers; the header, the feed, the card box, the status
//! row, the queue, the strip, the composer block and the hints take their
//! x and width from here.

/// The reading column: 3 columns of lead (the glyph column) + 76 of text.
pub(crate) const COLUMN: u16 = 79;
/// Tables and code start at the column's x and may run this wide.
pub(crate) const WIDE: u16 = 103;
/// The feed area must be at least this wide for the column to center.
const CENTER_FROM: u16 = 83;
/// The outer margin, left and right; `MARGIN_NARROW` under `NARROW_UNDER`
/// columns.
const MARGIN: u16 = 2;
const MARGIN_NARROW: u16 = 1;
const NARROW_UNDER: u16 = 64;
/// The top and bottom margin rows: `MARGIN_ROWS` each from `TALL_FROM` rows.
const MARGIN_ROWS: u16 = 1;
const TALL_FROM: u16 = 30;
/// The blank rows between the header row and the body: `HEADER_GAP`, 1
/// when the body is `SHORT_BODY` rows or fewer.
const HEADER_GAP: u16 = 1;
const SHORT_BODY: u16 = 8;

/// The agents panel: its x, its width, where names are cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Panel {
    pub(crate) x: u16,
    pub(crate) w: u16,
    pub(crate) name_cut: usize,
}

/// The columns of a screen `width` wide (x relative to the screen's left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cols {
    /// the outer margin, left and right
    pub(crate) margin: u16,
    /// the feed area: everything left of the panel, inside the margins
    pub(crate) feed_x: u16,
    pub(crate) feed_w: u16,
    /// the reading column: history, card box, status row, queue, strip,
    /// composer block and hints
    pub(crate) x0: u16,
    pub(crate) col_w: u16,
    /// how wide tables and code may run from x0 (to the feed area's edge)
    pub(crate) wide_w: u16,
    pub(crate) panel: Option<Panel>,
}

/// The columns by width tier: ≥ 100 a 28-column panel 3 columns from the
/// feed (names cut at 16); 90–99 a 24-column panel 2 columns away (names
/// cut at 12); < 90 no panel; margins 2 (1 under 64 columns). The column
/// is 79 wide at most, centered in the feed area when that is at least 83
/// wide, else at the left margin.
pub(crate) fn cols(width: u16) -> Cols {
    let margin = if width >= NARROW_UNDER { MARGIN } else { MARGIN_NARROW };
    let inner = width.saturating_sub(2 * margin);
    let (panel_w, gap, name_cut) = match width {
        w if w >= 100 => (28, 3, 16),
        w if w >= 90 => (24, 2, 12),
        _ => (0, 0, 0),
    };
    let feed_w = inner.saturating_sub(panel_w + gap).max(1);
    let feed_x = margin;
    let panel = (panel_w > 0).then(|| Panel { x: margin + feed_w + gap, w: panel_w, name_cut });
    let col_w = feed_w.min(COLUMN);
    let x0 = if feed_w >= CENTER_FROM { feed_x + (feed_w - col_w) / 2 } else { feed_x };
    let wide_w = (feed_x + feed_w - x0).min(WIDE);
    Cols { margin, feed_x, feed_w, x0, col_w, wide_w, panel }
}

/// The outer margin rows (top, bottom): 1 each, none under 30 rows.
pub(crate) fn margin_rows(height: u16) -> u16 {
    if height >= TALL_FROM { MARGIN_ROWS } else { 0 }
}

/// The header row inside the margins of `area` (1 blank column each side
/// of its text: it starts 1 column before the margin), and the rows it
/// takes with the gap under it; `None` when `area` is too short for one.
pub(crate) fn header(area: ratatui::layout::Rect, cols: Cols) -> Option<(ratatui::layout::Rect, u16)> {
    if area.height <= 3 {
        return None;
    }
    let x = area.x + cols.margin.saturating_sub(1);
    let w = (area.width.saturating_sub(2 * cols.margin) + 2).min(area.right().saturating_sub(x));
    let gap = if area.height > SHORT_BODY { HEADER_GAP } else { 0 };
    Some((ratatui::layout::Rect { x, width: w, height: 1, ..area }, 1 + gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiers() {
        // 160: panel 28 flush right at the margin, 3 blank columns, the
        // column centered in the feed area
        let c = cols(160);
        assert_eq!((c.margin, c.feed_x, c.feed_w), (2, 2, 125));
        assert_eq!(c.panel, Some(Panel { x: 130, w: 28, name_cut: 16 }));
        assert_eq!(c.panel.unwrap().x + 28 + 2, 160);
        assert_eq!((c.x0, c.col_w), (2 + (125 - 79) / 2, 79));
        assert_eq!(c.wide_w, 2 + 125 - c.x0);
        // 120: feed area 85, still centered
        let c = cols(120);
        assert_eq!((c.feed_w, c.col_w, c.x0), (85, 79, 5));
        // 100: feed area 65, not centered
        let c = cols(100);
        assert_eq!((c.feed_w, c.x0, c.col_w), (65, 2, 65));
        assert_eq!(c.panel.unwrap().x, 70);
        // 95: panel 24, gap 2, names cut at 12
        let c = cols(95);
        assert_eq!(c.panel, Some(Panel { x: 2 + 65 + 2, w: 24, name_cut: 12 }));
        // 80: no panel, column 76 (73 of text)
        let c = cols(80);
        assert_eq!((c.panel, c.x0, c.col_w), (None, 2, 76));
        // 60: margins 1, column 58 (55 of text)
        let c = cols(60);
        assert_eq!((c.margin, c.x0, c.col_w), (1, 1, 58));
        // tiny: never zero
        assert!(cols(3).col_w >= 1);
    }

    #[test]
    fn margin_rows_under_30_go() {
        assert_eq!(margin_rows(30), 1);
        assert_eq!(margin_rows(29), 0);
    }
}
