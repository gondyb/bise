use super::*;

fn no_env(_: &str) -> Option<String> {
    None
}

fn setup(cfg: &str) -> Setup {
    Setup::from_text(Some(cfg), &no_env)
}

#[test]
fn the_builtin_list_parses_with_no_warning() {
    let c = Catalog::builtin();
    assert_eq!(c.warnings, Vec::<String>::new());
    for id in [
        "anthropic", "foundry", "openai", "google", "mistral", "openrouter", "groq", "xai", "deepseek",
        "together", "fireworks", "cerebras", "ollama", "lmstudio", "azure", "vertex", "bedrock",
    ] {
        let p = c.provider(id).unwrap_or_else(|| panic!("provider {id}"));
        assert!(FAMILIES.contains(&p.api.as_str()), "{id}: {}", p.api);
        assert!(!p.base_url.ends_with('/'), "{id}");
    }
    // every listed model has a provider, no duplicate
    let mut names: Vec<String> = c.models.iter().map(|m| m.name()).collect();
    for m in &c.models {
        assert!(c.provider(&m.provider).is_some(), "{}", m.name());
    }
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(n, names.len());
    // the cloud wrappers wait for BISE-149, the local ones need no key
    for id in ["azure", "vertex", "bedrock"] {
        assert_eq!(c.provider(id).unwrap().needs, "BISE-149");
    }
    for id in ["ollama", "lmstudio"] {
        assert_eq!(c.provider(id).unwrap().key_env, "");
    }
}

#[test]
fn the_default_is_todays_setup() {
    let s = Setup::from_text(None, &no_env);
    assert_eq!(s.model, "foundry/claude-opus-5-5");
    assert_eq!(s.model_from, "default");
    let r = s.model_for("main");
    assert_eq!(r.known, Known::Listed);
    assert_eq!(r.api, "anthropic");
    assert_eq!(r.base_url, "https://foundry-proxy.cheetah-koi.ts.net/anthropic/v1");
    assert_eq!(r.key_env, "ANTHROPIC_FOUNDRY_API_KEY");
    assert_eq!(r.caps.context, 1_000_000);
}

/// the direct Anthropic API's beta flags (models.toml)
const ANTH_BETAS: &str = "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

#[test]
fn a_listed_model_takes_its_own_fields_then_its_providers() {
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-sonnet-4-5");
    assert_eq!(r.known, Known::Listed);
    assert_eq!(r.caps, Caps { context: 200_000, max_output: 64_000, vision: true, reasoning: true, tools: true, thinking: "budget".into(), betas: ANTH_BETAS.into() });
    let r = c.resolve("openai/gpt-4.1");
    assert_eq!((r.caps.context, r.caps.reasoning, r.caps.vision), (1_047_576, false, true));
}

#[test]
fn an_unlisted_model_gets_its_providers_defaults() {
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-future-9");
    assert_eq!(r.known, Known::Unlisted);
    assert_eq!((r.provider.as_str(), r.id.as_str()), ("anthropic", "claude-future-9"));
    assert_eq!(r.base_url, "https://api.anthropic.com/v1");
    assert_eq!(r.caps.context, 200_000);
    // a provider with no model default: the global ones
    let r = c.resolve("deepseek/deepseek-v9");
    assert_eq!(r.caps.context, DEFAULT_CAPS.context);
    assert_eq!(r.key_env, "DEEPSEEK_API_KEY");
    // local: any name
    let r = c.resolve("ollama/qwen3-coder:30b");
    assert_eq!((r.known, r.base_url.as_str(), r.key_env.as_str()), (Known::Unlisted, "http://localhost:11434/v1", ""));
}

#[test]
fn an_unknown_provider_resolves_without_failing() {
    let c = Catalog::builtin();
    let r = c.resolve("nowhere/some-model");
    assert_eq!(r.known, Known::NoProvider);
    assert_eq!(r.base_url, "");
    assert_eq!(r.caps, DEFAULT_CAPS);
    // and odd names never panic
    for n in ["", "/", "a/", "/b", "  ", "x//y", "ü/ß"] {
        let _ = c.resolve(n);
    }
}

