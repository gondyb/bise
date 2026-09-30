//! cmd+k (ctrl+s, `/switch`): find an agent by name and open it
//! (BISE-265, book §16 "Switch agents").
//!
//! The palette takes the composer pane, like the find field: the list
//! grows upward from the query row, the draft waits and comes back on
//! esc. Every agent of the hub: the live ones (main first, then the
//! panel's order; with a query, the best match first), then under
//! `earlier` the archived ones the query names (never with an empty
//! query). ⏎ opens the agent's view, as ⌥+number does; an archived
//! one opens its read-only history.
//!
//! A name matches, best first: its start, a word of it (`mode` in
//! `dark-mode`), anywhere in it, its initials (`ap` agent-palette),
//! its letters in order (`dkmd`); then the query in its objective, its
//! note or its last report (no highlight).

use super::*;
use crossterm::event::KeyEvent;
use unicode_width::UnicodeWidthStr;

/// The list shows at most this many rows (live, `earlier`, archived);
/// it scrolls past them.
const MAX_ROWS: usize = 12;
/// The name column is at most this wide (a longer name is cut).
const NAME_MAX: usize = 24;

/// The open palette.
#[derive(Debug, Default)]
pub(crate) struct Palette {
    pub(crate) query: String,
    /// the selected entry of [`items`]
    sel: usize,
    /// the first list row drawn (the list scrolls)
    top: usize,
    /// set by the last draw: the screen row of each entry (a click opens)
    hits: Vec<(u16, usize)>,
}

/// One agent the query names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Item {
    pub(super) name: String,
    pub(super) archived: bool,
    /// the chars of the name the query matched (their indices)
    pub(super) hits: Vec<usize>,
}

