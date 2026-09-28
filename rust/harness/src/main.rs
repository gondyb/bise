//! bend-harness — the single-executable entry point.
//!
//! One process tree per terminal window:
//!   - the Bend REPL (repl-live / repl-scripted) runs as a child process;
//!     it does EVERYTHING in Bend: the provider call (hub HTTP client +
//!     core/api.bend JSON mapping) and the bash tool (Base Process.run).
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
//!   bend-harness --continue     # resume the MOST RECENT session (by
//!                               # last activity — the file that got the
//!                               # latest save, not a fixed path)
//!   bend-harness --resume ID    # resume one session by id (a unique
//!                               # prefix of the id is accepted)
//!   bend-harness --headless     # EVERYTHING identical, minus the TUI:
//!                               # prints one READY line on stdout and
//!                               # lives until its stdin closes. This is
//!                               # how bend_client.py drives a session -
//!                               # the same script, the same binary, the
//!                               # same env, sessions and reload loop as
//!                               # the TUI; the client only speaks the
//!                               # wire protocol.
//!
//! BEND_SESSIONS_DIR overrides the sessions directory (default
//! ~/.bend-harness/sessions) - the ONLY thing the programmatic client
//! changes, so its test sessions never become the user's --continue.
//!
//! /reload (typed in the TUI, between turns) exits the Bend REPL
//! cleanly; this parent then recompiles the latest source, respawns
//! the child on the same port and reconnects the TUI — the checkpoint
//! restores the session, background commands keep running. This is
//! the self-improvement loop: the harness runs its own next version.

use std::io::Write;
use std::net::TcpListener;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ---- session ids and resolution (codex-style) ----
//
// id = UTC timestamp + pid: sortable, readable, unique across parallel
// terminals. --continue picks the file with the latest mtime (the last
// save IS the last message); --resume accepts an exact id or a unique
// prefix.

// Howard Hinnant's civil-from-days: epoch days -> (y, m, d), UTC
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn session_id_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{}",
        y,
        mo,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        std::process::id()
    )
}

fn new_session_path(sessions_dir: &str) -> String {
    format!("{}/{}.txt", sessions_dir, session_id_now())
}

