//! /help and /shortcuts: a scrollable overlay built from ONE table of
//! (section, keys, action) rows. A new feature adds one row to `ROWS`.
//!
//! /help shows the commands and the essential keys (rows marked `top`);
//! /shortcuts shows every row. Typing filters, Tab switches the page,
//! Esc clears the filter, then closes.

use crate::{App, BRAND, DIM};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Where a row applies: both clients, the Switchboard only, or the
/// single-agent client only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Scope {
    All,
    Sb,
    Solo,
}

/// One line of the table. `keys` holds the alternatives separated by
/// `|`; ` then ` inside one alternative is a sequence (Option+e then e).
/// Empty `keys`: a note written across the whole width.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row {
    pub section: &'static str,
    pub keys: &'static str,
    pub action: &'static str,
    pub scope: Scope,
    /// shown by /help too, not only by /shortcuts
    pub top: bool,
}

const fn r(section: &'static str, keys: &'static str, action: &'static str) -> Row {
    Row { section, keys, action, scope: Scope::All, top: false }
}

impl Row {
    const fn sb(mut self) -> Row {
        self.scope = Scope::Sb;
        self
    }
    const fn solo(mut self) -> Row {
        self.scope = Scope::Solo;
        self
    }
    const fn top(mut self) -> Row {
        self.top = true;
        self
    }
}

const TALK: &str = "Talk to agents";
const CONV: &str = "Conversation";
const TASKS: &str = "Tasks (empty composer)";
const CARDS: &str = "Cards";
const EDIT: &str = "Composer editing";
const ACCENTS: &str = "Accents & symbols";
const SELECT: &str = "Selection & copy";
const FEED: &str = "Feed";
const VOICE: &str = "Voice";
const TERM: &str = "Terminal panel";
const GHOSTTY: &str = "Ghostty tips";

