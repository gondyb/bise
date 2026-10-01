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
    let started = live_jobs(&cfg).and_then(|jobs| {
        let ports = live_ports(&jobs.listen)?;
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
    enter(app);
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
/// ("allow", "the first one"); plan §4.4. False: send them as words.
fn answer_by_voice(_app: &mut App, _text: &str) -> bool {
    false
}

/// The run loop's tick: the agent in view, the controller's step.
pub(crate) fn pump(app: &mut App) {
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
