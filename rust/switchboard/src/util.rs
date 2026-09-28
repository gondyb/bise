//! Small helpers shared by every module.

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The REPL wire carries one message per line: a real newline travels as
/// the two chars backslash-n, and nothing else is escaped (the exact
/// convention of core/api.bend `escape_nl` / `unescape_nl`).
pub fn wire_escape(s: &str) -> String {
    s.replace('\r', "").replace('\n', "\\n")
}

/// The reverse of `wire_escape` (`unescape_nl`): backslash-n becomes a
/// newline, any other backslash stays.
pub fn wire_unescape(s: &str) -> String {
    s.replace("\\n", "\n")
}

/// At most `max` chars, with an ellipsis when cut.
pub fn clip(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The last `max` chars (for "the end of the latest reply" excerpts).
pub fn clip_tail(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let mut out = String::from("…");
    out.extend(s.chars().skip(n - max + 1));
    out
}

/// One line: newlines and runs of spaces collapse.
pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "12m", "3h", "2j": a compact age for the task board.
pub fn age(from_ms: u64, now: u64) -> String {
    let s = now.saturating_sub(from_ms) / 1000;
    if s < 60 {
        format!("{}s", s)
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}j", s / 86_400)
    }
}

/// The model's reasoning rides inside the assistant text between think
/// markers; excerpts for other agents keep only the visible part.
pub fn strip_thinking(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        let s = "a\nb c\\d";
        assert_eq!(wire_unescape(&wire_escape(s)), s);
        assert!(!wire_escape(s).contains('\n'));
    }

    #[test]
    fn clip_marks_the_cut() {
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 4), "abc");
        assert_eq!(clip_tail("abcdef", 4), "…def");
    }

    #[test]
    fn thinking_is_removed() {
        assert_eq!(strip_thinking("<think>x</think>\\nHello"), "\\nHello");
        assert_eq!(strip_thinking("a<think>x</think>b"), "ab");
        assert_eq!(strip_thinking("<think>never closed"), "");
    }

    #[test]
    fn ages() {
        assert_eq!(age(0, 59_000), "59s");
        assert_eq!(age(0, 3_600_000), "1h");
        assert_eq!(age(0, 2 * 86_400_000), "2j");
    }
}
