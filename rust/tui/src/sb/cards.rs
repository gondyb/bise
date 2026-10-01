//! The inbox (cards v2, BISE-236; its arrows and ⏎: book screens
//! `inbox · 1-7`): the hub's questions and alerts at three levels. The
//! UI says "inbox" and the items' own nouns (a question, an approval);
//! the code keeps "card".
//!
//! One rule for the arrows: an empty composer, they belong to the inbox;
//! text in the composer, they belong to your text. In the thread they
//! never touch the inbox.
//!
//! 1. The quick look: the strip right above the divider, one row per
//!    open card, most blocking first (approvals, questions, the rest),
//!    numbered 1, 2, 3… in that order (the panel uses the same numbers).
//! 2. The card view (BISE-302: ctrl+N opens row N, a click on a row
//!    opens it; `/inbox` the top one): it takes the history's
//!    place; the cards are tabs. On an empty composer ↑↓ highlight an
//!    option (none on open: a reflex ⏎ never answers), ⏎ picks it, 1-9
//!    pick at once, ←→ move between the cards; with text the composer
//!    answers (⏎ sends it). ctrl+n / ctrl+p move, ctrl+x closes, esc
//!    goes back to the thread. Each card keeps its own draft; the
//!    thread's draft waits for the way back.
//!
//! A new card never takes the focus: a strip row, or a tab. Answering
//! moves to the next card, or back to the thread when none are left,
//! with a dim `✓ perf · you said both` in the history.
//!
//! Approvals (docs/approvals.md §7, approvals-plan.md round 2) plug in
//! as the kind `approval`: its text is the command (or a patch), a blank
//! line, then the reason; the options are allow once / always here /
//! deny, and ⏎ with text denies with the text as a note. The gate that
//! opens them is not built yet.

use super::*;
use crate::editor::Editor;
use crate::theme;
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct Card {
    pub(super) id: u64,
    pub(super) kind: String,
    pub(super) agent: String,
    pub(super) text: String,
    /// The card's age when the snapshot arrived, and when it arrived.
    pub(super) age_ms: u64,
    pub(super) seen_at: std::time::Instant,
    /// The hub's remark (the asker heard from main since...).
    pub(super) note: String,
    /// How the TUI's own items (setup, BISE-245) read: their words are
    /// the TUI's, not parsed from `text`.
    pub(super) look: Option<Box<Look>>,
}

/// A paragraph of an item the TUI writes itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Para {
    Text(String),
    /// said on the side: dim
    Dim(String),
    /// a unified diff, in the code colors
    Diff(String),
}

/// How an item of the TUI's own reads (the setup items): the strip's
/// row and its faint end, the view's title, meta, tab and body, the
/// options; a strip action and a key bar of its own when it has no
/// options (the connectors key: `⏎ paste it`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) row: String,
    pub(crate) row_note: String,
    pub(crate) title: String,
    pub(crate) meta: String,
    pub(crate) tab: String,
    pub(crate) body: Vec<Para>,
    pub(crate) options: Vec<String>,
    pub(crate) right: Option<(&'static str, &'static str)>,
    pub(crate) keys: Vec<(&'static str, &'static str)>,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            id: 0,
            kind: String::new(),
            agent: String::new(),
            text: String::new(),
            age_ms: 0,
            seen_at: std::time::Instant::now(),
            note: String::new(),
            look: None,
        }
    }
}

impl Card {
    /// How long ago the card opened, in ms.
    pub(super) fn age_now(&self) -> u64 {
        self.age_ms.saturating_add(self.seen_at.elapsed().as_millis() as u64)
    }
}

/// What a click on the strip or the card view does (set by the draw).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CardHit {
    /// open the card view on this card (a strip row, the panel's)
    Row(u64),
    /// answer this card with its option `i` at once
    Pick(u64, usize),
    /// show this card in the view (a tab)
    Tab(u64),
    /// open the card view on the top card
    Open,
}