#[test]
fn model_ids_may_hold_slashes() {
    let c = Catalog::builtin();
    let r = c.resolve("openrouter/anthropic/claude-sonnet-4.5");
    assert_eq!((r.provider.as_str(), r.id.as_str(), r.known), ("openrouter", "anthropic/claude-sonnet-4.5", Known::Listed));
    let r = c.resolve("groq/some/new-model");
    assert_eq!((r.id.as_str(), r.known), ("some/new-model", Known::Unlisted));
}

#[test]
fn old_bare_names_keep_working() {
    let c = Catalog::builtin();
    assert_eq!(c.canonical("opus-5.5"), "foundry/claude-opus-5-5");
    assert_eq!(c.canonical("claude-opus-5-5"), "foundry/claude-opus-5-5");
    assert_eq!(c.canonical("zai-glm-5-3"), "mistral/zai-glm-5-3");
    assert_eq!(c.resolve("mistral-large-latest").known, Known::Listed);
    assert_eq!(c.resolve("claude-opus-5-5").caps.context, 1_000_000);
}

#[test]
fn the_config_overrides_a_model_key_by_key() {
    let s = setup(
        r#"
model = "anthropic/claude-sonnet-4-5"
[models."anthropic/claude-sonnet-4-5"]
context = 1000000
"#,
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let r = s.model_for("main");
    assert_eq!(r.caps.context, 1_000_000);
    assert_eq!(r.caps.max_output, 64_000); // kept from the built-in entry
    assert_eq!(s.catalog.model("anthropic/claude-sonnet-4-5").unwrap().source, Source::Config);
}

#[test]
fn the_config_adds_a_model_to_a_known_provider() {
    let s = setup(
        r#"
[models."groq/new-model"]
context = 65536
vision = true
"#,
    );
    let r = s.catalog.resolve("groq/new-model");
    assert_eq!(r.known, Known::Listed);
    assert_eq!((r.caps.context, r.caps.vision, r.caps.max_output), (65_536, true, DEFAULT_CAPS.max_output));
    assert_eq!(r.base_url, "https://api.groq.com/openai/v1");
}

#[test]
fn the_config_adds_a_whole_provider() {
    let s = setup(
        r#"
model = "work/qwen3-coder-480b"
[providers.work]
name = "Corp LLM"
base_url = "https://llm.corp.example/v1/"
key_env = "CORP_LLM_KEY"
context = 262144
[models."work/small"]
context = 32768
"#,
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let r = s.model_for("main");
    assert_eq!(r.known, Known::Unlisted);
    assert_eq!(r.api, "openai-chat"); // the default family
    assert_eq!(r.base_url, "https://llm.corp.example/v1"); // no trailing '/'
    assert_eq!(r.key_env, "CORP_LLM_KEY");
    assert_eq!(r.caps.context, 262_144);
    assert_eq!(s.catalog.resolve("work/small").caps.context, 32_768);
    assert_eq!(s.catalog.provider("work").unwrap().source, Source::Config);
}

#[test]
fn the_config_overrides_a_known_provider_keeping_the_rest() {
    let s = setup(
        r#"
[providers.anthropic]
base_url = "https://proxy.example/anthropic/v1"
key_env = "MY_KEY"
"#,
    );
    let p = s.catalog.provider("anthropic").unwrap();
    assert_eq!((p.base_url.as_str(), p.key_env.as_str(), p.api.as_str()), ("https://proxy.example/anthropic/v1", "MY_KEY", "anthropic"));
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-4-5").caps.max_output, 64_000);
    // the order of the list is kept (models.toml's, not alphabetical)
    let ids: Vec<&str> = s.catalog.providers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(&ids[..4], ["anthropic", "foundry", "openai", "google"]);
}

#[test]
fn a_model_may_speak_another_family_than_its_provider() {
    let s = setup("[models.\"openai/gpt-5-pro\"]\napi = \"openai-responses\"\n");
    assert_eq!(s.catalog.resolve("openai/gpt-5-pro").api, "openai-responses");
    assert_eq!(s.catalog.resolve("openai/gpt-5").api, "openai-chat");
}

#[test]
fn bad_entries_are_warnings_never_errors() {
    let s = setup(
        r#"
[providers.x]
api = "carrier-pigeon"
context = -3
colour = "blue"
[models."no-slash"]
context = 1
[models."x/m"]
vision = "yes"
[provider.y]
base_url = "http://y"
"#,
    );
    let w = s.catalog.warnings.join("\n");
    for needle in ["providers.x.api", "providers.x.context", "unknown key colour", "models.\"no-slash\"", "models.\"x/m\".vision", "[providers.<id>]"] {
        assert!(w.contains(needle), "{needle} not in:\n{w}");
    }
    let r = s.catalog.resolve("x/m");
    assert_eq!((r.api.as_str(), r.caps.context, r.caps.vision), ("openai-chat", DEFAULT_CAPS.context, false));
}

#[test]
fn a_config_that_is_not_toml_still_gives_the_model() {
    // the Bend reader accepts bare words; the TOML one does not
    let s = setup("model = groq/openai/gpt-oss-120b # fast\nagent_model = \"cerebras/gpt-oss-120b\"\nthinking = high\n");
    assert_eq!(s.model, "groq/openai/gpt-oss-120b");
    assert_eq!(s.agent_model, "cerebras/gpt-oss-120b");
    assert!(s.catalog.warnings[0].contains("not valid TOML"));
    // the built-in list is still there
    assert!(s.catalog.provider("anthropic").is_some());
}

#[test]
fn agent_model_falls_back_to_model() {
    let s = setup("model = \"openai/gpt-5\"\n");
    assert_eq!(s.agent_model, "openai/gpt-5");
    assert_eq!(s.agent_model_from, "model");
    assert_eq!(s.model_for("agent").name, "openai/gpt-5");
    let s = setup("model = \"openai/gpt-5\"\nagent_model = \"openai/gpt-5-mini\"\n");
    assert_eq!(s.model_for("main").name, "openai/gpt-5");
    assert_eq!(s.model_for("agent").name, "openai/gpt-5-mini");
    // an alias as agent_model is resolved too
    let s = setup("model = \"openai/gpt-5\"\nagent_model = \"opus-5.5\"\n");
    assert_eq!(s.agent_model, "foundry/claude-opus-5-5");
}

#[test]
fn the_env_wins_over_the_config() {
    let cfg = "model = \"openai/gpt-5\"\nagent_model = \"openai/gpt-5-mini\"\n";
    let env = |k: &str| match k {
        "BEND_MODEL" => Some("mistral/devstral-medium-latest".to_string()),
        _ => None,
    };
    let s = Setup::from_text(Some(cfg), &env);
    assert_eq!((s.model.as_str(), s.model_from), ("mistral/devstral-medium-latest", "BEND_MODEL"));
    assert_eq!(s.agent_model, "openai/gpt-5-mini"); // its own key
    let env = |k: &str| match k {
        "BISE_MODEL" => Some("xai/grok-4".to_string()),
        "BEND_MODEL" => Some("ignored/x".to_string()),
        "BISE_AGENT_MODEL" => Some("  ".to_string()), // empty = unset
        _ => None,
    };
    let s = Setup::from_text(Some("model = \"openai/gpt-5\"\n"), &env);
    assert_eq!(s.model, "xai/grok-4");
    assert_eq!(s.agent_model, "xai/grok-4"); // follows the effective model
}

#[test]
fn the_handoff_reads_back_as_the_same_catalog() {
    let s = setup(
        r#"
[providers.work]
base_url = "https://llm.corp.example/v1"
key_env = "CORP"
[models."work/q"]
context = 99000
[models."openai/gpt-5-pro"]
api = "openai-responses"
[aliases]
fast = "groq/openai/gpt-oss-120b"
"#,
    );
    let text = s.handoff_toml();
    let back = Setup::from_text(Some(&text), &no_env);
    assert!(back.catalog.warnings.iter().all(|w| !w.contains("unknown key")), "{:?}", back.catalog.warnings);
    for m in &s.catalog.models {
        assert_eq!(back.catalog.resolve(&m.name()), s.catalog.resolve(&m.name()), "{}", m.name());
    }
    for p in &s.catalog.providers {
        let n = format!("{}/unlisted", p.id);
        assert_eq!(back.catalog.resolve(&n), s.catalog.resolve(&n));
    }
    assert_eq!(back.catalog.canonical("fast"), "groq/openai/gpt-oss-120b");
    assert_eq!(back.catalog.default_model, s.catalog.default_model);
    // the model choice is not in it (the runtime reads config.toml)
    assert!(!text.lines().any(|l| l.starts_with("model =") || l.starts_with("agent_model =")));
}

#[test]
fn the_handoff_is_small_and_flat_for_the_bend_reader() {
    let text = Setup::from_text(None, &no_env).handoff_toml();
    assert!(text.lines().count() < 400, "{} lines", text.lines().count());
    // core/config.bend: one `key = value` per line, [section] headers,
    // # comments; no inline tables, arrays or multi-line strings
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || (l.starts_with('[') && l.ends_with(']')) {
            continue;
        }
        let (_, v) = l.split_once(" = ").unwrap_or_else(|| panic!("{l}"));
        assert!(!v.starts_with('{') && !v.starts_with('[') && !v.starts_with("\"\"\""), "{l}");
    }
    // a model section only carries what differs from its provider
    assert!(text.contains("[models.\"anthropic/claude-sonnet-4-5\"]\nmax_output = 64000\n\n"));
}

#[test]
fn write_handoff_is_atomic_and_export_falls_back() {
    let dir = std::env::temp_dir().join(format!("bise-catalog-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = dir.join("config.toml");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&cfg, "model = \"openai/gpt-5\"\n").unwrap();
    let p = export_handoff(&cfg, &dir.join("cache")).unwrap();
    assert_eq!(p, dir.join("cache/models.toml"));
    assert!(std::fs::read_to_string(&p).unwrap().contains("[providers.openai]"));
    let left: Vec<_> = std::fs::read_dir(dir.join("cache")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // a cache dir that cannot be made (a file is in the way): the temp dir
    std::fs::write(dir.join("blocked"), "").unwrap();
    let p = export_handoff(&cfg, &dir.join("blocked/cache")).unwrap();
    assert!(p.starts_with(std::env::temp_dir()));
    let _ = std::fs::remove_file(&p);
    // no config file at all: fine
    assert!(export_handoff(&dir.join("none.toml"), &dir.join("cache")).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_listing_shows_keys_choices_and_warnings() {
    let s = setup("model = \"anthropic/claude-sonnet-4-5\"\nagent_model = \"work/x\"\n[providers.work]\nbase_url = \"http://w\"\n[models.\"z\"]\n");
    let env = |k: &str| (k == "ANTHROPIC_API_KEY").then(|| "sk".to_string());
    let mut store = crate::auth::Store::default();
    store.set("groq", "gsk-secret-1");
    let keys = crate::auth::Keys { env: &env, store: &store, files: &[] };
    let out = cli::render(&s, None, &keys, None);
    assert!(out.contains("model        anthropic/claude-sonnet-4-5  (config; listed)"), "{out}");
    assert!(out.contains("agent_model  work/x  (config; not listed: work's defaults)"), "{out}");
    assert!(out.contains("key: env ANTHROPIC_API_KEY"), "{out}");
    assert!(out.contains("key: auth.json"), "{out}");
    assert!(!out.contains("gsk-secret-1") && !out.contains("sk\n"), "{out}");
    assert!(out.contains("no key (OPENAI_API_KEY or 'bise login openai')"), "{out}");
    assert!(out.contains("ollama  Ollama (local) · openai-chat · no key needed"), "{out}");
    assert!(out.contains("not usable yet (BISE-149)"), "{out}");
    assert!(out.contains("from config.toml"), "{out}");
    assert!(out.contains("warning: config.toml: models.\"z\""), "{out}");
    // a filter keeps matching providers and models
    let out = cli::render(&s, Some("gpt-oss"), &keys, None);
    assert!(out.contains("groq/openai/gpt-oss-120b") && out.contains("cerebras/gpt-oss-120b"), "{out}");
    assert!(!out.contains("anthropic/claude-opus-4-5"), "{out}");
    let s = setup("model = \"nowhere/x\"\n");
    let empty = crate::auth::Store::default();
    let keys = crate::auth::Keys { env: &no_env, store: &empty, files: &[] };
    assert!(cli::render(&s, None, &keys, None).contains("unknown provider 'nowhere'"));
}

#[test]
fn token_counts_read_short() {
    assert_eq!(cli::tokens(128_000), "128k");
    assert_eq!(cli::tokens(1_000_000), "1M");
    assert_eq!(cli::tokens(1_047_576), "1.05M");
    assert_eq!(cli::tokens(950), "950");
    assert_eq!(cli::tokens(32_768), "32k");
}

#[test]
fn small_model_order() {
    // the agents' provider's small model by default
    let s = setup("model = \"anthropic/claude-opus-4-5\"\n");
    assert_eq!(s.small_model, "anthropic/claude-haiku-4-5");
    assert_eq!(s.small_model_from, "provider");
    // it follows agent_model's provider, not model's
    let s = setup("model = \"anthropic/claude-opus-4-5\"\nagent_model = \"openai/gpt-5\"\n");
    assert_eq!(s.small_model, "openai/gpt-5-mini");
    // the default setup (foundry) has one
    let s = setup("");
    assert_eq!(s.small_model, "foundry/claude-haiku-4-5");
    // a provider without one: agent_model
    let s = setup("model = \"groq/openai/gpt-oss-120b\"\n");
    assert_eq!(s.small_model, "groq/openai/gpt-oss-120b");
    assert_eq!(s.small_model_from, "agent_model");
    // config, then env, win
    let s = setup("small_model = \"openai/gpt-5-mini\"\n");
    assert_eq!(s.small_model, "openai/gpt-5-mini");
    assert_eq!(s.small_model_from, "config");
    let s = Setup::from_text(Some("small_model = \"openai/gpt-5-mini\"\n"), &|k| {
        (k == "BISE_SMALL_MODEL").then(|| "opus-5.5".to_string())
    });
    assert_eq!(s.small_model, "foundry/claude-opus-5-5");
    assert_eq!(s.small_model_from, "BISE_SMALL_MODEL");
    // a provider of the config may name its own
    let s = setup("model = \"acme/big\"\n[providers.acme]\nbase_url = \"http://x\"\nsmall_model = \"tiny\"\n");
    assert_eq!(s.small_model, "acme/tiny");
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
}

#[test]
fn thinking_and_betas_per_model_reach_the_handoff() {
    // built in: the direct API thinks with a budget, the foundry proxy
    // keeps the family's defaults (nothing written: today's body/headers)
    let c = Catalog::builtin();
    let r = c.resolve("anthropic/claude-haiku-4-5");
    assert_eq!((r.caps.thinking.as_str(), r.caps.betas.as_str(), r.caps.max_output), ("budget", ANTH_BETAS, 64_000));
    let f = c.resolve("foundry/claude-opus-5-5");
    assert_eq!((f.caps.thinking.as_str(), f.caps.betas.as_str(), f.caps.max_output), ("", "", 32_768));
    // config: per model, inherited from the provider, bad values warned
    let cfg = "[models.\"anthropic/claude-opus-4-6\"]\nthinking = \"adaptive\"\n[models.\"anthropic/x\"]\nthinking = \"high\"\nbetas = 3\n";
    let s = Setup::from_text(Some(cfg), &|_| None);
    assert_eq!(s.catalog.resolve("anthropic/claude-opus-4-6").caps.thinking, "adaptive");
    assert_eq!(s.catalog.resolve("anthropic/x").caps.thinking, "budget", "a bad value keeps the provider's");
    assert!(s.catalog.warnings.iter().any(|w| w.contains("thinking: one of adaptive, budget, none")), "{:?}", s.catalog.warnings);
    assert!(s.catalog.warnings.iter().any(|w| w.contains("betas: a string")), "{:?}", s.catalog.warnings);
    let h = s.handoff_toml();
    let anth = &h[h.find("[providers.anthropic]").unwrap()..];
    let anth = &anth[..anth[1..].find("\n[").unwrap()];
    assert!(anth.contains("thinking = \"budget\"\n") && anth.contains(&format!("betas = \"{}\"", ANTH_BETAS)), "{anth}");
    let foundry = &h[h.find("[providers.foundry]").unwrap()..];
    let foundry = &foundry[..foundry[1..].find("\n[").unwrap()];
    assert!(!foundry.contains("thinking") && !foundry.contains("betas"), "{foundry}");
    assert!(h.contains("[models.\"anthropic/claude-opus-4-6\"]\nthinking = \"adaptive\"\n"), "{h}");
}

#[test]
fn prices_come_from_the_model_then_its_provider() {
    let c = Catalog::builtin();
    let p = c.resolve("anthropic/claude-sonnet-4-5").price;
    assert_eq!(p, Price { input: Some(3_000_000), output: Some(15_000_000), cache_read: Some(300_000), cache_write: Some(3_750_000) });
    // nothing listed: no price, no cost
    assert_eq!(c.resolve("foundry/claude-opus-5-5").price, Price::default());
    assert_eq!(c.resolve("nowhere/m").price.cost(10, 10, 0, 0), None);
    // a config provider's prices are its models' defaults; a model's win
    let s = setup(
        "[providers.acme]\nbase_url = \"http://x\"\ninput_price = 1\noutput_price = 2.5\n\
         [models.\"acme/big\"]\noutput_price = 10\n",
    );
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let small = s.catalog.resolve("acme/small").price;
    assert_eq!((small.input, small.output), (Some(1_000_000), Some(2_500_000)));
    let big = s.catalog.resolve("acme/big").price;
    assert_eq!((big.input, big.output), (Some(1_000_000), Some(10_000_000)));
    // cost: cached tokens at their price, the rest at the input price
    let c = big.cost(1_000_000, 100_000, 0, 0).unwrap();
    assert!((c - 2.0).abs() < 1e-9, "{c}");
    // bad prices are warnings
    let s = setup("[models.\"acme/x\"]\ninput_price = -1\noutput_price = \"cheap\"\n");
    assert_eq!(s.catalog.warnings.len(), 2, "{:?}", s.catalog.warnings);
}

#[test]
fn the_default_threshold_is_80_percent_of_the_window() {
    let c = Catalog::builtin();
    assert_eq!(c.default_threshold("foundry/claude-opus-5-5"), 800_000);
    assert_eq!(c.default_threshold("anthropic/claude-sonnet-4-5"), 160_000);
    assert_eq!(c.default_threshold("nowhere/m"), 102_400);
    // the same rounding as runtime/provider-pure.bend threshold_of
    assert_eq!(threshold_of(131_072), 104_857);
    assert_eq!(threshold_of(0), 0);
}
