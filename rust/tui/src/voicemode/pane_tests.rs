//! The pane on a buffer (plan §5): 150/95/80 columns × 40 and 29 rows,
//! dark/light, NO_COLOR, ASCII, from hand-built views.

use super::*;
use crate::theme::{self, Mode};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

const MOVING: Form = Form { still: false, no_color: false, ascii: false };
const STILL: Form = Form { still: true, no_color: false, ascii: false };
const NO_COLOR: Form = Form { still: false, no_color: true, ascii: false };
const ASCII: Form = Form { still: false, no_color: false, ascii: true };

fn words(s: &str, st: WordState) -> Vec<(String, WordState)> {
    s.split(' ').map(|w| (w.to_string(), st)).collect()
}

fn view(phase: Phase) -> PaneView {
    let (who, w) = match phase {
        Phase::Speaking | Phase::CutIn => {
            let mut w = words("on it. perf takes the", WordState::Said);
            w.extend(words("signup, in its own worktree.", WordState::ToSay));
            (Who::Agent("main".into()), w)
        }
        _ => {
            let mut w = words("the signup is slow on mobile. can you", WordState::Heard);
            w.extend(words("have a", WordState::Partial));
            (Who::You, w)
        }
    };
    PaneView {
        phase,
        agent: "main".into(),
        who,
        words: w,
        you_level: 0.6,
        agent_level: 0.8,
        you_wave: (0..30).map(|i| ((i * 7) % 10) as f32 / 10.0).collect(),
        agent_wave: (0..60).map(|i| ((i * 3) % 10) as f32 / 10.0).collect(),
        elapsed: Duration::from_secs(134),
        route: Route::Headphones,
        heard_answer: None,
        work: Vec::new(),
    }
}

/// The pane's rows as text, trailing blanks cut.
fn draw_text(v: &PaneView, w: u16, h: u16, t: u64, form: Form) -> (Vec<String>, Buffer) {
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    draw_in(&mut buf, area, v, t, form);
    let rows = (0..h)
        .map(|y| {
            let mut s = String::new();
            let mut x = 0;
            while x < w {
                let c = &buf[(x, y)];
                s.push_str(c.symbol());
                x += c.symbol().width().max(1) as u16;
            }
            s.trim_end().to_string()
        })
        .collect();
    (rows, buf)
}

fn find(rows: &[String], needle: &str) -> Option<(usize, usize)> {
    rows.iter().enumerate().find_map(|(y, r)| r.find(needle).map(|b| (y, r[..b].width())))
}

#[test]
fn half_the_screen_from_30_rows_the_lanes_under() {
    assert_eq!(height(40), 19, "with the key bar: 20 of 40");
    assert_eq!(height(30), 14);
    assert_eq!(height(29), LANES_H);
    assert_eq!(height(12), LANES_H);
}

#[test]
fn the_big_pane_kiss_left_captions_right_status_under() {
    for w in [150u16, 95, 80] {
        let pane_w = w - 2; // inside the frame
        let (rows, _) = draw_text(&view(Phase::Listening), pane_w, height(40), 0, MOVING);
        assert!(rows.iter().all(|r| r.starts_with('│')), "the bar on every row at {w}");
        let (y, x) = find(&rows, "you").expect("who talks");
        assert_eq!(x as u16, CAPS_X, "captions column at {w}");
        // the captions wrap at 28 cells, under the name after one blank row
        let (cy, _) = find(&rows, "the signup is slow").expect("captions");
        assert_eq!(cy, y + 2);
        for r in &rows[cy..cy + 2] {
            assert!(r.width() <= CAPS_X as usize + CAPTION_W, "{r:?}");
        }
        let (sy, sx) = find(&rows, "● listening").expect("status row");
        assert_eq!((sy, sx as u16), (y - 1 + kiss::H as usize, CAPS_X));
        // the kiss's colon, 4 cells in from the bar + 6 dots
        assert!(rows[y + 1].chars().skip(KISS_X as usize + 6).take(4).all(|c| c == '█'), "{:?}", rows[y + 1]);
    }
}

#[test]
fn at_80_columns_everything_fits() {
    // the pane is 78 wide inside the frame: 48 + 28 cells of captions + 2
    let mut v = view(Phase::Speaking);
    v.words = words("cookies asks: the banner, smaller or gone? and one more thing to say here", WordState::Said);
    let (rows, _) = draw_text(&v, 78, 19, 0, MOVING);
    assert!(rows.iter().all(|r| r.width() <= 77), "{rows:#?}");
    assert!(find(&rows, ":* main").is_some());
}

