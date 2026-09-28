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
    voice_key(app, &KeyEvent::new(code, m), || Some("sk-test".into()))
}

fn ctrl_r(app: &mut App) -> bool {
    press(app, KeyCode::Char('r'), KeyModifiers::CONTROL)
}

/// The user's hard requirement: the words show up while talking, each
/// delta on the next tick, before any stop.
#[test]
fn deltas_land_in_the_composer_while_recording() {
    let (mut app, _rec, tr) = app_with_voice(true);
    app.ed.text = "Note: ".into();
    app.ed.cursor = 6;
    assert!(ctrl_r(&mut app));
    assert_eq!(app.voice.state(), VoiceState::Recording);
    tr.send(TranscribeEvent::SessionCreated);
    tr.send(TranscribeEvent::Delta("Hello".into()));
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "Note: Hello");
    tr.send(TranscribeEvent::Delta(" world".into()));
    pump_voice(&mut app);
    assert_eq!(app.ed.text, "Note: Hello world");
    assert_eq!(app.voice.state(), VoiceState::Recording, "no stop needed");
    assert_eq!(app.ed.cursor, "Note: Hello world".chars().count());
}

#[test]
fn any_key_stops_without_typing_then_done_goes_idle() {
    let (mut app, rec, tr) = app_with_voice(true);
    ctrl_r(&mut app);
    tr.send(TranscribeEvent::Delta("hi".into()));
    pump_voice(&mut app);
    assert!(press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.ed.text, "hi", "the stop key is not typed");
    assert_eq!(app.voice.state(), VoiceState::Flushing);
    assert!(*rec.stopped.borrow());
    // keys are eaten while flushing
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
fn esc_and_ctrl_c_cancel_and_keep_the_text() {
    for (code, m) in [(KeyCode::Esc, KeyModifiers::NONE), (KeyCode::Char('c'), KeyModifiers::CONTROL)] {
        let (mut app, _rec, tr) = app_with_voice(true);
        ctrl_r(&mut app);
        tr.send(TranscribeEvent::Delta("kept".into()));
        pump_voice(&mut app);
        assert!(press(&mut app, code, m));
        assert_eq!(app.voice.state(), VoiceState::Idle);
        assert!(tr.cancelled());
        assert_eq!(app.ed.text, "kept");
        assert!(!app.should_quit, "Ctrl+C cancels the recording, it does not quit");
    }
}

#[test]
fn ctrl_r_with_voice_off_says_how_to_enable() {
    let (mut app, _rec, _tr) = app_with_voice(false);
    assert!(ctrl_r(&mut app), "Ctrl+R is the voice key even when off");
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert_eq!(app.voice_note.as_ref().map(|(t, _)| t.as_str()), Some(voice::OFF_HINT));
}

#[test]
fn a_start_error_is_a_warning_in_the_feed() {
    let (mut app, _rec, _tr) = app_with_voice(true);
    assert!(voice_key(&mut app, &KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), || None));
    assert_eq!(app.voice.state(), VoiceState::Idle);
    assert!(matches!(
        app.events.last(),
        Some(Ev::Warn(m)) if m == "voice transcription needs an API key: set MISTRAL_API_KEY"
    ));
}

#[test]
fn transcription_errors_and_notices_are_shown() {
    let (mut app, _rec, tr) = app_with_voice(true);
    ctrl_r(&mut app);
    tr.send(TranscribeEvent::Error("HTTP 401 Unauthorized".into()));
    pump_voice(&mut app);
    assert!(matches!(
        app.events.last(),
        Some(Ev::Err(m)) if m == "voice transcription failed: HTTP 401 Unauthorized"
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

#[test]
fn recording_shows_the_meter_an_orange_border_and_dim_text() {
    let (mut app, _rec, tr) = app_with_voice(true);
    ctrl_r(&mut app);
    tr.send(TranscribeEvent::Delta("dictated".into()));
    pump_voice(&mut app);
    let buf = render(&mut app);
    // the fake capture's peak is 0.5: '▅', then the text
    let (x, y) = find(&buf, "▅ dictated").expect("meter before the text");
    assert_eq!(buf[(x, y)].fg, RECORDING);
    assert_eq!(buf[(x + 2, y)].fg, DIM);
    // the meter takes the place of the `›` prompt
    assert!(find(&buf, &format!("{} ", G_YOU)).is_none(), "no prompt while recording");
    assert!(find(&buf, "recording · any key stops").is_some());
    // idle again: no meter, the normal border
    app.voice.cancel();
    let buf = render(&mut app);
    assert!(find(&buf, "▅").is_none());
    let (x, y) = find(&buf, "dictated").unwrap();
    assert_eq!(buf[(x, y)].fg, TEXT);
}

#[test]
fn flushing_shows_the_fill_spinner() {
    let (mut app, _rec, _tr) = app_with_voice(true);
    ctrl_r(&mut app);
    press(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    let buf = render(&mut app);
    assert!(voice::FILL_BLOCKS.iter().any(|g| find(&buf, &g.to_string()).is_some()));
    assert!(find(&buf, "transcribing the last words").is_some());
}
