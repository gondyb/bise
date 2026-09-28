//! The single-agent client: connect to the REPL, ingest its wire lines,
//! and run either the interactive loop (ratatui) or line mode (piped).

use crate::*;
use crossterm::event::{
    poll, read, EnableBracketedPaste, EnableMouseCapture, Event, KeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// one wire line into the feed of the app (the focused view)
pub(crate) fn ingest_line(app: &mut App, line: String) {
    if line == "--- idle" {
        app.pending = false;
        app.interrupt_requested = false;
        // BR-002: a flag that outlived its turn must
        // not kill the next one
        let _ = std::fs::write(
            &app.info.interrupt_path,
            "",
        );
    }
    // thinking duration: the model's reply arrives one
    // batch after the previous wire line
    let now = std::time::Instant::now();
    let ms = app
        .last_line_at
        .map_or(0, |t| now.duration_since(t).as_millis());
    app.last_line_at = Some(now);
    let (line, replayed) = strip_history(&line);
    // a live line after a pause: a time mark first (BISE-14, book §10)
    if !replayed {
        crate::feed::pause_mark(&mut app.events, &mut app.cache, ms, crate::feed::local_hhmm);
    }
    // a replayed reasoning section has no duration
    let ms = if replayed { 0 } else { ms };
    let parsed = if replayed {
        parse_history_line(line)
    } else {
        parse_line(line)
    };
    if let Some(ev) = parsed {
        // the reasoning rides inside the assistant text
        // (think markers): it becomes its own collapsed
        // section, never raw history text
        let evs: Vec<Ev> = match ev {
            Ev::Assistant(t) => match split_thinking(&t) {
                Some((think, vis)) => {
                    let mut v = vec![Ev::Thinking {
                        ms,
                        text: think.to_string(),
                        open: app.show_thinking,
                    }];
                    if !vis.trim().is_empty() {
                        v.push(Ev::Assistant(vis.to_string()));
                    }
                    v
                }
                None => vec![Ev::Assistant(t)],
            },
            other => vec![other],
        };
        for ev in evs {
            let finished = match &ev {
                Ev::Tool(td) if !matches!(td.state, ToolState::Run) => {
                    Some(td.id)
                }
                _ => None,
            };
            // the view is top-anchored: a pinned view never
            // moves, a following view re-sticks in draw
            let appended =
                push_event(&mut app.events, &mut app.cache, ev);
            if appended && !app.follow {
                app.unseen += 1;
            }
            if let (true, Some(id)) = (replayed, finished) {
                hide_replayed_elapsed(&mut app.events, &mut app.cache, id);
            }
        }
    }
}

/// The terminal in UI mode: raw + alternate screen, mouse reports,
/// bracketed paste (a multi-line paste arrives as ONE Event::Paste
/// instead of a keystroke storm where every Enter would send), and the
/// kitty keyboard protocol (Shift+Enter reported distinctly; terminals
/// without support ignore the push, Ctrl+J remains the fallback).
/// Fallible, unlike `ratatui::init` (which panics), and without its
/// panic hook: `crash::install` restores every one of these modes.
fn init_terminal() -> io::Result<ratatui::DefaultTerminal> {
    use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
    let setup = || -> io::Result<ratatui::DefaultTerminal> {
        enable_raw_mode()?;
        // BISE-02: light or dark from the terminal background, before the alternate screen
        crate::theme_detect::init();
        crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
        let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
        let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
        let _ = crossterm::execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))
    };
    setup().inspect_err(|_| crash::restore_terminal())
}

