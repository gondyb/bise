//! What is said aloud of a message (owner: voice-tts; design §4, plan
//! §2). Pure. Stub: filled by voice-tts.

use super::Spoken;

/// The first sentence or two (~12 s), never code, paths, ids, hashes,
/// URLs or tables; numbers rounded and in words; lists as how many and
/// up to 3 items; "the rest is on screen" when there is more.
pub fn speakable(_msg: &str, _language: Option<&str>) -> Spoken {
    Spoken::default()
}
