//! Timing of long feeds (infinite-feed): `SB_BENCH_TRANSCRIPT=<transcript.log>
//! SB_BENCH_LINES=50000 cargo test --release -p bend-tui bench_long_feed -- --ignored --nocapture`.

use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::Instant;

pub(crate) fn test_app() -> App {
    let (a, _b) = UnixStream::pair().unwrap();
    let (_tx, rx) = mpsc::channel::<String>();
    let sb = new_sb(std::sync::Arc::new(std::sync::Mutex::new(a)), "bench".into());
    std::mem::forget(_b);
    App {
        connected: true,
        debug: false,
        line_tools: std::collections::HashMap::new(),
        follow: true,
        anchor: (0, 0),
        scroll: 0,
        vis_events: Vec::new(),
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
        win: FeedWindow::default(),
        area_w: 100,
        area_h: 24,
        events: Vec::new(),
        last_line_at: None,
        show_thinking: false,
        interrupt_requested: false,
        pending: false,
        ed: crate::editor::Editor::default(),
        composer: crate::ComposerArea::default(),
        flash: None,
        mouse: crate::MouseState::default(),
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        tick: 0,
        info: HarnessInfo {
            model: "switchboard".into(),
            threshold: String::new(),
            steer_path: String::new(),
            interrupt_path: String::new(),
        },
        host: String::new(),
        port: 0,
        session_id: "bench".into(),
        stream: None,
        rx,
        should_quit: false,
        sb: Some(sb),
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore]
fn bench_long_feed() {
    let path = std::env::var("SB_BENCH_TRANSCRIPT").expect("SB_BENCH_TRANSCRIPT");
    let want: usize = std::env::var("SB_BENCH_LINES").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let raw = std::fs::read_to_string(&path).unwrap();
    let src: Vec<String> = raw
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(_, r)| r.to_string()))
        .collect();
    let mut lines = Vec::with_capacity(want);
    while lines.len() < want {
        for l in &src {
            if lines.len() >= want {
                break;
            }
            lines.push(l.clone());
        }
    }
    let bytes: usize = lines.iter().map(|l| l.len()).sum();
    eprintln!("lines {} bytes {}", lines.len(), bytes);
    let mut app = test_app();
    let (w, h) = (200u16, 50u16);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    let t = Instant::now();
    for l in &lines {
        let j = json!({"ev": "line", "agent": "big", "line": l}).to_string();
        dispatch(&mut app, &j);
    }
    eprintln!("replay (dispatch, feed out of focus): {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "big");
    let f = ms(t);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("focus switch: focus() {:.1} ms + first draw {:.1} ms (events {})", f, ms(t), app.events.len());
    let mut worst = 0f64;
    for _ in 0..10 {
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("steady frame at the tail (worst of 10): {:.2} ms", worst);
    for (i, e) in app.events.iter().enumerate() {
        if let Ev::Tool(td) = e {
            if matches!(td.state, ToolState::Run) {
                let rows = app.cache[i].as_ref().map_or(0, |c| c.rows.len());
                let code = td.code.as_ref().map_or(0, |c| c.len());
                eprintln!("  live tool #{} at {} rows {} code {} bytes", td.id, i, rows, code);
            }
        }
    }
    app.follow = false;
    let mut worst = 0f64;
    for _ in 0..50 {
        app.scroll -= 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("PageUp frames (worst of 50): {:.2} ms", worst);
    app.anchor = (0, 0);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("jump to the top: {:.2} ms", ms(t));
    let mut worst = 0f64;
    for _ in 0..50 {
        app.scroll += 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst = worst.max(ms(t));
    }
    eprintln!("PageDown frames from the top (worst of 50): {:.2} ms", worst);
    let t = Instant::now();
    for l in lines.iter().take(20) {
        let j = json!({"ev": "line", "agent": "big", "line": l}).to_string();
        dispatch(&mut app, &j);
    }
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("20 live lines + frame: {:.2} ms", ms(t));
    term.backend_mut().resize(w - 30, h);
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("resize frame: {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "main");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    focus(&mut app, "big");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("focus away and back + 2 draws: {:.1} ms", ms(t));

    // what the hub does now: the last 1000 lines on connect, with their
    // positions, then pages of 1000 older lines while the user scrolls up
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    let n = lines.len();
    let t = Instant::now();
    for (i, l) in lines.iter().enumerate().skip(n.saturating_sub(1000)) {
        let j = json!({"ev": "line", "agent": "big", "line": l, "pos": i + 1}).to_string();
        dispatch(&mut app, &j);
    }
    eprintln!("windowed: replay of the last 1000 lines: {:.1} ms", ms(t));
    let t = Instant::now();
    focus(&mut app, "big");
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("windowed: focus switch + first draw: {:.1} ms", ms(t));
    app.follow = false;
    let (mut worst_frame, mut worst_page, mut pages, mut frames) = (0f64, 0f64, 0, 0);
    let total = Instant::now();
    while app.win.first_pos.is_some_and(|p| p > 1) || app.anchor != (0, 0) {
        app.scroll -= 25;
        let t = Instant::now();
        term.draw(|f| draw_sb(&mut app, f)).unwrap();
        worst_frame = worst_frame.max(ms(t));
        frames += 1;
        if app.win.loading {
            let before = app.win.first_pos.unwrap();
            let from = before.saturating_sub(1000).max(1);
            let page: Vec<Value> = (from..before)
                .map(|p| json!({"pos": p, "line": lines[p - 1]}))
                .collect();
            let j = json!({"ev": "history", "agent": "big", "before": before, "lines": page}).to_string();
            let t = Instant::now();
            dispatch(&mut app, &j);
            worst_page = worst_page.max(ms(t));
            pages += 1;
        }
        if frames > 1_000_000 {
            break;
        }
    }
    eprintln!(
        "windowed: PageUp to the very top: {} frames, worst {:.2} ms; {} pages, worst page ingest {:.1} ms; total {:.0} ms; events held {}",
        frames, worst_frame, pages, worst_page, ms(total), app.events.len()
    );
}

// ---- the anchored scroll (O(visible) frames) ----

fn screen(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    let w = buf.area.width as usize;
    buf.content
        .chunks(w)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
        .collect()
}

fn feed(app: &mut App, from: usize, to: usize) {
    for k in from..to {
        push_event(&mut app.events, &mut app.cache, Ev::Info(format!("event {}", k)));
    }
}

#[test]
fn a_pinned_view_does_not_move_when_lines_arrive() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    feed(&mut app, 0, 300);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.tail_visible);
    assert!(screen(&term).iter().any(|l| l.contains("event 299")));
    app.follow = false;
    app.scroll -= 40;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.tail_visible);
    // the text of the feed (the scrollbar thumb moves: more history)
    let text = |t: &Terminal<TestBackend>, h: usize| -> Vec<String> {
        screen(t).into_iter().take(h).map(|l| l.chars().take(40).collect()).collect()
    };
    let before = text(&term, app.area_h);
    assert!(!before.iter().any(|l| l.contains("event 299")));
    feed(&mut app, 300, 320);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let after = text(&term, app.area_h);
    assert_eq!(before, after);
    // back down: the view follows again
    app.scroll += 1000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.follow && app.tail_visible);
    assert!(screen(&term).iter().any(|l| l.contains("event 319")));
}

