//! `/models` and the role pickers (BISE-298): which model does what.
//!
//! `/models` lists the roles of `bise_catalog::roles::ROLES` (main,
//! agents, small jobs, voice) with the model each one runs: a picked one
//! in text, a fallback dim with its rule word then the id it resolves to
//! (`same as main · …`, `auto · …`), a role whose provider lost its key
//! `✗ no Anthropic key · enter fixes it`. Enter opens the role's picker,
//! on `/provider`'s screen with its key step: the chat roles list the
//! models of the providers set up (agents and small jobs first their
//! fallback), a typed id, `+ another provider…`; main and agents then ask
//! `how hard should it think?` when the model takes efforts. The voice
//! picker is the voice screen (designer's approved design): the ready
//! providers' voice models, then the others, each checked by transcribing
//! half a second of silence. A pick is written at once (`set_role`).

use super::*;
use bise_catalog::roles::{self as r, Source};
use std::sync::Mutex;

/// Which of the panel's screens shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    /// `/provider`'s list and menus
    #[default]
    Providers,
    /// `/models`
    Roles,
    /// one role's picker, and where esc or a pick goes back to
    Pick(&'static str, Back),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Back {
    /// `/models`, the changed row flashing
    Roles,
    /// the feed (ctrl+r, `/voice`, `/voice setup`)
    Close,
    /// `/provider`'s menu of that provider (`use it for…`), boxed: the
    /// screen stays small
    Menu(Box<Provider>),
}

/// Where a `/provider` request opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Open {
    #[default]
    Providers,
    Roles,
    /// a role's picker, back to the feed
    Pick(&'static str),
}

/// What the voice picker did, for the feed (run.rs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VoiceOut {
    /// a voice model picked and checked: voice is on
    On(String),
    /// esc while voice was being turned on: it stays off
    Off,
}

static VOICE_OUT: Mutex<Option<VoiceOut>> = Mutex::new(None);

pub(crate) fn take_voice_out() -> Option<VoiceOut> {
    VOICE_OUT.lock().unwrap_or_else(|e| e.into_inner()).take()
}

fn set_voice_out(v: VoiceOut) {
    *VOICE_OUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
}

/// How long a changed row of `/models` flashes.
pub(crate) const FLASH: Duration = Duration::from_millis(1000);

/// One row of a picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PRow {
    /// unset: the role follows its fallback (the id it resolves to)
    Fallback(String),
    Model(String),
    /// a typed id; false: its provider has no key yet
    Typed(String, bool),
    /// `+ another provider…`
    Another,
    /// voice: a provider not set up
    Setup(Provider),
}

/// The roles `/models` lists.
pub(crate) fn shown() -> Vec<&'static r::Role> {
    r::ROLES.iter().filter(|x| x.shown).collect()
}

/// The role column: the longest name and 3 spaces.
fn role_w() -> usize {
    shown().iter().map(|x| x.name.width()).max().unwrap_or(0) + 3
}

fn pad(t: &str, w: usize) -> String {
    let n = t.width();
    if n >= w {
        format!("{} ", t)
    } else {
        format!("{}{}", t, " ".repeat(w - n))
    }
}

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

impl Onb {
    fn pn(&self) -> &provider::Panel {
        self.panel.as_ref().expect("panel")
    }

    fn pn_mut(&mut self) -> &mut provider::Panel {
        self.panel.as_mut().expect("panel")
    }

