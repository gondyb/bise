//! bend-tui — Ratatui terminal UI for the Bend Unified Harness.
//!
//! Rust port of repl-tui (Ink). Same wire protocol; the presentation is
//! modeled on the REAL OpenCode TUI (packages/tui in the opencode repo):
//! no header bar — the screen is the conversation. Blocks breathe: a
//! blank line at every content transition, one column of margin on
//! each edge of the feed, blank rows separating the history from the
//! composer, and the user block paints its panel background the full
//! column. Status speaks in glyphs, not words: ✦ reasoning, ✓ ok,
//! ✗ fail, ▲ warning, ⟳ compaction, ≡ summary, ↳ preview. User
//! messages are
//! blocks with a colored left bar and a panel background; assistant
//! markdown renders in the OpenCode markdown colors; tool calls are
//! OpenCode inline tools (braille spinner while running, muted ✓ once
//! done, red ✗ on failure); the prompt is an OpenCode prompt (left
//! border, element background, agent/model meta row); commands filter
//! in an OpenCode autocomplete popup (split border, primary selection).
//! The status row carries the spinner + ctrl+c-to-interrupt hints.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode so
//! the UI stays scriptable — the same convention as the Ink version.

use crossterm::event::{
    poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
    EnableMouseCapture, Event, KeyCode, KeyboardEnhancementFlags,
    KeyModifiers, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph,
};
use ratatui::Frame;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

mod theme;
use theme::*;
mod wire;
use wire::*;
mod markdown;
use markdown::*;
mod code;
mod render;
use render::*;
mod feed;
use feed::*;
mod app;
use app::*;
mod commands;
pub use commands::HarnessInfo;
use commands::*;
mod ui;
use ui::*;
mod input;
use input::*;
mod sb;
mod skills;
mod emoji;
mod editor;
mod clipboard;
mod usage;
mod term;
mod feedsel;
mod keyprobe;
mod help;
mod voice;
#[cfg(test)]
mod voice_ui_tests;
#[cfg(test)]
mod composer_wrap_tests;
pub use keyprobe::keyprobe;
pub use sb::{run_switchboard, take_reexec};



