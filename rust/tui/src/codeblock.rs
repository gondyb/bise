//! The fenced code blocks of the feed's markdown (a reply, a brief, a
//! report): a rounded box like the bash / TypeScript box (toolbox.rs), the
//! fence's language in its top border (`╭─ ts ───╮`), the lines colored by
//! syntax.rs (the composer's highlighter), a long line wrapped with `»`.
//!
//! Copy: the mouse over a block shows a copy icon on its top border (on
//! its first row on screen when the border is scrolled off); a click on
//! it copies the block's code as written (clipboard.rs) and the icon says
//! `✓ copied` for a moment. ctrl+y copies the block under the mouse, else
//! the newest block on screen. The mouse moves are already reported (any-
//! motion tracking, run.rs; the turn's hover time, BISE-271): the icon is
//! drawn over the frame, the cached rows never change.
//!
//! Where the blocks are: [`lines`] notes each block it draws while an
//! event's rows are built ([`collect`]); [`locate`] then finds each top
//! border among the event's rows (a reply's rows carry a lead, a brief's
//! a bar), so a screen row gives back its block and its code.

use crate::app::App;
use crate::theme::{self, *};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::cell::RefCell;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// How long the icon says `copied` after a copy.
pub(crate) const COPIED_FOR: Duration = Duration::from_millis(1500);

/// A block among an event's rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    /// its top border's row in the event's rows
    pub(crate) row: usize,
    /// its rows, both borders included
    pub(crate) len: usize,
    /// the column of its left corner, and its width
    pub(crate) x: usize,
    pub(crate) w: usize,
    /// the code, as the message wrote it (tabs kept)
    pub(crate) code: String,
}

/// A block [`lines`] drew while the rows were built.
pub(crate) struct Found {
    top: String,
    len: usize,
    code: String,
}

thread_local! {
    static COLLECT: RefCell<Option<Vec<Found>>> = const { RefCell::new(None) };
}

/// Runs `f` (the build of an event's rows) and returns what it built and
/// the blocks [`lines`] drew in it, in order.
pub(crate) fn collect<T>(f: impl FnOnce() -> T) -> (T, Vec<Found>) {
    let saved = COLLECT.with(|c| c.borrow_mut().replace(Vec::new()));
    let out = f();
    let found = COLLECT.with(|c| std::mem::replace(&mut *c.borrow_mut(), saved)).unwrap_or_default();
    (out, found)
}

/// The box of one block, `width` columns: the top border with the
/// fence's tag (`ts`), the highlighted lines `hl` wrapped inside, the
/// bottom border. `code` is what a copy gives.
pub(crate) fn lines(tag: &str, hl: &[Vec<Span<'static>>], code: String, width: usize) -> Vec<Line<'static>> {
    let f = crate::toolbox::frame();
    let width = width.max(12);
    let inner = width - 4;
    let bst = Style::default().fg(faint());
    let mut top = vec![Span::styled(format!("{}{}", f.tl, f.h), bst)];
    if !tag.is_empty() {
        // a long tag is cut: two fill cells and the corner stay
        let label = crate::render::fit_chars(tag, width.saturating_sub(7));
        top.push(Span::styled(format!(" {} ", label), Style::default().fg(dim())));
    }
    let used: usize = top.iter().map(|s| s.content.width()).sum();
    top.push(Span::styled(format!("{}{}", f.h.repeat(width.saturating_sub(used + 1)), f.tr), bst));
    let top = Line::from(top);
    let mut rows = vec![top.clone()];
    // a wrapped line goes on after `» `; in ASCII after two blanks (the
    // designer: `}` reads as code inside code)
    let hang = if theme::ascii_mode() { "  ".to_string() } else { format!("{} ", G_WRAP) };
    let hang = Span::styled(hang, Style::default().fg(faint()));
    if hl.is_empty() {
        rows.extend(crate::toolbox::wrap_one_with(&[], inner, bst, hang.clone()));
    }
    for l in hl {
        rows.extend(crate::toolbox::wrap_one_with(l, inner, bst, hang.clone()));
    }
    rows.push(Line::from(Span::styled(format!("{}{}{}", f.bl, f.h.repeat(width - 2), f.br), bst)));
    let found = Found { top: crate::feedsel::line_text(&top), len: rows.len(), code };
    COLLECT.with(|c| {
        if let Some(v) = c.borrow_mut().as_mut() {
            v.push(found);
        }
    });
    rows
}

/// Where the blocks `found` are among an event's `rows`: each top border,
/// in order, the first row from the block before that ends with it.
pub(crate) fn locate(rows: &[Line], found: Vec<Found>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut at = 0;
    for f in found {
        let hit = (at..rows.len()).find_map(|r| {
            let t = crate::feedsel::line_text(&rows[r]);
            t.ends_with(&f.top).then(|| (r, t.width() - f.top.width()))
        });
        let Some((row, x)) = hit else { break };
        let len = f.len.min(rows.len() - row);
        out.push(Block { row, len, x, w: f.top.width(), code: f.code });
        at = row + len;
    }
    out
}

// ---- the copy icon ----

/// The copy icon the last frame drew: its cells and its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub(crate) rect: Rect,
    pub(crate) event: usize,
    pub(crate) block: usize,
}

/// The last copy: its block and when (the icon says `copied`).
#[derive(Clone, Debug)]
pub(crate) struct Copied {
    event: usize,
    block: usize,
    code: String,
    at: Instant,
}

/// The icon, on the mouse over a block: the word, in every mode (the
/// designer: a glyph like ⧉ is missing from many fonts, one more to learn).
pub(crate) const ICON: &str = " copy ";

/// The icon after a copy.
pub(crate) fn copied_label() -> &'static str {
    if theme::ascii_mode() {
        " copied "
    } else {
        " ✓ copied "
    }
}

