//! `bise --version` (BISE-165, packaging.md C5): the `VERSION` file of the
//! app root (a version dir, an install), else the crate version of a dev
//! build; the name is the one the command was called by (`bise`, or the
//! `bend-harness` link kept for one release).

use std::path::Path;

/// The name this process was called by: `bise`, `bend-harness`.
pub(crate) fn cmd_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|a| Path::new(&a).file_name().map(|n| n.to_string_lossy().to_string()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| bise_catalog::CLI.to_string())
}

/// What this binary was built for, in the tarballs' words: darwin-arm64.
pub(crate) fn build_target() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        o => o,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        a => a,
    };
    format!("{}-{}", os, arch)
}

/// One key of a VERSION file (`key=value` lines).
fn field<'a>(version: &'a str, key: &str) -> Option<&'a str> {
    version
        .lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

/// The version line: `bise <id> (<target>, commit <12>, built <date>)`, the
/// launcher's words; a dev build (no VERSION): `bise <crate>-dev (<target>, <root>)`.
pub(crate) fn line(cmd: &str, version: Option<&str>, target: &str, dev_root: &str) -> String {
    match version {
        Some(v) => {
            let mut parts = vec![field(v, "target").unwrap_or(target).to_string()];
            if let Some(c) = field(v, "commit") {
                parts.push(format!("commit {}", c.chars().take(12).collect::<String>()));
            }
            if let Some(b) = field(v, "built") {
                parts.push(format!("built {}", b));
            }
            format!("{} {} ({})", cmd, field(v, "id").unwrap_or("unknown"), parts.join(", "))
        }
        None => format!("{} {}-dev ({}, {})", cmd, env!("CARGO_PKG_VERSION"), target, dev_root),
    }
}

/// `--version`: the app root's VERSION when there is one.
pub(crate) fn print() {
    let root = crate::approot::locate("repl-live").ok().map(|(r, _)| r);
    let version = root.as_ref().and_then(|r| std::fs::read_to_string(r.join("VERSION")).ok());
    let dev = root.map(|r| r.display().to_string()).unwrap_or_else(|| "no app root".into());
    println!("{}", line(&cmd_name(), version.as_deref(), &build_target(), &dev));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_file_gives_the_launcher_line() {
        let v = "id=abc1234\ncommit=abc1234def5678901234\nsubject=x\nbuilt=2026-09-29T10:00:00Z\ntarget=darwin-x86_64\n";
        assert_eq!(
            line("bise", Some(v), "darwin-arm64", "/r"),
            "bise abc1234 (darwin-x86_64, commit abc1234def56, built 2026-09-29T10:00:00Z)"
        );
        // versions.sh writes no target: the build's
        assert_eq!(line("bend-harness", Some("id=a\n"), "darwin-arm64", "/r"), "bend-harness a (darwin-arm64)");
    }

    #[test]
    fn a_dev_build_says_dev_and_its_root() {
        let l = line("bise", None, "darwin-arm64", "/repo");
        assert!(l.starts_with("bise ") && l.ends_with("-dev (darwin-arm64, /repo)"), "{l}");
    }
}
