//! The agent palette (BISE-265): the matching, the order, the keys, the
//! look.

use super::*;
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn agent(name: &str, status: &str, objective: &str) -> Agent {
    Agent { name: name.into(), main: name == "main", status: status.into(), objective: objective.into(), ..Agent::default() }
}

fn old(name: &str, objective: &str, ms: u64) -> Agent {
    Agent { report_ms: Some(ms), ..agent(name, "archived", objective) }
}

fn app() -> App {
    let mut app = bench::test_app();
    app.sb.agents = vec![
        agent("main", "idle", ""),
        agent("dark-mode", "working", "dark colors, settings first"),
        agent("perf", "done", "signup is slow"),
        agent("update-deps", "idle", "bump the packages"),
        old("data-export", "the csv export", 20),
        old("docs", "rewrite the readme", 30),
    ];
    app
}

fn q(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn names(app: &App, query: &str) -> Vec<String> {
    items(&app.sb, query).into_iter().map(|i| i.name).collect()
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    crate::input::on_key(app, &KeyEvent::new(code, m));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

#[test]
fn a_name_matches_by_its_start_a_word_anywhere_its_initials_its_letters_then_its_objective() {
    assert_eq!(score("dark-mode", &[], &q("dar")), Some((0, vec![0, 1, 2])));
    assert_eq!(score("dark-mode", &[], &q("mo")), Some((1, vec![5, 6])));
    assert_eq!(score("dark-mode", &[], &q("ark")), Some((2, vec![1, 2, 3])));
    assert_eq!(score("agent-palette", &[], &q("ap")), Some((3, vec![0, 6])));
    assert_eq!(score("dark-mode", &[], &q("dkmd")), Some((4, vec![0, 3, 5, 7])));
    assert_eq!(score("Dark-Mode", &[], &q("dark")), Some((0, vec![0, 1, 2, 3])), "case-insensitive");
    assert_eq!(score("perf", &["signup is slow"], &q("signup")), Some((5, vec![])));
    assert_eq!(score("perf", &["signup is slow"], &q("zz")), None);
    // one letter: the name only (its letters in order would match everything)
    assert_eq!(score("perf", &[], &q("x")), None);
}

#[test]
fn empty_query_lists_the_live_agents_in_the_panel_order_no_archived() {
    let app = app();
    assert_eq!(names(&app, ""), ["main", "dark-mode", "perf", "update-deps"]);
}

#[test]
fn a_query_ranks_the_best_match_first_then_the_archived_ones() {
    let app = app();
    // `da`: dark-mode's start, update-deps anywhere; data-export archived
    assert_eq!(names(&app, "da"), ["dark-mode", "update-deps", "data-export"]);
    let it = items(&app.sb, "da");
    assert_eq!(it.iter().map(|i| i.archived).collect::<Vec<_>>(), [false, false, true]);
    // an objective matches after every name
    assert_eq!(names(&app, "readme"), ["docs"]);
    assert_eq!(names(&app, "zz"), Vec::<String>::new());
}

#[test]
fn a_tie_goes_to_the_agent_waiting_on_you() {
    let mut app = app();
    app.sb.agents.push(agent("pa", "blocked", ""));
    // both start with `p`... `pa` exactly and `perf`: both rank 0 for `p`
    assert_eq!(names(&app, "p")[0], "pa");
}

#[test]
fn ctrl_s_opens_typing_filters_enter_opens_the_agent_esc_closes() {
    let mut app = app();
    app.ed.insert("my draft");
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(is_open(&app));
    typed(&mut app, "perf");
    assert_eq!(app.palette.as_ref().unwrap().query, "perf");
    assert_eq!(app.ed.text, "my draft", "the draft waits");
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!is_open(&app));
    assert_eq!(app.sb.focus_name(), "perf");
    // cmd+k opens it too; esc closes, nothing else changes
    press(&mut app, KeyCode::Char('k'), KeyModifiers::SUPER);
    assert!(is_open(&app) && app.cmd_keys);
    typed(&mut app, "dar");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!is_open(&app));
    assert_eq!(app.sb.focus_name(), "perf");
}

