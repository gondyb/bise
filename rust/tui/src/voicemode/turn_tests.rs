//! The turn controller over the fakes (plan §5): every scenario drives
//! the clock by hand, the mic by blocks, the listener by words.

use super::super::config::{ListenMode, VoiceModeConfig};
use super::super::fakes::Fakes;
use super::super::{Heard, ListenMsg, LitState, MicBlock, Phase, Route, Sentence, Spoken, Synth, Who, Word, WordState};
use super::*;
use std::time::{Duration, Instant};

const MS: fn(u64) -> Duration = Duration::from_millis;

struct Rig {
    f: Fakes,
    vm: VoiceMode,
    t0: Instant,
    /// the mic's clock: the next block's capture time, in ms from t0
    at: u64,
}

impl Rig {
    fn new(route: Route) -> Rig {
        Rig::with(route, VoiceModeConfig::default(), true, true)
    }

    fn with(route: Route, cfg: VoiceModeConfig, releases: bool, voice: bool) -> Rig {
        let f = Fakes::new();
        let t0 = Instant::now();
        let vm = VoiceMode::start("main", f.ports(route), f.jobs(voice), cfg, releases, t0).unwrap();
        Rig { f, vm, t0, at: 0 }
    }

    fn now(&self) -> Instant {
        self.t0 + MS(self.at)
    }

    /// 100 ms blocks of speech (loud) or silence, ticking after each.
    fn audio(&mut self, ms: u64, loud: bool) -> Vec<Act> {
        let mut acts = Vec::new();
        for _ in 0..ms / 100 {
            let pcm = vec![if loud { 5000 } else { 10 }; 1600];
            let tx = self.f.mic.lock().unwrap().blocks.clone().unwrap();
            tx.send(MicBlock { pcm, at: self.t0 + MS(self.at) }).unwrap();
            self.at += 100;
            acts.extend(self.vm.tick(self.now()));
        }
        acts
    }

    fn talk(&mut self, ms: u64) -> Vec<Act> {
        self.audio(ms, true)
    }

    fn quiet(&mut self, ms: u64) -> Vec<Act> {
        self.audio(ms, false)
    }

    fn tick(&mut self) -> Vec<Act> {
        self.vm.tick(self.now())
    }

    /// The listener hears words (the last session).
    fn hear(&self, words: &str) {
        let l = self.f.listen.lock().unwrap();
        l.last().unwrap().heard.send(Heard::Text(words.into())).unwrap();
    }

    fn flushed(&self) {
        let l = self.f.listen.lock().unwrap();
        l.last().unwrap().heard.send(Heard::Flushed).unwrap();
    }

    /// What the controller sent the listener (all sessions), as tags.
    fn sent(&self) -> Vec<String> {
        let l = self.f.listen.lock().unwrap();
        let mut out = Vec::new();
        for s in l.iter() {
            while let Ok(m) = s.audio.try_recv() {
                out.push(match m {
                    ListenMsg::Audio(_) => "audio".to_string(),
                    ListenMsg::Flush => "flush".to_string(),
                    ListenMsg::Clear => "clear".to_string(),
                });
            }
        }
        out
    }

    fn phase(&self) -> Phase {
        self.vm.view(self.now()).phase
    }

    /// A full turn said and sent: "fix the tests".
    fn turn(&mut self, words: &str) -> Vec<Act> {
        self.talk(500);
        self.hear(words);
        let mut acts = self.quiet(1600);
        self.flushed();
        acts.extend(self.tick());
        acts
    }

    /// The agent says `text` as one sentence per '.'.
    fn message(&mut self, text: &str) {
        self.vm.on_message("main", text, spoken(text));
    }

    /// The synth of call `i` streams `n` samples then ends.
    fn synth_all(&self, i: usize, n: usize) {
        let s = self.f.synth.lock().unwrap();
        s[i].events.send(Synth::Audio(vec![0.1; n])).unwrap();
        s[i].events.send(Synth::Done).unwrap();
    }

