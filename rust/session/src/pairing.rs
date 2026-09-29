//! Calls and results stay paired (BISE-242): in the context a model
//! sees, the calls of an assistant message are answered, one
//! `tool_result` each, before any other message; a provider refuses
//! anything else (Anthropic: 400 "tool_use ids were found without
//! tool_result blocks immediately after"), on every later turn.
//!
//! A cut breaks it: a restart or a crash mid-call, or a hub down while
//! an adopted REPL sent the result. The fix closes each call left
//! without its result with a synthetic failed one, written to the log
//! so the context stays one event per Core message: at resume (all of
//! them), in the recorder (at the next cut event), and, last resort, in
//! the projection's text (core/history.bend pair_calls is the Core's
//! twin, and core/wire.bend pair_wire the guard before each request).
use crate::reader::Log;
use crate::state::State;
use crate::types::Payload;
use serde_json::{json, Value};

/// The text of a synthetic result, after "tool <name> failed: ".
pub const NO_RESULT: &str = "no result: bise restarted while this ran";

/// The Core's text of a synthetic result (session.bend tool_result_msg).
pub fn no_result_text(name: &str) -> String {
    format!("tool {name} failed: {NO_RESULT}")
}

/// The `tool_result` data answering `call` (tool `name`); `after`: the
/// context seq it goes right after, when it is not the context's end.
pub fn no_result(call: &str, name: &str, after: Option<u64>) -> Value {
    let mut d = json!({"call": call, "ok": false, "content": [{"kind": "text", "text": no_result_text(name)}]});
    if let Some(a) = after {
        d["after"] = json!(a);
    }
    d
}

/// The calls of one assistant message still waiting for their result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owed {
    /// (call id, tool name), in call order
    pub calls: Vec<(String, String)>,
    /// the seq of the group's last context event (the assistant
    /// message or its last result)
    pub last: u64,
    /// nothing follows the group in the context
    pub trailing: bool,
}

/// Every group of the context with a call left without its result, in
/// context order.
pub fn owed(log: &Log, st: &State) -> Vec<Owed> {
    let mut out = Vec::new();
    let mut open: Option<Owed> = None;
    let flush = |open: &mut Option<Owed>, out: &mut Vec<Owed>| {
        if let Some(o) = open.take().filter(|o| !o.calls.is_empty()) {
            out.push(o);
        }
    };
    for &s in &st.context {
        match log.by_seq(s).and_then(|e| e.payload.as_ref()) {
            Some(Payload::AssistantMessage(m)) => {
                flush(&mut open, &mut out);
                let calls = m.calls.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
                open = Some(Owed { calls, last: s, trailing: false });
            }
            Some(Payload::ToolResult(r)) => {
                if let Some(o) = open.as_mut().filter(|o| o.calls.iter().any(|(c, _)| *c == r.call)) {
                    o.calls.retain(|(c, _)| *c != r.call);
                    o.last = s;
                }
            }
            _ => flush(&mut open, &mut out),
        }
    }
    if let Some(o) = open.as_mut() {
        o.trailing = true;
    }
    flush(&mut open, &mut out);
    out
}

/// The `tool_result` events that close every owed call, as data, for
/// events appended from seq `next` on: a group inside the context gets
/// `after` (chained), a trailing one is appended at the end as usual.
pub fn closing(groups: &[Owed], mut next: u64) -> Vec<Value> {
    let mut out = Vec::new();
    for g in groups {
        let mut after = g.last;
        for (call, name) in &g.calls {
            out.push(no_result(call, name, (!g.trailing).then_some(after)));
            after = next;
            next += 1;
        }
    }
    out
}

/// What breaks the contract in `st`'s context, one line each (empty:
/// paired). For tests and checks.
pub fn violations(log: &Log, st: &State) -> Vec<String> {
    let mut v: Vec<String> = owed(log, st)
        .iter()
        .flat_map(|g| g.calls.iter().map(move |(c, _)| format!("{c} (after seq {}) has no result", g.last)))
        .collect();
    let mut open: Vec<String> = Vec::new();
    for &s in &st.context {
        match log.by_seq(s).and_then(|e| e.payload.as_ref()) {
            Some(Payload::AssistantMessage(m)) => open = m.calls.iter().map(|c| c.id.clone()).collect(),
            Some(Payload::ToolResult(r)) => {
                if !open.contains(&r.call) {
                    v.push(format!("seq {s}: result of {} with no call waiting", r.call));
                }
                open.retain(|c| *c != r.call);
            }
            _ => open.clear(),
        }
    }
    v
}
