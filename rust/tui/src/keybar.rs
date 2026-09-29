//! The key bar (BISE-99, book §8 "The frame", §13 "The composer pane").
//!
//! One row under the composer: the keys of the current mode in the text
//! color, what they do dim, 3 spaces between pairs; on the right, ending
//! at the row's last column, a dim tip, one per session, only in the
//! default mode while you don't type and when at least 3 columns separate
//! it from the keys. Track F places the row (BISE-98) and calls [`line`];
//! [`render`] is the pure part.

use crate::{attach, commands, files, help, theme, voice, App};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// What the key bar shows: one set of keys per mode (the sets of the old
/// hint row, the default one from book §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// the terminal panel is shown: keys go to the shell
    Terminal,
    /// voice: recording
    Recording,
    /// voice: the last words are being transcribed
    Transcribing,
    /// the `@` file popup
    FilePopup,
    /// images are attached to the draft (book §14)
    Images,
    /// `D` asks before dropping an agent (BISE-43)
    DropAsk,
    /// a yes / no question in the status row
    Confirm,
    /// the card full screen
    CardFull,
    /// an agent is selected in the panel
    Selected,
    /// in an archived agent
    Archived,
    /// switchboard, the agent in view is working
    Steer,
    /// switchboard, idle: the default bar (with the tip)
    Default,
    /// without switchboard, the turn runs
    SoloSteer,
    /// without switchboard, idle
    Solo,
}

/// A key and what it does; an empty key is a plain dim label.
type Pair = (&'static str, &'static str);

/// The way home from an agent's view (book §13, BISE-103): the first
/// pair there, never dropped.
const BACK: Pair = ("esc", "back to main");

impl Mode {
    fn pairs(self) -> &'static [Pair] {
        match self {
            Mode::Terminal => &[
                ("", "terminal: keys go to the shell"),
                ("ctrl+`", "hide"),
                ("wheel/shift+pgup", "scroll"),
                ("drag", "the border to resize"),
            ],
            Mode::Recording => &[("", "recording"), ("any key", "stops"), ("esc/ctrl+c", "cancel")],
            Mode::Transcribing => &[("", "transcribing the last words…"), ("esc/ctrl+c", "cancel")],
            Mode::FilePopup => &[
                ("⏎/tab", "insert"),
                ("⏎/tab/→", "open a folder"),
                ("←", "up"),
                ("↑↓", "select"),
                ("esc", "close"),
            ],
            Mode::Images => &[("ctrl+v", "paste image"), ("@", "file")],
            Mode::DropAsk => &[("y", "drop"), ("n or esc", "keep")],
            Mode::Confirm => &[("y", "yes"), ("n", "no"), ("esc", "cancel")],
            Mode::CardFull => &[("alt+r", "answer"), ("pgup/pgdn", "scroll"), ("ctrl+f", "back")],
            Mode::Selected => &[("⏎", "enter"), ("space", "preview"), ("D", "drop"), ("esc", "close")],
            Mode::Archived => &[BACK, ("/restore", "brings it back")],
            Mode::Steer => &[("tab", "queue"), ("⏎", "steer"), ("ctrl+c", "interrupt")],
            Mode::Default => &[
                ("⏎", "send"),
                ("@", "agent"),
                ("⌥0-9", "switch"),
                ("/", "commands"),
                ("?", "help"),
            ],
            Mode::SoloSteer => &[
                ("⏎", "steer"),
                ("tab", "queue"),
                ("ctrl+c", "interrupt"),
                ("/", "commands"),
                ("end", "bottom"),
            ],
            Mode::Solo => &[
                ("⏎", "send"),
                ("shift+⏎/ctrl+j", "new line"),
                ("/", "commands"),
                ("ctrl+o", "open/close all"),
                ("ctrl+c", "quit"),
            ],
        }
    }

    /// The keys of this mode in an agent's view (book §13 "Key bar in an
    /// agent's view"): `esc back to main` first; working, the book's set
    /// `esc back to main   ⏎ steer   ctrl+c interrupt`.
    fn agent_pairs(self) -> Vec<Pair> {
        match self {
            Mode::Default => std::iter::once(BACK).chain(self.pairs().iter().copied()).collect(),
            Mode::Steer => vec![BACK, ("⏎", "steer"), ("ctrl+c", "interrupt")],
            m => m.pairs().to_vec(),
        }
    }
}