    fn clock(&self, utt: UttId, ms: u64) {
        self.f.speaker.lock().unwrap().clock = Some((utt, MS(ms)));
    }

    fn done(&self, utt: UttId) {
        let mut s = self.f.speaker.lock().unwrap();
        s.done.insert(utt);
        s.clock = None;
    }
}

/// A message as said, one sentence per '.', every word mapped to itself.
fn spoken(text: &str) -> Spoken {
    let mut sentences = Vec::new();
    let mut base = 0;
    for part in text.split_inclusive('.') {
        let mut words = Vec::new();
        let mut off = 0;
        for w in part.split_whitespace() {
            let i = part[off..].find(w).unwrap() + off;
            words.push(Word { say: i..i + w.len(), src: Some(base + i..base + i + w.len()) });
            off = i + w.len();
        }
        if !words.is_empty() {
            sentences.push(Sentence { say: part.to_string(), words });
        }
        base += part.len();
    }
    Spoken { sentences, more: false }
}

fn sends(acts: &[Act]) -> Vec<String> {
    acts.iter()
        .filter_map(|a| match a {
            Act::Send { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_turn_fills_about_to_answer_then_is_sent_when_the_listener_has_every_word() {
    let mut r = Rig::new(Route::Headphones);
    assert_eq!(r.phase(), Phase::Listening);
    r.talk(800);
    r.hear(" fix the");
    r.tick();
    assert_eq!(r.phase(), Phase::Hearing);
    let v = r.vm.view(r.now());
    assert_eq!(v.who, Who::You);
    assert_eq!(v.words.last().unwrap(), &("the".to_string(), WordState::Partial));
    r.hear(" tests");
    r.quiet(900);
    match r.phase() {
        Phase::AboutToAnswer { fill } => assert!(fill > 0.3 && fill < 0.7, "{fill}"),
        p => panic!("{p:?}"),
    }
    assert!(sends(&r.quiet(700)).is_empty(), "waits for the listener");
    assert!(r.sent().ends_with(&["flush".to_string()]));
    r.flushed();
    let acts = r.tick();
    assert_eq!(acts, vec![Act::Send { agent: "main".into(), text: "fix the tests".into() }]);
}

#[test]
fn talking_again_empties_the_fill() {
    let mut r = Rig::new(Route::Headphones);
    r.talk(500);
    r.hear(" so");
    r.quiet(1000);
    assert!(matches!(r.phase(), Phase::AboutToAnswer { .. }));
    r.talk(300);
    assert_eq!(r.phase(), Phase::Hearing);
    r.quiet(1000);
    assert!(!r.sent().contains(&"flush".to_string()), "the fill restarted from your last word");
}

#[test]
fn a_breath_inside_a_sentence_never_starts_the_fill() {
    let mut r = Rig::new(Route::Headphones);
    r.talk(500);
    r.quiet(200);
    assert_eq!(r.phase(), Phase::Hearing);
}

#[test]
fn noise_without_words_sends_nothing() {
    let mut r = Rig::new(Route::Headphones);
    r.talk(300);
    r.quiet(1600);
    r.flushed();
    assert!(sends(&r.tick()).is_empty());
    assert_eq!(r.phase(), Phase::Listening);
}

#[test]
fn a_space_tap_sends_at_once_and_a_hold_keeps_the_floor() {
    let mut r = Rig::new(Route::Headphones);
    r.talk(500);
    r.hear(" check the build");
    r.vm.space(SpaceKey::Press, r.now());
    r.at += 100;
    r.vm.space(SpaceKey::Release, r.now());
    r.tick();
    assert!(r.sent().contains(&"flush".to_string()));
    r.flushed();
    assert_eq!(sends(&r.tick()), ["check the build"]);

    // hold: no fill however long the silence
    r.talk(400);
    r.hear(" and then");
    r.vm.space(SpaceKey::Press, r.now());
    r.quiet(3000);
    assert_eq!(r.phase(), Phase::Holding);
    assert!(!r.sent().contains(&"flush".to_string()));
    r.vm.space(SpaceKey::Release, r.now());
    // let go after a long hold: the fill starts again from the silence
    r.talk(200);
    r.hear(" the docs");
    r.quiet(1600);
    assert!(r.sent().contains(&"flush".to_string()));
    r.flushed();
    assert_eq!(sends(&r.tick()), ["and then the docs"]);
}

#[test]
fn without_release_events_a_lone_press_is_a_tap_and_repeats_are_a_hold() {
    let mut r = Rig::with(Route::Headphones, VoiceModeConfig::default(), false, true);
    r.talk(500);
    r.hear(" hello");
    // repeats every 50 ms: a hold
    for _ in 0..20 {
        r.vm.space(SpaceKey::Press, r.now());
        r.at += 50;
        r.tick();
    }
    assert_eq!(r.phase(), Phase::Holding);
    r.at += 200;
    r.tick();
    assert_ne!(r.phase(), Phase::Holding, "the repeats stopped: the hold ended");
    assert!(!r.sent().contains(&"flush".to_string()));
    // a lone press: a tap, sent once the repeat delay passed
    r.vm.space(SpaceKey::Press, r.now());
    r.at += 500;
    r.tick();
    assert!(r.sent().contains(&"flush".to_string()));
}

#[test]
fn the_answer_is_said_sentence_by_sentence_and_its_words_light_up_in_step() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" what broke");
    r.vm.on_turn("main", true);
    r.tick();
    assert_eq!(r.phase(), Phase::Working);
    let msg = "The login test. It waits now.";
    r.message(msg);
    r.tick();
    assert_eq!(r.phase(), Phase::Speaking);
    assert_eq!(r.f.synth.lock().unwrap().len(), 1, "one sentence at a time");
    assert_eq!(r.f.synth.lock().unwrap()[0].text, "The login test.");
    r.synth_all(0, 24_000);
    r.tick();
    assert_eq!(r.f.synth.lock().unwrap()[1].text, " It waits now.");
    assert!(r.f.speaker.lock().unwrap().ended.contains(&1));
    // half of the first sentence played
    r.clock(1, 500);
    r.tick();
    let lit = r.vm.lit().unwrap();
    assert_eq!((lit.agent.as_str(), lit.text.as_str()), ("main", msg));
    let said: Vec<&str> = lit.spans.iter().filter(|(_, s)| *s == LitState::Said).map(|(rg, _)| &msg[rg.clone()]).collect();
    assert_eq!(said, ["The"]);
    let v = r.vm.view(r.now());
    assert_eq!(v.who, Who::Agent("main".into()));
    assert_eq!(v.words[0], ("The".to_string(), WordState::Said));
    assert_eq!(v.words[1].1, WordState::ToSay);
    // the first sentence over, the second plays
    r.done(1);
    r.synth_all(1, 24_000);
    r.clock(2, 100);
    r.tick();
    let lit = r.vm.lit().unwrap();
    assert_eq!(lit.spans.iter().filter(|(_, s)| *s == LitState::Said).count(), 3);
    r.done(2);
    r.tick();
    assert!(r.vm.lit().unwrap().spans.iter().all(|(_, s)| *s == LitState::Said));
    r.vm.on_turn("main", false);
    assert_eq!(r.phase(), Phase::Listening);
}

#[test]
fn mm_and_ok_never_cut_the_agent_off() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    r.vm.on_turn("main", true);
    r.message("Running the tests now. It takes a minute.");
    r.tick();
    r.talk(700);
    r.hear(" mm ok");
    r.talk(200);
    assert_eq!(r.f.speaker.lock().unwrap().stops, 0);
    assert_eq!(r.phase(), Phase::Speaking);
    r.quiet(500);
    assert!(r.sent().contains(&"clear".to_string()), "the backchannel is dropped");
    assert_eq!(r.phase(), Phase::Speaking);
}

#[test]
fn real_words_cut_in_stop_the_voice_and_interrupt_the_running_turn() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    r.vm.on_turn("main", true);
    let msg = "Running the tests now.";
    r.message(msg);
    r.tick();
    r.synth_all(0, 48_000);
    r.clock(1, 300);
    r.tick();
    r.talk(200);
    r.hear(" wait stop");
    let acts = r.talk(300);
    assert!(acts.contains(&Act::Interrupt { agent: "main".into() }), "{acts:?}");
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
    assert_eq!(r.phase(), Phase::CutIn);
    let lit = r.vm.lit().unwrap();
    assert!(lit.spans.iter().any(|(_, s)| *s == LitState::Cut));
    // your words go on as the next turn
    assert_eq!(r.vm.view(r.now()).who, Who::You);
    r.hear(" use the other branch");
    r.quiet(1600);
    r.flushed();
    assert_eq!(sends(&r.tick()), ["wait stop use the other branch"]);
}

