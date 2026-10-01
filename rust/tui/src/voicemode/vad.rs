//! Is someone talking (owner: voice-audio; plan §2). Pure: blocks of
//! MIC_RATE PCM in, speech or not out, with an adaptive noise floor.
//! Stub: filled by voice-audio.

#[derive(Debug, Default)]
pub struct Vad {}

impl Vad {
    pub fn new() -> Vad {
        Vad {}
    }

    /// One block: is it speech (with a short hangover, so a breath
    /// between two words stays speech)?
    pub fn feed(&mut self, _pcm: &[i16]) -> bool {
        false
    }
}
