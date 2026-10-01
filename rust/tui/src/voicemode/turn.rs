//! The turn controller (owner: voice-mode, the lead; plan §4.4): who
//! talks when. Driven by the run loop's ticks, the keys, the mic blocks,
//! the listener's words, the agent's feed events and the speaker's
//! clock; tells the app what to do ([`Act`]) and what the pane shows
//! ([`PaneView`]). Pure over its ports: the tests use the fakes.

use super::config::{ListenMode, ReadAloud, VoiceModeConfig};
use super::{
    AckJob, Acker, Heard, ListenJob, ListenMsg, Listener, Lit, LitState, Mic, MicBlock, MicStream, PaneView, Phase,
    Route, SayJob, Sentence, Speaker, Spoken, Synth, Synthesizer, UttId, Who, Word, WordState, ACK_DEADLINE,
    BACKCHANNELS, BARGE_IN, BARGE_IN_ANYWAY, END_OF_TURN, MIC_RATE, PAUSE, TTS_RATE, WAVE_LEN, WAVE_STEP,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the controller asks of the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// your turn, said: send `text` to `agent` (the `sb` input, `voice:
    /// true`); the app first tries it as an answer to an open inbox item
    Send { agent: String, text: String },
    /// you cut in while `agent`'s turn still runs
    Interrupt { agent: String },
    /// a faint line in the thread (`· voice mode ended · 7 min · …`)
    Note(String),
}

/// A space key as the terminal reports it. Without the kitty protocol's
/// event types every report is a `Press` (repeats included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKey {
    Press,
    Repeat,
    Release,
}

/// Shorter than this, a press then release is a tap (send now).
const TAP: Duration = Duration::from_millis(250);
/// Without release events: no other press within this after the first
/// one, it was a tap (longer than the OS's usual repeat delay).
const TAP_NO_RELEASE: Duration = Duration::from_millis(450);
/// Without release events: repeats stopped this long ago, the hold ended.
const REPEAT_GAP: Duration = Duration::from_millis(160);
/// The listener's Flushed waits at most this long; then what was heard goes.
const FLUSH_WAIT: Duration = Duration::from_secs(3);
/// `you cut in` stays this long.
const CUT_SHOWN: Duration = Duration::from_millis(800);
/// `heard "…" → 1 smaller` stays this long (design §5).
const HEARD_SHOWN: Duration = Duration::from_millis(1500);
/// A failure line stays this long, then voice mode listens again.
const FAIL_SHOWN: Duration = Duration::from_secs(4);

/// Is a block speech: the VAD ([`super::vad::Vad`]), a fake in tests.
pub trait Voicing {
    fn feed(&mut self, pcm: &[i16]) -> bool;
}

impl Voicing for super::vad::Vad {
    fn feed(&mut self, pcm: &[i16]) -> bool {
        super::vad::Vad::feed(self, pcm)
    }
}

/// The ports, real or fake.
pub struct Ports {
    pub mic: Box<dyn Mic>,
    pub vad: Box<dyn Voicing>,
    pub speaker: Box<dyn Speaker>,
    pub listener: Box<dyn Listener>,
    pub synth: Box<dyn Synthesizer>,
    pub acker: Box<dyn Acker>,
    pub route: Route,
    /// the speaker's echo is cancelled in the mic (VoiceProcessingIO):
    /// barge-in works on speakers too
    pub aec: bool,
}

/// Round 2: speech with no words yet (the batch listener has none until
/// the turn ends) cuts the agent off after this long; "mm" is shorter.
const BARGE_IN_NO_WORDS: Duration = Duration::from_millis(800);
/// Without echo cancelling, the mic stays shut this long after the
/// agent's voice ends (the speaker's buffer and the room's tail).
const ECHO_TAIL: Duration = Duration::from_millis(500);
/// What is heard this long after the agent spoke is checked for its echo.
const ECHO_WINDOW: Duration = Duration::from_secs(3);

/// Lowercase letters and digits of each word (the echo check).
fn norm_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// `heard` is the agent's own voice coming back through the mic: most of
/// its words are words the agent just said (round 2: on speakers it
/// answered itself).
pub fn is_echo(heard: &str, said: &str) -> bool {
    let h = norm_words(heard);
    if h.is_empty() {
        return false;
    }
    let s: std::collections::HashSet<String> = norm_words(said).into_iter().collect();
    if s.is_empty() {
        return false;
    }
    let hits = h.iter().filter(|w| s.contains(*w)).count();
    hits * 10 >= h.len() * 6
}