    /// The role a picker is open for.
    pub(crate) fn picking(&self) -> Option<&'static str> {
        match self.panel.as_ref().map(|p| &p.screen) {
            Some(Screen::Pick(id, _)) => Some(id),
            _ => None,
        }
    }

    /// The voice picker is open: the key flow checks voice models.
    pub(crate) fn voice_pick(&self) -> bool {
        self.picking() == Some(r::VOICE)
    }

    /// Voice mode is on (the `voice` preference).
    pub(crate) fn voice_enabled(&self) -> bool {
        self.home.pref(bise_home::Pref::Voice).get().and_then(|v| v.as_bool()).unwrap_or(false)
    }

    /// The voice providers the voice screen offers: they transcribe, are
    /// usable, and are not hidden (Groq, Deepgram: the user, 2026-09-30).
    pub(crate) fn voice_providers(&self) -> Vec<Provider> {
        self.setup.catalog.stt_providers().filter(|p| p.needs.is_empty() && !p.hidden).map(Provider::of).collect()
    }

    /// A provider's voice models, its pick first.
    pub(crate) fn voice_models(&self, p: &Provider) -> Vec<String> {
        let c = &self.setup.catalog;
        let mut v: Vec<String> = c.models.iter().filter(|m| m.provider == p.id && m.stt).map(|m| m.name()).collect();
        let pick = c.provider(&p.id).map(|x| x.voice_model.clone()).unwrap_or_default();
        if let Some(i) = v.iter().position(|m| short_model(m) == pick) {
            let m = v.remove(i);
            v.insert(0, m);
        }
        v
    }

    fn provider_of(&self, model: &str) -> Option<Provider> {
        let id = bise_catalog::split_name(model)?.0;
        self.setup.catalog.provider(id).map(Provider::of)
    }

    /// The model a role runs and where it comes from.
    pub(crate) fn role_model(&self, id: &str) -> (String, Source) {
        self.setup.role_model(id)
    }

    /// The provider of a role's model has no key: its name.
    pub(crate) fn role_broken(&self, id: &str) -> Option<Provider> {
        let (m, src) = self.role_model(id);
        // a fallback is fixed where it comes from (main's row)
        if m.is_empty() || !matches!(src, Source::Picked | Source::Env(_)) {
            return None;
        }
        // voice left to the default and no key for it: off, not broken
        if id == r::VOICE && src == Source::Auto {
            return None;
        }
        self.provider_of(&m).filter(|p| !self.ready(p))
    }

    /// The roles that run on `p` (their names, `/provider`'s tags and the
    /// remove confirm). Voice only while it is on.
    pub(crate) fn roles_of(&self, p: &str) -> Vec<&'static r::Role> {
        shown()
            .into_iter()
            .filter(|x| x.id != r::VOICE || self.voice_enabled())
            .filter(|x| {
                let (m, src) = self.role_model(x.id);
                src != Source::None && bise_catalog::split_name(&m).is_some_and(|(pid, _)| pid == p)
            })
            .collect()
    }

    /// The chat models of the providers set up, in catalog order.
    fn ready_chat_models(&self) -> Vec<String> {
        let c = &self.setup.catalog;
        c.models
            .iter()
            .filter(|m| !m.stt)
            .filter(|m| c.provider(&m.provider).is_some_and(|p| !p.stt_only && p.needs.is_empty() && self.ready(&Provider::of(p))))
            .map(|m| m.name())
            .collect()
    }

    /// The rows of a role's picker for the text typed.
    pub(crate) fn pick_rows(&self, id: &str) -> Vec<PRow> {
        let q = self.pn().filter.trim().to_lowercase();
        let hit = |m: &str| q.is_empty() || m.to_lowercase().contains(&q);
        let mut v = Vec::new();
        let (current, src) = self.role_model(id);
        let listed: Vec<String>;
        if id == r::VOICE {
            let mut ready = Vec::new();
            let mut others = Vec::new();
            for p in self.voice_providers() {
                if self.ready(&p) {
                    ready.extend(self.voice_models(&p).into_iter().filter(|m| hit(m) || p.name.to_lowercase().contains(&q)));
                } else if q.is_empty() || p.name.to_lowercase().contains(&q) || p.id.contains(&q) {
                    others.push(p);
                }
            }
            // a voice model picked on a hidden provider stays in view
            let picked = src == Source::Picked && self.provider_of(&current).is_some_and(|p| self.ready(&p));
            if picked && !ready.contains(&current) && hit(&current) {
                ready.insert(0, current.clone());
            }
            listed = ready.clone();
            v.extend(ready.into_iter().map(PRow::Model));
            v.extend(others.into_iter().map(PRow::Setup));
        } else {
            if q.is_empty() && id != r::MAIN {
                let fallback = match id {
                    r::AGENTS => self.setup.model.clone(),
                    _ => self.setup.catalog.small_of(&self.setup.agent_model).unwrap_or_else(|| self.setup.agent_model.clone()),
                };
                v.push(PRow::Fallback(fallback));
            }
            listed = self.ready_chat_models().into_iter().filter(|m| hit(m)).collect();
            v.extend(listed.iter().cloned().map(PRow::Model));
        }
        let mine = bise_catalog::split_name(&current).map(|(p, _)| p.to_string()).unwrap_or_default();
        if let Some(t) = crate::models::free_id(&self.pn().filter, &mine).filter(|t| !listed.contains(t)) {
            let known = self.provider_of(&t).filter(|p| self.setup.catalog.provider(&p.id).is_some_and(|c| c.needs.is_empty()));
            let transcribes = |p: &Provider| self.setup.catalog.provider(&p.id).is_some_and(|c| !c.stt.is_empty());
            if let Some(p) = known.filter(|p| id != r::VOICE || transcribes(p)) {
                v.push(PRow::Typed(t, self.ready(&p)));
            }
        }
        if id != r::VOICE && (q.is_empty() || "another provider".contains(&q)) {
            v.push(PRow::Another);
        }
        v
    }

    /// The row a picker opens on: the role's model when picked, else its
    /// fallback (agents, small jobs), else the recommended voice model,
    /// else the first.
    pub(crate) fn preselect(&self, id: &str) -> usize {
        let rows = self.pick_rows(id);
        let (current, src) = self.role_model(id);
        if src == Source::Picked || src == Source::SameAs(r::MAIN) && id == r::MAIN {
            if let Some(i) = rows.iter().position(|x| *x == PRow::Model(current.clone())) {
                return i;
            }
        }
        if id == r::VOICE {
            if let Some(m) = self.recommended_voice() {
                if let Some(i) = rows.iter().position(|x| *x == PRow::Model(m.clone())) {
                    return i;
                }
            }
            // none ready: Mistral (the default's provider)
            let d = self.setup.catalog.default_voice_model.clone();
            let dp = bise_catalog::split_name(&d).map(|(p, _)| p.to_string()).unwrap_or_default();
            return rows.iter().position(|x| matches!(x, PRow::Setup(p) if p.id == dp)).unwrap_or(0);
        }
        if id == r::MAIN {
            return rows.iter().position(|x| *x == PRow::Model(current.clone())).unwrap_or(0);
        }
        0
    }

    /// The voice model the screen recommends: the default one when its
    /// provider is set up, else the first ready provider's pick.
    pub(crate) fn recommended_voice(&self) -> Option<String> {
        let d = self.setup.catalog.default_voice_model.clone();
        if self.provider_of(&d).is_some_and(|p| self.ready(&p)) {
            return Some(d);
        }
        self.voice_providers().into_iter().find(|p| self.ready(p)).and_then(|p| self.voice_models(&p).into_iter().next())
    }

    /// Open a role's picker.
    pub(crate) fn open_pick(&mut self, id: &'static str, back: Back) -> Sub {
        let pn = self.pn_mut();
        pn.screen = Screen::Pick(id, back);
        pn.filter.clear();
        pn.save_model = false;
        self.note = None;
        self.sel = self.preselect(id);
        Sub::List
    }

    /// Write a role's choice in config.toml and read the setup again.
    fn save_role(&mut self, id: &str, model: Option<&str>, effort: Option<&str>, env: Env) -> Result<(), String> {
        let file = self.home.config_file();
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, r::set_role(&text, id, model, effort)).map_err(|e| format!("couldn't write config.toml: {}", e))?;
        self.setup = setup_of(env, &self.home);
        self.model = self.setup.model.clone();
        self.mine = self.setup.catalog.resolve(&self.model).provider;
        Ok(())
    }

    /// A pick is done: saved, then back where the picker came from.
    fn picked(&mut self, id: &'static str, model: Option<&str>, effort: Option<&str>, env: Env) -> Sub {
        if let Err(e) = self.save_role(id, model, effort, env) {
            self.pn_mut().said = Some(format!("{} {}", theme::glyph(theme::G_FAILED), e));
            return Sub::List;
        }
        if id == r::VOICE {
            if let Some(m) = model {
                set_voice_out(VoiceOut::On(m.to_string()));
            }
        }
        self.back(Some(id))
    }

    /// Leave the picker: to `/models` (the row flashing when it changed),
    /// the provider's menu, or the feed.
    fn back(&mut self, changed: Option<&'static str>) -> Sub {
        let Screen::Pick(id, back) = self.pn().screen.clone() else { return Sub::List };
        self.pn_mut().filter.clear();
        match back {
            Back::Roles => {
                let pn = self.pn_mut();
                pn.screen = Screen::Roles;
                pn.flash = changed.map(|c| (c, Instant::now()));
                self.sel = shown().iter().position(|x| x.id == id).unwrap_or(0);
                Sub::List
            }
            Back::Menu(p) => {
                self.pn_mut().screen = Screen::Providers;
                let i = self.items(&p).iter().position(|x| *x == provider::Item::UseFor).unwrap_or(0);
                Sub::Menu(*p, i)
            }
            Back::Close => {
                if changed.is_none() && id == r::VOICE && self.pn().voice_on {
                    set_voice_out(VoiceOut::Off);
                }
                self.pn_mut().closed = true;
                Sub::List
            }
        }
    }

    /// The key flow of a picker ended with a model that works (a new key
    /// checked, a voice model checked): on to its effort, or saved.
    pub(crate) fn after_check(&mut self, model: String, env: Env) -> Sub {
        let Some(id) = self.picking() else { return Sub::List };
        self.chosen(id, model, env)
    }

    /// A chat model chosen for `id`: `how hard should it think?` when
    /// the role has an effort and the model takes some, else saved.
    fn chosen(&mut self, id: &'static str, model: String, env: Env) -> Sub {
        let (efforts, default) = self.efforts_of(&model);
        if (id == r::MAIN || id == r::AGENTS) && !efforts.is_empty() {
            let asked = self.setup.role_effort(id).to_string();
            let now = if self.role_model(id).0 == model && efforts.contains(&asked) { asked } else { default };
            let i = efforts.iter().position(|e| *e == now).unwrap_or(0);
            return Sub::Effort(model, i);
        }
        self.picked(id, Some(&model), None, env)
    }

    /// The efforts a model takes and its default.
    pub(crate) fn efforts_of(&self, model: &str) -> (Vec<String>, String) {
        let m = self.setup.catalog.resolve(model);
        (m.efforts(), m.default_effort())
    }

    /// A key on `/models` or a picker.
    pub(super) fn on_roles_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let plain = !k.modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER,
        );
        self.pn_mut().said = None;
        let updown = |i: usize, n: usize| {
            let n = n.max(1);
            if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n }
        };
        let screen = self.pn().screen.clone();
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        self.sub = match (&screen, sub, k.code) {
            (Screen::Roles, Sub::List, KeyCode::Esc) => return Out::Done,
            (Screen::Roles, Sub::List, KeyCode::Up | KeyCode::Down) => {
                self.pn_mut().flash = None;
                self.sel = updown(self.sel, shown().len());
                Sub::List
            }
            (Screen::Roles, Sub::List, KeyCode::Enter) => {
                let Some(role) = shown().get(self.sel).copied() else { return Out::Stay };
                self.pn_mut().flash = None;
                let sub = self.open_pick(role.id, Back::Roles);
                // a role whose provider lost its key: straight to its key step
                match self.role_broken(role.id) {
                    Some(p) => {
                        let m = self.role_model(role.id).0;
                        Sub::Paste(p, m, String::new())
                    }
                    None => sub,
                }
            }
            (Screen::Roles, s, _) => s,
            // ---- a picker ----
            (Screen::Pick(..), Sub::List, KeyCode::Esc) => {
                if self.pn().filter.is_empty() {
                    self.back(None)
                } else {
                    self.pn_mut().filter.clear();
                    self.sel = 0;
                    Sub::List
                }
            }
            (Screen::Pick(id, _), Sub::List, KeyCode::Up | KeyCode::Down) => {
                self.sel = updown(self.sel, self.pick_rows(id).len());
                Sub::List
            }
            (Screen::Pick(id, _), Sub::List, KeyCode::Backspace) => {
                let _ = id;
                self.pn_mut().filter.pop();
                self.sel = 0;
                Sub::List
            }
            (Screen::Pick(..), Sub::List, KeyCode::Char(c)) if plain && !c.is_whitespace() => {
                self.pn_mut().filter.push(c);
                self.sel = 0;
                Sub::List
            }
            (Screen::Pick(id, _), Sub::List, KeyCode::Enter) => {
                let id: &'static str = id;
                match self.pick_rows(id).get(self.sel).cloned() {
                    None => Sub::List,
                    Some(PRow::Fallback(_)) => self.picked(id, None, None, env),
                    Some(PRow::Another) => {
                        self.note = None;
                        Sub::Which(self.providers.iter().position(|p| p.id == self.mine).unwrap_or(0))
                    }
                    Some(PRow::Setup(p)) => {
                        let models = self.voice_models(&p);
                        match models.len() {
                            0 => Sub::List,
                            1 => Sub::Paste(p, models[0].clone(), String::new()),
                            _ => Sub::Model(p, 0, String::new()),
                        }
                    }
                    Some(PRow::Model(m) | PRow::Typed(m, _)) => {
                        let Some(p) = self.provider_of(&m) else { return Out::Stay };
                        if !self.ready(&p) {
                            self.paste_for(p, Some(m))
                        } else if id == r::VOICE {
                            // the voice model is checked with the key found
                            self.start_check(p, m, None, env)
                        } else {
                            self.chosen(id, m, env)
                        }
                    }
                }
            }
            (Screen::Pick(..), Sub::Effort(m, i), KeyCode::Up | KeyCode::Down) => {
                let n = self.efforts_of(&m).0.len();
                Sub::Effort(m, updown(i, n))
            }
            (Screen::Pick(id, _), Sub::Effort(m, i), KeyCode::Enter) => {
                let id: &'static str = id;
                let (efforts, default) = self.efforts_of(&m);
                let e = efforts.get(i).cloned().unwrap_or_default();
                // the model's default is not written: it follows the model
                let e = (e != default && !e.is_empty()).then_some(e);
                self.picked(id, Some(&m), e.as_deref(), env)
            }
            (Screen::Pick(id, _), Sub::Effort(..), KeyCode::Esc) => {
                let id: &'static str = id;
                self.sel = self.preselect(id);
                Sub::List
            }
            (Screen::Pick(..), s @ Sub::Effort(..), _) => s,
            // the key flow: the first run's, its way out to the picker
            (Screen::Pick(id, _), sub, _) => {
                let id: &'static str = id;
                self.sub = sub;
                self.on_model_sub(k, now, env);
                match std::mem::replace(&mut self.sub, Sub::List) {
                    // it worked (a found key): on
                    Sub::Works(_, m) => self.after_check(m, env),
                    // esc or tab: back to the picker's list
                    Sub::List | Sub::Which(_) if id == r::VOICE => {
                        self.sel = self.preselect(id);
                        Sub::List
                    }
                    Sub::List => {
                        self.sel = self.preselect(id);
                        Sub::List
                    }
                    s => s,
                }
            }
            (Screen::Providers, s, _) => s,
        };
        if self.pn().closed {
            return Out::Done;
        }
        Out::Stay
    }
}

