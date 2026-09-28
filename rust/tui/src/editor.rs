//! The composer's editing model: the text, a cursor and a selection on
//! grapheme boundaries, undo/redo, and the history recall that keeps the
//! draft. The key map (`action`) turns terminal key events into editing
//! actions; the popups, sending and voice live in lib.rs and call in.
//!
//! Positions are char indices into `text` (never inside a grapheme: an
//! emoji with a ZWJ, a skin tone or a variation selector is one step).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// ---- grapheme helpers ----

/// Byte offset of the char at index `ci` (the text end past the last).
pub(crate) fn byte_at_char(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// The char index of the grapheme before the one at `cursor`.
pub(crate) fn prev_grapheme(s: &str, cursor: usize) -> usize {
    let mut ci = 0usize;
    let mut prev = 0usize;
    for g in s.graphemes(true) {
        if ci >= cursor {
            break;
        }
        prev = ci;
        ci += g.chars().count();
    }
    prev
}

/// The char index of the grapheme after the one at `cursor`.
pub(crate) fn next_grapheme(s: &str, cursor: usize) -> usize {
    let mut ci = 0usize;
    for g in s.graphemes(true) {
        ci += g.chars().count();
        if ci > cursor {
            return ci;
        }
    }
    ci
}

/// The graphemes of `s` with their first char index.
fn graphemes_ci(s: &str) -> Vec<(usize, &str)> {
    let mut ci = 0usize;
    s.graphemes(true)
        .map(|g| {
            let at = ci;
            ci += g.chars().count();
            (at, g)
        })
        .collect()
}

fn is_word(g: &str) -> bool {
    g.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// macOS Option+←: skip the non-word graphemes before the cursor, then
/// the word before them.
pub(crate) fn word_left(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i > 0 && !is_word(gs[i - 1].1) {
        i -= 1;
    }
    while i > 0 && is_word(gs[i - 1].1) {
        i -= 1;
    }
    gs.get(i).map(|g| g.0).unwrap_or(0)
}

/// macOS Option+→: skip the non-word graphemes after the cursor, then
/// the word after them (the cursor lands at the word's end).
pub(crate) fn word_right(s: &str, cursor: usize) -> usize {
    let gs = graphemes_ci(s);
    let end = s.chars().count();
    let mut i = gs.iter().position(|&(ci, _)| ci >= cursor).unwrap_or(gs.len());
    while i < gs.len() && !is_word(gs[i].1) {
        i += 1;
    }
    while i < gs.len() && is_word(gs[i].1) {
        i += 1;
    }
    gs.get(i).map(|g| g.0).unwrap_or(end)
}

/// Start of the (newline-separated) line holding `cursor`.
pub(crate) fn line_start(s: &str, cursor: usize) -> usize {
    let head: Vec<char> = s.chars().take(cursor).collect();
    head.iter().rposition(|&c| c == '\n').map(|p| p + 1).unwrap_or(0)
}

/// End of the (newline-separated) line holding `cursor` (before its '\n').
pub(crate) fn line_end(s: &str, cursor: usize) -> usize {
    let mut ci = cursor;
    for c in s.chars().skip(cursor) {
        if c == '\n' {
            break;
        }
        ci += 1;
    }
    ci
}

/// The word (or the run of non-word graphemes) around `ci`, for a
/// double click.
pub(crate) fn word_at(s: &str, ci: usize) -> (usize, usize) {
    let gs = graphemes_ci(s);
    let Some(i) = gs.iter().rposition(|&(c, _)| c <= ci) else {
        return (0, 0);
    };
    if gs[i].1 == "\n" {
        return (gs[i].0, gs[i].0);
    }
    let kind = is_word(gs[i].1);
    let same = |g: &str| is_word(g) == kind && g != "\n" && (kind || !g.chars().all(char::is_whitespace) == !gs[i].1.chars().all(char::is_whitespace));
    let mut a = i;
    while a > 0 && same(gs[a - 1].1) {
        a -= 1;
    }
    let mut b = i + 1;
    while b < gs.len() && same(gs[b].1) {
        b += 1;
    }
    (gs[a].0, gs.get(b).map(|g| g.0).unwrap_or(s.chars().count()))
}

// ---- the wrapped layout ----

/// One cell of the composer layout: a grapheme, its first char index,
/// its width in columns. A newline is a 1-column slot shown only when the
/// cursor is on it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InputCell<'a> {
    pub(crate) ci: usize,
    pub(crate) text: &'a str,
    pub(crate) w: usize,
    pub(crate) newline: bool,
}

/// The composer rows at `inner` columns: newlines break rows, long rows
/// wrap by width (the widths ratatui uses, so a 2-column emoji never
/// overflows the row), and the end of the text gets a 1-column cursor
/// slot.
pub(crate) fn layout_input(input: &str, inner: usize) -> Vec<Vec<InputCell<'_>>> {
    let inner = inner.max(2);
    let mut rows: Vec<Vec<InputCell>> = vec![Vec::new()];
    let mut col = 0usize;
    let mut ci = 0usize;
    for g in input.graphemes(true) {
        let n = g.chars().count();
        if g == "\n" || g == "\r\n" {
            rows.last_mut().unwrap().push(InputCell { ci, text: " ", w: 1, newline: true });
            rows.push(Vec::new());
            col = 0;
            ci += n;
            continue;
        }
        // a control char (a pasted tab) shows as one blank column
        let (text, w) = if g.chars().any(char::is_control) { (" ", 1) } else { (g, g.width().max(1)) };
        if col > 0 && col + w > inner {
            rows.push(Vec::new());
            col = 0;
        }
        rows.last_mut().unwrap().push(InputCell { ci, text, w, newline: false });
        col += w;
        if col >= inner {
            rows.push(Vec::new());
            col = 0;
        }
        ci += n;
    }
    rows.last_mut().unwrap().push(InputCell { ci, text: " ", w: 1, newline: true });
    rows
}