/// The cards as the user sees them: the card view open or not, the card in it, its option highlighted, how far it
/// is scrolled, the drafts.
#[derive(Default)]
pub(super) struct CardView {
    /// The card view is up (it takes the history's place).
    pub(super) open: bool,
    /// The card in the view; its draft is in the composer while open.
    pub(super) sel: Option<u64>,
    /// The option highlighted in the view (none on open), and whether
    /// the next draw scrolls it into view.
    pub(super) opt: Option<usize>,
    pub(super) reveal: bool,
    pub(super) scroll: usize,
    /// Set by the last draw: the last scroll offset and the page size.
    pub(super) max_scroll: usize,
    pub(super) page: usize,
    /// Set by the last draw: the card view (the wheel over it scrolls
    /// it) and the strip.
    pub(super) area: Rect,
    pub(super) strip: Rect,
    /// The drafts of the cards out of view (esc keeps them).
    drafts: HashMap<u64, Editor>,
    /// The thread's draft while the view is open.
    pub(super) thread: Option<Editor>,
    /// Answered or closed here, still in the hub's last snapshot: hidden.
    answered: Vec<u64>,
    /// Set by the last draw: what a click hits.
    pub(super) hits: RefCell<Vec<(Rect, CardHit)>>,
}

impl CardView {
    /// Scroll by `d` rows (negative: up), within the card.
    pub(super) fn scroll_by(&mut self, d: isize) {
        self.scroll = if d < 0 {
            self.scroll.saturating_sub(d.unsigned_abs())
        } else {
            self.scroll.saturating_add(d.unsigned_abs()).min(self.max_scroll)
        };
    }

    pub(super) fn hit(&self, x: u16, y: u16) -> Option<CardHit> {
        let hits = self.hits.borrow();
        // the last drawn wins (an option over its row)
        hits.iter().rev().find(|(r, _)| x >= r.x && x < r.right() && y >= r.y && y < r.bottom()).map(|(_, h)| *h)
    }
}

/// What ⏎ does in the card view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enter {
    /// the text answers
    Answer,
    /// an approval: the text denies, as a note to the agent
    Deny,
    /// no words needed: an empty composer acknowledges (done, overlap)
    Ack,
}

/// A piece of a card's text in the view.
pub(super) enum Part {
    Text(String),
    /// a command or a patch: code colors on the raised tint
    Code(Vec<Vec<Span<'static>>>),
    /// why it runs (approvals), dim
    Reason(String),
    /// the hub's remark, dim italic
    Note(String),
}

/// A card as the strip and the view show it.
pub(super) struct Shape {
    /// after the glyph, bold: `perf needs you`, `release wants to run`
    pub(super) title: String,
    /// the strip's name: `perf`, `release wants to run`
    pub(super) who: String,
    /// the strip's text: the first line, `npm publish · 13 lines`
    pub(super) summary: String,
    pub(super) parts: Vec<Part>,
    /// the options, whole (the view), and short (the strip)
    pub(super) options: Vec<String>,
    pub(super) short: Vec<String>,
    pub(super) enter: Enter,
    /// faint after the strip's text (`1 min · i ask before changing
    /// anything`)
    pub(super) note: String,
    /// the view's meta instead of `2 of 3 · 6m · ⌥1 perf`, its tab's
    /// label instead of the agent
    pub(super) meta: Option<String>,
    pub(super) tab: Option<String>,
    /// the strip's action when there are no options (`⏎ paste it`)
    pub(super) right: Option<(&'static str, &'static str)>,
    /// typed words answer it (`type to answer in your words`)
    pub(super) words: bool,
    /// the view's key bar, when the item has its own
    pub(super) keys: Vec<(&'static str, &'static str)>,
    /// each option's digit when not 1, 2, 3… (a gate's card: `no` is
    /// always 3, designer: a thumb that learned 3 = no never allows)
    pub(super) nums: Vec<usize>,
}

impl Shape {
    /// The digit of option `i`.
    pub(super) fn num(&self, i: usize) -> usize {
        self.nums.get(i).copied().unwrap_or(i + 1)
    }

