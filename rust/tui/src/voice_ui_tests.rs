//! Speech-to-text in the composer: keys, live text, visuals. Fake
//! microphone and transcription client (voice::fakes): no device, no
//! network.

use super::*;
use crossterm::event::KeyEvent;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use voice::fakes::{FakeRecorder, FakeTranscriber};
use voice::{TranscribeEvent, Voice, VoiceState};

fn app_with_voice(enabled: bool) -> (App, FakeRecorder, FakeTranscriber) {
    let mut app = sb::bench::test_app();
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    app.voice = Voice::new(enabled, Box::new(rec.clone()), Box::new(tr.clone()));
    (app, rec, tr)
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) -> bool {
    voice_key(app, &KeyEvent::new(code, m), || Ok(voice::fakes::job()))
}

fn ctrl_r(app: &mut App) -> bool {
    press(app, KeyCode::Char('r'), KeyModifiers::CONTROL)
}

/// BISE-130: the clip is transcribed once stopped; its text lands at
/// the cursor, a space after a word.
#[test]
fn the_text_lands_at_the_cursor_once_transcribed() {
    let (mut app, _rec, tr) = app_with_voice(true);
    app.ed.text = "Note:".into();
    app.ed.cursor = 5;
    assert!(ctrl_r(&mut app));
    assert_eq!(app.voice.state(), VoiceState::Recording);
    assert!(press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.voice.state(), VoiceState::Flushing);
    tr.send(TranscribeEvent::Delta("Hello world".into()));
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "Note: Hello world");
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert_eq!(app.ed.cursor, "Note: Hello world".chars().count());
    // after a space or before punctuation: nothing added
    app.ed.text = "a ".into();
    app.ed.cursor = 2;
    apply_voice(&mut app, voice::VoiceOutput::Insert("b".into()), std::time::Instant::now());
    apply_voice(&mut app, voice::VoiceOutput::Insert(".".into()), std::time::Instant::now());
    assert_eq!(app.ed.text, "a b.");
}

#[test]
fn any_key_stops_without_typing_then_done_goes_idle() {
    let (mut app, rec, tr) = app_with_voice(true);
    ctrl_r(&mut app);
    tr.send(TranscribeEvent::Delta("hi".into()));
    pump_voice(&mut app);
    assert!(press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.ed.text, CHIP, "the stop key is not typed, the text waits");
    assert_eq!(app.voice.state(), VoiceState::Flushing);
    assert!(*rec.stopped.borrow());
    // what would send is eaten while transcribing
    assert!(press(&mut app, KeyCode::Enter, KeyModifiers::NONE));
    tr.send(TranscribeEvent::Delta(" there".into()));
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "hi there");
    assert_eq!(app.voice.state(), VoiceState::Idle);
    // back to normal: keys pass again
    assert!(!press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE));
}

#[test]
fn esc_and_ctrl_c_cancel_and_keep_the_text_around() {
    for (code, m) in [(KeyCode::Esc, KeyModifiers::NONE), (KeyCode::Char('c'), KeyModifiers::CONTROL)] {
        for stop_first in [false, true] {
            let (mut app, _rec, tr) = app_at("before after", 7);
            ctrl_r(&mut app);
            tr.send(TranscribeEvent::Delta("dropped".into()));
            pump_voice(&mut app);
            if stop_first {
                press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
            }
            assert!(press(&mut app, code, m));
            assert_eq!(app.voice.state(), VoiceState::Idle);
            assert!(tr.cancelled());
            assert_eq!((app.ed.text.as_str(), app.ed.cursor), ("before after", 7));
            assert!(!app.should_quit, "Ctrl+C cancels the recording, it does not quit");
            // the transcript of a cancelled clip never lands
            pump_voice(&mut app);
            assert_eq!(app.ed.text, "before after");
        }
    }
}

