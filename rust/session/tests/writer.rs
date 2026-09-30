//! BISE-192: the writer, the blob store, rotation.
use bise_session::{blob, read_dir, OpenError, Writer};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-session-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session").join(name)
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn start() -> serde_json::Value {
    json!({"session": "s-1", "format": 1, "created_by": "test", "cwd": "/"})
}

#[test]
fn a_new_session_is_private_and_starts_with_its_head() {
    let t = tmp("new");
    let dir = t.join("sessions/s-1");
    let mut w = Writer::create(&dir, &t.join("blobs"), start(), "test").unwrap();
    assert_eq!(w.append("user_message", json!({"content": [{"kind": "text", "text": "hi"}], "delivery": "prompt"}), Some(1)).unwrap(), 3);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("events.jsonl")), 0o600);
    assert_eq!(mode(&dir.join("lock")), 0o600);
    let log = read_dir(&dir).unwrap();
    let types: Vec<_> = log.events.iter().map(|e| e.typ.as_str()).collect();
    assert_eq!(types, ["session_start", "process_opened", "user_message"]);
    assert!(log.events[2].must && log.events[2].turn == Some(1));
    assert!(!log.events[1].must);
}

#[test]
fn a_second_writer_is_refused_while_the_first_lives() {
    let t = tmp("lock");
    let dir = t.join("s");
    let w = Writer::create(&dir, &t.join("blobs"), start(), "test").unwrap();
    assert!(matches!(Writer::open(&dir, &t.join("blobs")), Err(OpenError::Locked(_))));
    drop(w);
    assert!(Writer::open(&dir, &t.join("blobs")).is_ok());
}

#[test]
fn an_append_keeps_every_older_line_byte_for_byte() {
    for case in ["05-unknown-type", "07-unknown-fields"] {
        let t = tmp(case);
        copy(&fixture(case), &t.join("s"));
        let before = std::fs::read(t.join("s/events.jsonl")).unwrap();
        let (mut w, log) = Writer::open(&t.join("s"), &t.join("blobs")).unwrap();
        assert_eq!(w.last_seq(), log.last_seq());
        let next = log.last_seq() + 1;
        w.append("title_set", json!({"title": "x", "source": "auto"}), None).unwrap();
        let after = std::fs::read(t.join("s/events.jsonl")).unwrap();
        assert!(after.starts_with(&before), "{case}");
        let log = read_dir(&t.join("s")).unwrap();
        assert_eq!(log.events.last().unwrap().seq, next, "{case}: seq goes on");
    }
}

#[test]
fn an_unknown_must_event_makes_the_writer_refuse() {
    let t = tmp("must");
    copy(&fixture("06-unknown-must"), &t.join("s"));
    let before = std::fs::read(t.join("s/events.jsonl")).unwrap();
    assert!(matches!(Writer::open(&t.join("s"), &t.join("blobs")), Err(OpenError::ReadOnly(_))));
    assert_eq!(std::fs::read(t.join("s/events.jsonl")).unwrap(), before);
}

#[test]
fn a_torn_tail_is_cut_and_saved() {
    let t = tmp("torn");
    copy(&fixture("03-torn-last-line"), &t.join("s"));
    let (mut w, log) = Writer::open(&t.join("s"), &t.join("blobs")).unwrap();
    let torn = log.torn.clone().unwrap();
    let saved: Vec<_> = std::fs::read_dir(t.join("s")).unwrap().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("events.torn-")).collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(std::fs::read(saved[0].path()).unwrap(), torn);
    w.append("title_set", json!({"title": "x", "source": "auto"}), None).unwrap();
    let log = read_dir(&t.join("s")).unwrap();
    assert!(log.torn.is_none() && log.bad_lines.is_empty());
    assert_eq!(log.events.last().unwrap().seq, 15);
}

#[test]
fn a_big_text_goes_to_a_blob_written_before_its_line() {
    let t = tmp("blob");
    let dir = t.join("s");
    let mut w = Writer::create(&dir, &t.join("blobs"), start(), "test").unwrap();
    let big = "x".repeat(300 * 1024);
    w.append("tool_result", json!({"call": "call_1", "ok": true, "content": [{"kind": "text", "text": "small"}, {"kind": "text", "text": big}]}), Some(1)).unwrap();
    let raw = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
    assert!(raw.lines().all(|l| l.len() < 256 * 1024));
    let log = read_dir(&dir).unwrap();
    let content = &log.events[2].data["content"];
    assert_eq!(content[0]["text"], "small");
    assert_eq!(content[1]["kind"], "text_blob");
    let r: bise_session::types::BlobRef = serde_json::from_value(content[1]["blob"].clone()).unwrap();
    assert_eq!(blob::get(&t.join("blobs"), &r).unwrap(), big.as_bytes());
    let f = blob::path(&t.join("blobs"), &r.sha256);
    assert_eq!(mode(&f), 0o600);
    assert!(f.starts_with(t.join("blobs/sha256")) && f.parent().unwrap().file_name().unwrap().len() == 2);
}

#[test]
fn rotation_starts_a_segment_that_names_the_last_one() {
    let t = tmp("rotate");
    let dir = t.join("s");
    let mut w = Writer::create(&dir, &t.join("blobs"), start(), "test").unwrap();
    w.segment_max = 200;
    w.append("title_set", json!({"title": "a long enough title to pass 200 bytes of segment", "source": "auto"}), None).unwrap();
    assert!(w.wants_rotation());
    let old = std::fs::read(dir.join("events.jsonl")).unwrap();
    let cp = json!({"upto": 3, "context": [], "system": {"text": ""}, "tools": [], "model": {"model": "m"},
                    "limits": {}, "queue": [], "counters": {}, "usage_total": {}});
    w.rotate("s-1", cp).unwrap();
    assert!(w.size() < old.len() as u64 + 400, "the new segment holds only its head");
    w.append("title_set", json!({"title": "b", "source": "auto"}), None).unwrap();
    assert_eq!(std::fs::read(dir.join("events.000001.jsonl")).unwrap(), old);
    assert_eq!(mode(&dir.join("events.jsonl")), 0o600);
    let log = read_dir(&dir).unwrap();
    assert_eq!(log.open, bise_session::Open::Ok);
    let seqs: Vec<_> = log.events.iter().map(|e| (e.seq, e.typ.as_str())).collect();
    assert_eq!(seqs, [(1, "session_start"), (2, "process_opened"), (3, "title_set"), (4, "segment_start"), (5, "checkpoint"), (6, "title_set")]);
    let prev = &log.events[3].data["prev"];
    assert_eq!(prev["file"], "events.000001.jsonl");
    assert_eq!(prev["last_seq"], 3);
    assert_eq!(prev["sha256"], blob::sha256_hex(&old));
}

#[test]
fn session_ids_sort_by_time() {
    let id = bise_session::new_session_id();
    assert_eq!(id.len(), "s-20261001-091403-7f3a9c".len(), "{id}");
    assert!(id.starts_with("s-20"));
}