// ---- drawing ----

/// The lines of `/models` and of a picker's list and effort step; None:
/// a step of the key flow (drawn as on the first run).
pub(super) fn lines(o: &Onb, w: u16, gap: usize) -> Option<Vec<Line<'static>>> {
    let pn = o.panel.as_ref()?;
    let said = |v: &mut Vec<Line<'static>>| {
        if let Some(t) = &pn.said {
            v.push(Line::raw(""));
            v.push(Line::from(s(t.clone(), theme::error())));
        }
    };
    match (&pn.screen, &o.sub) {
        (Screen::Roles, Sub::List) => {
            let mut v = vec![title("which model does what?"), Line::from(s("main uses one model, the rest follow it unless you pick.", theme::dim()))];
            blanks(&mut v, gap);
            for (k, role) in shown().into_iter().enumerate() {
                let flash = pn.flash.as_ref().is_some_and(|(id, at)| *id == role.id && at.elapsed() < FLASH);
                v.push(role_row(o, role, k == o.sel, flash, w as usize));
                if role.id == r::AGENTS && !pn.overrides.is_empty() {
                    v.push(Line::from(s(format!("{}{}", " ".repeat(2 + role_w()), overrides_words(&pn.overrides)), theme::dim())));
                }
            }
            // what the role under the cursor is for (designer)
            if let Some(role) = shown().get(o.sel) {
                v.push(Line::raw(""));
                v.push(Line::from(s(format!("  {}: {}", role.name, role_hint(role.id)), theme::dim())));
            }
            said(&mut v);
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} change · {esc} back"));
            Some(v)
        }
        (Screen::Pick(id, _), Sub::List) => Some(pick_lines(o, id, w, gap, &said)),
        (Screen::Pick(id, _), Sub::Effort(m, i)) => {
            let role = r::role(id).expect("role");
            let mut v = vec![title("how hard should it think?"), Line::from(s(format!("{} for {}", m, role.name), theme::dim()))];
            blanks(&mut v, gap);
            let (efforts, default) = o.efforts_of(m);
            for (k, e) in efforts.iter().enumerate() {
                let mut name = vec![s(pad(e, 10), theme::text()), s(crate::commands::effort_hint(e), theme::dim())];
                if *e == default {
                    name.push(s(format!(" {} default", dot()), theme::dim()));
                }
                option(&mut v, k == *i, name, "", w);
            }
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
            Some(v)
        }
        _ => None,
    }
}

