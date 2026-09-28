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
        top: 0,
        max_top: 0,
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
        area_w: 100,
        area_h: 24,
        events: Vec::new(),
        last_line_at: None,
        show_thinking: false,
        interrupt_requested: false,
        pending: false,
        input: String::new(),
        cursor: 0,
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        hist_idx: None,
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
    app.top = 0;
    let t = Instant::now();
    term.draw(|f| draw_sb(&mut app, f)).unwrap();
    eprintln!("frame at the top: {:.2} ms", ms(t));
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
}
