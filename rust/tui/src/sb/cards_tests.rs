//! The inbox (cards v2): the strip, the card view, the keys (book
//! screens `inbox · 1-7`; BISE-302: ctrl+1-9 and a click open an item).

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
    (sb_app(sb, rx, false, 100, crate::voice::Voice::live(false)), b)
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

/// ctrl+1: the card view on the inbox's first row.
fn open(app: &mut App) {
    assert!(ctrl(app, '1'), "ctrl+1 opens row 1");
    assert!(app.sb.card.open);
}

/// A key through the whole handler (the composer included).
fn press(app: &mut App, code: KeyCode) {
    crate::input::on_key(app, &KeyEvent::new(code, KeyModifiers::NONE));
}

/// The key bar's text.
fn bar(app: &App) -> String {
    crate::keybar::line(app, 200).spans.iter().map(|s| s.content.as_ref()).collect()
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

/// Screen 1: the strip above the divider, most blocking first, each
/// row numbered (BISE-302: ctrl+N opens row N), the label row
/// `ctrl+1-3 open`; no other keys on the rows.
#[test]
fn the_strip_shows_every_card_most_blocking_first() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    let rows = draw(&mut app, 140, 40);
    let lab = row_of(&rows, "ctrl+1-3 open ");
    assert!(rows[lab].contains(" inbox · 3 waiting for you "), "{}", rows[lab]);
    let rel = row_of(&rows, " 1 ? release wants to run");
    let perf = row_of(&rows, " 2 ? perf · the hero image");
    let dark = row_of(&rows, " 3 ? dark-mode · ");
    assert_eq!((rel, perf, dark), (lab + 1, lab + 2, lab + 3), "{}", rows.join("\n"));
    assert!(rows[rel].contains("npm publish --tag next"), "{}", rows[rel]);
    for r in [rel, perf, dark] {
        assert!(!rows[r].contains('×') && !rows[r].contains("⏎") && !rows[r].contains('▸'), "no keys: {}", rows[r]);
    }
    assert!(rows[dark].contains("…"), "cut: {}", rows[dark]);
    // right above the blank row and the divider
    assert!(rows[dark + 2].contains("you → main"), "{}", rows[dark + 2]);
    assert!(bar(&app).starts_with("@ file   $ skills   / commands   ctrl+1 inbox"), "{}", bar(&app));
    // the label follows the rows shown: 1 row, 2 rows
    app.sb.cards.truncate(2);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("ctrl+1-2 open ")), "{}", rows.join("\n"));
    app.sb.cards.truncate(1);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("ctrl+1 open ")), "{}", rows.join("\n"));
    // nothing waits: no inbox pair
    app.sb.cards.clear();
    assert!(!bar(&app).contains("inbox"), "{}", bar(&app));
}

/// BISE-302: a terminal without ctrl+1-9 (reach.rs): the strip says
/// `click to open`, without clicks `/inbox opens it`; the key bar
/// `/inbox`; the rows keep their numbers.
#[test]
fn without_ctrl_digits_the_inbox_never_shows_them() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    app.ctrl_digits = false;
    let rows = draw(&mut app, 140, 40);
    let lab = row_of(&rows, " inbox · 3 waiting for you ");
    assert!(rows[lab].contains("click to open "), "{}", rows[lab]);
    assert!(rows[lab + 1].contains(" 1 ? release wants to run · "), "{}", rows[lab + 1]);
    assert!(!rows.iter().any(|r| r.contains("ctrl+1")), "{}", rows.join("\n"));
    assert!(bar(&app).starts_with("@ file   $ skills   / commands   /inbox"), "{}", bar(&app));
    app.clicks = false;
    let rows = draw(&mut app, 140, 40);
    assert!(rows[lab].contains("/inbox opens it "), "{}", rows[lab]);
    // the keys still work when they come
    assert!(ctrl(&mut app, '2'));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
}

/// More than 3 cards: `+ n more`; under 24 rows, one row: the top card
/// numbered 1 and `+ n`.
#[test]
fn many_cards_and_small_screens() {
    let (mut app, _hub) = app_with_hub();
    let mut cs = cast();
    cs.push(card(20, "done", "sad-404", "the dog has a hat"));
    cs.push(card(21, "failed", "t9", "the build broke"));
    app.sb.cards = cs;
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("  + 2 more  ")), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("ctrl+1-3 open ")), "the rows shown: {}", rows.join("\n"));
    let rows = draw(&mut app, 140, 20);
    assert!(!rows.iter().any(|r| r.contains("open ")), "{}", rows.join("\n"));
    let top = row_of(&rows, " 1 ? release wants to run");
    assert!(rows[top].contains("+ 4  "), "{}", rows[top]);
}

