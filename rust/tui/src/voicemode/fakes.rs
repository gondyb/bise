//! Fakes of the voice mode ports for the controller's tests (owner:
//! voice-mode, the lead). Never a real mic, speaker or network: each
//! fake keeps its side of the channels in a shared slot the test drives.

use super::turn::{Jobs, Ports, Voicing};
use super::{
    AckJob, Acker, Endpoint, Heard, ListenJob, ListenMsg, Listener, Mic, MicBlock, MicStream, Route, SayJob, Speaker,
    Synth, Synthesizer, UttId,
};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Shared<T> = Arc<Mutex<T>>;

fn shared<T: Default>() -> Shared<T> {
    Arc::new(Mutex::new(T::default()))
}

// ---- the mic ----

#[derive(Default)]
pub struct MicSlot {
    pub blocks: Option<Sender<MicBlock>>,
    pub level: f32,
    pub opened: u32,
}

pub struct FakeMic(pub Shared<MicSlot>);

struct FakeMicStream(Shared<MicSlot>);

impl MicStream for FakeMicStream {
    fn level(&self) -> f32 {
        self.0.lock().unwrap().level
    }
}

impl Mic for FakeMic {
    fn open(&mut self, blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        let mut s = self.0.lock().unwrap();
        s.blocks = Some(blocks);
        s.opened += 1;
        Ok(Box::new(FakeMicStream(self.0.clone())))
    }
}

/// A mic that cannot open (no device, refused).
pub struct BrokenMic;

impl Mic for BrokenMic {
    fn open(&mut self, _blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        Err("no microphone".into())
    }
}

/// Speech: any sample louder than 1000.
pub struct LoudVad;

impl Voicing for LoudVad {
    fn feed(&mut self, pcm: &[i16]) -> bool {
        pcm.iter().any(|s| s.unsigned_abs() > 1000)
    }
}

// ---- the listener ----

/// One listener session: what the controller sent, how to answer.
pub struct Session {
    pub audio: Receiver<ListenMsg>,
    pub heard: Sender<Heard>,
}

pub struct FakeListener(pub Shared<Vec<Session>>);

impl Listener for FakeListener {
    fn start(&self, _job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, _cancel: Arc<AtomicBool>) {
        self.0.lock().unwrap().push(Session { audio, heard: events });
    }
}

// ---- the voice ----

pub struct SynthCall {
    pub text: String,
    pub events: Sender<Synth>,
    pub cancel: Arc<AtomicBool>,
}

pub struct FakeSynth(pub Shared<Vec<SynthCall>>);

impl Synthesizer for FakeSynth {
    fn start(&self, _job: SayJob, text: String, events: Sender<Synth>, cancel: Arc<AtomicBool>) {
        self.0.lock().unwrap().push(SynthCall { text, events, cancel });
    }
}

#[derive(Default)]
pub struct SpeakerSlot {
    /// samples pushed per utt, in push order
    pub pushed: Vec<(UttId, usize)>,
    pub ended: HashSet<UttId>,
    pub done: HashSet<UttId>,
    pub stops: u32,
    pub clock: Option<(UttId, Duration)>,
    pub level: f32,
}

pub struct FakeSpeaker(pub Shared<SpeakerSlot>);

impl Speaker for FakeSpeaker {
    fn push(&mut self, utt: UttId, pcm: &[f32]) {
        self.0.lock().unwrap().pushed.push((utt, pcm.len()));
    }
    fn end(&mut self, utt: UttId) {
        self.0.lock().unwrap().ended.insert(utt);
    }
    fn stop(&mut self) {
        let mut s = self.0.lock().unwrap();
        s.stops += 1;
        s.clock = None;
    }
    fn clock(&self) -> Option<(UttId, Duration)> {
        self.0.lock().unwrap().clock
    }
    fn done(&self, utt: UttId) -> bool {
        self.0.lock().unwrap().done.contains(&utt)
    }
    fn level(&self) -> f32 {
        self.0.lock().unwrap().level
    }
}

pub struct AckCall {
    pub heard: String,
    pub events: Sender<String>,
}

pub struct FakeAcker(pub Shared<Vec<AckCall>>);

impl Acker for FakeAcker {
    fn start(&self, _job: AckJob, heard: String, events: Sender<String>, _cancel: Arc<AtomicBool>) {
        self.0.lock().unwrap().push(AckCall { heard, events });
    }
}

// ---- a whole set ----

pub fn endpoint(name: &str) -> Endpoint {
    Endpoint {
        name: name.into(),
        provider_name: "Mistral".into(),
        base_url: "http://127.0.0.1:9".into(),
        model: name.into(),
        key: "k".into(),
    }
}

pub fn listen_job() -> ListenJob {
    ListenJob {
        realtime: None,
        batch: crate::voice::VoiceJob {
            name: "mistral/voxtral-mini-latest".into(),
            provider_name: "Mistral".into(),
            billing_url: String::new(),
            api: "openai".into(),
            base_url: "http://127.0.0.1:9".into(),
            model: "voxtral-mini-latest".into(),
            key: "k".into(),
            language: None,
            vocabulary: Vec::new(),
        },
    }
}

/// Every fake and its slot.
pub struct Fakes {
    pub mic: Shared<MicSlot>,
    pub listen: Shared<Vec<Session>>,
    pub synth: Shared<Vec<SynthCall>>,
    pub speaker: Shared<SpeakerSlot>,
    pub acker: Shared<Vec<AckCall>>,
}

impl Fakes {
    pub fn new() -> Fakes {
        Fakes { mic: shared(), listen: shared(), synth: shared(), speaker: shared(), acker: shared() }
    }

    pub fn ports(&self, route: Route) -> Ports {
        Ports {
            mic: Box::new(FakeMic(self.mic.clone())),
            vad: Box::new(LoudVad),
            speaker: Box::new(FakeSpeaker(self.speaker.clone())),
            listener: Box::new(FakeListener(self.listen.clone())),
            synth: Box::new(FakeSynth(self.synth.clone())),
            acker: Box::new(FakeAcker(self.acker.clone())),
            route,
            aec: false,
        }
    }

    pub fn jobs(&self, voice: bool, ack: bool) -> Jobs {
        Jobs {
            listen: listen_job(),
            say: if voice {
                Ok(SayJob { api: endpoint("voxtral-mini-tts-2603"), voice: "v".into(), speed: 1.0 })
            } else {
                Err("voice mode needs a voice: /voice".into())
            },
            ack: ack.then(|| AckJob { api: endpoint("mistral-small-latest"), family: "openai".into() }),
        }
    }
}
