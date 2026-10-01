//! The quick spoken "on it" (owner: voice-tts; design §7, plan §2): the
//! small-jobs model answers in ≤ 5 words while the agent starts; past
//! ACK_DEADLINE a canned line. Stub: filled by voice-tts.

use super::{AckJob, Acker};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub struct SmallAck;

impl Acker for SmallAck {
    fn start(&self, _job: AckJob, _heard: String, _events: Sender<String>, _cancel: Arc<AtomicBool>) {}
}

/// A canned "on it", turn `n` (they take turns, never twice in a row).
pub fn canned(n: u64) -> &'static str {
    const LINES: [&str; 3] = ["on it.", "let me look.", "okay, one moment."];
    LINES[(n % LINES.len() as u64) as usize]
}
