//! Feed events as styled lines: messages, reasoning sections, notices
//! and the OpenCode inline tool lines.

use crate::code::*;
use crate::markdown::*;
use crate::theme::*;
use crate::wire::*;
use crate::wrap_line;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

// ---- tool-call rendering helpers ----

pub(crate) fn fmt_elapsed(started: std::time::Instant) -> String {
    let s = started.elapsed().as_secs_f64();
    if s < 10.0 {
        format!("{:.1}s", s)
    } else if s < 60.0 {
        format!("{:.0}s", s)
    } else {
        format!("{:.0}m{:02.0}s", (s / 60.0).floor(), s % 60.0)
    }
}

pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{}…", head)
    }
}

// naive "field":"value" extractor for JSON-ish args (no parser needed:
// the runtime caps the payload and the shape is known)
pub(crate) fn json_str_field(s: &str, field: &str) -> Option<String> {
    let pat = format!("\"{}\"", field);
    let i = s.find(&pat)?;
    let rest = s[i + pat.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut ch = rest.chars();
    while let Some(c) = ch.next() {
        match c {
            '\\' => match ch.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                // truncated payload: keep the partial value
                None => out.push('\\'),
            },
            '"' => return Some(out),
            _ => out.push(c),
        }
    }
    // the runtime caps the wire payload: a cut string is still useful
    Some(out)
}

// the args preview: what the engineer reads at a glance
//   run_typescript        the first line of main() (the signature)
//   search_tool_functions mode and the query
//   bash / mcp            the raw args
// the first non-empty line of a source, "…" when more lines follow
pub(crate) fn first_line_preview(code: &str) -> String {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut p = truncate_chars(first, 64);
    if code.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
        p.push_str(" …");
    }
    p
}

pub(crate) fn args_preview(name: &str, args: &str) -> String {
    if name == "run_typescript" {
        if let Some(code) = json_str_field(args, "code") {
            return first_line_preview(&code);
        }
    }
    if name == "search_tool_functions" {
        let mode = json_str_field(args, "mode").unwrap_or_else(|| "best_match".into());
        if let Some(q) = json_str_field(args, "query") {
            return format!("{} \"{}\"", mode, truncate_chars(&q, 48));
        }
        return mode;
    }
    truncate_chars(args.trim(), 80)
}

// one event renders as one or many lines (markdown expands messages).
// The shape follows the OpenCode message parts: user messages are blocks
// with a colored left bar and panel background; assistant text is
// markdown in the OpenCode colors; tools are inline tools.
pub(crate) fn ev_lines_t(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    // the feed twin: identical shape, but the tool spinner animates
    match ev {
        Ev::Tool(td) => tool_lines(td, tick, width),
        other => ev_lines(other, width),
    }
}

// a thinking section: collapsed it is one dim glyph + duration;
// expanded (ctrl+t, or a click) the reasoning shows under a faint rail
pub(crate) fn thinking_lines(ms: u128, text: &str, open: bool) -> Vec<Line<'static>> {
    let head = Line::from(vec![
        Span::styled(format!(" {} ", GLYPH_THINK), Style::default().fg(DIM)),
        Span::styled(
            fmt_think_ms(ms),
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ),
    ]);
    if !open || text.trim().is_empty() {
        return vec![head];
    }
    let mut rows = vec![head];
    for l in unescape_md(text).split('\n') {
        // the BENDSIG line carries the provider signature (the signed
        // thinking transport), never part of the reasoning itself
        if l.starts_with("BENDSIG::") {
            continue;
        }
        rows.push(Line::from(vec![
            Span::styled(format!(" {} ", GLYPH_RAIL), Style::default().fg(FAINT)),
            Span::styled(
                l.to_string(),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ]));
    }
    rows
}