/// Every shortcut, in display order (sections appear in first-row order).
#[rustfmt::skip]
pub(crate) const ROWS: &[Row] = &[
    r(TALK, "⏎", "send to the agent in view (main, or the task you entered); while it works, steer its turn").sb().top(),
    r(TALK, "@task …", "direct message to a task without leaving main; @main … from a task").sb().top(),
    r(TALK, "Ctrl+C", "interrupt the turn of the agent in view; again (or at idle) quit — the agents keep running").sb().top(),
    r(TALK, "Ctrl+Z", "cancel the last route not yet delivered").sb(),
    r(TALK, "Ctrl+O", "a shell in the folder of the agent in view (exit comes back)").sb(),
    r(TALK, "/", "the commands: Tab completes, ⏎ runs").sb().top(),
    r(CONV, "⏎", "send; during a turn, steer the model").solo().top(),
    r(CONV, "Tab", "during a turn: queue the draft for after the turn").solo().top(),
    r(CONV, "Ctrl+C", "interrupt the turn; again (or at idle) quit — the session survives").solo().top(),
    r(CONV, "/", "the commands: Tab completes, ⏎ runs").solo().top(),
    r(TASKS, "Ctrl+K|Alt+↓", "select the next task").sb().top(),
    r(TASKS, "Ctrl+J|Alt+↑", "select the previous task").sb(),
    r(TASKS, "⏎", "enter the selected task").sb().top(),
    r(TASKS, "Space", "preview the selected task without entering it").sb(),
    r(TASKS, "D", "drop the selected task").sb(),
    r(TASKS, "Esc", "close the selection; in a task, back to main").sb().top(),
    r(TASKS, "Alt+1 … Alt+9", "go to task N").sb().top(),
    r(TASKS, "Alt+0", "back to main").sb(),
    r(CARDS, "Ctrl+G|Ctrl+A", "show / hide the card box (Ctrl+A on an empty composer)").sb().top(),
    r(CARDS, "Ctrl+N|Ctrl+P", "next / previous card").sb(),
    r(CARDS, "Alt+R", "answer the card with the composer text (empty: acknowledge a done card)").sb().top(),
    r(CARDS, "Ctrl+F", "card full screen, again (or Esc) to shrink").sb(),
    r(CARDS, "Ctrl+X", "close the card without answering").sb(),
    r(CARDS, "PgUp|PgDn", "scroll the card").sb(),
    r(CARDS, "y|n|Esc", "a confirmation: yes / no / cancel").sb(),
    r(EDIT, "Shift+⏎|Alt+⏎|Ctrl+J", "new line (Ctrl+J on an empty Switchboard composer selects a task)").top(),
    r(EDIT, "Option+←|Option+→", "word left / right"),
    r(EDIT, "Ctrl+Option+←|Ctrl+Option+→", "subword left / right (camelCase, snake_case, kebab-case, digits)"),
    r(EDIT, "Cmd+←|Cmd+→|Ctrl+A|Ctrl+E|Home|End", "line start / end"),
    r(EDIT, "Ctrl+Home|Ctrl+End|Cmd+↑|Cmd+↓", "text start / end (Cmd+↑/↓: see Ghostty tips)"),
    r(EDIT, "↑|↓", "move between rows, then through the history (↓ past the newest brings the draft back)"),
    r(EDIT, "Option+Backspace|Ctrl+W", "delete the word before"),
    r(EDIT, "Option+Delete", "delete the word after"),
    r(EDIT, "Ctrl+Option+Backspace|Ctrl+Option+Delete", "delete a subword before / after"),
    r(EDIT, "Cmd+Backspace|Ctrl+U", "delete to the line start"),
    r(EDIT, "Ctrl+K", "delete to the line end (on an empty Switchboard composer: next task)"),
    r(EDIT, "Ctrl+/|Cmd+Z", "undo"),
    r(EDIT, "Alt+/|Ctrl+Shift+/|Cmd+Shift+Z", "redo"),
    r(EDIT, "Esc", "put the draft away in the history (↑ brings it back)").sb(),
    r(EDIT, "Tab|⏎", "pick from the / or @ popup (Esc closes it)"),
    r(EDIT, ":name:", "typed, becomes its emoji (:tada: → 🎉)").top(),
    r(ACCENTS, "Option+` then e", "è (grave)"),
    r(ACCENTS, "Option+e then e", "é (acute)"),
    r(ACCENTS, "Option+i then o", "ô (circumflex)"),
    r(ACCENTS, "Option+u then u", "ü (umlaut)"),
    r(ACCENTS, "Option+n then n", "ñ (tilde)"),
    r(ACCENTS, "Option+c|Option+q|Option+\\|Option+Shift+\\", "ç œ « » — every macOS U.S. Option character works"),
    r(SELECT, "Shift + any move", "extend the composer selection"),
    r(SELECT, "Cmd+A", "select the whole composer text (see Ghostty tips)"),
    r(SELECT, "click|drag|Shift+click", "composer: place the cursor, select, extend"),
    r(SELECT, "double click|triple click", "select a word / everything (feed: the word / the row)"),
    r(SELECT, "drag in the feed", "select; the release copies it"),
    r(SELECT, "Ctrl+Shift+C|Cmd+C", "copy the composer selection, else the feed's").top(),
    r(SELECT, "Ctrl+Shift+X|Cmd+X", "cut"),
    r(SELECT, "Esc", "drop the selection"),
    r(SELECT, "Shift+drag", "the terminal's own selection (outside the app)"),
    r(FEED, "PgUp|PgDn|wheel", "scroll the feed (the card when it is shown)"),
    r(FEED, "End", "back to the bottom"),
    r(FEED, "click ✦", "open / close one reasoning section"),
    r(FEED, "Ctrl+T", "open / close every reasoning section"),
    r(FEED, "Ctrl+L", "clear the display (/clear)"),
    r(VOICE, "Ctrl+R", "speech-to-text into the composer (turn it on with /voice)").top(),
    r(VOICE, "any key", "while recording: stop, keep the text"),
    r(VOICE, "Esc|Ctrl+C", "while recording: cancel"),
    r(TERM, "Ctrl+`|Ctrl+Space", "show / hide the terminal panel: a shell in the workspace, kept running while hidden").top(),
    r(TERM, "any key", "while shown: goes to the shell, Ctrl+C included"),
    r(TERM, "Shift+PgUp|Shift+PgDn|wheel", "scroll its history"),
    r(TERM, "drag the top border", "resize it"),
    r(GHOSTTY, "", "Ghostty keeps Cmd+↑/↓, Cmd+A, Cmd+C and Cmd+Z by default. To get them in the composer, add to ~/Library/Application Support/com.mitchellh.ghostty/config:"),
    r(GHOSTTY, "", "keybind = super+arrow_up=unbind"),
    r(GHOSTTY, "", "keybind = super+arrow_down=unbind"),
    r(GHOSTTY, "", "keybind = super+z=unbind"),
    r(GHOSTTY, "", "keybind = super+shift+z=unbind"),
    r(GHOSTTY, "", "keybind = super+c=performable:copy_to_clipboard (Cmd+C copies Ghostty's selection if any, else the app's)"),
    r(GHOSTTY, "", "Check what reaches the app: bend-harness keyprobe"),
];