#[test]
fn a_long_mmmm_cuts_in_anyway() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    r.message("Running the tests now.");
    r.tick();
    r.hear(" mmmm");
    r.talk(1300);
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
}

#[test]
fn on_speakers_the_mic_waits_while_the_agent_talks_and_space_cuts_in() {
    let mut r = Rig::new(Route::Speakers);
    r.turn(" go");
    r.sent();
    r.message("Running the tests now.");
    r.tick();
    r.talk(1500);
    assert!(r.sent().is_empty(), "its own voice never reaches the listener");
    assert_eq!(r.f.speaker.lock().unwrap().stops, 0);
    r.vm.space(SpaceKey::Press, r.now());
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
    r.talk(300);
    assert!(r.sent().contains(&"audio".to_string()));
}

#[test]
fn without_a_voice_the_words_show_all_said_and_nothing_plays() {
    let mut r = Rig::with(Route::Headphones, VoiceModeConfig::default(), true, false);
    assert!(matches!(r.phase(), Phase::Failed(_)));
    r.turn(" hi");
    r.message("Hello there.");
    r.tick();
    assert!(r.f.synth.lock().unwrap().is_empty());
    assert!(r.vm.lit().unwrap().spans.iter().all(|(_, s)| *s == LitState::Said));
}

#[test]
fn a_failed_listener_shows_its_line_and_the_next_words_open_a_new_one() {
    let mut r = Rig::new(Route::Headphones);
    r.talk(300);
    r.f.listen.lock().unwrap()[0].heard.send(Heard::Failed("voxtral said no".into())).unwrap();
    r.tick();
    assert_eq!(r.phase(), Phase::Failed("voxtral said no".into()));
    r.talk(200);
    assert_eq!(r.f.listen.lock().unwrap().len(), 2);
}