// 800ms -> "0.8s"; 4200ms -> "4.2s"; 12_300ms -> "12s"; 90_000 -> "1m30s"
pub(crate) fn fmt_think_ms(ms: u128) -> String {
    if ms == 0 {
        // no measured duration: a replayed (--resume) section, or lines
        // that arrived in the same batch
        "raisonnement".to_string()
    } else if ms < 10_000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

pub(crate) fn ev_lines(ev: &Ev, width: usize) -> Vec<Line<'static>> {
    match ev {
        // an image marker shows as `[Image #1 path]` (docs/images.md)
        Ev::You(t) => user_block_lines(&bend_images::display(t), width),
        Ev::Assistant(t) => md_to_lines(&unescape_md(t)),
        Ev::Thinking { ms, text, open } => thinking_lines(*ms, text, *open),
        Ev::Tool(td) => tool_lines(td, 0, width),
        Ev::Idle => vec![Line::from("")],
        Ev::Sub { name, ok, preview } => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_BRANCH), Style::default().fg(FAINT)),
            Span::styled(name.clone(), Style::default().fg(DIM)),
            Span::styled(
                if *ok {
                    format!(" {} ", GLYPH_OK)
                } else {
                    format!(" {} ", GLYPH_ERR)
                },
                Style::default().fg(if *ok { DIM } else { ERR }),
            ),
            Span::styled(
                truncate_chars(preview.trim(), 100),
                Style::default().fg(DIM),
            ),
        ])],
        Ev::Turn => vec![Line::from(vec![
            Span::styled(" ── tour ", Style::default().fg(FAINT)),
            Span::styled("─".repeat(24), Style::default().fg(FAINT)),
        ])],
        Ev::TurnDone => vec![Line::from(vec![
            Span::styled(" └─ ", Style::default().fg(FAINT)),
            Span::styled(GLYPH_OK, Style::default().fg(DIM)),
        ])],
        Ev::Compact(t) => vec![Line::from(vec![
            Span::styled(
                format!("  {} ", GLYPH_COMPACT),
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "compaction ",
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Compacted(t) => vec![Line::from(vec![
            Span::styled(
                format!("  {} ", GLYPH_SUMMARY),
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "summary ",
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                unescape_md(t),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::Warn(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_WARN), Style::default().fg(WARN)),
            Span::styled(t.clone(), Style::default().fg(WARN)),
        ])],
        Ev::Err(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_ERR), Style::default().fg(ERR)),
            Span::styled(t.clone(), Style::default().fg(ERR)),
        ])],
        Ev::Info(t) => vec![Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_INFO), Style::default().fg(FAINT)),
            Span::styled(
                bend_images::display(t),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            ),
        ])],
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } | Ev::ToolCode { .. } => vec![],
        Ev::Usage(u) => vec![Line::from(Span::styled(
            format!("  usage: {} (in {} · out {})", u.label(), u.input, u.output),
            Style::default().fg(DIM),
        ))],
        Ev::Raw(t) => vec![Line::from(Span::styled(
            format!("  {}", t),
            Style::default().fg(DIM),
        ))],
        Ev::AgentMsg { head, text } => {
            let mut rows = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" ◀ ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled(head.clone(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                ]),
            ];
            let bar = Span::styled(" │ ", Style::default().fg(ACCENT));
            rows.extend(barred_rows(&bar, md_to_lines(text), width));
            rows
        }
        Ev::Card(t) => vec![Line::from(vec![
            Span::styled("  ◆ ", Style::default().fg(WARN).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("carte {}", t),
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            ),
        ])],
    }
}

// the OpenCode user message block: colored left bar (┃ primary), panel
// background, one blank line above and below. Each line of the text is
// its own row (Shift+Enter, paste), wrapped under the bar.
pub(crate) fn user_block_lines(text: &str, width: usize) -> Vec<Line<'static>> {
    let mut rows = vec![Line::from("")];
    let style = Style::default().fg(TEXT).add_modifier(Modifier::BOLD);
    let lines = text
        .split('\n')
        .map(|l| Line::from(Span::styled(l.trim_end_matches('\r').to_string(), style)));
    let bar = Span::styled(" ┃ ", Style::default().fg(BRAND));
    rows.extend(barred_rows(&bar, lines, width));
    rows.push(Line::from(Span::styled("   ", Style::default().bg(PANEL))));
    rows
}

