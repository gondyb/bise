//! BISE-197: the BEND-SESSION 2 reader mirrors core/checkpoint.bend.
use bise_session::legacy::{parse, to_text};
use std::path::PathBuf;

fn fx(f: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session").join(f)).unwrap()
}

#[test]
fn what_the_loader_keeps_of_the_migrated_fixture() {
    let s = parse(&fx("15-migrated/session.txt")).unwrap();
    assert_eq!(s.dropped_lines, 2);
    assert_eq!(to_text(&s), fx("15-migrated/projection.txt"));
    let a = &s.msgs[2];
    assert_eq!(a.calls.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2], "calls keep their order");
    assert_eq!(a.calls[0].args, "ls \\tmp\necho a\rb");
}

#[test]
fn a_file_the_loader_refuses_gives_nothing() {
    assert!(parse("BEND-SESSION 3\nCFG 1 2 3 s\n").is_none());
    assert!(parse("BEND-SESSION 2\nMSG False user : hi\n").is_none(), "no CFG");
    let s = parse("BEND-SESSION 2\nCFG 1 2 3 s\nMSG odd\n  CALL 7 x : y\nMSG False weird : t\n").unwrap();
    assert_eq!(s.dropped_lines, 2, "a MSG it cannot split, a CALL with no message");
    assert_eq!(to_text(&s), "BEND-SESSION 2\nCFG 1 2 3 s\nCOUNT 0 0\nMSG False user : t\n");
}

#[test]
fn every_short_text_round_trips() {
    let t = "BEND-SESSION 2\nTOOL a : b\nCFG 10 20 3 x\\ny\nCOUNT 4 5\nQUEUE a\\Nb\nNOTIF n\nMSG True user : u\\\\n\nMSG False assistant : <think>t</think>ok\n  CALL 1 bash : ls\n  CALL 2 bash : pwd\nMSG False tool : r1\nMSG False tool : r2\n";
    assert_eq!(to_text(&parse(t).unwrap()), t);
}
