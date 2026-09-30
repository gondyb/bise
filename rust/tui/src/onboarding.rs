//! The first launch (BISE-60, onboarding v3, book §15, screens
//! `onboarding v3 · …`).
//!
//! Before the thread, three or four short steps: the typed welcome and the
//! `:*` pop (any key goes on; a key while it types shows it all), the
//! theme with two live previews (`←→`, `enter`), a key only when none is
//! found (the API keys the harness reads), how it works in three lines
//! (any key). No folder step: bise works where it was started (the header
//! says where). Then the thread, where the quiet setup card waits
//! (`setup.rs`).
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

static KEYS_ONLY: AtomicBool = AtomicBool::new(false);

/// The model in use can't run and nothing turned the onboarding off
/// (BISE-266): the key step shows at launch, even after the first run.
pub(crate) fn keys_due(env: Env) -> bool {
    if matches!(env(ENV).map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("off" | "0" | "no")) {
        return false;
    }
    let home = home_of(env);
    let setup = setup_of(env, &home);
    model_blocked(&setup, &find_keys(env, &home, &setup))
}

/// The launch of the Switchboard UI: the onboarding when it is due, else
/// only its key step when the model can't run.
pub(crate) fn request_if_due(app: &mut App) {
    if due(&real_env) {
        run(app);
    } else if keys_due(&real_env) {
        KEYS_ONLY.store(true, Ordering::SeqCst);
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
    /// the model a new user starts with (the catalog's `model`), its id
    /// without the provider; "" = none
    pub model: String,
    /// the catalog's words for the key step: a hint, its keys page, its
    /// sign-up page when not the keys page
    pub hint: String,
    pub keys_url: String,
    pub signup_url: String,
}

impl Provider {
    fn of(p: &bise_catalog::Provider) -> Provider {
        Provider {
            id: p.id.clone(),
            name: p.name.clone(),
            key_env: p.key_env.clone(),
            model: p.model.clone(),
            hint: p.hint.clone(),
            keys_url: p.keys_url.clone(),
            signup_url: p.signup_url.clone(),
        }
    }
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
        // hidden: a private proxy (BISE-266), never offered
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && !p.stt_only && !p.hidden)
        .map(Provider::of)
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
    setup
        .catalog
        .providers
        .iter()
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && !p.stt_only)
        .filter(|p| keys.find(&p.id, &p.key_env).is_some())
        .map(Provider::of)
        .collect()
}

/// The model in use can't run (BISE-266): its provider is unknown, not
/// usable yet, or has no key. The first run then asks for one, whatever
/// other keys are set: a key of another provider does not make the
/// first message work.
pub(crate) fn model_blocked(setup: &bise_catalog::Setup, found: &[Provider]) -> bool {
    let r = setup.catalog.resolve(&setup.model);
    r.known == bise_catalog::Known::NoProvider
        || !r.needs.is_empty()
        || (!r.key_env.is_empty() && !found.iter().any(|p| p.id == r.provider))
}

/// `provider/model`: the provider's pick (its catalog `model`), else the
/// model in use when it is of that provider; None when neither.
pub(crate) fn pick_of(p: &Provider, current: &str) -> Option<String> {
    if !p.model.is_empty() {
        return Some(format!("{}/{}", p.id, p.model));
    }
    bise_catalog::split_name(current).filter(|(pid, _)| *pid == p.id).map(|_| current.to_string())
}

/// Write `model` as config.toml's `model` (the rest of the file kept).
pub(crate) fn save_model(home: &bise_home::Home, model: &str) -> io::Result<()> {
    let file = home.config_file();
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, bise_catalog::with_model(&text, model))
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
    /// a key: only when none was found at the start
    Model,
    Lines,
}

/// One row of the model step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Opt {
    Use(Provider),
    Paste,
}

/// Where the model step is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Sub {
    List,
    /// which provider (BISE-266: its name and hint)
    Which(usize),
    /// which of its models (the catalog's pick first)
    Model(Provider, usize),
    /// the key for that model: its keys page, the field
    Paste(Provider, String, String),
    /// the file has this key already: enter replaces it
    Confirm(Provider, String, String),
    /// the live check of (provider, model); the pasted key to save when it
    /// passes (None: a key found where the harness finds it)
    Checking(Provider, String, Option<String>),
    Failed(Provider, String, crate::keycheck::Fail),
    /// it answered: the model is saved; the optional extras
    Works(Provider, String),
}

/// How a key is checked (the real call; the tests put their own).
pub(crate) type Checker = fn(&crate::keycheck::Call, Option<String>) -> Result<(), crate::keycheck::Fail>;

fn real_check(c: &crate::keycheck::Call, url: Option<String>) -> Result<(), crate::keycheck::Fail> {
    crate::keycheck::check(c, &move |k: &str| if k == "BEND_PROVIDER_URL" { url.clone() } else { None })
}

/// A note under the model options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
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
    /// a key other than enter on the welcome: it shows all of it at once
    pub rushed: bool,
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
    /// no key was found at the start: the key step shows
    pub ask_key: bool,
    /// only the key step (a launch whose model can't run, after the
    /// first run: BISE-266)
    pub keys_only: bool,
    /// the running check's answer
    pub pending: Option<std::sync::mpsc::Receiver<Result<(), crate::keycheck::Fail>>>,
    pub checker: Checker,
    /// the key just saved replaces another key the environment holds
    /// under this name (BISE-269: auth.json wins): said once, dim
    pub shadows: Option<String>,
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

impl Onb {
    pub(crate) fn new(env: Env) -> Onb {
        let home = home_of(env);
        let setup = setup_of(env, &home);
        let model = setup.model.clone();
        let mine = setup.catalog.resolve(&model).provider;
        let mut o = Onb {
            step: Step::Welcome,
            since: 0,
            rushed: false,
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
            ask_key: false,
            keys_only: false,
            pending: None,
            checker: real_check,
            shadows: None,
            home,
        };
        o.refresh_keys(env);
        o.ask_key = model_blocked(&o.setup, &o.found);
        o
    }

    /// The steps shown, in order (book §15): the key only when none was
    /// found.
    pub(crate) fn steps(&self) -> Vec<Step> {
        if self.keys_only {
            return vec![Step::Model];
        }
        let mut v = vec![Step::Welcome, Step::Theme];
        if self.ask_key {
            v.push(Step::Model);
        }
        v.push(Step::Lines);
        v
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
        self.rushed = false;
    }

    /// Go on: the next step, or done after the last.
    fn advance(&mut self, now: u64) -> Out {
        // the steps in their order, the ones not shown skipped
        let all = [Step::Welcome, Step::Theme, Step::Model, Step::Lines];
        let shown = self.steps();
        let next = all.iter().skip_while(|s| **s != self.step).skip(1).find(|s| shown.contains(s)).copied();
        match next {
            Some(s) => {
                self.go(s, now);
                Out::Stay
            }
            None => Out::Done,
        }
    }

    /// The welcome is fully written at `now`.
    fn welcome_done(&self, now: u64) -> bool {
        self.rushed || now.saturating_sub(self.since) >= WELCOME_END
    }

