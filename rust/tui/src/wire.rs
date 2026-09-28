//! The agent wire protocol as feed events: `Ev` and the parsers of the
//! runtime lines (live and replayed history).

use crate::{fmt_elapsed, sb, truncate_chars, unescape_md, usage};

// ---- feed events ----

#[derive(Clone)]
pub(crate) enum ToolState {
    Run,
    Ok,
    Fail,
}

// one tool call: enriched by the runtime annotations (name/args/result)
#[derive(Clone)]
pub(crate) struct ToolData {
    pub(crate) id: u32,
    pub(crate) name: Option<String>,
    pub(crate) args: Option<String>,
    // the source of a code tool (run_typescript args JSON, bash raw
    // command), wire-encoded (tool_code annotation); None otherwise
    pub(crate) code: Option<String>,
    pub(crate) state: ToolState,
    pub(crate) result: Option<(bool, String)>,
    pub(crate) started: std::time::Instant,
    // frozen at finish; None while running (elapsed ticks live)
    pub(crate) elapsed: Option<String>,
    // a long source block shows whole (a click on the tool toggles it)
    pub(crate) expanded: bool,
}

impl ToolData {
    /// A tool event as the runtime announces it (started or finished),
    /// before its annotations (name, args, code, result) merge in. Only
    /// a running tool keeps its elapsed live.
    pub(crate) fn bare(id: u32, state: ToolState) -> ToolData {
        let started = std::time::Instant::now();
        let elapsed = match state {
            ToolState::Run => None,
            _ => Some(fmt_elapsed(started)),
        };
        ToolData { id, name: None, args: None, code: None, state, result: None, started, elapsed, expanded: false }
    }
}

#[derive(Clone)]
pub(crate) enum Ev {
    You(String),
    Assistant(String),
    // the model's reasoning for the message that follows: rendered
    // collapsed as "thought for Ns"; ctrl+t expands every section, a
    // click on the section toggles just that one
    Thinking {
        ms: u128,
        text: String,
        open: bool,
    },
    Tool(ToolData),
    // a sub-call made inside a run_typescript program
    Sub {
        name: String,
        ok: bool,
        preview: String,
    },
    // runtime annotations, merged into the matching Tool by id
    ToolInfo {
        id: u32,
        name: String,
        args: String,
    },
    ToolResult {
        id: u32,
        ok: bool,
        preview: String,
    },
    // the source of a code tool (tool_code annotation: run_typescript,
    // bash): the FULL args, wire-encoded, merged into the matching Tool
    // by id
    ToolCode {
        id: u32,
        code: String,
    },
    Turn,
    TurnDone,
    Compact(String),
    Compacted(String),
    // token usage of the last model call (hidden; feeds the status row)
    Usage(usage::Usage),
    Warn(String),
    Err(String),
    Info(String),
    Idle,
    Raw(String),
    // switchboard (hub line protocol v2, contract C2): a message between
    // agents. `to` empty: the owner of the feed it is in (v1 `msg-in`).
    // `level`: 3 between agents (`msg`, `msg-in`), 2 an agent writing to
    // the user (`msg-you`, to = "you"). `id`: the message id (`m_3`)
    // when the line carries one (`msg-in`), else empty.
    AgentMsg {
        from: String,
        to: String,
        text: String,
        level: u8,
        id: String,
        // in memory only (not on the wire): a folded message (a report,
        // a brief) is disclosed (BISE-12, a click or feed::toggle_event)
        open: bool,
    },
    // switchboard (C2 `answered`): main answered an agent's question for
    // the user (level 2); `why` may be empty.
    Answered {
        agent: String,
        question: String,
        answer: String,
        why: String,
    },
    // switchboard: an attention card
    Card(String),
}

// the wire carries the model's reasoning wrapped in think markers inside
// the assistant text (the transport the API re-send depends on); the TUI
// never shows the markers: it splits them into a Thinking section
pub(crate) const THINK_START: &str = "<think>";
pub(crate) const THINK_END: &str = "</think>";

