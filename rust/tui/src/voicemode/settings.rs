//! `/voice`: voice's one settings screen, in the /models layout (design
//! §6, plan §4.7 and §8 #2, #7), and the first voice mode's "who hears
//! you" screen (owner: voice-settings2).
//!
//! The settings, one screen for dictation and voice mode: dictation (one
//! ctrl+r) on or off, speech to text (the voice role's model: ←→ the
//! models of the providers with a key, ⏎ the picker for the others and
//! the keys, then back here), voice (Mistral's voices, [`voices`]: name ·
//! language · gender, ▸ hear it), language (auto, or one the voices
//! speak: the transcription and what is said), listen, speed, read aloud,
//! sounds, who hears you. `/voice setup` (typed) and `/models`' voice row
//! open it on speech to text (from `/models`, esc goes back there). A
//! change is written to config.toml at once (`[voice]`,
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
use super::sample::Player;
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

#[path = "voices.rs"]
pub mod voices;
use voices::Voice;

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
/// The row the next settings screen opens on (None: the first).
static START: Mutex<Option<Row>> = Mutex::new(None);

/// Open the screen at the next frame (the run loop).
pub fn request(open: Open) {
    request_at(open, ROWS[0]);
}

/// `/voice setup` (typed: the menu no longer offers it): the screen, the
/// cursor on speech to text.
pub fn request_stt() {
    request_at(Open::Settings, Row::Stt);
}

/// Opened from `/models`' voice row (voice-menu, designer): esc goes
/// back to `/models`. Kept while the screen hands over to the voice
/// picker and comes back; taken when the screen closes for good.
static FROM_MODELS: Mutex<bool> = Mutex::new(false);

/// `/models` → voice: the screen on speech to text, esc back to `/models`.
pub fn request_from_models() {
    *FROM_MODELS.lock().unwrap_or_else(|e| e.into_inner()) = true;
    request_stt();
}

fn take_from_models() -> bool {
    std::mem::take(&mut *FROM_MODELS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The request statics are the process's: one test at a time on them
/// (these tests and `/models`' voice row's).
#[cfg(test)]
pub(crate) fn test_serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

fn request_at(open: Open, row: Row) {
    *START.lock().unwrap_or_else(|e| e.into_inner()) = Some(row);
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(open);
}

pub fn take_request() -> Option<Open> {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The sentence ▸ hear it says, in the language it is set to (`fr`:
/// French; else English).
pub fn sample(lang: Option<&str>) -> &'static str {
    match lang {
        Some("fr") => "bonjour, voici ma voix. je lis les réponses de l'agent à voix haute, et espace m'arrête.",
        _ => "hi, this is how i sound. i read the agent's answers aloud, and space stops me.",
    }
}

/// How long a changed row flashes.
const FLASH: Duration = Duration::from_millis(1000);

/// The rows of the settings, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Dictation,
    /// speech to text: the voice role's model
    Stt,
    Voice,
    Language,
    Listen,
    Speed,
    ReadAloud,
    Sounds,
    Who,
}

pub const ROWS: [Row; 9] = [
    Row::Dictation,
    Row::Stt,
    Row::Voice,
    Row::Language,
    Row::Listen,
    Row::Speed,
    Row::ReadAloud,
    Row::Sounds,
    Row::Who,
];

impl Row {
    fn name(self) -> &'static str {
        match self {
            Row::Dictation => "dictation",
            Row::Stt => "speech to text",
            Row::Voice => "voice",
            Row::Language => "language",
            Row::Listen => "listen",
            Row::Speed => "speed",
            Row::ReadAloud => "read aloud",
            Row::Sounds => "sounds",
            Row::Who => "who hears you",
        }
    }

    /// What the row is for, under the list for the row under the cursor.
    fn hint(self) -> &'static str {
        match self {
            Row::Dictation => "one ctrl+r: you talk, it types in the composer.",
            Row::Stt => "the model that writes down what you say, for dictation and voice mode. ⏎ for another provider or a key.",
            Row::Voice => "the voice that answers. ⏎ says a sample.",
            Row::Language => "the language you speak, and the one it answers in. auto detects it.",
            Row::Listen => "auto: hands-free with headphones. on speakers you hold space, or it would hear itself.",
            Row::Speed => "how fast it talks, 0.8× to 1.6×.",
            Row::ReadAloud => "what the agent's messages say aloud; the whole message is always on screen.",
            Row::Sounds => "a soft wind when your turn is sent and when it is done talking.",
            Row::Who => "the companies that hear you, and what is kept.",
        }
    }
}