    pub(crate) fn on_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(k.code, KeyCode::Char('c')) {
            return Out::Skip;
        }
        if self.step == Step::Model && self.sub != Sub::List {
            return self.on_model_sub(k, now, env);
        }
        match (self.step, k.code) {
            // esc on the theme: the thread with the defaults (the theme
            // the launch had: the terminal's, or the saved one)
            (Step::Theme, KeyCode::Esc) => {
                self.pick = self.start;
                theme::set_mode(self.start);
                Out::Skip
            }
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
                let n = self.opts().len();
                self.sel = if k.code == KeyCode::Down { (self.sel + 1) % n } else { (self.sel + n - 1) % n };
                Out::Stay
            }
            (Step::Model, KeyCode::Enter) => match self.opts().get(self.sel) {
                Some(Opt::Paste) => {
                    self.note = None;
                    self.sub = Sub::Which(self.providers.iter().position(|p| p.id == self.mine).unwrap_or(0));
                    Out::Stay
                }
                // BISE-266: a found key is checked too, with the model
                // picked for it; the model in use, when it runs, goes on
                Some(Opt::Use(p)) if p.id == self.mine && !model_blocked(&self.setup, &self.found) => self.advance(now),
                Some(Opt::Use(p)) => {
                    self.note = None;
                    self.sub = Sub::Model(p.clone(), 0);
                    Out::Stay
                }
                None => self.advance(now),
            },
            // any key: all of it at once while it types, then on
            (Step::Welcome, _) if !self.welcome_done(now) => {
                self.rushed = true;
                Out::Stay
            }
            (Step::Welcome | Step::Lines, _) => self.advance(now),
            _ => Out::Stay,
        }
    }

    /// The key flow (BISE-266): provider → model → key → live check →
    /// works. esc goes back to the options (it never skips).
    fn on_model_sub(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        let updown = |i: usize, n: usize| {
            let n = n.max(1);
            if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n }
        };
        self.sub = match (sub, k.code) {
            // a running check: esc drops it (its answer is ignored)
            (Sub::Checking(..), KeyCode::Esc) => {
                self.pending = None;
                Sub::List
            }
            (s @ Sub::Checking(..), _) => s,
            (Sub::Works(..), KeyCode::Enter | KeyCode::Esc) => {
                self.sub = Sub::List;
                return self.advance(now);
            }
            (s @ Sub::Works(..), _) => s,
            (_, KeyCode::Esc) => Sub::List,
            (Sub::Which(i), KeyCode::Up | KeyCode::Down) => Sub::Which(updown(i, self.providers.len())),
            (Sub::Which(i), KeyCode::Enter) => match self.providers.get(i) {
                Some(p) => Sub::Model(p.clone(), 0),
                None => Sub::List,
            },
            (Sub::Model(p, i), KeyCode::Up | KeyCode::Down) => {
                let n = self.models_of(&p).len();
                Sub::Model(p, updown(i, n))
            }
            (Sub::Model(p, i), KeyCode::Enter) => match self.models_of(&p).get(i).cloned() {
                // a key found for it: straight to the check
                Some(m) if self.found.iter().any(|f| f.id == p.id) => self.start_check(p, m, None, env),
                Some(m) => Sub::Paste(p, m, String::new()),
                None => Sub::Model(p, i),
            },
            (Sub::Paste(p, m, mut b), KeyCode::Backspace) => {
                b.pop();
                Sub::Paste(p, m, b)
            }
            (Sub::Paste(p, m, mut b), KeyCode::Char(c))
                if !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                b.push(c);
                Sub::Paste(p, m, b)
            }
            (Sub::Paste(p, m, b), KeyCode::Enter) => match clean_key(&b) {
                None if b.trim().is_empty() => Sub::Paste(p, m, b),
                None => {
                    self.note = Some(Note::NotAKey);
                    Sub::Paste(p, m, String::new())
                }
                Some(key) if stored(&self.home, &p.id) => Sub::Confirm(p, m, key),
                Some(key) => {
                    self.note = None;
                    self.start_check(p, m, Some(key), env)
                }
            },
            (Sub::Confirm(p, m, key), KeyCode::Enter) => self.start_check(p, m, Some(key), env),
            // a failed check: try again, or another provider
            (Sub::Failed(p, m, crate::keycheck::Fail::Model), KeyCode::Enter) => {
                let i = self.models_of(&p).iter().position(|x| *x == m).unwrap_or(0);
                Sub::Model(p, i)
            }
            (Sub::Failed(p, m, _), KeyCode::Enter) => Sub::Paste(p, m, String::new()),
            (Sub::Failed(p, _, _), KeyCode::Tab) => Sub::Which(self.providers.iter().position(|x| x.id == p.id).unwrap_or(0)),
            (s, _) => s,
        };
        Out::Stay
    }

    /// The models offered for `p`: its pick first, then the catalog's chat
    /// models of that provider.
    pub(crate) fn models_of(&self, p: &Provider) -> Vec<String> {
        let mut v: Vec<String> = pick_of(p, &self.model).into_iter().collect();
        for m in self.setup.catalog.models.iter().filter(|m| m.provider == p.id && !m.stt) {
            let full = format!("{}/{}", m.provider, m.id);
            if !v.contains(&full) {
                v.push(full);
            }
        }
        v
    }

    /// Start the live check of `model` with `key` (None: the key found for
    /// the provider) on a thread; [`Onb::tick`] takes its answer.
    fn start_check(&mut self, p: Provider, model: String, key: Option<String>, env: Env) -> Sub {
        let the_key = match &key {
            Some(k) => k.clone(),
            None => match self.found_key(&p, env) {
                Some(k) => k,
                None => return Sub::Paste(p, model, String::new()),
            },
        };
        let r = self.setup.catalog.resolve(&model);
        let call = crate::keycheck::Call {
            provider: p.id.clone(),
            api: r.api.clone(),
            base_url: r.base_url.clone(),
            model: r.id.clone(),
            key: the_key,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (check, url) = (self.checker, env("BEND_PROVIDER_URL"));
        std::thread::spawn(move || {
            let _ = tx.send(check(&call, url));
        });
        self.pending = Some(rx);
        Sub::Checking(p, model, key)
    }

    /// The key of `p` where the harness finds it (env, auth.json, .env).
    fn found_key(&self, p: &Provider, env: Env) -> Option<String> {
        use bise_catalog::auth::{EnvFile, Keys, Store};
        let paths = auth_paths(&self.home);
        let store = Store::read(&paths.auth_file).unwrap_or_default();
        let files = EnvFile::read_all(&paths.env_files);
        let keys = Keys { env, store: &store, files: &files };
        keys.find(&p.id, &p.key_env).map(|k| k.key)
    }

    /// The check's answer, when it came: it works (the key saved when it
    /// was pasted, the model written) or it failed (why).
    pub(crate) fn tick(&mut self, env: Env) {
        let Some(rx) = &self.pending else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err(crate::keycheck::Fail::Unreachable("the check stopped".into())),
        };
        self.pending = None;
        let Sub::Checking(p, model, key) = std::mem::replace(&mut self.sub, Sub::List) else { return };
        self.sub = match answer {
            Err(f) => Sub::Failed(p, model, f),
            Ok(()) => match self.keep(&p, &model, key.as_deref(), env) {
                Ok(()) => Sub::Works(p, model),
                Err(e) => {
                    self.note = Some(Note::Failed(e));
                    Sub::List
                }
            },
        };
    }

    /// A key that passed: saved by `login`'s own code (auth.json, 0600)
    /// when it was pasted, and its model written in config.toml.
    fn keep(&mut self, p: &Provider, model: &str, key: Option<&str>, env: Env) -> Result<(), String> {
        self.shadows = None;
        if let Some(key) = key {
            let paths = auth_paths(&self.home);
            let cp = self.setup.catalog.provider(&p.id).ok_or_else(|| format!("unknown provider {}", p.id))?;
            bise_catalog::auth_cli::login(&paths, cp, key, env).map(|_| ())?;
            let store = bise_catalog::auth::Store::read(&paths.auth_file).unwrap_or_default();
            self.shadows = bise_catalog::auth::Keys { env, store: &store, files: &[] }.shadowed(&p.id, &p.key_env);
        }
        save_model(&self.home, model).map_err(|e| format!("couldn't write config.toml: {}", e))?;
        self.setup = setup_of(env, &self.home);
        self.model = self.setup.model.clone();
        self.mine = self.setup.catalog.resolve(&self.model).provider;
        self.refresh_keys(env);
        Ok(())
    }

    /// A bracketed paste: into the key field only.
    pub(crate) fn on_paste(&mut self, s: &str) {
        if let Sub::Paste(_, _, b) = &mut self.sub {
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
const PRESS: &str = "any key ↵";
const HI_AT: u64 = 300;
const KISS_AT: u64 = HI_AT + 12 * 70 + 250;
/// the name's definition, a blank row under the first line, just after the
/// pop: its four lines one by one, `GLOSS_STEP` ms apart
const GLOSS_AT: u64 = KISS_AT + 900;
const GLOSS_STEP: u64 = 300;
const TAG_AT: u64 = GLOSS_AT + 3 * GLOSS_STEP + 1000;
const PRESS_AT: u64 = TAG_AT + (TAGLINE.len() as u64 - 1) * 35 + 500;
/// when the welcome is fully written
pub(crate) const WELCOME_END: u64 = PRESS_AT + 9 * 30;
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

/// The meanings of the definition, the third (what bise is) in text color.
const MEANINGS: [&str; 3] =
    ["a quick kiss on the cheek :*", "a brisk north wind", "a terminal where multi-agent coding is painless"];

/// The name's definition, as on the landing and the README: `bise` bold,
/// `/beez/ · french, n.` dim (the `·` follows BISE_ASCII), then the three
/// numbered meanings, dim, `:*` in accent. A block centered as a whole, its
/// lines left-aligned inside (padded to the widest); a meaning wider than
/// `w` wraps with a 3-column hanging indent. Line `i` shows from
/// `GLOSS_AT + i * GLOSS_STEP`; before, a blank row holds its place.
fn gloss(t: u64, w: u16) -> Vec<Line<'static>> {
    let mut rows: Vec<(usize, Vec<Span<'static>>)> = vec![(
        0,
        vec![
            bold("bise", theme::text()),
            s(format!(" /beez/ {} french, n.", theme::glyph("·")), theme::dim()),
        ],
    )];
    for (i, m) in MEANINGS.iter().enumerate() {
        let c = if i == 2 { theme::text() } else { theme::dim() };
        for (k, r) in words_in(m, (w as usize).saturating_sub(3)).into_iter().enumerate() {
            let r = format!("{}{}", if k == 0 { format!("{}. ", i + 1) } else { "   ".to_string() }, r);
            let spans = match r.strip_suffix(":*") {
                Some(head) => vec![s(head.to_string(), c), bold(theme::glyph(theme::G_MAIN), theme::accent())],
                None => vec![s(r, c)],
            };
            rows.push((i + 1, spans));
        }
    }
    let width = |sp: &[Span]| sp.iter().map(|x| x.content.width()).sum::<usize>();
    let block = rows.iter().map(|(_, sp)| width(sp)).max().unwrap_or(0);
    rows.into_iter()
        .map(|(i, mut sp)| {
            if t < GLOSS_AT + i as u64 * GLOSS_STEP {
                return Line::raw("");
            }
            let pad = block - width(&sp);
            if pad > 0 {
                sp.push(Span::raw(" ".repeat(pad)));
            }
            Line::from(sp)
        })
        .collect()
}

