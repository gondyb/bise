//! The client state: `App` (the feed in focus, the composer, the hub
//! connection) and the composer / mouse geometry of the last frame.

use crate::feed::*;
use crate::*;

// ---- app state ----

pub(crate) struct App {
    pub(crate) connected: bool,
    // the embedded terminal (Ctrl+`)
    pub(crate) term: term::Term,
    // the /help or /shortcuts overlay, when open
    pub(crate) help: Option<help::Overlay>,
    pub(crate) debug: bool,
    // feed scrollback: follow means stick to the bottom (any scroll up
    // turns it off, End/enter turn it back on). A pinned view is anchored
    // on its first row, (event, row in the event): new content never
    // moves it, and a frame only builds the rows it shows (no sum over
    // the whole history). `scroll` is the move asked since the last
    // frame, in rows; the frame applies it.
    pub(crate) follow: bool,
    pub(crate) anchor: (usize, usize),
    pub(crate) scroll: isize,
    // the event shown on each row of the feed by the last frame (clicks)
    pub(crate) vis_events: Vec<usize>,
    /// the row, among its event's rows, each feed row shows
    pub(crate) vis_rows: Vec<usize>,
    /// the screen column of the feed's first text column
    pub(crate) feed_x: u16,
    /// The screen row of the feed's first row (under the Switchboard
    /// header and the "inside an agent" line; 0 without).
    pub(crate) feed_y: u16,
    /// the in-app selection in the feed
    pub(crate) feed_sel: Option<feedsel::FeedSel>,
    // activity that arrived while pinned (shown by the back-to-bottom bar)
    pub(crate) unseen: usize,
    pub(crate) tail_visible: bool,
    pub(crate) bottom_bar_rect: Option<ratatui::layout::Rect>,
    // wrapped rows per event, keyed by event index (the codex layout
    // cache: rebuild on mutation, width change, or live-elapsed tools)
    pub(crate) cache: Vec<Option<EventRows>>,
    /// Switchboard: which part of the agent's transcript the feed holds.
    pub(crate) win: sb::FeedWindow,
    pub(crate) area_w: usize,
    pub(crate) area_h: usize,
    pub(crate) events: Vec<Ev>,
    // when the last wire line arrived (thinking duration = the delta to
    // the assistant line) and the ctrl+o state of new thinking sections
    pub(crate) last_line_at: Option<std::time::Instant>,
    pub(crate) show_thinking: bool,
    // a Ctrl+C interrupt is in flight (until the dying turn's idle):
    // a second Ctrl+C quits instead of interrupting again
    pub(crate) interrupt_requested: bool,
    pub(crate) pending: bool,
    /// the composer: text, cursor, selection, undo, history recall
    pub(crate) ed: editor::Editor,
    /// where the last frame drew the composer's text (mouse, Up/Down rows)
    pub(crate) composer: ComposerArea,
    /// a short note in the status row ("copied 12 chars") and when
    pub(crate) flash: Option<(String, std::time::Instant)>,
    /// the mouse gesture in progress (selection drags, multi-clicks)
    pub(crate) mouse: MouseState,
    /// speech-to-text (Ctrl+R, /voice)
    pub(crate) voice: voice::Voice,
    /// a voice notice in the status row ("no speech detected") and when
    pub(crate) voice_note: Option<(String, std::time::Instant)>,
    pub(crate) popup_sel: usize,
    /// The composer text the user closed the `@` popup on (Esc): the
    /// popup stays closed until the text changes.
    pub(crate) popup_dismissed: Option<String>,
    pub(crate) history: Vec<String>,
    pub(crate) tick: u32,
    /// the working gust's motion (BISE-107): set by the draw loop, still
    /// until then (and in tests)
    pub(crate) motion: crate::gust::Motion,
    /// the terminal lost the focus (focus reporting): the gust stands still
    pub(crate) focus_lost: bool,
    /// zen while you type (BISE-121): fed by the loop, read by the draw
    pub(crate) zen: crate::zen::Zen,
    /// the last key went to the composer's own arms (an edit, a move, a
    /// newline), not to an app shortcut: set by `on_key`, read by zen
    /// (BISE-128)
    pub(crate) key_in_composer: bool,
    pub(crate) session_id: String,
    pub(crate) rx: Receiver<String>,
    pub(crate) should_quit: bool,
    /// Switchboard (projects/switchboard): the hub connection, the
    /// agents, the cards and the feeds out of focus.
    pub(crate) sb: sb::Sb,
    /// the images attached in the composer (`[Image #N]`, attach.rs)
    pub(crate) attachments: Vec<crate::attach::Attachment>,
    /// messages queued for after the turn (BISE-89), this feed's
    pub(crate) queued: Vec<crate::queue::Queued>,
}

