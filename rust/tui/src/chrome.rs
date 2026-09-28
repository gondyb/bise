//! The frame of the Switchboard screen (book §8 "The frame", BISE-98):
//! the faint rounded border with `bise :*` and the summary in its top
//! edge, the panel's rule joined to it, and the divider `├─ you → main
//! ─…─ state ─┤` over the composer pane. Lines only: no background of
//! their own (the theme ground stays, BISE-92).

use crate::layout::Cols;
use crate::theme::{self, accent, dim, faint};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

/// The line pieces: rounded corners, joins, rules; `+ - |` in ASCII.
struct Pieces {
    set: border::Set,
    left_join: &'static str,
    right_join: &'static str,
    top_join: &'static str,
    bottom_join: &'static str,
}

fn pieces() -> Pieces {
    if theme::ascii_mode() {
        Pieces {
            set: border::Set {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                vertical_left: "|",
                vertical_right: "|",
                horizontal_top: "-",
                horizontal_bottom: "-",
            },
            left_join: "+",
            right_join: "+",
            top_join: "+",
            bottom_join: "+",
        }
    } else {
        Pieces { set: border::ROUNDED, left_join: "├", right_join: "┤", top_join: "┬", bottom_join: "┴" }
    }
}

fn line_style() -> Style {
    Style::default().fg(faint())
}

/// Write `spans` from `x` on row `y`, never past `end` (exclusive).
fn put(buf: &mut Buffer, x: u16, y: u16, spans: &[Span], end: u16) {
    let mut x = x;
    for s in spans {
        if x >= end {
            break;
        }
        let (nx, _) = buf.set_stringn(x, y, s.content.as_ref(), usize::from(end - x), s.style);
        x = nx;
    }
}

/// `spans` cut to `room` columns, a `…` at the cut.
pub(crate) fn fit(spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let w: usize = spans.iter().map(|s| s.content.width()).sum();
    if w <= room {
        return spans;
    }
    let ell = theme::ellipsis();
    let mut left = room.saturating_sub(ell.width());
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut last = Style::default();
    for s in spans {
        last = s.style;
        if left == 0 {
            break;
        }
        let mut t = String::new();
        for ch in s.content.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if cw > left {
                left = 0;
                break;
            }
            left -= cw;
            t.push(ch);
        }
        out.push(Span::styled(t, s.style));
    }
    if room >= ell.width() {
        out.push(Span::styled(ell, last));
    }
    out
}

/// The frame on `area`'s edge, framed screens only: the rounded border,
/// the title from column 3 (1 space each side) and the summary ending at
/// F − 4 in the top edge, the panel's rule from row 1 down to the divider
/// (joined `┬` on the top edge, `┴` on the divider, unless text covers
/// the join).
pub(crate) fn draw_frame(
    buf: &mut Buffer,
    area: Rect,
    cols: Cols,
    title: Vec<Span<'static>>,
    summary: impl FnOnce(usize) -> Vec<Span<'static>>,
    divider_y: u16,
) {
    let area = area.intersection(buf.area);
    if area.width < 8 || area.height < 3 {
        return;
    }
    let p = pieces();
    let st = line_style();
    let (l, r, t, b) = (area.x, area.right() - 1, area.y, area.bottom() - 1);
    for x in l..=r {
        let (top, bottom) = if x == l {
            (p.set.top_left, p.set.bottom_left)
        } else if x == r {
            (p.set.top_right, p.set.bottom_right)
        } else {
            (p.set.horizontal_top, p.set.horizontal_bottom)
        };
        buf[(x, t)].set_symbol(top).set_style(st);
        buf[(x, b)].set_symbol(bottom).set_style(st);
    }
    for y in t + 1..b {
        buf[(l, y)].set_symbol(p.set.vertical_left).set_style(st);
        buf[(r, y)].set_symbol(p.set.vertical_right).set_style(st);
    }
    // the panel's rule, joined on the top edge (the title may cover it)
    if let Some(x) = cols.panel.and_then(|p| p.rule).map(|x| area.x + x).filter(|x| *x > l && *x < r) {
        for y in t + 1..divider_y.min(b) {
            buf[(x, y)].set_symbol(p.set.vertical_left).set_style(st);
        }
        buf[(x, t)].set_symbol(p.top_join).set_style(st);
    }
    // the title from column 3, 1 space each side
    let mut head = vec![Span::raw(" ")];
    head.extend(title);
    head.push(Span::raw(" "));
    let head_w: u16 = head.iter().map(|s| s.content.width() as u16).sum();
    let hx = l + cols.margin - 1;
    put(buf, hx, t, &head, r);
    // the summary: 1 space each side, ending at F − 4; at least 1 rule
    // cell between it and the title
    let end = r - (cols.margin - 1); // exclusive: F - 3 holds its space
    let start = hx + head_w + 2;
    let room = end.saturating_sub(start) as usize;
    let s = summary(room);
    let w: u16 = s.iter().map(|s| s.content.width() as u16).sum();
    if w > 0 && w as usize <= room {
        let x = end - w;
        put(buf, x - 1, t, &[Span::raw(" ")], end);
        put(buf, x, t, &s, end);
        put(buf, end, t, &[Span::raw(" ")], end + 1);
    }
}

