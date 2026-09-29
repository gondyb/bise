//! The chrome of the switchboard mode: the header row, the agents panel
//! on the right, the status row, the key hints and the composer
//! placeholder (bise book §8, §17).

use super::*;
use unicode_width::UnicodeWidthStr;

/// The status glyph of an agent and its color (book §6): `∿` pulses
/// while it works, `·` while it starts; only "needs you" and a failure
/// get a hue.
pub(super) fn glyph(status: &str, tick: u32) -> (&'static str, Color) {
    match status {
        "working" => working_frame(tick),
        "starting" => starting_frame(tick),
        "waiting" => (G_WAITING, text()),
        "blocked" => (G_NEEDS_YOU, accent()),
        "done" => (G_DONE, text()),
        "failed" => (G_FAILED, error()),
        "idle" => (G_IDLE, dim()),
        "stopped" | "archived" => (G_STOPPED, dim()),
        _ => (G_STARTING, faint()),
    }
}

/// The workspace folder (the embedded terminal starts there).
pub(crate) fn workspace(app: &App) -> Option<String> {
    app.sb.as_ref().map(|sb| sb.workspace.clone()).filter(|w| !w.is_empty())
}

/// The feed and composer area, and the panel on the right when it fits.
pub(crate) fn split(app: &App, full: Rect) -> (Rect, Option<Rect>) {
    if app.sb.is_none() {
        return (full, None);
    }
    // the screen's layout (book §8, layout.rs): under the header and its
    // blank row, above the composer pane (the divider and what the
    // smallest composer takes)
    let c = crate::layout::cols(full.width, full.height);
    let r = crate::layout::rows(full.width, full.height);
    let pane = 2 + r.pad_top + r.min_text + r.pad_bottom;
    let bottom = r.keybar.saturating_sub(pane);
    let body = Rect { y: full.y + r.body, height: bottom.saturating_sub(r.body), ..full };
    let feed = Rect { x: full.x + c.feed_x, width: c.feed_w, ..body }.intersection(full);
    let panel = c.panel.map(|p| Rect { x: full.x + p.x, width: p.w, ..body }.intersection(full));
    (feed, panel)
}

/// The panel title: `agents · ⌥ + number` (the key part faint).
pub(crate) const PANEL_TITLE: (&str, &str) = ("agents", " · ⌥ + number");

/// The keys part of the panel title: `alt + number` in ASCII mode (QA 12;
/// the cell net would turn `⌥` into `M`).
fn panel_title_keys() -> &'static str {
    if theme::ascii_mode() {
        " . alt + number"
    } else {
        PANEL_TITLE.1
    }
}

/// The agent waits on you: it is blocked, or one of its cards asks you
/// something.
fn needs_you(sb: &Sb, a: &Agent) -> bool {
    a.status == "blocked"
        || (!matches!(a.status.as_str(), "failed" | "stopped" | "archived")
            && sb
                .cards
                .iter()
                .any(|c| c.agent == a.name && matches!(c.kind.as_str(), "question" | "blocked")))
}

/// A duration in the panel: `40s`, `12m`, `3h`, `2d`.
pub(crate) fn short_age(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{}s", s),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

/// What the right side of an agent's row says, and its color: its age
/// and context fill while it works, else its state in one word.
fn right_of(app: &App, sb: &Sb, a: &Agent) -> (String, Color) {
    if needs_you(sb, a) {
        return ("you".into(), accent());
    }
    let fill = || sb.usage_of(app, &a.name).map(|u| u.short());
    let busy = || {
        let parts: Vec<String> = a.turn_ms.map(short_age).into_iter().chain(fill()).collect();
        parts.join(" · ")
    };
    let s = match a.status.as_str() {
        "working" => busy(),
        // who it waits on (`sb wait` / `sb ask`), when the hub says
        "waiting" if !a.waiting_on.is_empty() => format!("waits {}", a.waiting_on),
        "waiting" => "waiting".into(),
        _ if a.main => String::new(),
        "idle" => fill().map_or_else(|| "idle".into(), |f| format!("idle · {}", f)),
        s => s.to_string(),
    };
    (s, dim())
}

/// One row of the panel, `w` columns: ` N G name marks …… right `. The
/// name is cut to leave room for the marks and the right side; `bg`
/// paints the whole row (the selection).
#[allow(clippy::too_many_arguments)]
fn row(
    num: Option<usize>,
    g: (&str, Color),
    name: &str,
    name_style: Style,
    marks: Vec<Span<'static>>,
    right: (String, Color),
    w: usize,
    bg: Option<Color>,
) -> Line<'static> {
    let num = match num {
        Some(n) if n <= 9 => n.to_string(),
        _ => " ".to_string(),
    };
    let lead = format!(" {} ", num);
    let gl = format!("{} ", g.0);
    let marks_w: usize = marks.iter().map(|s| s.content.width()).sum();
    let right_w = if right.0.is_empty() { 0 } else { right.0.width() + 1 };
    // 1 column of margin on the right
    let room = w.saturating_sub(lead.width() + gl.width() + marks_w + right_w + 1);
    let name = fit(name, room);
    let used = lead.width() + gl.width() + name.width() + marks_w;
    let pad = w.saturating_sub(used + right_w + 1);
    let mut spans = vec![
        Span::styled(lead, Style::default().fg(faint())),
        Span::styled(gl, Style::default().fg(g.1)),
        Span::styled(name, name_style),
    ];
    spans.extend(marks);
    spans.push(Span::raw(" ".repeat(pad)));
    if right_w > 0 {
        spans.push(Span::styled(format!(" {}", right.0), Style::default().fg(right.1)));
    }
    spans.push(Span::raw(" "));
    if let Some(bg) = bg {
        spans = spans.into_iter().map(|s| { let st = s.style.bg(bg); s.style(st) }).collect();
    }
    Line::from(spans)
}

/// `s` cut to `max` display columns, `…` at the cut.
fn fit(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + c.to_string().width() + 1 > max {
            break;
        }
        out.push(c);
    }
    if max > 0 {
        out.push('…');
    }
    out
}

/// The row of live agent `a`, entry `i` of the panel, number `num`
/// (0 main; blank after 9).
fn agent_row(app: &App, sb: &Sb, a: &Agent, i: usize, num: Option<usize>, w: usize, name_cut: usize) -> Line<'static> {
    let focused = a.name == sb.focus;
    let selected = sb.selected == Some(i);
    let g = if a.main {
        (G_MAIN, accent())
    } else if needs_you(sb, a) {
        (G_NEEDS_YOU, accent())
    } else {
        glyph(&a.status, app.tick)
    };
    let name_style = if focused {
        Style::default().fg(accent()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(text())
    };
    let mut marks = Vec::new();
    if sb.activity.contains(&a.name) && !focused {
        marks.push(Span::styled(format!(" {}", G_UNREAD), Style::default().fg(accent())));
    }
    if a.branch.is_some() || a.mode == "worktree" {
        marks.push(Span::styled(format!(" {}", G_WORKTREE), Style::default().fg(dim())));
    }
    if a.queued > 0 {
        marks.push(Span::styled(format!(" {}{}", G_MSG, a.queued), Style::default().fg(dim())));
    }
    // BISE-89: the messages queued here for after its turn
    let mine = if focused { app.queued.len() } else { sb.views.get(&a.name).map_or(0, |v| v.queued.len()) };
    if mine > 0 {
        marks.push(Span::styled(format!(" · {} queued", mine), Style::default().fg(faint())));
    }
    let bg = selected.then(selection_bg);
    // names cut at 16 columns (12 in a narrow panel), book §8
    let name = fit(&a.name, name_cut.max(1));
    row(num, g, &name, name_style, marks, right_of(app, sb, a), w, bg)
}

