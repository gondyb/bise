//! BISE-193: secrets never reach the file (fixture 14).
use bise_session::{read_dir, Redactor, Writer};
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session/14-redaction")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-redact-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// auth.json and .env of the fixture's secrets.json, as files of a home
fn home_files(t: &std::path::Path) -> (PathBuf, PathBuf) {
    let s: Value = serde_json::from_str(&std::fs::read_to_string(fixture().join("secrets.json")).unwrap()).unwrap();
    let auth = t.join("auth.json");
    // the real layout: {"<provider>": {"type": "api", "key": …}}
    let mut a = serde_json::Map::new();
    for (k, v) in s["auth.json"].as_object().unwrap() {
        a.insert(k.clone(), serde_json::json!({"type": "api", "key": v}));
    }
    std::fs::write(&auth, Value::Object(a).to_string()).unwrap();
    let env = t.join(".env");
    let lines: String = s[".env"].as_object().unwrap().iter().map(|(k, v)| format!("export {k}=\"{}\"\n", v.as_str().unwrap())).collect();
    std::fs::write(&env, lines).unwrap();
    (auth, env)
}

#[test]
fn the_writer_replaces_known_values_and_key_shapes() {
    let t = tmp("fixture");
    let (auth, env) = home_files(&t);
    let input = std::fs::read_to_string(fixture().join("input.jsonl")).unwrap();
    let want = read_dir(&fixture()).unwrap();
    let dir = t.join("s");
    let mut lines = input.lines().map(|l| serde_json::from_str::<Value>(l).unwrap());
    let first = lines.next().unwrap();
    let mut w = Writer::create(&dir, &t.join("blobs"), first["data"].clone(), "test").unwrap();
    w.redactor = Some(Redactor::from_home(&auth, &[env]));
    for e in lines.skip(1) {
        w.append(e["type"].as_str().unwrap(), e["data"].clone(), e["turn"].as_u64()).unwrap();
    }
    let got = read_dir(&dir).unwrap();
    let got: Vec<_> = got.events.iter().map(|e| (e.seq, e.typ.clone(), e.data.clone())).collect();
    let want: Vec<_> = want.events.iter().map(|e| (e.seq, e.typ.clone(), e.data.clone())).collect();
    // process_opened is the writer's own (pid, writer)
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        if g.1 != "process_opened" {
            assert_eq!(g, w);
        }
    }
    let raw = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
    assert!(!raw.contains("sk-ant-api03") && !raw.contains("hunter2"), "no key on disk");
    assert!(raw.contains("sk-not-a-key stays"));
}

#[test]
fn shapes_need_their_length_and_a_word_start() {
    let r = Redactor::new(vec![]);
    let k = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz";
    assert_eq!(r.text(&format!("key={k}.")), "key=«redacted:anthropic».");
    assert_eq!(r.text("sk-ant-short"), "sk-ant-short");
    assert_eq!(r.text("xsk-ant-api03-abcdefghijklmnopqrstuvwxyz"), "xsk-ant-api03-abcdefghijklmnopqrstuvwxyz");
    assert_eq!(r.text("AKIAABCDEFGHIJKLMNOP and ghp_0123456789abcdefghijklmnopqrstuvwxyzAB"), "«redacted:aws» and «redacted:github»");
    assert_eq!(r.text("é sk-proj-ABCDEFGHIJKLMNOPQRSTUVWXYZ012345 é"), "é «redacted:openai» é");
}

#[test]
fn short_values_are_not_redacted_and_the_longest_wins() {
    let r = Redactor::new(vec![("A".into(), "abc".into()), ("B".into(), "longsecret-1".into()), ("C".into(), "longsecret-12".into())]);
    assert_eq!(r.text("abc longsecret-12 longsecret-1"), "abc «redacted:C» «redacted:B»");
}
