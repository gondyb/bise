//! Discovery, manifest validation, precedence and components. Pure over
//! the file system: no process is started here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub const PLUGIN_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
pub const MCP_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";
pub const RESERVED: [&str; 6] = ["file_system", "process", "self", "skill", "subagent", "vibe"];

/// The user's home (`~/.agents/plugins` is under it).
pub fn home() -> PathBuf {
    bise_home::Home::from_env().user_home().to_path_buf()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    User,
    Workspace,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// the plugin is dropped
    Error,
    /// one component is dropped
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    /// the plugin name when known, else its folder
    pub plugin: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Loaded,
    Disabled,
    Shadowed,
    Invalid,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Loaded => "loaded",
            State::Disabled => "disabled",
            State::Shadowed => "shadowed",
            State::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    /// `<namespace>:<name>`
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StdioServer {
    pub id: String,
    /// resolved: a bare executable name or an absolute path
    pub command: String,
    pub args: Vec<String>,
    /// the declared env, placeholders expanded (PLUGIN_ROOT/DATA are
    /// added at spawn time)
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plugin {
    /// the manifest name; the folder name when the manifest is invalid
    pub name: String,
    pub namespace: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub scope: Scope,
    pub root: PathBuf,
    pub data_root: PathBuf,
    pub state: State,
    pub skills: Vec<Skill>,
    pub servers: Vec<StdioServer>,
    /// components present on disk but not supported yet
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Resolution {
    pub plugins: Vec<Plugin>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Resolution {
    pub fn loaded(&self) -> impl Iterator<Item = &Plugin> {
        self.plugins.iter().filter(|p| p.state == State::Loaded)
    }
}

#[derive(Clone, Debug)]
pub struct Roots {
    pub user: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    /// where `${PLUGIN_DATA}` roots live (one folder per plugin name)
    pub data: PathBuf,
    pub disabled: Vec<String>,
}

impl Roots {
    /// The standard roots: `$BEND_PLUGINS_HOME` or `~/.agents/plugins`,
    /// and `<workspace>/.agents/plugins`.
    pub fn standard(workspace: Option<&Path>) -> Roots {
        let user = match std::env::var("BEND_PLUGINS_HOME") {
            Ok(p) if !p.is_empty() => PathBuf::from(p),
            _ => home().join(".agents").join("plugins"),
        };
        let data = bise_home::Home::from_env().plugin_data_dir();
        Roots {
            user: Some(user),
            workspace: workspace.map(|w| w.join(".agents").join("plugins")),
            data,
            disabled: crate::state::disabled(&crate::state::state_path()),
        }
    }
}

fn diag(code: &'static str, severity: Severity, plugin: &str, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        plugin: plugin.to_string(),
        message,
    }
}

/// Any character outside `[A-Za-z0-9_$]` becomes `_`; a leading digit
/// gets a `_` in front; empty becomes `_`.
pub fn identifier(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    let ok_char = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-';
    let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= 64
        && b.iter().all(|&c| ok_char(c))
        && alnum(b[0])
        && alnum(b[b.len() - 1])
        && !name.contains("--")
        && !name.contains("..")
}

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub extensions: Vec<String>,
}

fn opt_string(o: &Map<String, Value>, k: &str) -> Result<Option<String>, String> {
    match o.get(k) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("/{} must be a string", k)),
    }
}

