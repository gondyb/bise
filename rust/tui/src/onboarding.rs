//! The first launch (BISE-60, book §15, mockup `tui-onboarding.html`).
//!
//! Six steps, `enter` to go on: the typed welcome and the `:*` pop, the
//! theme with two live previews, the model (the API keys the harness
//! reads), the folder and the honest line, how it works in three lines,
//! then the normal UI (step 6: the real first run; its one-time hints are
//! BISE-61).
//!
//! It runs once per user: the flag is the `onboarded` preference
//! (`bise_home`: a key of `~/.bise/prefs.json`, or the old
//! `~/.local/state/switchboard/onboarded` file). `esc` / `ctrl+c` skip it and mark
//! it seen too. `SB_ONBOARDING=off` never shows it, `on` always does (the
//! tmux tests set `off`). `/welcome` (BISE-41) calls [`run`] to replay it.
//!
//! The keys (book §15 step 3; providers.md §7.4): the providers come from
//! bise's catalog (`bise_catalog::Setup`: the built-in list merged with
//! config.toml; the model in use and its provider too). A key is found
//! where the harness finds it (`bise_catalog::auth::Keys`): the
//! environment, then auth.json, then the old `.env` files. A pasted key is
//! masked, never logged, and saved by `login`'s own code
//! (`auth_cli::login`: auth.json, 0600; a stored key is replaced only
//! after a confirm). The hub resolves the keys again at each REPL spawn,
//! so the agents it starts next use it. The model itself is not changed
//! here.

use crate::theme::{self, Mode, Palette};
use crate::App;
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// The env var: `off` never shows the onboarding, `on` always does.
pub(crate) const ENV: &str = "SB_ONBOARDING";

/// An environment lookup (the real one, or a map in the tests).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

// ---- the flag ----

/// bise's state for an environment (`bise_home`: `~/.bise`, `$BISE_HOME`,
/// or the old places).
pub(crate) fn home_of(env: Env) -> bise_home::Home {
    bise_home::Home::from_lookup(env)
}

/// Where the flag is kept: `onboarded` in `prefs.json`, or the old
/// `~/.local/state/switchboard/onboarded` file.
pub(crate) fn flag(env: Env) -> bise_home::Slot {
    home_of(env).pref(bise_home::Pref::Onboarded)
}

/// Show it at this launch: `SB_ONBOARDING` decides, else the flag.
pub(crate) fn due(env: Env) -> bool {
    match env(ENV).map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("off" | "0" | "no") => false,
        Some("on" | "1" | "yes") => true,
        _ => flag(env).get().is_none(),
    }
}

/// Seen: the flag goes on disk.
pub(crate) fn mark_seen(env: Env) -> io::Result<()> {
    flag(env).set(true.into())
}

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Play the onboarding at the next frame (`/welcome`, and the first
/// launch). The UI loop in `run.rs` takes the request.
pub(crate) fn run(_app: &mut App) {
    REQUESTED.store(true, Ordering::SeqCst);
}

/// The first launch of the Switchboard UI: request it when it is due.
pub(crate) fn request_if_due(app: &mut App) {
    if due(&real_env) {
        run(app);
    }
}

/// Take a pending request (the UI loop, once per frame).
pub(crate) fn take_request() -> bool {
    REQUESTED.swap(false, Ordering::SeqCst)
}

// ---- the providers and their keys ----

/// A provider that takes an API key (bise's catalog, `rust/catalog`:
/// the built-in list merged with config.toml; usable today, so not a
/// `needs = "BISE-149"` one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Provider {
    pub id: String,
    pub name: String,
    pub key_env: String,
}

/// The catalog and the model choice (`Setup`: `BISE_MODEL` >
/// `BEND_MODEL` > `model` in config.toml > the default).
pub(crate) fn setup_of(env: Env, home: &bise_home::Home) -> bise_catalog::Setup {
    let text = std::fs::read_to_string(home.config_file()).ok();
    bise_catalog::Setup::from_text(text.as_deref(), env)
}

/// Where `login` keeps the keys (auth.json) and where the old ones are.
pub(crate) fn auth_paths(home: &bise_home::Home) -> bise_catalog::auth_cli::Paths {
    bise_catalog::auth_cli::Paths {
        auth_file: home.auth_file(),
        config: home.config_file(),
        env_files: home.env_files(),
        home: Some(home.user_home().to_path_buf()),
    }
}

/// Every provider a key can be pasted for, in catalog order.
pub(crate) fn key_providers(setup: &bise_catalog::Setup) -> Vec<Provider> {
    setup
        .catalog
        .providers
        .iter()
        // the voice-only ones (BISE-130: elevenlabs, deepgram) run no agent
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && !p.stt_only)
        .map(|p| Provider { id: p.id.clone(), name: p.name.clone(), key_env: p.key_env.clone() })
        .collect()
}

/// The providers whose key is set, found where the harness finds it
/// (`bise_catalog::auth::Keys`): the environment, auth.json, the old
/// `.env` files.
pub(crate) fn find_keys(env: Env, home: &bise_home::Home, setup: &bise_catalog::Setup) -> Vec<Provider> {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let paths = auth_paths(home);
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env, store: &store, files: &files };
    key_providers(setup)
        .into_iter()
        .filter(|p| keys.find(&p.id, &p.key_env).is_some())
        .collect()
}

/// auth.json has a key for this provider already.
pub(crate) fn stored(home: &bise_home::Home, id: &str) -> bool {
    bise_catalog::auth::Store::read(&home.auth_file()).is_ok_and(|s| s.key(id).is_some())
}

/// A pasted key, cleaned: trimmed, quotes off; None when it can't be one
/// (empty, spaces or control characters inside).
pub(crate) fn clean_key(s: &str) -> Option<String> {
    let k = s.trim().trim_matches('"').trim_matches('\'');
    (!k.is_empty() && !k.chars().any(|c| c.is_whitespace() || c.is_control())).then(|| k.to_string())
}

// ---- the steps ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Welcome,
    Theme,
    Model,
    Folder,
    Lines,
}

impl Step {
    fn index(self) -> usize {
        self as usize
    }
    fn next(self) -> Option<Step> {
        match self {
            Step::Welcome => Some(Step::Theme),
            Step::Theme => Some(Step::Model),
            Step::Model => Some(Step::Folder),
            Step::Folder => Some(Step::Lines),
            Step::Lines => None,
        }
    }
}

/// One row of the model step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Opt {
    Use(Provider),
    Paste,
    /// sign in with the browser: not built, shown, never selected
    Browser,
}

/// Where the model step is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Sub {
    List,
    /// which key to paste
    Which(usize),
    Paste(Provider, String),
    /// the file has this key already: enter replaces it
    Confirm(Provider, String),
}

/// A note under the model options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    /// in auth.json (as shown, `~/…`)
    Saved(String),
    Failed(String),
    NotAKey,
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Out {
    Stay,
    /// the last step is over: the normal UI
    Done,
    /// esc / ctrl+c
    Skip,
}

