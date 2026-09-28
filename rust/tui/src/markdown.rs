//! Markdown rendering of user and assistant messages.

use crate::theme::*;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

// ---- markdown rendering (user + assistant messages) ----
// The wire carries newlines escaped as a literal backslash-n; the TUI
// unescapes and renders a pragmatic markdown subset: fenced code
// blocks, headers, bullet lists, blockquotes, and inline bold, italic
// and code.

pub(crate) fn unescape_md(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'n') {
            chars.next();
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

// inline styles in the OpenCode markdown colors: **strong** is
// markdownStrong (orange), *emph* is markdownEmph (yellow), `code` is
// markdownCode (green)
pub(crate) fn inline_spans(s: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '`' {
            if let Some(j) = (i + 1..cs.len()).find(|k| cs[*k] == '`') {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    Style::default().fg(OK).add_modifier(Modifier::BOLD),
                ));
                i = j + 1;
                continue;
            }
        }
        if c == '*' {
            if i + 1 < cs.len() && cs[i + 1] == '*' {
                if let Some(j) =
                    (i + 3..cs.len()).find(|k| cs[*k] == '*' && cs.get(k + 1) == Some(&'*'))
                {
                    if !plain.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut plain), base));
                    }
                    spans.push(Span::styled(
                        cs[i + 2..j].iter().collect::<String>(),
                        base.add_modifier(Modifier::BOLD).fg(WARN),
                    ));
                    i = j + 2;
                    continue;
                }
            } else if let Some(j) = (i + 2..cs.len()).find(|k| {
                cs[*k] == '*' && cs.get(k - 1) != Some(&'*') && cs.get(k + 1) != Some(&'*')
            }) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                spans.push(Span::styled(
                    cs[i + 1..j].iter().collect::<String>(),
                    base.add_modifier(Modifier::ITALIC).fg(HEAD),
                ));
                i = j + 1;
                continue;
            }
        }
        plain.push(c);
        i += 1;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, base));
    }
    spans
}

pub(crate) fn md_to_lines(text: &str) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    for raw in text.split('\n') {
        let line = raw.trim_end();
        if line.starts_with("```") {
            out.push(Line::from(Span::styled("  ", Style::default().bg(ELEMENT))));
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(TEXT).bg(ELEMENT),
            )));
            continue;
        }
        if line.is_empty() {
            out.push(Line::from(""));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim_start();
        let base = Style::default().fg(TEXT);
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            let head = t[level..].trim_start();
            out.push(Line::from(Span::styled(
                head.to_string(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        // OpenCode markdownListEnumeration: "N. item" in the info color
        let dot = t
            .char_indices()
            .find(|(i, c)| *i > 0 && *c == '.' && t[i + 1..].starts_with(' '));
        if let Some((i, _)) = dot {
            if t[..i].chars().all(|c| c.is_ascii_digit()) {
                out.push(Line::from_iter(
                    std::iter::once(Span::styled(
                        format!("  {} ", &t[..i + 1]),
                        Style::default().fg(INFO),
                    ))
                    .chain(inline_spans(t[i + 1..].trim_start(), base)),
                ));
                continue;
            }
        }
        if indent == 0 && (t.starts_with("- ") || t.starts_with("* ")) {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  - ", Style::default().fg(BRAND)))
                    .chain(inline_spans(&t[2..], base)),
            ));
            continue;
        }
        if indent == 0 && t.starts_with("> ") {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  | ", Style::default().fg(HEAD)))
                    .chain(inline_spans(&t[2..], Style::default().fg(HEAD))),
            ));
            continue;
        }
        out.push(Line::from(inline_spans(line, base)));
    }
    out
}
