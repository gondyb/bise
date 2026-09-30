//! BISE-242: calls and results stay paired. The real log of the incident
//! (main's session, 2026-09-29: call_4346 cut by a hub restart, every
//! later turn refused with a provider 400), and a property test over cut
//! points: whatever is lost where, the context a REPL loads is paired.
use bise_session::recorder::Recorder;
use bise_session::{pairing, project, read_dir, resume::resume, State};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn real_log() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session-real/orphan-call-4346/events.jsonl")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-pairing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The unpaired spots of a BEND-SESSION 2 text: an assistant MSG's CALL
/// lines must be followed by as many tool MSG lines, and a tool MSG must
/// answer a call.
fn text_violations(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut owed = 0usize;
    let mut calls = 0usize;
    let mut in_asst = false;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with("  CALL ") {
            if in_asst {
                calls += 1;
            }
            continue;
        }
        if in_asst {
            owed = calls;
            in_asst = false;
        }
        if !l.starts_with("MSG ") {
            continue;
        }
        if l.starts_with("MSG False tool :") {
            if owed == 0 {
                v.push(format!("line {}: a tool result no call waits for", i + 1));
            } else {
                owed -= 1;
            }
            continue;
        }
        if owed > 0 {
            v.push(format!("line {}: {owed} call(s) with no result before: {}", i + 1, &l[..l.len().min(60)]));
        }
        owed = 0;
        if l.contains(" assistant :") {
            in_asst = true;
            calls = 0;
        }
    }
    if in_asst {
        owed = calls;
    }
    if owed > 0 {
        v.push(format!("{owed} call(s) with no result at the end"));
    }
    v
}

fn projection(dir: &Path, blobs: &Path) -> String {
    let log = read_dir(dir).unwrap();
    project::project(&log, &State::rebuild(&log), blobs).unwrap()
}

#[test]
fn the_real_log_resumes_paired() {
    let t = tmp("real");
    let dir = t.join("s");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(real_log(), dir.join("events.jsonl")).unwrap();
    // the bug, as the log was: call_4346 (seq 2038) is followed by the
    // next turn's messages, with no result
    let log = read_dir(&dir).unwrap();
    let st = State::rebuild(&log);
    let v = pairing::violations(&log, &st);
    assert!(v.iter().any(|l| l.starts_with("call_4346 (after seq 2038)")), "{v:?}");
    // the resume closes it right after its assistant message
    let r = resume(&dir, &t.join("blobs"), "test").unwrap();
    let fix: Vec<_> = r.repaired.iter().map(|s| r.log.by_seq(*s).unwrap()).filter(|e| e.typ == "tool_result").collect();
    let fixed = fix.iter().find(|e| e.data["call"] == "call_4346").expect("a result for call_4346");
    assert_eq!(fixed.data["ok"], false);
    assert_eq!(fixed.data["after"], 2038);
    assert_eq!(fixed.data["content"][0]["text"], "tool bash failed: no result: bise restarted while this ran");
    let at = r.state.context.iter().position(|&s| s == 2038).unwrap();
    assert_eq!(r.state.context[at + 1], fixed.seq, "the result follows its call");
    assert_eq!(pairing::violations(&r.log, &r.state), Vec::<String>::new());
    assert_eq!(State::rebuild(&r.log).context, r.state.context, "the log replays to the same context");
    let text = projection(&dir, &t.join("blobs"));
    assert_eq!(text_violations(&text), Vec::<String>::new());
    // the REPL loads the answer right after the CALL line
    let i = text.find("sb inspect worktrees-home --last 3").unwrap();
    let after = &text[i..];
    let next_msg = after.lines().find(|l| l.starts_with("MSG ")).unwrap();
    assert!(next_msg.starts_with("MSG False tool : tool bash failed: no result"), "{next_msg}");
    // a second resume has nothing left to repair
    drop(r);
    assert!(resume(&dir, &t.join("blobs"), "test").unwrap().repaired.is_empty());
}

