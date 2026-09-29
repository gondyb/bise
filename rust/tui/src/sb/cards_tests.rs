//! Cards v2: the strip, the card view, the keys (book screens `cards v2
//! · 1-5`).

use super::*;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::io::Read;

/// An app whose hub end the test reads.
fn app_with_hub() -> (App, UnixStream) {
    let (a, b) = UnixStream::pair().unwrap();
    b.set_nonblocking(true).unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::mem::forget(tx);
    let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
    (sb_app(sb, rx, false, 100, crate::voice::Voice::live(false), "bench".into()), b)
}

/// What the TUI typed to the hub since the last call (`input` ops).
fn sent(b: &mut UnixStream) -> Vec<String> {
    let mut s = String::new();
    let mut buf = [0u8; 4096];
    while let Ok(n) = b.read(&mut buf) {
        if n == 0 {
            break;
        }
        s.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    s.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == "input")
        .map(|v| v["text"].as_str().unwrap_or("").to_string())
        .collect()
}

fn card(id: u64, kind: &str, agent: &str, text: &str) -> Card {
    Card { id, kind: kind.into(), agent: agent.into(), text: text.into(), age_ms: 360_000, ..Card::default() }
}

const PERF: &str = "the hero image is 4.2 MB. compress it, or lazy-load it?\n1. compress it (webp, ~300 kB)\n2. both: compress, and lazy-load below the fold";

/// The mocks' cast: release (an approval), perf (2 options), dark-mode
/// (9 options).
fn cast() -> Vec<Card> {
    let dark = format!(
        "the settings page has 3 grays in tokens.css and 2 hardcoded ones. i found them because the dark toggle left two borders white.\n{}",
        (1..=9).map(|i| format!("{i}. option {i}")).collect::<Vec<_>>().join("\n")
    );
    vec![
        card(12, "question", "perf", PERF),
        card(13, "question", "dark-mode", &dark),
        card(14, "approval", "release", "npm publish --tag next\n\npublishes 2.5.0 to npm under your name"),
    ]
}

fn key(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
    super::super::key(app, &KeyEvent::new(code, m), false)
}

fn ctrl(app: &mut App, c: char) -> bool {
    key(app, KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

fn row_of(rows: &[String], needle: &str) -> usize {
    rows.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("no {needle:?} in\n{}", rows.join("\n")))
}

fn click(app: &mut App, x: u16, y: u16) -> bool {
    card_mouse(
        app,
        &MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE },
    )
}

fn col_of(row: &str, needle: &str) -> u16 {
    let i = row.find(needle).unwrap();
    row[..i].chars().count() as u16
}

/// Screen 1: the strip above the divider, most blocking first, the label
/// row, the options on the right, `9 options`, the `×`.
#[test]
fn the_strip_shows_every_card_most_blocking_first() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    let rows = draw(&mut app, 140, 40);
    let lab = row_of(&rows, "ctrl+g open");
    assert!(rows[lab].contains(" 3 cards "), "{}", rows[lab]);
    let rel = row_of(&rows, "release wants to run");
    let perf = row_of(&rows, "? perf · the hero image");
    let dark = row_of(&rows, "? dark-mode · ");
    assert_eq!((rel, perf, dark), (lab + 1, lab + 2, lab + 3), "{}", rows.join("\n"));
    assert!(rows[rel].contains("npm publish --tag next"), "{}", rows[rel]);
    assert!(rows[rel].contains("1 allow  2 always  3 deny  ×"), "{}", rows[rel]);
    assert!(rows[perf].contains("1 compress  2 both  ×"), "{}", rows[perf]);
    assert!(rows[dark].contains("9 options  ×"), "{}", rows[dark]);
    assert!(rows[dark].contains("…"), "cut: {}", rows[dark]);
    // right above the blank row and the divider
    assert!(rows[dark + 2].contains("you → main"), "{}", rows[dark + 2]);
    // the history is still there, the composer still talks to main
    assert!(!app.sb.card.open);
}

/// More than 3 cards: `+ n more`; under 24 rows, one row: the top card
/// and `+ n`.
#[test]
fn many_cards_and_small_screens() {
    let (mut app, _hub) = app_with_hub();
    let mut cs = cast();
    cs.push(card(20, "done", "sad-404", "the dog has a hat"));
    cs.push(card(21, "failed", "t9", "the build broke"));
    app.sb.cards = cs;
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("  + 2 more  ")), "{}", rows.join("\n"));
    let rows = draw(&mut app, 140, 20);
    assert!(!rows.iter().any(|r| r.contains("ctrl+g open")), "{}", rows.join("\n"));
    let top = row_of(&rows, "release wants to run");
    assert!(rows[top].contains("+ 4  1 allow"), "{}", rows[top]);
}