/// `any key ↵` typed, dim, `any key` in text color.
fn press(t: u64) -> Line<'static> {
    let shown = typed(PRESS, PRESS_AT, 30, t).chars().count();
    let mut spans = Vec::new();
    let mut at = 0;
    for (part, key) in [("any key", true), (" ↵", false)] {
        let n = part.chars().count().min(shown.saturating_sub(at));
        if n > 0 {
            let p: String = part.chars().take(n).collect();
            spans.push(s(p, if key { theme::text() } else { theme::dim() }));
        }
        at += part.chars().count();
    }
    Line::from(spans)
}

/// The welcome in a column `w` wide: hi, a blank row, the definition, a
/// blank row, the tagline, `gap` rows, the hint.
fn welcome(t: u64, gap: usize, w: u16) -> Vec<Line<'static>> {
    let mut v = vec![Line::from(vec![bold(typed(HI, HI_AT, 70, t).to_string(), theme::text()), kiss(t)])];
    blanks(&mut v, 1);
    v.extend(gloss(t, w));
    blanks(&mut v, 1);
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
        let border = if m == pick { theme::accent() } else { theme::rule() };
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

/// Every provider you can paste a key for: "anthropic, openai, … and zai"
/// (all of them: "and 9 more" does not say what you can paste).
fn paste_sub(ps: &[Provider]) -> String {
    let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
    match ids.split_last() {
        None => String::new(),
        Some((last, [])) => last.to_string(),
        Some((last, rest)) => format!("{} and {}", rest.join(", "), last),
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
    use crate::keycheck::Fail;
    let dim = |t: String| Line::from(s(t, theme::dim()));
    match &o.sub {
        Sub::List => model_list(o, w, gap),
        Sub::Which(i) => {
            let mut v = vec![title("which provider?")];
            blanks(&mut v, gap);
            // a window of WHICH_ROWS rows around the cursor
            let n = o.providers.len();
            let from = i.saturating_sub(WHICH_ROWS / 2).min(n.saturating_sub(WHICH_ROWS));
            if from > 0 {
                v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
            }
            for (k, p) in o.providers.iter().enumerate().skip(from).take(WHICH_ROWS) {
                option(&mut v, k == *i, vec![s(format!("{} · {}  ", k + 1, p.name), theme::text()), s(p.hint.clone(), theme::dim())], "", w);
            }
            if from + WHICH_ROWS < n {
                v.push(Line::from(s(format!("  ↓ {} more", n - from - WHICH_ROWS), theme::dim())));
            }
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
            v
        }
        Sub::Model(p, i) => {
            let ms = o.models_of(p);
            let mut v = vec![title("which model?"), dim("you can change it any time with /model.".into())];
            blanks(&mut v, gap);
            let from = i.saturating_sub(WHICH_ROWS / 2).min(ms.len().saturating_sub(WHICH_ROWS));
            for (k, m) in ms.iter().enumerate().skip(from).take(WHICH_ROWS) {
                let mut name = vec![s(format!("{} · {}", k + 1, m), theme::text())];
                if k == 0 && !p.model.is_empty() {
                    name.push(s("  recommended", theme::accent()));
                }
                option(&mut v, k == *i, name, "", w);
            }
            if ms.is_empty() {
                v.push(dim(format!("{} has no model listed: set model in config.toml.", p.name)));
            }
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
            v
        }
        Sub::Paste(p, _, b) => {
            let dots: String = "•".repeat(b.chars().count().min(48));
            let mut v = vec![title(format!("paste your {} key", p.name))];
            blanks(&mut v, 1);
            if !p.keys_url.is_empty() {
                v.push(Line::from(vec![s("get one: ", theme::dim()), s(p.keys_url.clone(), theme::text())]));
            }
            if !p.signup_url.is_empty() {
                v.push(Line::from(vec![s("no account yet? ", theme::dim()), s(p.signup_url.clone(), theme::text())]));
            }
            blanks(&mut v, gap);
            v.push(Line::from(vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent()), s(dots, theme::text()), s("█", theme::text())]));
            if o.note == Some(Note::NotAKey) {
                v.push(Line::raw(""));
                v.push(Line::from(s(format!("{} that doesn't look like a key: no spaces inside.", theme::glyph(theme::G_FAILED)), theme::error())));
            }
            blanks(&mut v, 1);
            v.push(dim(format!("saved in {}. only you can read it.", auth_shown(o))));
            blanks(&mut v, gap);
            v.push(keyline("{enter} check · {esc} back"));
            v
        }
        Sub::Confirm(p, _, _) => {
            let mut v = vec![title(format!("{} has a key in {} already.", p.name, auth_shown(o)))];
            blanks(&mut v, gap);
            v.push(keyline("{enter} replaces it · {esc} keeps the old one"));
            v
        }
        Sub::Checking(p, m, _) => {
            let mut v = vec![title("checking your key with one tiny call…"), dim(format!("{} on {}", short_model(m), p.name))];
            blanks(&mut v, gap);
            v.push(keyline("{esc} back"));
            v
        }
        Sub::Failed(p, m, f) => {
            let link = if p.keys_url.is_empty() { None } else { Some(p.keys_url.clone()) };
            let err = |t: String| Line::from(s(format!("{} {}", theme::glyph(theme::G_FAILED), t), theme::error()));
            let mut v = Vec::new();
            match f {
                Fail::WrongKey => {
                    v.push(err(format!("{} says this key is wrong.", p.name)));
                    if let Some(u) = &link {
                        v.push(Line::from(vec![s("copy it again from ", theme::dim()), s(u.clone(), theme::text())]));
                    }
                }
                Fail::NoCredit => {
                    v.push(err("the key works, but the account has no credit.".into()));
                    if let Some(u) = &link {
                        v.push(Line::from(vec![s("add some from ", theme::dim()), s(u.clone(), theme::text()), s(", then enter.", theme::dim())]));
                    }
                }
                Fail::Model => v.push(err(format!("{} doesn't know {}. pick another model.", p.name, short_model(m)))),
                Fail::Unreachable(e) => {
                    v.push(err(format!("i couldn't reach {}: {}.", p.name, e.trim_end_matches('.'))));
                    v.push(dim("check your network, then enter.".into()));
                }
            }
            blanks(&mut v, gap);
            v.push(keyline(if *f == Fail::Model {
                "{enter} pick another model · {tab} another provider · {esc} back"
            } else {
                "{enter} try again · {tab} another provider · {esc} back"
            }));
            v
        }
        Sub::Works(_, m) => {
            let mut v = vec![title(format!("it works: {} answered.", short_model(m))), dim(format!("main uses {}.", m))];
            if let Some(n) = &o.shadows {
                v.push(dim(format!("{} in your environment holds another key: i use this one.", n)));
            }
            let extras = extras(o);
            if !extras.is_empty() {
                blanks(&mut v, gap);
                v.push(dim("optional. add these any time:".into()));
                for e in extras {
                    v.push(Line::from(s(format!("  {}", e), theme::text())));
                }
            }
            blanks(&mut v, gap);
            v.push(keyline("{enter} go on"));
            v
        }
    }
}

