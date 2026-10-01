//! What each part of a bash command is (design §3, §5.2, §5.3, §6.3):
//! a hard rule, allowed at once, or open (a saved rule, the cache or the
//! checker decides). Pure: the disk is reached only through `Fs`.

use std::path::{Path, PathBuf};

use super::arity;
use super::parse::{Kind, Part, Target, Word};
use super::paths::{self, Fs, Protected, Roots};
use super::CacheKey;

/// A hard rule (tier 0): a card with no "always". Ordered by which reason
/// the card shows when several apply (designer: 4, 3, 2, 1, 7, 6, 5).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Hard {
    DeletesRoot(RootKind),
    Root,
    ForcePush(String),
    PushMain(String),
    Protected(String),
    Secret(String),
    PipeToShell,
    /// dev-flow §6: a write on the forge (merge, approve, comment, close,
    /// `gh api` writes): always the user's.
    Forge(String),
    /// dev-flow §6: `sb land` in a PR flow (no landing on main).
    LandInPrFlow,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RootKind {
    Disk,
    Home,
    Bise,
    Repo,
}

impl Hard {
    /// The card's reason line (designer, m_3195).
    pub fn reason(&self) -> String {
        let what = match self {
            Hard::DeletesRoot(RootKind::Repo) => "it deletes the whole repo.".to_string(),
            Hard::DeletesRoot(RootKind::Home) => "it deletes your home folder.".to_string(),
            Hard::DeletesRoot(RootKind::Bise) => "it deletes bise's own data.".to_string(),
            Hard::DeletesRoot(RootKind::Disk) => "it deletes the whole disk.".to_string(),
            Hard::Root => "it runs as root.".to_string(),
            Hard::ForcePush(b) if b == "main" || b == "master" => format!("it rewrites {b}."),
            Hard::ForcePush(b) => format!("it rewrites the history of {b}."),
            Hard::PushMain(b) => format!("it pushes to {b}."),
            Hard::Protected(p) if p == ".git" => {
                "it writes inside .git, which bise protects.".to_string()
            }
            Hard::Protected(p) => format!("it writes to {p}, which bise protects."),
            Hard::Secret(p) => format!("it reads a secret: {p}."),
            Hard::PipeToShell => "it downloads a script and runs it.".to_string(),
            Hard::Forge(what) => format!("it {what} on GitHub."),
            Hard::LandInPrFlow => {
                "this repo ships through pull requests: commit with sb land --here, then open a PR."
                    .to_string()
            }
        };
        format!("{what} this one always asks.")
    }
}

/// The risk classes the checker is kept for once the sandbox is on
/// (design §6.3; used by the sandbox part to open the network).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Talks to a network service by name: `curl`, `gh`, `git push`,
    /// installs, publishes.
    Network,
    /// Can lose work inside the repo: `rm -r`, `git reset`, `checkout`…
    LosesWork,
    /// Stops or starts other processes: `kill`, `launchctl`.
    Processes,
    /// Drives infrastructure: `docker`, `kubectl`, `terraform`…
    Infra,
}

/// An open part: tiers 2–5 decide it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    /// The checker cache key (design §4.4).
    pub key: CacheKey,
    /// The "always" rule the card offers: the arity pattern, or the exact
    /// text for a guarded, unreadable or inline part (design §5.4).
    pub rule: String,
    /// Inline interpreter code that writes files (tier 3, deny once).
    pub inline_write: bool,
    pub risks: Vec<Risk>,
    /// Why it is open (debug log, the corpus report).
    pub why: Why,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Why {
    Unparsed,
    UnreadableProgram,
    UnreadableWrite,
    WriteOutside,
    GuardedRead,
    InlineCode,
    Script,
    UnreadableArgs,
    SecretVar,
    LosesWork,
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Hard(Hard),
    /// Tier 1.
    Allowed,
    Open(Open),
    /// dev-flow §6: the flow makes it the user's call; the card offers
    /// "always" (`rule`), a saved rule runs it.
    Ask(Ask),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The card's reason line.
    pub reason: String,
    /// The "always" rule the card offers.
    pub rule: String,
}

/// What moves between the parts of one command: the base folder (`cd`).
pub struct Walk<'a> {
    pub roots: &'a Roots,
    pub fs: &'a dyn Fs,
    /// `None` after `cd $X`: relative paths are unreadable.
    pub base: Option<PathBuf>,
    /// The earlier parts of the command, for `curl … | sh`.
    pub fetched: bool,
    /// Variables whose value the hub knows (`TMPDIR` → the agent's temp
    /// folder, `HOME`), when the command does not set them.
    pub known: Vec<(String, String)>,
    /// The repo's flow (dev-flow §6's rows); None: unknown.
    pub flow: Option<&'a super::FlowRules>,
}

impl Walk<'_> {
    /// The variables the hub sets in the agent's env, minus the ones this
    /// command sets.
    pub fn known_vars(roots: &Roots, assigned: &[String]) -> Vec<(String, String)> {
        let tmp = roots.tmp.display().to_string();
        let home = roots.home.display().to_string();
        [
            ("TMPDIR", tmp.clone()),
            ("TMP", tmp.clone()),
            ("TEMP", tmp),
            ("HOME", home),
        ]
        .into_iter()
        .filter(|(k, _)| !assigned.iter().any(|a| a == k))
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }

    /// A word's text with the known variables put in, when they are its
    /// only unreadable pieces (`"$TMPDIR/x.patch"` → `<tmp>/x.patch`).
    fn text_of(&self, w: &Word) -> Option<String> {
        if !w.unreadable {
            return Some(w.text.clone());
        }
        let mut t = w.text.clone();
        for (k, v) in &self.known {
            t = t.replace(&format!("${{{k}}}"), v);
            // `$TMPDIR` but not `$TMPDIRX`
            let mut out = String::new();
            let mut rest = t.as_str();
            let pat = format!("${k}");
            while let Some(i) = rest.find(&pat) {
                let after = &rest[i + pat.len()..];
                out.push_str(&rest[..i]);
                if after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                    out.push_str(&pat);
                } else {
                    out.push_str(v);
                }
                rest = after;
            }
            out.push_str(rest);
            t = out;
        }
        let clean = !t.contains(['$', '`', '\\'])
            && !(t.contains('{') && (t.contains(',') || t.contains("..")))
            && !(t.starts_with('~') && !t.starts_with("~/") && t != "~");
        clean.then_some(t)
    }
}

