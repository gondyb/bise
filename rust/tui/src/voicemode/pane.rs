//! Voice mode's pane in place of the composer (owner: voice-tui; design
//! §0, plan §2, the mocks' E: site/content/voice-ux.py `E`, `call_comp`).
//!
//! From [`BIG_FROM`] rows, half the screen: the composer's bar on every
//! row, the big kiss ([`super::kiss`]) on the left, on the right who
//! talks (`you` in accent, `:* main`), the words as they are said (≤
//! [`CAPTION_W`] cells a line, so 80 columns fit) and under the kiss the
//! status row (`● listening ▂▅▃`, `about to answer ●●●··`, `∿ main is
//! on it`, `))) speaking`, `you cut in`, `○ muted`, `hold space to talk`).
//! Under [`BIG_FROM`] rows (or too narrow for the kiss): B's two lanes,
//! you and the agent, a level wave each. The keys row is the key bar's
//! ([`keys`]); the header ([`header`]), the divider ([`divider`]) and
//! the thread's lighting ([`lit_style`]) are drawn by ui.rs / chrome.rs /
//! the feed from the same view.
//!
//! No state: the motion comes from `t_ms` (the blink, the bar's breath,
//! the kiss); `BISE_REDUCE_MOTION`: still frames. `NO_COLOR`: bold and
//! dim carry what the colors do. `BISE_ASCII=1`: the waves in `_ . - =
//! #` (the frame's asciify does the glyphs; block elements stay).

use super::kiss::{self, Pose};
use super::{LitState, PaneView, Phase, Route, Who, WordState, Work, WorkKind, WorkState};
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// From this many screen rows the big kiss, else the two lanes.
pub const BIG_FROM: u16 = 30;
/// The rows of the lanes' pane (the bar row, you, the agent); the key
/// bar under it makes B's 4 rows.
pub const LANES_H: u16 = 3;
/// Captions wrap at this many cells (80 columns fit).
pub const CAPTION_W: usize = 28;
/// The caption lines beside the kiss (the last ones that fit).
pub const CAPTION_LINES: usize = 6;
/// The kiss from the bar: the bar, 3 blank cells.
const KISS_X: u16 = 4;
/// The captions from the bar: the kiss, 3 blank cells.
const CAPS_X: u16 = KISS_X + kiss::W + 3;
/// The narrowest pane that holds the kiss and 20 cells of captions.
pub const BIG_MIN_W: u16 = CAPS_X + 20;
/// The `●` blinks: on, then off, this long each (the dictation chip's).
pub const BLINK_MS: u64 = 600;
/// The bar breathes accent → rule → accent in this long while it listens.
pub const BREATH_MS: u64 = 2000;
/// The status row's wave of your level.
const STATUS_WAVE: usize = 8;
/// A level in ASCII (`_ . - = #`), the dictation chip's.
const ASCII_LEVELS: [char; 8] = ['_', '.', '-', '-', '=', '#', '#', '#'];
/// The rolling thinking wave (indices in the level blocks).
const THINK_WAVE: [usize; 6] = [1, 2, 4, 5, 4, 2];

/// The composer's rows in voice mode on a screen `screen_h` rows high,
/// the key bar not counted: half the screen with it from [`BIG_FROM`]
/// rows, else the lanes ([`LANES_H`]).
pub fn height(screen_h: u16) -> u16 {
    if screen_h < BIG_FROM {
        LANES_H
    } else {
        (screen_h / 2).saturating_sub(1)
    }
}

/// How the pane is drawn (the environment, fixed in the tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    /// `BISE_REDUCE_MOTION`: still frames
    pub still: bool,
    /// `NO_COLOR`: bold/dim instead of colors
    pub no_color: bool,
    /// `BISE_ASCII=1`: ASCII waves
    pub ascii: bool,
}

impl Form {
    pub fn now() -> Form {
        Form {
            still: crate::gust::reduce_motion(),
            no_color: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
            ascii: theme::ascii_mode(),
        }
    }
}

/// Draws `v` into `area` at the animation time `t_ms`: the big kiss when
/// `area` is tall and wide enough, else the lanes.
pub fn draw(buf: &mut Buffer, area: Rect, v: &PaneView, t_ms: u64) {
    draw_in(buf, area, v, t_ms, Form::now());
}

pub fn draw_in(buf: &mut Buffer, area: Rect, v: &PaneView, t_ms: u64, form: Form) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    let bar = Span::styled("│", bar_style(v, t_ms, form));
    for y in area.y..area.bottom() {
        put(buf, area.x, y, area.right(), std::slice::from_ref(&bar));
    }
    if area.height >= kiss::H + 2 && area.width >= BIG_MIN_W {
        draw_big(buf, area, v, t_ms, form);
    } else {
        draw_lanes(buf, area, v, t_ms, form);
    }
}

