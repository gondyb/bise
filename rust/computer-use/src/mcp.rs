//! `bise computer-use mcp`: the stdio MCP server of the built-in plugin
//! `computer` (one per agent session). It publishes C1's tools and
//! forwards each call to the broker (C3), starting the broker when none
//! runs.
//!
//! A tool result is one text block: the C1 result as JSON. A C1 error is
//! `isError: true` with the text `{"error": {code, message, candidates?,
//! summary?}}`, so a program that gets the text back can tell them apart.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::client::{Conn, Starter};
use crate::paths::Paths;
use crate::proto::{err, TOOLS};

/// The tools and their descriptions (cu-sdk owns the texts).
pub const TOOLS_JSON: &str = include_str!("tools.json");

pub fn tools() -> Value {
    serde_json::from_str(TOOLS_JSON).unwrap_or_else(|_| json!([]))
}

/// Who this session is (C3 hello).
#[derive(Clone, Debug)]
pub struct Me {
    pub agent: String,
    pub session: String,
    pub tmpdir: PathBuf,
}

impl Me {
    /// `SB_AGENT`; a `bise --headless` session: its session name.
    pub fn from_env() -> Me {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let session = get("BEND_SESSION_FILE")
            .and_then(|f| PathBuf::from(f).file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| format!("pid-{}", std::process::id()));
        let agent = get("SB_AGENT").unwrap_or_else(|| session.clone());
        let tmpdir = get("TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        Me { agent, session, tmpdir }
    }

    fn hello(&self) -> Value {
        json!({"op": "hello", "agent": self.agent, "session": self.session, "tmpdir": self.tmpdir})
    }
}

/// Reads change nothing: safe to send again after the broker restarted.
fn idempotent(op: &str) -> bool {
    matches!(op, "status" | "tabs" | "apps" | "snapshot" | "screenshot")
}

pub struct Server {
    paths: Paths,
    start: Arc<Starter>,
    me: Me,
    conn: Option<Conn>,
}

impl Server {
    pub fn new(paths: Paths, start: Arc<Starter>, me: Me) -> Server {
        Server { paths, start, me, conn: None }
    }

    fn conn(&mut self) -> std::io::Result<&mut Conn> {
        if self.conn.is_none() {
            self.conn = Some(Conn::open(&self.paths, Some(&*self.start), &self.me.hello())?);
        }
        Ok(self.conn.as_mut().expect("just set"))
    }

    /// One C1 call through the broker.
    pub fn call(&mut self, op: &str, args: &Value) -> Result<Value, Value> {
        let down = |e: std::io::Error| err("no_browser", format!("the computer-use broker can't start ({}); ask the user to run /computer-use", e));
        for attempt in 0..2 {
            let r = match self.conn() {
                Ok(c) => c.call(op, args),
                Err(e) => return Err(down(e)),
            };
            match r {
                Ok(r) => return r,
                Err(_) => {
                    self.conn = None;
                    if attempt == 1 || !idempotent(op) {
                        return Err(err(
                            "timeout",
                            "the computer-use broker restarted during this action; check the page with snapshot, then try again",
                        ));
                    }
                }
            }
        }
        unreachable!()
    }

    /// One JSON-RPC message; the reply, None for a notification.
    pub fn message(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned()?;
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(crate::mcp::PROTOCOL),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "bise-computer-use", "version": env!("CARGO_PKG_VERSION")},
            }),
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                let r = if TOOLS.contains(&name) {
                    self.call(name, &args)
                } else {
                    Err(err("bad_args", format!("unknown tool {:?}", name)))
                };
                match r {
                    Ok(v) => json!({"content": [{"type": "text", "text": v.to_string()}], "isError": false}),
                    Err(e) => json!({"content": [{"type": "text", "text": json!({"error": e}).to_string()}], "isError": true}),
                }
            }
            _ => return Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("no method {}", method)}})),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }
}

pub const PROTOCOL: &str = "2025-06-18";

/// Serve MCP on `input`/`output` until `input` ends.
pub fn run(mut server: Server, input: impl BufRead, mut output: impl Write) -> i32 {
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.message(&msg),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
        };
        if let Some(r) = reply {
            let mut s = r.to_string();
            s.push('\n');
            if output.write_all(s.as_bytes()).and_then(|_| output.flush()).is_err() {
                break;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_json_lists_c1() {
        let t = tools();
        let names: Vec<&str> = t.as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
        assert_eq!(names, TOOLS);
        for x in t.as_array().unwrap() {
            assert!(!x["description"].as_str().unwrap().is_empty());
            assert_eq!(x["inputSchema"]["type"], "object");
        }
    }
}
