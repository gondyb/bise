//! When each word of a sentence is said (owner: voice-tts; plan §4.3).
//! The TTS gives no word timings: the words share the audio by weight
//! (syllables, a pause after punctuation). Pure. Stub: filled by voice-tts.

use super::Sentence;
use std::time::Duration;

/// How long `say` takes at `speed` before its audio is all in.
pub fn estimate(say: &str, speed: f32) -> Duration {
    Duration::from_millis((say.chars().count() as f32 * 65.0 / speed.max(0.1)) as u64)
}

/// How many words of `s` are said after `played` of `total` audio.
pub fn said_upto(s: &Sentence, played: Duration, total: Duration) -> usize {
    if total.is_zero() {
        return 0;
    }
    let f = (played.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
    (f * s.words.len() as f32).floor() as usize
}