// ---- the big kiss ----

fn draw_big(buf: &mut Buffer, area: Rect, v: &PaneView, t_ms: u64, form: Form) {
    // the kiss and the status row under it, in the middle of the pane
    let block = kiss::H + 1;
    let top = area.y + (area.height - block) / 2;
    let (pose, ink) = pose(v);
    let face = kiss::frame(pose, t_ms, v.agent_level, form.still);
    let face_st = fg(ink, form);
    for (r, row) in face.iter().enumerate() {
        put(buf, area.x + KISS_X, top + r as u16, area.right(), &[Span::styled(row.clone(), face_st)]);
    }
    let cx = area.x + CAPS_X;
    let cw = (area.right().saturating_sub(cx + 1) as usize).min(CAPTION_W);
    if let Some(q) = &v.question {
        draw_asked(buf, area, top, q, v, t_ms, form);
        return;
    }
    put(buf, cx, top + 1, area.right(), &speaker(v, form));
    // the work: its own column right of the captions when the pane has
    // room for it (the captions' place stays blank while it works: the
    // placeholder would point away from it); else, while the agent works,
    // in the captions' place (they are empty then)
    let wx = cx + CAPTION_W as u16 + WORK_GAP;
    let column = work_shows(v) && area.right() > wx + WORK_MIN_W;
    let caps: Vec<Line<'static>> = match v.phase {
        Phase::Working if column => Vec::new(),
        Phase::Working if work_shows(v) => work_lines(&v.work, cw, CAPTION_LINES, t_ms, form),
        Phase::Working => {
            let st = fg(theme::faint(), form);
            vec![Line::from(Span::styled("what it does shows in the", st)), Line::from(Span::styled("thread above", st))]
        }
        _ => captions(&v.words, cw, form),
    };
    let skip = caps.len().saturating_sub(CAPTION_LINES);
    for (i, l) in caps.into_iter().skip(skip).enumerate() {
        put(buf, cx, top + 3 + i as u16, area.right(), &l.spans);
    }
    put(buf, cx, top + kiss::H, area.right(), &status(v, t_ms, form, STATUS_WAVE));
    if column {
        // from the title row to the status row, the newest on the status row
        let ww = (area.right() - wx - 1).min(WORK_W) as usize;
        let rows = work_lines(&v.work, ww, kiss::H as usize, t_ms, form);
        let first = top + kiss::H + 1 - rows.len() as u16;
        for (i, l) in rows.into_iter().enumerate() {
            put(buf, wx, first + i as u16, area.right(), &l.spans);
        }
    }
}

// ---- your last question above the answer (voice-lastq, the user) ----

/// Your question's lines (the rest is cut, `…` at the end).
pub const QUESTION_LINES: usize = 2;
/// Under the question: a blank row, then the agent's name, then its words.
const ANSWER_ROW: u16 = QUESTION_LINES as u16 + 2;

/// The captions' column once your turn is sent, until you talk again
/// (designer): row 0 `you` (accent), rows 1-2 your question dim (settled:
/// the answer is the live thing), a blank row always (the answer's top
/// stays put between turns), row 4 `:* main`, rows 5-8 its words or, at
/// 95/80 while it works, its work; the work column at 150 as before.
fn draw_asked(buf: &mut Buffer, area: Rect, top: u16, q: &str, v: &PaneView, t_ms: u64, form: Form) {
    let cx = area.x + CAPS_X;
    let cw = (area.right().saturating_sub(cx + 1) as usize).min(CAPTION_W);
    let you = fg(theme::accent(), form);
    let you = if form.no_color { you.add_modifier(Modifier::BOLD) } else { you };
    put(buf, cx, top, area.right(), &[Span::styled("you", you)]);
    for (i, l) in question_lines(q, cw, QUESTION_LINES).into_iter().enumerate() {
        put(buf, cx, top + 1 + i as u16, area.right(), &[Span::styled(l, fg(theme::dim(), form))]);
    }
    let agent = match &v.who {
        Who::Agent(a) => a.clone(),
        Who::You => v.agent.clone(),
    };
    put(buf, cx, top + ANSWER_ROW, area.right(), &speaker(&PaneView { who: Who::Agent(agent), ..v.clone() }, form));
    let n = (kiss::H - ANSWER_ROW - 1) as usize;
    let wx = cx + CAPTION_W as u16 + WORK_GAP;
    let column = work_shows(v) && area.right() > wx + WORK_MIN_W;
    let rows: Vec<Line<'static>> = match (&v.phase, &v.who) {
        (Phase::Working, _) if column => Vec::new(),
        (Phase::Working, _) if work_shows(v) => work_lines(&v.work, cw, n, t_ms, form),
        (_, Who::Agent(_)) => captions(&v.words, cw, form),
        (_, Who::You) => Vec::new(),
    };
    let skip = rows.len().saturating_sub(n);
    for (i, l) in rows.into_iter().skip(skip).enumerate() {
        put(buf, cx, top + ANSWER_ROW + 1 + i as u16, area.right(), &l.spans);
    }
    put(buf, cx, top + kiss::H, area.right(), &status(v, t_ms, form, STATUS_WAVE));
    if column {
        let ww = (area.right() - wx - 1).min(WORK_W) as usize;
        let rows = work_lines(&v.work, ww, kiss::H as usize, t_ms, form);
        let first = top + kiss::H + 1 - rows.len() as u16;
        for (i, l) in rows.into_iter().enumerate() {
            put(buf, wx, first + i as u16, area.right(), &l.spans);
        }
    }
}