#[test]
fn messages_before_any_turn_are_not_said_unless_read_aloud_is_all() {
    let mut r = Rig::new(Route::Headphones);
    r.message("Unasked news.");
    r.tick();
    assert!(r.f.synth.lock().unwrap().is_empty());
    let cfg = VoiceModeConfig { read_aloud: super::super::config::ReadAloud::All, ..VoiceModeConfig::default() };
    let mut r = Rig::with(Route::Headphones, cfg, true, true);
    r.message("Unasked news.");
    r.tick();
    assert_eq!(r.f.synth.lock().unwrap().len(), 1);
}

#[test]
fn hold_mode_listens_only_while_space_is_held_and_the_release_sends() {
    let cfg = VoiceModeConfig { listen: ListenMode::Hold, ..VoiceModeConfig::default() };
    let mut r = Rig::with(Route::Headphones, cfg, true, true);
    assert_eq!(r.phase(), Phase::HoldToTalk);
    r.talk(500);
    assert!(r.sent().is_empty());
    r.vm.space(SpaceKey::Press, r.now());
    r.talk(500);
    r.hear(" ship it");
    r.vm.space(SpaceKey::Release, r.now());
    assert!(r.sent().contains(&"flush".to_string()));
    r.flushed();
    assert_eq!(sends(&r.tick()), ["ship it"]);
}