#[test]
fn arrows_choose_and_an_archived_agent_opens_its_history() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    typed(&mut app, "da");
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.sb.focus_name(), "data-export");
    assert!(app.sb.focus_archived());
    // up from the first wraps to the last
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Up, KeyModifiers::NONE);
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.sb.focus_name(), "update-deps");
}

#[test]
fn switch_opens_it_on_the_rest_of_the_line() {
    let mut app = app();
    handle_input(&mut app, "/switch dark");
    assert_eq!(app.palette.as_ref().map(|p| p.query.as_str()), Some("dark"));
}

#[test]
fn the_palette_takes_the_composer_pane_with_marks_notes_and_keys() {
    let mut app = app();
    app.ed.insert("my draft");
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    let rows = draw(&mut app, 100, 30);
    let all = rows.join("\n");
    assert!(all.contains("you → find an agent"), "{all}");
    let main = rows.iter().find(|r| r.contains("▸ :*")).unwrap_or_else(|| panic!("{all}"));
    assert!(main.contains("▸ :* main") && main.contains("⌥0"), "{main}");
    let dark = rows.iter().find(|r| r.contains("dark colors")).unwrap_or_else(|| panic!("{all}"));
    assert!(dark.contains("dark-mode    dark colors, settings first") && dark.contains("⌥1"), "{dark}");
    assert!(all.contains("type part of a name"), "{all}");
    assert!(!all.contains("my draft"), "{all}");
    assert!(!all.contains("data-export"), "no archived on an empty query");
    let bar: String = crate::keybar::line(&app, 200).spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(bar, "↑↓ choose   ⏎ open   esc close");
    // a query: the archived ones under `earlier`
    typed(&mut app, "da");
    let rows = draw(&mut app, 100, 30);
    // under the divider: the panel names them too
    let div = rows.iter().position(|r| r.contains("find an agent")).unwrap();
    let at = |s: &str| (div..rows.len()).find(|&i| rows[i].contains(s)).unwrap_or_else(|| panic!("{s}: {}", rows.join("\n")));
    assert!(at("dark-mode") < at("update-deps") && at("update-deps") < at("earlier") && at("earlier") < at("data-export"));
    assert!(rows[at("da ")].contains("3 agents"), "{}", rows.join("\n"));
    // no match
    typed(&mut app, "zz");
    let all = draw(&mut app, 100, 30).join("\n");
    assert!(all.contains("no agent called “dazz” · esc closes"), "{all}");
    // esc: the draft is back
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let all = draw(&mut app, 100, 30).join("\n");
    assert!(all.contains("my draft") && !all.contains("find an agent"), "{all}");
}

#[test]
fn a_click_on_a_row_opens_that_agent() {
    let mut app = app();
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    let rows = draw(&mut app, 100, 30);
    let y = rows.iter().position(|r| r.contains("bump the packages")).unwrap() as u16;
    let x = rows[y as usize].find("update-deps").unwrap() as u16;
    let m = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE };
    crate::input::on_mouse(&mut app, &m, 30);
    assert!(!is_open(&app));
    assert_eq!(app.sb.focus_name(), "update-deps");
}

#[test]
fn a_long_list_scrolls_with_the_selection() {
    let mut app = app();
    for i in 0..30 {
        app.sb.agents.push(old(&format!("old-{i:02}"), "", 100 + i));
    }
    press(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    typed(&mut app, "old");
    for _ in 0..20 {
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    }
    let rows = draw(&mut app, 100, 30);
    let sel = rows.iter().find(|r| r.contains("▸ –")).unwrap_or_else(|| panic!("{}", rows.join("\n")));
    // newest first: old-29 … the 21st is old-09
    assert!(sel.contains("old-09"), "{}", rows.join("\n"));
    assert!(rows.iter().any(|r| r.contains("30 agents")));
}
