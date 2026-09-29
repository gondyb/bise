//! `sb`: what an agent runs, through its bash tool, to reach the group
//! (RFC 0003 §4, RFC 0001 §7.2 and §7.5). It speaks to the hub of its
//! workspace (`SB_SOCKET`) as `SB_AGENT`.

use serde_json::{json, Map, Value};
use std::io::Read;
use std::time::Duration;

pub const USAGE: &str = "\
sb list
sb tasks
sb send <agent> \"<text>\" [--expect-reply] [--reply-to m_<n>] [--mode steer|queued] [--why \"<reason>\"]
sb ask <agent> \"<question>\" [--timeout <s>]
sb wait m_<n> [--timeout <s>]
sb status working|done|blocked [--note \"<text>\"]
sb report progress|done|failed|blocked \"<summary>\" [--decision \"<text>\"]...
sb inspect <agent> [--query <text>] [--before|--after|--around|--at #<pos>] [--limit <n>]
sb inspect main --origin
main only:
sb spawn <name> --objective \"…\" [--context \"…\"] [--constraint \"…\"]... [--done-when \"…\"] [--report-format \"…\"] [--worktree [--with-changes]]
sb interrupt <agent> | sb stop <agent> \"<reason>\" | sb drop <agent>
sb close <card> [\"<note>\"] | sb rename <agent> <new-name>
sb restore <agent> | sb isolate <agent>   (only on the user's explicit request)
sb card \"<question for the user>\" [--for m_<n>]
sb history \"<query>\"
sb version [list | switch <commit|id|tree> | rollback]   (versions of Switchboard itself; list: everyone)
sb restart [current|<commit>]   (main only: restart the hub safely, agents kept; default: build + restart on the latest commit; current: the running version, no rebuild)
A text argument `-` reads the text from stdin.";

/// Split flags from positional words. `flags` take a value, `switches`
/// do not; a repeated flag accumulates.
fn parse_args(
    args: &[String],
    flags: &[&str],
    switches: &[&str],
) -> Result<(Vec<String>, Map<String, Value>), String> {
    let mut pos: Vec<String> = Vec::new();
    let mut opts: Map<String, Value> = Map::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = a.strip_prefix("--") {
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (name.to_string(), None),
            };
            if switches.contains(&name.as_str()) {
                opts.insert(name, json!(true));
            } else if flags.contains(&name.as_str()) {
                let v = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        args.get(i)
                            .cloned()
                            .ok_or(format!("--{} expects a value", name))?
                    }
                };
                match opts.get_mut(&name) {
                    Some(Value::Array(a)) => a.push(json!(v)),
                    Some(prev) => {
                        let p = prev.clone();
                        *prev = json!([p, v]);
                    }
                    None => {
                        opts.insert(name, json!(v));
                    }
                }
            } else {
                return Err(format!("unknown option: --{}", name));
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Ok((pos, opts))
}

fn text_of(words: &[String]) -> Result<String, String> {
    if words.len() == 1 && words[0] == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        return Ok(s.trim().to_string());
    }
    let t = words.join(" ");
    if t.trim().is_empty() {
        return Err("missing text".into());
    }
    Ok(t)
}

fn list_of(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The first positional word, an agent name (`@name` accepted).
fn agent_arg(pos: &[String], usage: impl Into<String>) -> Result<String, String> {
    pos.first()
        .map(|a| a.trim_start_matches('@').to_string())
        .ok_or_else(|| usage.into())
}

/// `--timeout <s>`, 20 s by default.
fn timeout_of(opts: &Map<String, Value>) -> u64 {
    str_of(opts, "timeout").parse::<u64>().unwrap_or(20)
}

fn str_of(opts: &Map<String, Value>, k: &str) -> String {
    match opts.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.last().and_then(|x| x.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

/// The JSON request for `sb <args>`.
pub fn build(args: &[String]) -> Result<Value, String> {
    let Some(cmd) = args.first() else {
        return Err(USAGE.into());
    };
    let rest = &args[1..];
    let mut req = Map::new();
    req.insert("cmd".into(), json!(cmd));
    match cmd.as_str() {
        "list" | "tasks" | "history" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            if cmd == "history" {
                req.insert("query".into(), json!(text_of(&pos)?));
            }
        }
        "send" => {
            let (pos, o) = parse_args(rest, &["reply-to", "mode", "why"], &["expect-reply"])?;
            let to = agent_arg(&pos, "usage: sb send <agent> \"<text>\"")?;
            req.insert("to".into(), json!(to));
            req.insert("text".into(), json!(text_of(&pos[1..])?));
            req.insert("expect_reply".into(), json!(o.contains_key("expect-reply")));
            if o.contains_key("mode") {
                match str_of(&o, "mode").as_str() {
                    m @ ("steer" | "queued") => {
                        req.insert("mode".into(), json!(m));
                    }
                    m => return Err(format!("unknown mode: {} (steer|queued)", m)),
                }
            }
            if o.contains_key("reply-to") {
                req.insert("reply_to".into(), json!(str_of(&o, "reply-to")));
            }
            // main answering a task for the user: the reason, shown to
            // the user in main's feed (C2 `answered`)
            if o.contains_key("why") {
                req.insert("why".into(), json!(str_of(&o, "why")));
            }
        }
        "ask" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            let to = agent_arg(&pos, "usage: sb ask <agent> \"<question>\"")?;
            req.insert("to".into(), json!(to));
            req.insert("text".into(), json!(text_of(&pos[1..])?));
            req.insert("timeout_s".into(), json!(timeout_of(&o)));
        }
        "wait" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            req.insert(
                "msg".into(),
                json!(pos.first().ok_or("usage : sb wait m_<n>")?),
            );
            req.insert("timeout_s".into(), json!(timeout_of(&o)));
        }
        "status" => {
            let (pos, o) = parse_args(rest, &["note"], &[])?;
            req.insert(
                "status".into(),
                json!(pos
                    .first()
                    .ok_or("usage: sb status working|done|blocked")?),
            );
            req.insert("note".into(), json!(str_of(&o, "note")));
        }
        "report" => {
            let (pos, o) = parse_args(rest, &["decision"], &[])?;
            req.insert(
                "kind".into(),
                json!(pos.first().ok_or("usage: sb report <kind> \"<summary>\"")?),
            );
            req.insert("summary".into(), json!(text_of(&pos[1..])?));
            req.insert("decisions".into(), json!(list_of(o.get("decision"))));
        }
        "spawn" => {
            let (pos, o) = parse_args(
                rest,
                &[
                    "objective",
                    "context",
                    "constraint",
                    "done-when",
                    "report-format",
                ],
                &["worktree", "with-changes"],
            )?;
            req.insert(
                "name".into(),
                json!(pos.first().cloned().unwrap_or_default()),
            );
            let mut objective = str_of(&o, "objective");
            if objective.is_empty() && pos.len() > 1 {
                objective = pos[1..].join(" ");
            }
            if objective.is_empty() {
                return Err("sb spawn: --objective is required".into());
            }
            req.insert("objective".into(), json!(objective));
            req.insert("context".into(), json!(str_of(&o, "context")));
            req.insert("constraints".into(), json!(list_of(o.get("constraint"))));
            req.insert("done_when".into(), json!(str_of(&o, "done-when")));
            req.insert("report_format".into(), json!(str_of(&o, "report-format")));
            req.insert("worktree".into(), json!(o.contains_key("worktree")));
            req.insert("with_changes".into(), json!(o.contains_key("with-changes")));
        }
        "interrupt" | "drop" | "restore" | "isolate" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            req.insert("agent".into(), json!(agent_arg(&pos, format!("usage: sb {} <agent>", cmd))?));
        }
        "stop" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            req.insert("agent".into(), json!(agent_arg(&pos, "usage: sb stop <agent> \"<reason>\"")?));
            req.insert("reason".into(), json!(pos[1..].join(" ")));
        }
        "close" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let card = pos
                .first()
                .and_then(|c| c.trim_start_matches('#').parse::<u64>().ok())
                .ok_or("usage: sb close <card> [\"<note>\"]")?;
            req.insert("card".into(), json!(card));
            req.insert("note".into(), json!(pos[1..].join(" ")));
        }
        "rename" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            match pos.as_slice() {
                [a, b] => {
                    req.insert("agent".into(), json!(a.trim_start_matches('@')));
                    req.insert("new_name".into(), json!(b.trim_start_matches('@')));
                }
                _ => return Err("usage: sb rename <agent> <new-name>".into()),
            }
        }
        "card" => {
            let (pos, o) = parse_args(rest, &["for"], &[])?;
            req.insert("text".into(), json!(text_of(&pos)?));
            if o.contains_key("for") {
                req.insert("for".into(), json!(str_of(&o, "for")));
            }
        }
        "inspect" => {
            let (pos, o) = parse_args(
                rest,
                &["last", "limit", "query", "before", "after", "around", "at"],
                &["origin"],
            )?;
            req.insert("agent".into(), json!(agent_arg(&pos, "usage: sb inspect <agent>")?));
            let n = if o.contains_key("limit") {
                str_of(&o, "limit")
            } else {
                str_of(&o, "last")
            };
            req.insert("last".into(), json!(n.parse::<u64>().unwrap_or(20)));
            req.insert("query".into(), json!(str_of(&o, "query")));
            for k in ["before", "after", "around", "at"] {
                if o.contains_key(k) {
                    let p = str_of(&o, k);
                    if crate::transcript::parse_pos(&p).is_none() {
                        return Err(format!("--{} expects a position (#<n>)", k));
                    }
                    req.insert(k.into(), json!(p));
                }
            }
            req.insert("origin".into(), json!(o.contains_key("origin")));
        }
        "help" | "--help" | "-h" => return Err(USAGE.into()),
        other => return Err(format!("unknown command: {}\n{}", other, USAGE)),
    }
    Ok(Value::Object(req))
}

