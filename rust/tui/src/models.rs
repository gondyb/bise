//! What bise's catalog says of the agents' models (BISE-150): the
//! context window of the gauge, the prices of the usage line, whether a
//! model reads images (checked before a message with images goes out).
//!
//! The catalog is the built-in list merged with config.toml, read once
//! (a new `[models]` table needs a restart, as for the REPLs). Names
//! are the full `provider/model` ids the REPL announces (usage lines,
//! harness-info); an old bare id goes through the legacy rule
//! (`claude*` -> foundry, else mistral), like everywhere else.

use bise_catalog::{Known, Resolved, Setup};
use std::sync::OnceLock;

fn setup() -> &'static Setup {
    static SETUP: OnceLock<Setup> = OnceLock::new();
    SETUP.get_or_init(load)
}

#[cfg(not(test))]
fn load() -> Setup {
    Setup::load(&bise_home::Home::from_env().config_file())
}

/// Tests: the built-in list alone, whatever the machine's config says.
#[cfg(test)]
fn load() -> Setup {
    Setup::from_text(None, &|_| None)
}

fn resolve(model: &str) -> Option<Resolved> {
    let m = model.trim();
    (!m.is_empty()).then(|| setup().catalog.resolve(m))
}

/// The context window of a model, in tokens: listed, else its
/// provider's default. None: no model, or a provider nobody knows.
pub(crate) fn context_window(model: &str) -> Option<u64> {
    resolve(model).filter(|r| r.known != Known::NoProvider).map(|r| r.caps.context)
}

/// The cost of one call in USD; None when its prices are not known.
pub(crate) fn cost(model: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> Option<f64> {
    resolve(model)?.price.cost(input, output, cache_read, cache_write)
}

/// The model an agent starts with: main's, or the sub-agents'.
pub(crate) fn model_for(main: bool) -> String {
    setup().model_for(if main { "main" } else { "agent" }).name
}

/// A listed model the catalog says cannot read images. An unlisted one
/// is not refused here: the provider says (the no-vision line then
/// comes from its error).
pub(crate) fn lacks_vision(model: &str) -> bool {
    resolve(model).is_some_and(|r| r.known == Known::Listed && !r.caps.vision)
}

/// `$0.0042`, `$0.13`, `$2.40`.
pub(crate) fn fmt_cost(usd: f64) -> String {
    if usd >= 0.1 {
        format!("${:.2}", usd)
    } else {
        format!("${:.4}", usd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_come_from_the_catalog() {
        assert_eq!(context_window("foundry/claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("anthropic/claude-sonnet-4-5"), Some(200_000));
        assert_eq!(context_window("openai/gpt-4.1"), Some(1_047_576));
        // an unlisted model: its provider's default
        assert_eq!(context_window("groq/brand-new"), Some(131_072));
        // the old bare ids of the usage lines
        assert_eq!(context_window("claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("zai-glm-5-3"), Some(200_000));
        // nothing knows it: no window, never a panic
        assert_eq!(context_window("nowhere/m"), None);
        assert_eq!(context_window(""), None);
    }

    #[test]
    fn a_call_costs_its_prices() {
        // sonnet 4.5: 3 in, 15 out, 0.30 cache read, 3.75 cache write
        let c = cost("anthropic/claude-sonnet-4-5", 1_000_000, 100_000, 0, 0).unwrap();
        assert!((c - 4.5).abs() < 1e-9, "{c}");
        let c = cost("anthropic/claude-sonnet-4-5", 100_000, 0, 90_000, 10_000).unwrap();
        assert!((c - (0.027 + 0.0375)).abs() < 1e-9, "{c}");
        assert_eq!(cost("foundry/claude-opus-5-5", 10, 10, 0, 0), None);
        assert_eq!(cost("nowhere/m", 10, 10, 0, 0), None);
        assert_eq!(fmt_cost(4.5), "$4.50");
        assert_eq!(fmt_cost(0.0042), "$0.0042");
    }

    #[test]
    fn only_a_listed_model_without_vision_is_refused() {
        assert!(lacks_vision("mistral/codestral-latest"));
        assert!(!lacks_vision("mistral/mistral-medium-latest"));
        assert!(!lacks_vision("anthropic/claude-haiku-4-5"));
        // unlisted or unknown: the provider decides
        assert!(!lacks_vision("ollama/llava"));
        assert!(!lacks_vision("nowhere/m"));
        assert!(!lacks_vision(""));
    }
}
