//! Find in the history (BISE-237): the scan order, the keys, the
//! counter, folds, the highlight, and a 50 000-event history.

use super::*;
use crate::wire::{Mark, ToolState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn app_with(events: Vec<Ev>) -> App {
    let mut app = crate::sb::bench::test_app();
    app.events = events;
    app.cache.clear();
    app
}

fn tool(id: u32, intent: &str, out: &str) -> Ev {
    let mut td = ToolData::bare(id, ToolState::Ok);
    td.name = Some("bash".into());
    td.code = Some("echo hi".into());
    td.intent = Some(intent.into());
    td.elapsed = Some("0.1s".into());
    td.took = Some(Duration::from_millis(100));
    td.result = Some((true, out.into()));
    Ev::Tool(td)
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    crate::input::on_key(app, &KeyEvent::new(code, m));
}

fn typed(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn draw(app: &mut App) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| crate::run::draw_frame(app, f)).unwrap();
    t
}

fn screen(t: &Terminal<TestBackend>) -> String {
    let b = t.backend().buffer();
    (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect::<String>() + "\n").collect()
}

fn counter(app: &App) -> String {
    app.find.as_ref().unwrap().counter(false, Instant::now())
}

#[test]
fn lower_keeps_the_bytes_where_they_are() {
    for s in ["Déjà VU", "İstanbul", "ẞig", "ÉCOLE 日本 Ω"] {
        assert_eq!(lower(s).len(), s.len(), "{s}");
    }
    assert_eq!(lower("Déjà VU"), "déjà vu");
    // columns, not bytes; wide chars count 2
    assert_eq!(matches_in("日本 Signup and signup", "signup", false), vec![(5, 11), (16, 22)]);
    assert_eq!(matches_in("Signup signup", "Signup", true), vec![(0, 6)]);
    assert!(matches_in("abc", "", false).is_empty());
}

#[test]
fn ctrl_f_opens_the_field_and_esc_closes_it() {
    let mut app = app_with(vec![Ev::You("ship the signup page".into(), Mark::Sent)]);
    app.ed.insert("my draft");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert!(app.find.is_some());
    let t = draw(&mut app);
    let s = screen(&t);
    assert!(s.contains("find in the history"), "{s}");
    assert!(s.contains("find in main"), "the divider names the feed: {s}");
    assert!(!s.contains("my draft"), "{s}");
    assert!(s.contains("⏎ older"), "{s}");
    typed(&mut app, "sign");
    assert_eq!(app.find.as_ref().unwrap().query, "sign");
    assert_eq!(app.ed.text, "my draft", "the draft waits");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.find.is_none());
    let s = screen(&draw(&mut app));
    assert!(s.contains("my draft"), "{s}");
}

#[test]
fn messages_come_first_then_up_and_down_with_a_counter() {
    let mut app = app_with(vec![
        Ev::You("deploy the site".into(), Mark::Sent),     // 0
        Ev::Assistant("deploy done, then deploy docs".into()), // 1: 2 matches
        tool(7, "run the deploy script", "ok"),               // 2
        Ev::Info("nothing here".into()),                      // 3
    ]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "deploy");
    draw(&mut app);
    let f = app.find.as_ref().unwrap();
    // the newest message wins over the newer tool call
    assert_eq!(f.cur, Some((1, 1)));
    assert_eq!(counter(&app), "3 of 4");
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    press(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)));
    assert_eq!(counter(&app), "1 of 4");
    // past the oldest: back to the newest, and it says so
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((2, 0)));
    assert_eq!(counter(&app), "back to the newest");
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((0, 0)));
    assert_eq!(counter(&app), "back to the oldest");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    // no match, then smart-case
    typed(&mut app, "zz");
    draw(&mut app);
    assert_eq!(counter(&app), "no match");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typed(&mut app, "Deploy");
    draw(&mut app);
    assert_eq!(counter(&app), "no match", "no `Deploy` with a capital");
    assert_eq!(app.find.as_ref().unwrap().cur, None);
}

#[test]
fn the_matches_are_painted_the_current_one_on_the_accent() {
    let mut app = app_with(vec![Ev::You("alpha signup beta signup".into(), Mark::Sent)]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "signup");
    let t = draw(&mut app);
    let b = t.backend().buffer();
    let s = screen(&t);
    let y = s.lines().position(|l| l.contains("alpha signup")).expect("the message is shown") as u16;
    let row: String = (0..100).map(|x| b[(x, y)].symbol()).collect();
    let x1 = row.find("signup").unwrap() as u16;
    let x2 = row.rfind("signup").unwrap() as u16;
    // the newest match is current: the second one
    assert_eq!(b[(x2, y)].bg, crate::theme::accent());
    assert_eq!(b[(x1, y)].bg, crate::theme::pill_bg());
    assert!(s.contains("2 of 2"), "{s}");
}