// ---- the overlay state ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Page {
    Help,
    Shortcuts,
}

#[derive(Debug)]
pub(crate) struct Overlay {
    page: Page,
    filter: String,
    scroll: usize,
    // set by the last draw
    max_scroll: usize,
    visible: usize,
}

impl Overlay {
    pub(crate) fn new(page: Page) -> Overlay {
        Overlay { page, filter: String::new(), scroll: 0, max_scroll: 0, visible: 1 }
    }
}

/// The page a command opens: /help, /shortcuts and its aliases.
pub(crate) fn page_of(cmd: &str) -> Option<Page> {
    match cmd {
        "/help" => Some(Page::Help),
        "/shortcuts" | "/shortcut" | "/keys" => Some(Page::Shortcuts),
        _ => None,
    }
}

/// Keys while the overlay is open: it takes them all. `true` when
/// handled (always, while open).
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(o) = app.help.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let page = o.visible.saturating_sub(1).max(1);
    match k.code {
        KeyCode::Esc if !o.filter.is_empty() => {
            o.filter.clear();
            o.scroll = 0;
        }
        KeyCode::Esc => app.help = None,
        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => app.help = None,
        KeyCode::Tab | KeyCode::BackTab => {
            o.page = match o.page {
                Page::Help => Page::Shortcuts,
                Page::Shortcuts => Page::Help,
            };
            o.scroll = 0;
        }
        KeyCode::Up => o.scroll = o.scroll.saturating_sub(1),
        KeyCode::Down => o.scroll = (o.scroll + 1).min(o.max_scroll),
        KeyCode::PageUp => o.scroll = o.scroll.saturating_sub(page),
        KeyCode::PageDown => o.scroll = (o.scroll + page).min(o.max_scroll),
        KeyCode::Home => o.scroll = 0,
        KeyCode::End => o.scroll = o.max_scroll,
        KeyCode::Backspace => {
            o.filter.pop();
            o.scroll = 0;
        }
        KeyCode::Char(c) if !ctrl => {
            o.filter.push(c);
            o.scroll = 0;
        }
        _ => {}
    }
    true
}

/// The mouse while the overlay is open: the wheel scrolls it, the
/// rest is swallowed. `true` when handled.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let Some(o) = app.help.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => o.scroll = o.scroll.saturating_sub(3),
        MouseEventKind::ScrollDown => o.scroll = (o.scroll + 3).min(o.max_scroll),
        _ => {}
    }
    true
}

// ---- rendering (pure: rows in, lines out) ----

const CHIP: Style = Style::new()
    .fg(Color::Rgb(0xee, 0xee, 0xee))
    .bg(Color::Rgb(0x3a, 0x3a, 0x44));
const CODE: Style = Style::new().fg(Color::Rgb(0xb8, 0xd4, 0xf0));

/// The rows of a page for a client, filtered (case-insensitive, over the
/// section, the keys and the action).
pub(crate) fn rows(page: Page, sb: bool, filter: &str) -> Vec<&'static Row> {
    let f = filter.to_lowercase();
    ROWS.iter()
        .filter(|r| match r.scope {
            Scope::All => true,
            Scope::Sb => sb,
            Scope::Solo => !sb,
        })
        .filter(|r| page == Page::Shortcuts || r.top)
        .filter(|r| {
            f.is_empty()
                || [r.section, r.keys, r.action]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&f))
        })
        .collect()
}

/// Word wrap `text` into rows of at most `width` columns (a word longer
/// than a row is cut).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if !cur.is_empty() && cur.width() + 1 + word.width() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
        while cur.width() > width {
            let mut head = String::new();
            for ch in cur.chars() {
                if head.width() + ch.width().unwrap_or(0) > width {
                    break;
                }
                head.push(ch);
            }
            if head.is_empty() {
                break;
            }
            cur = cur[head.len()..].to_string();
            out.push(head);
        }
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