/// The speech-to-text row's choices: the voice role's model and the
/// models ←→ goes through (the providers with a key; their own pick
/// first: Transcribe 3 on Mistral).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stt {
    /// "provider/id"
    pub model: String,
    /// "provider/id", the current one included
    pub options: Vec<String>,
    /// provider id → its name ("mistral" → "Mistral")
    pub names: Vec<(String, String)>,
    /// BISE_VOICE_MODEL decides: a change is saved but not used
    pub env: bool,
}

impl Default for Stt {
    fn default() -> Stt {
        let m = "mistral/voxtral-transcribe-3".to_string();
        Stt { model: m.clone(), options: vec![m], names: vec![("mistral".into(), "Mistral".into())], env: false }
    }
}

impl Stt {
    pub fn of(setup: &bise_catalog::Setup, keys: &bise_catalog::auth::Keys) -> Stt {
        let c = &setup.catalog;
        let mut options: Vec<String> = Vec::new();
        let mut names: Vec<(String, String)> = Vec::new();
        for p in c.stt_providers().filter(|p| p.needs.is_empty() && !p.hidden) {
            names.push((p.id.clone(), p.name.clone()));
            if !p.key_env.is_empty() && keys.for_provider(p).is_none() {
                continue;
            }
            let mut v: Vec<String> = c.models.iter().filter(|m| m.provider == p.id && m.stt).map(|m| m.name()).collect();
            if let Some(i) = v.iter().position(|m| m.split_once('/').is_some_and(|(_, id)| id == p.voice_model)) {
                let m = v.remove(i);
                v.insert(0, m);
            }
            options.extend(v);
        }
        let model = setup.voice.model.clone();
        if !options.contains(&model) {
            options.insert(0, model.clone());
        }
        Stt { model, options, names, env: setup.voice.from == "BISE_VOICE_MODEL" }
    }

    /// The real setup and keys.
    pub fn load() -> Stt {
        let home = bise_home::Home::from_env();
        let setup = bise_catalog::Setup::load(&home.config_file());
        let store = bise_catalog::auth::Store::read(&home.auth_file()).unwrap_or_default();
        let files = bise_catalog::auth::EnvFile::read_all(&home.env_files());
        let env = |k: &str| std::env::var(k).ok();
        let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &files };
        Stt::of(&setup, &keys)
    }

    /// `Mistral · Voxtral Transcribe 3` (designer: a name, never an id)
    fn line(&self, d: &str) -> String {
        match self.model.split_once('/') {
            Some((p, id)) => {
                let name = self.names.iter().find(|(i, _)| i == p).map_or(p, |(_, n)| n.as_str());
                format!("{} {} {}", name, d, model_name(id))
            }
            None => model_name(&self.model),
        }
    }
}