/// The mode of `app` (the order of the old hint row).
pub(crate) fn mode(app: &App) -> Mode {
    if app.term.shown() {
        Mode::Terminal
    } else if app.voice.state() == voice::VoiceState::Recording {
        Mode::Recording
    } else if app.voice.state() == voice::VoiceState::Flushing {
        Mode::Transcribing
    } else if commands::popup_open(app) && files::token(&app.ed.text, app.ed.cursor).is_some() {
        Mode::FilePopup
    } else if !app.pending && app.sb.is_some() && attach::strip_height(app) > 0 {
        Mode::Images
    } else if let Some(m) = crate::sb::key_mode(app) {
        m
    } else if app.pending {
        Mode::SoloSteer
    } else {
        Mode::Solo
    }
}

/// The key bar of `app` for a row `width` columns wide: the mode's keys,
/// the session's tip (none in an agent's view).
pub(crate) fn line(app: &App, width: u16) -> Line<'static> {
    let agent = app.sb.as_ref().is_some_and(|sb| !sb.is_main_focus());
    render(mode(app), width, !app.ed.text.is_empty(), agent, Some(session_tip()))
}

/// The tip of this session: one of [`help::TIPS`], chosen at the first draw.
fn session_tip() -> &'static str {
    #[cfg(test)]
    {
        help::TIPS[0]
    }
    #[cfg(not(test))]
    {
        use std::sync::OnceLock;
        static TIP: OnceLock<&'static str> = OnceLock::new();
        TIP.get_or_init(|| {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as usize);
            help::TIPS[t % help::TIPS.len()]
        })
    }
}

/// A key's ASCII form (book §6): `alt+` for `⌥`, words for the arrows
/// and `⏎`; the key itself outside ASCII mode.
fn key_text(k: &str) -> String {
    if !theme::ascii_mode() {
        return k.to_string();
    }
    k.replace("↑↓", "up/down")
        .replace('↑', "up")
        .replace('↓', "down")
        .replace('⏎', "enter")
        .replace('⌥', "alt+")
        .replace('←', "left")
        .replace('→', "right")
}

fn label_text(l: &str) -> String {
    l.replace('…', theme::ellipsis())
}

/// The width of a pair drawn: the key, a space, what it does.
fn pair_width((k, l): Pair) -> usize {
    let (k, l) = (key_text(k), label_text(l));
    k.width() + l.width() + usize::from(!k.is_empty() && !l.is_empty())
}

/// The pairs of an agent's view that fit in `width` (book §13): `esc back
/// to main` stays; the others drop from the right, `/ commands` first.
fn fit_agent(mut pairs: Vec<Pair>, width: usize) -> Vec<Pair> {
    let total = |p: &[Pair]| p.iter().map(|&x| pair_width(x)).sum::<usize>() + 3 * p.len().saturating_sub(1);
    while pairs.len() > 1 && total(&pairs) > width {
        match pairs.iter().position(|&(k, _)| k == "/") {
            Some(i) => pairs.remove(i),
            None => pairs.pop().unwrap_or(BACK),
        };
    }
    pairs
}

