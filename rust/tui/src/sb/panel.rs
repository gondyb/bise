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
    for (i, a) in nav.iter().enumerate() {
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
        let act = sb.activity.get(&a.name).copied().unwrap_or(0);
        let mut spans = vec![
            Span::styled(format!(" {} ", g), Style::default().fg(gc)),
            Span::styled(truncate_chars(&label, w.saturating_sub(12)), name_style),
            Span::styled(format!(" {}", a.status), Style::default().fg(DIM)),
        ];
        if let Some(u) = sb.usage_of(app, &a.name) {
            spans.push(Span::styled(format!(" {}", u.short()), Style::default().fg(FAINT)));
        }
        if act > 0 && !focused {
            spans.push(Span::styled(" •", Style::default().fg(INFO)));
        }
        if a.queued > 0 {
            spans.push(Span::styled(
                format!(" ✉{}", a.queued),
                Style::default().fg(WARN),
            ));
        }
        lines.push(Line::from(spans));
        if !a.main {
            let mut sub = truncate_chars(&a.objective, w.saturating_sub(3));
            if let Some(b) = &a.branch {
                sub = truncate_chars(&format!("⎇ {} · {}", b, a.objective), w.saturating_sub(3));
            }
            lines.push(Line::from(Span::styled(
                format!("   {}", sub),
                Style::default().fg(FAINT),
            )));
            if !a.note.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("   {}", truncate_chars(&a.note, w.saturating_sub(3))),
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )));
            }
        }
        if a.main && nav.len() > 1 {
            lines.push(Line::from(Span::styled(
                format!(" {}", "─".repeat(w.saturating_sub(1))),
                Style::default().fg(FAINT),
            )));
        }
    }
    let archived = sb.agents.iter().filter(|a| a.archived()).count();
    if archived > 0 {
        lines.push(Line::from(Span::styled(
            format!(
                " {} archived",
                archived
            ),
            Style::default().fg(FAINT),
        )));
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
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(FAINT)),
        ),
        area,
    );
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
    if sb.focus != "main" {
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
        "⏎ enter · Space preview · D drop · Ctrl+K/J select · Esc close"
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
    } else {
        format!("Direct message to @{}…", sb.focus)
    })
}