/// ctrl+N opens row N in the view, the rows under `+ n more` too; in the
/// view it shows that item; past the last row: nothing. A French layout's
/// top row counts as its digits (`&` is 1, `é` 2).
#[test]
fn ctrl_digit_opens_row_n() {
    let (mut app, mut hub) = app_with_hub();
    let mut cs = cast();
    cs.push(card(20, "done", "sad-404", "the dog has a hat"));
    cs.push(card(21, "failed", "t9", "the build broke"));
    app.sb.cards = cs;
    let ids = super::super::card_draw::strip_ids(&app.sb);
    assert_eq!(ids.len(), 5);
    app.ed.insert("my draft");
    assert!(!ctrl(&mut app, '6'), "past the last row");
    assert!(!app.sb.card.open);
    assert!(ctrl(&mut app, '2'));
    assert!(app.sb.card.open);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[1]));
    assert_eq!(app.ed.text, "", "the thread's draft waits");
    // in the view: that item, a hidden row too
    assert!(ctrl(&mut app, '5'));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[4]));
    assert!(key(&mut app, KeyCode::Char('&'), KeyModifiers::CONTROL));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[0]));
    assert!(key(&mut app, KeyCode::Char('é'), KeyModifiers::CONTROL | KeyModifiers::SHIFT));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(ids[1]));
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.sb.card.open);
    assert_eq!(app.ed.text, "my draft");
    assert!(sent(&mut hub).is_empty(), "opening answers nothing");
    // nothing waits: ctrl+1 is not the inbox's
    app.sb.cards.clear();
    assert!(!ctrl(&mut app, '1'));
}

/// From the thread only ctrl+1-9 act on the inbox: the old keys (ctrl+g,
/// ctrl+x/n/p, alt+r, ctrl+a, ctrl+f, digits) and the arrows do nothing
/// to it; a digit is text.
#[test]
fn from_the_thread_only_ctrl_digits() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    for c in ['g', 'x', 'n', 'p', 'a', 'f'] {
        ctrl(&mut app, c);
    }
    key(&mut app, KeyCode::Char('r'), KeyModifiers::ALT);
    assert!(!key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE));
    for k in [KeyCode::Up, KeyCode::Down, KeyCode::Enter, KeyCode::Right] {
        assert!(!key(&mut app, k, KeyModifiers::NONE), "{k:?} is the thread's");
    }
    assert!(sent(&mut hub).is_empty());
    assert!(!app.sb.card.open);
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.ed.text, "2", "a digit is text");
}

/// What only a terminal with ctrl+1-9 sends turns them on (reach.rs);
/// ctrl+4-7 come from any terminal (0x1c-0x1f) and prove nothing.
#[test]
fn a_ctrl_digit_proves_the_terminal_sends_them() {
    let k = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    assert!(super::proves_ctrl_digits(&k('1')) && super::proves_ctrl_digits(&k('9')));
    assert!(!super::proves_ctrl_digits(&k('4')) && !super::proves_ctrl_digits(&k('7')));
    assert!(!super::proves_ctrl_digits(&KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)));
    let (mut app, _hub) = app_with_hub();
    app.ctrl_digits = false;
    crate::input::on_key(&mut app, &k('5'));
    assert!(!app.ctrl_digits);
    crate::input::on_key(&mut app, &k('2'));
    assert!(app.ctrl_digits);
}

/// Screen 2: the card view takes the history's place: the tabs, the bar,
/// the title and its meta, the options one per row; the divider says
/// who reads the composer; the key bar has the view's keys.
#[test]
fn the_card_view_takes_the_history_place() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    ctrl(&mut app, 'n');
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "? release");
    assert!(rows[tabs].contains("? perf") && rows[tabs].contains("? dark-mode"), "{}", rows[tabs]);
    assert!(rows[tabs].contains("? dark-mode") && rows[tabs].contains("  ←→  "), "{}", rows[tabs]);
    let t = row_of(&rows, "? perf needs you");
    assert!(rows[t].contains("2 of 3 · 6m"), "{}", rows[t]);
    assert!(rows[t].contains("┃ ? perf needs you"), "{}", rows[t]);
    assert!(rows[row_of(&rows, "the hero image")].contains('┃'));
    let o1 = row_of(&rows, "  1 compress it (webp");
    assert!(rows[o1 + 1].contains("  2 both: compress"), "{}", rows[o1 + 1]);
    assert!(!rows.iter().any(|r| r.contains('▸')), "nothing highlighted on open");
    assert!(rows.iter().any(|r| r.contains("you → ? perf · your answer")), "{}", rows.join("\n"));
    assert_eq!(bar(&app), "↑↓ choose   1-2 pick   ←→ other items   type to answer in your words   esc back");
    // the history is gone meanwhile: no strip either
    assert!(!rows.iter().any(|r| r.contains("waiting for you")));
}

