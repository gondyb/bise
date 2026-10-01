//! `/voice`'s screen and the first voice mode's: keys, the words, the
//! frames at 150/95/80 columns. ▸ hear it is a step, never a sound here.

use super::*;
use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// The request statics are the process's: one test at a time on them.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _one = serial();
    request(Open::Settings);
    assert_eq!(take_request(), Some(Open::Settings));
    assert_eq!(take_request(), None);
}

#[test]
fn arrows_change_a_row_and_save_it_at_once() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    at(&mut s, Row::Listen);
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
    // moving over the voice row never plays anything; no list yet: ←→
    // stays on the voice set
    at(&mut s, Row::Stt);
    assert_eq!(s.on_key(key(KeyCode::Down), now), Step::Stay);
    assert_eq!(s.row(), Row::Voice);
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Stay);
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::Hear);
    // with the list, ←→ picks another voice, saved; still no sound
    s.voices = voices::State::Ready(voices::fake());
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Save);
    assert_ne!(s.cfg.voice, crate::voicemode::tts::DEFAULT_VOICE);
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
    // the row names who hears you: the speech-to-text provider, and the
    // voice's when it is another (designer)
    assert_eq!(who().hears(), "Mistral");
    let w = Who { listen: "Mistral".into(), speak: Ok("Mistral".into()), ack: "OpenAI".into() };
    assert_eq!(w.hears(), "Mistral");
    let w = Who { listen: "Mistral".into(), speak: Ok("OpenAI".into()), ack: "OpenAI".into() };
    assert_eq!(w.hears(), "Mistral hears you, OpenAI speaks");
    let w = Who { listen: "Mistral".into(), speak: Err("OpenAI can't speak yet".into()), ack: "OpenAI".into() };
    assert_eq!(w.hears(), "Mistral");
}

#[test]
fn the_settings_fit_150_95_and_80_columns() {
    let mut s = screen(Open::Settings);
    at(&mut s, Row::Voice);
    for (w, h) in [(150, 40), (95, 40), (80, 29)] {
        let f = frame(&s, w, h);
        for want in [
            "ctrl+r twice",
            "dictation",
            "speech to text",
            "Mistral · Voxtral Transcribe 3",
            "listen",
            "voice",
            "speed",
            "1.0×",
            "read aloud",
            "sounds",
            "language",
            "who hears you",
            "hear it",
            "esc back",
        ] {
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
    at(&mut s, Row::Listen);
    s.on_key(key(KeyCode::Right), now);
    let lines = s.lines(80, now);
    let row = lines.iter().map(|l| l.to_string()).find(|l| l.contains("listen")).unwrap();
    assert!(row.starts_with('✓') || row.starts_with('v'), "{row}");
    let later = s.lines(80, now + Duration::from_secs(2));
    let row = later.iter().map(|l| l.to_string()).find(|l| l.contains("listen")).unwrap();
    assert!(row.starts_with('›') || row.starts_with('>'), "{row}");
}

#[test]
fn speech_to_text_cycles_the_models_with_a_key_and_enter_opens_the_picker() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    s.stt = Stt {
        model: "mistral/voxtral-transcribe-3".into(),
        options: vec!["mistral/voxtral-transcribe-3".into(), "openai/gpt-transcribe".into()],
        names: vec![("mistral".into(), "Mistral".into()), ("openai".into(), "OpenAI".into())],
        env: false,
    };
    at(&mut s, Row::Stt);
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::SaveStt);
    assert_eq!(s.stt.model, "openai/gpt-transcribe");
    assert!(frame(&s, 95, 40).contains("OpenAI · GPT Transcribe"));
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::SaveStt);
    assert_eq!(s.stt.model, "mistral/voxtral-transcribe-3");
    assert_eq!(s.on_key(key(KeyCode::Enter), now), Step::PickStt);
    // one model only: nothing to change
    s.stt.options.truncate(1);
    assert_eq!(s.on_key(key(KeyCode::Left), now), Step::Stay);
    // BISE_VOICE_MODEL decides: said on the row
    s.stt.env = true;
    assert!(frame(&s, 150, 40).contains("BISE_VOICE_MODEL decides"));
}

