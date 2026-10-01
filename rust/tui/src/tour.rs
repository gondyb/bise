//! The demo's guided tips (BISE-286): while bise's built-in demo runs
//! (prompts/skills/bise-demo), a tip at each step says what to press, so
//! you do things instead of reading a recap.
//!
//! The signal: the demo's agents have an objective that starts with
//! `bise demo` ([`MARK`]). A tour starts when such an agent appears in a
//! state after the first one (a TUI started or restarted during a demo
//! shows none) and ends when you used its last tip, or no demo agent is
//! left. One tip at a time, in the style of the one-time hints (hints.rs),
//! computed from what the TUI already sees:
//!
//! 1. [`Tip::Start`] the agents appear: left of the panel, at the first
//!    one. Goes when you look inside one.
//! 2. [`Tip::Inside`] inside a demo agent: at the top of its feed. Goes
//!    when you are back in main.
//! 3. [`Tip::Card`] a card is in the inbox: above it. Goes when answered.
//! 4. [`Tip::Steer`] once the card is answered, the last agent waits for
//!    a word from you: left of the panel at its row. Goes when you leave
//!    its feed after writing to it.
//! 5. [`Tip::End`] one demo agent is archived (main dropped it): left of
//!    the panel (a card still open first). Goes on ctrl+s or when you
//!    open another demo agent.
//!
//! Any tip goes when you send a message (its step counts as done). No
//! timeout: each tip waits for its action.

use crate::hints;
use crate::App;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::cell::RefCell;

/// How a demo agent's objective starts (the bise-demo skill's spawns).
pub(crate) const MARK: &str = "bise demo";

/// `objective` is a demo agent's.
pub(crate) fn is_demo(objective: &str) -> bool {
    objective.trim_start().to_lowercase().starts_with(MARK)
}

/// A demo agent, as the TUI sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) name: String,
    /// its ⌥ number in the panel (live ones)
    pub(crate) number: Option<usize>,
    pub(crate) archived: bool,
}

/// What the tour looks at, read from the app (sb.rs `tour_snap`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Snap {
    /// the demo agents, in the hub's (creation) order
    pub(crate) members: Vec<Member>,
    /// the feed in view
    pub(crate) focus: String,
    /// a card of the hub waits in the inbox, and who it is about
    pub(crate) card: Option<String>,
    /// the agent palette (ctrl+s) is open
    pub(crate) palette: bool,
    /// the terminal sends no ctrl+1-9 (BISE-302, reach.rs): the card tip
    /// says click
    pub(crate) no_ctrl_digits: bool,
}

impl Snap {
    fn live(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| !m.archived)
    }

    /// The feed in view is a live demo agent's.
    fn inside(&self) -> Option<&Member> {
        self.live().find(|m| m.name == self.focus)
    }

    /// Who waits for a word from you: the last demo agent spawned.
    fn target(&self) -> Option<&Member> {
        self.live().last()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tip {
    Start,
    Inside,
    Card,
    Steer,
    End,
}

/// The tour's progress.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tour {
    /// the demo agents seen so far (None before the first state)
    known: Option<Vec<String>>,
    /// this run's agents: the ones that appeared since it started
    run: Vec<String>,
    on: bool,
    /// you looked inside a demo agent
    opened: bool,
    /// then came back to main
    back: bool,
    /// a card came / is gone (or you moved on)
    card_seen: bool,
    card_done: bool,
    /// you wrote to the target / then left its feed
    steered: bool,
    steer_done: bool,
    /// a demo agent was archived: the end; the feed in view then
    ended: Option<String>,
}

impl Tour {
    /// Running.
    pub(crate) fn on(&self) -> bool {
        self.on
    }

    /// `s` with this run's agents only (an earlier demo's left idle
    /// or archived are not part of it).
    pub(crate) fn scoped(&self, s: &Snap) -> Snap {
        let members = s.members.iter().filter(|m| self.run.contains(&m.name)).cloned().collect();
        Snap { members, ..s.clone() }
    }

