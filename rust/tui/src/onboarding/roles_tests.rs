//! `/models` and the role pickers (BISE-298): the rows and their words,
//! a pick per role (the fallback row, a model and its effort, a typed id,
//! a provider set up on the way), the voice picker (ready / another
//! provider, the transcription check), `/provider`'s `use it for…`.

use super::provider::*;
use super::roles::*;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-roles-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn screen_w(o: &Onb, w: u16) -> String {
    let h = 34;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, o, 10)).unwrap();
    let b = t.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen(o: &Onb) -> String {
    screen_w(o, 110)
}

/// The row that starts with `what` (a mark `›` or `✓` before it aside).
fn line_of<'a>(sc: &'a str, what: &str) -> &'a str {
    let starts = |l: &str| {
        let t = l.trim_start();
        let t = t.strip_prefix("› ").or_else(|| t.strip_prefix("✓ ")).unwrap_or(t);
        t.trim_start().starts_with(what)
    };
    sc.lines().find(|l| starts(l)).unwrap_or_else(|| panic!("no {what}:\n{sc}"))
}

/// The tests that read CALLS run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// The key check: a key holding "bad" is refused; the calls it made.
static CALLS: std::sync::Mutex<Vec<(String, bool)>> = std::sync::Mutex::new(Vec::new());

fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).push((format!("{}/{} {}", c.provider, c.model, c.api), c.voice));
    if c.key.contains("bad") {
        return Err(crate::keycheck::Fail { why: Why::WrongKey, said: "Invalid API Key".into() });
    }
    Ok(())
}