#[test]
fn speech_to_text_lists_the_ready_providers_their_pick_first() {
    let none = |_: &str| None;
    let env = |k: &str| (k == "MISTRAL_API_KEY").then(|| "sk".to_string());
    let store = bise_catalog::auth::Store::default();
    let setup = bise_catalog::Setup::from_text(Some(""), &none);
    let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &[] };
    let stt = Stt::of(&setup, &keys);
    assert_eq!(stt.model, setup.voice.model);
    assert!(stt.options.iter().all(|m| m.starts_with("mistral/")), "{:?}", stt.options);
    let pick = setup.catalog.provider("mistral").unwrap().voice_model.clone();
    assert_eq!(stt.options[0], format!("mistral/{}", pick));
    assert!(!stt.env);
}

#[test]
fn the_voices_list_with_language_and_gender_and_a_language_picks_a_voice_that_speaks_it() {
    let mut s = screen(Open::Settings);
    let now = Instant::now();
    s.voices = voices::State::Ready(voices::fake());
    at(&mut s, Row::Voice);
    let f = frame(&s, 95, 40);
    assert!(f.contains("Mistral · Paul, neutral · English · male") && f.contains("hear it"), "{f}");
    // the language row offers auto and the voices' languages
    at(&mut s, Row::Language);
    assert_eq!(s.on_key(key(KeyCode::Right), now), Step::Save);
    assert_eq!(s.cfg.language.as_deref(), Some("en"));
    assert_eq!(s.cfg.voice, "en_paul_neutral", "Paul speaks English: kept");
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.language.as_deref(), Some("fr"));
    assert!(s.cfg.voice.starts_with("fr_"), "{}", s.cfg.voice);
    assert_eq!(s.sample_language().as_deref(), Some("fr"));
    assert!(sample(Some("fr")).starts_with("bonjour"));
    // the voice row then goes through the French voices first
    at(&mut s, Row::Voice);
    s.on_key(key(KeyCode::Right), now);
    assert!(s.cfg.voice.starts_with("fr_"), "{}", s.cfg.voice);
    s.on_key(key(KeyCode::Right), now);
    s.on_key(key(KeyCode::Right), now);
    assert!(s.cfg.voice.starts_with("en_"), "{}", s.cfg.voice);
    at(&mut s, Row::Language);
    s.on_key(key(KeyCode::Right), now);
    assert_eq!(s.cfg.language, None);
}

#[test]
fn the_voices_while_they_load_and_when_they_did_not() {
    let mut s = screen(Open::Settings);
    at(&mut s, Row::Voice);
    s.voices = voices::State::Loading;
    assert!(frame(&s, 95, 40).contains("loading Mistral's voices…"));
    s.voices = voices::State::Failed(voices::failed_line("Mistral", &voices::FetchError::Status(500)));
    let f = frame(&s, 150, 40);
    assert!(f.contains("Mistral's voices didn't load (HTTP 500). esc, then /voice tries again."), "{f}");
    assert!(f.contains("Mistral · Paul, neutral"), "the voice set stays: {f}");
}

#[test]
fn voice_setup_opens_the_same_screen_on_speech_to_text() {
    let _one = serial();
    request_stt();
    assert_eq!(take_request(), Some(Open::Settings));
    assert_eq!(START.lock().unwrap().take(), Some(Row::Stt));
    let mut s = screen(Open::Settings);
    s.at(Row::Stt);
    assert_eq!(s.row(), Row::Stt);
    let f = frame(&s, 95, 40);
    assert!(f.contains("the model that writes down what you say") && f.contains("enter other providers"), "{f}");
}

#[test]
fn hands_free_says_how_you_cut_in() {
    let mut s = screen(Open::Settings);
    s.cfg.listen = ListenMode::HandsFree;
    let f = frame(&s, 150, 40);
    assert!(f.contains("hands-free · cut in by voice with headphones"), "{f}");
    let f = frame(&s, 80, 29);
    assert!(f.contains("hands-free · cut in by voice with headphones"), "no cut at 80: {f}");
    // at 80 columns a long voice loses its gender, then its language,
    // never its name
    s.voices = voices::State::Ready(voices::fake());
    s.cfg.voice = "en_jane_cheerful".into();
    s.at(Row::Voice);
    let f = frame(&s, 95, 40);
    assert!(f.contains("Mistral · Jane, cheerful · English · female  ▸ hear it"), "{f}");
    let f = frame(&s, 80, 29);
    let row = f.lines().find(|l| l.contains("hear it") && l.contains("voice")).unwrap();
    assert!(row.contains("Mistral · Jane, cheerful  ▸ hear it") && !row.contains('…'), "{f}");
    s.cfg.voice = "en_paul_neutral".into();
    let f = frame(&s, 80, 29);
    assert!(f.contains("Mistral · Paul, neutral · English  ▸ hear it"), "{f}");
}

