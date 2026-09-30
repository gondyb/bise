//! The composer's live markdown (BISE-276): what you type is formatted
//! as you type it, every mark still there (no hidden char: the message
//! sent is the text typed, the cursor and the wrap never shift).
//!
//! - Lists, like a rich text editor: the newline keys continue a `- `,
//!   `* `, `+ `, `1. ` or `- [ ] ` item (numbers count up, the list
//!   renumbers), a newline on an empty item ends the list (an indented
//!   one steps out a level); Tab / Shift+Tab indent / outdent the items
//!   under the cursor or the selection. Each is one undo step.
//! - Fenced code blocks: the fence and its tag dim, the body colored by
//!   syntax.rs when the tag names a language (plain text else); in a
//!   block a newline keeps the line's indentation and plain ⏎ makes a
//!   newline (you do not send half a block), never a list.
//! - Inline: `code`, **bold**, *italic*, ~~strike~~, [links](url),
//!   # headings, > quotes; the marks dim (designer: every mark dim, the
//!   composer is already pink).
//!
//! [`styles`] colors only the chars the composer shows; a code line is
//! lexed once per (language, state, text) ([`Cache`]).

use crate::editor::byte_at_char;
use crate::syntax::{self, Lang, State, Tok};
use crate::theme;
use ratatui::style::{Modifier, Style};
use std::collections::HashMap;

/// BISE-276: plain ⏎ on a list item continues the list too, like Slack
/// (off: plain ⏎ sends, the newline keys continue the list).
pub(crate) const ENTER_CONTINUES_LISTS: bool = false;

// ---- lists ----

/// A list item's line: `  - [ ] text`, `3. text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
    /// the leading whitespace, in chars
    pub(crate) indent: usize,
    /// a numbered item: its number and delimiter (`.` or `)`)
    pub(crate) num: Option<(u64, char)>,
    /// `-`, `*`, `+`, or the number's delimiter
    pub(crate) bullet: char,
    /// past the marker and its space (a char offset in the line)
    pub(crate) marker_end: usize,
    /// a task item: checked or not
    pub(crate) check: Option<bool>,
    /// where the item's text starts
    pub(crate) content: usize,
}