/// The divider on row `y` (book §8): framed, a rule joining the frame
/// (`├ … ┤`, `┴` under the panel's rule); bare, a plain rule. The label
/// ` you → name ` from the margin, the `state` (cut to fit) ending 1
/// column before the right margin's space. The state's rect is returned
/// (a click on `↓ back to the bottom` jumps to the tail).
pub(crate) fn draw_divider(buf: &mut Buffer, area: Rect, cols: Cols, y: u16, name: &str, state: Vec<Span<'static>>) -> Rect {
    let area = area.intersection(buf.area);
    if area.width < 4 || y < area.y || y >= area.bottom() {
        return Rect::default();
    }
    let p = pieces();
    let st = line_style();
    let (l, r) = (area.x, area.right() - 1);
    for x in l..=r {
        buf[(x, y)].set_symbol(p.set.horizontal_top).set_style(st);
    }
    if cols.framed {
        buf[(l, y)].set_symbol(p.left_join).set_style(st);
        buf[(r, y)].set_symbol(p.right_join).set_style(st);
        if let Some(x) = cols.panel.and_then(|p| p.rule).map(|x| area.x + x).filter(|x| *x > l && *x < r) {
            buf[(x, y)].set_symbol(p.bottom_join).set_style(st);
        }
    }
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    let label = vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), Style::default().fg(dim())),
        Span::styled(name.to_string(), Style::default().fg(accent())),
        Span::raw(" "),
    ];
    let lx = l + cols.margin - 1;
    // the state's end: framed F − 4 (F − 3 its space), bare the last column
    let end = r + 1 - cols.margin;
    put(buf, lx, y, &label, end + 1);
    let label_w: u16 = label.iter().map(|s| s.content.width() as u16).sum();
    let start = lx + label_w + 2;
    let room = end.saturating_sub(start) as usize;
    let state = fit(state, room);
    let w: u16 = state.iter().map(|s| s.content.width() as u16).sum();
    if w == 0 {
        return Rect::default();
    }
    let x = end - w;
    put(buf, x - 1, y, &[Span::raw(" ")], end);
    put(buf, x, y, &state, end);
    if end <= r {
        put(buf, end, y, &[Span::raw(" ")], end + 1);
    }
    Rect { x, y, width: w, height: 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        let s = vec![Span::raw("idle · "), Span::raw("210k / 1M tokens")];
        let out = fit(s.clone(), 100);
        assert_eq!(out.len(), 2);
        let out: String = fit(s, 10).iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(out, "idle · 21…");
        assert_eq!(out.width(), 10);
    }
}
