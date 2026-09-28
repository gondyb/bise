//! Wire line -> feed rows, end to end: parsing, merging, code blocks,
//! wrapping and display widths.

use super::*;
use crate::code::*;
use crate::editor::layout_input;
use unicode_width::UnicodeWidthStr;

#[test]
fn provider_retry_and_restart_render() {
    match parse_line("  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s") {
        Some(Ev::Warn(t)) => assert_eq!(
            t,
            "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s"
        ),
        _ => panic!("provider_retry must render as a warning"),
    }
    match parse_line("  obs: harness_restarted: exit status: 1 · bend: out of memory") {
        Some(Ev::Err(t)) => assert!(t.contains("bend: out of memory") && t.contains("restarted")),
        _ => panic!("harness_restarted must render as an error"),
    }
}

#[test]
fn split_thinking_takes_every_span() {
    let t = "<think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok";
    let (think, vis) = split_thinking(t).unwrap();
    assert_eq!(think, "a\\nBENDSIG::s1\\nb\\nBENDSIG::s2");
    assert_eq!(vis, "ok");
    assert!(split_thinking("no markers").is_none());
    assert!(split_thinking("<think>open").is_none());
}

// the runtime's wire encoding, as emit_code_ann produces it
fn wire_encode(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\r', "\\R")
        .replace('\n', "\\N")
}

// proper JSON args, as the model emits them: quotes and backslashes
// escaped inside the code string, newlines as \n
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// the feed receives tool_started, the tool annotations, then
// tool_finished: the block must render under the merged line
#[test]
fn tool_code_block_renders_under_the_tool_line() {
    let code = "// orchestre les appels
async function main(): Promise<unknown> {
  const xs = [1, 2, 3];
  const s = \"une chaîne\";
  return xs.length + s.length;
}";
    let args = format!("{{\"code\": \"{}\"}}", json_escape(code));
    let wire_lines = vec![
        "  obs: tool_started #3".to_string(),
        format!("tool #3 run_typescript : {}", args),
        format!("tool_code #3 : {}", wire_encode(&args)),
        "  obs: tool_finished #3 ok".to_string(),
    ];
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in &wire_lines {
        let ev = parse_line(l).expect("parse");
        push_event(&mut events, &mut cache, ev);
    }
    let tool = events
        .iter()
        .find_map(|e| match e {
            Ev::Tool(td) => Some(td.clone()),
            _ => None,
        })
        .expect("the merged tool");
    assert_eq!(tool.id, 3);
    assert_eq!(tool.name.as_deref(), Some("run_typescript"));
    assert!(tool.code.is_some());
    let rows = ev_lines(&Ev::Tool(tool), 80);
    let joined: Vec<String> = rows
        .iter()
        .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
        .collect();
    for (i, l) in joined.iter().enumerate() {
        println!("{:2} | {}", i, l);
    }
    assert!(joined[0].contains("run_typescript") && !joined[0].contains("orchestre"));
    assert!(joined.iter().any(|l| l.starts_with(" │ async function main")));
    assert!(joined.iter().any(|l| l.contains("une chaîne")));
    // one row per source line, behind the rail
    assert_eq!(joined.iter().filter(|l| l.starts_with(" │ ")).count(), 6);
}

fn merged_tool(wire_lines: &[String]) -> ToolData {
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in wire_lines {
        let ev = parse_line(l).expect("parse");
        push_event(&mut events, &mut cache, ev);
    }
    events
        .iter()
        .find_map(|e| match e {
            Ev::Tool(td) => Some(td.clone()),
            _ => None,
        })
        .expect("the merged tool")
}