/// `q` wrapped at `w` cells, at most `n` lines; cut: the last line ends
/// with `…` (the start is what you recognise).
pub fn question_lines(q: &str, w: usize, n: usize) -> Vec<String> {
    let w = w.max(2);
    let mut lines: Vec<String> = vec![String::new()];
    for word in q.split_whitespace() {
        let word = if word.width() > w { cut(word, w) } else { word.to_string() };
        let line = lines.last_mut().expect("one line at least");
        if line.is_empty() {
            line.push_str(&word);
        } else if line.width() + 1 + word.width() <= w {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(word);
        }
    }
    lines.retain(|l| !l.is_empty());
    if lines.len() > n {
        lines.truncate(n);
        if let Some(last) = lines.last_mut() {
            *last = if last.width() < w { format!("{last}…") } else { format!("{}…", cut(last, w - 1)) };
        }
    }
    lines
}

// ---- the work, minified (round 2, the user: the mocks' D history) ----

/// The work column: this many cells after the captions' 28.
const WORK_GAP: u16 = 4;
/// The narrowest work column (else the work goes in the captions' place
/// while the agent works, and nowhere while it talks).
pub const WORK_MIN_W: u16 = 24;
/// The widest work column (the mocks' D).
pub const WORK_W: u16 = 44;

/// The work shows while the agent works and talks (not while you talk,
/// cutting in included: the pane is yours, the kiss and your words).
fn work_shows(v: &PaneView) -> bool {
    !v.work.is_empty() && matches!(v.phase, Phase::Working | Phase::Speaking | Phase::HoldToTalk)
}

/// The last `n` rows of `work`, one per line, cut at `w` cells, the newest
/// last: `∿ bash cargo test` running (lit, the mark moves), `· bash cargo
/// test ✓` done (dim, `✓` faint), `· bash cargo test ✗` failed (dim,
/// only the `✗` in the error color, as in the thread),
/// thinking in italics (`· thinking…` when it has no words yet).
/// `NO_COLOR`: running bold, done dim, failed by its `✗`.
pub fn work_lines(work: &[Work], w: usize, n: usize, t_ms: u64, form: Form) -> Vec<Line<'static>> {
    let skip = work.len().saturating_sub(n);
    work[skip..].iter().map(|x| work_line(x, w.max(4), t_ms, form)).collect()
}

fn work_line(x: &Work, w: usize, t_ms: u64, form: Form) -> Line<'static> {
    let text = x.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = match (x.kind, text.is_empty()) {
        (WorkKind::Thinking, true) => "thinking".to_string(),
        (_, _) => text,
    };
    let text = if x.kind == WorkKind::Thinking && x.state == WorkState::Running { format!("{text}…") } else { text };
    let (mark, mark_st, text_st, end) = match x.state {
        WorkState::Running => {
            let (g, c) = if form.still { (theme::glyph(theme::G_WORKING), theme::text()) } else { theme::working_frame((t_ms / 100) as u32) };
            let lit = fg(theme::text(), form);
            let lit = if form.no_color { lit.add_modifier(Modifier::BOLD) } else { lit };
            (g, fg(c, form), lit, None)
        }
        WorkState::Done => {
            let end = (x.kind == WorkKind::Tool).then(|| (theme::glyph(theme::G_DONE), dimmed(theme::faint(), form)));
            ("·", dimmed(theme::faint(), form), dimmed(theme::dim(), form), end)
        }
        WorkState::Failed => {
            let e = fg(theme::error(), form);
            let e = if form.no_color { e.add_modifier(Modifier::BOLD) } else { e };
            ("·", dimmed(theme::faint(), form), dimmed(theme::dim(), form), Some((theme::glyph(theme::G_FAILED), e)))
        }
    };
    let text_st = if x.kind == WorkKind::Thinking { text_st.add_modifier(Modifier::ITALIC) } else { text_st };
    // the mark, a blank, the text, then ` ✓` / ` ✗`: the text is cut first
    let room = w.saturating_sub(2 + end.map_or(0, |(g, _)| g.width() + 1));
    let shown = if text.width() > room { format!("{}…", cut(&text, room.saturating_sub(1))) } else { text };
    let mut s = vec![Span::styled(mark.to_string(), mark_st), Span::raw(" "), Span::styled(shown, text_st)];
    if let Some((g, st)) = end {
        s.extend([Span::raw(" "), Span::styled(g.to_string(), st)]);
    }
    Line::from(s)
}