const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "fish", "ksh"];
const INTERPRETERS: &[&str] = &[
    "python",
    "python3",
    "node",
    "bun",
    "deno",
    "perl",
    "ruby",
    "php",
    "osascript",
    "tsx",
    "ts-node",
    "lua",
    "Rscript",
];
const BUILTINS: &[&str] = &[
    "cd", "pushd", "popd", "export", "unset", "set", "shift", "local", "read", "exit", "return",
    ":", "true", "false", "test", "[", "wait", "trap", "declare", "typeset", "alias", "shopt",
    "break", "continue", "mktemp", "sleep", "type", "hash", "ulimit", "umask", "command",
    "builtin", "let", "printf", "echo", "getopts", "jobs", "disown", "times",
];
/// Vibe's read-only programs (design §5.2) and a few more of ours.
const READS: &[&str] = &[
    "cat",
    "head",
    "tail",
    "ls",
    "wc",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "find",
    "fd",
    "stat",
    "file",
    "diff",
    "sort",
    "uniq",
    "cut",
    "tr",
    "jq",
    "yq",
    "pwd",
    "which",
    "whereis",
    "date",
    "basename",
    "dirname",
    "readlink",
    "realpath",
    "du",
    "df",
    "shasum",
    "sha1sum",
    "sha256sum",
    "md5",
    "md5sum",
    "cksum",
    "tree",
    "nl",
    "column",
    "od",
    "hexdump",
    "xxd",
    "strings",
    "fold",
    "comm",
    "paste",
    "expand",
    "unexpand",
    "tac",
    "rev",
    "seq",
    "whoami",
    "uname",
    "hostname",
    "id",
    "groups",
    "ps",
    "pgrep",
    "lsof",
    "cmp",
    "fmt",
    "bat",
    "ag",
    "awk",
    "gawk",
    "sed",
    "yes",
    "locale",
    "sw_vers",
    "sysctl",
    "cal",
    "bc",
    "expr",
    "tty",
    "nproc",
    "uptime",
    "vm_stat",
    "otool",
    "nm",
    "size",
    "lipo",
    "codesign",
    "mdls",
    "look",
    "iconv",
    "base64",
    "numfmt",
    "join",
    "tsort",
    "pr",
    "wait4path",
    "man",
    "info",
    "apropos",
];
/// Git subcommands that only read (with their guards below).
const GIT_READS: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "rev-parse",
    "ls-files",
    "blame",
    "grep",
    "cat-file",
    "merge-base",
    "describe",
    "shortlog",
    "ls-tree",
    "for-each-ref",
    "rev-list",
    "name-rev",
    "show-ref",
    "diff-tree",
    "diff-index",
    "diff-files",
    "check-ignore",
    "check-attr",
    "var",
    "help",
    "version",
    "--version",
    "count-objects",
    "whatchanged",
    "range-diff",
    "cherry",
    "annotate",
    "show-branch",
    "verify-commit",
    "fsck",
    "ls-remote-local",
    "reflog",
];
/// Git subcommands that list with no argument and change refs with one.
const GIT_LISTS: &[&str] = &[
    "branch",
    "tag",
    "stash",
    "worktree",
    "remote",
    "config",
    "notes",
    "symbolic-ref",
];
/// Local git that loses nothing (design §3 tier 1: the private-index
/// commits agents make).
const GIT_LOCAL: &[&str] = &[
    "add",
    "commit",
    "apply",
    "write-tree",
    "read-tree",
    "hash-object",
    "update-index",
    "commit-tree",
    "mktree",
    "mktag",
];
/// Plain writes (design §5.3).
const WRITERS: &[&str] = &[
    "mkdir", "touch", "cp", "mv", "rm", "rmdir", "ln", "truncate", "chmod", "tee", "unlink",
];

pub fn classify(part: &Part, w: &mut Walk) -> Class {
    let mut hard: Vec<Hard> = vec![];
    let mut open: Option<Open> = None;
    let open_exact = |why: Why| Open {
        key: CacheKey::Exact(part.exact()),
        rule: part.exact(),
        inline_write: false,
        risks: risks(part),
        why,
    };

    // redirections, for every kind of part
    for r in &part.redirs {
        match (&r.target, r.written()) {
            (_, Some(t)) => match write_target(t, w) {
                Target_::Hard(h) => hard.push(h),
                Target_::Open(why) => open = open.or(Some(open_exact(why))),
                Target_::Fine => {}
            },
            (Target::Path(t), None) if r.op == "<" || r.op == "<&" => {
                if let Some(h) = secret_read(t, w) {
                    hard.push(h)
                }
            }
            _ => {}
        }
    }

    let verdict = match &part.kind {
        Kind::Unparsed(_) => Class::Open(open_exact(Why::Unparsed)),
        Kind::Compound(_) if part.words.iter().any(|w| w.unreadable) => {
            Class::Open(open_exact(Why::UnreadableArgs))
        }
        Kind::Compound(_) => Class::Allowed,
        Kind::Simple => simple(part, w),
    };
    let mut ask = None;
    match verdict {
        Class::Hard(h) => hard.push(h),
        Class::Open(o) => open = Some(o),
        Class::Ask(a) => ask = Some(a),
        Class::Allowed => {}
    }
    if let Some(h) = hard.into_iter().min() {
        return Class::Hard(h);
    }
    // the flow's ask is the user's anyway: a written file's check waits
    if let Some(a) = ask {
        return Class::Ask(a);
    }
    open.map(Class::Open).unwrap_or(Class::Allowed)
}

enum Target_ {
    Hard(Hard),
    Open(Why),
    Fine,
}

fn write_target(t: &Word, w: &Walk) -> Target_ {
    let Some(text) = w.text_of(t) else {
        return Target_::Open(Why::UnreadableWrite);
    };
    let Some(lex) = w.roots.resolve(w.base.as_deref(), &text, &paths::LexicalFs) else {
        return Target_::Open(Why::UnreadableWrite);
    };
    // a symlink can only turn a writable-looking path into another one:
    // the disk is asked for those only (the rest is protected or outside
    // already)
    let p = if w.roots.writable(&lex) && !lex.starts_with("/dev") {
        w.fs.real(&lex)
    } else {
        lex
    };
    match w.roots.protected(&p) {
        Some(Protected::Git) => Target_::Hard(Hard::Protected(".git".into())),
        Some(Protected::File) => Target_::Hard(Hard::Protected(w.roots.show(&p))),
        None if w.roots.writable(&p) => Target_::Fine,
        None => Target_::Open(Why::WriteOutside),
    }
}