#[test]
fn the_incident_through_the_recorder() {
    // the hub restarted and adopted main's REPL right after tool_started
    // (seq 2039); the REPL's result and turn_ended were lost; the next
    // line it sent was turn 112's turn_started
    let t = tmp("incident");
    let dir = t.join("s");
    std::fs::create_dir_all(&dir).unwrap();
    let raw = std::fs::read_to_string(real_log()).unwrap();
    let head: String = raw
        .lines()
        .take_while(|l| serde_json::from_str::<Value>(l).unwrap()["seq"].as_u64().unwrap() <= 2039)
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(dir.join("events.jsonl"), head).unwrap();
    let mut r = Recorder::attach(&dir, &t.join("blobs")).unwrap();
    assert!(r.repaired.is_empty(), "the last call may still get its result");
    let ts = r.on_ev(&json!({"type": "turn_started", "v": 1, "data": {"cause": "user"}}).to_string()).unwrap().unwrap();
    r.on_ev(&json!({"type": "agent_message", "v": 1, "data": {"hub_msg": "m_2144", "from": "hub-lag", "relation": "child", "expects_reply": false, "content": [{"kind": "text", "text": "hi"}]}}).to_string()).unwrap();
    let log = read_dir(&dir).unwrap();
    let res = log.by_seq(2040).unwrap();
    assert_eq!((res.typ.as_str(), res.data["call"].as_str()), ("tool_result", Some("call_4346")));
    assert_eq!(log.by_seq(2041).unwrap().typ, "turn_ended");
    assert_eq!(log.by_seq(2041).unwrap().data["outcome"], "crashed");
    assert_eq!(ts, 2042);
    assert_eq!(log.by_seq(ts).unwrap().turn, Some(112));
    let st = State::rebuild(&log);
    assert_eq!(st.context, r.state().context);
    assert_eq!(pairing::violations(&log, &st), Vec::<String>::new());
    assert_eq!(text_violations(&projection(&dir, &t.join("blobs"))), Vec::<String>::new());
}

#[test]
fn the_projection_guard_pairs_what_the_log_did_not() {
    // a log the repair never saw (read-only): the text is still paired
    let log = bise_session::read_bytes(&[("events.jsonl".into(), std::fs::read(real_log()).unwrap())]);
    let st = State::rebuild(&log);
    let text = project::project(&log, &st, Path::new("/nonexistent")).unwrap();
    assert_eq!(text_violations(&text), Vec::<String>::new());
    // and the checker does see the bug in a raw text
    let bad = "BEND-SESSION 2\nMSG False assistant : \n  CALL 1 bash : x\nMSG False user : next\n";
    assert_eq!(text_violations(bad).len(), 1);
}

// ---- the property: cut anywhere, lose anything, the context is paired ----

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn txt(t: &str) -> Value {
    json!([{"kind": "text", "text": t}])
}

/// The ev lines of a random healthy session: turns of user message,
/// rounds of tool calls (parallel ones too), a final answer; agent
/// messages and injected context between turns.
fn session(rng: &mut Rng) -> Vec<(String, Value)> {
    let mut evs: Vec<(String, Value)> = Vec::new();
    let mut e = |t: &str, d: Value| evs.push((t.to_string(), d));
    let mut call = 0;
    let mut req = 0;
    for turn in 0..1 + rng.below(4) {
        e("turn_started", json!({"cause": "user"}));
        e("user_message", json!({"content": txt(&format!("q{turn}")), "delivery": "prompt"}));
        for _ in 0..rng.below(4) {
            req += 1;
            let n = 1 + rng.below(3);
            let calls: Vec<Value> =
                (0..n).map(|k| json!({"id": format!("call_{}", call + k), "name": "bash", "args": "x"})).collect();
            e("assistant_message", json!({"req": req, "model": "m", "parts": txt(""), "calls": calls}));
            for k in 0..n {
                e("tool_started", json!({"call": format!("call_{}", call + k)}));
            }
            for k in 0..n {
                let tool = format!("call_{}", call + k);
                e("tool_result", json!({"call": tool, "ok": true, "content": txt(&format!("tool bash ok: {tool}"))}));
            }
            call += n;
        }
        req += 1;
        e("assistant_message", json!({"req": req, "model": "m", "parts": txt("done"), "calls": []}));
        e("turn_ended", json!({"outcome": "done"}));
        if rng.below(2) == 0 {
            e("agent_message", json!({"hub_msg": "m_1", "from": "peer", "relation": "peer", "expects_reply": false, "content": txt("note")}));
        }
    }
    evs
}

