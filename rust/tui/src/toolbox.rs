//! The bash / TypeScript box (book §11 "Scripts: a box", BISE-96): a
//! rounded box the width of the code measure, its title in the top border
//! (`╭─ $ bash ∿ 12s ───╮`), the script in full, a faint rule, then the
//! output, dim, 15 rows at most while closed. Other tools stay one line.

use crate::code::*;
use crate::render::{elapsed_label, fmt_elapsed};
use crate::theme::{self, *};
use crate::wire::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Output rows kept while the box is closed.
pub(crate) const CLOSED_ROWS: usize = 15;
/// A done box keeps its first rows and its last ones around `▸ n more`.
const HEAD_ROWS: usize = 5;
const TAIL_ROWS: usize = 9;

/// A tool drawn as a box: bash and TypeScript.
pub(crate) fn is_boxed(td: &ToolData) -> bool {
    matches!(td.name.as_deref(), Some("bash") | Some("run_typescript"))
}

/// A sub-call of a TypeScript run, as the box shows it.
pub(crate) struct SubCall<'a> {
    pub(crate) name: &'a str,
    pub(crate) ok: bool,
    pub(crate) preview: &'a str,
}

// corners, lines and joints: rounded, or plain ASCII (book §11)
struct Frame {
    tl: &'static str,
    tr: &'static str,
    bl: &'static str,
    br: &'static str,
    h: &'static str,
    v: &'static str,
    lj: &'static str,
    rj: &'static str,
}

fn frame() -> Frame {
    if theme::ascii_mode() {
        Frame { tl: "+", tr: "+", bl: "+", br: "+", h: "-", v: "|", lj: "+", rj: "+" }
    } else {
        Frame { tl: "╭", tr: "╮", bl: "╰", br: "╯", h: "─", v: "│", lj: "├", rj: "┤" }
    }
}

/// The border: text while running (not the accent), faint when done,
/// error when failed.
fn border(td: &ToolData) -> Color {
    match td.state {
        ToolState::Run => text(),
        ToolState::Ok => faint(),
        ToolState::Fail => error(),
    }
}

/// The result text lines of the box (images aside: their chip is a line).
fn result_lines(td: &ToolData) -> (Option<Vec<Span<'static>>>, Vec<String>) {
    let Some((_, preview)) = &td.result else { return (None, Vec::new()) };
    let images = crate::attach::result_spans(preview);
    let shown = if images.is_some() { crate::attach::without_markers(preview) } else { preview.clone() };
    let t = shown.trim_end();
    let lines = if t.trim().is_empty() { Vec::new() } else { t.lines().map(str::to_string).collect() };
    (images, lines)
}

/// The box has more output than it shows closed (`▸` / ctrl+o / click).
pub(crate) fn box_folds(td: &ToolData) -> bool {
    result_lines(td).1.len() > CLOSED_ROWS
}

/// The top border with the title, the only row a running box redraws.
pub(crate) fn box_top(td: &ToolData, tick: u32, width: usize) -> Line<'static> {
    let f = frame();
    let bst = Style::default().fg(border(td));
    let (g, label) = if td.name.as_deref() == Some("bash") { (G_BASH, "bash") } else { (G_TS, "typescript") };
    let mut spans = vec![
        Span::styled(format!("{}{} ", f.tl, f.h), bst),
        Span::styled(format!("{} {}", glyph(g), label), Style::default().fg(text())),
    ];
    let dim_st = Style::default().fg(dim());
    match td.state {
        ToolState::Run => {
            let (p, c) = working_frame(tick);
            spans.push(Span::styled(format!(" {}", p), Style::default().fg(c)));
            spans.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            let ok = if theme::ascii_mode() { "ok" } else { G_RECEIVED };
            spans.push(Span::styled(format!(" {}{}", ok, elapsed_label(&td.elapsed)), dim_st));
        }
        ToolState::Fail => {
            spans.push(Span::styled(
                format!(" {}{}", glyph(G_FAILED), elapsed_label(&td.elapsed)),
                Style::default().fg(error()),
            ));
        }
    }
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let fill = width.saturating_sub(used + 2);
    spans.push(Span::styled(format!(" {}{}", f.h.repeat(fill), f.tr), bst));
    Line::from(spans)
}

