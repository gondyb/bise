//! Token usage and context-window fill of an agent.
//!
//! After each model call the REPL prints one feed line
//! (runtime/usage-pure.bend):
//! `  obs: usage: model=M in=I out=O cache_read=R cache_write=W`.
//! `in` counts every input token of the call (cached ones included):
//! the context the model saw. The context after the call is `in + out`
//! (the reply joins the history). A compaction resets it: the last
//! usage before a `compaction_done` no longer describes the context.

use crate::Ev;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    /// Parse the text after `obs: usage: `. Unknown keys are ignored; a
    /// line without `in=` is not a usage line.
    pub fn parse(t: &str) -> Option<Usage> {
        let mut u = Usage::default();
        let mut has_in = false;
        for kv in t.split_whitespace() {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let n = || v.parse::<u64>().ok();
            match k {
                "model" => u.model = v.to_string(),
                "in" => {
                    u.input = n()?;
                    has_in = true;
                }
                "out" => u.output = n()?,
                "cache_read" => u.cache_read = n()?,
                "cache_write" => u.cache_write = n()?,
                _ => {}
            }
        }
        has_in.then_some(u)
    }

    /// Tokens in the context after the call.
    pub fn context(&self) -> u64 {
        self.input + self.output
    }

    /// "42k / 200k tokens · 21%"; a model with no known window: "42k tokens".
    pub fn label(&self) -> String {
        let used = self.context();
        match context_window(&self.model) {
            Some(w) => format!("{} / {} tokens · {}%", fmt_tokens(used), fmt_tokens(w), percent(used, w)),
            None => format!("{} tokens", fmt_tokens(used)),
        }
    }

    /// The compact form for the task list: "21%", or "42k" without a
    /// known window.
    pub fn short(&self) -> String {
        let used = self.context();
        match context_window(&self.model) {
            Some(w) => format!("{}%", percent(used, w)),
            None => fmt_tokens(used),
        }
    }
}

/// The context window of a model, in tokens (None: unknown model).
/// The harness sends Anthropic's 1M-context beta header on every call
/// (runtime/provider.bend), so Claude models get 1M.
pub fn context_window(model: &str) -> Option<u64> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("claude") {
        return Some(1_000_000);
    }
    if m.contains("glm") {
        return Some(200_000);
    }
    if m.starts_with("mistral-large") || m.starts_with("mistral-medium") || m.starts_with("devstral") {
        return Some(128_000);
    }
    None
}

fn percent(used: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    (used * 100 + window / 2) / window
}

/// 950 -> "950", 42_310 -> "42k", 1_250_000 -> "1.2M".
pub fn fmt_tokens(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{}k", (n + 500) / 1000)
    } else {
        let tenths = (n + 50_000) / 100_000;
        if tenths.is_multiple_of(10) {
            format!("{}M", tenths / 10)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    }
}

/// The usage of the current context of a feed: the last usage event,
/// unless a compaction came after it.
pub fn current(events: &[Ev]) -> Option<&Usage> {
    for e in events.iter().rev() {
        match e {
            Ev::Usage(u) => return Some(u),
            Ev::Compacted(_) => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_repl_line() {
        let u = Usage::parse("model=claude-opus-5-5 in=40312 out=512 cache_read=40000 cache_write=300").unwrap();
        assert_eq!(u.model, "claude-opus-5-5");
        assert_eq!((u.input, u.output, u.cache_read, u.cache_write), (40312, 512, 40000, 300));
        assert_eq!(u.context(), 40824);
        assert!(Usage::parse("model=m out=3").is_none());
        assert!(Usage::parse("model=m in=x").is_none());
    }

    #[test]
    fn labels() {
        let u = Usage { model: "claude-opus-5-5".into(), input: 209_500, output: 500, ..Default::default() };
        assert_eq!(u.label(), "210k / 1M tokens · 21%");
        assert_eq!(u.short(), "21%");
        let g = Usage { model: "zai-glm-5-3".into(), input: 42_000, output: 0, ..Default::default() };
        assert_eq!(g.label(), "42k / 200k tokens · 21%");
        let x = Usage { model: "some-model".into(), input: 950, output: 0, ..Default::default() };
        assert_eq!(x.label(), "950 tokens");
        assert_eq!(x.short(), "950");
    }

    #[test]
    fn formats_tokens() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(42_310), "42k");
        assert_eq!(fmt_tokens(1_000_000), "1M");
        assert_eq!(fmt_tokens(1_250_000), "1.3M");
    }

    #[test]
    fn the_feed_line_is_a_hidden_event() {
        let ev = crate::parse_line("  obs: usage: model=claude-opus-5-5 in=100 out=5 cache_read=0 cache_write=0");
        assert!(matches!(&ev, Some(Ev::Usage(u)) if u.input == 100 && u.output == 5));
        assert!(!crate::ev_visible(&ev.unwrap(), false));
    }

    #[test]
    fn compaction_resets_the_count() {
        let u = |n| Ev::Usage(Usage { model: "claude-x".into(), input: n, ..Default::default() });
        let evs = vec![u(10), Ev::Info("x".into()), u(20)];
        assert_eq!(current(&evs).map(|u| u.input), Some(20));
        let evs = vec![u(10), u(900), Ev::Compacted("sum...".into())];
        assert_eq!(current(&evs), None);
        let evs = vec![u(900), Ev::Compacted("sum...".into()), u(30)];
        assert_eq!(current(&evs).map(|u| u.input), Some(30));
        assert_eq!(current(&[]), None);
    }
}