/// The key bar for `mode` in a row `width` columns wide. Pairs that don't
/// fit are dropped from the end (the first is cut if it alone doesn't
/// fit). In an agent's view (`agent`), `esc back to main` comes first and
/// stays, the others drop from the right, `/ commands` first. The tip
/// shows only in [`Mode::Default`] out of an agent's view, while not
/// `typing`, right-aligned, with at least 3 columns between it and the
/// keys.
pub(crate) fn render(mode: Mode, width: u16, typing: bool, agent: bool, tip: Option<&str>) -> Line<'static> {
    let width = usize::from(width);
    let key = Style::default().fg(theme::text());
    let dim = Style::default().fg(theme::dim());
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    let pairs = if agent { fit_agent(mode.agent_pairs(), width) } else { mode.pairs().to_vec() };
    for (i, (k, l)) in pairs.iter().enumerate() {
        let (k, l) = (key_text(k), label_text(l));
        let gap = if i == 0 { 0 } else { 3 };
        let w = k.width() + l.width() + usize::from(!k.is_empty() && !l.is_empty());
        if used + gap + w > width {
            if i == 0 {
                // not even the first pair: cut it, as dim text
                let s = if k.is_empty() { l } else { format!("{k} {l}") };
                spans.push(Span::styled(cut(&s, width), dim));
                used = spans[0].width();
            }
            break;
        }
        if gap > 0 {
            spans.push(Span::raw("   "));
        }
        if !k.is_empty() {
            spans.push(Span::styled(k.clone(), key));
        }
        if !l.is_empty() {
            let sep = if k.is_empty() { "" } else { " " };
            spans.push(Span::styled(format!("{sep}{l}"), dim));
        }
        used += gap + w;
    }
    if let Some(t) = tip.filter(|_| mode == Mode::Default && !agent && !typing).map(key_text) {
        let t = if theme::ascii_mode() { format!("tip: {t}") } else { format!("tip · {t}") };
        let tw = t.width();
        if used + 3 + tw <= width {
            spans.push(Span::raw(" ".repeat(width - used - tw)));
            spans.push(Span::styled(t, dim));
        }
    }
    Line::from(spans)
}