/// A model's id without its provider.
fn short_model(m: &str) -> String {
    bise_catalog::split_name(m).map_or(m.to_string(), |(_, id)| id.to_string())
}

/// What the optional keys unlock, for those not set (BISE-266): the
/// connectors (web search…) run on a Mistral key; voice input on one of
/// the providers that transcribe.
fn extras(o: &Onb) -> Vec<String> {
    let has = |id: &str| o.found.iter().any(|p| p.id == id);
    let mut v = Vec::new();
    if !has("mistral") {
        v.push("web search and other tools: a Mistral key · /setup".to_string());
    }
    let voice = ["mistral", "openai", "groq"].iter().any(|id| has(id));
    if !voice {
        v.push("voice input (ctrl+r): a Mistral, OpenAI, Groq, ElevenLabs or Deepgram key · bise login".to_string());
    }
    v
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
                    // BISE-266: enter moves the model to this provider
                    match pick_of(&p, &o.model) {
                        Some(m) => format!("{}. i'll use {}.", p.name, short_model(&m)),
                        None => format!("{}. set model in {}.", p.id, bise_catalog::auth::tilde(&o.home.config_file(), Some(o.home.user_home()))),
                    }
                },
            ),
            Opt::Paste => (
                vec![s(
                    format!("{} · {}", n, if o.found.is_empty() { "set up a provider" } else { "set up another provider" }),
                    theme::text(),
                )],
                paste_sub(&o.providers),
            ),
        };
        option(&mut v, i == o.sel, name, &sub, w);
    }
    if let Some(Note::Failed(e)) = &o.note {
        v.push(Line::raw(""));
        v.push(Line::from(s(format!("{} couldn't save the key: {}", theme::glyph(theme::G_FAILED), e), theme::error())));
    }
    blanks(&mut v, gap);
    v.push(keyline("{↑↓} choose · {enter} ok"));
    v
}

/// How it works (designer's v4 copy, as the landing: main is your team
/// lead): the three numbered lines, `me` and `i` (bise) in accent, the
/// numbers dim, no final periods. Each fits the 64-column column.
const HOW: [[&str; 3]; 3] = [
    ["you talk to ", "me", ": main, your team lead. any time, keep typing"],
    ["", "i", " start an agent when a job needs one. they sync on their own"],
    ["only the real decisions reach you, in your inbox · ctrl+g", "", ""],
];
/// The faint footer under the three lines.
const HOW_FOOT: &str = "ctrl+o opens everything folded · ⌥0-9 talk to an agent";

/// One HOW line in a column `w` wide: the number dim, `me`/`i` bold in
/// accent, the rest in text color; wider than `w`, it wraps at the words
/// with a 3-column hanging indent (like the welcome's meanings).
fn how_rows(i: usize, [a, me, b]: [&str; 3], w: u16) -> Vec<Line<'static>> {
    let (a_end, me_end) = (a.chars().count(), a.chars().count() + me.chars().count());
    let mut at = 0; // chars of the whole line already placed
    let mut out = Vec::new();
    for (k, row) in words_in(&format!("{}{}{}", a, me, b), (w as usize).saturating_sub(3)).into_iter().enumerate() {
        let mut spans = vec![if k == 0 { s(format!("{}  ", i + 1), theme::dim()) } else { Span::raw("   ") }];
        // cut the row where the accent starts and ends
        let n = row.chars().count();
        let cut = |x: usize| x.saturating_sub(at).min(n);
        let part = |from: usize, to: usize| row.chars().skip(from).take(to - from).collect::<String>();
        let (c1, c2) = (cut(a_end), cut(me_end));
        for (text, accent) in [(part(0, c1), false), (part(c1, c2), true), (part(c2, n), false)] {
            if !text.is_empty() {
                spans.push(if accent { bold(text, theme::accent()) } else { s(text, theme::text()) });
            }
        }
        out.push(Line::from(spans));
        at += n + 1; // the space the wrap ate
    }
    out
}

fn how_lines(t: u64, gap: usize, w: u16) -> Vec<Line<'static>> {
    let shown = |i: u64| t >= 400 + i * 900;
    let mut v = vec![title("how it works")];
    blanks(&mut v, gap);
    for (i, line) in HOW.iter().enumerate() {
        if i > 0 {
            v.push(Line::raw(""));
        }
        for row in how_rows(i, *line, w) {
            v.push(if shown(i as u64) { row } else { Line::raw("") });
        }
    }
    blanks(&mut v, 1);
    v.push(if shown(3) { Line::from(s(HOW_FOOT, theme::faint())) } else { Line::raw("") });
    blanks(&mut v, gap);
    v.push(if shown(3) { keyline("{any key} ↵") } else { Line::raw("") });
    v
}

/// The step dots, one per step shown (`○ ● ○ ○`, a key found: 3), faint,
/// the current one in accent.
fn dots(o: &Onb) -> Line<'static> {
    let mut v = Vec::new();
    for (i, st) in o.steps().into_iter().enumerate() {
        if i > 0 {
            v.push(Span::raw(" "));
        }
        v.push(if st == o.step { s("●", theme::accent()) } else { s("○", theme::faint()) });
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

/// Each `https://` span of `lines` (the keys pages, a row of their own
/// that fits the column) drawn as a link: the link look, and a hit where
/// it lands, so the backend wraps its cells in OSC 8.
fn linked(mut lines: Vec<Line<'static>>, col: Rect, y: u16) -> Vec<Line<'static>> {
    crate::links::begin_frame();
    let mut tag = 0u8;
    for i in 0..lines.len() {
        let row = y + height_of(&lines[..i], col.width);
        let mut x = col.x;
        for sp in lines[i].spans.iter_mut() {
            let w = sp.content.width() as u16;
            if sp.content.starts_with("https://") && x + w <= col.right() {
                tag += 1;
                let url = sp.content.to_string();
                sp.style = crate::links::link_style(sp.style, theme::text(), tag);
                crate::links::push_hit(crate::links::Hit { y: row, x0: x, x1: x + w, tag, id: format!("onb{}", tag), url });
            }
            x += w;
        }
    }
    lines
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
        Step::Welcome => (welcome(if o.rushed { u64::MAX } else { t }, gap, col.width), 0),
        // the previews (1 blank row above), then the key line after a gap
        Step::Theme => (theme_text(o), 1 + PREVIEW_H + gap as u16 + 1),
        Step::Model => (model_lines(o, col.width, gap), 0),
        Step::Lines => (how_lines(t, gap, col.width), 0),
    };
    let text_h = height_of(&lines, col.width).min(body.height);
    let h = text_h + extra;
    let y = body.y + body.height.saturating_sub(h) * 2 / 5;
    // BISE-266: the keys pages are clickable (OSC 8, links.rs)
    let lines = linked(lines, col, y);
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
        f.render_widget(Paragraph::new(dots(o)).alignment(Alignment::Center), Rect { y: dy, height: 1, ..area });
    }
}

// ---- the mouse (BISE-281) ----

/// What a mouse gesture asks the loop to do.
#[derive(Debug, PartialEq)]
enum Act {
    Open(String),
    Copy(String),
}

/// How long the note under the dots stays, in ms.
const NOTE_MS: u64 = 2500;

/// The mouse over the onboarding. bise has the mouse (capture on, so the
/// terminal's own click and selection never happen), so it does what a
/// click and a drag do in the feed: a plain click on a link opens it
/// (`links::open`: `BISE_OPEN`, `open`, `xdg-open`; cmd+click stays the
/// terminal's own), a drag selects (highlighted) and the release copies,
/// a double click selects the word (a whole url), a triple the row. The
/// note under the dots says what happened, in the feed's words.
#[derive(Default)]
struct Mouse {
    clicks: crate::app::MouseState,
    /// the press and where the drag is now, (x, y) cells
    sel: Option<((u16, u16), (u16, u16))>,
    /// the press became a selection (a drag, a double or triple click)
    moved: bool,
    /// the button is down
    held: bool,
    /// where the mouse was last seen
    at: Option<(u16, u16)>,
    /// the note and when it goes (the onboarding's clock, ms)
    note: Option<(String, u64)>,
}

