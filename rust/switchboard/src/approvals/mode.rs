//! The global mode (design §8): `yolo` or `auto`, remembered in
//! config.toml (`approvals = "auto"`), `BISE_APPROVALS` for one session.
//! The hub holds the live one and writes it next to each agent's gate
//! file, where the runtime reads it before each gated call.

use super::Mode;
use std::path::{Path, PathBuf};

/// Wins over config.toml for this session, never written.
pub const ENV: &str = "BISE_APPROVALS";

/// The mode file in the agent's `run/` folder (the runtime reads it).
pub const MODE_FILE: &str = "approvals-mode";

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "yolo" => Some(Mode::Yolo),
            "auto" => Some(Mode::Auto),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Mode::Yolo => "yolo",
            Mode::Auto => "auto",
        }
    }

    /// What `shift+tab` switches to.
    pub fn other(self) -> Mode {
        match self {
            Mode::Yolo => Mode::Auto,
            Mode::Auto => Mode::Yolo,
        }
    }
}

/// config.toml's top-level `approvals` (absent or unreadable: `yolo`).
pub fn from_config(text: &str) -> Mode {
    text.lines()
        .map(str::trim)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "approvals")
        .and_then(|(_, v)| Mode::parse(v.split('#').next().unwrap_or("").trim().trim_matches('"')))
        .unwrap_or(Mode::Yolo)
}

/// The session's mode: `BISE_APPROVALS` when it names one (then true:
/// a switch is for this session only), else config.toml's.
pub fn resolve(config: &str, env: Option<&str>) -> (Mode, bool) {
    match env.and_then(Mode::parse) {
        Some(m) => (m, true),
        None => (from_config(config), false),
    }
}

/// Where the hub answers the runtime's gate lines (spec §3): next to the
/// interrupt file, one per REPL process.
pub fn gate_file(run: &Path, port: u16) -> PathBuf {
    run.join(format!("bend-gate-{port}.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_comes_from_the_env_then_the_config() {
        assert_eq!(from_config(""), Mode::Yolo);
        assert_eq!(from_config("model = \"a/b\"\napprovals = \"auto\" # mine\n"), Mode::Auto);
        assert_eq!(from_config("approvals = auto\n"), Mode::Auto);
        assert_eq!(from_config("[roles]\napprovals = \"auto\"\n"), Mode::Yolo, "top level only");
        assert_eq!(from_config("approvals = \"maybe\"\n"), Mode::Yolo);
        assert_eq!(resolve("approvals = \"auto\"", None), (Mode::Auto, false));
        assert_eq!(resolve("approvals = \"auto\"", Some("yolo")), (Mode::Yolo, true));
        assert_eq!(resolve("", Some("nonsense")), (Mode::Yolo, false));
        assert_eq!(Mode::Yolo.other(), Mode::Auto);
        assert_eq!(gate_file(Path::new("/h/agents/a/run"), 7702), PathBuf::from("/h/agents/a/run/bend-gate-7702.txt"));
    }
}
