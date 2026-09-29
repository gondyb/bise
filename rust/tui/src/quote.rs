//! Ask about this (BISE-134, book §13 "Ask about a selection"): text
//! selected in the history becomes context for the agent.
//!
//! Select text in the history (the release copies it, as before), then
//! type: the first typed key puts the selection in the composer as a
//! quote chip `❝ 1` at the composer's cursor, like an image (BISE-207),
//! and ends the selection; the key then types right after the chip. The strip above the composer lists
//! it: `❝ 1 “first words of the selection…”  main · 3 lines`; a
//! backspace on the chip removes it, like an image. Several selections,
//! several quotes: at most [`MAX`].
//!
//! On send, each quote still in the text leaves it and goes in front of
//! the message as one tag, the text after them:
//!
//! ```text
//! <selection from="main">
//! the selected text
//! </selection>
//! what does this mean?
//! ```
//!
//! `from`: who wrote the selected lines (the agent in view, `you`, the
//! agent a message came from), several joined with `, `. The history
//! draws each tag as one dim line over your text: `❝ the selected
//! text… · main · 3 lines`.
//!
//! A quote is an [`Attachment`](crate::attach::Attachment): its label
//! is `[Quote #N]`, its marker the tag. So the drafts file, the queue
//! and the per-view drafts keep it like an image, with no new state.

use crate::app::App;
use crate::attach::Attachment;

/// Quotes in one message, at most.
pub(crate) const MAX: usize = 4;
/// A quote keeps at most this many characters of the selection.
pub(crate) const MAX_CHARS: usize = 8_000;
/// How the label of a quote starts (`[Quote #2]`).
pub(crate) const OPEN: &str = "[Quote #";

const TAG_OPEN: &str = "<selection from=\"";
const TAG_CLOSE: &str = "</selection>";

/// The label of quote `n`.
pub(crate) fn label(n: usize) -> String {
    format!("{OPEN}{n}]")
}

pub(crate) fn is_quote(label: &str) -> bool {
    label.starts_with(OPEN)
}

/// The tag a quote sends: `<selection from="main">\n{text}\n</selection>`.
/// A `</selection>` inside the text is broken (`</selection >`) so the
/// tag always ends where it should; a `"` in `from` becomes `'`.
pub(crate) fn tag(from: &str, text: &str) -> String {
    let from = from.replace('"', "'");
    let text = text.replace(TAG_CLOSE, "</selection >");
    format!("{TAG_OPEN}{from}\">\n{text}\n{TAG_CLOSE}")
}

/// A quote read back from its tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quote {
    pub(crate) from: String,
    pub(crate) text: String,
}

/// The tag at the start of `s` (spaces and newlines before it allowed):
/// the quote and the rest after it.
fn parse_one(s: &str) -> Option<(Quote, &str)> {
    let s = s.trim_start();
    let rest = s.strip_prefix(TAG_OPEN)?;
    let (from, rest) = rest.split_once("\">")?;
    if from.contains('\n') {
        return None;
    }
    let (text, rest) = rest.split_once(TAG_CLOSE)?;
    let text = text.strip_prefix('\n').unwrap_or(text);
    let text = text.strip_suffix('\n').unwrap_or(text);
    Some((Quote { from: from.to_string(), text: text.to_string() }, rest))
}

/// The tags in front of a message, and its text after them (the newline
/// after the last tag dropped). No tag: none, the whole message.
pub(crate) fn split(msg: &str) -> (Vec<Quote>, &str) {
    let mut out = Vec::new();
    let mut rest = msg;
    while let Some((q, r)) = parse_one(rest) {
        out.push(q);
        rest = r;
    }
    if out.is_empty() {
        return (out, msg);
    }
    (out, rest.strip_prefix('\n').unwrap_or(rest))
}

/// Lines in `text` (a last empty line not counted; at least 1).
pub(crate) fn line_count(text: &str) -> usize {
    text.trim_end_matches('\n').split('\n').count().max(1)
}

/// The start of the quote on one line: its words, newlines as spaces,
/// at most `max` columns (`…` when cut).
pub(crate) fn preview(text: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.width() <= max {
        return flat;
    }
    let mut out = String::new();
    let mut used = 1; // the `…`
    for ch in flat.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > max {
            break;
        }
        used += w;
        out.push(ch);
    }
    format!("{}…", out.trim_end())
}

/// What the strip and the history say after the words: `main · 3 lines`.
pub(crate) fn about(q: &Quote) -> String {
    let n = line_count(&q.text);
    format!("{} · {} line{}", q.from, n, if n == 1 { "" } else { "s" })
}

/// The quote an attachment holds (a quote's marker is its tag).
pub(crate) fn of(a: &Attachment) -> Option<Quote> {
    if !is_quote(&a.label) {
        return None;
    }
    parse_one(&a.marker).map(|(q, _)| q)
}

/// The quote chips in `text` (first char, past it, N).
pub(crate) fn chips(text: &str) -> Vec<(usize, usize, usize)> {
    crate::attach::find_labels(text, OPEN)
}

