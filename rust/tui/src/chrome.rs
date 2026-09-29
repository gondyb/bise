//! The frame of the Switchboard screen (book §8 "The frame", BISE-98):
//! the faint rounded border with `bise :*` and the summary in its top
//! edge, the panel's rule joined to it, and the divider `├─ you → main
//! ─…─ state ─┤` over the composer pane. Lines only: no background of
//! their own (the theme ground stays, BISE-92).

use crate::gust;
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

/// The role line keeps at least this many columns (its ` · ` included)
/// before the summary shortens for it; under it, it goes (BISE-126).
const ROLE_MIN: usize = 12;
/// What the summary leaves to the role line at most: the path goes
/// before the line is cut under this.
const ROLE_KEEP: usize = 27;

/// The role line and the summary in `room` columns (BISE-126): the
/// summary drops its path to leave the line up to ROLE_KEEP columns, then
/// ROLE_MIN, but never a count: the counts come first; the line takes
/// what the summary leaves.
pub(crate) fn share_room(
    room: usize,
    role: Vec<Span<'static>>,
    summary: impl Fn(usize) -> Vec<Span<'static>>,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let role_w: usize = role.iter().map(|s| s.content.width()).sum();
    let full = summary(room);
    if role_w == 0 {
        return (Vec::new(), full);
    }
    let counts = |s: &[Span]| -> usize {
        let t: String = s.iter().map(|x| x.content.as_ref()).collect();
        t.split_whitespace().filter(|w| w.chars().all(|c| c.is_ascii_digit())).count()
    };
    let want = counts(&full);
    let s = [role_w.min(ROLE_KEEP), ROLE_MIN.min(role_w)]
        .into_iter()
        .map(|r| summary(room.saturating_sub(r)))
        .find(|s| counts(s) == want)
        .unwrap_or(full);
    let w: usize = s.iter().map(|x| x.content.width()).sum();
    (fit_role(role, room.saturating_sub(w)), s)
}

/// The role line in `room` columns: cut with `…`, or nothing under
/// ROLE_MIN.
fn fit_role(role: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let w: usize = role.iter().map(|s| s.content.width()).sum();
    if w == 0 || room < ROLE_MIN.min(w) {
        return Vec::new();
    }
    fit(role, room)
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
    role: Vec<Span<'static>>,
    summary: impl Fn(usize) -> Vec<Span<'static>>,
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
    // the title from column 3, 1 space each side; the viewed task's role
    // line right after it (BISE-126)
    let mut head = vec![Span::raw(" ")];
    head.extend(title);
    head.push(Span::raw(" "));
    let head_w: u16 = head.iter().map(|s| s.content.width() as u16).sum();
    let hx = l + cols.margin - 1;
    // the summary: 1 space each side, ending at F − 4; at least 1 rule
    // cell between it and the title
    let end = r - (cols.margin - 1); // exclusive: F - 3 holds its space
    let start = hx + head_w + 2;
    let room = end.saturating_sub(start) as usize;
    let (role, s) = share_room(room, role, summary);
    let w: u16 = s.iter().map(|s| s.content.width() as u16).sum();
    let space = head.pop();
    head.extend(role);
    head.extend(space);
    put(buf, hx, t, &head, r);
    if w > 0 && w as usize <= room {
        let x = end - w;
        put(buf, x - 1, t, &[Span::raw(" ")], end);
        put(buf, x, t, &s, end);
        put(buf, end, t, &[Span::raw(" ")], end + 1);
    }
}

/// What the divider says of the agent you view while it works (book §8,
/// BISE-105): the gust's motion and the current turn's age (`42s`).
pub(crate) struct Working {
    pub(crate) motion: gust::Motion,
    pub(crate) age: Option<String>,
}

/// How much of the label the divider shows, while the agent works (book
/// §9 "Short on room (the gust)"): the steps in the order they are
/// dropped, the right-side state first.
#[derive(Clone, Copy)]
struct Step {
    state: bool,
    words: bool,
    size: gust::Size,
    age: bool,
    name_cut: Option<usize>,
}

const STEPS: [Step; 8] = {
    use gust::Size::{Five, One, Three};
    let full = Step { state: true, words: true, size: Five, age: true, name_cut: None };
    [
        full,
        Step { state: false, ..full },
        Step { state: false, words: false, ..full },
        Step { state: false, words: false, size: Three, ..full },
        Step { state: false, words: false, size: One, ..full },
        Step { state: false, words: false, size: One, age: false, name_cut: None },
        Step { state: false, words: false, size: One, age: false, name_cut: Some(20) },
        Step { state: false, words: false, size: One, age: false, name_cut: Some(12) },
    ]
};

