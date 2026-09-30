//! The chrome of the switchboard mode: the header row, the agents panel
//! on the right, the status row, the key hints and the composer
//! placeholder (bise book §8, §17).

use super::*;
use unicode_width::UnicodeWidthStr;

/// The status glyph of an agent and its color (book §6): the gust
/// breathes in its cell while it works (BISE-107), `·` pulses while it
/// starts; only "needs you" and a failure get a hue, and done's check is
/// accent (BISE-100).
pub(super) fn glyph(status: &str, tick: u32, motion: crate::gust::Motion) -> (&'static str, Color) {
    match status {
        "working" => crate::gust::cell(motion),
        "starting" => starting_frame(tick),
        "waiting" => (G_WAITING, text()),
        "blocked" => (G_NEEDS_YOU, accent()),
        "done" => (crate::theme::done_glyph(), accent()),
        "failed" => (G_FAILED, error()),
        "idle" => (G_IDLE, dim()),
        "stopped" | "archived" => (G_STOPPED, dim()),
        _ => (G_STARTING, faint()),
    }
}

/// The workspace folder (the embedded terminal starts there).
/// The model of the agent in view (the no-vision check, BISE-150).
pub(crate) fn focus_model(app: &App) -> String {
    app.sb.focus_model(app)
}

pub(crate) fn workspace(app: &App) -> Option<String> {
    Some(app.sb.workspace.clone()).filter(|w| !w.is_empty())
}

/// BISE-264: the folders a relative path in `who`'s feed resolves
/// against (file links): its private worktree, its own folder, then
/// the workspace.
pub(crate) fn feed_dirs(app: &App, who: &str) -> Vec<std::path::PathBuf> {
    let sb = &app.sb;
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    if let Some(a) = sb.agent(who) {
        out.extend([&a.place, &a.path].into_iter().filter(|p| !p.is_empty()).map(Into::into));
    }
    if !sb.workspace.is_empty() {
        out.push(sb.workspace.clone().into());
    }
    out
}