#[test]
fn the_status_row_of_each_phase() {
    let cases: Vec<(Phase, &str)> = vec![
        (Phase::Listening, "● listening ▄▁▇▅▂█▅▃"),
        (Phase::Hearing, "● listening"),
        (Phase::AboutToAnswer { fill: 0.6 }, "about to answer ●●●··"),
        (Phase::AboutToAnswer { fill: 1.0 }, "about to answer ●●●●●"),
        (Phase::Holding, "● the floor is yours"),
        (Phase::Working, "∿ main is on it"),
        (Phase::Speaking, "speaking"),
        (Phase::CutIn, "● you cut in"),
        (Phase::Muted, "○ muted"),
        (Phase::HoldToTalk, "hold space to talk"),
        (Phase::Failed("voice: no key for mistral".into()), "voice: no key for mistral"),
    ];
    for (p, want) in cases {
        let (rows, _) = draw_text(&view(p.clone()), 98, 19, 0, MOVING);
        assert!(find(&rows, want).is_some(), "{p:?}: {want:?} in {rows:#?}");
    }
    // on speakers the agent's voice is cut by holding space
    let mut v = view(Phase::Speaking);
    v.route = Route::Speakers;
    let (rows, _) = draw_text(&v, 98, 19, 0, MOVING);
    assert!(find(&rows, "hold space to cut in").is_some());
    // a heard answer wins for its 1.5 s
    let mut v = view(Phase::Listening);
    v.heard_answer = Some("heard \"the first one\" → 1 smaller".into());
    let (rows, _) = draw_text(&v, 98, 19, 0, MOVING);
    assert!(find(&rows, "heard \"the first one\" → 1 smaller").is_some());
}

#[test]
fn while_it_works_the_pane_points_at_the_thread() {
    let (rows, _) = draw_text(&view(Phase::Working), 98, 19, 0, MOVING);
    // designer: who it is on the title row, on it on the status row, once
    let (y, x) = find(&rows, ":* main").expect("the title row");
    assert_eq!(x as u16, CAPS_X);
    assert_eq!(rows.iter().filter(|r| r.contains("main is on it")).count(), 1);
    assert!(rows[y - 1 + kiss::H as usize].ends_with("∿ main is on it"));
    assert!(find(&rows, "what it does shows in the").is_some());
    assert!(find(&rows, "the signup is slow").is_none(), "no captions while it works");
}