/// The list item on `line`, if it is one.
pub(crate) fn item(line: &str) -> Option<Item> {
    let cs: Vec<char> = line.chars().collect();
    let indent = cs.iter().take_while(|c| **c == ' ' || **c == '\t').count();
    let mut i = indent;
    let (num, bullet) = match *cs.get(i)? {
        c @ ('-' | '*' | '+') => {
            i += 1;
            (None, c)
        }
        c if c.is_ascii_digit() => {
            let d = cs[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            let delim = *cs.get(i + d)?;
            if d > 9 || !matches!(delim, '.' | ')') {
                return None;
            }
            let n: u64 = cs[i..i + d].iter().collect::<String>().parse().ok()?;
            i += d + 1;
            (Some((n, delim)), delim)
        }
        _ => return None,
    };
    if cs.get(i) != Some(&' ') {
        return None;
    }
    i += 1;
    let marker_end = i;
    let check = match (cs.get(i), cs.get(i + 1), cs.get(i + 2), cs.get(i + 3)) {
        (Some('['), Some(x @ (' ' | 'x' | 'X')), Some(']'), None | Some(' ')) => {
            i += 3 + usize::from(cs.get(i + 3).is_some());
            Some(*x != ' ')
        }
        _ => None,
    };
    Some(Item { indent, num, bullet, marker_end, check, content: i })
}

/// The marker of the item after `it` on `line`: the same indentation,
/// the next number, an unchecked box for a task.
fn next_marker(it: &Item, line: &str) -> String {
    let lead: String = line.chars().take(it.indent).collect();
    let m = match it.num {
        Some((n, d)) => format!("{}{} ", n + 1, d),
        None => format!("{} ", it.bullet),
    };
    format!("{lead}{m}{}", if it.check.is_some() { "[ ] " } else { "" })
}

/// Sets the number of the item on `line` (its marker only).
fn set_number(line: &str, it: &Item, n: u64) -> String {
    let lead: String = line.chars().take(it.indent).collect();
    let d = it.num.map(|x| x.1).unwrap_or('.');
    let rest: String = line.chars().skip(it.marker_end).collect();
    format!("{lead}{n}{d} {rest}")
}

/// Renumbers the numbered items of the list around `lines[at]` (the
/// non-blank lines, up to a fence): each item after a sibling numbered
/// `p` is `p + 1`; the first of a (sub)list keeps its number, or 1 when
/// in `reset` (an item just indented).
fn renumber(lines: &mut [String], at: usize, reset: &[usize]) {
    let stop = |l: &str| l.trim().is_empty() || fence(l).is_some();
    let mut s = at;
    while s > 0 && !stop(&lines[s - 1]) {
        s -= 1;
    }
    let mut e = at;
    while e < lines.len() && !stop(&lines[e]) {
        e += 1;
    }
    // (indent, the number of the last item at that level)
    let mut stack: Vec<(usize, Option<u64>)> = Vec::new();
    for (k, line) in lines.iter_mut().enumerate().take(e).skip(s) {
        let Some(it) = item(line) else { continue };
        while stack.last().is_some_and(|x| x.0 > it.indent) {
            stack.pop();
        }
        let prev = stack.last().filter(|x| x.0 == it.indent).map(|x| x.1);
        let n = match (it.num, prev) {
            (Some(_), Some(Some(p))) => Some(p + 1),
            (Some(_), _) if reset.contains(&k) => Some(1),
            (Some((n0, _)), _) => Some(n0),
            (None, _) => None,
        };
        if let (Some(n), Some((n0, _))) = (n, it.num) {
            if n != n0 {
                *line = set_number(line, &it, n);
            }
        }
        match stack.last_mut() {
            Some(top) if top.0 == it.indent => top.1 = n,
            _ => stack.push((it.indent, n)),
        }
    }
}

// ---- fenced blocks ----

/// A fence line (```, any indentation): its info string (`ts`, ``).
fn fence(line: &str) -> Option<&str> {
    let t = line.trim_start();
    t.starts_with("```").then(|| t.trim_start_matches('`').trim())
}

/// What a line is.
#[derive(Clone, Copy)]
enum Kind {
    Prose,
    /// an opening fence
    Open,
    Close,
    /// a line of a code block
    Code(Option<&'static Lang>),
}

/// Reads the lines in order: a fence opens a block, a bare fence closes
/// it (CommonMark: a fence with a tag inside a block is code).
#[derive(Default)]
struct Walk {
    open: Option<Option<&'static Lang>>,
}

impl Walk {
    fn next(&mut self, line: &str) -> Kind {
        match (self.open, fence(line)) {
            (None, Some(info)) => {
                self.open = Some(syntax::lang_of(info));
                Kind::Open
            }
            (None, None) => Kind::Prose,
            (Some(_), Some("")) => {
                self.open = None;
                Kind::Close
            }
            (Some(l), _) => Kind::Code(l),
        }
    }
}

/// The lines of `text` (split on `\n`) with the char index they start at.
fn lines_ci(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut ci = 0usize;
    text.split('\n').map(move |l| {
        let at = ci;
        ci += l.chars().count() + 1;
        (at, l)
    })
}

/// The line under char `ci`: its index, its first char index, its text
/// and what it is.
fn line_at(text: &str, ci: usize) -> (usize, usize, &str, Kind) {
    let mut walk = Walk::default();
    let mut last = (0, 0, "", Kind::Prose);
    for (k, (at, l)) in lines_ci(text).enumerate() {
        let kind = walk.next(l);
        last = (k, at, l, kind);
        if ci <= at + l.chars().count() {
            break;
        }
    }
    last
}

/// In a code block (its lines or its opening fence), where a newline
/// keeps the indentation and plain ⏎ makes one.
fn in_code(kind: Kind) -> bool {
    matches!(kind, Kind::Open | Kind::Code(_))
}

/// Plain ⏎ at `cursor` makes a newline instead of sending: in a code
/// block, or on a list item when [`ENTER_CONTINUES_LISTS`].
pub(crate) fn enter_makes_newline(text: &str, cursor: usize) -> bool {
    let (_, at, line, kind) = line_at(text, cursor);
    in_code(kind)
        || (ENTER_CONTINUES_LISTS && matches!(kind, Kind::Prose) && item(line).is_some_and(|it| cursor - at >= it.content))
}

// ---- the edits: (new text, cursor, anchor), one undo step ----

/// An edit of the composer: the whole new text, the cursor, the anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    pub(crate) anchor: Option<usize>,
}

fn join(lines: &[String]) -> String {
    lines.join("\n")
}

/// The char index of (line, col).
fn ci_of(lines: &[String], line: usize, col: usize) -> usize {
    lines[..line].iter().map(|l| l.chars().count() + 1).sum::<usize>() + col
}

/// (line, col) of char `ci`.
fn pos_of(lines: &[String], ci: usize) -> (usize, usize) {
    let mut at = 0usize;
    for (k, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if ci <= at + n {
            return (k, ci - at);
        }
        at += n + 1;
    }
    let k = lines.len().saturating_sub(1);
    (k, lines.get(k).map(|l| l.chars().count()).unwrap_or(0))
}

/// A position after `lines[line]` became `new`: a column in or after
/// the item's marker moves with the marker's change of width.
fn shift(old: &str, new: &str, col: usize) -> usize {
    let (Some(a), Some(b)) = (item(old), item(new)) else { return col.min(new.chars().count()) };
    if col >= a.marker_end {
        col + b.marker_end - a.marker_end
    } else if col >= a.indent {
        b.indent + (col - a.indent).min(b.marker_end - b.indent)
    } else {
        col.min(b.indent)
    }
}

/// The newline keys at `cursor` (the selection `a..b` replaced first):
/// in a code block the newline keeps the line's indentation; on a list
/// item it continues the list, or ends it on an empty item. None: a
/// plain newline (the editor types it).
pub(crate) fn newline(text: &str, a: usize, b: usize) -> Option<Edit> {
    let (ba, bb) = (byte_at_char(text, a), byte_at_char(text, b));
    let t = format!("{}{}", &text[..ba], &text[bb..]);
    let (k, at, line, kind) = line_at(&t, a);
    let col = a - at;
    let mut lines: Vec<String> = t.split('\n').map(str::to_string).collect();
    if in_code(kind) {
        let ws: String = line.chars().take(col).take_while(|c| *c == ' ' || *c == '\t').collect();
        let bi = byte_at_char(&t, a);
        let text = format!("{}\n{}{}", &t[..bi], ws, &t[bi..]);
        return Some(Edit { text, cursor: a + 1 + ws.chars().count(), anchor: None });
    }
    if !matches!(kind, Kind::Prose) {
        return None;
    }
    let it = item(line).filter(|it| col >= it.content)?;
    let content: String = line.chars().skip(it.content).collect();
    if content.trim().is_empty() {
        // an empty item: step out a level, or end the list
        if it.indent > 0 {
            return outdent_line(&mut lines, k).map(|_| {
                let col = lines[k].chars().count();
                Edit { cursor: ci_of(&lines, k, col), text: join(&lines), anchor: None }
            });
        }
        lines[k] = String::new();
        renumber(&mut lines, k.saturating_sub(1), &[]);
        return Some(Edit { cursor: ci_of(&lines, k, 0), text: join(&lines), anchor: None });
    }
    let head: String = line.chars().take(col).collect();
    let tail: String = line.chars().skip(col).collect();
    let marker = next_marker(&it, line);
    lines[k] = head;
    lines.insert(k + 1, format!("{marker}{tail}"));
    let (old, mut ccol) = (lines[k + 1].clone(), marker.chars().count());
    if it.num.is_some() {
        renumber(&mut lines, k + 1, &[]);
        ccol = shift(&old, &lines[k + 1], ccol);
    }
    Some(Edit { cursor: ci_of(&lines, k + 1, ccol), text: join(&lines), anchor: None })
}

/// The nearest item above `lines[k]` (in its list: no blank line or
/// fence between) whose indentation passes `keep`.
fn item_above(lines: &[String], k: usize, keep: impl Fn(usize) -> bool) -> Option<Item> {
    lines[..k]
        .iter()
        .rev()
        .take_while(|l| !l.trim().is_empty() && fence(l).is_none())
        .filter_map(|l| item(l))
        .find(|it| keep(it.indent))
}

fn with_indent(line: &str, it: &Item, n: usize) -> String {
    let rest: String = line.chars().skip(it.indent).collect();
    format!("{}{}", " ".repeat(n), rest)
}

/// One level out: the indentation of the item's parent (0 without one).
/// None when the line is not an indented item.
fn outdent_line(lines: &mut [String], k: usize) -> Option<()> {
    let it = item(&lines[k]).filter(|it| it.indent > 0)?;
    let to = item_above(lines, k, |i| i < it.indent).map(|p| p.indent).unwrap_or(0);
    lines[k] = with_indent(&lines[k], &it, to);
    renumber(lines, k, &[k]);
    Some(())
}

/// Tab (`out`: Shift+Tab) on the list items of the lines from `a` to `b`
/// (the cursor and the selection's other end): one level in (under the
/// item above: its marker's width) or out. None when the cursor's line
/// is not a list item (Tab keeps its other meanings).
pub(crate) fn indent(text: &str, cursor: usize, anchor: Option<usize>, out: bool) -> Option<Edit> {
    let (_, _, line, kind) = line_at(text, cursor);
    if !matches!(kind, Kind::Prose) || item(line).is_none() {
        return None;
    }
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let (cl, cc) = pos_of(&lines, cursor);
    let (al, ac) = pos_of(&lines, anchor.unwrap_or(cursor));
    let (first, last) = (cl.min(al), cl.max(al));
    let before = lines.clone();
    let mut moved = Vec::new();
    for k in first..=last {
        let Some(it) = item(&lines[k]) else { continue };
        let to = if out {
            if it.indent == 0 {
                continue;
            }
            item_above(&lines, k, |i| i < it.indent).map(|p| p.indent).unwrap_or(0)
        } else {
            let unit = item_above(&lines, k, |i| i <= it.indent)
                .filter(|p| p.indent == it.indent)
                .map(|p| p.marker_end - p.indent)
                .unwrap_or(it.marker_end - it.indent);
            it.indent + unit
        };
        lines[k] = with_indent(&lines[k], &it, to);
        moved.push(k);
    }
    if let Some(&k) = moved.first() {
        renumber(&mut lines, k, &moved);
    }
    let cursor = ci_of(&lines, cl, shift(&before[cl], &lines[cl], cc));
    let anchor = anchor.map(|_| ci_of(&lines, al, shift(&before[al], &lines[al], ac)));
    Some(Edit { text: join(&lines), cursor, anchor })
}

// ---- the styles ----

/// The lexed code lines: (language, state in, text) → (runs, state out).
#[derive(Default)]
pub(crate) struct Cache {
    lines: HashMap<u64, (Vec<(usize, Tok)>, State)>,
}

impl Cache {
    fn line(&mut self, lang: &'static Lang, st: State, l: &str) -> &(Vec<(usize, Tok)>, State) {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (lang.names[0], st, l).hash(&mut h);
        let key = h.finish();
        if self.lines.len() > 20_000 && !self.lines.contains_key(&key) {
            self.lines.clear();
        }
        self.lines.entry(key).or_insert_with(|| syntax::line(lang, st, l))
    }
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// The style of every char of `text` from `lo` to `hi` (char indices;
/// a newline gets the text's): only these lines are styled, the code
/// lines above them in their block lexed through `cache`.
pub(crate) fn styles(text: &str, lo: usize, hi: usize, cache: &mut Cache) -> Vec<Style> {
    let base = Style::default().fg(theme::text());
    let mut out = vec![base; hi.saturating_sub(lo)];
    let mark = if no_color() { base } else { base.fg(theme::dim()) };
    let mut walk = Walk::default();
    let mut st = State::Normal;
    for (at, l) in lines_ci(text) {
        if at >= hi {
            break;
        }
        let n = l.chars().count();
        let kind = walk.next(l);
        let shown = at + n >= lo;
        let line: Vec<Style> = match kind {
            Kind::Open | Kind::Close => {
                st = State::Normal;
                if !shown {
                    continue;
                }
                vec![mark; n]
            }
            Kind::Code(None) => {
                if !shown {
                    continue;
                }
                vec![base; n]
            }
            Kind::Code(Some(lang)) => {
                let (runs, next) = cache.line(lang, st, l);
                st = *next;
                if !shown {
                    continue;
                }
                runs.iter().flat_map(|&(k, t)| std::iter::repeat_n(syntax::style(t), k)).collect()
            }
            Kind::Prose => {
                if !shown {
                    continue;
                }
                prose(l, base, mark)
            }
        };
        for (i, s) in line.into_iter().enumerate() {
            if let Some(o) = (at + i).checked_sub(lo).and_then(|j| out.get_mut(j)) {
                *o = s;
            }
        }
    }
    out
}

/// The styles of a prose line: quote marks, a heading, a list marker,
/// a rule, then the inline marks.
fn prose(l: &str, base: Style, mark: Style) -> Vec<Style> {
    let cs: Vec<char> = l.chars().collect();
    let n = cs.len();
    let mut st = vec![base; n];
    let mut i = cs.iter().take_while(|c| c.is_whitespace()).count();
    // a rule: 3+ of - * _ alone
    let t = l.trim();
    if t.len() >= 3 && t.chars().filter(|c| !c.is_whitespace()).count() >= 3 {
        if let Some(c) = t.chars().next().filter(|c| matches!(c, '-' | '*' | '_')) {
            if t.chars().all(|x| x == c || x == ' ') {
                return vec![mark; n];
            }
        }
    }
    // > quotes
    while cs.get(i) == Some(&'>') {
        st[i] = mark;
        i += 1;
        while cs.get(i).is_some_and(|c| *c == ' ') {
            i += 1;
        }
    }
    // # heading: the hashes dim, the rest bold
    let h = cs[i..].iter().take_while(|c| **c == '#').count();
    if (1..=6).contains(&h) && cs.get(i + h).is_none_or(|c| *c == ' ') {
        for s in &mut st[i..i + h] {
            *s = mark;
        }
        for s in &mut st[i + h..] {
            *s = s.add_modifier(Modifier::BOLD);
        }
        i += h;
    } else {
        let rest: String = cs[i..].iter().collect();
        if let Some(it) = item(&rest) {
            for s in &mut st[i + it.indent..i + it.content] {
                *s = mark;
            }
            if it.check == Some(true) {
                for s in &mut st[i + it.content..] {
                    *s = mark;
                }
            }
            i += it.content;
        }
    }
    inline(&cs, &mut st, i, mark);
    st
}

/// The inline marks of `cs` from `from`: `code` (its content in the
/// feed's inline code style), [links](url), **bold**, *italic*,
/// ~~strike~~; the marks dim.
fn inline(cs: &[char], st: &mut [Style], from: usize, mark: Style) {
    let n = cs.len();
    let mut taken = vec![false; n];
    let code = if no_color() {
        Style::default().fg(theme::text()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::ok()).add_modifier(Modifier::BOLD)
    };
    // `code` (a run of k backticks closes with k)
    let mut i = from;
    while i < n {
        if cs[i] != '`' {
            i += 1;
            continue;
        }
        let k = cs[i..].iter().take_while(|c| **c == '`').count();
        let close = (i + k..n).find(|&j| {
            cs[j..].iter().take_while(|c| **c == '`').count() == k && (j == 0 || cs[j - 1] != '`')
        });
        match close {
            Some(j) if j > i + k => {
                for x in i..j + k {
                    st[x] = if x < i + k || x >= j { mark } else { code };
                    taken[x] = true;
                }
                i = j + k;
            }
            _ => i += k,
        }
    }
    // [label](url): the brackets and the url dim, the label underlined
    let mut i = from;
    while i < n {
        if cs[i] != '[' || taken[i] {
            i += 1;
            continue;
        }
        let Some(c) = (i + 1..n).find(|&j| cs[j] == ']' || cs[j] == '[') else { break };
        if cs[c] != ']' || cs.get(c + 1) != Some(&'(') || taken[c] {
            i += 1;
            continue;
        }
        let Some(e) = (c + 2..n).find(|&j| cs[j] == ')') else { break };
        st[i] = mark;
        for s in &mut st[i + 1..c] {
            *s = s.add_modifier(Modifier::UNDERLINED);
        }
        for x in c..=e {
            st[x] = mark;
            taken[x] = true;
        }
        taken[i] = true;
        i = e + 1;
    }
    // pairs of marks: **bold** __bold__ ~~strike~~ then *italic* _italic_
    for (m, len, modi) in [
        ('*', 2, Modifier::BOLD),
        ('_', 2, Modifier::BOLD),
        ('~', 2, Modifier::CROSSED_OUT),
        ('*', 1, Modifier::ITALIC),
        ('_', 1, Modifier::ITALIC),
    ] {
        let run = |j: usize| cs[j..].iter().take_while(|c| **c == m).count();
        let free = |taken: &[bool], j: usize| (j..j + len).all(|x| x < n && !taken[x] && cs[x] == m);
        let mut i = from;
        while i < n {
            // an opener: exactly `len` marks (not part of a longer run
            // unless the longer marks were taken), then a non-space
            let at_run = free(&taken, i) && (i == 0 || cs[i - 1] != m || taken[i - 1]) && (run(i) == len || taken.get(i + len) == Some(&true));
            let word_ok = m != '_' || i == 0 || !cs[i - 1].is_alphanumeric();
            if !at_run || !word_ok || cs.get(i + len).is_none_or(|c| c.is_whitespace()) {
                i += 1;
                continue;
            }
            let close = (i + len + 1..n).find(|&j| {
                free(&taken, j)
                    && !cs[j - 1].is_whitespace()
                    && (j + len >= n || cs[j + len] != m || taken[j + len])
                    && (m != '_' || cs.get(j + len).is_none_or(|c| !c.is_alphanumeric()))
            });
            let Some(j) = close else {
                i += len;
                continue;
            };
            for x in (i..i + len).chain(j..j + len) {
                st[x] = mark;
                taken[x] = true;
            }
            for s in &mut st[i + len..j] {
                *s = s.add_modifier(modi);
            }
            i = j + len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Editor;

    /// Applies the newline keys to `s` with the cursor at `|` (what the
    /// composer does: the edit, else a plain newline).
    fn nl(s: &str) -> String {
        let c = s.find('|').map(|b| s[..b].chars().count()).unwrap();
        let t = s.replacen('|', "", 1);
        match newline(&t, c, c) {
            Some(e) => mark(&e.text, e.cursor),
            None => {
                let b = byte_at_char(&t, c);
                format!("{}\n|{}", &t[..b], &t[b..])
            }
        }
    }

    fn mark(t: &str, c: usize) -> String {
        let b = byte_at_char(t, c);
        format!("{}|{}", &t[..b], &t[b..])
    }

    fn tab(s: &str, out: bool) -> String {
        let c = s.find('|').map(|b| s[..b].chars().count()).unwrap();
        let t = s.replacen('|', "", 1);
        let e = indent(&t, c, None, out).expect("a list item");
        mark(&e.text, e.cursor)
    }

    #[test]
    fn items_parse() {
        let it = item("  - [x] done").unwrap();
        assert_eq!((it.indent, it.bullet, it.marker_end, it.check, it.content), (2, '-', 4, Some(true), 8));
        assert_eq!(item("12) x").unwrap().num, Some((12, ')')));
        assert!(item("-x").is_none());
        assert!(item("**bold**").is_none());
        assert!(item("1.5 apples").is_none());
        assert_eq!(item("- ").unwrap().content, 2);
    }

    #[test]
    fn newline_continues_the_list() {
        assert_eq!(nl("- one|"), "- one\n- |");
        assert_eq!(nl("* one|"), "* one\n* |");
        assert_eq!(nl("  + nested|"), "  + nested\n  + |");
        assert_eq!(nl("1. one|"), "1. one\n2. |");
        assert_eq!(nl("9) nine|"), "9) nine\n10) |");
        assert_eq!(nl("- [x] done|"), "- [x] done\n- [ ] |");
        assert_eq!(nl("- [ ] todo|"), "- [ ] todo\n- [ ] |");
        // the text after the cursor moves to the new item
        assert_eq!(nl("- one|two"), "- one\n- |two");
        // the items below renumber
        assert_eq!(nl("1. a|\n2. b\n3. c"), "1. a\n2. |\n3. b\n4. c");
        // before the marker: a plain newline
        assert_eq!(nl("|- one"), "\n|- one");
        assert_eq!(nl("plain|"), "plain\n|");
    }

    #[test]
    fn newline_on_an_empty_item_ends_the_list() {
        assert_eq!(nl("- one\n- |"), "- one\n|");
        assert_eq!(nl("1. one\n2. |"), "1. one\n|");
        assert_eq!(nl("- [ ] a\n- [ ] |"), "- [ ] a\n|");
        // an indented empty item steps out one level (and renumbers)
        assert_eq!(nl("1. a\n   - |"), "1. a\n- |");
        assert_eq!(nl("1. a\n   1. b\n   2. |"), "1. a\n   1. b\n2. |");
    }

    #[test]
    fn tab_indents_and_outdents() {
        assert_eq!(tab("- a\n- b|", false), "- a\n  - b|");
        assert_eq!(tab("- a\n  - b|", true), "- a\n- b|");
        // numbered: under the parent's text, restarts at 1; out, counts on
        assert_eq!(tab("1. a\n2. b|\n3. c", false), "1. a\n   1. b|\n2. c");
        assert_eq!(tab("1. a\n   1. b|\n2. c", true), "1. a\n2. b|\n3. c");
        // the cursor keeps its place in the text
        assert_eq!(tab("- a\n- b|c", false), "- a\n  - b|c");
        // top level: out does nothing
        assert_eq!(tab("- a|", true), "- a|");
        // not a list item, or in code: not ours
        assert!(indent("plain", 2, None, false).is_none());
        assert!(indent("```\n- a\n```", 6, None, false).is_none());
    }

    #[test]
    fn tab_on_a_selection_indents_its_items() {
        let t = "- a\n- b\n- c";
        let e = indent(t, 11, Some(4), false).unwrap();
        assert_eq!(e.text, "- a\n  - b\n  - c");
        assert_eq!((e.anchor, e.cursor), (Some(6), 15));
    }

    #[test]
    fn code_blocks_keep_indentation_and_never_continue_lists() {
        assert_eq!(nl("```ts\n  if (x) {|"), "```ts\n  if (x) {\n  |");
        assert_eq!(nl("```\n- item|\n```"), "```\n- item\n|\n```");
        assert_eq!(nl("```ts|"), "```ts\n|");
        // after the block: prose again
        assert_eq!(nl("```\nx\n```\n- a|"), "```\nx\n```\n- a\n- |");
        assert!(enter_makes_newline("```ts\nlet x", 12));
        assert!(enter_makes_newline("```ts", 5));
        assert!(!enter_makes_newline("```ts\nlet x\n```", 15));
        assert!(!enter_makes_newline("- item", 6));
    }

    #[test]
    fn undo_takes_the_marker_with_the_newline() {
        let mut ed = Editor::default();
        ed.insert("- one");
        let e = newline(&ed.text, ed.cursor, ed.cursor).unwrap();
        ed.set(&e.text, e.cursor);
        assert_eq!(ed.text, "- one\n- ");
        ed.insert("t");
        ed.insert("w");
        assert_eq!(ed.text, "- one\n- tw");
        ed.undo();
        assert_eq!(ed.text, "- one\n- ");
        ed.undo();
        assert_eq!((ed.text.as_str(), ed.cursor), ("- one", 5));
    }

    fn fg_of(text: &str, word: &str) -> Style {
        let b = text.find(word).unwrap();
        let ci = text[..b].chars().count();
        let n = text.chars().count();
        styles(text, 0, n, &mut Cache::default())[ci]
    }

    #[test]
    fn styles_mark_dim_content_styled() {
        let dim = theme::dim();
        let t = "# Title **b** *i* `c` [l](u)\n- [x] done\n1. first";
        assert_eq!(fg_of(t, "#").fg, Some(dim));
        assert!(fg_of(t, "Title").add_modifier.contains(Modifier::BOLD));
        assert_eq!(fg_of(t, "**").fg, Some(dim));
        assert!(fg_of(t, "b*").add_modifier.contains(Modifier::BOLD));
        assert!(fg_of(t, "i*").add_modifier.contains(Modifier::ITALIC));
        assert_eq!(fg_of(t, "`").fg, Some(dim));
        assert_eq!(fg_of(t, "c`").fg, Some(theme::ok()));
        assert!(fg_of(t, "l]").add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(fg_of(t, "(u)").fg, Some(dim));
        assert_eq!(fg_of(t, "- [x]").fg, Some(dim));
        assert_eq!(fg_of(t, "done").fg, Some(dim));
        assert_eq!(fg_of(t, "1.").fg, Some(dim));
        assert_eq!(fg_of(t, "first").fg, Some(theme::text()));
        // a lone * or snake_case is not a mark
        assert_eq!(fg_of("a * b snake_case_x", "*").fg, Some(theme::text()));
        assert!(!fg_of("a * b snake_case_x", "case").add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn styles_color_code_blocks() {
        let t = "```ts\nconst x = load(1) // c\n```\nconst";
        assert_eq!(fg_of(t, "```ts").fg, Some(theme::dim()));
        assert_eq!(fg_of(t, "const").fg, Some(theme::syntax_keyword()));
        assert_eq!(fg_of(t, "load").fg, Some(theme::syntax_call()));
        assert_eq!(fg_of(t, "1)").fg, Some(theme::syntax_number()));
        assert_eq!(fg_of(t, "// c").fg, Some(theme::syntax_comment()));
        // after the block, prose: `const` is text
        let last = t.rfind("const").unwrap();
        let n = t.chars().count();
        assert_eq!(styles(t, 0, n, &mut Cache::default())[last].fg, Some(theme::text()));
        // an unknown language: plain text
        assert_eq!(fg_of("```brainfuck\nconst\n```", "const").fg, Some(theme::text()));
    }

    #[test]
    fn styles_of_a_window_match_the_whole() {
        // a block comment opened above the window still colors in it
        let t = "```rust\n/* a\nb\nc */ fn\n```";
        let n = t.chars().count();
        let all = styles(t, 0, n, &mut Cache::default());
        let lo = t.find("c */").unwrap();
        let part = styles(t, lo, n, &mut Cache::default());
        assert_eq!(&all[lo..], &part[..]);
        assert_eq!(part[0].fg, Some(theme::syntax_comment()));
    }
    /// Typing latency in a 2,000-line draft (a ```ts block): the frame
    /// with the live markdown, and the styling alone (the frame before
    /// BISE-276 is the difference). `cargo test --release -p bend-tui
    /// bench_md_typing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_md_typing() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        use std::time::Instant;
        let mut body = String::from("```ts\n");
        for i in 0..2000 {
            body.push_str(&format!("  const x{i} = await load(\"item {i}\", {i}); // line {i}\n"));
        }
        let mut app = crate::sb::bench::test_app();
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        app.ed.set(&body, body.chars().count());
        term.draw(|f| crate::sb::draw_sb(&mut app, f)).unwrap();
        let keys = 200;
        let t = Instant::now();
        for _ in 0..keys {
            app.ed.insert("a");
            term.draw(|f| crate::sb::draw_sb(&mut app, f)).unwrap();
        }
        let frame = t.elapsed() / keys;
        let n = app.ed.text.chars().count();
        let t = Instant::now();
        for _ in 0..keys {
            std::hint::black_box(styles(&app.ed.text, n - 900, n, &mut app.md_cache));
        }
        let md = t.elapsed() / keys;
        // the cold case: a new text (a paste, a recall), no line cached
        let t = Instant::now();
        std::hint::black_box(styles(&app.ed.text, n - 900, n, &mut Cache::default()));
        println!("per key: frame {frame:?}, of which live markdown {md:?}; cold styling of 2,000 lines {:?}", t.elapsed());
    }
}
