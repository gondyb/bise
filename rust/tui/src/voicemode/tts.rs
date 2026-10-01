//! Text to speech (owner: voice-tts; plan §2): Voxtral TTS, streamed
//! (POST {base}/audio/speech, SSE of base64 PCM at TTS_RATE). Stub:
//! filled by voice-tts.

use super::{SayJob, Synth, Synthesizer};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// The default voice until the brand voice is picked by ear.
pub const DEFAULT_VOICE: &str = "";

pub struct VoxtralTts;

impl Synthesizer for VoxtralTts {
    fn start(&self, _job: SayJob, _text: String, events: Sender<Synth>, _cancel: Arc<AtomicBool>) {
        let _ = events.send(Synth::Failed("voice mode's voice is not built yet".into()));
    }
}
