//! `bise models [filter]`: the models bise knows, and which keys are set.

use std::path::Path;

use crate::{Catalog, Known, Provider, Resolved, Setup, Source};

/// "128k", "1M", "1.04M"
pub fn tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        let s = format!("{:.2}", m);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{}M", s)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

/// The state of a provider's key, for the list.
pub fn key_state(p: &Provider, env: &dyn Fn(&str) -> Option<String>) -> String {
    if !p.needs.is_empty() {
        return format!("not usable yet ({})", p.needs);
    }
    if p.key_env.is_empty() {
        return "no key needed".into();
    }
    let set = env(&p.key_env).is_some_and(|v| !v.trim().is_empty());
    format!("{} {}", p.key_env, if set { "set" } else { "not set" })
}

fn caps_text(r: &Resolved) -> String {
    let c = &r.caps;
    let mut s = format!("{:>6} ctx {:>5} out", tokens(c.context), tokens(c.max_output));
    for (on, w) in [(c.vision, "vision"), (c.reasoning, "reasoning"), (!c.tools, "no-tools")] {
        if on {
            s.push(' ');
            s.push_str(w);
        }
    }
    s
}

fn choice_line(label: &str, c: &Catalog, name: &str, from: &str) -> String {
    let r = c.resolve(name);
    let what = match r.known {
        Known::Listed => "listed".to_string(),
        Known::Unlisted => format!("not listed: {}'s defaults", r.provider),
        Known::NoProvider => format!("unknown provider '{}': add [providers.{}] to config.toml", r.provider, r.provider),
    };
    format!("{:<12} {}  ({}; {})\n             {}\n", label, r.name, from, what, caps_text(&r))
}

/// The whole listing, pure (tests).
pub fn render(s: &Setup, filter: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> String {
    let c = &s.catalog;
    let f = filter.map(|f| f.to_ascii_lowercase());
    let hit = |x: &str| f.as_deref().is_none_or(|f| x.to_ascii_lowercase().contains(f));
    let mut o = String::new();
    o.push_str(&choice_line("model", c, &s.model, s.model_from));
    let agent_from = if s.agent_model_from == "model" { "same as model" } else { s.agent_model_from };
    o.push_str(&choice_line("agent_model", c, &s.agent_model, agent_from));
    for p in &c.providers {
        let models: Vec<_> = c.models.iter().filter(|m| m.provider == p.id).collect();
        let p_hit = hit(&p.id) || hit(&p.name);
        let shown: Vec<_> = models.iter().filter(|m| p_hit || hit(&m.name())).collect();
        if !p_hit && shown.is_empty() {
            continue;
        }
        let custom = if p.source == Source::Config { ", from config.toml" } else { "" };
        o.push_str(&format!(
            "\n{}  {} · {} · {}{}\n",
            p.id,
            p.name,
            p.api,
            key_state(p, env),
            custom
        ));
        for m in shown {
            let r = c.resolve(&m.name());
            let mark = if m.source == Source::Config { "  (config)" } else { "" };
            o.push_str(&format!("  {:<52} {}{}\n", m.name(), caps_text(&r), mark));
        }
        let base = Resolved {
            caps: c.provider_caps(p),
            ..c.resolve(&format!("{}/-", p.id))
        };
        o.push_str(&format!("  {:<52} {}\n", format!("{}/<any other>", p.id), caps_text(&base)));
    }
    if !c.warnings.is_empty() {
        o.push('\n');
        for w in &c.warnings {
            o.push_str(&format!("warning: {}\n", w));
        }
    }
    o
}

const USAGE: &str = "usage: bise models [filter]
  Lists the providers and models bise knows (built in, plus config.toml's
  [providers.<id>] and [models.\"<provider>/<model>\"]), whether each
  provider's key is set, and the models in use (model, agent_model).
  Any \"<provider>/<model>\" works, listed or not.";

/// `bise models [filter]`, reading `config`; returns the exit code.
pub fn main(args: &[String], config: &Path) -> i32 {
    let mut filter = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{}", USAGE);
                return 0;
            }
            s if s.starts_with('-') || filter.is_some() => {
                eprintln!("{}", USAGE);
                return 2;
            }
            s => filter = Some(s.to_string()),
        }
    }
    let setup = Setup::load(config);
    print!("{}", render(&setup, filter.as_deref(), &|k| std::env::var(k).ok()));
    println!("\nconfig: {}", config.display());
    0
}
