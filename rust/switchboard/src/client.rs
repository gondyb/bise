//! How a client reaches the hub of a workspace: connect to `hub.sock`,
//! starting the hub first when none runs.

use crate::paths::Paths;
use serde_json::{json, Value};
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
        // the hub uses the TUI's app root, never its own lookup's
        .env("BISE_APP_ROOT", app_root)
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
                        "the hub did not start (see {})",
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

/// Stop the hub of a workspace and every agent REPL. `keep_agents`:
/// the REPLs keep running (their turns too) for the next hub to adopt -
/// a version switch, a hub restart.
pub fn stop(paths: &Paths, keep_agents: bool) -> std::io::Result<bool> {
    match UnixStream::connect(paths.socket()) {
        Ok(mut s) => {
            let req = json!({"op": "stop_hub", "keep_agents": keep_agents});
            s.write_all(format!("{{\"op\":\"hello\"}}\n{}\n", req).as_bytes())?;
            // wait for the socket to go away, reading what the hub
            // writes: its hello (the versions list alone is tens of KB
            // with long commit subjects) must not fill the socket
            // buffer, or the hub blocks on it and never stops
            let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
            let mut sink = [0u8; 65536];
            let t0 = Instant::now();
            while paths.socket().exists() && t0.elapsed() < Duration::from_secs(5) {
                match std::io::Read::read(&mut s, &mut sink) {
                    Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

/// One request, one JSON answer (the `sb` CLI).
pub fn request(socket: &Path, req: &Value, timeout: Duration) -> Result<Value, String> {
    let mut s = UnixStream::connect(socket)
        .map_err(|e| format!("hub injoignable ({}) : {}", socket.display(), e))?;
    s.set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut answer = String::new();
    r.read_line(&mut answer)
        .map_err(|e| format!("no answer from the hub: {}", e))?;
    serde_json::from_str(answer.trim())
        .map_err(|e| format!("unreadable answer: {} ({})", e, answer.trim()))
}

/// `request`, through a hub restart (a version switch, a crash): a hub
/// that is not there yet is waited for (up to 20 s); a connection the hub
/// closed without answering is retried when `idempotent` (a read, a
/// wait) - never a send, which may have been done already.
pub fn request_retry(
    socket: &Path,
    req: &Value,
    timeout: Duration,
    idempotent: bool,
) -> Result<Value, String> {
    let t0 = Instant::now();
    loop {
        let connected = UnixStream::connect(socket).is_ok();
        let r = if connected {
            request(socket, req, timeout)
        } else {
            Err("hub absent".to_string())
        };
        let again = match &r {
            Ok(_) => false,
            Err(_) if !connected => true,
            Err(e) => idempotent && e.starts_with("unreadable answer"),
        };
        if !again || t0.elapsed() > Duration::from_secs(20) {
            return r;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