#[test]
fn mute_and_typing_close_the_ears() {
    let mut r = Rig::new(Route::Headphones);
    r.vm.toggle_mute();
    r.talk(500);
    assert_eq!(r.phase(), Phase::Muted);
    assert!(!r.sent().contains(&"audio".to_string()));
    r.vm.toggle_mute();
    r.vm.set_typing(true);
    r.talk(500);
    assert_eq!(r.phase(), Phase::Typing);
    assert!(!r.sent().contains(&"audio".to_string()));
    r.vm.typed_sent();
    r.message("Got it.");
    r.tick();
    assert_eq!(r.f.synth.lock().unwrap().len(), 1, "a typed message's answer is said too");
}

#[test]
fn muted_the_agent_still_works_speaks_and_kisses() {
    // voice-mute (the user): your mic off greys your side, not its face
    let mut r = Rig::new(Route::Headphones);
    r.turn(" what broke");
    r.vm.on_turn("main", true);
    r.tick();
    r.vm.toggle_mute();
    r.sent(); // your turn's audio, before the mute
    assert_eq!(r.phase(), Phase::Working, "it thinks while you are muted");
    assert!(r.vm.view(r.now()).muted);
    r.message("It waits now.");
    r.tick();
    assert_eq!(r.phase(), Phase::Speaking, "it speaks while you are muted");
    let v = r.vm.view(r.now());
    assert!(v.muted);
    assert_eq!(v.who, Who::Agent("main".into()));
    r.synth_all(0, 24_000);
    r.tick();
    r.done(1);
    r.vm.on_turn("main", false);
    r.tick();
    let v = r.vm.view(r.now());
    assert_eq!(v.phase, Phase::Muted, "at rest");
    assert!(v.kiss_ms.is_some(), "the end-of-turn kiss plays muted too");
    assert!(!r.sent().contains(&"audio".to_string()), "the mic stayed off");
    // m again: you are back, its face unchanged
    r.vm.toggle_mute();
    assert!(!r.vm.view(r.now()).muted);
    assert!(r.vm.view(r.now()).kiss_ms.is_some());
}

#[test]
fn leaving_stops_the_voice_and_counts_the_turns() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" one");
    r.message("First.");
    r.tick();
    r.at += 7 * 60_000;
    let acts = r.vm.leave(r.now());
    assert_eq!(acts, vec![Act::Note("voice mode ended · 7 min · 1 thing said, 1 answer".into())]);
    assert!(r.f.speaker.lock().unwrap().stops >= 1);
}

#[test]
fn another_agent_in_view_stops_the_voice_and_answers_from_then_on() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" one");
    r.message("First.");
    r.tick();
    r.vm.set_agent("cookies");
    assert_eq!(r.vm.agent(), "cookies");
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
    r.vm.on_message("main", "Late.", spoken("Late."));
    r.turn(" two");
    assert_eq!(r.vm.view(r.now()).agent, "cookies");
}

#[test]
fn the_waves_keep_one_level_per_step() {
    let mut r = Rig::new(Route::Headphones);
    r.f.mic.lock().unwrap().level = 0.5;
    r.talk(1000);
    let v = r.vm.view(r.now());
    assert_eq!(v.you_wave.len(), 10);
    assert!(v.you_wave.iter().all(|l| *l == 0.5));
    r.talk(6000);
    assert_eq!(r.vm.view(r.now()).you_wave.len(), super::super::WAVE_LEN);
}

#[test]
fn backchannels_are_words_that_never_count() {
    assert!(only_backchannels(" mm, ok. Right!"));
    assert!(only_backchannels(""));
    assert!(!only_backchannels(" ok wait"));
    assert!(only_backchannels(" ouais d'accord"));
}

// ---- round 2: the echo (the user: on speakers it answered itself) ----

