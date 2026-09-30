//! The bash parser for approvals (design §5): `brush-parser` 0.4.0,
//! pinned and wrapped here so that nothing else sees its types.
//!
//! `parse(cmd)` gives the simple commands ("parts") of a command, found
//! through lists, pipes, subshells, groups, `if`/`for`/`while`/`case`,
//! `$(…)` / backtick / `<(…)` bodies, `bash -c` and static `eval` strings.
//! Wrappers (`env`, `time`, `nohup`, `timeout`, `nice`, `command`, `exec`,
//! `xargs`, `caffeinate`, `stdbuf`) are looked through; `git -C <dir>` is
//! read as `git` with its folder kept. Pure: no file, no environment.

use brush_parser::ast;
use brush_parser::word::{self, WordPiece, WordPieceWithSource};
use brush_parser::{Parser, ParserOptions};

/// How deep `$(…)`, `bash -c` and `eval` strings are parsed again. A
/// deeper string is an unreadable part.
const MAX_DEPTH: usize = 4;

/// One word after quote removal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    /// The static pieces unquoted, the unreadable ones as written
    /// (`$S/lib.rs`, `$(date)`).
    pub text: String,
    /// A piece the parser cannot know before the shell runs: a variable,
    /// `$(…)`, backticks, `$((…))`, `~user`, brace expansion, `<(…)`.
    pub unreadable: bool,
}

impl Word {
    fn plain(text: &str) -> Word {
        Word {
            text: text.to_string(),
            unreadable: false,
        }
    }
}

/// Where a redirection points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A file (`> f`, `&> f`, `< f`, `>&f` with a name).
    Path(Word),
    /// A descriptor (`2>&1`, `>&-`): no file.
    Fd,
    /// A here-document: its body is in `Part::heredocs`.
    HereDoc,
    /// A here-string (`<<< word`).
    HereString,
    /// A process substitution as a target (`> >(cmd)`): its body is parsed
    /// into its own parts.
    ProcSub,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redir {
    /// `<`, `>`, `>>`, `<>`, `>|`, `<&`, `>&`, `&>`, `&>>`, `<<`, `<<<`.
    pub op: String,
    pub target: Target,
}

impl Redir {
    /// The file this redirection writes, if any: `2>&1` and `>/dev/null`
    /// write none (Vibe's `_file_redirect_reason`).
    pub fn written(&self) -> Option<&Word> {
        let writes = matches!(
            self.op.as_str(),
            ">" | ">>" | "<>" | ">|" | "&>" | "&>>" | ">&"
        );
        match &self.target {
            Target::Path(w) if writes && w.text != "/dev/null" => Some(w),
            _ => None,
        }
    }
}

/// What a part is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A simple command: `words[0]` is the program (after the wrappers).
    Simple,
    /// Syntax with no program of its own that still matters: a compound
    /// command's redirections, `[[ … ]]`, `(( … ))`, a function definition,
    /// `coproc`. Its `words` are empty or a marker.
    Compound(&'static str),
    /// Text the parser could not read (a syntax error, or nested too deep).
    Unparsed(String),
}

/// One simple command of a bash command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub kind: Kind,
    /// The command as written (the parser's rendering of it).
    pub source: String,
    /// Program then arguments, after the wrappers.
    pub words: Vec<Word>,
    /// `NAME=value` before the program, and those given to `env`.
    pub assigns: Vec<String>,
    /// The wrappers looked through, in order (`timeout`, `env`, `xargs`…).
    pub wrappers: Vec<String>,
    pub redirs: Vec<Redir>,
    /// Here-document bodies, as text.
    pub heredocs: Vec<String>,
    /// It reads a pipe (not the first command of a pipeline).
    pub piped: bool,
    /// Started with `&`.
    pub background: bool,
    /// `xargs` adds arguments read from stdin: they are unreadable.
    pub stdin_args: bool,
    /// `git -C <dir>` (or `--work-tree`): the folder git runs in.
    pub git_dir: Option<Word>,
    /// `git -c k=v` (or `--exec-path`, `--config-env`): git configured on
    /// the command line, a guarded option.
    pub git_config: bool,
    /// The bodies of the `$(…)`, backticks and `<(…)` of this part (each is
    /// also parsed into parts of its own, after this one).
    pub subs: Vec<String>,
}