fn settle(o: &mut Onb, e: Env) {
    for _ in 0..400 {
        o.tick(e);
        if !matches!(o.sub, Sub::Checking(..)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the check never answered");
}

/// A home with `cfg` as config.toml and these keys in the environment.
fn home(tag: &str, cfg: &str, keys: &[&'static str]) -> (impl Fn(&str) -> Option<String>, bise_home::Home) {
    let h = tmp(tag);
    let mut m: HashMap<&'static str, String> = HashMap::from([("HOME", h.to_string_lossy().to_string())]);
    for k in keys {
        m.insert(k, "sk-good".into());
    }
    let e = move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty());
    let hm = home_of(&e);
    std::fs::create_dir_all(hm.config_file().parent().unwrap()).unwrap();
    std::fs::write(hm.config_file(), cfg).unwrap();
    (e, hm)
}

fn open(e: Env, open: Open) -> Onb {
    let mut o = Onb::provider_panel(e, Ask { open, ..Ask::default() });
    o.checker = fake_check;
    o
}

fn cfg(hm: &bise_home::Home) -> String {
    std::fs::read_to_string(hm.config_file()).unwrap()
}

fn typed(o: &mut Onb, e: Env, t: &str) {
    for c in t.chars() {
        o.on_key(key(KeyCode::Char(c)), 1, e);
    }
}

#[test]
fn models_lists_each_role_with_what_runs_and_why() {
    let (e, _hm) = home("list", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let o = open(&e, Open::Roles);
    let sc = screen(&o);
    assert!(sc.contains("which model does what?") && sc.contains("main uses one model, the rest follow it unless you pick."), "{sc}");
    // picked: the id; a fallback: its rule, then the id it runs
    assert!(line_of(&sc, "main  ").contains("mistral/mistral-medium-latest · high"), "{sc}");
    assert!(line_of(&sc, "agents ").contains("same as main · mistral/mistral-medium-latest"), "{sc}");
    assert!(line_of(&sc, "small jobs ").contains("auto · mistral/mistral-small-latest"), "{sc}");
    // voice mode off: off, and how to set it up
    assert!(line_of(&sc, "voice ").trim_end().ends_with("off · enter sets it up"), "{sc}");
    // what the role under the cursor is for, once, under the list
    assert!(sc.contains("main: talks with you and starts the agents.") && !sc.contains("your team lead"), "{sc}");
    assert!(sc.contains("↑↓ choose · enter change · esc back"), "{sc}");
    // auto-confirm waits for its feature
    assert!(!sc.contains("auto-confirm"), "{sc}");
}

#[test]
fn a_role_whose_provider_has_no_key_says_so_and_enter_fixes_it() {
    let (e, _hm) = home("broken", "[roles]\nmain = \"anthropic/claude-opus-5-5\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    let sc = screen(&o);
    // the id stays on screen
    assert!(line_of(&sc, "main  ").contains("anthropic/claude-opus-5-5  ✗ no key · enter fixes it"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "anthropic" && m == "anthropic/claude-opus-5-5"), "{:?}", o.sub);
}

#[test]
fn the_agents_picker_offers_same_as_main_first_then_saves_a_model_and_its_effort() {
    let (e, hm) = home("agents", "model = \"mistral/mistral-medium-latest\"\nagent_model = \"mistral/mistral-small-latest\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    o.on_key(key(KeyCode::Down), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("which model do the agents use?"), "{sc}");
    assert!(o.pick_rows("agents")[0] == PRow::Fallback("mistral/mistral-medium-latest".into()), "{:?}", o.pick_rows("agents"));
    assert!(line_of(&sc, "same as main").contains("same as main · mistral/mistral-medium-latest"), "{sc}");
    // picked today: its row is selected and marked
    assert!(line_of(&sc, "mistral/mistral-small-latest").contains("✓ now"), "{sc}");
    assert!(line_of(&sc, "mistral/mistral-small-latest").contains("›"), "{sc}");
    assert!(sc.contains("+ another provider…"), "{sc}");
    // the medium one: how hard should it think? (Mistral: none, high)
    while o.pick_rows("agents").get(o.sel) != Some(&PRow::Model("mistral/mistral-medium-latest".into())) {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("how hard should it think?") && sc.contains("mistral/mistral-medium-latest for agents"), "{sc}");
    assert!(line_of(&sc, "high").contains("default"), "{sc}");
    // none: written with the model, the old key gone
    o.on_key(key(KeyCode::Up), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    let c = cfg(&hm);
    assert!(c.contains("[roles.agents]\nmodel = \"mistral/mistral-medium-latest\"\neffort = \"none\"") && !c.contains("agent_model"), "{c}");
    // back on /models, the row flashing ✓
    let sc = screen(&o);
    assert!(sc.contains("which model does what?"), "{sc}");
    assert!(line_of(&sc, "agents ").contains("✓") && line_of(&sc, "agents ").contains("mistral/mistral-medium-latest · none"), "{sc}");
    // same as main again (its first row): the role leaves config.toml
    o.on_key(key(KeyCode::Enter), 1, &e);
    while o.sel != 0 {
        o.on_key(key(KeyCode::Up), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    let c = cfg(&hm);
    assert!(!c.contains("agents") && c.contains("model = \"mistral/mistral-medium-latest\""), "{c}");
    assert!(line_of(&screen(&o), "agents ").contains("same as main"));
}

#[test]
fn a_typed_id_of_a_provider_without_a_key_goes_through_its_key_step() {
    let (e, hm) = home("typed", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(screen(&o).contains("which model is your team lead?"));
    typed(&mut o, &e, "mistral-large-9");
    assert!(screen(&o).contains("+ use mistral/mistral-large-9"), "{}", screen(&o));
    for _ in 0.."mistral-large-9".len() {
        o.on_key(key(KeyCode::Backspace), 1, &e);
    }
    typed(&mut o, &e, "openai/gpt-6-luna");
    let sc = screen(&o);
    assert!(sc.contains("+ set up OpenAI for openai/gpt-6-luna") && line_of(&sc, "+ set up").contains("no key yet"), "{sc}");
    let last = o.pick_rows("main").len() - 1;
    while o.sel != last {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "openai" && m == "openai/gpt-6-luna"), "{:?}", o.sub);
    // a wrong key: the first run's words; a good one: its effort, saved
    o.on_paste("sk-bad");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(o.sub, Sub::Failed(..)), "{:?}", o.sub);
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_paste("sk-good-openai");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(&o.sub, Sub::Effort(m, _) if m == "openai/gpt-6-luna"), "{:?}", o.sub);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(cfg(&hm).contains("main = \"openai/gpt-6-luna\""), "{}", cfg(&hm));
    assert!(stored(&hm, "openai"), "a normal provider key");
}

#[test]
fn the_voice_picker_lists_the_ready_models_then_the_other_providers() {
    let (e, _hm) = home("voice", "", &["OPENAI_API_KEY"]);
    let o = open(&e, Open::Pick("voice"));
    let sc = screen(&o);
    assert!(sc.contains("which model should listen to you?") && sc.contains("you talk, it types in the composer. ctrl+r starts, any key stops."), "{sc}");
    assert!(sc.contains("› type to filter"), "{sc}");
    // ready: OpenAI's voice models, its pick first and recommended
    let ready = sc.find("ready").expect("ready");
    let another = sc.find("another provider").expect("another provider");
    assert!(ready < another, "{sc}");
    // under their provider's line
    assert!(sc.contains("  OpenAI · your key"), "{sc}");
    // the ids without their provider under it
    let first = line_of(&sc, "gpt-transcribe ");
    assert!(first.contains("›") && first.contains("recommended") && !first.contains("openai/"), "{sc}");
    assert!(line_of(&sc, "whisper-1").trim() == "whisper-1", "{sc}");
    assert!(line_of(&sc, "Mistral ").contains("voxtral-mini-latest · the key also works for chat"), "{sc}");
    assert!(line_of(&sc, "ElevenLabs ").contains("scribe_v2 · voice only"), "{sc}");
    // Groq and Deepgram: not offered
    assert!(!sc.contains("Groq") && !sc.contains("Deepgram") && !sc.contains("groq/"), "{sc}");
    assert!(sc.contains("esc not now"), "{sc}");
}

#[test]
fn no_key_at_all_preselects_mistral_and_esc_leaves_voice_off() {
    let (e, _hm) = home("novoice", "", &[]);
    let mut o = Onb::provider_panel(&e, Ask { open: Open::Pick("voice"), voice_on: true, ..Ask::default() });
    o.checker = fake_check;
    let sc = screen(&o);
    assert!(!sc.contains("ready"), "no ready group: {sc}");
    assert!(line_of(&sc, "Mistral ").contains("›"), "{sc}");
    let _ = take_voice_out();
    assert_eq!(o.on_key(key(KeyCode::Esc), 1, &e), Out::Done);
    assert_eq!(take_voice_out(), Some(VoiceOut::Off));
}

#[test]
fn a_voice_provider_is_set_up_its_models_picked_and_checked_by_a_transcription() {
    let _one = serial();
    let (e, hm) = home("voicekey", "", &[]);
    let mut o = Onb::provider_panel(&e, Ask { open: Open::Pick("voice"), voice_on: true, ..Ask::default() });
    o.checker = fake_check;
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let _ = take_voice_out();
    // Mistral: its voice models (three), its pick first
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(matches!(&o.sub, Sub::Model(p, 0, _) if p.id == "mistral"), "{:?}", o.sub);
    assert!(sc.contains("voxtral-mini-latest") && sc.contains("voxtral-transcribe-3"), "{sc}");
    assert!(!sc.contains("mistral-medium-latest"), "voice models only: {sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "mistral" && m == "mistral/voxtral-mini-latest"), "{:?}", o.sub);
    assert!(screen(&o).contains("paste your Mistral key"));
    // wrong key, then a good one: checked by a transcription
    o.on_paste("sk-bad");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(o.sub, Sub::Failed(..)), "{:?}", o.sub);
    assert!(screen(&o).contains("Invalid API Key"), "{}", screen(&o));
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_paste("sk-good-mistral");
    assert_eq!(o.on_key(key(KeyCode::Enter), 1, &e), Out::Stay);
    settle(&mut o, &e);
    assert!(o.panel.as_ref().unwrap().closed, "done: back to the feed");
    // (the chat checks of the other tests run alongside: the voice ones)
    let calls: Vec<_> = CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|(_, v)| *v).cloned().collect();
    assert!(calls.len() == 2 && calls.iter().all(|(c, _)| c == "mistral/voxtral-mini-latest mistral"), "{calls:?}");
    assert_eq!(take_voice_out(), Some(VoiceOut::On("mistral/voxtral-mini-latest".into())));
    assert!(cfg(&hm).contains("[roles]\nvoice = \"mistral/voxtral-mini-latest\""), "{}", cfg(&hm));
    assert!(stored(&hm, "mistral"), "a normal provider key: /provider shows it, chat can use it");
}

#[test]
fn a_ready_voice_model_is_one_enter_and_one_check() {
    let _one = serial();
    let (e, hm) = home("voiceready", "", &["OPENAI_API_KEY"]);
    let mut o = open(&e, Open::Pick("voice"));
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let _ = take_voice_out();
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(o.panel.as_ref().unwrap().closed);
    let calls: Vec<_> = CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|(_, v)| *v).cloned().collect();
    assert_eq!(calls, vec![("openai/gpt-transcribe openai".to_string(), true)]);
    assert_eq!(take_voice_out(), Some(VoiceOut::On("openai/gpt-transcribe".into())));
    assert!(cfg(&hm).contains("voice = \"openai/gpt-transcribe\""));
}

#[test]
fn provider_use_it_for_lists_the_roles_it_can_run() {
    let _one = serial();
    let (e, hm) = home("usefor", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY", "ELEVENLABS_API_KEY"]);
    let mut o = Onb::provider_panel(&e, Ask { provider: Some("mistral".into()), ..Ask::default() });
    o.checker = fake_check;
    let sc = screen(&o);
    assert!(line_of(&sc, "2 · use it for…").contains("main · agents · small jobs"), "{sc}");
    o.on_key(key(KeyCode::Char('2')), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("use Mistral for…"), "{sc}");
    for r in ["main", "agents", "small jobs", "voice"] {
        assert!(sc.contains(r), "{r}: {sc}");
    }
    assert!(line_of(&sc, "main  ").contains("✓ mistral-medium-latest"), "{sc}");
    // voice: its picker filtered to Mistral, checked, back to the menu
    while o.uses_of(&o.every_of("mistral")).get(match o.sub { Sub::UseFor(_, i) => i, _ => 0 }).map(|r| r.id) != Some("voice") {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("which model should listen to you?") && sc.contains("› mistral/▏"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(&o.sub, Sub::Menu(p, 1) if p.id == "mistral"), "{:?}", o.sub);
    assert!(cfg(&hm).contains("voice = \"mistral/voxtral-mini-latest\""), "{}", cfg(&hm));
    // a voice-only provider with a key shows in the list: its menu has
    // no chat role
    o.on_key(key(KeyCode::Esc), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("ElevenLabs"), "{sc}");
    let el = o.every_of("elevenlabs");
    assert_eq!(o.uses_of(&el).iter().map(|r| r.id).collect::<Vec<_>>(), vec!["voice"]);
}

#[test]
fn models_fits_narrow_screens_by_cutting_the_descriptions_first() {
    let (e, _hm) = home("narrow", "[roles.main]\nmodel = \"anthropic/claude-opus-5-5\"\neffort = \"high\"\n", &["ANTHROPIC_API_KEY"]);
    let o = open(&e, Open::Roles);
    let wide = screen_w(&o, 110);
    let narrow = screen_w(&o, 50);
    assert!(line_of(&wide, "main  ").contains("anthropic/claude-opus-5-5 · high"), "{wide}");
    // a row never wraps: the fallback's id loses its provider, then goes;
    // the picked id stays
    let n = line_of(&narrow, "main  ");
    assert!(n.contains("anthropic/claude-opus-5-5"), "{narrow}");
    let a = line_of(&narrow, "agents  ");
    assert!(a.contains("same as main") && !a.contains("anthropic/"), "{narrow}");
    assert!(!narrow.lines().any(|l| l.trim_start().starts_with("anthropic") || l.trim_start().starts_with("claude")), "{narrow}");
}

#[test]
fn an_agent_with_its_own_model_shows_under_agents() {
    let (e, _hm) = home("own", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let o = Onb::provider_panel(&e, Ask { open: Open::Roles, overrides: vec![("perf".into(), "openai/gpt-6-sol".into())], ..Ask::default() });
    let sc = screen(&o);
    let i = sc.lines().position(|l| l.contains("agents   ")).expect("agents row");
    assert!(sc.lines().nth(i + 1).unwrap().contains("1 agent uses its own model: perf · openai/gpt-6-sol"), "{sc}");
    // designer's review: the screen as the user sees it (SB_DUMP=dir)
    if let Ok(d) = std::env::var("SB_DUMP") {
        std::fs::write(format!("{}/models-3-agent-own-model.txt", d), screen_w(&o, 120)).unwrap();
    }
}
