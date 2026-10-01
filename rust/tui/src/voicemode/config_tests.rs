//! Voice mode's settings: config.toml read and written, the jobs and
//! their keys (never printed).

use super::*;

fn none(_: &str) -> Option<String> {
    None
}

fn keys_env(k: &str) -> Option<String> {
    matches!(k, "MISTRAL_API_KEY" | "OPENAI_API_KEY").then(|| format!("sk-test-{}", k.len()))
}

fn with<T>(text: &str, f: impl FnOnce(&Setup, &Keys) -> T) -> T {
    let setup = Setup::from_text(Some(text), &none);
    let store = Store::default();
    let keys = Keys { env: &keys_env, store: &store, files: &[] };
    f(&setup, &keys)
}

#[test]
fn no_config_is_the_defaults() {
    let c = VoiceModeConfig::of_text(None, &none);
    assert_eq!(c, VoiceModeConfig::default());
    assert_eq!(c.tts_model, "mistral/voxtral-mini-tts-2603");
}

#[test]
fn the_config_is_read_with_the_dictations_language() {
    let c = VoiceModeConfig::of_text(
        Some("[voice]\nlanguage = \"fr\"\nlisten = \"hands-free\"\nspeed = 1.3\nread_aloud = \"nothing\"\nsounds = false\nseen_privacy = true\n"),
        &none,
    );
    assert_eq!((c.listen, c.read_aloud, c.sounds, c.seen_privacy), (ListenMode::HandsFree, ReadAloud::Nothing, false, true));
    assert_eq!(c.language.as_deref(), Some("fr"));
    assert!((c.speed - 1.3).abs() < 1e-6);
    // another voice role's provider: no TTS until one is picked
    let c = VoiceModeConfig::of_text(Some("[voice]\nmodel = \"openai/whisper-1\"\n"), &none);
    assert_eq!(c.tts_model, "");
}

#[test]
fn saving_writes_only_what_changed_and_keeps_the_rest() {
    let text = "# mine\nmodel = \"anthropic/claude-sonnet-4-5\"\n\n[voice]\nmodel = \"mistral/voxtral-mini-latest\" # dictation\n";
    let mut c = VoiceModeConfig::of_text(Some(text), &none);
    // nothing changed: nothing written
    assert_eq!(with_config(text, &c, &none), text);
    c.seen_privacy = true;
    c.speed = 1.2;
    let t = with_config(text, &c, &none);
    assert!(t.starts_with("# mine\n") && t.contains("# dictation"), "{t}");
    assert!(t.contains("seen_privacy = true") && t.contains("speed = 1.2"), "{t}");
    assert!(!t.contains("listen") && !t.contains("tts_model") && !t.contains("sounds"), "{t}");
    assert_eq!(VoiceModeConfig::of_text(Some(&t), &none), c);
    // back to a default value: the line stays, with the value
    c.speed = 1.0;
    let t2 = with_config(&t, &c, &none);
    assert!(t2.contains("speed = 1.0"), "{t2}");
    // the language: set, then auto (the key goes)
    c.language = Some("fr".into());
    let t3 = with_config(&t2, &c, &none);
    assert!(t3.contains("language = \"fr\""), "{t3}");
    c.language = None;
    assert!(!with_config(&t3, &c, &none).contains("language"));
}

#[test]
fn saving_with_no_voice_table_makes_one() {
    let c = VoiceModeConfig { listen: ListenMode::Hold, ..VoiceModeConfig::default() };
    let t = with_config("model = \"x/y\"\n", &c, &none);
    assert!(t.contains("[voice]\nlisten = \"hold\""), "{t}");
}

#[test]
fn the_tts_model_and_the_voice_are_written_only_when_picked() {
    let mut c = VoiceModeConfig::of_text(Some(""), &none);
    c.voice = super::super::tts::DEFAULT_VOICE.into();
    assert!(!with_config("", &c, &none).contains("tts_"));
    c.voice = "a-voice".into();
    assert!(with_config("", &c, &none).contains("tts_voice = \"a-voice\""));
}

#[test]
fn the_listen_job_is_the_batch_transcribe_3_never_realtime() {
    let j = with("", listen_job_of).unwrap();
    // round 2: no realtime model (Voxtral Mini's is retired)
    assert!(j.realtime.is_none(), "{j:?}");
    assert_eq!(j.batch.name, "mistral/voxtral-transcribe-3");
    assert_eq!(j.batch.key, "sk-test-15");
    // the key is never printed
    let shown = format!("{:?}", j);
    assert!(!shown.contains("sk-test"), "{shown}");
    let j = with("[voice]\nmodel = \"openai/whisper-1\"\n", listen_job_of).unwrap();
    assert!(j.realtime.is_none());
    assert_eq!(j.batch.name, "openai/whisper-1");
}

#[test]
fn the_say_job_takes_the_voice_and_the_speed() {
    let mut c = VoiceModeConfig::of_text(Some(""), &none);
    c.voice = "v1".into();
    c.speed = 1.4;
    let j = with("", |s, k| say_job_of(s, k, &c)).unwrap();
    assert_eq!((j.api.name.as_str(), j.voice.as_str()), ("mistral/voxtral-mini-tts-2603", "v1"));
    assert!((j.speed - 1.4).abs() < 1e-6);
    assert!(!format!("{:?}", j).contains("sk-test"));
    // a voice role elsewhere: the one-line reason
    let text = "[voice]\nmodel = \"openai/whisper-1\"\n";
    let c = VoiceModeConfig::of_text(Some(text), &none);
    let e = with(text, |s, k| say_job_of(s, k, &c)).unwrap_err();
    assert!(e.contains("can't speak yet") && !e.contains('\n'), "{e}");
}

#[test]
fn the_ack_job_is_the_small_jobs_model() {
    let j = with("small_model = \"mistral/mistral-small-latest\"\n", ack_job_of).unwrap();
    assert_eq!(j.api.name, "mistral/mistral-small-latest");
    assert!(!j.family.is_empty());
    // no key: the line says which
    let setup = Setup::from_text(Some("small_model = \"mistral/mistral-small-latest\"\n"), &none);
    let store = Store::default();
    let keys = Keys { env: &none, store: &store, files: &[] };
    assert!(ack_job_of(&setup, &keys).unwrap_err().contains("MISTRAL_API_KEY"));
}

#[test]
fn the_speech_to_text_model_is_written_in_voice_and_the_default_unwritten() {
    let default = Setup::from_text(Some(""), &none).catalog.default_voice_model;
    let t = "[voice]\nlanguage = \"fr\"\n";
    let t2 = with_stt_model(t, "openai/gpt-transcribe", &none);
    assert!(t2.contains("model = \"openai/gpt-transcribe\"") && t2.contains("language = \"fr\""), "{t2}");
    assert_eq!(Setup::from_text(Some(&t2), &none).voice.model, "openai/gpt-transcribe");
    // the same again: nothing changes
    assert_eq!(with_stt_model(&t2, "openai/gpt-transcribe", &none), t2);
    // back to the catalog's default: the key goes (a newer default applies)
    let t3 = with_stt_model(&t2, &default, &none);
    assert!(!t3.contains("model ="), "{t3}");
    assert!(t3.contains("language = \"fr\""), "{t3}");
    assert_eq!(with_stt_model("", &default, &none), "");
    // BISE_VOICE_MODEL is not what the file says
    let env = |k: &str| (k == "BISE_VOICE_MODEL").then(|| "openai/gpt-transcribe".to_string());
    assert!(with_stt_model("", "openai/gpt-transcribe", &env).contains("model = \"openai/gpt-transcribe\""));
}