#[test]
fn the_word_being_said_is_underlined_the_rest_dim() {
    let v = view(Phase::Speaking);
    let lines = captions(&v.words, CAPTION_W, MOVING);
    let spans: Vec<&Span> = lines.iter().flat_map(|l| l.spans.iter()).filter(|s| !s.content.trim().is_empty()).collect();
    let the = spans.iter().find(|s| s.content == "the").unwrap();
    assert!(the.style.add_modifier.contains(Modifier::UNDERLINED));
    let on = spans.iter().find(|s| s.content == "on").unwrap();
    assert!(!on.style.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(on.style.fg, Some(theme::text()));
    let signup = spans.iter().find(|s| s.content == "signup,").unwrap();
    assert_eq!(signup.style.fg, Some(theme::dim()));
    // all said: nothing underlined
    let done: Vec<_> = v.words.iter().map(|(w, _)| (w.clone(), WordState::Said)).collect();
    assert!(captions(&done, CAPTION_W, MOVING).iter().flat_map(|l| l.spans.iter()).all(|s| !s.style.add_modifier.contains(Modifier::UNDERLINED)));
    // cut words: faint
    let cut: Vec<_> = vec![("so".to_string(), WordState::Said), ("then".to_string(), WordState::Cut)];
    let l = captions(&cut, CAPTION_W, MOVING);
    assert_eq!(l[0].spans.last().unwrap().style.fg, Some(theme::faint()));
}

#[test]
fn captions_wrap_at_28_cells_and_cut_a_longer_word() {
    let w = words("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa b c", WordState::Heard);
    let lines = captions(&w, CAPTION_W, MOVING);
    assert_eq!(lines[0].width(), 28);
    assert_eq!(lines[1].to_string(), "b c");
    // the words carry their leading space (the listener's): trimmed
    let w = vec![(" hello".to_string(), WordState::Heard), (" there".to_string(), WordState::Partial)];
    assert_eq!(captions(&w, CAPTION_W, MOVING)[0].to_string(), "hello there");
}

#[test]
fn only_the_last_caption_lines_that_fit() {
    let mut v = view(Phase::Hearing);
    v.words = words(&"word ".repeat(60), WordState::Heard);
    v.words.push(("last".into(), WordState::Partial));
    let (rows, _) = draw_text(&v, 98, 19, 0, MOVING);
    let n = rows.iter().filter(|r| r.contains("word") || r.contains("last")).count();
    assert_eq!(n, CAPTION_LINES);
    assert!(find(&rows, "last").is_some(), "the newest words stay");
}

#[test]
fn under_30_rows_two_lanes_you_and_the_agent() {
    for (w, n) in [(148u16, 52usize), (93, 40), (78, 40)] {
        let (rows, _) = draw_text(&view(Phase::Speaking), w, LANES_H, 0, MOVING);
        assert_eq!(rows[0], "│");
        assert!(rows[1].starts_with("│   you  "), "{:?}", rows[1]);
        assert!(rows[2].starts_with("│   :o   "), "the mouth opens: {:?}", rows[2]);
        assert!(rows[2].contains("))) speaking"));
        let wave: String = rows[2].chars().skip(9).take(n).collect();
        assert!(wave.chars().all(|c| crate::voice::PEAK_BLOCKS.contains(&c)), "{wave:?}");
        assert!(rows.iter().all(|r| r.width() <= w as usize));
    }
    let (rows, _) = draw_text(&view(Phase::Listening), 93, LANES_H, 0, MOVING);
    assert!(rows[1].ends_with("● listening"), "{:?}", rows[1]);
    assert!(rows[2].starts_with("│   :*   "));
    let (rows, _) = draw_text(&view(Phase::Working), 93, LANES_H, 0, MOVING);
    assert!(rows[2].ends_with("∿ main is on it"), "{:?}", rows[2]);
    let (rows, _) = draw_text(&view(Phase::AboutToAnswer { fill: 0.4 }), 93, LANES_H, 0, MOVING);
    assert!(rows[1].ends_with("about to answer ●●···"), "{:?}", rows[1]);
    // another agent: its name instead of the mouth, the waves in place
    let mut v = view(Phase::Speaking);
    v.agent = "cookies".into();
    let (rows, _) = draw_text(&v, 93, LANES_H, 0, MOVING);
    assert!(rows[2].starts_with("│   cookies "), "{:?}", rows[2]);
}

#[test]
fn a_narrow_pane_falls_back_to_the_lanes() {
    let (rows, _) = draw_text(&view(Phase::Listening), BIG_MIN_W - 1, 19, 0, MOVING);
    assert!(!rows.iter().any(|r| r.contains("▀▀▀▀")), "no kiss");
    assert!(rows[17].starts_with("│   you"), "{rows:#?}");
}

#[test]
fn reduce_motion_still_frames() {
    for p in [Phase::Listening, Phase::Working, Phase::Speaking, Phase::CutIn] {
        let a = draw_text(&view(p.clone()), 98, 19, 0, STILL).1;
        for t in [300, 650, 1200, 4321] {
            assert_eq!(a, draw_text(&view(p.clone()), 98, 19, t, STILL).1, "{p:?} at {t}");
        }
    }
    // moving, the blink and the breath change the cells
    let a = draw_text(&view(Phase::Listening), 98, 19, 0, MOVING).1;
    assert_ne!(a, draw_text(&view(Phase::Listening), 98, 19, BLINK_MS, MOVING).1);
}

#[test]
fn the_bar_breathes_accent_to_rule_while_it_listens() {
    theme::set_mode(Mode::Dark);
    let at = |t| draw_text(&view(Phase::Listening), 98, 19, t, MOVING).1[(0, 0)].fg;
    assert_eq!(at(0), theme::accent());
    assert_ne!(at(BREATH_MS / 2), theme::accent());
    assert_eq!(at(BREATH_MS), theme::accent());
    let speaking = draw_text(&view(Phase::Speaking), 98, 19, BREATH_MS / 2, MOVING).1;
    assert_eq!(speaking[(0, 0)].fg, theme::accent(), "accent while the agent talks");
    let muted = draw_text(&view(Phase::Muted), 98, 19, 0, MOVING).1;
    assert_eq!(muted[(0, 0)].fg, theme::rule());
}

#[test]
fn no_color_bold_and_dim_carry_it() {
    let (_, b) = draw_text(&view(Phase::AboutToAnswer { fill: 0.6 }), 98, 19, 0, NO_COLOR);
    let (rows, _) = draw_text(&view(Phase::AboutToAnswer { fill: 0.6 }), 98, 19, 0, NO_COLOR);
    let (y, x) = find(&rows, "●●●").unwrap();
    assert!(b[(x as u16, y as u16)].modifier.contains(Modifier::BOLD));
    assert!(b[(x as u16 + 3, y as u16)].modifier.contains(Modifier::DIM), "the dots to come");
    // the bar's breath: bold, then plain
    let bar = |t| draw_text(&view(Phase::Listening), 98, 19, t, NO_COLOR).1[(0, 0)].modifier;
    assert!(bar(0).contains(Modifier::BOLD));
    assert!(!bar(BREATH_MS / 2).contains(Modifier::BOLD));
    // the blink: bold on/off
    let blink = |t| {
        let (rows, b) = draw_text(&view(Phase::Listening), 98, 19, t, NO_COLOR);
        let (y, x) = find(&rows, "● listening").unwrap();
        b[(x as u16, y as u16)].modifier
    };
    assert!(blink(0).contains(Modifier::BOLD));
    assert!(!blink(BLINK_MS).contains(Modifier::BOLD));
}

#[test]
fn ascii_waves() {
    let (rows, _) = draw_text(&view(Phase::Speaking), 93, LANES_H, 0, ASCII);
    let cells: String = rows[2].chars().skip(9).take(40).collect();
    assert!(cells.chars().all(|c| "_.-=#".contains(c)), "{cells:?}");
    assert_eq!(wave(&[0.0, 0.5, 1.0], 5, true), "___=#");
}

#[test]
fn the_wave_keeps_the_newest_on_the_right() {
    assert_eq!(wave(&[0.0, 0.3, 0.9], 5, false), "▁▁▁▃█");
    let many: Vec<f32> = (0..60).map(|i| if i == 59 { 1.0 } else { 0.0 }).collect();
    let w = wave(&many, 52, false);
    assert_eq!(w.chars().count(), 52);
    assert!(w.ends_with('█'));
}

#[test]
fn light_theme_uses_the_light_palette() {
    theme::set_mode(Mode::Light);
    let (rows, b) = draw_text(&view(Phase::Listening), 98, 19, 0, MOVING);
    let (y, x) = find(&rows, "you").unwrap();
    assert_eq!(b[(x as u16, y as u16)].fg, theme::palette_of(Mode::Light).accent);
    theme::set_mode(Mode::Dark);
}

#[test]
fn the_header_the_divider_and_the_keys() {
    let text = |s: Vec<Span>| s.iter().map(|s| s.content.to_string()).collect::<String>();
    let v = view(Phase::Listening);
    assert_eq!(text(header(&v)), "● voice mode 2:14");
    assert_eq!(header(&v)[0].style.fg, Some(theme::accent()));
    let mut m = view(Phase::Muted);
    m.elapsed = Duration::from_secs(61);
    assert_eq!(text(header(&m)), "○ voice mode 1:01");
    assert_eq!(header(&m)[0].style.fg, Some(theme::faint()));
    assert_eq!(text(divider(&v)), " you ⇄ main · voice mode · headphones ");
    let mut s = view(Phase::Listening);
    s.route = Route::Speakers;
    s.agent = "cookies".into();
    assert_eq!(text(divider(&s)), " you ⇄ cookies · voice mode · speakers ");
    s.route = Route::Unknown;
    assert_eq!(text(divider(&s)), " you ⇄ cookies · voice mode ");
    assert_eq!(keys(&v, 120).to_string(), " esc leave   m mute   space send now · hold: keep the floor   tab type");
    assert_eq!(keys(&v, 78).to_string(), " esc leave   m mute   space send · hold");
    assert_eq!(keys(&view(Phase::AboutToAnswer { fill: 0.2 }), 120).to_string(), " keep talking, or   space send now   hold space i'm thinking");
    assert_eq!(keys(&view(Phase::Muted), 120).to_string(), " m unmute   esc leave");
    assert!(keys(&v, 78).width() <= 76);
}

#[test]
fn lit_styles() {
    assert_eq!(lit_style(LitState::Said).fg, Some(theme::text()));
    assert_eq!(lit_style(LitState::ToSay).fg, Some(theme::dim()));
    assert_eq!(lit_style(LitState::Cut).fg, Some(theme::faint()));
}