/// The feed and composer area, and the panel on the right when it fits.
pub(crate) fn split(full: Rect) -> (Rect, Option<Rect>) {
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
pub(super) fn needs_you(sb: &Sb, a: &Agent) -> bool {
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
        // the glyph cell alone takes its color: a new gust frame rewrites
        // that one cell, not the space after it
        Span::styled(g.0.to_string(), Style::default().fg(g.1)),
        Span::raw(" "),
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

/// BISE-136: where `a` works when it is not the shared checkout, for
/// `ψ {label}`: its hub worktree's branch (`sb spawn --worktree`,
/// `/isolate`), else the name of the private worktree it told the hub
/// about (`gate.sh new`: `/tmp/<task>-wt`). None: the shared checkout,
/// which shows nothing (quiet is normal).
pub(crate) fn place_label(a: &Agent) -> Option<String> {
    let base = |p: &str| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p).to_string();
    if let Some(b) = &a.branch {
        return Some(b.clone());
    }
    if a.mode == "worktree" {
        return Some(base(&a.path));
    }
    (!a.place.is_empty()).then(|| base(&a.place))
}

/// The row of live agent `a`, entry `i` of the panel, number `num`
/// (0 main; blank after 9).
fn agent_row(app: &App, sb: &Sb, a: &Agent, i: usize, num: Option<usize>, w: usize, tag: &Tag) -> Line<'static> {
    let focused = a.name == sb.focus;
    let selected = sb.selected == Some(i);
    // BISE-119: main's status sits in the same column as every agent's
    // (the breathing gust while it works, `○` idle); its `:*` follows its name
    let g = if !a.main && needs_you(sb, a) {
        (G_NEEDS_YOU, accent())
    } else {
        glyph(&a.status, app.tick, app.motion_away)
    };
    let name_style = if focused {
        Style::default().fg(accent()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(text())
    };
    let mut marks = Vec::new();
    if a.main {
        marks.push(Span::styled(format!(" {}", G_MAIN), Style::default().fg(accent())));
    }
    if sb.activity.contains(&a.name) && !focused {
        marks.push(Span::styled(format!(" {}", G_UNREAD), Style::default().fg(accent())));
    }
    if place_label(a).is_some() {
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
    // BISE-135: the model·effort tag in its column, before the state;
    // it goes first when the row is narrow (the name keeps 6 columns)
    let right = right_of(app, sb, a);
    if tag.width > 0 && w >= TAG_MIN_PANEL {
        let right_w = if right.0.is_empty() { 0 } else { right.0.width() + 1 };
        let marks_w: usize = marks.iter().map(|s| s.content.width()).sum();
        // ` N G ` + marks + `  tag` + the widest right side + the margin:
        // every row's tag starts in the same column
        let fixed = 5 + marks_w + 2 + tag.width + tag.right + 1;
        if w >= fixed + a.name.width().min(6) {
            let t = tag.of(a);
            let color = if tag.differs(a) { dim() } else { faint() };
            let pad = (w - fixed).saturating_sub(a.name.width());
            let after = tag.width.saturating_sub(t.width()) + tag.right.saturating_sub(right_w);
            marks.push(Span::raw(" ".repeat(pad + 2)));
            marks.push(Span::styled(t, Style::default().fg(color)));
            marks.push(Span::raw(" ".repeat(after)));
        }
    }
    // the name takes all the room left of the marks and the state; it is
    // cut only there (BISE-109: no fixed cap)
    let mut l = row(num, g, &a.name, name_style, marks, right, w, bg);
    // option held (ctrlhint.rs): ` 1 ` reads `⌥1 `, in the accent
    if let (Some(k), Some(first)) = (num.and_then(|n| crate::ctrlhint::number(app, n)), l.spans.first_mut()) {
        *first = Span::styled(format!("{k} "), first.style.fg(accent()));
    }
    l
}

/// The panel is at least this wide to show the tags (the designer's
/// layout: narrower, the state keeps the room).
const TAG_MIN_PANEL: usize = 44;

/// The tags of the panel's rows (BISE-135): `opus·hi`, aligned in one
/// column (the longest tag, at most 12 columns); a tag that differs
/// from main's is dim, the others faint.
pub(super) struct Tag {
    width: usize,
    /// the widest right side of the rows (its leading space included)
    right: usize,
    main: String,
    models: Vec<String>,
}

impl Tag {
    fn new(app: &App, sb: &Sb, agents: &[&Agent]) -> Tag {
        let models: Vec<String> = agents.iter().map(|a| a.model.clone()).collect();
        let main = agents.iter().find(|a| a.main).map(|a| a.model.clone() + "|" + &a.effort).unwrap_or_default();
        let right = agents
            .iter()
            .map(|a| right_of(app, sb, a).0.width())
            .filter(|w| *w > 0)
            .map(|w| w + 1)
            .max()
            .unwrap_or(0);
        let mut t = Tag { width: 0, right, main, models };
        t.width = agents.iter().map(|a| t.of(a).width()).max().unwrap_or(0).min(12);
        t
    }

    pub(super) fn of(&self, a: &Agent) -> String {
        let others: Vec<&str> = self.models.iter().map(String::as_str).collect();
        let t = crate::models::tag(&a.model, &a.effort, &others);
        fit(&t, 12)
    }

    fn differs(&self, a: &Agent) -> bool {
        !a.main && a.model.clone() + "|" + &a.effort != self.main
    }
}

/// The live agents (main and the archived left out) by what the header
/// counts: working, waiting, needs you, done; then the open cards
/// (BISE-125). None: no agent and no card.
fn counts(sb: &Sb) -> Option<[usize; 5]> {
    let live: Vec<&Agent> = sb.agents.iter().filter(|a| !a.main && !a.archived()).collect();
    if live.is_empty() && sb.cards.is_empty() {
        return None;
    }
    let mut n = [0, 0, 0, 0, sb.cards.len()];
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
/// cards, working, waiting, done), shown in the §8 order. `gust` leads the
/// working count (BISE-107). The open cards (`# 3 in the inbox`, dim) come last,
/// so the number main says (`card #153`) is found in the panel or with
/// ctrl+g even when the panel is hidden (BISE-125).
fn fit_counts(n: [usize; 5], short: bool, room: usize, gust: &[Span<'static>]) -> Vec<Span<'static>> {
    // (glyph, word, glyph color, text color): done's check is accent on
    // dim words (BISE-100)
    let parts = [
        (G_WORKING, "working", dim(), dim()),
        (G_WAITING, "waiting", dim(), dim()),
        (G_NEEDS_YOU, "needs you", accent(), accent()),
        (crate::theme::done_glyph(), "done", accent(), dim()),
        ("#", "in the inbox", dim(), dim()),
    ];
    let spans = |keep: &[usize], words: bool| -> Vec<Span<'static>> {
        let mut out: Vec<Span<'static>> = Vec::new();
        for k in (0..5).filter(|k| keep.contains(k)) {
            if !out.is_empty() {
                out.push(Span::styled(" · ", Style::default().fg(dim())));
            }
            let (g, word, g_color, color) = parts[k];
            let t = if words { format!(" {} {}", n[k], word) } else { format!(" {}", n[k]) };
            if k == 0 {
                out.extend(gust.iter().cloned());
            } else {
                out.push(Span::styled(g, Style::default().fg(g_color)));
            }
            out.push(Span::styled(t, Style::default().fg(color)));
        }
        out
    };
    let by_importance: Vec<usize> = [2, 4, 0, 1, 3].into_iter().filter(|&k| n[k] > 0).collect();
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
    /// non-zero counts `∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1
    /// done` (`? … needs you` in accent), shortened to `∿ 3 · ? 1` when
    /// `short` (no panel), or `no agents yet`. A method, so `ui.rs` reaches
    /// it through `app.sb` (the `panel` module is private to `sb`). `gust`
    /// leads the working count (BISE-107).
    pub(crate) fn header(&self, width: u16, short: bool, gust: &[Span<'static>]) -> Line<'static> {
        let mut spans = vec![Span::raw(" ")];
        spans.extend(self.title());
        let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
        // one column of margin on the right, two of gap after `bise :*`
        let room = (width as usize).saturating_sub(left_w + 3);
        let (role, right) = chrome::share_room(room, self.role_spans(), |r| self.summary(r, short, gust));
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        spans.extend(role);
        let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
        let pad = (width as usize).saturating_sub(left_w + right_w + 1);
        if pad >= 2 {
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(right);
        }
        Line::from(spans)
    }

    /// The role line of the task you view (BISE-126), dim, after the
    /// title: ` · fixing the safari login`; nothing in main's view or
    /// before the hub sends one.
    pub(crate) fn role_spans(&self) -> Vec<Span<'static>> {
        let Some(a) = self.agents.iter().find(|a| a.name == self.focus && !a.main) else {
            return Vec::new();
        };
        let role = a.role.split_whitespace().collect::<Vec<_>>().join(" ");
        if role.is_empty() {
            return Vec::new();
        }
        let st = Style::default().fg(dim());
        vec![Span::styled(format!(" {} ", crate::theme::glyph("·")), st), Span::styled(role, st)]
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
    pub(crate) fn summary(&self, room: usize, short: bool, gust: &[Span<'static>]) -> Vec<Span<'static>> {
        // a release running (BISE-235): one more item after the counts
        let item = super::release::header_item(self, gust);
        let item_w: usize = item.iter().map(|s| s.content.width()).sum();
        if item.is_empty() || item_w + 3 > room {
            return self.counts_summary(room, short, gust);
        }
        let mut out = self.counts_summary(room - item_w - 3, short, gust);
        if !out.is_empty() {
            out.push(Span::styled(" · ", Style::default().fg(dim())));
        }
        out.extend(item);
        out
    }

    fn counts_summary(&self, room: usize, short: bool, gust: &[Span<'static>]) -> Vec<Span<'static>> {
        let fitted = |room: usize| -> Vec<Span<'static>> {
            match counts(self) {
                None => vec![Span::styled("no agents yet", Style::default().fg(dim()))],
                Some(n) => fit_counts(n, short, room, gust),
            }
        };
        // the path only beside the counts as they are when nothing is short
        let whole = fitted(usize::MAX);
        let whole_w: usize = whole.iter().map(|s| s.content.width()).sum();
        let path = home_path(&self.workspace);
        if path.is_empty() || path.width() + 3 + whole_w > room {
            return fitted(room);
        }
        // only idle agents: no counts, so no separator after the path (QA F)
        if whole.is_empty() {
            return vec![Span::styled(path, Style::default().fg(dim()))];
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

pub(crate) fn draw_panel(app: &App, frame: &mut Frame, area: Rect) {
    let sb = &app.sb;
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
    let tags = Tag::new(app, sb, &nav[..live]);
    let mut owners: Vec<(usize, Hit)> = Vec::new();
    // the first row of the selected entry (the panel scrolls to it)
    let mut sel_row = None;
    for (i, a) in nav.iter().take(live).enumerate() {
        if sb.selected == Some(i) {
            sel_row = Some(lines.len());
        }
        owners.push((lines.len(), Hit::Agent(a.name.clone())));
        let n = numbers.iter().find(|(name, _)| *name == a.name).map(|(_, n)| *n);
        lines.push(agent_row(app, sb, a, i, n, w, &tags));
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
    cards_lines(sb, w, &mut lines, &mut owners, &mut sel_row);
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
        // BISE-272: the hand over the rows a click opens
        for (y, _) in &hits.rows {
            crate::pointer::region(Rect { y: *y, height: 1, ..area }.intersection(area), crate::pointer::Shape::Pointer);
        }
    }
    // the title, then 1 blank row (book §8)
    let mut all = vec![title, Line::from("")];
    all.extend(body);
    frame.render_widget(Paragraph::new(all), area);
    // BISE-290: its text selects, copies and has links
    crate::textlayer::text(area);
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
            Hit::Agent(_) | Hit::Card(_) => 1,
            Hit::Cards => 0,
            Hit::Archived if !archived_open => archived,
            Hit::Archived => 0,
        })
        .sum::<usize>();
    if n == 0 {
        return (lines.len() - h, None);
    }
    (top, Some(n))
}

/// The cards section, under the live agents (BISE-125): a title row
/// `cards · ctrl+g`, then one row per open card, newest first: `#153 ✓
/// debt-solo  its first line…` (the number main says, the kind's glyph
/// in its color, the agent, the text cut to the row). The card in the
/// box is on the selection color; with no agent selected, the panel
/// scrolls to it. Nothing while no card is open.
fn cards_lines(
    sb: &Sb,
    w: usize,
    lines: &mut Vec<Line<'static>>,
    owners: &mut Vec<(usize, Hit)>,
    sel_row: &mut Option<usize>,
) {
    if sb.cards.is_empty() {
        return;
    }
    let mut cards: Vec<&Card> = sb.cards.iter().collect();
    cards.sort_by_key(|c| std::cmp::Reverse(c.id));
    let shown = sb.card.open.then(|| sb.current_card().map(|c| c.id)).flatten();
    lines.push(Line::from(""));
    owners.push((lines.len(), Hit::Cards));
    lines.push(Line::from(vec![
        Span::styled(" inbox", Style::default().fg(text())),
        Span::styled(" · ctrl+g", Style::default().fg(faint())),
    ]));
    for c in cards {
        if shown == Some(c.id) && sel_row.is_none() {
            *sel_row = Some(lines.len());
        }
        owners.push((lines.len(), Hit::Card(c.id)));
        lines.push(card_row(c, w, (shown == Some(c.id)).then(selection_bg)));
    }
}

/// One card's row, `w` columns: ` #153 ✓ debt-solo  first line…`, 1
/// column of margin on the right. The agent keeps its whole name while
/// 6 columns are left for the text, then it is cut too.
fn card_row(c: &Card, w: usize, bg: Option<Color>) -> Line<'static> {
    // the TUI's own cards (setup) have no hub number
    let num = if super::setup::is_local(c.id) { " ".to_string() } else { format!(" #{} ", c.id) };
    let g = super::cards::kind_look(&c.kind).1;
    let lead = num.width() + g.width() + 1;
    let room = w.saturating_sub(lead + 1);
    let first = c.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let agent = if first.is_empty() || room < 12 {
        fit(&c.agent, room)
    } else {
        fit(&c.agent, room.saturating_sub(7).max(room / 2))
    };
    let rest = room.saturating_sub(agent.width() + 2);
    let title = if rest >= 3 { fit(first, rest) } else { String::new() };
    let mut spans = vec![
        Span::styled(num, Style::default().fg(dim())),
        Span::styled(g.to_string(), Style::default().fg(super::cards::glyph_color(&c.kind))),
        Span::raw(" "),
        Span::styled(agent, Style::default().fg(text())),
    ];
    if !title.is_empty() {
        spans.push(Span::styled(format!("  {}", title), Style::default().fg(dim())));
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    spans.push(Span::raw(" ".repeat(w.saturating_sub(used))));
    if let Some(bg) = bg {
        spans = spans.into_iter().map(|s| { let st = s.style.bg(bg); s.style(st) }).collect();
    }
    Line::from(spans)
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
    /// An open card: shown in the card box (BISE-125).
    Card(u64),
    /// The title of the cards section: ctrl+g.
    Cards,
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
    let sb = &app.sb;
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
            let sb = &mut app.sb;
            sb.toggle_archived();
        }
        Some(Hit::Card(id)) => super::cards::open_view(app, Some(id)),
        Some(Hit::Cards) if app.sb.card.open => super::cards::close_view(app),
        Some(Hit::Cards) => super::cards::open_view(app, None),
        _ => {}
    }
    true
}

/// The state of the agent you talk to (book §8 "The frame": the right of
/// What the divider says after the name of the agent you view
/// (BISE-135, BISE-136): its model and effort, where it works.
pub(crate) fn viewed_who(app: &App) -> crate::chrome::Who {
    let sb = &app.sb;
    let Some(a) = sb.agent(&sb.focus) else {
        return crate::chrome::Who::default();
    };
    let others: Vec<&str> = sb.agents.iter().filter(|x| !x.archived()).map(|x| x.model.as_str()).collect();
    crate::chrome::Who {
        model: if a.model.is_empty() { String::new() } else { crate::models::long_name(&a.model) },
        effort: a.effort.clone(),
        tag: crate::models::tag(&a.model, &a.effort, &others),
        place: place_label(a),
    }
}

/// The model of the agent in view, its effort and the efforts its model
/// takes (the `/model` and `/reasoning` popups, BISE-135): (name,
/// model, effort, efforts).
pub(crate) fn viewed_model(app: &App) -> (String, String, String, Vec<String>) {
    let sb = &app.sb;
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let model = if a.model.is_empty() { sb.focus_model(app) } else { a.model.clone() };
    let efforts = if a.efforts.is_empty() && a.model.is_empty() { crate::models::efforts(&model).0 } else { a.efforts.clone() };
    (sb.focus.clone(), model, a.effort, efforts)
}

/// The agent you view, when it works (book §8, BISE-105): the gust's
/// motion and the current turn's age, for the divider's label.
pub(crate) fn viewed_working(app: &App) -> Option<crate::chrome::Working> {
    let sb = &app.sb;
    let a = sb.agent(&sb.focus).filter(|a| a.status == "working")?;
    Some(crate::chrome::Working { motion: app.motion, age: a.turn_age_ms().map(short_age) })
}

/// The state of the agent you talk to (book §8 "The frame": the right of
/// the divider; it was the status row), dim: its state, the turn's
/// duration, its context (`ψ branch` moved to the label, with the
/// model: [`viewed_who`]), then the notes
/// (preview, read-only, cards, the hub's version); or the `D` question,
/// in accent. `idle · 210k / 1M tokens · 21%`.
pub(crate) fn status_state(app: &App) -> Option<Line<'static>> {
    let sb = &app.sb;
    if let Some(name) = &sb.drop_ask {
        return Some(Line::from(Span::styled(drop_question(name), Style::default().fg(accent()))));
    }
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let d = |t: String| Span::styled(format!(" · {}", t), Style::default().fg(dim()));
    let mut spans: Vec<Span<'static>> = Vec::new();
    // working: the label says `working · 42s` already (viewed_working),
    // one status and one timer (QA N); else the state, and the turn's age
    // as it moves on
    if a.status != "working" {
        if !a.status.is_empty() {
            spans.push(d(a.status.clone()));
        }
        if let Some(ms) = a.turn_age_ms().filter(|_| app.pending) {
            spans.push(d(short_age(ms)));
        }
    }
    if let Some(u) = crate::usage::current(&app.events) {
        spans.push(d(u.label()));
    }
    if a.archived() {
        spans.push(d("read-only history · /restore brings it back".into()));
    }
    if sb.preview {
        if let Some(sel) = sb.selected_agent().map(|a| a.name.clone()) {
            spans.push(d(format!("preview of {}", sel)));
        }
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
/// The header's gust without motion: one `∿` (tests draw still).
#[cfg(test)]
fn still_gust() -> Vec<Span<'static>> {
    crate::gust::mark(crate::gust::Motion::Still, crate::gust::Size::Five)
}

#[cfg(test)]
pub(crate) fn status_text(app: &App) -> String {
    let sb = &app.sb;
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

/// The key bar's mode once no overlay or popup has it (BISE-99,
/// [`crate::keybar`]).
pub(crate) fn key_mode(app: &App) -> crate::keybar::Mode {
    use crate::keybar::Mode;
    let sb = &app.sb;
    if sb.drop_ask.is_some() {
        Mode::DropAsk
    } else if sb.confirm.is_some() {
        Mode::Confirm
    } else if sb.card.open {
        Mode::Card
    } else if sb.card.inbox.is_some() {
        Mode::Inbox
    } else if sb.selected.is_some() {
        Mode::Selected
    } else if sb.focus_archived() {
        Mode::Archived
    } else if app.pending {
        Mode::Steer
    } else {
        Mode::Default
    }
}

/// What the empty composer shows after the cursor, dim (book §8 "The
/// frame"): `what's on your mind?` to main, `talk to auth-fix directly`
/// inside an agent, a read-only note in an archived agent's history.
pub(crate) fn placeholder(app: &App) -> Option<String> {
    let sb = &app.sb;
    Some(if sb.card.inbox.is_some() && !sb.card.open {
        // the inbox selected (ctrl+g): the composer waits
        "your message waits here".to_string()
    } else if sb.focus_archived() {
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
    "try: \"show me what you can do\"",
];

/// The first-run text's suggestion (BISE-284): a click on it fills the
/// composer, and the first open after the onboarding starts with it.
pub(crate) const DEMO: &str = "show me what you can do";

/// What the first-run text's last line says while the composer holds
/// [`DEMO`] (designer, BISE-284): the key, then the words.
pub(crate) const DEMO_READY: (&str, &str) = ("⏎", " try it · or just type your own");

/// The composer holds `show me what you can do`, selected: typing
/// replaces it, enter sends it, an arrow or esc keeps it (BISE-284).
pub(crate) fn fill_demo(app: &mut App) {
    app.ed.set(DEMO, DEMO.chars().count());
    app.ed.select_all();
    app.feed_sel = None;
}

/// The first open of main's thread after the first-run onboarding: the
/// composer starts with [`DEMO`] (BISE-284), when the first-run text
/// shows and nothing is typed yet.
pub(crate) fn prefill_demo(app: &mut App) {
    let said = app.events.iter().any(|e| matches!(e, crate::Ev::You(..)));
    if app.sb.first_run().is_some() && app.ed.is_empty() && !said {
        fill_demo(app);
    }
}

/// The composer holds [`DEMO`] as it came: the first-run text says
/// `⏎ try it` (BISE-284).
pub(crate) fn demo_ready(app: &App) -> bool {
    app.ed.text == DEMO
}

impl Sb {
    /// No agents yet and main in view: the first-run text, which shows
    /// until your first message (the setup card may wait in the strip,
    /// BISE-245).
    pub(crate) fn first_run(&self) -> Option<[&'static str; 3]> {
        let live = self.agents.iter().any(|a| !a.main && !a.archived());
        (self.focus == "main" && !live && !self.preview).then_some(FIRST_RUN)
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
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        bench::add_agent(&mut app, "alpha", "first objective");
        bench::add_agent(&mut app, "beta", "second objective");
        bench::add_agent(&mut app, "gamma", "third objective");
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let draw = |app: &mut App, term: &mut Terminal<TestBackend>| {
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        };
        draw(&mut app, &mut term);
        let panel_x = app.sb.panel_hits.borrow().area.x;
        for (label, name) in [("alpha", "alpha"), ("beta", "beta")] {
            let (x, y) = find(&screen(&term), panel_x, label);
            click(&mut app, x, y);
            assert_eq!(app.sb.focus, name, "click on {:?}", label);
            draw(&mut app, &mut term);
        }
        // the selected agent shows its objective under its row
        let sb = &mut app.sb;
        sb.selected = Some(3);
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "third objective");
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "gamma");
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, "main");
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "main");
        // the title row: nothing happens
        draw(&mut app, &mut term);
        let (x, y) = find(&screen(&term), panel_x, PANEL_TITLE.0);
        click(&mut app, x, y);
        assert_eq!(app.sb.focus, "main");
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
        let sb = &mut app.sb;
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
            look: None,
        });
        sb.activity.insert("auth-fix".into());
        app
    }

    /// The panel alone, `w` columns wide, `h` rows high.
    fn panel_rows(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_panel(app, f, f.area())).unwrap();
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
            assert_eq!(row("main"), format!(" 0 {} main {}", G_IDLE, G_MAIN));
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
        // 28 columns: the whole name when it fits; narrower, it is cut
        // only there, the worktree mark kept
        let t = trimmed(&panel_rows(&app, 28, 16));
        assert!(t.iter().any(|r| r.contains(&format!("big-refactor-of-auth {}", G_WORKTREE))), "{t:?}");
        let t = trimmed(&panel_rows(&app, 22, 16));
        let big = t.iter().find(|r| r.contains("big-")).unwrap();
        assert!(big.contains("…") && big.contains(G_WORKTREE), "{:?}", big);
        // with room, the whole name: no fixed cap (BISE-109)
        let t = trimmed(&panel_rows(&app, 40, 16));
        assert!(t.iter().any(|r| r.contains(&format!("big-refactor-of-auth {}", G_WORKTREE))), "{}", t.join("\n"));
    }

    /// BISE-135: each row shows its model·effort tag in one column,
    /// before the state; a tag unlike main's is dim, the others faint;
    /// under 44 columns, no tag (the state keeps its room).
    #[test]
    fn each_row_names_its_model() {
        let mut app = bench::test_app_drained();
        let with = |a: Agent, m: &str, e: &str| Agent { model: m.into(), effort: e.into(), ..a };
        app.sb.agents = vec![
            with(Agent { main: true, ..agent("main", "idle") }, "foundry/claude-opus-5-5", "high"),
            with(agent("auth-fix", "working"), "foundry/claude-opus-5-5", "high"),
            with(agent("release", "working"), "anthropic/claude-sonnet-4-5", "low"),
            with(agent("docs", "done"), "openai/gpt-4.1", ""),
        ];
        let t = trimmed(&panel_rows(&app, 50, 8));
        let row = |n: &str| t.iter().find(|r| r.contains(n)).cloned().unwrap_or_default();
        assert!(row("main").contains("opus·hi"), "{t:?}");
        assert!(row("auth-fix").contains("opus·hi"), "{t:?}");
        assert!(row("release").contains("sonnet·lo"), "{t:?}");
        assert!(row("docs").contains("gpt-4.1"), "{t:?}");
        // one column: the tags start at the same x
        let col = |n: &str, tag: &str| row(n).find(tag).map(|i| row(n)[..i].chars().count());
        assert_eq!(col("auth-fix", "opus"), col("release", "sonnet"), "{t:?}");
        assert_eq!(col("main", "opus"), col("docs", "gpt"), "{t:?}");
        // colors: the one unlike main's is dim
        let mut term = Terminal::new(TestBackend::new(50, 8)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        let color_of = |n: &str, tag: &str| {
            let y = t.iter().position(|r| r.contains(n))? as u16;
            let x = col(n, tag)? as u16;
            Some(buf[(x, y)].fg)
        };
        assert_eq!(color_of("release", "sonnet"), Some(dim()));
        assert_eq!(color_of("auth-fix", "opus"), Some(faint()));
        // narrow: no tag, the state stays
        let t = trimmed(&panel_rows(&app, 40, 8));
        assert!(!t.iter().any(|r| r.contains("opus") || r.contains("sonnet")), "{t:?}");
        // the divider: main's long form
        app.sb.focus = "release".into();
        let w = viewed_who(&app);
        assert_eq!((w.model.as_str(), w.effort.as_str(), w.tag.as_str()), ("sonnet 4.5", "low", "sonnet·lo"));
    }

    /// BISE-136: ψ marks an agent out of the shared checkout (a hub
    /// worktree or a private one), with its branch or worktree name in
    /// the divider; the shared checkout shows nothing.
    #[test]
    fn where_an_agent_works() {
        let hub = Agent { branch: Some("sb/big".into()), mode: "worktree".into(), ..agent("big", "working") };
        let private = Agent { mode: "shared".into(), place: "/tmp/fix-wt/".into(), ..agent("fix", "working") };
        let shared = Agent { mode: "shared".into(), path: "/ws".into(), ..agent("docs", "idle") };
        assert_eq!(place_label(&hub).as_deref(), Some("sb/big"));
        assert_eq!(place_label(&private).as_deref(), Some("fix-wt"));
        assert_eq!(place_label(&shared), None);
        let mut app = bench::test_app_drained();
        app.sb.agents = vec![Agent { main: true, ..agent("main", "idle") }, private, shared];
        let t = trimmed(&panel_rows(&app, 40, 8));
        assert!(t.iter().any(|r| r.contains(&format!("fix {}", G_WORKTREE))), "{t:?}");
        assert!(!t.iter().any(|r| r.contains("docs") && r.contains(G_WORKTREE)), "{t:?}");
        // the place is in the divider's label (BISE-135: after the model),
        // not in the state
        app.sb.focus = "fix".into();
        assert_eq!(viewed_who(&app).place.as_deref(), Some("fix-wt"));
        let state: String = status_state(&app).unwrap().spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains(G_WORKTREE), "{state:?}");
        app.sb.focus = "docs".into();
        assert_eq!(viewed_who(&app).place, None);
        let state: String = status_state(&app).unwrap().spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains(G_WORKTREE) && !state.contains("shared"), "{state:?}");
    }

    /// Colors: the number faint, the agent in view in accent, "needs
    /// you" in accent, a failure in error, the right side dim.
    #[test]
    fn panel_colors() {
        let mut app = every_state();
        app.sb.focus = "bench".into();
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
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

    /// BISE-119: main's row lines up with the others: its status in the
    /// agents' glyph column (the breath while it works, a new frame
    /// rewrites that one cell only; `○` idle; no motion, one static `∿`),
    /// its name in their name column, `:*` (accent, still) after it.
    #[test]
    fn main_status_sits_in_the_glyph_column() {
        use crate::gust::{Motion, W1};
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "working".into(), ..Agent::default() });
        sb.agents.push(agent("ideas", "idle"));
        let mut term = Terminal::new(TestBackend::new(28, 6)).unwrap();
        let mut draw = |app: &mut App, m: Motion| {
            (app.motion, app.motion_away) = (m, m);
            term.draw(|f| draw_panel(app, f, f.area())).unwrap();
            (screen(&term), term.backend().buffer().clone())
        };
        let row_of = |rows: &[String], n: &str| rows.iter().find(|r| r.contains(n)).unwrap().trim_end().to_string();
        let mark_x = " 0 ∿ main ".chars().count() as u16;
        let mut last = None;
        for i in 0..W1.breath.len() as u64 {
            let (rows, buf) = draw(&mut app, Motion::Frame(i));
            let breath = W1.breath[i as usize];
            assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", breath, G_MAIN), "frame {i}");
            // the same columns as an agent's glyph and name
            let ideas = row_of(&rows, "ideas");
            assert_eq!(ideas.chars().position(|c| c == 'i'), row_of(&rows, "main").chars().position(|c| c == 'm'));
            let y = rows.iter().position(|r| r.contains("main")).unwrap() as u16;
            assert_eq!(buf.cell((3, y)).unwrap().fg, crate::gust::cell(Motion::Frame(i)).1, "frame {i}");
            assert_eq!(buf.cell((mark_x, y)).unwrap().fg, accent(), "`:*` stays accent");
            // the next frame rewrites main's gust cell, nothing else
            if let Some(prev) = last.replace(buf.clone()) {
                let d = prev.diff(&buf);
                assert_eq!(d.iter().map(|(x, y, _)| (*x, *y)).collect::<Vec<_>>(), vec![(3, y)], "frame {i}");
            }
        }
        // no motion (focus lost, BISE_REDUCE_MOTION, a slow draw): one still `∿`
        let (rows, a) = draw(&mut app, Motion::Still);
        assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", W1.still.0, G_MAIN));
        let (_, b) = draw(&mut app, Motion::Still);
        assert!(a.diff(&b).is_empty());
        // idle: an idle agent's glyph, and the frames change no cell
        app.sb.agents[0].status = "idle".into();
        let (rows, a) = draw(&mut app, Motion::Frame(0));
        assert_eq!(row_of(&rows, "main"), format!(" 0 {} main {}", G_IDLE, G_MAIN));
        assert!(row_of(&rows, "ideas").starts_with(&format!(" 1 {} ideas", G_IDLE)));
        let (_, b) = draw(&mut app, Motion::Frame(1));
        assert!(a.diff(&b).is_empty());
        // zen (BISE-132): the panel's gust stands still while the one of
        // the agent in view (the divider's label) is calm
        app.sb.agents[0].status = "working".into();
        app.motion = Motion::Calm(3);
        app.motion_away = Motion::Still;
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        assert_eq!(row_of(&screen(&term), "main"), format!(" 0 {} main {}", W1.still.0, G_MAIN));
        app.sb.focus = "main".into();
        assert_eq!(viewed_working(&app).map(|w| w.motion), Some(Motion::Calm(3)));
    }

    /// Numbers stay while an agent lives: a drop does not renumber the
    /// others, Alt+N follows the number shown; a newcomer takes the free
    /// number, and the rows go in number order (QA M).
    #[test]
    fn numbers_survive_a_drop() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut app = bench::test_app_drained();
        {
            let sb = &mut app.sb;
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for n in ["a", "b", "c"] {
                sb.agents.push(agent(n, "working"));
            }
        }
        let num = |app: &App, n: &str| app.sb.numbers().into_iter().find(|(x, _)| x == n).map(|(_, k)| k);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!((num(&app, "a"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        // a is dropped (archived): b and c keep 2 and 3
        app.sb.agents[1].status = "archived".into();
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 2 {} b", G_WORKING))), "{}", t.join("\n"));
        assert!(t.iter().any(|r| r.starts_with(&format!(" 3 {} c", G_WORKING))), "{}", t.join("\n"));
        assert_eq!(num(&app, "a"), None);
        // Alt+3 goes to c, Alt+1 to no one, Alt+0 to main
        key(&mut app, &KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "c");
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "c", "no agent 1 any more");
        key(&mut app, &KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "main");
        // a newcomer takes the free number 1; the others keep theirs
        app.sb.agents.push(agent("d", "starting"));
        assert_eq!((num(&app, "d"), num(&app, "b"), num(&app, "c")), (Some(1), Some(2), Some(3)));
        // QA M: its row is at its number, before b and c (not last)
        let t = trimmed(&panel_rows(&app, 28, 10));
        let row_of = |p: &str| t.iter().position(|r| r.starts_with(p)).unwrap_or(usize::MAX);
        assert!(row_of(" 1 ") < row_of(" 2 ") && row_of(" 2 ") < row_of(" 3 "), "{}", t.join("\n"));
        key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT), false);
        assert_eq!(app.sb.focus, "d");
        // restored, a comes back with a free number (4)
        app.sb.agents[1].status = "idle".into();
        assert_eq!(num(&app, "a"), Some(4));
    }

    /// More agents than rows: the list ends with `+ {n} more`, and
    /// scrolls to keep the selection in view.
    #[test]
    fn overflow_ends_with_more() {
        let mut app = bench::test_app_drained();
        {
            let sb = &mut app.sb;
            sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
            for i in 1..=30 {
                sb.agents.push(agent(&format!("a{:02}", i), "working"));
            }
        }
        let t = trimmed(&panel_rows(&app, 28, 10));
        // title + 1 blank row + 7 agents + the more row
        assert_eq!(t[9], " + 24 more", "{}", t.join("\n"));
        app.sb.selected = Some(30);
        let t = trimmed(&panel_rows(&app, 28, 10));
        assert!(t.iter().any(|r| r.contains("a30")), "{}", t.join("\n"));
        assert!(!t.iter().any(|r| r.contains("more")), "nothing left below");
        app.sb.selected = Some(15);
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
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        bench::add_agent(&mut app, "alpha", "live objective");
        let sb = &mut app.sb;
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
        let x = app.sb.panel_hits.borrow().area.x;
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
        assert_eq!(app.sb.focus, "mid");
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
        assert_eq!(app.sb.nav().len(), 2);
        press(&mut app, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(app.sb.archived_open);
        assert_eq!(app.sb.selected, Some(0), "selection kept");
        press(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
        let sb = &app.sb;
        assert_eq!(sb.selected_agent().map(|a| a.name.as_str()), Some("old"));
        let x = sb.panel_hits.borrow().area.x;
        let p = panel(&draw(&mut app, &mut term), x.max(90));
        assert!(row_of(&p, "old did its job").is_some(), "{}", p.join("\n"));
        press(&mut app, KeyCode::Char('D'), KeyModifiers::SHIFT);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.sb.focus, "old");
        press(&mut app, KeyCode::Char('2'), KeyModifiers::ALT);
        assert_eq!(app.sb.focus, "old", "Alt+2: no live task 2");
        press(&mut app, KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(app.sb.focus, "alpha");
    }

    /// Hundreds of archived tasks, expanded: the panel scrolls to keep
    /// the selected row in view, and clicks still hit the right row.
    #[test]
    fn a_long_archived_list_scrolls_to_the_selection() {
        let mut app = app();
        let sb = &mut app.sb;
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
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let rows = draw(&mut app, &mut term);
        let x = app.sb.panel_hits.borrow().area.x;
        let p = panel(&rows, x);
        let y = row_of(&p, &format!("{} t000", G_STOPPED)).unwrap_or_else(|| panic!("{}", p.join("\n")));
        click(&mut app, x + 5, y as u16);
        assert_eq!(app.sb.focus, "t000");
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
        app.sb.agents.push(Agent {
            name: "main".into(),
            main: true,
            status: "idle".into(),
            ..Agent::default()
        });
        app
    }

    fn busy() -> App {
        let mut app = with_main();
        let sb = &mut app.sb;
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
        let n = [3, 1, 1, 2, 0];
        let text = |room| fit_counts(n, false, room, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
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

    /// The raised pane (book §13, BISE-102, BISE-212): the grey fills the
    /// inside of the frame under the divider, edge to edge between the
    /// side edges; the lines (the divider, the side and bottom edges) stay
    /// on the history's ground, outside the grey. 6 rows at rest (the
    /// divider, a blank bar row, 1 text row, a blank bar row, the key
    /// bar, the frame; BISE-219), 4 under 20 rows; under 14 rows the key bar takes the divider's right
    /// side.
    #[test]
    fn the_pane_under_the_divider_is_raised() {
        let mut app = with_main();
        let screen = |app: &mut App, w: u16, h: u16| {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
            term.backend().buffer().clone()
        };
        let row = |b: &ratatui::buffer::Buffer, y: u16| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>();
        for (h, pane) in [(40u16, 6u16), (30, 6), (24, 6), (20, 6), (19, 4), (16, 4)] {
            let b = screen(&mut app, 120, h);
            let div = (0..h).find(|&y| row(&b, y).starts_with("├─ you → main")).unwrap_or_else(|| panic!("{h}: no divider"));
            assert_eq!(h - div, pane, "{h} rows: the pane takes {pane}");
            // the history's ground, and the grey is another color
            let ground = b[(60, div - 1)].bg;
            assert_ne!(ground, raised(), "{h} rows");
            for y in div + 1..h - 1 {
                for x in 1..119 {
                    assert_eq!(b[(x, y)].bg, raised(), "{h} rows: ({x}, {y}) is raised");
                }
                // the side edges: on the ground, outside the grey
                assert_eq!((b[(0, y)].symbol(), b[(0, y)].bg), ("│", ground), "{h} rows: the left edge");
                assert_eq!((b[(119, y)].symbol(), b[(119, y)].bg), ("│", ground), "{h} rows: the right edge");
            }
            // the divider (its corners, labels and the panel's join) and
            // the bottom edge: on the ground too, like the history above
            for y in [div - 1, div, h - 1] {
                assert!((0..120).all(|x| b[(x, y)].bg == ground), "{h} rows: row {y} is on the ground: {:?}", row(&b, y));
            }
            assert!(row(&b, h - 1).starts_with('╰'), "{h} rows: {:?}", row(&b, h - 1));
            // the panel's rule stops at the divider: nothing under its join
            let join = row(&b, div).chars().position(|c| c == '┴').unwrap_or_else(|| panic!("{h}: no join"));
            assert!((div + 1..h - 1).all(|y| !["│", "┃"].contains(&b[(join as u16, y)].symbol())), "{h} rows: under the join");
            // the text keeps its colors on the tint: the bar a quiet line, the placeholder dim
            let t = (div + 1..h).find(|&y| row(&b, y).contains(PLACEHOLDER_MAIN)).unwrap();
            assert_eq!((b[(3, t)].symbol(), b[(3, t)].fg), ("│", crate::theme::rule()));
        }
        // bare (under 16 rows): the full width under the divider, down
        // to the last row
        let b = screen(&mut app, 120, 15);
        let div = (0..15).find(|&y| row(&b, y).contains("you → main")).unwrap();
        assert!((div + 1..15).all(|y| (0..120).all(|x| b[(x, y)].bg == raised())));
        assert!((0..120).all(|x| b[(x, div)].bg != raised()), "the divider stays on the ground");
        // under 14 rows the key bar is on the divider's right, no row of its own
        let b = screen(&mut app, 120, 13);
        let div = (0..13).find(|&y| row(&b, y).contains("you → main")).unwrap();
        assert!(row(&b, div).contains("⏎ send   @ file"), "{:?}", row(&b, div));
        assert_eq!(13 - div, 2, "the divider and 1 text row");
    }

    #[test]
    fn header_at_60_and_120() {
        let mut app = busy();
        let rows = draw(&mut app, 120, 20);
        // framed (book §8 "The frame"): the title from column 3 in the
        // top edge, the path and the counts ending at F - 4
        let head = &rows[0];
        assert!(head.starts_with("╭─ bise :* ─"), "{:?}", head);
        let right = "bench · ∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done";
        assert!(head.ends_with(&format!(" {} ─╮", right)), "{:?}", head);
        assert_eq!(head.chars().count(), 120, "{:?}", head);
        assert!(!rows.iter().any(|r| r.contains("Switchboard")));
        // 60 columns, no panel: the short counts
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].ends_with(" bench · ∿ 3 · … 1 · ? 1 · ✓ 1 ─╮"), "{:?}", rows[0]);
        // too narrow for the path: it goes first
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].starts_with("╭─ bise :* ─"), "{:?}", rows[0]);
        // under 60 columns: no frame, the header row with margins of 1
        let rows = draw(&mut app, 59, 20);
        let summary = "bench · ∿ 3 · … 1 · ? 1 · ✓ 1";
        assert_eq!(rows[0], format!(" bise :*{}{}", " ".repeat(59 - 8 - summary.chars().count() - 1), summary));
        // no agents: the words
        let mut app = with_main();
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :*") && rows[0].ends_with("no agents yet ─╮"), "{:?}", rows[0]);
        let rows = draw(&mut app, 59, 20);
        assert!(rows[0].ends_with("no agents yet"), "{:?}", rows[0]);
    }

    /// QA F: only idle agents, no count to show: the path ends the header,
    /// with no lone ` · ` after it.
    #[test]
    fn idle_agents_leave_no_lone_separator_in_the_header() {
        let mut app = busy();
        for a in app.sb.agents.iter_mut().filter(|a| !a.main) {
            a.status = "idle".into();
        }
        app.sb.cards.clear();
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].ends_with(" bench ─╮"), "{:?}", rows[0]);
        let rows = draw(&mut app, 59, 20);
        assert!(!rows[0].trim_end().ends_with('·'), "{:?}", rows[0]);
    }

    /// BISE-126: viewing a task, its role line follows the title, dim;
    /// the path goes before the line is cut under ROLE_KEEP; too narrow,
    /// it is cut, then it goes; main's view shows none.
    #[test]
    fn the_viewed_task_s_role_line_follows_the_title() {
        let mut app = busy();
        for a in app.sb.agents.iter_mut().filter(|a| a.name == "auth-fix") {
            a.role = "fixing the safari login redirect".into();
        }
        let rows = draw(&mut app, 120, 20);
        assert!(!rows[0].contains("safari"), "main's view: {:?}", rows[0]);
        app.sb.focus = "auth-fix".into();
        let rows = draw(&mut app, 120, 20);
        let full = "∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done";
        assert!(rows[0].starts_with("╭─ bise :* · fixing the safari login redirect ─"), "{:?}", rows[0]);
        assert!(rows[0].ends_with(&format!(" {} ─╮", full)), "the path went first: {:?}", rows[0]);
        assert_eq!(rows[0].chars().count(), 120);
        // dim, like the summary
        let line = app.sb.header(200, false, &super::still_gust());
        let role = line.spans.iter().find(|s| s.content.contains("safari")).unwrap();
        assert_eq!(role.style.fg, Some(crate::theme::dim()));
        // wide: the path comes back
        let rows = draw(&mut app, 160, 20);
        assert!(rows[0].contains("redirect ─") && rows[0].contains("bench · ∿ 3 working"), "{:?}", rows[0]);
        // narrow: the counts shorten, the line is cut with …
        let rows = draw(&mut app, 60, 20);
        assert!(rows[0].starts_with("╭─ bise :* · fixing"), "{:?}", rows[0]);
        assert!(rows[0].contains('…') && rows[0].ends_with("∿ 3 · … 1 · ? 1 · ✓ 1 ─╮"), "{:?}", rows[0]);
        assert_eq!(rows[0].chars().count(), 60);
        // no frame: the header row, the same order
        let rows = draw(&mut app, 59, 20);
        assert!(rows[0].starts_with(" bise :* · fixing"), "{:?}", rows[0]);
        assert!(rows[0].ends_with("∿ 3 · … 1 · ? 1 · ✓ 1"), "{:?}", rows[0]);
        // no room left: no line at all, never a lone "·"
        let rows = draw(&mut app, 44, 20);
        assert!(rows[0].starts_with(" bise :* · fixing t…  ∿ 3"), "at least 12 columns: {:?}", rows[0]);
        let rows = draw(&mut app, 40, 20);
        assert!(!rows[0].contains("fix") && !rows[0].contains(" · f"), "{:?}", rows[0]);
        // a task without a line yet (an old hub): nothing
        for a in app.sb.agents.iter_mut() {
            a.role.clear();
        }
        let rows = draw(&mut app, 120, 20);
        assert!(rows[0].starts_with("╭─ bise :* ─"), "{:?}", rows[0]);
    }

    /// The summary drops the path first, then the words.
    #[test]
    fn summary_drops_the_path_first() {
        let app = busy();
        let sb = &app.sb;
        let text = |room: usize, short: bool| sb.summary(room, short, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
        let full = "∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done";
        assert_eq!(text(100, false), format!("bench · {}", full));
        assert_eq!(text(full.chars().count() + 7, false), full);
        assert_eq!(text(full.chars().count() - 1, false), "∿ 3 · … 1 · ? 1 · ✓ 1");
    }

    /// Colors of the header: `:*` and "needs you" in accent, the rest dim.
    #[test]
    fn header_colors() {
        let app = busy();
        let line = app.sb.header(120, false, &super::still_gust());
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
        assert_eq!(FIRST_RUN[2], "try: \"show me what you can do\"");
        // the composer pane (book §8 "The frame"): the divider says who
        // you talk to and what it does
        let at = rows.iter().position(|r| r.starts_with("├─ you → main ─")).unwrap_or_else(|| panic!("{}", all));
        assert!(rows[at].ends_with(" idle ─┤"), "{:?}", rows[at]);
        assert!(rows[at].contains('┴'), "the panel's rule joins it: {:?}", rows[at]);
        // then the raised pane (book §13): the composer, its bar at x0
        // (column 3 here) on a blank row, the text row, a blank row
        // (BISE-219), then the key bar from the text's column, the
        // frame's bottom edge
        assert_eq!(rows.len() - at, 6, "6 rows at rest: {}", all);
        assert!(rows[at + 1..at + 4].iter().all(|r| r.starts_with("│  │")), "{}", all);
        assert_eq!(rows[at + 1].trim_end_matches(['│', ' ']), "", "{:?}", rows[at + 1]);
        assert_eq!(rows[at + 3].trim_end_matches(['│', ' ']), "", "{:?}", rows[at + 3]);
        // empty, the composer asks; the text at x0 + 4 (BISE-XPAD)
        assert!(rows[at + 2].starts_with(&format!("│  │     {}", PLACEHOLDER_MAIN)), "{:?}", rows[at + 2]);
        let keys = &rows[rows.len() - 2];
        assert!(keys.starts_with("│      ⏎ send   @ file"), "{:?}", keys);
        assert!(rows.last().unwrap().starts_with("╰─"), "{}", all);
        // a panel with main only
        assert!(rows.iter().any(|r| r.contains(&format!("│  0 {} main {}", G_IDLE, G_MAIN))), "{}", all);
        // the block sits at 2/5 of the history's free rows (designer):
        // blank rows above it, more below it; the text stays left, at
        // the feed's indent
        let big = draw(&mut app, 120, 48);
        let at48 = big.iter().position(|r| r.starts_with("├─ you → main ─")).unwrap();
        let top = big.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        let bottom = big.iter().position(|r| r.contains(FIRST_RUN[2])).unwrap();
        let (above, below) = (top - 1, at48 - bottom - 1);
        assert!(above >= 8 && below > above && below - above <= above, "{above} above, {below} below:\n{}", big.join("\n"));
        let t = rows.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        assert_eq!(big[top].find(FIRST_RUN[0]), rows[t].find(FIRST_RUN[0]), "same column");
        assert!(t > 4, "centered at 24 rows too: {}", all);
        // a short history (under 12 rows): on top, after one blank row
        let small = draw(&mut app, 120, 16);
        let t = small.iter().position(|r| r.contains(FIRST_RUN[0])).unwrap();
        assert!(t <= 3 && small[t - 1].chars().take(60).all(|c| c == '│' || c == ' '), "{}", small.join("\n"));
        // once there is an agent, the first-run text goes
        bench::add_agent(&mut app, "auth-fix", "the safari login");
        let rows = draw(&mut app, 120, 24);
        assert!(!rows.iter().any(|r| r.contains(FIRST_RUN[1])));
    }

    /// The styled cells of a draw (the underline, the colors).
    fn cells(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::super::draw_sb(app, f)).unwrap();
        term.backend().buffer().clone()
    }

    fn mouse(app: &mut App, kind: crossterm::event::MouseEventKind, column: u16, row: u16) {
        let m = crossterm::event::MouseEvent { kind, column, row, modifiers: crossterm::event::KeyModifiers::NONE };
        crate::input::on_mouse(app, &m, 0);
    }

    fn type_key(app: &mut App, code: crossterm::event::KeyCode) {
        crate::input::on_key(app, &crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE));
    }

    /// BISE-284: the first-run text's `show me what you can do` is a
    /// link: the hand and an accent underline under the mouse; a click
    /// fills the composer with it, selected, and sends nothing; `try:`
    /// is no link.
    #[test]
    fn a_click_on_the_suggestion_fills_the_composer() {
        use crossterm::event::{MouseButton, MouseEventKind};
        use crate::pointer::{at, Shape};
        let mut app = with_main();
        assert_eq!(FIRST_RUN[2], format!("try: \"{DEMO}\""));
        let rows = draw(&mut app, 120, 24);
        let y = rows.iter().position(|r| r.contains(FIRST_RUN[2])).unwrap() as u16;
        let row = &rows[y as usize];
        let x = row[..row.find(DEMO).unwrap()].chars().count() as u16;
        let r = app.demo_rect.expect("the suggestion's place");
        assert_eq!((r.x, r.y, r.width, r.height), (x, y, DEMO.len() as u16, 1));
        assert_eq!(at(x, y), Shape::Pointer);
        assert_eq!(at(x + DEMO.len() as u16 - 1, y), Shape::Pointer);
        assert_eq!(at(x - 3, y), Shape::Default, "`try:` is no link");
        assert_eq!(at(x + DEMO.len() as u16, y), Shape::Default, "the closing quote is no link");
        // at rest: dim, no underline; under the mouse: accent, underlined
        let b = cells(&mut app, 120, 24);
        assert_eq!(b[(x, y)].fg, crate::theme::dim());
        assert!(!b[(x, y)].modifier.contains(Modifier::UNDERLINED));
        mouse(&mut app, MouseEventKind::Moved, x + 2, y);
        let b = cells(&mut app, 120, 24);
        assert_eq!(b[(x, y)].fg, crate::theme::accent());
        assert!(b[(x, y)].modifier.contains(Modifier::UNDERLINED));
        assert!(!b[(x - 3, y)].modifier.contains(Modifier::UNDERLINED), "only the sentence");
        // a click on `try:`: nothing
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x - 4, y);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x - 4, y);
        assert_eq!(app.ed.text, "");
        // a click on the sentence: the composer holds it, all selected
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 5, y);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x + 5, y);
        assert_eq!(app.ed.text, DEMO);
        assert_eq!(app.ed.selection(), Some((0, DEMO.chars().count())));
        assert!(!app.events.iter().any(|e| matches!(e, crate::Ev::You(..))), "not sent");
        // the last line now says what enter does
        let rows = draw(&mut app, 120, 24);
        let ready = format!("{}{}", DEMO_READY.0, DEMO_READY.1);
        assert_eq!(ready, "⏎ try it · or just type your own");
        assert!(rows.iter().any(|r| r.contains(&ready)), "{}", rows.join("\n"));
        assert!(!rows.iter().any(|r| r.contains(FIRST_RUN[2])));
        assert!(app.demo_rect.is_none(), "no link while it says ⏎ try it");
    }

    /// BISE-284: the first open after the onboarding: the composer holds
    /// the suggestion, selected, and the first-run text says `⏎ try it`;
    /// a typed key replaces it and the text says `try:` again; esc keeps
    /// it (unselected). Not in an agent, not over a draft, not once
    /// there are agents.
    #[test]
    fn the_first_open_after_the_onboarding_holds_the_suggestion() {
        use crossterm::event::KeyCode;
        let mut app = with_main();
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, DEMO);
        assert_eq!(app.ed.selection(), Some((0, DEMO.chars().count())));
        let rows = draw(&mut app, 120, 24);
        let all = rows.join("\n");
        let at = rows.iter().position(|r| r.contains("⏎ try it · or just type your own")).unwrap_or_else(|| panic!("{}", all));
        assert!(rows[..at].iter().any(|r| r.contains(FIRST_RUN[0])), "{}", all);
        assert!(rows.iter().any(|r| r.starts_with("│  │") && r.contains(DEMO)), "the composer holds it:\n{}", all);
        let b = cells(&mut app, 120, 24);
        let x = rows[at].find('⏎').map(|i| rows[at][..i].chars().count() as u16).unwrap();
        assert_eq!(b[(x, at as u16)].fg, crate::theme::accent(), "the key in the accent");
        assert_eq!(b[(x + 3, at as u16)].fg, crate::theme::dim());
        // esc: kept, not selected
        type_key(&mut app, KeyCode::Esc);
        assert_eq!((app.ed.text.as_str(), app.ed.selection()), (DEMO, None));
        // selected again, a key replaces it: back to `try:`
        fill_demo(&mut app);
        type_key(&mut app, KeyCode::Char('h'));
        assert_eq!(app.ed.text, "h");
        let rows = draw(&mut app, 120, 24);
        assert!(rows.iter().any(|r| r.contains(FIRST_RUN[2])), "{}", rows.join("\n"));
        // a draft stays as it is
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "h");
        // not inside an agent, not once there are agents
        let mut app = with_main();
        bench::add_agent(&mut app, "auth-fix", "the safari login");
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "");
        app.sb.focus = "auth-fix".into();
        prefill_demo(&mut app);
        assert_eq!(app.ed.text, "");
    }

    /// Inside an agent (mockup "inside an agent"): the pinned line of the
    /// copy deck on top of the feed, the status row starts with the
    /// agent's name in accent, then dim; the shared checkout shows no
    /// place (BISE-136).
    #[test]
    fn inside_an_agent_screen() {
        let mut app = with_main();
        app.sb.agents.push(Agent {
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
        assert!(rows[y].ends_with(" idle ─┤"), "{:?}", rows[y]);
        let cell = |x: usize| buf.cell((x as u16, y as u16)).unwrap().fg;
        assert_eq!(cell(3), dim(), "you → dim");
        assert_eq!(cell(9), accent(), "the name in accent");
        let idle = rows[y].chars().count() - " idle ─┤".chars().count() + 1;
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
        assert_eq!(key_mode(&app), crate::keybar::Mode::Default);
        app.pending = true;
        assert_eq!(key_mode(&app), crate::keybar::Mode::Steer);
        app.pending = false;
        let sb = &mut app.sb;
        sb.selected = Some(1);
        sb.preview = true;
        let text = status_text(&app);
        assert!(text.ends_with("preview of auth-fix"), "{:?}", text);
    }

    /// QA N: viewing a working agent, the label says `working · 12m`; the
    /// right side does not say `working · 0s` again. Another state keeps
    /// its word and the turn's age.
    #[test]
    fn a_working_agent_has_one_status_and_one_timer() {
        let mut app = busy();
        app.sb.agents.retain(|a| a.name != "auth-fix");
        app.sb.agents.push(Agent {
            name: "auth-fix".into(),
            status: "working".into(),
            turn_ms: Some(12 * 60_000),
            ..Agent::default()
        });
        app.sb.focus = "auth-fix".into();
        app.pending = true;
        assert_eq!(viewed_working(&app).and_then(|w| w.age).as_deref(), Some("12m"));
        let state: String = status_state(&app).unwrap().spans.iter().map(|s| s.content.to_string()).collect();
        assert!(!state.contains("working") && !state.contains("12m") && !state.contains("0s"), "{state:?}");
        for a in app.sb.agents.iter_mut().filter(|a| a.name == "auth-fix") {
            a.status = "blocked".into();
        }
        let state: String = status_state(&app).unwrap().spans.iter().map(|s| s.content.to_string()).collect();
        assert!(state.starts_with("blocked · 12m"), "{state:?}");
    }
}

#[cfg(test)]
mod cards_tests {
    //! BISE-125: the open cards in the panel, under the agents.
    use super::super::bench;
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, Terminal};

    fn card(id: u64, kind: &str, agent: &str, text: &str) -> Card {
        Card { id, kind: kind.into(), agent: agent.into(), text: text.into(), ..Card::default() }
    }

    /// main, two agents, three open cards (ids 12, 153, 40).
    fn app() -> App {
        let mut app = bench::test_app_drained();
        let sb = &mut app.sb;
        sb.agents.push(Agent { name: "main".into(), main: true, status: "idle".into(), ..Agent::default() });
        sb.agents.push(Agent { name: "debt-solo".into(), status: "done".into(), ..Agent::default() });
        sb.agents.push(Agent { name: "docs".into(), status: "working".into(), ..Agent::default() });
        sb.cards.push(card(12, "question", "docs", "\nv1 or v2?\n1. v1\n2. v2"));
        sb.cards.push(card(153, "done", "debt-solo", "the debt list is cleared, 14 items closed and two left for later"));
        sb.cards.push(card(40, "blocked", "docs", "no access to the wiki"));
        app
    }

    fn rows(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_panel(app, f, f.area())).unwrap();
        let buf = term.backend().buffer();
        buf.content
            .chunks(w as usize)
            .map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
            .collect()
    }

    /// Under the agents: a blank row, `inbox · ctrl+g`, then one row per
    /// card, newest first, `#N glyph agent  first line`; the text cut to
    /// the panel with `…`, never past its width.
    #[test]
    fn the_open_cards_list_under_the_agents() {
        let app = app();
        let t = rows(&app, 40, 14);
        let at = |s: &str| t.iter().position(|r| r.contains(s)).unwrap_or_else(|| panic!("{s} missing:\n{}", t.join("\n")));
        assert!(at("docs") < at(" inbox · ctrl+g"), "{}", t.join("\n"));
        assert_eq!(t[at(" inbox · ctrl+g") - 1], "");
        let first = at(" inbox · ctrl+g") + 1;
        assert_eq!(t[first], format!(" #153 {} debt-solo  the debt list is cl…", crate::theme::done_glyph()));
        assert_eq!(t[first + 1], format!(" #40 {} docs  no access to the wiki", G_NEEDS_YOU));
        // the first line of the text, not the blank one before it
        assert_eq!(t[first + 2], format!(" #12 {} docs  v1 or v2?", G_NEEDS_YOU));
        // 28 columns: still one row each, cut at the width
        for w in [28u16, 24] {
            let t = rows(&app, w, 14);
            let r = t.iter().find(|r| r.contains("#153")).unwrap();
            assert!(r.chars().count() < w as usize, "{r:?} at {w}");
            assert!(r.contains(if w == 28 { "debt-solo" } else { "debt-so…" }) && r.ends_with('…'), "{r:?} at {w}");
        }
        // a long agent name is cut too, a few columns of text kept
        let mut app = app;
        app.sb.cards.push(card(200, "question", "a-very-long-agent-name-indeed", "which one?"));
        let t = rows(&app, 28, 14);
        let r = t.iter().find(|r| r.contains("#200")).unwrap();
        assert!(r.contains("…  wh"), "{r:?}");
        assert!(r.chars().count() < 28);
        // colors: the number dim, the glyph in its kind's color, the text dim
        let mut term = Terminal::new(TestBackend::new(40, 14)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let t = rows(&app, 40, 14);
        let y = t.iter().position(|r| r.contains("#40")).unwrap() as u16;
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((1, y)).unwrap().fg, dim());
        assert_eq!(buf.cell((5, y)).unwrap().fg, accent(), "blocked: needs you, accent");
        assert_eq!(buf.cell((7, y)).unwrap().fg, text());
        assert_eq!(buf.cell((13, y)).unwrap().fg, dim());
        // no card: no section
        app.sb.cards.clear();
        assert!(!rows(&app, 40, 14).iter().any(|r| r.contains("cards")));
    }

    /// Many cards: the panel ends with `+ n more`; the card in the box
    /// is on the selection color and scrolled into view.
    #[test]
    fn many_cards_end_with_more_and_the_shown_one_stays_in_view() {
        let mut app = app();
        for i in 0..30 {
            app.sb.cards.push(card(1000 + i, "done", "debt-solo", &format!("report {i}")));
        }
        let t = rows(&app, 28, 14);
        assert!(t[13].starts_with(" + ") && t[13].ends_with(" more"), "{}", t.join("\n"));
        // 33 cards; title, blank, 3 agents, blank, cards title, 6 cards: 27 below
        assert_eq!(t[13], " + 27 more", "{}", t.join("\n"));
        assert!(t.iter().any(|r| r.contains("#1029")), "newest first");
        // the oldest card shown in the box: the panel scrolls to it
        super::super::cards::open_view(&mut app, Some(12));
        let t = rows(&app, 28, 14);
        assert!(t.iter().any(|r| r.contains("#12 ")), "{}", t.join("\n"));
        let mut term = Terminal::new(TestBackend::new(28, 14)).unwrap();
        term.draw(|f| draw_panel(&app, f, f.area())).unwrap();
        let y = t.iter().position(|r| r.contains("#12 ")).unwrap() as u16;
        assert_eq!(term.backend().buffer().cell((20, y)).unwrap().bg, selection_bg());
    }

    fn click(app: &mut App, column: u16, row: u16) -> bool {
        let m = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE };
        panel_mouse(app, &m)
    }

    /// A click on a card row opens the card view on it (cards v2); on
    /// another card the view follows; the section title toggles the view
    /// like ctrl+g.
    #[test]
    fn a_click_on_a_card_opens_it() {
        let mut app = app();
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let mut draw = |app: &mut App| {
            term.draw(|f| super::super::draw_sb(app, f)).unwrap();
            let buf = term.backend().buffer();
            buf.content.chunks(120).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>()
        };
        let screen = draw(&mut app);
        let x = app.sb.panel_hits.borrow().area.x;
        let y_of = |s: &[String], l: &str| s.iter().position(|r| r.chars().skip(x as usize).collect::<String>().contains(l)).unwrap() as u16;
        assert!(!app.sb.card.open);
        assert!(click(&mut app, x + 3, y_of(&screen, "#40")));
        assert!(app.sb.card.open);
        assert_eq!(app.sb.current_card().map(|c| c.id), Some(40));
        let screen = draw(&mut app);
        assert!(screen.iter().any(|r| r.contains("docs is blocked")), "the view shows #40:\n{}", screen.join("\n"));
        // another card: the view follows
        click(&mut app, x + 3, y_of(&screen, "#153"));
        assert_eq!((app.sb.card.open, app.sb.current_card().map(|c| c.id)), (true, Some(153)));
        // the section title: back to the thread, then the view again
        let screen = draw(&mut app);
        click(&mut app, x + 3, y_of(&screen, " inbox · ctrl+g"));
        assert!(!app.sb.card.open);
        let screen = draw(&mut app);
        click(&mut app, x + 3, y_of(&screen, " inbox · ctrl+g"));
        assert!(app.sb.card.open);
        assert_eq!(app.sb.focus, "main", "a card click does not change the view");
    }

    /// The header counts the open cards, last (`# 3 in the inbox`), and keeps
    /// them when the panel is hidden (`# 3`), right after needs you.
    #[test]
    fn the_header_counts_the_cards() {
        let app = app();
        let text = |room: usize, short: bool| app.sb.summary(room, short, &super::still_gust()).iter().map(|s| s.content.to_string()).collect::<String>();
        let t = text(200, false);
        assert!(t.ends_with(&format!("{} 1 done · # 3 in the inbox", crate::theme::done_glyph())), "{t:?}");
        assert!(text(200, true).ends_with("· # 3"), "{:?}", text(200, true));
        // short on room: needs you, then the cards, before the rest
        assert_eq!(text(14, true), format!("{} 1 · # 3", G_NEEDS_YOU));
        let one = [0, 0, 0, 0, 1];
        let s: String = fit_counts(one, false, 100, &[]).iter().map(|s| s.content.to_string()).collect();
        assert_eq!(s, "# 1 in the inbox");
    }
}