/// Row `y` of `b` as text: one char per column (a wide grapheme's
/// hidden cells skipped), so a column of the string is a cell.
fn row_text(b: &ratatui::buffer::Buffer, y: u16) -> String {
    let mut s = String::new();
    let mut skip = 0usize;
    for x in b.area.x..b.area.right() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        let sym = b[(x, y)].symbol();
        skip = sym.width().saturating_sub(1);
        s.push_str(if sym.is_empty() { " " } else { sym });
    }
    s
}

/// The columns [from, to] of `s` that are text (not the margins).
fn text_cols(s: &str) -> Option<(usize, usize)> {
    let from = s.len() - s.trim_start().len();
    let to = s.trim_end().width().checked_sub(1)?;
    Some((s[..from].width(), to))
}

impl Mouse {
    /// The selection, ordered, when there is one to show.
    fn range(&self) -> Option<((u16, u16), (u16, u16))> {
        let (a, b) = self.sel.filter(|_| self.moved)?;
        Some(if (a.1, a.0) <= (b.1, b.0) { (a, b) } else { (b, a) })
    }

    /// The selected columns [from, to] of row `y` of `b`, text only.
    fn cols(&self, b: &ratatui::buffer::Buffer, y: u16) -> Option<(u16, u16)> {
        let (a, z) = self.range()?;
        if y < a.1 || y > z.1 {
            return None;
        }
        let (t0, t1) = text_cols(&row_text(b, y))?;
        let from = if y == a.1 { (a.0 as usize).max(t0) } else { t0 };
        let to = if y == z.1 { (z.0 as usize).min(t1) } else { t1 };
        (from <= to).then_some((from as u16 + b.area.x, to as u16 + b.area.x))
    }

    /// The selected text of `b`: its rows trimmed, one per line.
    fn text(&self, b: &ratatui::buffer::Buffer) -> String {
        let Some((a, z)) = self.range() else { return String::new() };
        let rows: Vec<String> = (a.1..=z.1)
            .map(|y| match self.cols(b, y) {
                Some((f, t)) => crate::feedsel::slice_cols(&row_text(b, y), (f - b.area.x) as usize, (t - b.area.x) as usize + 1),
                None => String::new(),
            })
            .collect();
        rows.join("\n").trim_matches('\n').to_string()
    }

    /// The selection on `b`: the feed's tint under it.
    fn paint(&self, b: &mut ratatui::buffer::Buffer) {
        let Some((a, z)) = self.range() else { return };
        for y in a.1..=z.1.min(b.area.bottom().saturating_sub(1)) {
            if let Some((f, t)) = self.cols(b, y) {
                for x in f..=t.min(b.area.right().saturating_sub(1)) {
                    b[(x, y)].bg = theme::selection_bg();
                }
            }
        }
    }