fn feed(r: &mut Recorder, evs: &[(String, Value)]) {
    for (t, d) in evs {
        r.on_ev(&json!({"type": t, "v": 1, "data": d}).to_string()).unwrap();
    }
}

fn start() -> Value {
    json!({"session": "s-fuzz", "format": 1, "created_by": "t", "cwd": "/"})
}

fn next_turn() -> Vec<(String, Value)> {
    vec![
        ("turn_started".into(), json!({"cause": "user"})),
        ("user_message".into(), json!({"content": txt("again"), "delivery": "prompt"})),
        ("assistant_message".into(), json!({"req": 999, "model": "m", "parts": txt("ok"), "calls": []})),
        ("turn_ended".into(), json!({"outcome": "done"})),
    ]
}

/// A REPL starts on this folder (resume, projection) and runs a turn: the
/// log and the projection are paired at each step.
fn check_resumes_paired(dir: &Path, blobs: &Path, what: &str) {
    let mut r = Recorder::resume(dir, blobs, "t").unwrap_or_else(|e| {
        let log = read_dir(dir).unwrap();
        panic!("{what}: {e}: malformed {:?}", log.malformed)
    });
    let log = read_dir(dir).unwrap();
    assert_eq!(pairing::violations(&log, r.state()), Vec::<String>::new(), "{what}: after resume");
    assert_eq!(State::rebuild(&log).context, r.state().context, "{what}: the log replays");
    let text = projection(dir, blobs);
    assert_eq!(text_violations(&text), Vec::<String>::new(), "{what}: projection\n{text}");
    feed(&mut r, &next_turn());
    let log = read_dir(dir).unwrap();
    assert_eq!(pairing::violations(&log, r.state()), Vec::<String>::new(), "{what}: after the next turn");
}

/// Run `run` of the property (its own seed): every cut point of one
/// random session, as a crash and as an adoption that lost lines.
fn cut_points(run: u64) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ (run + 1).wrapping_mul(0x2545_f491_4f6c_dd1d));
    let t = tmp(&format!("cuts-{run}"));
    let blobs = t.join("blobs");
    let mut cases = 0;
    {
        let evs = session(&mut rng);
        for cut in 0..=evs.len() {
            // a crash at `cut`: the REPL died, a new one resumes
            let dir = t.join(format!("crash-{run}-{cut}"));
            let mut r = Recorder::create(&dir, &blobs, start(), "t").unwrap();
            feed(&mut r, &evs[..cut]);
            drop(r);
            check_resumes_paired(&dir, &blobs, &format!("run {run}, crash at {cut}"));
            // a hub restart at `cut`: it adopts the REPL, whose next
            // lines (up to 4) are lost, then the REPL goes on
            let lost = (rng.below(5) as usize).min(evs.len() - cut);
            let dir = t.join(format!("adopt-{run}-{cut}"));
            let mut r = Recorder::create(&dir, &blobs, start(), "t").unwrap();
            feed(&mut r, &evs[..cut]);
            drop(r);
            let mut r = Recorder::attach(&dir, &blobs).unwrap();
            feed(&mut r, &evs[cut + lost..]);
            feed(&mut r, &next_turn());
            let log = read_dir(&dir).unwrap();
            let what = format!("run {run}, adopted at {cut}, {lost} lost");
            assert_eq!(State::rebuild(&log).context, r.state().context, "{what}: the log replays");
            assert_eq!(pairing::violations(&log, r.state()), Vec::<String>::new(), "{what}");
            drop(r);
            check_resumes_paired(&dir, &blobs, &what);
            cases += 2;
        }
    }
    assert!(cases >= 2);
    let _ = std::fs::remove_dir_all(&t);
}

/// The runs of shard `k` of 4 (the shards run in parallel): one each by
/// default, more with FUZZ_RUNS (the full gate's 2000 gives 2 each).
fn shard(k: u64) {
    let fuzz: u64 = std::env::var("FUZZ_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let per = (fuzz / 1000).max(1);
    for i in 0..per {
        cut_points(k + 4 * i);
    }
}

#[test]
fn any_cut_point_resumes_paired_0() {
    shard(0)
}
#[test]
fn any_cut_point_resumes_paired_1() {
    shard(1)
}
#[test]
fn any_cut_point_resumes_paired_2() {
    shard(2)
}
#[test]
fn any_cut_point_resumes_paired_3() {
    shard(3)
}
