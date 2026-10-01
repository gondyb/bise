//! `/voice`: voice mode's settings screen, in the /models layout (design
//! §6, plan §4.7), and the first voice mode's "who hears you" screen
//! (owner: voice-settings).
//!
//! The settings: listen, voice (provider · voice · ▸ hear it), speed,
//! read aloud, sounds, language, who hears you, and dictation (one
//! ctrl+r) on or off. A change is written to config.toml at once (`[voice]`,
//! [`super::config::save`]) and its row flashes. ▸ hear it says a sample
//! only when you press ⏎ on the voice row ([`Step::Hear`]); the tests
//! never do.
//!
//! The first voice mode (`seen_privacy` unset): who hears you, what is
//! kept (the words, never the audio), when it listens; `1 start voice
//! mode · 2 hold-to-talk only · 3 not now`.
//!
//! The run loop shows it over the whole screen like `/models`:
//! [`request`] then, at the next frame, [`take_request`] and [`show`].

use super::config::{self, ListenMode, ReadAloud, VoiceModeConfig, SPEED_MAX, SPEED_MIN};
use super::SayJob;
use crate::theme;
use crate::App;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use std::io;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// What the screen opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    /// `/voice`
    Settings,
    /// the first voice mode: who hears you, then start or not
    Privacy,
}

/// How the screen closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// the settings closed (esc)
    Closed,
    /// the first voice mode: start it
    Start,
    /// start it, listening only while space is held (saved: listen = hold)
    HoldOnly,
    /// not now: voice mode stays closed, the screen shows again next time
    NotNow,
}

static REQUEST: Mutex<Option<Open>> = Mutex::new(None);

/// Open the screen at the next frame (the run loop).
pub fn request(open: Open) {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(open);
}

pub fn take_request() -> Option<Open> {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The sentence ▸ hear it says.
pub const SAMPLE: &str = "hi, this is how i sound. i'll say the start of each answer, the rest stays on screen.";

/// How long a changed row flashes.
const FLASH: Duration = Duration::from_millis(1000);

/// The rows of the settings, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Listen,
    Voice,
    Speed,
    ReadAloud,
    Sounds,
    Language,
    Who,
    Dictation,
}

pub const ROWS: [Row; 8] =
    [Row::Listen, Row::Voice, Row::Speed, Row::ReadAloud, Row::Sounds, Row::Language, Row::Who, Row::Dictation];

impl Row {
    fn name(self) -> &'static str {
        match self {
            Row::Listen => "listen",
            Row::Voice => "voice",
            Row::Speed => "speed",
            Row::ReadAloud => "read aloud",
            Row::Sounds => "sounds",
            Row::Language => "language",
            Row::Who => "who hears you",
            Row::Dictation => "dictation",
        }
    }

    /// What the row is for, under the list for the row under the cursor.
    fn hint(self) -> &'static str {
        match self {
            Row::Listen => "auto: hands-free with headphones, hold space on speakers (it would hear itself).",
            Row::Voice => "the voice that answers. one voice for now; ⏎ says a sample.",
            Row::Speed => "how fast it talks, 0.8× to 1.6×.",
            Row::ReadAloud => "what the agent's messages say aloud; the whole message is always on screen.",
            Row::Sounds => "a soft click when it starts listening and when it sends.",
            Row::Language => "the language you speak; auto: detected. dictation uses it too.",
            Row::Who => "the companies that hear you, and what is kept.",
            Row::Dictation => "one ctrl+r: you talk, it types in the composer.",
        }
    }
}

/// Who hears you, for the screen's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    /// the voice role's provider ("Mistral"): it writes your words down
    pub listen: String,
    /// the TTS's provider; Err: why voice mode can't speak
    pub speak: Result<String, String>,
    /// the small-jobs model's provider: the spoken "on it" reads your words
    pub ack: String,
}