fn rows_text(rows: &[Line<'static>]) -> Vec<String> {
    rows.iter()
        .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
        .collect()
}

// BISE-11: a script always shows whole (book §11), whatever its length
#[test]
fn a_long_script_renders_whole() {
    let cmd: String = (1..=200).map(|i| format!("echo {}", i)).collect::<Vec<_>>().join("\n");
    let bash = merged_tool(&[
        "  obs: tool_started #4".to_string(),
        "tool #4 bash : echo".to_string(),
        format!("tool_code #4 : {}", wire_encode(&cmd)),
        "  obs: tool_finished #4 ok".to_string(),
    ]);
    let rows = rows_text(&ev_lines(&Ev::Tool(bash), 80));
    assert_eq!(rows.iter().filter(|l| l.starts_with(" │ echo")).count(), 200);
    assert!(rows.iter().any(|l| l == " │ echo 200"));
    assert!(!rows.iter().any(|l| l.contains("more lines")));
    // a TypeScript program too
    let src: String = (1..=200).map(|i| format!("const x{i} = {i};")).collect::<Vec<_>>().join("\n");
    let args = format!("{{\"code\": \"{}\"}}", json_escape(&src));
    let ts = merged_tool(&[
        "  obs: tool_started #5".to_string(),
        format!("tool #5 run_typescript : {}", args),
        format!("tool_code #5 : {}", wire_encode(&args)),
        "  obs: tool_finished #5 ok".to_string(),
    ]);
    let rows = rows_text(&ev_lines(&Ev::Tool(ts), 80));
    assert_eq!(rows.iter().filter(|l| l.starts_with(" │ const x")).count(), 200);
}

// other code (a patch, however long) stays behind its ▸ until opened,
// then shows whole (BISE-12: the edit line is the fold)
#[test]
fn a_long_patch_folds_until_clicked() {
    let body: String = (1..=200).map(|i| format!("+line {}", i)).collect::<Vec<_>>().join("\n");
    let patch = format!("*** Begin Patch\n*** Add File: big.txt\n{}\n*** End Patch\n", body);
    let mut tool = merged_tool(&[
        "  obs: tool_started #6".to_string(),
        "tool #6 apply_patch : big".to_string(),
        format!("tool_code #6 : {}", wire_encode(&patch)),
        "  obs: tool_finished #6 ok".to_string(),
    ]);
    let folded = rows_text(&ev_lines(&Ev::Tool(tool.clone()), 80));
    assert_eq!(folded, vec![" ± edit big.txt ✓ +200 ▸".to_string()]);
    tool.expanded = true;
    let whole = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    assert_eq!(whole.iter().filter(|l| l.contains("│ +line")).count(), 200);
}

#[test]
fn bash_code_block_renders_under_the_tool_line() {
    let cmd = "# compte les fichiers
for f in *.rs; do
  echo \"$f: $(wc -l < \"$f\")\"
done | sort -n";
    let wire_lines = vec![
        "  obs: tool_started #4".to_string(),
        format!("tool #4 bash : {}", cmd.replace('\n', " ")),
        format!("tool_code #4 : {}", wire_encode(cmd)),
        "  obs: tool_finished #4 ok".to_string(),
    ];
    let tool = merged_tool(&wire_lines);
    assert_eq!(tool.name.as_deref(), Some("bash"));
    let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    for (i, l) in joined.iter().enumerate() {
        println!("{:2} | {}", i, l);
    }
    // the tool line names the tool; the source shows only in the block
    assert!(joined[0].contains("bash") && !joined[0].contains("compte"));
    assert!(joined.iter().any(|l| l.contains("for f in *.rs; do")));
    assert!(joined.iter().any(|l| l.contains("done | sort -n")));
    // one row per source line (4 lines), behind the rail
    assert_eq!(joined.iter().filter(|l| l.starts_with(" │ ")).count(), 4);
}

// a bash tool line without tool_code (an older runtime) keeps the
// flattened args preview and renders no block
#[test]
fn bash_without_code_has_no_block() {
    let tool = merged_tool(&[
        "  obs: tool_started #5".to_string(),
        "tool #5 bash : ls -la".to_string(),
        "  obs: tool_finished #5 ok".to_string(),
    ]);
    let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    assert!(joined[0].contains("ls -la"));
    assert!(!joined.iter().any(|l| l.starts_with(" │ ")));
}

fn style_of(lines: &[Vec<Span<'static>>], tok: &str) -> Style {
    lines
        .iter()
        .flatten()
        // adjacent same-style spans merge (whitespace included)
        .find(|s| s.content.trim() == tok)
        .map(|s| s.style)
        .unwrap_or_else(|| panic!("no span {:?}", tok))
}

// the lines LAWS.resume_replays_history pins on the Bend side: the
// client rebuilds the conversation, tools merged, no fake duration
#[test]
fn resume_history_rebuilds_the_feed() {
    let wire = [
        "history   obs: compaction_done: court...",
        "history you : salut\\nça va",
        "history   obs: tool_started #7",
        "history tool #7 bash : ls -la",
        "history tool_code #7 : ls -la",
        "history tool_result #7 fail : boom",
        "history   obs: tool_finished #7 failed",
        "history   obs: assistant: fini",
        "history   obs: tool_started #9",
        "history tool #9 bash : pwd",
        "history tool_code #9 : pwd",
        "history   obs: tool_finished #9 failed",
        "history injected : [notification] bg 0 done",
        "  obs: session_restored: 7 messages",
    ];
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in wire {
        let (line, replayed) = strip_history(l);
        let ev = if replayed { parse_history_line(line) } else { parse_line(line) };
        let Some(ev) = ev else { continue };
        let finished = match &ev {
            Ev::Tool(td) if !matches!(td.state, ToolState::Run) => Some(td.id),
            _ => None,
        };
        push_event(&mut events, &mut cache, ev);
        if let (true, Some(id)) = (replayed, finished) {
            hide_replayed_elapsed(&mut events, &mut cache, id);
        }
    }
    let kinds: Vec<String> = events
        .iter()
        .map(|e| match e {
            Ev::Compacted(t) => format!("compacted {}", t),
            Ev::You(t) => format!("you {}", t),
            Ev::Assistant(t) => format!("assistant {}", t),
            Ev::Tool(td) => format!(
                "tool {} {} ok={} code={} elapsed={:?}",
                td.id,
                td.name.clone().unwrap_or_default(),
                matches!(td.state, ToolState::Ok),
                td.code.is_some(),
                td.elapsed
            ),
            Ev::Info(t) => format!("info {}", t),
            _ => "other".to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "compacted court...".to_string(),
            "you salut\nça va".to_string(),
            "tool 7 bash ok=false code=true elapsed=Some(\"\")".to_string(),
            "assistant fini".to_string(),
            "tool 9 bash ok=false code=true elapsed=Some(\"\")".to_string(),
            "info injected · [notification] bg 0 done".to_string(),
            "info session restored · 7 messages".to_string(),
        ]
    );
}

// a long line wraps under the rail with a hanging indent and a faint
// ↪: nothing is lost, no row is wider than the block, the continuation
// rows join the row before when copied
#[test]
fn long_code_lines_wrap_with_a_hanging_indent() {
    let cmd = "cd /Users/someone/lab/project && cargo test -p some-crate --release 2>&1 | rg 'test result|FAILED|panicked' | head -20\necho ok";
    let lines = code_block_lines(cmd, CodeLang::Bash, &ToolState::Ok, 50);
    let rows = rows_text(&lines);
    for r in &rows {
        println!("{}", r);
    }
    assert!(rows.iter().all(|r| r.width() <= 50), "{rows:#?}");
    // 2 source lines, the first on several rows
    assert!(rows.len() > 3);
    assert!(rows[0].starts_with(" │ cd "));
    assert!(rows[1].starts_with(" │ ↪ "), "{rows:#?}");
    assert!(crate::feedsel::is_soft(&lines[1]) && !crate::feedsel::is_soft(&lines[0]));
    assert_eq!(rows.last().unwrap(), " │ echo ok");
    // the text after the rail and the wrap marks reassembles the source
    let body: String = rows
        .iter()
        .map(|r| {
            let t = r.strip_prefix(" │ ").unwrap();
            t.strip_prefix("↪ ").unwrap_or(t).to_string()
        })
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(body.replace(' ', ""), cmd.replace(['\n', ' '], ""));
    // the rail and the wrap mark are faint
    let faint = Some(crate::theme::faint());
    assert_eq!(lines[1].spans[0].style.fg, faint);
    assert_eq!(lines[1].spans[1].style.fg, faint);
}

// a token longer than the row breaks hard, never overflows
#[test]
fn wrap_breaks_long_tokens() {
    let spans = vec![Span::raw("x".repeat(25))];
    let rows = wrap_code_line_hanging(&spans, 10, 10);
    let ws: Vec<usize> = rows.iter().map(|(_, w)| *w).collect();
    assert_eq!(ws, vec![10, 10, 5]);
    assert_eq!(wrap_code_line_hanging(&[], 10, 10).len(), 1);
    // continuation rows get their own, narrower width
    let ws: Vec<usize> = wrap_code_line_hanging(&spans, 10, 8).iter().map(|(_, w)| *w).collect();
    assert_eq!(ws, vec![10, 8, 7]);
}

// apply_patch renders as a diff: summary in the tool line, the
// envelope dropped, file headers, colored bands to the border
#[test]
fn apply_patch_renders_a_diff_block() {
    let patch = "*** Begin Patch
*** Update File: core/obs.bend
@@ def show_obs
   case T.Assistant{text}:
-    old line
+    new line
+    another line
*** Add File: notes.md
+# Notes
*** End Patch
";
    let tool = merged_tool(&[
        "  obs: tool_started #6".to_string(),
        format!("tool #6 apply_patch : {}", patch.replace('\n', " ")),
        format!("tool_code #6 : {}", wire_encode(patch)),
        "tool_result #6 ok : Done!".to_string(),
        "  obs: tool_finished #6 ok".to_string(),
    ]);
    let mut tool = tool;
    tool.expanded = true;
    let lines = ev_lines(&Ev::Tool(tool), 70);
    let rows = rows_text(&lines);
    for r in &rows {
        println!("{}", r);
    }
    assert_eq!(rows[0], " ± edit 2 files ✓ +3 −1 ▾");
    assert_eq!(patch_summary(&wire_decode(&wire_encode(patch))), "core/obs.bend +2 −1, notes.md +1");
    assert!(!rows.iter().any(|r| r.contains("Begin Patch") || r.contains("End Patch")));
    assert!(rows.iter().any(|r| r.starts_with(" │ ~ core/obs.bend")));
    assert!(rows.iter().any(|r| r.starts_with(" │ + notes.md · new")));
    // no line-number gutter in a diff
    assert!(rows.iter().any(|r| r.starts_with(" │ -    old line")));
}

#[test]
fn bash_highlighting_classifies_tokens() {
    let hl = highlight_bash(
        "# note
X=1 grep -n \"$HOME\" f.txt | wc -l && for i in 1 2; do echo $i; done",
    );
    let fg = |tok: &str| style_of(&hl, tok).fg;
    use crate::theme::{syntax_call, syntax_comment, syntax_keyword, syntax_number, syntax_type, text};
    assert_eq!(fg("# note"), Some(syntax_comment()));
    assert_eq!(fg("X"), Some(syntax_number())); // assignment name
    assert_eq!(fg("grep"), Some(syntax_call())); // command word
    assert_eq!(fg("-n"), Some(syntax_type())); // option
    assert_eq!(fg("$HOME"), Some(syntax_number())); // expansion in quotes
    assert_eq!(fg("f.txt"), Some(text())); // argument
    assert_eq!(fg("wc"), Some(syntax_call())); // command after a pipe
    assert_eq!(fg("for"), Some(syntax_keyword()));
    assert_eq!(fg("in"), Some(syntax_keyword()));
    assert_eq!(fg("do"), Some(syntax_keyword()));
    assert_eq!(fg("echo"), Some(syntax_call())); // command after "do"
    assert_eq!(fg("done"), Some(syntax_keyword()));
    assert_eq!(hl.len(), 2); // one span list per source line
}

// ---- display widths (emoji are 2 columns) ----

fn row_widths(input: &str, inner: usize) -> Vec<usize> {
    layout_input(input, inner)
        .iter()
        .map(|r| r.iter().filter(|c| !c.newline).map(|c| c.w).sum())
        .collect()
}

#[test]
fn composer_rows_count_emojis_as_two_columns() {
    // 5 emojis at 6 columns: 3 per row, the 4th never splits a row
    assert_eq!(row_widths("👏👏👏👏👏", 6), vec![6, 4]);
    // an emoji that does not fit the row end moves to the next row
    assert_eq!(row_widths("abcde👏", 6), vec![5, 2]);
    for inner in 2..12 {
        for input in ["a👏b👍🏽c❤️d👨‍👩‍👧e🇫🇷", "👏👏👏👏👏👏👏", "x y 👏👏 z\nq👏"] {
            assert!(row_widths(input, inner).iter().all(|&w| w <= inner), "{input} @ {inner}");
        }
    }
}

#[test]
fn composer_cells_are_graphemes_with_char_indices() {
    let rows = layout_input("a👍🏽❤️👨‍👩‍👧b", 40);
    let cells: Vec<(usize, &str, usize)> = rows[0].iter().map(|c| (c.ci, c.text, c.w)).collect();
    assert_eq!(
        cells,
        vec![(0, "a", 1), (1, "👍🏽", 2), (3, "❤️", 2), (5, "👨‍👩‍👧", 2), (10, "b", 1), (11, " ", 1)]
    );
    // the end cursor slot sits after the last char
    assert!(rows[0].last().unwrap().newline);
}

#[test]
fn feed_wrap_counts_graphemes_as_drawn() {
    let w = |l: &Line| l.spans.iter().map(|s| s.content.width()).sum::<usize>();
    let rows = wrap_line(Line::from("👨‍👩‍👧 👨‍👩‍👧 👨‍👩‍👧"), 8);
    assert_eq!(rows.len(), 1, "3 × 2 cols + 2 spaces fit 8");
    let rows = wrap_line(Line::from("👏👏👏👏👏"), 4);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| w(r) <= 4));
    let rows = wrap_code_line_hanging(&[Span::raw("❤️❤️❤️")], 4, 4);
    assert_eq!(rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec![4, 2]);
}