/// Reads are judged on the lexical path: symlinks are resolved for writes
/// only (a syscall per path; a link to a secret is a card when it is made,
/// `ln -s ~/.ssh k` names a secret path).
fn secret_read(t: &Word, w: &Walk) -> Option<Hard> {
    let text = w.text_of(t)?;
    let p = w
        .roots
        .resolve(w.base.as_deref(), &text, &paths::LexicalFs)?;
    paths::secret(&p).then(|| Hard::Secret(w.roots.show(&p)))
}

/// An argument that names a path (not an option, not a pattern).
fn path_like(a: &Word) -> bool {
    let t = a.text.as_str();
    !t.starts_with('-') && !t.is_empty()
}

fn simple(part: &Part, w: &mut Walk) -> Class {
    let Some(first) = part.words.first() else {
        // only assignments (`X=1`) or only redirections
        // their `$(…)` are parts of their own
        return Class::Allowed;
    };
    if first.unreadable {
        return open(part, Why::UnreadableProgram, true);
    }
    let name = part.name().unwrap_or("");
    let args: Vec<&str> = part.args().iter().map(|a| a.text.as_str()).collect();

    // tier 0
    if matches!(name, "sudo" | "doas" | "su" | "pkexec") {
        return Class::Hard(Hard::Root);
    }
    if name == "security"
        && args
            .first()
            .is_some_and(|a| a.starts_with("find-") && a.ends_with("-password"))
    {
        return Class::Hard(Hard::Secret("the keychain".into()));
    }
    if part.piped && w.fetched && reads_code_from_stdin(name, &args) {
        return Class::Hard(Hard::PipeToShell);
    }
    if (SHELLS.contains(&name) || name == "eval" || name == "source" || name == ".")
        && part.subs.iter().any(|s| fetches(s))
    {
        return Class::Hard(Hard::PipeToShell);
    }
    if name == "curl" || name == "wget" {
        w.fetched = true;
    }
    if let Some(c) = w.flow.and_then(|f| flow_row(part, name, &args, f)) {
        return c;
    }
    if name == "git" {
        if let Some(h) = git_push(&args) {
            return Class::Hard(h);
        }
    }
    if name == "rm" && recursive(&args) {
        if let Some(h) = deletes_root(part, w) {
            return Class::Hard(h);
        }
    }
    // a secret read by path, whatever the program (`cat ~/.ssh/id_rsa`,
    // `cp .env x`); a grep pattern is not a path
    let skip = usize::from(matches!(
        name,
        "grep" | "egrep" | "fgrep" | "rg" | "ag" | "sed" | "awk" | "gawk" | "jq"
    ));
    for a in part.args().iter().filter(|a| path_like(a)).skip(skip) {
        if let Some(h) = secret_read(a, w) {
            return Class::Hard(h);
        }
    }

    // tier 1
    if name == "sb" {
        return Class::Allowed;
    }
    if name == "cd" || name == "pushd" {
        w.base = match part.args().iter().find(|a| !a.text.starts_with('-')) {
            None => Some(w.roots.home.clone()),
            Some(a) if a.text == "-" => None,
            Some(a) => w
                .text_of(a)
                .and_then(|t| w.roots.resolve(w.base.as_deref(), &t, &paths::LexicalFs)),
        };
        return Class::Allowed;
    }
    if name == "popd" {
        w.base = None;
        return Class::Allowed;
    }
    if part
        .args()
        .iter()
        .any(|a| a.unreadable && secret_var(&a.text))
    {
        return open(part, Why::SecretVar, true);
    }
    if BUILTINS.contains(&name) {
        return Class::Allowed;
    }
    if name == "git" {
        return git(part, &args, w);
    }
    if READS.contains(&name) {
        return read(part, name, &args, w);
    }
    if WRITERS.contains(&name) {
        return write(part, name, &args, w);
    }
    if INTERPRETERS.contains(&name) || SHELLS.contains(&name) || name.starts_with("python3.") {
        return interpreter(part, name, &args, w);
    }
    if matches!(name, "source" | ".") {
        return script(part);
    }
    if part.unreadable() {
        return open(part, Why::UnreadableArgs, false);
    }
    open(part, Why::Opaque, false)
}

/// A part with no tier 1 answer. `exact`: its key and rule are its text.
fn open(part: &Part, why: Why, exact: bool) -> Class {
    let risks = risks(part);
    let words: Vec<&str> = part.words.iter().map(|w| w.text.as_str()).collect();
    let exact = exact || !pattern_ok(part) || risks.contains(&Risk::Network) || words.is_empty();
    let (key, rule) = if exact {
        (CacheKey::Exact(part.exact()), part.exact())
    } else {
        let p = arity::pattern(&normal_program(&words));
        (CacheKey::Pattern(p.clone()), p)
    };
    Class::Open(Open {
        key,
        rule,
        inline_write: false,
        risks,
        why,
    })
}

/// A pattern (`kill *`) still names what this part runs (Vibe's
/// `_identity_survives`): every unreadable word comes after the words the
/// arity table names, the program is in the table, no guarded option, no
/// unreadable folder or file. `kill $PID` → `kill *`; `git $SUB` → exact.
pub fn pattern_ok(part: &Part) -> bool {
    if part.git_config || part.git_dir.as_ref().is_some_and(|d| d.unreadable) {
        return false;
    }
    if part
        .redirs
        .iter()
        .any(|r| matches!(&r.target, Target::Path(w) if w.unreadable))
    {
        return false;
    }
    let Some(first) = part.words.iter().position(|w| w.unreadable) else {
        return true;
    };
    let words: Vec<&str> = part.words.iter().map(|w| w.text.as_str()).collect();
    let words = normal_program(&words);
    if matches!(
        words[0],
        "env" | "source" | "." | "eval" | "exec" | "command" | "sudo"
    ) {
        return false;
    }
    match arity::arity(&words) {
        Some(a) => first >= a && !words[..a].iter().any(|w| w.starts_with('-')),
        None => false,
    }
}

/// The words with a program from a bin folder named by its name
/// (`/usr/bin/git` → `git`); a script keeps its path (`./tests/gate.sh`).
fn normal_program<'a>(words: &[&'a str]) -> Vec<&'a str> {
    let mut out = words.to_vec();
    if let Some(p) = out.first_mut() {
        let bins = [
            "/bin/",
            "/usr/bin/",
            "/usr/local/bin/",
            "/opt/homebrew/bin/",
            "/sbin/",
            "/usr/sbin/",
        ];
        if let Some(b) = bins
            .iter()
            .find(|b| p.starts_with(**b) && !p[b.len()..].contains('/'))
        {
            *p = &p[b.len()..];
        }
    }
    out
}