/// (row, column) of the char index `ci` in the layout.
pub(crate) fn row_col(rows: &[Vec<InputCell>], ci: usize) -> (usize, usize) {
    let mut last = (0, 0);
    for (r, row) in rows.iter().enumerate() {
        let mut col = 0;
        for c in row {
            if c.ci == ci {
                return (r, col);
            }
            if c.ci > ci {
                return last;
            }
            last = (r, col);
            col += c.w;
        }
    }
    last
}

/// The char index at (row, column): the cell covering the column, or the
/// row's last cell when the column is past it.
pub(crate) fn ci_at(rows: &[Vec<InputCell>], row: usize, col: usize) -> usize {
    let Some(cells) = rows.get(row.min(rows.len().saturating_sub(1))) else {
        return 0;
    };
    let mut x = 0;
    for c in cells {
        if col < x + c.w {
            // the right half of a wide grapheme: after it, like a text field
            return if col > x && c.w > 1 && !c.newline && col >= x + c.w / 2 + c.w % 2 {
                c.ci + c.text.chars().count()
            } else {
                c.ci
            };
        }
        x += c.w;
    }
    cells.last().map(|c| c.ci).unwrap_or(0)
}

// ---- the editor ----

/// What an undo step groups: consecutive edits of one kind merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Typing,
    Deleting,
    Voice,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
struct Snap {
    text: String,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    TextStart,
    TextEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Grapheme,
    Word,
    /// to the line start (backward) or the line end (forward)
    Line,
}

