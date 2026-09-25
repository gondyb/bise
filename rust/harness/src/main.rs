//! bend-harness — the single-executable entry point.
//!
//! One process tree per terminal window:
//!   - the Bend REPL (repl-live / repl-scripted) runs as a child process;
//!     it does EVERYTHING in Bend: the provider call (hub HTTP client +
//!     core/api.bend JSON mapping) and the bash tool (hub snap runner).
//!     No bridge process anymore.
//!   - the ratatui TUI runs in the main thread.
//!
//! Ports are picked automatically, so several instances run side by side
//! with fully independent sessions. The child REPL dies with this
//! process: no orphaned listeners.
//!
//! Usage:
//!   bend-harness                # live session (real model + bash)
//!   bend-harness --scripted     # scripted session (no API)
//!   bend-harness --model NAME   # BEND_MODEL for the provider call
//!   bend-harness --port N       # force the REPL port (default: pick free)
//!   bend-harness --debug        # show turn separators and idle markers
//!   bend-harness --continue     # resume the last session (checkpoint)

use std::io::Write;
use std::net::TcpListener;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
    let mut scripted = false;
    let mut debug = false;
    let mut resume = false;
    let mut forced_port: Option<u16> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scripted" => scripted = true,
            "--debug" => debug = true,
            "--continue" => resume = true,
            "--model" if i + 1 < args.len() => {
                i += 1;
                std::env::set_var("BEND_MODEL", &args[i]);
            }
            "--port" if i + 1 < args.len() => {
                i += 1;
                forced_port = args[i].parse().ok();
            }
            other => {
                eprintln!("argument inconnu : {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // locate the Bend REPL binary next to this executable, then in cwd
    let repl_name = if scripted { "repl-scripted" } else { "repl-live" };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let repl_bin = exe_dir
        .iter()
        .map(|d| d.join(repl_name))
        .chain([std::env::current_dir()?.join(repl_name)])
        .find(|p| p.exists())
        .unwrap_or_else(|| {
            eprintln!(
                "REPL Bend introuvable : {} (compile avec `bend runtime/{} -o {}`)",
                repl_name,
                if scripted { "repl.bend" } else { "repl-live.bend" },
                repl_name
            );
            std::process::exit(1);
        });

    // REPL port: forced, or pick a free one (bind 0, drop, hand over)
    let repl_port = match forced_port {
        Some(p) => p,
        None => {
            let probe = TcpListener::bind(("127.0.0.1", 0))?;
            probe.local_addr()?.port()
        }
    };

    // session persistence: the REPL always checkpoints to this file,
    // and restores it when --continue is set
    let session_file = std::env::var("HOME")
        .map(|h| format!("{}/.bend-harness/session-{}.txt", h, repl_name))
        .unwrap_or_else(|_| format!(".bend-session-{}.txt", repl_name));
    if let Some(dir) = std::path::Path::new(&session_file).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::env::set_var("BEND_SESSION_FILE", &session_file);
    if resume {
        std::env::set_var("BEND_CONTINUE", "1");
    }

    // MCP connector index: the live REPL bootstraps the connector catalog
    // here at startup; search_mcp_tools/call_mcp_tool read it
    let mcp_index = std::env::var("HOME")
        .map(|h| format!("{}/.bend-harness/mcp-index.txt", h))
        .unwrap_or_else(|_| ".bend-mcp-index.txt".to_string());
    if let Some(dir) = std::path::Path::new(&mcp_index).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::env::set_var("BEND_MCP_INDEX", &mcp_index);

    // child REPL log, out of the TUI's way
    let log_dir = std::env::current_dir()?.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("harness-{}.log", std::process::id()));
    let log_file = std::fs::File::create(&log_path)?;
    let mut child = Command::new(&repl_bin)
        .env("BEND_REPL_PORT", repl_port.to_string())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::inherit())
        .spawn()?;

    // wait for the REPL's banner in the log — a TCP probe would steal
    // the --continue greeting (it counts as a connection)
    let start = Instant::now();
    loop {
        if let Ok(content) = std::fs::read_to_string(&log_path) {
            if content.contains("REPL on") {
                break;
            }
        }
        if start.elapsed() > Duration::from_secs(15) {
            eprintln!("le REPL Bend n'a pas démarré sur le port {}", repl_port);
            let _ = child.kill();
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = std::io::stderr().flush();
    let result = bend_tui::run("127.0.0.1".to_string(), repl_port, !scripted, debug);

    // give the REPL a beat to checkpoint the session, then die with us
    std::thread::sleep(Duration::from_millis(200));
    let _ = child.kill();
    let _ = child.wait();
    result
}
