//! The app root: the folder that holds bise's own files (the Bend REPLs,
//! sb-core, bend-jsrt, the tool descriptions and prompts). Never the cwd
//! (BISE-163, packaging.md C1): running an installed `bise` inside the dev
//! repo must not pick the repo's REPL.
//!
//! In order:
//!   1. `BISE_APP_ROOT` (run.sh, a hub started by a TUI, a version switch);
//!   2. the executable's folder when it holds `VERSION` (a version dir, an
//!      installed bundle);
//!   3. dev only: the executable's folder or one of its 3 parents that
//!      holds the REPL (`rust/target/<profile>/` -> the repo), then, in a
//!      debug build, the source tree it was built from (a test binary in
//!      an agent's own CARGO_TARGET_DIR).

use std::path::{Path, PathBuf};

/// The variable that names the app root.
pub(crate) const ENV: &str = "BISE_APP_ROOT";

/// Where the lookup starts: the environment, the executable, the source tree.
pub(crate) struct Inputs {
    pub(crate) env_root: Option<PathBuf>,
    /// the executable, symlinks resolved (`~/.local/bin/bise` -> its version dir)
    pub(crate) exe: Option<PathBuf>,
    /// the source tree of a debug build (`CARGO_MANIFEST_DIR/../..`), None in release
    pub(crate) dev_tree: Option<PathBuf>,
}

impl Inputs {
    pub(crate) fn from_env() -> Inputs {
        Inputs {
            env_root: std::env::var_os(ENV).filter(|v| !v.is_empty()).map(PathBuf::from),
            exe: std::env::current_exe().ok().map(|p| std::fs::canonicalize(&p).unwrap_or(p)),
            dev_tree: dev_tree(),
        }
    }
}

#[cfg(debug_assertions)]
fn dev_tree() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Some(std::fs::canonicalize(&p).unwrap_or(p))
}

#[cfg(not(debug_assertions))]
fn dev_tree() -> Option<PathBuf> {
    None
}

/// How the root was found (for `bise doctor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Via {
    Env,
    Bundle,
    DevTree,
}

/// The app root holding `repl` (a file name: repl-live, repl-scripted), or
/// why there is none. `has(dir, name)`: the file exists in dir.
pub(crate) fn find(
    inp: &Inputs,
    repl: &str,
    has: &dyn Fn(&Path, &str) -> bool,
) -> Result<(PathBuf, Via), String> {
    if let Some(r) = &inp.env_root {
        let r = std::path::absolute(r).unwrap_or_else(|_| r.clone());
        return if has(&r, repl) {
            Ok((r, Via::Env))
        } else {
            Err(format!("{}={} has no {}", ENV, r.display(), repl))
        };
    }
    let exe_dir = inp.exe.as_ref().and_then(|e| e.parent()).map(Path::to_path_buf);
    if let Some(d) = &exe_dir {
        if has(d, "VERSION") {
            return if has(d, repl) {
                Ok((d.clone(), Via::Bundle))
            } else {
                Err(format!("{} is missing from {} (reinstall bise)", repl, d.display()))
            };
        }
    }
    let dev = exe_dir
        .iter()
        .flat_map(|d| d.ancestors().take(4))
        .map(Path::to_path_buf)
        .chain(inp.dev_tree.clone());
    for d in dev {
        if has(&d, repl) {
            return Ok((d, Via::DevTree));
        }
    }
    Err(format!(
        "{} not found: no VERSION next to the executable ({}); set {} to the folder that holds it, or build it in the repo with ./run.sh",
        repl,
        exe_dir.map(|d| d.display().to_string()).unwrap_or_else(|| "unknown".into()),
        ENV
    ))
}

/// `find` on the real environment and file system.
pub(crate) fn locate(repl: &str) -> Result<(PathBuf, Via), String> {
    find(&Inputs::from_env(), repl, &|d, f| d.join(f).exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fs(files: &[&str]) -> impl Fn(&Path, &str) -> bool {
        let set: HashSet<String> = files.iter().map(|s| s.to_string()).collect();
        move |d: &Path, f: &str| set.contains(&d.join(f).to_string_lossy().to_string())
    }

    fn inp(env: Option<&str>, exe: &str, dev: Option<&str>) -> Inputs {
        Inputs {
            env_root: env.map(PathBuf::from),
            exe: Some(PathBuf::from(exe)),
            dev_tree: dev.map(PathBuf::from),
        }
    }

    #[test]
    fn env_wins_and_must_hold_the_repl() {
        let has = fs(&["/r/repl-live", "/v/VERSION", "/v/repl-live"]);
        assert_eq!(find(&inp(Some("/r"), "/v/bise", None), "repl-live", &has), Ok(("/r".into(), Via::Env)));
        let err = find(&inp(Some("/x"), "/v/bise", None), "repl-live", &has).unwrap_err();
        assert!(err.contains("BISE_APP_ROOT=/x has no repl-live"), "{err}");
    }

    #[test]
    fn a_version_dir_is_its_own_root() {
        let has = fs(&["/v/VERSION", "/v/repl-live", "/v/repl-scripted"]);
        assert_eq!(find(&inp(None, "/v/bise", None), "repl-live", &has), Ok(("/v".into(), Via::Bundle)));
        // a broken bundle is an error, never a fallback to a dev tree
        let has = fs(&["/v/VERSION", "/src/repl-live"]);
        let err = find(&inp(None, "/v/bise", Some("/src")), "repl-live", &has).unwrap_err();
        assert!(err.contains("repl-live is missing from /v"), "{err}");
    }

    #[test]
    fn the_dev_tree_is_found_from_the_executable_never_the_cwd() {
        let has = fs(&["/repo/repl-live"]);
        let got = find(&inp(None, "/repo/rust/target/release/bise", None), "repl-live", &has);
        assert_eq!(got, Ok(("/repo".into(), Via::DevTree)));
        // an agent's own CARGO_TARGET_DIR: the source tree of a debug build
        let got = find(&inp(None, "/tmp/t/debug/bise", Some("/wt")), "repl-live", &fs(&["/wt/repl-live"]));
        assert_eq!(got, Ok(("/wt".into(), Via::DevTree)));
    }

    #[test]
    fn nothing_found_says_how_to_fix() {
        let err = find(&inp(None, "/usr/local/bin/bise", None), "repl-live", &fs(&[])).unwrap_err();
        assert!(err.contains("set BISE_APP_ROOT"), "{err}");
    }
}