/// From the thread only ctrl+g acts on the cards: the old card keys
/// (ctrl+x/n/p, alt+r, ctrl+a, ctrl+f, digits) do nothing to a card.
#[test]
fn from_the_thread_only_ctrl_g() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    for c in ['x', 'n', 'p', 'a', 'f'] {
        ctrl(&mut app, c);
    }
    key(&mut app, KeyCode::Char('r'), KeyModifiers::ALT);
    assert!(!key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE));
    assert!(sent(&mut hub).is_empty());
    assert!(!app.sb.card.open);
    assert!(ctrl(&mut app, 'g'));
    assert!(app.sb.card.open);
    // the top card: the approval
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(14));
    assert!(ctrl(&mut app, 'g'));
    assert!(!app.sb.card.open);
}

/// Screen 2: the card view takes the history's place: the tabs, the bar,
/// the title and its meta, the options one per row; the divider says
/// who reads the composer; the key bar has the view's keys.
#[test]
fn the_card_view_takes_the_history_place() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    ctrl(&mut app, 'g');
    ctrl(&mut app, 'n');
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "? release");
    assert!(rows[tabs].contains("? perf") && rows[tabs].contains("? dark-mode"), "{}", rows[tabs]);
    assert!(rows[tabs].contains("ctrl+n / ctrl+p"), "{}", rows[tabs]);
    let t = row_of(&rows, "? perf needs you");
    assert!(rows[t].contains("2 of 3 · 6m"), "{}", rows[t]);
    assert!(rows[t].contains("┃ ? perf needs you"), "{}", rows[t]);
    assert!(rows[row_of(&rows, "the hero image")].contains('┃'));
    let o1 = row_of(&rows, "1 compress it (webp");
    assert!(rows[o1 + 1].contains("2 both: compress"), "{}", rows[o1 + 1]);
    assert!(rows.iter().any(|r| r.contains("you → ? perf's card · your answer")), "{}", rows.join("\n"));
    assert!(
        rows.iter().any(|r| r.contains("1-2 pick   ⏎ answer   ctrl+x close   ctrl+n next   esc back")),
        "{}",
        rows.join("\n")
    );
    // the history is gone meanwhile: no strip either
    assert!(!rows.iter().any(|r| r.contains("ctrl+g open")));
}

/// Screen 4: an approval: the command on the raised tint, the reason,
/// allow once / always here / deny; ⏎ with text denies with a note.
#[test]
fn an_approval_plugs_in() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    ctrl(&mut app, 'g');
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("? release wants to run")));
    assert!(rows.iter().any(|r| r.contains("publishes 2.5.0 to npm")));
    for o in ["1 allow once", "2 always here", "3 deny"] {
        row_of(&rows, o);
    }
    assert!(rows.iter().any(|r| r.contains("1-3 pick   ⏎ deny with a note")), "{}", rows.join("\n"));
    app.ed.insert("not on friday");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 14 deny: not on friday"]);
    // on to the next card
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
}

/// 1-9 picks on an empty composer only; ⏎ sends the text; the history
/// says `✓ perf · you said both`; each card keeps its draft; esc brings
/// the thread's draft back; the last answer goes back to the thread.
#[test]
fn answering_picking_drafts_and_back() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("to main");
    ctrl(&mut app, 'g');
    assert_eq!(app.ed.text, "", "the card's own composer");
    ctrl(&mut app, 'n');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    // a digit on an empty composer picks
    assert!(key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(sent(&mut hub), vec!["/answer 12 both: compress, and lazy-load below the fold"]);
    assert!(matches!(app.events.last(), Some(Ev::Info(t)) if t.ends_with("perf · you said both")));
    // the next card: dark-mode; typed digits are text
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    app.ed.insert("x");
    assert!(!key(&mut app, KeyCode::Char('3'), KeyModifiers::NONE));
    app.ed.insert("3 then");
    // a draft per card
    ctrl(&mut app, 'p');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(14));
    assert_eq!(app.ed.text, "");
    ctrl(&mut app, 'n');
    assert_eq!(app.ed.text, "x3 then");
    // esc: back to the thread, its draft back
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "to main");
    // ctrl+g: the top card; ctrl+x closes it
    ctrl(&mut app, 'g');
    ctrl(&mut app, 'x');
    assert_eq!(sent(&mut hub), vec!["/close 14"]);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert_eq!(app.ed.text, "x3 then", "dark-mode's draft kept");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 13 x3 then"]);
    // none left: back to the thread with its draft
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "to main");
}

