//! C6's files: `state.json` (rewritten whole on each change; the TUI reads
//! it by mtime) and `events.jsonl` (appended; the hub turns `stopped` into
//! main's feed line).

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::{json, Map, Value};

use crate::paths::Paths;

/// One agent, as the TUI shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agent {
    /// "Chrome", "Edge", "TextEdit": what it drives now; None when idle
    pub driving: Option<String>,
    /// "amazon.fr", "TextEdit"
    pub place: Option<String>,
    pub since_ms: Option<u64>,
    /// the targets the user took over (C4/C5 `paused`)
    pub paused: Vec<String>,
    pub stopped: bool,
}

impl Agent {
    /// Nothing to show: dropped from the file.
    pub fn idle(&self) -> bool {
        self.driving.is_none() && self.paused.is_empty() && !self.stopped
    }

    fn json(&self) -> Value {
        json!({
            "driving": self.driving,
            "where": self.place,
            "since_ms": self.since_ms,
            "paused": !self.paused.is_empty(),
            "stopped": self.stopped,
        })
    }
}

/// The whole state.json.
pub fn render(agents: &BTreeMap<String, Agent>, browsers: Value, apps: Value) -> Value {
    let a: Map<String, Value> = agents.iter().filter(|(_, a)| !a.idle()).map(|(n, a)| (n.clone(), a.json())).collect();
    json!({"agents": a, "browsers": browsers, "apps": apps})
}

/// Write state.json atomically (0600).
pub fn write(paths: &Paths, v: &Value) -> std::io::Result<()> {
    paths.ensure()?;
    let f = paths.state_file();
    let tmp = f.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap_or_default() + "\n")?;
    crate::paths::private(&tmp, 0o600)?;
    std::fs::rename(&tmp, &f)
}

pub fn read(paths: &Paths) -> Value {
    std::fs::read_to_string(paths.state_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({"agents": {}, "browsers": [], "apps": {}}))
}

/// The agents a new broker starts with: the stopped ones stay stopped
/// (a restart must not hand the wheel back), and the paused stay paused.
pub fn restore(paths: &Paths) -> BTreeMap<String, Agent> {
    let v = read(paths);
    let mut out = BTreeMap::new();
    if let Some(m) = v.get("agents").and_then(Value::as_object) {
        for (name, a) in m {
            let stopped = a.get("stopped").and_then(Value::as_bool).unwrap_or(false);
            if stopped {
                out.insert(name.clone(), Agent { stopped, ..Agent::default() });
            }
        }
    }
    out
}

/// Append one line to events.jsonl: `stopped|paused|resumed`, by
/// `you|cancel_bar|group_closed`.
pub fn event(paths: &Paths, agent: &str, event: &str, by: &str) -> std::io::Result<()> {
    paths.ensure()?;
    let line = json!({"t": crate::now_ms(), "agent": agent, "event": event, "by": by}).to_string() + "\n";
    let f = paths.events_file();
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&f)?;
    crate::paths::private(&f, 0o600)?;
    file.write_all(line.as_bytes())
}

/// The events, oldest first (tests, `status`).
pub fn events(paths: &Paths) -> Vec<Value> {
    std::fs::read_to_string(paths.events_file())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_restores_and_appends() {
        let d = std::env::temp_dir().join(format!("cu-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = Paths::new(d.join("run"), d.join("bise"), d.join("home"));
        let mut agents = BTreeMap::new();
        agents.insert("idle".to_string(), Agent::default());
        agents.insert(
            "api-v2".to_string(),
            Agent { driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5), ..Agent::default() },
        );
        agents.insert("held".to_string(), Agent { stopped: true, ..Agent::default() });
        let v = render(&agents, json!([]), json!({"helper": "absent"}));
        write(&p, &v).unwrap();
        let back = read(&p);
        assert_eq!(back["agents"]["api-v2"], json!({"driving": "Chrome", "where": "amazon.fr", "since_ms": 5, "paused": false, "stopped": false}));
        assert!(back["agents"].get("idle").is_none());
        let r = restore(&p);
        assert_eq!(r.keys().collect::<Vec<_>>(), ["held"]);
        event(&p, "api-v2", "stopped", "you").unwrap();
        event(&p, "api-v2", "resumed", "you").unwrap();
        let ev = events(&p);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0]["event"], "stopped");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(p.state_file()).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }
}