#[test]
fn ctrl_r_with_voice_off_says_how_to_enable() {
    let (mut app, _rec, _tr) = app_with_voice(false);
    assert!(ctrl_r(&mut app), "Ctrl+R is the voice key even when off");
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert_eq!(app.voice_note.as_ref().map(|(t, _)| t.as_str()), Some(voice::OFF_HINT));
    // qa-explore P: and the screen shows it
    let buf = render(&mut app);
    assert!(find(&buf, voice::OFF_HINT).is_some(), "the hint is drawn");
}

#[test]
fn a_start_error_is_a_warning_in_the_feed() {
    let (mut app, _rec, _tr) = app_with_voice(true);
    assert!(voice_key(&mut app, &KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), || {
        Err("no audio input device found.".into())
    }));
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert!(matches!(app.events.last(), Some(Ev::Warn(m)) if m == "no audio input device found."));
    // BISE-298: no key is an error with its way out
    assert!(voice_key(&mut app, &KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), || Err(voice::NEEDS_KEY.into())));
    assert!(matches!(app.events.last(), Some(Ev::Err(m)) if m == "voice needs a key. /voice picks one."));
}

#[test]
fn transcription_errors_and_notices_are_shown() {
    let (mut app, _rec, tr) = app_with_voice(true);
    ctrl_r(&mut app);
    let wrong = voice::Failure { kind: voice::FailKind::WrongKey, said: "Invalid API Key".into() };
    tr.send(TranscribeEvent::Failed(wrong, vec![1; 16_000]));
    pump_voice(&mut app);
    assert!(matches!(
        app.events.last(),
        Some(Ev::Said { glyph: "✗", head, dim })
            if head == "Mistral says the voice key is wrong. /provider fixes it."
            && *dim == vec!["Invalid API Key".to_string(), "your recording is kept: ctrl+r retry".to_string()]
    ));
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.voice_note.as_ref().map(|(t, _)| t.as_str()), Some("no speech detected"));
}

fn render(app: &mut App) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| sb::draw_sb(app, f)).unwrap();
    term.backend().buffer().clone()
}

fn find(buf: &ratatui::buffer::Buffer, text: &str) -> Option<(u16, u16)> {
    let area = buf.area;
    for y in 0..area.height {
        let row: String = (0..area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
        if let Some(i) = row.find(text) {
            let x = row[..i].chars().count() as u16;
            return Some((x, y));
        }
    }
    None
}

const CHIP: &str = voice::chip::LABEL;

/// An app with voice on, `text` in the composer, the cursor at `cursor`.
fn app_at(text: &str, cursor: usize) -> (App, FakeRecorder, FakeTranscriber) {
    let (mut app, rec, tr) = app_with_voice(true);
    app.ed.set(text, cursor);
    app.ed.break_undo();
    (app, rec, tr)
}

/// BISE-222: the chip sits at the cursor in the text, the text around
/// it stays; the transcript replaces it in place, one space where it
/// touches a word, the cursor at its end.
#[test]
fn the_chip_sits_at_the_cursor_and_the_transcript_replaces_it() {
    let (mut app, _rec, tr) = app_at("fix thebug please", 7);
    ctrl_r(&mut app);
    assert_eq!(app.ed.text, format!("fix the{CHIP}bug please"));
    assert_eq!(app.ed.cursor, 7 + CHIP.len(), "the cursor after the chip");
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(app.ed.text, format!("fix the{CHIP}bug please"), "the stop key is not typed");
    tr.send(TranscribeEvent::Delta(" login".into()));
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "fix the login bug please");
    assert_eq!(app.ed.cursor, "fix the login".len());
    // one undo takes the transcript back, and never the chip
    app.ed.apply(&editor::Action::Undo);
    assert_eq!(app.ed.text, "fix thebug please");
    app.ed.apply(&editor::Action::Undo);
    assert!(!app.ed.text.contains(CHIP));
    // between spaces: no space added
    let (mut app, _rec, tr) = app_at("a  b", 2);
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
    tr.send(TranscribeEvent::Delta("said".into()));
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "a said b");
}