    /// One mouse event over the last frame `b` and its links `hits`.
    fn on(&mut self, m: &crossterm::event::MouseEvent, b: &ratatui::buffer::Buffer, hits: &[crate::links::Hit], at: Instant) -> Option<Act> {
        use crossterm::event::{MouseButton, MouseEventKind};
        let inside = b.area.contains((m.column, m.row).into());
        let (x, y) = (
            m.column.clamp(b.area.x, b.area.right().saturating_sub(1)),
            m.row.clamp(b.area.y, b.area.bottom().saturating_sub(1)),
        );
        self.at = Some((m.column, m.row));
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) if inside => {
                let n = self.clicks.press(x, y, at);
                let row = row_text(b, y);
                let col = (x - b.area.x) as usize;
                let span = match n {
                    2 => Some(crate::feedsel::word_cols(&row, col)),
                    3 => text_cols(&row),
                    _ => None,
                };
                self.held = true;
                match span {
                    Some((f, t)) => {
                        self.sel = Some(((f as u16 + b.area.x, y), (t as u16 + b.area.x, y)));
                        self.moved = true;
                    }
                    None => {
                        self.sel = Some(((x, y), (x, y)));
                        self.moved = false;
                    }
                }
                None
            }
            MouseEventKind::Drag(MouseButton::Left) if self.held => {
                if let Some((a, h)) = self.sel.as_mut() {
                    if *h != (x, y) {
                        *h = (x, y);
                        self.moved |= *a != (x, y);
                    }
                }
                None
            }
            MouseEventKind::Up(MouseButton::Left) if self.held => {
                self.held = false;
                if self.moved {
                    return Some(self.text(b)).filter(|t| !t.is_empty()).map(Act::Copy);
                }
                let (px, py) = self.sel.take()?.0;
                hits.iter().find(|h| h.y == py && px >= h.x0 && px < h.x1).map(|h| Act::Open(h.url.clone()))
            }
            _ => None,
        }
    }

    /// Does `act` (opens the url, copies the text) and keeps its note.
    fn act(&mut self, act: Act, now: u64) {
        let note = match act {
            Act::Open(url) if crate::links::open(&url) => format!("opening {}", url),
            Act::Open(url) => format!("could not open {}", url),
            Act::Copy(t) if crate::clipboard::copy(&t) => {
                let n = t.chars().count();
                format!("copied {} char{}", n, if n == 1 { "" } else { "s" })
            }
            Act::Copy(_) => "copy failed (no pbcopy, and the terminal refused OSC 52)".to_string(),
        };
        self.note = Some((note, now + NOTE_MS));
    }

    /// The pointer's shape: a hand over a link, none while it drags.
    fn shape(&self) -> crate::pointer::Shape {
        match self.at {
            Some((x, y)) if !self.held => crate::pointer::at(x, y),
            _ => crate::pointer::Shape::Default,
        }
    }

    /// The note on the last row, under the dots, while it lasts.
    fn draw_note(&self, f: &mut Frame, now: u64) {
        let area = f.area();
        if let Some((n, _)) = self.note.as_ref().filter(|(_, until)| now < *until && area.height >= 4) {
            let r = Rect { y: area.bottom() - 1, height: 1, ..area };
            f.render_widget(Paragraph::new(Line::from(s(n.clone(), theme::dim()))).alignment(Alignment::Center), r);
        }
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
    let mut o = Onb::new(&real_env);
    if KEYS_ONLY.swap(false, Ordering::SeqCst) && o.ask_key {
        o.keys_only = true;
        o.go(Step::Model, 0);
    }
    let _ = terminal.clear();
    let mut mouse = Mouse::default();
    let mut last = ratatui::buffer::Buffer::empty(Rect::default());
    let r = (|| -> io::Result<()> {
        loop {
            pump(app);
            if app.should_quit {
                return Ok(());
            }
            // BISE-266: the key check's answer, when it came
            o.tick(&real_env);
            let now = t0.elapsed().as_millis() as u64;
            // BISE-92: the switch repaints the terminal's background too
            crate::theme_detect::sync_terminal_bg();
            terminal.draw(|f| {
                crate::pointer::begin_frame(); // BISE-272: the hand over the links
                draw(f, &o, now);
                mouse.draw_note(f, now);
                theme::paint(f.buffer_mut()); // BISE-92: bise paints its ground
                theme::asciify(f.buffer_mut()); // BISE-84: BISE_ASCII=1
                // BISE-281: what the mouse selects is read from this frame
                last = f.buffer_mut().clone();
                mouse.paint(f.buffer_mut());
            })?;
            terminal.backend_mut().set_pointer(mouse.shape())?;
            if !poll(Duration::from_millis(33))? {
                continue;
            }
            let now = t0.elapsed().as_millis() as u64;
            // the ctrl hints' flags: a modifier alone or a release is no key
            match crate::ctrlhint::for_handlers(read()?) {
                Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    // the screen changes: the selection goes
                    mouse.sel = None;
                    if o.on_key(k, now, &real_env) != Out::Stay {
                        return Ok(());
                    }
                }
                Some(Event::Paste(p)) => o.on_paste(&p),
                // BISE-281: a click on a link opens it, a drag copies
                Some(Event::Mouse(m)) => {
                    if let Some(act) = mouse.on(&m, &last, &crate::links::frame_hits(), Instant::now()) {
                        mouse.act(act, now);
                    }
                }
                _ => {}
            }
        }
    })();
    let _ = terminal.backend_mut().set_pointer(crate::pointer::Shape::Default);
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
    use std::path::{Path, PathBuf};

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

    fn onb(home: &Path, _ws: &str) -> Onb {
        let e = env_of(HashMap::from([("HOME", home.to_string_lossy().to_string())]));
        Onb::new(&e)
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
        let o = Onb::new(&none);
        // BISE-266: no built-in model
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("", ""));
        std::fs::create_dir_all(h.join(".bend-harness")).unwrap();
        std::fs::write(h.join(".bend-harness/config.toml"), "# c\nmodel = \"zai-glm-5-3\" # glm\n").unwrap();
        let o = Onb::new(&none);
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("mistral/zai-glm-5-3", "mistral"));
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BEND_MODEL", "anthropic/claude-haiku-4-5".to_string())]));
        let o = Onb::new(&e);
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
        // the definition comes after the pop, line by line, before the
        // tagline, a blank row under the name
        let gloss = [
            "bise /beez/ · french, n.",
            "1. a quick kiss on the cheek :*",
            "2. a brisk north wind",
            "3. a terminal where multi-agent coding is painless",
        ];
        let sc = screen(&o, KISS_AT + 700, 100, 30);
        assert!(sc.contains("hi, i'm bise :*") && !sc.contains("bise /beez/"), "{}", sc);
        let sc = screen(&o, GLOSS_AT + GLOSS_STEP, 100, 30);
        assert!(sc.contains(gloss[1]) && !sc.contains(gloss[2]), "one line at a time: {}", sc);
        let rows_at = |t: u64| {
            let sc = screen(&o, t, 100, 30);
            let rows: Vec<String> = sc.lines().map(str::to_string).collect();
            (sc, rows)
        };
        let (sc, rows) = rows_at(GLOSS_AT + 3 * GLOSS_STEP);
        assert!(!sc.contains("ideas in."), "{}", sc);
        let hi = rows.iter().position(|r| r.contains("hi, i'm bise :*")).unwrap();
        assert!(rows[hi + 1].trim().is_empty(), "{}", sc);
        // a block centered as a whole, its lines left-aligned inside
        let left = rows[hi + 2].find("bise /beez/").unwrap();
        for (k, g) in gloss.iter().enumerate() {
            assert_eq!(rows[hi + 2 + k].find(g), Some(left), "{}\n{}", g, sc);
        }
        let right = left + gloss[3].len();
        assert!(left.abs_diff(100 - right) <= 1, "centered: {}", sc);
        // the layout does not move when the lines come
        let (_, before) = rows_at(GLOSS_AT - 1);
        assert_eq!(before.iter().position(|r| r.contains("hi, i'm bise :*")), Some(hi));
        let w = welcome(GLOSS_AT + 3 * GLOSS_STEP, 2, 64);
        let fg = |l: usize, sp: usize| w[l].spans[sp].style.fg;
        assert!(w[2].spans[0].style.add_modifier.contains(Modifier::BOLD), "bise: bold");
        assert_eq!((fg(2, 1), fg(3, 0), fg(4, 0)), (Some(theme::dim()), Some(theme::dim()), Some(theme::dim())), "read: dim");
        assert_eq!(fg(3, 1), Some(theme::accent()), ":* in accent");
        assert_eq!(fg(5, 0), Some(theme::text()), "what bise is: text");
        // narrow: the meanings wrap with a hanging indent, the block still aligned
        let sc = screen(&o, WELCOME_END, 36, 30);
        assert!(flat(&sc).contains("3. a terminal where multi-agent coding is painless"), "{}", sc);
        let r3 = sc.lines().position(|r| r.contains("3. a terminal")).unwrap();
        let rows: Vec<&str> = sc.lines().collect();
        let x = rows[r3].find("3.").unwrap();
        assert_eq!(rows[r3 + 1].find(|c: char| c != ' '), Some(x + 3), "hanging indent: {}", sc);
        assert_eq!(rows[r3 - 1].find("2."), Some(x), "{}", sc);
        // any key while it types shows it all at once (enter too), then
        // any key goes on
        let none = env_of(HashMap::new());
        for k in [KeyCode::Char('x'), KeyCode::Enter] {
            let mut r = onb(&h, "/w");
            assert_eq!(r.on_key(key(k), 5, &none), Out::Stay);
            assert_eq!(r.step, Step::Welcome);
            let sc = screen(&r, 10, 100, 30);
            assert!(sc.contains(gloss[3]) && sc.contains("any key ↵"), "{}", sc);
            assert_eq!(r.on_key(key(KeyCode::Char('y')), 20, &none), Out::Stay);
            assert_eq!(r.step, Step::Theme);
        }
        let mut r = onb(&h, "/w");
        assert_eq!(r.on_key(key(KeyCode::Char(' ')), WELCOME_END, &none), Out::Stay);
        assert_eq!(r.step, Step::Theme, "written: any key goes on, no enter needed");
        let sc = screen(&o, WELCOME_END, 100, 30);
        assert!(!sc.contains("press enter"), "{}", sc);
        // no key in this home: welcome, theme, the key, how it works
        for s in ["hi, i'm bise :*", "ideas in. little kisses out. also pull requests.", "any key ↵", "● ○ ○ ○"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
    }

    #[test]
    fn step_2_theme_switches_live() {
        let h = tmp("s2");
        let mut o = onb(&h, "/w");
        o.detected = Some(Mode::Dark);
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Enter), WELCOME_END, &none), Out::Stay);
        assert_eq!(o.step, Step::Theme);
        let sc = screen(&o, 20, 100, 30);
        for s in [
            "your terminal looks dark, so i picked dark.",
            "you can change it any time with /theme.",
            "fix the flaky login test",
            "on it: auth-fix takes it.",
            "auth-fix is done.",
            "←→ switch · enter keep",
            "○ ● ○ ○",
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
    fn step_3_model_lists_found_keys_and_offers_every_provider() {
        let h = tmp("s3");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("ANTHROPIC_FOUNDRY_API_KEY", "k".to_string()),
            ("BISE_MODEL", "opus-5.5".to_string()),
        ]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        for s in [
            "which model should do the work?",
            "i found a key in your environment.",
            "1 · use ANTHROPIC_FOUNDRY_API_KEY found",
            "foundry, already set up. nothing to paste.",
            "2 · set up another provider",
            "↑↓ choose · enter ok",
        ] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // every provider offered, by name, no "and 9 more"; the private
        // proxy never (BISE-266)
        let all = paste_sub(&o.providers);
        assert!(all.starts_with("anthropic, openai, ") && !all.contains("foundry") && !all.ends_with('.'), "{}", all);
        assert!(flat(&sc).contains(&all) && !sc.contains(" more"), "{}", sc);
        // API keys only: no browser sign-in row (BISE-215); ↑↓ wrap
        assert!(!sc.contains("3 · ") && !sc.contains("browser"), "{}", sc);
        o.on_key(key(KeyCode::Down), 1, &e);
        assert_eq!(o.sel, 1);
        o.on_key(key(KeyCode::Down), 1, &e);
        assert_eq!(o.sel, 0);
        // the providers: name and hint, 9 rows at a time
        o.on_key(key(KeyCode::Up), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert_eq!(o.sub, Sub::Which(0));
        let sc = screen(&o, 10, 110, 30);
        for s in ["which provider?", "1 · Anthropic  Claude, by Anthropic", "OpenRouter  one key for most models", "↓ 2 more"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // enter on the model in use goes on
        o.on_key(key(KeyCode::Esc), 1, &e);
        o.sel = 0;
        o.on_key(key(KeyCode::Enter), 5, &e);
        assert_eq!(o.step, Step::Lines);
    }

    /// A fake check: "bad" keys are wrong, "broke" ones have no credit,
    /// the others pass.
    fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
        use crate::keycheck::Fail;
        match c.key.as_str() {
            k if k.contains("bad") => Err(Fail::WrongKey),
            k if k.contains("broke") => Err(Fail::NoCredit),
            _ => Ok(()),
        }
    }

    /// Wait for the check's answer (its thread).
    fn settle(o: &mut Onb, e: Env) {
        for _ in 0..200 {
            o.tick(e);
            if !matches!(o.sub, Sub::Checking(..)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the check never answered");
    }

    fn type_key(o: &mut Onb, e: Env, k: &str) {
        o.on_paste(k);
        o.on_key(key(KeyCode::Enter), 1, e);
        settle(o, e);
    }

    #[test]
    fn a_pasted_key_is_checked_then_saved_with_its_model() {
        let h = tmp("flow");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.checker = fake_check;
        // no model at all (BISE-266): the step shows
        assert!(o.ask_key && o.found.is_empty() && o.model.is_empty());
        o.go(Step::Model, 0);
        assert!(screen(&o, 10, 110, 30).contains("1 · set up a provider"));
        o.on_key(key(KeyCode::Enter), 1, &e);
        let mistral = o.providers.iter().position(|p| p.id == "mistral").unwrap();
        o.sub = Sub::Which(mistral);
        o.on_key(key(KeyCode::Enter), 1, &e);
        // its models, the pick first and recommended
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("which model?") && sc.contains("1 · mistral/mistral-medium-latest  recommended"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        // the keys page, the field, where it goes
        let sc = screen(&o, 10, 110, 30);
        for s in ["paste your Mistral key", "get one: https://console.mistral.ai/api-keys", "saved in ~/.bend-harness/auth.json. only you can read it.", "enter check · esc back"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // the link is a hit for the OSC 8 backend
        assert!(crate::links::frame_hits().iter().any(|h| h.url == "https://console.mistral.ai/api-keys"));
        // a wrong key: said plainly, nothing saved, enter tries again
        type_key(&mut o, &e, "bad-key");
        assert!(matches!(&o.sub, Sub::Failed(p, _, crate::keycheck::Fail::WrongKey) if p.id == "mistral"));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("Mistral says this key is wrong.") && sc.contains("copy it again from https://console.mistral.ai/api-keys"), "{}", sc);
        assert!(sc.contains("enter try again · tab another provider · esc back"), "{}", sc);
        assert!(!hm(&h).auth_file().exists() && !hm(&h).config_file().exists());
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Paste(p, m, b) if p.id == "mistral" && m == "mistral/mistral-medium-latest" && b.is_empty()));
        // no credit: its own words
        type_key(&mut o, &e, "broke-key");
        assert!(screen(&o, 10, 110, 30).contains("the key works, but the account has no credit."));
        // tab: another provider
        o.on_key(key(KeyCode::Tab), 1, &e);
        assert_eq!(o.sub, Sub::Which(mistral));
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        type_key(&mut o, &e, "good-key");
        // it works: the key in auth.json (0600), the model in config.toml
        assert!(matches!(&o.sub, Sub::Works(_, m) if m == "mistral/mistral-medium-latest"));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("it works: mistral-medium-latest answered.") && sc.contains("main uses mistral/mistral-medium-latest."), "{}", sc);
        // a Mistral key runs the connectors and the voice input: no extras
        assert!(!sc.contains("optional.") && !sc.contains("web search"), "{}", sc);
        use std::os::unix::fs::PermissionsExt;
        let f = hm(&h).auth_file();
        assert_eq!(bise_catalog::auth::Store::read(&f).unwrap().key("mistral"), Some("good-key"));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(o.model, "mistral/mistral-medium-latest");
        assert!(!model_blocked(&o.setup, &o.found));
        assert!(!sc.contains("good-key"));
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert_eq!(o.step, Step::Lines);
    }

    #[test]
    fn a_found_key_is_checked_too_and_a_stored_one_asks_first() {
        let h = tmp("found2");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string()), ("OPENAI_API_KEY", "sk-env".to_string())]));
        let mut o = Onb::new(&e);
        o.checker = fake_check;
        assert!(o.ask_key);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("1 · use OPENAI_API_KEY found") && sc.contains("OpenAI. i'll use gpt-5.5."), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Model(p, 0) if p.id == "openai"));
        o.on_key(key(KeyCode::Enter), 1, &e);
        settle(&mut o, &e);
        assert!(matches!(&o.sub, Sub::Works(_, m) if m == "openai/gpt-5.5"));
        // nothing pasted: nothing stored; the model written
        assert!(bise_catalog::auth::Store::read(&hm(&h).auth_file()).unwrap_or_default().key("openai").is_none());
        let cfg = std::fs::read_to_string(hm(&h).config_file()).unwrap();
        assert!(cfg.starts_with("model = \"openai/gpt-5.5\""), "{}", cfg);
        // a key already in auth.json: enter replaces it only after a yes
        std::fs::create_dir_all(hm(&h).auth_file().parent().unwrap()).unwrap();
        let paths = auth_paths(&o.home);
        bise_catalog::auth_cli::login(&paths, o.setup.catalog.provider("groq").unwrap(), "old", &e).unwrap();
        o.sub = Sub::Paste(Provider::of(o.setup.catalog.provider("groq").unwrap()), "groq/openai/gpt-oss-120b".into(), String::new());
        o.on_paste("new");
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Confirm(p, _, _) if p.id == "groq"));
        assert!(screen(&o, 10, 110, 30).contains("Groq has a key in"));
        o.on_key(key(KeyCode::Esc), 1, &e);
        assert_eq!(bise_catalog::auth::Store::read(&hm(&h).auth_file()).unwrap().key("groq"), Some("old"));
    }

    #[test]
    fn step_2_says_why_when_the_theme_is_forced() {
        use crate::theme_detect::{save_in, Choice};
        let h = tmp("s2f");
        let home = h.to_string_lossy().to_string();
        // BISE_THEME: no detection, it says so
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BISE_THEME", "light".to_string())]));
        theme::set_mode(Mode::Light);
        let mut o = Onb::new(&e);
        assert_eq!(o.theme_from, ThemeFrom::Env);
        o.go(Step::Theme, 0);
        let sc = screen(&o, 10, 100, 30);
        assert!(sc.contains("BISE_THEME is set to light, so i picked it."), "{}", sc);
        assert!(!sc.contains("couldn't read"), "{}", sc);
        // a saved choice: it was picked before
        save_in(&hm(&h), Choice::Light).unwrap();
        let e = env_of(HashMap::from([("HOME", home.clone())]));
        let mut o = Onb::new(&e);
        assert_eq!(o.theme_from, ThemeFrom::Saved);
        o.go(Step::Theme, 0);
        assert!(screen(&o, 10, 100, 30).contains("you picked light last time, so i kept it."));
        // a saved choice kept as is: nothing written; changed: the new pick
        assert_eq!(o.theme_choice(), None);
        o.pick = Mode::Dark;
        assert_eq!(o.theme_choice(), Some(Choice::Dark));
        // BISE_THEME=auto: the terminal decides
        let e = env_of(HashMap::from([("HOME", home), ("BISE_THEME", "auto".to_string())]));
        assert_eq!(Onb::new(&e).theme_from, ThemeFrom::Terminal);
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
            let mut o = Onb::new(&e);
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
        let mut o = Onb::new(&e);
        o.go(Step::Theme, 0);
        o.on_key(key(KeyCode::Enter), 2, &e);
        assert_eq!(load_in(&hm(&h)), Some(Choice::Dark));
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn wrapped_details_keep_their_indent() {
        let h = tmp("wrap");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
        ]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 44, 30);
        let rows: Vec<&str> = sc.lines().collect();
        let i = rows.iter().position(|r| r.contains("Mistral. i'll use")).expect("the detail");
        // the wrapped row starts where the detail starts (same column)
        let col = |r: &str, pat: &str| r.chars().collect::<String>().find(pat).map(|b| r[..b].chars().count());
        let start = col(rows[i], "Mistral").unwrap();
        let next: Vec<char> = rows[i + 1].chars().collect();
        assert!(next[start] != ' ' && next[start - 4..start].iter().all(|c| *c == ' '), "{}", sc);
        assert!(sc.contains("mistral-medium-latest."), "{}", sc);
    }

    #[test]
    fn step_3_without_a_key() {
        let h = tmp("s3b");
        let mut o = onb(&h, "/w");
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("i found no key in your environment.") && sc.contains("1 · set up a provider"), "{}", sc);
    }

    #[test]
    fn step_5_three_lines_one_by_one_then_done() {
        let h = tmp("s5");
        let mut o = onb(&h, "/w");
        o.go(Step::Lines, 100);
        let none = env_of(HashMap::new());
        let sc = screen(&o, 100 + 500, 110, 30);
        assert!(sc.contains("1  you talk to me: main") && !sc.contains("i start an agent"), "{}", sc);
        let sc = screen(&o, 100 + LINES_END, 110, 30);
        for s in [
            "how it works",
            "1  you talk to me: main, your team lead. any time, keep typing",
            "2  i start an agent when a job needs one. they sync on their own",
            "3  only the real decisions reach you, in your inbox · ctrl+g",
            "ctrl+o opens everything folded · ⌥0-9 talk to an agent",
            "any key ↵",
            "○ ○ ○ ●",
        ] {
            assert!(flat(&sc).contains(s), "{}\n{}", s, sc);
        }
        assert!(!sc.contains("how it works,") && !sc.contains("typing."), "no final periods: {}", sc);
        // bise (me, i) in accent, the numbers dim
        let l = how_lines(LINES_END, 2, 64);
        assert_eq!(l[3].spans[0].style.fg, Some(theme::dim()));
        assert_eq!(l[3].spans[2].content, "me");
        assert_eq!(l[3].spans[2].style.fg, Some(theme::accent()));
        assert_eq!(l[5].spans[1].content, "i");
        assert_eq!(l[5].spans[1].style.fg, Some(theme::accent()));
        // each line fits the 64-column column (100 and 120 columns): one row
        // each; title, gap, 3 lines and 2 blanks, blank, foot, gap, key
        assert_eq!(l.len(), 1 + 2 + 5 + 1 + 1 + 2 + 1, "{:?}", l);
        // narrow: a line wraps at the words with a 3-column hanging indent
        let sc = screen(&o, 100 + LINES_END, 40, 30);
        assert!(flat(&sc).contains("2  i start an agent when a job needs one. they sync on their own"), "{}", sc);
        let rows: Vec<&str> = sc.lines().collect();
        let r2 = rows.iter().position(|r| r.contains("2  i start")).unwrap();
        let x = rows[r2].find("2  ").unwrap();
        assert_eq!(rows[r2 + 1].find(|c: char| c != ' '), Some(x + 3), "hanging indent: {}", sc);
        assert_eq!(o.on_key(key(KeyCode::Char('q')), 1, &none), Out::Done, "any key");
    }

    #[test]
    fn esc_and_ctrl_c_skip() {
        let h = tmp("skip");
        let mut o = onb(&h, "/w");
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Esc), 1, &none), Out::Skip);
        o.go(Step::Theme, 0);
        assert_eq!(o.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), 1, &none), Out::Skip);
        // esc on the theme: the theme the launch had
        let start = theme::mode();
        o.go(Step::Theme, 0);
        o.on_key(key(KeyCode::Right), 1, &none);
        assert_ne!(theme::mode(), start);
        assert_eq!(o.on_key(key(KeyCode::Esc), 2, &none), Out::Skip);
        assert_eq!(theme::mode(), start);
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn a_found_key_skips_the_key_step_and_its_dot() {
        let h = tmp("found");
        // BISE-266: a key of another provider than the model's does not
        // make the first message work: the step shows
        let other = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
        ]));
        assert!(Onb::new(&other).ask_key);
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
            ("BISE_MODEL", "mistral/mistral-medium-latest".to_string()),
        ]));
        let mut o = Onb::new(&e);
        assert_eq!(o.steps(), vec![Step::Welcome, Step::Theme, Step::Lines]);
        o.go(Step::Theme, 0);
        assert!(screen(&o, 10, 100, 30).contains("○ ● ○"), "three dots");
        assert!(!screen(&o, 10, 100, 30).contains("○ ● ○ ○"));
        o.on_key(key(KeyCode::Enter), 5, &e);
        assert_eq!(o.step, Step::Lines);
        // saving a key on the key step does not take the step away
        let mut o = onb(&h, "/w");
        assert!(o.ask_key);
        o.found = vec![o.providers[0].clone()];
        assert!(o.steps().contains(&Step::Model));
    }

    #[test]
    fn narrow_screens_do_not_panic() {
        let h = tmp("narrow");
        let mut o = onb(&h, "/w");
        for step in [Step::Welcome, Step::Theme, Step::Model, Step::Lines] {
            o.go(step, 0);
            for (w, hh) in [(20, 5), (1, 1), (60, 12), (200, 60)] {
                screen(&o, 99_999, w, hh);
            }
        }
    }

    /// The paste step of Mistral drawn at 110x30: the frame and its links.
    fn paste_frame() -> (ratatui::buffer::Buffer, Vec<crate::links::Hit>) {
        let h = tmp("mouse");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let mistral = o.providers.iter().position(|p| p.id == "mistral").unwrap();
        o.sub = Sub::Which(mistral);
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Paste(..)));
        let mut t = Terminal::new(TestBackend::new(110, 30)).unwrap();
        t.draw(|f| draw(f, &o, 10)).unwrap();
        (t.backend().buffer().clone(), crate::links::frame_hits())
    }

    fn mouse(kind: crossterm::event::MouseEventKind, x: u16, y: u16) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }
    }

    /// Where `text` starts on the frame.
    fn cell_of(b: &ratatui::buffer::Buffer, text: &str) -> (u16, u16) {
        (0..b.area.height)
            .find_map(|y| row_text(b, y).find(text).map(|x| (row_text(b, y)[..x].width() as u16, y)))
            .unwrap_or_else(|| panic!("no {:?} on screen", text))
    }

    #[test]
    fn a_click_on_the_keys_page_opens_it() {
        use crossterm::event::{MouseButton::Left, MouseEventKind::*};
        let (b, hits) = paste_frame();
        let url = "https://console.mistral.ai/api-keys";
        let (x, y) = cell_of(&b, url);
        let mut m = Mouse::default();
        let t = Instant::now();
        assert_eq!(m.on(&mouse(Down(Left), x + 5, y), &b, &hits, t), None);
        assert_eq!(m.on(&mouse(Up(Left), x + 5, y), &b, &hits, t), Some(Act::Open(url.into())));
        // the words before it are no link
        let (gx, gy) = cell_of(&b, "get one:");
        let t = t + Duration::from_secs(1);
        m.on(&mouse(Down(Left), gx + 1, gy), &b, &hits, t);
        assert_eq!(m.on(&mouse(Up(Left), gx + 1, gy), &b, &hits, t), None);
        // the loop does it: the opener gets the url, the note says so
        m.act(Act::Open(url.into()), 100);
        assert_eq!(crate::links::OPENED.with(|o| o.borrow().last().cloned()), Some(url.to_string()));
        assert_eq!(m.note, Some((format!("opening {}", url), 100 + NOTE_MS)));
    }

    #[test]
    fn a_drag_or_a_double_click_copies_the_keys_page() {
        use crossterm::event::{MouseButton::Left, MouseEventKind::*};
        let (b, hits) = paste_frame();
        let url = "https://console.mistral.ai/api-keys";
        let (x, y) = cell_of(&b, url);
        let end = x + url.len() as u16 - 1;
        // a drag over the url, past its end: the url, highlighted, copied
        let mut m = Mouse::default();
        let t = Instant::now();
        m.on(&mouse(Down(Left), x, y), &b, &hits, t);
        m.on(&mouse(Drag(Left), end + 20, y), &b, &hits, t);
        let mut shown = b.clone();
        m.paint(&mut shown);
        assert_eq!(shown[(x, y)].bg, theme::selection_bg());
        assert_eq!(shown[(end, y)].bg, theme::selection_bg());
        assert_ne!(shown[(end + 1, y)].bg, theme::selection_bg());
        assert_eq!(m.on(&mouse(Up(Left), end + 20, y), &b, &hits, t), Some(Act::Copy(url.into())));
        m.act(Act::Copy(url.into()), 5);
        assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some(url));
        assert_eq!(m.note.as_ref().map(|n| n.0.as_str()), Some("copied 35 chars"));
        // a double click on it: the whole url, nothing opened
        let mut m = Mouse::default();
        let t = t + Duration::from_secs(1);
        m.on(&mouse(Down(Left), x + 9, y), &b, &hits, t);
        assert_eq!(m.on(&mouse(Up(Left), x + 9, y), &b, &hits, t), Some(Act::Open(url.into())));
        m.on(&mouse(Down(Left), x + 9, y), &b, &hits, t + Duration::from_millis(100));
        assert_eq!(m.on(&mouse(Up(Left), x + 9, y), &b, &hits, t), Some(Act::Copy(url.into())));
        // a drag over two rows: their text, not the margins
        let (tx, ty) = cell_of(&b, "paste your Mistral key");
        let mut m = Mouse::default();
        m.on(&mouse(Down(Left), tx, ty), &b, &hits, t);
        m.on(&mouse(Drag(Left), end, y), &b, &hits, t);
        let Some(Act::Copy(two)) = m.on(&mouse(Up(Left), end, y), &b, &hits, t) else { panic!("no copy") };
        assert_eq!(two, format!("paste your Mistral key\n\nget one: {}", url));
    }
}
