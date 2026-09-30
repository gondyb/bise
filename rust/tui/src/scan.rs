//! `bise setup scan` (BISE-273): what an install prompt needs to set bise
//! up from another agent's setup, read in one go and never a secret. The
//! agent that follows bise.dev/setup.md runs it instead of opening the
//! files that hold keys and tokens (shell rc files, `.env`,
//! `~/.claude.json`, `~/.codex/config.toml`): the output names where a
//! key is, which MCP servers exist, never a value.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// An environment lookup (the real one, or a map in the tests).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// One provider key found: the provider, its variable, where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyAt {
    pub provider: String,
    pub var: String,
    /// "env" or a file path
    pub at: String,
}

/// An MCP server of another agent, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mcp {
    pub name: String,
    /// "stdio", "http", "sse"…
    pub kind: String,
}

/// Claude Code's or Codex's setup, as far as bise cares.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Agent {
    pub found: bool,
    pub model: Option<String>,
    /// its global instructions file, when it has text
    pub instructions: Option<PathBuf>,
    pub skills: Vec<String>,
    pub mcp: Vec<Mcp>,
    pub repos: Vec<PathBuf>,
}

/// The shell files a key is often exported from.
const RC_FILES: [&str; 8] =
    [".zshrc", ".zprofile", ".zshenv", ".bashrc", ".bash_profile", ".profile", ".config/fish/config.fish", ".env"];

/// `NAME=value`, `export NAME=value`, fish's `set -gx NAME value`: the
/// value of `name` in `text` (non-empty), else None.
pub(crate) fn var_in(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        let l = l.strip_prefix("export ").unwrap_or(l);
        let v = if let Some(rest) = l.strip_prefix("set ") {
            let mut w = rest.split_whitespace().filter(|w| !w.starts_with('-'));
            (w.next() == Some(name)).then(|| w.collect::<Vec<_>>().join(" "))
        } else {
            l.split_once('=').filter(|(k, _)| k.trim() == name).map(|(_, v)| v.to_string())
        };
        let v = v.map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string()).filter(|v| !v.is_empty() && !v.starts_with('$'));
        if v.is_some() {
            return v;
        }
    }
    None
}

/// The providers' keys: the environment, the shell files, the repos'
/// `.env` / `.env.local`. Every place a key is, in that order.
pub(crate) fn keys(env: Env, home: &Path, setup: &bise_catalog::Setup, repos: &[PathBuf]) -> Vec<KeyAt> {
    let mut files: Vec<PathBuf> = RC_FILES.iter().map(|f| home.join(f)).collect();
    for r in repos {
        files.push(r.join(".env"));
        files.push(r.join(".env.local"));
    }
    let texts: Vec<(PathBuf, String)> = files.into_iter().filter_map(|f| std::fs::read_to_string(&f).ok().map(|t| (f, t))).collect();
    let mut out = Vec::new();
    for p in crate::onboarding::key_providers(setup) {
        for var in bise_catalog::auth::env_names(&p.key_env) {
            if env(var).is_some_and(|v| !v.trim().is_empty()) {
                out.push(KeyAt { provider: p.id.clone(), var: var.to_string(), at: "env".into() });
            }
            for (f, t) in &texts {
                if var_in(t, var).is_some() {
                    out.push(KeyAt { provider: p.id.clone(), var: var.to_string(), at: tilde(f, home) });
                }
            }
        }
    }
    out
}

fn tilde(p: &Path, home: &Path) -> String {
    bise_catalog::auth::tilde(p, Some(home))
}

fn has_text(p: &Path) -> Option<PathBuf> {
    std::fs::read_to_string(p).ok().filter(|t| !t.trim().is_empty()).map(|_| p.to_path_buf())
}

/// The skill folders (a SKILL.md inside) of `dir`, sorted.
fn skills_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("SKILL.md").is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

fn mcp_of(map: Option<&Value>) -> Vec<Mcp> {
    let Some(m) = map.and_then(Value::as_object) else { return Vec::new() };
    m.iter()
        .map(|(name, s)| {
            let kind = s.get("type").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| {
                if s.get("command").is_some() { "stdio" } else { "http" }.to_string()
            });
            Mcp { name: name.clone(), kind }
        })
        .collect()
}

/// Claude Code: `~/.claude/` (settings.json's model, CLAUDE.md, skills)
/// and `~/.claude.json` (the user's MCP servers, the projects).
pub(crate) fn claude(home: &Path) -> Agent {
    let dir = home.join(".claude");
    let json = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let settings = json(dir.join("settings.json"));
    let state = json(home.join(".claude.json"));
    let mut repos: Vec<PathBuf> = state
        .as_ref()
        .and_then(|s| s.get("projects")?.as_object().map(|o| o.keys().map(PathBuf::from).filter(|p| p.is_dir()).collect()))
        .unwrap_or_default();
    repos.sort();
    Agent {
        found: dir.is_dir() || state.is_some(),
        model: settings.as_ref().and_then(|s| s.get("model")?.as_str().map(str::to_string)),
        instructions: has_text(&dir.join("CLAUDE.md")),
        skills: skills_in(&dir.join("skills")),
        mcp: mcp_of(state.as_ref().and_then(|s| s.get("mcpServers"))),
        repos,
    }
}