/// `1 agent uses its own model: perf · openai/gpt-6-sol`.
fn overrides_words(o: &[(String, String)]) -> String {
    match o {
        [(a, m)] => format!("1 agent uses its own model: {} {} {}", a, dot(), m),
        _ => format!(
            "{} agents use their own model: {}",
            o.len(),
            o.iter().map(|(a, m)| format!("{} {} {}", a, dot(), m)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// One row of `/models`: the role, what runs, what it does. On a narrow
/// column the description goes first, then the effort; never the id.
fn role_row(o: &Onb, role: &r::Role, selected: bool, flash: bool, w: usize) -> Line<'static> {
    let (m, src) = o.role_model(role.id);
    let d = dot();
    // (rule word, resolved id) of a fallback, or the picked id; its effort
    let mut pieces: Vec<Span<'static>> = Vec::new();
    let mut rule = String::new();
    let mut resolved = String::new();
    let mut effort = String::new();
    let broken = o.role_broken(role.id);
    let voice_off = role.id == r::VOICE && !o.voice_enabled();
    let voice_none = role.id == r::VOICE && src == Source::Auto && !o.provider_of(&m).is_some_and(|p| o.ready(&p));
    if broken.is_some() {
        // the thing that runs stays on screen (designer)
        pieces.push(s(m.clone(), theme::text()));
        pieces.push(s(format!("  {} ", theme::glyph(theme::G_FAILED)), theme::error()));
        pieces.push(s(format!("no key {} enter fixes it", d), theme::dim()));
    } else {
        match src {
            _ if voice_none || (voice_off && src != Source::Picked) => pieces.push(s(format!("off {} enter sets it up", d), theme::dim())),
            Source::None => pieces.push(s(format!("none yet {} enter picks one", d), theme::dim())),
            Source::Picked | Source::Env(_) => pieces.push(s(m.clone(), theme::text())),
            Source::SameAs(of) => {
                rule = format!("same as {}", r::role(of).map_or(of, |x| x.name));
                resolved = m.clone();
            }
            Source::Auto => {
                rule = "auto".into();
                resolved = m.clone();
            }
        }
        if let Source::Env(n) = src {
            effort = format!(" {} from {}", d, n);
        } else if (role.id == r::MAIN || role.id == r::AGENTS) && matches!(src, Source::Picked | Source::SameAs(_)) {
            let e = o.setup.role_effort(if src == Source::Picked { role.id } else { r::MAIN });
            let e = o.setup.catalog.resolve(&m).effort_for(e);
            if !e.is_empty() {
                effort = format!(" {} {}", d, e);
            }
        }
        if role.id == r::VOICE && voice_off && src == Source::Picked {
            effort = format!(" {} off", d);
        }
    }
    let lead: Vec<Span<'static>> = if flash {
        vec![s(format!("{} ", theme::glyph(theme::G_DONE)), theme::accent())]
    } else if selected {
        vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())]
    } else {
        vec![Span::raw("  ")]
    };
    let name = s(pad(role.name, role_w()), theme::text());
    let mut row = lead;
    row.push(if selected { name.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { name });
    // a row never wraps: the resolved id loses its provider, then goes,
    // then the effort; a picked id always stays (designer)
    let room = w.saturating_sub(2 + role_w());
    let fixed: usize = pieces.iter().map(|x| x.content.width()).sum();
    row.extend(pieces);
    if !rule.is_empty() {
        let short = short_model(&resolved);
        let fits = |v: &str, e: &str| rule.width() + v.width() + e.width() <= room;
        let (v, e) = if fits(&format!(" {} {}", d, resolved), &effort) {
            (format!(" {} {}", d, resolved), effort.clone())
        } else if fits(&format!(" {} {}", d, short), &effort) {
            (format!(" {} {}", d, short), effort.clone())
        } else if fits("", &effort) {
            (String::new(), effort.clone())
        } else {
            (String::new(), String::new())
        };
        row.push(s(format!("{}{}{}", rule, v, e), theme::dim()));
    } else if fixed + effort.width() <= room {
        row.push(s(effort, theme::dim()));
    }
    Line::from(row)
}

/// What a role is for, under `/models`' list for the row under the
/// cursor (the picker's dim line, designer).
fn role_hint(id: &str) -> &'static str {
    match id {
        r::MAIN => "talks with you and starts the agents.",
        r::AGENTS => "the ones main starts, for the work it hands out.",
        r::SMALL => "titles and summaries. a small fast model is enough.",
        _ => "you talk, it types in the composer.",
    }
}