/// BISE-222: while the clip is transcribed you keep typing; the chip
/// stays where it was, atomic, and the transcript lands there.
#[test]
fn typing_while_transcribing_keeps_the_chip_where_it_was() {
    let (mut app, _rec, tr) = app_at("Note: ", 6);
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(app.voice.state(), VoiceState::Flushing);
    // the keys reach the composer again
    for c in " later".chars() {
        let k = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        if !voice_key(&mut app, &k, || Ok(voice::fakes::job())) {
            app.ed.insert(&c.to_string());
        }
    }
    assert_eq!(app.ed.text, format!("Note: {CHIP} later"));
    // the cursor steps over the chip whole
    app.ed.apply(&editor::Action::Move(editor::Motion::TextEnd, false));
    for _ in 0..7 {
        app.ed.apply(&editor::Action::Move(editor::Motion::Left, false));
    }
    assert_eq!(app.ed.cursor, 6, "one step over the chip");
    app.ed.insert("(");
    tr.send(TranscribeEvent::Delta("Hello world".into()));
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "Note: (Hello world later");
    assert_eq!(app.voice.state(), VoiceState::Idle);
    // undo: the transcript, then the typing; the chip never comes back
    for _ in 0..6 {
        app.ed.apply(&editor::Action::Undo);
        assert!(!app.ed.text.contains(CHIP), "{:?}", app.ed.text);
    }
}

/// A chip deleted while the clip is transcribed cancels it; a chip left
/// with no voice at work (voice mode turned off) goes.
#[test]
fn deleting_the_chip_cancels_and_a_stale_chip_goes() {
    let (mut app, _rec, tr) = app_at("ab", 1);
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    app.ed.apply(&editor::Action::DeleteBack(editor::Unit::Grapheme));
    assert_eq!(app.ed.text, "ab", "a backspace takes the chip whole");
    pump_voice(&mut app);
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert!(tr.cancelled());
    let (mut app, _rec, _tr) = app_at("ab", 1);
    ctrl_r(&mut app);
    app.voice.cancel(); // what /voice off does
    pump_voice(&mut app);
    assert_eq!((app.ed.text.as_str(), app.ed.cursor), ("ab", 1));
}

/// A failure (an API error, no audio): the chip goes, the text stays,
/// the warning shows where it shows today.
#[test]
fn a_failure_takes_the_chip_away_and_keeps_the_text() {
    let (mut app, _rec, tr) = app_at("keep me", 4);
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    tr.send(TranscribeEvent::Failed(voice::Failure { kind: voice::FailKind::Down, said: "it answered 500".into() }, Vec::new()));
    pump_voice(&mut app);
    assert_eq!((app.ed.text.as_str(), app.ed.cursor), ("keep me", 4));
    assert!(matches!(app.events.last(), Some(Ev::Said { head, dim, .. })
        if head.starts_with("i couldn't reach Mistral to transcribe.") && *dim == vec!["it answered 500".to_string()]));
    // no speech: the chip goes, the note in the status row
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    tr.send(TranscribeEvent::Done);
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "keep me");
    assert_eq!(app.voice_note.as_ref().map(|(t, _)| t.as_str()), Some("no speech detected"));
}

fn render_w(app: &mut App, w: u16) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(w, 24)).unwrap();
    term.draw(|f| sb::draw_sb(app, f)).unwrap();
    term.backend().buffer().clone()
}