    /// The option digit `d` picks.
    pub(super) fn by_digit(&self, d: usize) -> Option<usize> {
        (0..self.options.len()).find(|&i| self.num(i) == d)
    }
}

impl Shape {
    fn plain(title: String, who: String, summary: String, parts: Vec<Part>, options: Vec<String>, short: Vec<String>, enter: Enter) -> Shape {
        Shape {
            title,
            who,
            summary,
            parts,
            options,
            short,
            enter,
            note: String::new(),
            meta: None,
            tab: None,
            right: None,
            words: true,
            keys: Vec::new(),
            nums: Vec::new(),
        }
    }
}

/// The options of an approval (approvals.md §7), whole and short.
const ALLOW: [&str; 3] = ["allow once", "always here", "deny"];
const ALLOW_SHORT: [&str; 3] = ["allow", "always", "deny"];

/// The shape of card `c`.
pub(super) fn shape(c: &Card) -> Shape {
    if c.kind == "approval" {
        return approval_shape(c);
    }
    if c.kind == "confirm" {
        return confirm_shape(c);
    }
    if c.kind == "setup" {
        return setup_shape(c);
    }
    let (body, options) = if no_words(&c.kind) { (c.text.clone(), Vec::new()) } else { split_choices(&c.text) };
    let summary = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
    let mut parts = vec![Part::Text(body)];
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    Shape::plain(
        kind_title(&c.kind, &c.agent),
        if c.kind == "question" { c.agent.clone() } else { kind_title(&c.kind, &c.agent) },
        summary,
        parts,
        options.clone(),
        short_labels(&options),
        if no_words(&c.kind) { Enter::Ack } else { Enter::Answer },
    )
}

/// An approval: the command (or a patch) up to the first blank line,
/// then the reason.
fn approval_shape(c: &Card) -> Shape {
    let text = c.text.trim_matches('\n');
    let (head, reason) = match text.find("\n\n") {
        Some(i) => (&text[..i], text[i + 2..].trim()),
        None => (text, ""),
    };
    let lines: Vec<&str> = head.lines().collect();
    let patch = head.starts_with("diff --git") || head.starts_with("--- ");
    let (title, who, summary, code) = if patch {
        let files: Vec<&str> =
            lines.iter().filter_map(|l| l.strip_prefix("+++ ")).map(|f| f.trim().trim_start_matches("b/")).collect();
        let plus = lines.iter().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
        let minus = lines.iter().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();
        let first = files.first().copied().unwrap_or("a file");
        let more = match files.len() {
            0 | 1 => String::new(),
            n => format!(" +{} file{} ·", n - 1, if n > 2 { "s" } else { "" }),
        };
        let minus_sign = if theme::ascii_mode() { "-" } else { "−" };
        (
            format!("{} wants to edit {}", c.agent, first),
            format!("{} wants to edit", c.agent),
            format!("{first}{more} +{plus} {minus_sign}{minus}"),
            crate::code::highlight_patch(head),
        )
    } else {
        let first = lines.first().map_or("", |l| l.trim());
        let summary = if lines.len() > 1 { format!("{first} · {} lines", lines.len()) } else { first.to_string() };
        (format!("{} wants to run", c.agent), format!("{} wants to run", c.agent), summary, crate::code::highlight_bash(head))
    };
    let mut parts = vec![Part::Code(code)];
    if !reason.is_empty() {
        parts.push(Part::Reason(reason.to_string()));
    }
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    Shape::plain(
        title,
        who,
        summary,
        parts,
        ALLOW.iter().map(|s| s.to_string()).collect(),
        ALLOW_SHORT.iter().map(|s| s.to_string()).collect(),
        Enter::Deny,
    )
}

/// A gate's card (approvals-design.md §9), as the hub writes it: the
/// title (`wants to run`), `= ` the parts already allowed (dim, above),
/// `| ` what needs you, `reason: `, one `always: ` per rule it saves (none:
/// a hard rule, no option 2). Options: allow, always allow … here, no;
/// typed words say no, with the words as the note.
fn confirm_shape(c: &Card) -> Shape {
    let (mut head, mut done, mut body, mut reason, mut always) = (String::new(), vec![], vec![], String::new(), vec![]);
    for (i, l) in c.text.lines().enumerate() {
        if i == 0 {
            head = l.to_string();
        } else if let Some(x) = l.strip_prefix("= ") {
            done.push(x.to_string());
        } else if let Some(x) = l.strip_prefix("| ").or(l.strip_prefix("|")) {
            body.push(x.to_string());
        } else if let Some(x) = l.strip_prefix("reason: ") {
            reason = x.to_string();
        } else if let Some(x) = l.strip_prefix("always: ") {
            always.push(x.to_string());
        }
    }
    let n = c.agent.split(", ").count();
    let title = if n > 1 {
        format!("{} agents {}", n, head.replacen("wants", "want", 1))
    } else {
        format!("{} {}", c.agent, head)
    };
    // `wants to run it outside the sandbox`: a sandbox card (brief 1e)
    let bash = head.starts_with("wants to run");
    let rerun = head.ends_with("outside the sandbox");
    let edit = head.starts_with("wants to edit");
    // an edit or a connector: its first 3 lines (after the path), the rest counted
    let (shown, more) = if bash {
        (body.clone(), 0)
    } else {
        let keep = if edit { 4 } else { 3 };
        (body.iter().take(keep).cloned().collect::<Vec<_>>(), body.len().saturating_sub(keep))
    };
    let mut parts: Vec<Part> = done.iter().map(|d| Part::Reason(format!("$ {d}"))).collect();
    let text = shown.join("\n");
    parts.push(Part::Code(if bash {
        // the bash mark in accent before the command (designer)
        let mut rows = crate::code::highlight_bash(&text);
        if let Some(first) = rows.first_mut() {
            first.insert(0, Span::styled("$ ", Style::default().fg(theme::accent())));
        }
        rows
    } else if edit {
        crate::code::highlight_patch(&text)
    } else {
        text.lines().map(|l| vec![Span::raw(l.to_string())]).collect()
    }));
    if more > 0 {
        parts.push(Part::Reason(format!("{} {} more line{}", theme::glyph("▸"), more, if more == 1 { "" } else { "s" })));
    }
    if !reason.is_empty() {
        parts.push(Part::Reason(reason));
    }
    if !c.note.is_empty() {
        parts.push(Part::Note(c.note.clone()));
    }
    let first = body.first().map_or("", |l| l.trim());
    let summary = if body.len() > 1 && bash { format!("{first} · {} lines", body.len()) } else { first.to_string() };
    let mut options = vec![if rerun { "run it again without the sandbox" } else { "allow" }.to_string()];
    let mut short = vec![if rerun { "run again" } else { "allow" }.to_string()];
    if !always.is_empty() {
        options.push(always_option(rerun, &always, &body));
        short.push("always".into());
    }
    options.push("no".into());
    short.push("no".into());
    let who = title.clone();
    let hard = always.is_empty();
    let mut s = Shape::plain(title, who, summary, parts, options, short, Enter::Deny);
    s.note = String::new();
    // `no` is always 3 (designer): a hard rule has no 2
    if hard {
        s.nums = vec![1, 3];
    }
    s
}

/// A confirm card's option 2. A sandbox card's saves a "run outside the
/// sandbox" rule, and says so (designer): `it` when the rule is the
/// command shown, else the rule.
fn always_option(rerun: bool, always: &[String], body: &[String]) -> String {
    let rules = always.join(", ");
    if !rerun {
        format!("always allow {rules} here")
    } else if rules == body.join("\n").trim() {
        "always run it outside the sandbox here".to_string()
    } else {
        format!("always run {rules} outside the sandbox here")
    }
}

/// A setup item (BISE-245): its look, written by setup.rs; without one
/// (never from setup.rs), its first line in the strip, a diff in code
/// colors, the rest as text.
fn setup_shape(c: &Card) -> Shape {
    let Some(l) = c.look.as_deref() else {
        let (body, options) = split_choices(&c.text);
        let summary = body.lines().next().unwrap_or("").trim().to_string();
        let parts = body
            .split("\n\n")
            .map(|p| if p.starts_with("--- ") { Part::Code(crate::code::highlight_patch(p)) } else { Part::Text(p.to_string()) })
            .collect();
        let mut s = Shape::plain(c.agent.clone(), c.agent.clone(), summary, parts, options.clone(), options, Enter::Answer);
        s.words = false;
        return s;
    };
    let parts = l
        .body
        .iter()
        .map(|p| match p {
            Para::Text(t) => Part::Text(t.clone()),
            Para::Dim(t) => Part::Reason(t.clone()),
            Para::Diff(t) => Part::Code(crate::code::highlight_patch(t)),
        })
        .collect();
    let mut s = Shape::plain(l.title.clone(), c.agent.clone(), l.row.clone(), parts, l.options.clone(), l.options.clone(), Enter::Answer);
    s.note = l.row_note.clone();
    s.meta = Some(l.meta.clone()).filter(|m| !m.is_empty());
    s.tab = Some(l.tab.clone()).filter(|t| !t.is_empty());
    s.right = l.right;
    s.words = false;
    s.keys = l.keys.clone();
    s
}

/// The strip's labels of `options`: their first words when those differ
/// (`compress`, `both`), else the start of each, cut.
fn short_labels(options: &[String]) -> Vec<String> {
    let first: Vec<String> = options
        .iter()
        .map(|o| o.split_whitespace().next().unwrap_or("").trim_end_matches([':', ',', '.', ';']).to_string())
        .collect();
    let distinct = first.iter().enumerate().all(|(i, a)| !a.is_empty() && !first[..i].contains(a));
    if distinct {
        first
    } else {
        options.iter().map(|o| cut(o, 16)).collect()
    }
}

/// `s` in at most `w` columns, cut with `…`.
pub(super) fn cut(s: &str, w: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if s.width() <= w {
        return s.to_string();
    }
    let e = theme::ellipsis();
    let room = w.saturating_sub(e.width());
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > room {
            break;
        }
        used += cw;
        out.push(ch);
    }
    let out = out.trim_end().to_string();
    if w >= e.width() {
        out + e
    } else {
        out
    }
}

