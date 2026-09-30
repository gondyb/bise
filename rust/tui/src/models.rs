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

// ---- the model and effort shown (BISE-135) ----

/// A model's name for people: its id without the provider, the date or
/// build suffix, `-latest` and the `claude-`/`zai-` prefix, version
/// digits joined: `foundry/claude-opus-5-5` -> `opus 5.5`,
/// `mistral/devstral-medium-2509` -> `devstral-medium`, `openai/gpt-5.1-codex`
/// -> `gpt-5.1-codex`.
pub(crate) fn long_name(model: &str) -> String {
    let id = model.rsplit('/').next().unwrap_or(model);
    let mut parts: Vec<&str> = id.split('-').filter(|p| !p.is_empty()).collect();
    while parts.len() > 1 {
        let last = parts[parts.len() - 1];
        let dated = last.len() >= 4 && last.chars().all(|c| c.is_ascii_digit());
        if last == "latest" || dated {
            parts.pop();
        } else {
            break;
        }
    }
    if parts.len() > 1 && matches!(parts[0], "claude" | "zai") {
        parts.remove(0);
    }
    // the trailing one-digit groups are a version: opus-5-5 -> opus 5.5
    let digits = |p: &str| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit());
    let n = parts.iter().rev().take_while(|p| digits(p)).count();
    if n > 0 && n < parts.len() {
        let (head, ver) = parts.split_at(parts.len() - n);
        return format!("{} {}", head.join("-"), ver.join("."));
    }
    parts.join("-")
}

/// A model's family: the first word of its [`long_name`], with its
/// version when that is glued to it (`gpt-5.1`, `gemini-2.5`): `opus`,
/// `devstral`, `glm`.
pub(crate) fn family(model: &str) -> String {
    let long = long_name(model);
    let head = long.split(' ').next().unwrap_or("");
    let mut words = head.split('-');
    let first = words.next().unwrap_or("").to_string();
    match words.next() {
        Some(v) if v.starts_with(|c: char| c.is_ascii_digit()) => format!("{}-{}", first, v),
        _ => first,
    }
}

/// The short form of an effort: lo, med, hi, max; `off` for none.
pub(crate) fn short_effort(effort: &str) -> &str {
    match effort {
        "low" => "lo",
        "medium" => "med",
        "high" => "hi",
        "none" => "off",
        e => e,
    }
}

/// The panel's tag, `opus·hi` (ASCII `opus.hi`): the family (with its
/// version when `others` run another model of the same family), cut to
/// 10 columns; the effort's short form after a middle dot, none when
/// the model takes none.
pub(crate) fn tag(model: &str, effort: &str, others: &[&str]) -> String {
    if model.is_empty() {
        return String::new();
    }
    let fam = family(model);
    let clash = others.iter().any(|o| !o.is_empty() && *o != model && family(o) == fam);
    let mut name = if clash { long_name(model).replace(' ', "") } else { fam };
    if name.chars().count() > 10 {
        name = name.chars().take(9).collect::<String>() + "…";
    }
    if effort.is_empty() {
        return name;
    }
    let dot = if crate::theme::ascii_mode() { "." } else { "·" };
    format!("{}{}{}", name, dot, short_effort(effort))
}

/// The efforts a model takes and the one it gets by default (the
/// catalog's rule, rust/catalog Resolved::efforts).
pub(crate) fn efforts(model: &str) -> (Vec<String>, String) {
    match resolve(model) {
        Some(r) => (r.efforts(), r.default_effort()),
        None => (Vec::new(), String::new()),
    }
}

/// One model the `/model` popup offers.
pub(crate) struct Pick {
    /// what `/model` takes: a full id, or an alias
    pub(crate) value: String,
    pub(crate) desc: String,
}