/// `s` lowered char by char, one char for one (the indices stay).
fn low(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// How well `q` (lowered, trimmed, not empty) names an agent called
/// `name`: its rank (0 best) and the chars of the name it matched;
/// `extra` (objective, note, report) is the last resort.
pub(super) fn score(name: &str, extra: &[&str], q: &[char]) -> Option<(u8, Vec<usize>)> {
    let n = low(name);
    let len = q.len();
    let word_start = |i: usize| i == 0 || !n[i - 1].is_alphanumeric();
    let at: Vec<usize> = if len <= n.len() { (0..=n.len() - len).filter(|&i| n[i..i + len] == *q).collect() } else { Vec::new() };
    let run = |i: usize| (i..i + len).collect::<Vec<_>>();
    if at.first() == Some(&0) {
        return Some((0, run(0)));
    }
    if let Some(&i) = at.iter().find(|&&i| word_start(i)) {
        return Some((1, run(i)));
    }
    if let Some(&i) = at.first() {
        return Some((2, run(i)));
    }
    let starts: Vec<usize> = (0..n.len()).filter(|&i| n[i].is_alphanumeric() && word_start(i)).collect();
    if len >= 2 && starts.len() >= len && starts.iter().zip(q).all(|(&i, c)| n[i] == *c) {
        return Some((3, starts[..len].to_vec()));
    }
    if len >= 2 {
        let mut from = 0;
        let mut idx = Vec::with_capacity(len);
        for c in q {
            match (from..n.len()).find(|&i| n[i] == *c) {
                Some(i) => {
                    idx.push(i);
                    from = i + 1;
                }
                None => break,
            }
        }
        if idx.len() == len {
            return Some((4, idx));
        }
    }
    let q: String = q.iter().collect();
    extra.iter().any(|f| low(f).into_iter().collect::<String>().contains(&q)).then(|| (5, Vec::new()))
}

/// The agents `query` names, in the palette's order: the live ones, then
/// the archived ones (only with a query).
pub(super) fn items(sb: &Sb, query: &str) -> Vec<Item> {
    let numbers = sb.numbers();
    let num = |a: &Agent| numbers.iter().find(|(n, _)| *n == a.name).map_or(usize::MAX, |(_, k)| *k);
    let live = sb.agents.iter().filter(|a| !a.archived() && !a.name.is_empty());
    let q = low(query.trim());
    if q.is_empty() {
        let mut out: Vec<&Agent> = live.collect();
        out.sort_by_key(|a| (!a.main, num(a)));
        return out.into_iter().map(|a| Item { name: a.name.clone(), archived: false, hits: Vec::new() }).collect();
    }
    let hit = |a: &Agent| score(&a.name, &[&a.objective, &a.role, &a.report, &a.note], &q);
    let mut out: Vec<(u8, bool, usize, Item)> = live
        .filter_map(|a| {
            let (rank, hits) = hit(a)?;
            // an agent waiting on you wins a tie
            let item = Item { name: a.name.clone(), archived: false, hits };
            Some((rank, a.main || !panel::needs_you(sb, a), num(a), item))
        })
        .collect();
    out.sort_by_key(|(rank, calm, n, _)| (*rank, *calm, *n));
    let mut old: Vec<(u8, Item)> = sb
        .archived()
        .into_iter()
        .filter_map(|a| hit(a).map(|(rank, hits)| (rank, Item { name: a.name.clone(), archived: true, hits })))
        .collect();
    // stable: the most recent first within a rank
    old.sort_by_key(|(rank, _)| *rank);
    out.into_iter().map(|(_, _, _, i)| i).chain(old.into_iter().map(|(_, i)| i)).collect()
}

/// The palette is open.
pub(crate) fn is_open(app: &App) -> bool {
    app.palette.is_some()
}

/// Open the palette on `query` (`/switch dark`); the find field closes.
pub(crate) fn open(app: &mut App, query: &str) {
    crate::find::close(app);
    leave_inbox(app);
    app.palette = Some(Palette { query: query.trim().to_string(), ..Palette::default() });
}

fn close(app: &mut App) {
    app.palette = None;
}

/// Open entry `i` of the list (the palette closes).
fn pick(app: &mut App, i: usize) {
    let Some(p) = app.palette.as_ref() else { return };
    let Some(item) = items(&app.sb, &p.query).into_iter().nth(i) else { return };
    close(app);
    focus(app, &item.name);
}

/// cmd+k (SUPER under the kitty keyboard protocol, e.g. Ghostty with
/// `keybind = super+k=unbind`) or ctrl+s, in any terminal.
pub(crate) fn opens(k: &KeyEvent) -> bool {
    matches!((k.code, k.modifiers), (KeyCode::Char('k'), KeyModifiers::SUPER) | (KeyCode::Char('s'), KeyModifiers::CONTROL))
}

/// The palette's keys; `true` when handled. Open, it takes every key but
/// the feed's scroll keys.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(p) = app.palette.as_mut() else {
        if opens(k) {
            open(app, "");
            return true;
        }
        return false;
    };
    let n = items(&app.sb, &p.query).len();
    let edited = |p: &mut Palette| {
        p.sel = 0;
        p.top = 0;
    };
    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => close(app),
        _ if opens(k) => close(app),
        (KeyCode::Enter, _) => {
            let sel = p.sel;
            pick(app, sel);
        }
        (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) | (KeyCode::BackTab, _) => {
            p.sel = if p.sel == 0 { n.saturating_sub(1) } else { p.sel - 1 };
        }
        (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) | (KeyCode::Tab, _) => {
            p.sel = if p.sel + 1 >= n { 0 } else { p.sel + 1 };
        }
        (KeyCode::PageUp | KeyCode::PageDown | KeyCode::End | KeyCode::Home, _) => return false,
        (KeyCode::Backspace, m) if m.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) => {
            word(&mut p.query);
            edited(p);
        }
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            word(&mut p.query);
            edited(p);
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            p.query.clear();
            edited(p);
        }
        (KeyCode::Backspace, _) => {
            p.query.pop();
            edited(p);
        }
        (KeyCode::Char(c), m) if !m.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
            p.query.push(c);
            edited(p);
        }
        _ => {}
    }
    true
}

