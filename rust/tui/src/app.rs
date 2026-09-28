//! The single-agent client state: `App` (feed, composer, connection)
//! and the composer / mouse geometry of the last frame.

use crate::feed::*;
use crate::wire::*;
use crate::*;

// ---- app state ----

pub(crate) struct App {
    pub(crate) connected: bool,
    // the embedded terminal (Ctrl+`)
    pub(crate) term: term::Term,
    // the /help or /shortcuts overlay, when open
    pub(crate) help: Option<help::Overlay>,
    pub(crate) debug: bool,
    // line mode holds running tools until they finish so the printed
    // line carries the merged annotations (name, args, result)
    pub(crate) line_tools: std::collections::HashMap<u32, ToolData>,
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
    pub(crate) info: HarnessInfo,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) session_id: String,
    pub(crate) stream: Option<TcpStream>,
    pub(crate) rx: Receiver<String>,
    pub(crate) should_quit: bool,
    /// Switchboard mode (projects/switchboard): the hub connection and
    /// the feeds out of focus.
    pub(crate) sb: Option<sb::Sb>,
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
    /// A fresh screen: empty feed following the tail, empty composer.
    /// The connection fields (`info`, `host`, `port`, `stream`, `sb`)
    /// start empty; callers set theirs with struct update syntax.
    pub(crate) fn new(
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
            line_tools: std::collections::HashMap::new(),
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
            info: HarnessInfo::default(),
            host: String::new(),
            port: 0,
            session_id,
            stream: None,
            rx,
            should_quit: false,
            sb: None,
            attachments: Vec::new(),
            queued: Vec::new(),
        }
    }

    pub(crate) fn send(&mut self, line: &str) {
        self.pending = true;
        // the socket protocol is line-oriented: real newlines in the
        // composer escape to a literal backslash-n (the REPL unescapes
        // the say/steer text; the message carries the real newlines)
        let wire = line.replace('\n', "\\n");
        if let Some(s) = self.stream.as_mut() {
            let _ = s.write_all(format!("{}\n", wire).as_bytes());
        }
    }

    // line mode: one printed line per finished tool, carrying the merged
    // annotations; sub-calls print live as they complete
    pub(crate) fn feed_line(&mut self, line: &str) {
        if line == "--- idle" {
            self.pending = false;
        }
        // every wire line moves the timing reference: a thinking
        // duration is the delta from the previous line's arrival
        let now = std::time::Instant::now();
        let ms = self
            .last_line_at
            .map_or(0, |t0| now.duration_since(t0).as_millis());
        self.last_line_at = Some(now);
        let (line, replayed) = strip_history(line);
        let parsed = if replayed {
            parse_history_line(line)
        } else {
            parse_line(line)
        };
        let Some(ev) = parsed else { return };
        match ev {
            Ev::Tool(td) if matches!(td.state, ToolState::Run) => {
                self.line_tools.insert(td.id, td);
            }
            Ev::ToolInfo { id, name, args } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.name = Some(name);
                    td.args = Some(args);
                }
            }
            Ev::ToolResult { id, ok, preview } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.result = Some((ok, preview));
                }
            }
            Ev::ToolCode { id, code } => {
                if let Some(td) = self.line_tools.get_mut(&id) {
                    td.code = Some(code);
                }
            }
            Ev::Tool(td) => {
                if let Some(mut held) = self.line_tools.remove(&td.id) {
                    held.state = td.state;
                    if held.elapsed.is_none() {
                        held.elapsed = Some(fmt_elapsed(held.started));
                    }
                    print_ev_of(&Ev::Tool(held), self.debug, self.area_w);
                } else {
                    print_ev_of(&Ev::Tool(td), self.debug, self.area_w);
                }
            }
            ev2 @ (Ev::TurnDone | Ev::Idle) => {
                // abandoned running tools get one final line
                let mut held: Vec<ToolData> = self.line_tools.drain().map(|(_, td)| td).collect();
                held.sort_by_key(|td| td.id);
                for mut td in held {
                    td.state = ToolState::Fail;
                    td.elapsed = Some(fmt_elapsed(td.started));
                    if td.result.is_none() {
                        td.result = Some((false, "interrompu".to_string()));
                    }
                    print_ev_of(&Ev::Tool(td), self.debug, self.area_w);
                }
                print_ev_of(&ev2, self.debug, self.area_w);
            }
            Ev::Assistant(t) => {
                // the line mode shows the same collapsed section: the
                // raw reasoning never prints (unless --debug)
                match split_thinking(&t) {
                    Some((think, vis)) => {
                        print_ev_of(
                            &Ev::Thinking {
                                ms,
                                text: think.to_string(),
                                open: self.debug,
                            },
                            self.debug,
                            self.area_w,
                        );
                        if !vis.trim().is_empty() {
                            print_ev_of(&Ev::Assistant(vis.to_string()), self.debug, self.area_w);
                        }
                    }
                    None => print_ev_of(&Ev::Assistant(t), self.debug, self.area_w),
                }
            }
            other => print_ev_of(&other, self.debug, self.area_w),
        }
    }
}