/// The selected text as a quote in the composer: its chip at the cursor
/// ([`crate::attach::insert_chip`]), one undo step. `Err`: why not (a
/// flash). The text is trimmed and cut to [`MAX_CHARS`].
pub(crate) fn add(app: &mut App, from: &str, text: &str) -> Result<String, String> {
    let text = text.trim_matches('\n').trim_end();
    if text.trim().is_empty() {
        return Err("nothing selected".into());
    }
    if chips(&app.ed.text).len() >= MAX {
        return Err(format!("{MAX} quotes at most: backspace on one to make room"));
    }
    let text: String = match text.char_indices().nth(MAX_CHARS) {
        Some((b, _)) => format!("{}…", &text[..b]),
        None => text.to_string(),
    };
    // forget the quotes whose chip left the text; the lowest free number
    let t = app.ed.text.clone();
    app.attachments.retain(|a| !is_quote(&a.label) || t.contains(&a.label));
    let n = (1..).find(|n| !app.attachments.iter().any(|a| a.label == label(*n))).unwrap_or(1);
    let l = label(n);
    app.attachments.push(Attachment { label: l.clone(), marker: tag(from, &text), info: Default::default() });
    crate::attach::insert_chip(&mut app.ed, &l);
    Ok(crate::attach::chip_name(&l))
}

/// Who wrote the events `from..=to` of the feed in view: `you`, the
/// sender of an agent message, else the agent in view; in order, once
/// each, joined with `, `.
pub(crate) fn speakers(app: &App, from: usize, to: usize) -> String {
    use crate::wire::Ev;
    let mut who: Vec<String> = Vec::new();
    for ev in app.events.iter().take(to.saturating_add(1)).skip(from) {
        let w = match ev {
            Ev::You(..) | Ev::Undelivered { .. } => "you".to_string(),
            Ev::AgentMsg { from, .. } if !from.is_empty() => from.trim_start_matches('@').to_string(),
            Ev::TimeMark(_) | Ev::Idle | Ev::Turn | Ev::TurnDone | Ev::Usage(_) => continue,
            _ => app.sb.focus_name().to_string(),
        };
        if !who.contains(&w) {
            who.push(w);
        }
    }
    if who.is_empty() {
        who.push(app.sb.focus_name().to_string());
    }
    who.join(", ")
}

