//! Speech to text for voice mode (owner: voice-models; plan §4.3, §8):
//! the voice role's batch model (Voxtral Transcribe 3 by default), one
//! request per turn. The controller's VAD ends the turn
//! ([`ListenMsg::Flush`]); the turn's audio goes to
//! `voice::transcribe_clip`, its words come back as one [`Heard::Text`],
//! then [`Heard::Flushed`]. [`ListenMsg::Clear`] drops the half turn.
//!
//! Round 2 (the user's notes on 699f710): no realtime listener. The only
//! realtime model was Voxtral Mini's, and it got too many words wrong;
//! Transcribe 3 has no streaming endpoint, so `ListenJob::realtime` is
//! always None now (bise_catalog::voice::realtime_model).
//!
//! Failures: [`Heard::Failed`] with `voice::fail_lines`' wording ends the
//! turn (no `Flushed` for it).

use super::{Heard, ListenJob, ListenMsg, Listener, MIC_RATE};
use crate::voice::{self, http, Failure, VoiceJob};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// One batch request.
const BATCH_TIMEOUT: Duration = Duration::from_secs(60);
/// The batch clip keeps the last this many samples (5 min, ~9.6 MB):
/// the mic sends audio while nobody talks too.
const MAX_CLIP: usize = 5 * 60 * MIC_RATE as usize;

/// The listener for `job`: the batch one (no provider has a realtime
/// model we offer).
pub fn listener_for(_job: &ListenJob) -> Box<dyn Listener> {
    Box::new(BatchListener)
}

/// One batch request per turn (voice::transcribe_clip), at Flush.
pub struct BatchListener;

impl Listener for BatchListener {
    fn start(&self, job: ListenJob, audio: Receiver<ListenMsg>, events: Sender<Heard>, cancel: Arc<AtomicBool>) {
        std::thread::spawn(move || {
            run_batch(&job.batch, &audio, &events, &cancel, &|req| http::send(req, BATCH_TIMEOUT));
        });
    }
}

/// A failure as voice mode says it: `voice::fail_lines`' first line.
fn fail_line(f: &Failure, provider: &str, model: &str, billing: &str) -> String {
    voice::fail_lines(f, provider, model, billing, false).head
}

/// One `voice::transcribe_clip` per Flush over the audio since the last
/// one, until the controller goes or `cancel`. `send`: the HTTP call.
fn run_batch(
    job: &VoiceJob,
    audio: &Receiver<ListenMsg>,
    events: &Sender<Heard>,
    cancel: &AtomicBool,
    send: &dyn Fn(&http::Request) -> Result<http::Response, String>,
) {
    let mut clip: Vec<i16> = Vec::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        match audio.recv_timeout(Duration::from_millis(50)) {
            Ok(ListenMsg::Audio(pcm)) => {
                clip.extend(pcm);
                if clip.len() > MAX_CLIP + MAX_CLIP / 4 {
                    clip.drain(..clip.len() - MAX_CLIP);
                }
            }
            Ok(ListenMsg::Clear) => clip.clear(),
            Ok(ListenMsg::Flush) => {
                let result = voice::transcribe_clip(job, &clip, cancel, send);
                clip.clear();
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let _ = match result {
                    Ok(text) => {
                        if !text.is_empty() {
                            let _ = events.send(Heard::Text(format!(" {}", text)));
                        }
                        events.send(Heard::Flushed)
                    }
                    Err(f) => events.send(Heard::Failed(fail_line(&f, &job.provider_name, &job.model, &job.billing_url))),
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
#[path = "listen_tests.rs"]
mod tests;