impl Sb {
    /// The cards in reading order, the ones answered here left out: what
    /// blocks an agent first (approvals, questions), then the oldest.
    pub(super) fn sorted_cards(&self) -> Vec<&Card> {
        let mut v: Vec<&Card> = self.cards.iter().filter(|c| !self.card.answered.contains(&c.id)).collect();
        v.sort_by_key(|c| (kind_look(&c.kind).0, c.id));
        v
    }

    fn card_ids(&self) -> Vec<u64> {
        self.sorted_cards().iter().map(|c| c.id).collect()
    }

    /// The card in the view while it is open, else the top card.
    pub(super) fn current_card(&self) -> Option<&Card> {
        let v = self.sorted_cards();
        self.card
            .sel
            .filter(|_| self.card.open)
            .and_then(|id| v.iter().find(|c| c.id == id).copied())
            .or_else(|| v.first().copied())
    }

    fn card_by_id(&self, id: u64) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// The panel number of `agent` (⌥N), if it has one.
    pub(super) fn number_of(&self, agent: &str) -> Option<usize> {
        (0..10).find(|n| self.agent_numbered(*n).as_deref() == Some(agent))
    }
}

/// A card kind: its rank in reading order (what blocks an agent first),
/// its glyph and its color. Color means attention (book §5): needs you
/// in accent, failures in error, the rest plain text.
pub(super) fn kind_look(kind: &str) -> (u8, &'static str, Color) {
    match kind {
        "approval" | "confirm" => (0, theme::G_NEEDS_YOU, theme::accent()),
        "question" => (1, theme::G_NEEDS_YOU, theme::accent()),
        "blocked" => (2, theme::G_NEEDS_YOU, theme::accent()),
        "failed" => (3, theme::G_FAILED, theme::error()),
        "restart" => (3, theme::G_RESTART_FAILED, theme::error()),
        "drop" => (4, theme::G_STOPPED, theme::text()),
        "overlap" => (5, theme::G_OVERLAP, theme::text()),
        "done" => (6, theme::done_glyph(), theme::text()),
        // the setup card and its offers (BISE-245): they block nothing
        "setup" => (7, theme::G_NEEDS_YOU, theme::accent()),
        _ => (6, theme::G_CARD, theme::accent()),
    }
}

