//! The saved rules: `~/.bise/approvals.toml` (design §5.4, approvals.md §5
//! format with `prefix` → `pattern`). Reading and rendering are pure;
//! `load` and `save` are the thin file edges.
//!
//! ```toml
//! [[allow]]
//! project = "/Users/me/lab/api"   # the git common root; absent: every project
//! tool = "bash"
//! pattern = "cargo test *"        # or the exact text of one part
//! added = "2026-10-12T14:03:00Z"
//! from = "card #12, api-v2"
//!
//! [[allow]]
//! tool = "bash"
//! pattern = "cp *"
//! sandbox = false                 # runs outside the sandbox (a sandbox card's "always")
//!
//! [[allow]]
//! tool = "apply_patch"             # the edit tools share their rules
//! path = "/Users/me/notes/"        # a folder outside the roots
//! ```

use std::path::{Path, PathBuf};

/// One "always allow" rule.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rule {
    /// The git common root it applies to; `None`: every project.
    pub project: Option<PathBuf>,
    /// `bash`, an edit tool, or a connector tool (`gmail.send_email`).
    pub tool: String,
    /// bash: an arity pattern (`cargo test *`) or the exact text of a part.
    pub pattern: Option<String>,
    /// edit tools: a folder (or file) the edits may touch.
    pub path: Option<PathBuf>,
    pub added: Option<String>,
    pub from: Option<String>,
    /// `Some(false)`: a bash rule saved on a sandbox card: its parts run
    /// outside the sandbox (brief 1e). `None`: the sandbox applies.
    pub sandbox: Option<bool>,
}

/// Every saved rule (the whole file; `judge` keeps the call's project's).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rules {
    pub rules: Vec<Rule>,
}

/// The edit tools: one family for rules (a request has one of them).
pub const EDIT_TOOLS: [&str; 3] = ["edit", "write_file", "apply_patch"];

impl Rule {
    fn applies(&self, tool: &str, repo: &Path) -> bool {
        let same_tool = self.tool == tool
            || (EDIT_TOOLS.contains(&tool) && EDIT_TOOLS.contains(&self.tool.as_str()));
        same_tool && self.project.as_deref().is_none_or(|p| p == repo)
    }
}

impl Rules {
    /// A bash part's text matches a rule of this repo.
    pub fn allows_bash(&self, repo: &Path, text: &str, exact: &str, readable: bool) -> bool {
        self.rules
            .iter()
            .filter(|r| r.applies("bash", repo))
            .filter_map(|r| r.pattern.as_deref())
            .any(|p| {
                // an unreadable part is never allowed by a `*` (design §5.2)
                if p.ends_with(" *") {
                    readable && super::arity::matches(p, text)
                } else {
                    p == text || p == exact
                }
            })
    }

    /// A bash part matches a rule saved on a sandbox card (`sandbox =
    /// false`): it runs outside the sandbox.
    pub fn outside_sandbox(&self, repo: &Path, text: &str, exact: &str, readable: bool) -> bool {
        let only = Rules {
            rules: self
                .rules
                .iter()
                .filter(|r| r.sandbox == Some(false))
                .cloned()
                .collect(),
        };
        only.allows_bash(repo, text, exact, readable)
    }

    /// An edit tool may write this path (a `path` rule of this repo).
    pub fn allows_edit(&self, repo: &Path, tool: &str, path: &Path) -> bool {
        self.rules
            .iter()
            .filter(|r| r.applies(tool, repo))
            .filter_map(|r| r.path.as_deref())
            .any(|p| path.starts_with(p))
    }

    /// A connector tool is allowed as a whole.
    pub fn allows_tool(&self, repo: &Path, tool: &str) -> bool {
        self.rules
            .iter()
            .any(|r| r.applies(tool, repo) && r.pattern.is_none() && r.path.is_none())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RulesErr {
    /// Not TOML, or a field of the wrong type: the file is kept as is.
    Syntax(String),
    Io(String),
}

impl std::fmt::Display for RulesErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulesErr::Syntax(e) => write!(f, "approvals.toml: {e}"),
            RulesErr::Io(e) => write!(f, "approvals.toml: {e}"),
        }
    }
}