// ---- the measure (BISE-10, book §11) ----

fn feed_rows(events: &[Ev], width: usize) -> Vec<Vec<String>> {
    (0..events.len())
        .map(|i| rows_text(&build_rows(events, i, false, width, 0)))
        .collect()
}

fn measure_feed() -> Vec<Ev> {
    let prose = "the login breaks on safari because the session cookie is set with SameSite=None and without Secure, so the browser drops it on the redirect and the user lands on the sign-in page again. ".repeat(2);
    let long_line = format!("cargo test -p bend-tui --release -- {} --nocapture", "feed_render ".repeat(14));
    let tool = merged_tool(&[
        "  obs: tool_started #3".to_string(),
        "tool #3 bash : cargo".to_string(),
        format!("tool_code #3 : {}", wire_encode(&format!("cd rust\n{}", long_line))),
        "  obs: tool_finished #3 ok".to_string(),
    ]);
    vec![
        Ev::You(prose.clone()),
        Ev::Assistant(prose.clone()),
        Ev::AgentMsg { from: "docs".into(), to: "main".into(), text: prose.clone(), level: 3, id: String::new(), open: false },
        Ev::Thinking { ms: 1200, text: prose.clone(), open: true },
        Ev::Tool(tool),
    ]
}

#[test]
fn prose_wraps_at_76_and_code_at_100() {
    for width in [60usize, 100, 160] {
        let events = measure_feed();
        let rows = feed_rows(&events, width);
        let widest = |rs: &[String]| rs.iter().map(|r| r.trim_end().width()).max().unwrap_or(0);
        // prose: the user block, the reply, the message, the reasoning
        for (i, rs) in rows[..4].iter().enumerate() {
            let w = widest(rs);
            assert!(w <= 76.min(width), "event {i} at {width}: {w} columns\n{rs:#?}");
            // the text fills its measure: it is not wrapped narrower
            assert!(w >= 76.min(width) - 12, "event {i} at {width}: only {w} columns\n{rs:#?}");
        }
        // the user block's panel stops at the measure too (a wrapped
        // row keeps the blank after its last word, as before)
        assert!(rows[0].iter().all(|r| r.trim_end().width() <= 76.min(width) && r.width() <= 77.min(width + 1)), "{width}: {:#?}", rows[0]);
        // code: the tool line and its script
        let code = &rows[4];
        let w = widest(code);
        assert!(w <= 100.min(width), "code at {width}: {w}\n{code:#?}");
        if width > 100 {
            // the long script line uses the whole code measure
            assert!(w >= 90, "code at {width}: {w}\n{code:#?}");
        }
        assert!(code.iter().any(|r| r.starts_with(" │ ↪ ")), "code at {width}: {code:#?}");
    }
}