/// The calls voice mode makes; a missing voice or small-jobs model
/// leaves voice mode working, silent (its reason shown once).
pub struct Jobs {
    pub listen: ListenJob,
    pub say: Result<SayJob, String>,
    pub ack: Option<AckJob>,
}

/// The agent's message being said (or the "on it" line: `src` None).
struct Speech {
    /// (agent, message text) for the thread's lighting; None: the ack
    src: Option<(String, String)>,
    spoken: Spoken,
    /// sentence i plays as utt `first + i`
    first: UttId,
    /// the next sentence to synthesize
    next: usize,
    synth: Option<(usize, Receiver<Synth>, Arc<AtomicBool>)>,
    /// per sentence: samples received, all in, words said
    samples: Vec<usize>,
    complete: Vec<bool>,
    said: Vec<usize>,
    cut: bool,
    over: bool,
}

impl Speech {
    fn new(src: Option<(String, String)>, spoken: Spoken, first: UttId) -> Speech {
        let n = spoken.sentences.len();
        Speech {
            src,
            spoken,
            first,
            next: 0,
            synth: None,
            samples: vec![0; n],
            complete: vec![false; n],
            said: vec![0; n],
            cut: false,
            over: n == 0,
        }
    }

    fn last_utt(&self) -> UttId {
        self.first + self.spoken.sentences.len().saturating_sub(1) as UttId
    }

    fn all_said(&mut self) {
        for (i, s) in self.spoken.sentences.iter().enumerate() {
            self.said[i] = s.words.len();
        }
    }

    fn stop_synth(&mut self) {
        if let Some((_, _, cancel)) = self.synth.take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Every word with its state, in order.
    fn word_states(&self) -> Vec<(&Word, &Sentence, WordState)> {
        let mut out = Vec::new();
        for (i, s) in self.spoken.sentences.iter().enumerate() {
            for (j, w) in s.words.iter().enumerate() {
                let st = if j < self.said[i] {
                    WordState::Said
                } else if self.cut {
                    WordState::Cut
                } else {
                    WordState::ToSay
                };
                out.push((w, s, st));
            }
        }
        out
    }
}

/// A line said as is (the "on it"): every word added, none from the thread.
pub fn plain_spoken(text: &str) -> Spoken {
    let mut words = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                words.push(Word { say: s..i, src: None });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        words.push(Word { say: s..text.len(), src: None });
    }
    if words.is_empty() {
        return Spoken::default();
    }
    Spoken { sentences: vec![Sentence { say: text.to_string(), words }], more: false }
}

/// The next sentence of `sp` goes to the synthesizer (one at a time).
fn start_next(synth: &dyn Synthesizer, sp: &mut Speech, job: &SayJob) {
    if sp.synth.is_some() || sp.next >= sp.spoken.sentences.len() {
        return;
    }
    let i = sp.next;
    sp.next += 1;
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    synth.start(job.clone(), sp.spoken.sentences[i].say.clone(), tx, cancel.clone());
    sp.synth = Some((i, rx, cancel));
}

fn is_backchannel(word: &str) -> bool {
    let w: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-').collect::<String>().to_lowercase();
    w.is_empty() || BACKCHANNELS.contains(&w.as_str())
}

/// Only backchannels ("mm", "ok", "right") in `text` (or nothing).
pub fn only_backchannels(text: &str) -> bool {
    text.split_whitespace().all(is_backchannel)
}

struct Ack {
    rx: Receiver<String>,
    cancel: Arc<AtomicBool>,
    deadline: Instant,
    /// the language you spoke (the canned line follows it)
    lang: Option<&'static str>,
}

pub struct VoiceMode {
    agent: String,
    started: Instant,
    cfg: VoiceModeConfig,
    route: Route,
    releases: bool,

    // audio in
    _mic_stream: Box<dyn MicStream>,
    mic_rx: Receiver<MicBlock>,
    vad: Box<dyn Voicing>,
    listener: Box<dyn Listener>,
    listen_job: ListenJob,
    listen_tx: Option<Sender<ListenMsg>>,
    heard_rx: Option<Receiver<Heard>>,
    listen_cancel: Arc<AtomicBool>,