/// The color of a card kind's glyph: its hue, except done's check, in
/// accent on a plain title (BISE-100, book §6).
pub(super) fn glyph_color(kind: &str) -> Color {
    match kind {
        "done" => theme::accent(),
        k => kind_look(k).2,
    }
}

/// A card's title, after its glyph (copy deck §17: `{name} needs you`).
fn kind_title(kind: &str, agent: &str) -> String {
    match kind {
        "question" => format!("{agent} needs you"),
        "blocked" => format!("{agent} is blocked"),
        "failed" => format!("{agent} failed"),
        "restart" => "restart failed".into(),
        "drop" => format!("drop {agent}?"),
        "overlap" => "overlap".into(),
        "done" => format!("{agent} is done"),
        k => format!("{agent}: {k}"),
    }
}

/// The cards answered with no words: ⏎ on an empty composer.
fn no_words(kind: &str) -> bool {
    matches!(kind, "done" | "overlap")
}

// ---- the actions ----

/// Open the card view on card `id` (none, or gone: the top card). The
/// thread's draft waits for the way back.
pub(super) fn open_view(app: &mut App, id: Option<u64>) {
    let ids = app.sb.card_ids();
    let Some(id) = id.filter(|i| ids.contains(i)).or_else(|| ids.first().copied()) else { return };
    if !app.sb.card.open {
        app.sb.card.thread = Some(std::mem::take(&mut app.ed));
        app.sb.card.open = true;
        app.sb.card.sel = None;
    }
    show(app, id);
}