/// The live agents (main and the archived left out) by what the header
/// counts: working, waiting, needs you, done.
fn counts(sb: &Sb) -> Option<[usize; 4]> {
    let live: Vec<&Agent> = sb.agents.iter().filter(|a| !a.main && !a.archived()).collect();
    if live.is_empty() {
        return None;
    }
    let mut n = [0; 4];
    for a in live {
        let k = if needs_you(sb, a) {
            2
        } else {
            match a.status.as_str() {
                "working" => 0,
                "waiting" => 1,
                "done" => 3,
                _ => continue,
            }
        };
        n[k] += 1;
    }
    Some(n)
}

/// The header counts that fit in `room` columns (QA 14): all of them with
/// their words when they fit (not `short`), else the numbers only; still
/// too wide, the least important counts go first ("needs you" stays, then
/// working, waiting, done), shown in the §8 order.
fn fit_counts(n: [usize; 4], short: bool, room: usize) -> Vec<Span<'static>> {
    let parts = [
        (G_WORKING, "working", dim()),
        (G_WAITING, "waiting", dim()),
        (G_NEEDS_YOU, "needs you", accent()),
        (G_DONE, "done", dim()),
    ];
    let spans = |keep: &[usize], words: bool| -> Vec<Span<'static>> {
        let mut out: Vec<Span<'static>> = Vec::new();
        for k in (0..4).filter(|k| keep.contains(k)) {
            if !out.is_empty() {
                out.push(Span::styled(" · ", Style::default().fg(dim())));
            }
            let (g, word, color) = parts[k];
            let t = if words { format!("{} {} {}", g, n[k], word) } else { format!("{} {}", g, n[k]) };
            out.push(Span::styled(t, Style::default().fg(color)));
        }
        out
    };
    let by_importance: Vec<usize> = [2, 0, 1, 3].into_iter().filter(|&k| n[k] > 0).collect();
    let fits = |out: &Vec<Span<'static>>| out.iter().map(|s| s.content.width()).sum::<usize>() <= room;
    if !short {
        let out = spans(&by_importance, true);
        if fits(&out) {
            return out;
        }
    }
    for keep in (1..=by_importance.len()).rev() {
        let out = spans(&by_importance[..keep], false);
        if fits(&out) {
            return out;
        }
    }
    Vec::new()
}

impl Sb {
    /// The header row (book §8): `bise :*` on the left; on the right the
    /// non-zero counts `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1
    /// done` (`? … needs you` in accent), shortened to `∿ 3 · ? 1` when
    /// `short` (no panel), or `no agents yet`. A method, so `ui.rs` reaches
    /// it through `app.sb` (the `panel` module is private to `sb`).
    pub(crate) fn header(&self, width: u16, short: bool) -> Line<'static> {
        let mut spans = vec![Span::raw(" ")];
        spans.extend(self.title());
        let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
        // one column of margin on the right, two of gap after `bise :*`
        let right = self.summary((width as usize).saturating_sub(left_w + 3), short);
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        let pad = (width as usize).saturating_sub(left_w + right_w + 1);
        if pad >= 2 {
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(right);
        }
        Line::from(spans)
    }

    /// The title: `bise` bold in the text color, `:*` in accent.
    pub(crate) fn title(&self) -> Vec<Span<'static>> {
        vec![
            Span::styled("bise ", Style::default().fg(text()).add_modifier(Modifier::BOLD)),
            Span::styled(crate::theme::glyph(G_MAIN), Style::default().fg(accent())),
        ]
    }

    /// The summary in at most `room` columns (book §8 "The frame"): the
    /// workspace (`~/acme`, dim) then the counts; not enough room, the
    /// path goes first, then the counts shorten (`∿ 3 · ? 1`).
    pub(crate) fn summary(&self, room: usize, short: bool) -> Vec<Span<'static>> {
        let fitted = |room: usize| -> Vec<Span<'static>> {
            match counts(self) {
                None => vec![Span::styled("no agents yet", Style::default().fg(dim()))],
                Some(n) => fit_counts(n, short, room),
            }
        };
        // the path only beside the counts as they are when nothing is short
        let whole = fitted(usize::MAX);
        let whole_w: usize = whole.iter().map(|s| s.content.width()).sum();
        let path = home_path(&self.workspace);
        if path.is_empty() || path.width() + 3 + whole_w > room {
            return fitted(room);
        }
        let mut out = vec![Span::styled(format!("{} · ", path), Style::default().fg(dim()))];
        out.extend(whole);
        out
    }
}

/// `path` with the home folder as `~`.
fn home_path(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && (path == h || path.starts_with(&format!("{}/", h))) => format!("~{}", &path[h.len()..]),
        _ => path.to_string(),
    }
}

pub(crate) fn draw_panel(app: &App, frame: &mut Frame, area: Rect, name_cut: usize) {
    let Some(sb) = app.sb.as_ref() else { return };
    // no rule on its left: whitespace and alignment do the job (book §8)
    let w = area.width as usize;
    let title = Line::from(vec![
        Span::styled(format!(" {}", PANEL_TITLE.0), Style::default().fg(text())),
        Span::styled(panel_title_keys(), Style::default().fg(faint())),
    ]);
    let mut lines: Vec<Line> = Vec::new();
    let numbers = sb.numbers();
    let nav = sb.nav();
    let live = nav.iter().filter(|a| !a.archived()).count();
    let mut owners: Vec<(usize, Hit)> = Vec::new();
    // the first row of the selected entry (the panel scrolls to it)
    let mut sel_row = None;
    for (i, a) in nav.iter().take(live).enumerate() {
        if sb.selected == Some(i) {
            sel_row = Some(lines.len());
        }
        owners.push((lines.len(), Hit::Agent(a.name.clone())));
        let n = numbers.iter().find(|(name, _)| *name == a.name).map(|(_, n)| *n);
        lines.push(agent_row(app, sb, a, i, n, w, name_cut));
        // the selected agent: what it is for and its last note, under its row
        if sb.selected == Some(i) && !a.main {
            for t in [&a.objective, &a.note].into_iter().filter(|t| !t.is_empty()) {
                owners.push((lines.len(), Hit::Agent(a.name.clone())));
                lines.push(Line::from(Span::styled(
                    format!("     {}", fit(t, w.saturating_sub(6))),
                    Style::default().fg(dim()),
                )));
            }
        }
    }
    archived_lines(sb, live, w, &mut lines, &mut owners, &mut sel_row);
    // the body under the title: scrolled to keep the selection in view;
    // what does not fit below ends in `+ {n} more`
    let h = (area.height as usize).saturating_sub(2);
    let (top, more) = window(&lines, &owners, sel_row, h, sb.archived_open, sb.archived().len());
    let mut body: Vec<Line> = lines.into_iter().skip(top).take(if more.is_some() { h - 1 } else { h }).collect();
    if let Some(n) = more {
        body.push(Line::from(Span::styled(format!(" + {} more", n), Style::default().fg(dim()))));
    }
    let shown = body.len() - usize::from(more.is_some());
    if let Ok(mut hits) = sb.panel_hits.try_borrow_mut() {
        hits.area = area;
        hits.rows = owners
            .into_iter()
            .filter(|(r, _)| *r >= top && *r - top < shown)
            .map(|(r, hit)| (area.y.saturating_add(2 + (r - top) as u16), hit))
            .collect();
    }
    // the title, then 1 blank row (book §8)
    let mut all = vec![title, Line::from("")];
    all.extend(body);
    frame.render_widget(Paragraph::new(all), area);
}

