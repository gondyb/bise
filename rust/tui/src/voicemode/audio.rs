//! The mic and the speaker of voice mode (owner: voice-audio; plan §2).
//! Stub: the contract's entry points, filled by voice-audio.

use super::{Mic, MicBlock, MicStream, Speaker, UttId};
use std::sync::mpsc::Sender;
use std::time::Duration;

/// The default input device, open for the whole voice mode (cpal; reuses
/// voice.rs's resampler and meter).
pub struct CpalMic;

impl Mic for CpalMic {
    fn open(&mut self, _blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
        Err("voice mode's mic is not built yet".into())
    }
}

/// The default output device (cpal, f32, resampled from TTS_RATE).
pub fn open_speaker() -> Result<Box<dyn Speaker>, String> {
    Err("voice mode's speaker is not built yet".into())
}

/// A speaker that plays nothing (no output device; tests).
#[derive(Default)]
pub struct NoSpeaker;

impl Speaker for NoSpeaker {
    fn push(&mut self, _utt: UttId, _pcm: &[f32]) {}
    fn end(&mut self, _utt: UttId) {}
    fn stop(&mut self) {}
    fn clock(&self) -> Option<(UttId, Duration)> {
        None
    }
    fn done(&self, _utt: UttId) -> bool {
        true
    }
    fn level(&self) -> f32 {
        0.0
    }
}