// one wire line into the feed of the app (the focused view)
fn ingest_line(app: &mut App, line: String) {
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

fn run_tui(app: &mut App) -> io::Result<()> {
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

fn print_ev_of(ev: &Ev, debug: bool, width: usize) {
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
fn wait_idle(app: &mut App, max: Duration) {
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

fn run_line_mode(app: &mut App) -> io::Result<()> {
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
    let stream = TcpStream::connect((host.as_str(), port));
    let connected = true;
    let stream = match stream {
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
        connected,
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

    // "connected" updates when the reader ends: reflect it via a probe
    // (the channel closes when the socket closes)
    {
        let tx_probe = app.stream.as_ref().map(|_| ());
        let _ = tx_probe;
    }

    let interactive = io::stdout().is_terminal() && io::stdin().is_terminal();
    if interactive {
        run_tui(&mut app)
    } else {
        run_line_mode(&mut app)
    }?;

    // final drain of the channel in line mode: detect close
    while app.rx.try_recv().is_ok() {}
    let _ = connected;
    Ok(())
}

// ---- tests ----
#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn provider_retry_and_restart_render() {
        match parse_line("  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s") {
            Some(Ev::Warn(t)) => assert_eq!(
                t,
                "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s"
            ),
            _ => panic!("provider_retry must render as a warning"),
        }
        match parse_line("  obs: harness_restarted: exit status: 1 · bend: out of memory") {
            Some(Ev::Err(t)) => assert!(t.contains("bend: out of memory") && t.contains("restarted")),
            _ => panic!("harness_restarted must render as an error"),
        }
    }

    #[test]
    fn split_thinking_takes_every_span() {
        let t = "<think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok";
        let (think, vis) = split_thinking(t).unwrap();
        assert_eq!(think, "a\\nBENDSIG::s1\\nb\\nBENDSIG::s2");
        assert_eq!(vis, "ok");
        assert!(split_thinking("no markers").is_none());
        assert!(split_thinking("<think>open").is_none());
    }

    // the runtime's wire encoding, as emit_code_ann produces it
    fn wire_encode(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('\r', "\\R")
            .replace('\n', "\\N")
    }

    // proper JSON args, as the model emits them: quotes and backslashes
    // escaped inside the code string, newlines as \n
    fn json_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    // the feed receives tool_started, the tool annotations, then
    // tool_finished: the block must render under the merged line
    #[test]
    fn tool_code_block_renders_under_the_tool_line() {
        let code = "// orchestre les appels
async function main(): Promise<unknown> {
  const xs = [1, 2, 3];
  const s = \"une chaîne\";
  return xs.length + s.length;
}";
        let args = format!("{{\"code\": \"{}\"}}", json_escape(code));
        let wire_lines = vec![
            "  obs: tool_started #3".to_string(),
            format!("tool #3 run_typescript : {}", args),
            format!("tool_code #3 : {}", wire_encode(&args)),
            "  obs: tool_finished #3 ok".to_string(),
        ];
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in &wire_lines {
            let ev = parse_line(l).expect("parse");
            push_event(&mut events, &mut cache, ev);
        }
        let tool = events
            .iter()
            .find_map(|e| match e {
                Ev::Tool(td) => Some(td.clone()),
                _ => None,
            })
            .expect("the merged tool");
        assert_eq!(tool.id, 3);
        assert_eq!(tool.name.as_deref(), Some("run_typescript"));
        assert!(tool.code.is_some());
        let rows = ev_lines(&Ev::Tool(tool), 80);
        let joined: Vec<String> = rows
            .iter()
            .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        for (i, l) in joined.iter().enumerate() {
            println!("{:2} | {}", i, l);
        }
        assert!(joined[0].contains("run_typescript") && !joined[0].contains("orchestre"));
        assert!(joined.iter().any(|l| l.contains("╭─ typescript")));
        assert!(joined.iter().any(|l| l.contains("async function main")));
        assert!(joined.iter().any(|l| l.contains("une chaîne")));
        // one bordered row per source line, top and bottom included
        assert_eq!(joined.iter().filter(|l| l.contains('│')).count(), 6);
    }

    fn merged_tool(wire_lines: &[String]) -> ToolData {
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in wire_lines {
            let ev = parse_line(l).expect("parse");
            push_event(&mut events, &mut cache, ev);
        }
        events
            .iter()
            .find_map(|e| match e {
                Ev::Tool(td) => Some(td.clone()),
                _ => None,
            })
            .expect("the merged tool")
    }

    fn rows_text(rows: &[Line<'static>]) -> Vec<String> {
        rows.iter()
            .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect()
    }

    // bash gets the same block as run_typescript: the raw command (no
    // JSON), bash header, one row per line, no preview on the tool line
    #[test]
    fn a_long_source_block_folds_until_clicked() {
        let cmd: String = (1..=200).map(|i| format!("echo {}", i)).collect::<Vec<_>>().join("\n");
        let wire_lines = vec![
            "  obs: tool_started #4".to_string(),
            "tool #4 bash : echo".to_string(),
            format!("tool_code #4 : {}", wire_encode(&cmd)),
            "  obs: tool_finished #4 ok".to_string(),
        ];
        let mut tool = merged_tool(&wire_lines);
        let folded = rows_text(&ev_lines(&Ev::Tool(tool.clone()), 80));
        assert_eq!(folded.iter().filter(|l| l.contains("│ echo")).count(), CODE_FOLD_SHOW);
        assert!(folded.iter().any(|l| l.contains("160 more lines")));
        tool.expanded = true;
        let whole = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        assert_eq!(whole.iter().filter(|l| l.contains("│ echo")).count(), 200);
        assert!(!whole.iter().any(|l| l.contains("more lines")));
    }

    #[test]
    fn bash_code_block_renders_under_the_tool_line() {
        let cmd = "# compte les fichiers
for f in *.rs; do
  echo \"$f: $(wc -l < \"$f\")\"
done | sort -n";
        let wire_lines = vec![
            "  obs: tool_started #4".to_string(),
            format!("tool #4 bash : {}", cmd.replace('\n', " ")),
            format!("tool_code #4 : {}", wire_encode(cmd)),
            "  obs: tool_finished #4 ok".to_string(),
        ];
        let tool = merged_tool(&wire_lines);
        assert_eq!(tool.name.as_deref(), Some("bash"));
        let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        for (i, l) in joined.iter().enumerate() {
            println!("{:2} | {}", i, l);
        }
        // the tool line names the tool; the source shows only in the block
        assert!(joined[0].contains("bash") && !joined[0].contains("compte"));
        assert!(joined.iter().any(|l| l.contains("╭─ bash")));
        assert!(joined.iter().any(|l| l.contains("for f in *.rs; do")));
        assert!(joined.iter().any(|l| l.contains("done | sort -n")));
        // one bordered row per source line (4 lines)
        assert_eq!(joined.iter().filter(|l| l.contains('│')).count(), 4);
    }

    // a bash tool line without tool_code (an older runtime) keeps the
    // flattened args preview and renders no block
    #[test]
    fn bash_without_code_has_no_block() {
        let tool = merged_tool(&[
            "  obs: tool_started #5".to_string(),
            "tool #5 bash : ls -la".to_string(),
            "  obs: tool_finished #5 ok".to_string(),
        ]);
        let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
        assert!(joined[0].contains("ls -la"));
        assert!(!joined.iter().any(|l| l.contains('╭')));
    }

    fn style_of(lines: &[Vec<Span<'static>>], tok: &str) -> Style {
        lines
            .iter()
            .flatten()
            // adjacent same-style spans merge (whitespace included)
            .find(|s| s.content.trim() == tok)
            .map(|s| s.style)
            .unwrap_or_else(|| panic!("no span {:?}", tok))
    }

    // the lines LAWS.resume_replays_history pins on the Bend side: the
    // client rebuilds the conversation, tools merged, no fake duration
    #[test]
    fn resume_history_rebuilds_the_feed() {
        let wire = [
            "history   obs: compaction_done: court...",
            "history you : salut\\nça va",
            "history   obs: tool_started #7",
            "history tool #7 bash : ls -la",
            "history tool_code #7 : ls -la",
            "history tool_result #7 fail : boom",
            "history   obs: tool_finished #7 failed",
            "history   obs: assistant: fini",
            "history   obs: tool_started #9",
            "history tool #9 bash : pwd",
            "history tool_code #9 : pwd",
            "history   obs: tool_finished #9 failed",
            "history injected : [notification] bg 0 done",
            "  obs: session_restored: 7 messages",
        ];
        let mut events: Vec<Ev> = Vec::new();
        let mut cache: Vec<Option<EventRows>> = Vec::new();
        for l in wire {
            let (line, replayed) = strip_history(l);
            let ev = if replayed { parse_history_line(line) } else { parse_line(line) };
            let Some(ev) = ev else { continue };
            let finished = match &ev {
                Ev::Tool(td) if !matches!(td.state, ToolState::Run) => Some(td.id),
                _ => None,
            };
            push_event(&mut events, &mut cache, ev);
            if let (true, Some(id)) = (replayed, finished) {
                hide_replayed_elapsed(&mut events, &mut cache, id);
            }
        }
        let kinds: Vec<String> = events
            .iter()
            .map(|e| match e {
                Ev::Compacted(t) => format!("compacted {}", t),
                Ev::You(t) => format!("you {}", t),
                Ev::Assistant(t) => format!("assistant {}", t),
                Ev::Tool(td) => format!(
                    "tool {} {} ok={} code={} elapsed={:?}",
                    td.id,
                    td.name.clone().unwrap_or_default(),
                    matches!(td.state, ToolState::Ok),
                    td.code.is_some(),
                    td.elapsed
                ),
                Ev::Info(t) => format!("info {}", t),
                _ => "other".to_string(),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "compacted court...".to_string(),
                "you salut\nça va".to_string(),
                "tool 7 bash ok=false code=true elapsed=Some(\"\")".to_string(),
                "assistant fini".to_string(),
                "tool 9 bash ok=false code=true elapsed=Some(\"\")".to_string(),
                "info injected · [notification] bg 0 done".to_string(),
                "info session restored · 7 messages".to_string(),
            ]
        );
    }

    // a long line wraps inside the box: nothing is lost, every row has
    // the box width, continuation rows have an empty gutter
    #[test]
    fn long_code_lines_wrap_inside_the_box() {
        let cmd = "cd /Users/someone/lab/project && cargo test -p some-crate --release 2>&1 | rg 'test result|FAILED|panicked' | head -20
echo ok";
        let rows = rows_text(&code_block_lines(cmd, CodeLang::Bash, &ToolState::Ok, 50));
        for r in &rows {
            println!("{}", r);
        }
        let widths: Vec<usize> = rows.iter().map(|r| r.width()).collect();
        assert!(widths.iter().all(|&w| w == widths[0]), "aligned box: {:?}", widths);
        // top + 2 source lines, the first on several rows + bottom
        assert!(rows.len() > 4);
        // the text between the gutter bars reassembles the source
        let body: String = rows[1..rows.len() - 1]
            .iter()
            .map(|r| {
                // between the gutter's closing bar and the right border
                let first = r.find('│').unwrap();
                let second = first + 3 + r[first + 3..].find('│').unwrap();
                let last = r.rfind('│').unwrap();
                r[second + 3..last].to_string()
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(body.replace(' ', ""), cmd.replace(['\n', ' '], ""));
        // line 2 starts on a numbered row; continuation rows are blank
        assert!(rows[1].contains(" 1 │ cd "));
        assert!(rows[2].starts_with("  │   │ "));
        assert!(rows.iter().any(|r| r.contains(" 2 │ echo ok")));
    }

    // a token longer than the row breaks hard, never overflows
    #[test]
    fn wrap_breaks_long_tokens() {
        let spans = vec![Span::raw("x".repeat(25))];
        let rows = wrap_code_line(&spans, 10);
        let ws: Vec<usize> = rows.iter().map(|(_, w)| *w).collect();
        assert_eq!(ws, vec![10, 10, 5]);
        assert_eq!(wrap_code_line(&[], 10).len(), 1);
    }

    // apply_patch renders as a diff: summary in the tool line, the
    // envelope dropped, file headers, colored bands to the border
    #[test]
    fn apply_patch_renders_a_diff_block() {
        let patch = "*** Begin Patch
*** Update File: core/obs.bend
@@ def show_obs
   case T.Assistant{text}:
-    old line
+    new line
+    another line
*** Add File: notes.md
+# Notes
*** End Patch
";
        let tool = merged_tool(&[
            "  obs: tool_started #6".to_string(),
            format!("tool #6 apply_patch : {}", patch.replace('\n', " ")),
            format!("tool_code #6 : {}", wire_encode(patch)),
            "tool_result #6 ok : Done!".to_string(),
            "  obs: tool_finished #6 ok".to_string(),
        ]);
        let lines = ev_lines(&Ev::Tool(tool), 70);
        let rows = rows_text(&lines);
        for r in &rows {
            println!("{}", r);
        }
        assert!(rows[0].contains("apply_patch") && rows[0].ends_with("· core/obs.bend +2 −1, notes.md +1"));
        assert!(rows.iter().any(|r| r.contains("╭─ diff")));
        assert!(!rows.iter().any(|r| r.contains("Begin Patch") || r.contains("End Patch")));
        assert!(rows.iter().any(|r| r.contains("│ ~ core/obs.bend")));
        assert!(rows.iter().any(|r| r.contains("│ + notes.md · new")));
        // no line-number gutter in a diff
        assert!(rows.iter().any(|r| r.contains("│ -    old line")));
        // box aligned
        let boxed: Vec<usize> = rows.iter().filter(|r| r.contains('│') || r.contains('╭') || r.contains('╰')).map(|r| r.width()).collect();
        assert!(boxed.iter().all(|&w| w == boxed[0]), "{:?}", boxed);
        // the added line's band reaches the right border (padding included)
        let add_row = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("+    new line")))
            .expect("the added row");
        let n = add_row.spans.len();
        assert_eq!(add_row.spans[n - 2].style.bg, Some(DIFF_ADD_BG));
        let del_row = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("-    old line")))
            .expect("the removed row");
        assert!(del_row.spans.iter().any(|s| s.style.bg == Some(DIFF_DEL_BG)));
    }

    #[test]
    fn bash_highlighting_classifies_tokens() {
        let hl = highlight_bash(
            "# note
X=1 grep -n \"$HOME\" f.txt | wc -l && for i in 1 2; do echo $i; done",
        );
        let fg = |tok: &str| style_of(&hl, tok).fg;
        assert_eq!(fg("# note"), Some(SYNTAX_COMMENT));
        assert_eq!(fg("X"), Some(SYNTAX_NUMBER)); // assignment name
        assert_eq!(fg("grep"), Some(SYNTAX_FUNC)); // command word
        assert_eq!(fg("-n"), Some(HEAD)); // option
        assert_eq!(fg("$HOME"), Some(SYNTAX_NUMBER)); // expansion in quotes
        assert_eq!(fg("f.txt"), Some(TEXT)); // argument
        assert_eq!(fg("wc"), Some(SYNTAX_FUNC)); // command after a pipe
        assert_eq!(fg("for"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("in"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("do"), Some(SYNTAX_KEYWORD));
        assert_eq!(fg("echo"), Some(SYNTAX_FUNC)); // command after "do"
        assert_eq!(fg("done"), Some(SYNTAX_KEYWORD));
        assert_eq!(hl.len(), 2); // one span list per source line
    }
}

#[cfg(test)]
mod harness_info_tests {
    use super::HarnessInfo;

    // the exact string LAWS.bend pins for Rt.info_line (law
    // info_line_format): the two sides of the contract agree
    const BEND_LINE: &str = "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt";

    #[test]
    fn parses_the_line_bend_prints() {
        let info = HarnessInfo::parse(BEND_LINE).expect("parses");
        assert_eq!(info.model, "claude-opus-5-5");
        assert_eq!(info.threshold, "800000");
        assert_eq!(info.steer_path, "/tmp/bend-steer-7.txt");
        assert_eq!(info.interrupt_path, "/tmp/bend-interrupt-7.txt");
    }

    #[test]
    fn finds_the_line_in_a_repl_log() {
        let log = format!("{}\nbend-harness LIVE REPL on 127.0.0.1:7 ...\n[mcp] connector index written\n", BEND_LINE);
        assert_eq!(HarnessInfo::from_log(&log).expect("found").model, "claude-opus-5-5");
    }

    #[test]
    fn rejects_an_incomplete_line() {
        assert!(HarnessInfo::parse("harness-info model=m threshold=1").is_none());
        assert!(HarnessInfo::parse("bend-harness LIVE REPL on 127.0.0.1:7").is_none());
    }
}

#[cfg(test)]
mod popup_tests {
    use super::popup_top;

    #[test]
    fn the_selection_stays_in_view() {
        assert_eq!(popup_top(0, 5, 8), 0);
        assert_eq!(popup_top(4, 5, 8), 0);
        assert_eq!(popup_top(7, 12, 8), 0);
        assert_eq!(popup_top(8, 12, 8), 1);
        assert_eq!(popup_top(11, 12, 8), 4);
        assert_eq!(popup_top(99, 12, 8), 4); // clamped like the selection
    }
}

#[cfg(test)]
mod emoji_width_tests {
    use super::*;
    use crate::code::wrap_code_line;
    use crate::editor::layout_input;
    use unicode_width::UnicodeWidthStr;

    fn row_widths(input: &str, inner: usize) -> Vec<usize> {
        layout_input(input, inner)
            .iter()
            .map(|r| r.iter().filter(|c| !c.newline).map(|c| c.w).sum())
            .collect()
    }

    #[test]
    fn composer_rows_count_emojis_as_two_columns() {
        // 5 emojis at 6 columns: 3 per row, the 4th never splits a row
        assert_eq!(row_widths("👏👏👏👏👏", 6), vec![6, 4]);
        // an emoji that does not fit the row end moves to the next row
        assert_eq!(row_widths("abcde👏", 6), vec![5, 2]);
        for inner in 2..12 {
            for input in ["a👏b👍🏽c❤️d👨‍👩‍👧e🇫🇷", "👏👏👏👏👏👏👏", "x y 👏👏 z\nq👏"] {
                assert!(row_widths(input, inner).iter().all(|&w| w <= inner), "{input} @ {inner}");
            }
        }
    }

    #[test]
    fn composer_cells_are_graphemes_with_char_indices() {
        let rows = layout_input("a👍🏽❤️👨‍👩‍👧b", 40);
        let cells: Vec<(usize, &str, usize)> = rows[0].iter().map(|c| (c.ci, c.text, c.w)).collect();
        assert_eq!(
            cells,
            vec![(0, "a", 1), (1, "👍🏽", 2), (3, "❤️", 2), (5, "👨‍👩‍👧", 2), (10, "b", 1), (11, " ", 1)]
        );
        // the end cursor slot sits after the last char
        assert!(rows[0].last().unwrap().newline);
    }

    #[test]
    fn feed_wrap_counts_graphemes_as_drawn() {
        let w = |l: &Line| l.spans.iter().map(|s| s.content.width()).sum::<usize>();
        let rows = wrap_line(Line::from("👨‍👩‍👧 👨‍👩‍👧 👨‍👩‍👧"), 8);
        assert_eq!(rows.len(), 1, "3 × 2 cols + 2 spaces fit 8");
        let rows = wrap_line(Line::from("👏👏👏👏👏"), 4);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| w(r) <= 4));
        let rows = wrap_code_line(&[Span::raw("❤️❤️❤️")], 4);
        assert_eq!(rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec![4, 2]);
    }
}
