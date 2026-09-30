//! The mouse pointer's shape (BISE-272): a hand over what a click does
//! (links, file links, the rows that open or close, the panel's and the
//! inbox's rows, the palette's entries, back to the bottom), the text
//! cursor over the composer, `ns-resize` on the terminal panel's top
//! border (the one part a drag resizes), the default everywhere else.
//!
//! Each frame the draw says what is where ([`begin_frame`], [`region`]):
//! the regions of the frame in draw order, the last drawn wins (a popup
//! over a link covers it with [`Shape::Default`]). After the draw, the
//! loop asks [`wanted`] for the shape under the mouse and the backend
//! writes it as OSC 22 (`ESC ]22;<css name> ESC \`), only on a change
//! (`LinkBackend::set_pointer`), `default` again on exit.
//!
//! OSC 22 with CSS names is kitty's spec (0.31+), Ghostty's too; xterm
//! wants X cursor names, WezTerm, iTerm2 and Terminal.app do not have it.
//! So it is written only where the terminal says it is Ghostty or kitty,
//! never inside tmux (it does not pass it on); `BISE_POINTER=0` turns it
//! off, `BISE_POINTER=1` on (another terminal that has it).

use crate::app::{App, DragIn};
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// A pointer shape: its CSS name ([`Shape::name`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Shape {
    #[default]
    Default,
    /// a click does something here
    Pointer,
    /// you type here (a click places the cursor, a drag selects)
    Text,
    /// a drag resizes, up and down
    NsResize,
}

impl Shape {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Shape::Default => "default",
            Shape::Pointer => "pointer",
            Shape::Text => "text",
            Shape::NsResize => "ns-resize",
        }
    }
}

/// The OSC 22 that sets `s`.
pub(crate) fn osc22(s: Shape) -> String {
    format!("\x1b]22;{}\x1b\\", s.name())
}

// ---- the frame: what is where ----

thread_local! {
    static FRAME: RefCell<Vec<(Rect, Shape)>> = const { RefCell::new(Vec::new()) };
}

/// A new frame: nothing on screen has a shape yet.
pub(crate) fn begin_frame() {
    FRAME.with(|f| f.borrow_mut().clear());
}

/// `r` has the shape `s`, over what was drawn before it.
pub(crate) fn region(r: Rect, s: Shape) {
    if !r.is_empty() {
        FRAME.with(|f| f.borrow_mut().push((r, s)));
    }
}

/// The shape of the cell (x, y) in the last frame: the last region drawn
/// there, else the default.
pub(crate) fn at(x: u16, y: u16) -> Shape {
    FRAME.with(|f| {
        f.borrow()
            .iter()
            .rev()
            .find(|(r, _)| x >= r.x && x < r.right() && y >= r.y && y < r.bottom())
            .map_or(Shape::Default, |(_, s)| *s)
    })
}

/// The shape the pointer should have now: a drag keeps the shape it
/// started with (resizing the terminal panel, selecting in the composer);
/// a selection in the history, the terminal panel's own selection or a
/// program there that takes the mouse, and a terminal out of focus: the
/// default; else the frame's shape under the mouse.
pub(crate) fn wanted(app: &App) -> Shape {
    if app.focus_lost {
        return Shape::Default;
    }
    if app.term.resizing() {
        return Shape::NsResize;
    }
    match app.mouse.drag {
        Some(DragIn::Composer) => return Shape::Text,
        Some(DragIn::Feed { .. }) => return Shape::Default,
        None => {}
    }
    if app.term.mouse_held() {
        return Shape::Default;
    }
    app.pointer_at.map_or(Shape::Default, |(x, y)| at(x, y))
}

// ---- which terminals ----

/// OSC 22 in this terminal, from its environment (`var`): Ghostty or
/// kitty, outside tmux; `BISE_POINTER` decides first.
pub(crate) fn supported_by(var: impl Fn(&str) -> Option<String>) -> bool {
    let set = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    if let Some(v) = var("BISE_POINTER") {
        match v.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" => return false,
            "1" | "true" | "yes" | "on" => return true,
            _ => {}
        }
    }
    let term = var("TERM").unwrap_or_default();
    if set("TMUX") || term.starts_with("tmux") || term.starts_with("screen") {
        return false;
    }
    let program = var("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    program == "ghostty" || term == "xterm-ghostty" || term == "xterm-kitty" || set("KITTY_WINDOW_ID")
}

/// OSC 22 on for this process (tests: on, unless [`OFF`]).
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    {
        !OFF.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| supported_by(|k| std::env::var(k).ok()))
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A shape other than the default is on (for [`restore`]: a crash exit).
static SHOWN: AtomicBool = AtomicBool::new(false);

/// The backend wrote `s`.
pub(crate) fn wrote(s: Shape) {
    SHOWN.store(s != Shape::Default, Ordering::SeqCst);
}

/// The terminal given back (`crash::restore_terminal`): the default
/// shape again if another one is on.
pub(crate) fn restore(out: &mut impl std::io::Write) {
    if SHOWN.swap(false, Ordering::SeqCst) {
        let _ = out.write_all(osc22(Shape::Default).as_bytes());
    }
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