impl Part {
    fn new(kind: Kind, source: String) -> Part {
        Part {
            kind,
            source,
            words: vec![],
            assigns: vec![],
            wrappers: vec![],
            redirs: vec![],
            heredocs: vec![],
            piped: false,
            background: false,
            stdin_args: false,
            git_dir: None,
            git_config: false,
            subs: vec![],
        }
    }

    /// The program as written (`git`, `./tests/gate.sh`), if any.
    pub fn program(&self) -> Option<&str> {
        match self.kind {
            Kind::Simple => self.words.first().map(|w| w.text.as_str()),
            _ => None,
        }
    }

    /// The program's name: the last path segment (`/usr/bin/git` → `git`).
    pub fn name(&self) -> Option<&str> {
        self.program().map(|p| p.rsplit('/').next().unwrap_or(p))
    }

    pub fn args(&self) -> &[Word] {
        self.words.get(1..).unwrap_or(&[])
    }

    /// Any unreadable piece in the program, its arguments or a written
    /// target (Vibe's dynamic nodes).
    pub fn unreadable(&self) -> bool {
        self.stdin_args
            || self.words.iter().any(|w| w.unreadable)
            || self
                .redirs
                .iter()
                .any(|r| matches!(&r.target, Target::Path(w) if w.unreadable))
            || self.git_dir.as_ref().is_some_and(|w| w.unreadable)
    }

    /// The part as rules and the card read it: the program and its
    /// arguments, wrappers and assignments left out (design §5.4:
    /// `GIT_INDEX_FILE=x git commit` reads `git commit`).
    pub fn text(&self) -> String {
        match &self.kind {
            Kind::Simple => self
                .words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => self.source.clone(),
        }
    }

    /// The exact text of the part: what it runs, here-document bodies
    /// included (the key of an inline script, and its "always" rule).
    pub fn exact(&self) -> String {
        let mut s = self.source.clone();
        for body in &self.heredocs {
            s.push('\n');
            s.push_str(body);
        }
        s
    }
}

/// The parts of a command, in the order they run (a `$(…)` body after
/// the part that holds it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub parts: Vec<Part>,
    /// The first syntax error, if any (its text is also an `Unparsed` part).
    pub error: Option<String>,
    /// The variables the command sets (`X=1`, `export X`, `read X`, `for X
    /// in`, `unset X`…): their values are not the hub's.
    pub assigned: Vec<String>,
}

/// Parse a bash command into its parts. Never fails: what cannot be read
/// is an `Unparsed` part.
pub fn parse(cmd: &str) -> Parsed {
    let mut w = Walker {
        parts: vec![],
        error: None,
        depth: 0,
        assigned: vec![],
    };
    w.program(cmd, Ctx::default());
    let mut assigned = w.assigned;
    for p in &w.parts {
        assigned.extend(
            p.assigns
                .iter()
                .map(|a| a.split('=').next().unwrap_or("").to_string()),
        );
        if matches!(
            p.name(),
            Some(
                "export"
                    | "declare"
                    | "local"
                    | "typeset"
                    | "readonly"
                    | "read"
                    | "unset"
                    | "getopts"
                    | "mapfile"
                    | "readarray"
                    | "printf"
            )
        ) {
            assigned.extend(
                p.args()
                    .iter()
                    .filter(|a| !a.text.starts_with('-'))
                    .map(|a| a.text.split('=').next().unwrap_or("").to_string()),
            );
        }
    }
    Parsed {
        parts: w.parts,
        error: w.error,
        assigned,
    }
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    piped: bool,
    background: bool,
}

struct Walker {
    parts: Vec<Part>,
    error: Option<String>,
    depth: usize,
    assigned: Vec<String>,
}

fn options() -> ParserOptions {
    ParserOptions::default()
}