// ---- progressive disclosure (BISE-12, book §11) ----

fn tool_with(id: u32, name: &str, code: Option<&str>, result: Option<(bool, &str)>, ok: bool) -> ToolData {
    let mut lines = vec![format!("  obs: tool_started #{id}"), format!("tool #{id} {name} : x")];
    if let Some(c) = code {
        lines.push(format!("tool_code #{id} : {}", wire_encode(c)));
    }
    if let Some((rok, r)) = result {
        lines.push(format!("tool_result #{id} {} : {r}", if rok { "ok" } else { "fail" }));
    }
    lines.push(format!("  obs: tool_finished #{id} {}", if ok { "ok" } else { "failed" }));
    merged_tool(&lines)
}

fn text_of(ev: &Ev, width: usize) -> Vec<String> {
    rows_text(&ev_rows(ev, 0, width))
}

#[test]
fn an_output_is_one_line_until_opened() {
    let out = "running 12 tests  test login ... FAILED  1 failed, 11 passed (6.1s)";
    let mut td = tool_with(3, "bash", Some("npx playwright test"), Some((true, out)), true);
    let closed = text_of(&Ev::Tool(td.clone()), 100);
    // the script in full, then one line for the output
    assert!(closed.iter().any(|r| r == " │ npx playwright test"), "{closed:#?}");
    assert_eq!(closed.last().unwrap(), "   ▸ output · 1 failed", "{closed:#?}");
    assert!(!closed.iter().any(|r| r.contains("11 passed")));
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert!(open.iter().any(|r| r == "   ▾ output"), "{open:#?}");
    assert!(open.iter().any(|r| r.starts_with(" │ running 12 tests") && r.contains("11 passed")), "{open:#?}");
    // no count when the output does not say
    let td = tool_with(4, "search", None, Some((true, "3 results")), true);
    assert_eq!(text_of(&Ev::Tool(td), 100).last().unwrap(), "   ▸ output");
}