fn recursive(args: &[&str]) -> bool {
    args.iter().take_while(|a| **a != "--").any(|a| {
        *a == "--recursive"
            || (a.starts_with('-') && !a.starts_with("--") && (a.contains('r') || a.contains('R')))
    })
}

fn deletes_root(part: &Part, w: &Walk) -> Option<Hard> {
    let r = w.roots;
    part.args().iter().filter(|a| path_like(a)).find_map(|a| {
        let t = w.text_of(a)?;
        let p = r.resolve(
            w.base.as_deref(),
            t.trim_end_matches("/*").trim_end_matches('*'),
            &paths::LexicalFs,
        )?;
        let kind = if p == Path::new("/") {
            RootKind::Disk
        } else if p == r.home || r.home.starts_with(&p) {
            RootKind::Home
        } else if p == r.bise {
            RootKind::Bise
        } else if p == r.cwd || r.cwd.starts_with(&p) || p.file_name().is_some_and(|n| n == ".git")
        {
            RootKind::Repo
        } else {
            return None;
        };
        Some(Hard::DeletesRoot(kind))
    })
}

/// `git push` to main/master, or forced (design §3 tier 0, H4).
fn git_push(args: &[&str]) -> Option<Hard> {
    if args.first() != Some(&"push") {
        return None;
    }
    let rest = &args[1..];
    let forced = rest.iter().any(|a| {
        matches!(
            *a,
            "-f" | "--force" | "--force-with-lease" | "--force-if-includes"
        ) || a.starts_with("--force-with-lease=")
            || (a.starts_with('-') && !a.starts_with("--") && a.contains('f'))
    });
    let refs: Vec<&str> = rest
        .iter()
        .filter(|a| !a.starts_with('-'))
        .skip(1)
        .copied()
        .collect();
    let branch_of = |r: &str| -> String {
        let r = r.trim_start_matches('+');
        let dst = r.rsplit(':').next().unwrap_or(r);
        dst.trim_start_matches("refs/heads/").to_string()
    };
    let plus = refs.iter().find(|r| r.starts_with('+'));
    if forced || plus.is_some() {
        let b = plus
            .or(refs.first())
            .map(|r| branch_of(r))
            .unwrap_or_else(|| "the branch".into());
        return Some(Hard::ForcePush(b));
    }
    refs.iter()
        .map(|r| branch_of(r))
        .find(|b| b == "main" || b == "master")
        .map(Hard::PushMain)
}

/// dev-flow §6, "Approvals (auto mode)": the rows the repo's flow sets.
/// None: no row, the other tiers decide.
///
/// | command | PR flow | trunk flow |
/// | `git push` of its own branch | runs | asks |
/// | `git push` to the default branch | always asks | runs if `push`, else asks |
/// | `gh pr create/view/checks/diff/list/status` | runs | asks |
/// | `gh pr merge/close/comment/review`, `gh api` writes | always ask | always ask |
/// | `sb land` | refused | runs |
fn flow_row(part: &Part, name: &str, args: &[&str], f: &super::FlowRules) -> Option<Class> {
    use crate::flow::FlowMode::{Pr, Trunk};
    let ask = |reason: String, rule: &str| {
        Some(Class::Ask(Ask {
            reason,
            rule: if pattern_ok(part) { rule.to_string() } else { part.exact() },
        }))
    };
    match name {
        "git" if args.first() == Some(&"push") => {
            if git_push(args).is_some_and(|h| matches!(h, Hard::ForcePush(_))) {
                return None; // a forced push stays a hard rule
            }
            let target = push_target(args, f)?;
            if target == f.base {
                return match (f.mode, f.push) {
                    (Pr, _) => Some(Class::Hard(Hard::PushMain(target))),
                    (Trunk, true) => Some(Class::Allowed),
                    (Trunk, false) => ask(
                        format!("it pushes {target}, and this repo keeps its lands local (push = false)."),
                        "git push *",
                    ),
                };
            }
            if f.branch.as_deref() != Some(target.as_str()) {
                return None; // another branch: the checker decides
            }
            match f.mode {
                Pr => Some(Class::Allowed),
                Trunk => ask(
                    format!("it pushes {target}, and this repo lands on {}: no PR here.", f.base),
                    "git push *",
                ),
            }
        }
        "gh" => {
            let (a0, a1) = (args.first().copied().unwrap_or(""), args.get(1).copied().unwrap_or(""));
            if a0 == "api" {
                return gh_api_writes(args).then(|| Class::Hard(Hard::Forge("writes through the API".into())));
            }
            if a0 != "pr" {
                return None;
            }
            match a1 {
                "merge" => Some(Class::Hard(Hard::Forge("merges a PR".into()))),
                "close" | "reopen" | "lock" | "unlock" => Some(Class::Hard(Hard::Forge(format!("{a1}s a PR")))),
                "comment" => Some(Class::Hard(Hard::Forge("comments on a PR".into()))),
                "review" => Some(Class::Hard(Hard::Forge(if args.iter().any(|a| *a == "--approve" || *a == "-a") {
                    "approves a PR".into()
                } else {
                    "reviews a PR".into()
                }))),
                "create" | "view" | "checks" | "diff" | "list" | "status" => match f.mode {
                    Pr => Some(Class::Allowed),
                    Trunk => ask(
                        format!("it uses a PR, and this repo lands on {}: no PR here.", f.base),
                        &format!("gh pr {a1} *"),
                    ),
                },
                _ => None,
            }
        }
        "sb" if args.first() == Some(&"land") && !args.contains(&"--here") && f.mode == Pr => {
            Some(Class::Hard(Hard::LandInPrFlow))
        }
        _ => None,
    }
}

/// The branch a non-forced `git push` updates: its refspec's
/// destination, else the agent's own branch (the default branch in the
/// shared folder). None: unreadable (`HEAD` in the shared folder is the
/// default branch too).
fn push_target(args: &[&str], f: &super::FlowRules) -> Option<String> {
    let refs: Vec<&str> = args[1..]
        .iter()
        .filter(|a| !a.starts_with('-'))
        .skip(1)
        .copied()
        .collect();
    let here = || f.branch.clone().unwrap_or_else(|| f.base.clone());
    match refs.as_slice() {
        [] => Some(here()),
        [r] => {
            let dst = r.rsplit(':').next().unwrap_or(r).trim_start_matches("refs/heads/");
            Some(if dst == "HEAD" { here() } else { dst.to_string() })
        }
        _ => None,
    }
}