fn spans_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// The chips of one keys field, as rows of spans at most `width` wide
/// (an alternative never splits).
fn chip_rows(keys: &str, width: usize) -> Vec<Vec<Span<'static>>> {
    let units = keys.split('|').map(|alt| {
        let mut u = Vec::new();
        for (i, step) in alt.split(" then ").enumerate() {
            if i > 0 {
                u.push(Span::styled(" then ", Style::default().fg(DIM)));
            }
            u.push(Span::styled(format!(" {} ", step), CHIP));
        }
        u
    });
    let mut out: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    for u in units {
        let line = out.last_mut().unwrap();
        let used = spans_width(line);
        if used > 0 && used + 1 + spans_width(&u) > width {
            out.push(u);
        } else {
            if used > 0 {
                line.push(Span::raw(" "));
            }
            line.extend(u);
        }
    }
    out
}

fn header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
    ))
}

/// The two-column body (keys | action) of `rows`, `width` columns wide,
/// a header per section.
pub(crate) fn table_lines(rows: &[&Row], width: usize) -> Vec<Line<'static>> {
    let width = width.max(20);
    // the key column: its chips wrap at a third of the width (30 at
    // most); one alternative wider than that widens the column
    let cap = (width / 3).clamp(16, 30);
    let key_w = rows
        .iter()
        .filter(|r| !r.keys.is_empty())
        .flat_map(|r| chip_rows(r.keys, cap))
        .map(|l| spans_width(&l))
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(10));
    let act_w = width.saturating_sub(key_w + 2).max(8);
    let mut out = Vec::new();
    let mut section = "";
    for r in rows {
        if r.section != section {
            if !out.is_empty() {
                out.push(Line::default());
            }
            out.push(header(r.section));
            section = r.section;
        }
        if r.keys.is_empty() {
            let style = if r.action.starts_with("keybind") { CODE } else { Style::default() };
            for l in wrap(r.action, width.saturating_sub(2)) {
                out.push(Line::from(vec![Span::raw("  "), Span::styled(l, style)]));
            }
            continue;
        }
        let keys = chip_rows(r.keys, key_w);
        let acts = wrap(r.action, act_w);
        for i in 0..keys.len().max(acts.len()) {
            let mut spans = keys.get(i).cloned().unwrap_or_default();
            let used = spans_width(&spans);
            spans.push(Span::raw(" ".repeat(key_w + 2 - used.min(key_w + 2))));
            if let Some(a) = acts.get(i) {
                spans.push(Span::raw(a.clone()));
            }
            out.push(Line::from(spans));
        }
    }
    out
}

/// The whole text of a page: /help = the commands + the essential keys,
/// /shortcuts = the full table.
pub(crate) fn page_lines(
    page: Page,
    sb: bool,
    filter: &str,
    commands: &[(&'static str, &'static str)],
    width: usize,
) -> Vec<Line<'static>> {
    let rows = rows(page, sb, filter);
    let mut out = Vec::new();
    if page == Page::Help {
        let f = filter.to_lowercase();
        let cmds: Vec<_> = commands
            .iter()
            .filter(|(n, d)| f.is_empty() || n.contains(&f) || d.to_lowercase().contains(&f))
            .collect();
        if !cmds.is_empty() {
            out.push(header("Commands"));
            let name_w = cmds.iter().map(|(n, _)| n.width()).max().unwrap_or(0);
            let desc_w = width.saturating_sub(name_w + 2).max(8);
            for (n, d) in cmds {
                for (i, l) in wrap(d, desc_w).into_iter().enumerate() {
                    let name = if i == 0 { *n } else { "" };
                    out.push(Line::from(vec![
                        Span::styled(format!("{:<w$}  ", name, w = name_w), Style::default().fg(BRAND)),
                        Span::raw(l),
                    ]));
                }
            }
            out.push(Line::default());
        }
        if !rows.is_empty() {
            out.push(Line::from(Span::styled(
                "Essential keys · every key: /shortcuts (Tab here)",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )));
        }
    }
    out.extend(table_lines(&rows, width));
    if out.is_empty() {
        out.push(Line::from(Span::styled(
            format!("nothing matches “{}” · Backspace or Esc", filter),
            Style::default().fg(DIM),
        )));
    }
    out
}