/// What the agent reads back.
pub fn render(cmd: &str, v: &Value) -> (bool, String) {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    if v.get("ok") != Some(&json!(true)) {
        let mut e = format!("error: {}", s("error"));
        if !s("hint").is_empty() {
            e.push_str(&format!("\n{}", s("hint")));
        }
        return (false, e);
    }
    let text = match cmd {
        "list" | "tasks" | "inspect" | "history" => s("text"),
        "send" => format!("sent {} to {} ({}, thread {})", s("message_id"), s("to"), s("delivery"), s("thread")),
        "ask" | "wait" => match s("type").as_str() {
            // `answers m_8`: the question's id (BISE-110: the TUI hides
            // an `sb ask` box whose question and reply both show)
            "reply" => format!(
                "reply from {} ({}{}{}):\n{}",
                s("from"),
                s("message_id"),
                if s("asked").is_empty() { String::new() } else { format!(", answers {}", s("asked")) },
                if v.get("auto") == Some(&json!(true)) { ", automatic: the end of its turn" } else { "" },
                s("message")
            ),
            _ => format!(
                "incoming message from {} ({}, thread {}{}) — your wait ended so you can answer it:\n{}",
                s("from"),
                s("message_id"),
                s("thread"),
                if v.get("expects_reply") == Some(&json!(true)) { ", expects a reply" } else { "" },
                s("message")
            ),
        },
        "spawn" => format!(
            "agent {} created{} — it starts now; its answer will come back as a message",
            s("name"),
            v.get("branch").and_then(|b| b.as_str()).map(|b| format!(" in worktree {} (branch {})", s("path"), b)).unwrap_or_default()
        ),
        "drop" => {
            if v.get("dropped") == Some(&json!(true)) {
                "dropped".to_string()
            } else {
                format!("not dropped: {} (card #{})", s("reason"), v.get("card").and_then(|c| c.as_u64()).unwrap_or(0))
            }
        }
        "card" => format!("card #{} opened for the user", v.get("card").and_then(|c| c.as_u64()).unwrap_or(0)),
        "report" => format!("reported ({})", s("message_id")),
        "close" => format!("card #{} closed", v.get("card").and_then(|c| c.as_u64()).unwrap_or(0)),
        "rename" => format!("renamed: now @{} (the old name still works)", s("name")),
        "restore" => format!("@{} restored", s("name")),
        "isolate" => format!("@{} now works in its own git worktree", s("name")),
        _ => "ok".to_string(),
    };
    (true, text)
}