/// The closed 1.0.0 manifest schema.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {}", e))?;
    let o = v.as_object().ok_or("the manifest must be a JSON object")?;
    for k in o.keys() {
        let known = [
            "$schema", "name", "version", "description", "author", "homepage", "repository",
            "license", "keywords", "extensions",
        ];
        if !known.contains(&k.as_str()) {
            return Err(format!("unknown field /{} (the schema is closed)", k));
        }
    }
    match o.get("$schema") {
        Some(Value::String(s)) if s == PLUGIN_SCHEMA => {}
        Some(_) => return Err(format!("/$schema must be \"{}\"", PLUGIN_SCHEMA)),
        None => return Err("missing /$schema".into()),
    }
    let name = match o.get("name") {
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err("/name must be a string".into()),
        None => return Err("missing /name".into()),
    };
    if !valid_name(&name) {
        return Err(format!(
            "/name {:?} must be 1-64 chars of [a-z0-9.-], start and end alphanumeric, no -- or ..",
            name
        ));
    }
    for k in ["homepage", "repository", "license"] {
        opt_string(o, k)?;
    }
    if let Some(a) = o.get("author") {
        let a = a.as_object().ok_or("/author must be an object")?;
        for (k, v) in a {
            if !["name", "email", "url"].contains(&k.as_str()) {
                return Err(format!("unknown field /author/{}", k));
            }
            if !v.is_string() {
                return Err(format!("/author/{} must be a string", k));
            }
        }
    }
    if let Some(kw) = o.get("keywords") {
        let ok = kw.as_array().is_some_and(|a| a.iter().all(Value::is_string));
        if !ok {
            return Err("/keywords must be an array of strings".into());
        }
    }
    let mut extensions = Vec::new();
    if let Some(ex) = o.get("extensions") {
        let ex = ex.as_object().ok_or("/extensions must be an object")?;
        for (k, v) in ex {
            if !v.is_object() {
                return Err(format!("/extensions/{} must be an object", k));
            }
            extensions.push(k.clone());
        }
    }
    Ok(Manifest {
        name,
        version: opt_string(o, "version")?,
        description: opt_string(o, "description")?,
        extensions,
    })
}

/// `name:` and `description:` of a SKILL.md YAML frontmatter. Plain,
/// quoted and folded (`>` / `|`) scalars.
pub fn frontmatter(text: &str) -> Result<(String, String), String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err("no YAML frontmatter (the file must start with ---)".into());
    }
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut closed = false;
    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(k) = &current {
                let e = fields.entry(k.clone()).or_default();
                if !e.is_empty() {
                    e.push(' ');
                }
                e.push_str(line.trim());
            }
            continue;
        }
        current = None;
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_string();
            let v = v.trim();
            let v = if v == ">" || v == "|" || v == ">-" || v == "|-" { "" } else { v };
            let v = v
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(v);
            fields.insert(k.clone(), v.to_string());
            current = Some(k);
        }
    }
    if !closed {
        return Err("the frontmatter is not closed by ---".into());
    }
    let get = |k: &str| fields.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let name = get("name").ok_or("the frontmatter has no name")?;
    let desc = get("description").ok_or("the frontmatter has no description")?;
    Ok((name, desc))
}

fn inside(root: &Path, p: &Path) -> bool {
    match (root.canonicalize(), p.canonicalize()) {
        (Ok(r), Ok(t)) => t.starts_with(r),
        _ => false,
    }
}

fn load_skills(p: &mut Plugin, out: &mut Vec<Diagnostic>) {
    let dir = p.root.join("skills");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for d in entries {
        let file = d.join("SKILL.md");
        if !d.is_dir() || !file.exists() {
            continue;
        }
        let label = d.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if !inside(&p.root, &file) {
            out.push(diag("plugin.path.outside_root", Severity::Warning, &p.name,
                format!("skills/{}/SKILL.md resolves outside the plugin root; skipped", label)));
            continue;
        }
        let parsed = std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|t| frontmatter(&t));
        match parsed {
            Ok((name, desc)) => {
                let full = format!("{}:{}", p.namespace, name);
                if p.skills.iter().any(|s| s.name == full) {
                    out.push(diag("plugin.skill.invalid", Severity::Warning, &p.name,
                        format!("skills/{}: duplicate skill name {:?}; skipped", label, name)));
                    continue;
                }
                p.skills.push(Skill {
                    name: full,
                    description: desc,
                    path: file.canonicalize().unwrap_or(file),
                });
            }
            Err(e) => out.push(diag("plugin.skill.invalid", Severity::Warning, &p.name,
                format!("skills/{}/SKILL.md: {}; skipped", label, e))),
        }
    }
}