    /// A new state or frame: start, move on, or end the tour.
    pub(crate) fn observe(&mut self, all: &Snap) {
        let names: Vec<String> = all.members.iter().map(|m| m.name.clone()).collect();
        let Some(known) = self.known.as_mut() else {
            self.known = Some(names);
            return;
        };
        let fresh: Vec<String> = all.live().filter(|m| !known.contains(&m.name)).map(|m| m.name.clone()).collect();
        for n in names {
            if !known.contains(&n) {
                known.push(n);
            }
        }
        if !fresh.is_empty() && !self.on {
            let known = self.known.take();
            *self = Tour { known, on: true, ..Tour::default() };
        }
        if !self.on {
            return;
        }
        self.run.extend(fresh);
        let s = &self.scoped(all);
        if s.live().next().is_none() {
            self.on = false;
            return;
        }
        let inside = s.inside().map(|m| m.name.clone());
        if inside.is_some() {
            self.opened = true;
        } else if self.opened {
            self.back = true;
        }
        if s.card.is_some() {
            self.card_seen = true;
        } else if self.card_seen {
            self.card_done = true;
        }
        if self.steered && s.target().is_none_or(|t| t.name != s.focus) {
            self.steer_done = true;
        }
        if self.ended.is_none() && s.members.iter().any(|m| m.archived) {
            self.ended = Some(s.focus.clone());
        }
        if let Some(at) = &self.ended {
            if s.palette || inside.as_ref().is_some_and(|n| n != at) {
                self.on = false;
            }
        }
    }

    /// The tip to show now.
    pub(crate) fn tip(&self, all: &Snap) -> Option<Tip> {
        if !self.on {
            return None;
        }
        let s = &self.scoped(all);
        // a card still open comes first, even at the end (a quick main
        // can drop pm before you answered)
        let card = s.card.is_some() && !self.card_done;
        if self.ended.is_some() {
            let inside = s.inside().is_some();
            return Some(if card && !inside { Tip::Card } else { Tip::End });
        }
        // after the card (designer's question comes first)
        let steer = self.card_done && !self.steer_done && s.target().is_some();
        if let Some(m) = s.inside() {
            if !self.back {
                return Some(Tip::Inside);
            }
            let target = s.target().is_some_and(|t| t.name == m.name);
            return (target && steer).then_some(Tip::Steer);
        }
        if card {
            return Some(Tip::Card);
        }
        if !self.opened {
            return Some(Tip::Start);
        }
        (self.back && steer).then_some(Tip::Steer)
    }

    /// You sent a message: the tip up goes (its step counts as done);
    /// writing in the target's feed is the steer.
    pub(crate) fn sent(&mut self, all: &Snap) {
        let s = &self.scoped(all);
        match self.tip(all) {
            Some(Tip::Start) => {
                self.opened = true;
                self.back = true;
            }
            Some(Tip::Inside) => self.back = true,
            Some(Tip::Card) => self.card_done = true,
            Some(Tip::Steer) if s.target().is_some_and(|t| t.name == s.focus) => self.steered = true,
            Some(Tip::Steer) => self.steer_done = true,
            Some(Tip::End) => self.on = false,
            None => {}
        }
        if s.target().is_some_and(|t| t.name == s.focus) && self.back {
            self.steered = true;
        }
    }
}

/// `⌥N` of a member, `{…}`-marked (a key in the tip).
fn key(m: &Member) -> Option<String> {
    m.number.map(|n| format!("{{⌥{n}}}"))
}

/// The others' keys, `{⌥2} {⌥3}`.
fn keys<'a>(ms: impl Iterator<Item = &'a Member>) -> String {
    ms.filter_map(key).collect::<Vec<_>>().join(" ")
}