/// An editing action, the output of the key map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Move(Motion, bool),
    /// Up/Down: a row move, the history at the edges (the caller decides:
    /// popups first). `true` = extend the selection (no history).
    Up(bool),
    Down(bool),
    DeleteBack(Unit),
    DeleteForward(Unit),
    Insert(String),
    Undo,
    Redo,
    SelectAll,
    Copy,
    Cut,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Editor {
    pub(crate) text: String,
    /// char index, on a grapheme boundary
    pub(crate) cursor: usize,
    /// the other end of the selection (none when equal to the cursor)
    pub(crate) anchor: Option<usize>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Option<Kind>,
    /// history browsing: the entry shown (0 = newest), the draft saved on
    /// the first Up, and the edits made to recalled entries
    hist_idx: Option<usize>,
    draft: Option<Snap>,
    scratch: std::collections::HashMap<usize, String>,
}

impl Editor {
    pub(crate) fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The selected range, if not empty.
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        Some(self.text.chars().skip(a).take(b - a).collect())
    }

    pub(crate) fn browsing(&self) -> bool {
        self.hist_idx.is_some()
    }

    fn snap(&self) -> Snap {
        Snap { text: self.text.clone(), cursor: self.cursor }
    }

    /// Records the state before an edit of `kind` (merged with the edit
    /// before when of the same kind; typing breaks at word starts).
    fn checkpoint(&mut self, kind: Kind, word_start: bool) {
        let merge = self.last == Some(kind) && kind != Kind::Other && !word_start;
        if !merge {
            self.undo.push(self.snap());
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some(kind);
    }

    /// Ends the current undo group (a move, a pause in the voice).
    pub(crate) fn break_undo(&mut self) {
        self.last = None;
    }

    fn replace(&mut self, a: usize, b: usize, s: &str) {
        let (ba, bb) = (byte_at_char(&self.text, a), byte_at_char(&self.text, b));
        self.text.replace_range(ba..bb, s);
        self.cursor = a + s.chars().count();
        self.anchor = None;
    }

    fn insert_kind(&mut self, s: &str, kind: Kind) {
        if s.is_empty() {
            return;
        }
        let (a, b) = self.selection().unwrap_or((self.cursor, self.cursor));
        // a new word starts an undo step ("hello world" undoes by word)
        let word_start = kind == Kind::Typing
            && a > 0
            && !s.starts_with(char::is_whitespace)
            && self.text.chars().nth(a - 1).is_some_and(char::is_whitespace);
        self.checkpoint(if a != b { Kind::Other } else { kind }, word_start);
        self.replace(a, b, s);
        if a != b {
            self.last = Some(kind);
        }
    }

    /// Typed text at the cursor (replaces the selection).
    pub(crate) fn insert(&mut self, s: &str) {
        self.insert_kind(s, Kind::Typing);
    }

    /// A paste: its own undo step.
    pub(crate) fn paste(&mut self, s: &str) {
        self.break_undo();
        self.insert_kind(s, Kind::Other);
        self.break_undo();
    }

    /// Voice deltas: consecutive ones undo together.
    pub(crate) fn insert_voice(&mut self, s: &str) {
        self.insert_kind(s, Kind::Voice);
    }

    /// Replaces the whole text (a popup completion), one undo step.
    pub(crate) fn set(&mut self, s: &str, cursor: usize) {
        if s == self.text {
            self.cursor = cursor.min(self.len());
            self.anchor = None;
            return;
        }
        self.checkpoint(Kind::Other, false);
        self.text = s.to_string();
        self.cursor = cursor.min(self.len());
        self.anchor = None;
    }

    /// Empties the composer (Esc on a popup): undoable.
    pub(crate) fn clear(&mut self) {
        self.set("", 0);
    }

    /// Takes the text to send: the composer, the undo and the history
    /// browsing all start over.
    pub(crate) fn take(&mut self) -> String {
        let t = std::mem::take(&mut self.text);
        *self = Editor::default();
        t
    }

    fn target(&self, m: Motion) -> usize {
        let (s, c) = (&self.text, self.cursor);
        match m {
            Motion::Left => prev_grapheme(s, c),
            Motion::Right => next_grapheme(s, c),
            Motion::WordLeft => word_left(s, c),
            Motion::WordRight => word_right(s, c),
            Motion::LineStart => line_start(s, c),
            Motion::LineEnd => line_end(s, c),
            Motion::TextStart => 0,
            Motion::TextEnd => self.len(),
        }
    }

    fn move_to(&mut self, to: usize, select: bool) {
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = to;
        self.break_undo();
    }

    pub(crate) fn move_cursor(&mut self, m: Motion, select: bool) {
        // ←/→ on a selection collapse it to its edge, like a text field
        if let (false, Some((a, b))) = (select, self.selection()) {
            match m {
                Motion::Left => return self.move_to(a, false),
                Motion::Right => return self.move_to(b, false),
                _ => {}
            }
        }
        let to = self.target(m);
        self.move_to(to, select);
    }

    /// Places the cursor (a click), or extends the selection to `ci`
    /// (a drag / shift-click).
    pub(crate) fn click(&mut self, ci: usize, select: bool) {
        let ci = ci.min(self.len());
        self.move_to(ci, select);
    }

    pub(crate) fn select_range(&mut self, a: usize, b: usize) {
        let n = self.len();
        self.anchor = Some(a.min(n));
        self.cursor = b.min(n);
        self.break_undo();
    }

    pub(crate) fn select_all(&mut self) {
        self.select_range(0, self.len());
    }

    /// Up one visual row at `width` columns. False on the first row (the
    /// caller recalls the history); the cursor then goes to the text
    /// start when there is no history to show.
    pub(crate) fn row_up(&mut self, width: usize, select: bool) -> bool {
        let rows = layout_input(&self.text, width);
        let (r, col) = row_col(&rows, self.cursor);
        if r == 0 {
            return false;
        }
        let to = ci_at(&rows, r - 1, col);
        self.move_to(to, select);
        true
    }

    /// Down one visual row. False on the last row.
    pub(crate) fn row_down(&mut self, width: usize, select: bool) -> bool {
        let rows = layout_input(&self.text, width);
        let (r, col) = row_col(&rows, self.cursor);
        if r + 1 >= rows.len() {
            return false;
        }
        let to = ci_at(&rows, r + 1, col);
        self.move_to(to, select);
        true
    }

    pub(crate) fn delete_back(&mut self, u: Unit) {
        if let Some((a, b)) = self.selection() {
            self.checkpoint(Kind::Other, false);
            return self.replace(a, b, "");
        }
        let c = self.cursor;
        let a = match u {
            Unit::Grapheme => prev_grapheme(&self.text, c),
            Unit::Word => word_left(&self.text, c),
            // at a line start, joins the line above
            Unit::Line => match line_start(&self.text, c) {
                s if s == c => prev_grapheme(&self.text, c),
                s => s,
            },
        };
        if a < c {
            self.checkpoint(if u == Unit::Grapheme { Kind::Deleting } else { Kind::Other }, false);
            self.replace(a, c, "");
        }
    }

    pub(crate) fn delete_forward(&mut self, u: Unit) {
        if let Some((a, b)) = self.selection() {
            self.checkpoint(Kind::Other, false);
            return self.replace(a, b, "");
        }
        let c = self.cursor;
        let b = match u {
            Unit::Grapheme => next_grapheme(&self.text, c),
            Unit::Word => word_right(&self.text, c),
            Unit::Line => match line_end(&self.text, c) {
                e if e == c => next_grapheme(&self.text, c),
                e => e,
            },
        };
        if b > c {
            self.checkpoint(if u == Unit::Grapheme { Kind::Deleting } else { Kind::Other }, false);
            self.replace(c, b, "");
            self.cursor = c;
        }
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(self.snap());
        self.restore(s);
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(self.snap());
        self.restore(s);
        true
    }

    fn restore(&mut self, s: Snap) {
        self.text = s.text;
        self.cursor = s.cursor.min(self.len());
        self.anchor = None;
        self.last = None;
    }

    /// Up on the first row: the next older history entry (`history[0]` is
    /// the newest). The draft is saved on the first step, the edits made
    /// to a recalled entry are kept while browsing. False when there is
    /// nothing older.
    pub(crate) fn history_up(&mut self, history: &[String]) -> bool {
        let next = self.hist_idx.map_or(0, |i| i + 1);
        if next >= history.len() {
            return false;
        }
        match self.hist_idx {
            None => self.draft = Some(self.snap()),
            Some(i) => self.keep_scratch(history, i),
        }
        self.show_entry(history, next);
        true
    }

    /// Down on the last row while browsing: the next newer entry, then
    /// the saved draft as it was. False when not browsing.
    pub(crate) fn history_down(&mut self, history: &[String]) -> bool {
        let Some(i) = self.hist_idx else { return false };
        self.keep_scratch(history, i);
        if i == 0 {
            let d = self.draft.take().unwrap_or(Snap { text: String::new(), cursor: 0 });
            self.hist_idx = None;
            self.scratch.clear();
            self.text = d.text;
            self.cursor = d.cursor.min(self.len());
            self.anchor = None;
            self.last = None;
        } else {
            self.show_entry(history, i - 1);
        }
        true
    }

    fn keep_scratch(&mut self, history: &[String], i: usize) {
        if history.get(i) != Some(&self.text) {
            self.scratch.insert(i, self.text.clone());
        } else {
            self.scratch.remove(&i);
        }
    }

    fn show_entry(&mut self, history: &[String], i: usize) {
        self.hist_idx = Some(i);
        self.text = self.scratch.get(&i).cloned().unwrap_or_else(|| history[i].clone());
        self.cursor = self.len();
        self.anchor = None;
        self.last = None;
    }

    /// Applies an editing action. `Up`/`Down`/`Copy`/`Cut` need the
    /// caller (layout width, history, clipboard): they return false here.
    pub(crate) fn apply(&mut self, a: &Action) -> bool {
        match a {
            Action::Move(m, sel) => self.move_cursor(*m, *sel),
            Action::DeleteBack(u) => self.delete_back(*u),
            Action::DeleteForward(u) => self.delete_forward(*u),
            Action::Insert(s) => self.insert(s),
            Action::Undo => {
                self.undo();
            }
            Action::Redo => {
                self.redo();
            }
            Action::SelectAll => self.select_all(),
            Action::Up(_) | Action::Down(_) | Action::Copy | Action::Cut => return false,
        }
        true
    }

    /// Cut: the selected text, removed (one undo step).
    pub(crate) fn cut(&mut self) -> Option<String> {
        let t = self.selected_text()?;
        self.delete_back(Unit::Grapheme);
        Some(t)
    }
}