/// `gh api` that writes: `-X`/`--method` other than GET, or fields
/// (`-f`, `-F`, `--field`, `--raw-field`, `--input`: a POST by default).
fn gh_api_writes(args: &[&str]) -> bool {
    let mut it = args.iter().peekable();
    let mut method: Option<String> = None;
    let mut fields = false;
    while let Some(a) = it.next() {
        match *a {
            "-X" | "--method" => method = it.next().map(|m| m.to_ascii_uppercase()),
            "-f" | "-F" | "--field" | "--raw-field" | "--input" => fields = true,
            a if a.starts_with("--method=") => method = Some(a["--method=".len()..].to_ascii_uppercase()),
            a if a.starts_with("-X") && a.len() > 2 => method = Some(a[2..].to_ascii_uppercase()),
            a if a.starts_with("--field=") || a.starts_with("--raw-field=") || a.starts_with("--input=") => fields = true,
            _ => {}
        }
    }
    match method {
        Some(m) => m != "GET" && m != "HEAD",
        None => fields,
    }
}

fn fetches(src: &str) -> bool {
    src.split(|c: char| c.is_whitespace() || c == '|' || c == ';' || c == '(')
        .any(|w| {
            let n = w.rsplit('/').next().unwrap_or(w);
            n == "curl" || n == "wget"
        })
}

/// A shell or interpreter reading its program from a pipe.
fn reads_code_from_stdin(name: &str, args: &[&str]) -> bool {
    if !(SHELLS.contains(&name) || INTERPRETERS.contains(&name) || name.starts_with("python3.")) {
        return false;
    }
    let files = args
        .iter()
        .filter(|a| !a.starts_with('-') || **a == "-")
        .collect::<Vec<_>>();
    let code_flag = args
        .iter()
        .any(|a| matches!(*a, "-c" | "-e" | "-E" | "--eval" | "-p" | "-m"));
    (files.is_empty() || files[0] == &"-") && !code_flag || args.contains(&"-s")
}

/// A variable that looks like it holds a key (`$OPENAI_API_KEY`).
fn secret_var(text: &str) -> bool {
    let up = text.to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL"]
        .iter()
        .any(|k| up.contains(k))
}

/// A short option cluster holds `opt` (`-no` holds `o`), stopping at an
/// option that takes a value (Vibe's `_contains_short_option`).
fn short_has(a: &str, opt: char, takes_value: &str) -> bool {
    if !a.starts_with('-') || a.starts_with("--") || a.len() < 2 {
        return false;
    }
    for c in a[1..].chars() {
        if c == opt {
            return true;
        }
        if takes_value.contains(c) {
            return false;
        }
    }
    false
}

fn long_has(a: &str, opt: &str) -> bool {
    let a = a.split('=').next().unwrap_or(a);
    a == opt
}

/// Vibe's option guardrails on the read programs (design §5.2): a hit
/// makes the read open, keyed by its exact text.
fn guarded(name: &str, args: &[&str]) -> bool {
    let opts: Vec<&str> = args.iter().take_while(|a| **a != "--").copied().collect();
    let any = |f: &dyn Fn(&str) -> bool| opts.iter().any(|a| f(a));
    match name {
        "find" => any(&|a| {
            matches!(
                a,
                "-exec"
                    | "-execdir"
                    | "-ok"
                    | "-okdir"
                    | "-delete"
                    | "-fls"
                    | "-fprint"
                    | "-fprint0"
                    | "-fprintf"
                    | "-files0-from"
            )
        }),
        "sort" => any(&|a| {
            [
                "--output",
                "--compress-program",
                "--temporary-directory",
                "--files0-from",
            ]
            .iter()
            .any(|o| long_has(a, o))
                || short_has(a, 'o', "kSt")
                || short_has(a, 'T', "kSt")
        }),
        "rg" => {
            any(&|a| long_has(a, "--pre") || long_has(a, "--search-zip") || short_has(a, 'z', ""))
        }
        "tree" => any(&|a| {
            long_has(a, "--output")
                || (a.starts_with('-') && !a.starts_with("--") && a.contains('o'))
        }),
        "file" => any(&|a| {
            ["--compile", "--files-from", "--uncompress"]
                .iter()
                .any(|o| long_has(a, o))
                || ['C', 'f', 'z', 'Z']
                    .iter()
                    .any(|c| short_has(a, *c, "eFmP"))
        }),
        "date" => any(&|a| long_has(a, "--set") || short_has(a, 's', "dfIr")),
        "shasum" | "sha1sum" | "sha256sum" | "md5sum" => {
            any(&|a| long_has(a, "--check") || short_has(a, 'c', "a"))
        }
        "du" | "wc" => any(&|a| long_has(a, "--files0-from")),
        "sed" => {
            any(&|a| {
                a == "-i"
                    || a.starts_with("-i")
                    || a.starts_with("--in-place")
                    || short_has(a, 'i', "ef")
            }) || args
                .iter()
                .filter(|a| !a.starts_with('-'))
                .take(1)
                .any(|s| sed_writes(s))
                || args.windows(2).any(|w| w[0] == "-e" && sed_writes(w[1]))
        }
        "awk" | "gawk" => args.iter().any(|a| awk_writes(a)),
        // two operands: the second is written
        "xxd" | "uniq" => args.iter().filter(|a| !a.starts_with('-')).count() >= 2,
        "base64" => any(&|a| long_has(a, "--output") || short_has(a, 'o', "")),
        "codesign" => !any(&|a| {
            matches!(
                a,
                "-d" | "--display" | "-v" | "--verify" | "-dv" | "-dvv" | "-dvvv"
            )
        }),
        "sysctl" => args.iter().any(|a| a.contains('=') || *a == "-w"),
        "hostname" => args.iter().any(|a| !a.starts_with('-')),
        _ => false,
    }
}

/// sed's `w file` / `e cmd` commands and `s///w file` flag.
fn sed_writes(script: &str) -> bool {
    let bytes: Vec<char> = script.chars().collect();
    bytes.iter().enumerate().any(|(i, c)| {
        (*c == 'w' || *c == 'W' || *c == 'e')
            && bytes
                .get(i + 1)
                .is_none_or(|n| n.is_whitespace() || *n == ';')
            && (i == 0 || {
                let p = bytes[i - 1];
                p == ';'
                    || p == '\n'
                    || p == '}'
                    || p == '/'
                    || p.is_ascii_digit()
                    || p == '$'
                    || p.is_whitespace()
                    || "gpiImM".contains(p)
            })
    })
}