    // your turn
    heard: String,
    /// the last voiced audio's end, the start of this run of speech
    last_voice: Option<Instant>,
    speech_start: Option<Instant>,
    /// this turn had speech (else the fill never starts)
    spoke: bool,
    flushing: Option<Instant>,
    holding: bool,
    hold_start: Option<Instant>,
    last_press: Option<Instant>,
    presses: u32,
    muted: bool,
    typing: bool,

    // the agent
    speaker: Box<dyn Speaker>,
    synth: Box<dyn Synthesizer>,
    say_job: Option<SayJob>,
    acker: Box<dyn Acker>,
    ack_job: Option<AckJob>,
    ack: Option<Ack>,
    acks: u64,
    /// sent, the agent's messages until the next turn are said
    awaiting: bool,
    turn_running: bool,
    queue: VecDeque<Speech>,
    speech: Option<Speech>,
    /// the last message said, kept lit (all said, or cut) until the next turn
    last: Option<Speech>,
    next_utt: UttId,

    // shown
    cut_at: Option<Instant>,
    failed: Option<(String, Instant)>,
    heard_answer: Option<(String, Instant)>,
    /// whose words the captions show
    captions_you: bool,
    turns: u32,
    answers: u32,
    acts: Vec<Act>,
    /// the lanes' waves (one level per WAVE_STEP) and the last step
    you_wave: VecDeque<f32>,
    agent_wave: VecDeque<f32>,
    wave_at: Instant,
    /// the agent's work this turn, minified for the pane's right side
    work: Vec<super::Work>,
    /// the speaker's echo is cancelled in the mic
    aec: bool,
    /// what the agent said lately (the echo check) and when its voice ended
    echo_said: String,
    speech_end: Option<Instant>,
}

impl VoiceMode {
    /// Opens the mic and the listener for `agent` (the agent in view).
    /// `releases`: the terminal reports key releases (the kitty protocol's
    /// event types). Err: the one-line reason; voice mode stays closed.
    pub fn start(
        agent: &str,
        mut ports: Ports,
        jobs: Jobs,
        cfg: VoiceModeConfig,
        releases: bool,
        now: Instant,
    ) -> Result<VoiceMode, String> {
        let (tx, mic_rx) = mpsc::channel();
        let mic_stream = ports.mic.open(tx)?;
        let (say_job, failed) = match jobs.say {
            Ok(j) => (Some(j), None),
            Err(e) => (None, Some((e, now))),
        };
        let mut vm = VoiceMode {
            agent: agent.to_string(),
            started: now,
            cfg,
            route: ports.route,
            aec: ports.aec,
            echo_said: String::new(),
            speech_end: None,
            releases,
            _mic_stream: mic_stream,
            mic_rx,
            vad: ports.vad,
            listener: ports.listener,
            listen_job: jobs.listen,
            listen_tx: None,
            heard_rx: None,
            listen_cancel: Arc::new(AtomicBool::new(false)),
            heard: String::new(),
            last_voice: None,
            speech_start: None,
            spoke: false,
            flushing: None,
            holding: false,
            hold_start: None,
            last_press: None,
            presses: 0,
            muted: false,
            typing: false,
            speaker: ports.speaker,
            synth: ports.synth,
            say_job,
            acker: ports.acker,
            ack_job: jobs.ack,
            ack: None,
            acks: 0,
            awaiting: false,
            turn_running: false,
            queue: VecDeque::new(),
            speech: None,
            last: None,
            next_utt: 1,
            cut_at: None,
            failed,
            heard_answer: None,
            captions_you: true,
            turns: 0,
            answers: 0,
            acts: Vec::new(),
            you_wave: VecDeque::new(),
            agent_wave: VecDeque::new(),
            wave_at: now,
            work: Vec::new(),
        };
        vm.start_listener();
        Ok(vm)
    }

