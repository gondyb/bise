//! How a client reaches the hub of a workspace: connect to `hub.sock`,
//! starting the hub first when none runs.

use crate::paths::Paths;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Start `exe sbd --workspace <ws>` detached from this terminal (its own
/// process group: closing the terminal does not stop the agents).
pub fn start_hub(paths: &Paths, exe: &Path, app_root: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    std::fs::create_dir_all(&paths.state)?;
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"))?;
    Command::new(exe)
        .arg("sbd")
        .arg("--workspace")
        .arg(&paths.workspace)
        .current_dir(app_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(err))
        .process_group(0)
        .spawn()?;
    Ok(())
}

/// A client connection (the TUI, a test): `hello` already sent.
pub fn connect(paths: &Paths, exe: &Path, app_root: &Path) -> std::io::Result<UnixStream> {
    let mut s = match UnixStream::connect(paths.socket()) {
        Ok(s) => s,
        Err(_) => {
            start_hub(paths, exe, app_root)?;
            let t0 = Instant::now();
            loop {
                if let Ok(s) = UnixStream::connect(paths.socket()) {
                    break s;
                }
                if t0.elapsed() > Duration::from_secs(15) {
                    return Err(std::io::Error::other(format!(
                        "le hub n'a pas démarré (voir {})",
                        paths.state.join("hub.err").display()
                    )));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    s.write_all(b"{\"op\":\"hello\"}\n")?;
    Ok(s)
}

/// Stop the hub of a workspace and every agent REPL.
pub fn stop(paths: &Paths) -> std::io::Result<bool> {
    match UnixStream::connect(paths.socket()) {
        Ok(mut s) => {
            s.write_all(b"{\"op\":\"hello\"}\n{\"op\":\"stop_hub\"}\n")?;
            // wait for the socket to go away
            let t0 = Instant::now();
            while paths.socket().exists() && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

/// One request, one JSON answer (the `sb` CLI).
pub fn request(socket: &Path, req: &Value, timeout: Duration) -> Result<Value, String> {
    let mut s = UnixStream::connect(socket).map_err(|e| format!("hub injoignable ({}) : {}", socket.display(), e))?;
    s.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut answer = String::new();
    r.read_line(&mut answer).map_err(|e| format!("pas de réponse du hub : {}", e))?;
    serde_json::from_str(answer.trim()).map_err(|e| format!("réponse illisible : {} ({})", e, answer.trim()))
}
