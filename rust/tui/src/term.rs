//! The embedded terminal: Ctrl+` shows a real shell (a PTY, `$SHELL` in
//! the workspace) at the bottom of the window and gives it the keyboard;
//! Ctrl+` again hides it (the shell keeps running) and the composer gets
//! the keys back. The shell is spawned on the first show, respawned on a
//! show after it exited, and killed when the TUI exits.
//!
//! Pure parts (tested): `is_toggle`, `key_bytes`, `split`. The rest is
//! the PTY plumbing: a reader thread feeds a vt100 parser that the
//! `tui-term` widget draws.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tui_term::widget::PseudoTerminal;

const SCROLLBACK: usize = 5000;
const MIN_ROWS: u16 = 5;

/// Ctrl+`. With the kitty DISAMBIGUATE flag it arrives as '`' + Ctrl; a
/// legacy terminal (tmux without extended keys) sends NUL for it, which
/// crossterm reads as Ctrl+Space (also Ctrl+@ and Ctrl+2).
pub(crate) fn is_toggle(k: &KeyEvent) -> bool {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let others = k.modifiers - KeyModifiers::CONTROL - KeyModifiers::SHIFT;
    ctrl && others.is_empty() && matches!(k.code, KeyCode::Char('`') | KeyCode::Char(' ') | KeyCode::Char('@'))
}

/// The bytes a key sends to the shell, xterm style. `app_cursor`: the
/// program asked for the application cursor keys (ESC O A…).
pub(crate) fn key_bytes(k: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let m = k.modifiers;
    let alt = m.contains(KeyModifiers::ALT);
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let shift = m.contains(KeyModifiers::SHIFT);
    // the xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl
    let param = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;
    let esc = |alt: bool, mut b: Vec<u8>| {
        if alt {
            b.insert(0, 0x1b);
        }
        b
    };
    // arrows, Home/End: ESC [ X, ESC O X in application mode, ESC [1;m X
    let letter = |c: u8| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[1;{}{}", param, c as char).into_bytes()
        } else if app_cursor {
            vec![0x1b, b'O', c]
        } else {
            vec![0x1b, b'[', c]
        }
    };
    // Insert/Delete/PgUp/PgDn/F5+: ESC [ n ~, ESC [ n;m ~
    let tilde = |n: u8| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[{};{}~", n, param).into_bytes()
        } else {
            format!("\x1b[{}~", n).into_bytes()
        }
    };
    Some(match k.code {
        KeyCode::Char(c) if ctrl => {
            let b = match c.to_ascii_lowercase() {
                c @ 'a'..='z' => c as u8 - b'a' + 1,
                ' ' | '@' | '2' | '`' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '_' | '-' | '/' | '7' => 0x1f,
                '?' | '8' => 0x7f,
                _ => return None,
            };
            esc(alt, vec![b])
        }
        KeyCode::Char(c) => esc(alt, c.to_string().into_bytes()),
        KeyCode::Enter => esc(alt, vec![b'\r']),
        KeyCode::Tab if shift => b"\x1b[Z".to_vec(),
        KeyCode::Tab => esc(alt, vec![b'\t']),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace if ctrl => esc(alt, vec![0x08]),
        KeyCode::Backspace => esc(alt, vec![0x7f]),
        KeyCode::Esc => esc(alt, vec![0x1b]),
        KeyCode::Up => letter(b'A'),
        KeyCode::Down => letter(b'B'),
        KeyCode::Right => letter(b'C'),
        KeyCode::Left => letter(b'D'),
        KeyCode::Home => letter(b'H'),
        KeyCode::End => letter(b'F'),
        KeyCode::Insert => tilde(2),
        KeyCode::Delete => tilde(3),
        KeyCode::PageUp => tilde(5),
        KeyCode::PageDown => tilde(6),
        KeyCode::F(n @ 1..=4) => {
            let c = b'P' + (n - 1);
            if param > 1 {
                format!("\x1b[1;{}{}", param, c as char).into_bytes()
            } else {
                vec![0x1b, b'O', c]
            }
        }
        KeyCode::F(n @ 5..=12) => tilde([15, 17, 18, 19, 20, 21, 23, 24][(n - 5) as usize]),
        _ => return None,
    })
}

/// The screen split: the area left for the app above, the panel below
/// (`pct` % of the height, at least MIN_ROWS + borders, the app keeps 8).
pub(crate) fn split(full: Rect, pct: u16) -> (Rect, Rect) {
    let want = (full.height as u32 * pct as u32 / 100) as u16;
    let h = want.max(MIN_ROWS + 2).min(full.height.saturating_sub(8)).max(3.min(full.height));
    let top = Rect { height: full.height - h, ..full };
    let panel = Rect { y: full.y + full.height - h, height: h, ..full };
    (top, panel)
}