/// Takes the waiting wire lines for at most 12 ms. The hub replays whole
/// feeds on connect: the lines are taken in slices, a frame in between,
/// so the UI never waits for a replay to end. True when lines are left
/// (the next frame must not wait for input).
fn drain_lines(app: &mut App) -> bool {
    let slice = std::time::Instant::now();
    loop {
        if slice.elapsed() >= Duration::from_millis(12) {
            return true;
        }
        match app.rx.try_recv() {
            Ok(line) => {
                if app.sb.is_some() {
                    sb::dispatch(app, &line);
                } else {
                    ingest_line(app, line);
                    // the turn ended: the oldest queued message goes (BISE-89)
                    if let Some(m) = crate::queue::next(app) {
                        handle_input(app, &format!("say {}", m));
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // the REPL process is gone: a /reload exits it (the
                // parent respawns and reconnects), a crash does not.
                // Either way this UI is dead — stop, let the parent
                // decide.
                app.connected = false;
                app.should_quit = true;
                return false;
            }
        }
    }
}

/// A caught panic of one handler: shown in the feed (it was logged by
/// the hook), the editor state clamped back to valid.
fn report_crash(app: &mut App, c: &crash::Crash, what: &str) {
    app.ed.cursor = app.ed.cursor.min(app.ed.len());
    app.ed.anchor = app.ed.anchor.map(|a| a.min(app.ed.len()));
    push_event(&mut app.events, &mut app.cache, Ev::Err(c.line(what)));
}

/// Consecutive frames that panicked before the UI gives up (a draw that
/// always panics would loop forever): it exits with the terminal back.
const MAX_DRAW_CRASHES: u32 = 3;

pub(crate) fn run_tui(app: &mut App) -> io::Result<()> {
    crash::install();
    let mut terminal = init_terminal()?;
    crash::set_ui_thread(true);
    // BISE-60: the first launch of the switchboard UI plays the onboarding
    crate::onboarding::request_if_due(app);
    let r = ui_loop(app, &mut terminal);
    crash::set_ui_thread(false);
    // no orphan shell
    app.term.shutdown();
    crash::restore_terminal();
    r
}

/// Startup timing (SB_TIMING): what the loop spent until the first
/// usable frame (the hub's replay taken in, then drawn).
#[derive(Default)]
struct Startup {
    on: bool,
    done: bool,
    frames: u32,
    drain_ms: f64,
    draw_ms: f64,
    ready_seen: bool,
    caught_up: bool,
}

impl Startup {
    fn ready(app: &App) -> bool {
        sb::is_ready(app)
    }
    fn after_drain(&mut self, app: &App, t: std::time::Instant, backlog: bool) {
        if !self.on || self.done {
            return;
        }
        self.drain_ms += t.elapsed().as_secs_f64() * 1000.0;
        if !self.ready_seen && Self::ready(app) {
            self.ready_seen = true;
            crate::timing::mark(&format!(
                "ready dispatched ({} frames, drain {:.1} ms, draw {:.1} ms so far)",
                self.frames, self.drain_ms, self.draw_ms
            ));
        }
        if self.ready_seen && !backlog && !self.caught_up {
            self.caught_up = true;
            let evs: usize = app.events.len()
                + sb::background_events(app);
            crate::timing::mark(&format!(
                "caught up ({} events in all feeds, {} in focus, drain {:.1} ms)",
                evs,
                app.events.len(),
                self.drain_ms
            ));
        }
    }
    fn after_draw(&mut self, t: std::time::Instant) {
        if !self.on || self.done {
            return;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        self.draw_ms += ms;
        self.frames += 1;
        if self.frames == 1 {
            crate::timing::mark(&format!("first frame ({:.1} ms)", ms));
        }
        if self.caught_up {
            self.done = true;
            crate::timing::mark(&format!(
                "usable frame ({:.1} ms; {} frames, drain {:.1} ms, draw {:.1} ms in all)",
                ms, self.frames, self.drain_ms, self.draw_ms
            ));
        }
    }
}

/// One frame of the UI: the view, the one-time hints, then the frame
/// passes: the theme's ground on every cell (BISE-92), `BISE_ASCII`.
pub(crate) fn draw_frame(app: &mut App, f: &mut ratatui::Frame) {
    if app.sb.is_some() {
        sb::draw_sb(app, f)
    } else {
        draw(app, f)
    }
    crate::hints::draw(app, f); // BISE-61: one-time hints
    crate::theme::paint(f.buffer_mut()); // BISE-92: bise paints its ground
    crate::theme::asciify(f.buffer_mut()); // BISE-84: BISE_ASCII=1
}

fn ui_loop(app: &mut App, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut draw_crashes = 0u32;
    let mut startup = Startup { on: crate::timing::enabled(), ..Startup::default() };
    loop {
        let t_drain = std::time::Instant::now();
        let backlog = match crash::guarded(|| drain_lines(app)) {
            Ok(b) => b,
            Err(c) => {
                report_crash(app, &c, "a hub line");
                true
            }
        };
        startup.after_drain(app, t_drain, backlog);
        for note in crash::take_notes() {
            push_event(&mut app.events, &mut app.cache, Ev::Err(note));
        }
        if app.should_quit {
            break;
        }
        // BISE-60: the onboarding (first launch, /welcome) over the whole screen
        if crate::onboarding::take_request() {
            let shown = crash::guarded(|| {
                crate::onboarding::show(app, terminal, &mut |app| {
                    if let Err(c) = crash::guarded(|| drain_lines(app)) {
                        report_crash(app, &c, "a hub line");
                    }
                })
            });
            match shown {
                Ok(r) => r?,
                Err(c) => report_crash(app, &c, "the onboarding"),
            }
            continue;
        }
        pump_voice(app);
        let t_draw = std::time::Instant::now();
        let drawn = crash::guarded(|| {
            // BISE-92: the terminal's own background follows the theme
            crate::theme_detect::sync_terminal_bg();
            terminal.draw(|f| draw_frame(app, f))
        });
        startup.after_draw(t_draw);
        match drawn {
            Ok(r) => {
                r?;
                draw_crashes = 0;
            }
            Err(c) => {
                draw_crashes += 1;
                if draw_crashes >= MAX_DRAW_CRASHES {
                    return Err(io::Error::other(format!(
                        "the screen could not be drawn ({} panics in a row): {} at {}{}",
                        draw_crashes,
                        c.message,
                        c.location,
                        c.log.map(|p| format!(" — details: {}", p.display())).unwrap_or_default()
                    )));
                }
                report_crash(app, &c, "a frame");
                // the half-drawn buffer is dropped: the next frame repaints all
                let _ = terminal.clear();
            }
        }
        // the level meter moves every 50 ms while recording (Vibe's poll)
        let wait = if backlog {
            Duration::ZERO
        } else if app.voice.active() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(80)
        };
        if poll(wait)? {
            let ev = read()?;
            let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
            let handled = crash::guarded(|| match ev {
                Event::Mouse(m) => {
                    on_mouse(app, &m, term_h);
                    false
                }
                Event::Paste(text) => {
                    on_paste(app, &text);
                    false
                }
                Event::Key(k) => on_key(app, &k),
                _ => false,
            });
            match handled {
                Ok(true) => break,
                Ok(false) => {}
                Err(c) => report_crash(app, &c, "an input event"),
            }
        }
        app.tick = app.tick.wrapping_add(1);
    }
    Ok(())
}

// ---- line mode (non-interactive stdin) ----

pub(crate) fn print_ev_of(ev: &Ev, debug: bool, width: usize) {
    if !ev_visible(ev, debug) {
        return;
    }
    let mut out = io::stdout();
    for l in ev_lines(ev, width) {
        for span in l.spans.iter() {
            let _ = write!(out, "{}", span.content);
        }
        let _ = writeln!(out);
    }
}

// waits for the turn to finish ("--- idle") or the channel to close,
// printing every event as it arrives; gives up after `max`
pub(crate) fn wait_idle(app: &mut App, max: Duration) {
    let start = std::time::Instant::now();
    loop {
        match app.rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                app.feed_line(&line);
                if line == "--- idle" {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > max {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                app.connected = false;
                return;
            }
        }
    }
}

pub(crate) fn run_line_mode(app: &mut App) -> io::Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        // show anything the harness sent since the last command
        // (the --continue greeting arrives at connect)
        while let Ok(l) = app.rx.try_recv() {
            app.feed_line(&l);
        }
        let line = line?;
        let v = line.trim().to_string();
        if v.is_empty() {
            continue;
        }
        let local = handle_input(app, &v);
        for ev in &local {
            print_ev_of(ev, app.debug, app.area_w);
        }
        if app.should_quit {
            break;
        }
        // a sent command runs a turn: wait for it to finish
        if app.pending {
            wait_idle(app, Duration::from_secs(120));
        } else {
            while let Ok(line) = app.rx.try_recv() {
                app.feed_line(&line);
            }
        }
    }
    Ok(())
}