fn session_mtime(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

// every *.txt in the sessions dir, newest first
fn session_files(sessions_dir: &str) -> Vec<String> {
    let mut out: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(sessions_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
                .map(|e| (session_mtime(&e.path()), e.path().to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|e| std::cmp::Reverse(e.0));
    out.into_iter().map(|(_, p)| p).collect()
}

fn latest_session(sessions_dir: &str) -> Option<String> {
    session_files(sessions_dir).into_iter().next()
}

fn session_ids(sessions_dir: &str) -> Vec<String> {
    session_files(sessions_dir)
        .iter()
        .map(|p| {
            std::path::Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .collect()
}

// exact id, or a unique prefix
fn resolve_session(sessions_dir: &str, id: &str) -> Result<String, String> {
    let exact = format!("{}/{}.txt", sessions_dir, id);
    if std::path::Path::new(&exact).exists() {
        return Ok(exact);
    }
    let hits: Vec<String> = session_ids(sessions_dir)
        .into_iter()
        .filter(|s| s.starts_with(id))
        .collect();
    match hits.len() {
        0 => Err(format!("session {} introuvable", id)),
        1 => Ok(format!("{}/{}.txt", sessions_dir, hits[0])),
        _ => Err(format!("préfixe {} ambigu ({} sessions)", id, hits.len())),
    }
}

fn list_sessions(sessions_dir: &str) {
    let ids = session_ids(sessions_dir);
    if ids.is_empty() {
        eprintln!("aucune session sauvegardée");
        return;
    }
    eprintln!("sessions disponibles (les plus récentes d'abord) :");
    for id in ids.iter().take(15) {
        eprintln!("  {}", id);
    }
}

// API keys the vibe way: KEY=VALUE lines from ~/.bend-harness/.env,
// then ~/.vibe/.env (where the vibe CLI keeps ANTHROPIC_FOUNDRY_API_KEY
// and friends). A variable already set in the real environment always
// wins; an earlier file wins over a later one. Loaded here, in the one
// entry point, so the TUI and --headless (bend_client) see the same keys.
fn load_env_files() {
    let Ok(home) = std::env::var("HOME") else { return };
    for path in [format!("{}/.bend-harness/.env", home), format!("{}/.vibe/.env", home)] {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if k.is_empty() || std::env::var_os(k).is_some_and(|x| !x.is_empty()) {
                continue;
            }
            std::env::set_var(k, v);
        }
    }
}

// ---- switchboard (projects/switchboard): one main agent, task agents ----

/// The app root: the directory holding the Bend REPL binaries (the dev
/// tree's repo root, or the bundle next to the executable).
fn app_root(repl_name: &str) -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    candidates.extend(cwd);
    if let Some(d) = exe_dir {
        // the bundle, then the dev tree (rust/target/debug -> repo root)
        candidates.push(d.clone());
        candidates.extend(d.ancestors().skip(1).take(3).map(|p| p.to_path_buf()));
    }
    candidates.into_iter().find(|d| d.join(repl_name).exists())
}

/// The workspace a switchboard command is about: --workspace, else the
/// directory the user launched from (run.sh exports it before its cd).
fn sb_workspace(args: &[String]) -> std::path::PathBuf {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--workspace" && i + 1 < args.len() {
            return std::path::PathBuf::from(&args[i + 1]);
        }
        i += 1;
    }
    std::env::var("SB_LAUNCH_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn run_sbd(args: &[String]) -> std::io::Result<()> {
    let paths = switchboard::paths::Paths::for_workspace(&sb_workspace(args));
    let Some(root) = app_root("repl-live") else {
        eprintln!("repl-live introuvable (compile avec `bend runtime/repl-live.bend -o repl-live`)");
        std::process::exit(1);
    };
    // the hub's decisions (sb-core) come from the same app root as the
    // REPLs: a version runs its own sb-core, not the dev tree's
    if std::env::var_os("SB_CORE_BIN").is_none() && root.join("sb-core").exists() {
        std::env::set_var("SB_CORE_BIN", root.join("sb-core"));
    }
    // the agents' REPLs load their MCP index like a normal session
    if std::env::var_os("BEND_MCP_INDEX").is_none() {
        if let Ok(h) = std::env::var("HOME") {
            std::env::set_var("BEND_MCP_INDEX", format!("{}/.bend-harness/mcp-index.txt", h));
        }
    }
    switchboard::daemon::run(switchboard::daemon::Opts {
        paths,
        repl_bin: root.join("repl-live"),
        app_root: root,
        exe: std::env::current_exe()?,
    })
}

fn run_switchboard(args: &[String], debug: bool) -> std::io::Result<()> {
    let paths = switchboard::paths::Paths::for_workspace(&sb_workspace(args));
    if args.iter().any(|a| a == "--stop") {
        let keep = args.iter().any(|a| a == "--keep-agents");
        match switchboard::client::stop(&paths, keep)? {
            true => eprintln!("hub arrêté ({})", paths.workspace.display()),
            false => eprintln!("aucun hub pour {}", paths.workspace.display()),
        }
        return Ok(());
    }
    let Some(root) = app_root("repl-live") else {
        eprintln!("repl-live introuvable");
        std::process::exit(1);
    };
    let exe = std::env::current_exe()?;
    let stream = switchboard::client::connect(&paths, &exe, &root)?;
    bend_tui::run_switchboard(
        stream,
        paths.socket(),
        paths.workspace.to_string_lossy().to_string(),
        debug,
    )
}

fn main() -> std::io::Result<()> {
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(|s| s.as_str()) {
            Some("sb") => std::process::exit(switchboard::cli::main(&args[1..])),
            Some("sbd") => {
                load_env_files();
                return run_sbd(&args[1..]);
            }
            Some("switchboard") => {
                let debug = args.iter().any(|a| a == "--debug");
                return run_switchboard(&args[1..], debug);
            }
            _ => {}
        }
    }
    load_env_files();
    let mut scripted = false;
    let mut headless = false;
    let mut debug = false;
    let mut resume = false;
    let mut resume_id: Option<String> = None;
    let mut forced_port: Option<u16> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scripted" => scripted = true,
            "--headless" => headless = true,
            "--debug" => debug = true,
            "--continue" => resume = true,
            "--resume" if i + 1 < args.len() => {
                i += 1;
                resume_id = Some(args[i].clone());
            }
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
    if resume && resume_id.is_some() {
        eprintln!("utilise --continue OU --resume <id>, pas les deux");
        std::process::exit(1);
    }

    // locate the Bend REPL binary next to this executable, then in cwd
    let repl_name = if scripted { "repl-scripted" } else { "repl-live" };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    // run FROM ANYWHERE: the bundle is self-contained, every runtime
    // path (the bend sources, the tool descriptions, the jsrt engine,
    // the reload recompile) is relative to the application root - the
    // directory this executable lives in. Move the process there once,
    // so neither the user cwd nor a launcher location can break it.
    // The DEV tree is the opposite case: the launch cwd IS the app
    // root (./run.sh from the repo), and the executable lives in
    // rust/target/debug - moving there would break every relative
    // path. Skip the move when the cwd already has the REPL.
    let cwd_has_repl = std::env::current_dir()
        .ok()
        .is_some_and(|d| d.join(repl_name).exists());
    if !cwd_has_repl {
        if let Some(root) = exe_dir.as_ref() {
            let _ = std::env::set_current_dir(root);
        }
    }
    // ONE location for the REPL: the app root, which the cwd now is in
    // both layouts (the dev tree, where ./run.sh builds ./repl-live, and
    // the bundle, where we just moved next to the executable). The old
    // lookup tried next-to-the-executable FIRST: in the dev tree that is
    // rust/target/debug/, where a stale repl-live had landed (a /reload
    // recompiled into whatever path was found, perpetuating it) - the
    // TUI ran an old runtime while ./repl-live, the one built, tested
    // and committed, sat unused.
    let repl_bin = Some(std::env::current_dir()?.join(repl_name))
        .filter(|p| p.exists())
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

    // session persistence, codex-style: every fresh run checkpoints to
    // its OWN file (~/.bend-harness/sessions/<id>.txt, id = UTC timestamp
    // + pid), --continue resumes the file with the LATEST mtime (the
    // session that received the last message), --resume <id> one exact
    // session (a unique id prefix is accepted). The single fixed file
    // of the old scheme is the fallback when no per-session file exists
    // yet (back-compat with the pre-sessions checkpoint).
    let sessions_dir = std::env::var("BEND_SESSIONS_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(|h| format!("{}/.bend-harness/sessions", h))
                .unwrap_or_else(|_| ".bend-sessions".to_string())
        });
    let _ = std::fs::create_dir_all(&sessions_dir);
    let legacy_file = std::env::var("HOME")
        .map(|h| format!("{}/.bend-harness/session-{}.txt", h, repl_name))
        .unwrap_or_else(|_| format!(".bend-session-{}.txt", repl_name));
    let session_file = if let Some(id) = &resume_id {
        match resolve_session(&sessions_dir, id) {
            Ok(path) => path,
            Err(msg) => {
                eprintln!("{}", msg);
                list_sessions(&sessions_dir);
                std::process::exit(1);
            }
        }
    } else if resume {
        latest_session(&sessions_dir).unwrap_or_else(|| {
            if std::path::Path::new(&legacy_file).exists() {
                eprintln!("--continue : aucune session nommée, reprise du checkpoint historique");
                legacy_file.clone()
            } else {
                eprintln!("--continue : aucune session à reprendre, nouvelle session");
                new_session_path(&sessions_dir)
            }
        })
    } else {
        new_session_path(&sessions_dir)
    };
    let session_id = session_file
        .rsplit('/')
        .next()
        .unwrap_or("session")
        .trim_end_matches(".txt")
        .to_string();
    eprintln!("session : {}", session_id);
    std::env::set_var("BEND_SESSION_FILE", &session_file);
    if resume || resume_id.is_some() {
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

    // child REPL log, out of the TUI's way. Recreated per spawn, so
    // the reload-exit marker of one generation never leaks to the next.
    let log_dir = std::env::current_dir()?.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("harness-{}.log", std::process::id()));

    // the run loop: spawn → wait banner → TUI. When the TUI returns,
    // WHY it returned decides what happens:
    //   - child exited with the "reload-exit" marker: a /reload ran.
    //     Recompile the latest source, respawn with BEND_CONTINUE=1
    //     (same port, same session file), reconnect. This is the
    //     self-improvement loop: run the harness on its own next
    //     version without losing the session.
    //   - child died any other way (a crash): respawn it on the
    //     checkpointed session with BEND_CRASH_NOTE, which the new
    //     REPL shows the user after the replayed history.
    //   - child alive: the user quit the TUI — die together.
    // headless: the client's hang-up is our stdin closing (a crashed
    // client closes it too) - the child never outlives its client
    let stdin_closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if headless {
        let flag = stdin_closed.clone();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    // the child's stderr: where the Bend runtime writes why it died
    // ("bend: out of memory", "bend: memory fault", ...) before its
    // _exit(1). Inherited, it vanished behind the TUI's alternate
    // screen; appended here, it survives every generation.
    let err_path = log_dir.join(format!("harness-{}.err", std::process::id()));

    let mut reloads = 0usize;
    // a crashed REPL respawns on the checkpointed session (written
    // before every provider call, so the turn's history up to its last
    // call is back); a crash loop stops after MAX_CRASH_RESTARTS
    // generations that each died young
    let mut crashes = 0usize;
    let mut crash_note: Option<String> = None;
    loop {
        let log_file = std::fs::File::create(&log_path)?;
        let err_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&err_path)?;
        let err_start = std::fs::metadata(&err_path).map(|m| m.len()).unwrap_or(0);
        let mut cmd = Command::new(&repl_bin);
        cmd.env("BEND_REPL_PORT", repl_port.to_string())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file));
        match crash_note.take() {
            Some(note) => cmd.env("BEND_CRASH_NOTE", note),
            None => cmd.env_remove("BEND_CRASH_NOTE"),
        };
        let spawned_at = Instant::now();
        let mut child = cmd.spawn()?;

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

        // what the REPL announced - the single source of truth for the
        // model, the threshold and the side-channel paths
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        let info = match bend_tui::HarnessInfo::from_log(&log) {
            Some(i) => i,
            None => {
                eprintln!("le REPL Bend n'a pas annoncé sa configuration (harness-info)");
                let _ = child.kill();
                std::process::exit(1);
            }
        };

        let _ = std::io::stderr().flush();
        let result = if headless {
            // the machine handshake: one line, the same facts the TUI
            // displays, plus where the child logs
            println!(
                "READY port={} session={} model={} threshold={} steer={} interrupt={} log={}",
                repl_port,
                session_id,
                info.model,
                info.threshold,
                info.steer_path,
                info.interrupt_path,
                log_path.display()
            );
            let _ = std::io::stdout().flush();
            // live until the child exits (a reload or a crash) or the
            // client hangs up
            loop {
                if stdin_closed.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        } else {
            bend_tui::run("127.0.0.1".to_string(), repl_port, info, debug, session_id.clone())
        };

        // the connection closed — why did the child stop?
        // A /reload closes the socket BEFORE exiting (it checkpoints
        // after the close), so the TUI always disconnects while the
        // child is still landing its exit status. Without a grace wait
        // every reload raced into the user-closed-TUI branch and the
        // parent killed the session (the manual --resume every time).
        // Give the child up to 3s to exit on its own: a reload exits in
        // milliseconds; only a user-closed TUI leaves it alive.
        let mut exited = child.try_wait().ok().flatten();
        if exited.is_none() {
            for _ in 0..60 {
                std::thread::sleep(Duration::from_millis(50));
                exited = child.try_wait().ok().flatten();
                if exited.is_some() {
                    break;
                }
            }
        }
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        match (&exited, log.contains("reload-exit")) {
            (Some(status), true) if status.success() => {
                reloads += 1;
                if reloads > 10 {
                    eprintln!("reload : trop de redémarrages d'affilée, arrêt.");
                    return Ok(());
                }
                eprintln!(
                    "reload : recompilation de la dernière version de {} (1-2 min)...",
                    repl_name
                );
                if !recompile(repl_name, &repl_bin) {
                    eprintln!(
                        "reload : la recompilation a échoué — on garde le binaire précédent (session intacte)."
                    );
                }
                // the respawn restores the checkpointed session
                std::env::set_var("BEND_CONTINUE", "1");
                continue;
            }
            (Some(status), _) => {
                // a crash: never take the session down with it
                if spawned_at.elapsed() > CRASH_WINDOW {
                    crashes = 0;
                }
                crashes += 1;
                let why = crash_reason(&err_path, err_start, &status.to_string());
                if crashes > MAX_CRASH_RESTARTS {
                    eprintln!(
                        "le REPL Bend a planté {} fois d'affilée ({}) — arrêt. Détails : {}",
                        MAX_CRASH_RESTARTS,
                        why,
                        err_path.display()
                    );
                    return result;
                }
                eprintln!(
                    "le REPL Bend a planté ({}) — redémarrage sur la session sauvegardée ({}/{})...",
                    why, crashes, MAX_CRASH_RESTARTS
                );
                std::env::set_var("BEND_CONTINUE", "1");
                crash_note = Some(why);
                continue;
            }
            (None, _) => {
                // child alive: the user closed the TUI — give the REPL a
                // beat to checkpoint the session, then die with us
                std::thread::sleep(Duration::from_millis(200));
                let _ = child.kill();
                let _ = child.wait();
                return result;
            }
        }
    }
}