/// Read the file's text. `[[allow]]` entries with no `tool` are skipped;
/// the old `prefix = "cargo test"` reads as `pattern = "cargo test *"`.
pub fn parse(text: &str) -> Result<Rules, RulesErr> {
    let doc: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| RulesErr::Syntax(e.message().to_string()))?;
    let Some(list) = doc.get("allow") else {
        return Ok(Rules::default());
    };
    let list = list
        .as_array()
        .ok_or_else(|| RulesErr::Syntax("`allow` is not a list of [[allow]] tables".into()))?;
    let s = |t: &toml::Table, k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let rules = list
        .iter()
        .filter_map(|v| v.as_table())
        .filter_map(|t| {
            let tool = s(t, "tool")?;
            let pattern = s(t, "pattern").or_else(|| s(t, "prefix").map(|p| format!("{p} *")));
            Some(Rule {
                project: s(t, "project").map(PathBuf::from),
                tool,
                pattern,
                path: s(t, "path").map(PathBuf::from),
                added: s(t, "added"),
                from: s(t, "from"),
                sandbox: t.get("sandbox").and_then(|v| v.as_bool()),
            })
        })
        .collect();
    Ok(Rules { rules })
}

const HEADER: &str = "# ~/.bise/approvals.toml: what bise runs without asking you.\n# Written when you pick \"always allow\" on a card; edit freely.\n";

/// The file's text with one more rule at its end (the rest kept byte for
/// byte, comments included).
pub fn append(text: &str, rule: &Rule) -> String {
    let mut out = if text.trim().is_empty() {
        HEADER.to_string()
    } else {
        text.to_string()
    };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n[[allow]]\n");
    let q = |v: &str| toml::Value::String(v.to_string()).to_string();
    if let Some(p) = &rule.project {
        out.push_str(&format!("project = {}\n", q(&p.to_string_lossy())));
    }
    out.push_str(&format!("tool = {}\n", q(&rule.tool)));
    for (k, v) in [
        ("pattern", rule.pattern.clone()),
        (
            "path",
            rule.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        ),
        ("added", rule.added.clone()),
        ("from", rule.from.clone()),
    ] {
        if let Some(v) = v {
            out.push_str(&format!("{k} = {}\n", q(&v)));
        }
    }
    if let Some(b) = rule.sandbox {
        out.push_str(&format!("sandbox = {b}\n"));
    }
    out
}

/// `<bise>/approvals.toml`.
pub fn file(bise: &Path) -> PathBuf {
    bise.join("approvals.toml")
}

/// Read the rules; no file is no rules.
pub fn load(path: &Path) -> Result<Rules, RulesErr> {
    match std::fs::read_to_string(path) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Rules::default()),
        Err(e) => Err(RulesErr::Io(e.to_string())),
    }
}

/// Add a rule to the file: written to a temp file next to it, then
/// renamed (only the user can read it). A file that is not TOML is not
/// touched.
pub fn save(path: &Path, rule: &Rule) -> Result<(), RulesErr> {
    let old = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(RulesErr::Io(e.to_string())),
    };
    parse(&old)?;
    let new = append(&old, rule);
    let tmp = path.with_extension("toml.tmp");
    let io = |e: std::io::Error| RulesErr::Io(e.to_string());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(&tmp, new).map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(io)
}

/// The file's text without the first `[[allow]]` entry that reads as
/// `rule` (every field equal), the rest kept byte for byte; None: no such
/// entry. An entry runs from its `[[allow]]` line to the next one.
pub fn without(text: &str, rule: &Rule) -> Option<String> {
    let mut starts: Vec<usize> = Vec::new();
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        if l.trim() == "[[allow]]" {
            starts.push(at);
        }
        at += l.len();
    }
    let ends = starts.iter().skip(1).copied().chain(std::iter::once(text.len()));
    let (s, e) = starts.iter().copied().zip(ends).find(|&(s, e)| {
        parse(&text[s..e]).is_ok_and(|r| r.rules.len() == 1 && r.rules[0] == *rule)
    })?;
    // the blank line `append` puts before an entry goes with it (an
    // entry before another holds that one's blank line already)
    let s = if e == text.len() && text[..s].ends_with("\n\n") { s - 1 } else { s };
    Some(format!("{}{}", &text[..s], &text[e..]))
}

/// Remove a rule from the file (`/approvals`, backspace): same writes as
/// [`save`]. `Ok(false)`: no entry reads as it (edited meanwhile).
pub fn remove(path: &Path, rule: &Rule) -> Result<bool, RulesErr> {
    let old = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(RulesErr::Io(e.to_string())),
    };
    parse(&old)?;
    let Some(new) = without(&old, rule) else { return Ok(false) };
    let tmp = path.with_extension("toml.tmp");
    let io = |e: std::io::Error| RulesErr::Io(e.to_string());
    std::fs::write(&tmp, new).map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(io)?;
    Ok(true)
}