/// `sb …` entry point: the process exit code.
/// `sb version [list | switch <commit|id|tree> | rollback]`: the
/// versions of Switchboard itself (a hub op, not an agent request).
fn version(args: &[String]) -> i32 {
    let socket = std::env::var("SB_SOCKET").unwrap_or_default();
    if socket.is_empty() {
        eprintln!("sb : SB_SOCKET manque");
        return 2;
    }
    let what = args.get(1).map(|s| s.as_str()).unwrap_or("list");
    let from = std::env::var("SB_AGENT").unwrap_or_default();
    let req = json!({"op": "version", "do": what, "to": args.get(2).cloned().unwrap_or_default(), "from": from});
    match crate::client::request_retry(
        std::path::Path::new(&socket),
        &req,
        Duration::from_secs(30),
        what == "list",
    ) {
        Ok(v) if v.get("ok") == Some(&json!(false)) => {
            eprintln!(
                "error: {}",
                v.get("error").and_then(|t| t.as_str()).unwrap_or("")
            );
            1
        }
        Ok(v) => {
            println!("{}", v.get("text").and_then(|t| t.as_str()).unwrap_or(""));
            0
        }
        Err(e) => {
            eprintln!("sb : {}", e);
            1
        }
    }
}

pub fn main(args: &[String]) -> i32 {
    if args.first().map(|s| s.as_str()) == Some("version") {
        return version(args);
    }
    if args.first().map(|s| s.as_str()) == Some("restart") {
        // `sb restart [current|<commit>]` (main only): the hub op
        let mut a = vec!["version".to_string(), "restart".to_string()];
        a.extend(args.get(1).cloned());
        return version(&a);
    }
    let req = match build(args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let socket = std::env::var("SB_SOCKET").unwrap_or_default();
    let from = std::env::var("SB_AGENT").unwrap_or_default();
    if socket.is_empty() || from.is_empty() {
        eprintln!(
            "sb: SB_SOCKET and SB_AGENT are missing (sb only runs inside a Switchboard agent)"
        );
        return 2;
    }
    let mut req = req;
    req["op"] = json!("agent");
    req["from"] = json!(from);
    let cmd = args[0].clone();
    let timeout = req.get("timeout_s").and_then(|t| t.as_u64()).unwrap_or(0) + 30;
    let idempotent = matches!(
        cmd.as_str(),
        "list" | "tasks" | "history" | "inspect" | "wait"
    );
    match crate::client::request_retry(
        std::path::Path::new(&socket),
        &req,
        Duration::from_secs(timeout),
        idempotent,
    ) {
        Ok(v) => {
            let (ok, text) = render(&cmd, &v);
            if ok {
                println!("{}", text);
                0
            } else {
                eprintln!("{}", text);
                1
            }
        }
        Err(e) => {
            eprintln!("sb : {}", e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn inspect_cursors() {
        let r = build(&a(&[
            "inspect",
            "@main",
            "--query",
            "mode sombre",
            "--before",
            "#120",
            "--limit",
            "5",
        ]))
        .unwrap();
        assert_eq!(r["agent"], "main");
        assert_eq!(r["query"], "mode sombre");
        assert_eq!(r["before"], "#120");
        assert_eq!(r["last"], 5);
        assert_eq!(r["origin"], false);
        let o = build(&a(&["inspect", "main", "--origin"])).unwrap();
        assert_eq!(o["origin"], true);
        assert!(build(&a(&["inspect", "main", "--around", "abc"])).is_err());
    }

    #[test]
    fn send_flags() {
        let r = build(&a(&[
            "send",
            "@docs",
            "v2",
            "please",
            "--expect-reply",
            "--reply-to",
            "m_4",
        ]))
        .unwrap();
        assert_eq!(r["to"], "docs");
        assert_eq!(r["text"], "v2 please");
        assert_eq!(r["expect_reply"], true);
        assert_eq!(r["reply_to"], "m_4");
        assert!(r.get("mode").is_none());
        assert!(r.get("why").is_none());
        let w = build(&a(&["send", "docs", "v2", "--reply-to", "m_4", "--why", "the brief says v2"])).unwrap();
        assert_eq!(w["why"], "the brief says v2");
        assert_eq!(w["text"], "v2");
        let q = build(&a(&["send", "docs", "later", "--mode", "queued"])).unwrap();
        assert_eq!(q["mode"], "queued");
        assert!(build(&a(&["send", "docs", "x", "--mode", "soon"])).is_err());
    }

    #[test]
    fn spawn_repeats_constraints() {
        let r = build(&a(&[
            "spawn",
            "fix",
            "--objective",
            "fix it",
            "--constraint",
            "no push",
            "--constraint",
            "tests",
            "--worktree",
        ]))
        .unwrap();
        assert_eq!(r["constraints"], json!(["no push", "tests"]));
        assert_eq!(r["worktree"], true);
        assert!(build(&a(&["spawn", "fix"])).is_err());
    }

    #[test]
    fn requests_parse_in_the_core() {
        for args in [
            vec!["list"],
            vec!["ask", "main", "why?"],
            vec!["wait", "m_3"],
            vec!["status", "blocked", "--note", "need key"],
            vec!["report", "done", "all good", "--decision", "v2"],
            vec!["card", "--for", "m_2", "v1 or v2?"],
            vec!["stop", "x", "no", "longer", "needed"],
            vec!["close", "#3", "handled"],
            vec!["close", "4"],
            vec!["rename", "@a", "b"],
            vec!["restore", "a"],
            vec!["isolate", "a"],
        ] {
            let r = build(&a(&args)).unwrap();
            crate::core::AgentReq::from_json(&r).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
        }
    }

    #[test]
    fn main_controls_parse() {
        let r = build(&a(&["close", "#3", "handled", "by", "docs"])).unwrap();
        assert_eq!(r["card"], 3);
        assert_eq!(r["note"], "handled by docs");
        assert_eq!(build(&a(&["close", "3"])).unwrap()["note"], "");
        assert!(build(&a(&["close", "x"])).is_err());
        assert!(build(&a(&["close"])).is_err());
        let r = build(&a(&["rename", "@old", "new"])).unwrap();
        assert_eq!((r["agent"].as_str(), r["new_name"].as_str()), (Some("old"), Some("new")));
        assert!(build(&a(&["rename", "old"])).is_err());
        assert_eq!(build(&a(&["restore", "@x"])).unwrap()["agent"], "x");
        assert_eq!(build(&a(&["isolate", "x"])).unwrap()["agent"], "x");
        assert!(build(&a(&["isolate"])).is_err());
    }

    #[test]
    fn rendering() {
        let (ok, t) = render(
            "ask",
            &json!({"ok": true, "type": "reply", "from": "main", "message_id": "m_3", "message": "v2", "auto": false}),
        );
        assert!(ok && t.starts_with("reply from main (m_3):\nv2"), "{}", t);
        let (_, t) = render(
            "ask",
            &json!({"ok": true, "type": "reply", "from": "docs", "message_id": "m_9", "asked": "m_8", "message": "v2", "auto": true}),
        );
        assert!(t.starts_with("reply from docs (m_9, answers m_8, automatic: the end of its turn):\nv2"), "{}", t);
        let (ok, t) = render(
            "wait",
            &json!({"ok": false, "error": "timeout", "hint": "end your turn"}),
        );
        assert!(!ok && t.contains("timeout") && t.contains("end your turn"));
    }
}