pub(crate) fn split_thinking(s: &str) -> Option<(String, String)> {
    // one reply can carry several thinking blocks: every span joins the
    // section, the text around them stays visible
    let mut think: Vec<&str> = Vec::new();
    let mut visible = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(THINK_START) {
        let after = &rest[start + THINK_START.len()..];
        let Some(end) = after.find(THINK_END) else {
            break;
        };
        visible.push_str(&rest[..start]);
        think.push(&after[..end]);
        rest = &after[end + THINK_END.len()..];
        // the wire carries newlines as a literal backslash-n escape
        if let Some(r) = rest.strip_prefix("\\n") {
            rest = r;
        }
    }
    if think.is_empty() {
        return None;
    }
    visible.push_str(rest);
    Some((think.join("\\n"), visible))
}

// --resume / reload: the REPL replays the restored history as the live
// wire lines, each prefixed "history " (runtime/main.bend replay). Two
// lines exist only there: "you : <text>" (a user message: live, the
// client echoes what it sends) and "injected : <text>" (steering and
// notifications the Core committed). Everything else is a live line.
pub(crate) fn strip_history(line: &str) -> (&str, bool) {
    match line.strip_prefix("history ") {
        Some(rest) => (rest, true),
        None => (line, false),
    }
}

pub(crate) fn parse_history_line(line: &str) -> Option<Ev> {
    if let Some(t) = line.strip_prefix("you : ") {
        return Some(Ev::You(unescape_md(t)));
    }
    if let Some(t) = line.strip_prefix("injected : ") {
        let flat = unescape_md(t).replace('\n', " ");
        return Some(Ev::Info(format!("injected · {}", truncate_chars(flat.trim(), 110))));
    }
    parse_line(line)
}

// "2/10 · provider 529 (transient) · retry in 4s" -> the warning the
// user reads while the call waits
pub(crate) fn provider_retry_text(t: &str) -> String {
    let parts: Vec<&str> = t.split(" · ").collect();
    match parts.as_slice() {
        // "2/10" failed: the plan is attempt 3/10 after the pause
        [n, why, wait] => {
            let next = n
                .split_once('/')
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()? + 1, b)))
                .map(|(a, b)| format!("retry {}/{}", a, b))
                .unwrap_or_else(|| "retry".into());
            format!(
                "model call failed (attempt {}): {} · {} in {}",
                n,
                why,
                next,
                wait.trim_start_matches("retry in ")
            )
        }
        _ => format!("model call failed: {}", t),
    }
}