/// A typed key with a selection in the history: the selection becomes a
/// quote (and ends). None without a selection.
pub(crate) fn take_selection(app: &mut App) -> Option<Result<String, String>> {
    let sel = app.feed_sel?;
    if app.mouse.drag.is_some() {
        return None;
    }
    let text = crate::input::feed_selection_text(app).unwrap_or_default();
    app.feed_sel = None;
    let ((e0, _, _), (e1, _, _)) = sel.range();
    let from = speakers(app, e0, e1);
    Some(add(app, &from, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_reads_back() {
        let t = tag("main", "fn a() {}\n  b");
        assert_eq!(t, "<selection from=\"main\">\nfn a() {}\n  b\n</selection>");
        let msg = format!("{t}\n{}\nwhy?", tag("you, docs", "x </selection> y"));
        let (qs, rest) = split(&msg);
        assert_eq!(rest, "why?");
        assert_eq!(qs[0], Quote { from: "main".into(), text: "fn a() {}\n  b".into() });
        assert_eq!(qs[1], Quote { from: "you, docs".into(), text: "x </selection > y".into() });
        assert_eq!(split("hello <selection from=\"x\">"), (vec![], "hello <selection from=\"x\">"));
        assert_eq!(split("<selection from=\"x\">\nno end"), (vec![], "<selection from=\"x\">\nno end"));
    }

    use crate::wire::Ev;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(app: &mut App, c: KeyCode) {
        crate::input::on_key(app, &KeyEvent::new(c, KeyModifiers::NONE));
    }

    /// main's feed: an answer, the whole of its first row selected.
    fn selected() -> App {
        let mut app = crate::sb::bench::test_app_drained();
        app.sb.focus = "main".into();
        app.events = vec![Ev::Assistant("the login breaks on safari".into())];
        app.cache = vec![None];
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        app
    }

    /// Typing with a selection: the chip first, then the key; the
    /// selection ends; the strip lists the quote; the key bar says so
    /// before.
    #[test]
    fn typing_with_a_selection_quotes_it() {
        let mut app = selected();
        assert_eq!(crate::keybar::mode(&app), crate::keybar::Mode::Quote);
        press(&mut app, KeyCode::Char('w'));
        assert_eq!(app.ed.text, "[Quote #1] w");
        assert_eq!(app.ed.cursor, 12);
        assert!(app.feed_sel.is_none(), "the selection is taken");
        let q = of(&app.attachments[0]).unwrap();
        assert_eq!(q.from, "main");
        assert!(q.text.contains("the login breaks on safari"), "{q:?}");
        assert_eq!(crate::attach::strip_height(&app), 2);
        let rows: Vec<String> = crate::attach::strip_lines(&app, 70)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(rows[1].starts_with(" ❝ 1  “") && rows[1].ends_with("main · 1 line"), "{rows:?}");
        // the next key just types; a second selection is quote 2, at the
        // cursor
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.ed.text, "[Quote #1] wh");
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.ed.text, "[Quote #1] wh [Quote #2] y");
        assert_eq!(app.ed.cursor, app.ed.text.chars().count());
        // no selection: no quote mode
        assert_ne!(crate::keybar::mode(&app), crate::keybar::Mode::Quote);
    }

    /// BISE-207: the chip goes at the cursor (a space before it when it
    /// would touch a word or a chip, one after), the key right after it;
    /// one undo step takes the key, the next the chip.
    #[test]
    fn the_quote_goes_at_the_cursor() {
        let cases = [
            ("hello", 0, "[Quote #1] whello", 11),
            ("hello", 2, "he [Quote #1] wllo", 14),
            ("hello", 5, "hello [Quote #1] w", 17),
            ("see [Quote #1]", 14, "see [Quote #1] [Quote #2] w", 26),
            ("a [Image #1] b", 12, "a [Image #1] [Quote #1] w b", 24),
        ];
        for (text, cursor, want, at) in cases {
            let mut app = selected();
            if text.contains("[Quote #1]") {
                app.attachments.push(Attachment { label: label(1), marker: tag("you", "q"), info: Default::default() });
            }
            app.ed.set(text, cursor);
            press(&mut app, KeyCode::Char('w'));
            assert_eq!(app.ed.text, want, "{text:?} at {cursor}");
            assert_eq!(app.ed.cursor, at + 1, "{text:?} at {cursor}: the key lands after the chip");
            app.ed.undo();
            app.ed.undo();
            assert_eq!(app.ed.text, text, "{text:?}: two undo steps");
        }
    }

    /// A backspace on the chip removes the quote; at most MAX.
    #[test]
    fn a_quote_is_removed_and_capped() {
        let mut app = selected();
        press(&mut app, KeyCode::Char('x'));
        app.ed.cursor = 10; // right after `[Quote #1]`
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.ed.text, " x");
        assert_eq!(crate::attach::strip_height(&app), 0);
        assert_eq!(crate::attach::expand(&mut app, " x"), " x");
        for _ in 0..MAX {
            app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
            press(&mut app, KeyCode::Char('a'));
        }
        assert_eq!(chips(&app.ed.text).len(), MAX);
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(chips(&app.ed.text).len(), MAX);
        assert!(app.flash.as_ref().is_some_and(|(f, _)| f.contains("at most")));
        assert_eq!(app.ed.text.matches('b').count(), 1, "the key still types");
    }

    /// ⏎: the tags first, one per quote, then the text; the history
    /// draws a dim quote line over your words.
    #[test]
    fn the_sent_message_carries_the_tag() {
        let mut app = selected();
        for c in "why?".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        let sent = app.history[0].clone();
        let (qs, rest) = split(&sent);
        assert!(sent.starts_with("<selection from=\"main\">\n"), "{sent}");
        assert!(sent.contains("\n</selection>\nwhy?"), "{sent}");
        assert_eq!(rest, "why?");
        assert_eq!(qs.len(), 1);
        assert!(app.attachments.is_empty());
        let rows: Vec<String> = crate::render::user_block_lines(&sent, crate::wire::Mark::Sent, 60)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows[0].contains("❝ ") && rows[0].contains("safari · main · 1 line"), "{rows:?}");
        assert!(rows[1].contains("why?"), "{rows:?}");
        // a quote in the middle of the text still goes first, its label gone
        app.attachments.push(Attachment { label: label(1), marker: tag("you", "q"), info: Default::default() });
        assert_eq!(
            crate::attach::expand(&mut app, "look [Quote #1] here"),
            "<selection from=\"you\">\nq\n</selection>\nlook here"
        );
    }

    #[test]
    fn speakers_name_who_wrote_the_lines() {
        let mut app = crate::sb::bench::test_app();
        app.sb.focus = "docs".into();
        app.events = vec![
            Ev::You("hi".into(), crate::wire::Mark::Read),
            Ev::Assistant("a".into()),
            Ev::TimeMark("14:02".into()),
            Ev::Assistant("b".into()),
        ];
        assert_eq!(speakers(&app, 0, 3), "you, docs");
        assert_eq!(speakers(&app, 1, 3), "docs");
    }

    #[test]
    fn preview_and_about() {
        assert_eq!(preview("the login\n  breaks on safari", 40), "the login breaks on safari");
        assert_eq!(preview("the login breaks on safari", 12), "the login b…");
        let q = Quote { from: "main".into(), text: "a\nb\nc\n".into() };
        assert_eq!(about(&q), "main · 3 lines");
        assert_eq!(about(&Quote { from: "you".into(), text: "a".into() }), "you · 1 line");
    }
}