impl Who {
    pub fn of(setup: &bise_catalog::Setup, cfg: &VoiceModeConfig) -> Who {
        let name = |p: &str| setup.catalog.provider(p).map_or(p.to_string(), |x| x.name.clone());
        let mode = bise_catalog::voice::VoiceModeKeys {
            tts_model: (!cfg.tts_model.is_empty()).then(|| cfg.tts_model.clone()),
            ..Default::default()
        };
        let speak = setup.tts_name(&mode).map(|m| name(m.split('/').next().unwrap_or("")));
        Who {
            listen: name(&setup.voice_provider()),
            speak,
            ack: name(&setup.catalog.resolve(&setup.small_model).provider),
        }
    }

    /// The real setup.
    pub fn load(cfg: &VoiceModeConfig) -> Who {
        let home = bise_home::Home::from_env();
        Who::of(&bise_catalog::Setup::load(&home.config_file()), cfg)
    }

    /// One company or several: "Mistral", "Mistral and OpenAI".
    fn companies(&self) -> String {
        let mut v: Vec<&str> = vec![&self.listen];
        for p in [self.speak.as_deref().ok(), Some(self.ack.as_str())].into_iter().flatten() {
            if !p.is_empty() && !v.contains(&p) {
                v.push(p);
            }
        }
        match v.as_slice() {
            [one] => one.to_string(),
            [init @ .., last] => format!("{} and {}", init.join(", "), last),
            [] => String::new(),
        }
    }
}

/// What a key did.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Stay,
    /// `cfg` changed: write it
    Save,
    /// ▸ hear it: say [`SAMPLE`] with the voice and speed shown
    Hear,
    /// dictation on or off
    Dictation,
    Done(Out),
}

/// The screen's state.
#[derive(Clone, Debug)]
pub struct Screen {
    pub open: Open,
    /// the settings row, or the privacy option, under the cursor
    pub sel: usize,
    pub cfg: VoiceModeConfig,
    pub who: Who,
    /// dictation (one ctrl+r) is on
    pub dictation: bool,
    /// "who hears you" opened from the settings: read only, esc back
    pub reading: bool,
    /// a line over the keys (a failed save, why it can't speak)
    pub said: Option<String>,
    pub flash: Option<(Row, Instant)>,
}

/// The languages offered (more by config.toml: `[voice] language`).
const LANGUAGES: [Option<&str>; 3] = [None, Some("en"), Some("fr")];

fn language_name(l: Option<&str>) -> String {
    match l {
        None => "auto".into(),
        Some("en") => "English".into(),
        Some("fr") => "French".into(),
        Some(o) => o.to_string(),
    }
}

impl Screen {
    pub fn new(open: Open, cfg: VoiceModeConfig, who: Who, dictation: bool) -> Screen {
        Screen { open, sel: 0, cfg, who, dictation, reading: false, said: None, flash: None }
    }

    fn privacy(&self) -> bool {
        self.open == Open::Privacy || self.reading
    }