fn awk_writes(a: &str) -> bool {
    a.contains("system(") || a.contains("| \"") || a.contains("|\"") || a.contains("|&") || {
        // print … > "file" / >> file
        let mut s = a;
        let mut hit = false;
        while let Some(i) = s.find("print") {
            let rest = &s[i..];
            let stmt = rest.split([';', '}', '\n']).next().unwrap_or("");
            if stmt.contains('>')
                && !stmt.contains("> 0")
                && !stmt.contains(">0")
                && !stmt.contains(">=")
            {
                // `print $1 > 3` compares only inside parens; a bare `>` redirects
                let after = stmt.split('>').nth(1).unwrap_or("").trim_start();
                if after.starts_with('"')
                    || after.starts_with('/')
                    || after.starts_with('>')
                    || after.chars().next().is_some_and(|c| c.is_alphabetic())
                {
                    hit = true;
                }
            }
            s = &rest[5..];
        }
        hit
    }
}

fn read(part: &Part, name: &str, args: &[&str], _w: &Walk) -> Class {
    // `sed -i` is a write
    if (name == "sed")
        && args
            .iter()
            .any(|a| a.starts_with("-i") || a.starts_with("--in-place") || short_has(a, 'i', "ef"))
    {
        return write(part, name, args, _w);
    }
    if guarded(name, args) {
        return open(part, Why::GuardedRead, true);
    }
    Class::Allowed
}

fn git(part: &Part, args: &[&str], w: &Walk) -> Class {
    let Some(sub) = args.first().copied() else {
        return Class::Allowed;
    };
    let rest = &args[1..];
    if part.git_config {
        return open(part, Why::GuardedRead, true);
    }
    let git_guard = rest.iter().any(|a| {
        [
            "--output",
            "--ext-diff",
            "--textconv",
            "--exec",
            "--upload-pack",
            "--receive-pack",
            "--open-files-in-pager",
        ]
        .iter()
        .any(|o| long_has(a, o))
    });
    // where git writes: its -C folder inside the roots
    let dir_inside = match &part.git_dir {
        None => w
            .base
            .as_deref()
            .is_some_and(|b| w.roots.writable(b) || w.roots.inside(b)),
        Some(d) => w
            .text_of(d)
            .and_then(|t| w.roots.resolve(w.base.as_deref(), &t, w.fs))
            .is_some_and(|p| w.roots.inside(&p)),
    };
    if GIT_READS.contains(&sub) {
        if git_guard {
            return open(part, Why::GuardedRead, true);
        }
        if sub == "reflog"
            && rest
                .first()
                .is_some_and(|a| matches!(*a, "expire" | "delete"))
        {
            return open(part, Why::LosesWork, false);
        }
        if sub == "fsck" && rest.contains(&"--lost-found") {
            return open(part, Why::Opaque, false);
        }
        return Class::Allowed;
    }
    if GIT_LISTS.contains(&sub) && git_lists_only(sub, rest) {
        return Class::Allowed;
    }
    if GIT_LOCAL.contains(&sub) && dir_inside && !git_guard {
        return Class::Allowed;
    }
    if part.unreadable() {
        return open(part, Why::UnreadableArgs, false);
    }
    let why = if risks(part).contains(&Risk::LosesWork) {
        Why::LosesWork
    } else {
        Why::Opaque
    };
    open(part, why, false)
}

/// `git branch` / `tag` / `stash list` … that only list.
fn git_lists_only(sub: &str, rest: &[&str]) -> bool {
    let first = rest.first().copied();
    match sub {
        "branch" => {
            rest.iter().all(|a| {
                matches!(
                    *a,
                    "-a" | "-r"
                        | "-v"
                        | "-vv"
                        | "-av"
                        | "-avv"
                        | "--all"
                        | "--remotes"
                        | "--list"
                        | "-l"
                        | "--show-current"
                        | "--merged"
                        | "--no-merged"
                        | "--contains"
                        | "--no-contains"
                        | "--verbose"
                        | "--no-color"
                        | "--color"
                        | "--sort"
                        | "--format"
                ) || a.starts_with("--sort=")
                    || a.starts_with("--format=")
                    || a.starts_with("--points-at")
            }) || (rest.iter().any(|a| {
                matches!(
                    *a,
                    "--list" | "-l" | "--contains" | "--merged" | "--no-merged" | "--points-at"
                )
            }) && !rest.iter().any(|a| {
                matches!(
                    *a,
                    "-d" | "-D"
                        | "-m"
                        | "-M"
                        | "-c"
                        | "-C"
                        | "-f"
                        | "--delete"
                        | "--force"
                        | "--move"
                        | "--copy"
                )
            }))
        }
        "tag" => {
            first.is_none()
                || rest.iter().any(|a| {
                    matches!(
                        *a,
                        "-l" | "--list" | "--contains" | "--points-at" | "-n" | "--merged"
                    )
                }) && !rest
                    .iter()
                    .any(|a| matches!(*a, "-d" | "-a" | "-s" | "-f" | "--delete" | "-m"))
        }
        "stash" => matches!(first, Some("list") | Some("show")),
        "worktree" => matches!(first, Some("list")),
        "remote" => {
            first.is_none()
                || matches!(
                    first,
                    Some("-v") | Some("--verbose") | Some("get-url") | Some("show")
                )
        }
        "config" => {
            rest.iter().any(|a| {
                matches!(
                    *a,
                    "--get"
                        | "--get-all"
                        | "--get-regexp"
                        | "--list"
                        | "-l"
                        | "--show-origin"
                        | "get"
                        | "list"
                )
            }) || (rest.iter().filter(|a| !a.starts_with('-')).count() == 1
                && !rest.iter().any(|a| {
                    matches!(
                        *a,
                        "--unset"
                            | "--unset-all"
                            | "--add"
                            | "--replace-all"
                            | "--edit"
                            | "-e"
                            | "--remove-section"
                            | "--rename-section"
                    )
                }))
        }
        "notes" => matches!(first, None | Some("list") | Some("show")),
        "symbolic-ref" => {
            rest.iter().filter(|a| !a.starts_with('-')).count() <= 1
                && !rest.contains(&"-d")
                && !rest.contains(&"--delete")
        }
        _ => false,
    }
}