/// The title and the dim line of each role's picker.
fn pick_words(id: &str) -> (&'static str, &'static str) {
    match id {
        r::MAIN => ("which model is your team lead?", "main talks with you and starts the agents."),
        r::AGENTS => ("which model do the agents use?", "the agents main starts, for the work it hands out."),
        r::SMALL => ("which model writes titles and summaries?", "a small fast model is enough."),
        _ => ("which model should listen to you?", "you talk, it types in the composer. ctrl+r starts, any key stops."),
    }
}

fn pick_lines(o: &Onb, id: &'static str, w: u16, gap: usize, said: &dyn Fn(&mut Vec<Line<'static>>)) -> Vec<Line<'static>> {
    let pn = o.panel.as_ref().expect("panel");
    let (t, sub) = pick_words(id);
    let mut v = vec![title(t), Line::from(s(sub, theme::dim()))];
    blanks(&mut v, gap);
    let mut line = vec![s("› ", theme::accent())];
    if pn.filter.is_empty() {
        line.push(s("type to filter", theme::dim()));
    } else {
        line.push(s(format!("{}▏", pn.filter), theme::text()));
    }
    v.push(Line::from(line));
    blanks(&mut v, 1);
    let rows = o.pick_rows(id);
    let (current, src) = o.role_model(id);
    let d = dot();
    let rec_voice = if id == r::VOICE { o.recommended_voice() } else { None };
    let small_pick = if id == r::SMALL { o.setup.catalog.small_of(&o.setup.agent_model) } else { None };
    let mut group = "";
    let mut last_provider = String::new();
    // no listed row matches what is typed (designer: above the typed row)
    if !pn.filter.is_empty() && !rows.iter().any(|x| matches!(x, PRow::Model(_) | PRow::Setup(_))) && rows.iter().any(|x| matches!(x, PRow::Typed(..))) {
        v.push(Line::from(s("  no listed model matches.", theme::dim())));
    }
    const LIST_ROWS: usize = 12;
    let from = o.sel.saturating_sub(LIST_ROWS / 2).min(rows.len().saturating_sub(LIST_ROWS));
    if from > 0 {
        v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
    }
    for (k, row) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
        // the voice screen's two groups
        if id == r::VOICE {
            let g = match row {
                PRow::Model(_) => "ready",
                PRow::Setup(_) => "another provider",
                _ => "",
            };
            if !g.is_empty() && g != group {
                v.push(Line::from(s(g, theme::dim())));
                group = g;
            }
            // the ready models under their provider: `OpenAI · your key`
            if let PRow::Model(m) = row {
                let pid = bise_catalog::split_name(m).map(|(p, _)| p.to_string()).unwrap_or_default();
                if pid != last_provider {
                    let name = o.provider_of(m).map(|p| p.name).unwrap_or_else(|| pid.clone());
                    v.push(Line::from(s(format!("  {} {} your key", name, d), theme::dim())));
                    last_provider = pid;
                }
            }
        }
        let sel = k == o.sel;
        let mut name: Vec<Span<'static>> = Vec::new();
        match row {
            PRow::Fallback(m) => {
                let word = if id == r::AGENTS { "same as main" } else { "auto" };
                name.push(s(format!("{} {} ", word, d), theme::text()));
                name.push(s(m.clone(), theme::dim()));
                if src != Source::Picked {
                    name.push(s(format!("   {} now", theme::glyph(theme::G_DONE)), theme::accent()));
                }
            }
            PRow::Model(m) => {
                if id == r::VOICE {
                    // indented under its provider's line
                    name.push(s(format!("  {}", short_model(m)), theme::text()));
                    if rec_voice.as_deref() == Some(m.as_str()) {
                        name.push(s("   recommended", theme::accent()));
                    }
                } else {
                    name.push(s(m.clone(), theme::text()));
                    if small_pick.as_deref() == Some(m.as_str()) {
                        name.push(s("   recommended", theme::accent()));
                    }
                }
                if *m == current && src == Source::Picked {
                    name.push(s(format!("   {} now", theme::glyph(theme::G_DONE)), theme::accent()));
                }
            }
            PRow::Setup(p) => {
                let nw = o.voice_providers().iter().map(|x| x.name.width()).max().unwrap_or(0) + 2;
                name.push(s(pad(&p.name, nw), theme::text()));
                let pick = o.voice_models(p).first().map(|m| short_model(m)).unwrap_or_default();
                let also = o.setup.catalog.provider(&p.id).is_some_and(|c| c.stt_only);
                let note = if also { "voice only" } else { "the key also works for chat" };
                name.push(s(format!("{} {} {}", pick, d, note), theme::dim()));
            }
            PRow::Typed(t, true) => {
                name.push(s(format!("+ use {}", t), theme::text()));
                // the agreed tail (BISE-289), when the row has room
                let tail = "   not in my list: i'll try it with one tiny call";
                if 2 + format!("+ use {}", t).width() + tail.width() <= w as usize {
                    name.push(s(tail, theme::dim()));
                }
            }
            PRow::Typed(t, false) => {
                let pname = o.provider_of(t).map(|p| p.name).unwrap_or_default();
                name.push(s(format!("+ set up {} for {}", pname, t), theme::text()));
                name.push(s("   no key yet", theme::dim()));
            }
            PRow::Another => {
                name.push(s("+ another provider…", theme::text()));
            }
        }
        option(&mut v, sel, name, "", w);
    }
    if from + LIST_ROWS < rows.len() {
        v.push(Line::from(s(format!("  ↓ {} more", rows.len() - from - LIST_ROWS), theme::dim())));
    }
    if rows.is_empty() {
        v.push(Line::from(s("  no model matches.", theme::dim())));
    }
    said(&mut v);
    blanks(&mut v, gap);
    let esc = match &pn.screen {
        Screen::Pick(_, Back::Close) if id == r::VOICE => "not now",
        _ => "back",
    };
    v.push(keyline(&format!("{{↑↓}} choose · {{enter}} ok · {{esc}} {}", esc)));
    v
}