impl Walker {
    fn program(&mut self, src: &str, ctx: Ctx) {
        // Bash drops backslash-newline before it reads the words; so do we,
        // or `rm \<newline>-rf x` would read as two words
        let src = src.replace("\\\n", "");
        let mut p = Parser::new(std::io::Cursor::new(src.as_str()), &options());
        match p.parse_program() {
            Ok(prog) => prog
                .complete_commands
                .iter()
                .for_each(|cc| self.list(cc, ctx)),
            Err(e) => self.unparsed(&src, e.to_string()),
        }
    }

    fn unparsed(&mut self, src: &str, err: String) {
        if self.error.is_none() {
            self.error = Some(err.clone());
        }
        self.parts
            .push(Part::new(Kind::Unparsed(err), src.to_string()));
    }

    /// Parse a nested string (`$(…)`, `bash -c`, `eval`) one level deeper.
    fn nested(&mut self, src: &str, ctx: Ctx) {
        if self.depth >= MAX_DEPTH {
            self.unparsed(src, "nested too deep".into());
            return;
        }
        self.depth += 1;
        self.program(src, ctx);
        self.depth -= 1;
    }

    fn list(&mut self, l: &ast::CompoundList, ctx: Ctx) {
        for ast::CompoundListItem(ao, sep) in &l.0 {
            let bg = matches!(sep, ast::SeparatorOperator::Async);
            let c = Ctx {
                background: ctx.background || bg,
                ..ctx
            };
            self.pipeline(&ao.first, c);
            for x in &ao.additional {
                match x {
                    ast::AndOr::And(p) | ast::AndOr::Or(p) => self.pipeline(p, c),
                }
            }
        }
    }

    fn pipeline(&mut self, p: &ast::Pipeline, ctx: Ctx) {
        for (i, c) in p.seq.iter().enumerate() {
            self.command(
                c,
                Ctx {
                    piped: ctx.piped || i > 0,
                    ..ctx
                },
            );
        }
    }

    fn command(&mut self, c: &ast::Command, ctx: Ctx) {
        match c {
            ast::Command::Simple(sc) => self.simple(sc, ctx),
            ast::Command::Compound(cc, rl) => {
                let mut subs = vec![];
                let redirs = rl
                    .as_ref()
                    .map(|r| r.0.iter().map(|x| redir(x, &mut subs)).collect::<Vec<_>>())
                    .unwrap_or_default();
                let heredocs = heredoc_bodies(
                    rl.as_ref().map(|r| r.0.as_slice()).unwrap_or(&[]),
                    &mut subs,
                );
                if !redirs.is_empty() {
                    let mut part = Part::new(Kind::Compound("redirect"), c.to_string());
                    part.redirs = redirs;
                    part.heredocs = heredocs;
                    part.piped = ctx.piped;
                    part.background = ctx.background;
                    part.subs = subs.clone();
                    self.parts.push(part);
                }
                self.compound(cc, ctx);
                subs.iter().for_each(|s| self.nested(s, ctx));
            }
            ast::Command::Function(f) => {
                // A function body runs only when called: its parts are
                // judged as if they ran (the call itself is a plain name).
                self.parts.push(Part::new(
                    Kind::Compound("function"),
                    format!("{}()", f.fname),
                ));
                self.compound(&f.body.0, ctx);
            }
            ast::Command::ExtendedTest(t, _) => {
                // `[[ … ]]` only tests; a `$(…)` inside still runs
                let src = t.to_string();
                let mut subs = vec![];
                expand(&src, &mut subs);
                let mut part = Part::new(Kind::Compound("test"), src);
                part.subs = subs.clone();
                self.parts.push(part);
                subs.iter().for_each(|s| self.nested(s, ctx));
            }
        }
    }