/// The first body row shown in `h` rows, and the `+ {n} more` count
/// when rows are left below: the agents under the window (a folded
/// archived section counts its agents).
fn window(
    lines: &[Line],
    owners: &[(usize, Hit)],
    sel_row: Option<usize>,
    h: usize,
    archived_open: bool,
    archived: usize,
) -> (usize, Option<usize>) {
    if lines.len() <= h || h < 2 {
        return (0, None);
    }
    // the selection (and the row under it) stays in view, over the
    // `+ n more` row
    let top = sel_row.map_or(0, |r| (r + 3).saturating_sub(h));
    if top + h >= lines.len() {
        // the end of the list fits: no `+ n more`
        return (lines.len() - h, None);
    }
    let end = top + h - 1;
    let mut below: Vec<&Hit> = Vec::new();
    for (r, hit) in owners {
        if *r >= end && !below.contains(&hit) {
            below.push(hit);
        }
    }
    let n = below
        .iter()
        .map(|h| match h {
            Hit::Agent(_) => 1,
            Hit::Archived if !archived_open => archived,
            Hit::Archived => 0,
        })
        .sum::<usize>();
    if n == 0 {
        return (lines.len() - h, None);
    }
    (top, Some(n))
}

/// The archived section, at the bottom: a dim folded row `▸ {n}
/// archived` (`▾` open), then, open, one row per agent, newest first:
/// its name and how long ago it was last heard of; the selected or
/// focused one also shows its last report (or its objective). Folded,
/// only the archived agent in focus is listed, so the view in focus is
/// always found in the panel.
fn archived_lines(
    sb: &Sb,
    live: usize,
    w: usize,
    lines: &mut Vec<Line<'static>>,
    owners: &mut Vec<(usize, Hit)>,
    sel_row: &mut Option<usize>,
) {
    let all = sb.archived();
    if all.is_empty() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    lines.push(Line::from(""));
    owners.push((lines.len(), Hit::Archived));
    let arrow = if sb.archived_open { G_OPEN } else { G_CLOSED };
    lines.push(Line::from(Span::styled(
        format!(" {} {} archived", arrow, all.len()),
        Style::default().fg(dim()),
    )));
    for (k, a) in all.iter().enumerate() {
        let focused = a.name == sb.focus;
        if !sb.archived_open && !focused {
            continue;
        }
        let selected = sb.archived_open && sb.selected == Some(live + k);
        if selected {
            *sel_row = Some(lines.len());
        }
        let age = a.report_ms.map(|t| short_age(now.saturating_sub(t))).unwrap_or_default();
        let name_style = Style::default().fg(if focused { accent() } else { dim() });
        let first = lines.len();
        let bg = selected.then(selection_bg);
        lines.push(row(None, (G_STOPPED, faint()), &a.name, name_style, Vec::new(), (age, dim()), w, bg));
        if selected || focused {
            let what = if a.report.is_empty() { &a.objective } else { &a.report };
            lines.push(Line::from(Span::styled(
                format!("     {}", fit(what, w.saturating_sub(6))),
                Style::default().fg(dim()),
            )));
        }
        owners.extend((first..lines.len()).map(|r| (r, Hit::Agent(a.name.clone()))));
    }
}

/// What a panel row leads to when clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hit {
    /// Focus this agent (an archived one opens read-only).
    Agent(String),
    /// The header of the archived section: expand / collapse.
    Archived,
}

/// Where the last frame drew the panel, and what each of its rows leads
/// to (screen row, target): a click on an agent row focuses the agent.
#[derive(Debug, Default, Clone)]
pub(crate) struct PanelHits {
    area: Rect,
    rows: Vec<(u16, Hit)>,
    /// The panel's numbers (Alt+N): agent name → number, kept while the
    /// agent lives (see [`Sb::numbers`]).
    slots: Vec<(String, usize)>,
}

impl Sb {
    /// The number of every live agent: main 0; an agent keeps its number
    /// while it lives (a drop does not renumber the others); a new one
    /// takes the smallest free number, so the first agents get 1, 2, 3 in
    /// creation order. Only 0-9 are shown and reachable with Alt+N.
    pub(crate) fn numbers(&self) -> Vec<(String, usize)> {
        let live: Vec<&Agent> = self.agents.iter().filter(|a| !a.archived()).collect();
        let Ok(mut hits) = self.panel_hits.try_borrow_mut() else {
            return assign(Vec::new(), &live);
        };
        let slots = assign(std::mem::take(&mut hits.slots), &live);
        hits.slots = slots.clone();
        slots
    }

    /// The live agent with number `n`, if any.
    pub(crate) fn agent_numbered(&self, n: usize) -> Option<String> {
        self.numbers().into_iter().find(|(_, k)| *k == n).map(|(name, _)| name)
    }
}

/// `slots` brought up to date with the `live` agents (hub order): the
/// gone ones free their number, main is 0, a newcomer takes the smallest
/// free number from 1.
fn assign(mut slots: Vec<(String, usize)>, live: &[&Agent]) -> Vec<(String, usize)> {
    slots.retain(|(name, n)| live.iter().any(|a| a.name == *name && (a.main == (*n == 0))));
    for a in live {
        if slots.iter().any(|(name, _)| *name == a.name) {
            continue;
        }
        let n = if a.main {
            0
        } else {
            (1..).find(|k| !slots.iter().any(|(_, n)| n == k)).unwrap_or(1)
        };
        slots.push((a.name.clone(), n));
    }
    slots
}

impl PanelHits {
    /// What is drawn at screen cell (`x`, `y`), if anything.
    fn hit_at(&self, x: u16, y: u16) -> Option<&Hit> {
        if !self.contains(x, y) {
            return None;
        }
        self.rows.iter().find(|(r, _)| *r == y).map(|(_, h)| h)
    }

    fn contains(&self, x: u16, y: u16) -> bool {
        let a = self.area;
        x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height)
    }
}

