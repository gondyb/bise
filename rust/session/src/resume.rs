//! Resume (§7): open the log to append, rebuild the state, close what a
//! crash left open, write process_opened.
use crate::pairing;
use crate::reader::Log;
use crate::state::State;
use crate::types::*;
use crate::writer::{OpenError, Writer};
use serde_json::json;
use std::path::Path;

pub const INTERRUPTED_BY_RESTART: &str = "interrupted by a restart";

pub struct Resumed {
    pub writer: Writer,
    pub log: Log,
    pub state: State,
    /// the seqs the repair appended (§7 step 5)
    pub repaired: Vec<u64>,
}

/// The repair a crash needs (§7 step 5), as (type, data, turn), for
/// the state rebuilt from `log`; `next` is the seq the first repair
/// event gets. Every call of the context left without its result gets
/// a failed one (BISE-242), wherever it is: a call cut inside an older
/// turn (a hub restart that lost the result) goes right after its
/// assistant message (`after`), so the context stays paired.
pub fn repair_plan(log: &Log, st: &State, next: u64) -> Vec<(&'static str, serde_json::Value, Option<u64>)> {
    let mut out = Vec::new();
    let groups = pairing::owed(log, st);
    for data in pairing::closing(&groups, next) {
        out.push(("tool_result", data, st.open_turn));
    }
    if let Some(turn) = st.open_turn {
        let pending: Vec<String> =
            groups.iter().filter(|g| g.trailing).flat_map(|g| g.calls.iter().map(|(c, _)| c.clone())).collect();
        let during = if !pending.is_empty() {
            "tool"
        } else if st.open_compaction.is_some() {
            "compaction"
        } else {
            "request"
        };
        let mut i = json!({"by": "restart", "during": during});
        if !pending.is_empty() {
            i["pending_calls"] = json!(pending);
        }
        out.push(("interrupted", i, Some(turn)));
        out.push(("turn_ended", json!({"outcome": "crashed"}), Some(turn)));
    }
    if let Some((_, id)) = st.open_compaction {
        out.push((
            "compaction_failed",
            json!({"id": id, "attempt": 1, "error": {"kind": "internal", "message": INTERRUPTED_BY_RESTART}}),
            None,
        ));
    }
    out
}

/// Resume a session folder: §7 steps 1-6. `writer` names this bise.
pub fn resume(dir: &Path, blobs: &Path, writer: &str) -> Result<Resumed, OpenError> {
    let (mut w, log) = Writer::open(dir, blobs)?;
    let mut state = State::rebuild(&log);
    let mut repaired = Vec::new();
    for (typ, data, turn) in repair_plan(&log, &state, w.last_seq() + 1) {
        let seq = w.append(typ, data.clone(), turn)?;
        let p = Payload::parse(typ, 1, &data).and_then(Result::ok);
        state.apply(seq, turn, p.as_ref());
        repaired.push(seq);
    }
    w.append("process_opened", json!({"writer": writer, "pid": std::process::id(), "resume": true}), None)?;
    // the log with the repair in it (projection reads the repaired events)
    let log = crate::reader::read_dir(dir)?;
    Ok(Resumed { writer: w, log, state, repaired })
}