/// The query's last word goes (ctrl+w, option+backspace).
fn word(q: &mut String) {
    let t = q.trim_end().len();
    let cut = q[..t].char_indices().rfind(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8());
    q.truncate(cut);
}

/// A paste while the palette is open goes to the query (one line).
pub(crate) fn on_paste(app: &mut App, text: &str) -> bool {
    let Some(p) = app.palette.as_mut() else { return false };
    p.query.push_str(&text.split_whitespace().collect::<Vec<_>>().join(" "));
    p.sel = 0;
    p.top = 0;
    true
}

/// A click on an entry opens it; any click elsewhere closes the palette
/// and does its job.
pub(crate) fn on_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(p) = app.palette.as_ref() else { return false };
    if m.kind != MouseEventKind::Down(MouseButton::Left) {
        return false;
    }
    match p.hits.iter().find(|(y, _)| *y == m.row) {
        Some(&(_, i)) if m.column >= app.composer.x.saturating_sub(3) => {
            pick(app, i);
            true
        }
        _ => {
            close(app);
            false
        }
    }
}

/// The rows the palette wants in the composer pane: its list, a blank
/// row, the query row.
pub(crate) fn rows_wanted(app: &App) -> u16 {
    let Some(p) = app.palette.as_ref() else { return 1 };
    (list_rows(&items(&app.sb, &p.query)).min(MAX_ROWS) + 2) as u16
}

/// The list's rows for `items`: the live ones; a blank row, `earlier`
/// and the archived ones; `no agent called …` when none.
fn list_rows(items: &[Item]) -> usize {
    let old = items.iter().filter(|i| i.archived).count();
    let live = items.len() - old;
    if items.is_empty() {
        1
    } else if old == 0 {
        live
    } else {
        live + usize::from(live > 0) + 1 + old
    }
}