    fn start_listener(&mut self) {
        self.listen_cancel.store(true, Ordering::SeqCst);
        let (atx, arx) = mpsc::channel();
        let (htx, hrx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.listener.start(self.listen_job.clone(), arx, htx, cancel.clone());
        self.listen_tx = Some(atx);
        self.heard_rx = Some(hrx);
        self.listen_cancel = cancel;
    }

    fn listen(&mut self, msg: ListenMsg) {
        if self.listen_tx.is_none() {
            self.start_listener();
        }
        let gone = self.listen_tx.as_ref().is_some_and(|tx| tx.send(msg).is_err());
        if gone {
            self.listen_tx = None;
        }
    }

    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// The agent in view changed: voice mode talks with it now; what the
    /// other one was saying stops.
    pub fn set_agent(&mut self, agent: &str) {
        if agent != self.agent {
            self.stop_talking(false);
            self.queue.clear();
            self.last = None;
            self.awaiting = false;
            self.turn_running = false;
            self.agent = agent.to_string();
        }
    }

    // ---- inputs ----

    pub fn space(&mut self, k: SpaceKey, now: Instant) {
        if self.typing {
            return;
        }
        if self.releases {
            match k {
                SpaceKey::Press if !self.holding => {
                    self.holding = true;
                    self.hold_start = Some(now);
                    self.take_floor();
                }
                SpaceKey::Press | SpaceKey::Repeat => {}
                SpaceKey::Release => {
                    let tap = self.hold_start.is_some_and(|t| now.duration_since(t) < TAP);
                    self.holding = false;
                    self.hold_start = None;
                    if tap || self.cfg.listen == ListenMode::Hold {
                        self.send_now(now);
                    }
                }
            }
            return;
        }
        // no releases: a run of presses close together is a hold
        if k == SpaceKey::Release {
            return;
        }
        let close = self.last_press.is_some_and(|t| now.duration_since(t) < TAP_NO_RELEASE);
        if self.holding && close {
            self.presses += 1;
        } else {
            self.holding = true;
            self.hold_start = Some(now);
            self.presses = 1;
            self.take_floor();
        }
        self.last_press = Some(now);
    }

    /// Space went down: the floor is yours. Over the agent's voice, that
    /// is a cut (hold-to-talk on speakers is deliberate).
    fn take_floor(&mut self) {
        if self.speaking() {
            self.cut_in(Instant::now(), true);
        }
    }

    /// m: the mic off or on again.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if self.muted {
            self.reset_turn();
            self.listen(ListenMsg::Clear);
        }
    }

    /// tab: you type (the mic waits) or back to talking.
    pub fn set_typing(&mut self, on: bool) {
        self.typing = on;
        if on {
            self.holding = false;
            self.reset_turn();
            self.listen(ListenMsg::Clear);
        }
    }

    pub fn typing(&self) -> bool {
        self.typing
    }

    /// You typed and sent a message in voice mode: its answer is said.
    pub fn typed_sent(&mut self) {
        self.after_send();
        self.typing = false;
    }

    /// An inbox item was answered by voice: `heard "…" → 1 smaller`.
    pub fn show_heard(&mut self, line: String, now: Instant) {
        self.heard_answer = Some((line, now));
    }

    /// The agent in view's turn started or ended.
    pub fn on_turn(&mut self, agent: &str, running: bool) {
        if agent == self.agent {
            self.turn_running = running;
        }
    }

    /// A message of `agent` (whole, as the thread shows it).
    pub fn on_message(&mut self, agent: &str, text: &str, spoken: Spoken) {
        if agent != self.agent {
            return;
        }
        let say = match self.cfg.read_aloud {
            ReadAloud::Nothing => false,
            ReadAloud::All => true,
            ReadAloud::Needs => self.awaiting,
        };
        if !say || spoken.sentences.is_empty() {
            return;
        }
        // the agent answered: no "on it" any more
        if let Some(a) = self.ack.take() {
            a.cancel.store(true, Ordering::SeqCst);
        }
        let first = self.next_utt;
        self.next_utt += spoken.sentences.len() as UttId;
        self.answers += 1;
        self.queue.push_back(Speech::new(Some((agent.to_string(), text.to_string())), spoken, first));
    }

    // ---- the tick ----

    pub fn tick(&mut self, now: Instant) -> Vec<Act> {
        self.pump_mic(now);
        self.pump_heard(now);
        self.pump_ack(now);
        self.pump_speech(now);
        self.turn_taking(now);
        self.wave(now);
        if self.failed.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > FAIL_SHOWN) {
            self.failed = None;
        }
        if self.heard_answer.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > HEARD_SHOWN) {
            self.heard_answer = None;
        }
        std::mem::take(&mut self.acts)
    }

    /// One level per WAVE_STEP for each lane (a late tick fills the gap
    /// with the level now).
    fn wave(&mut self, now: Instant) {
        let you = if self.mic_live_at(now) { self._mic_stream.level() } else { 0.0 };
        let agent = self.speaker.level();
        while now.duration_since(self.wave_at) >= WAVE_STEP {
            self.wave_at += WAVE_STEP;
            for (w, l) in [(&mut self.you_wave, you), (&mut self.agent_wave, agent)] {
                w.push_back(l);
                while w.len() > WAVE_LEN {
                    w.pop_front();
                }
            }
        }
    }

    /// The agent talks (or has words queued to say).
    fn speaking(&self) -> bool {
        self.speech.as_ref().is_some_and(|s| !s.over) || !self.queue.is_empty()
    }

    /// Your voice may cut the agent off: the echo is cancelled, or the
    /// voice is in headphones. Never on bare speakers, hands-free or not
    /// (round 2: it heard itself and answered itself).
    fn barge_ok(&self) -> bool {
        self.cfg.listen != ListenMode::Hold && (self.aec || self.route == Route::Headphones)
    }

    /// The mic's words go to the listener now.
    fn mic_live(&self) -> bool {
        self.mic_live_at(Instant::now())
    }

    fn mic_live_at(&self, now: Instant) -> bool {
        if self.muted || self.typing {
            return false;
        }
        if self.cfg.listen == ListenMode::Hold {
            return self.holding;
        }
        if self.speaking() && !self.barge_ok() {
            return self.holding;
        }
        // the voice just ended: its tail is still in the room
        if !self.barge_ok() && !self.holding && self.speech_end.is_some_and(|t| now.saturating_duration_since(t) < ECHO_TAIL) {
            return false;
        }
        true
    }

    /// `heard` is the agent's own voice (while it talks or just after).
    fn echo(&self, heard: &str, now: Instant) -> bool {
        let recent = self.speaking() || self.speech_end.is_some_and(|t| now.saturating_duration_since(t) < ECHO_WINDOW);
        recent && is_echo(heard, &self.echo_said)
    }

    /// The agent's work this turn (live.rs builds it from the feed).
    pub fn set_work(&mut self, work: Vec<super::Work>) {
        self.work = work;
    }

    fn pump_mic(&mut self, now: Instant) {
        while let Ok(b) = self.mic_rx.try_recv() {
            if !self.mic_live_at(now) || self.flushing.is_some() {
                continue;
            }
            let voiced = self.vad.feed(&b.pcm);
            let end = b.at + Duration::from_secs_f64(b.pcm.len() as f64 / MIC_RATE as f64);
            if voiced {
                if self.speech_start.is_none() || self.last_voice.is_some_and(|t| b.at.duration_since(t) > PAUSE) {
                    self.speech_start = Some(b.at);
                }
                self.last_voice = Some(end);
                self.spoke = true;
                self.captions_you = true;
            }
            self.listen(ListenMsg::Audio(b.pcm));
        }
    }

    fn pump_heard(&mut self, now: Instant) {
        let Some(rx) = &self.heard_rx else { return };
        let mut got = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(h) => got.push(h),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.heard_rx = None;
                    self.listen_tx = None;
                    break;
                }
            }
        }
        for h in got {
            match h {
                Heard::Text(t) => {
                    self.heard.push_str(&t);
                    self.captions_you = true;
                }
                Heard::Flushed => {
                    if self.flushing.is_some() {
                        self.finish_turn(now);
                    }
                }
                Heard::Failed(line) => {
                    self.failed = Some((line, now));
                    self.listen_tx = None;
                    self.heard_rx = None;
                    if self.flushing.is_some() {
                        self.finish_turn(now);
                    }
                }
            }
        }
    }

    fn pump_ack(&mut self, now: Instant) {
        let Some(a) = &self.ack else { return };
        let line = match a.rx.try_recv() {
            Ok(l) if !l.trim().is_empty() => Some(l),
            _ if now >= a.deadline => Some(super::ack::canned_in(self.acks, a.lang).to_string()),
            _ => None,
        };
        if let Some(line) = line {
            if let Some(a) = self.ack.take() {
                a.cancel.store(true, Ordering::SeqCst);
            }
            self.acks += 1;
            if self.speaking() {
                return;
            }
            let spoken = plain_spoken(line.trim());
            let first = self.next_utt;
            self.next_utt += spoken.sentences.len() as UttId;
            self.queue.push_front(Speech::new(None, spoken, first));
        }
    }

    /// The speech: the next one starts, its synth streams to the speaker
    /// sentence by sentence, the speaker's clock lights its words.
    fn pump_speech(&mut self, now: Instant) {
        let was_talking = self.speaking();
        self.pump_speech_step();
        // the echo check: what is said, and when the voice stopped
        if let Some(sp) = self.speech.as_ref().filter(|s| !s.over) {
            let said: String = sp.spoken.sentences.iter().map(|s| s.say.as_str()).collect::<Vec<_>>().join(" ");
            if !self.echo_said.ends_with(&said) {
                self.echo_said = said;
            }
        }
        if was_talking && !self.speaking() {
            self.speech_end = Some(now);
        }
    }

    fn pump_speech_step(&mut self) {
        if self.speech.as_ref().is_none_or(|s| s.over) {
            if let Some(next) = self.queue.pop_front() {
                if let Some(old) = self.speech.take() {
                    if old.src.is_some() {
                        self.last = Some(old);
                    }
                }
                self.speech = Some(next);
                self.captions_you = false;
            }
        }
        let speed = self.say_job.as_ref().map_or(1.0, |j| j.speed);
        let Some(sp) = self.speech.as_mut() else { return };
        if sp.over {
            return;
        }
        // no voice: the words show, nothing is said
        let Some(job) = self.say_job.clone() else {
            sp.all_said();
            sp.over = true;
            return;
        };
        start_next(self.synth.as_ref(), sp, &job);
        let mut failed = None;
        if let Some((i, rx, _)) = &sp.synth {
            let (i, utt) = (*i, sp.first + *i as UttId);
            let mut ended = false;
            loop {
                match rx.try_recv() {
                    Ok(Synth::Audio(pcm)) => {
                        sp.samples[i] += pcm.len();
                        self.speaker.push(utt, &pcm);
                    }
                    Ok(Synth::Done) | Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                    Ok(Synth::Failed(e)) => {
                        failed = Some(e);
                        ended = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if ended {
                self.speaker.end(utt);
                sp.complete[i] = true;
                sp.synth = None;
                // the next sentence's voice starts while this one plays
                if failed.is_none() {
                    start_next(self.synth.as_ref(), sp, &job);
                }
            }
        }
        if let Some(e) = failed {
            self.failed = Some((e, Instant::now()));
            sp.stop_synth();
            sp.all_said();
            sp.over = true;
            return;
        }
        // the clock: which sentence plays, how far
        if let Some((utt, played)) = self.speaker.clock() {
            if utt >= sp.first && utt <= sp.last_utt() {
                let i = (utt - sp.first) as usize;
                for k in 0..i {
                    sp.said[k] = sp.spoken.sentences[k].words.len();
                }
                let s = &sp.spoken.sentences[i];
                let got = Duration::from_secs_f64(sp.samples[i] as f64 / TTS_RATE as f64);
                let total = if sp.complete[i] { got } else { got.max(super::timing::estimate(&s.say, speed)) };
                let n = super::timing::said_upto(s, played, total);
                sp.said[i] = sp.said[i].max(n);
            }
        }
        // a sentence is over once all its audio played
        for i in 0..sp.spoken.sentences.len() {
            if sp.complete[i] && self.speaker.done(sp.first + i as UttId) {
                sp.said[i] = sp.spoken.sentences[i].words.len();
            }
        }
        if sp.next == sp.spoken.sentences.len() && sp.synth.is_none() && self.speaker.done(sp.last_utt()) {
            sp.all_said();
            sp.over = true;
        }
    }

    fn turn_taking(&mut self, now: Instant) {
        // without releases a hold ends when the repeats stop; a lone
        // press is a tap
        if !self.releases && self.holding {
            if let Some(last) = self.last_press {
                let gap = now.duration_since(last);
                if self.presses <= 1 && gap >= TAP_NO_RELEASE {
                    self.holding = false;
                    self.send_now(now);
                } else if self.presses > 1 && gap >= REPEAT_GAP {
                    self.holding = false;
                    if self.cfg.listen == ListenMode::Hold {
                        self.send_now(now);
                    }
                }
            }
        }
        if let Some(t) = self.flushing {
            if now.duration_since(t) >= FLUSH_WAIT {
                self.finish_turn(now);
            }
            return;
        }
        // over the agent's voice: a cut, or a backchannel let go
        if self.speaking() {
            if let (Some(start), Some(last)) = (self.speech_start, self.last_voice) {
                let talking = now.saturating_duration_since(last) < PAUSE;
                let long = last.saturating_duration_since(start);
                // the batch listener has no words before the turn ends:
                // the length of the speech decides ("mm" is short)
                let none_yet = self.heard.trim().is_empty();
                let words = !only_backchannels(&self.heard) && !self.echo(&self.heard, now);
                let cut = (long >= BARGE_IN && words) || (none_yet && long >= BARGE_IN_NO_WORDS) || long >= BARGE_IN_ANYWAY;
                if talking && cut && !(!none_yet && self.echo(&self.heard, now)) {
                    self.cut_in(now, false);
                } else if !talking && !self.holding {
                    // "mm", "ok": the agent goes on
                    self.reset_turn();
                    self.listen(ListenMsg::Clear);
                }
            }
            return;
        }
        if self.spoke && !self.holding && self.cfg.listen != ListenMode::Hold {
            if let Some(last) = self.last_voice {
                if now.saturating_duration_since(last) >= PAUSE + END_OF_TURN {
                    self.flush(now);
                }
            }
        }
    }

    /// space, or a release in hold-to-talk: the turn goes now.
    fn send_now(&mut self, now: Instant) {
        if self.spoke || !self.heard.trim().is_empty() {
            self.flush(now);
        }
    }

    fn flush(&mut self, now: Instant) {
        self.flushing = Some(now);
        self.listen(ListenMsg::Flush);
        // no listener to answer (it failed): what was heard goes now
        if self.listen_tx.is_none() {
            self.finish_turn(now);
        }
    }

    /// The listener has every word: send them (nothing heard: noise).
    fn finish_turn(&mut self, now: Instant) {
        self.flushing = None;
        let text = self.heard.trim().to_string();
        self.reset_turn();
        // the agent's own voice through the mic: never a turn
        if text.is_empty() || self.echo(&text, now) {
            return;
        }
        self.turns += 1;
        self.acts.push(Act::Send { agent: self.agent.clone(), text: text.clone() });
        self.after_send();
        if let Some(job) = self.ack_job.clone() {
            let (tx, rx) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let lang = super::speak::language(&text);
            self.acker.start(job, text, tx, cancel.clone());
            self.ack = Some(Ack { rx, cancel, deadline: now + ACK_DEADLINE, lang });
        }
    }

    fn after_send(&mut self) {
        self.awaiting = true;
        self.last = None;
        self.captions_you = true;
    }

    fn reset_turn(&mut self) {
        self.heard.clear();
        self.spoke = false;
        self.speech_start = None;
        self.last_voice = None;
    }

    /// You cut in: the voice stops, the unsaid words are cut, the agent's
    /// running turn is interrupted (not for a space tap: a skip).
    fn cut_in(&mut self, now: Instant, by_key: bool) {
        self.stop_talking(true);
        self.cut_at = Some(now);
        self.captions_you = true;
        if self.turn_running && !by_key {
            self.acts.push(Act::Interrupt { agent: self.agent.clone() });
        }
    }

    fn stop_talking(&mut self, cut: bool) {
        self.speaker.stop();
        if let Some(a) = self.ack.take() {
            a.cancel.store(true, Ordering::SeqCst);
        }
        self.queue.clear();
        if let Some(mut sp) = self.speech.take() {
            sp.stop_synth();
            if !sp.over {
                sp.cut = cut;
                sp.over = true;
            }
            if sp.src.is_some() {
                self.last = Some(sp);
            }
        }
    }

    /// esc: voice mode ends. The speaker stops, the half turn is dropped,
    /// the mic closes (when `self` drops).
    pub fn leave(&mut self, now: Instant) -> Vec<Act> {
        self.stop_talking(true);
        self.listen_cancel.store(true, Ordering::SeqCst);
        self.listen_tx = None;
        let mins = (now.duration_since(self.started).as_secs() + 30) / 60;
        let n = |k: u32, one: &str, many: &str| format!("{} {}", k, if k == 1 { one } else { many });
        vec![Act::Note(format!(
            "voice mode ended · {} min · {}, {}",
            mins,
            n(self.turns, "thing said", "things said"),
            n(self.answers, "answer", "answers")
        ))]
    }

    // ---- what shows ----

    fn phase(&self, now: Instant) -> Phase {
        if let Some((line, _)) = &self.failed {
            return Phase::Failed(line.clone());
        }
        if self.typing {
            return Phase::Typing;
        }
        if self.muted {
            return Phase::Muted;
        }
        if self.cut_at.is_some_and(|t| now.duration_since(t) < CUT_SHOWN) {
            return Phase::CutIn;
        }
        if self.flushing.is_some() {
            return Phase::AboutToAnswer { fill: 1.0 };
        }
        if self.holding {
            return Phase::Holding;
        }
        if self.speaking() {
            return Phase::Speaking;
        }
        if let Some(last) = self.last_voice {
            let quiet = now.saturating_duration_since(last);
            if quiet < PAUSE {
                return Phase::Hearing;
            }
            if self.spoke {
                let fill = (quiet - PAUSE).as_secs_f32() / END_OF_TURN.as_secs_f32();
                return Phase::AboutToAnswer { fill: fill.clamp(0.0, 1.0) };
            }
        }
        if self.awaiting && self.turn_running {
            return Phase::Working;
        }
        if self.cfg.listen == ListenMode::Hold {
            return Phase::HoldToTalk;
        }
        Phase::Listening
    }

    /// The speech whose words show: the one playing, else the last one.
    fn shown_speech(&self) -> Option<&Speech> {
        self.speech.as_ref().filter(|s| !s.over || s.src.is_some()).or(self.last.as_ref())
    }

    pub fn view(&self, now: Instant) -> PaneView {
        let phase = self.phase(now);
        let (who, words) = if self.captions_you || self.shown_speech().is_none() {
            let mut w: Vec<(String, WordState)> =
                self.heard.split_whitespace().map(|x| (x.to_string(), WordState::Heard)).collect();
            if phase == Phase::Hearing {
                if let Some(l) = w.last_mut() {
                    l.1 = WordState::Partial;
                }
            }
            (Who::You, w)
        } else {
            let sp = self.shown_speech().expect("checked");
            let w = sp.word_states().into_iter().map(|(w, s, st)| (s.say[w.say.clone()].to_string(), st)).collect();
            (Who::Agent(self.agent.clone()), w)
        };
        PaneView {
            phase,
            agent: self.agent.clone(),
            who,
            words,
            you_level: if self.mic_live_at(now) { self._mic_stream.level() } else { 0.0 },
            agent_level: self.speaker.level(),
            you_wave: self.you_wave.iter().copied().collect(),
            agent_wave: self.agent_wave.iter().copied().collect(),
            elapsed: now.duration_since(self.started),
            route: self.route,
            heard_answer: self.heard_answer.as_ref().map(|(l, _)| l.clone()),
            work: self.work.clone(),
        }
    }

    /// The agent's message being said (or last said), lit in the thread.
    pub fn lit(&self) -> Option<Lit> {
        let sp = [self.speech.as_ref(), self.last.as_ref()].into_iter().flatten().find(|s| s.src.is_some())?;
        let (agent, text) = sp.src.clone()?;
        let spans = sp
            .word_states()
            .into_iter()
            .filter_map(|(w, _, st)| {
                let st = match st {
                    WordState::Said => LitState::Said,
                    WordState::Cut => LitState::Cut,
                    _ => LitState::ToSay,
                };
                w.src.clone().map(|r| (r, st))
            })
            .collect();
        Some(Lit { agent, text, spans })
    }

    /// The pane moves (the loop wakes every 50 ms while voice mode is on).
    pub fn active(&self) -> bool {
        true
    }
}

impl Drop for VoiceMode {
    fn drop(&mut self) {
        self.listen_cancel.store(true, Ordering::SeqCst);
        if let Some(sp) = self.speech.as_mut() {
            sp.stop_synth();
        }
        if let Some(a) = self.ack.take() {
            a.cancel.store(true, Ordering::SeqCst);
        }
        self.speaker.stop();
    }
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