    fn compound(&mut self, cc: &ast::CompoundCommand, ctx: Ctx) {
        use ast::CompoundCommand as C;
        match cc {
            C::BraceGroup(b) => self.list(&b.list, ctx),
            C::Subshell(s) => self.list(&s.list, ctx),
            C::ForClause(f) => {
                self.assigned.push(f.variable_name.clone());
                let mut subs = vec![];
                f.values.iter().flatten().for_each(|w| {
                    expand(&w.value, &mut subs);
                });
                subs.iter().for_each(|s| self.nested(s, ctx));
                self.list(&f.body.list, ctx);
            }
            C::IfClause(i) => {
                self.list(&i.condition, ctx);
                self.list(&i.then, ctx);
                for e in i.elses.iter().flatten() {
                    if let Some(c) = &e.condition {
                        self.list(c, ctx);
                    }
                    self.list(&e.body, ctx);
                }
            }
            C::WhileClause(w) | C::UntilClause(w) => {
                self.list(&w.0, ctx);
                self.list(&w.1.list, ctx);
            }
            C::CaseClause(c) => {
                let mut subs = vec![];
                expand(&c.value.value, &mut subs);
                subs.iter().for_each(|s| self.nested(s, ctx));
                c.cases
                    .iter()
                    .filter_map(|it| it.cmd.as_ref())
                    .for_each(|l| self.list(l, ctx));
            }
            C::Coprocess(_) => self.unparsed(&cc.to_string(), "coproc".into()),
            C::Arithmetic(_) | C::ArithmeticForClause(_) => {
                // `(( … ))` can hold `$(…)`: unreadable
                let mut part = Part::new(Kind::Compound("arithmetic"), cc.to_string());
                part.words = vec![Word {
                    text: "((".into(),
                    unreadable: true,
                }];
                self.parts.push(part);
            }
        }
    }

    fn simple(&mut self, sc: &ast::SimpleCommand, ctx: Ctx) {
        let mut words: Vec<Word> = vec![];
        let mut assigns = vec![];
        let mut redirs = vec![];
        let mut subs = vec![];
        let mut raw_redirs = vec![];
        let items = sc
            .prefix
            .iter()
            .flat_map(|p| p.0.iter())
            .map(Item::Prefix)
            .chain(sc.word_or_name.iter().map(Item::Name))
            .chain(sc.suffix.iter().flat_map(|p| p.0.iter()).map(Item::Prefix));
        for it in items {
            match it {
                Item::Name(w) => words.push(expand(&w.value, &mut subs)),
                Item::Prefix(ast::CommandPrefixOrSuffixItem::Word(w)) => {
                    words.push(expand(&w.value, &mut subs))
                }
                Item::Prefix(ast::CommandPrefixOrSuffixItem::AssignmentWord(a, w)) => {
                    // an assignment after the program is a plain argument
                    // (`make X=1`); before it, an environment variable
                    let value = assignment_value(&a.value, &mut subs);
                    if words.is_empty() {
                        assigns.push(format!("{}={}", a.name, value.text));
                    } else {
                        words.push(Word {
                            text: w.value.clone(),
                            unreadable: value.unreadable,
                        });
                    }
                }
                Item::Prefix(ast::CommandPrefixOrSuffixItem::IoRedirect(r)) => {
                    redirs.push(redir(r, &mut subs));
                    raw_redirs.push(r.clone());
                }
                Item::Prefix(ast::CommandPrefixOrSuffixItem::ProcessSubstitution(_, s)) => {
                    subs.push(s.list.to_string());
                    words.push(Word {
                        text: format!("<({})", s.list),
                        unreadable: true,
                    });
                }
            }
        }
        let heredocs = heredoc_bodies(&raw_redirs, &mut subs);
        let mut part = Part::new(Kind::Simple, sc.to_string());
        part.assigns = assigns;
        part.redirs = redirs;
        part.heredocs = heredocs;
        part.piped = ctx.piped;
        part.background = ctx.background;
        part.subs = subs.clone();
        let (words, looked) = unwrap(words, &mut part);
        part.words = words;
        part.wrappers = looked;
        if part.words.is_empty() && part.assigns.is_empty() && part.redirs.is_empty() {
            return;
        }
        // `bash -c '<script>'`, `eval '<script>'`: the string is what runs
        if let Some(inner) = inline_shell(&part) {
            let has_files = part.redirs.iter().any(|r| r.written().is_some());
            if has_files || !part.heredocs.is_empty() {
                let mut outer = Part::new(Kind::Compound("redirect"), part.source.clone());
                outer.redirs = part.redirs.clone();
                outer.heredocs = part.heredocs.clone();
                outer.piped = part.piped;
                outer.background = part.background;
                self.parts.push(outer);
            }
            self.nested(&inner, ctx);
        } else {
            self.parts.push(part);
        }
        subs.iter().for_each(|s| {
            self.nested(
                s,
                Ctx {
                    piped: false,
                    ..ctx
                },
            )
        });
    }
}

