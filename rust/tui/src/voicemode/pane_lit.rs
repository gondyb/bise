//! The thread's lighting (design §0 round 3, plan §4.6): the agent's
//! message lands whole in the thread; while it is said, its spoken words
//! light up there in step with the voice, the same words as beside the
//! kiss. The feed renders the message as usual (markdown, wrapped); this
//! restyles the rendered rows from [`super::super::Lit`]'s byte ranges in
//! the message's source.
//!
//! The rendered rows and the source are matched char by char: markdown
//! drops marks (`**`, `#`, `` ` ``) and wraps, it never reorders, so each
//! rendered char is found a little further in the source. Only the
//! prose is matched and restyled (cells in the text color): the `:*`
//! mark, code, links and bullets keep their own looks.

use super::lit_style;
use crate::theme;
use crate::voicemode::LitState;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::ops::Range;

/// How far ahead in the source a rendered char may be (the marks and
/// spaces markdown drops between two chars of prose).
const LOOKAHEAD: usize = 24;

/// `rows` (one message as the feed drew it) with the lit words restyled.
pub fn light(rows: &[Line<'static>], src: &str, spans: &[(Range<usize>, LitState)]) -> Vec<Line<'static>> {
    light_with(rows, src, spans, lit_style)
}

pub fn light_with(rows: &[Line<'static>], src: &str, spans: &[(Range<usize>, LitState)], style: impl Fn(LitState) -> Style) -> Vec<Line<'static>> {
    let mut pos = 0usize;
    let state_at = |b: usize| spans.iter().find(|(r, _)| r.contains(&b)).map(|(_, s)| *s);
    // a space takes the state of the char before it
    let mut prev: Option<LitState> = None;
    rows.iter()
        .map(|line| {
            let mut out: Vec<Span<'static>> = Vec::with_capacity(line.spans.len());
            for span in &line.spans {
                if !is_prose(span.style) {
                    // code, a link: the cursor goes past it in the source
                    let t = span.content.trim();
                    if let Some(j) = src.get(pos..).filter(|_| !t.is_empty()).and_then(|r| r.find(t)).filter(|j| *j <= LOOKAHEAD) {
                        pos += j + t.len();
                    }
                    out.push(span.clone());
                    continue;
                }
                // split the span where the state changes
                let mut run = String::new();
                let mut run_state: Option<LitState> = None;
                for c in span.content.chars() {
                    let st = if c.is_whitespace() {
                        prev
                    } else {
                        match find(src, pos, c) {
                            Some(j) => {
                                pos = j + c.len_utf8();
                                state_at(j)
                            }
                            None => None,
                        }
                    };
                    prev = st;
                    if st != run_state && !run.is_empty() {
                        out.push(styled(std::mem::take(&mut run), span.style, run_state, &style));
                    }
                    run_state = st;
                    run.push(c);
                }
                if !run.is_empty() {
                    out.push(styled(run, span.style, run_state, &style));
                }
            }
            Line { spans: out, ..line.clone() }
        })
        .collect()
}

/// The byte of the next `c` in `src` from `pos`, within [`LOOKAHEAD`].
fn find(src: &str, pos: usize, c: char) -> Option<usize> {
    let rest = src.get(pos..)?;
    rest.char_indices().take_while(|(i, _)| *i <= LOOKAHEAD).find(|(_, d)| *d == c).map(|(i, _)| pos + i)
}

/// A cell of prose: the text color (or none: the ground's ink).
fn is_prose(st: Style) -> bool {
    matches!(st.fg, None | Some(Color::Reset)) || st.fg == Some(theme::text())
}

fn styled(s: String, base: Style, state: Option<LitState>, style: &impl Fn(LitState) -> Style) -> Span<'static> {
    match state {
        Some(st) => Span::styled(s, base.patch(style(st))),
        None => Span::styled(s, base),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn txt(s: &str) -> Span<'static> {
        Span::styled(s.to_string(), Style::default().fg(theme::text()))
    }

    /// (text, state) runs of a line, the unlit ones `None`.
    fn runs(l: &Line) -> Vec<(String, Option<LitState>)> {
        l.spans
            .iter()
            .map(|s| {
                let st = [LitState::Said, LitState::ToSay, LitState::Cut].into_iter().find(|&k| s.style.fg == lit_style(k).fg && s.style.fg != Some(theme::text()));
                let st = if st.is_none() && s.style.fg == Some(theme::text()) && s.style == Style::default().fg(theme::text()) { None } else { st };
                (s.content.to_string(), st)
            })
            .collect()
    }

    #[test]
    fn the_said_words_light_up_through_the_markdown_and_the_wrap() {
        let src = "**on it.** perf takes the signup, in its own worktree.\n\nthe rest is detail.";
        // said: "on it. perf takes the"; to say: "signup, in its own worktree."
        let said = 2..25;
        let to_say = 26..54;
        assert_eq!(&src[said.clone()], "on it.** perf takes the");
        assert_eq!(&src[to_say.clone()], "signup, in its own worktree.");
        let rows = vec![
            Line::from(vec![Span::styled(" :* ", Style::default().fg(theme::accent())), txt("on it."), txt(" perf takes the")]),
            Line::from(vec![Span::raw("    "), txt("signup, in its own worktree.")]),
            Line::from(""),
            Line::from(vec![Span::raw("    "), txt("the rest is detail.")]),
        ];
        let spans = vec![(said, LitState::Said), (to_say, LitState::ToSay)];
        let lit = light_with(&rows, src, &spans, |s| match s {
            LitState::Said => Style::default().fg(Color::Indexed(1)),
            LitState::ToSay => Style::default().fg(Color::Indexed(2)),
            LitState::Cut => Style::default().fg(Color::Indexed(3)),
        });
        let fg = |l: usize, s: usize| lit[l].spans[s].style.fg;
        // the mark keeps its accent
        assert_eq!(lit[0].spans[0].style.fg, Some(theme::accent()));
        assert!(lit[0].spans[1..].iter().all(|s| s.style.fg == Some(Color::Indexed(1))), "{:?}", lit[0]);
        assert_eq!(lit[1].spans[1].content, "signup, in its own worktree.");
        assert_eq!(fg(1, 1), Some(Color::Indexed(2)));
        // what is only shown keeps its look
        assert_eq!(lit[3].spans[1].style.fg, Some(theme::text()));
        assert_eq!(lit[3].spans[1].content, "the rest is detail.");
    }

    #[test]
    fn a_word_cut_in_the_middle_of_a_span() {
        let src = "so we go now";
        let rows = vec![Line::from(vec![txt("so we go now")])];
        let lit = light_with(&rows, src, &[(0..5, LitState::Said), (6..12, LitState::Cut)], |s| match s {
            LitState::Cut => Style::default().fg(Color::Indexed(3)).add_modifier(Modifier::DIM),
            _ => Style::default().fg(Color::Indexed(1)),
        });
        let parts: Vec<(&str, Option<Color>)> = lit[0].spans.iter().map(|s| (s.content.as_ref(), s.style.fg)).collect();
        assert_eq!(parts, vec![("so we ", Some(Color::Indexed(1))), ("go now", Some(Color::Indexed(3)))]);
        assert!(lit[0].spans[1].style.add_modifier.contains(Modifier::DIM));
        let _ = runs(&lit[0]);
    }

    #[test]
    fn code_and_unknown_cells_never_move_the_cursor_off() {
        // inline code is not said: its cells keep their color and the
        // prose after it still lines up
        let src = "run `cargo test` then ship";
        let code = Span::styled("cargo test", Style::default().fg(theme::syntax_string()));
        let rows = vec![Line::from(vec![txt("run "), code.clone(), txt(" then ship")])];
        let lit = light_with(&rows, src, &[(0..3, LitState::Said), (17..26, LitState::ToSay)], |s| match s {
            LitState::Said => Style::default().fg(Color::Indexed(1)),
            _ => Style::default().fg(Color::Indexed(2)),
        });
        assert_eq!(lit[0].spans[0].style.fg, Some(Color::Indexed(1)));
        assert_eq!(lit[0].spans[1], code);
        assert!(lit[0].spans.iter().any(|s| s.content.contains("then") && s.style.fg == Some(Color::Indexed(2))), "{:?}", lit[0]);
    }

    #[test]
    fn nothing_lit_nothing_changes() {
        let rows = vec![Line::from(vec![txt("hello there")])];
        assert_eq!(light(&rows, "hello there", &[]), rows);
    }
}