#[test]
fn scrolling_up_then_down_comes_back() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    feed(&mut app, 0, 300);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    app.follow = false;
    app.scroll -= 100;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    let a = app.anchor;
    app.scroll -= 37;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    app.scroll += 37;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert_eq!(app.anchor, a);
    // the top of the feed is reachable and stops there
    app.scroll -= 100_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(screen(&term)[0].contains("event 0"));
    // every row of the feed maps to the event it shows (clicks)
    assert_eq!(app.vis_events[0], 0);
    assert_eq!(app.vis_events.len(), app.area_h);
}

// ---- the bounded window and the pages of older history ----

fn line(pos: usize) -> String {
    json!({"ev": "line", "agent": "main", "line": format!("  obs: assistant: message {}", pos), "pos": pos})
        .to_string()
}

#[test]
fn a_following_feed_keeps_its_last_events_then_pages_back() {
    let mut app = test_app();
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    for p in 1..=5000 {
        dispatch(&mut app, &line(p));
    }
    assert!(app.events.len() <= MAX_EVENTS, "{}", app.events.len());
    let first = app.win.first_pos.unwrap();
    assert!(first > 1);
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(!app.win.loading);
    // up to the top of what the feed holds: a page is asked
    app.follow = false;
    app.scroll -= 1_000_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.win.loading);
    let shown = |t: &Terminal<TestBackend>| -> Vec<String> {
        screen(t).into_iter().take(10).map(|l| l.chars().take(40).collect()).collect()
    };
    let before = shown(&term);
    let n0 = app.events.len();
    let page: Vec<Value> = (first.saturating_sub(PAGE_LINES).max(1)..first)
        .map(|p| json!({"pos": p, "line": format!("  obs: assistant: message {}", p)}))
        .collect();
    let got = page.len();
    dispatch(
        &mut app,
        &json!({"ev": "history", "agent": "main", "before": first, "lines": page}).to_string(),
    );
    assert!(!app.win.loading);
    assert_eq!(app.events.len(), n0 + got);
    assert_eq!(app.win.first_pos, Some(first - got));
    // the view did not move: it can keep scrolling up
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert_eq!(shown(&term), before);
    // a stale page (the feed moved since the ask) is ignored
    dispatch(
        &mut app,
        &json!({"ev": "history", "agent": "main", "before": 99999, "lines": [{"pos": 1, "line": "x"}]}).to_string(),
    );
    assert_eq!(app.events.len(), n0 + got);
    // back to the tail: the next line trims the feed again
    app.scroll += 10_000_000;
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    assert!(app.follow);
    dispatch(&mut app, &line(5001));
    assert!(app.events.len() <= MAX_EVENTS);
}

#[test]
fn a_feed_out_of_focus_is_bounded_too() {
    let mut app = test_app();
    for p in 1..=5000 {
        dispatch(
            &mut app,
            &json!({"ev": "line", "agent": "other", "line": format!("  obs: assistant: m {}", p), "pos": p}).to_string(),
        );
    }
    focus(&mut app, "other");
    assert!(app.events.len() <= MAX_EVENTS);
    assert!(app.win.first_pos.unwrap() > 1);
}
