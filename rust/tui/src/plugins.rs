//! `/plugins` in the TUI (projects/switchboard/docs/plugins.md).
//!
//! - `/plugins`: the plugins of the workspace (the static listing).
//! - `/plugins enable|disable NAME`: edits `~/.bend-harness/plugins.json`;
//!   applies at the next `/reload` (or task restart).

use std::path::Path;

/// The plugins of `workspace` matching `q` (the `/plugins enable|disable`
/// argument, BISE-117). Resolved again at most every 5 s: the popup asks
/// at every frame.
pub(crate) fn choices(workspace: &Path, q: &str) -> Vec<crate::commands::Choice> {
    use std::cell::RefCell;
    use std::time::{Duration, Instant};
    type Cached = Option<(std::path::PathBuf, Instant, Vec<(String, String)>)>;
    thread_local! {
        static CACHE: RefCell<Cached> = const { RefCell::new(None) };
    }
    let all = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let fresh = c.as_ref().is_some_and(|(w, t, _)| w == workspace && t.elapsed() < Duration::from_secs(5));
        if !fresh {
            let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(workspace)));
            let list = res
                .plugins
                .iter()
                .map(|p| {
                    let what = p.description.clone().unwrap_or_default();
                    (p.name.clone(), format!("{} · {}", p.state.as_str(), what).trim_end_matches(" · ").to_string())
                })
                .collect();
            *c = Some((workspace.to_path_buf(), Instant::now(), list));
        }
        c.as_ref().map(|(_, _, l)| l.clone()).unwrap_or_default()
    });
    all.into_iter()
        .filter(|(n, _)| crate::commands::matches(q, &[n]))
        .map(|(n, d)| crate::commands::Choice { value: n.clone(), label: n, desc: d, mark: None })
        .collect()
}

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