fn expand(v: &str, root: &Path, data: &Path) -> String {
    v.replace("${PLUGIN_ROOT}", &root.to_string_lossy())
        .replace("${PLUGIN_DATA}", &data.to_string_lossy())
}

/// Lexical normalization (`.` and `..` segments) for paths that may
/// not exist yet.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

fn contained(base: &Path, p: &Path) -> bool {
    let base = base.canonicalize().unwrap_or_else(|_| normalize(base));
    let target = p.canonicalize().unwrap_or_else(|_| normalize(p));
    target.starts_with(base)
}

fn parse_server(id: &str, v: &Value, root: &Path, data: &Path) -> Result<StdioServer, String> {
    let o = v.as_object().ok_or("must be an object")?;
    let ty = o.get("type").and_then(Value::as_str).ok_or("missing \"type\"")?;
    if ty != "stdio" {
        return Err(format!("unsupported:type {:?} (only stdio is supported yet)", ty));
    }
    for k in o.keys() {
        if !["type", "command", "args", "env", "cwd"].contains(&k.as_str()) {
            return Err(format!("unknown field {:?}", k));
        }
    }
    let raw = o
        .get("command")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("\"command\" must be a non-empty string")?;
    let command = expand(raw, root, data);
    let command = if let Some(rel) = raw.strip_prefix("./") {
        let p = root.join(rel);
        if !contained(root, &p) {
            return Err("\"command\" resolves outside the plugin root".into());
        }
        p.to_string_lossy().to_string()
    } else if raw.starts_with("${PLUGIN_ROOT}/") {
        if !contained(root, Path::new(&command)) {
            return Err("\"command\" resolves outside the plugin root".into());
        }
        command
    } else if raw.contains('/') || raw == "." || raw == ".." {
        return Err("\"command\" must be a bare executable or start with ./".into());
    } else {
        command
    };
    let args = match o.get("args") {
        None => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|s| s.as_str().map(|s| expand(s, root, data)))
            .collect::<Option<Vec<_>>>()
            .ok_or("\"args\" must be an array of strings")?,
        Some(_) => return Err("\"args\" must be an array of strings".into()),
    };
    let mut env = Vec::new();
    if let Some(e) = o.get("env") {
        let e = e.as_object().ok_or("\"env\" must be an object")?;
        for (k, v) in e {
            if k == "PLUGIN_ROOT" || k == "PLUGIN_DATA" {
                return Err(format!("\"env\" must not set {} (reserved)", k));
            }
            let v = v.as_str().ok_or(format!("env {} must be a string", k))?;
            env.push((k.clone(), expand(v, root, data)));
        }
    }
    let cwd = match o.get("cwd") {
        None => root.to_path_buf(),
        Some(Value::String(c)) => {
            let (base, p) = if let Some(rel) = c.strip_prefix("./") {
                (root, root.join(rel))
            } else if c == "${PLUGIN_ROOT}" || c.starts_with("${PLUGIN_ROOT}/") {
                (root, PathBuf::from(expand(c, root, data)))
            } else if c == "${PLUGIN_DATA}" || c.starts_with("${PLUGIN_DATA}/") {
                (data, PathBuf::from(expand(c, root, data)))
            } else {
                return Err("\"cwd\" must start with ./, ${PLUGIN_ROOT} or ${PLUGIN_DATA}".into());
            };
            if !contained(base, &p) {
                return Err("\"cwd\" resolves outside its root".into());
            }
            normalize(&p)
        }
        Some(_) => return Err("\"cwd\" must be a string".into()),
    };
    Ok(StdioServer {
        id: id.to_string(),
        command,
        args,
        env,
        cwd,
    })
}