/// Connect to the REPL and run the UI (interactive ratatui when stdin and
/// stdout are TTYs, line mode otherwise). `info` is what the REPL
/// announced (model, threshold, side-channel paths).
pub fn run(host: String, port: u16, info: HarnessInfo, debug: bool, session_id: String) -> io::Result<()> {
    let stream = match TcpStream::connect((host.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("couldn't connect: {}", e);
            eprintln!("start the harness with ./run.sh");
            return Ok(());
        }
    };
    let reader = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || forward_lines(reader, tx));
    let _ = stream.set_nodelay(true);

    // line mode renders without a frame: the terminal width (or a sane
    // default) sizes the code blocks; interactive mode overwrites this
    // every frame
    let area_w = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(100).max(40);
    let voice = voice::Voice::live(voice::load_voice_enabled());
    let mut app = App {
        info,
        host,
        port,
        stream: Some(stream),
        ..App::new(rx, debug, area_w, voice, session_id)
    };

    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        run_tui(&mut app)
    } else {
        run_line_mode(&mut app)
    }
}

/// Forwards each `\n`-terminated line of the REPL socket, trailing
/// whitespace trimmed, until EOF, an error, or the receiver is gone. A
/// last line without its `\n` is dropped (the REPL died mid-write).
fn forward_lines(reader: impl Read, tx: mpsc::Sender<String>) {
    let mut r = io::BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(_) if buf.pop() == Some(b'\n') => {
                let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                if tx.send(line).is_err() {
                    return;
                }
            }
            // EOF (possibly after an unterminated tail) or a read error
            _ => return,
        }
    }
}

