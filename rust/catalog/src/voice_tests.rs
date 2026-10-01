//! Voice mode's keys of `[voice]` and its calls (docs/voice-mode-plan.md
//! §4.7).

use super::*;
use crate::auth::{Keys, Store};

fn setup(cfg: &str) -> Setup {
    Setup::from_text(Some(cfg), &|_| None)
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn mistral_key(k: &str) -> Option<String> {
    (k == "MISTRAL_API_KEY").then(|| "sk-secret-mistral".to_string())
}

#[test]
fn voice_mode_keys_default_when_unset() {
    let k = VoiceModeKeys::from_text(Some("[voice]\nmodel = \"mistral/voxtral-mini-latest\"\n"));
    assert_eq!(k, VoiceModeKeys::default());
    assert_eq!((k.listen, k.read_aloud, k.speed, k.sounds, k.seen_privacy), (ListenMode::Auto, ReadAloud::Needs, 1.0, true, false));
    assert_eq!(VoiceModeKeys::from_text(None), VoiceModeKeys::default());
    assert_eq!(VoiceModeKeys::from_text(Some("not = [toml")), VoiceModeKeys::default());
}

#[test]
fn voice_mode_keys_are_read_and_written_back_the_same() {
    let text = "[voice]\nlisten = \"hold\"\ntts_model = \"mistral/voxtral-mini-tts-2603\"\ntts_voice = \"paul\"\nspeed = 1.2\nread_aloud = \"all\"\nsounds = false\nseen_privacy = true\n";
    let k = VoiceModeKeys::from_text(Some(text));
    assert_eq!(k.listen, ListenMode::Hold);
    assert_eq!(k.tts_model.as_deref(), Some("mistral/voxtral-mini-tts-2603"));
    assert_eq!(k.tts_voice.as_deref(), Some("paul"));
    assert!((k.speed - 1.2).abs() < 1e-6);
    assert_eq!((k.read_aloud, k.sounds, k.seen_privacy), (ReadAloud::All, false, true));
    // no warning for voice mode's keys
    assert!(setup(text).catalog.warnings.is_empty(), "{:?}", setup(text).catalog.warnings);
    // written: the same values
    let mut back = String::from("[voice]\n");
    for (key, v) in k.written() {
        if let Some(v) = v {
            back.push_str(&format!("{} = {}\n", key, v));
        }
    }
    assert_eq!(VoiceModeKeys::from_text(Some(&back)), k);
}

#[test]
fn the_words_of_listen_and_read_aloud() {
    for m in ListenMode::ALL {
        assert_eq!(ListenMode::of(m.word()), Some(m));
    }
    assert_eq!(ListenMode::of("Hands Free"), Some(ListenMode::HandsFree));
    assert_eq!(ListenMode::of("handsfree"), Some(ListenMode::HandsFree));
    assert_eq!(ListenMode::of("always"), None);
    for r in ReadAloud::ALL {
        assert_eq!(ReadAloud::of(r.word()), Some(r));
    }
    assert_eq!(ReadAloud::of("some"), None);
}

#[test]
fn a_wrong_voice_mode_value_is_a_warning_and_keeps_its_default() {
    let text = "[voice]\nlisten = \"always\"\nspeed = 3.0\nread_aloud = 1\nsounds = \"yes\"\ntts_model = \"voxtral\"\nseen_privacy = 1\n";
    let w = setup(text).catalog.warnings.join("\n");
    for k in [
        "voice.listen: auto, hands-free or hold",
        "voice.speed: a number from 0.8 to 1.6",
        "voice.read_aloud: needs, all or nothing",
        "voice.sounds: true or false",
        "voice.tts_model: a \"provider/model\" name",
        "voice.seen_privacy: true or false",
    ] {
        assert!(w.contains(k), "{k}: {w}");
    }
    assert_eq!(VoiceModeKeys::from_text(Some(text)), VoiceModeKeys::default());
    // an integer speed in range is a speed
    assert_eq!(VoiceModeKeys::from_text(Some("[voice]\nspeed = 1\n")).speed, 1.0);
    assert_eq!(speed_written(1.0), "1.0");
    assert_eq!(speed_written(9.0), "1.6");
}

#[test]
fn mistral_listens_in_realtime_and_speaks_the_others_do_not_yet() {
    assert_eq!(realtime_model("mistral"), Some("voxtral-mini-transcribe-realtime-2602"));
    assert_eq!(tts_model("mistral"), Some("voxtral-mini-tts-2603"));
    for p in ["openai", "groq", "elevenlabs", "deepgram"] {
        assert_eq!((realtime_model(p), tts_model(p)), (None, None), "{p}");
    }
}

#[test]
fn the_calls_take_the_voice_roles_provider_and_its_key() {
    let store = Store::default();
    let keys = Keys { env: &mistral_key, store: &store, files: &[] };
    let s = setup("");
    assert_eq!(s.voice_provider(), "mistral");
    let rt = s.realtime_call(&keys).unwrap().unwrap();
    assert_eq!(rt.name, "mistral/voxtral-mini-transcribe-realtime-2602");
    assert_eq!((rt.model.as_str(), rt.key.as_str(), rt.provider_name.as_str()), ("voxtral-mini-transcribe-realtime-2602", "sk-secret-mistral", "Mistral"));
    assert!(!rt.base_url.ends_with('/') && rt.base_url.starts_with("https://"), "{}", rt.base_url);
    // the key never shows
    assert!(!format!("{:?}", rt).contains("sk-secret"));
    let tts = s.tts_call(&VoiceModeKeys::default(), &keys).unwrap();
    assert_eq!(tts.name, "mistral/voxtral-mini-tts-2603");
    // no key: one line that says what to do
    let none = Keys { env: &no_env, store: &store, files: &[] };
    let e = s.tts_call(&VoiceModeKeys::default(), &none).unwrap_err();
    assert!(e.contains("needs a Mistral key") && e.contains("MISTRAL_API_KEY"), "{e}");
}

#[test]
fn another_voice_provider_has_no_realtime_and_no_voice_by_default() {
    let store = Store::default();
    let env = |k: &str| (k == "OPENAI_API_KEY" || k == "MISTRAL_API_KEY").then(|| "k".to_string());
    let keys = Keys { env: &env, store: &store, files: &[] };
    let s = setup("[voice]\nmodel = \"openai/whisper-1\"\n");
    assert_eq!(s.realtime_call(&keys).unwrap(), None);
    // one key, one company hears you: no silent move to Mistral
    let e = s.tts_call(&VoiceModeKeys::default(), &keys).unwrap_err();
    assert!(e.contains("OpenAI can't speak yet"), "{e}");
    // picked by hand, Mistral speaks
    let k = VoiceModeKeys { tts_model: Some("mistral/voxtral-mini-tts-2603".into()), ..Default::default() };
    assert_eq!(s.tts_call(&k, &keys).unwrap().provider, "mistral");
    // a tts_model of a provider that does not speak
    let k = VoiceModeKeys { tts_model: Some("openai/tts-1".into()), ..Default::default() };
    assert!(s.tts_call(&k, &keys).unwrap_err().contains("can't speak yet"));
}

#[test]
fn the_small_jobs_call_has_its_family() {
    let store = Store::default();
    let keys = Keys { env: &mistral_key, store: &store, files: &[] };
    let s = setup("small_model = \"mistral/mistral-small-latest\"\n");
    let (call, family) = s.small_call(&keys).unwrap();
    assert_eq!(call.name, "mistral/mistral-small-latest");
    assert!(!family.is_empty());
}
