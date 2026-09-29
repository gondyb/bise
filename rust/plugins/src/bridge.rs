//! The per-session bridge (`bise plugins serve`): starts the
//! stdio MCP servers of the enabled plugins, writes the index files the
//! Bend REPL reads, then serves each server over loopback HTTP (the
//! JSON-response subset of MCP Streamable HTTP) until the REPL is gone.
//! Design: projects/switchboard/docs/plugins.md.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::report::{self, ServerStatus};
use crate::resolve::{self, Diagnostic, Plugin, Resolution, Severity, StdioServer};
use crate::stdio::Client;

pub const START_TIMEOUT: Duration = Duration::from_secs(10);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Opts {
    /// the session's plugin dir (index files, logs, ready marker)
    pub dir: PathBuf,
    /// exit when this pid is gone (the REPL)
    pub parent: Option<u32>,
    pub roots: resolve::Roots,
}

/// One stdio server of one plugin, as the bridge holds it.
struct Entry {
    spec: StdioServer,
    plugin_root: PathBuf,
    data_root: PathBuf,
    log: PathBuf,
    client: Mutex<Option<Client>>,
    /// published tool name -> the server's own name
    names: HashMap<String, String>,
}

impl Entry {
    fn start(&self) -> Result<Client, String> {
        Client::start(&self.spec, &self.plugin_root, &self.data_root, &self.log, START_TIMEOUT)
    }

    /// Forward one JSON-RPC request; a dead server is restarted once.
    fn forward(&self, method: &str, mut params: Value) -> Result<Value, String> {
        if method == "tools/call" {
            let published = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(src) = self.names.get(&published) {
                params["name"] = json!(src);
            }
        }
        let mut guard = self.client.lock().unwrap_or_else(|e| e.into_inner());
        let dead = match guard.as_mut() {
            Some(c) => !c.alive(),
            None => true,
        };
        if dead {
            *guard = Some(self.start()?);
        }
        let c = guard.as_ref().ok_or("no server")?;
        c.request_raw(method, params, CALL_TIMEOUT)
    }
}

pub struct Session {
    pub index: String,
    pub skills: String,
    pub report: String,
}