#[test]
fn a_failure_is_one_line_with_its_reason() {
    let reason = "error[E0425]: cannot find value `width` in this scope";
    let long = format!("{reason}  {}", "--> tui/src/feed.rs:12:5 ".repeat(8));
    let mut td = tool_with(5, "bash", Some("cargo test"), Some((false, &long)), false);
    let closed = text_of(&Ev::Tool(td.clone()), 100);
    let last = closed.last().unwrap();
    assert!(last.starts_with(&format!("   {reason}")) && last.ends_with("… ▸"), "{closed:#?}");
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert!(open.iter().any(|r| r == "   ▾ output"), "{open:#?}");
    assert!(open.iter().any(|r| r.contains("feed.rs:12:5")));
    // a short reason: nothing more to show, no ▸
    let td = tool_with(6, "bash", Some("false"), Some((false, "exit 1")), false);
    assert_eq!(text_of(&Ev::Tool(td), 100).last().unwrap(), "   exit 1");
}

#[test]
fn an_edit_is_one_line_its_diff_when_opened() {
    let patch = "*** Begin Patch\n*** Update File: web/src/auth/session.ts\n@@\n   cookie: {\n-    sameSite: \"none\",\n+    sameSite: \"none\",\n+    secure: true,\n*** End Patch\n";
    let mut td = tool_with(7, "apply_patch", Some(patch), Some((true, "Done!")), true);
    let closed = text_of(&Ev::Tool(td.clone()), 100);
    assert_eq!(closed, vec![" ± edit web/src/auth/session.ts ✓ +2 −1 ▸".to_string()]);
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert_eq!(open[0], " ± edit web/src/auth/session.ts ✓ +2 −1 ▾");
    assert!(open.iter().any(|r| r == " │ +    secure: true,"), "{open:#?}");
    // several files; a failure gives its reason
    let two = "*** Begin Patch\n*** Update File: a.rs\n+x\n*** Add File: b.rs\n+y\n*** End Patch\n";
    let td = tool_with(8, "apply_patch", Some(two), Some((false, "the patch doesn't apply: context not found")), false);
    assert_eq!(
        text_of(&Ev::Tool(td), 100),
        vec![" ± edit 2 files ✗ the patch doesn't apply: context not found ▸".to_string()]
    );
}

