//! BISE-199: the transcript and the context of fixture sessions.
use bise_session::{read_dir, show, State};
use std::path::{Path, PathBuf};

fn fx(n: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/session").join(n)
}

#[test]
fn the_short_session_reads_as_a_transcript() {
    let log = read_dir(&fx("01-short")).unwrap();
    let t = show::transcript(&log, Path::new("/none"));
    let want = "09:14:01 session s-20261001-091403-7f3a9c in /Users/ada/code/shop
09:14:02 opened by bise 0.9.4 (a1b2c3d) (pid 4242)
09:14:05 model anthropic/claude-sonnet-x (medium)
── turn 1 ──
09:14:07 › you
  How many tests are in this repo?
09:14:08 ◆ anthropic/claude-sonnet-x
  (thinking)
    Count the test files.
  → call_1 bash: rg -c '^def test_' tests | wc -l
   usage: in 12400 out 230 cache 5200
09:14:11 ← call_1 ok
  tool bash ok: 37
09:14:12 ◆ anthropic/claude-sonnet-x
  There are 37 test files.
   usage: in 12690 out 12 cache 12400
";
    assert_eq!(t, want);
}

#[test]
fn the_context_is_what_the_model_sees_after_a_compaction() {
    let log = read_dir(&fx("11-compaction")).unwrap();
    let c = show::context(&log, &State::rebuild(&log), Path::new("/none"));
    assert!(c.starts_with("09:14:25 compaction 1: replaces 7..23\n  Summary: 37 test files.\n09:14:16 › you\n"), "{c}");
    assert!(!c.contains("There are 37 test files."), "the replaced answer is out of the context");
    let t = show::transcript(&log, Path::new("/none"));
    assert!(t.contains("There are 37 test files."), "but still in the transcript");
}

#[test]
fn events_from_a_newer_bise_and_bad_lines_are_named() {
    let log = read_dir(&fx("05-unknown-type")).unwrap();
    assert!(show::transcript(&log, Path::new("/none")).contains("[plan_updated v1: from a newer bise]"));
    let log = read_dir(&fx("04-bad-line-in-the-middle")).unwrap();
    assert!(show::transcript(&log, Path::new("/none")).ends_with("[2 unreadable line(s) skipped]\n"));
}