/// The words of tip `t` (hints.rs marks: `{key}`, `[glyph]`, `<bold>`).
pub(crate) fn text(t: Tip, s: &Snap) -> String {
    match t {
        Tip::Start => match s.live().next() {
            Some(m) => format!(
                "your team just started. they work on their own. {} looks inside {}. →",
                key(m).unwrap_or_else(|| "{ctrl+s}".into()),
                m.name
            ),
            None => String::new(),
        },
        Tip::Inside => {
            let others = keys(s.live().filter(|m| m.name != s.focus));
            let others = if others.is_empty() { String::new() } else { format!("{others} the others · ") };
            format!("this is {}'s own thread, live. {others}{{esc}} back to main.", s.focus)
        }
        Tip::Card => {
            let who = s.card.as_deref().filter(|w| s.live().any(|m| m.name == *w)).unwrap_or("your team");
            if s.no_ctrl_digits {
                format!("{who} needs you. {{click}} the question, {{1}} or {{2}} answers. ↓")
            } else {
                format!("{who} needs you. {{ctrl+1}} opens the question, {{1}} or {{2}} answers. ↓")
            }
        }
        Tip::Steer => {
            let Some(m) = s.target() else { return String::new() };
            let how = if s.focus == m.name { String::new() } else { key(m).map(|k| format!("{k}, ")).unwrap_or_default() };
            format!("{} waits for a word from you: {how}type, {{⏎}}. [✓] it got it, [✓✓] it read it.", m.name)
        }
        Tip::End => {
            let gone = s.members.iter().find(|m| m.archived).map(|m| m.name.as_str()).unwrap_or("it");
            let live: Vec<&Member> = s.live().collect();
            let rest = match live.len() {
                0 => String::new(),
                1 => format!(" {} opens {}.", keys(live.into_iter()), s.live().next().map_or("", |m| m.name.as_str())),
                2 => format!(" {} open the other two.", keys(live.into_iter())),
                _ => format!(" {} open the others.", keys(live.into_iter())),
            };
            format!("that's it. {gone} is archived: {{ctrl+s}} finds it.{rest}")
        }
    }
}

thread_local! {
    static TOUR: RefCell<Tour> = RefCell::new(Tour::default());
}

/// A new state from the hub (sb.rs `apply_state`).
pub(crate) fn on_state(app: &App) {
    let s = crate::sb::tour_snap(app);
    TOUR.with(|t| {
        let mut t = t.borrow_mut();
        t.observe(&s);
        if t.on() {
            // the tour teaches these: the one-time hints stay down
            for h in [hints::Hint::FirstAgent, hints::Hint::FirstCard, hints::Hint::FirstSteer] {
                hints::covered(h);
            }
        }
    });
}

/// You sent a message (sb.rs `handle_input`), from the feed in view.
pub(crate) fn on_sent(app: &App) {
    let s = crate::sb::tour_snap(app);
    TOUR.with(|t| t.borrow_mut().sent(&s));
}

/// The tip up now, as the next frame sees it (tests).
#[cfg(test)]
pub(crate) fn current(app: &App) -> Option<Tip> {
    let s = crate::sb::tour_snap(app);
    TOUR.with(|t| {
        let mut t = t.borrow_mut();
        t.observe(&s);
        t.tip(&s)
    })
}

/// Tests: a fresh tour on this thread.
#[cfg(test)]
pub(crate) fn reset() {
    TOUR.with(|t| *t.borrow_mut() = Tour::default());
}

/// Draw the tip up, if any, after the frame (its anchor is read from
/// it). True when the tour runs: the one-time hints wait.
pub(crate) fn draw(app: &App, f: &mut Frame) -> bool {
    let s = crate::sb::tour_snap(app);
    let tip = TOUR.with(|t| {
        let mut t = t.borrow_mut();
        t.observe(&s);
        t.on().then(|| (t.tip(&s), t.scoped(&s)))
    });
    let Some((tip, s)) = tip else { return false };
    let Some(tip) = tip else { return true };
    let area = f.area();
    let (feed, panel) = crate::sb::split(area);
    let lines = hints::wrap(&text(tip, &s), TEXT_W);
    if let Some(r) = place(tip, &s, f.buffer_mut(), lines.len() as u16, area, feed, panel) {
        hints::draw_box(f, r, lines);
    }
    true
}