#[test]
fn its_own_words_through_the_mic_are_never_a_turn() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    r.message("Running the tests now.");
    r.tick();
    r.synth_all(0, 24_000);
    r.done(1);
    r.tick();
    assert_ne!(r.phase(), Phase::Speaking);
    // the room gives its last words back
    r.talk(500);
    r.hear(" running the tests now");
    r.quiet(1600);
    r.flushed();
    assert!(sends(&r.tick()).is_empty(), "its echo is dropped");
    // later, your own words go
    r.at += 4_000;
    assert_eq!(sends(&r.turn(" what about the docs")), ["what about the docs"]);
}

#[test]
fn hands_free_on_bare_speakers_never_lets_its_voice_cut_itself() {
    let cfg = VoiceModeConfig { listen: ListenMode::HandsFree, ..VoiceModeConfig::default() };
    let mut r = Rig::with(Route::Speakers, cfg, true, true);
    r.turn(" go");
    r.sent();
    r.message("Running the tests now.");
    r.tick();
    r.talk(1500);
    assert!(r.sent().is_empty(), "the mic waits while it talks");
    assert_eq!(r.f.speaker.lock().unwrap().stops, 0);
}

#[test]
fn with_echo_cancelling_speakers_cut_in_by_voice() {
    let f = Fakes::new();
    let mut p = f.ports(Route::Speakers);
    p.cut_in_by_voice = true;
    let t0 = Instant::now();
    let vm = VoiceMode::start("main", p, f.jobs(true), VoiceModeConfig::default(), true, t0).unwrap();
    let mut r = Rig { f, vm, t0, at: 0 };
    r.turn(" go");
    r.message("Running the tests now.");
    r.tick();
    // no words yet (the batch listener): 0.8 s of speech cuts in
    r.talk(500);
    assert_eq!(r.f.speaker.lock().unwrap().stops, 0, "0.5 s: maybe a mm");
    r.talk(400);
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
}

#[test]
fn without_echo_cancelling_the_mic_waits_for_the_voice_tail() {
    let mut r = Rig::new(Route::Speakers);
    r.turn(" go");
    r.message("Done.");
    r.tick();
    r.synth_all(0, 2_400);
    r.done(1);
    r.tick();
    r.sent();
    r.talk(300);
    assert!(!r.sent().contains(&"audio".to_string()), "the tail is still in the room");
    r.talk(400);
    assert!(r.sent().contains(&"audio".to_string()));
}

#[test]
fn the_echo_check_is_about_most_of_the_words() {
    assert!(is_echo(" running the tests now", "Running the tests now."));
    assert!(is_echo("les tests passent", "Les tests passent tous."));
    assert!(!is_echo(" no stop, use the other branch", "Running the tests now."));
    assert!(!is_echo(" hello", ""));
}

#[test]
fn a_mic_that_cannot_open_keeps_voice_mode_closed() {
    let f = Fakes::new();
    let mut p = f.ports(Route::Headphones);
    p.mic = Box::new(super::super::fakes::BrokenMic);
    let e = VoiceMode::start("main", p, f.jobs(true), VoiceModeConfig::default(), true, Instant::now());
    assert_eq!(e.err().as_deref(), Some("no microphone"));
}

// ---- voice-echo3: the echo again, the sounds, the end-of-turn kiss ----

/// A rig whose voice may cut in on speakers (echo cancelled, asked for).
fn rig_cut_in_by_voice() -> Rig {
    let f = Fakes::new();
    let mut p = f.ports(Route::Speakers);
    p.cut_in_by_voice = true;
    let t0 = Instant::now();
    let vm = VoiceMode::start("main", p, f.jobs(true), VoiceModeConfig::default(), true, t0).unwrap();
    Rig { f, vm, t0, at: 0 }
}

/// The sounds pushed to the speaker, by length.
fn sounds_played(r: &Rig) -> Vec<usize> {
    r.f.speaker.lock().unwrap().pushed.iter().filter(|(u, _)| *u >= SOUND_UTTS).map(|(_, n)| *n).collect()
}