fn token() -> String {
    let mut b = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_ok();
    if !ok {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            ^ (std::process::id() as u128) << 64;
        b = t.to_le_bytes();
    }
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// Descriptions and schemas ride on one index line.
fn flat(s: &str) -> String {
    s.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

/// `name\tdescription\tpath`, tabs and quotes stripped (the scan's rule)
pub fn skill_line(name: &str, desc: &str, path: &Path) -> String {
    let clean = |s: &str| -> String { flat(s).chars().filter(|c| *c != '\t' && *c != '"').collect() };
    format!("{}\t{}\t{}\n", clean(name), clean(desc), path.display())
}

/// The connector index format: `<cid> <group> <tool> : #<desc> | input: <schema>`
pub fn index_line(cid: &str, group: &str, tool: &str, desc: &str, schema: Option<&Value>) -> String {
    let schema = schema.map(|s| format!(" | input: {}", flat(&s.to_string()))).unwrap_or_default();
    format!("{} {} {} : #{}{}\n", cid, group, tool, flat(desc), schema)
}

struct Started {
    plugin: usize,
    server: StdioServer,
    result: Result<(Client, Vec<Value>), String>,
}

fn start_all(res: &Resolution, dir: &Path) -> Vec<Started> {
    let mut handles = Vec::new();
    for (i, p) in res.plugins.iter().enumerate() {
        if p.state != resolve::State::Loaded {
            continue;
        }
        for s in &p.servers {
            let (s, root, data) = (s.clone(), p.root.clone(), p.data_root.clone());
            let log = dir.join(format!("{}.{}.log", p.name, s.id));
            handles.push(std::thread::spawn(move || {
                let result = Client::start(&s, &root, &data, &log, START_TIMEOUT).and_then(|c| {
                    match c.list_tools(START_TIMEOUT) {
                        Ok(ts) => Ok((c, ts)),
                        Err(e) => Err(format!("tools/list: {}", e)),
                    }
                });
                let result = result.map_err(|e| {
                    let tail = std::fs::read_to_string(&log).unwrap_or_default();
                    let tail: Vec<&str> = tail.lines().rev().take(3).collect();
                    if tail.is_empty() {
                        e
                    } else {
                        let t: Vec<&str> = tail.into_iter().rev().collect();
                        format!("{} (stderr: {})", e, t.join(" | "))
                    }
                });
                Started { plugin: i, server: s, result }
            }));
        }
    }
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// Start the servers and build the index files. The entries are keyed
/// by `<plugin>/<server>`.
fn build(
    res: &mut Resolution,
    dir: &Path,
    base: &str,
) -> (HashMap<String, Arc<Entry>>, Session) {
    let started = start_all(res, dir);
    let mut entries = HashMap::new();
    let mut index = String::new();
    let mut status: Vec<ServerStatus> = Vec::new();
    // published names per plugin, across its servers
    let mut taken: HashMap<usize, Vec<String>> = HashMap::new();
    let mut extra: Vec<Diagnostic> = Vec::new();
    for st in started {
        let p: &Plugin = &res.plugins[st.plugin];
        let (client, tools) = match st.result {
            Ok(ct) => ct,
            Err(e) => {
                extra.push(Diagnostic {
                    code: "plugin.mcp.connection_failed",
                    severity: Severity::Warning,
                    plugin: p.name.clone(),
                    message: format!("MCP server {:?}: {}", st.server.id, e),
                });
                status.push(ServerStatus { plugin: p.name.clone(), server: st.server.id.clone(), tools: Err(e) });
                continue;
            }
        };
        let cid = format!("{}/{}/{}", base, p.name, st.server.id);
        let mut names = HashMap::new();
        let mut count = 0;
        let used = taken.entry(st.plugin).or_default();
        for t in &tools {
            let Some(src) = t.get("name").and_then(Value::as_str) else { continue };
            let published = resolve::identifier(src);
            if used.contains(&published) {
                extra.push(Diagnostic {
                    code: "plugin.tool.name_collision",
                    severity: Severity::Warning,
                    plugin: p.name.clone(),
                    message: format!("tool {}.{} (server {:?}) is already taken; dropped", p.namespace, published, st.server.id),
                });
                continue;
            }
            used.push(published.clone());
            let desc = t.get("description").and_then(Value::as_str).unwrap_or("");
            index.push_str(&index_line(&cid, &p.namespace, &published, desc, t.get("inputSchema")));
            names.insert(published, src.to_string());
            count += 1;
        }
        status.push(ServerStatus { plugin: p.name.clone(), server: st.server.id.clone(), tools: Ok(count) });
        entries.insert(
            format!("{}/{}", p.name, st.server.id),
            Arc::new(Entry {
                log: dir.join(format!("{}.{}.log", p.name, st.server.id)),
                spec: st.server,
                plugin_root: p.root.clone(),
                data_root: p.data_root.clone(),
                client: Mutex::new(Some(client)),
                names,
            }),
        );
    }
    res.diagnostics.extend(extra);
    let mut skills = String::new();
    for p in res.loaded() {
        for s in &p.skills {
            skills.push_str(&skill_line(&s.name, &s.description, &s.path));
        }
    }
    status.sort_by(|a, b| (&a.plugin, &a.server).cmp(&(&b.plugin, &b.server)));
    let report = report::text(res, Some(&status));
    (entries, Session { index, skills, report })
}

// ---- HTTP ----

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
    close: bool,
}

fn read_request(r: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    if r.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut len = 0usize;
    let mut close = false;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            if k == "content-length" {
                len = v.trim().parse().ok()?;
            } else if k == "connection" && v.trim().eq_ignore_ascii_case("close") {
                close = true;
            }
        }
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    Some(Request { method, path, body, close })
}

fn respond(w: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    let ctype = if body.is_empty() { "" } else { "Content-Type: application/json\r\n" };
    write!(
        w,
        "HTTP/1.1 {}\r\n{}Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
        status,
        ctype,
        body.len(),
        body
    )?;
    w.flush()
}

fn rpc_error(id: &Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// One JSON-RPC message for one server: (HTTP status, body).
fn handle_rpc(entry: &Entry, body: &[u8]) -> (&'static str, String) {
    let Ok(msg) = serde_json::from_slice::<Value>(body) else {
        return ("400 Bad Request", rpc_error(&Value::Null, -32700, "parse error"));
    };
    let Some(id) = msg.get("id").cloned() else {
        // a notification: accepted, nothing to answer
        return ("202 Accepted", String::new());
    };
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    if method == "initialize" {
        let init = entry
            .client
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|c| c.init.clone())
            .unwrap_or_else(|| json!({"protocolVersion": crate::stdio::PROTOCOL, "capabilities": {"tools": {}},
                                     "serverInfo": {"name": "bend-plugin-bridge", "version": "1"}}));
        return ("200 OK", json!({"jsonrpc": "2.0", "id": id, "result": init}).to_string());
    }
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    match entry.forward(method, params) {
        Ok(answer) => ("200 OK", stamp(answer, id)),
        Err(e) => ("200 OK", rpc_error(&id, -32000, &e)),
    }
}

/// The server's answer with the client's id: a JSON object gets `id`
/// and `jsonrpc`; anything else becomes an error answer (indexing a
/// non-object `Value` panics).
fn stamp(answer: Value, id: Value) -> String {
    match answer {
        Value::Object(mut o) => {
            o.insert("id".into(), id);
            o.insert("jsonrpc".into(), json!("2.0"));
            Value::Object(o).to_string()
        }
        other => rpc_error(&id, -32000, &format!("the plugin server answered a non-object: {}", other)),
    }
}

fn connection(stream: TcpStream, token: String, entries: Arc<HashMap<String, Arc<Entry>>>) {
    let Ok(mut w) = stream.try_clone() else { return };
    let mut r = BufReader::new(stream);
    while let Some(req) = read_request(&mut r) {
        let prefix = format!("/{}/", token);
        let key = req.path.strip_prefix(&prefix).map(|k| k.trim_end_matches('/').to_string());
        let out = match (req.method.as_str(), key.and_then(|k| entries.get(&k).cloned())) {
            ("POST", Some(e)) => handle_rpc(&e, &req.body),
            (_, Some(_)) => ("405 Method Not Allowed", String::new()),
            (_, None) => ("404 Not Found", String::new()),
        };
        if respond(&mut w, out.0, &out.1).is_err() || req.close {
            break;
        }
    }
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Run the bridge. Returns when the parent is gone, or at once when
/// no server is up (the index files are written either way).
pub fn serve(opts: Opts) -> std::io::Result<()> {
    std::fs::create_dir_all(&opts.dir)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = token();
    let base = format!("http://127.0.0.1:{}/{}", port, token);
    let mut res = resolve::resolve(&opts.roots);
    let (entries, session) = build(&mut res, &opts.dir, &base);
    write_atomic(&opts.dir.join("mcp-index.txt"), &session.index)?;
    write_atomic(&opts.dir.join("skills-index.txt"), &session.skills)?;
    write_atomic(&opts.dir.join("report.txt"), &session.report)?;
    write_atomic(&opts.dir.join("ready"), &format!("{}\n", std::process::id()))?;
    let entries = Arc::new(entries);
    // nothing to serve: the files say so, no process lingers
    if entries.is_empty() {
        return Ok(());
    }
    {
        let entries = entries.clone();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let (t, e) = (token.clone(), entries.clone());
                std::thread::spawn(move || connection(s, t, e));
            }
        });
    }
    match opts.parent {
        Some(pid) => {
            while alive(pid) {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        None => loop {
            std::thread::sleep(Duration::from_secs(3600));
        },
    }
    for e in entries.values() {
        if let Some(mut c) = e.client.lock().unwrap_or_else(|e| e.into_inner()).take() {
            c.stop();
        }
    }
    Ok(())
}

#[cfg(test)]
mod stamp_tests {
    use super::*;

    #[test]
    fn a_non_object_answer_is_an_error_not_a_panic() {
        let ok: Value = serde_json::from_str(&stamp(json!({"result": 1}), json!(7))).unwrap_or_default();
        assert_eq!((ok["id"].clone(), ok["result"].clone()), (json!(7), json!(1)));
        for bad in [json!([1, 2]), json!(3), json!("s"), Value::Null] {
            let e: Value = serde_json::from_str(&stamp(bad, json!(8))).unwrap_or_default();
            assert_eq!(e["id"], json!(8));
            assert!(e["error"].is_object(), "{e}");
        }
    }
}