// each line wrapped to the width left after the bar, every row (the
// wrapped continuations too) behind the same bar, so the text stays
// aligned; a continuation keeps its soft mark (the copy joins it)
fn barred_rows(
    bar: &Span<'static>,
    lines: impl IntoIterator<Item = Line<'static>>,
    width: usize,
) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let inner = width.saturating_sub(bar.content.width()).max(1);
    let mut rows = Vec::new();
    for l in lines {
        for r in wrap_line(l, inner) {
            let mut spans = vec![bar.clone()];
            spans.extend(r.spans);
            let mut row = Line::from(spans);
            row.alignment = r.alignment;
            rows.push(row);
        }
    }
    rows
}


// the tool line, OpenCode inline-tool style: 2-col icon, name,
// elapsed, args preview; result preview on the next line. The state is
// a glyph: braille spinner while running, green ✓ once ok, red ✗ on
// failure.
// " 1.2s" after the tool name; nothing for a replayed tool (no duration)
pub(crate) fn elapsed_label(elapsed: &Option<String>) -> String {
    match elapsed.as_deref() {
        Some(e) if !e.is_empty() => format!(" {}", e),
        _ => String::new(),
    }
}

// the name and the one-line args of a tool, and its decoded source
pub(crate) fn tool_meta(td: &ToolData) -> (String, String, Option<(CodeLang, String)>) {
    let name = td.name.clone().unwrap_or_else(|| format!("#{}", td.id));
    // the source of a code tool (run_typescript, bash, apply_patch), when
    // the runtime sent it: rendered in full, highlighted, under the line
    let code = match (&td.code, code_lang(&name)) {
        (Some(raw), Some(lang)) => {
            Some((lang, tool_source(lang, wire_decode(raw)))).filter(|(_, c)| !c.trim().is_empty())
        }
        _ => None,
    };
    let args = match &code {
        // the block shows the whole source: a gray copy on the tool line
        // would only repeat it
        Some((CodeLang::Bash | CodeLang::TypeScript, _)) => String::new(),
        // a patch keeps its one-line summary (the files it touches)
        Some((CodeLang::Patch, src)) => patch_summary(src),
        None => td
            .args
            .as_deref()
            .map(|a| args_preview(&name, a))
            .unwrap_or_default(),
    };
    (name, args, code)
}

// the tool line itself: the only part of a running tool that changes
// from one frame to the next (spinner, elapsed)
pub(crate) fn tool_head(td: &ToolData, tick: u32, name: &str, args: &str) -> Line<'static> {
    let args_span = |st: Style| {
        Span::styled(
            if args.is_empty() {
                String::new()
            } else {
                format!(" · {}", args)
            },
            st,
        )
    };
    match td.state {
        ToolState::Run => Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(spinner_frame(tick / 2), Style::default().fg(BRAND)),
            Span::styled(format!(" {}", name), Style::default().fg(TEXT)),
            Span::styled(format!(" {}", fmt_elapsed(td.started)), Style::default().fg(DIM)),
            args_span(Style::default().fg(DIM)),
        ]),
        ToolState::Ok => Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_OK), Style::default().fg(OK)),
            Span::styled(name.to_string(), Style::default().fg(TOOL)),
            Span::styled(elapsed_label(&td.elapsed), Style::default().fg(TOOL)),
            args_span(Style::default().fg(TOOL)),
        ]),
        ToolState::Fail => Line::from(vec![
            Span::styled(format!("  {} ", GLYPH_ERR), Style::default().fg(ERR)),
            Span::styled(name.to_string(), Style::default().fg(ERR)),
            Span::styled(elapsed_label(&td.elapsed), Style::default().fg(ERR)),
            args_span(Style::default().fg(ERR)),
        ]),
    }
}