#[test]
fn its_echo_is_dropped_however_late_its_words_come_back() {
    // the batch listener answers seconds after the turn ends: the echo
    // check follows when the speech was heard, not when its words came
    let mut r = Rig::new(Route::Speakers);
    r.turn(" go");
    r.message("Running the tests now, it takes a minute.");
    r.tick();
    r.synth_all(0, 24_000);
    r.done(1);
    r.tick();
    r.quiet(600);
    r.talk(800);
    r.hear(" running the tests now it takes a minute");
    r.quiet(1600);
    r.at += 3_000;
    r.flushed();
    assert!(sends(&r.tick()).is_empty(), "its echo is dropped");
}

#[test]
fn a_cut_by_its_own_echo_never_becomes_a_turn() {
    // the loop on 66eb802: the leaked echo cut the agent off (no words
    // needed), the cut never ended its voice for the echo check, and its
    // own words went back to it as yours
    let mut r = rig_cut_in_by_voice();
    r.turn(" go");
    r.vm.on_turn("main", true);
    r.message("Running the tests now. It takes a minute.");
    r.tick();
    r.synth_all(0, 48_000);
    r.clock(1, 300);
    r.tick();
    r.talk(900);
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1, "cut by voice");
    r.hear(" running the tests now it takes");
    r.quiet(1600);
    r.at += 3_000;
    r.flushed();
    assert!(sends(&r.tick()).is_empty(), "its echo is never a turn");
    // your own words after it still go
    r.at += 4_000;
    assert_eq!(sends(&r.turn(" use the other branch")), ["use the other branch"]);
}

#[test]
fn what_it_said_before_your_last_turn_is_not_its_echo_any_more() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    r.message("Running the tests now.");
    r.tick();
    r.synth_all(0, 2_400);
    r.done(1);
    r.tick();
    r.at += 4_000;
    assert_eq!(sends(&r.turn(" ok thanks")), ["ok thanks"]);
    r.message("Sure.");
    r.tick();
    r.synth_all(1, 2_400);
    r.done(2);
    r.tick();
    // right after "Sure.", words of its message before your turn: yours
    assert_eq!(sends(&r.turn(" are the tests running now")), ["are the tests running now"]);
}

#[test]
fn gone_when_your_turn_is_taken_back_when_it_finished_and_never_after_a_cut() {
    let gone = sounds::pcm(Sound::Gone).len();
    let back = sounds::pcm(Sound::Back).len();
    let mut r = Rig::new(Route::Headphones);
    r.turn(" go");
    assert_eq!(sounds_played(&r), [gone], "your turn is taken");
    assert!(r.f.synth.lock().unwrap().is_empty(), "no spoken 'on it' any more");
    r.message("Done.");
    r.tick();
    assert_eq!(sounds_played(&r), [gone], "nothing before its voice");
    r.synth_all(0, 2_400);
    r.done(1);
    r.tick();
    assert_eq!(sounds_played(&r), [gone, back], "it finished: the mic is yours");
    assert_eq!(r.vm.view(r.now()).kiss_ms, Some(0));
    r.at += 500;
    assert_eq!(r.vm.view(r.now()).kiss_ms, Some(500));
    // you talk: the kiss is over
    r.talk(200);
    assert_eq!(r.vm.view(r.now()).kiss_ms, None);
    r.quiet(1600);
    r.flushed();
    r.tick();
    assert_eq!(sounds_played(&r), [gone, back, gone]);
    // cut with space: no back, no kiss
    r.message("Running the tests now.");
    r.tick();
    r.synth_all(1, 48_000);
    r.tick();
    r.vm.space(SpaceKey::Press, r.now());
    r.vm.space(SpaceKey::Release, r.now() + MS(100));
    r.tick();
    assert_eq!(r.f.speaker.lock().unwrap().stops, 1);
    assert_eq!(sounds_played(&r), [gone, back, gone]);
    assert_eq!(r.vm.view(r.now()).kiss_ms, None);
}