/// The kiss's pose and color for the phase. Round 5 (the user): it
/// smiles at rest and blows a kiss for [`kiss::KISS_MS`] once the agent's
/// voice ended on its own (`kiss_ms`, set by the turn; you talking,
/// typing or muting ends it at once: the phase changes).
fn pose(v: &PaneView) -> (Pose, Color) {
    let kiss = v.kiss_ms.filter(|&ms| ms < kiss::KISS_MS).map(Pose::Kiss);
    match v.phase {
        Phase::Listening => (kiss.unwrap_or(Pose::Listen), theme::accent()),
        Phase::Hearing | Phase::AboutToAnswer { .. } | Phase::Holding | Phase::CutIn => (Pose::Talk, theme::accent()),
        Phase::Working => (Pose::Think, theme::accent()),
        Phase::Speaking => (Pose::Speak, theme::accent()),
        // voice-mute (the user): your mic off is yours, the face is the
        // agent's: it smiles and kisses in its color
        Phase::HoldToTalk | Phase::Muted => (kiss.unwrap_or(Pose::Rest), theme::accent()),
        Phase::Typing | Phase::Failed(_) => (Pose::Rest, theme::faint()),
    }
}

/// Who talks: `you` (accent), `:* main`, another agent's name in bold;
/// the agent while it works (the status row says it's on it).
fn speaker(v: &PaneView, form: Form) -> Vec<Span<'static>> {
    let bold = |c| fg(c, form).add_modifier(Modifier::BOLD);
    let agent = |a: &str| {
        if a == "main" {
            vec![Span::styled(":*", bold(theme::accent())), Span::raw(" "), Span::styled("main", bold(theme::text()))]
        } else {
            vec![Span::styled(a.to_string(), bold(theme::text()))]
        }
    };
    match (&v.phase, &v.who) {
        (Phase::Working, _) => agent(&v.agent),
        (_, Who::You) if v.muted => vec![Span::styled("you", fg(theme::faint(), form))],
        (_, Who::You) => vec![Span::styled("you", fg(theme::accent(), form))],
        (_, Who::Agent(a)) => agent(a),
    }
}

/// The words wrapped at `w` cells, each in its state's style; the word
/// being said (the last said one before the unsaid) underlined.
pub fn captions(words: &[(String, WordState)], w: usize, form: Form) -> Vec<Line<'static>> {
    let w = w.max(1);
    let words: Vec<(&str, WordState)> = words.iter().map(|(s, st)| (s.trim(), *st)).filter(|(s, _)| !s.is_empty()).collect();
    let now = words.iter().rposition(|(_, s)| *s == WordState::Said).filter(|&i| words.get(i + 1).is_some_and(|(_, s)| *s == WordState::ToSay));
    let mut lines: Vec<Vec<Span<'static>>> = vec![vec![]];
    let mut col = 0usize;
    for (i, (word, state)) in words.iter().enumerate() {
        let mut word = word.to_string();
        let mut ww = word.width();
        if ww > w {
            word = cut(&word, w);
            ww = word.width();
        }
        if col > 0 && col + 1 + ww > w {
            lines.push(vec![]);
            col = 0;
        }
        let st = word_style(*state, form);
        let line = lines.last_mut().expect("one line at least");
        if col > 0 {
            line.push(Span::styled(" ", st));
            col += 1;
        }
        let st = if Some(i) == now { st.add_modifier(Modifier::UNDERLINED) } else { st };
        line.push(Span::styled(word, st));
        col += ww;
    }
    lines.into_iter().filter(|l| !l.is_empty()).map(Line::from).collect()
}

fn word_style(s: WordState, form: Form) -> Style {
    match s {
        WordState::Heard | WordState::Said => fg(theme::text(), form),
        WordState::Partial | WordState::ToSay => dimmed(theme::dim(), form),
        WordState::Cut => dimmed(theme::faint(), form),
    }
}