/// The divider's label: ` you → name `; while the agent works, the gust
/// 1 space after the name, then `working · 42s` dim (as much of it as
/// `step` keeps).
fn label(name: &str, working: Option<(&Working, Step)>) -> Vec<Span<'static>> {
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    let name = match working.and_then(|(_, s)| s.name_cut) {
        Some(n) => fit(vec![Span::raw(name.to_string())], n).into_iter().map(|s| s.content.into_owned()).collect(),
        None => name.to_string(),
    };
    let mut out = vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), Style::default().fg(dim())),
        Span::styled(name, Style::default().fg(accent())),
    ];
    if let Some((w, step)) = working {
        out.push(Span::raw(" "));
        out.extend(gust::mark(w.motion, step.size));
        let words = match (step.words, step.age.then_some(w.age.as_deref()).flatten()) {
            (true, Some(age)) => format!(" working · {}", age),
            (true, None) => " working".to_string(),
            (false, Some(age)) => format!(" {}", age),
            (false, None) => String::new(),
        };
        if !words.is_empty() {
            out.push(Span::styled(words, Style::default().fg(dim())));
        }
    }
    out.push(Span::raw(" "));
    out
}

fn width_of(spans: &[Span]) -> u16 {
    spans.iter().map(|s| s.content.width() as u16).sum()
}

/// The columns the divider leaves for its right side on a screen `width`
/// wide: from 1 rule cell and a space after the label to the state's end.
/// While the agent works, the label is the whole one (its first step).
pub(crate) fn divider_room(width: u16, cols: Cols, name: &str, working: Option<&Working>) -> u16 {
    let label_w = width_of(&label(name, working.map(|w| (w, STEPS[0]))));
    let start = cols.margin - 1 + label_w + 2;
    let end = width.saturating_sub(cols.margin);
    end.saturating_sub(start)
}

