//! `/plugins` in the TUI (projects/switchboard/docs/plugins.md).
//!
//! - `/plugins`: the plugins of the workspace (the static listing).
//! - `/plugins enable|disable NAME`: edits `~/.bend-harness/plugins.json`;
//!   applies at the next `/reload` (or task restart).

use std::path::Path;

/// The text `/plugins [args]` prints.
pub(crate) fn command(typed: &str, workspace: &Path) -> String {
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
            let text = bend_plugins::cli::listing(workspace);
            format!("plugins ({})\n{}", workspace.display(), text.trim_end())
        }
        _ => "usage: /plugins [list] | /plugins enable NAME | /plugins disable NAME".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_workspace_plugins() {
        let dir = std::env::temp_dir().join(format!("tui-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let t = command("/plugins", &dir);
        assert!(t.starts_with("plugins ("), "{}", t);
        assert!(command("/plugins bogus", &dir).starts_with("usage:"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