/// Screen 4: an approval: the command on the raised tint, the reason,
/// allow once / always here / deny; ⏎ with text denies with a note.
#[test]
fn an_approval_plugs_in() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    let rows = draw(&mut app, 140, 40);
    assert!(rows.iter().any(|r| r.contains("? release wants to run")));
    assert!(rows.iter().any(|r| r.contains("publishes 2.5.0 to npm")));
    for o in ["1 allow once", "2 always here", "3 deny"] {
        row_of(&rows, o);
    }
    assert_eq!(bar(&app), "↑↓ choose   1-3 pick   type why not, ⏎ says no   ←→ other items   esc back");
    // a reflex ⏎ approves nothing
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(sent(&mut hub).is_empty());
    app.ed.insert("not on friday");
    assert_eq!(bar(&app), "⏎ says no, with your words   ctrl+n next item   esc back, draft kept");
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
    open(&mut app);
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
    // ctrl+1: the top card; ctrl+x closes it
    open(&mut app);
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

/// Screen 5: a card longer than the area scrolls with pgup/pgdn and the
/// wheel, not ↑↓: they highlight an option, and the view scrolls to it.
#[test]
fn a_long_card_scrolls() {
    let (mut app, _hub) = app_with_hub();
    let long = (1..=60).map(|i| format!("line {i:02} of the card")).collect::<Vec<_>>().join("\n") + "\n1. yes\n2. no";
    app.sb.cards = vec![card(3, "question", "t1", &long)];
    open(&mut app);
    let rows = draw(&mut app, 120, 30);
    assert!(rows.iter().any(|r| r.contains("more lines · pgdn")), "{}", rows.join("\n"));
    assert!(!rows.iter().any(|r| r.contains("1 yes")));
    key(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
    assert!(app.sb.card.scroll > 0);
    let a = app.sb.card.area;
    let wheel = MouseEvent { kind: MouseEventKind::ScrollUp, column: a.x + 3, row: a.y + 3, modifiers: KeyModifiers::NONE };
    let before = app.sb.card.scroll;
    assert!(card_mouse(&mut app, &wheel));
    assert_eq!(app.sb.card.scroll, before - 3);
    // ↑: the last option, brought into view
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1));
    let rows = draw(&mut app, 120, 30);
    let no = row_of(&rows, "▸ 2 no");
    assert!(rows[no - 1].contains("  1 yes"), "{}", rows.join("\n"));
    assert!(rows[no - 3].contains("line 60"), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("end · pgup")));
}

/// Screens 3, 4, 7: nothing highlighted on open (⏎ does nothing); the
/// first ↓ is option 1, the first ↑ the last, no wrap; ⏎ picks it and
/// the key bar says so; typing dims the options and hides the highlight
/// (remembered), ⏎ sends the text; ←→ other cards.
#[test]
fn the_arrows_choose_an_option() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    ctrl(&mut app, '2');
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    assert_eq!(app.sb.card.opt, None);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(sent(&mut hub).is_empty(), "a reflex ⏎ answers nothing");
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1), "the first ↑: the last option");
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(1), "no wrap");
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.sb.card.opt, Some(0));
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    let rows = draw(&mut app, 140, 40);
    assert!(rows[row_of(&rows, "2 both: compress")].contains("▸ 2 both"), "{}", rows.join("\n"));
    assert_eq!(bar(&app), "↑↓ choose   ⏎ pick “both: compress, and lazy-load b…”   ←→ other items   esc back");
    // typing: the text's arrows, the highlight hidden but kept
    press(&mut app, KeyCode::Char('m'));
    press(&mut app, KeyCode::Left);
    assert_eq!((app.ed.cursor, app.sb.current_card().map(|c| c.id)), (0, Some(12)), "← moved the caret");
    let rows = draw(&mut app, 140, 40);
    assert!(!rows.iter().any(|r| r.contains('▸')), "{}", rows.join("\n"));
    assert_eq!(bar(&app), "⏎ send as your answer   ctrl+n next item   esc back, draft kept");
    // emptied: the arrows and the highlight are back
    press(&mut app, KeyCode::Delete);
    assert_eq!(app.ed.text, "");
    assert!(bar(&app).contains("⏎ pick “both"), "{}", bar(&app));
    // ← → the other cards, the highlight goes with the card
    key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!((app.sb.current_card().map(|c| c.id), app.sb.card.opt), (Some(13), None));
    key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
    // ↓ ⏎ picks option 1; the next card opens
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent(&mut hub), vec!["/answer 12 compress it (webp, ~300 kB)"]);
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    // typed text + ⏎ answers with the text
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(sent(&mut hub), vec!["/answer 13 k"]);
}

