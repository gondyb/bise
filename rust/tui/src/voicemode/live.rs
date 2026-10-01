//! Voice mode in the app (owner: voice-mode, the lead; plan §6): ctrl+r
//! twice enters, the keys while it is on, the run loop's pump (the
//! controller's acts → the hub), the agent's feed events → the
//! controller, the real ports.

use super::config::{self, VoiceModeConfig};
use super::turn::{Act, Jobs, Ports, SpaceKey, VoiceMode};
use crate::app::App;
use crate::feed::push_event;
use crate::wire::Ev;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// The terminal reports key releases (the kitty protocol's event types,
/// set by run.rs when the terminal confirmed them): hold space is exact.
pub static RELEASES: AtomicBool = AtomicBool::new(false);

/// The real ports: the default mic and speaker, the listener of the
/// voice role, Voxtral TTS, the small-jobs "on it".
fn live_ports(listen: &super::ListenJob) -> Result<Ports, String> {
    let speaker = super::audio::open_speaker().unwrap_or_else(|_| Box::new(super::audio::NoSpeaker));
    Ok(Ports {
        mic: Box::new(super::audio::CpalMic),
        vad: Box::new(super::vad::Vad::new()),
        speaker,
        listener: super::listen::listener_for(listen),
        synth: Box::new(super::tts::VoxtralTts),
        acker: Box::new(super::ack::SmallAck),
        route: super::route::output_route(),
    })
}

// ---- BISE_VOICE_FAKE: voice mode with no mic, no sound, no network ----
//
// BISE_VOICE_FAKE=<a 16 kHz mono WAV> plays that file as the mic, paced
// (then silence); the listener hears BISE_VOICE_FAKE_HEARD (default
// below) at each flush; the voice is silence as long as the sentence
// would take, on a speaker with a real clock and no device. For the
// tmux e2e and the designer's captures.

const FAKE_HEARD: &str = "what is the state of the build";

struct FakeListener(String);

impl super::Listener for FakeListener {
    fn start(
        &self,
        _job: super::ListenJob,
        audio: std::sync::mpsc::Receiver<super::ListenMsg>,
        events: std::sync::mpsc::Sender<super::Heard>,
        _cancel: std::sync::Arc<AtomicBool>,
    ) {
        let heard = self.0.clone();
        std::thread::spawn(move || {
            while let Ok(m) = audio.recv() {
                if m == super::ListenMsg::Flush {
                    let _ = events.send(super::Heard::Text(format!(" {}", heard)));
                    let _ = events.send(super::Heard::Flushed);
                }
            }
        });
    }
}

struct FakeSynth;

impl super::Synthesizer for FakeSynth {
    fn start(
        &self,
        job: super::SayJob,
        text: String,
        events: std::sync::mpsc::Sender<super::Synth>,
        cancel: std::sync::Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            let n = (super::timing::estimate(&text, job.speed).as_secs_f64() * super::TTS_RATE as f64) as usize;
            // a soft tone, so the mouth moves; nobody hears it
            let pcm: Vec<f32> = (0..n).map(|i| 0.2 * ((i as f32) * 0.05).sin()).collect();
            for chunk in pcm.chunks(super::TTS_RATE as usize / 10) {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let _ = events.send(super::Synth::Audio(chunk.to_vec()));
            }
            let _ = events.send(super::Synth::Done);
        });
    }
}

fn fake_ports(wav: &str) -> Result<(Ports, Jobs), String> {
    let mic = super::audio::ScriptedMic::from_wav_file(std::path::Path::new(wav))?;
    let heard = std::env::var("BISE_VOICE_FAKE_HEARD").ok().filter(|h| !h.trim().is_empty());
    let ports = Ports {
        mic: Box::new(mic),
        vad: Box::new(super::vad::Vad::new()),
        speaker: super::audio::silent_speaker(),
        listener: Box::new(FakeListener(heard.unwrap_or_else(|| FAKE_HEARD.into()))),
        synth: Box::new(FakeSynth),
        acker: Box::new(super::ack::SmallAck),
        route: super::Route::Headphones,
    };
    let fake = |name: &str| super::Endpoint {
        name: name.into(),
        provider_name: "fake".into(),
        base_url: String::new(),
        model: name.into(),
        key: String::new(),
    };
    let listen = config::listen_job().unwrap_or_else(|_| super::ListenJob {
        realtime: None,
        batch: crate::voice::VoiceJob {
            name: "fake".into(),
            provider_name: "fake".into(),
            billing_url: String::new(),
            api: "openai".into(),
            base_url: String::new(),
            model: "fake".into(),
            key: String::new(),
            language: None,
            vocabulary: Vec::new(),
        },
    });
    let say = super::SayJob { api: fake("fake-tts"), voice: "fake".into(), speed: 1.0 };
    Ok((ports, Jobs { listen, say: Ok(say), ack: None }))
}