fn load_mcp(p: &mut Plugin, out: &mut Vec<Diagnostic>) {
    let file = p.root.join("mcp.json");
    if !file.exists() {
        return;
    }
    if !inside(&p.root, &file) {
        out.push(diag("plugin.path.outside_root", Severity::Warning, &p.name,
            "mcp.json resolves outside the plugin root; no MCP server loads".into()));
        return;
    }
    let bad = |m: String| diag("plugin.mcp.invalid", Severity::Warning, &p.name,
        format!("mcp.json: {}; no MCP server loads", m));
    let v: Value = match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| {
        serde_json::from_str(&t).map_err(|e| format!("not JSON: {}", e))
    }) {
        Ok(v) => v,
        Err(e) => return out.push(bad(e)),
    };
    let Some(o) = v.as_object() else {
        return out.push(bad("must be a JSON object".into()));
    };
    if let Some(k) = o.keys().find(|k| *k != "$schema" && *k != "mcpServers") {
        return out.push(bad(format!("unknown field /{} (the schema is closed)", k)));
    }
    if o.get("$schema").and_then(Value::as_str) != Some(MCP_SCHEMA) {
        return out.push(bad(format!("/$schema must be \"{}\"", MCP_SCHEMA)));
    }
    let Some(servers) = o.get("mcpServers").and_then(Value::as_object) else {
        return out.push(bad("/mcpServers must be an object".into()));
    };
    let data = p.data_root.clone();
    for (id, sv) in servers {
        match parse_server(id, sv, &p.root, &data) {
            Ok(s) => p.servers.push(s),
            Err(e) => match e.strip_prefix("unsupported:") {
                Some(why) => {
                    p.unsupported.push(format!("mcp server {}", id));
                    out.push(diag("plugin.component.unsupported", Severity::Warning, &p.name,
                        format!("mcp.json server {:?}: {}; skipped", id, why)));
                }
                None => out.push(diag("plugin.mcp.server_invalid", Severity::Warning, &p.name,
                    format!("mcp.json server {:?}: {}; skipped", id, e))),
            },
        }
    }
}

fn load_unsupported(p: &mut Plugin, extensions: &[String], out: &mut Vec<Diagnostic>) {
    let vibe = p.root.join("ai.mistral.vibe");
    let found = [
        (vibe.join("hooks.toml"), "hooks (ai.mistral.vibe/hooks.toml)"),
        (vibe.join("agents"), "agents (ai.mistral.vibe/agents)"),
        (vibe.join("knowledge"), "knowledge (ai.mistral.vibe/knowledge)"),
        (vibe.join("views"), "views (ai.mistral.vibe/views)"),
        (p.root.join("connectors.json"), "connectors (connectors.json)"),
        (p.root.join("libraries.json"), "libraries (libraries.json)"),
    ];
    for (path, what) in found {
        if path.exists() {
            p.unsupported.push(what.to_string());
            out.push(diag("plugin.component.unsupported", Severity::Info, &p.name,
                format!("{} is not supported yet; ignored", what)));
        }
    }
    for ext in extensions {
        p.unsupported.push(format!("extension {}", ext));
        out.push(diag("plugin.extension.unsupported", Severity::Info, &p.name,
            format!("manifest extension {:?} is not supported; ignored", ext)));
    }
}

struct Candidate {
    plugin: Plugin,
    extensions: Vec<String>,
}

fn discover(root: &Path, scope: Scope, data: &Path, out: &mut Vec<Diagnostic>) -> Vec<Candidate> {
    let rd = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            out.push(diag("plugin.discovery.root_unreadable", Severity::Warning,
                &root.to_string_lossy(), format!("{} root {}: {}", scope.as_str(), root.display(), e)));
            return Vec::new();
        }
    };
    let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        let manifest = dir.join("plugin.json");
        if !manifest.exists() {
            continue;
        }
        let folder = dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let root = dir.canonicalize().unwrap_or(dir.clone());
        let mut p = Plugin {
            name: folder.clone(),
            namespace: identifier(&folder),
            version: None,
            description: None,
            scope,
            root,
            data_root: PathBuf::new(),
            state: State::Invalid,
            skills: Vec::new(),
            servers: Vec::new(),
            unsupported: Vec::new(),
        };
        let parsed = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("unreadable: {}", e))
            .and_then(|t| parse_manifest(&t));
        let mut extensions = Vec::new();
        match parsed {
            Ok(m) => {
                p.namespace = identifier(&m.name);
                p.data_root = data.join(&m.name);
                p.name = m.name;
                p.version = m.version;
                p.description = m.description;
                p.state = State::Loaded;
                extensions = m.extensions;
            }
            Err(e) => out.push(diag("plugin.manifest.invalid", Severity::Error, &folder,
                format!("{}: {}", manifest.display(), e))),
        }
        found.push(Candidate { plugin: p, extensions });
    }
    found
}