/// The style of a word of the agent's message lit in the thread (the
/// same as beside the kiss): said in the text color, not said yet dim,
/// cut faint. `NO_COLOR`: dim for both.
pub fn lit_style(s: LitState) -> Style {
    let form = Form::now();
    match s {
        LitState::Said => fg(theme::text(), form),
        LitState::ToSay => dimmed(theme::dim(), form),
        LitState::Cut => dimmed(theme::faint(), form),
    }
}

/// `heard "the first one" → 1 smaller` wins over the phase's row.
fn status(v: &PaneView, t_ms: u64, form: Form, wave_n: usize) -> Vec<Span<'static>> {
    let d = |s: &str| Span::styled(s.to_string(), fg(theme::dim(), form));
    let f = |s: &str| Span::styled(s.to_string(), dimmed(theme::faint(), form));
    let sp = || Span::raw(" ");
    if let Some(h) = &v.heard_answer {
        return vec![Span::styled(h.clone(), fg(theme::text(), form))];
    }
    // voice-mute (designer): the agent's state as usual, then your
    // ` · ○ muted` faint; alone at rest it is the news: dim
    if v.muted && v.phase != Phase::Muted {
        let mut s = match &v.phase {
            Phase::Speaking => vec![d("speaking")],
            _ => status(&PaneView { muted: false, ..v.clone() }, t_ms, form, wave_n),
        };
        s.push(f(" · ○ muted"));
        return s;
    }
    match &v.phase {
        Phase::Listening | Phase::Hearing => {
            vec![blink(t_ms, form), sp(), d("listening"), sp(), wave_span(&you_wave(v), wave_n, theme::accent(), form)]
        }
        Phase::AboutToAnswer { fill } => {
            let mut s = vec![d("about to answer"), sp()];
            s.extend(fill_dots(*fill, form));
            s
        }
        Phase::Holding => vec![blink(t_ms, form), sp(), d("the floor is yours"), sp(), wave_span(&you_wave(v), wave_n, theme::accent(), form)],
        Phase::Working => {
            let (g, c) = if form.still { (theme::glyph(theme::G_WORKING), theme::text()) } else { theme::working_frame((t_ms / 100) as u32) };
            vec![Span::styled(g, fg(c, form)), sp(), d(&format!("{} is on it", v.agent))]
        }
        // the arcs are the kiss's (beside its mouth), not a status
        Phase::Speaking => vec![d(if v.route == Route::Headphones { "speaking" } else { "hold space to cut in" })],
        Phase::CutIn => vec![blink(t_ms, form), sp(), f("you cut in"), sp(), wave_span(&you_wave(v), wave_n, theme::accent(), form)],
        Phase::Muted => vec![d("○ muted")],
        Phase::Typing => vec![f("typing: the mic waits")],
        Phase::HoldToTalk => vec![d("hold space to talk")],
        Phase::Failed(line) => vec![Span::styled(line.clone(), fg(theme::error(), form))],
    }
}

/// `●●●··`: the fill of the end of your turn, 5 dots.
fn fill_dots(fill: f32, form: Form) -> Vec<Span<'static>> {
    let k = ((fill.clamp(0.0, 1.0) * 5.0).floor() as usize).min(5);
    let on = fg(theme::accent(), form);
    let on = if form.no_color { on.add_modifier(Modifier::BOLD) } else { on };
    vec![Span::styled("●".repeat(k), on), Span::styled("·".repeat(5 - k), dimmed(theme::faint(), form))]
}

/// The `●` of a listening mic: accent and dim every [`BLINK_MS`] (bold
/// on/off under `NO_COLOR`), lit when still.
fn blink(t_ms: u64, form: Form) -> Span<'static> {
    let on = form.still || (t_ms / BLINK_MS).is_multiple_of(2);
    let st = match (form.no_color, on) {
        (true, true) => Style::default().fg(theme::accent()).add_modifier(Modifier::BOLD),
        (true, false) => Style::default().fg(theme::accent()),
        (false, true) => Style::default().fg(theme::accent()),
        (false, false) => Style::default().fg(theme::dim()),
    };
    Span::styled("●", st)
}

// ---- the lanes (under 30 rows) ----

