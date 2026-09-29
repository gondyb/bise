//! BISE-110 (book §9 'A box that only sends'): a bash box hides iff its
//! script is one `sb send|ask|report` (after one optional `cd <dir> &&`),
//! it succeeded, and the message it sent (matched by the id in its
//! output) is drawn below it in this feed. ctrl+o shows it again.

use super::*;

// the runtime's wire encoding of a tool's code
fn enc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\r', "\\R").replace('\n', "\\N")
}

/// A bash tool `id` running `script`, its result `out`, ok or not.
fn bash(id: u32, script: &str, out: &str, ok: bool) -> Vec<String> {
    vec![
        format!("  obs: tool_started #{id}"),
        format!("tool #{id} bash : x"),
        format!("tool_code #{id} : {}", enc(script)),
        format!("tool_result #{id} {} : {out}", if ok { "ok" } else { "fail" }),
        format!("  obs: tool_finished #{id} {}", if ok { "ok" } else { "failed" }),
    ]
}

/// The bash tool, with main's feed line of the message at `at` (the hub
/// writes it while the tool runs: before its result).
fn with_msg(mut tool: Vec<String>, at: usize, msg: &str) -> Vec<String> {
    tool.insert(at, msg.to_string());
    tool
}

fn feed(lines: &[String]) -> (Vec<Ev>, Vec<Option<EventRows>>) {
    let (mut events, mut cache) = (Vec::new(), Vec::new());
    push_event(&mut events, &mut cache, Ev::Assistant("sending it".into()));
    for l in lines {
        push_event(&mut events, &mut cache, parse_line(l).expect("parse"));
    }
    (events, cache)
}

/// The feed as text, from its cache as a frame builds it.
fn screen(events: &[Ev], cache: &mut [Option<EventRows>]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, c) in cache.iter_mut().enumerate().take(events.len()) {
        let er = c.get_or_insert_with(|| event_rows(events, i, false, 80, 0));
        for l in &er.rows {
            out.push(l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string());
        }
    }
    out
}

/// The call shows: its row (BISE-223, every view) or its open box.
fn has_box(rows: &[String]) -> bool {
    rows.iter().any(|r| r.starts_with(" $ ") || r.starts_with("╭─ $ bash"))
}

fn has_chip(rows: &[String], text: &str) -> bool {
    // the chip, its text under it (BISE-127)
    rows.windows(2).any(|w| w[0].contains("main → docs") && w[1] == format!("  {text}"))
}

const SEND: &str = r#"sb send docs "use v2""#;
const SENT: &str = "sent m_12 to docs (delivered, thread t_12)";
const MSG: &str = "sb msg : main → docs m_12 : use v2";

/// One `sb send`, its message drawn: the box hides, the chip stays;
/// the line may come before or after the result.
#[test]
fn a_lone_send_hides_its_box() {
    for at in [3, 5] {
        let (ev, mut cache) = feed(&with_msg(bash(1, SEND, SENT, true), at, MSG));
        let rows = screen(&ev, &mut cache);
        assert!(!has_box(&rows) && has_chip(&rows, "use v2"), "{at}: {rows:#?}");
        // the reply above, one blank row, the chip: no hole for the box
        assert!(rows[0].trim() == "sending it" && rows[1].is_empty(), "{rows:#?}");
    }
}

/// Any flags, and one leading `cd <dir> &&` (quotes allowed), still hide.
#[test]
fn flags_and_one_cd_still_hide() {
    for script in [
        r#"sb send docs --expect-reply --reply-to m_3 "use v2""#,
        r#"cd /Users/g/lab/harness && sb send docs "use v2""#,
        r#"cd '/tmp/my wt' && sb send docs --mode queued 'use v2'"#,
        "sb send docs \\\n  \"use v2\"",
    ] {
        let (ev, mut cache) = feed(&with_msg(bash(1, script, SENT, true), 3, MSG));
        assert!(!has_box(&screen(&ev, &mut cache)), "{script}");
    }
}

/// Anything else in the script keeps the box.
#[test]
fn anything_else_keeps_the_box() {
    for script in [
        r#"cd x; sb send docs "use v2""#,
        r#"cd x && sb send docs "use v2" | cat"#,
        r#"echo hi && sb send docs "use v2""#,
        r#"cd x && cd y && sb send docs "use v2""#,
        r#"sb send docs "use v2" && sb list"#,
        r#"sb send docs "$(cat notes.md)""#,
        r#"sb send docs "use v2" > /tmp/out"#,
        r#"sb send docs "use v2" &"#,
        "sb send docs \"use v2\"\nsb list",
        r#"FOO=1 sb send docs "use v2""#,
        r#"sb send docs "use v2"#,
        r#"sb send docs `cat notes`"#,
        r#"sb spawn docs --objective "use v2""#,
    ] {
        let (ev, mut cache) = feed(&with_msg(bash(1, script, SENT, true), 3, MSG));
        let rows = screen(&ev, &mut cache);
        assert!(has_box(&rows) && has_chip(&rows, "use v2"), "{script}: {rows:#?}");
    }
}