/// A left click in the panel: on an agent's rows it focuses that agent,
/// the same path as Alt+N. `true` when the click was the panel's.
pub(crate) fn panel_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    if m.kind != MouseEventKind::Down(MouseButton::Left) {
        return false;
    }
    let Some(sb) = app.sb.as_ref() else { return false };
    let target = {
        let Ok(hits) = sb.panel_hits.try_borrow() else { return false };
        if !hits.contains(m.column, m.row) {
            return false;
        }
        hits.hit_at(m.column, m.row).cloned()
    };
    match target {
        Some(Hit::Agent(name)) if sb.agent(&name).is_some() => focus(app, &name),
        Some(Hit::Archived) => {
            if let Some(sb) = app.sb.as_mut() {
                sb.toggle_archived();
            }
        }
        _ => {}
    }
    true
}

/// The state of the agent you talk to (book §8 "The frame": the right of
/// the divider; it was the status row), dim: its state, the turn's
/// duration, its context, `shared folder` or `⎇ branch`, then the notes
/// (preview, read-only, cards, the hub's version); or the `D` question,
/// in accent. `idle · 210k / 1M tokens · 21%`.
pub(crate) fn status_state(app: &App) -> Option<Line<'static>> {
    let sb = app.sb.as_ref()?;
    if let Some(name) = &sb.drop_ask {
        return Some(Line::from(Span::styled(drop_question(name), Style::default().fg(accent()))));
    }
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let d = |t: String| Span::styled(format!(" · {}", t), Style::default().fg(dim()));
    let mut spans: Vec<Span<'static>> = Vec::new();
    if !a.status.is_empty() {
        spans.push(d(a.status.clone()));
    }
    if let Some(ms) = a.turn_ms.filter(|_| app.pending) {
        spans.push(d(short_age(ms)));
    }
    if let Some(u) = crate::usage::current(&app.events) {
        spans.push(d(u.label()));
    }
    if let Some(b) = &a.branch {
        spans.push(d(format!("{} {}", G_WORKTREE, b)));
    } else if !a.main && !a.path.is_empty() && a.mode == "shared" {
        spans.push(d("shared folder".into()));
    }
    if a.archived() {
        spans.push(d("read-only history · /restore brings it back".into()));
    }
    if sb.preview {
        if let Some(sel) = sb.selected_agent().map(|a| a.name.clone()) {
            spans.push(d(format!("preview of {}", sel)));
        }
    }
    if !sb.cards.is_empty() && !sb.card.shown {
        let n = sb.cards.len();
        spans.push(Span::styled(" · ", Style::default().fg(dim())));
        spans.push(Span::styled(
            format!("{} {} card{} · ctrl+g", G_CARD, n, if n > 1 { "s" } else { "" }),
            Style::default().fg(accent()),
        ));
    }
    for i in &sb.versions {
        if i.marks.iter().any(|m| m == "building") {
            spans.push(d(format!("{} building {}", G_BUILDING, i.rev)));
        }
        if i.marks.iter().any(|m| m == "trial") {
            spans.push(d(format!("{} {} on trial", G_BUILDING, i.rev)));
        }
    }
    if !sb.version.is_empty() {
        spans.push(d(format!("v {}", sb.version.chars().take(24).collect::<String>())));
    }
    if !app.connected {
        spans.push(Span::styled(" · ", Style::default().fg(dim())));
        spans.push(Span::styled(
            format!("{} hub disconnected · reconnecting…", G_IDLE),
            Style::default().fg(error()),
        ));
    }
    // the first note has no ` · ` before it
    if let Some(first) = spans.first_mut() {
        if let Some(t) = first.content.strip_prefix(" · ") {
            first.content = t.to_string().into();
        } else if first.content == " · " {
            spans.remove(0);
        }
    }
    Some(Line::from(spans))
}

/// The divider's text as one string, `name · state` (tests).
#[cfg(test)]
pub(crate) fn status_text(app: &App) -> String {
    let sb = app.sb.as_ref().unwrap();
    let state: String = status_state(app).unwrap().spans.iter().map(|s| s.content.as_ref()).collect();
    if sb.drop_ask.is_some() {
        state
    } else if state.is_empty() {
        sb.focus.clone()
    } else {
        format!("{} · {}", sb.focus, state)
    }
}

/// What `D` asks in the status row (book §16, BISE-43).
pub(crate) fn drop_question(name: &str) -> String {
    format!("drop {}? its history stays in archived. y / n", name)
}

/// The key bar's mode in switchboard (BISE-99, [`crate::keybar`]); `None`
/// without switchboard.
pub(crate) fn key_mode(app: &App) -> Option<crate::keybar::Mode> {
    use crate::keybar::Mode;
    let sb = app.sb.as_ref()?;
    Some(if sb.drop_ask.is_some() {
        Mode::DropAsk
    } else if sb.confirm.is_some() {
        Mode::Confirm
    } else if sb.card.full {
        Mode::CardFull
    // a shown card box carries its own keys (QA 11): no repeat here
    } else if sb.selected.is_some() {
        Mode::Selected
    } else if sb.focus_archived() {
        Mode::Archived
    } else if app.pending {
        Mode::Steer
    } else {
        Mode::Default
    })
}

/// What the empty composer shows after the cursor, dim (book §8 "The
/// frame"): `what's on your mind?` to main, `talk to auth-fix directly`
/// inside an agent, a read-only note in an archived agent's history.
pub(crate) fn placeholder(app: &App) -> Option<String> {
    let sb = app.sb.as_ref()?;
    Some(if sb.focus_archived() {
        format!("{} is archived: read-only", sb.focus)
    } else if sb.is_main_focus() {
        PLACEHOLDER_MAIN.to_string()
    } else {
        format!("talk to {} directly", sb.focus)
    })
}

/// The empty composer's question, to main (copy deck §17).
pub(crate) const PLACEHOLDER_MAIN: &str = "what's on your mind?";

/// The first-run text (book §8, §17): shown dim in main's empty feed
/// while there are no agents yet.
pub(crate) const FIRST_RUN: [&str; 3] = [
    "what's on your mind?",
    "say it and keep talking. the work runs in the background, i'm always here.",
    "try: \"fix the flaky login test, and draft the release note\"",
];

impl Sb {
    /// No agents yet and main in view: the first-run text, which shows
    /// while main's feed is empty.
    pub(crate) fn first_run(&self) -> Option<[&'static str; 3]> {
        (self.focus == "main" && counts(self).is_none() && !self.preview).then_some(FIRST_RUN)
    }