#[test]
fn with_the_sounds_row_off_nothing_plays_but_the_voice() {
    let cfg = VoiceModeConfig { sounds: false, ..VoiceModeConfig::default() };
    let mut r = Rig::with(Route::Headphones, cfg, true, true);
    r.turn(" go");
    r.message("Done.");
    r.tick();
    r.synth_all(0, 2_400);
    r.done(1);
    r.tick();
    assert!(sounds_played(&r).is_empty());
    assert_eq!(r.vm.view(r.now()).kiss_ms, Some(0), "the kiss still ends the turn");
}

#[test]
fn on_speakers_the_mic_waits_for_its_own_sound() {
    let mut r = Rig::new(Route::Speakers);
    r.turn(" go");
    r.sent();
    // `gone` started at the flush (2.0 s), plays 260 ms, its tail 150 ms
    r.talk(200);
    assert!(!r.sent().contains(&"audio".to_string()), "the sound never reaches the listener");
    r.talk(400);
    assert!(r.sent().contains(&"audio".to_string()));
}

#[test]
fn a_block_heard_in_the_voice_tail_is_dropped_even_when_pumped_late() {
    let mut r = Rig::with(Route::Speakers, VoiceModeConfig { sounds: false, ..VoiceModeConfig::default() }, true, true);
    r.turn(" go");
    r.message("Done.");
    r.tick();
    r.synth_all(0, 2_400);
    r.done(1);
    r.tick();
    r.sent();
    // captured 100 ms after its voice ended, pumped 700 ms later
    let tx = r.f.mic.lock().unwrap().blocks.clone().unwrap();
    tx.send(MicBlock { pcm: vec![5000; 1600], at: r.now() + MS(100) }).unwrap();
    r.at += 800;
    r.tick();
    assert!(!r.sent().contains(&"audio".to_string()));
}

#[test]
fn the_last_question_shows_from_its_transcript_until_you_talk_again() {
    let mut r = Rig::new(Route::Headphones);
    assert_eq!(r.vm.view(r.now()).question, None, "nothing asked yet");
    r.talk(500);
    r.hear(" check the build");
    r.tick();
    assert_eq!(r.vm.view(r.now()).question, None, "your live words, not a question");
    r.quiet(1600);
    assert_eq!(r.vm.view(r.now()).question, None, "about to answer: still yours");
    r.flushed();
    assert_eq!(sends(&r.tick()), ["check the build"]);
    assert_eq!(r.vm.view(r.now()).question.as_deref(), Some("check the build"), "as soon as the transcript is in");
    r.vm.on_turn("main", true);
    assert_eq!(r.phase(), Phase::Working);
    assert_eq!(r.vm.view(r.now()).question.as_deref(), Some("check the build"), "while it works");
    r.message("It builds.");
    r.tick();
    r.synth_all(0, 48_000);
    r.clock(1, 300);
    r.tick();
    let v = r.vm.view(r.now());
    assert_eq!((v.phase.clone(), v.question.as_deref()), (Phase::Speaking, Some("check the build")), "while it speaks");
    // you cut in: the pane is yours again
    r.talk(200);
    r.hear(" wait");
    r.talk(300);
    assert_eq!(r.vm.view(r.now()).question, None, "a cut-in is talking again");
    r.quiet(1600);
    r.flushed();
    r.tick();
    assert_eq!(r.vm.view(r.now()).question.as_deref(), Some("wait"), "the next turn replaces it");
}

#[test]
fn noise_brings_the_last_question_back() {
    let mut r = Rig::new(Route::Headphones);
    r.turn(" check the build");
    r.talk(300);
    assert_eq!(r.vm.view(r.now()).question, None);
    r.quiet(1600);
    r.flushed();
    r.tick();
    assert_eq!(r.vm.view(r.now()).question.as_deref(), Some("check the build"));
}
