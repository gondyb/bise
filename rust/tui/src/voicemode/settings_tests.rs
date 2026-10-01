//! `/voice`'s screen and the first voice mode's: keys, the words, the
//! frames at 150/95/80 columns. ▸ hear it is a step, never a sound here.

use super::*;
use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn who() -> Who {
    Who { listen: "Mistral".into(), speak: Ok("Mistral".into()), ack: "Mistral".into() }
}

fn screen(open: Open) -> Screen {
    Screen::new(open, VoiceModeConfig::default(), who(), true)
}

fn at(s: &mut Screen, row: Row) {
    s.sel = ROWS.iter().position(|r| *r == row).unwrap();
}

fn frame(s: &Screen, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now();
    t.draw(|f| draw_in(f, s, now)).unwrap();
    let buf = t.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        let row: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

#[test]
fn request_is_taken_once() {
    request(Open::Settings);
    assert_eq!(take_request(), Some(Open::Settings));
    assert_eq!(take_request(), None);
}

#[test]
fn arrows_change_a_row_and_save_it_at_once() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Save);
    assert_eq!(s.cfg.listen, ListenMode::HandsFree);
    assert_eq!(s.on_key(key(KeyCode::Left), now), Step::Save);
    assert_eq!(s.on_key(key(KeyCode::Left), now), Step::Save);
    assert_eq!(s.cfg.listen, ListenMode::Hold);
    // the row flashes
    assert!(s.flash.is_some_and(|(r, _)| r == Row::Listen));
    at(&mut s, Row::Speed);
    for _ in 0..20 {
        s.on_key(key(KeyCode::Right), now);
    }
    assert!((s.cfg.speed - SPEED_MAX).abs() < 1e-4);
    // at its end it stays, nothing to save
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Stay);
    s.on_key(key(KeyCode::Left), now);
    assert!((s.cfg.speed - 1.5).abs() < 1e-4);
    at(&mut s, Row::ReadAloud);
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.read_aloud, ReadAloud::All);
    at(&mut s, Row::Sounds);
    s.on_key(key(KeyCode::Enter), now);
    assert!(!s.cfg.sounds);
    at(&mut s, Row::Language);
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.language.as_deref(), Some("en"));
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.language.as_deref(), Some("fr"));
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.language, None);
}

#[test]
fn hear_it_only_on_enter_on_the_voice_row() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    // moving over the voice row never plays anything
    at(&mut s, Row::Listen);
    assert_eq!(s.on_key(key(KeyCode::Down), now), Step::Stay);
    assert_eq!(s.row(), Row::Voice);
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Stay);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Hear);
    // a provider that can't speak: the reason, no sound
    s.who.speak = Err("OpenAI can't speak yet".into());
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Stay);
    assert_eq!(s.said.as_deref(), Some("OpenAI can't speak yet"));
}

#[test]
fn dictation_and_who_hears_you_from_the_settings() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    at(&mut s, Row::Dictation);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Dictation);
    at(&mut s, Row::Who);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Stay);
    assert!(s.reading);
    let f = frame(&s, 95, 40);
    assert!(f.contains("who hears you") && f.contains("esc back") && !f.contains("1 start voice mode"), "{f}");
    // read only: digits do nothing, esc goes back to the list
    assert_eq!(s.on_key(key(KeyCode::Char('1')), now), Step::Stay);
    s.on_key(key(KeyCode::Esc), now);
    assert!(!s.reading);
    assert_eq!(s.on_key(key(KeyCode::Esc), now), Step::Done(Out::Closed));
}

#[test]
fn the_first_voice_mode_asks_start_hold_or_not_now() {
    let now = Instant::now();
    let mut s = screen(Open::Privacy);
    assert_eq!(s.on_key(key(KeyCode::Char('1')), now), Step::Done(Out::Start));
    assert_eq!(s.on_key(key(KeyCode::Char('2')), now), Step::Done(Out::HoldOnly));
    assert_eq!(s.on_key(key(KeyCode::Char('3')), now), Step::Done(Out::NotNow));
    assert_eq!(s.on_key(key(KeyCode::Esc), now), Step::Done(Out::NotNow));
    // the cursor starts on start; ⏎ picks what is under it
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Done(Out::Start));
    s.on_key(key(KeyCode::Down), now);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Done(Out::HoldOnly));
    s.on_key(key(KeyCode::Up), now);
    s.on_key(key(KeyCode::Up), now);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Done(Out::NotNow));
    assert_eq!(s.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), now), Step::Done(Out::NotNow));
}