/// The designer's captures of the one `/voice` screen (the voices
/// listed, rows under the cursor): `VOICE_CAPTURES=<dir> cargo test -p
/// bend-tui settings_captures -- --ignored`. Text and ANSI, dark, light
/// and NO_COLOR (attributes only).
#[test]
#[ignore]
fn settings_captures() {
    use crate::theme::Mode;
    use ratatui::style::Color;
    let Some(dir) = std::env::var_os("VOICE_CAPTURES") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ansi = |buf: &ratatui::buffer::Buffer, color: bool| {
        let mut out = String::new();
        for row in buf.content.chunks(buf.area.width as usize) {
            let mut skip = 0;
            for c in row {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                let mut sgr = vec!["0".to_string()];
                if let (true, Color::Rgb(r, g, b)) = (color, c.fg) {
                    sgr.push(format!("38;2;{r};{g};{b}"));
                }
                if c.modifier.contains(Modifier::BOLD) {
                    sgr.push("1".into());
                }
                if c.modifier.contains(Modifier::DIM) {
                    sgr.push("2".into());
                }
                out.push_str(&format!("\x1b[{}m{}", sgr.join(";"), c.symbol()));
                skip = c.symbol().width().saturating_sub(1);
            }
            out.push_str("\x1b[0m\n");
        }
        out
    };
    let cfg =
        VoiceModeConfig { voice: "fr_marie_neutral".into(), language: Some("fr".into()), ..VoiceModeConfig::default() };
    let cases: Vec<(&str, Row, voices::State, Option<&str>)> = vec![
        ("stt", Row::Stt, voices::State::Ready(voices::fake()), None),
        ("voice", Row::Voice, voices::State::Ready(voices::fake()), None),
        ("language", Row::Language, voices::State::Ready(voices::fake()), None),
        ("loading", Row::Voice, voices::State::Loading, None),
        ("failed", Row::Voice, voices::State::Failed(voices::failed_line("Mistral", &voices::FetchError::Status(500))), None),
        ("cant-speak", Row::Voice, voices::State::Idle, Some("OpenAI can't speak yet: voice mode talks with Mistral (/voice picks its voice)")),
    ];
    let mut index = String::new();
    for (fname, mode, no_color) in [("dark", Mode::Dark, false), ("light", Mode::Light, false), ("nocolor", Mode::Dark, true)] {
        theme::set_mode(mode);
        for (cname, row, st, cant) in &cases {
            let mut s = Screen::new(Open::Settings, cfg.clone(), who(), true);
            s.stt.options.push("openai/gpt-transcribe".into());
            s.voices = st.clone();
            s.at(*row);
            if let Some(e) = cant {
                s.who = Who { listen: "OpenAI".into(), speak: Err(e.to_string()), ack: "OpenAI".into() };
                s.cfg.voice = String::new();
            }
            for (w, h) in [(150u16, 40u16), (95, 40), (80, 40), (80, 29)] {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| draw_in(f, &s, Instant::now())).unwrap();
                let buf = t.backend().buffer().clone();
                let rows: Vec<String> = buf
                    .content
                    .chunks(w as usize)
                    .map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
                    .collect();
                let name = format!("voice-settings-{cname}-{w}x{h}-{fname}");
                std::fs::write(dir.join(format!("{name}.txt")), rows.join("\n") + "\n").unwrap();
                std::fs::write(dir.join(format!("{name}.ans")), ansi(&buf, !no_color)).unwrap();
                index.push_str(&name);
                index.push('\n');
            }
        }
    }
    std::fs::write(dir.join("index.txt"), index).unwrap();
}

#[test]
fn a_speech_model_is_named_never_its_id() {
    assert_eq!(model_name("voxtral-transcribe-3"), "Voxtral Transcribe 3");
    assert_eq!(model_name("gpt-transcribe"), "GPT Transcribe");
    assert_eq!(model_name("nova-3"), "Nova 3");
    let s = screen(Open::Settings);
    let f = frame(&s, 95, 40);
    assert!(f.contains("speech to text   Mistral · Voxtral Transcribe 3") && !f.contains("voxtral-transcribe"), "{f}");
    // the subtitle never repeats the dictation row, and fits one line at 95
    assert!(f.contains("ctrl+r twice starts voice mode with the agent in view. esc leaves."), "{f}");
}