pub(crate) fn parse_line(line: &str) -> Option<Ev> {
    if line.is_empty() {
        return None;
    }
    // switchboard: the hub's own lines in a feed
    if let Some(rest) = line.strip_prefix("sb ") {
        return sb::parse_hub_line(rest);
    }
    if line == "--- idle" {
        return Some(Ev::Idle);
    }
    // runtime annotations: tool #<id> <name> : <args>
    if let Some(r) = line.strip_prefix("tool #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolInfo {
            id,
            name: name.trim().to_string(),
            args: args.to_string(),
        });
    }
    // tool_code #<id> : <full args, wire-encoded> (run_typescript only)
    if let Some(r) = line.strip_prefix("tool_code #") {
        let (id_s, rest) = r.split_once(" : ")?;
        let id: u32 = id_s.trim().parse().ok()?;
        return Some(Ev::ToolCode {
            id,
            code: rest.to_string(),
        });
    }
    // tool_result #<id> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("tool_result #") {
        let (id_s, rest) = r.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::ToolResult {
            id,
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
    }
    // subtool <name> <ok|fail> : <preview>
    if let Some(r) = line.strip_prefix("subtool ") {
        let (name, rest) = r.split_once(' ')?;
        let (st, preview) = rest.split_once(" : ").unwrap_or((rest, ""));
        return Some(Ev::Sub {
            name: name.to_string(),
            ok: st.trim() == "ok",
            preview: preview.to_string(),
        });
    }
    if let Some(r) = line.strip_prefix("core rejected: ") {
        // BR-003: right after an interrupt the in-flight completion has
        // no turn to land on - expected plumbing, not an error
        if r == "no pending completion" || r == "no pending tool result" {
            return Some(Ev::Info(
                "in-flight response dropped (turn interrupted)".into(),
            ));
        }
        return Some(Ev::Err(r.to_string()));
    }
    let Some(o) = line.strip_prefix("  obs: ") else {
        return Some(Ev::Raw(line.to_string()));
    };
    if o == "turn_started" {
        return Some(Ev::Turn);
    }
    if let Some(t) = o.strip_prefix("assistant: ") {
        // tool-call-only replies carry no text
        return if t.is_empty() {
            None
        } else {
            Some(Ev::Assistant(t.to_string()))
        };
    }
    if o == "assistant:" {
        return None;
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        let id: u32 = n.parse().ok()?;
        return Some(Ev::Tool(ToolData::bare(id, ToolState::Run)));
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let (id_s, tail) = rest.split_once(' ')?;
        let id: u32 = id_s.parse().ok()?;
        let state = match tail {
            "ok" => ToolState::Ok,
            _ => ToolState::Fail,
        };
        return Some(Ev::Tool(ToolData::bare(id, state)));
    }
    if o.starts_with("tool_result_committed") {
        return None;
    }
    if let Some(t) = o.strip_prefix("steering_received: ") {
        return Some(Ev::Info(format!("steering received: {}", t)));
    }
    if let Some(t) = o.strip_prefix("steered: ") {
        return Some(Ev::Info(format!("steering passed to the model: {}", t)));
    }
    if let Some(t) = o.strip_prefix("notification_received: ") {
        return Some(Ev::Info(format!("notification : {}", t)));
    }
    if let Some(t) = o.strip_prefix("notification_delivered: ") {
        return Some(Ev::Info(format!("notification delivered to the model: {}", t)));
    }
    if let Some(t) = o.strip_prefix("provider_retry: ") {
        return Some(Ev::Warn(provider_retry_text(t)));
    }
    if let Some(t) = o.strip_prefix("harness_restarted: ") {
        return Some(Ev::Err(format!(
            "the harness crashed ({}) and restarted — the current turn is interrupted, the history is restored up to the last model call",
            t
        )));
    }
    if let Some(t) = o.strip_prefix("candidate_discarded: ") {
        return Some(Ev::Warn(format!("candidate discarded: {}", t)));
    }
    if let Some(t) = o.strip_prefix("compaction_started #") {
        return Some(Ev::Compact(t.to_string()));
    }
    if let Some(t) = o.strip_prefix("context_compaction_failed: ") {
        return Some(Ev::Err(format!("compaction failed: {}", t)));
    }
    if let Some(t) = o.strip_prefix("session_restored: ") {
        return Some(Ev::Info(format!(
            "session restored · {} messages",
            t.trim_end_matches(" messages")
        )));
    }
    if let Some(t) = o.strip_prefix("compaction_done: ") {
        return Some(Ev::Compacted(t.to_string()));
    }
    if let Some(t) = o.strip_prefix("usage: ") {
        return usage::Usage::parse(t).map(Ev::Usage);
    }
    if o == "null_iteration" {
        return Some(Ev::Warn(
            "empty response from the model — retrying".into(),
        ));
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        // a completed turn needs no annotation; a failure or an
        // interrupt must never disappear — the turn just stops
        if t == "completed" {
            return Some(Ev::TurnDone);
        }
        if let Some(why) = t.strip_prefix("failed: ") {
            return Some(Ev::Err(format!("turn failed: {}", why)));
        }
        if t == "interrupted" {
            return Some(Ev::Warn("turn interrupted".into()));
        }
        return Some(Ev::Err(format!("turn stopped: {}", t)));
    }
    // the runtime ran out of execution budget mid-turn (never silent)
    if let Some(t) = o.strip_prefix("turn_stalled: ") {
        return Some(Ev::Err(format!("turn stopped: {}", t)));
    }
    Some(Ev::Raw(o.to_string()))
}

// decode the tool_code wire encoding: "\N" newline, "\R" CR, backslash
// doubled (the same reversible encoding the provider wire uses)
pub(crate) fn wire_decode(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < cs.len() {
        if cs[i] == '\\' && i + 1 < cs.len() {
            match cs[i + 1] {
                'N' => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                'R' => {
                    out.push('\r');
                    i += 2;
                    continue;
                }
                '\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}