/// Codex: `~/.codex/` (`$CODEX_HOME`): config.toml's model, its
/// `[mcp_servers.*]` and `[projects."…"]` tables, AGENTS.md, skills.
/// Read line by line: names only, no value is kept.
pub(crate) fn codex(env: Env, home: &Path) -> Agent {
    let dir = env("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
    let text = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    let (mut model, mut mcp, mut repos) = (None, Vec::new(), Vec::new());
    let mut table = String::new();
    let unq = |s: &str| s.trim().trim_matches('"').trim_matches('\'').to_string();
    for l in text.lines().map(str::trim) {
        if let Some(t) = l.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            table = t.trim().to_string();
            if let Some(n) = table.strip_prefix("mcp_servers.") {
                if !n.contains('.') || n.starts_with('"') {
                    mcp.push(Mcp { name: unq(n), kind: "stdio".into() });
                }
            } else if let Some(p) = table.strip_prefix("projects.") {
                let p = PathBuf::from(unq(p));
                if p.is_dir() {
                    repos.push(p);
                }
            }
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let k = k.trim();
        if table.is_empty() && k == "model" {
            model = Some(unq(v.split('#').next().unwrap_or("")));
        }
        if k == "url" && table.starts_with("mcp_servers.") {
            if let Some(m) = mcp.last_mut() {
                m.kind = "http".into();
            }
        }
    }
    Agent {
        found: dir.is_dir(),
        model,
        instructions: has_text(&dir.join("AGENTS.md")),
        skills: skills_in(&dir.join("skills")),
        mcp,
        repos,
    }
}

/// The catalog model closest to another agent's choice: Claude Code's
/// `opus`/`sonnet`/`haiku` (or a full id) → the first listed anthropic
/// model of that family; Codex's `gpt-5.5` → `openai/gpt-5.5` when
/// listed. None: no match.
pub(crate) fn model_like(setup: &bise_catalog::Setup, provider: &str, theirs: &str) -> Option<String> {
    let t = theirs.trim().to_ascii_lowercase();
    let t = t.split('[').next().unwrap_or(&t).to_string(); // claude-opus-4-8[1m]
    let ms: Vec<&bise_catalog::Model> =
        setup.catalog.models.iter().filter(|m| m.provider == provider && !m.stt).collect();
    if let Some(m) = ms.iter().find(|m| m.id.to_ascii_lowercase() == t) {
        return Some(m.name());
    }
    let family = ["opus", "sonnet", "haiku"].into_iter().find(|f| t.contains(f));
    match family {
        Some(f) => ms.iter().find(|m| m.id.contains(f)).map(|m| m.name()),
        None => None,
    }
}

/// The nearest folder from `dir` up holding a `.git`.
fn git_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find(|d| d.join(".git").exists()).map(Path::to_path_buf)
}

