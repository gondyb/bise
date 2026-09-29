//! The hub's side of an agent's session (BISE-196): the REPL's `ev:`
//! lines (BISE-195, core/ev.bend) become log events. The writer rules
//! agreed with the REPL: seq, at and turn stamped here (turn_started ..
//! turn_ended inclusive); `from_queue: 0` = the oldest queued input of
//! the same kind with equal content; compaction `replaces` / `kept` are
//! 1-based positions in the current context list; an assistant `model`
//! "" is the current model; a config fact equal to the state is skipped.
//! A checkpoint follows each compaction; rotation happens at a turn end.
use crate::project::{materialize_images, project};
use crate::reader::{read_dir, Log};
use crate::resume::resume;
use crate::state::State;
use crate::types::Payload;
use crate::writer::{OpenError, Writer};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Recorder {
    w: Writer,
    st: State,
    session: String,
    in_turn: bool,
    /// queued inputs not consumed: seq → (kind, content)
    queued: BTreeMap<u64, (String, Value)>,
    /// the repair a resume appended (§7 step 5)
    pub repaired: Vec<u64>,
}

fn session_of(log: &Log) -> String {
    log.events
        .first()
        .and_then(|e| e.data.get("session"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn queued_of(log: &Log, st: &State) -> BTreeMap<u64, (String, Value)> {
    st.queue
        .iter()
        .filter_map(|s| {
            let e = log.by_seq(*s)?;
            Some((*s, (e.data.get("kind")?.as_str()?.to_string(), e.data.get("content")?.clone())))
        })
        .collect()
}

impl Recorder {
    /// A new session folder.
    pub fn create(dir: &Path, blobs: &Path, start: Value, writer: &str) -> Result<Recorder, OpenError> {
        let session = start.get("session").and_then(Value::as_str).unwrap_or("").to_string();
        let w = Writer::create(dir, blobs, start, writer)?;
        Ok(Recorder { w, st: State::default(), session, in_turn: false, queued: BTreeMap::new(), repaired: Vec::new() })
    }

    /// An existing session, for a fresh REPL: resumed (§7: the crash
    /// repair, process_opened).
    pub fn resume(dir: &Path, blobs: &Path, writer: &str) -> Result<Recorder, OpenError> {
        let r = resume(dir, blobs, writer)?;
        let queued = queued_of(&r.log, &r.state);
        Ok(Recorder { session: session_of(&r.log), queued, st: r.state, w: r.writer, in_turn: false, repaired: r.repaired })
    }

    /// An existing session whose REPL still runs (a hub restarted and
    /// adopted it): no repair, the open turn goes on.
    pub fn attach(dir: &Path, blobs: &Path) -> Result<Recorder, OpenError> {
        let (w, log) = Writer::open(dir, blobs)?;
        let st = State::rebuild(&log);
        let queued = queued_of(&log, &st);
        let in_turn = st.open_turn.is_some();
        Ok(Recorder { session: session_of(&log), queued, st, w, in_turn, repaired: Vec::new() })
    }

    pub fn state(&self) -> &State {
        &self.st
    }
    pub fn dir(&self) -> &Path {
        self.w.dir()
    }
    pub fn set_redactor(&mut self, r: crate::redact::Redactor) {
        self.w.redactor = Some(r);
    }

    /// The BEND-SESSION 2 text a fresh REPL loads, written to `path`
    /// (tmp + rename, 0600), with the images' base64 files in place.
    pub fn project_to(&self, path: &Path) -> Result<(), String> {
        let log = read_dir(self.w.dir()).map_err(|e| e.to_string())?;
        let text = project(&log, &self.st, self.w.blobs())?;
        for e in materialize_images(&log, &self.st, self.w.blobs()) {
            eprintln!("session {}: image: {e}", self.session);
        }
        write_private(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether the log holds anything to resume (config or context).
    pub fn has_context(&self) -> bool {
        !self.st.context.is_empty() || !self.st.tools.is_empty()
    }

    /// One `ev:` line's JSON (the text after "ev: "). Err: not an event
    /// (logged by the caller, never fatal).
    pub fn on_ev(&mut self, json_text: &str) -> Result<Option<u64>, String> {
        let v: Value = serde_json::from_str(json_text).map_err(|e| format!("ev: not JSON: {e}"))?;
        let typ = v.get("type").and_then(Value::as_str).ok_or("ev: no type")?.to_string();
        let mut data = v.get("data").cloned().unwrap_or(json!({}));
        if !self.resolve(&typ, &mut data)? {
            return Ok(None);
        }
        let turn = match typ.as_str() {
            "turn_started" => {
                self.in_turn = true;
                Some(self.st.counters.turn + 1)
            }
            _ if self.in_turn => Some(self.st.counters.turn.max(1)),
            _ => None,
        };
        if typ == "turn_ended" {
            self.in_turn = false;
        }
        // the payload as written (redaction and blobs applied)
        let (seq, data) = self.w.append_data(&typ, data, turn).map_err(|e| e.to_string())?;
        let payload = Payload::parse(&typ, 1, &data).and_then(Result::ok);
        if let Some(Payload::InputQueued(q)) = &payload {
            let kind = serde_json::to_value(q.kind).ok().and_then(|k| k.as_str().map(String::from)).unwrap_or_default();
            self.queued.insert(seq, (kind, serde_json::to_value(&q.content).unwrap_or(Value::Null)));
        }
        self.st.apply(seq, turn, payload.as_ref());
        self.queued.retain(|s, _| self.st.queue.contains(s));
        if typ == "compaction_done" {
            let cp = self.st.checkpoint(seq);
            let s2 = self.w.append("checkpoint", cp, None).map_err(|e| e.to_string())?;
            self.st.apply(s2, None, None);
        }
        if typ == "turn_ended" && self.w.wants_rotation() {
            let cp = self.st.checkpoint(self.w.last_seq());
            self.w.rotate(&self.session.clone(), cp).map_err(|e| e.to_string())?;
        }
        Ok(Some(seq))
    }

    /// The writer rules; false: skip this fact (equal config).
    fn resolve(&mut self, typ: &str, data: &mut Value) -> Result<bool, String> {
        match typ {
            "context_set" => {
                let sys_same = data.get("system").is_none_or(|s| serde_json::to_value(&self.st.system).ok().as_ref() == Some(s));
                let tools_same = data.get("tools").is_none_or(|t| serde_json::to_value(&self.st.tools).ok().as_ref() == Some(t));
                if sys_same && tools_same && !self.st.tools.is_empty() {
                    return Ok(false);
                }
            }
            "limits_set" => {
                if serde_json::to_value(&self.st.limits).ok().as_ref() == Some(data) {
                    return Ok(false);
                }
            }
            "model_set" => {
                let m = &self.st.model;
                let same = data.get("model").and_then(Value::as_str) == Some(m.model.as_str())
                    && data.get("effort").and_then(Value::as_str) == m.effort.as_deref()
                    && data.get("provider").and_then(Value::as_str) == m.provider.as_deref();
                if same {
                    return Ok(false);
                }
            }
            "assistant_message" => {
                if data.get("model").and_then(Value::as_str) == Some("") {
                    data["model"] = json!(self.st.model.model);
                }
            }
            "compaction_done" => {
                let ctx = &self.st.context;
                let at = |p: &Value| -> Result<u64, String> {
                    let i = p.as_u64().ok_or("compaction: a position is not a number")? as usize;
                    ctx.get(i.wrapping_sub(1)).copied().ok_or(format!("compaction: position {i} of {}", ctx.len()))
                };
                let from = at(&data["replaces"]["from"])?;
                let to = at(&data["replaces"]["to"])?;
                let kept: Result<Vec<u64>, String> = data["kept"].as_array().cloned().unwrap_or_default().iter().map(at).collect();
                data["replaces"] = json!({"from": from, "to": to});
                data["kept"] = json!(kept?);
            }
            _ => {}
        }
        if data.get("from_queue").and_then(Value::as_u64) == Some(0) {
            let kind = match typ {
                "user_message" => "user",
                "agent_message" => "agent_message",
                _ => "notification",
            };
            let content = data.get("content").cloned().unwrap_or(Value::Null);
            let hit = self.queued.iter().find(|(_, (k, c))| k == kind && *c == content).map(|(s, _)| *s);
            match hit {
                Some(s) => data["from_queue"] = json!(s),
                None => {
                    data.as_object_mut().map(|o| o.remove("from_queue"));
                }
            }
        }
        Ok(true)
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp: PathBuf = path.with_extension(format!("tmp.{}", std::process::id()));
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}
