//! BISE-197: a `BEND-SESSION 2` file becomes a session log, once,
//! checked: the log projects back to exactly what today's loader keeps
//! of the file (`legacy::to_text(legacy::parse(txt))`), else the new
//! folder is removed and the session stays on its `.txt`. The `.txt` is
//! never changed (the backup).
use crate::legacy::{self, Role};
use crate::project::{image_marker, project};
use crate::reader::read_dir;
use crate::state::State;
use crate::writer::{session_id_at, Writer};
use crate::{blob, types::AgentRef};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Source {
    pub txt: PathBuf,
    pub cwd: String,
    pub agent: Option<AgentRef>,
    /// the model to record (`model_set {source: migration}`)
    pub model: Option<String>,
}

#[derive(Debug)]
pub enum Outcome {
    Migrated { id: String, dir: PathBuf, messages: usize, dropped_lines: usize, images: usize },
    /// today's loader gives nothing back from it (no CFG, unknown version)
    Unloadable,
    Failed(String),
}

fn text(t: &str) -> Value {
    json!({"kind": "text", "text": t})
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A user/tool text as parts: each image marker whose `.b64` file exists
/// becomes an image part (its bytes in the blob store); the rest stays
/// text. Joining the parts gives the text back.
fn plain_parts(t: &str, blobs: &Path, images: &mut usize) -> Vec<Value> {
    const OPEN: &str = "<image name=\"";
    let mut parts = Vec::new();
    let mut rest = t;
    while let Some(i) = rest.find(OPEN) {
        let m = (|| {
            let r = &rest[i + OPEN.len()..];
            let (name, r) = r.split_once("\" path=\"")?;
            let (path, r) = r.split_once("\" mime=\"")?;
            let (mime, r) = r.split_once("\" b64=\"")?;
            let (b64, r) = r.split_once("\">")?;
            if [name, path, mime, b64].iter().any(|v| v.contains(['"', '>', '\n', '\r'])) {
                return None;
            }
            let bytes = b64_decode(&std::fs::read_to_string(b64).ok()?)?;
            let r#ref = blob::put(blobs, &bytes, mime).ok()?;
            debug_assert_eq!(image_marker(name, path, mime, b64).len(), rest.len() - i - r.len());
            Some((json!({"kind": "image", "image": r#ref, "name": name, "path": path, "b64": b64}), rest.len() - r.len()))
        })();
        match m {
            Some((part, end)) => {
                if i > 0 {
                    parts.push(text(&rest[..i]));
                }
                parts.push(part);
                *images += 1;
                rest = &rest[end..];
            }
            None => {
                let end = i + OPEN.len();
                // keep scanning after this opener; the text is kept whole
                parts.push(text(&rest[..end]));
                rest = &rest[end..];
            }
        }
    }
    if !rest.is_empty() || parts.is_empty() {
        parts.push(text(rest));
    }
    merge_texts(parts)
}

fn merge_texts(parts: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for p in parts {
        if let (Some(last), Some(t)) = (out.last_mut(), p.get("text").and_then(Value::as_str)) {
            if last["kind"] == "text" && p["kind"] == "text" {
                let joined = format!("{}{t}", last["text"].as_str().unwrap_or(""));
                *last = text(&joined);
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// An assistant text as parts: `<think>…</think>` spans become thinking
/// parts (the signature split off its `\nBENDSIG::` line), the rest text.
fn assistant_parts(t: &str) -> Vec<Value> {
    let mut parts = Vec::new();
    let mut rest = t;
    while let Some(i) = rest.find("<think>") {
        let Some(j) = rest[i + 7..].find("</think>") else { break };
        if i > 0 {
            parts.push(text(&rest[..i]));
        }
        let inner = &rest[i + 7..i + 7 + j];
        let signed = inner.rfind('\n').and_then(|n| inner[n + 1..].strip_prefix("BENDSIG::").map(|s| (n, s)));
        parts.push(match signed {
            Some((n, sig)) if !sig.is_empty() => json!({"kind": "thinking", "text": &inner[..n], "signature": sig}),
            _ => json!({"kind": "thinking", "text": inner}),
        });
        rest = &rest[i + 7 + j + 8..];
    }
    if !rest.is_empty() || parts.is_empty() {
        parts.push(text(rest));
    }
    parts
}

/// The attributes of an `<agent_message …>` opening tag.
fn agent_attrs(t: &str) -> Option<serde_json::Map<String, Value>> {
    let tag = &t.strip_prefix("<agent_message ")?[..t.find('>')? - "<agent_message ".len()];
    let mut m = serde_json::Map::new();
    let mut r = tag;
    while let Some(eq) = r.find("=\"") {
        let key = r[..eq].trim().to_string();
        let v = &r[eq + 2..];
        let end = v.find('"')?;
        m.insert(key, Value::String(v[..end].to_string()));
        r = &v[end + 1..];
    }
    Some(m)
}

fn injected_kind(t: &str) -> &'static str {
    if t.starts_with("# Your role") {
        "preamble"
    } else if t.starts_with("<switchboard_state>") {
        "hub_state"
    } else if t.starts_with("Summary of the earlier conversation") || t.starts_with("The earlier conversation") {
        "summary"
    } else if t.starts_with("[switchboard]") {
        "resume_note"
    } else {
        "notification"
    }
}

/// "tool bash failed: …" → false
fn tool_ok(t: &str) -> bool {
    let mut w = t.splitn(4, ' ');
    !(w.next() == Some("tool") && w.next().is_some() && w.next().is_some_and(|s| s.starts_with("failed")))
}

/// Convert one file into `sessions/<id>/`. `writer` names this bise.
pub fn migrate_txt(src: &Source, sessions: &Path, blobs: &Path, writer: &str) -> Outcome {
    let bytes = match std::fs::read(&src.txt) {
        Ok(b) => b,
        Err(e) => return Outcome::Failed(format!("{}: {e}", src.txt.display())),
    };
    let Ok(txt) = String::from_utf8(bytes.clone()) else {
        return Outcome::Failed("not UTF-8".into());
    };
    let Some(sess) = legacy::parse(&txt) else { return Outcome::Unloadable };
    let mtime = std::fs::metadata(&src.txt).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::now());
    let id = session_id_at(mtime);
    let dir = sessions.join(&id);
    let r = write(src, &sess, &id, &dir, blobs, writer, &bytes).and_then(|images| {
        let log = read_dir(&dir).map_err(|e| e.to_string())?;
        let st = State::rebuild(&log);
        let got = project(&log, &st, blobs)?;
        let want = legacy::to_text(&sess);
        if got != want {
            let at = got.bytes().zip(want.bytes()).take_while(|(a, b)| a == b).count();
            return Err(format!("the log does not project back to the file (first difference at byte {at})"));
        }
        Ok(images)
    });
    match r {
        Ok(images) => Outcome::Migrated { id, dir, messages: sess.msgs.len(), dropped_lines: sess.dropped_lines, images },
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Outcome::Failed(e)
        }
    }
}

fn write(src: &Source, s: &legacy::Session, id: &str, dir: &Path, blobs: &Path, writer: &str, bytes: &[u8]) -> Result<usize, String> {
    let mut start = json!({"session": id, "format": 1, "created_by": writer, "cwd": src.cwd,
        "migrated_from": {"path": src.txt.to_string_lossy(), "format": "BEND-SESSION 2",
                          "sha256": blob::sha256_hex(bytes), "dropped_lines": s.dropped_lines}});
    if let Some(a) = &src.agent {
        start["agent"] = json!(a);
    }
    let mut w = Writer::create(dir, blobs, start, writer).map_err(|e| e.to_string())?;
    let ap = |w: &mut Writer, t: &str, d: Value| w.append(t, d, None).map_err(|e| e.to_string());
    let tools: Vec<Value> = s.cfg.tools.iter().map(|(n, d)| json!({"name": n, "description": d})).collect();
    ap(&mut w, "context_set", json!({"system": {"text": s.cfg.system}, "tools": tools}))?;
    let limits = json!({"compact_threshold": s.cfg.threshold, "select_budget": s.cfg.select, "max_nulls": s.cfg.max_nulls});
    ap(&mut w, "limits_set", limits.clone())?;
    let model = src.model.clone().unwrap_or_else(|| "unknown".into());
    ap(&mut w, "model_set", json!({"model": model, "source": "migration"}))?;
    let mut queue = Vec::new();
    let mut images = 0;
    for q in &s.queued {
        queue.push(ap(&mut w, "input_queued", json!({"kind": "user", "content": plain_parts(q, blobs, &mut images)}))?);
    }
    for q in &s.notifs {
        queue.push(ap(&mut w, "input_queued", json!({"kind": "notification", "content": plain_parts(q, blobs, &mut images)}))?);
    }
    let mut context = Vec::new();
    let mut calls: Vec<String> = Vec::new();
    let mut next_call = 0;
    let mut req = 0;
    for m in &s.msgs {
        let (typ, data) = match m.role {
            Role::Assistant => {
                req += 1;
                calls = m.calls.iter().map(|c| format!("call_{}", c.id)).collect();
                next_call = 0;
                let cs: Vec<Value> = m.calls.iter().map(|c| json!({"id": format!("call_{}", c.id), "name": c.name, "args": c.args})).collect();
                ("assistant_message", json!({"req": req, "model": model, "parts": assistant_parts(&m.text), "calls": cs}))
            }
            Role::Tool => {
                let call = calls.get(next_call).cloned().unwrap_or_default();
                next_call += 1;
                ("tool_result", json!({"call": call, "ok": tool_ok(&m.text), "content": plain_parts(&m.text, blobs, &mut images)}))
            }
            Role::System => ("context_injected", json!({"kind": "other", "role": "system", "injected": false, "content": plain_parts(&m.text, blobs, &mut images)})),
            Role::User => {
                let content = plain_parts(&m.text, blobs, &mut images);
                match agent_attrs(&m.text) {
                    Some(a) => {
                        let mut d = json!({"hub_msg": a.get("id").cloned().unwrap_or(json!("")),
                            "from": a.get("from").cloned().unwrap_or(json!("")),
                            "relation": a.get("relation").cloned().unwrap_or(json!("other")),
                            "expects_reply": a.get("expects_reply").and_then(Value::as_str) == Some("true"),
                            "content": content});
                        for k in ["thread", "reply_to"] {
                            if let Some(v) = a.get(k) {
                                d[k] = v.clone();
                            }
                        }
                        if !m.injected {
                            d["injected"] = json!(false);
                        }
                        ("agent_message", d)
                    }
                    None if m.injected => ("context_injected", json!({"kind": injected_kind(&m.text), "content": content})),
                    None => ("user_message", json!({"content": content, "delivery": "prompt"})),
                }
            }
        };
        context.push(ap(&mut w, typ, data)?);
    }
    let upto = w.last_seq();
    let cp = json!({"upto": upto, "context": context, "system": {"text": s.cfg.system}, "tools": tools,
        "model": {"model": model}, "limits": limits, "queue": queue,
        "counters": {"req": req, "turn": 0, "compaction": 0, "inputs": s.inputs, "actions": s.actions},
        "usage_total": {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0}});
    ap(&mut w, "checkpoint", cp)?;
    Ok(images)
}