    /// The line pinned on top of the feed: the preview of the selected
    /// agent, or, inside an agent, that main is out of the loop.
    pub(crate) fn feed_banner(&self) -> Option<Line<'static>> {
        let dim = Style::default().fg(dim());
        if self.preview {
            let name = self.selected_agent().map(|a| a.name.clone()).filter(|n| *n != self.focus)?;
            return Some(Line::from(vec![
                Span::styled("preview · ", dim),
                Span::styled(name, Style::default().fg(text())),
                Span::styled(" · ⏎ enter · esc close", dim),
            ]));
        }
        if self.focus == "main" || self.focus_archived() || self.agent(&self.focus).is_none() {
            return None;
        }
        Some(Line::from(Span::styled(
            format!("you're talking to {} directly. main isn't in the loop. esc back to main.", self.focus),
            dim,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        let w = buf.area.width as usize;
        buf.content
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect()
    }

    fn click(app: &mut App, column: u16, row: u16) {
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        crate::input::on_mouse(app, &m, 0);
    }

    /// The screen row and column where `label` shows in the panel.
    fn find(rows: &[String], panel_x: u16, label: &str) -> (u16, u16) {
        rows.iter()
            .enumerate()
            .find_map(|(y, r)| {
                let tail: String = r.chars().skip(panel_x as usize).collect();
                tail.find(label).map(|_| (panel_x + 3, y as u16))
            })
            .unwrap_or_else(|| panic!("{} not in the panel:\n{}", label, rows.join("\n")))
    }

    /// A click on an agent's row focuses it, like Alt+N (the objective
    /// under the selected row too); a click on the title changes nothing.
    #[test]
    fn a_click_on_an_agent_row_focuses_it() {
        let mut app = bench::test_app_drained();
        if let Some(sb) = app.sb.as_mut() {
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        }
        bench::add_agent(&mut app, "alpha", "first objective");
        bench::add_agent(&mut app, "beta", "second objective");
        bench::add_agent(&mut app, "gamma", "third objective");
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let draw = |app: &mut App, term: &mut Terminal<TestBackend>| {
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        };
        draw(&mut app, &mut term);
        let panel_x = app.sb.as_ref().unwrap().panel_hits.borrow().area.x;
        for (label, name) in [("alpha", "alpha"), ("beta", "beta")] {
            let (x, y) = find(&screen(&term), panel_x, label);
            click(&mut app, x, y);
            assert_eq!(app.sb.as_ref().unwrap().focus, name, "click on {:?}", label);
            draw(&mut app, &mut term);
        }
        // the selected agent shows its objective under its row
        if let Some(sb) = app.sb.as_mut() {
            sb.selected = Some(3);
        }
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "third objective");
        click(&mut app, x, y);
        assert_eq!(app.sb.as_ref().unwrap().focus, "gamma");
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "main");
        click(&mut app, x, y);
        assert_eq!(app.sb.as_ref().unwrap().focus, "main");
        // the title row: nothing happens
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, PANEL_TITLE.0);
        click(&mut app, x, y);
        assert_eq!(app.sb.as_ref().unwrap().focus, "main");
        // a click in the feed is not the panel's
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!panel_mouse(&mut app, &m));
    }

    fn agent(name: &str, status: &str) -> Agent {
        Agent { name: name.into(), status: status.into(), ..Agent::default() }
    }

    /// main and one agent in every state, as in the mockup.
    fn every_state() -> App {
        let mut app = bench::test_app_drained();
        let sb = app.sb.as_mut().unwrap();
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        sb.agents.push(Agent { turn_ms: Some(12 * 60_000), ..agent("auth-fix", "working") });
        sb.agents.push(agent("tests", "starting"));
        sb.agents.push(agent("docs", "waiting"));
        sb.agents.push(Agent { waiting_on: "docs".into(), ..agent("api-v2", "waiting") });
        sb.agents.push(agent("bench", "done"));
        sb.agents.push(agent("deploy", "failed"));
        sb.agents.push(agent("ideas", "idle"));
        sb.agents.push(agent("old-spike", "stopped"));
        sb.agents.push(Agent { branch: Some("sb/big".into()), mode: "worktree".into(), ..agent("big-refactor-of-auth", "working") });
        sb.agents.push(agent("eleventh", "idle"));
        sb.cards.push(Card {
            id: 1,
            kind: "question".into(),
            agent: "docs".into(),
            text: "v1 or v2?".into(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
        });
        sb.activity.insert("auth-fix".into());
        app
    }

    /// The panel alone, `w` columns wide, `h` rows high.
    fn panel_rows(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_panel(app, f, f.area(), 16)).unwrap();
        screen(&term)
    }

    fn trimmed(rows: &[String]) -> Vec<String> {
        rows.iter().map(|r| r.trim_end().to_string()).collect()
    }

    /// The row layout at the two panel widths: number (0-9, blank
    /// after), status glyph, name, marks, the right side flush right
    /// with one column of margin; the title never wraps.
    #[test]
    fn panel_rows_at_28_and_40() {
        let app = every_state();
        for w in [28u16, 40] {
            let rows = panel_rows(&app, w, 16);
            let t = trimmed(&rows);
            let row = |n: &str| t.iter().find(|r| r.contains(n)).unwrap_or_else(|| panic!("{} missing:\n{}", n, t.join("\n"))).clone();
            assert_eq!(t[0], format!(" {}{}", PANEL_TITLE.0, PANEL_TITLE.1), "the title on one row at {}", w);
            assert_eq!(row("main"), format!(" 0 {} main", G_MAIN));
            let flush = |n: &str, right: &str| {
                let r = row(n);
                assert!(r.ends_with(right), "{:?} ends with {:?} at {}", r, right, w);
                // one column of margin on the right
                assert_eq!(rows.iter().find(|x| x.contains(n)).unwrap().chars().count(), w as usize);
                assert_eq!(r.chars().count(), w as usize - 1, "{:?} flush right at {}", r, w);
            };
            flush("auth-fix", "12m");
            assert!(row("auth-fix").starts_with(&format!(" 1 {} auth-fix {}", G_WORKING, G_UNREAD)));
            flush("tests", "starting");
            flush("docs", "you");
            assert!(row("docs").starts_with(&format!(" 3 {} docs", G_NEEDS_YOU)));
            flush("api-v2", "waits docs");
            assert!(row("api-v2").starts_with(&format!(" 4 {} api-v2", G_WAITING)));
            flush("bench", "done");
            assert!(row("bench").starts_with(&format!(" 5 {} bench", G_DONE)));
            flush("deploy", "failed");
            assert!(row("deploy").starts_with(&format!(" 6 {} deploy", G_FAILED)));
            flush("ideas", "idle");
            flush("old-spike", "stopped");
            assert!(row("old-spike").starts_with(&format!(" 8 {} old-spike", G_STOPPED)));
            assert!(row("big-re").starts_with(&format!(" 9 {} big-re", G_WORKING)));
            assert!(row("big-re").contains(G_WORKTREE));
            // no number after 9
            assert!(row("eleventh").starts_with(&format!("   {} eleventh", G_IDLE)), "{:?}", row("eleventh"));
            assert!(!t.iter().any(|r| r.to_lowercase().contains("task")), "no \"task\" in the panel");
        }
        // 28 columns: a long name is cut, the worktree mark kept
        let t = trimmed(&panel_rows(&app, 28, 16));
        let big = t.iter().find(|r| r.contains("big-")).unwrap();
        assert!(big.contains("…") && big.contains(G_WORKTREE), "{:?}", big);
        // names are cut at 16 even with room (book §8)
        let t = trimmed(&panel_rows(&app, 40, 16));
        assert!(t.iter().any(|r| r.contains(&format!("big-refactor-of… {}", G_WORKTREE))), "{}", t.join("\n"));
    }

    /// Colors: the number faint, the agent in view in accent, "needs
    /// you" in accent, a failure in error, the right side dim.
    #[test]
    fn panel_colors() {
        let mut app = every_state();
        app.sb.as_mut().unwrap().focus = "bench".into();
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area(), 16)).unwrap();
        let rows = screen(&term);
        let buf = term.backend().buffer();
        let at = |n: &str, what: &str| {
            let y = rows.iter().position(|r| r.contains(n)).unwrap();
            let x = rows[y].find(what).map(|b| rows[y][..b].chars().count()).unwrap();
            buf.cell((x as u16, y as u16)).unwrap().fg
        };
        assert_eq!(at("main", "0"), faint());
        assert_eq!(at("main", G_MAIN), accent());
        assert_eq!(at("bench", "bench"), accent(), "the agent in view");
        assert_eq!(at("auth-fix", "auth-fix"), text());
        assert_eq!(at("docs", G_NEEDS_YOU), accent());
        assert_eq!(at("docs", "you"), accent());
        assert_eq!(at("deploy", G_FAILED), error());
        assert_eq!(at("deploy", "failed"), dim());
        assert_eq!(at("auth-fix", G_UNREAD), accent());
    }

    /// Numbers stay while an agent lives: a drop does not renumber the
    /// others, Alt+N follows the number shown; a newcomer takes the free
    /// number.
    #[test]
    fn numbers_survive_a_drop() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut app = bench::test_app_drained();
        {
            let sb = app.sb.as_mut().unwrap();
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for n in ["a", "b", "c"] {
                sb.agents.push(agent(n, "working"));
            }
        }
        let num = |app: &App, n: &str| app.sb.as_ref().unwrap().numbers().into_iter().find(|(x, _)| x == n).map(|(_, k)| k);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!((num(&app, "a"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        // a is dropped (archived): b and c keep 2 and 3
        app.sb.as_mut().unwrap().agents[1].status = "archived".into();
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 2 {} b", G_WORKING))), "{}", t.join("\n"));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!(num(&app, "a"), None);
        // Alt+3 goes to c, Alt+1 to no one, Alt+0 to main
        key(&mut app, &KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.as_ref().unwrap().focus, "c");
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.as_ref().unwrap().focus, "c", "no agent 1 any more");
        key(&mut app, &KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.as_ref().unwrap().focus, "main");
        // a newcomer takes the free number 1; the others keep theirs
        app.sb.as_mut().unwrap().agents.push(agent("d", "starting"));
        assert_eq!((num(&app, "d"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.as_ref().unwrap().focus, "d");
        // restored, a comes back with a free number (4)
        app.sb.as_mut().unwrap().agents[1].status = "idle".into();
        assert_eq!(num(&app, "a"), Some(4));
    }

    /// More agents than rows: the list ends with `+ {n} more`, and
    /// scrolls to keep the selection in view.
    #[test]
    fn overflow_ends_with_more() {
        let mut app = bench::test_app_drained();
        {
            let sb = app.sb.as_mut().unwrap();
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for i in 1..=30 {
                sb.agents.push(agent(&format!("a{:02}", i), "working"));
            }
        }
        let t = trimmed(&panel_rows(&app, 28, 10));
        // title + 1 blank row + 7 agents + the more row
        assert_eq!(t[9], " + 24 more", "{}", t.join("\n"));
        app.sb.as_mut().unwrap().selected = Some(30);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.contains("a30")), "{}", t.join("\n"));
        assert!(!t.iter().any(|r| r.contains("more")), "nothing left below");
        app.sb.as_mut().unwrap().selected = Some(15);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.contains("a15")), "{}", t.join("\n"));
        assert!(t[9].contains("more"), "{}", t.join("\n"));
    }
}