/// Screen 3: a card longer than the area scrolls (pgdn, ↓ on an empty
/// composer, the wheel); the options come after the text.
#[test]
fn a_long_card_scrolls() {
    let (mut app, _hub) = app_with_hub();
    let long = (1..=60).map(|i| format!("line {i:02} of the card")).collect::<Vec<_>>().join("\n") + "\n1. yes\n2. no";
    app.sb.cards = vec![card(3, "question", "t1", &long)];
    ctrl(&mut app, 'g');
    let rows = draw(&mut app, 120, 30);
    assert!(rows.iter().any(|r| r.contains("more lines · pgdn")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("1 yes")));
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.sb.card.scroll, 1);
    for _ in 0..10 {
        key(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
    }
    let rows = draw(&mut app, 120, 30);
    let yes = row_of(&rows, "1 yes");
    assert!(rows[yes - 2].contains("line 60"), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("end · pgup")));
    let a = app.sb.card.area;
    let wheel = MouseEvent { kind: MouseEventKind::ScrollUp, column: a.x + 3, row: a.y + 3, modifiers: KeyModifiers::NONE };
    let before = app.sb.card.scroll;
    assert!(card_mouse(&mut app, &wheel));
    assert_eq!(app.sb.card.scroll, before - 3);
}

/// The mouse on the strip: an option answers at once, `×` closes, a row
/// opens the card view on it; a tab switches cards.
#[test]
fn the_mouse_on_the_strip_and_the_tabs() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("draft to main");
    let rows = draw(&mut app, 140, 40);
    let perf = row_of(&rows, "? perf · ");
    assert!(click(&mut app, col_of(&rows[perf], "2 both"), perf as u16));
    assert_eq!(sent(&mut hub), vec!["/answer 12 both: compress, and lazy-load below the fold"]);
    assert_eq!(app.ed.text, "draft to main", "you never left your message");
    let rows = draw(&mut app, 140, 40);
    let rel = row_of(&rows, "release wants to run");
    assert!(click(&mut app, col_of(&rows[rel], "×"), rel as u16));
    assert_eq!(sent(&mut hub), vec!["/close 14"]);
    let rows = draw(&mut app, 140, 40);
    let dark = row_of(&rows, "? dark-mode · ");
    assert!(click(&mut app, col_of(&rows[dark], "dark-mode"), dark as u16));
    assert!(app.sb.card.open);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    // a new card only adds a tab
    app.sb.cards.push(card(30, "question", "api", "v1 or v2?"));
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "? api");
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert!(click(&mut app, col_of(&rows[tabs], "api"), tabs as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(30));
}

/// The card in view answered elsewhere: the view moves on, its draft
/// goes to the history; no card left: back to the thread.
#[test]
fn a_card_closed_elsewhere_moves_the_view_on() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = vec![card(1, "question", "a", "one?"), card(2, "question", "b", "two?")];
    app.ed.insert("mine");
    ctrl(&mut app, 'g');
    app.ed.insert("half an answer");
    app.sb.cards.remove(0);
    sync(&mut app);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(2));
    assert_eq!(app.history.first().map(String::as_str), Some("half an answer"));
    app.sb.cards.clear();
    sync(&mut app);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "mine");
}

/// ⌥N in the view: to that agent; the view goes, the drafts stay.
#[test]
fn going_to_an_agent_closes_the_view() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("to main");
    ctrl(&mut app, 'g');
    app.ed.insert("note");
    crate::sb::focus(&mut app, "perf");
    assert!(!app.sb.card.open);
    crate::sb::focus(&mut app, "main");
    assert_eq!(app.ed.text, "to main");
    ctrl(&mut app, 'g');
    assert_eq!(app.ed.text, "note");
}

/// A done card needs no words: ⏎ on an empty composer acknowledges it.
#[test]
fn a_done_card_is_acknowledged_with_enter() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = vec![card(5, "done", "sad-404", "the dog has a hat")];
    ctrl(&mut app, 'g');
    assert_eq!(crate::sb::card_key_pairs(&app)[0], ("⏎", "got it"));
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 5 seen"]);
    assert!(!app.sb.card.open);
}

/// The shapes: an approval's command and reason, a patch's summary, the
/// short labels of the strip.
#[test]
fn shapes() {
    let c = card(1, "approval", "r", "set -e\nnpm test\nnpm publish\n\nships it");
    let s = shape(&c);
    assert_eq!((s.who.as_str(), s.summary.as_str(), s.enter), ("r wants to run", "set -e · 3 lines", Enter::Deny));
    assert!(matches!(&s.parts[1], Part::Reason(r) if r == "ships it"));
    let patch = "diff --git a/x b/x\n--- a/src/a.rs\n+++ b/src/a.rs\n+x\n+y\n-z\n--- a/b.rs\n+++ b/b.rs\n+q\n--- a/c.rs\n+++ b/c.rs\n-w\n\nrewrites it";
    let s = shape(&card(2, "approval", "r", patch));
    assert_eq!(s.title, "r wants to edit src/a.rs");
    assert_eq!(s.summary, "src/a.rs +2 files · +3 −2");
    let s = shape(&card(3, "question", "perf", PERF));
    assert_eq!(s.short, vec!["compress", "both"]);
    assert_eq!(s.summary, "the hero image is 4.2 MB. compress it, or lazy-load it?");
    assert_eq!(short_labels(&["map a".into(), "map b".into()]), vec!["map a", "map b"]);
}