/// The composer's text area in the last frame: its screen origin, its
/// width in columns, its visible rows and the first layout row shown.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ComposerArea {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) top: usize,
}

/// What a left press started: a selection in the composer (or none),
/// and the last press for double/triple clicks.
#[derive(Debug, Default, Clone)]
pub(crate) struct MouseState {
    pub(crate) drag: Option<DragIn>,
    pub(crate) last_press: Option<(std::time::Instant, u16, u16)>,
    pub(crate) clicks: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragIn {
    Composer,
    /// a press in the feed; `moved` once it selects (a drag, a double
    /// or triple click) rather than clicks
    Feed { moved: bool },
}

impl MouseState {
    /// Counts this press: 1, 2 (double) or 3 (triple click), for a press
    /// at the same cell within 400 ms of the last one.
    pub(crate) fn press(&mut self, x: u16, y: u16, now: std::time::Instant) -> u8 {
        let again = self
            .last_press
            .is_some_and(|(t, px, py)| px == x && py == y && now.duration_since(t) < Duration::from_millis(400));
        self.clicks = if again { self.clicks % 3 + 1 } else { 1 };
        self.last_press = Some((now, x, y));
        self.clicks
    }
}

impl ComposerArea {
    /// The char index under the screen cell (x, y), if inside the text.
    pub(crate) fn hit(&self, text: &str, x: u16, y: u16, clamp: bool) -> Option<usize> {
        let inside = x >= self.x.saturating_sub(1)
            && (x as usize) < self.x as usize + self.w
            && y >= self.y
            && (y as usize) < self.y as usize + self.h;
        if !inside && !clamp {
            return None;
        }
        let rows = editor::layout_input(text, self.w);
        let dy = y as isize - self.y as isize;
        let last = rows.len().saturating_sub(1) as isize;
        let row = (self.top as isize + dy).clamp(0, last) as usize;
        let col = x.saturating_sub(self.x) as usize;
        Some(editor::ci_at(&rows, row, col))
    }
}

impl App {
    /// A fresh screen: empty feed following the tail, empty composer,
    /// main in focus, fed by the hub lines of `rx`.
    pub(crate) fn new(
        sb: sb::Sb,
        rx: std::sync::mpsc::Receiver<String>,
        debug: bool,
        area_w: usize,
        voice: crate::voice::Voice,
        session_id: String,
    ) -> App {
        App {
            connected: true,
            term: crate::term::Term::default(),
            help: None,
            debug,
            follow: true,
            anchor: (0, 0),
            scroll: 0,
            vis_events: Vec::new(),
            vis_rows: Vec::new(),
            feed_x: 0,
            feed_y: 0,
            feed_sel: None,
            unseen: 0,
            tail_visible: true,
            bottom_bar_rect: None,
            cache: Vec::new(),
            win: Default::default(),
            area_w,
            area_h: 24,
            events: Vec::new(),
            last_line_at: None,
            show_thinking: false,
            interrupt_requested: false,
            pending: false,
            ed: crate::editor::Editor::default(),
            composer: ComposerArea::default(),
            flash: None,
            voice,
            voice_note: None,
            mouse: MouseState::default(),
            popup_sel: 0,
            popup_dismissed: None,
            history: Vec::new(),
            tick: 0,
            motion: crate::gust::Motion::Still,
            focus_lost: false,
            zen: crate::zen::Zen::default(),
            key_in_composer: false,
            session_id,
            rx,
            should_quit: false,
            sb,
            attachments: Vec::new(),
            queued: Vec::new(),
        }
    }
}