/// The paths a plain write writes (design §5.3).
fn targets<'a>(name: &str, args: &'a [Word]) -> Vec<&'a Word> {
    let mut ops: Vec<&Word> = vec![];
    let mut i = 0;
    let mut t_dir: Option<&Word> = None;
    let mut ended = false;
    while let Some(a) = args.get(i) {
        let t = a.text.as_str();
        if !ended && t == "--" {
            ended = true;
        } else if !ended && t.starts_with('-') && t.len() > 1 {
            if (name == "cp" || name == "mv" || name == "ln")
                && (t == "-t" || t == "--target-directory")
            {
                t_dir = args.get(i + 1);
                i += 1;
            } else if name == "truncate"
                && (t == "-s" || t == "-r" || t == "--size" || t == "--reference")
            {
                i += 1;
            } else if name == "chmod"
                && t.len() > 1
                && t[1..].chars().all(|c| "rwxXstugoa".contains(c))
                && ops.is_empty()
            {
                // `chmod -x f` is a mode
                ops.push(a);
            } else if name == "mkdir" && (t == "-m" || t == "--mode") {
                i += 1;
            }
        } else {
            ops.push(a);
        }
        i += 1;
    }
    match name {
        "cp" | "ln" => match t_dir {
            Some(d) => vec![d],
            None if ops.len() == 1 && name == "ln" => {
                // `ln -s /a/b` makes ./b
                vec![]
            }
            None => ops.last().copied().into_iter().collect(),
        },
        "mv" => match t_dir {
            Some(d) => ops.into_iter().chain(std::iter::once(d)).collect(),
            None => ops,
        },
        "chmod" => ops.into_iter().skip(1).collect(),
        _ => ops,
    }
}

fn write(part: &Part, name: &str, _args: &[&str], w: &Walk) -> Class {
    if name == "chmod"
        && part.args().iter().any(|a| {
            a.text.contains('s') && a.text.starts_with(['+', 'u', 'g']) && !a.text.contains('/')
        })
    {
        return open(part, Why::Opaque, true);
    }
    if part.stdin_args {
        return open(part, Why::UnreadableWrite, true);
    }
    let words: Vec<Word> = if name == "sed" {
        // sed: the script is the first operand unless -e/-f gave it
        let given = part
            .args()
            .iter()
            .any(|a| a.text == "-e" || a.text == "-f" || a.text.starts_with("--expression"));
        let mut ops: Vec<Word> = vec![];
        let mut skip_next = false;
        for a in part.args() {
            if skip_next {
                skip_next = false;
                continue;
            }
            if a.text == "-e" || a.text == "-f" {
                skip_next = true;
                continue;
            }
            if a.text.starts_with('-') {
                continue;
            }
            ops.push(a.clone());
        }
        if given {
            ops
        } else {
            ops.into_iter().skip(1).collect()
        }
    } else {
        targets(name, part.args()).into_iter().cloned().collect()
    };
    let rec = name == "rm"
        && recursive(
            &part
                .args()
                .iter()
                .map(|a| a.text.as_str())
                .collect::<Vec<_>>(),
        );
    let mut outside = false;
    for t in &words {
        match write_target(t, w) {
            Target_::Hard(h) => return Class::Hard(h),
            Target_::Open(Why::UnreadableWrite) => return open(part, Why::UnreadableWrite, true),
            Target_::Open(_) => outside = true,
            Target_::Fine => {}
        }
    }
    // `ln -s /etc x`: the link is inside, what it opens is not
    if name == "ln"
        && part.args().iter().any(|a| {
            a.text.starts_with('-') && !a.text.starts_with("--") && a.text.contains('s')
                || a.text == "--symbolic"
        })
    {
        let src = part.args().iter().find(|a| !a.text.starts_with('-'));
        let base = w.base.as_deref().map(Path::to_path_buf);
        let points_out = src.is_some_and(|s| match w.text_of(s) {
            None => true,
            Some(t) => {
                // a relative link target is read from the link's folder
                let from = words
                    .last()
                    .and_then(|l| w.text_of(l))
                    .and_then(|l| w.roots.resolve(base.as_deref(), &l, &paths::LexicalFs))
                    .and_then(|l| l.parent().map(Path::to_path_buf))
                    .or(base.clone());
                w.roots
                    .resolve(from.as_deref(), &t, &paths::LexicalFs)
                    .is_none_or(|p| !w.roots.writable(&p))
            }
        });
        if points_out {
            return open(part, Why::WriteOutside, true);
        }
    }
    if outside {
        return open(part, Why::WriteOutside, true);
    }
    if rec || (name == "sed" && guarded("sed", &[])) {
        // a recursive delete can lose work: the checker looks (design §6.3)
        return open(part, Why::LosesWork, true);
    }
    // sed -i with a `w`/`e` command in its script
    if name == "sed"
        && part
            .args()
            .iter()
            .any(|a| !a.text.starts_with('-') && sed_writes(&a.text))
    {
        return open(part, Why::GuardedRead, true);
    }
    Class::Allowed
}

/// Words in inline code that write or delete files.
fn code_writes(code: &str) -> bool {
    const MARKS: &[&str] = &[
        ".write(",
        "write_text(",
        "write_bytes(",
        "writeFile",
        "appendFile",
        "fs.rm",
        "unlink",
        "rmtree",
        "os.remove",
        "shutil.",
        "rename(",
        "replace(",
        ".mkdir(",
        "makedirs",
        "open(",
        "File.write",
        "File.open",
        "IO.write",
        "print >",
        "copyfile",
        "truncate(",
        "rmdir",
        "fopen(",
        "file_put_contents",
        "Path(",
        "> ",
        ">>",
    ];
    // `open(` for reading is common: count it only with a write mode
    let opens_for_write = code.contains("open(")
        && [
            "'w'", "\"w\"", "'a'", "\"a\"", "'wb'", "\"wb\"", "'r+'", "\"r+\"", "'x'", "mode=",
        ]
        .iter()
        .any(|m| code.contains(m));
    MARKS
        .iter()
        .filter(|m| !matches!(**m, "open(" | "Path(" | "> " | ">>" | "replace("))
        .any(|m| code.contains(m))
        || opens_for_write
        || (code.contains("Path(")
            && (code.contains(".write_") || code.contains(".unlink") || code.contains(".touch")))
}

