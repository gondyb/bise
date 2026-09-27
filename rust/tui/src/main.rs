//! bend-tui — the terminal UI, standalone entry point.
//!
//! The app lives in the lib so the single-executable harness
//! (crates/harness) can embed it.

fn main() -> std::io::Result<()> {
    let (host, port) = parse_args();
    let debug = std::env::args().any(|a| a == "--debug");
    let session_id = std::env::args()
        .skip(1)
        .skip_while(|a| a != "--session")
        .nth(1)
        .unwrap_or_default();
    // 7700 is the scripted REPL convention; anything else is live
    bend_tui::run(host, port, port != 7700, debug, session_id)
}

fn parse_args() -> (String, u16) {
    let mut host = "127.0.0.1".to_string();
    let mut port: u16 = 7702;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--host" && i + 1 < args.len() {
            i += 1;
            host = args[i].clone();
        } else if args[i] == "--port" && i + 1 < args.len() {
            i += 1;
            port = args[i].parse().unwrap_or(7702);
        }
        i += 1;
    }
    (host, port)
}
