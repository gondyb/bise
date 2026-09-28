//! The chrome of the switchboard mode: the task panel on the right,
//! the status row, the key hints and the composer placeholder.

use super::*;

pub(super) fn glyph(status: &str, tick: u32) -> (&'static str, Color) {
    match status {
        "working" => (spinner_frame(tick / 2), BRAND),
        "waiting" => ("◌", INFO),
        "starting" => ("…", DIM),
        "idle" => ("○", DIM),
        "done" => (GLYPH_OK, OK),
        "blocked" => (GLYPH_WARN, WARN),
        "failed" => (GLYPH_ERR, ERR),
        "stopped" => ("■", DIM),
        _ => ("·", FAINT),
    }
}

/// The workspace folder (the embedded terminal starts there).
pub(crate) fn workspace(app: &App) -> Option<String> {
    app.sb.as_ref().map(|sb| sb.workspace.clone()).filter(|w| !w.is_empty())
}

/// The feed and composer area, and the panel on the right when it fits.
pub(crate) fn split(app: &App, full: Rect) -> (Rect, Option<Rect>) {
    if app.sb.is_none() || full.width < 70 {
        return (full, None);
    }
    let w = (full.width / 4).clamp(28, 40);
    (
        Rect {
            width: full.width - w,
            ..full
        },
        Some(Rect {
            x: full.x + full.width - w,
            width: w,
            ..full
        }),
    )
}

pub(crate) fn draw_panel(app: &App, frame: &mut Frame, area: Rect) {
    let Some(sb) = app.sb.as_ref() else { return };
    let w = area.width.saturating_sub(3) as usize;
    let mut lines: Vec<Line> = Vec::new();
    let ws = std::path::Path::new(&sb.workspace)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled(
            " Switchboard ",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            truncate_chars(&ws, w.saturating_sub(13)),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(""));
    let nav = sb.nav();
    let live = nav.iter().filter(|a| !a.archived()).count();
    let mut owners: Vec<(usize, Hit)> = Vec::new();
    // the first row of the selected entry (the panel scrolls to it)
    let mut sel_row = None;
    for (i, a) in nav.iter().take(live).enumerate() {
        let rows = agent_lines(app, sb, a, i, w);
        if sb.selected == Some(i) {
            sel_row = Some(lines.len());
        }
        owners.extend((lines.len()..lines.len() + rows.len()).map(|r| (r, Hit::Agent(a.name.clone()))));
        lines.extend(rows);
        if a.main && live > 1 {
            lines.push(Line::from(Span::styled(
                format!(" {}", "─".repeat(w.saturating_sub(1))),
                Style::default().fg(FAINT),
            )));
        }
    }
    if !sb.cards.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" ◆ cards ({}) · Ctrl+G", sb.cards.len()),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        )));
        for c in &sb.cards {
            lines.push(Line::from(vec![
                Span::styled(format!(" #{} ", c.id), Style::default().fg(WARN)),
                Span::styled(
                    truncate_chars(&format!("{} @{}", c.kind, c.agent), w.saturating_sub(5)),
                    Style::default().fg(TEXT),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                format!("   {}", truncate_chars(&c.text, w.saturating_sub(3))),
                Style::default().fg(DIM),
            )));
        }
    }
    archived_lines(sb, live, w, &mut lines, &mut owners, &mut sel_row);
    // keep the selected entry in view (the archived list can be long)
    let h = area.height as usize;
    let top = sel_row.map_or(0, |r| (r + 3).saturating_sub(h));
    if let Ok(mut hits) = sb.panel_hits.try_borrow_mut() {
        *hits = PanelHits {
            area,
            rows: owners
                .into_iter()
                .filter(|(r, _)| *r >= top && *r - top < h)
                .map(|(r, hit)| (area.y.saturating_add((r - top) as u16), hit))
                .collect(),
        };
    }
    frame.render_widget(
        Paragraph::new(lines).scroll((top.min(u16::MAX as usize) as u16, 0)).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(FAINT)),
        ),
        area,
    );
}

