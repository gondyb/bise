//! The REPL wire lines the hub needs to understand (the full vocabulary
//! is documented in `.agents/skills/bend-harness-qa/reference/`).

use crate::util::{strip_thinking, wire_unescape};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wire {
    TurnStarted,
    /// The batch ended: the REPL is idle and reads its socket again.
    Idle,
    /// Visible assistant text (reasoning removed), unescaped.
    Assistant(String),
    TurnDone(String),
    /// `tool #<id> <name> : <args>` (a live tool call annotation).
    Tool { name: String, args: String },
    /// A line replayed from a restored session.
    History,
    Other,
}

pub fn parse(line: &str) -> Wire {
    if line.starts_with("history ") {
        return Wire::History;
    }
    if line == "--- idle" {
        return Wire::Idle;
    }
    let t = line.trim_start();
    if t == "obs: turn_started" {
        return Wire::TurnStarted;
    }
    if let Some(rest) = t.strip_prefix("obs: assistant: ") {
        return Wire::Assistant(strip_thinking(&wire_unescape(rest)));
    }
    if let Some(rest) = t.strip_prefix("obs: turn_done: ") {
        return Wire::TurnDone(rest.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("tool #") {
        if let Some((_, rest)) = rest.split_once(' ') {
            let (name, args) = rest.split_once(" : ").unwrap_or((rest, ""));
            return Wire::Tool {
                name: name.trim().to_string(),
                args: args.to_string(),
            };
        }
    }
    Wire::Other
}

/// The files an `apply_patch` call touches (its V4A headers). The
/// annotation carries the JSON args with escaped newlines.
pub fn patch_files(args: &str) -> Vec<String> {
    let text = args.replace("\\\\n", "\n").replace("\\n", "\n");
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches('"');
        for marker in ["*** Update File: ", "*** Add File: ", "*** Delete File: ", "*** Move to: "] {
            if let Some(p) = l.strip_prefix(marker) {
                let p = p.trim().trim_end_matches('"').trim_end_matches(',').trim();
                if !p.is_empty() && !out.iter().any(|x| x == p) {
                    out.push(p.to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_markers() {
        assert_eq!(parse("  obs: turn_started"), Wire::TurnStarted);
        assert_eq!(parse("--- idle"), Wire::Idle);
        assert_eq!(parse("  obs: turn_done: completed"), Wire::TurnDone("completed".into()));
        assert_eq!(parse("history   obs: turn_started"), Wire::History);
    }

    #[test]
    fn assistant_text_is_unescaped_without_reasoning() {
        assert_eq!(
            parse("  obs: assistant: <think>hmm</think>\\nFait.\\nOK"),
            Wire::Assistant("Fait.\nOK".into())
        );
    }

    #[test]
    fn tool_annotation() {
        assert_eq!(
            parse("tool #3 bash : {\"arg\":\"sb list\"}"),
            Wire::Tool {
                name: "bash".into(),
                args: "{\"arg\":\"sb list\"}".into()
            }
        );
    }

    #[test]
    fn patch_headers() {
        let args = "{\"arg\":\"*** Begin Patch\\n*** Update File: src/a.rs\\n@@\\n-x\\n+y\\n*** Add File: b.md\\n+z\\n*** End Patch\"}";
        assert_eq!(patch_files(args), vec!["src/a.rs".to_string(), "b.md".to_string()]);
    }
}
