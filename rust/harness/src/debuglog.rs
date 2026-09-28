//! The session debug log, parent side.
//!
//! Every session keeps a debug directory next to its checkpoint
//! (`<sessions>/<id>.debug/`). The Bend REPL and this parent append to
//! the same `events.jsonl`, one JSON object per line: the REPL its
//! breadcrumbs and provider incidents (runtime/debug.bend), the parent
//! the lifecycle of the REPL process - spawns, reloads, crashes, its own
//! panics. A crash also leaves a snapshot directory `crash-<ts>/` with
//! what the respawn would otherwise overwrite or lose: the dead
//! generation's stderr and stdout, the checkpoint it restarts from, and
//! the BEND_* environment. Best effort everywhere: a failed write never
//! stops the harness.

use std::io::Write;
use std::path::{Path, PathBuf};

// events.jsonl beyond this is rotated to events.1.jsonl at startup
const ROTATE_BYTES: u64 = 8 * 1024 * 1024;
// the most of a log file a crash snapshot keeps (its tail)
const SNAPSHOT_TAIL_BYTES: usize = 256 * 1024;

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One events.jsonl line, as the REPL writes them (src = "parent").
pub fn event_line(ts: u128, kind: &str, fields: &[(&str, String)]) -> String {
    let mut line = format!(
        "{{\"ts\":{},\"src\":\"parent\",\"kind\":{}",
        ts,
        json_str(kind)
    );
    for (k, v) in fields {
        line.push(',');
        line.push_str(&json_str(k));
        line.push(':');
        line.push_str(&json_str(v));
    }
    line.push('}');
    line
}

/// The debug directory of the session checkpointed at `session_file`.
pub fn dir_for(session_file: &str) -> PathBuf {
    Path::new(session_file).with_extension("debug")
}

#[derive(Clone)]
pub struct DebugLog {
    dir: PathBuf,
}

impl DebugLog {
    /// Creates the directory and rotates an oversized events.jsonl.
    pub fn open(dir: PathBuf) -> DebugLog {
        let _ = std::fs::create_dir_all(&dir);
        let events = dir.join("events.jsonl");
        if std::fs::metadata(&events).map(|m| m.len()).unwrap_or(0) > ROTATE_BYTES {
            let _ = std::fs::rename(&events, dir.join("events.1.jsonl"));
        }
        DebugLog { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn event(&self, kind: &str, fields: &[(&str, String)]) {
        let line = event_line(now_ms(), kind, fields);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.jsonl"))
        {
            let _ = writeln!(f, "{}", line);
        }
    }

    /// Snapshot what a crashed generation leaves behind, before the
    /// respawn overwrites it. Returns the snapshot directory.
    pub fn crash_snapshot(
        &self,
        stderr_path: &Path,
        stderr_from: u64,
        stdout_path: &Path,
        session_file: &str,
    ) -> PathBuf {
        let snap = self.dir.join(format!("crash-{}", now_ms()));
        let _ = std::fs::create_dir_all(&snap);
        let stderr = std::fs::read(stderr_path)
            .map(|b| b[(stderr_from as usize).min(b.len())..].to_vec())
            .unwrap_or_default();
        let _ = std::fs::write(snap.join("stderr.txt"), tail(&stderr));
        let stdout = std::fs::read(stdout_path).unwrap_or_default();
        let _ = std::fs::write(snap.join("stdout.txt"), tail(&stdout));
        let _ = std::fs::copy(session_file, snap.join("session.txt"));
        let env: String = std::env::vars()
            .filter(|(k, _)| k.starts_with("BEND_"))
            .map(|(k, v)| format!("{}={}\n", k, v))
            .collect();
        let _ = std::fs::write(snap.join("env.txt"), env);
        snap
    }

    /// Record a panic of this process (the TUI runs in it) with its
    /// backtrace, then let the previous hook (ratatui's terminal
    /// restore, the default report) run.
    pub fn install_panic_hook(&self) {
        let log = self.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let backtrace = std::backtrace::Backtrace::force_capture();
            let location = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default();
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            log.event(
                "panic",
                &[
                    ("message", message),
                    ("location", location),
                    ("backtrace", backtrace.to_string()),
                ],
            );
            previous(info);
        }));
    }
}

fn tail(bytes: &[u8]) -> &[u8] {
    &bytes[bytes.len().saturating_sub(SNAPSHOT_TAIL_BYTES)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_line_escapes_and_orders_fields() {
        let line = event_line(
            7,
            "crash",
            &[("why", "a \"b\"\nc".to_string()), ("n", "1".to_string())],
        );
        assert_eq!(
            line,
            "{\"ts\":7,\"src\":\"parent\",\"kind\":\"crash\",\"why\":\"a \\\"b\\\"\\nc\",\"n\":\"1\"}"
        );
    }

    #[test]
    fn dir_sits_next_to_the_checkpoint() {
        assert_eq!(
            dir_for("/h/.bend-harness/sessions/20260927-120000-42.txt"),
            PathBuf::from("/h/.bend-harness/sessions/20260927-120000-42.debug")
        );
    }

    #[test]
    fn crash_snapshot_keeps_the_generation_stderr_and_the_checkpoint() {
        let root = std::env::temp_dir().join(format!("bend-debuglog-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let err = root.join("x.err");
        std::fs::write(&err, "old generation\nbend: boom\n").unwrap();
        let out = root.join("x.log");
        std::fs::write(&out, "REPL on 1\n").unwrap();
        let sess = root.join("s.txt");
        std::fs::write(&sess, "checkpoint").unwrap();
        let log = DebugLog::open(root.join("s.debug"));
        let snap = log.crash_snapshot(&err, 15, &out, sess.to_str().unwrap());
        assert_eq!(
            std::fs::read_to_string(snap.join("stderr.txt")).unwrap(),
            "bend: boom\n"
        );
        assert_eq!(
            std::fs::read_to_string(snap.join("stdout.txt")).unwrap(),
            "REPL on 1\n"
        );
        assert_eq!(
            std::fs::read_to_string(snap.join("session.txt")).unwrap(),
            "checkpoint"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