fn interpreter(part: &Part, name: &str, args: &[&str], _w: &Walk) -> Class {
    let flag_code = |f: &[&str]| {
        args.windows(2)
            .find(|w| f.contains(&w[0]))
            .map(|w| w[1].to_string())
    };
    let code = match name {
        "node" | "bun" | "deno" | "tsx" => flag_code(&["-e", "--eval", "-p", "--print"]),
        "perl" | "ruby" => {
            // `perl -pi -e`: an in-place edit (design §3 tier 3)
            if args
                .iter()
                .any(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('i'))
            {
                return deny_once(part);
            }
            args.iter()
                .position(|a| a.starts_with('-') && a.contains('e'))
                .and_then(|i| args.get(i + 1))
                .map(|s| s.to_string())
        }
        "osascript" => flag_code(&["-e"]),
        _ => flag_code(&["-c"]).or_else(|| {
            args.iter()
                .find(|a| a.starts_with("-c") && a.len() > 2 && !a.starts_with("--"))
                .map(|a| a[2..].to_string())
        }),
    };
    let heredoc = part
        .heredocs
        .first()
        .filter(|_| args.iter().all(|a| a.starts_with('-') || *a == "-"));
    let inline = code.clone().or_else(|| heredoc.cloned());
    match inline {
        Some(c) if code_writes(&c) => deny_once(part),
        Some(_) => open(part, Why::InlineCode, true),
        None if args.iter().any(|a| matches!(*a, "-m")) => open(part, Why::Opaque, false),
        None if args.iter().any(|a| !a.starts_with('-')) => script(part),
        None => open(part, Why::InlineCode, true),
    }
}

/// A script file run by an interpreter (`python3 tools/y.py a`, `bash
/// x.sh`, `. env.sh`): keyed by the interpreter and the file, `python3
/// tools/y.py *` (design §4.4; the checker part adds the file's content
/// hash to the key, `Open::script`). Unreadable: its exact text.
fn script(part: &Part) -> Class {
    let file = part.args().iter().position(|a| !a.text.starts_with('-'));
    match file {
        Some(i) if !part.unreadable() => {
            let head: Vec<&str> = part.words[..i + 2]
                .iter()
                .map(|w| w.text.as_str())
                .collect();
            let p = format!("{} *", normal_program(&head).join(" "));
            Class::Open(Open {
                key: CacheKey::Pattern(p.clone()),
                rule: p,
                inline_write: false,
                risks: risks(part),
                why: Why::Script,
            })
        }
        _ => open(part, Why::Script, true),
    }
}

/// The script file a part runs, if any (`tools/y.py` of `python3
/// tools/y.py a`), for the checker's state and its cache key.
pub fn script_file(part: &Part) -> Option<&str> {
    let name = part.name()?;
    let runs_file = INTERPRETERS.contains(&name)
        || SHELLS.contains(&name)
        || matches!(name, "source" | ".")
        || name.starts_with("python3.");
    if !runs_file {
        return None;
    }
    let args = part.args();
    let code_flag = args
        .iter()
        .any(|a| matches!(a.text.as_str(), "-c" | "-e" | "--eval" | "-p" | "-m"));
    if code_flag {
        return None;
    }
    args.iter()
        .find(|a| !a.text.starts_with('-'))
        .filter(|a| !a.unreadable)
        .map(|a| a.text.as_str())
}

fn deny_once(part: &Part) -> Class {
    match open(part, Why::InlineCode, true) {
        Class::Open(o) => Class::Open(Open {
            inline_write: true,
            ..o
        }),
        c => c,
    }
}

/// The risk classes of a part (design §6.3).
pub fn risks(part: &Part) -> Vec<Risk> {
    let Some(name) = part.name() else {
        return vec![];
    };
    let args: Vec<&str> = part.args().iter().map(|a| a.text.as_str()).collect();
    let a0 = args.first().copied().unwrap_or("");
    let a1 = args.get(1).copied().unwrap_or("");
    let mut out = vec![];
    let network = match name {
        "curl" | "wget" | "ssh" | "scp" | "sftp" | "rsync" | "nc" | "ncat" | "telnet" | "ftp"
        | "http" | "https" | "xh" | "gh" | "twine" | "gsutil" | "rclone" | "ngrok" => true,
        "git" => {
            matches!(a0, "push" | "fetch" | "pull" | "clone" | "ls-remote" | "submodule" | "remote" if a0 != "remote" || matches!(a1, "update" | "prune" | "show"))
        }
        "npm" | "pnpm" | "yarn" | "bun" => matches!(
            a0,
            "install"
                | "i"
                | "ci"
                | "add"
                | "publish"
                | "update"
                | "upgrade"
                | "dlx"
                | "x"
                | "create"
                | "login"
        ),
        "npx" | "pnpx" | "bunx" | "uvx" | "pipx" => true,
        "pip" | "pip3" => matches!(a0, "install" | "download" | "upload"),
        "uv" => matches!(a0, "add" | "sync" | "pip" | "tool" | "publish" | "lock"),
        "cargo" => matches!(
            a0,
            "install"
                | "add"
                | "update"
                | "fetch"
                | "publish"
                | "search"
                | "login"
                | "yank"
                | "owner"
        ),
        "brew" => matches!(a0, "install" | "upgrade" | "update" | "tap" | "reinstall"),
        "go" => matches!(a0, "get" | "install" | "mod"),
        "gem" => matches!(a0, "install" | "push" | "update"),
        "docker" | "podman" => {
            matches!(a0, "push" | "pull" | "login")
                || (a0 == "image" && matches!(a1, "push" | "pull"))
        }
        "python" | "python3" => a0 == "-m" && matches!(a1, "pip" | "http.server" | "twine"),
        _ => false,
    };
    if network {
        out.push(Risk::Network);
    }
    let loses = match name {
        "rm" => recursive(&args),
        "git" => match a0 {
            "reset" | "checkout" | "clean" | "restore" | "switch" | "rebase" | "merge"
            | "cherry-pick" | "revert" | "am" => true,
            "stash" => matches!(
                a1,
                "" | "drop" | "clear" | "pop" | "push" | "save" | "apply"
            ),
            "branch" => args
                .iter()
                .any(|a| matches!(*a, "-D" | "-d" | "--delete" | "-f" | "--force" | "-M")),
            "worktree" => matches!(a1, "remove" | "prune"),
            "tag" => args.contains(&"-d"),
            "update-ref" => args.contains(&"-d"),
            _ => false,
        },
        "find" => args.contains(&"-delete"),
        _ => false,
    };
    if loses {
        out.push(Risk::LosesWork);
    }
    if matches!(
        name,
        "kill"
            | "pkill"
            | "killall"
            | "launchctl"
            | "systemctl"
            | "shutdown"
            | "reboot"
            | "crontab"
            | "at"
    ) {
        out.push(Risk::Processes);
    }
    if matches!(
        name,
        "docker"
            | "podman"
            | "kubectl"
            | "helm"
            | "terraform"
            | "pulumi"
            | "aws"
            | "gcloud"
            | "az"
            | "flyctl"
            | "vercel"
            | "heroku"
            | "doctl"
            | "eksctl"
            | "kind"
            | "minikube"
    ) {
        out.push(Risk::Infra);
    }
    out
}