/// The whole scan as text (what `bise setup scan` prints).
pub(crate) fn render(env: Env, home: &Path, bise: &bise_home::Home, cwd: &Path) -> String {
    let setup = crate::onboarding::setup_of(env, bise);
    let (cl, cx) = (claude(home), codex(env, home));
    let mut repos: Vec<PathBuf> = Vec::new();
    if let Some(r) = git_root(cwd) {
        repos.push(r);
    }
    for r in cl.repos.iter().chain(&cx.repos) {
        if !repos.contains(r) {
            repos.push(r.clone());
        }
    }
    let found = keys(env, home, &setup, &repos);
    let t = |p: &Path| tilde(p, home);
    let mut o = String::from("bise setup scan · names and places only, never a key\n\n");
    o.push_str("keys (provider  variable  where):\n");
    if found.is_empty() {
        o.push_str("  none found: the user gets a key from a provider (bise's first run asks for it)\n");
    }
    for k in &found {
        o.push_str(&format!("  {:<11} {:<20} {}\n", k.provider, k.var, k.at));
    }
    let paths = crate::onboarding::auth_paths(bise);
    let store = bise_catalog::auth::Store::read(&paths.auth_file).unwrap_or_default();
    let saved = store.providers();
    o.push_str(&format!("  saved in bise: {}\n", if saved.is_empty() { "none".to_string() } else { saved.join(", ") }));
    let has_model = std::fs::read_to_string(bise.config_file()).is_ok_and(|t| bise_catalog::config_cli::get(&t, "model").is_some());
    o.push_str(&format!(
        "\nbise: model {}{} · {}\n",
        setup.model,
        if has_model { " (config.toml)" } else { " (default)" },
        match has_text(&bise.root().join("AGENTS.md")) {
            Some(p) => format!("{} exists", t(&p)),
            None => format!("{} missing", t(&bise.root().join("AGENTS.md"))),
        }
    ));
    for (name, a, prov) in [("claude code", &cl, "anthropic"), ("codex", &cx, "openai")] {
        if !a.found {
            o.push_str(&format!("\n{}: not found\n", name));
            continue;
        }
        o.push_str(&format!("\n{}:\n", name));
        if let Some(m) = &a.model {
            let like = model_like(&setup, prov, m).map(|x| format!(" → bise: {}", x)).unwrap_or_default();
            o.push_str(&format!("  model        {}{}\n", m, like));
        }
        if let Some(p) = &a.instructions {
            o.push_str(&format!("  instructions {}\n", t(p)));
        }
        if !a.skills.is_empty() {
            o.push_str(&format!("  skills       {}\n", a.skills.join(", ")));
        }
        if !a.mcp.is_empty() {
            let l: Vec<String> = a.mcp.iter().map(|m| if m.kind == "stdio" { m.name.clone() } else { format!("{} ({}, not imported)", m.name, m.kind) }).collect();
            o.push_str(&format!("  mcp servers  {}\n", l.join(", ")));
        }
    }
    let have = skills_in(&home.join(".agents/skills"));
    if !have.is_empty() {
        o.push_str(&format!("\n~/.agents/skills (bise reads it): {}\n", have.join(", ")));
    }
    o.push_str("\nrepos:\n");
    if repos.is_empty() {
        o.push_str("  none found\n");
    }
    for r in &repos {
        let has = |f: &str| r.join(f).is_file();
        let doc = match (has("AGENTS.md"), has("CLAUDE.md")) {
            (true, _) => "AGENTS.md",
            (false, true) => "CLAUDE.md only",
            _ => "no AGENTS.md",
        };
        o.push_str(&format!("  {}  {}\n", t(r), doc));
    }
    let term = env("TERM_PROGRAM").unwrap_or_default();
    o.push_str(&format!("\nterminal: {}\n", if term.is_empty() { "unknown" } else { &term }));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-ant-SECRET-42";

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-scan-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(p: PathBuf, t: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, t).unwrap();
    }

    #[test]
    fn shell_lines_give_their_value() {
        assert_eq!(var_in("export A=\"x1\"\n", "A").as_deref(), Some("x1"));
        assert_eq!(var_in("set -gx A x2\n", "A").as_deref(), Some("x2"));
        assert_eq!(var_in("# export A=x\nAB=y\nA=$(pass a)\n", "A"), None, "a comment, another name, a command are not keys");
        assert_eq!(var_in("A=\n", "A"), None);
    }

    #[test]
    fn the_scan_names_places_and_servers_never_values() {
        let h = tmp("all");
        let repo = h.join("work/app");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        put(repo.join("CLAUDE.md"), "# app\n");
        put(repo.join(".env"), "OPENAI_API_KEY=sk-openai-SECRET\n");
        put(h.join(".zshrc"), &format!("export ANTHROPIC_API_KEY=\"{SECRET}\"\n"));
        put(h.join(".claude/settings.json"), r#"{"model":"opus"}"#);
        put(h.join(".claude/CLAUDE.md"), "rules\n");
        put(h.join(".claude/skills/review-pr/SKILL.md"), "---\nname: review-pr\n---\n");
        let cj = serde_json::json!({"mcpServers": {"github": {"command": "npx", "env": {"T": "ghp_SECRET"}}, "linear": {"type": "http", "url": "u"}},
            "projects": {repo.to_string_lossy(): {}}, "oauthAccount": {"x": 1}});
        put(h.join(".claude.json"), &cj.to_string());
        put(h.join(".codex/config.toml"), "model = \"gpt-6-astra\" # mine\n[mcp_servers.fs]\ncommand = \"npx\"\n[mcp_servers.fs.env]\nTOKEN = \"tok-SECRET\"\n[mcp_servers.remote]\nurl = \"https://x\"\n");
        let vars = [("HOME", h.to_string_lossy().to_string()), ("BISE_HOME", h.join(".bise").to_string_lossy().to_string()), ("TERM_PROGRAM", "ghostty".into())];
        let env = move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
        let bise = bise_home::Home::from_lookup(&env);
        let out = render(&env, &h, &bise, &repo);
        for secret in [SECRET, "sk-openai-SECRET", "ghp_SECRET", "tok-SECRET"] {
            assert!(!out.contains(secret), "{out}");
        }
        for s in ["anthropic   ANTHROPIC_API_KEY    ~/.zshrc", "openai      OPENAI_API_KEY       ~/work/app/.env", "saved in bise: none",
                  "instructions ~/.claude/CLAUDE.md", "skills       review-pr", "mcp servers  github, linear (http, not imported)",
                  "mcp servers  fs, remote (http, not imported)", "~/work/app  CLAUDE.md only", "terminal: ghostty"] {
            assert!(out.contains(s), "missing {s:?} in\n{out}");
        }
        assert_eq!(out.matches("~/work/app  ").count(), 1, "the repo once: cwd and ~/.claude.json\n{out}");
        assert!(out.contains("model        opus → bise: anthropic/claude-opus"), "{out}");
        assert!(out.contains("model        gpt-6-astra → bise: openai/gpt-6-astra"), "{out}");
        let _ = std::fs::remove_dir_all(&h);
    }
}