/// The whole onboarding state; drawn by [`draw`] at a time `now` (ms).
pub(crate) struct Onb {
    pub step: Step,
    /// when the step started (ms): its animations count from there
    pub since: u64,
    pub detected: Option<Mode>,
    /// where the mode at start came from
    pub theme_from: ThemeFrom,
    /// the mode at start (a saved choice kept as is is not written again)
    pub start: Mode,
    pub pick: Mode,
    /// bise's state (keys, config, the saved theme)
    pub home: bise_home::Home,
    /// the catalog (built-in + config.toml) and the model choice
    pub setup: bise_catalog::Setup,
    /// the model in use ("provider/id") and its provider
    pub model: String,
    pub mine: String,
    /// every provider a key can be pasted for, and those with a key
    pub providers: Vec<Provider>,
    pub found: Vec<Provider>,
    pub sel: usize,
    pub sub: Sub,
    pub note: Option<Note>,
    /// the folder as shown (`~/…`) and whether it is in a git repo
    pub folder: String,
    pub git: bool,
    /// `o` pressed on the folder step
    pub other_folder: bool,
}

/// Where the mode at start came from (book §15 step 2 says which).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeFrom {
    /// the terminal's background (`detected`; None: no answer)
    Terminal,
    /// `BISE_THEME=light|dark`
    Env,
    /// the choice saved by step 2 or `/theme` (BISE-62)
    Saved,
}

/// `BISE_THEME` first, then the saved choice, like `theme_detect::init`.
pub(crate) fn theme_from(env: Env, home: &bise_home::Home) -> ThemeFrom {
    use crate::theme_detect::{load_in, Choice};
    match env(crate::theme_detect::ENV).and_then(|v| Choice::parse(&v)) {
        Some(Choice::Light | Choice::Dark) => ThemeFrom::Env,
        Some(Choice::Auto) => ThemeFrom::Terminal,
        None => match load_in(home) {
            Some(Choice::Light | Choice::Dark) => ThemeFrom::Saved,
            _ => ThemeFrom::Terminal,
        },
    }
}

/// `/Users/x/lab/app` → `~/lab/app` under that home.
pub(crate) fn tilde(path: &str, home: Option<&Path>) -> String {
    match home.map(|h| h.to_string_lossy().to_string()) {
        Some(h) if !h.is_empty() && (path == h || path.starts_with(&format!("{}/", h))) => {
            format!("~{}", &path[h.len()..])
        }
        _ => path.to_string(),
    }
}

/// The folder or one of its parents holds `.git`.
pub(crate) fn in_git(dir: &Path) -> bool {
    dir.ancestors().any(|d| d.join(".git").exists())
}

impl Onb {
    pub(crate) fn new(workspace: &str, env: Env) -> Onb {
        let home = home_of(env);
        let setup = setup_of(env, &home);
        let model = setup.model.clone();
        let mine = setup.catalog.resolve(&model).provider;
        let mut o = Onb {
            step: Step::Welcome,
            since: 0,
            detected: crate::theme_detect::detected(),
            theme_from: theme_from(env, &home),
            start: theme::mode(),
            pick: theme::mode(),
            providers: key_providers(&setup),
            setup,
            model,
            mine,
            found: Vec::new(),
            sel: 0,
            sub: Sub::List,
            note: None,
            folder: tilde(workspace, env("HOME").map(PathBuf::from).as_deref()),
            git: in_git(Path::new(workspace)),
            other_folder: false,
            home,
        };
        o.refresh_keys(env);
        o
    }

    fn refresh_keys(&mut self, env: Env) {
        let mut found = find_keys(env, &self.home, &self.setup);
        // the key of the model in use first
        found.sort_by_key(|p| p.id != self.mine);
        self.found = found;
    }

    /// The rows of the model step.
    pub(crate) fn opts(&self) -> Vec<Opt> {
        let mut v: Vec<Opt> = self.found.iter().map(|p| Opt::Use(p.clone())).collect();
        v.push(Opt::Paste);
        v.push(Opt::Browser);
        v
    }

    /// What step 2 saves, if anything. `BISE_THEME` is a session
    /// override: never saved, whatever the pick. A saved choice kept as is:
    /// nothing to write. Else auto when the pick is what the terminal gave
    /// (or dark, with no answer), else the pick.
    pub(crate) fn theme_choice(&self) -> Option<crate::theme_detect::Choice> {
        use crate::theme_detect::Choice;
        let explicit = |m: Mode| if m == Mode::Light { Choice::Light } else { Choice::Dark };
        match self.theme_from {
            ThemeFrom::Env => None,
            ThemeFrom::Saved if self.pick == self.start => None,
            ThemeFrom::Saved => Some(explicit(self.pick)),
            ThemeFrom::Terminal if self.pick == self.detected.unwrap_or(Mode::Dark) => Some(Choice::Auto),
            ThemeFrom::Terminal => Some(explicit(self.pick)),
        }
    }

    fn go(&mut self, step: Step, now: u64) {
        self.step = step;
        self.since = now;
    }

    /// Go on: the next step, or done after the last.
    fn advance(&mut self, now: u64) -> Out {
        match self.step.next() {
            Some(s) => {
                self.go(s, now);
                Out::Stay
            }
            None => Out::Done,
        }
    }