// ---- the key map ----

/// The editing action of a key, macOS text-field style. What Ghostty
/// sends by default: Option+←/→ = ESC b / ESC f (Alt+b/f), Cmd+←/→ =
/// Ctrl+A / Ctrl+E, Cmd+Backspace = Ctrl+U; Cmd+↑/↓, Cmd+A/C/Z are its
/// own unless unbound (then they arrive with SUPER under the kitty
/// keyboard protocol).
pub(crate) fn action(k: &KeyEvent) -> Option<Action> {
    use Action::*;
    use Motion::*;
    let m = k.modifiers;
    let shift = m.contains(KeyModifiers::SHIFT);
    let alt = m.contains(KeyModifiers::ALT);
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let sup = m.contains(KeyModifiers::SUPER);
    let plain = !alt && !ctrl && !sup;
    Some(match k.code {
        KeyCode::Left | KeyCode::Right => {
            let right = k.code == KeyCode::Right;
            let mo = if sup {
                if right { LineEnd } else { LineStart }
            } else if alt || ctrl {
                if right { WordRight } else { WordLeft }
            } else if right {
                Right
            } else {
                Left
            };
            Move(mo, shift)
        }
        KeyCode::Up if sup || ctrl => Move(TextStart, shift),
        KeyCode::Down if sup || ctrl => Move(TextEnd, shift),
        KeyCode::Up if plain => Up(shift),
        KeyCode::Down if plain => Down(shift),
        KeyCode::Home if ctrl || sup => Move(TextStart, shift),
        KeyCode::End if ctrl || sup => Move(TextEnd, shift),
        KeyCode::Home => Move(LineStart, shift),
        KeyCode::End => Move(LineEnd, shift),
        KeyCode::Backspace if sup => DeleteBack(Unit::Line),
        KeyCode::Backspace if alt || ctrl => DeleteBack(Unit::Word),
        KeyCode::Backspace => DeleteBack(Unit::Grapheme),
        KeyCode::Delete if sup => DeleteForward(Unit::Line),
        KeyCode::Delete if alt || ctrl => DeleteForward(Unit::Word),
        KeyCode::Delete => DeleteForward(Unit::Grapheme),
        KeyCode::Char(c) if ctrl && !alt && !sup => match c.to_ascii_lowercase() {
            'a' => Move(LineStart, shift),
            'e' => Move(LineEnd, shift),
            'b' if !shift => Move(Left, false),
            'f' if !shift => Move(Right, false),
            'u' => DeleteBack(Unit::Line),
            'k' => DeleteForward(Unit::Line),
            'w' => DeleteBack(Unit::Word),
            'h' => DeleteBack(Unit::Grapheme),
            'd' => DeleteForward(Unit::Grapheme),
            'c' if shift => Copy,
            'x' if shift => Cut,
            // Ctrl+/ (kitty) = Ctrl+_ = 0x1F, which the legacy parser
            // reads as Ctrl+7; with Shift (Ctrl+?) it redoes
            '/' | '_' | '7' if !shift => Undo,
            '/' | '?' | '_' => Redo,
            _ => return None,
        },
        KeyCode::Char(c) if alt && !ctrl && !sup => match c {
            'b' => Move(WordLeft, false),
            'f' => Move(WordRight, false),
            'B' => Move(WordLeft, true),
            'F' => Move(WordRight, true),
            'd' => DeleteForward(Unit::Word),
            '/' => Redo,
            _ => return None,
        },
        KeyCode::Char(c) if sup && !ctrl && !alt => match c.to_ascii_lowercase() {
            'z' if shift => Redo,
            'z' => Undo,
            'a' => SelectAll,
            'c' => Copy,
            'x' => Cut,
            _ => return None,
        },
        KeyCode::Char(c) if plain => Insert(c.to_string()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str, cursor: usize) -> Editor {
        Editor { text: text.into(), cursor, ..Default::default() }
    }

    fn key(code: KeyCode, m: KeyModifiers) -> Option<Action> {
        action(&KeyEvent::new(code, m))
    }

    #[test]
    fn word_moves_skip_punctuation_and_emojis() {
        let s = "hello, wörld 👍🏽 foo_bar";
        let mut stops = vec![s.chars().count()];
        loop {
            let c = word_left(s, *stops.last().unwrap());
            if c == *stops.last().unwrap() {
                break;
            }
            stops.push(c);
        }
        assert_eq!(stops, vec![23, 16, 7, 0]);
        let mut stops = vec![0];
        loop {
            let c = word_right(s, *stops.last().unwrap());
            if c == *stops.last().unwrap() {
                break;
            }
            stops.push(c);
        }
        assert_eq!(stops, vec![0, 5, 12, 23]);
    }

    #[test]
    fn line_bounds_are_logical_lines() {
        let s = "ab\ncde\n";
        assert_eq!((line_start(s, 4), line_end(s, 4)), (3, 6));
        assert_eq!((line_start(s, 7), line_end(s, 7)), (7, 7));
        assert_eq!((line_start(s, 0), line_end(s, 0)), (0, 2));
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut e = ed("hello world", 11);
        e.move_cursor(Motion::WordLeft, true);
        assert_eq!(e.selected_text().as_deref(), Some("world"));
        e.insert("there");
        assert_eq!(e.text, "hello there");
        assert_eq!(e.selection(), None);
        e.move_cursor(Motion::LineStart, true);
        e.delete_back(Unit::Grapheme);
        assert_eq!((e.text.as_str(), e.cursor), ("", 0));
    }

    #[test]
    fn arrows_collapse_a_selection_to_its_edge() {
        let mut e = ed("abcdef", 2);
        e.move_cursor(Motion::Right, true);
        e.move_cursor(Motion::Right, true);
        assert_eq!(e.selection(), Some((2, 4)));
        e.move_cursor(Motion::Left, false);
        assert_eq!((e.cursor, e.selection()), (2, None));
    }

    #[test]
    fn selection_is_grapheme_aware() {
        let mut e = ed("a👨‍👩‍👧b", 1);
        e.move_cursor(Motion::Right, true);
        assert_eq!(e.selected_text().as_deref(), Some("👨‍👩‍👧"));
        assert_eq!(e.cut().as_deref(), Some("👨‍👩‍👧"));
        assert_eq!(e.text, "ab");
    }

    #[test]
    fn delete_units() {
        let mut e = ed("one two three", 13);
        e.delete_back(Unit::Word);
        assert_eq!(e.text, "one two ");
        e.delete_back(Unit::Line);
        assert_eq!(e.text, "");
        let mut e = ed("ab\ncd", 3);
        e.delete_back(Unit::Line); // at a line start: joins
        assert_eq!(e.text, "abcd");
        let mut e = ed("ab cd\nef", 1);
        e.delete_forward(Unit::Line);
        assert_eq!((e.text.as_str(), e.cursor), ("a\nef", 1));
        e.delete_forward(Unit::Line); // at a line end: joins
        assert_eq!(e.text, "aef");
    }

    #[test]
    fn undo_groups_words_and_redo_replays() {
        let mut e = Editor::default();
        for c in "hello world".chars() {
            e.insert(&c.to_string());
        }
        e.undo();
        assert_eq!(e.text, "hello ");
        e.undo();
        assert_eq!(e.text, "");
        assert!(!e.undo());
        e.redo();
        e.redo();
        assert_eq!(e.text, "hello world");
        // backspaces group; a move ends the group
        e.delete_back(Unit::Grapheme);
        e.delete_back(Unit::Grapheme);
        e.move_cursor(Motion::Left, false);
        e.delete_back(Unit::Grapheme);
        e.undo();
        assert_eq!(e.text, "hello wor");
        e.undo();
        assert_eq!(e.text, "hello world");
        // a new edit drops the redo
        e.insert("!");
        assert!(!e.redo());
    }

    #[test]
    fn voice_deltas_undo_together() {
        let mut e = ed("say: ", 5);
        for d in ["hello ", "there ", "friend"] {
            e.insert_voice(d);
        }
        e.undo();
        assert_eq!(e.text, "say: ");
    }

    #[test]
    fn up_past_history_and_down_restores_the_draft() {
        let hist = vec!["newest".to_string(), "older".to_string()];
        let mut e = ed("my draft", 3);
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "newest");
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "older");
        assert!(!e.history_up(&hist));
        // an edit to a recalled entry survives browsing
        e.insert("!");
        assert!(e.history_down(&hist));
        assert_eq!(e.text, "newest");
        assert!(e.history_up(&hist));
        assert_eq!(e.text, "older!");
        assert!(e.history_down(&hist));
        assert!(e.history_down(&hist));
        assert_eq!((e.text.as_str(), e.cursor), ("my draft", 3));
        assert!(!e.browsing());
        assert!(!e.history_down(&hist));
        // an empty draft comes back empty
        let mut e = Editor::default();
        e.history_up(&hist);
        e.history_down(&hist);
        assert_eq!(e.text, "");
    }

    #[test]
    fn rows_move_across_wraps_before_the_history() {
        // 10 columns: "aaaa bbbb cccc" wraps after 10 chars
        let mut e = ed("aaaa bbbb cccc", 14);
        assert!(e.row_up(10, false));
        assert_eq!(e.cursor, 4);
        assert!(!e.row_up(10, false));
        assert!(e.row_down(10, false));
        assert_eq!(e.cursor, 14);
        assert!(!e.row_down(10, false));
        // emojis keep the display column
        let mut e = ed("👏👏x\nabcdef", 2);
        assert!(e.row_down(40, false));
        assert_eq!(e.cursor, 8);
        assert!(e.row_up(40, false));
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn double_click_word() {
        let s = "say hello, world";
        assert_eq!(word_at(s, 5), (4, 9));
        assert_eq!(word_at(s, 9), (9, 10));
        assert_eq!(word_at(s, 3), (3, 4));
    }

    #[test]
    fn ghostty_default_encodings() {
        use KeyCode::*;
        let (n, s, a, c, sup) =
            (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::CONTROL, KeyModifiers::SUPER);
        // Option+←/→ = ESC b / ESC f
        assert_eq!(key(Char('b'), a), Some(Action::Move(Motion::WordLeft, false)));
        assert_eq!(key(Char('f'), a), Some(Action::Move(Motion::WordRight, false)));
        assert_eq!(key(Left, a), Some(Action::Move(Motion::WordLeft, false)));
        // Cmd+←/→ = Ctrl+A / Ctrl+E, Cmd+Backspace = Ctrl+U
        assert_eq!(key(Char('a'), c), Some(Action::Move(Motion::LineStart, false)));
        assert_eq!(key(Char('e'), c), Some(Action::Move(Motion::LineEnd, false)));
        assert_eq!(key(Char('u'), c), Some(Action::DeleteBack(Unit::Line)));
        // kitty protocol: Cmd arrives as SUPER
        assert_eq!(key(Left, sup | s), Some(Action::Move(Motion::LineStart, true)));
        assert_eq!(key(Up, sup), Some(Action::Move(Motion::TextStart, false)));
        assert_eq!(key(Char('z'), sup | s), Some(Action::Redo));
        assert_eq!(key(Char('c'), sup), Some(Action::Copy));
        // selection
        assert_eq!(key(Right, s), Some(Action::Move(Motion::Right, true)));
        assert_eq!(key(Right, s | a), Some(Action::Move(Motion::WordRight, true)));
        assert_eq!(key(Up, s), Some(Action::Up(true)));
        assert_eq!(key(Up, n), Some(Action::Up(false)));
        // deletes
        assert_eq!(key(Backspace, a), Some(Action::DeleteBack(Unit::Word)));
        assert_eq!(key(Char('w'), c), Some(Action::DeleteBack(Unit::Word)));
        assert_eq!(key(Delete, a), Some(Action::DeleteForward(Unit::Word)));
        // undo: Ctrl+/ (kitty) and its legacy byte 0x1F (Ctrl+7); redo
        assert_eq!(key(Char('/'), c), Some(Action::Undo));
        assert_eq!(key(Char('7'), c), Some(Action::Undo));
        assert_eq!(key(Char('/'), a), Some(Action::Redo));
        assert_eq!(key(Char('?'), c | s), Some(Action::Redo));
        // copy/cut without Cmd
        assert_eq!(key(Char('C'), c | s), Some(Action::Copy));
        // text
        assert_eq!(key(Char('É'), s), Some(Action::Insert("É".into())));
        // Alt+↑/↓ stay the task navigation
        assert_eq!(key(Up, a), None);
    }
}
