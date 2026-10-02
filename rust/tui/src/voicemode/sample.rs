//! ▸ hear it on /voice: one sample at a time. A new sample cuts the one
//! playing (its speaker fades out, [`super::Speaker::stop`]) and drops its
//! TTS request still pending ([`Synthesizer`]'s cancel); leaving the
//! screen cuts it too ([`Player`] drops). Each sample says its words on
//! its own thread with its own speaker, opened there (a device may not
//! move between threads).

use super::{SayJob, Speaker, Synth, Synthesizer, UttId, STOP_WITHIN};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Opens the speaker of one sample, on its thread.
pub type OpenSpeaker = Arc<dyn Fn() -> Result<Box<dyn Speaker>, String> + Send + Sync>;

/// The one utterance of a sample's speaker.
const UTT: UttId = 1;
/// How often a sample's thread looks at its cancel while it waits.
const TICK: Duration = Duration::from_millis(20);
/// A sample stops by itself after this, played or not.
const LONGEST: Duration = Duration::from_secs(30);

/// The single slot of ▸ hear it.
pub struct Player {
    synth: Arc<dyn Synthesizer + Send + Sync>,
    open: OpenSpeaker,
    /// the cancel of the sample playing (or done: setting it is harmless)
    playing: Option<Arc<AtomicBool>>,
}

impl Player {
    pub fn new(synth: Arc<dyn Synthesizer + Send + Sync>, open: OpenSpeaker) -> Player {
        Player { synth, open, playing: None }
    }

    /// Voxtral TTS on the default output device.
    pub fn live() -> Player {
        Player::new(Arc::new(super::tts::VoxtralTts), Arc::new(super::audio::open_speaker))
    }

    /// Says `text` with `job`, after cutting the sample playing.
    pub fn play(&mut self, job: SayJob, text: String) {
        self.stop();
        let cancel = Arc::new(AtomicBool::new(false));
        self.playing = Some(cancel.clone());
        let (synth, open) = (self.synth.clone(), self.open.clone());
        std::thread::spawn(move || say(&*synth, &open, job, text, &cancel));
    }

    /// Cuts the sample playing, if any (and its pending TTS request).
    pub fn stop(&mut self) {
        if let Some(cancel) = self.playing.take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One sample, on its thread: the TTS streams into the speaker until it
/// ends or `cancel`; a cut fades out before the speaker closes.
fn say(synth: &dyn Synthesizer, open: &OpenSpeaker, job: SayJob, text: String, cancel: &Arc<AtomicBool>) {
    let Ok(mut speaker) = open() else { return };
    let (tx, rx) = channel();
    synth.start(job, text, tx, cancel.clone());
    let cut = |speaker: &mut Box<dyn Speaker>| {
        speaker.stop();
        // the fade plays before the device closes (no click)
        std::thread::sleep(STOP_WITHIN);
    };
    let until = Instant::now() + LONGEST;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return cut(&mut speaker);
        }
        match rx.recv_timeout(TICK) {
            Ok(Synth::Audio(pcm)) => speaker.push(UTT, &pcm),
            Ok(Synth::Done | Synth::Failed(_)) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) if Instant::now() < until => {}
            Err(RecvTimeoutError::Timeout) => return cut(&mut speaker),
        }
    }
    speaker.end(UTT);
    while !speaker.done(UTT) && Instant::now() < until {
        if cancel.load(Ordering::SeqCst) {
            return cut(&mut speaker);
        }
        std::thread::sleep(TICK);
    }
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