/// The divider's label while the palette is open: ` you → find an agent `.
pub(crate) fn divider_label(app: &App) -> Option<Vec<Span<'static>>> {
    app.palette.as_ref()?;
    let arrow = if theme::ascii_mode() { "->" } else { "→" };
    Some(vec![
        Span::raw(" "),
        Span::styled(format!("you {} ", arrow), Style::default().fg(dim())),
        Span::styled("find an agent", Style::default().fg(accent())),
        Span::raw(" "),
    ])
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// Draw the palette in the composer's place: the bar in accent, the
/// list, a blank row, the query (`area`: the composer pane, its text
/// `inner` columns from `lead`).
pub(crate) fn draw(app: &mut App, frame: &mut Frame, area: Rect, inner: usize, lead: u16, pad_top: u16) {
    let Some(query) = app.palette.as_ref().map(|p| p.query.clone()) else { return };
    // a short terminal: the pane may hang below the screen
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    let items = items(&app.sb, &query);
    let text_y = area.y + pad_top.min(area.height.saturating_sub(1));
    let text_h = area.bottom().saturating_sub(text_y);
    // the query is the pane's last text row; the list above it, 1 blank row between
    let list_h = text_h.saturating_sub(2) as usize;
    let query_y = text_y + text_h.saturating_sub(1);
    app.composer = ComposerArea { x: area.x + lead, y: query_y, w: inner.max(1), h: 1, top: 0 };
    let bar = Span::styled(format!("│{}", " ".repeat(lead.saturating_sub(1) as usize)), Style::default().fg(accent()));
    // the list: (row, the entry it opens)
    let mut list: Vec<(Line<'static>, Option<usize>)> = Vec::new();
    let name_w = items.iter().map(|i| i.name.width()).max().unwrap_or(0).min(NAME_MAX);
    let p = app.palette.as_ref().map_or(0, |p| p.sel);
    let sel = p.min(items.len().saturating_sub(1));
    if items.is_empty() {
        let what = if app.sb.agents.is_empty() { "no agents yet".to_string() } else { format!("no agent called “{}”", query.trim()) };
        list.push((Line::from(Span::styled(format!("  {} · esc closes", what), Style::default().fg(dim()))), None));
    }
    let first_old = items.iter().position(|i| i.archived);
    for (i, item) in items.iter().enumerate() {
        if Some(i) == first_old {
            if i > 0 {
                list.push((Line::default(), None));
            }
            list.push((Line::from(Span::styled("  earlier · read-only", Style::default().fg(faint()))), None));
        }
        list.push((row(app, item, i == sel, name_w, inner), Some(i)));
    }
    // scroll: the selected entry stays in view
    let sel_row = list.iter().position(|(_, e)| *e == Some(sel)).unwrap_or(0);
    let h = list_h.max(1);
    let top = {
        let p = app.palette.as_mut().expect("open");
        if sel_row < p.top {
            p.top = sel_row;
        } else if sel_row >= p.top + h {
            p.top = sel_row + 1 - h;
        }
        // the earlier label stays with the first archived entry
        if first_old.is_some_and(|f| Some(f) == Some(sel)) {
            p.top = p.top.min(sel_row.saturating_sub(1));
        }
        p.top = p.top.min(list.len().saturating_sub(h));
        p.top
    };
    let shown: Vec<(Line<'static>, Option<usize>)> = list.into_iter().skip(top).take(list_h).collect();
    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    for _ in area.y..text_y {
        lines.push(Line::from(bar.clone()));
    }
    // the list sits on the query: blank rows above it when short
    let blank_above = list_h.saturating_sub(shown.len());
    for _ in 0..blank_above {
        lines.push(Line::from(bar.clone()));
    }
    let mut hits = Vec::new();
    for (k, (l, e)) in shown.into_iter().enumerate() {
        if let Some(e) = e {
            hits.push((text_y + (blank_above + k) as u16, e));
        }
        let mut spans = vec![bar.clone()];
        spans.extend(l.spans);
        lines.push(Line::from(spans));
    }
    while (lines.len() as u16) < query_y.saturating_sub(area.y) {
        lines.push(Line::from(bar.clone()));
    }
    let mut q = vec![bar.clone()];
    q.extend(query_row(&query, inner, items.len()).spans);
    lines.push(Line::from(q));
    while lines.len() < area.height as usize {
        lines.push(Line::from(bar.clone()));
    }
    // BISE-272: the hand over the entries (a click there picks, see `on_mouse`)
    let from = (area.x + lead).saturating_sub(3);
    for (y, _) in &hits {
        let r = Rect { x: from, y: *y, width: area.right().saturating_sub(from), height: 1 };
        crate::pointer::region(r.intersection(frame.area()), crate::pointer::Shape::Pointer);
    }
    if let Some(p) = app.palette.as_mut() {
        p.hits = hits;
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// One entry: `▸` when selected, the status mark, the name (the matched
/// chars bold in accent), what it does (dim, cut), its ⌥ number on the right.
fn row(app: &App, item: &Item, selected: bool, name_w: usize, w: usize) -> Line<'static> {
    let sb = &app.sb;
    let Some(a) = sb.agent(&item.name) else { return Line::default() };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let pointer = if theme::ascii_mode() { ">" } else { "▸" };
    spans.push(Span::styled(
        if selected { format!("{} ", pointer) } else { "  ".into() },
        Style::default().fg(accent()).add_modifier(Modifier::BOLD),
    ));
    let (g, color) = if a.main {
        (G_MAIN, accent())
    } else if item.archived {
        (G_STOPPED, faint())
    } else if panel::needs_you(sb, a) {
        (G_NEEDS_YOU, accent())
    } else {
        glyph(&a.status, app.tick, app.motion_away)
    };
    let g = theme::glyph(g);
    spans.push(Span::styled(format!("{:<2} ", g), Style::default().fg(color)));
    let mut used = 2 + g.width().max(2) + 1;
    if !a.main && panel::place_label(a).is_some() {
        spans.push(Span::styled(format!("{} ", theme::glyph(G_WORKTREE)), Style::default().fg(dim())));
        used += 2;
    }
    let base = if item.archived { Style::default().fg(dim()) } else { Style::default().fg(text()) };
    let base = if selected { base.add_modifier(Modifier::BOLD) } else { base };
    let hit = if no_color() { base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED) } else { base.fg(accent()).add_modifier(Modifier::BOLD) };
    // the ⌥ number (live agents 0-9), then the note in what is left
    let key = if item.archived {
        String::new()
    } else {
        sb.numbers().into_iter().find(|(n, _)| *n == a.name).filter(|(_, k)| *k <= 9).map_or(String::new(), |(_, k)| {
            if theme::ascii_mode() {
                format!("alt+{}", k)
            } else {
                format!("⌥{}", k)
            }
        })
    };
    let key = if w < 40 { String::new() } else { key };
    let key_w = if key.is_empty() { 0 } else { key.width() + 2 };
    let room = w.saturating_sub(used + key_w);
    let name_w = name_w.min(room);
    let cut = crate::fit_chars(&a.name, name_w);
    let mut buf = String::new();
    let mut on = false;
    let mut name_spans = Vec::new();
    for (i, c) in cut.chars().enumerate() {
        let h = item.hits.contains(&i);
        if h != on && !buf.is_empty() {
            name_spans.push(Span::styled(std::mem::take(&mut buf), if on { hit } else { base }));
        }
        on = h;
        buf.push(c);
    }
    if !buf.is_empty() {
        name_spans.push(Span::styled(buf, if on { hit } else { base }));
    }
    spans.extend(name_spans);
    used += cut.width();
    let pad = name_w.saturating_sub(cut.width());
    let note = if a.main && a.role.is_empty() && a.objective.is_empty() {
        "your team lead".to_string()
    } else if item.archived {
        first_line(if a.report.is_empty() { &a.objective } else { &a.report })
    } else {
        first_line(if a.role.is_empty() { &a.objective } else { &a.role })
    };
    let note_room = w.saturating_sub(used + pad + 2 + key_w);
    if !note.is_empty() && note_room >= 8 {
        let n = crate::fit_chars(&note, note_room);
        spans.push(Span::raw(" ".repeat(pad + 2)));
        used += pad + 2 + n.width();
        spans.push(Span::styled(n, Style::default().fg(if item.archived { faint() } else { dim() })));
    }
    if !key.is_empty() {
        let gap = w.saturating_sub(used + key.width()).max(2);
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(key, Style::default().fg(faint())));
    }
    Line::from(spans)
}

/// `s` on one line, its blanks squeezed.
fn first_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The query row: `dar▏` (a dim placeholder when empty), the count on
/// the right, dim.
fn query_row(query: &str, w: usize, n: usize) -> Line<'static> {
    let d = Style::default().fg(dim());
    let cursor = Span::styled(" ", Style::default().fg(text()).add_modifier(Modifier::REVERSED));
    let count = if query.trim().is_empty() {
        String::new()
    } else if n == 1 {
        "1 agent".to_string()
    } else {
        format!("{} agents", n)
    };
    let room = w.saturating_sub(count.width() + 2).max(1);
    let mut spans = Vec::new();
    let used = if query.is_empty() {
        spans.push(cursor);
        let p: String = " type part of a name".chars().take(room.saturating_sub(1)).collect();
        let n = 1 + p.width();
        spans.push(Span::styled(p, d));
        n
    } else {
        let shown = crate::ui::truncate_left(query, room.saturating_sub(1));
        let n = shown.width() + 1;
        spans.push(Span::styled(shown, Style::default().fg(text())));
        spans.push(cursor);
        n
    };
    if !count.is_empty() {
        spans.push(Span::raw(" ".repeat(w.saturating_sub(used + count.width()).max(1))));
        spans.push(Span::styled(count, d));
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
