//! The source blocks of code tools (run_typescript, bash, apply_patch):
//! the syntax highlighters and the bordered, wrapped code block.

use crate::theme::*;
use crate::wire::ToolState;
use crate::{cell_widths, feedsel, json_str_field, truncate_chars};
use unicode_width::UnicodeWidthStr;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// ---- the run_typescript code block ----


pub(crate) const TS_KEYWORDS: &[&str] = &[
    "abstract", "any", "as", "asserts", "async", "await", "boolean", "break", "case", "catch",
    "class", "const", "continue", "debugger", "declare", "default", "delete", "do", "else",
    "enum", "export", "extends", "false", "finally", "for", "from", "function", "if",
    "implements", "import", "in", "infer", "instanceof", "interface", "is", "keyof", "let",
    "namespace", "never", "new", "null", "number", "object", "of", "private", "protected",
    "public", "readonly", "return", "satisfies", "static", "string", "super", "switch",
    "symbol", "this", "throw", "true", "try", "type", "typeof", "undefined", "unknown", "var",
    "void", "while", "with", "yield",
];

pub(crate) fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

pub(crate) fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

// append one token to the per-line span list, splitting on newlines and
// merging adjacent same-style spans (fewer spans render faster)
pub(crate) fn push_tok(lines: &mut Vec<Vec<Span<'static>>>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    let mut first = true;
    for part in text.split('\n') {
        if !first {
            lines.push(Vec::new());
        }
        first = false;
        if part.is_empty() {
            continue;
        }
        let line = lines.last_mut().expect("push_tok: a line exists");
        if let Some(last) = line.last_mut() {
            if last.style == style {
                last.content.to_mut().push_str(part);
                continue;
            }
        }
        line.push(Span::styled(part.to_string(), style));
    }
}

// tokenize TypeScript into highlighted per-line spans, one pass, char by
// char: keywords, strings and template literals, comments (line and
// block, block may span lines), numbers, call sites, capitalized types
pub(crate) fn highlight_ts(src: &str) -> Vec<Vec<Span<'static>>> {
    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let comment = Style::default().fg(SYNTAX_COMMENT).add_modifier(Modifier::ITALIC);
    let string = Style::default().fg(SYNTAX_STRING);
    let number = Style::default().fg(SYNTAX_NUMBER);
    let keyword = Style::default().fg(SYNTAX_KEYWORD).add_modifier(Modifier::BOLD);
    let func = Style::default().fg(SYNTAX_FUNC);
    let typ = Style::default().fg(HEAD);
    let plain = Style::default().fg(TEXT);
    let punct = Style::default().fg(DIM);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut i = 0usize;
    while i < n {
        let c = cs[i];
        // line comment
        if c == '/' && cs.get(i + 1) == Some(&'/') {
            let mut t = String::new();
            while i < n && cs[i] != '\n' {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // block comment (may span lines; push_tok splits them)
        if c == '/' && cs.get(i + 1) == Some(&'*') {
            let mut t = String::from("/*");
            i += 2;
            while i < n {
                if cs[i] == '*' && cs.get(i + 1) == Some(&'/') {
                    t.push_str("*/");
                    i += 2;
                    break;
                }
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // strings and template literals
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let mut t = String::new();
            t.push(quote);
            i += 1;
            while i < n {
                if cs[i] == '\\' && i + 1 < n {
                    t.push(cs[i]);
                    t.push(cs[i + 1]);
                    i += 2;
                    continue;
                }
                t.push(cs[i]);
                if cs[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            push_tok(&mut lines, &t, string);
            continue;
        }
        // number
        if c.is_ascii_digit() && !cs.get(i.wrapping_sub(1)).is_some_and(|&p| is_id_char(p)) {
            let mut t = String::new();
            while i < n && (cs[i].is_ascii_alphanumeric() || matches!(cs[i], '.' | '_')) {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, number);
            continue;
        }
        // identifier: keyword, call, type or plain name
        if is_id_start(c) {
            let mut t = String::new();
            while i < n && is_id_char(cs[i]) {
                t.push(cs[i]);
                i += 1;
            }
            let mut k = i;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let style = if TS_KEYWORDS.contains(&t.as_str()) {
                keyword
            } else if cs.get(k) == Some(&'(') {
                func
            } else if t.starts_with(|ch: char| ch.is_uppercase()) {
                typ
            } else {
                plain
            };
            push_tok(&mut lines, &t, style);
            continue;
        }
        // any other char is punctuation
        push_tok(&mut lines, &c.to_string(), punct);
        i += 1;
    }
    lines
}

// ---- the bash code block ----

pub(crate) const BASH_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "select", "while", "until",
    "do", "done", "in", "function", "time", "!", "[[", "]]", "coproc",
];

// keywords after which the next word is a command again
pub(crate) const BASH_CMD_AFTER: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "time", "!", "coproc",
];

pub(crate) fn is_bash_word_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '|' | '&' | ';' | '<' | '>' | '(' | ')' | '"' | '\'' | '`' | '$')
}