/// `s` cut to `w` columns, ending with the ellipsis when cut.
fn cut(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    let e = theme::ellipsis();
    let room = w.saturating_sub(e.width());
    let mut out = String::new();
    for c in s.chars() {
        if (out.clone() + &c.to_string()).width() > room {
            break;
        }
        out.push(c);
    }
    if w >= e.width() {
        out.push_str(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    const TIP: &str = "ctrl+o opens everything folded";

    #[test]
    fn default_bar_and_tip_at_the_right_edge() {
        let l = render(Mode::Default, 100, false, false, Some(TIP));
        let s = text(&l);
        assert!(s.starts_with("⏎ send   @ agent   ⌥0-9 switch   / commands   ? help"), "{s}");
        assert!(s.ends_with("tip · ctrl+o opens everything folded"), "{s}");
        assert_eq!(s.width(), 100, "the tip ends at the row's last column");
    }

    #[test]
    fn keys_in_text_color_what_they_do_dim() {
        let l = render(Mode::Default, 100, false, false, Some(TIP));
        let key = l.spans.iter().find(|s| s.content == "⏎").unwrap();
        assert_eq!(key.style.fg, Some(theme::text()));
        let what = l.spans.iter().find(|s| s.content == " send").unwrap();
        assert_eq!(what.style.fg, Some(theme::dim()));
        let tip = l.spans.last().unwrap();
        assert_eq!(tip.style.fg, Some(theme::dim()));
    }

    #[test]
    fn no_tip_while_typing_or_in_another_mode() {
        assert!(!text(&render(Mode::Default, 100, true, false, Some(TIP))).contains("tip"));
        assert!(!text(&render(Mode::Steer, 100, false, false, Some(TIP))).contains("tip"));
    }

    #[test]
    fn no_tip_under_3_free_columns() {
        let keys = text(&render(Mode::Default, 200, false, false, None)).width();
        let tip = "tip · ".width() + TIP.width();
        let fits = render(Mode::Default, (keys + 3 + tip) as u16, false, false, Some(TIP));
        assert!(text(&fits).ends_with(TIP));
        let close = render(Mode::Default, (keys + 2 + tip) as u16, false, false, Some(TIP));
        assert!(!text(&close).contains("tip"));
    }

    #[test]
    fn narrow_drops_pairs_from_the_end() {
        let s = text(&render(Mode::Default, 30, false, false, Some(TIP)));
        assert_eq!(s, "⏎ send   @ agent   ⌥0-9 switch");
        let s = text(&render(Mode::Default, 4, false, false, None));
        assert!(s.width() <= 4 && s.ends_with('…'), "{s:?}");
    }

    #[test]
    fn ascii_forms() {
        theme::set_ascii_for_tests(true);
        let s = text(&render(Mode::Default, 100, false, false, Some(TIP)));
        let f = text(&render(Mode::FilePopup, 100, false, false, None));
        let t = text(&render(Mode::Transcribing, 100, false, false, None));
        theme::set_ascii_for_tests(false);
        assert!(s.starts_with("enter send   @ agent   alt+0-9 switch"), "{s}");
        assert!(s.ends_with("tip: ctrl+o opens everything folded"), "{s}");
        assert!(f.contains("enter/tab/right open a folder   left up   up/down select"), "{f}");
        assert!(t.contains("words..."), "{t}");
        for x in [s, f, t] {
            assert!(x.is_ascii(), "{x}");
        }
    }

    #[test]
    fn an_agents_view_starts_with_esc_back_to_main() {
        let idle = render(Mode::Default, 120, false, true, Some(TIP));
        let s = text(&idle);
        assert_eq!(s, "esc back to main   ⏎ send   @ agent   ⌥0-9 switch   / commands   ? help");
        let esc = idle.spans.iter().find(|x| x.content == "esc").unwrap();
        assert_eq!(esc.style.fg, Some(theme::text()));
        let what = idle.spans.iter().find(|x| x.content == " back to main").unwrap();
        assert_eq!(what.style.fg, Some(theme::dim()));
        assert!(!s.contains("tip"), "no tip in an agent's view");
        let working = text(&render(Mode::Steer, 120, false, true, None));
        assert_eq!(working, "esc back to main   ⏎ steer   ctrl+c interrupt");
        let archived = text(&render(Mode::Archived, 120, false, true, None));
        assert!(archived.starts_with("esc back to main   /restore"), "{archived}");
    }

    #[test]
    fn in_an_agents_view_commands_drop_first_and_esc_never() {
        let all = "esc back to main   ⏎ send   @ agent   ⌥0-9 switch   / commands   ? help".width();
        let s = text(&render(Mode::Default, (all - 1) as u16, false, true, None));
        assert_eq!(s, "esc back to main   ⏎ send   @ agent   ⌥0-9 switch   ? help");
        let s = text(&render(Mode::Default, 40, false, true, None));
        assert_eq!(s, "esc back to main   ⏎ send   @ agent");
        let s = text(&render(Mode::Steer, 20, false, true, None));
        assert_eq!(s, "esc back to main");
        let s = text(&render(Mode::Default, 10, false, true, None));
        assert!(s.starts_with("esc") && s.width() <= 10, "{s:?}");
        // other modes keep their own keys (esc does something else there)
        let s = text(&render(Mode::Selected, 120, false, true, None));
        assert!(s.starts_with("⏎ enter"), "{s}");
    }

    #[test]
    fn every_tip_names_a_key_of_the_help() {
        for t in help::TIPS {
            theme::set_ascii_for_tests(true);
            let a = text(&render(Mode::Default, 200, false, false, Some(t)));
            theme::set_ascii_for_tests(false);
            assert!(a.is_ascii() && a.contains("tip: "), "{a}");
            let k = t.split(' ').next().unwrap();
            assert!(
                help::ROWS.iter().any(|r| r.keys.split('|').any(|x| x == k)) || k.starts_with('/'),
                "{t}: {k} is not in the help"
            );
        }
    }
}