fn draw_lanes(buf: &mut Buffer, area: Rect, v: &PaneView, t_ms: u64, form: Form) {
    // the bar row, you, the agent: the last 2 rows of the area
    if area.height < 2 {
        let y = area.y;
        put(buf, area.x + KISS_X, y, area.right(), &you_lane(v, t_ms, form, lane_w(area.width)));
        return;
    }
    let y = area.bottom() - 2;
    let n = lane_w(area.width);
    put(buf, area.x + KISS_X, y, area.right(), &you_lane(v, t_ms, form, n));
    put(buf, area.x + KISS_X, y + 1, area.right(), &agent_lane(v, t_ms, form, n));
}

/// A lane's wave: 52 cells from 100 columns, 40 from 70, else what the
/// width leaves for it and 18 cells of words.
fn lane_w(width: u16) -> usize {
    match width {
        98.. => 52,
        68..=97 => 40,
        _ => (width as usize).saturating_sub(KISS_X as usize + 7 + 18).max(6),
    }
}

/// `you  ▁▂▅▃▁…  ● listening`
fn you_lane(v: &PaneView, t_ms: u64, form: Form, n: usize) -> Vec<Span<'static>> {
    let d = |s: &str| Span::styled(s.to_string(), fg(theme::dim(), form));
    let f = |s: &str| Span::styled(s.to_string(), dimmed(theme::faint(), form));
    let (name, wave, tail): (Span<'static>, Span<'static>, Vec<Span<'static>>) = match &v.phase {
        // voice-mute: your lane greys whatever the agent does on its own;
        // `○ muted` dim at rest (the only news), faint while the agent acts
        Phase::Muted => (f("you"), wave_span(&[], n, theme::faint(), form), vec![d("○ muted")]),
        _ if v.muted => (f("you"), wave_span(&[], n, theme::faint(), form), vec![f("○ muted")]),
        Phase::Listening | Phase::Hearing | Phase::Holding => (
            d("you"),
            wave_span(&you_wave(v), n, theme::accent(), form),
            vec![blink(t_ms, form), Span::raw(" "), d(if v.phase == Phase::Holding { "the floor is yours" } else { "listening" })],
        ),
        Phase::AboutToAnswer { fill } => {
            let mut t = vec![d("about to answer"), Span::raw(" ")];
            t.extend(fill_dots(*fill, form));
            (d("you"), wave_span(&you_wave(v), n, theme::accent(), form), t)
        }
        Phase::CutIn => (d("you"), wave_span(&you_wave(v), n, theme::accent(), form), vec![blink(t_ms, form), Span::raw(" "), f("you cut in")]),
        Phase::HoldToTalk => (d("you"), wave_span(&[], n, theme::faint(), form), vec![d("hold space to talk")]),
        Phase::Speaking if v.route != Route::Headphones => {
            (d("you"), wave_span(&[], n, theme::faint(), form), vec![d("hold space to cut in")])
        }
        Phase::Typing => (d("you"), wave_span(&[], n, theme::faint(), form), vec![f("typing: the mic waits")]),
        Phase::Failed(line) => (d("you"), wave_span(&[], n, theme::faint(), form), vec![Span::styled(line.clone(), fg(theme::error(), form))]),
        Phase::Working | Phase::Speaking => (d("you"), wave_span(&[], n, theme::faint(), form), vec![]),
    };
    let tail = match &v.heard_answer {
        Some(h) => vec![Span::styled(h.clone(), fg(theme::text(), form))],
        None => tail,
    };
    let mut out = vec![name, Span::raw("  "), wave, Span::raw("  ")];
    out.extend(tail);
    out
}

/// `:*   ▁▃▆▅▂…  ))) speaking`: the agent's mouth `:*` ↔ `:o` with its
/// level (`:O` reads as shock: never).
fn agent_lane(v: &PaneView, t_ms: u64, form: Form, n: usize) -> Vec<Span<'static>> {
    let d = |s: &str| Span::styled(s.to_string(), fg(theme::dim(), form));
    let mouth_st = fg(theme::accent(), form).add_modifier(Modifier::BOLD);
    let open = v.phase == Phase::Speaking && !form.still && v.agent_level >= 0.25;
    let mouth = Span::styled(if open { ":o" } else { ":*" }, mouth_st);
    let (wave, tail) = match &v.phase {
        Phase::Working => {
            let (g, c) = if form.still { (theme::glyph(theme::G_WORKING), theme::text()) } else { theme::working_frame((t_ms / 100) as u32) };
            (think_span(n, t_ms, form), vec![Span::styled(g, fg(c, form)), Span::raw(" "), d(&format!("{} is on it", v.agent))])
        }
        Phase::Speaking => {
            let arcs = ")".repeat(kiss::arcs_for(if form.still { 0.45 } else { v.agent_level }));
            (wave_span(&agent_wave(v), n, theme::text(), form), vec![Span::styled(format!("{arcs:<3}"), fg(theme::dim(), form)), Span::raw(" "), d("speaking")])
        }
        Phase::CutIn => (wave_span(&agent_wave(v), n, theme::text(), form), vec![]),
        _ => (wave_span(&[], n, theme::faint(), form), vec![]),
    };
    let name = if v.agent == "main" { vec![mouth, Span::raw("  ")] } else { vec![Span::styled(v.agent.clone(), fg(theme::text(), form).add_modifier(Modifier::BOLD)), Span::raw(" ")] };
    let mut out = name;
    // the agent's name may be longer than the mouth: the waves keep their column
    let used: usize = out.iter().map(|s| s.content.width()).sum();
    if used < 4 {
        out.push(Span::raw(" ".repeat(4 - used)));
    }
    out.push(Span::raw(" "));
    out.push(wave);
    out.push(Span::raw("  "));
    out.extend(tail);
    out
}

