//! `bise plugins import-mcp NAME [--dry-run] < servers.json` (BISE-273):
//! the MCP servers of another agent (Claude Code's `mcpServers`, Codex's
//! `mcp_servers` as JSON) become one Agent Plugin in the user root,
//! `~/.agents/plugins/NAME/` (plugin.json + mcp.json, 0600: the servers'
//! env may hold tokens). Read on stdin so the tokens go through a pipe,
//! never through an agent's screen; the output names servers, never
//! their env values.
//!
//! Only stdio servers load in bise today: an http/sse one is skipped
//! (said). A command given as an absolute path runs through
//! `sh -c 'exec "$0" "$@"'` (the plugin schema takes a bare executable
//! or a path inside the plugin). Run again, it rewrites its own plugin
//! (marked in plugin.json's description); another plugin of that name is
//! never touched.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::resolve::{valid_name, MCP_SCHEMA, PLUGIN_SCHEMA};

/// The mark in plugin.json's description: the folder is import-mcp's.
pub const MARK: &str = "Imported by bise plugins import-mcp";

/// What the import makes of the input: the servers kept (the mcp.json
/// value) and one line per server (kept or skipped, why).
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub servers: Map<String, Value>,
    pub lines: Vec<String>,
}

/// Read `{"mcpServers": {...}}` or `{"mcp_servers": {...}}` or the bare
/// server map.
pub fn plan(input: &Value) -> Result<Plan, String> {
    let map = input
        .get("mcpServers")
        .or_else(|| input.get("mcp_servers"))
        .unwrap_or(input)
        .as_object()
        .ok_or("expected a JSON object of MCP servers ({\"mcpServers\": {...}})")?;
    let mut servers = Map::new();
    let mut lines = Vec::new();
    for (id, v) in map {
        match server(v) {
            Ok((s, notes)) => {
                let extra = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
                lines.push(format!("+ {}{}", id, extra));
                servers.insert(id.clone(), s);
            }
            Err(why) => lines.push(format!("- {}: skipped, {}", id, why)),
        }
    }
    Ok(Plan { servers, lines })
}

/// One server in the plugin schema's stdio shape; Err = why it can't be.
fn server(v: &Value) -> Result<(Value, Vec<String>), String> {
    let o = v.as_object().ok_or("not an object")?;
    let ty = o.get("type").and_then(Value::as_str).unwrap_or("stdio");
    if o.get("enabled").and_then(Value::as_bool) == Some(false) || o.get("disabled").and_then(Value::as_bool) == Some(true) {
        return Err("disabled there".into());
    }
    let Some(cmd) = o.get("command").and_then(Value::as_str).filter(|c| !c.is_empty()) else {
        return Err(match o.get("url") {
            Some(_) => format!("a remote ({}) server: bise runs stdio servers only for now", if ty == "stdio" { "http" } else { ty }),
            None => "no command".into(),
        });
    };
    if ty != "stdio" {
        return Err(format!("type {:?}: bise runs stdio servers only for now", ty));
    }
    let mut args: Vec<Value> = Vec::new();
    match o.get("args") {
        None | Some(Value::Null) => {}
        Some(Value::Array(a)) if a.iter().all(Value::is_string) => args = a.clone(),
        Some(_) => return Err("args is not a list of strings".into()),
    }
    let mut notes = Vec::new();
    let command = if cmd.starts_with('/') {
        notes.push("absolute command, run through sh".to_string());
        let mut a = vec![json!("-c"), json!("exec \"$0\" \"$@\""), json!(cmd)];
        a.append(&mut args);
        args = a;
        "sh".to_string()
    } else if cmd.contains('/') {
        return Err(format!("command {:?} is a relative path: give its absolute path", cmd));
    } else {
        cmd.to_string()
    };
    let mut out = Map::new();
    out.insert("type".into(), json!("stdio"));
    out.insert("command".into(), json!(command));
    if !args.is_empty() {
        out.insert("args".into(), Value::Array(args));
    }
    if let Some(env) = o.get("env").and_then(Value::as_object).filter(|e| !e.is_empty()) {
        let mut e = Map::new();
        for (k, v) in env {
            match v {
                Value::String(_) if k != "PLUGIN_ROOT" && k != "PLUGIN_DATA" => {
                    e.insert(k.clone(), v.clone());
                }
                Value::Number(_) | Value::Bool(_) => {
                    e.insert(k.clone(), json!(v.to_string()));
                }
                _ => return Err(format!("env {} is not a string", k)),
            }
        }
        notes.push(format!("{} env var(s)", e.len()));
        out.insert("env".into(), Value::Object(e));
    }
    let known = ["type", "command", "args", "env", "enabled", "disabled"];
    let dropped: Vec<&str> = o.keys().map(String::as_str).filter(|k| !known.contains(k)).collect();
    if !dropped.is_empty() {
        notes.push(format!("ignored: {}", dropped.join(", ")));
    }
    Ok((Value::Object(out), notes))
}

