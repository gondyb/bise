use super::*;
use crate::voicemode::fakes::{endpoint, FakeSpeaker, FakeSynth, Shared, SpeakerSlot, SynthCall};
use std::sync::Mutex;

/// The fake TTS's calls and one fake (silent) speaker per sample.
struct Rig {
    calls: Shared<Vec<SynthCall>>,
    speakers: Shared<Vec<Shared<SpeakerSlot>>>,
}

impl Rig {
    fn player() -> (Player, Rig) {
        let calls: Shared<Vec<SynthCall>> = Default::default();
        let speakers: Shared<Vec<Shared<SpeakerSlot>>> = Default::default();
        let s = speakers.clone();
        let open: OpenSpeaker = Arc::new(move || {
            let slot: Shared<SpeakerSlot> = Arc::new(Mutex::new(SpeakerSlot::default()));
            s.lock().unwrap().push(slot.clone());
            Ok(Box::new(FakeSpeaker(slot)) as Box<dyn Speaker>)
        });
        (Player::new(Arc::new(FakeSynth(calls.clone())), open), Rig { calls, speakers })
    }

    fn cancelled(&self, i: usize) -> bool {
        self.calls.lock().unwrap()[i].cancel.load(Ordering::SeqCst)
    }

    fn speaker(&self, i: usize) -> Shared<SpeakerSlot> {
        self.speakers.lock().unwrap()[i].clone()
    }
}

fn job() -> SayJob {
    SayJob { api: endpoint("voxtral-mini-tts-2603"), voice: "v".into(), speed: 1.0 }
}

/// Waits (at most 2 s) for `cond`, the samples' threads being async.
fn eventually(what: &str, cond: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(2);
    while !cond() {
        assert!(Instant::now() < until, "never: {}", what);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn one_sample_at_a_time_a_new_one_cuts_the_one_playing() {
    let (mut player, rig) = Rig::player();
    // the first sample plays
    player.play(job(), "a".into());
    eventually("the first TTS call", || rig.calls.lock().unwrap().len() == 1);
    rig.calls.lock().unwrap()[0].events.send(Synth::Audio(vec![0.0; 240])).unwrap();
    let first = rig.speaker(0);
    eventually("the first sample's audio", || !first.lock().unwrap().pushed.is_empty());
    // a second ⏎: the first is cut (TTS cancelled, its speaker faded out)
    player.play(job(), "b".into());
    eventually("the second TTS call", || rig.calls.lock().unwrap().len() == 2);
    assert!(rig.cancelled(0));
    assert!(!rig.cancelled(1));
    eventually("the first speaker stops", || first.lock().unwrap().stops == 1);
    let second = rig.speaker(1);
    assert_eq!(second.lock().unwrap().stops, 0);
    // a third while the second's TTS is still pending: that request drops
    player.play(job(), "c".into());
    eventually("the third TTS call", || rig.calls.lock().unwrap().len() == 3);
    assert!(rig.cancelled(1));
    eventually("the second speaker stops", || second.lock().unwrap().stops == 1);
    assert!(second.lock().unwrap().pushed.is_empty());
    assert_eq!(rig.calls.lock().unwrap().iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    // leaving the screen (the player drops) cuts the last one too
    drop(player);
    assert!(rig.cancelled(2));
    let third = rig.speaker(2);
    eventually("the third speaker stops", || third.lock().unwrap().stops == 1);
    assert_eq!(first.lock().unwrap().stops, 1, "a cut sample is cut once");
}

#[test]
fn a_sample_that_ends_is_not_cut() {
    let (mut player, rig) = Rig::player();
    player.play(job(), "a".into());
    eventually("the TTS call", || rig.calls.lock().unwrap().len() == 1);
    {
        let calls = rig.calls.lock().unwrap();
        calls[0].events.send(Synth::Audio(vec![0.0; 240])).unwrap();
        calls[0].events.send(Synth::Done).unwrap();
    }
    let sp = rig.speaker(0);
    eventually("its end", || sp.lock().unwrap().ended.contains(&UTT));
    sp.lock().unwrap().done.insert(UTT);
    std::thread::sleep(TICK * 3);
    player.stop();
    std::thread::sleep(TICK * 3);
    assert_eq!(sp.lock().unwrap().stops, 0);
}
