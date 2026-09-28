//! Startup timing marks: `<epoch ms> tui <what>` appended to the file
//! named by `SB_TIMING` (unset: nothing, one env lookup). The hub writes
//! the same format (switchboard::util::timing): one clock for both.

use std::io::Write;

/// Whether marks are written (read once).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SB_TIMING").is_some())
}

/// One mark (nothing when `SB_TIMING` is unset).
pub fn mark(what: &str) {
    if !enabled() {
        return;
    }
    let Some(path) = std::env::var_os("SB_TIMING") else { return };
    let us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{:.1} tui {}", us as f64 / 1000.0, what);
    }
}
