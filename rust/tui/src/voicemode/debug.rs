//! Voice mode's debug log (voice-echo3): `BISE_VOICE_DEBUG=1` appends
//! what voice mode decided to `~/.bise/logs/voice-debug.log` (or
//! `BISE_VOICE_DEBUG=<a path>`): the route, the echo cancelling on or off
//! and why, the mic opening and closing around the agent's voice, how
//! loud the mic was while the agent talked (the echo that leaks through),
//! the turns sent or dropped as echo, the cuts, the sounds. One line per
//! event, `HH:MM:SS.mmm event · details`. Off (the default): nothing is
//! written, nothing is formatted. Never words of the user's beyond the
//! turn's transcript, which the thread shows anyway.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Where the log goes for `BISE_VOICE_DEBUG`'s value (`home`: bise's
/// home, `~/.bise`); None: off.
pub fn path_for(env: Option<&str>, home: Option<PathBuf>) -> Option<PathBuf> {
    let v = env?.trim();
    match v.to_ascii_lowercase().as_str() {
        "" | "0" | "off" | "false" | "no" => None,
        "1" | "on" | "true" | "yes" => Some(home?.join("logs").join("voice-debug.log")),
        _ => Some(PathBuf::from(v)),
    }
}

fn bise_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("BISE_HOME").filter(|h| !h.is_empty()) {
        return Some(PathBuf::from(h));
    }
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".bise"))
}

fn path() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| path_for(std::env::var("BISE_VOICE_DEBUG").ok().as_deref(), bise_home())).as_ref()
}

/// The log is on (format only then).
pub fn on() -> bool {
    path().is_some()
}

/// The log's path when on (shown once in the thread).
pub fn file() -> Option<String> {
    path().map(|p| p.display().to_string())
}

/// `HH:MM:SS.mmm` (UTC) of the wall clock.
fn stamp() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}.{:03}", (s / 3600) % 24, (s / 60) % 60, s % 60, d.subsec_millis())
}

/// One line; `line` is only built when the log is on.
pub fn log(line: impl FnOnce() -> String) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{} {}", stamp(), line());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_is_off_unless_asked_and_goes_to_the_home_logs() {
        let home = Some(PathBuf::from("/h/.bise"));
        assert_eq!(path_for(None, home.clone()), None);
        assert_eq!(path_for(Some("0"), home.clone()), None);
        assert_eq!(path_for(Some(""), home.clone()), None);
        assert_eq!(path_for(Some("1"), home.clone()), Some(PathBuf::from("/h/.bise/logs/voice-debug.log")));
        assert_eq!(path_for(Some("/tmp/v.log"), home), Some(PathBuf::from("/tmp/v.log")));
        assert_eq!(path_for(Some("1"), None), None);
    }
}
