//! Speech to text for voice mode (owner: voice-stt; plan §2): Voxtral
//! Realtime over a websocket (the protocol of 0659158's voice.rs), else
//! the voice role's batch model, one request per turn. Stub: filled by
//! voice-stt.

use super::{Heard, ListenJob, ListenMsg, Listener};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// The listener for `job`: realtime when it has an endpoint, else batch.
pub fn listener_for(_job: &ListenJob) -> Box<dyn Listener> {
    Box::new(BatchListener)
}

/// One batch request per turn (voice::transcribe_clip), at Flush.
pub struct BatchListener;

impl Listener for BatchListener {
    fn start(&self, _job: ListenJob, _audio: Receiver<ListenMsg>, events: Sender<Heard>, _cancel: Arc<AtomicBool>) {
        let _ = events.send(Heard::Failed("voice mode's listener is not built yet".into()));
    }
}
