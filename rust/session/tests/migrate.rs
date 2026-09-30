//! BISE-197: a .txt becomes a log that projects back to it.
use bise_session::migrate::{migrate_txt, Outcome, Source};
use bise_session::read_dir;
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session/15-migrated")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-migrate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn strip(mut v: Value) -> Value {
    // what the writer and the moment decide
    for k in ["created_by", "writer", "pid", "session"] {
        v.as_object_mut().map(|o| o.remove(k));
    }
    if let Some(m) = v.get_mut("migrated_from").and_then(Value::as_object_mut) {
        m.remove("path");
    }
    v
}

#[test]
fn the_fixture_migrates_to_its_events() {
    let t = tmp("fixture");
    let txt = t.join("session.txt");
    std::fs::copy(fixture().join("session.txt"), &txt).unwrap();
    let src = Source { txt: txt.clone(), cwd: "/Users/ada/code/shop".into(), agent: None, model: None };
    let Outcome::Migrated { dir, messages, dropped_lines, images, .. } = migrate_txt(&src, &t.join("sessions"), &t.join("blobs"), "test") else {
        panic!("not migrated")
    };
    assert_eq!((messages, dropped_lines, images), (11, 2, 0));
    let got = read_dir(&dir).unwrap();
    let want = read_dir(&fixture()).unwrap();
    assert_eq!(got.events.len(), want.events.len());
    for (g, w) in got.events.iter().zip(&want.events) {
        assert_eq!((g.seq, &g.typ, g.must, strip(g.data.clone())), (w.seq, &w.typ, w.must, strip(w.data.clone())), "seq {}", g.seq);
    }
    assert_eq!(std::fs::read(&txt).unwrap(), std::fs::read(fixture().join("session.txt")).unwrap(), "the .txt is untouched");
}

#[test]
fn an_image_whose_file_exists_becomes_a_blob_and_projects_back() {
    let t = tmp("image");
    let b64 = t.join("img.b64");
    std::fs::write(&b64, "aGVsbG8hPw==").unwrap();
    let txt = t.join("s.txt");
    let marker = format!("<image name=\"[Image #1]\" path=\"shot.png\" mime=\"image/png\" b64=\"{}\">", b64.display());
    std::fs::write(&txt, format!("BEND-SESSION 2\nCFG 1 2 3 sys\nCOUNT 1 0\nMSG False user : look {marker} here\n")).unwrap();
    let src = Source { txt, cwd: "/".into(), agent: None, model: Some("m".into()) };
    let Outcome::Migrated { dir, images, .. } = migrate_txt(&src, &t.join("sessions"), &t.join("blobs"), "test") else { panic!() };
    assert_eq!(images, 1);
    let log = read_dir(&dir).unwrap();
    let um = log.events.iter().find(|e| e.typ == "user_message").unwrap();
    assert_eq!(um.data["content"][1]["kind"], "image");
    let r: bise_session::types::BlobRef = serde_json::from_value(um.data["content"][1]["image"].clone()).unwrap();
    assert_eq!(bise_session::blob::get(&t.join("blobs"), &r).unwrap(), b"hello!?");
    // the .b64 goes away: the projection writes it back from the blob
    std::fs::remove_file(&b64).unwrap();
    let st = bise_session::State::rebuild(&log);
    assert!(bise_session::project::materialize_images(&log, &st, &t.join("blobs")).is_empty());
    assert_eq!(std::fs::read_to_string(&b64).unwrap(), "aGVsbG8hPw==");
}

#[test]
fn an_unloadable_file_is_left_alone() {
    let t = tmp("unloadable");
    let txt = t.join("s.txt");
    std::fs::write(&txt, "BEND-SESSION 9\n").unwrap();
    let src = Source { txt, cwd: "/".into(), agent: None, model: None };
    assert!(matches!(migrate_txt(&src, &t.join("sessions"), &t.join("blobs"), "t"), Outcome::Unloadable));
    assert!(!t.join("sessions").exists() || std::fs::read_dir(t.join("sessions")).unwrap().count() == 0);
}