    pub(crate) fn on_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(k.code, KeyCode::Char('c')) {
            return Out::Skip;
        }
        if self.step == Step::Model && self.sub != Sub::List {
            return self.on_model_sub(k, env);
        }
        match (self.step, k.code) {
            (_, KeyCode::Esc) => Out::Skip,
            (Step::Theme, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down) => {
                self.pick = if self.pick == Mode::Dark { Mode::Light } else { Mode::Dark };
                theme::set_mode(self.pick);
                Out::Stay
            }
            (Step::Theme, KeyCode::Enter) => {
                // BISE-62: kept for the next launches; the terminal's own
                // mode stays "auto" (it follows the terminal)
                if let Some(c) = self.theme_choice() {
                    let _ = crate::theme_detect::save_in(&self.home, c);
                }
                self.advance(now)
            }
            (Step::Model, KeyCode::Up | KeyCode::Down) => {
                let n = self.opts().iter().filter(|o| **o != Opt::Browser).count();
                self.sel = if k.code == KeyCode::Down { (self.sel + 1) % n } else { (self.sel + n - 1) % n };
                Out::Stay
            }
            (Step::Model, KeyCode::Enter) => match self.opts().get(self.sel) {
                Some(Opt::Paste) => {
                    self.note = None;
                    self.sub = Sub::Which(self.providers.iter().position(|p| p.id == self.mine).unwrap_or(0));
                    Out::Stay
                }
                _ => self.advance(now),
            },
            (Step::Folder, KeyCode::Char('o')) => {
                self.other_folder = true;
                Out::Stay
            }
            (_, KeyCode::Enter) => self.advance(now),
            _ => Out::Stay,
        }
    }

    /// The paste flow: esc goes back to the options (it never skips).
    fn on_model_sub(&mut self, k: KeyEvent, env: Env) -> Out {
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        self.sub = match (sub, k.code) {
            (_, KeyCode::Esc) => Sub::List,
            (Sub::Which(i), KeyCode::Up | KeyCode::Down) => {
                let n = self.providers.len().max(1);
                Sub::Which(if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n })
            }
            (Sub::Which(i), KeyCode::Enter) => match self.providers.get(i) {
                Some(p) => Sub::Paste(p.clone(), String::new()),
                None => Sub::List,
            },
            (Sub::Paste(p, mut b), KeyCode::Backspace) => {
                b.pop();
                Sub::Paste(p, b)
            }
            (Sub::Paste(p, mut b), KeyCode::Char(c))
                if !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                b.push(c);
                Sub::Paste(p, b)
            }
            (Sub::Paste(p, b), KeyCode::Enter) => match clean_key(&b) {
                None if b.trim().is_empty() => Sub::Paste(p, b),
                None => {
                    self.note = Some(Note::NotAKey);
                    Sub::Paste(p, String::new())
                }
                Some(key) if stored(&self.home, &p.id) => Sub::Confirm(p, key),
                Some(key) => self.save(p, &key, env),
            },
            (Sub::Confirm(p, key), KeyCode::Enter) => self.save(p, &key, env),
            (s, _) => s,
        };
        Out::Stay
    }

    /// `login`'s own code (bise_catalog::auth_cli): auth.json, 0600.
    fn save(&mut self, p: Provider, key: &str, env: Env) -> Sub {
        let paths = auth_paths(&self.home);
        let r = match self.setup.catalog.provider(&p.id) {
            Some(cp) => bise_catalog::auth_cli::login(&paths, cp, key, env).map(|_| ()),
            None => Err(format!("unknown provider {}", p.id)),
        };
        self.note = Some(match r {
            Ok(()) => Note::Saved(bise_catalog::auth::tilde(&paths.auth_file, paths.home.as_deref())),
            Err(e) => Note::Failed(e),
        });
        self.refresh_keys(env);
        // the cursor on the key just saved
        self.sel = self.found.iter().position(|f| *f == p).unwrap_or(0);
        Sub::List
    }

    /// A bracketed paste: into the key field only.
    pub(crate) fn on_paste(&mut self, s: &str) {
        if let Sub::Paste(_, b) = &mut self.sub {
            b.push_str(s.trim());
        }
    }
}

// ---- drawing ----

/// The first chars of `text` typed from `start` ms at `per` ms a char.
pub(crate) fn typed(text: &str, start: u64, per: u64, t: u64) -> &str {
    if t < start {
        return "";
    }
    let n = ((t - start) / per + 1) as usize;
    match text.char_indices().nth(n) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

// the welcome timeline (the mockup's): typing, the kiss, the tagline, the hint
const HI: &str = "hi, i'm bise ";
const TAGLINE: &str = "ideas in. little kisses out. also pull requests.";
const PRESS: &str = "press enter ↵";
const HI_AT: u64 = 300;
const KISS_AT: u64 = HI_AT + 12 * 70 + 250;
/// the name gloss, faint, under the first line, just after the pop
const GLOSS_AT: u64 = KISS_AT + 900;
const TAG_AT: u64 = KISS_AT + 1900;
const PRESS_AT: u64 = TAG_AT + (TAGLINE.len() as u64 - 1) * 35 + 500;
/// when the welcome is fully written
#[cfg(test)]
pub(crate) const WELCOME_END: u64 = PRESS_AT + 12 * 30;
/// the three lines of step 5 and the last hint appear one by one
#[cfg(test)]
pub(crate) const LINES_END: u64 = 400 + 3 * 900;

fn s(t: impl Into<String>, c: Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c))
}

fn bold(t: impl Into<String>, c: Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c).add_modifier(Modifier::BOLD))
}

// ---- layout (book §15 'Layout', BISE-94) ----

/// The content column: 64 wide, centered; width − 8 when narrower, − 4
/// under 50 columns.
pub(crate) fn column(area: Rect) -> Rect {
    let w = if area.width < 50 { area.width.saturating_sub(4) } else { area.width.saturating_sub(8).min(64) };
    Rect { x: area.x + (area.width - w) / 2, width: w, ..area }
}

/// Blank rows between blocks: 2, or 1 when the terminal is under 22 rows.
fn gap_of(area: Rect) -> usize {
    if area.height < 22 {
        1
    } else {
        2
    }
}

fn blanks(v: &mut Vec<Line<'static>>, n: usize) {
    v.extend(std::iter::repeat_n(Line::raw(""), n));
}

/// A step's title: bold, text color.
fn title(t: impl Into<String>) -> Line<'static> {
    Line::from(bold(t, theme::text()))
}

/// A key line: dim, the keys (`{…}`) in text color (key lines are read:
/// never faint).
pub(crate) fn keyline(text: &str) -> Line<'static> {
    Line::from(
        text.split(['{', '}'])
            .enumerate()
            .filter(|(_, p)| !p.is_empty())
            .map(|(i, p)| s(p.to_string(), if i % 2 == 1 { theme::text() } else { theme::dim() }))
            .collect::<Vec<_>>(),
    )
}

/// The `:*` pop: a dot, then big (bold), then itself, bold (scale 0.4 →
/// 1.5 → 1 in the mockup; a terminal has one size).
fn kiss(t: u64) -> Span<'static> {
    match t.checked_sub(KISS_AT) {
        None => Span::raw(""),
        Some(d) if d < 200 => s(" ·", theme::accent()),
        Some(_) => bold(theme::glyph(theme::G_MAIN), theme::accent()),
    }
}

/// `bise /beez/ · french: a kiss on the cheek. also a north wind.` (the
/// `·` follows BISE_ASCII)
fn gloss() -> String {
    format!("bise /beez/ {} french: a kiss on the cheek. also a north wind.", theme::glyph("·"))
}

/// `press enter ↵` typed, dim, `enter` in text color.
fn press(t: u64) -> Line<'static> {
    let shown = typed(PRESS, PRESS_AT, 30, t).chars().count();
    let mut spans = Vec::new();
    let mut at = 0;
    for (part, key) in [("press ", false), ("enter", true), (" ↵", false)] {
        let n = part.chars().count().min(shown.saturating_sub(at));
        if n > 0 {
            let p: String = part.chars().take(n).collect();
            spans.push(s(p, if key { theme::text() } else { theme::dim() }));
        }
        at += part.chars().count();
    }
    Line::from(spans)
}

fn welcome(t: u64, gap: usize) -> Vec<Line<'static>> {
    let mut v = vec![
        Line::from(vec![bold(typed(HI, HI_AT, 70, t).to_string(), theme::text()), kiss(t)]),
        Line::from(s(if t >= GLOSS_AT { gloss() } else { String::new() }, theme::dim())),
    ];
    blanks(&mut v, gap);
    v.push(Line::from(s(typed(TAGLINE, TAG_AT, 35, t), theme::text())));
    blanks(&mut v, gap);
    v.push(press(t));
    v
}

fn name_of(m: Mode) -> &'static str {
    match m {
        Mode::Dark => "dark",
        Mode::Light => "light",
    }
}