const MAX_CRASH_RESTARTS: usize = 5;
// a generation that lived longer than this was not a crash loop
const CRASH_WINDOW: Duration = Duration::from_secs(120);

// why the child died: the runtime's last "bend: ..." line on stderr
// (written since this generation started), else its last line, else
// the exit status alone
fn crash_reason(err_path: &std::path::Path, from: u64, status: &str) -> String {
    let text = std::fs::read(err_path)
        .map(|b| String::from_utf8_lossy(&b[(from as usize).min(b.len())..]).into_owned())
        .unwrap_or_default();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let said = lines
        .iter()
        .rev()
        .find(|l| l.starts_with("bend:"))
        .or(lines.last())
        .map(|l| l.chars().take(200).collect::<String>());
    match said {
        Some(l) => format!("{} · {}", status, l),
        None => status.to_string(),
    }
}

// recompile the REPL from the checked-out source so a reload runs the
// latest code; on failure the caller keeps the previous binary
fn recompile(repl_name: &str, repl_bin: &std::path::Path) -> bool {
    let source = match repl_name {
        "repl-scripted" => "runtime/repl.bend",
        _ => "runtime/repl-live.bend",
    };
    let Some(src) = std::env::current_dir()
        .ok()
        .map(|d| d.join(source))
        .filter(|p| p.exists())
    else {
        eprintln!("reload : source {} introuvable, on garde le binaire.", source);
        return false;
    };
    let home_bend = format!("{}/.bend/bin/bend", std::env::var("HOME").unwrap_or_default());
    let bend = ["bend", home_bend.as_str()]
        .into_iter()
        .find(|c| which_lookup(c))
        .unwrap_or("bend");
    let status = Command::new(bend)
        .arg(&src)
        .arg("-o")
        .arg(repl_bin)
        .status();
    matches!(status, Ok(s) if s.success())
}

fn which_lookup(cmd: &str) -> bool {
    if cmd.contains('/') {
        return std::path::Path::new(cmd).exists();
    }
    std::env::var("PATH")
        .map(|p| {
            p.split(':').any(|dir| {
                std::path::Path::new(dir)
                    .join(cmd)
                    .try_exists()
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