#[cfg(test)]
mod forward_lines_tests {
    use super::*;

    fn lines_of(input: &[u8]) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        forward_lines(input, tx);
        rx.into_iter().collect()
    }

    #[test]
    fn splits_trims_and_drops_an_unterminated_tail() {
        let got = lines_of(b"a b  \r\n\nsecond\xff\npartial");
        assert_eq!(got, vec!["a b".to_string(), String::new(), "second\u{fffd}".to_string()]);
    }

    /// `cargo test --release -p bend-tui bench_forward_lines -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_forward_lines() {
        let mut input = Vec::new();
        for i in 0..20_000 {
            input.extend_from_slice(format!("[tool] {} some ordinary wire line of text\n", i).as_bytes());
        }
        for _ in 0..4 {
            input.extend(std::iter::repeat_n(b'x', 256 * 1024));
            input.push(b'\n');
        }
        let t = std::time::Instant::now();
        let n = lines_of(&input).len();
        eprintln!("forward_lines: {} lines, {} bytes: {:.1} ms", n, input.len(), t.elapsed().as_secs_f64() * 1000.0);
    }
}

/// BISE-92: bise paints its own ground: no cell of a frame keeps the
/// terminal's default background, in any view, in both themes.
#[cfg(test)]
mod paint_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use serde_json::json;

    fn resets(app: &mut App) -> Vec<(u16, u16)> {
        let mut t = Terminal::new(TestBackend::new(120, 36)).unwrap();
        t.draw(|f| draw_frame(app, f)).unwrap();
        let b = t.backend().buffer();
        let mut out = Vec::new();
        for y in 0..36 {
            for x in 0..120 {
                let c = &b[(x, y)];
                if c.bg == Color::Reset || c.fg == Color::Reset {
                    out.push((x, y));
                }
            }
        }
        out
    }

    fn with_agents_and_a_card(app: &mut App) {
        let state = json!({"ev": "state", "agents": [
            {"name": "main", "main": true, "status": "idle"},
            {"name": "docs", "status": "working", "objective": "write the docs"},
        ], "cards": [
            {"id": 1, "kind": "question", "agent": "docs", "text": "v1 or v2 for the api docs?", "age_ms": 0},
        ]});
        sb::dispatch(app, &state.to_string());
        for l in ["sb you : ship it", "sb msg : docs → main : found it", "sb card : #1 question @docs : v1 or v2?"] {
            sb::dispatch(app, &json!({"ev": "line", "agent": "main", "line": l}).to_string());
        }
    }

    #[test]
    fn no_cell_keeps_the_terminals_background() {
        for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
            crate::theme::set_mode(mode);
            let mut app = crate::sb::bench::test_app();
            with_agents_and_a_card(&mut app);
            assert_eq!(resets(&mut app), vec![], "main, {mode:?}");
            // the card box above the composer
            let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
            sb::key(&mut app, &g, false);
            assert_eq!(resets(&mut app), vec![], "card, {mode:?}");
            // the / popup
            app.ed.text = "/".into();
            app.ed.cursor = 1;
            assert_eq!(resets(&mut app), vec![], "popup, {mode:?}");
            app.ed.text.clear();
            app.ed.cursor = 0;
            // /help
            sb::handle_input(&mut app, "/help");
            assert!(app.help.is_some());
            assert_eq!(resets(&mut app), vec![], "help, {mode:?}");
            app.help = None;
            // inside an agent
            sb::focus(&mut app, "docs");
            assert_eq!(resets(&mut app), vec![], "agent, {mode:?}");
            // every cell with no color of its own is the theme's ground
            let mut t = Terminal::new(TestBackend::new(40, 5)).unwrap();
            t.draw(|f| draw_frame(&mut app, f)).unwrap();
            assert!(t.backend().buffer().content.iter().any(|c| c.bg == crate::theme::bg()));
        }
        crate::theme::set_mode(crate::theme::Mode::Dark);
    }
}