#[test]
fn a_match_in_a_closed_call_opens_it_and_moving_on_closes_it() {
    let out = (1..=40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n") + "\nnpm publish done";
    let mut app = app_with(vec![
        Ev::You("publish it".into(), Mark::Sent),
        tool(3, "release the package", &out),
        Ev::Assistant("released".into()),
    ]);
    draw(&mut app);
    let closed = |app: &App| matches!(&app.events[1], Ev::Tool(td) if !td.opened && !td.expanded);
    assert!(closed(&app));
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "npm publish");
    let s = screen(&draw(&mut app));
    assert_eq!(app.find.as_ref().unwrap().cur, Some((1, 0)));
    assert!(matches!(&app.events[1], Ev::Tool(td) if td.opened && td.expanded));
    assert!(s.contains("npm publish done"), "the box is open on the match: {s}");
    // a message match next: the call closes again
    typed(&mut app, "\u{8}");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typed(&mut app, "publish");
    draw(&mut app);
    assert!(closed(&app), "{:?}", app.find.as_ref().unwrap().cur);
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(closed(&app));
}

#[test]
fn the_view_moves_to_an_old_match_and_stays_there_on_esc() {
    let mut events = vec![Ev::You("the needle is here".into(), Mark::Sent)];
    for i in 0..200 {
        events.push(Ev::Assistant(format!("filler reply {i}")));
    }
    let mut app = app_with(events);
    draw(&mut app);
    assert!(app.follow);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    typed(&mut app, "needle");
    let s = screen(&draw(&mut app));
    assert!(!app.follow);
    assert!(s.contains("the needle is here"), "{s}");
    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let s = screen(&draw(&mut app));
    assert!(s.contains("the needle is here"), "pinned on the match: {s}");
    assert!(!app.follow);
}

#[test]
fn a_paste_goes_to_the_query_and_new_lines_are_searched() {
    let mut app = app_with(vec![Ev::You("one".into(), Mark::Sent)]);
    draw(&mut app);
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    // ctrl+w after a wide blank (fuzz): cut at a char boundary
    crate::input::on_paste(&mut app, "x\u{3000}y");
    press(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    crate::input::on_paste(&mut app, "late\nnews");
    assert_eq!(app.find.as_ref().unwrap().query, "late news");
    draw(&mut app);
    assert_eq!(counter(&app), "no match");
    app.events.push(Ev::Assistant("the late news".into()));
    draw(&mut app);
    assert_eq!(counter(&app), "1 of 1");
}

/// A long history: 50 000 events (your messages, replies, calls with
/// 2 KB outputs). Opening and typing never block a frame longer than
/// the budget (plus one event); messages are found before the calls.
/// `cargo test -p bend-tui --release find_is_fast -- --nocapture`
/// prints the numbers.
#[test]
fn find_is_fast_on_50k_events() {
    let out: String = (0..40).map(|i| format!("out line {i} of the build, nothing to see\n")).collect();
    let mut events = Vec::with_capacity(50_000);
    for i in 0..50_000u32 {
        events.push(match i % 4 {
            0 => Ev::You(format!("message {i}: please check the login flow and the signup page"), Mark::Sent),
            1 => Ev::Assistant(format!("reply {i}: I checked the **login** flow; the signup page works. {}", "More words. ".repeat(20))),
            _ => tool(i, "run the build", &out),
        });
    }
    events[10].clone_from(&Ev::You("the rare zebra word".into(), Mark::Sent));
    let mut f = Find::new("bench", events.len(), None);
    let budget = Duration::from_millis(6);
    let mut slow = Duration::ZERO;
    let mut steps = 0;
    let t0 = Instant::now();
    f.query = "zebra".into();
    f.restart();
    while f.busy() {
        let t = Instant::now();
        f.scan(&events, budget);
        slow = slow.max(t.elapsed());
        steps += 1;
    }
    let first = t0.elapsed();
    assert_eq!(f.total, 1);
    assert_eq!(f.cur, Some((10, 0)));
    // a second query: the index is built, only the scan runs
    let t1 = Instant::now();
    f.query = "signup".into();
    f.restart();
    let mut steps2 = 0;
    let mut found_at = None;
    while f.busy() {
        let t = Instant::now();
        f.scan(&events, budget);
        slow = slow.max(t.elapsed());
        steps2 += 1;
        if found_at.is_none() && f.cur.is_some() {
            found_at = Some(steps2);
        }
    }
    let second = t1.elapsed();
    assert_eq!(f.total, 25_000);
    assert_eq!(found_at, Some(1), "the newest message is found in the first slice");
    assert_eq!(f.cur, Some((49_997, 0)));
    eprintln!(
        "find 50k events: first query {:?} in {} slices (index built), next query {:?} in {} slices, slowest slice {:?}",
        first, steps, second, steps2, slow
    );
    // a slice stops at the budget (checked every 64 events); generous
    // for a debug build on a busy machine
    assert!(slow < budget + Duration::from_millis(150), "slowest slice {:?}", slow);
}