fn live_jobs(cfg: &VoiceModeConfig) -> Result<Jobs, String> {
    Ok(Jobs { listen: config::listen_job()?, say: config::say_job(cfg), ack: config::ack_job().ok() })
}

/// A faint line in the thread in view.
fn note(app: &mut App, text: String) {
    push_event(&mut app.events, &mut app.cache, Ev::Info(format!("· {}", text)));
}

/// ctrl+r twice (or the first-time screen's "start"): voice mode with
/// the agent in view. A failure is one line in the thread.
pub(crate) fn enter(app: &mut App) {
    if app.voice_mode.is_some() {
        return;
    }
    let cfg = config::load();
    let now = Instant::now();
    let fake = std::env::var("BISE_VOICE_FAKE").ok().filter(|v| !v.is_empty());
    let ready = match fake {
        Some(wav) => fake_ports(&wav),
        None => live_jobs(&cfg).and_then(|jobs| Ok((live_ports(&jobs.listen)?, jobs))),
    };
    let started = ready.and_then(|(ports, jobs)| {
        VoiceMode::start(&app.sb.focus, ports, jobs, cfg, RELEASES.load(Ordering::Relaxed), now)
    });
    match started {
        Ok(vm) => {
            app.voice_mode = Some(vm);
            note(app, format!("voice mode · {}", crate::when::mark_now(crate::when::now_ms())));
        }
        Err(e) => {
            push_event(&mut app.events, &mut app.cache, Ev::Warn(format!("voice mode: {}", e)));
        }
    }
}

/// ctrl+r twice: voice mode, after the first-time "who hears you"
/// screen when it was never shown (voice-settings' settings.rs).
pub(crate) fn request(app: &mut App) {
    if !config::load().seen_privacy && std::env::var("BISE_VOICE_FAKE").map_or(true, |v| v.is_empty()) {
        super::settings::request(super::settings::Open::Privacy);
        return;
    }
    enter(app);
}

/// How the first-time screen closed: start (hands-free or hold-to-talk,
/// both saved by the screen) or not now.
pub(crate) fn after_settings(app: &mut App, open: super::settings::Open, out: super::settings::Out) {
    use super::settings::{Open, Out};
    if open == Open::Privacy && matches!(out, Out::Start | Out::HoldOnly) {
        enter(app);
    }
}