/// Narrow: the card view's key bar drops from the right, `↑↓ choose`
/// and `⏎ …` last.
#[test]
fn the_card_key_bar_keeps_choose_and_enter() {
    let v = vec![("↑↓", "choose".to_string()), ("⏎", "pick “both”".into()), ("←→", "other items".into()), ("esc", "back".into())];
    let keys = |w| fit_card_pairs(v.clone(), w).iter().map(|p| p.0).collect::<Vec<_>>();
    assert_eq!(keys(200), ["↑↓", "⏎", "←→", "esc"]);
    assert_eq!(keys(30), ["↑↓", "⏎"]);
    assert_eq!(keys(10), ["↑↓"]);
}

/// The mouse on the strip (BISE-302): a click on a row opens it in the
/// view, the thread's draft waits; a tab switches cards.
#[test]
fn the_mouse_on_the_strip_and_the_tabs() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = cast();
    app.ed.insert("draft to main");
    let rows = draw(&mut app, 140, 40);
    let dark = row_of(&rows, "? dark-mode · ");
    assert!(click(&mut app, col_of(&rows[dark], "dark-mode"), dark as u16));
    assert!(app.sb.card.open, "a click on a row opens it");
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert_eq!(app.ed.text, "");
    assert!(sent(&mut hub).is_empty());
    // a new card only adds a tab
    app.sb.cards.push(card(30, "question", "api", "v1 or v2?"));
    let rows = draw(&mut app, 140, 40);
    let tabs = row_of(&rows, "? api");
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(13));
    assert!(click(&mut app, col_of(&rows[tabs], "api"), tabs as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(30));
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.ed.text, "draft to main");
    // the number or the summary: the whole row opens
    let rows = draw(&mut app, 140, 40);
    let perf = row_of(&rows, "? perf · ");
    assert!(click(&mut app, col_of(&rows[perf], "2 ?"), perf as u16));
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(12));
}

/// The card view's divider keeps a fresh note on its right (a copy from
/// the card's text: `✓ copied 9 chars`), as the thread's divider does.
#[test]
fn the_card_view_divider_shows_a_fresh_note() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = cast();
    open(&mut app);
    app.flash = Some(("copied 9 chars".into(), std::time::Instant::now()));
    let rows = draw(&mut app, 140, 40);
    let d = row_of(&rows, "you → ? release · your answer");
    assert!(rows[d].contains("your answer  ✓ copied 9 chars "), "{}", rows[d]);
}

/// The card in view answered elsewhere: the view moves on, its draft
/// goes to the history; no card left: back to the thread.
#[test]
fn a_card_closed_elsewhere_moves_the_view_on() {
    let (mut app, _hub) = app_with_hub();
    app.sb.cards = vec![card(1, "question", "a", "one?"), card(2, "question", "b", "two?")];
    app.ed.insert("mine");
    open(&mut app);
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
    open(&mut app);
    app.ed.insert("note");
    crate::sb::focus(&mut app, "perf");
    assert!(!app.sb.card.open);
    crate::sb::focus(&mut app, "main");
    assert_eq!(app.ed.text, "to main");
    open(&mut app);
    assert_eq!(app.ed.text, "note");
}

/// A done card needs no words: ⏎ on an empty composer acknowledges it.
#[test]
fn a_done_card_is_acknowledged_with_enter() {
    let (mut app, mut hub) = app_with_hub();
    app.sb.cards = vec![card(5, "done", "sad-404", "the dog has a hat")];
    open(&mut app);
    assert_eq!(bar(&app), "⏎ got it   esc back");
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

/// A sandbox card (brief 1e): 1 runs it again, 2 says it saves a "run
/// outside the sandbox" rule (designer): `it` when the rule is the command.
#[test]
fn a_sandbox_card_says_outside_the_sandbox() {
    let text = "wants to run it outside the sandbox\n| echo hi > ~/Desktop/x.txt\nreason: the sandbox stopped a write outside the repo: ~/Desktop/x.txt.\nalways: echo hi > ~/Desktop/x.txt";
    let s = shape(&card(4, "confirm", "t3", text));
    assert_eq!(s.options, vec!["run it again without the sandbox", "always run it outside the sandbox here", "no"]);
    let text = "wants to run it outside the sandbox\n| cp a.txt ~/Desktop/\nreason: the sandbox stopped a write outside the repo: ~/Desktop.\nalways: cp *";
    let s = shape(&card(5, "confirm", "t3", text));
    assert_eq!(s.options[1], "always run cp * outside the sandbox here");
    let text = "wants to run\n| npm run build\nreason: the checker is off, so commands ask first.\nalways: npm run build *";
    let s = shape(&card(6, "confirm", "t1", text));
    assert_eq!(s.options[1], "always allow npm run build * here");
}