/// Discover, validate and load every plugin under `roots`.
pub fn resolve(roots: &Roots) -> Resolution {
    let mut diags = Vec::new();
    let mut cands = Vec::new();
    for (root, scope) in [(&roots.user, Scope::User), (&roots.workspace, Scope::Workspace)] {
        if let Some(r) = root {
            cands.extend(discover(r, scope, &roots.data, &mut diags));
        }
    }
    // same name twice in one root: both dropped
    let mut dup: Vec<(Scope, String)> = Vec::new();
    for (i, a) in cands.iter().enumerate() {
        let p = &a.plugin;
        if p.state == State::Loaded
            && cands[..i].iter().any(|b| b.plugin.state == State::Loaded && b.plugin.scope == p.scope && b.plugin.name == p.name)
            && !dup.contains(&(p.scope, p.name.clone()))
        {
            dup.push((p.scope, p.name.clone()));
        }
    }
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && dup.contains(&(c.plugin.scope, c.plugin.name.clone())) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.name.collision", Severity::Error, &c.plugin.name,
                format!("two {} plugins are named {:?} ({}); both dropped", c.plugin.scope.as_str(),
                    c.plugin.name, c.plugin.root.display())));
        }
    }
    // workspace over user
    let ws_names: Vec<String> = cands
        .iter()
        .filter(|c| c.plugin.state == State::Loaded && c.plugin.scope == Scope::Workspace)
        .map(|c| c.plugin.name.clone())
        .collect();
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && c.plugin.scope == Scope::User && ws_names.contains(&c.plugin.name) {
            c.plugin.state = State::Shadowed;
            diags.push(diag("plugin.shadowed", Severity::Info, &c.plugin.name,
                format!("the workspace plugin {:?} shadows the user one at {}", c.plugin.name, c.plugin.root.display())));
        }
    }
    // namespaces
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && RESERVED.contains(&c.plugin.namespace.as_str()) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.namespace.reserved", Severity::Error, &c.plugin.name,
                format!("the namespace {:?} is reserved", c.plugin.namespace)));
        }
    }
    let live: Vec<(String, String)> = cands
        .iter()
        .filter(|c| c.plugin.state == State::Loaded)
        .map(|c| (c.plugin.name.clone(), c.plugin.namespace.clone()))
        .collect();
    for c in cands.iter_mut() {
        let p = &c.plugin;
        if p.state == State::Loaded && live.iter().any(|(n, ns)| *ns == p.namespace && *n != p.name) {
            c.plugin.state = State::Invalid;
            diags.push(diag("plugin.namespace.collision", Severity::Error, &c.plugin.name,
                format!("another plugin also maps to the namespace {:?}; both dropped", c.plugin.namespace)));
        }
    }
    // enable state, then components
    for c in cands.iter_mut() {
        if c.plugin.state == State::Loaded && roots.disabled.contains(&c.plugin.name) {
            c.plugin.state = State::Disabled;
        }
        if c.plugin.state == State::Loaded {
            load_skills(&mut c.plugin, &mut diags);
            load_mcp(&mut c.plugin, &mut diags);
            load_unsupported(&mut c.plugin, &c.extensions, &mut diags);
        }
    }
    Resolution {
        plugins: cands.into_iter().map(|c| c.plugin).collect(),
        diagnostics: diags,
    }
}

#[cfg(test)]
mod tests;
