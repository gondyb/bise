//! `bise session show [<id> | <folder>] [--context] [--raw]` (BISE-199):
//! a session log for people. No id: the newest session.
use bise_session::{read_dir, show, State};
use std::path::PathBuf;

const USAGE: &str = "bise session show [<session id> | <folder>] [--context] [--raw]: a session log

  the transcript of a session (every event), or with --context what the
  model sees now; --raw prints the log lines. No id: the newest session.";

/// The session folder an argument names: a folder, or an id in the
/// sessions folder; none: the newest there.
fn find(arg: Option<&str>, sessions: &std::path::Path) -> Result<PathBuf, String> {
    match arg {
        Some(a) if std::path::Path::new(a).join("events.jsonl").exists() => Ok(PathBuf::from(a)),
        Some(a) if sessions.join(a).join("events.jsonl").exists() => Ok(sessions.join(a)),
        Some(a) => Err(format!("no session {a} in {}", sessions.display())),
        None => {
            let mut ids: Vec<String> = std::fs::read_dir(sessions)
                .map_err(|e| format!("{}: {e}", sessions.display()))?
                .flatten()
                .filter(|e| e.path().join("events.jsonl").exists())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with("s-"))
                .collect();
            ids.sort();
            ids.pop().map(|id| sessions.join(id)).ok_or_else(|| format!("no session in {}", sessions.display()))
        }
    }
}

pub fn main(args: &[String]) -> i32 {
    if args.first().map(String::as_str) != Some("show") || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return if args.first().map(String::as_str) == Some("show") { 0 } else { 2 };
    }
    let flags = |f: &str| args.iter().any(|a| a == f);
    let target = args[1..].iter().find(|a| !a.starts_with("--")).map(String::as_str);
    let home = bise_home::Home::from_env();
    let dir = match find(target, &home.sessions_dir()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}", bise_home::style::Style::stderr().fail(&format!("bise session: {e}")));
            return 1;
        }
    };
    let log = match read_dir(&dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", bise_home::style::Style::stderr().fail(&format!("bise session: {}: {e}", dir.display())));
            return 1;
        }
    };
    if let bise_session::Open::Refused(why) = &log.open {
        eprintln!("{}", bise_home::style::Style::stderr().fail(&format!("bise session: {why}")));
        return 1;
    }
    let blobs = home.blobs_dir();
    let text = if flags("--raw") {
        log.events.iter().map(|e| format!("{}\n", e.raw)).collect()
    } else if flags("--context") {
        show::context(&log, &State::rebuild(&log), &blobs)
    } else {
        show::transcript(&log, &blobs)
    };
    // `| head` closes the pipe early: not an error
    let _ = std::io::Write::write_all(&mut std::io::stdout().lock(), text.as_bytes());
    0
}