// ---- the waves ----

/// Your recent levels, oldest first (one per [`super::WAVE_STEP`]); the
/// level now when the controller kept none.
fn you_wave(v: &PaneView) -> Vec<f32> {
    if v.you_wave.is_empty() {
        vec![v.you_level]
    } else {
        v.you_wave.clone()
    }
}

/// The agent's recent levels, oldest first.
fn agent_wave(v: &PaneView) -> Vec<f32> {
    if v.agent_wave.is_empty() {
        vec![v.agent_level]
    } else {
        v.agent_wave.clone()
    }
}

fn level_char(i: usize, ascii: bool) -> char {
    let i = i.min(7);
    if ascii {
        ASCII_LEVELS[i]
    } else {
        crate::voice::PEAK_BLOCKS[i]
    }
}

/// The last `n` levels, the newest on the right, the floor before them.
pub fn wave(levels: &[f32], n: usize, ascii: bool) -> String {
    let skip = levels.len().saturating_sub(n);
    let pad = n - (levels.len() - skip);
    let lv = levels[skip..].iter().map(|&p| level_char((p.clamp(0.0, 1.0) * 8.0) as usize, ascii));
    std::iter::repeat_n(level_char(0, ascii), pad).chain(lv).collect()
}

fn wave_span(levels: &[f32], n: usize, c: Color, form: Form) -> Span<'static> {
    let st = if c == theme::faint() { dimmed(c, form) } else { fg(c, form) };
    Span::styled(wave(levels, n, form.ascii), st)
}

/// Thinking: the wave rolls one cell every 120 ms, dim.
fn think_span(n: usize, t_ms: u64, form: Form) -> Span<'static> {
    let k = if form.still { 0 } else { (t_ms / 120) as usize };
    let s: String = (0..n).map(|i| level_char(THINK_WAVE[(i + k) % THINK_WAVE.len()], form.ascii)).collect();
    Span::styled(s, dimmed(theme::dim(), form))
}

// ---- the keys, the header, the divider ----

/// The key bar in voice mode, for `width` columns (the mocks' keys row):
/// under 98 columns `tab` goes and `space` says less.
pub fn keys(v: &PaneView, width: u16) -> Line<'static> {
    let form = Form::now();
    let k = |s: &str| Span::styled(s.to_string(), fg(theme::text(), form));
    let d = |s: &str| Span::styled(s.to_string(), fg(theme::dim(), form));
    let gap = || Span::raw("   ");
    let wide = width >= 98;
    let mut s = vec![Span::raw(" ")];
    match &v.phase {
        _ if v.muted => s.extend([k("m"), Span::raw(" "), d("unmute"), gap(), k("esc"), Span::raw(" "), d("leave")]),
        Phase::AboutToAnswer { .. } => {
            s.extend([d("keep talking, or"), gap(), k("space"), Span::raw(" "), d("send now")]);
            if wide {
                s.extend([gap(), k("hold space"), Span::raw(" "), d("i'm thinking")]);
            }
        }
        Phase::Muted => s.extend([k("m"), Span::raw(" "), d("unmute"), gap(), k("esc"), Span::raw(" "), d("leave")]),
        Phase::HoldToTalk => s.extend([k("space"), Span::raw(" "), d("hold to talk"), gap(), k("esc"), Span::raw(" "), d("leave"), gap(), k("m"), Span::raw(" "), d("mute")]),
        Phase::Speaking if v.route != Route::Headphones => {
            s.extend([k("space"), Span::raw(" "), d("hold to cut in"), gap(), k("esc"), Span::raw(" "), d("leave"), gap(), k("m"), Span::raw(" "), d("mute")])
        }
        _ => {
            s.extend([k("esc"), Span::raw(" "), d("leave"), gap(), k("m"), Span::raw(" "), d("mute"), gap(), k("space"), Span::raw(" ")]);
            s.push(d(if wide { "send now · hold: keep the floor" } else { "send · hold" }));
            if wide {
                s.extend([gap(), k("tab"), Span::raw(" "), d("type")]);
            }
        }
    }
    Line::from(s)
}