/// Back to the thread: the card's draft is kept, the thread's comes back.
pub(super) fn close_view(app: &mut App) {
    if !app.sb.card.open {
        return;
    }
    park_draft(app);
    let cv = &mut app.sb.card;
    app.ed = cv.thread.take().unwrap_or_default();
    cv.open = false;
    cv.scroll = 0;
}

/// The composer's draft goes to the card in view's slot.
fn park_draft(app: &mut App) {
    let cv = &mut app.sb.card;
    if let Some(old) = cv.sel.take() {
        let d = std::mem::take(&mut app.ed);
        if d.text.is_empty() {
            cv.drafts.remove(&old);
        } else {
            cv.drafts.insert(old, d);
        }
    }
}

/// Card `id` in the view, with its draft in the composer.
fn show(app: &mut App, id: u64) {
    if app.sb.card.sel == Some(id) {
        return;
    }
    park_draft(app);
    let cv = &mut app.sb.card;
    app.ed = cv.drafts.remove(&id).unwrap_or_default();
    cv.sel = Some(id);
    cv.opt = None;
    cv.reveal = false;
    cv.scroll = 0;
    cv.max_scroll = 0;
}

/// ctrl+n / ctrl+p: the next or previous card, around.
fn step(app: &mut App, d: isize) {
    let ids = app.sb.card_ids();
    if ids.is_empty() {
        return;
    }
    let i = app.sb.card.sel.and_then(|s| ids.iter().position(|x| *x == s)).unwrap_or(0) as isize;
    let j = (i + d).rem_euclid(ids.len() as isize) as usize;
    show(app, ids[j]);
}

/// Answer card `id` with `reply`; the history says `✓ agent · you said
/// {said}`; the view moves on.
fn answer(app: &mut App, id: u64, reply: &str, said: &str) {
    // the TUI's own cards: answered here, their own result row
    if super::setup::is_local(id) {
        if super::setup::valid(app, id, reply) {
            retire(app, id);
            super::setup::answer(app, id, reply);
        }
        return;
    }
    let Some((agent, kind)) = app.sb.card_by_id(id).map(|c| (c.agent.clone(), c.kind.clone())) else { return };
    app.sb.send_input(format!("/answer {} {}", id, reply));
    // a gate's card folds with the hub's own line (`✓ you allowed …`)
    if kind != "confirm" {
        let line = format!("{} {} · you said {}", theme::done_glyph(), agent, said);
        push_event(&mut app.events, &mut app.cache, Ev::Info(line));
    }
    retire(app, id);
}

/// Card `id` is answered or closed: hidden until the hub drops it; the
/// view goes to the next card, or back to the thread.
fn retire(app: &mut App, id: u64) {
    let ids = app.sb.card_ids();
    let cv = &mut app.sb.card;
    cv.answered.push(id);
    cv.drafts.remove(&id);
    if !(cv.open && cv.sel == Some(id)) {
        return;
    }
    // the answer is sent: its text is gone from the composer
    app.ed = Editor::default();
    cv.sel = None;
    let i = ids.iter().position(|x| *x == id).unwrap_or(0);
    let next = ids.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| ids.get(j))).copied();
    match next {
        Some(n) => show(app, n),
        None => close_view(app),
    }
}

/// Option `i` of card `id` answers it (a digit, a click).
/// The option of card `id` whose digit is `d`.
fn pick_digit(app: &mut App, id: u64, d: usize) -> bool {
    match app.sb.card_by_id(id).map(shape).and_then(|s| s.by_digit(d)) {
        Some(i) => pick(app, id, i),
        None => false,
    }
}

fn pick(app: &mut App, id: u64, i: usize) -> bool {
    let Some(s) = app.sb.card_by_id(id).map(shape) else { return false };
    let (Some(reply), Some(said)) = (s.options.get(i), s.short.get(i)) else { return false };
    answer(app, id, reply, said);
    true
}