// everything under the tool line: the result preview, the source block
pub(crate) fn tool_body(td: &ToolData, code: &Option<(CodeLang, String)>, width: usize) -> Vec<Line<'static>> {
    let mut ls = Vec::new();
    if let Some((ok, preview)) = &td.result {
        if !preview.trim().is_empty() {
            ls.push(Line::from(vec![
                Span::styled(format!("    {} ", GLYPH_BRANCH), Style::default().fg(TOOL)),
                Span::styled(
                    truncate_chars(bend_images::display(preview).trim(), 110),
                    Style::default().fg(if *ok { TOOL } else { ERR }),
                ),
            ]));
        }
    }
    // the source block, highlighted, in a bordered box: whole when
    // short or unfolded, else its first lines (a huge patch or program
    // would cost thousands of rows to build and scroll past)
    if let Some((lang, src)) = code {
        let total = src.lines().count();
        if total > CODE_FOLD_AT && !td.expanded {
            let head: String = src.lines().take(CODE_FOLD_SHOW).collect::<Vec<_>>().join("\n");
            ls.extend(code_block_lines(&head, *lang, &td.state, width));
            ls.push(Line::from(Span::styled(
                format!("    … {} more lines · click to show all", total - CODE_FOLD_SHOW),
                Style::default().fg(DIM),
            )));
        } else {
            ls.extend(code_block_lines(src, *lang, &td.state, width));
        }
    }
    ls
}

/// A source block longer than this shows its first `CODE_FOLD_SHOW` lines.
pub(crate) const CODE_FOLD_AT: usize = 60;
pub(crate) const CODE_FOLD_SHOW: usize = 40;

pub(crate) fn tool_lines(td: &ToolData, tick: u32, width: usize) -> Vec<Line<'static>> {
    let (name, args, code) = tool_meta(td);
    let mut ls = vec![tool_head(td, tick, &name, &args)];
    ls.extend(tool_body(td, &code, width));
    ls
}

// a multi-line user message (Shift+Enter, paste) and an agent message:
// one row per line, a long line wrapped, every row behind the bar
#[cfg(test)]
mod multiline_tests {
    use crate::feed::build_rows;
    use crate::wire::Ev;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    fn screen(ev: Ev, width: u16) -> Vec<String> {
        let rows = build_rows(&[ev], 0, false, width as usize, 0);
        let h = rows.len() as u16;
        let mut term = Terminal::new(TestBackend::new(width, h)).unwrap();
        term.draw(|f| f.render_widget(Paragraph::new(rows), f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
            .collect()
    }

    #[test]
    fn user_message_keeps_its_line_breaks() {
        let long = "word ".repeat(12);
        let text = format!("first line\nsecond line\n{}end", long);
        let s = screen(Ev::You(text), 30);
        let body: Vec<&String> = s.iter().filter(|r| r.starts_with(" ┃ ")).collect();
        assert_eq!(body[0].as_str(), " ┃ first line", "{s:#?}");
        assert_eq!(body[1].as_str(), " ┃ second line", "{s:#?}");
        // the long line wraps into several rows, all behind the bar at
        // the same column
        assert!(body.len() >= 5, "{s:#?}");
        for r in &body[2..] {
            assert!(r.starts_with(" ┃ word") || r.starts_with(" ┃ end"), "{s:#?}");
            assert!(r.chars().count() <= 30);
        }
        assert!(body.last().unwrap().ends_with("end"), "{s:#?}");
        assert!(!s.iter().any(|r| r.contains('\n')));
    }

    #[test]
    fn agent_message_wraps_behind_its_bar() {
        let text = format!("one\ntwo {}", "x ".repeat(30));
        let s = screen(Ev::AgentMsg { head: "main".into(), text }, 24);
        let body: Vec<&String> = s.iter().filter(|r| r.starts_with(" │ ")).collect();
        assert_eq!(body[0].as_str(), " │ one", "{s:#?}");
        assert!(body[1].starts_with(" │ two x"), "{s:#?}");
        assert!(body.len() >= 4, "{s:#?}");
        assert!(s.iter().filter(|r| !r.is_empty() && !r.starts_with(" ◀ ")).all(|r| r.starts_with(" │ ")), "{s:#?}");
    }
}