/// A speech model's id as a name: `voxtral-transcribe-3` → `Voxtral
/// Transcribe 3`, `gpt-transcribe` → `GPT Transcribe`, `nova-3` → `Nova 3`.
pub fn model_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "gpt" | "tts" | "stt" | "asr" => w.to_uppercase(),
            _ => {
                let mut c = w.chars();
                c.next().map(|h| h.to_uppercase().chain(c).collect()).unwrap_or_default()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// A voice's name as the screen says it, never its id (designer):
/// `en_paul_neutral` → `Paul, neutral`; empty: the default voice's.
pub fn voice_name(id: &str) -> String {
    let id = if id.trim().is_empty() { super::tts::DEFAULT_VOICE } else { id.trim() };
    // a language prefix (`en_`, `fr_`) is not part of the name
    let mut parts: Vec<&str> = id.split(['_', '-']).filter(|p| !p.is_empty()).collect();
    if parts.len() > 1 && parts[0].len() == 2 && parts[0].chars().all(|c| c.is_ascii_lowercase()) {
        parts.remove(0);
    }
    let Some((first, rest)) = parts.split_first() else {
        return id.to_string();
    };
    let mut c = first.chars();
    let name: String = c.next().map(|h| h.to_uppercase().chain(c).collect()).unwrap_or_default();
    if rest.is_empty() {
        name
    } else {
        format!("{}, {}", name, rest.join(" "))
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

    /// Who hears you, for the row (designer): the speech-to-text
    /// provider, and the voice's when it is another one: "Mistral",
    /// "Mistral hears you, OpenAI speaks".
    fn hears(&self) -> String {
        match self.speak.as_deref() {
            Ok(p) if !p.is_empty() && p != self.listen => format!("{} hears you, {} speaks", self.listen, p),
            _ => self.listen.clone(),
        }
    }
}

/// What a key did.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Stay,
    /// `cfg` changed: write it
    Save,
    /// the speech-to-text model changed (`stt.model`): write the voice
    /// role
    SaveStt,
    /// ⏎ on speech to text: the voice picker (other providers, keys),
    /// then this screen again
    PickStt,
    /// ▸ hear it: say [`sample`] with the voice and speed shown
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
    /// the speech-to-text row's model and choices
    pub stt: Stt,
    /// Mistral's voices (the session's list, [`voices::state`])
    pub voices: voices::State,
    /// "who hears you" opened from the settings: read only, esc back
    pub reading: bool,
    /// a line over the keys (a failed save, why it can't speak)
    pub said: Option<String>,
    pub flash: Option<(Row, Instant)>,
}

fn language_name(l: Option<&str>) -> String {
    l.map_or_else(|| "auto".into(), voices::language_name)
}

impl Screen {
    pub fn new(open: Open, cfg: VoiceModeConfig, who: Who, dictation: bool) -> Screen {
        Screen {
            open,
            sel: 0,
            cfg,
            who,
            dictation,
            stt: Stt::default(),
            voices: voices::State::Idle,
            reading: false,
            said: None,
            flash: None,
        }
    }

    /// The cursor on `row`.
    pub fn at(&mut self, row: Row) {
        self.sel = ROWS.iter().position(|r| *r == row).unwrap_or(0);
    }

    /// The voices listed, None while there is no list.
    fn voice_list(&self) -> Option<&[Voice]> {
        match &self.voices {
            voices::State::Ready(v) if !v.is_empty() => Some(v),
            _ => None,
        }
    }

    /// The voice set, from the list when it is in it.
    fn voice(&self) -> Option<&Voice> {
        self.voice_list()?.iter().find(|v| v.id == self.cfg.voice)
    }

    /// The language row's choices: auto, the voices' languages, and the
    /// one config.toml names when it is none of them.
    fn languages(&self) -> Vec<Option<String>> {
        let mut v: Vec<Option<String>> = vec![None];
        v.extend(voices::languages(self.voice_list().unwrap_or(&[])).into_iter().map(Some));
        if let Some(l) = &self.cfg.language {
            if !v.contains(&Some(l.clone())) {
                v.push(Some(l.clone()));
            }
        }
        v
    }

    /// The language ▸ hear it speaks: the one set, else the voice's.
    pub fn sample_language(&self) -> Option<String> {
        self.cfg
            .language
            .as_deref()
            .map(voices::base_language)
            .or_else(|| self.voice().and_then(|v| v.langs().into_iter().next()))
            .or_else(|| self.cfg.voice.split_once('_').map(|(p, _)| p.to_string()))
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
                Row::Stt => Step::PickStt,
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
                let langs = self.languages();
                let i = langs.iter().position(|l| *l == self.cfg.language).unwrap_or(0);
                self.cfg.language = langs[cycle(i, langs.len())].clone();
                // a voice that can't speak the language: the first one that can
                if let (Some(l), Some(list)) = (self.cfg.language.clone(), self.voice_list()) {
                    let l = voices::base_language(&l);
                    if !self.voice().is_some_and(|v| v.speaks(&l)) {
                        if let Some(v) = voices::ordered(list, Some(&l)).into_iter().find(|v| v.speaks(&l)) {
                            self.cfg.voice = v.id;
                        }
                    }
                }
            }
            Row::Voice => {
                if self.who.speak.is_err() {
                    return Step::Stay;
                }
                let Some(list) = self.voice_list() else { return Step::Stay };
                let lang = self.cfg.language.as_deref().map(voices::base_language);
                let list = voices::ordered(list, lang.as_deref());
                self.cfg.voice = match list.iter().position(|v| v.id == self.cfg.voice) {
                    Some(i) => list[cycle(i, list.len())].id.clone(),
                    // the default (not listed) or a voice gone: the first
                    None => list[if d < 0 { list.len() - 1 } else { 0 }].id.clone(),
                };
            }
            Row::Stt => {
                let o = &self.stt.options;
                if o.len() < 2 {
                    return Step::Stay;
                }
                let i = o.iter().position(|m| *m == self.stt.model).unwrap_or(0);
                self.stt.model = o[cycle(i, o.len())].clone();
                self.flash = Some((row, now));
                return Step::SaveStt;
            }
            Row::Dictation => {
                self.flash = Some((row, now));
                return Step::Dictation;
            }
            // who hears you opens on ⏎
            Row::Who => return Step::Stay,
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
                    ListenMode::HandsFree => format!("hands-free {} cut in by voice with headphones", d),
                    ListenMode::Hold => "hold space to talk".into(),
                },
                text,
            ),
            Row::Stt => {
                let env = if self.stt.env { format!(" {} BISE_VOICE_MODEL decides", d) } else { String::new() };
                (format!("{}{}", self.stt.line(d), env), text)
            }
            Row::Voice => match &self.who.speak {
                Ok(p) => {
                    let name = match self.voice() {
                        Some(v) => v.line(d),
                        None => voice_name(&c.voice),
                    };
                    (format!("{} {} {}", p, d, name), text)
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
            Row::Who => (format!("{} {} the words are kept, never the audio", self.who.hears(), d), theme::dim()),
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
        let mut v =
            vec![title("voice"), dim("ctrl+r twice starts voice mode with the agent in view. esc leaves.".into())];
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
            let room = room.saturating_sub(if *row == Row::Voice { 12 } else { 0 });
            // a voice too long for the row: without its gender, then its
            // language (designer: never a cut name at 80 columns)
            let value = match (row, self.voice(), &self.who.speak) {
                (Row::Voice, Some(voice), Ok(p)) if value.width() > room => {
                    let d = dot();
                    let lang = voice.langs().first().map(|l| voices::language_name(l));
                    let short = lang.map(|l| format!("{} {} {} {} {}", p, d, voice.label(), d, l));
                    match short.filter(|t| t.width() <= room) {
                        Some(t) => t,
                        None => format!("{} {} {}", p, d, voice.label()),
                    }
                }
                _ => value,
            };
            spans.push(s(cut(&value, room), color));
            if *row == Row::Voice && self.who.speak.is_ok() {
                let play = if theme::ascii_mode() { ">" } else { "▸" };
                spans.push(s(format!("  {} hear it", play), if selected { theme::accent() } else { theme::dim() }));
            }
            v.push(Line::from(spans));
        }
        v.push(Line::raw(""));
        // designer: never two colons on the line (`listen: auto: …`): a
        // hint with its own `x: ` goes without the row's name
        let hint = self.row().hint();
        let help = if hint.contains(": ") { format!("  {}", hint) } else { format!("  {}: {}", self.row().name(), hint) };
        v.push(dim(help));
        // the voices' list, while it loads or when it didn't
        if matches!(self.row(), Row::Voice | Row::Language) && self.who.speak.is_ok() {
            match &self.voices {
                voices::State::Loading => v.push(dim(format!("  loading {}'s voices…", self.who.speak.as_deref().unwrap_or("")))),
                voices::State::Failed(e) => v.push(Line::from(s(format!("  {}", e), theme::error()))),
                _ => {}
            }
        }
        if let Some(t) = &self.said {
            v.push(Line::raw(""));
            v.push(Line::from(s(t.clone(), theme::error())));
        }
        v.push(Line::raw(""));
        let enter = match self.row() {
            Row::Voice => "{enter} hear it · ",
            Row::Stt => "{enter} other providers · ",
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
        // designer: one line per company, not one per job
        match &who.speak {
            Ok(p) if *p == who.listen => v.push(text(format!("{} turns your voice into words, and the answers into a voice.", p))),
            Ok(p) => {
                v.push(text(format!("{} turns your voice into words.", who.listen)));
                v.push(text(format!("{} turns the answers into a voice.", p)));
            }
            Err(e) => {
                v.push(text(format!("{} turns your voice into words.", who.listen)));
                v.push(dim(format!("nobody says the answers: {}", e)));
            }
        }
        v.push(text(format!("a small {} model reads your words to say a quick \"on it\".", who.ack)));
        v.push(dim("then your words go to the agent's model, like a typed message.".into()));
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
/// a change saved at once, ▸ hear it through `player` (the TTS and the
/// speaker; played only on the user's ⏎, one sample at a time). Leaving
/// the screen cuts the sample playing.
pub fn show(
    app: &mut App,
    terminal: &mut crate::links::Tui,
    open: Open,
    pump: &mut dyn FnMut(&mut App),
    player: &mut Player,
) -> io::Result<Out> {
    let shown = show_until_closed(app, terminal, open, pump, player);
    player.stop();
    shown
}

fn show_until_closed(
    app: &mut App,
    terminal: &mut crate::links::Tui,
    open: Open,
    pump: &mut dyn FnMut(&mut App),
    player: &mut Player,
) -> io::Result<Out> {
    use crossterm::event::{poll, read, Event};
    let cfg = config::load();
    let who = Who::load(&cfg);
    // the list of voices, once per session, in the background
    if let Ok(job) = config::say_job(&cfg) {
        voices::load(job.api);
    }
    let mut screen = Screen::new(open, cfg, who, app.voice.enabled);
    screen.stt = Stt::load();
    if let Some(row) = std::mem::take(&mut *START.lock().unwrap_or_else(|e| e.into_inner())) {
        screen.at(row);
    }
    let _ = terminal.clear();
    loop {
        pump(app);
        screen.voices = voices::state();
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
        match apply(&mut screen, step, app, player) {
            Some(out) => return Ok(out),
            None => continue,
        }
    }
}

/// What a step does outside the screen; Some: it closes.
fn apply(screen: &mut Screen, step: Step, app: &mut App, player: &mut Player) -> Option<Out> {
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
                Ok(job) => player.play(job, sample(screen.sample_language().as_deref()).to_string()),
                Err(e) => screen.said = Some(e),
            }
            None
        }
        Step::SaveStt => {
            match config::save_stt_model(&screen.stt.model) {
                Ok(()) => screen.who = Who::load(&screen.cfg),
                Err(e) => screen.said = Some(format!("not saved: {}", e)),
            }
            None
        }
        // the voice picker (the onboarding's, over the screen) shows
        // first at the next frame, then this screen again
        Step::PickStt => {
            crate::onboarding::provider_request(crate::onboarding::Ask {
                open: crate::onboarding::Open::Pick(bise_catalog::roles::VOICE),
                from_settings: true,
                ..Default::default()
            });
            request_stt();
            Some(Out::Closed)
        }
        Step::Dictation => match crate::input::toggle_voice(app, crate::voice::resolve_job) {
            Some(ev) => {
                crate::feed::push_event(&mut app.events, &mut app.cache, ev);
                screen.dictation = app.voice.enabled;
                None
            }
            // no voice model works yet: its picker opens, then this
            // screen again
            None => {
                request_at(Open::Settings, Row::Dictation);
                Some(Out::Closed)
            }
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
            // opened from /models' voice row: back there, on that row
            if take_from_models() && out == Out::Closed {
                crate::onboarding::provider_request(crate::onboarding::Ask {
                    open: crate::onboarding::Open::RolesAt(bise_catalog::roles::VOICE),
                    ..Default::default()
                });
            }
            Some(out)
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