/// The theme step's title and note (the previews and the key line are
/// drawn by `draw`).
fn theme_text(o: &Onb) -> Vec<Line<'static>> {
    let first = match (o.theme_from, o.detected) {
        // forced: say so, no detection involved
        (ThemeFrom::Env, _) => format!("{} is set to {}, so i picked it.", crate::theme_detect::ENV, name_of(o.pick)),
        (ThemeFrom::Saved, _) => format!("you picked {} last time, so i kept it.", name_of(o.pick)),
        (ThemeFrom::Terminal, Some(m)) => format!("your terminal looks {}, so i picked {}.", name_of(m), name_of(m)),
        (ThemeFrom::Terminal, None) => {
            format!("i couldn't read your terminal's background, so i picked {}.", name_of(o.pick))
        }
    };
    vec![title(first), Line::from(s("you can change it any time with /theme.", theme::dim()))]
}

// a preview shows its palette on that palette's ground (theme `bg`)

fn who(g: &str) -> String {
    format!("{:<3}", g)
}

/// The same four lines of a bise feed, in one palette.
fn preview_lines(p: &Palette, m: Mode) -> Vec<Line<'static>> {
    let ps = |t: &str, c: Color| Span::styled(t.to_string(), Style::default().fg(c));
    vec![
        Line::from(ps(name_of(m), p.dim)),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::glyph(theme::G_YOU)), p.dim), ps("fix the flaky login test", p.text)]),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::glyph(theme::G_MAIN)), p.accent), ps("on it: auth-fix takes it.", p.text)]),
        Line::raw(""),
        Line::from(vec![
            ps(" │ ", p.faint),
            ps(&format!("{} auth-fix → main  found it", theme::glyph(theme::G_MSG)), p.dim),
        ]),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::done_glyph()), p.accent), ps("auth-fix is done.", p.text)]),
    ]
}

const PREVIEW_H: u16 = 11;

fn draw_previews(f: &mut Frame, area: Rect, pick: Mode) {
    let bw = 44.min(area.width.saturating_sub(3) / 2);
    let x0 = area.x + area.width.saturating_sub(bw * 2 + 3) / 2;
    for (i, m) in [Mode::Dark, Mode::Light].into_iter().enumerate() {
        let p = theme::palette_of(m);
        let bg = p.bg;
        let border = if m == pick { theme::accent() } else { theme::faint() };
        let r = Rect { x: x0 + i as u16 * (bw + 3), y: area.y, width: bw, height: PREVIEW_H.min(area.height) };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(bg).fg(p.text))
            .padding(Padding::horizontal(1));
        f.render_widget(Paragraph::new(preview_lines(p, m)).block(block), r);
    }
}

/// An option: the selected one `›` accent + its name bold, the others
/// indented 2; its sub-line dim, indented 2 more, wrapped at `w`.
fn option(v: &mut Vec<Line<'static>>, selected: bool, name: Vec<Span<'static>>, sub: &str, w: u16) {
    let mut row = if selected { vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())] } else { vec![Span::raw("  ")] };
    row.extend(name.into_iter().map(|sp| if selected { sp.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { sp }));
    v.push(Line::from(row));
    if !sub.is_empty() {
        for r in words_in(sub, (w as usize).saturating_sub(4)) {
            v.push(Line::from(s(format!("    {}", r), theme::dim())));
        }
    }
}

/// The rows of the provider list (the paste flow's first question).
const WHICH_ROWS: usize = 9;

/// auth.json as shown (`~/…`).
fn auth_shown(o: &Onb) -> String {
    bise_catalog::auth::tilde(&o.home.auth_file(), Some(o.home.user_home()))
}

/// "anthropic, openai, mistral and 10 more."
fn paste_sub(ps: &[Provider]) -> String {
    let first: Vec<&str> = ps.iter().take(3).map(|p| p.id.as_str()).collect();
    match ps.len().saturating_sub(3) {
        0 => format!("{}.", first.join(", ")),
        more => format!("{} and {} more.", first.join(", "), more),
    }
}

/// `text` in lines of at most `w` columns, cut at spaces.
fn words_in(text: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![String::new()];
    for word in text.split(' ') {
        let cur = out.last_mut().expect("one line");
        if !cur.is_empty() && cur.width() + 1 + word.width() > w.max(1) {
            out.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    out
}

fn model_lines(o: &Onb, w: u16, gap: usize) -> Vec<Line<'static>> {
    match &o.sub {
        Sub::List => model_list(o, w, gap),
        Sub::Which(i) => {
            let mut v = vec![title("which key do you want to paste?")];
            blanks(&mut v, gap);
            // a window of WHICH_ROWS rows around the cursor
            let n = o.providers.len();
            let from = i.saturating_sub(WHICH_ROWS / 2).min(n.saturating_sub(WHICH_ROWS));
            if from > 0 {
                v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
            }
            for (k, p) in o.providers.iter().enumerate().skip(from).take(WHICH_ROWS) {
                option(&mut v, k == *i, vec![s(format!("{} · {}  ", k + 1, p.name), theme::text()), s(p.key_env.clone(), theme::dim())], "", w);
            }
            if from + WHICH_ROWS < n {
                v.push(Line::from(s(format!("  ↓ {} more", n - from - WHICH_ROWS), theme::dim())));
            }
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
            v
        }
        Sub::Paste(p, b) => {
            let dots: String = "•".repeat(b.chars().count().min(48));
            let mut v = vec![
                title(format!("paste your {}:", p.key_env)),
                Line::from(s(format!("it goes in {}, only you can read it.", auth_shown(o)), theme::dim())),
            ];
            blanks(&mut v, gap);
            v.push(Line::from(vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent()), s(dots, theme::text()), s("█", theme::text())]));
            if o.note == Some(Note::NotAKey) {
                v.push(Line::raw(""));
                v.push(Line::from(s(format!("{} that doesn't look like a key: no spaces inside.", theme::glyph(theme::G_FAILED)), theme::error())));
            }
            blanks(&mut v, gap);
            v.push(keyline("{enter} save · {esc} back"));
            v
        }
        Sub::Confirm(p, _) => {
            let mut v = vec![title(format!("{} has a key in {} already.", p.name, auth_shown(o)))];
            blanks(&mut v, gap);
            v.push(keyline("{enter} replaces it · {esc} keeps the old one"));
            v
        }
    }
}