    fn row(&self) -> Row {
        ROWS[self.sel.min(ROWS.len() - 1)]
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) -> Step {
        if k.kind != KeyEventKind::Press {
            return Step::Stay;
        }
        self.said = None;
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return Step::Done(if self.open == Open::Privacy { Out::NotNow } else { Out::Closed });
        }
        if self.privacy() {
            return self.privacy_key(k);
        }
        let n = ROWS.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Step::Done(Out::Closed),
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel = (self.sel + n - 1) % n;
                Step::Stay
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.sel = (self.sel + 1) % n;
                Step::Stay
            }
            KeyCode::Left | KeyCode::Char('h') => self.change(-1, now),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => self.change(1, now),
            KeyCode::Enter => match self.row() {
                Row::Voice => match &self.who.speak {
                    Ok(_) => Step::Hear,
                    Err(e) => {
                        self.said = Some(e.clone());
                        Step::Stay
                    }
                },
                Row::Who => {
                    self.reading = true;
                    Step::Stay
                }
                _ => self.change(1, now),
            },
            _ => Step::Stay,
        }
    }

    fn privacy_key(&mut self, k: KeyEvent) -> Step {
        if self.reading {
            if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                self.reading = false;
            }
            return Step::Stay;
        }
        let pick = |i: usize| Step::Done([Out::Start, Out::HoldOnly, Out::NotNow][i]);
        match k.code {
            KeyCode::Char(c @ '1'..='3') => pick(c as usize - '1' as usize),
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel = (self.sel + 2) % 3;
                Step::Stay
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.sel = (self.sel + 1) % 3;
                Step::Stay
            }
            KeyCode::Enter => pick(self.sel.min(2)),
            KeyCode::Esc | KeyCode::Char('q') => Step::Done(Out::NotNow),
            _ => Step::Stay,
        }
    }

    /// ←→ on a row: the next value (wraps, speed stops at its ends).
    fn change(&mut self, d: i32, now: Instant) -> Step {
        let row = self.row();
        let c = &mut self.cfg;
        let cycle = |i: usize, n: usize| ((i as i32 + d).rem_euclid(n as i32)) as usize;
        match row {
            Row::Listen => {
                let i = ListenMode::ALL.iter().position(|m| *m == c.listen).unwrap_or(0);
                c.listen = ListenMode::ALL[cycle(i, 3)];
            }
            Row::ReadAloud => {
                let i = ReadAloud::ALL.iter().position(|m| *m == c.read_aloud).unwrap_or(0);
                c.read_aloud = ReadAloud::ALL[cycle(i, 3)];
            }
            Row::Speed => {
                let s = ((c.speed * 10.0).round() as i32 + d) as f32 / 10.0;
                let s = s.clamp(SPEED_MIN, SPEED_MAX);
                if (s - c.speed).abs() < 1e-4 {
                    return Step::Stay;
                }
                c.speed = s;
            }
            Row::Sounds => c.sounds = !c.sounds,
            Row::Language => {
                let i = LANGUAGES.iter().position(|l| *l == c.language.as_deref());
                // a language set in config.toml and not offered: next is auto
                let next = match i {
                    Some(i) => LANGUAGES[cycle(i, LANGUAGES.len())],
                    None => None,
                };
                c.language = next.map(str::to_string);
            }
            Row::Dictation => {
                self.flash = Some((row, now));
                return Step::Dictation;
            }
            // one voice for now; who hears you opens on ⏎
            Row::Voice | Row::Who => return Step::Stay,
        }
        self.flash = Some((row, now));
        Step::Save
    }

    // ---- the words ----

    fn value(&self, row: Row) -> (String, Color) {
        let c = &self.cfg;
        let d = dot();
        let text = theme::text();
        match row {
            Row::Listen => (
                match c.listen {
                    ListenMode::Auto => format!("auto {} hands-free with headphones, hold on speakers", d),
                    ListenMode::HandsFree => "hands-free".into(),
                    ListenMode::Hold => "hold space to talk".into(),
                },
                text,
            ),
            Row::Voice => match &self.who.speak {
                Ok(p) => {
                    let v = if c.voice.is_empty() { "default voice".to_string() } else { c.voice.clone() };
                    (format!("{} {} {}", p, d, v), text)
                }
                Err(_) => (format!("{} can't speak yet {} ⏎ says why", self.who.listen, d), theme::dim()),
            },
            Row::Speed => (format!("{:.1}×", c.speed), text),
            Row::ReadAloud => (
                match c.read_aloud {
                    ReadAloud::Needs => "what needs you + what you asked",
                    ReadAloud::All => "everything the agent writes",
                    ReadAloud::Nothing => "nothing: the words stay on screen",
                }
                .into(),
                text,
            ),
            Row::Sounds => ((if c.sounds { "on" } else { "off" }).into(), text),
            Row::Language => (language_name(c.language.as_deref()), text),
            Row::Who => (format!("{} {} the words are kept, never the audio", self.who.companies(), d), theme::dim()),
            Row::Dictation => (
                (if self.dictation { "on · ctrl+r: you talk, it types" } else { "off" }).replace('·', d),
                if self.dictation { text } else { theme::dim() },
            ),
        }
    }

    /// The screen's lines at width `w` (`now`: the flash).
    pub fn lines(&self, w: u16, now: Instant) -> Vec<Line<'static>> {
        if self.privacy() {
            return self.privacy_lines(w);
        }
        let dim = |t: String| Line::from(s(t, theme::dim()));
        let mut v = vec![title("voice mode"), dim("ctrl+r twice talks with the agent in view, esc leaves.".into())];
        v.push(Line::raw(""));
        let nw = ROWS.iter().map(|r| r.name().width()).max().unwrap_or(0) + 3;
        for (i, row) in ROWS.iter().enumerate() {
            let selected = i == self.sel;
            let flash = self.flash.is_some_and(|(r, at)| r == *row && now.duration_since(at) < FLASH);
            let lead = if flash {
                s(format!("{} ", theme::glyph(theme::G_DONE)), theme::accent())
            } else if selected {
                s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())
            } else {
                Span::raw("  ")
            };
            let name = s(pad(row.name(), nw), theme::text());
            let (value, color) = self.value(*row);
            let room = (w as usize).saturating_sub(2 + nw);
            let mut spans = vec![lead, if selected { name.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { name }];
            spans.push(s(cut(&value, room.saturating_sub(if *row == Row::Voice { 12 } else { 0 })), color));
            if *row == Row::Voice && self.who.speak.is_ok() {
                let play = if theme::ascii_mode() { ">" } else { "▸" };
                spans.push(s(format!("  {} hear it", play), if selected { theme::accent() } else { theme::dim() }));
            }
            v.push(Line::from(spans));
        }
        v.push(Line::raw(""));
        v.push(dim(format!("  {}: {}", self.row().name(), self.row().hint())));
        if let Some(t) = &self.said {
            v.push(Line::raw(""));
            v.push(Line::from(s(t.clone(), theme::error())));
        }
        v.push(Line::raw(""));
        let enter = match self.row() {
            Row::Voice => "{enter} hear it · ",
            Row::Who => "{enter} read · ",
            _ => "",
        };
        v.push(keyline(&format!("{{↑↓}} choose · {{←→}} change · {}{{esc}} back", enter)));
        v
    }

    fn privacy_lines(&self, w: u16) -> Vec<Line<'static>> {
        let who = &self.who;
        let dim = |t: String| Line::from(s(t, theme::dim()));
        let text = |t: String| Line::from(s(t, theme::text()));
        let mut v = vec![title("who hears you")];
        v.push(Line::raw(""));
        v.push(text(format!("{} writes down what you say.", who.listen)));
        match &who.speak {
            Ok(p) => v.push(text(format!("{} says the agent's answers.", p))),
            Err(e) => v.push(dim(format!("nobody says the answers: {}", e))),
        }
        v.push(text(format!("{} reads your words for the short \"on it\".", who.ack)));
        v.push(dim("then your words go to the agent, like a typed message.".into()));
        v.push(Line::raw(""));
        v.push(title("what is kept"));
        v.push(text("the words, in the thread. never the audio.".into()));
        v.push(Line::raw(""));
        v.push(title("when it listens"));
        v.push(text("only in voice mode: from ctrl+r twice to esc. m mutes.".into()));
        v.push(dim(match self.cfg.listen {
            ListenMode::Auto => "with headphones, hands-free; on speakers, while you hold space.".into(),
            ListenMode::HandsFree => "hands-free, headphones or not.".into(),
            ListenMode::Hold => "while you hold space.".into(),
        }));
        v.push(Line::raw(""));
        if self.reading {
            v.push(keyline("{esc} back"));
            return v;
        }
        let opts = ["start voice mode", "hold-to-talk only", "not now"];
        for (i, o) in opts.iter().enumerate() {
            let selected = i == self.sel;
            let lead = if selected { s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent()) } else { Span::raw("  ") };
            let label = s(format!("{} {}", i + 1, o), theme::text());
            v.push(Line::from(vec![lead, if selected { label.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { label }]));
        }
        v.push(Line::raw(""));
        v.push(keyline("{1-3} pick · {↑↓} choose · {enter} ok · {esc} not now"));
        let _ = w;
        v
    }
}