/// The chat models of the catalog (built in and config.toml's, usable
/// ones), then its aliases: `/model`'s list (BISE-117 completion).
pub(crate) fn picks() -> Vec<Pick> {
    let c = &setup().catalog;
    let mut out = Vec::new();
    for m in c.models.iter().filter(|m| !m.stt) {
        let Some(p) = c.provider(&m.provider).filter(|p| !p.stt_only && p.needs.is_empty()) else {
            continue;
        };
        let r = c.resolve(&m.name());
        let ctx = match r.caps.context {
            n if n >= 1_000_000 => format!("{}M", n / 1_000_000),
            n => format!("{}k", n / 1000),
        };
        let mine = if m.source == bise_catalog::Source::Config { " · config.toml" } else { "" };
        out.push(Pick { value: m.name(), desc: format!("{} · {} · {}{}", long_name(&m.name()), p.name, ctx, mine) });
    }
    for (a, to) in &c.aliases {
        out.push(Pick { value: a.clone(), desc: format!("= {}", to) });
    }
    out
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
        assert_eq!(context_window("anthropic/claude-haiku-4-5"), Some(200_000));
        assert_eq!(context_window("openai/gpt-6-astra"), Some(1_050_000));
        // an unlisted model: its provider's default
        assert_eq!(context_window("groq/brand-new"), Some(131_072));
        // the old bare ids of the usage lines
        assert_eq!(context_window("claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("zai-glm-5-3"), Some(1_048_576));
        // nothing knows it: no window, never a panic
        assert_eq!(context_window("nowhere/m"), None);
        assert_eq!(context_window(""), None);
    }

    #[test]
    fn a_call_costs_its_prices() {
        // haiku 4.5: 1 in, 5 out, 0.10 cache read, 1.25 cache write
        let c = cost("anthropic/claude-haiku-4-5", 1_000_000, 100_000, 0, 0).unwrap();
        assert!((c - 1.5).abs() < 1e-9, "{c}");
        let c = cost("anthropic/claude-haiku-4-5", 100_000, 0, 90_000, 10_000).unwrap();
        assert!((c - (0.009 + 0.0125)).abs() < 1e-9, "{c}");
        assert_eq!(cost("fireworks/accounts/fireworks/models/glm-5p3", 10, 10, 0, 0), None);
        assert_eq!(cost("nowhere/m", 10, 10, 0, 0), None);
        assert_eq!(fmt_cost(4.5), "$4.50");
        assert_eq!(fmt_cost(0.0042), "$0.0042");
    }

    #[test]
    fn names_and_tags() {
        assert_eq!(long_name("foundry/claude-opus-5-5"), "opus 5.5");
        assert_eq!(long_name("anthropic/claude-sonnet-4-5"), "sonnet 4.5");
        assert_eq!(long_name("mistral/devstral-medium-2509"), "devstral-medium");
        assert_eq!(long_name("mistral/mistral-large-latest"), "mistral-large");
        assert_eq!(long_name("openai/gpt-5.1-codex"), "gpt-5.1-codex");
        assert_eq!(long_name("mistral/zai-glm-5-3"), "glm 5.3");
        assert_eq!(long_name("anthropic/claude-3-5-sonnet-20241022"), "3-5-sonnet");
        assert_eq!(family("foundry/claude-opus-5-5"), "opus");
        assert_eq!(family("openai/gpt-5.1-codex"), "gpt-5.1");
        assert_eq!(family("mistral/devstral-medium-2509"), "devstral");
        assert_eq!(tag("foundry/claude-opus-5-5", "high", &[]), "opus·hi");
        assert_eq!(tag("anthropic/claude-sonnet-4-5", "low", &[]), "sonnet·lo");
        assert_eq!(tag("openai/gpt-4.1", "", &[]), "gpt-4.1");
        assert_eq!(tag("mistral/zai-glm-5-3", "none", &[]), "glm·off");
        // two models of one family: the version tells them apart
        let others = ["foundry/claude-opus-5-5", "anthropic/claude-opus-4-5"];
        assert_eq!(tag("anthropic/claude-opus-4-5", "medium", &others), "opus4.5·med");
        assert_eq!(tag("foundry/claude-opus-5-5", "max", &others[..1]), "opus·max");
        assert_eq!(tag("x/averyveryverylongmodelname", "high", &[]), "averyvery…·hi");
    }

    #[test]
    fn the_model_list_has_the_catalog_and_its_aliases() {
        let p = picks();
        let has = |v: &str| p.iter().find(|x| x.value == v);
        assert!(has("foundry/claude-opus-5-5").unwrap().desc.starts_with("opus 5.5 · Anthropic (foundry proxy) · 1M"));
        assert!(has("anthropic/claude-sonnet-5-5").is_some());
        assert!(has("openai/gpt-6-astra").is_some());
        assert_eq!(has("opus-5.5").unwrap().desc, "= foundry/claude-opus-5-5");
        // no speech-to-text model, no provider that is not usable yet
        assert!(has("mistral/voxtral-mini-latest").is_none());
        assert!(!p.iter().any(|x| x.value.starts_with("bedrock/")));
        assert_eq!(efforts("foundry/claude-opus-5-5").1, "high");
        assert_eq!(efforts("mistral/zai-glm-5-3").0, ["none", "high"]);
        assert!(efforts("mistral/mistral-large-latest").0.is_empty());
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