enum Item<'a> {
    Prefix(&'a ast::CommandPrefixOrSuffixItem),
    Name(&'a ast::Word),
}

/// The string of `bash -c '<s>'` (`sh`, `zsh`, `-lc`, `-ec`…) or of
/// `eval <static words>`, when it is readable.
fn inline_shell(part: &Part) -> Option<String> {
    let name = part.name()?;
    let args = part.args();
    if name == "eval" {
        return (!args.is_empty() && args.iter().all(|w| !w.unreadable)).then(|| {
            args.iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        });
    }
    if !matches!(name, "bash" | "sh" | "zsh" | "dash") {
        return None;
    }
    // flags before the string: `-c`, `-lc`, `-e -c`, `-o pipefail -c`
    let mut i = 0;
    while let Some(w) = args.get(i) {
        let t = w.text.as_str();
        if !t.starts_with('-') || t == "-" || t == "--" {
            return None;
        }
        if t == "-o" || t == "+o" {
            i += 2;
            continue;
        }
        if !t.starts_with("--") && t[1..].contains('c') {
            let s = args.get(i + 1)?;
            return (!s.unreadable).then(|| s.text.clone());
        }
        i += 1;
    }
    None
}

/// Look through the wrappers: gives the words from the real program on and
/// the wrappers' names; `env`'s assignments go to the part, `xargs` marks
/// its extra arguments, `git -C` keeps its folder.
fn unwrap(mut w: Vec<Word>, part: &mut Part) -> (Vec<Word>, Vec<String>) {
    let mut looked = vec![];
    while let Some(first) = w.first() {
        if first.unreadable {
            break;
        }
        let name = first.text.rsplit('/').next().unwrap_or("").to_string();
        let rest: Vec<Word> = w[1..].to_vec();
        let after = match name.as_str() {
            "env" => {
                let (vars, rest) = env_args(rest);
                part.assigns.extend(vars);
                rest
            }
            "time" => skip_flags(rest, &[]),
            "nohup" => rest,
            "command" => {
                // `command -v x` asks where x is: nothing runs
                if rest
                    .first()
                    .is_some_and(|a| a.text == "-v" || a.text == "-V")
                {
                    break;
                }
                skip_flags(rest, &[])
            }
            "exec" => skip_flags(rest, &["-a"]),
            "nice" => skip_flags(rest, &["-n"]),
            "timeout" => {
                let r = skip_flags(rest, &["-s", "-k", "--signal", "--kill-after"]);
                // the duration
                if r.first()
                    .is_some_and(|a| a.text.starts_with(|c: char| c.is_ascii_digit()))
                {
                    r[1..].to_vec()
                } else {
                    r
                }
            }
            "caffeinate" => skip_flags(rest, &["-t", "-w"]),
            "stdbuf" => skip_flags(rest, &["-i", "-o", "-e"]),
            "xargs" => {
                let r = skip_flags(
                    rest,
                    &[
                        "-I", "-i", "-n", "-P", "-L", "-l", "-d", "-s", "-E", "-e", "-a", "-J",
                        "-R", "-S",
                    ],
                );
                part.stdin_args = true;
                if r.is_empty() {
                    // `xargs` alone runs `echo`
                    vec![Word::plain("echo")]
                } else {
                    r
                }
            }
            "git" => {
                let r = git_globals(rest, part);
                w = std::iter::once(w[0].clone()).chain(r).collect();
                break;
            }
            _ => break,
        };
        if after.is_empty() {
            // a wrapper with no program (`env`, `time`): it is the program
            break;
        }
        looked.push(name);
        w = after;
    }
    (w, looked)
}

/// `env [-i] [-u NAME] [NAME=v]… prog …`: the variables, then the program.
fn env_args(rest: Vec<Word>) -> (Vec<String>, Vec<Word>) {
    let mut vars = vec![];
    let mut i = 0;
    while let Some(a) = rest.get(i) {
        let t = a.text.as_str();
        if t == "-u" || t == "--unset" || t == "-C" || t == "--chdir" {
            i += 2;
        } else if t == "-S" || t.starts_with("--split-string") {
            // `env -S "prog args"` splits a string: unreadable
            return (
                vars,
                vec![Word {
                    text: "env -S".into(),
                    unreadable: true,
                }],
            );
        } else if t.starts_with('-') {
            i += 1;
        } else if t.contains('=') && !t.starts_with('=') {
            vars.push(t.to_string());
            i += 1;
        } else {
            break;
        }
    }
    (vars, rest[i.min(rest.len())..].to_vec())
}

/// Drop leading flags; `with_value` flags take the next word (unless glued).
fn skip_flags(rest: Vec<Word>, with_value: &[&str]) -> Vec<Word> {
    let mut i = 0;
    while let Some(a) = rest.get(i) {
        let t = a.text.as_str();
        if t == "--" {
            i += 1;
            break;
        }
        if !t.starts_with('-') || t == "-" {
            break;
        }
        i += if with_value.contains(&t) { 2 } else { 1 };
    }
    rest[i.min(rest.len())..].to_vec()
}

/// git's global options before the subcommand: `-C dir`, `--git-dir`,
/// `--work-tree`, `--no-pager`, `-c k=v`.
fn git_globals(rest: Vec<Word>, part: &mut Part) -> Vec<Word> {
    let mut i = 0;
    while let Some(a) = rest.get(i) {
        let t = a.text.as_str();
        match t {
            "-C" | "--work-tree" => {
                if let Some(d) = rest.get(i + 1) {
                    part.git_dir = Some(match &part.git_dir {
                        // `git -C a -C b` is `a/b`
                        Some(prev) if !d.text.starts_with('/') => Word {
                            text: format!("{}/{}", prev.text, d.text),
                            unreadable: prev.unreadable || d.unreadable,
                        },
                        _ => d.clone(),
                    });
                }
                i += 2;
            }
            "--git-dir" | "--namespace" => i += 2,
            "-c" | "--config-env" | "--exec-path" => {
                part.git_config = true;
                i += 2;
            }
            _ if t.starts_with("--git-dir=")
                || t.starts_with("--work-tree=")
                || t.starts_with("--namespace=") =>
            {
                if let Some(d) = t.strip_prefix("--work-tree=") {
                    part.git_dir = Some(Word {
                        text: d.into(),
                        unreadable: a.unreadable,
                    });
                }
                i += 1;
            }
            _ if t.starts_with("--exec-path") || t.starts_with("--config-env") => {
                part.git_config = true;
                i += 1;
            }
            "--no-pager"
            | "-P"
            | "--paginate"
            | "-p"
            | "--no-optional-locks"
            | "--bare"
            | "--literal-pathspecs"
            | "--no-replace-objects"
            | "--glob-pathspecs"
            | "--noglob-pathspecs"
            | "--icase-pathspecs" => i += 1,
            _ => break,
        }
    }
    rest[i.min(rest.len())..].to_vec()
}

fn assignment_value(v: &ast::AssignmentValue, subs: &mut Vec<String>) -> Word {
    match v {
        ast::AssignmentValue::Scalar(w) => expand(&w.value, subs),
        ast::AssignmentValue::Array(items) => {
            let ws: Vec<Word> = items.iter().map(|(_, w)| expand(&w.value, subs)).collect();
            Word {
                text: format!(
                    "({})",
                    ws.iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                unreadable: ws.iter().any(|w| w.unreadable),
            }
        }
    }
}

fn redir(r: &ast::IoRedirect, subs: &mut Vec<String>) -> Redir {
    use ast::IoFileRedirectTarget as T;
    match r {
        ast::IoRedirect::File(_, kind, t) => {
            let op = kind.to_string();
            let target = match t {
                T::Filename(w) => Target::Path(expand(&w.value, subs)),
                T::Fd(_) => Target::Fd,
                T::Duplicate(w) => {
                    // `>&2`, `>&-` renumber; `>&file` writes the file
                    if w.value.chars().all(|c| c.is_ascii_digit() || c == '-') {
                        Target::Fd
                    } else {
                        Target::Path(expand(&w.value, subs))
                    }
                }
                T::ProcessSubstitution(_, sc) => {
                    subs.push(sc.list.to_string());
                    Target::ProcSub
                }
            };
            Redir { op, target }
        }
        ast::IoRedirect::HereDocument(..) => Redir {
            op: "<<".into(),
            target: Target::HereDoc,
        },
        ast::IoRedirect::HereString(_, w) => {
            expand(&w.value, subs);
            Redir {
                op: "<<<".into(),
                target: Target::HereString,
            }
        }
        ast::IoRedirect::OutputAndError(w, append) => Redir {
            op: if *append { "&>>".into() } else { "&>".into() },
            target: Target::Path(expand(&w.value, subs)),
        },
    }
}

/// The bodies of the here-documents; an unquoted delimiter expands the
/// body, so its `$(…)` run: they are collected (quotes are literal in a
/// body, so they are blanked before the word parser reads it).
fn heredoc_bodies(rs: &[ast::IoRedirect], subs: &mut Vec<String>) -> Vec<String> {
    rs.iter()
        .filter_map(|r| match r {
            ast::IoRedirect::HereDocument(_, h) => {
                let body = h.doc.value.clone();
                if h.requires_expansion && (body.contains("$(") || body.contains('`')) {
                    let blanked: String = body
                        .chars()
                        .map(|c| if c == '\'' || c == '"' { '_' } else { c })
                        .collect();
                    expand(&blanked, subs);
                }
                Some(body)
            }
            _ => None,
        })
        .collect()
}

/// Quote removal on one word; `$(…)` bodies go to `subs`.
fn expand(w: &str, subs: &mut Vec<String>) -> Word {
    // most words have nothing to remove or expand: skip the word parser
    if !w.contains(['$', '`', '\'', '"', '\\', '~', '{']) {
        return Word::plain(w);
    }
    let Ok(pieces) = word::parse(w, &options()) else {
        return Word {
            text: w.to_string(),
            unreadable: true,
        };
    };
    let mut out = Word {
        text: String::new(),
        unreadable: false,
    };
    pieces_into(w, &pieces, &mut out, subs, false);
    out
}

fn pieces_into(
    src: &str,
    ps: &[WordPieceWithSource],
    out: &mut Word,
    subs: &mut Vec<String>,
    quoted: bool,
) {
    for p in ps {
        let raw = src.get(p.start_index..p.end_index).unwrap_or("");
        match &p.piece {
            WordPiece::Text(t) => {
                // unquoted `{a,b}` / `{1..3}` expands to other words
                if !quoted && t.contains('{') && (t.contains(',') || t.contains("..")) {
                    out.unreadable = true;
                }
                out.text.push_str(t)
            }
            WordPiece::SingleQuotedText(t) => out.text.push_str(t),
            WordPiece::AnsiCQuotedText(t) => {
                // escapes we do not decode
                out.unreadable |= t.contains('\\');
                out.text.push_str(t)
            }
            WordPiece::EscapeSequence(t) => out.text.push_str(t.strip_prefix('\\').unwrap_or(t)),
            WordPiece::DoubleQuotedSequence(v) | WordPiece::GettextDoubleQuotedSequence(v) => {
                pieces_into(src, v, out, subs, true)
            }
            WordPiece::TildeExpansion(word::TildeExpr::Home) => out.text.push('~'),
            WordPiece::TildeExpansion(_) => {
                out.unreadable = true;
                out.text.push_str(raw)
            }
            WordPiece::ParameterExpansion(_) | WordPiece::ArithmeticExpression(_) => {
                out.unreadable = true;
                out.text.push_str(raw)
            }
            WordPiece::CommandSubstitution(c) | WordPiece::BackquotedCommandSubstitution(c) => {
                out.unreadable = true;
                subs.push(c.clone());
                out.text.push_str(raw)
            }
        }
    }
}