// one $-expansion starting at cs[i] == '$': ${...}, $name, $1, $?, ...
// "$(" is not an expansion (the caller treats it as a command start).
// Returns the token and the index after it.
pub(crate) fn bash_var(cs: &[char], i: usize) -> Option<(String, usize)> {
    let n = cs.len();
    match cs.get(i + 1) {
        Some('{') => {
            let mut j = i + 2;
            while j < n && cs[j] != '}' && cs[j] != '\n' {
                j += 1;
            }
            let end = if j < n && cs[j] == '}' { j + 1 } else { j };
            Some((cs[i..end].iter().collect(), end))
        }
        Some(&c) if c.is_ascii_alphabetic() || c == '_' => {
            let mut j = i + 1;
            while j < n && (cs[j].is_ascii_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            Some((cs[i..j].iter().collect(), j))
        }
        Some(&c) if c.is_ascii_digit() || matches!(c, '?' | '@' | '#' | '$' | '!' | '*' | '-') => {
            Some((cs[i..i + 2].iter().collect(), i + 2))
        }
        _ => None,
    }
}

// tokenize bash into highlighted per-line spans, one pass: comments,
// strings (with $-expansions inside double quotes), variables,
// keywords, the command word of each simple command, options, numbers
// and operators (pipes, lists, redirections)
pub(crate) fn highlight_bash(src: &str) -> Vec<Vec<Span<'static>>> {
    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let comment = Style::default().fg(SYNTAX_COMMENT).add_modifier(Modifier::ITALIC);
    let string = Style::default().fg(SYNTAX_STRING);
    let number = Style::default().fg(SYNTAX_NUMBER);
    let var = Style::default().fg(SYNTAX_NUMBER);
    let keyword = Style::default().fg(SYNTAX_KEYWORD).add_modifier(Modifier::BOLD);
    let op = Style::default().fg(SYNTAX_KEYWORD);
    let func = Style::default().fg(SYNTAX_FUNC);
    let plain = Style::default().fg(TEXT);
    let option = Style::default().fg(HEAD);
    let punct = Style::default().fg(DIM);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    // the next word is a command name (start, after ; | && || ( $( ...)
    let mut cmd_pos = true;
    // "for x in", "case x in": the "in" after the name is a keyword
    let mut want_in = false;
    let mut i = 0usize;
    while i < n {
        let c = cs[i];
        if c == '\n' {
            push_tok(&mut lines, "\n", plain);
            cmd_pos = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            push_tok(&mut lines, &c.to_string(), plain);
            i += 1;
            continue;
        }
        // comment: '#' at the start of a word
        if c == '#' && (i == 0 || cs[i - 1].is_whitespace() || matches!(cs[i - 1], ';' | '(' | '|' | '&')) {
            let mut t = String::new();
            while i < n && cs[i] != '\n' {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, comment);
            continue;
        }
        // single quotes: literal to the closing quote
        if c == '\'' {
            let mut t = String::from("'");
            i += 1;
            while i < n {
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '\'' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            cmd_pos = false;
            continue;
        }
        // double quotes: $-expansions keep their own color
        if c == '"' {
            let mut t = String::from("\"");
            i += 1;
            while i < n {
                if cs[i] == '\\' && i + 1 < n {
                    t.push(cs[i]);
                    t.push(cs[i + 1]);
                    i += 2;
                    continue;
                }
                if cs[i] == '$' {
                    if let Some((v, j)) = bash_var(&cs, i) {
                        push_tok(&mut lines, &t, string);
                        t.clear();
                        push_tok(&mut lines, &v, var);
                        i = j;
                        continue;
                    }
                }
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '"' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            cmd_pos = false;
            continue;
        }
        // backquotes: a command substitution, shown as a string
        if c == '`' {
            let mut t = String::from("`");
            i += 1;
            while i < n {
                t.push(cs[i]);
                i += 1;
                if cs[i - 1] == '`' {
                    break;
                }
            }
            push_tok(&mut lines, &t, string);
            continue;
        }
        if c == '$' {
            // $( and $(( open a new command / arithmetic
            if cs.get(i + 1) == Some(&'(') {
                push_tok(&mut lines, "$(", op);
                i += 2;
                cmd_pos = true;
                continue;
            }
            if let Some((v, j)) = bash_var(&cs, i) {
                push_tok(&mut lines, &v, var);
                i = j;
                cmd_pos = false;
                continue;
            }
            push_tok(&mut lines, "$", plain);
            i += 1;
            continue;
        }
        // operators: lists, pipes, subshells, redirections
        if matches!(c, '|' | '&' | ';' | '(' | ')' | '<' | '>') {
            let mut t = String::new();
            t.push(c);
            i += 1;
            while i < n && matches!(cs[i], '|' | '&' | ';' | '<' | '>') && t.len() < 3 {
                t.push(cs[i]);
                i += 1;
            }
            push_tok(&mut lines, &t, op);
            // a redirection target is an argument; everything else
            // starts a new command
            cmd_pos = !(t.contains('<') || t.contains('>') || t == ")");
            continue;
        }
        // a word
        let mut t = String::new();
        while i < n && is_bash_word_char(cs[i]) {
            if cs[i] == '\\' && i + 1 < n {
                t.push(cs[i]);
                t.push(cs[i + 1]);
                i += 2;
                continue;
            }
            t.push(cs[i]);
            i += 1;
        }
        if t.is_empty() {
            // a lone special char the branches above did not take
            push_tok(&mut lines, &c.to_string(), punct);
            i += 1;
            continue;
        }
        let w = t.as_str();
        if want_in && w == "in" {
            push_tok(&mut lines, w, keyword);
            want_in = false;
            cmd_pos = false;
        } else if (cmd_pos && BASH_KEYWORDS.contains(&w)) || matches!(w, "]]" | "{" | "}") {
            push_tok(&mut lines, w, keyword);
            want_in = matches!(w, "for" | "case" | "select");
            cmd_pos = BASH_CMD_AFTER.contains(&w) || w == "{";
        } else if cmd_pos
            && w.find('=').is_some_and(|k| {
                k > 0 && w[..k].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            })
        {
            // NAME=value before the command: still in command position
            let k = w.find('=').unwrap_or(0);
            push_tok(&mut lines, &w[..k], var);
            push_tok(&mut lines, "=", punct);
            push_tok(&mut lines, &w[k + 1..], plain);
        } else if cmd_pos {
            push_tok(&mut lines, w, func);
            cmd_pos = false;
        } else if w.starts_with('-') && w.len() > 1 {
            push_tok(&mut lines, w, option);
        } else if w.chars().all(|ch| ch.is_ascii_digit()) {
            push_tok(&mut lines, w, number);
        } else {
            push_tok(&mut lines, w, plain);
        }
    }
    lines
}

// ---- the apply_patch diff block ----

// the V4A patch as a diff: file headers, hunk markers, added lines on a
// green band, removed lines on a red band, context dimmed. The
// Begin/End Patch envelope is noise and never shows.
pub(crate) fn highlight_patch(src: &str) -> Vec<Vec<Span<'static>>> {
    let file = |glyph: &str, path: &str, color: Color, note: &str| {
        let mut v = vec![
            Span::styled(format!("{} ", glyph), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(path.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ];
        if !note.is_empty() {
            v.push(Span::styled(format!(" · {}", note), Style::default().fg(DIM)));
        }
        v
    };
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    for l in src.split('\n') {
        let header = if let Some(p) = l.strip_prefix("*** Update File: ") {
            Some(file("~", p, HEAD, ""))
        } else if let Some(p) = l.strip_prefix("*** Add File: ") {
            Some(file("+", p, OK, "new"))
        } else { l.strip_prefix("*** Delete File: ").map(|p| file("−", p, ERR, "deleted")) };
        if let Some(h) = header {
            // a blank row between two files
            if !lines.is_empty() {
                lines.push(Vec::new());
            }
            lines.push(h);
            continue;
        }
        if let Some(p) = l.strip_prefix("*** Move to: ") {
            lines.push(file("→", p, HEAD, "renamed"));
            continue;
        }
        if l.starts_with("*** ") || (l.is_empty() && lines.is_empty()) {
            // Begin Patch, End Patch, End of File
            continue;
        }
        let span = if l.starts_with("@@") {
            Span::styled(l.to_string(), Style::default().fg(SYNTAX_FUNC))
        } else if l.starts_with('+') {
            Span::styled(l.to_string(), Style::default().fg(OK).bg(DIFF_ADD_BG))
        } else if l.starts_with('-') {
            Span::styled(l.to_string(), Style::default().fg(ERR).bg(DIFF_DEL_BG))
        } else {
            Span::styled(l.to_string(), Style::default().fg(DIM))
        };
        lines.push(vec![span]);
    }
    // a trailing blank (the text's final newline) is not a row
    while lines.last().is_some_and(|l| l.iter().all(|s| s.content.trim().is_empty())) {
        lines.pop();
    }
    lines
}

// the tool-line preview of a patch: each file with its line counts,
// "core/obs.bend +3 −1, LAWS.bend +12"
pub(crate) fn patch_summary(src: &str) -> String {
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for l in src.split('\n') {
        let path = l
            .strip_prefix("*** Update File: ")
            .or_else(|| l.strip_prefix("*** Add File: "))
            .or_else(|| l.strip_prefix("*** Delete File: "));
        if let Some(p) = path {
            files.push((p.trim().to_string(), 0, 0));
            continue;
        }
        if let Some(p) = l.strip_prefix("*** Move to: ") {
            if let Some(f) = files.last_mut() {
                f.0 = format!("{} → {}", f.0, p.trim());
            }
            continue;
        }
        if let Some(f) = files.last_mut() {
            if l.starts_with('+') {
                f.1 += 1;
            } else if l.starts_with('-') {
                f.2 += 1;
            }
        }
    }
    let parts: Vec<String> = files
        .into_iter()
        .map(|(p, add, del)| {
            let mut s = p;
            if add > 0 {
                s.push_str(&format!(" +{}", add));
            }
            if del > 0 {
                s.push_str(&format!(" −{}", del));
            }
            s
        })
        .collect();
    truncate_chars(&parts.join(", "), 80)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum CodeLang {
    TypeScript,
    Bash,
    Patch,
}

// the code tools: which ones render a source block, in which language
pub(crate) fn code_lang(tool: &str) -> Option<CodeLang> {
    match tool {
        "run_typescript" => Some(CodeLang::TypeScript),
        "bash" => Some(CodeLang::Bash),
        "apply_patch" => Some(CodeLang::Patch),
        _ => None,
    }
}

// the source a code tool ran, from its tool_code args: the "code" field
// of the run_typescript JSON, the raw command for bash
pub(crate) fn tool_source(lang: CodeLang, decoded: String) -> String {
    match lang {
        CodeLang::TypeScript => json_str_field(&decoded, "code").unwrap_or(decoded),
        CodeLang::Bash | CodeLang::Patch => decoded,
    }
}

// wrap one line of spans into rows of at most `w` display columns, so
// the block shows the whole source. A row breaks after its last
// whitespace when that keeps at least half the row; otherwise (a long
// token) it breaks hard at the width. Returns each row with its width.
pub(crate) fn wrap_code_line(spans: &[Span<'static>], w: usize) -> Vec<(Vec<Span<'static>>, usize)> {
    let w = w.max(1);
    let widths = cell_widths(spans.iter().flat_map(|sp| sp.content.chars()));
    let cells: Vec<(char, Style, usize)> = spans
        .iter()
        .flat_map(|sp| sp.content.chars().map(move |ch| (ch, sp.style)))
        .zip(widths)
        .map(|((ch, st), w)| (ch, st, w))
        .collect();
    let mut rows = Vec::new();
    let mut start = 0usize;
    while start < cells.len() {
        // the longest run that fits
        let mut end = start;
        let mut used = 0usize;
        while end < cells.len() && used + cells[end].2 <= w {
            used += cells[end].2;
            end += 1;
        }
        if end == start {
            // a single cell wider than the row: take it anyway
            end = start + 1;
        } else if end < cells.len() {
            // prefer breaking after the last whitespace in the row
            if let Some(k) = (start..end).rev().find(|&k| cells[k].0.is_whitespace()) {
                let before: usize = cells[start..=k].iter().map(|c| c.2).sum();
                if before * 2 >= w {
                    end = k + 1;
                }
            }
        }
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut row_w = 0usize;
        for &(ch, style, cw) in &cells[start..end] {
            row_w += cw;
            match row.last_mut() {
                Some(last) if last.style == style => last.content.to_mut().push(ch),
                _ => row.push(Span::styled(ch.to_string(), style)),
            }
        }
        rows.push((row, row_w));
        start = end;
    }
    if rows.is_empty() {
        // an empty source line still gets its row
        rows.push((Vec::new(), 0));
    }
    rows
}

// the code block: a rounded border (orange while the tool runs, dim once
// done, red on failure), a header, a line-number gutter, and the whole
// source: a long line wraps inside the box, its continuation rows with
// an empty gutter (the numbers mark where each source line starts)
pub(crate) fn code_block_lines(
    code: &str,
    lang: CodeLang,
    state: &ToolState,
    width: usize,
) -> Vec<Line<'static>> {
    let hl = match lang {
        CodeLang::TypeScript => highlight_ts(code),
        CodeLang::Bash => highlight_bash(code),
        CodeLang::Patch => highlight_patch(code),
    };
    // a diff has no meaningful line numbers: no gutter
    let numbered = lang != CodeLang::Patch;
    if hl.is_empty() {
        return Vec::new();
    }
    let border = match state {
        ToolState::Run => BRAND,
        ToolState::Ok => BORDER_ACTIVE,
        ToolState::Fail => ERR,
    };
    let bstyle = Style::default().fg(border);
    // 2 feed margin, "│ " and "│" borders, 1 right pad, and when
    // numbered the gutter with its " │ " separator
    let gutter_w = hl.len().to_string().len();
    let overhead = if numbered { gutter_w + 9 } else { 6 };
    let cw = width.saturating_sub(overhead).max(8);
    let inner = if numbered { gutter_w + cw + 5 } else { cw + 2 };
    let mut rows: Vec<Line<'static>> = Vec::new();
    // top: "  ╭─ typescript ───…───╮" (or "─ bash ")
    let header = match lang {
        CodeLang::TypeScript => "─ typescript ",
        CodeLang::Bash => "─ bash ",
        CodeLang::Patch => "─ diff ",
    };
    let fill = inner.saturating_sub(header.width()).max(1);
    rows.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("╭", bstyle),
        Span::styled(header.to_string(), bstyle),
        Span::styled("─".repeat(fill), bstyle),
        Span::styled("╮", bstyle),
    ]));
    for (i, spans) in hl.iter().enumerate() {
        // a background band (diff lines) runs to the right border
        let band = spans
            .first()
            .and_then(|sp| sp.style.bg)
            .map_or(Style::default(), |bg| Style::default().bg(bg));
        for (r, (content, used)) in wrap_code_line(spans, cw).into_iter().enumerate() {
            let mut ls = vec![
                Span::styled("  ", Style::default()),
                Span::styled("│ ", bstyle),
            ];
            if numbered {
                let gutter = if r == 0 {
                    format!("{:>gw$}", i + 1, gw = gutter_w)
                } else {
                    " ".repeat(gutter_w)
                };
                ls.push(Span::styled(gutter, Style::default().fg(FAINT)));
                ls.push(Span::styled(" │ ", bstyle));
            }
            ls.extend(content);
            // pad to the row width, plus the 1-column right pad
            ls.push(Span::styled(" ".repeat(cw - used.min(cw) + 1), band));
            ls.push(Span::styled("│", bstyle));
            let mut line = Line::from(ls);
            if r > 0 {
                feedsel::mark_soft(&mut line);
            }
            rows.push(line);
        }
    }
    rows.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("╰", bstyle),
        Span::styled("─".repeat(inner), bstyle),
        Span::styled("╯", bstyle),
    ]));
    rows
}
