//! BISE-196: the REPL's ev lines through the writer rules.
use bise_session::recorder::Recorder;
use bise_session::{read_dir, State};
use serde_json::json;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-rec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn ev(r: &mut Recorder, typ: &str, data: serde_json::Value) -> Option<u64> {
    r.on_ev(&json!({"type": typ, "v": 1, "data": data}).to_string()).unwrap()
}

fn txt(t: &str) -> serde_json::Value {
    json!([{"kind": "text", "text": t}])
}

#[test]
fn the_writer_rules() {
    let t = tmp("rules");
    let dir = t.join("s");
    let mut r = Recorder::create(&dir, &t.join("blobs"), json!({"session": "s-x", "format": 1, "created_by": "t", "cwd": "/"}), "t").unwrap();
    let cfg = json!({"system": {"text": "sys"}, "tools": [{"name": "bash", "description": "d"}]});
    assert!(ev(&mut r, "context_set", cfg.clone()).is_some());
    assert!(ev(&mut r, "context_set", cfg).is_none(), "equal config skipped");
    assert!(ev(&mut r, "limits_set", json!({"compact_threshold": 10, "select_budget": 2, "max_nulls": 3})).is_some());
    assert!(ev(&mut r, "limits_set", json!({"compact_threshold": 10, "select_budget": 2, "max_nulls": 3})).is_none());
    ev(&mut r, "model_set", json!({"model": "m1", "source": "config"}));
    assert!(ev(&mut r, "model_set", json!({"model": "m1", "source": "config"})).is_none());
    let q1 = ev(&mut r, "input_queued", json!({"kind": "user", "content": txt("later")})).unwrap();
    ev(&mut r, "turn_started", json!({"cause": "user"}));
    let u = ev(&mut r, "user_message", json!({"content": txt("q1"), "delivery": "prompt"})).unwrap();
    let a = ev(&mut r, "assistant_message", json!({"req": 7, "model": "", "parts": txt("a1"), "calls": []})).unwrap();
    ev(&mut r, "turn_ended", json!({"outcome": "done", "counts": {"inputs": 1, "actions": 1}}));
    ev(&mut r, "turn_started", json!({"cause": "queue"}));
    let u2 = ev(&mut r, "user_message", json!({"content": txt("later"), "delivery": "prompt", "from_queue": 0})).unwrap();
    // compaction of positions 1..2 (u, a), nothing kept
    let c = ev(&mut r, "compaction_done", json!({"id": 1, "summary": txt("pre"), "replaces": {"from": 1, "to": 2}, "kept": []})).unwrap();
    let s = ev(&mut r, "context_injected", json!({"kind": "summary", "content": txt("sum")})).unwrap();
    ev(&mut r, "turn_ended", json!({"outcome": "done", "counts": {"inputs": 2, "actions": 1}}));
    let log = read_dir(&dir).unwrap();
    let get = |seq: u64| log.by_seq(seq).unwrap();
    assert_eq!(get(u).turn, Some(1));
    assert_eq!(get(u2).turn, Some(2));
    assert_eq!(get(q1).turn, None);
    assert_eq!(get(a).data["model"], "m1");
    assert_eq!(get(u2).data["from_queue"], q1);
    assert_eq!(get(c).data["replaces"], json!({"from": u, "to": a}));
    assert_eq!(get(c + 1).typ, "checkpoint", "a checkpoint follows a compaction");
    let st = State::rebuild(&log);
    assert_eq!(st.context, [c, u2, s], "summary preamble, the queued question, the summary");
    assert!(st.queue.is_empty());
    assert_eq!(st.counters.turn, 2);
    assert_eq!(r.state().context, st.context, "the recorder's state is the log's");
    // a fresh REPL gets the projection
    let p = t.join("resume.txt");
    r.project_to(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(text.starts_with("BEND-SESSION 2\nTOOL bash : d\nCFG 10 2 3 sys\nCOUNT 2 1\n"), "{text}");
    drop(r);
    // resumed: nothing to repair, the queue map rebuilt
    let r2 = Recorder::resume(&dir, &t.join("blobs"), "t").unwrap();
    assert!(r2.repaired.is_empty());
    assert_eq!(r2.state().context, st.context);
}

#[test]
fn a_crash_mid_turn_is_repaired_at_resume_but_not_at_attach() {
    let t = tmp("crash");
    let dir = t.join("s");
    let mut r = Recorder::create(&dir, &t.join("blobs"), json!({"session": "s-y", "format": 1, "created_by": "t", "cwd": "/"}), "t").unwrap();
    ev(&mut r, "turn_started", json!({"cause": "user"}));
    ev(&mut r, "user_message", json!({"content": txt("go"), "delivery": "prompt"}));
    ev(&mut r, "assistant_message", json!({"req": 1, "model": "m", "parts": [], "calls": [{"id": "call_3", "name": "bash", "args": "sleep 9"}]}));
    drop(r);
    let mut a = Recorder::attach(&dir, &t.join("blobs")).unwrap();
    assert!(a.repaired.is_empty());
    // the adopted REPL's turn goes on: its events keep the turn
    let s = ev(&mut a, "tool_result", json!({"call": "call_3", "ok": true, "content": txt("done")})).unwrap();
    assert_eq!(read_dir(&dir).unwrap().by_seq(s).unwrap().turn, Some(1));
    ev(&mut a, "assistant_message", json!({"req": 2, "model": "m", "parts": [], "calls": [{"id": "call_4", "name": "bash", "args": "x"}]}));
    drop(a);
    let r = Recorder::resume(&dir, &t.join("blobs"), "t").unwrap();
    assert_eq!(r.repaired.len(), 3, "tool_result for call_4, interrupted, turn_ended");
}