/// A failure, a message not drawn (none, another id, another feed's),
/// an output with more in it, and `sb list` keep the box.
#[test]
fn a_failure_or_no_drawn_message_keeps_the_box() {
    let cases = [
        with_msg(bash(1, SEND, "error: agent inconnu : docs", false), 3, MSG),
        with_msg(bash(1, SEND, SENT, false), 3, MSG),
        bash(1, SEND, SENT, true),
        with_msg(bash(1, SEND, SENT, true), 3, "sb msg : main → docs m_13 : use v2"),
        with_msg(bash(1, SEND, &format!("{SENT}   warning: slow hub"), true), 3, MSG),
        with_msg(bash(1, SEND, "sent m_12 to docs (delivered, thread t_12) (again)", true), 3, MSG),
        with_msg(bash(1, "sb list", "main idle", true), 3, MSG),
    ];
    for (n, lines) in cases.iter().enumerate() {
        let (ev, mut cache) = feed(lines);
        assert!(has_box(&screen(&ev, &mut cache)), "case {n}");
    }
    // the message before the box is not "below it"
    let mut lines = vec![MSG.to_string()];
    lines.extend(bash(1, SEND, SENT, true));
    let (ev, mut cache) = feed(&lines);
    assert!(has_box(&screen(&ev, &mut cache)));
}

/// `sb ask`: the question and the reply both drawn hide it; the question
/// alone (the reply is not in this feed) keeps it.
#[test]
fn an_ask_hides_when_question_and_reply_show() {
    let ask = r#"sb ask docs "v1 or v2?""#;
    let out = "reply from docs (m_13, answers m_12):\\nv2";
    let q = "sb msg : main → docs m_12 : v1 or v2?";
    let r = "sb msg-in : docs m_13 : v2";
    let mut lines = with_msg(bash(1, ask, out, true), 3, q);
    lines.insert(4, r.to_string());
    let (ev, mut cache) = feed(&lines);
    assert!(!has_box(&screen(&ev, &mut cache)));
    let (ev, mut cache) = feed(&with_msg(bash(1, ask, out, true), 3, q));
    assert!(has_box(&screen(&ev, &mut cache)));
    // an older CLI's output has no question id: the box stays
    let mut lines = with_msg(bash(1, ask, "reply from docs (m_13):\\nv2", true), 3, q);
    lines.insert(4, r.to_string());
    let (ev, mut cache) = feed(&lines);
    assert!(has_box(&screen(&ev, &mut cache)));
}

/// `sb report` from a task: its message is not drawn in its own feed.
#[test]
fn a_report_in_a_tasks_feed_keeps_its_box() {
    let (ev, mut cache) = feed(&bash(1, r#"sb report done "all good""#, "reported (m_20)", true));
    assert!(has_box(&screen(&ev, &mut cache)));
}

/// ctrl+o opens everything: the hidden box shows, then hides again.
#[test]
fn ctrl_o_shows_the_hidden_box() {
    let (mut ev, mut cache) = feed(&with_msg(bash(1, SEND, SENT, true), 3, MSG));
    assert!(anything_closed(&ev));
    set_everything(&mut ev, &mut cache, true);
    let rows = screen(&ev, &mut cache);
    assert!(has_box(&rows) && has_chip(&rows, "use v2"), "{rows:#?}");
    set_everything(&mut ev, &mut cache, false);
    assert!(!has_box(&screen(&ev, &mut cache)));
}

/// Hidden boxes are not in the way: main's sends to one agent stack.
#[test]
fn hidden_sends_stack() {
    let mut lines = with_msg(bash(1, SEND, SENT, true), 3, MSG);
    lines.extend(with_msg(
        bash(2, r#"sb send docs "and the api""#, "sent m_14 to docs (delivered, thread t_12)", true),
        3,
        "sb msg : main → docs m_14 : and the api",
    ));
    let (ev, mut cache) = feed(&lines);
    let rows = screen(&ev, &mut cache);
    let a = rows.iter().position(|r| r.contains("use v2")).unwrap();
    assert!(!has_box(&rows) && rows[a + 1].contains("and the api"), "{rows:#?}");
}
