//! `bend-harness plugins ...`

use std::path::PathBuf;

use crate::{bridge, report, resolve, state};

pub const USAGE: &str = "usage:
  bend-harness plugins [list] [--workspace DIR] [--json]
  bend-harness plugins enable|disable NAME
  bend-harness plugins serve --dir DIR [--workspace DIR] [--parent PID]   (internal: the session bridge)

Roots: ~/.agents/plugins (or $BEND_PLUGINS_HOME) and <workspace>/.agents/plugins.
Enable state: ~/.bend-harness/plugins.json. Changes apply at the next session start or /reload.";

fn val(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

/// The workspace: `--workspace`, else `$BEND_WORKDIR`, else the cwd.
pub fn workspace(args: &[String]) -> PathBuf {
    val(args, "--workspace")
        .or_else(|| std::env::var("BEND_WORKDIR").ok().filter(|s| !s.is_empty()))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The static listing for a workspace (no server started).
pub fn listing(ws: &std::path::Path) -> String {
    report::text(&resolve::resolve(&resolve::Roots::standard(Some(ws))), None)
}

/// Returns the process exit code.
pub fn main(args: &[String]) -> i32 {
    let sub = args.first().map(String::as_str).unwrap_or("list");
    let sub = if sub.starts_with("--") { "list" } else { sub };
    match sub {
        "list" => {
            let res = resolve::resolve(&resolve::Roots::standard(Some(&workspace(args))));
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&report::json(&res)).unwrap_or_default());
            } else {
                print!("{}", report::text(&res, None));
            }
            0
        }
        "enable" | "disable" => {
            let Some(name) = args.get(1) else {
                eprintln!("{}", USAGE);
                return 2;
            };
            let on = sub == "enable";
            let path = state::state_path();
            match state::set_enabled(&path, name, on) {
                Ok(changed) => {
                    let known = resolve::resolve(&resolve::Roots::standard(Some(&workspace(args))))
                        .plugins
                        .iter()
                        .any(|p| &p.name == name);
                    println!(
                        "{} {}{}{}",
                        name,
                        if on { "enabled" } else { "disabled" },
                        if changed { "" } else { " (unchanged)" },
                        if known { "" } else { " — note: no such plugin in the roots right now" }
                    );
                    println!("applies at the next session start or /reload ({})", path.display());
                    0
                }
                Err(e) => {
                    eprintln!("{}: {}", path.display(), e);
                    1
                }
            }
        }
        "serve" => {
            let Some(dir) = val(args, "--dir") else {
                eprintln!("{}", USAGE);
                return 2;
            };
            let parent = val(args, "--parent").and_then(|p| p.parse().ok());
            let ws = workspace(args);
            let opts = bridge::Opts {
                dir: PathBuf::from(dir),
                parent,
                roots: resolve::Roots::standard(Some(&ws)),
            };
            match bridge::serve(opts) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("plugins serve: {}", e);
                    1
                }
            }
        }
        "help" | "-h" | "--help" => {
            println!("{}", USAGE);
            0
        }
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}