/// The chip in the composer: recording ` ● ▁▁▁▁▁▁ 0:00 ` (no audio yet) on the pill,
/// the text around it; transcribing ` ∿ ▂▃▅▆▅▃ 0:00 ` at the same place
/// and width; the key bar; narrow forms; ASCII.
#[test]
fn the_chip_is_drawn_in_the_text() {
    let (mut app, _rec, _tr) = app_at("say  now", 4);
    ctrl_r(&mut app);
    let buf = render(&mut app);
    let (x, y) = find(&buf, "say  ● ▁▁▁▁▁▁ 0:00  now").expect("the chip in the text");
    assert_eq!(buf[(x + 4, y)].bg, theme::pill_bg(), "the pill's padding cell");
    assert_eq!(buf[(x + 5, y)].fg, theme::accent(), "the dot");
    assert_eq!(buf[(x + 7, y)].fg, theme::accent(), "the live bars");
    assert_eq!(buf[(x + 14, y)].fg, theme::text(), "the timer");
    assert_eq!(buf[(x, y)].fg, theme::text(), "the text around keeps its color");
    assert!(find(&buf, "any key stop").is_some() && find(&buf, "esc cancel").is_some());
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    let buf = render(&mut app);
    let at = find(&buf, "say  ∿ ▂▃▅▆▅▃ 0:00  now").expect("transcribing, same place");
    assert_eq!(at, (x, y));
    assert_eq!(buf[(x + 7, y)].fg, theme::dim(), "the wave is dim");
    assert!(find(&buf, "transcribing").is_none(), "the chip says it");
    assert!(find(&buf, "esc cancel").is_some() && find(&buf, "any key").is_none());
    // narrow: no timer, then 3 bars
    let buf = render_w(&mut app, 40);
    assert!(find(&buf, "say  ∿ ▂▃▅▆▅▃  now").is_some());
    let buf = render_w(&mut app, 24);
    assert!(find(&buf, " ∿ ▂▃▅ ").is_some());
    // ASCII: brackets, the same width
    theme::set_ascii_for_tests(true);
    let buf = render(&mut app);
    theme::set_ascii_for_tests(false);
    assert!(find(&buf, "say [~ .-=#=- 0:00] now").is_some());
}

/// BISE-246: while recording, the chip moves by itself, no key between
/// the frames: the loop's clock (run::frame_clock) blinks the dot and
/// runs the timer, the audio thread's blocks scroll the meter, and
/// speech rises above the lowest bar (the meter was linear: a voice sat
/// on `▁` and the chip looked like a dead pink bar until the wave of
/// the transcription).
#[test]
fn the_recording_chip_moves_between_frames_without_a_key() {
    use std::time::Duration;
    let (mut app, rec, _tr) = app_at("say  now", 4);
    ctrl_r(&mut app);
    let t0 = std::time::Instant::now();
    // one turn of the loop at t0 + `ms`: pump, clock, draw
    let frame = |app: &mut App, ms: u64| {
        pump_voice(app);
        frame_clock(app, t0 + Duration::from_millis(ms), Duration::ZERO, false);
        let buf = render(app);
        let (x, y) = find(&buf, "say  ").expect("the text before the chip");
        let chip: String = (x + 4..x + 19).map(|i| buf[(i, y)].symbol().to_string()).collect();
        (chip, buf[(x + 5, y)].fg)
    };
    let (a, dot_a) = frame(&mut app, 0);
    assert_eq!(a, " ● ▁▁▁▁▁▁ 0:00 ", "silence: flat");
    assert_eq!(dot_a, theme::accent());
    // a word: two blocks, normal then quieter speech
    rec.speak(0.1, 1);
    rec.speak(0.02, 1);
    let (b, dot_b) = frame(&mut app, 700);
    assert_eq!(b, " ● ▁▁▁▁▅▃ 0:00 ", "the meter: the newest on the right");
    assert_eq!(dot_b, theme::dim(), "the dot blinks off");
    // silence again: the word scrolls left, the timer runs, the dot is back
    rec.speak(0.0, 2);
    let (c, dot_c) = frame(&mut app, 1300);
    assert_eq!(c, " ● ▁▁▅▃▁▁ 0:01 ");
    assert_eq!(dot_c, theme::accent());
    assert_eq!(app.voice.state(), VoiceState::Recording, "no key: still recording");
}
