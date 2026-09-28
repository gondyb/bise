//! The single-agent client: connect to the REPL, ingest its wire lines,
//! and run either the interactive loop (ratatui) or line mode (piped).

use crate::*;
use crossterm::event::{
    poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
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

pub(crate) fn run_tui(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
    // a multi-line paste arrives as ONE Event::Paste instead of a
    // keystroke storm where every Enter would send
    let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
    // the kitty keyboard protocol reports Shift+Enter distinctly (the
    // plain terminal encodings cannot); terminals without support just
    // ignore the push, and Ctrl+J remains the universal fallback
    let _ = crossterm::execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    loop {
        // the hub replays whole feeds on connect: the lines are taken in
        // slices of a few ms, a frame in between, so the UI never waits
        // for a replay to end
        let slice = std::time::Instant::now();
        let mut backlog = false;
        loop {
            if slice.elapsed() >= Duration::from_millis(12) {
                backlog = true;
                break;
            }
            match app.rx.try_recv() {
                Ok(line) => {
                    if app.sb.is_some() {
                        sb::dispatch(app, &line);
                    } else {
                        ingest_line(app, line);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // the REPL process is gone: a /reload exits it (the
                    // parent respawns and reconnects), a crash does not.
                    // Either way this UI is dead — stop, let the parent
                    // decide.
                    app.connected = false;
                    app.should_quit = true;
                    break;
                }
            }
        }
        if app.should_quit {
            break;
        }
        // switchboard Ctrl+O: a shell in the agent's directory; the TUI
        // gives the terminal back when it exits
        if let Some(dir) = sb::take_shell(app) {
            let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
            ratatui::restore();
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            println!("shell in {} — exit to return to Switchboard", dir);
            let _ = std::process::Command::new(shell).current_dir(&dir).status();
            terminal = ratatui::init();
            let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
            let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
            let _ = terminal.clear();
        }
        pump_voice(app);
        terminal.draw(|f| {
            if app.sb.is_some() {
                sb::draw_sb(app, f)
            } else {
                draw(app, f)
            }
        })?;
        // the level meter moves every 50 ms while recording (Vibe's poll)
        let wait = if backlog {
            Duration::ZERO
        } else if app.voice.active() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(80)
        };
        if poll(wait)? {
            match read()? {
                Event::Mouse(m) => {
                    let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
                    on_mouse(app, &m, term_h);
                }
                Event::Paste(text) => on_paste(app, &text),
                Event::Key(k) if on_key(app, &k) => break,
                _ => {}
            }
        }
        app.tick = app.tick.wrapping_add(1);
    }
    // no orphan shell
    app.term.shutdown();
    let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
    let _ = crossterm::execute!(io::stdout(), DisableBracketedPaste);
    let _ = crossterm::execute!(io::stdout(), PopKeyboardEnhancementFlags);
    ratatui::restore();
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
            eprintln!("connexion impossible : {}", e);
            eprintln!("start the harness with ./run.sh");
            return Ok(());
        }
    };
    let reader = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut r = reader;
        let mut bytes: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match r.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    bytes.push(byte[0]);
                    while let Some(i) = bytes.iter().position(|b| *b == b'\n') {
                        let line = String::from_utf8_lossy(&bytes[..i]).trim_end().to_string();
                        bytes.drain(..=i);
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    let _ = stream.set_nodelay(true);

    let mut app = App {
        connected: true,
        term: term::Term::default(),
        help: None,
        debug,
        line_tools: std::collections::HashMap::new(),
        follow: true,
        anchor: (0, 0),
        scroll: 0,
        vis_events: Vec::new(),
        vis_rows: Vec::new(),
        feed_x: 0,
        feed_sel: None,
        unseen: 0,
        tail_visible: true,
        bottom_bar_rect: None,
        cache: Vec::new(),
        win: Default::default(),
        // line mode renders without a frame: the terminal width (or a
        // sane default) sizes the code blocks; interactive mode
        // overwrites this every frame
        area_w: crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(100)
            .max(40),
        area_h: 24,
        events: Vec::new(),
        last_line_at: None,
        show_thinking: false,
        interrupt_requested: false,
        pending: false,
        ed: editor::Editor::default(),
        composer: ComposerArea::default(),
        flash: None,
        voice: voice::Voice::live(voice::load_voice_enabled()),
        voice_note: None,
        mouse: MouseState::default(),
        popup_sel: 0,
        popup_dismissed: None,
        history: Vec::new(),
        tick: 0,
        info,
        host,
        port,
        session_id,
        stream: Some(stream),
        rx,
        should_quit: false,
        sb: None,
    };

    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        run_tui(&mut app)
    } else {
        run_line_mode(&mut app)
    }
}