#[cfg(test)]
mod archived_tests {
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }

    /// main, one live task, three archived ones (`old` 5 h ago, `mid`
    /// 2 h, `new` 10 min: listed new, mid, old).
    fn app() -> App {
        let mut app = bench::test_app_drained();
        let now = now_ms();
        if let Some(sb) = app.sb.as_mut() {
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        }
        bench::add_agent(&mut app, "alpha", "live objective");
        if let Some(sb) = app.sb.as_mut() {
            for (name, h_ago) in [("old", 300u64), ("new", 10), ("mid", 120)] {
                sb.agents.push(Agent {
                    name: name.into(),
                    status: "archived".into(),
                    objective: format!("{} objective", name),
                    report: format!("{} did its job", name),
                    report_ms: Some(now - h_ago * 60_000),
                    ..Agent::default()
                });
            }
        }
        app
    }

    fn draw(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<String> {
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        let buf = term.backend().buffer();
        let w = buf.area.width as usize;
        buf.content
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect()
    }

    fn panel(rows: &[String], x: u16) -> Vec<String> {
        rows.iter().map(|r| r.chars().skip(x as usize).collect::<String>()).collect()
    }

    fn row_of(rows: &[String], label: &str) -> Option<usize> {
        rows.iter().position(|r| r.contains(label))
    }

    fn click(app: &mut App, column: u16, row: u16) {
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        crate::input::on_mouse(app, &m, 0);
    }

    fn press(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
        key(app, &KeyEvent::new(code, m), false)
    }

    /// Collapsed: one dim header, no archived name. A click on it
    /// expands the list, newest first, dim; a click on a row opens that
    /// task's history read-only (status row, placeholder, typed text not
    /// sent); a second click on the header folds the list, the task in
    /// focus stays listed.
    #[test]
    fn archived_section_folds_expands_and_opens_read_only() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let rows = draw(&mut app, &mut term);
        let x = app.sb.as_ref().unwrap().panel_hits.borrow().area.x;
        let p = panel(&rows, x);
        let head = row_of(&p, "▸ 3 archived").unwrap_or_else(|| panic!("{}", p.join("\n")));
        for n in [&format!("{} old", G_STOPPED), &format!("{} mid", G_STOPPED), &format!("{} new", G_STOPPED)] {
            assert!(row_of(&p, n).is_none(), "{} shown while folded", n);
        }
        let buf = term.backend().buffer().clone();
        let cell = buf.cell((x + 3, head as u16)).unwrap();
        assert_eq!(cell.fg, dim(), "the header is dim");

        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▾ 3 archived").is_some(), "{}", p.join("\n"));
        let (n, m, o) = (
            row_of(&p, &format!("{} new", G_STOPPED)).unwrap(),
            row_of(&p, &format!("{} mid", G_STOPPED)).unwrap(),
            row_of(&p, &format!("{} old", G_STOPPED)).unwrap(),
        );
        assert!(n < m && m < o, "newest first:\n{}", p.join("\n"));
        assert!(p[n].contains("10m") && p[o].contains("5h"), "{}", p.join("\n"));
        assert!(row_of(&p, "did its job").is_none(), "no report line when not selected");
        let buf = term.backend().buffer().clone();
        let name_x = x + p[m].find("mid").map(|b| p[m][..b].chars().count()).unwrap() as u16;
        assert_eq!(buf.cell((name_x, m as u16)).unwrap().fg, dim(), "archived names are dim");

        click(&mut app, x + 5, m as u16);
        assert_eq!(app.sb.as_ref().unwrap().focus, "mid");
        let rows = draw(&mut app, &mut term);
        let all = rows.join("\n");
        assert!(all.contains("read-only history"), "{}", all);
        assert!(panel(&rows, x).iter().any(|r| r.contains("mid did its job")), "{}", all);
        assert_eq!(placeholder(&app).unwrap(), "mid is archived: read-only");
        let out = handle_input(&mut app, "hello");
        assert!(matches!(&out[..], [Ev::Warn(w)] if w.contains("/restore")), "not sent");

        let p = panel(&rows, x);
        let head = row_of(&p, "▾ 3 archived").unwrap();
        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▸ 3 archived").is_some());
        assert!(row_of(&p, &format!("{} mid", G_STOPPED)).is_some(), "the focused archived task stays listed");
        assert!(row_of(&p, &format!("{} new", G_STOPPED)).is_none());
    }

    /// Keys: A expands from a selection, Ctrl+K/J walk into the archived
    /// rows (the selected one shows its report), ⏎ opens it; Alt+N never
    /// lands on an archived task; D does not drop one.
    #[test]
    fn archived_keys() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        press(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(app.sb.as_ref().unwrap().nav().len(), 2);
        press(&mut app, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(app.sb.as_ref().unwrap().archived_open);
        assert_eq!(app.sb.as_ref().unwrap().selected, Some(0), "selection kept");
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        let sb = app.sb.as_ref().unwrap();
        assert_eq!(sb.selected_agent().map(|a| a.name.as_str()), Some("old"));
        let x = sb.panel_hits.borrow().area.x;
        let p = panel(&draw(&mut app, &mut term), x.max(90));
        assert!(row_of(&p, "old did its job").is_some(), "{}", p.join("\n"));
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.as_ref().unwrap().focus, "old");
        press(&mut app, KeyCode::Char('2'), KeyModifiers::ALT);
        assert_eq!(app.sb.as_ref().unwrap().focus, "old", "Alt+2: no live task 2");
        press(&mut app, KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(app.sb.as_ref().unwrap().focus, "alpha");
    }

    /// Hundreds of archived tasks, expanded: the panel scrolls to keep
    /// the selected row in view, and clicks still hit the right row.
    #[test]
    fn a_long_archived_list_scrolls_to_the_selection() {
        let mut app = app();
        if let Some(sb) = app.sb.as_mut() {
            for i in 0..300u64 {
                sb.agents.push(Agent {
                    name: format!("t{:03}", i),
                    status: "archived".into(),
                    report_ms: Some(1_000 + i),
                    ..Agent::default()
                });
            }
            sb.archived_open = true;
            // the oldest: t000, last of the list
            sb.selected = sb.nav().iter().position(|a| a.name == "t000");
        }
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let rows = draw(&mut app, &mut term);
        let x = app.sb.as_ref().unwrap().panel_hits.borrow().area.x;
        let p = panel(&rows, x);
        let y = row_of(&p, &format!("{} t000", G_STOPPED)).unwrap_or_else(|| panic!("{}", p.join("\n")));
        click(&mut app, x + 5, y as u16);
        assert_eq!(app.sb.as_ref().unwrap().focus, "t000");
    }
}