fn model_list(o: &Onb, w: u16, gap: usize) -> Vec<Line<'static>> {
    let found = match o.found.len() {
        0 => "i found no key in your environment.".to_string(),
        1 => "i found a key in your environment.".to_string(),
        n => format!("i found {} keys in your environment.", n),
    };
    let mut v = vec![title("which model should do the work?"), Line::from(s(found, theme::dim()))];
    blanks(&mut v, gap);
    for (i, opt) in o.opts().into_iter().enumerate() {
        if i > 0 {
            v.push(Line::raw(""));
        }
        let n = i + 1;
        let (name, sub) = match opt {
            Opt::Use(p) => (
                vec![s(format!("{} · use {} ", n, p.key_env), theme::text()), s("found", theme::accent())],
                if p.id == o.mine {
                    format!("{}, already set up. nothing to paste.", p.id)
                } else {
                    format!(
                        "{}. your model is {}: set model in {}.",
                        p.id,
                        o.model,
                        bise_catalog::auth::tilde(&o.home.config_file(), Some(o.home.user_home()))
                    )
                },
            ),
            Opt::Paste => (
                vec![s(
                    format!("{} · {}", n, if o.found.is_empty() { "paste a key" } else { "paste another key" }),
                    theme::text(),
                )],
                paste_sub(&o.providers),
            ),
            Opt::Browser => (vec![s(format!("{} · sign in with the browser", n), theme::dim())], "not built yet.".to_string()),
        };
        option(&mut v, i == o.sel, name, &sub, w);
    }
    match &o.note {
        Some(Note::Saved(file)) => {
            v.push(Line::raw(""));
            v.push(Line::from(s(format!("✓ saved in {}. the agents i start from now on use it.", file), theme::dim())));
        }
        Some(Note::Failed(e)) => {
            v.push(Line::raw(""));
            v.push(Line::from(s(format!("{} couldn't save the key: {}", theme::glyph(theme::G_FAILED), e), theme::error())));
        }
        _ => {}
    }
    blanks(&mut v, gap);
    v.push(keyline("{↑↓} choose · {enter} ok"));
    v
}

fn folder_lines(o: &Onb, w: u16, gap: usize) -> Vec<Line<'static>> {
    let repo = if o.git { "· a git repo ✓" } else { "· not a git repo" };
    let first = vec![bold("i'll work in ", theme::text()), bold(o.folder.clone(), theme::text())];
    // the repo note never breaks: on the same row, else on its own
    let fits = "i'll work in ".width() + o.folder.width() + 1 + repo.width() <= w as usize;
    let mut v = if fits {
        let mut l = first;
        l.push(s(format!(" {}", repo), theme::dim()));
        vec![Line::from(l)]
    } else {
        vec![Line::from(first), Line::from(s(repo, theme::dim()))]
    };
    blanks(&mut v, gap);
    v.extend([
        Line::from(s(
            "all your agents share this folder and know about each other. no worktrees to merge.",
            theme::dim(),
        )),
        Line::raw(""),
        Line::from(vec![
            s("one honest thing: ", theme::text()),
            s("agents run commands here without asking you.", theme::accent()),
            s(
                if o.git {
                    " git is your safety net, so commit often."
                } else {
                    " git would be your safety net: git init here, and commit often."
                },
                theme::dim(),
            ),
        ]),
    ]);
    if o.other_folder {
        v.push(Line::raw(""));
        v.push(Line::from(s(
            "another folder? start me there: cd into it, then run switchboard.",
            theme::dim(),
        )));
    }
    blanks(&mut v, gap);
    v.push(keyline("{enter} ok · {o} another folder"));
    v
}

fn how_lines(t: u64, gap: usize) -> Vec<Line<'static>> {
    let (wave, wave_c) = theme::working_frame((t / 80) as u32);
    let rows: [Line<'static>; 3] = [
        Line::from(vec![
            s(who(theme::glyph(theme::G_YOU)), theme::dim()),
            s("you talk to me. i start agents for the work, in the background.", theme::text()),
        ]),
        Line::from(vec![
            s(who(wave), wave_c),
            s("they show up on the right. ⌥ + number to look inside, esc to come back.", theme::text()),
        ]),
        Line::from(vec![
            s(who(theme::glyph(theme::G_CARD)), theme::accent()),
            s("when someone needs you, you get a card. the rest can wait.", theme::text()),
        ]),
    ];
    let shown = |i: u64| t >= 400 + i * 900;
    let mut v = vec![title("how it works, in three lines:")];
    blanks(&mut v, gap);
    for (i, l) in rows.into_iter().enumerate() {
        if i > 0 {
            v.push(Line::raw(""));
        }
        v.push(if shown(i as u64) { l } else { Line::raw("") });
    }
    blanks(&mut v, gap);
    v.push(if shown(3) { keyline("{enter}, and say what's on your mind.") } else { Line::raw("") });
    v
}

/// The step dots: `○ ● ○ ○ ○ ○`, the current one in accent.
fn dots(step: Step) -> Line<'static> {
    let mut v = Vec::new();
    for i in 0..6 {
        if i > 0 {
            v.push(Span::raw(" "));
        }
        v.push(if i == step.index() { s("●", theme::accent()) } else { s("○", theme::faint()) });
    }
    Line::from(v)
}

/// Rows `lines` take at `width` once word-wrapped (greedy, like the
/// paragraph: a word that doesn't fit goes to the next row, a word longer
/// than a row is cut).
fn height_of(lines: &[Line], width: u16) -> u16 {
    let w = width.max(1) as usize;
    let rows = |l: &Line| -> usize {
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        let (mut rows, mut col) = (1usize, 0usize);
        for word in text.split(' ') {
            let ww = word.width();
            let need = if col == 0 { ww } else { col + 1 + ww };
            if need <= w {
                col = need;
            } else if ww <= w {
                rows += 1;
                col = ww;
            } else {
                // a long word: it starts on a new row and fills rows
                rows += usize::from(col > 0) + (ww - 1) / w;
                col = (ww - 1) % w + 1;
            }
        }
        rows
    };
    lines.iter().map(|l| rows(l) as u16).sum()
}

/// One frame of the onboarding at `now` ms (book §15 'Layout'): one
/// content column, the block at 2/5 of the free rows from the top, the
/// step dots 2 rows above the bottom.
pub(crate) fn draw(f: &mut Frame, o: &Onb, now: u64) {
    let area = f.area();
    let t = now.saturating_sub(o.since);
    let gap = gap_of(area);
    // the rows above the dots
    let body = Rect { height: area.height.saturating_sub(3), ..area };
    let col = column(body);
    let centered = matches!(o.step, Step::Welcome | Step::Theme);
    let (lines, extra) = match o.step {
        Step::Welcome => (welcome(t, gap), 0),
        // the previews (1 blank row above), then the key line after a gap
        Step::Theme => (theme_text(o), 1 + PREVIEW_H + gap as u16 + 1),
        Step::Model => (model_lines(o, col.width, gap), 0),
        Step::Folder => (folder_lines(o, col.width, gap), 0),
        Step::Lines => (how_lines(t, gap), 0),
    };
    let text_h = height_of(&lines, col.width).min(body.height);
    let h = text_h + extra;
    let y = body.y + body.height.saturating_sub(h) * 2 / 5;
    // every row down to the body's end: an estimate too short never cuts
    // the key line
    let r = Rect { y, height: if o.step == Step::Theme { text_h } else { body.bottom().saturating_sub(y) }, ..col };
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(if centered { para.alignment(Alignment::Center) } else { para }, r);
    if o.step == Step::Theme {
        let py = y + text_h + 1;
        let prev = Rect { y: py, height: body.bottom().saturating_sub(py), ..body };
        draw_previews(f, prev, o.pick);
        let hy = py + PREVIEW_H + gap as u16;
        if hy < body.bottom() {
            f.render_widget(
                Paragraph::new(keyline("{←→} switch · {enter} keep")).alignment(Alignment::Center),
                Rect { y: hy, height: 1, ..col },
            );
        }
    }
    if area.height >= 3 {
        let dy = area.bottom() - 2;
        f.render_widget(Paragraph::new(dots(o.step)).alignment(Alignment::Center), Rect { y: dy, height: 1, ..area });
    }
}