fn block(app: &App, i: usize, k: usize) -> Option<&Block> {
    app.cache.get(i)?.as_ref()?.blocks.get(k)
}

/// The block at a screen row (of `vis_events`/`vis_rows`) and column.
fn block_at(app: &App, area: Rect, vis: (&[usize], &[usize]), (x, y): (u16, u16)) -> Option<(usize, usize)> {
    if x < area.x || x >= area.right() || y < area.y {
        return None;
    }
    let row = (y - area.y) as usize;
    let (&i, &ri) = (vis.0.get(row)?, vis.1.get(row)?);
    let col = (x - area.x) as usize;
    let er = app.cache.get(i)?.as_ref()?;
    let k = er.blocks.iter().position(|b| (b.row..b.row + b.len).contains(&ri) && (b.x..b.x + b.w).contains(&col))?;
    Some((i, k))
}

/// The newest block on screen (its lowest row first).
fn last_on_screen(app: &App) -> Option<(usize, usize)> {
    app.vis_events.iter().zip(&app.vis_rows).rev().find_map(|(&i, &ri)| {
        let er = app.cache.get(i)?.as_ref()?;
        let k = er.blocks.iter().position(|b| (b.row..b.row + b.len).contains(&ri))?;
        Some((i, k))
    })
}

/// A block shows on screen (the ctrl hints say ctrl+y).
pub(crate) fn any_on_screen(app: &App) -> bool {
    last_on_screen(app).is_some()
}

/// Draws the copy icon over the frame: `copied` on the block just
/// copied, the icon on the block under the mouse. Sets `app.copy_hit`
/// (a click there copies) and the hand over it.
pub(crate) fn draw(app: &mut App, buf: &mut Buffer, area: Rect, vis_events: &[usize], vis_rows: &[usize]) {
    app.copy_hit = None;
    let vis = (vis_events, vis_rows);
    let copied = app
        .code_copied
        .as_ref()
        .filter(|c| c.at.elapsed() < COPIED_FOR)
        .filter(|c| block(app, c.event, c.block).is_some_and(|b| b.code == c.code))
        .map(|c| (c.event, c.block));
    if let Some((i, k)) = copied {
        let st = Style::default().fg(accent());
        label_at(app, buf, area, vis, (i, k), copied_label(), st);
    }
    let hovered = app.hover.and_then(|p| block_at(app, area, vis, p)).filter(|h| Some(*h) != copied);
    if let Some((i, k)) = hovered {
        let st = Style::default().fg(text()).add_modifier(Modifier::BOLD);
        if let Some(rect) = label_at(app, buf, area, vis, (i, k), ICON, st) {
            app.copy_hit = Some(Hit { rect, event: i, block: k });
            crate::pointer::region(rect, crate::pointer::Shape::Pointer);
        }
    }
}

/// `label` at the right of block `(i, k)`'s top border, before `─╮`
/// (its first row on screen when the border is off it). Its cells.
fn label_at(app: &App, buf: &mut Buffer, area: Rect, vis: (&[usize], &[usize]), (i, k): (usize, usize), label: &str, st: Style) -> Option<Rect> {
    let b = block(app, i, k)?;
    let y = vis.0.iter().zip(vis.1).position(|(&e, &r)| e == i && (b.row..b.row + b.len).contains(&r))?;
    let lw = label.width();
    // `─╮` stay; the tag (`╭─ ts `) is never covered
    let end = b.x + b.w - 2;
    let x0 = end.checked_sub(lw).filter(|&x| x >= b.x + 2)?;
    let (x0, y) = (area.x + x0 as u16, area.y + y as u16);
    if x0 + lw as u16 > area.right() || y >= area.bottom() {
        return None;
    }
    buf.set_string(x0, y, label, st);
    Some(Rect { x: x0, y, width: lw as u16, height: 1 })
}

/// A press on the icon the last frame drew: copies its block. True when
/// the press was the icon's.
pub(crate) fn click(app: &mut App, x: u16, y: u16) -> bool {
    let Some(h) = app.copy_hit.filter(|h| h.rect.contains((x, y).into())) else { return false };
    copy(app, h.event, h.block);
    true
}

/// ctrl+y: copies the block under the mouse, else the newest on screen.
pub(crate) fn copy_key(app: &mut App) {
    match app.copy_hit.map(|h| (h.event, h.block)).or_else(|| last_on_screen(app)) {
        Some((i, k)) => copy(app, i, k),
        None => app.flash = Some(("no code block on screen".to_string(), Instant::now())),
    }
}

fn copy(app: &mut App, i: usize, k: usize) {
    let Some(code) = block(app, i, k).map(|b| b.code.clone()) else { return };
    let note = crate::textlayer::copy_note(&code);
    if note.starts_with("copied") {
        app.code_copied = Some(Copied { event: i, block: k, code, at: Instant::now() });
    }
    app.flash = Some((note, Instant::now()));
}