/// The divider on row `y` (book §8): framed, a rule joining the frame
/// (`├ … ┤`, `┴` under the panel's rule); bare, a plain rule. The label
/// ` you → name ` from the margin, the `state` (cut to fit) ending 1
/// column before the right margin's space, 3 columns at least between
/// them. While the agent works, the label says it (` you → name ≈∿~·
/// working · 42s `) and keeps its gust: short on room, the state goes
/// whole first, then the label shrinks step by step ([`STEPS`]). The
/// state's rect is returned (a click on `↓ back to the bottom` jumps to
/// the tail), then the label's (zen keeps it, BISE-121).
pub(crate) fn draw_divider(
    buf: &mut Buffer,
    area: Rect,
    cols: Cols,
    y: u16,
    name: &str,
    working: Option<&Working>,
    state: Vec<Span<'static>>,
) -> (Rect, Rect) {
    let area = area.intersection(buf.area);
    if area.width < 4 || y < area.y || y >= area.bottom() {
        return (Rect::default(), Rect::default());
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
    let lx = l + cols.margin - 1;
    // the state's end: framed F − 4 (F − 3 its space), bare the last column
    let end = r + 1 - cols.margin;
    let (label, state) = match working {
        None => {
            let room = divider_room(area.width, cols, name, None);
            (label(name, None), fit(state, room as usize))
        }
        Some(wk) => {
            let state_w = width_of(&state);
            let fits = |label: &[Span], with_state: bool| {
                let w = u32::from(lx) + u32::from(width_of(label));
                if with_state && state_w > 0 {
                    w + 2 + u32::from(state_w) <= u32::from(end)
                } else {
                    w <= u32::from(end) + 1
                }
            };
            let pick = STEPS.iter().find(|s| fits(&label(name, Some((wk, **s))), s.state)).unwrap_or(&STEPS[STEPS.len() - 1]);
            (label(name, Some((wk, *pick))), if pick.state { state } else { Vec::new() })
        }
    };
    put(buf, lx, y, &label, end + 1);
    let label_rect = Rect { x: lx, y, width: width_of(&label).min((end + 1).saturating_sub(lx)), height: 1 };
    let w = width_of(&state);
    if w == 0 {
        return (Rect::default(), label_rect);
    }
    let x = end - w;
    put(buf, x - 1, y, &[Span::raw(" ")], end);
    put(buf, x, y, &state, end);
    if end <= r {
        put(buf, end, y, &[Span::raw(" ")], end + 1);
    }
    (Rect { x, y, width: w, height: 1 }, label_rect)
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

    fn divider_row_of(width: u16, name: &str, working: Option<&Working>, state: &str) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        let cols = crate::layout::cols(width, 40);
        draw_divider(&mut buf, area, cols, 0, name, working, vec![Span::raw(state.to_string())]);
        (0..width).map(|x| buf[(x, 0)].symbol().to_string()).collect()
    }

    fn divider_row(width: u16, working: Option<&Working>, state: &str) -> String {
        divider_row_of(width, "marketing", working, state)
    }

    /// Frame 3 of the gust: 5 cells `·~∿≈ `, 3 cells `·~∿`, 1 cell `≈`.
    fn working(age: Option<&str>) -> Working {
        Working { motion: gust::Motion::Frame(3), age: age.map(String::from) }
    }

    #[test]
    fn the_divider_says_the_viewed_agent_works() {
        // BISE-105: the gust 1 space after the name, then the words; the
        // right side unchanged
        let row = divider_row(100, Some(&working(Some("42s"))), "working · 42s · 18k tokens");
        assert!(row.starts_with("├─ you → marketing ·~∿≈  working · 42s ─"), "{row}");
        assert!(row.ends_with("─ working · 42s · 18k tokens ─┤"), "{row}");
        // no age yet: the word alone
        let row = divider_row(100, Some(&working(None)), "");
        assert!(row.starts_with("├─ you → marketing ·~∿≈  working ─"), "{row}");
        // no motion: one static wave
        let still = Working { motion: gust::Motion::Still, age: Some("42s".into()) };
        let row = divider_row(100, Some(&still), "idle");
        assert!(row.starts_with("├─ you → marketing ∿ working · 42s ─"), "{row}");
    }

    #[test]
    fn the_divider_says_nothing_after_the_name_when_idle() {
        let row = divider_row(100, None, "idle · 18k tokens");
        assert!(row.starts_with("├─ you → marketing ───"), "{row}");
        assert!(row.ends_with("─ idle · 18k tokens ─┤"), "{row}");
    }

    #[test]
    fn short_on_room_the_divider_drops_in_the_book_order() {
        // book §9 "Short on room (the gust)"; bare rows (F < 60) start at
        // column 0 and end at the last one
        let w = working(Some("42s"));
        let state = "18k tokens";
        let row = divider_row(50, Some(&w), state);
        assert!(row.starts_with(" you → marketing ·~∿≈  working · 42s ─") && row.ends_with("─ 18k tokens "), "{row}");
        // (1) the state
        let row = divider_row(49, Some(&w), state);
        assert!(row.starts_with(" you → marketing ·~∿≈  working · 42s ─") && !row.contains("18k"), "{row}");
        // (2) the word
        let row = divider_row(36, Some(&w), state);
        assert_eq!(row, format!(" you → marketing ·~∿≈  42s {}", "─".repeat(9)));
        // (3) 5 cells → 3
        assert_eq!(divider_row(26, Some(&w), state), " you → marketing ·~∿ 42s ─");
        // (4) → the breath
        assert_eq!(divider_row(24, Some(&w), state), " you → marketing ≈ 42s ─");
        // (5) the seconds
        assert_eq!(divider_row(22, Some(&w), state), " you → marketing ≈ ───");
        // (6) last: the name at 20, then 12 (BISE-109, was 12 then 8)
        let name = "release-notes-writer-v2";
        assert!(divider_row_of(33, name, Some(&w), state).starts_with(" you → release-notes-writer-v2 ≈ "));
        assert!(divider_row_of(32, name, Some(&w), state).starts_with(" you → release-notes-write… ≈ ─"));
        assert!(divider_row_of(30, name, Some(&w), state).starts_with(" you → release-notes-write… ≈ "));
        assert_eq!(divider_row_of(29, name, Some(&w), state), format!(" you → release-not… ≈ {}", "─".repeat(7)));
        assert_eq!(divider_row_of(22, name, Some(&w), state), " you → release-not… ≈ ");
    }

    #[test]
    fn the_gust_stays_while_the_agent_works_at_any_width() {
        let w = working(Some("42s"));
        for width in 20..130 {
            let row = divider_row(width, Some(&w), "working · 42s · 18k / 1M tokens · 2%");
            assert_eq!(row.chars().count(), width as usize, "{width}: {row}");
            assert!(row.contains("marketing ≈") || row.contains("marketing ·~∿"), "{width}: {row}");
            // the state is whole or gone, never cut
            assert!(row.contains("2%") || !row.contains("tokens"), "{width}: {row}");
        }
    }
}