/// ⏎ in the card view: the text answers (an approval: denies with the
/// text as a note); an empty composer acknowledges a card that needs no
/// words, else does nothing.
fn submit(app: &mut App) {
    let Some((id, enter)) = app.sb.current_card().map(|c| (c.id, shape(c).enter)) else { return };
    let text = app.ed.text.trim().to_string();
    // a setup card: never in the history (the key card's text is a key)
    if super::setup::is_local(id) {
        if !text.is_empty() {
            answer(app, id, &text, "");
        }
        return;
    }
    if text.is_empty() {
        if enter == Enter::Ack {
            answer(app, id, "seen", "seen");
        }
        return;
    }
    app.history.insert(0, app.ed.text.clone());
    let short = cut(&one_line(&text), 40);
    match enter {
        Enter::Deny => answer(app, id, &format!("deny: {text}"), &format!("deny: {short}")),
        _ => answer(app, id, &text, &short),
    }
}

/// ctrl+x: close card `id` without answering.
fn close_card(app: &mut App, id: u64) {
    if app.sb.card_by_id(id).is_none() {
        return;
    }
    if super::setup::is_local(id) {
        retire(app, id);
        super::setup::close(app, id);
        return;
    }
    app.sb.send_input(format!("/close {}", id));
    retire(app, id);
}

/// A new snapshot: forget what the hub closed; the card in view went
/// away (answered elsewhere): its draft goes to the history (↑ brings it
/// back) and the view moves on.
pub(super) fn sync(app: &mut App) {
    let live: Vec<u64> = app.sb.cards.iter().map(|c| c.id).collect();
    let cv = &mut app.sb.card;
    cv.answered.retain(|x| live.contains(x));
    cv.drafts.retain(|k, _| live.contains(k));
    if !cv.open {
        return;
    }
    let ids = app.sb.card_ids();
    let cv = &mut app.sb.card;
    if cv.sel.is_some_and(|s| ids.contains(&s)) {
        return;
    }
    let d = std::mem::take(&mut app.ed);
    if !d.text.trim().is_empty() {
        app.history.insert(0, d.text);
    }
    cv.sel = None;
    match ids.first() {
        Some(&n) => show(app, n),
        None => close_view(app),
    }
}

/// ctrl+1-9 (BISE-302): the digit, for inbox row N; any other key: none.
/// Ctrl with or without shift (a layout where digits need shift). On a
/// French layout the kitty protocol reports the top row's own character
/// (`&é"'(-è_çà`, a Mac's `§` and `!`): it counts as its digit.
pub(crate) fn ctrl_digit(k: &crossterm::event::KeyEvent) -> Option<usize> {
    if k.modifiers != KeyModifiers::CONTROL && k.modifiers != KeyModifiers::CONTROL | KeyModifiers::SHIFT {
        return None;
    }
    let KeyCode::Char(c) = k.code else { return None };
    let d = match c {
        '1'..='9' => c as usize - '0' as usize,
        '&' => 1,
        'é' => 2,
        '"' => 3,
        '\'' => 4,
        '(' => 5,
        '-' | '§' => 6,
        'è' => 7,
        '_' | '!' => 8,
        'ç' => 9,
        _ => return None,
    };
    Some(d)
}

/// A key only a terminal that reports ctrl+digits sends (BISE-302): one
/// without them turns ctrl+1 into `1`, ctrl+2 into ctrl+space, ctrl+3
/// into esc, ctrl+8 into backspace and ctrl+9 into `9`; ctrl+4-7 come as
/// they are from any terminal (the bytes 0x1c-0x1f), so they prove
/// nothing.
pub(crate) fn proves_ctrl_digits(k: &crossterm::event::KeyEvent) -> bool {
    matches!(ctrl_digit(k), Some(1 | 2 | 3 | 8 | 9))
}

/// Inbox row `n` (1 = the most blocking, the strip's numbers) in the
/// card view; past the last row: nothing. `true` when it opened.
pub(super) fn open_row(app: &mut App, n: usize) -> bool {
    let rows = super::card_draw::strip_ids(&app.sb);
    let Some(&id) = n.checked_sub(1).and_then(|i| rows.get(i)) else { return false };
    if app.sb.card.open {
        show(app, id);
    } else {
        open_view(app, Some(id));
    }
    true
}