struct Pty {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    exited: Arc<AtomicBool>,
    size: (u16, u16),
}

impl Pty {
    fn spawn(shell: &str, cwd: &str, rows: u16, cols: u16) -> Result<Pty, String> {
        let pair = native_pty_system()
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let mut cmd = CommandBuilder::new(shell);
        if std::path::Path::new(cwd).is_dir() {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let exited = Arc::new(AtomicBool::new(false));
        let (p, x) = (parser.clone(), exited.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut p) = p.lock() {
                            p.process(&buf[..n]);
                        }
                    }
                }
            }
            x.store(true, Ordering::SeqCst);
        });
        Ok(Pty { parser, writer, master: pair.master, child, exited, size: (rows, cols) })
    }

    fn alive(&mut self) -> bool {
        !self.exited.load(Ordering::SeqCst) && matches!(self.child.try_wait(), Ok(None))
    }

    fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if self.size != (rows, cols) && rows > 0 && cols > 0 {
            self.size = (rows, cols);
            let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
            if let Ok(mut p) = self.parser.lock() {
                p.set_size(rows, cols);
            }
        }
    }

    /// SIGHUP (what closing a terminal does), then SIGKILL if the shell
    /// is still there after a short grace.
    fn kill(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.child.kill();
        for _ in 0..20 {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Some(pid) = self.child.process_id() {
            // no libc dependency: the kill command does it
            let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        let _ = self.child.wait();
    }
}

/// The panel's state, held by the App.
pub(crate) struct Term {
    shown: bool,
    pty: Option<Pty>,
    /// the panel height, % of the window
    pct: u16,
    /// where the panel was drawn (mouse hits)
    area: Option<Rect>,
    /// dragging the top border
    resizing: bool,
    /// rows scrolled back into the history
    scroll: usize,
    error: Option<String>,
}

impl Default for Term {
    fn default() -> Self {
        Term { shown: false, pty: None, pct: 30, area: None, resizing: false, scroll: 0, error: None }
    }
}

impl Term {
    pub(crate) fn shown(&self) -> bool {
        self.shown
    }

    /// Show (spawning the shell in `cwd` if none runs) or hide.
    pub(crate) fn toggle(&mut self, cwd: &str) {
        if self.shown {
            self.shown = false;
            self.resizing = false;
            return;
        }
        self.shown = true;
        self.scroll = 0;
        let alive = self.pty.as_mut().is_some_and(|p| p.alive());
        if !alive {
            if let Some(mut p) = self.pty.take() {
                p.kill();
            }
            // the real size comes with the first draw
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            match Pty::spawn(&shell, cwd, 10, 80) {
                Ok(p) => {
                    self.pty = Some(p);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("cannot start the shell: {}", e)),
            }
        }
    }

    /// A key while shown: to the shell (the caller handles the toggle).
    /// False when hidden: the key is the app's.
    pub(crate) fn key(&mut self, k: &KeyEvent) -> bool {
        if !self.shown {
            return false;
        }
        let Some(pty) = self.pty.as_mut() else { return true };
        // Shift+PgUp/PgDn scroll the history, like most terminals
        if k.modifiers == KeyModifiers::SHIFT && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown) {
            let page = (pty.size.0 as usize / 2).max(1);
            self.scroll = if k.code == KeyCode::PageUp {
                self.scroll + page
            } else {
                self.scroll.saturating_sub(page)
            };
            return true;
        }
        let app_cursor = pty.parser.lock().map(|p| p.screen().application_cursor()).unwrap_or(false);
        if let Some(b) = key_bytes(k, app_cursor) {
            self.scroll = 0;
            pty.send(&b);
        }
        true
    }

    /// A paste while shown goes to the shell (bracketed when it asked).
    pub(crate) fn paste(&mut self, text: &str) -> bool {
        if !self.shown {
            return false;
        }
        if let Some(pty) = self.pty.as_mut() {
            let bracketed = pty.parser.lock().map(|p| p.screen().bracketed_paste()).unwrap_or(false);
            let text = text.replace("\r\n", "\r").replace('\n', "\r");
            if bracketed {
                pty.send(format!("\x1b[200~{}\x1b[201~", text).as_bytes());
            } else {
                pty.send(text.as_bytes());
            }
            self.scroll = 0;
        }
        true
    }

    /// The mouse over the panel: the wheel scrolls the history, the top
    /// border drags to resize. True when the panel took the event.
    pub(crate) fn mouse(&mut self, m: &MouseEvent, screen_h: u16) -> bool {
        if !self.shown {
            return false;
        }
        if self.resizing {
            match m.kind {
                MouseEventKind::Drag(MouseButton::Left) => {
                    let h = screen_h.saturating_sub(m.row).max(1);
                    self.pct = ((h as u32 * 100 / screen_h.max(1) as u32) as u16).clamp(10, 90);
                }
                MouseEventKind::Up(_) => self.resizing = false,
                _ => {}
            }
            return true;
        }
        let Some(r) = self.area else { return false };
        let inside = m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        if !inside {
            return false;
        }
        match m.kind {
            MouseEventKind::ScrollUp => self.scroll += 3,
            MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_sub(3),
            MouseEventKind::Down(MouseButton::Left) if m.row == r.y => self.resizing = true,
            _ => {}
        }
        true
    }

    /// Draw the panel when shown; returns the area left for the app.
    pub(crate) fn draw(&mut self, frame: &mut Frame, full: Rect) -> Rect {
        if !self.shown {
            self.area = None;
            return full;
        }
        let (top, panel) = split(full, self.pct);
        self.area = Some(panel);
        let inner = Block::default().borders(Borders::ALL).inner(panel);
        let mut title = " terminal · ctrl+` hide ".to_string();
        let border = Style::default().fg(Color::DarkGray);
        let Some(pty) = self.pty.as_mut() else {
            let msg = self.error.clone().unwrap_or_default();
            frame.render_widget(
                ratatui::widgets::Paragraph::new(msg)
                    .block(Block::default().borders(Borders::ALL).title(title).border_style(border)),
                panel,
            );
            return top;
        };
        pty.resize(inner.height, inner.width);
        let alive = pty.alive();
        let Ok(mut parser) = pty.parser.lock() else { return top };
        parser.set_scrollback(self.scroll);
        // the clamped offset: the history may be shorter than asked
        self.scroll = parser.screen().scrollback();
        if self.scroll > 0 {
            title = format!(" terminal · ↑ {} lines · ctrl+` hide ", self.scroll);
        }
        if !alive {
            title = " terminal · the shell exited · ctrl+` twice for a new one ".to_string();
        }
        let block = Block::default().borders(Borders::ALL).title(title).border_style(border);
        let mut w = PseudoTerminal::new(parser.screen()).block(block);
        if self.scroll > 0 || !alive {
            w = w.cursor(tui_term::widget::Cursor::default().visibility(false));
        }
        frame.render_widget(w, panel);
        parser.set_scrollback(0);
        top
    }

    /// Kill the shell (the TUI exits).
    pub(crate) fn shutdown(&mut self) {
        if let Some(mut p) = self.pty.take() {
            p.kill();
        }
        self.shown = false;
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ---- the App glue (called from the run loop in lib.rs) ----

fn cwd(app: &crate::App) -> String {
    crate::sb::workspace(app)
        .or_else(|| std::env::current_dir().ok().map(|d| d.to_string_lossy().to_string()))
        .unwrap_or_else(|| ".".into())
}

/// A key event: Ctrl+` toggles; while shown every other key goes to the
/// shell. True when the terminal took the key.
pub(crate) fn on_key(app: &mut crate::App, k: &KeyEvent) -> bool {
    if k.kind != crossterm::event::KeyEventKind::Press {
        return app.term.shown();
    }
    if is_toggle(k) {
        let dir = cwd(app);
        app.term.toggle(&dir);
        return true;
    }
    app.term.key(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn toggle_key() {
        assert!(is_toggle(&k(KeyCode::Char('`'), KeyModifiers::CONTROL)));
        // legacy NUL, as crossterm reads it
        assert!(is_toggle(&k(KeyCode::Char(' '), KeyModifiers::CONTROL)));
        assert!(is_toggle(&k(KeyCode::Char('@'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)));
        assert!(!is_toggle(&k(KeyCode::Char('`'), KeyModifiers::NONE)));
        assert!(!is_toggle(&k(KeyCode::Char(' '), KeyModifiers::NONE)));
        assert!(!is_toggle(&k(KeyCode::Char('`'), KeyModifiers::CONTROL | KeyModifiers::ALT)));
        assert!(!is_toggle(&k(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }

    fn b(code: KeyCode, m: KeyModifiers) -> Vec<u8> {
        key_bytes(&k(code, m), false).unwrap()
    }

    #[test]
    fn plain_and_control_keys() {
        assert_eq!(b(KeyCode::Char('a'), KeyModifiers::NONE), b"a");
        assert_eq!(b(KeyCode::Char('A'), KeyModifiers::SHIFT), b"A");
        assert_eq!(b(KeyCode::Char('é'), KeyModifiers::NONE), "é".as_bytes());
        assert_eq!(b(KeyCode::Char('c'), KeyModifiers::CONTROL), vec![3]);
        assert_eq!(b(KeyCode::Char('d'), KeyModifiers::CONTROL), vec![4]);
        assert_eq!(b(KeyCode::Char('L'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), vec![12]);
        assert_eq!(b(KeyCode::Char('['), KeyModifiers::CONTROL), vec![0x1b]);
        assert_eq!(b(KeyCode::Char('b'), KeyModifiers::ALT), b"\x1bb");
        assert_eq!(b(KeyCode::Enter, KeyModifiers::NONE), b"\r");
        assert_eq!(b(KeyCode::Tab, KeyModifiers::NONE), b"\t");
        assert_eq!(b(KeyCode::BackTab, KeyModifiers::SHIFT), b"\x1b[Z");
        assert_eq!(b(KeyCode::Backspace, KeyModifiers::NONE), vec![0x7f]);
        assert_eq!(b(KeyCode::Backspace, KeyModifiers::ALT), vec![0x1b, 0x7f]);
        assert_eq!(b(KeyCode::Esc, KeyModifiers::NONE), vec![0x1b]);
        assert!(key_bytes(&k(KeyCode::Char('é'), KeyModifiers::CONTROL), false).is_none());
    }

    #[test]
    fn cursor_and_function_keys() {
        assert_eq!(b(KeyCode::Up, KeyModifiers::NONE), b"\x1b[A");
        assert_eq!(key_bytes(&k(KeyCode::Up, KeyModifiers::NONE), true).unwrap(), b"\x1bOA");
        assert_eq!(b(KeyCode::Left, KeyModifiers::ALT), b"\x1b[1;3D");
        assert_eq!(b(KeyCode::Right, KeyModifiers::CONTROL), b"\x1b[1;5C");
        assert_eq!(b(KeyCode::Home, KeyModifiers::NONE), b"\x1b[H");
        assert_eq!(b(KeyCode::End, KeyModifiers::SHIFT), b"\x1b[1;2F");
        assert_eq!(b(KeyCode::Delete, KeyModifiers::NONE), b"\x1b[3~");
        assert_eq!(b(KeyCode::PageUp, KeyModifiers::NONE), b"\x1b[5~");
        assert_eq!(b(KeyCode::PageDown, KeyModifiers::CONTROL), b"\x1b[6;5~");
        assert_eq!(b(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP");
        assert_eq!(b(KeyCode::F(5), KeyModifiers::NONE), b"\x1b[15~");
        assert_eq!(b(KeyCode::F(12), KeyModifiers::NONE), b"\x1b[24~");
    }

    #[test]
    fn hidden_panel_leaves_keys_to_the_app() {
        let mut t = Term::default();
        assert!(!t.key(&k(KeyCode::Char('a'), KeyModifiers::NONE)));
        assert!(!t.paste("x"));
    }

    #[test]
    fn split_keeps_the_app_usable() {
        let full = Rect::new(0, 0, 100, 40);
        let (top, panel) = split(full, 30);
        assert_eq!(panel.height, 12);
        assert_eq!(top.height + panel.height, 40);
        assert_eq!(panel.y, 28);
        // a tall panel leaves the app 8 rows; a tiny one keeps 5 rows of shell
        assert_eq!(split(full, 90).1.height, 32);
        assert_eq!(split(full, 1).1.height, 7);
    }

    #[test]
    fn shell_runs_and_keeps_state_while_hidden() {
        // a clean /bin/sh, not the user's $SHELL: rc files (zsh, prompts)
        // can take seconds under load and zle may drop or bracket input
        let mut t = Term::default();
        t.pty = Some(Pty::spawn("/bin/sh", "/tmp", 10, 80).unwrap());
        t.toggle("/tmp");
        assert!(t.shown());
        let screen = |t: &Term| t.pty.as_ref().unwrap().parser.lock().unwrap().screen().contents();
        // wait with a deadline (generous: the whole suite runs in parallel)
        let wait_for = |t: &Term, what: &str| {
            let t0 = std::time::Instant::now();
            while !screen(t).contains(what) && t0.elapsed().as_secs() < 30 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let s = screen(t);
            assert!(s.contains(what), "no {:?} on the screen:\n{}", what, s);
        };
        t.paste("X=kept; echo hi-$((40+2))\n");
        wait_for(&t, "\nhi-42");
        t.toggle("/tmp");
        assert!(!t.shown());
        t.toggle("/tmp");
        assert!(t.shown());
        t.paste("echo $X\n");
        wait_for(&t, "\nkept");
        let pid = t.pty.as_ref().unwrap().child.process_id().unwrap();
        t.shutdown();
        let gone = !std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(gone, "the shell survived the shutdown");
    }
}