#[test]
fn the_privacy_screen_says_who_hears_what_is_kept_and_when() {
    let s = screen(Open::Privacy);
    let f = frame(&s, 95, 40);
    for want in [
        "who hears you",
        "Mistral turns your voice into words, and the answers into a voice.",
        "a small Mistral model reads your words to say a quick \"on it\".",
        "then your words go to the agent's model, like a typed message.",
        "what is kept",
        "the words, in the thread. never the audio.",
        "when it listens",
        "ctrl+r twice to esc",
        "1 start voice mode",
        "2 hold-to-talk only",
        "3 not now",
    ] {
        assert!(f.contains(want), "{want}:\n{f}");
    }
    // two companies: both named
    let mut s = screen(Open::Privacy);
    s.who = Who { listen: "OpenAI".into(), speak: Err("OpenAI can't speak yet".into()), ack: "Anthropic".into() };
    let f = frame(&s, 95, 40);
    assert!(f.contains("OpenAI turns your voice into words.") && f.contains("nobody says the answers") && f.contains("a small Anthropic model reads"), "{f}");
    // two companies, both speaking: one line each
    s.who = Who { listen: "Groq".into(), speak: Ok("Mistral".into()), ack: "Mistral".into() };
    let f = frame(&s, 95, 40);
    assert!(f.contains("Groq turns your voice into words.") && f.contains("Mistral turns the answers into a voice."), "{f}");
}

#[test]
fn the_voice_row_says_a_name_never_an_id() {
    assert_eq!(voice_name("en_paul_neutral"), "Paul, neutral");
    assert_eq!(voice_name(""), "Paul, neutral", "the default voice");
    assert_eq!(voice_name("fr_marie_warm_slow"), "Marie, warm slow");
    assert_eq!(voice_name("nova"), "Nova");
    let mut s = screen(Open::Settings);
    at(&mut s, Row::Voice);
    let f = frame(&s, 95, 40);
    assert!(f.contains("Mistral · Paul, neutral") && !f.contains("en_paul"), "{f}");
    // the help line: one colon at most
    at(&mut s, Row::Listen);
    let f = frame(&s, 150, 40);
    let one: String = f.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(one.contains("auto: hands-free with headphones. on speakers you hold space, or it would hear itself."), "{f}");
    assert!(!f.contains("listen: auto:"), "{f}");
}

#[test]
fn the_companies_on_the_who_row() {
    assert_eq!(who().companies(), "Mistral");
    let w = Who { listen: "Mistral".into(), speak: Ok("Mistral".into()), ack: "OpenAI".into() };
    assert_eq!(w.companies(), "Mistral and OpenAI");
    let w = Who { listen: "Groq".into(), speak: Ok("Mistral".into()), ack: "OpenAI".into() };
    assert_eq!(w.companies(), "Groq, Mistral and OpenAI");
}

#[test]
fn the_settings_fit_150_95_and_80_columns() {
    let mut s = screen(Open::Settings);
    at(&mut s, Row::Voice);
    for (w, h) in [(150, 40), (95, 40), (80, 29)] {
        let f = frame(&s, w, h);
        for want in ["voice mode", "listen", "voice", "speed", "1.0×", "read aloud", "sounds", "language", "who hears you", "dictation", "hear it", "esc back"] {
            assert!(f.contains(want), "{w}×{h} {want}:\n{f}");
        }
        // no row wraps: one line each
        assert_eq!(f.lines().filter(|l| l.contains("hear it")).count(), 2, "{w}×{h}:\n{f}");
        assert!(f.lines().all(|l| l.chars().count() <= w as usize));
    }
}

#[test]
fn the_flash_and_the_selection_marks() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    s.on_key(key(KeyCode::Right), now);
    let lines = s.lines(80, now);
    let row = lines.iter().map(|l| l.to_string()).find(|l| l.contains("listen")).unwrap();
    assert!(row.starts_with('✓') || row.starts_with('v'), "{row}");
    let later = s.lines(80, now + Duration::from_secs(2));
    let row = later.iter().map(|l| l.to_string()).find(|l| l.contains("listen")).unwrap();
    assert!(row.starts_with('›') || row.starts_with('>'), "{row}");
}
