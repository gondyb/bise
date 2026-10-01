//! Headphones or speakers (owner: voice-audio; plan §2). macOS: the
//! default output device's transport and data source (CoreAudio);
//! elsewhere Unknown. Stub: filled by voice-audio.

use super::Route;

pub fn output_route() -> Route {
    Route::Unknown
}