#[cfg(test)]
mod chrome_tests {
    use super::super::bench;
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        let buf = term.backend().buffer();
        buf.content
            .chunks(w as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect()
    }

    fn with_main() -> App {
        let mut app = bench::test_app_drained();
        app.sb.as_mut().unwrap().agents.push(Agent {
            name: "main".into(),
            main: true,
            status: "idle".into(),
            ..Agent::default()
        });
        app
    }

    fn busy() -> App {
        let mut app = with_main();
        let sb = app.sb.as_mut().unwrap();
        for (n, st) in [("auth-fix", "working"), ("release", "working"), ("big", "working"), ("api-v2", "waiting"), ("docs", "blocked"), ("bench", "done"), ("ideas", "idle")] {
            sb.agents.push(Agent { name: n.into(), status: st.into(), ..Agent::default() });
        }
        app
    }

    /// The header at 120 columns (panel shown): `bise :*` left, the long
    /// counts flush right; at 60 (no panel) the short counts.
    /// QA 14: a narrow header keeps what fits, "needs you" first, in
    /// the §8 order; the words go before the counts do.
    #[test]
    fn a_narrow_header_keeps_needs_you_first() {
        let n = [3, 1, 1, 2];
        let text = |room| fit_counts(n, false, room).iter().map(|s| s.content.to_string()).collect::<String>();
        let all = text(200);
        assert!(all.contains("working") && all.contains("needs you"), "{all}");
        let numbers = text(30);
        assert!(!numbers.contains("working"), "{numbers}");
        assert!(numbers.contains("? 1") && numbers.contains("∿ 3"), "{numbers}");
        let two = text(9);
        assert_eq!(two, "∿ 3 · ? 1");
        assert_eq!(text(3), "? 1");
        assert_eq!(text(2), "");
    }