/// The overlay, over the whole frame, when open.
pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let sb = app.sb.is_some();
    let Some(o) = app.help.as_mut() else { return };
    let full = frame.area();
    if full.width < 24 || full.height < 6 {
        return;
    }
    let w = full.width.saturating_sub(2).min(110);
    let h = full.height.saturating_sub(2);
    let area = Rect { x: full.x + (full.width - w) / 2, y: full.y + 1, width: w, height: h };
    let commands: Vec<(&'static str, &'static str)> = if sb {
        crate::sb::SB_COMMANDS.iter().map(|c| (c.name, c.desc)).collect()
    } else {
        crate::COMMANDS.iter().map(|c| (c.name, c.desc)).collect()
    };
    let lines = page_lines(o.page, sb, &o.filter, &commands, (w as usize).saturating_sub(4));
    let visible = (h as usize).saturating_sub(2).max(1);
    o.visible = visible;
    o.max_scroll = lines.len().saturating_sub(visible);
    o.scroll = o.scroll.min(o.max_scroll);
    let tab = |p: Page, name: &str| {
        let style = if o.page == p {
            Style::default().fg(Color::Black).bg(BRAND).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        };
        Span::styled(format!(" {} ", name), style)
    };
    let mut title = vec![
        Span::raw(" "),
        tab(Page::Help, "/help"),
        Span::raw(" "),
        tab(Page::Shortcuts, "/shortcuts"),
        Span::raw(" "),
    ];
    if !o.filter.is_empty() {
        title.push(Span::styled(format!(" filter: {}▏ ", o.filter), Style::default().fg(BRAND)));
    }
    let pos = if o.max_scroll > 0 {
        format!(
            "{}–{}/{} ↑↓ PgUp/PgDn · ",
            o.scroll + 1,
            (o.scroll + visible).min(lines.len()),
            lines.len()
        )
    } else {
        String::new()
    };
    let foot = format!(" {}type to filter · Tab switch · Esc close ", pos);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BRAND))
        .title(Line::from(title))
        .title_bottom(Line::from(Span::styled(foot, Style::default().fg(DIM))).right_aligned())
        .padding(Padding::horizontal(1));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((o.scroll as u16, 0)), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every entry of the table renders in each client where it applies,
    /// narrow and wide, and no line overflows the width.
    #[test]
    fn every_row_renders() {
        for width in [40usize, 76, 106] {
            for sb in [true, false] {
                let lines = page_lines(Page::Shortcuts, sb, "", &[], width);
                let all = text(&lines);
                for l in &lines {
                    let w = spans_width(&l.spans);
                    assert!(w <= width, "overflow {} > {}: {:?}", w, width, l);
                }
                let applies = |r: &&Row| match r.scope {
                    Scope::All => true,
                    Scope::Sb => sb,
                    Scope::Solo => !sb,
                };
                for r in ROWS.iter().filter(applies) {
                    assert!(all.contains(r.section), "section {}", r.section);
                    for alt in r.keys.split('|').filter(|k| !k.is_empty()) {
                        for step in alt.split(" then ") {
                            assert!(all.contains(&format!(" {} ", step)), "key {} at {}", step, width);
                        }
                    }
                    // the action, rewrapped: every word is there
                    for word in r.action.split(' ').filter(|w| w.width() <= width - 2) {
                        assert!(all.contains(word), "action {} ({})", r.action, word);
                    }
                }
            }
        }
    }

    #[test]
    fn help_is_commands_and_essentials() {
        let lines = page_lines(Page::Help, true, "", &[("/help", "commands and keys")], 80);
        let all = text(&lines);
        assert!(all.contains("Commands") && all.contains("/help"));
        assert!(all.contains(" Ctrl+G "), "a top row");
        assert!(!all.contains(" Ctrl+X "), "a /shortcuts-only row");
    }

    #[test]
    fn filter_keeps_matching_rows() {
        let r = rows(Page::Shortcuts, true, "subword");
        assert!(r.len() >= 2 && r.iter().all(|r| r.action.contains("subword")));
        assert!(rows(Page::Shortcuts, true, "zzzz").is_empty());
        assert!(rows(Page::Shortcuts, false, "ctrl+g").is_empty(), "no cards outside Switchboard");
    }

    #[test]
    fn aliases_open_shortcuts() {
        for c in ["/shortcuts", "/shortcut", "/keys"] {
            assert_eq!(page_of(c), Some(Page::Shortcuts));
        }
        assert_eq!(page_of("/help"), Some(Page::Help));
        assert_eq!(page_of("/h"), None);
    }
}