/// The archived section, under the cards: a dim header `▸ N archived`
/// (`▾` expanded), then, expanded, one row per task, newest first: name
/// and how long ago it was last heard of; the selected or focused one
/// also shows its last report (or its objective). Collapsed, only the
/// archived task in focus is listed, so the view in focus is always
/// found in the panel.
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
    let (arrow, keys) = if sb.archived_open { ("▾", "") } else { ("▸", " · click or /archived") };
    lines.push(Line::from(vec![
        Span::styled(format!(" {} {} archived", arrow, all.len()), Style::default().fg(DIM)),
        Span::styled(truncate_chars(keys, w.saturating_sub(14)), Style::default().fg(FAINT)),
    ]));
    for (k, a) in all.iter().enumerate() {
        let focused = a.name == sb.focus;
        if !sb.archived_open && !focused {
            continue;
        }
        let selected = sb.archived_open && sb.selected == Some(live + k);
        if selected {
            *sel_row = Some(lines.len());
        }
        let age = a.report_ms.map(|t| cards::ago(now.saturating_sub(t))).unwrap_or_default();
        let mut name_style = Style::default().fg(if focused { BRAND } else { DIM });
        if selected {
            name_style = name_style.add_modifier(Modifier::REVERSED);
        }
        let first = lines.len();
        lines.push(Line::from(vec![
            Span::styled("   · ", Style::default().fg(FAINT)),
            Span::styled(truncate_chars(&a.name, w.saturating_sub(12)), name_style),
            Span::styled(format!(" {}", age), Style::default().fg(FAINT)),
        ]));
        if selected || focused {
            let what = if a.report.is_empty() { &a.objective } else { &a.report };
            lines.push(Line::from(Span::styled(
                format!("     {}", truncate_chars(what, w.saturating_sub(5))),
                Style::default().fg(DIM),
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

/// The status row in switchboard mode (None: the plain one).
pub(crate) fn status_line(app: &App) -> Option<Line<'static>> {
    let sb = app.sb.as_ref()?;
    let a = sb.agent(&sb.focus).cloned().unwrap_or_default();
    let mut spans: Vec<Span<'static>> = Vec::new();
    if !sb.version.is_empty() {
        spans.push(Span::styled(
            format!("  v {}", sb.version.chars().take(24).collect::<String>()),
            Style::default().fg(DIM),
        ));
    }
    for i in &sb.versions {
        if i.marks.iter().any(|m| m == "building") {
            spans.push(Span::styled(format!("  ⧗ building {}", i.rev), Style::default().fg(WARN)));
        }
        if i.marks.iter().any(|m| m == "trial") {
            spans.push(Span::styled(format!("  ⧗ {} on trial", i.rev), Style::default().fg(WARN)));
        }
    }
    if app.pending {
        spans.push(Span::styled(
            format!("  {} ", spinner_frame(app.tick / 2)),
            Style::default().fg(BRAND),
        ));
    } else {
        spans.push(Span::styled("  ● ", Style::default().fg(BRAND)));
    }
    if sb.focus == "main" {
        spans.push(Span::styled(
            "main".to_string(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            format!("@{}", sb.focus),
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        format!(" · {}", a.status),
        Style::default().fg(DIM),
    ));
    if let Some(b) = &a.branch {
        spans.push(Span::styled(
            format!(" · ⎇ {}", b),
            Style::default().fg(DIM),
        ));
    } else if !a.main && !a.path.is_empty() && a.mode == "shared" {
        spans.push(Span::styled(
            " · shared folder".to_string(),
            Style::default().fg(DIM),
        ));
    }
    if let Some(u) = crate::usage::current(&app.events) {
        spans.push(Span::styled(format!(" · {}", u.label()), Style::default().fg(DIM)));
    }
    if let Some(ms) = a.turn_ms.filter(|_| app.pending) {
        spans.push(Span::styled(
            format!(" · {}s", ms / 1000),
            Style::default().fg(DIM),
        ));
    }
    if a.archived() {
        spans.push(Span::styled(
            " · read-only history · /restore brings it back · Esc → main".to_string(),
            Style::default().fg(DIM),
        ));
    } else if sb.focus != "main" {
        spans.push(Span::styled(
            " · you talk to the task directly · Esc → main".to_string(),
            Style::default().fg(INFO),
        ));
    }
    if sb.preview {
        if let Some(sel) = sb
            .selected_agent()
            .map(|a| a.name.clone())
        {
            spans.push(Span::styled(
                format!(" · preview of @{} (⏎ enter, Esc close)", sel),
                Style::default().fg(WARN),
            ));
        }
    }
    if !sb.cards.is_empty() && !sb.card.shown {
        spans.push(Span::styled(
            format!(
                " · ◆ {} card{} · Ctrl+G",
                sb.cards.len(),
                if sb.cards.len() > 1 { "s" } else { "" }
            ),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ));
    }
    if !app.connected {
        spans.push(Span::styled(
            " · ○ hub disconnected".to_string(),
            Style::default().fg(ERR),
        ));
    }
    Some(Line::from(spans))
}

pub(crate) fn hint(app: &App) -> Option<&'static str> {
    let sb = app.sb.as_ref()?;
    Some(if sb.confirm.is_some() {
        "y yes · n no · Esc cancel"
    } else if sb.card.full {
        "Alt+R answer · PgUp/PgDn scroll · Ctrl+N/P card · Ctrl+X close · Ctrl+F/Esc shrink · Ctrl+G hide"
    } else if sb.card.shown {
        "Alt+R answer (⏎ still goes to main) · PgUp/PgDn scroll · Ctrl+N/P card · Ctrl+F full screen · Ctrl+X close · Ctrl+G hide"
    } else if sb.selected.is_some() {
        "⏎ enter · Space preview · D drop · A archived · Ctrl+K/J select · Esc close"
    } else if sb.focus_archived() {
        "archived: read-only · /restore brings it back · Esc back to main · Ctrl+K/J tasks · /help"
    } else if app.pending {
        "⏎ steer · Ctrl+C interrupt · Ctrl+K/J tasks · Alt+N° task N · Esc main · /help"
    } else if sb.focus != "main" {
        "⏎ send to the task · @main … for main · Esc back to main · Ctrl+K/J tasks · Alt+N° task N · /help"
    } else {
        "⏎ send to main · @task … direct · Ctrl+K/J tasks · Alt+N° task N · Ctrl+G card · /help"
    })
}

pub(crate) fn placeholder(app: &App) -> Option<String> {
    let sb = app.sb.as_ref()?;
    Some(if sb.focus == "main" {
        "Message to main…".to_string()
    } else if sb.focus_archived() {
        format!("@{} is archived (read-only) · /restore brings it back", sb.focus)
    } else {
        format!("Direct message to @{}…", sb.focus)
    })
}

/// The rows of agent `a`, entry `i` of the panel, `w` columns wide: its
/// glyph, name, status, context use, unseen lines and queue, then (a
/// task) its branch and objective and its note.
fn agent_lines(app: &App, sb: &Sb, a: &Agent, i: usize, w: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let (g, gc) = glyph(&a.status, app.tick);
    let focused = a.name == sb.focus;
    let selected = sb.selected == Some(i);
    let mut name_style = Style::default().fg(if focused { BRAND } else { TEXT });
    if focused {
        name_style = name_style.add_modifier(Modifier::BOLD);
    }
    if selected {
        name_style = name_style.add_modifier(Modifier::REVERSED);
    }
    let label = if a.main {
        "main".to_string()
    } else {
        format!("{} {}", i, a.name)
    };
    let mut spans = vec![
        Span::styled(format!(" {} ", g), Style::default().fg(gc)),
        Span::styled(truncate_chars(&label, w.saturating_sub(12)), name_style),
        Span::styled(format!(" {}", a.status), Style::default().fg(DIM)),
    ];
    if let Some(u) = sb.usage_of(app, &a.name) {
        spans.push(Span::styled(format!(" {}", u.short()), Style::default().fg(FAINT)));
    }
    if sb.activity.contains(&a.name) && !focused {
        spans.push(Span::styled(" •", Style::default().fg(INFO)));
    }
    if a.queued > 0 {
        spans.push(Span::styled(
            format!(" ✉{}", a.queued),
            Style::default().fg(WARN),
        ));
    }
    out.push(Line::from(spans));
    if !a.main {
        let mut sub = truncate_chars(&a.objective, w.saturating_sub(3));
        if let Some(b) = &a.branch {
            sub = truncate_chars(&format!("⎇ {} · {}", b, a.objective), w.saturating_sub(3));
        }
        out.push(Line::from(Span::styled(
            format!("   {}", sub),
            Style::default().fg(FAINT),
        )));
        if !a.note.is_empty() {
            out.push(Line::from(Span::styled(
                format!("   {}", truncate_chars(&a.note, w.saturating_sub(3))),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )));
        }
    }
    out
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

    /// A click on an agent's row (its name or its objective) focuses it,
    /// like Alt+N; a click on a blank panel row changes nothing.
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
        for (label, name) in [("1 alpha", "alpha"), ("third objective", "gamma"), ("2 beta", "beta")] {
            let (x, y) = find(&screen(&term), panel_x, label);
            click(&mut app, x, y);
            assert_eq!(app.sb.as_ref().unwrap().focus, name, "click on {:?}", label);
            draw(&mut app, &mut term);
        }
        let (x, y) = find(&screen(&term), panel_x, "main");
        click(&mut app, x, y);
        assert_eq!(app.sb.as_ref().unwrap().focus, "main");
        // the blank row under the title: nothing happens
        draw(&mut app, &mut term);
        click(&mut app, panel_x + 3, 1);
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
        for n in ["· old", "· mid", "· new"] {
            assert!(row_of(&p, n).is_none(), "{} shown while folded", n);
        }
        let buf = term.backend().buffer().clone();
        let cell = buf.cell((x + 3, head as u16)).unwrap();
        assert_eq!(cell.fg, DIM, "the header is dim");

        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▾ 3 archived").is_some(), "{}", p.join("\n"));
        let (n, m, o) = (
            row_of(&p, "· new").unwrap(),
            row_of(&p, "· mid").unwrap(),
            row_of(&p, "· old").unwrap(),
        );
        assert!(n < m && m < o, "newest first:\n{}", p.join("\n"));
        assert!(p[n].contains("10 min") && p[o].contains("5 h"), "{}", p.join("\n"));
        assert!(row_of(&p, "did its job").is_none(), "no report line when not selected");
        let buf = term.backend().buffer().clone();
        let name_x = x + p[m].find("mid").map(|b| p[m][..b].chars().count()).unwrap() as u16;
        assert_eq!(buf.cell((name_x, m as u16)).unwrap().fg, DIM, "archived names are dim");

        click(&mut app, x + 5, m as u16);
        assert_eq!(app.sb.as_ref().unwrap().focus, "mid");
        let rows = draw(&mut app, &mut term);
        let all = rows.join("\n");
        assert!(all.contains("read-only history"), "{}", all);
        assert!(panel(&rows, x).iter().any(|r| r.contains("mid did its job")), "{}", all);
        assert_eq!(
            placeholder(&app).unwrap(),
            "@mid is archived (read-only) · /restore brings it back"
        );
        let out = handle_input(&mut app, "hello");
        assert!(matches!(&out[..], [Ev::Warn(w)] if w.contains("/restore")), "not sent");

        let p = panel(&rows, x);
        let head = row_of(&p, "▾ 3 archived").unwrap();
        click(&mut app, x + 3, head as u16);
        let p = panel(&draw(&mut app, &mut term), x);
        assert!(row_of(&p, "▸ 3 archived").is_some());
        assert!(row_of(&p, "· mid").is_some(), "the focused archived task stays listed");
        assert!(row_of(&p, "· new").is_none());
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
        let y = row_of(&p, "· t000").unwrap_or_else(|| panic!("{}", p.join("\n")));
        click(&mut app, x + 5, y as u16);
        assert_eq!(app.sb.as_ref().unwrap().focus, "t000");
    }
}