// ---- drawing ----

fn s(t: impl Into<String>, c: Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c))
}

fn title(t: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(t.into(), Style::default().fg(theme::text()).add_modifier(Modifier::BOLD)))
}

fn keyline(t: &str) -> Line<'static> {
    crate::onboarding::keyline(t)
}

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

fn pad(t: &str, w: usize) -> String {
    let n = t.width();
    if n >= w {
        format!("{} ", t)
    } else {
        format!("{}{}", t, " ".repeat(w - n))
    }
}

/// `t` cut to `w` cells, `…` at the end.
fn cut(t: &str, w: usize) -> String {
    if t.width() <= w {
        return t.to_string();
    }
    let mut out = String::new();
    for ch in t.chars() {
        if out.width() + ch.to_string().width() + 1 > w {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

/// The screen over `area`: the /models column (80 wide from 90 columns),
/// the block at 2/5 of the free rows.
pub fn draw_in(buf_frame: &mut Frame, screen: &Screen, now: Instant) {
    let area = buf_frame.area();
    let body = Rect { height: area.height.saturating_sub(3), ..area };
    let col = if area.width >= 90 {
        Rect { x: body.x + (body.width - 80) / 2, width: 80, ..body }
    } else {
        crate::onboarding::column(body)
    };
    let lines = screen.lines(col.width, now);
    let h = (lines.len() as u16).min(body.height);
    let y = body.y + body.height.saturating_sub(h) * 2 / 5;
    let r = Rect { y, height: body.bottom().saturating_sub(y), ..col };
    buf_frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), r);
}

// ---- the loop ----

/// Show the screen until it closes: keys, the hub's lines through `pump`,
/// a change saved at once, ▸ hear it through `hear` (the TTS and the
/// speaker; called only on the user's ⏎).
pub fn show(
    app: &mut App,
    terminal: &mut crate::links::Tui,
    open: Open,
    pump: &mut dyn FnMut(&mut App),
    hear: &mut dyn FnMut(SayJob, String),
) -> io::Result<Out> {
    use crossterm::event::{poll, read, Event};
    let cfg = config::load();
    let who = Who::load(&cfg);
    let mut screen = Screen::new(open, cfg, who, app.voice.enabled);
    let _ = terminal.clear();
    loop {
        pump(app);
        if app.should_quit {
            return Ok(Out::Closed);
        }
        let now = Instant::now();
        crate::theme_detect::sync_terminal_bg();
        terminal.draw(|f| {
            draw_in(f, &screen, now);
            theme::paint(f.buffer_mut());
            theme::asciify(f.buffer_mut());
        })?;
        if !poll(Duration::from_millis(50))? {
            continue;
        }
        let Some(Event::Key(k)) = crate::ctrlhint::for_handlers(read()?) else { continue };
        let step = screen.on_key(k, Instant::now());
        match apply(&mut screen, step, app, hear) {
            Some(out) => return Ok(out),
            None => continue,
        }
    }
}

/// What a step does outside the screen; Some: it closes.
fn apply(screen: &mut Screen, step: Step, app: &mut App, hear: &mut dyn FnMut(SayJob, String)) -> Option<Out> {
    match step {
        Step::Stay => None,
        Step::Save => {
            if let Err(e) = config::save(&screen.cfg) {
                screen.said = Some(format!("not saved: {}", e));
            }
            None
        }
        Step::Hear => {
            match config::say_job(&screen.cfg) {
                Ok(job) => hear(job, SAMPLE.to_string()),
                Err(e) => screen.said = Some(e),
            }
            None
        }
        Step::Dictation => match crate::input::toggle_voice(app, crate::voice::resolve_job) {
            Some(ev) => {
                crate::feed::push_event(&mut app.events, &mut app.cache, ev);
                screen.dictation = app.voice.enabled;
                None
            }
            // no voice model works yet: its picker opens over the feed
            None => Some(Out::Closed),
        },
        Step::Done(out) => {
            let saved = match out {
                Out::Start => {
                    screen.cfg.seen_privacy = true;
                    config::save(&screen.cfg)
                }
                Out::HoldOnly => {
                    screen.cfg.seen_privacy = true;
                    screen.cfg.listen = ListenMode::Hold;
                    config::save(&screen.cfg)
                }
                Out::Closed | Out::NotNow => Ok(()),
            };
            if let Err(e) = saved {
                crate::feed::push_event(&mut app.events, &mut app.cache, crate::wire::Ev::Warn(format!("voice mode: not saved: {}", e)));
            }
            Some(out)
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
