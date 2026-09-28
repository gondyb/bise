//! `/plugins` in both TUIs (projects/switchboard/docs/plugins.md).
//!
//! - `/plugins`: the plugins of the workspace. A single-agent session
//!   shows its own bridge report when it exists (it adds the MCP tool
//!   counts and connection failures), else the static listing.
//! - `/plugins enable|disable NAME`: edits `~/.bend-harness/plugins.json`;
//!   applies at the next `/reload` (or task restart).

use std::path::{Path, PathBuf};

/// The bridge report of the REPL on `port`, written at its startup.
pub(crate) fn session_report(port: u16) -> PathBuf {
    bend_plugins::resolve::home()
        .join(".bend-harness/run")
        .join(port.to_string())
        .join("plugins/report.txt")
}

/// The workspace of a single-agent session: `$BEND_WORKDIR`, else the cwd.
pub(crate) fn single_workspace() -> PathBuf {
    bend_plugins::cli::workspace(&[])
}

/// The text `/plugins [args]` prints.
pub(crate) fn command(typed: &str, workspace: &Path, report: Option<&Path>) -> String {
    let mut words = typed.split_whitespace().skip(1);
    match (words.next(), words.next()) {
        (Some(sub @ ("enable" | "disable")), Some(name)) => {
            let path = bend_plugins::state::state_path();
            match bend_plugins::state::set_enabled(&path, name, sub == "enable") {
                Ok(changed) => format!(
                    "{} {}{} — applies at the next /reload",
                    name,
                    if sub == "enable" { "enabled" } else { "disabled" },
                    if changed { "" } else { " (unchanged)" }
                ),
                Err(e) => format!("{}: {}", path.display(), e),
            }
        }
        (None, _) | (Some("list"), _) => {
            let from_session = report.and_then(|p| std::fs::read_to_string(p).ok());
            let text = match from_session {
                Some(t) => t,
                None => bend_plugins::cli::listing(workspace),
            };
            format!("plugins ({})\n{}", workspace.display(), text.trim_end())
        }
        _ => "usage: /plugins [list] | /plugins enable NAME | /plugins disable NAME".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_session_report_or_the_static_listing() {
        let dir = std::env::temp_dir().join(format!("tui-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rep = dir.join("report.txt");
        std::fs::write(&rep, "hello-plugin v1 [loaded] workspace\n").unwrap();
        let t = command("/plugins", &dir, Some(&rep));
        assert!(t.contains("hello-plugin v1 [loaded]"), "{}", t);
        let t = command("/plugins", &dir, Some(&dir.join("missing")));
        assert!(t.starts_with("plugins ("), "{}", t);
        assert!(command("/plugins bogus", &dir, None).starts_with("usage:"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