/// The card keys; `true` when handled. From the thread only ctrl+1-9
/// (open inbox row N, BISE-302); the rest in the card view.
pub(super) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    if let Some(n) = ctrl_digit(k) {
        return open_row(app, n);
    }
    if !app.sb.card.open {
        return false;
    }
    let empty = app.ed.text.is_empty();
    let sel = app.sb.current_card().map(|c| c.id);
    let n = app.sb.current_card().map_or(0, |c| shape(c).options.len());
    // the arrows are the inbox's on an empty composer, else your text's
    let arrows = empty && !popup_open;
    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) if !popup_open => close_view(app),
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => step(app, 1),
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => step(app, -1),
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            if let Some(id) = sel {
                close_card(app, id);
            }
        }
        // an empty composer picks the option highlighted (none: nothing,
        // a reflex ⏎ never answers); text answers
        (KeyCode::Enter, KeyModifiers::NONE) if !popup_open => match (sel, app.sb.card.opt.filter(|_| empty)) {
            (Some(id), Some(i)) if i < n => {
                pick(app, id, i);
            }
            _ => submit(app),
        },
        // 1-9 picks on an empty composer; once you typed, digits are text
        (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) if empty => {
            return sel.is_some_and(|id| pick_digit(app, id, c as usize - '0' as usize));
        }
        (KeyCode::PageUp, _) if !popup_open => {
            let page = app.sb.card.page.max(1) as isize;
            app.sb.card.scroll_by(-page);
        }
        (KeyCode::PageDown, _) if !popup_open => {
            let page = app.sb.card.page.max(1) as isize;
            app.sb.card.scroll_by(page);
        }
        // ↑↓ choose an option, no wrap: the first ↓ lands on option 1,
        // the first ↑ on the last (no option: nothing, never the thread's
        // history recalled into a card)
        (KeyCode::Up | KeyCode::Down, KeyModifiers::NONE) if arrows => {
            if n > 0 {
                let cv = &mut app.sb.card;
                let down = k.code == KeyCode::Down;
                cv.opt = Some(match cv.opt.map(|i| i.min(n - 1)) {
                    None if down => 0,
                    None => n - 1,
                    Some(i) if down => (i + 1).min(n - 1),
                    Some(i) => i.saturating_sub(1),
                });
                cv.reveal = true;
            }
        }
        // ←→ the other cards
        (KeyCode::Left, KeyModifiers::NONE) if arrows => step(app, -1),
        (KeyCode::Right, KeyModifiers::NONE) if arrows => step(app, 1),
        // no queue for an answer
        (KeyCode::Tab, _) if !popup_open => {}
        _ => return false,
    }
    true
}

/// The mouse on the strip or the card view; `true` when handled.
pub(crate) fn card_mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let cv = &app.sb.card;
    let a = cv.area;
    let over = |r: Rect| m.column >= r.x && m.column < r.right() && m.row >= r.y && m.row < r.bottom();
    match m.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if cv.open && over(a) => {
            let d = if m.kind == MouseEventKind::ScrollUp { -3 } else { 3 };
            app.sb.card.scroll_by(d);
            true
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(hit) = cv.hit(m.column, m.row) else { return false };
            match hit {
                CardHit::Row(id) => open_view(app, Some(id)),
                CardHit::Open => open_view(app, None),
                CardHit::Tab(id) => show(app, id),
                CardHit::Pick(id, i) => {
                    pick(app, id, i);
                }
            }
            true
        }
        _ => false,
    }
}

/// The choices an agent gives at the end of its question: its last
/// lines `1. v1` / `1) v1` / `1 - v1`, numbered 1, 2, … (two to nine).
/// Returns the text without them, and the choices (none: the text as is).
fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return (text.to_string(), Vec::new());
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}


/// The open cards matching `q` (their number, kind, agent or text):
/// the card argument of `/close` and `/answer`.
pub(crate) fn card_choices(app: &App, q: &str) -> Vec<Choice> {
    app.sb
        .sorted_cards()
        .into_iter()
        .filter(|c| crate::commands::matches(q, &[&c.id.to_string(), &c.kind, &c.agent, &c.text]))
        .map(|c| {
            let (_, icon, _) = kind_look(&c.kind);
            Choice {
                value: c.id.to_string(),
                label: if super::setup::is_local(c.id) { "setup".into() } else { format!("#{}", c.id) },
                desc: format!("{} @{} · {}", c.kind, c.agent, truncate_chars(&one_line(&c.text), 80)),
                mark: Some((icon, glyph_color(&c.kind))),
            }
        })
        .collect()
}

/// The text on one line: runs of whitespace become one space.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}


#[cfg(test)]
#[path = "cards_tests.rs"]
mod tests;