fn agent_msg(from: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: String::new(), text: text.into(), level: 3, id: "m_3".into(), open: false }
}

#[test]
fn a_report_is_one_line_until_opened() {
    let mut ev = agent_msg("bench", "[report: done] p95 at 180 ms, nothing to fix.\n- ran 3 times on staging\n- p99 410 ms");
    let closed: Vec<String> = text_of(&ev, 100).into_iter().filter(|r| !r.is_empty()).collect();
    assert_eq!(closed, vec![" ♡ bench: p95 at 180 ms, nothing to fix. ▸ report".to_string()]);
    let mut cache: Vec<Option<EventRows>> = vec![None];
    assert!(toggle_event(std::slice::from_mut(&mut ev), &mut cache, 0));
    let open: Vec<String> = text_of(&ev, 100).into_iter().filter(|r| !r.is_empty()).collect();
    assert_eq!(open[0], " ♡ bench: p95 at 180 ms, nothing to fix. ▾ report");
    assert!(open.iter().any(|r| r.starts_with(" │ ") && r.contains("p99 410 ms")), "{open:#?}");
    // the kind gives the glyph; a one-line report has no ▸
    let failed = text_of(&agent_msg("deploy", "[report: failed] the staging token expired."), 100);
    assert!(failed.iter().any(|r| r == " ✗ deploy: the staging token expired."), "{failed:#?}");
    let blocked = text_of(&agent_msg("docs", "[report: blocked] it needs the v2 schema file."), 100);
    assert!(blocked.iter().any(|r| r.starts_with(" ? docs: ")), "{blocked:#?}");
    // a long summary is cut to one line, the ▸ shows the rest
    let long = text_of(&agent_msg("docs", &format!("[report: done] {}", "word ".repeat(40))), 76);
    let row = long.iter().find(|r| !r.is_empty()).unwrap();
    assert!(row.ends_with("… ▸ report") && row.width() <= 76, "{long:#?}");
}