/// The header's mark in every view: `● voice mode 2:14` (the `●` and
/// the words accent, the time dim); muted: `○` faint. Never the error
/// color: voice mode isn't an error.
pub fn header(v: &PaneView) -> Vec<Span<'static>> {
    let form = Form::now();
    let secs = v.elapsed.as_secs();
    let time = Span::styled(format!("{}:{:02}", secs / 60, secs % 60), fg(theme::dim(), form));
    if v.muted || v.phase == Phase::Muted {
        vec![Span::styled("○ voice mode", dimmed(theme::faint(), form)), Span::raw(" "), time]
    } else {
        let st = fg(theme::accent(), form);
        let st = if form.no_color { st.add_modifier(Modifier::BOLD) } else { st };
        vec![Span::styled("● voice mode", st), Span::raw(" "), time]
    }
}

/// The divider's left: `you ⇄ main · voice mode · headphones` (the
/// route unsaid when unknown).
pub fn divider(v: &PaneView) -> Vec<Span<'static>> {
    let form = Form::now();
    let d = |s: &str| Span::styled(s.to_string(), fg(theme::dim(), form));
    let a = |s: &str| Span::styled(s.to_string(), fg(theme::accent(), form));
    let dot = || Span::styled(" · ", fg(theme::faint(), form));
    let mut s = vec![Span::raw(" "), d("you"), Span::raw(" "), a("⇄"), Span::raw(" "), a(&v.agent), dot(), d("voice mode")];
    match v.route {
        Route::Headphones => s.extend([dot(), d("headphones")]),
        Route::Speakers => s.extend([dot(), d("speakers")]),
        Route::Unknown => {}
    }
    s.push(Span::raw(" "));
    s
}

// ---- styles and cells ----

/// The bar breathes accent → rule in one slow ease while the mic waits
/// for you (listening, hearing, about to answer, cut in); else accent.
/// `NO_COLOR`: bold, then plain at the breath's low; muted: rule.
fn bar_style(v: &PaneView, t_ms: u64, form: Form) -> Style {
    let breathes = matches!(v.phase, Phase::Listening | Phase::Hearing | Phase::AboutToAnswer { .. } | Phase::CutIn);
    if v.muted || matches!(v.phase, Phase::Muted | Phase::Typing) {
        return Style::default().fg(theme::rule());
    }
    if !breathes || form.still {
        return Style::default().fg(theme::accent());
    }
    let p = 0.65 + 0.35 * (2.0 * std::f32::consts::PI * (t_ms % BREATH_MS) as f32 / BREATH_MS as f32).cos();
    if form.no_color {
        let st = Style::default().fg(theme::accent());
        return if p > 0.6 { st.add_modifier(Modifier::BOLD) } else { st };
    }
    Style::default().fg(mix(theme::accent(), theme::rule(), p))
}

/// `p` of `a` and `1 - p` of `b` (RGB only; else `a`).
fn mix(a: Color, b: Color, p: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let m = |x: u8, y: u8| (x as f32 * p + y as f32 * (1.0 - p)).round().clamp(0.0, 255.0) as u8;
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => a,
    }
}

fn fg(c: Color, _form: Form) -> Style {
    Style::default().fg(c)
}

/// A quiet color, and the dim attribute under `NO_COLOR` (where the
/// colors may not show).
fn dimmed(c: Color, form: Form) -> Style {
    let st = Style::default().fg(c);
    if form.no_color {
        st.add_modifier(Modifier::DIM)
    } else {
        st
    }
}

/// `s` cut to `w` cells.
fn cut(s: &str, w: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + c.to_string().width() > w {
            break;
        }
        out.push(c);
    }
    out
}

/// The spans from `(x, y)`, clipped at `end`.
fn put(buf: &mut Buffer, x: u16, y: u16, end: u16, spans: &[Span]) {
    if y < buf.area.y || y >= buf.area.bottom() || x >= end {
        return;
    }
    let mut x = x;
    for s in spans {
        if x >= end {
            break;
        }
        let (nx, _) = buf.set_stringn(x, y, s.content.as_ref(), (end - x) as usize, s.style);
        x = nx;
    }
}

/// The thread's lighting from [`super::Lit`].
#[path = "pane_lit.rs"]
pub mod lit;

#[cfg(test)]
#[path = "pane_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pane_screens_tests.rs"]
mod screens_tests;