/// Inner text width of a tip (the hints' 36ch).
const TEXT_W: usize = 36;

/// Where tip `t` goes in the drawn frame (None: its thing is not on
/// screen, or no room).
fn place(t: Tip, s: &Snap, buf: &ratatui::buffer::Buffer, lines: u16, area: Rect, feed: Rect, panel: Option<Rect>) -> Option<Rect> {
    let bw = (TEXT_W as u16 + 4).min(feed.width.saturating_sub(4));
    let bh = lines + 2;
    if bw < 16 || bh > area.height {
        return None;
    }
    // left of the panel, level with the entry numbered `n`; no panel
    // (narrow): at the top of the feed
    let at_panel = |n: Option<usize>| -> Option<(u16, u16)> {
        match panel {
            Some(p) => {
                let y = n.and_then(|n| hints::panel_row(buf, p, n)).unwrap_or(p.y + 1);
                Some((p.x.checked_sub(bw + 1)?, y.saturating_sub(1).max(area.y)))
            }
            None => Some((feed.x + 4, feed.y + 1)),
        }
    };
    let (x, y) = match t {
        Tip::Start => at_panel(s.live().next().and_then(|m| m.number))?,
        Tip::Steer => at_panel(s.target().and_then(|m| m.number))?,
        Tip::End => at_panel(s.live().next().and_then(|m| m.number))?,
        // at the top of the agent's feed, right-aligned: its first rows
        // stay readable on the left
        Tip::Inside => (feed.right().saturating_sub(bw + 2).max(feed.x), feed.y + 1),
        // above the card, the arrow pointing down at it
        Tip::Card => {
            let y = hints::card_row(buf, feed)?;
            (feed.x + 4, y.checked_sub(bh)?.max(area.y))
        }
    };
    Some(Rect { x, y, width: bw, height: bh }.intersection(area))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, n: usize) -> Member {
        Member { name: name.into(), number: Some(n), archived: false }
    }

    fn team() -> Vec<Member> {
        vec![m("pm", 1), m("designer", 2), m("dev-api", 3)]
    }

    fn snap(members: Vec<Member>, focus: &str) -> Snap {
        Snap { members, focus: focus.into(), ..Snap::default() }
    }

    /// A tour past its first state (main alone), then the team spawned.
    fn started() -> (Tour, Snap) {
        let mut t = Tour::default();
        t.observe(&snap(vec![], "main"));
        let s = snap(team(), "main");
        t.observe(&s);
        (t, s)
    }

    #[test]
    fn only_demo_objectives_count() {
        assert!(is_demo("bise demo (role-play, not real work): you are the pm"));
        assert!(is_demo("  Bise Demo: x"));
        assert!(!is_demo("fix the bise demo skill"));
        assert!(!is_demo(""));
    }

    #[test]
    fn no_tour_for_agents_already_there_at_the_first_state() {
        let mut t = Tour::default();
        let s = snap(team(), "main");
        t.observe(&s);
        t.observe(&s);
        assert!(!t.on());
        assert_eq!(t.tip(&s), None);
    }

    #[test]
    fn the_tips_in_order() {
        let (mut t, mut s) = started();
        // 1. the team started
        assert_eq!(t.tip(&s), Some(Tip::Start));
        assert_eq!(text(Tip::Start, &s), "your team just started. they work on their own. {⌥1} looks inside pm. →");
        // 2. inside pm
        s.focus = "pm".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Inside));
        assert_eq!(text(Tip::Inside, &s), "this is pm's own thread, live. {⌥2} {⌥3} the others · {esc} back to main.");
        // a card came while inside: the inside tip first
        s.card = Some("designer".into());
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Inside));
        // 3. back in main: the card
        s.focus = "main".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Card));
        assert_eq!(text(Tip::Card, &s), "designer needs you. {ctrl+1} opens the question, {1} or {2} answers. ↓");
        let click = Snap { no_ctrl_digits: true, ..s.clone() };
        assert_eq!(text(Tip::Card, &click), "designer needs you. {click} the question, {1} or {2} answers. ↓");
        // 4. answered: a word for dev-api
        s.card = None;
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Steer));
        assert_eq!(text(Tip::Steer, &s), "dev-api waits for a word from you: {⌥3}, type, {⏎}. [✓] it got it, [✓✓] it read it.");
        // inside another agent: nothing; inside dev-api: the tip, no ⌥3
        s.focus = "designer".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), None);
        s.focus = "dev-api".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Steer));
        assert_eq!(text(Tip::Steer, &s), "dev-api waits for a word from you: type, {⏎}. [✓] it got it, [✓✓] it read it.");
        // you wrote: it stays while you watch the marks, goes when you leave
        t.sent(&s);
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Steer));
        s.focus = "main".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), None);
        // 5. main dropped pm: the end
        s.members[0] = Member { name: "pm".into(), number: None, archived: true };
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::End));
        assert_eq!(text(Tip::End, &s), "that's it. pm is archived: {ctrl+s} finds it. {⌥2} {⌥3} open the other two.");
        // ctrl+s: the tour is over, for good
        s.palette = true;
        t.observe(&s);
        assert!(!t.on());
        s.palette = false;
        t.observe(&s);
        assert_eq!(t.tip(&s), None);
    }

    #[test]
    fn an_open_card_comes_before_the_end() {
        let (mut t, mut s) = started();
        s.card = Some("designer".into());
        s.members[0].archived = true;
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Card));
        s.card = None;
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::End));
    }

    #[test]
    fn the_end_goes_when_you_open_another_agent() {
        let (mut t, mut s) = started();
        s.focus = "designer".into();
        s.members[0].archived = true;
        t.observe(&s);
        // it came while you were inside designer: it stays there
        assert_eq!(t.tip(&s), Some(Tip::End));
        s.focus = "main".into();
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::End));
        s.focus = "dev-api".into();
        t.observe(&s);
        assert!(!t.on());
    }

    #[test]
    fn a_message_moves_past_the_tip_up() {
        let (mut t, mut s) = started();
        // you talk to main instead of opening an agent: the start tip goes;
        // the word for dev-api waits for the card
        t.sent(&s);
        t.observe(&s);
        assert_eq!(t.tip(&s), None);
        // the card; a message skips it too
        s.card = Some("designer".into());
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::Card));
        t.sent(&s);
        assert_eq!(t.tip(&s), Some(Tip::Steer));
        t.sent(&s);
        assert_eq!(t.tip(&s), None);
        s.members[0].archived = true;
        t.observe(&s);
        assert_eq!(t.tip(&s), Some(Tip::End));
        t.sent(&s);
        assert!(!t.on());
    }

    #[test]
    fn it_ends_when_no_demo_agent_is_left_and_a_new_run_starts_fresh() {
        let (mut t, mut s) = started();
        for x in s.members.iter_mut() {
            x.archived = true;
        }
        t.observe(&s);
        assert!(!t.on());
        // a second demo: new names, a new tour; the first run's agents
        // are not part of it
        s.members.push(m("pm-2", 4));
        t.observe(&s);
        assert!(t.on());
        assert_eq!(t.tip(&s), Some(Tip::Start));
        assert_eq!(text(Tip::Start, &t.scoped(&s)), "your team just started. they work on their own. {⌥4} looks inside pm-2. →");
    }

    #[test]
    fn tips_fit_their_box() {
        let (_, mut s) = started();
        s.card = Some("designer".into());
        for tip in [Tip::Start, Tip::Inside, Tip::Card, Tip::Steer, Tip::End] {
            let ls = hints::wrap(&text(tip, &s), TEXT_W);
            assert!(ls.len() <= 4, "{tip:?}: {} lines", ls.len());
            assert!(ls.iter().all(|l| l.width() <= TEXT_W), "{tip:?}");
        }
    }
}