/// Write the plan as plugin `name` under `root` (the user plugins root).
/// Refuses a folder that holds another plugin. Returns the folder.
pub fn write(root: &Path, name: &str, p: &Plan) -> Result<PathBuf, String> {
    let dir = root.join(name);
    let manifest = dir.join("plugin.json");
    if dir.exists() {
        let ours = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("description").and_then(Value::as_str).map(|d| d.starts_with(MARK)))
            .unwrap_or(false);
        if !ours {
            return Err(format!("{} exists and is not an import: pick another name", dir.display()));
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;
    let m = json!({
        "$schema": PLUGIN_SCHEMA,
        "name": name,
        "version": "0.1.0",
        "description": format!("{}: {} MCP server(s).", MARK, p.servers.len()),
    });
    let mcp = json!({ "$schema": MCP_SCHEMA, "mcpServers": Value::Object(p.servers.clone()) });
    write_private(&manifest, &(serde_json::to_string_pretty(&m).unwrap_or_default() + "\n"))?;
    write_private(&dir.join("mcp.json"), &(serde_json::to_string_pretty(&mcp).unwrap_or_default() + "\n"))?;
    Ok(dir)
}

fn write_private(file: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = file.with_extension("tmp-import");
    let res = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        std::fs::rename(&tmp, file)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot write {}: {}", file.display(), e)
    })
}

/// `bise plugins import-mcp NAME [--dry-run]`, the JSON on stdin; the
/// exit code.
pub fn main(args: &[String], root: &Path) -> i32 {
    let usage = "usage: bise plugins import-mcp NAME [--dry-run] < servers.json
  servers.json: {\"mcpServers\": {...}} (Claude Code's shape; Codex's mcp_servers as JSON works too).
  Writes ~/.agents/plugins/NAME/ (plugin.json, mcp.json 0600); stdio servers only.";
    let dry = args.iter().any(|a| a == "--dry-run");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--dry-run").collect();
    let name = match rest.as_slice() {
        [n] if valid_name(n) => n.as_str(),
        [n] if !n.starts_with('-') => {
            eprintln!("{:?} is not a plugin name: [a-z0-9.-], 1-64 chars, alphanumeric at both ends", n);
            return 2;
        }
        _ => {
            eprintln!("{}", usage);
            return 2;
        }
    };
    let mut text = String::new();
    if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
        eprintln!("cannot read stdin: {}", e);
        return 1;
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("stdin is not JSON ({}): nothing written", e.classify_name());
            return 1;
        }
    };
    let p = match plan(&v) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    for l in &p.lines {
        println!("{}", l);
    }
    if p.servers.is_empty() {
        println!("no server bise can run: nothing written");
        return 0;
    }
    if dry {
        println!("dry run: would write {}", root.join(name).display());
        return 0;
    }
    match write(root, name, &p) {
        Ok(dir) => {
            println!("wrote {} ({} server(s)); they start with the next session or /reload", dir.display(), p.servers.len());
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

trait Classify {
    fn classify_name(&self) -> &'static str;
}

impl Classify for serde_json::Error {
    /// The kind of error without the text (it may quote the input).
    fn classify_name(&self) -> &'static str {
        match self.classify() {
            serde_json::error::Category::Io => "io",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "bad data",
            serde_json::error::Category::Eof => "cut short",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, Roots};

    const TOKEN: &str = "ghp_SECRET_123";

    fn input() -> Value {
        json!({ "mcpServers": {
            "github": { "type": "stdio", "command": "npx", "args": ["-y", "@mcp/github"], "env": { "GITHUB_TOKEN": TOKEN } },
            "local": { "command": "/opt/tools/mcp-local", "args": ["--quiet"], "startup_timeout_sec": 20 },
            "linear": { "type": "http", "url": "https://mcp.linear.app/mcp" },
            "codex-remote": { "url": "https://x.test/mcp" },
            "off": { "command": "uvx", "enabled": false },
            "rel": { "command": "./bin/x" }
        }})
    }

    #[test]
    fn stdio_servers_are_kept_the_rest_said_and_no_token_shows() {
        let p = plan(&input()).unwrap();
        let ids: Vec<&str> = p.servers.keys().map(String::as_str).collect();
        assert_eq!(ids, ["github", "local"]);
        assert_eq!(p.servers["local"]["command"], "sh");
        assert_eq!(p.servers["local"]["args"], json!(["-c", "exec \"$0\" \"$@\"", "/opt/tools/mcp-local", "--quiet"]));
        let text = p.lines.join("\n");
        assert!(text.contains("- linear: skipped, a remote (http) server"), "{text}");
        assert!(text.contains("- codex-remote: skipped, a remote (http)"), "{text}");
        assert!(text.contains("- off: skipped, disabled there"), "{text}");
        assert!(text.contains("- rel: skipped"), "{text}");
        assert!(text.contains("ignored: startup_timeout_sec"), "{text}");
        assert!(!text.contains(TOKEN));
        // Codex's key and the bare map work too
        assert_eq!(plan(&json!({ "mcp_servers": { "a": { "command": "x" } } })).unwrap().servers.len(), 1);
        assert_eq!(plan(&json!({ "a": { "command": "x" } })).unwrap().servers.len(), 1);
    }

    #[test]
    fn the_plugin_written_resolves_and_a_rerun_replaces_only_its_own() {
        let d = std::env::temp_dir().join(format!("bise-import-mcp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let root = d.join("plugins");
        let p = plan(&input()).unwrap();
        let dir = write(&root, "from-claude-code", &p).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("mcp.json")).unwrap().permissions().mode() & 0o777, 0o600);
        let roots = Roots { user: Some(root.clone()), workspace: None, data: d.join("data"), disabled: vec![] };
        let r = resolve(&roots);
        assert_eq!(r.plugins.len(), 1, "{:?}", r.diagnostics);
        let ids: Vec<&str> = r.plugins[0].servers.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["github", "local"], "{:?}", r.diagnostics);
        assert!(write(&root, "from-claude-code", &p).is_ok(), "a rerun rewrites its own import");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(root.join("mine/plugin.json"), "{\"name\":\"mine\"}").unwrap();
        assert!(write(&root, "mine", &p).unwrap_err().contains("not an import"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