    #[test]
    fn header_at_60_and_120() {
        let mut app = busy();
        let rows = draw(&mut app, 120, 20);
        // framed (book §8 "The frame"): the title from column 3 in the
        // top edge, the path and the counts ending at F - 4
        let head = &rows[0];
        assert!(head.starts_with("╭─ bise :* ─"), "{:?}", head);
        let right = "bench · ∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done";
        assert!(head.ends_with(&format!(" {} ─╮", right)), "{:?}", head);
        assert_eq!(head.chars().count(), 120, "{:?}", head);
        assert!(!rows.iter().any(|r| r.contains("Switchboard")));
        // 60 columns, no panel: the short counts
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].ends_with(" bench · ∿ 3 · … 1 · ? 1 · ♡ 1 ─╮"), "{:?}", rows[0]);
        // too narrow for the path: it goes first
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].starts_with("╭─ bise :* ─"), "{:?}", rows[0]);
        // under 60 columns: no frame, the header row with margins of 1
        let rows = draw(&mut app, 59, 20);
        let summary = "bench · ∿ 3 · … 1 · ? 1 · ♡ 1";
        assert_eq!(rows[0], format!(" bise :*{}{}", " ".repeat(59 - 8 - summary.chars().count() - 1), summary));
        // no agents: the words
        let mut app = with_main();
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :*") && rows[0].ends_with("no agents yet ─╮"), "{:?}", rows[0]);
        let rows = draw(&mut app, 59, 20);
        assert!(rows[0].ends_with("no agents yet"), "{:?}", rows[0]);
    }

    /// The summary drops the path first, then the words.
    #[test]
    fn summary_drops_the_path_first() {
        let app = busy();
        let sb = app.sb.as_ref().unwrap();
        let text = |room: usize, short: bool| sb.summary(room, short).iter().map(|s| s.content.to_string()).collect::<String>();
        let full = "∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done";
        assert_eq!(text(100, false), format!("bench · {}", full));
        assert_eq!(text(full.chars().count() + 7, false), full);
        assert_eq!(text(full.chars().count() - 1, false), "∿ 3 · … 1 · ? 1 · ♡ 1");
    }

    /// Colors of the header: `:*` and "needs you" in accent, the rest dim.
    #[test]
    fn header_colors() {
        let app = busy();
        let line = app.sb.as_ref().unwrap().header(120, false);
        let color_of = |t: &str| line.spans.iter().find(|s| s.content.contains(t)).map(|s| s.style.fg);
        assert_eq!(color_of(":*"), Some(Some(accent())));
        assert_eq!(color_of("needs you"), Some(Some(accent())));
        assert_eq!(color_of("working"), Some(Some(dim())));
    }

    /// First run: the header says `no agents yet`, the feed the three
    /// dim lines of the copy deck, the status row `main · idle`, the
    /// composer `›` with the main hints (mockup "first run").
    #[test]
    fn first_run_screen() {
        let mut app = with_main();
        let rows = draw(&mut app, 120, 24);
        let all = rows.join("\n");
        for l in FIRST_RUN {
            assert!(rows.iter().any(|r| r.contains(l)), "{:?} missing:\n{}", l, all);
        }
        assert_eq!(FIRST_RUN[0], "what's on your mind?");
        assert_eq!(FIRST_RUN[1], "say it and keep talking. the work runs in the background, i'm always here.");
        assert_eq!(FIRST_RUN[2], "try: \"fix the flaky login test, and draft the release note\"");
        // the composer pane (book §8 "The frame"): the divider says who
        // you talk to and what it does
        let at = rows.iter().position(|r| r.starts_with("├─ you → main ─")).unwrap_or_else(|| panic!("{}", all));
        assert!(rows[at].ends_with(" idle ─┤"), "{:?}", rows[at]);
        assert!(rows[at].contains('┴'), "the panel's rule joins it: {:?}", rows[at]);
        // then 1 blank bar row, the text, 1 blank bar row: the bar at
        // column 3; the key bar from column 3; the frame's bottom edge
        assert!(rows[at + 1..rows.len() - 2].iter().all(|r| r.starts_with("│  │")), "{}", all);
        // empty, the composer asks
        assert!(rows[at + 2].starts_with(&format!("│  │   {}", PLACEHOLDER_MAIN)), "{:?}", rows[at + 2]);
        let keys = &rows[rows.len() - 2];
        assert!(keys.starts_with("│  ⏎ send   @ agent"), "{:?}", keys);
        assert!(rows.last().unwrap().starts_with("╰─"), "{}", all);
        // a panel with main only
        assert!(rows.iter().any(|r| r.contains(&format!("│  0 {} main", G_MAIN))), "{}", all);
        // once there is an agent, the first-run text goes
        bench::add_agent(&mut app, "auth-fix", "the safari login");
        let rows = draw(&mut app, 120, 24);
        assert!(!rows.iter().any(|r| r.contains(FIRST_RUN[1])));
    }

    /// Inside an agent (mockup "inside an agent"): the pinned line of the
    /// copy deck on top of the feed, the status row starts with the
    /// agent's name in accent, then dim; `shared folder`.
    #[test]
    fn inside_an_agent_screen() {
        let mut app = with_main();
        app.sb.as_mut().unwrap().agents.push(Agent {
            name: "auth-fix".into(),
            status: "idle".into(),
            mode: "shared".into(),
            path: "/ws".into(),
            ..Agent::default()
        });
        focus(&mut app, "auth-fix");
        let mut term = Terminal::new(TestBackend::new(112, 24)).unwrap();
        term.draw(|f| super::super::draw_sb(&mut app, f)).unwrap();
        let buf = term.backend().buffer().clone();
        let rows: Vec<String> = buf
            .content
            .chunks(112)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect();
        let all = rows.join("\n");
        let line = "you're talking to auth-fix directly. main isn't in the loop. esc back to main.";
        // the header row, 1 blank row, then the pinned line (book §8),
        // wrapped in the column when it is narrower
        let left = |r: &String| r.chars().skip(1).take(76).collect::<String>().trim().to_string();
        assert!(format!("{} {}", left(&rows[2]), left(&rows[3])).trim().contains(line), "{}", all);
        // the divider: `you →` dim, the name in accent, the state dim
        let y = rows.iter().position(|r| r.starts_with("├─ you → auth-fix ─")).unwrap_or_else(|| panic!("{}", all));
        assert!(rows[y].ends_with(" idle · shared folder ─┤"), "{:?}", rows[y]);
        let cell = |x: usize| buf.cell((x as u16, y as u16)).unwrap().fg;
        assert_eq!(cell(3), dim(), "you → dim");
        assert_eq!(cell(9), accent(), "the name in accent");
        let idle = rows[y].chars().count() - " idle · shared folder ─┤".chars().count() + 1;
        assert_eq!(cell(idle), dim(), "the state dim");
        assert!(!all.contains("task"), "no \"task\" in the chrome:\n{}", all);
    }

    /// A click lands on the feed row drawn under it: the feed starts
    /// under the header (and, inside an agent, under the pinned line).
    #[test]
    fn feed_clicks_land_on_the_row_under_the_header() {
        for inside in [false, true] {
            let mut app = with_main();
            if inside {
                bench::add_agent(&mut app, "auth-fix", "the safari login");
                focus(&mut app, "auth-fix");
            }
            for k in 0..5 {
                push_event(&mut app.events, &mut app.cache, Ev::Info(format!("event {}", k)));
            }
            let rows = draw(&mut app, 100, 24);
            assert!(app.feed_y >= 1, "the header is above the feed");
            if inside {
                assert!(rows[2].contains("you're talking to auth-fix"));
                // under the pinned line (1 or 2 rows) and a blank row
                assert!(app.feed_y >= 4, "{}", app.feed_y);
            }
            for k in 0..5 {
                let label = format!("event {}", k);
                let y = rows.iter().position(|r| r.contains(&label)).unwrap_or_else(|| panic!("{}", rows.join("\n")));
                let x = rows[y].find(&label).unwrap() as u16;
                let pos = crate::input::feed_pos(&app, x, y as u16, false).expect("a feed row");
                assert_eq!(pos.0, k, "inside {}: {} at screen row {}", inside, label, y);
            }
            // the header row is not the feed
            assert!(crate::input::feed_pos(&app, 5, 0, false).is_none());
        }
    }

    /// The status row: lowercase, the context, a preview note, the
    /// steer hints during a turn.
    #[test]
    fn status_row_and_hints() {
        let mut app = busy();
        assert_eq!(status_text(&app), "main · idle");
        assert_eq!(key_mode(&app), Some(crate::keybar::Mode::Default));
        app.pending = true;
        assert_eq!(key_mode(&app), Some(crate::keybar::Mode::Steer));
        app.pending = false;
        let sb = app.sb.as_mut().unwrap();
        sb.selected = Some(1);
        sb.preview = true;
        let text = status_text(&app);
        assert!(text.ends_with("preview of auth-fix"), "{:?}", text);
    }
}