/// ▸ hear it on /voice (only on the user's press): the sample sentence
/// in the chosen voice, on the default speaker, off the UI thread.
pub(crate) fn hear(job: super::SayJob, text: String) {
    std::thread::spawn(move || {
        let Ok(mut speaker) = super::audio::open_speaker() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        use super::Synthesizer;
        super::tts::VoxtralTts.start(job, text, tx, cancel);
        while let Ok(ev) = rx.recv() {
            match ev {
                super::Synth::Audio(pcm) => speaker.push(1, &pcm),
                _ => break,
            }
        }
        speaker.end(1);
        let until = Instant::now() + std::time::Duration::from_secs(30);
        while !speaker.done(1) && Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

/// esc: voice mode ends (the mic closes when the controller drops).
pub(crate) fn leave(app: &mut App) {
    if let Some(mut vm) = app.voice_mode.take() {
        let acts = vm.leave(Instant::now());
        apply(app, acts);
    }
}

fn apply(app: &mut App, acts: Vec<Act>) {
    for a in acts {
        match a {
            Act::Send { agent, text } => {
                if !answer_by_voice(app, &text) {
                    app.sb.send(serde_json::json!({"op": "input", "focus": agent, "text": text, "voice": true}));
                    app.pending = true;
                }
            }
            Act::Interrupt { agent } => {
                app.sb.send(serde_json::json!({"op": "interrupt", "agent": agent}));
                app.interrupt_requested = true;
            }
            Act::Note(t) => note(app, t),
        }
    }
}

/// Your words as an answer to the open inbox item, when they are one
/// ("allow", "the first one"); plan §4.4: the heard line shows on the
/// item and in the pane for 1.5 s, the answer counts as the key would.
/// False: send them as words. "yes", "ok", "mm" never allow.
fn answer_by_voice(app: &mut App, text: &str) -> bool {
    use super::answers;
    let Some(q) = crate::sb::voice_question(app) else { return false };
    let now = Instant::now();
    let (i, line) = if q.approval {
        if !answers::is_allow(text) {
            return false;
        }
        // the first option is the item's "allow" (once)
        let Some(label) = q.options.first() else { return false };
        (0, answers::heard_allow(text, label))
    } else {
        let Some(i) = answers::pick(text, &q.options) else { return false };
        (i, answers::heard_line(text, i + 1, &q.options[i]))
    };
    crate::sb::show_heard(q.id, line.clone(), now);
    if let Some(vm) = app.voice_mode.as_mut() {
        vm.show_heard(line, now);
    }
    crate::sb::answer_by_voice(app, q.id, i)
}

/// The run loop's tick: the dictation picker a lone ctrl+r asked for,
/// the agent in view, the controller's step.
pub(crate) fn pump(app: &mut App) {
    if app.voice_setup_at.is_some_and(|t| Instant::now() >= t) {
        app.voice_setup_at = None;
        crate::input::open_voice_setup(app, true);
    }
    let Some(vm) = app.voice_mode.as_mut() else { return };
    if vm.agent() != app.sb.focus {
        vm.set_agent(&app.sb.focus.clone());
    }
    let acts = vm.tick(Instant::now());
    apply(app, acts);
}

/// The feed events of `agent`'s line (live, not replayed): its messages
/// are said, its turn's state counts for a cut.
pub(crate) fn on_events(app: &mut App, agent: &str, evs: &[Ev]) {
    let Some(vm) = app.voice_mode.as_mut() else { return };
    let lang = config::load().language;
    for ev in evs {
        match ev {
            Ev::Assistant(text) => vm.on_message(agent, text, super::speak::speakable(text, lang.as_deref())),
            Ev::Turn => vm.on_turn(agent, true),
            Ev::TurnDone | Ev::Idle => vm.on_turn(agent, false),
            _ => {}
        }
    }
}

/// A space before the handlers (they never see releases): the floor.
/// True when voice mode took it.
pub(crate) fn space(app: &mut App, k: &KeyEvent) -> bool {
    let Some(vm) = app.voice_mode.as_mut() else { return false };
    if vm.typing() || k.code != KeyCode::Char(' ') || !(k.modifiers - KeyModifiers::SHIFT).is_empty() {
        return false;
    }
    let kind = match k.kind {
        KeyEventKind::Press => SpaceKey::Press,
        KeyEventKind::Repeat => SpaceKey::Repeat,
        KeyEventKind::Release => SpaceKey::Release,
    };
    vm.space(kind, Instant::now());
    true
}

/// The keys while voice mode is on (after help, the terminal…): esc
/// leaves, m mutes, tab types; typing, esc and tab come back to talking
/// and ⏎ sends as usual (its answer is said). Ctrl and alt keys go on
/// to their handlers (ctrl+c, the agents). True: taken.
pub(crate) fn key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(vm) = app.voice_mode.as_mut() else { return false };
    let plain = (k.modifiers - KeyModifiers::SHIFT).is_empty();
    if vm.typing() {
        match k.code {
            KeyCode::Esc | KeyCode::Tab if plain => {
                vm.set_typing(false);
                true
            }
            KeyCode::Enter if plain && !app.ed.text.trim().is_empty() => {
                vm.typed_sent();
                false
            }
            _ => false,
        }
    } else {
        match k.code {
            KeyCode::Esc => {
                leave(app);
                true
            }
            KeyCode::Tab if plain => {
                vm.set_typing(true);
                true
            }
            KeyCode::Char('m') if plain => {
                vm.toggle_mute();
                true
            }
            // the composer is the pane: letters and edits go nowhere
            _ if plain => true,
            _ => false,
        }
    }
}

/// ctrl+r then ctrl+r within DOUBLE_CTRL_R: voice mode. `last` keeps
/// the first press. True: the second press, voice mode is asked for.
pub(crate) fn double_ctrl_r(last: &mut Option<Instant>, k: &KeyEvent, now: Instant) -> bool {
    if k.code != KeyCode::Char('r') || k.modifiers != KeyModifiers::CONTROL {
        return false;
    }
    if last.take().is_some_and(|t| now.duration_since(t) <= super::DOUBLE_CTRL_R) {
        return true;
    }
    *last = Some(now);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn ctrl_r_twice_within_400_ms_is_voice_mode_and_a_third_starts_over() {
        let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        let t = Instant::now();
        let mut last = None;
        assert!(!double_ctrl_r(&mut last, &r, t));
        assert!(double_ctrl_r(&mut last, &r, t + Duration::from_millis(300)));
        assert!(!double_ctrl_r(&mut last, &r, t + Duration::from_millis(400)));
        assert!(!double_ctrl_r(&mut last, &r, t + Duration::from_millis(900)), "too slow: a new first press");
        let x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(!double_ctrl_r(&mut last, &x, t + Duration::from_millis(1000)));
    }
}