// ---- the loop ----

/// Play the onboarding over the whole screen until it ends or is
/// skipped; `pump` keeps the hub lines flowing meanwhile. Marks it seen.
pub(crate) fn show(
    app: &mut App,
    terminal: &mut crate::links::Tui,
    pump: &mut dyn FnMut(&mut App),
) -> io::Result<()> {
    let t0 = Instant::now();
    let mode_before = theme::mode();
    let mut o = Onb::new(&app.session_id, &real_env);
    let _ = terminal.clear();
    let r = (|| -> io::Result<()> {
        loop {
            pump(app);
            if app.should_quit {
                return Ok(());
            }
            let now = t0.elapsed().as_millis() as u64;
            // BISE-92: the switch repaints the terminal's background too
            crate::theme_detect::sync_terminal_bg();
            terminal.draw(|f| {
                draw(f, &o, now);
                theme::paint(f.buffer_mut()); // BISE-92: bise paints its ground
                theme::asciify(f.buffer_mut()); // BISE-84: BISE_ASCII=1
            })?;
            if !poll(Duration::from_millis(33))? {
                continue;
            }
            let now = t0.elapsed().as_millis() as u64;
            // the ctrl hints' flags: a modifier alone or a release is no key
            match crate::ctrlhint::for_handlers(read()?) {
                Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    if o.on_key(k, now, &real_env) != Out::Stay {
                        return Ok(());
                    }
                }
                Some(Event::Paste(p)) => o.on_paste(&p),
                _ => {}
            }
        }
    })();
    let _ = mark_seen(&real_env);
    // the feed's rows carry their colors: a new theme builds them again
    if theme::mode() != mode_before {
        app.cache.clear();
    }
    let _ = terminal.clear();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashMap;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-onb-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn env_of(m: HashMap<&'static str, String>) -> impl Fn(&str) -> Option<String> {
        move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty())
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    /// The screen's text, rows trimmed and joined: phrases across wraps.
    fn flat(sc: &str) -> String {
        sc.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
    }

    fn screen(o: &Onb, now: u64, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, o, now)).unwrap();
        let b = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The old-layout home of a test HOME.
    fn hm(home: &Path) -> bise_home::Home {
        let h = home.to_string_lossy().to_string();
        bise_home::Home::from_lookup(&move |k: &str| (k == "HOME").then(|| h.clone()))
    }

    fn onb(home: &Path, ws: &str) -> Onb {
        let e = env_of(HashMap::from([("HOME", home.to_string_lossy().to_string())]));
        Onb::new(ws, &e)
    }

    #[test]
    fn the_flag_follows_the_state_root_and_the_env_var() {
        let d = tmp("flag");
        let ds = d.to_string_lossy().to_string();
        // the old layout: the flag file an older version reads
        let e = env_of(HashMap::from([("HOME", ds.clone())]));
        assert_eq!(flag(&e).file, d.join(".local/state/switchboard/onboarded"));
        assert!(due(&e));
        mark_seen(&e).unwrap();
        assert!(!due(&e));
        assert!(d.join(".local/state/switchboard/onboarded").exists());
        let on = env_of(HashMap::from([("HOME", ds.clone()), (ENV, "on".into())]));
        assert!(due(&on));
        let home = env_of(HashMap::from([("HOME", "/h".into()), (ENV, "off".into())]));
        assert!(!due(&home));
        // BISE_HOME: a key of prefs.json
        let b = env_of(HashMap::from([("HOME", ds.clone()), ("BISE_HOME", format!("{ds}/b"))]));
        assert!(due(&b));
        mark_seen(&b).unwrap();
        assert!(!due(&b));
        assert!(d.join("b/prefs.json").exists());
    }

    fn ids(ps: &[Provider]) -> Vec<&str> {
        ps.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn keys_come_from_the_env_then_auth_json_then_the_old_files() {
        let h = tmp("keys");
        let home = h.to_string_lossy().to_string();
        let none = env_of(HashMap::from([("HOME", home.clone())]));
        let st = setup_of(&none, &hm(&h));
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), Vec::<&str>::new());
        std::fs::create_dir_all(h.join(".vibe")).unwrap();
        std::fs::write(h.join(".vibe/.env"), "# x\nexport MISTRAL_API_KEY=\"abc\"\n").unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["mistral"]);
        let e = env_of(HashMap::from([("HOME", home.clone()), ("ANTHROPIC_FOUNDRY_API_KEY", "k".to_string())]));
        assert_eq!(ids(&find_keys(&e, &hm(&h), &st)), vec!["foundry", "mistral"]);
        // auth.json (what `login` writes)
        let mut store = bise_catalog::auth::Store::default();
        store.set("openai", "o");
        store.write(&hm(&h).auth_file()).unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["openai", "mistral"]);
        // an empty value is no key
        std::fs::write(h.join(".vibe/.env"), "MISTRAL_API_KEY=\n").unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["openai"]);
        // every provider that takes a key and is usable, none that is not
        let all = key_providers(&st);
        assert!(ids(&all).contains(&"anthropic") && ids(&all).contains(&"groq"), "{:?}", ids(&all));
        assert!(!ids(&all).contains(&"ollama") && !ids(&all).contains(&"bedrock"), "{:?}", ids(&all));
    }

    #[test]
    fn the_model_and_its_provider_come_from_the_catalog() {
        let h = tmp("model");
        let home = h.to_string_lossy().to_string();
        let none = env_of(HashMap::from([("HOME", home.clone())]));
        let o = Onb::new("/w", &none);
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("foundry/claude-opus-5-5", "foundry"));
        std::fs::create_dir_all(h.join(".bend-harness")).unwrap();
        std::fs::write(h.join(".bend-harness/config.toml"), "# c\nmodel = \"zai-glm-5-3\" # glm\n").unwrap();
        let o = Onb::new("/w", &none);
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("mistral/zai-glm-5-3", "mistral"));
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BEND_MODEL", "anthropic/claude-haiku-4-5".to_string())]));
        let o = Onb::new("/w", &e);
        assert_eq!(o.mine, "anthropic");
        assert_eq!(clean_key("  'sk-1'\n"), Some("sk-1".into()));
        assert_eq!(clean_key("a b"), None);
        assert_eq!(clean_key(" "), None);
    }

    #[test]
    fn typing_follows_the_clock() {
        assert_eq!(typed("abc", 100, 10, 50), "");
        assert_eq!(typed("abc", 100, 10, 100), "a");
        assert_eq!(typed("abc", 100, 10, 115), "ab");
        assert_eq!(typed("abc", 100, 10, 999), "abc");
        assert_eq!(typed("↵x", 0, 10, 0), "↵");
    }

    #[test]
    fn step_1_welcome_types_then_pops() {
        let h = tmp("s1");
        let o = onb(&h, "/w");
        let sc = screen(&o, 1000, 100, 30);
        assert!(sc.contains("hi, i'm b") && !sc.contains(":*"), "{}", sc);
        // the gloss comes after the pop, before the tagline, right under the name
        let gloss = "bise /beez/ · french: a kiss on the cheek. also a north wind.";
        let sc = screen(&o, KISS_AT + 700, 100, 30);
        assert!(sc.contains("hi, i'm bise :*") && !sc.contains(gloss), "{}", sc);
        let sc = screen(&o, GLOSS_AT, 100, 30);
        assert!(sc.contains(gloss) && !sc.contains("ideas in."), "{}", sc);
        let rows: Vec<&str> = sc.lines().collect();
        let hi = rows.iter().position(|r| r.contains("hi, i'm bise :*")).unwrap();
        assert!(rows[hi + 1].contains(gloss), "{}", sc);
        assert_eq!(welcome(GLOSS_AT, 2)[1].spans[0].style.fg, Some(theme::dim()), "the gloss is read: dim");
        let sc = screen(&o, WELCOME_END, 100, 30);
        for s in ["hi, i'm bise :*", "ideas in. little kisses out. also pull requests.", "press enter ↵", "● ○ ○ ○ ○ ○"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
    }

    #[test]
    fn step_2_theme_switches_live() {
        let h = tmp("s2");
        let mut o = onb(&h, "/w");
        o.detected = Some(Mode::Dark);
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Enter), 10, &none), Out::Stay);
        assert_eq!(o.step, Step::Theme);
        let sc = screen(&o, 20, 100, 30);
        for s in [
            "your terminal looks dark, so i picked dark.",
            "you can change it any time with /theme.",
            "fix the flaky login test",
            "on it: auth-fix takes it.",
            "auth-fix is done.",
            "←→ switch · enter keep",
            "○ ● ○ ○ ○ ○",
        ] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        o.on_key(key(KeyCode::Right), 30, &none);
        assert_eq!(theme::mode(), Mode::Light);
        o.on_key(key(KeyCode::Left), 40, &none);
        assert_eq!(theme::mode(), Mode::Dark);
        // BISE-62: enter saves the pick (auto when it is the terminal's)
        use crate::theme_detect::{load_in, Choice};
        assert_eq!(o.theme_choice(), Some(Choice::Auto));
        o.on_key(key(KeyCode::Right), 50, &none);
        assert_eq!(o.theme_choice(), Some(Choice::Light));
        o.on_key(key(KeyCode::Enter), 60, &none);
        assert_eq!((o.step, load_in(&hm(&h))), (Step::Model, Some(Choice::Light)));
    }

    #[test]
    fn step_3_model_lists_found_keys_and_saves_a_pasted_one() {
        let h = tmp("s3");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("ANTHROPIC_FOUNDRY_API_KEY", "k".to_string()),
        ]));
        let mut o = Onb::new("/w", &e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        for s in [
            "which model should do the work?",
            "i found a key in your environment.",
            "1 · use ANTHROPIC_FOUNDRY_API_KEY found",
            "foundry, already set up. nothing to paste.",
            "2 · paste another key",
            "anthropic, foundry, openai and ",
            "3 · sign in with the browser",
            "not built yet.",
            "↑↓ choose · enter ok",
        ] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // ↑↓ skip the browser row
        o.on_key(key(KeyCode::Down), 1, &e);
        assert_eq!(o.sel, 1);
        o.on_key(key(KeyCode::Down), 1, &e);
        assert_eq!(o.sel, 0);
        // paste: the catalog's providers, the model's one under the cursor
        o.on_key(key(KeyCode::Up), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        let foundry = o.providers.iter().position(|p| p.id == "foundry").unwrap();
        let mistral = o.providers.iter().position(|p| p.id == "mistral").unwrap();
        assert_eq!(o.sub, Sub::Which(foundry));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("which key do you want to paste?") && sc.contains("OPENAI_API_KEY"), "{}", sc);
        assert!(sc.contains("↓ ") && sc.contains(" more"), "a window over the list: {}", sc);
        for _ in foundry..mistral {
            o.on_key(key(KeyCode::Down), 1, &e);
        }
        assert_eq!(o.sub, Sub::Which(mistral));
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_paste("secret-xyz\n");
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("paste your MISTRAL_API_KEY:") && sc.contains("••••••••••"), "{}", sc);
        assert!(sc.contains("it goes in ~/.bend-harness/auth.json"), "{}", sc);
        assert!(!sc.contains("secret"), "{}", sc);
        o.on_key(key(KeyCode::Esc), 1, &e);
        assert_eq!((o.sub.clone(), o.step), (Sub::List, Step::Model));
        o.on_key(key(KeyCode::Enter), 1, &e);
        for _ in foundry..mistral {
            o.on_key(key(KeyCode::Down), 1, &e);
        }
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_paste("secret-xyz");
        o.on_key(key(KeyCode::Enter), 1, &e);
        // login's store: auth.json 0600, no .env written
        use std::os::unix::fs::PermissionsExt;
        let f = hm(&h).auth_file();
        let store = bise_catalog::auth::Store::read(&f).unwrap();
        assert_eq!(store.key("mistral"), Some("secret-xyz"));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!hm(&h).key_file().exists());
        assert_eq!(ids(&o.found), vec!["foundry", "mistral"]);
        assert_eq!(o.opts()[o.sel], Opt::Use(o.found[1].clone()));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("i found 2 keys") && sc.contains("✓ saved in ~/.bend-harness/auth.json."), "{}", sc);
        assert!(sc.contains("mistral. your model is foundry/claude-opus-5-5"), "{}", sc);
        // a second paste asks before replacing
        o.sel = 2;
        o.on_key(key(KeyCode::Enter), 1, &e);
        for _ in foundry..mistral {
            o.on_key(key(KeyCode::Down), 1, &e);
        }
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_paste("new");
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Confirm(p, _) if p.id == "mistral"));
        assert!(screen(&o, 10, 110, 30).contains("Mistral has a key in ~/.bend-harness/auth.json already."));
        o.on_key(key(KeyCode::Esc), 1, &e);
        assert_eq!(bise_catalog::auth::Store::read(&f).unwrap().key("mistral"), Some("secret-xyz"));
        // enter on a found key goes on
        o.sel = 0;
        o.on_key(key(KeyCode::Enter), 5, &e);
        assert_eq!(o.step, Step::Folder);
    }

    #[test]
    fn step_2_says_why_when_the_theme_is_forced() {
        use crate::theme_detect::{save_in, Choice};
        let h = tmp("s2f");
        let home = h.to_string_lossy().to_string();
        // BISE_THEME: no detection, it says so
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BISE_THEME", "light".to_string())]));
        theme::set_mode(Mode::Light);
        let mut o = Onb::new("/w", &e);
        assert_eq!(o.theme_from, ThemeFrom::Env);
        o.go(Step::Theme, 0);
        let sc = screen(&o, 10, 100, 30);
        assert!(sc.contains("BISE_THEME is set to light, so i picked it."), "{}", sc);
        assert!(!sc.contains("couldn't read"), "{}", sc);
        // a saved choice: it was picked before
        save_in(&hm(&h), Choice::Light).unwrap();
        let e = env_of(HashMap::from([("HOME", home.clone())]));
        let mut o = Onb::new("/w", &e);
        assert_eq!(o.theme_from, ThemeFrom::Saved);
        o.go(Step::Theme, 0);
        assert!(screen(&o, 10, 100, 30).contains("you picked light last time, so i kept it."));
        // a saved choice kept as is: nothing written; changed: the new pick
        assert_eq!(o.theme_choice(), None);
        o.pick = Mode::Dark;
        assert_eq!(o.theme_choice(), Some(Choice::Dark));
        // BISE_THEME=auto: the terminal decides
        let e = env_of(HashMap::from([("HOME", home), ("BISE_THEME", "auto".to_string())]));
        assert_eq!(Onb::new("/w", &e).theme_from, ThemeFrom::Terminal);
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn bise_theme_is_never_saved() {
        use crate::theme_detect::{load_in, save_in, settings, Choice};
        let h = tmp("s2env");
        let home = h.to_string_lossy().to_string();
        let e = env_of(HashMap::from([("HOME", home), ("BISE_THEME", "light".to_string())]));
        theme::set_mode(Mode::Light);
        // kept, or switched away and back, or switched: enter writes nothing
        for toggles in [0, 2, 1] {
            let mut o = Onb::new("/w", &e);
            o.go(Step::Theme, 0);
            for _ in 0..toggles {
                o.on_key(key(KeyCode::Right), 1, &e);
            }
            assert_eq!(o.theme_choice(), None);
            o.on_key(key(KeyCode::Enter), 2, &e);
            assert_eq!(o.step, Step::Model);
            assert!(settings(&hm(&h)).get().is_none(), "BISE_THEME was saved ({} toggles)", toggles);
        }
        // a real saved choice stays what it was
        save_in(&hm(&h), Choice::Dark).unwrap();
        let mut o = Onb::new("/w", &e);
        o.go(Step::Theme, 0);
        o.on_key(key(KeyCode::Enter), 2, &e);
        assert_eq!(load_in(&hm(&h)), Some(Choice::Dark));
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn wrapped_details_keep_their_indent_and_the_repo_note_never_breaks() {
        let h = tmp("wrap");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
        ]));
        let mut o = Onb::new("/w", &e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 70, 30);
        let rows: Vec<&str> = sc.lines().collect();
        let i = rows.iter().position(|r| r.contains("mistral. your model is")).expect("the detail");
        // the wrapped row starts where the detail starts (same column)
        let col = |r: &str, pat: &str| r.chars().collect::<String>().find(pat).map(|b| r[..b].chars().count());
        let start = col(rows[i], "mistral").unwrap();
        let next: Vec<char> = rows[i + 1].chars().collect();
        assert!(next[start] != ' ' && next[start - 4..start].iter().all(|c| *c == ' '), "{}", sc);
        assert!(sc.contains("config.toml."), "{}", sc);
        // a long path: the repo note goes on its own row, whole
        let ws = h.join("a-rather-long-folder-name/and-another-one-that-goes-on/app");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        let mut o = onb(&h, &ws.to_string_lossy());
        o.go(Step::Folder, 0);
        let sc = screen(&o, 10, 80, 30);
        assert!(sc.lines().any(|r| r.trim() == "· a git repo ✓"), "{}", sc);
        // a short one: on the title row
        let ws = h.join("lab/app");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        let mut o = onb(&h, &ws.to_string_lossy());
        o.go(Step::Folder, 0);
        assert!(screen(&o, 10, 200, 30).contains("i'll work in ~/lab/app · a git repo ✓"));
    }

    #[test]
    fn the_key_line_is_never_cut_by_a_long_path() {
        let h = tmp("cut");
        let ws = h.join("a-rather-long-folder-name-that-goes-on-and-on/and-another-one-that-goes-on/app");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        let mut o = onb(&std::path::PathBuf::from("/nowhere"), &ws.to_string_lossy());
        o.go(Step::Folder, 0);
        for (w, hh) in [(120, 34), (80, 24), (60, 21)] {
            let sc = screen(&o, 10, w, hh);
            assert!(sc.contains("enter ok · o another folder"), "{w}x{hh}\n{sc}");
        }
    }

    #[test]
    fn step_3_without_a_key() {
        let h = tmp("s3b");
        let mut o = onb(&h, "/w");
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("i found no key in your environment.") && sc.contains("1 · paste a key"), "{}", sc);
    }

    #[test]
    fn step_4_folder_and_the_honest_line() {
        let h = tmp("s4");
        let ws = h.join("lab/app");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        let mut o = onb(&h, &ws.to_string_lossy());
        o.go(Step::Folder, 0);
        let none = env_of(HashMap::new());
        let sc = screen(&o, 10, 150, 30);
        for s in [
            "i'll work in ~/lab/app · a git repo ✓",
            "all your agents share this folder and know about each other. no worktrees to merge.",
            "one honest thing: agents run commands here without asking you. git is your safety net, so commit often.",
            "enter ok · o another folder",
            "○ ○ ○ ● ○ ○",
        ] {
            assert!(flat(&sc).contains(s), "{}\n{}", s, sc);
        }
        o.on_key(key(KeyCode::Char('o')), 1, &none);
        assert!(screen(&o, 10, 120, 30).contains("another folder? start me there"));
    }

    #[test]
    fn step_5_three_lines_one_by_one_then_done() {
        let h = tmp("s5");
        let mut o = onb(&h, "/w");
        o.go(Step::Lines, 100);
        let none = env_of(HashMap::new());
        let sc = screen(&o, 100 + 500, 110, 30);
        assert!(sc.contains("you talk to me.") && !sc.contains("they show up"), "{}", sc);
        let sc = screen(&o, 100 + LINES_END, 110, 30);
        for s in [
            "how it works, in three lines:",
            "›  you talk to me. i start agents for the work, in the background.",
            "they show up on the right. ⌥ + number to look inside, esc to come back.",
            "?  when someone needs you, you get a card. the rest can wait.",
            "enter, and say what's on your mind.",
            "○ ○ ○ ○ ● ○",
        ] {
            assert!(flat(&sc).contains(s), "{}\n{}", s, sc);
        }
        assert_eq!(o.on_key(key(KeyCode::Enter), 1, &none), Out::Done);
    }

    #[test]
    fn esc_and_ctrl_c_skip() {
        let h = tmp("skip");
        let mut o = onb(&h, "/w");
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Esc), 1, &none), Out::Skip);
        o.go(Step::Theme, 0);
        assert_eq!(o.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), 1, &none), Out::Skip);
    }

    #[test]
    fn narrow_screens_do_not_panic() {
        let h = tmp("narrow");
        let mut o = onb(&h, "/w");
        for step in [Step::Welcome, Step::Theme, Step::Model, Step::Folder, Step::Lines] {
            o.go(step, 0);
            for (w, hh) in [(20, 5), (1, 1), (60, 12), (200, 60)] {
                screen(&o, 99_999, w, hh);
            }
        }
    }
}