/// One inner row: `│ content … │`, padded to the box.
fn inner_row(content: Vec<Span<'static>>, inner: usize, bst: Style, soft: bool) -> Line<'static> {
    let f = frame();
    let used: usize = content.iter().map(|s| s.content.width()).sum();
    let mut spans = vec![Span::styled(format!("{} ", f.v), bst)];
    spans.extend(content);
    spans.push(Span::raw(" ".repeat(inner.saturating_sub(used))));
    spans.push(Span::styled(format!(" {}", f.v), bst));
    let mut l = Line::from(spans);
    if soft {
        crate::feedsel::mark_soft(&mut l);
    }
    l
}

/// Styled logical lines wrapped inside the box (a hanging `»` on the
/// continuation rows, like the code blocks).
fn wrapped(lines: &[Vec<Span<'static>>], inner: usize, bst: Style, out: &mut Vec<Line<'static>>) {
    let hang = Span::styled(format!("{} ", glyph(G_WRAP)), Style::default().fg(faint()));
    let rest = inner.saturating_sub(hang.content.width()).max(4);
    for spans in lines {
        if spans.iter().all(|s| s.content.is_empty()) {
            out.push(inner_row(Vec::new(), inner, bst, false));
            continue;
        }
        for (r, (content, _)) in wrap_code_line_hanging(spans, inner.max(4), rest).into_iter().enumerate() {
            let mut c = Vec::new();
            if r > 0 {
                c.push(hang.clone());
            }
            c.extend(content);
            out.push(inner_row(c, inner, bst, r > 0));
        }
    }
}

/// The whole box: top border, script, rule, output, bottom border.
/// `subs` are the TypeScript sub-calls (shown in full, before the result).
pub(crate) fn box_lines(
    td: &ToolData,
    code: &Option<(CodeLang, String)>,
    subs: &[SubCall],
    tick: u32,
    width: usize,
) -> Vec<Line<'static>> {
    let f = frame();
    let width = width.max(12);
    let inner = width - 4;
    let bst = Style::default().fg(border(td));
    let dim_st = Style::default().fg(dim());
    let faint_st = Style::default().fg(faint());
    let mut out = vec![box_top(td, tick, width)];
    // the script, in full, highlighted
    let script: Vec<Vec<Span<'static>>> = match code {
        Some((CodeLang::Bash, src)) => highlight_bash(src),
        Some((CodeLang::TypeScript, src)) => highlight_ts(src),
        _ => td
            .args
            .as_deref()
            .map(|a| vec![vec![Span::styled(crate::render::args_preview(td.name.as_deref().unwrap_or(""), a), dim_st)]])
            .unwrap_or_default(),
    };
    wrapped(&script, inner, bst, &mut out);
    // the output: sub-calls, then the result, 15 rows while closed
    let mut shown: Vec<Vec<Span<'static>>> = subs
        .iter()
        .map(|s| {
            let mut l = vec![Span::styled(format!("{} {}", glyph(G_SUBCALL), s.name), dim_st)];
            if s.ok {
                l.push(Span::styled(format!(" {}", glyph(G_RECEIVED)), dim_st));
            } else {
                let why = crate::render::fit_chars(s.preview.trim(), 80);
                l.push(Span::styled(format!(" {} {}", glyph(G_FAILED), why).trim_end().to_string(), Style::default().fg(error())));
            }
            l
        })
        .collect();
    let (images, lines) = result_lines(td);
    if let Some(chip) = images {
        shown.push(chip);
    }
    let line = |t: &str| vec![Span::styled(t.to_string(), dim_st)];
    let n = lines.len();
    if td.expanded || n <= CLOSED_ROWS {
        shown.extend(lines.iter().map(|t| line(t)));
    } else if matches!(td.state, ToolState::Ok) {
        let more = n - HEAD_ROWS - TAIL_ROWS;
        let mark = if theme::ascii_mode() { ">" } else { G_CLOSED };
        shown.extend(lines[..HEAD_ROWS].iter().map(|t| line(t)));
        shown.push(vec![Span::styled(format!("{} {} more lines", mark, more), dim_st)]);
        shown.extend(lines[n - TAIL_ROWS..].iter().map(|t| line(t)));
    } else {
        // running or failed: the end is what matters
        let above = n - CLOSED_ROWS;
        shown.push(vec![Span::styled(format!("{} {} lines above", theme::ellipsis(), above), faint_st)]);
        shown.extend(lines[above..].iter().map(|t| line(t)));
    }
    if !shown.is_empty() {
        out.push(Line::from(Span::styled(format!("{}{}{}", f.lj, f.h.repeat(width - 2), f.rj), faint_st)));
        wrapped(&shown, inner, bst, &mut out);
    }
    out.push(Line::from(Span::styled(format!("{}{}{}", f.bl, f.h.repeat(width - 2), f.br), bst)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, state: ToolState, out: usize) -> ToolData {
        let mut td = ToolData::bare(1, state);
        td.name = Some(name.into());
        td.code = Some("cargo test".into());
        td.elapsed = Some("0.9s".into());
        if out > 0 {
            let ok = !matches!(td.state, ToolState::Fail);
            td.result = Some((ok, (1..=out).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")));
        }
        td
    }

    fn text(td: &ToolData, subs: &[SubCall]) -> Vec<String> {
        let code = Some((CodeLang::Bash, "cargo test".to_string()));
        box_lines(td, &code, subs, 0, 60)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect()
    }

    fn inner(rows: &[String]) -> Vec<String> {
        rows.iter().map(|r| r.trim_start_matches("│ ").trim_end_matches([' ', '│']).to_string()).collect()
    }

    /// Rounded, the title in the top border, every row the box's width.
    #[test]
    fn a_box_has_its_title_in_the_border() {
        let rows = text(&tool("bash", ToolState::Ok, 3), &[]);
        assert!(rows[0].starts_with("╭─ $ bash ✓ 0.9s ─") && rows[0].ends_with('╮'), "{rows:#?}");
        assert!(rows[1].starts_with("│ cargo test"), "{rows:#?}");
        assert!(rows[2].starts_with('├') && rows.last().unwrap().starts_with('╰'), "{rows:#?}");
        assert!(rows.iter().all(|r| r.width() == 60), "{rows:#?}");
        assert!(!box_folds(&tool("bash", ToolState::Ok, 15)));
        assert!(box_folds(&tool("bash", ToolState::Ok, 16)));
    }

    /// Done: the first 5, `▸ n more lines`, the last 9; open: everything.
    #[test]
    fn a_done_box_keeps_its_head_and_tail() {
        let mut td = tool("bash", ToolState::Ok, 42);
        let rows = inner(&text(&td, &[]));
        let out: Vec<&String> = rows.iter().skip_while(|r| !r.starts_with('├')).skip(1).collect();
        assert_eq!(out.len(), 5 + 1 + 9 + 1, "{out:#?}");
        assert_eq!(out[0], "line 1");
        assert_eq!(out[5], "▸ 28 more lines");
        assert_eq!(out[14], "line 42");
        td.expanded = true;
        let open = inner(&text(&td, &[]));
        assert!(open.iter().any(|r| r == "line 20") && !open.iter().any(|r| r.contains("more lines")));
    }

    /// Running or failed: the last 15 under `… n lines above`; the border
    /// is the error color when it failed.
    #[test]
    fn a_running_or_failed_box_keeps_its_end() {
        for state in [ToolState::Run, ToolState::Fail] {
            let td = tool("bash", state, 40);
            let rows = inner(&text(&td, &[]));
            let out: Vec<&String> = rows.iter().skip_while(|r| !r.starts_with('├')).skip(1).collect();
            assert_eq!(out[0], "… 25 lines above", "{out:#?}");
            assert_eq!(out[1], "line 26");
            assert_eq!(out[15], "line 40");
        }
        let lines = box_lines(&tool("bash", ToolState::Fail, 1), &None, &[], 0, 40);
        assert_eq!(lines[0].spans[0].style.fg, Some(error()));
        let lines = box_lines(&tool("bash", ToolState::Ok, 1), &None, &[], 0, 40);
        assert_eq!(lines[0].spans[0].style.fg, Some(faint()));
    }

    /// TypeScript sub-calls are output lines inside the box.
    #[test]
    fn sub_calls_are_inside() {
        let td = tool("run_typescript", ToolState::Ok, 1);
        let subs = [SubCall { name: "github.search_issues", ok: true, preview: "" }];
        let rows = inner(&text(&td, &subs));
        assert!(rows[0].starts_with("╭─ λ typescript"), "{rows:#?}");
        let out: Vec<&String> = rows.iter().skip_while(|r| !r.starts_with('├')).skip(1).collect();
        assert_eq!(out[0], "↳ github.search_issues ✓");
        assert_eq!(out[1], "line 1");
    }

    /// ASCII: `+- $ bash ok 0.9s ---+`, `|`, `> n more lines`.
    #[test]
    fn the_ascii_box() {
        theme::set_ascii_for_tests(true);
        let rows = text(&tool("bash", ToolState::Ok, 20), &[]);
        theme::set_ascii_for_tests(false);
        assert!(rows[0].starts_with("+- $ bash ok 0.9s -") && rows[0].ends_with('+'), "{rows:#?}");
        assert!(rows[1].starts_with("| cargo test") && rows[1].ends_with(" |"));
        assert!(rows.iter().any(|r| r.starts_with("| > 6 more lines")), "{rows:#?}");
        assert!(rows.iter().all(|r| r.is_ascii()), "{rows:#?}");
    }
}