#[test]
fn the_brief_is_one_line_until_opened() {
    let mut ev = agent_msg("main", "# Task `auth-fix`\n\nObjective: the login breaks on safari 18.");
    assert_eq!(text_of(&ev, 100), vec![" ◇ brief ▸".to_string()]);
    let mut cache: Vec<Option<EventRows>> = vec![None];
    toggle_event(std::slice::from_mut(&mut ev), &mut cache, 0);
    let open = text_of(&ev, 100);
    assert_eq!(open[0], " ◇ brief ▾");
    assert!(open.iter().any(|r| r == " │ Objective: the login breaks on safari 18."), "{open:#?}");
    assert!(!open.iter().any(|r| r.contains("# Task")));
}

#[test]
fn toggles_one_item_and_all_outputs() {
    let mut app = crate::sb::bench::test_app();
    let out = tool_with(1, "bash", Some("ls"), Some((true, "a b c")), true);
    let quiet = tool_with(2, "bash", Some("true"), None, true);
    app.events = vec![
        Ev::Tool(out.clone()),
        Ev::Thinking { ms: 10, text: "hm".into(), open: false },
        Ev::Tool(out),
        Ev::Tool(quiet),
        Ev::Info("x".into()),
    ];
    app.cache = (0..5).map(|_| None).collect();
    let expanded = |app: &App| -> Vec<bool> {
        app.events.iter().filter_map(|e| match e { Ev::Tool(td) => Some(td.expanded), _ => None }).collect()
    };
    // no selection: nothing to toggle
    assert!(!toggle_selected(&mut app));
    // the selection on the thinking section opens it
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (1, 0, 0), head: (1, 0, 3) });
    assert!(toggle_selected(&mut app));
    assert!(matches!(app.events[1], Ev::Thinking { open: true, .. }));
    // on an output
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 0) });
    ensure_rows(&app.events, &mut app.cache, 0, false, 80, 0);
    assert!(toggle_selected(&mut app));
    assert_eq!(expanded(&app), vec![true, false, false]);
    assert!(app.cache[0].is_none(), "its rows rebuild");
    // nothing behind a notice or a tool without output
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (4, 0, 0), head: (4, 0, 0) });
    assert!(!toggle_selected(&mut app));
    assert!(!toggle_event(&mut app.events, &mut app.cache, 3));
    // all outputs: one is closed, so all open; again, all close
    assert!(toggle_all_outputs(&mut app));
    assert_eq!(expanded(&app), vec![true, true, false]);
    assert!(!toggle_all_outputs(&mut app));
    assert_eq!(expanded(&app), vec![false, false, false]);
    // thinking is ctrl+t's, untouched
    assert!(matches!(app.events[1], Ev::Thinking { open: true, .. }));
}

#[test]
fn fit_chars_keeps_the_ellipsis_inside() {
    assert_eq!(crate::render::fit_chars("abcdef", 6), "abcdef");
    assert_eq!(crate::render::fit_chars("abcdefg", 6), "abcde…");
}
