//! Syntax highlighting of fenced code blocks (BISE-276): the composer
//! colors a ```ts block as you type it (mdlive.rs).
//!
//! A small lexer per family of languages, no dependency (syntect with
//! its bundled grammars is ~1 MB of binary and tens of ms to load; the
//! roles here are the six `syntax_*` of theme.rs anyway). It runs one
//! line at a time: a line starts in the [`State`] the line before left
//! (a block comment, a string that spans lines) and returns its own, so
//! a caller highlights only the lines it shows and caches each line by
//! (language, state, text).

use crate::theme;
use ratatui::style::{Modifier, Style};

/// What a run of chars is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tok {
    Plain,
    Punct,
    Keyword,
    String,
    Comment,
    Number,
    /// a call, a macro, a key (JSON, TOML, YAML), a CSS property
    Call,
    /// a type, a CSS selector, an HTML attribute
    Type,
    /// a diff's added and removed lines
    Added,
    Removed,
}

/// What a line hands to the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum State {
    #[default]
    Normal,
    /// inside a block comment, `n` deep (Rust nests them)
    Block(u8),
    /// inside a string that spans lines, tripled (`"""`) or not
    Str { quote: char, triple: bool },
    /// inside a Rust raw string with `n` hashes
    Raw(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    /// C, TypeScript, Rust, Go, Python, SQL...: comments, strings,
    /// numbers, keywords, calls, types
    Code,
    Bash,
    Json,
    Toml,
    Yaml,
    Css,
    Html,
    Diff,
}

/// A language: its fence tags and how to read it.
pub(crate) struct Lang {
    pub(crate) names: &'static [&'static str],
    family: Family,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    line_comments: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    /// the quotes of strings
    quotes: &'static str,
    /// the quotes whose strings may span lines (a template literal)
    multiline: &'static str,
    /// `"""` and `'''` strings (Python)
    triple: bool,
    /// a capitalized name is a type
    caps_types: bool,
    /// keywords in any case (SQL)
    any_case: bool,
    /// Rust: nested block comments, raw strings, char literals and
    /// lifetimes, `name!` macros
    rust: bool,
}

const BASE: Lang = Lang {
    names: &[],
    family: Family::Code,
    keywords: &[],
    types: &[],
    line_comments: &[],
    block: None,
    quotes: "\"'",
    multiline: "",
    triple: false,
    caps_types: true,
    any_case: false,
    rust: false,
};

const RUST_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "yield",
];
const RUST_TYPES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64",
];
const PY_KW: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "match",
    "case", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "self",
];
const PY_TYPES: &[&str] = &["int", "float", "str", "bool", "list", "dict", "set", "tuple", "bytes", "object"];
const GO_KW: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go",
    "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type",
    "var", "true", "false", "nil",
];
const GO_TYPES: &[&str] = &[
    "bool", "byte", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64", "rune", "string",
    "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "any",
];
/// C, C++, Java, C#, Kotlin, Swift, Dart, Scala: one list for the family
const C_KW: &[&str] = &[
    "abstract", "auto", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else",
    "enum", "extends", "extern", "false", "final", "finally", "for", "fun", "func", "goto", "if", "implements",
    "import", "in", "interface", "let", "namespace", "new", "null", "nullptr", "object", "override", "package",
    "private", "protected", "public", "return", "sizeof", "static", "struct", "super", "switch", "template", "this",
    "throw", "throws", "true", "try", "typedef", "typename", "union", "using", "val", "var", "virtual", "void",
    "volatile", "when", "while", "#include", "#define", "#if", "#ifdef", "#ifndef", "#endif", "#else", "#pragma",
];
const C_TYPES: &[&str] = &[
    "bool", "boolean", "byte", "char", "double", "float", "int", "long", "short", "signed", "unsigned", "size_t",
    "string", "String", "uint8_t", "uint32_t", "uint64_t", "int32_t", "int64_t",
];
const RUBY_KW: &[&str] = &[
    "alias", "and", "begin", "break", "case", "class", "def", "defined?", "do", "else", "elsif", "end", "ensure",
    "false", "for", "if", "in", "module", "next", "nil", "not", "or", "redo", "rescue", "retry", "return", "self",
    "super", "then", "true", "undef", "unless", "until", "when", "while", "yield", "require",
];
const LUA_KW: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];
const SQL_KW: &[&str] = &[
    "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create",
    "table", "drop", "alter", "add", "index", "on", "join", "left", "right", "inner", "outer", "full", "group",
    "by", "order", "having", "limit", "offset", "as", "distinct", "union", "all", "case", "when", "then", "else",
    "end", "null", "is", "in", "like", "between", "exists", "primary", "key", "foreign", "references", "default",
    "with", "returning", "asc", "desc", "true", "false", "count", "sum", "avg", "min", "max",
];
const JSON_KW: &[&str] = &["true", "false", "null"];
const YAML_KW: &[&str] = &["true", "false", "null", "yes", "no", "on", "off", "~"];

/// The languages a fence may name (its first word, any case).
pub(crate) static LANGS: &[Lang] = &[
    Lang {
        names: &["ts", "typescript", "tsx", "js", "javascript", "jsx", "mjs", "cjs", "mts"],
        keywords: crate::code::TS_KEYWORDS,
        line_comments: &["//"],
        block: Some(("/*", "*/")),
        quotes: "\"'`",
        multiline: "`",
        ..BASE
    },
    Lang {
        names: &["rust", "rs"],
        keywords: RUST_KW,
        types: RUST_TYPES,
        line_comments: &["//"],
        block: Some(("/*", "*/")),
        quotes: "\"",
        multiline: "\"",
        rust: true,
        ..BASE
    },
    Lang {
        names: &["python", "py", "py3", "python3"],
        keywords: PY_KW,
        types: PY_TYPES,
        line_comments: &["#"],
        triple: true,
        ..BASE
    },
    Lang {
        names: &["go", "golang"],
        keywords: GO_KW,
        types: GO_TYPES,
        line_comments: &["//"],
        block: Some(("/*", "*/")),
        quotes: "\"'`",
        multiline: "`",
        ..BASE
    },
    Lang {
        names: &[
            "c", "h", "cpp", "c++", "cc", "cxx", "hpp", "objc", "java", "kotlin", "kt", "swift", "cs", "csharp",
            "dart", "scala", "zig",
        ],
        keywords: C_KW,
        types: C_TYPES,
        line_comments: &["//"],
        block: Some(("/*", "*/")),
        ..BASE
    },
    Lang { names: &["ruby", "rb"], keywords: RUBY_KW, line_comments: &["#"], ..BASE },
    Lang { names: &["lua"], keywords: LUA_KW, line_comments: &["--"], caps_types: false, ..BASE },
    Lang {
        names: &["sql", "postgres", "postgresql", "mysql", "sqlite", "psql"],
        keywords: SQL_KW,
        line_comments: &["--"],
        block: Some(("/*", "*/")),
        caps_types: false,
        any_case: true,
        ..BASE
    },
    Lang {
        names: &["bash", "sh", "shell", "zsh", "console", "shellscript", "fish", "ksh"],
        family: Family::Bash,
        quotes: "\"'",
        multiline: "\"'",
        ..BASE
    },
    Lang {
        names: &["json", "jsonc", "json5", "jsonl"],
        family: Family::Json,
        keywords: JSON_KW,
        line_comments: &["//"],
        block: Some(("/*", "*/")),
        quotes: "\"",
        ..BASE
    },
    Lang { names: &["toml"], family: Family::Toml, keywords: JSON_KW, line_comments: &["#"], triple: true, ..BASE },
    Lang { names: &["yaml", "yml"], family: Family::Yaml, keywords: YAML_KW, line_comments: &["#"], ..BASE },
    Lang { names: &["css", "scss", "less", "sass"], family: Family::Css, block: Some(("/*", "*/")), ..BASE },
    Lang {
        names: &["html", "xml", "svg", "htm", "xhtml", "vue", "svelte", "plist"],
        family: Family::Html,
        block: Some(("<!--", "-->")),
        ..BASE
    },
    Lang { names: &["diff", "patch"], family: Family::Diff, ..BASE },
];

/// The language of a fence's info string (```ts, ```rust title=x):
/// its first word; None for none or an unknown one (plain text).
pub(crate) fn lang_of(info: &str) -> Option<&'static Lang> {
    let tag = info.split(|c: char| c.is_whitespace() || c == ',' || c == '{').next()?.trim_start_matches('.');
    if tag.is_empty() {
        return None;
    }
    let tag = tag.to_ascii_lowercase();
    LANGS.iter().find(|l| l.names.contains(&tag.as_str()))
}

/// The style of a token: the `syntax_*` roles; under `NO_COLOR` only
/// the modifiers (bold keywords, italic comments).
pub(crate) fn style(t: Tok) -> Style {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let s = match t {
        Tok::Keyword => Style::default().add_modifier(Modifier::BOLD),
        Tok::Comment => Style::default().add_modifier(Modifier::ITALIC),
        _ => Style::default(),
    };
    if no_color {
        return s.fg(theme::text());
    }
    s.fg(match t {
        Tok::Plain => theme::text(),
        Tok::Punct => theme::dim(),
        Tok::Keyword => theme::syntax_keyword(),
        Tok::String => theme::syntax_string(),
        Tok::Comment => theme::syntax_comment(),
        Tok::Number => theme::syntax_number(),
        Tok::Call => theme::syntax_call(),
        Tok::Type => theme::syntax_type(),
        Tok::Added => theme::ok(),
        Tok::Removed => theme::error(),
    })
}

/// Runs of (chars, token) that cover a line, merged.
struct Runs(Vec<(usize, Tok)>);

impl Runs {
    fn push(&mut self, n: usize, t: Tok) {
        if n == 0 {
            return;
        }
        match self.0.last_mut() {
            Some(last) if last.1 == t => last.0 += n,
            _ => self.0.push((n, t)),
        }
    }
}

/// One line of `lang` from `state`: its runs of (chars, token), which
/// cover every char of the line, and the state the next line starts in.
pub(crate) fn line(lang: &Lang, state: State, s: &str) -> (Vec<(usize, Tok)>, State) {
    let cs: Vec<char> = s.chars().collect();
    let mut r = Runs(Vec::new());
    let next = match lang.family {
        Family::Code => code_line(lang, state, &cs, &mut r),
        Family::Bash => bash_line(state, &cs, &mut r),
        Family::Json | Family::Toml | Family::Yaml => data_line(lang, state, &cs, &mut r),
        Family::Css => css_line(lang, state, &cs, &mut r),
        Family::Html => html_line(lang, state, &cs, &mut r),
        Family::Diff => {
            let t = if s.starts_with("+++") || s.starts_with("---") {
                Tok::Keyword
            } else if s.starts_with("@@") {
                Tok::Call
            } else if s.starts_with('+') {
                Tok::Added
            } else if s.starts_with('-') {
                Tok::Removed
            } else {
                Tok::Plain
            };
            r.push(cs.len(), t);
            State::Normal
        }
    };
    (r.0, next)
}

fn at(cs: &[char], i: usize, pat: &str) -> bool {
    (i..).zip(pat.chars()).all(|(k, p)| cs.get(k) == Some(&p))
}

fn is_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The rest of a block comment from `i`: where it ends (past its close)
/// and the state after, `depth` deep (nests when `nest`).
fn block_end(cs: &[char], mut i: usize, (open, close): (&str, &str), mut depth: u8, nest: bool) -> (usize, State) {
    while i < cs.len() {
        if at(cs, i, close) {
            i += close.chars().count();
            depth -= 1;
            if depth == 0 {
                return (i, State::Normal);
            }
        } else if nest && at(cs, i, open) {
            i += open.chars().count();
            depth = depth.saturating_add(1);
        } else {
            i += 1;
        }
    }
    (i, State::Block(depth))
}

/// The rest of a string from `i` (past its opening quote): where it ends
/// and the state after. `escapes`: a backslash escapes the next char.
fn str_end(cs: &[char], mut i: usize, quote: char, triple: bool, escapes: bool, spans: bool) -> (usize, State) {
    while i < cs.len() {
        if escapes && cs[i] == '\\' {
            i += 2;
            continue;
        }
        if cs[i] == quote && (!triple || (cs.get(i + 1) == Some(&quote) && cs.get(i + 2) == Some(&quote))) {
            return (i + if triple { 3 } else { 1 }, State::Normal);
        }
        i += 1;
    }
    let st = if spans || triple { State::Str { quote, triple } } else { State::Normal };
    (cs.len(), st)
}

/// A Rust raw string's end from `i`: `"` then `hashes` `#`.
fn raw_end(cs: &[char], mut i: usize, hashes: u8) -> (usize, State) {
    while i < cs.len() {
        if cs[i] == '"' && (1..=hashes as usize).all(|k| cs.get(i + k) == Some(&'#')) {
            return (i + 1 + hashes as usize, State::Normal);
        }
        i += 1;
    }
    (cs.len(), State::Raw(hashes))
}

/// Continues the state the line starts in: the chars it covers.
fn resume(lang: &Lang, state: State, cs: &[char], r: &mut Runs, escapes: bool) -> (usize, State) {
    let (end, st) = match state {
        State::Normal => return (0, State::Normal),
        State::Block(d) => {
            let b = lang.block.unwrap_or(("/*", "*/"));
            let (e, st) = block_end(cs, 0, b, d.max(1), lang.rust);
            r.push(e, Tok::Comment);
            return (e, st);
        }
        State::Str { quote, triple } => str_end(cs, 0, quote, triple, escapes && quote != '\'', true),
        State::Raw(h) => raw_end(cs, 0, h),
    };
    r.push(end, Tok::String);
    (end, st)
}

fn number_len(cs: &[char], i: usize) -> usize {
    let mut k = i;
    while k < cs.len() && (cs[k].is_ascii_alphanumeric() || cs[k] == '_' || (cs[k] == '.' && cs.get(k + 1).is_some_and(|c| c.is_ascii_digit()))) {
        k += 1;
    }
    k - i
}

fn code_line(lang: &Lang, state: State, cs: &[char], r: &mut Runs) -> State {
    let (mut i, mut st) = resume(lang, state, cs, r, true);
    if st != State::Normal {
        return st;
    }
    let n = cs.len();
    while i < n {
        let c = cs[i];
        if c.is_whitespace() {
            r.push(1, Tok::Plain);
            i += 1;
            continue;
        }
        if lang.line_comments.iter().any(|p| at(cs, i, p)) && !(c == '#' && lang.keywords == C_KW) {
            r.push(n - i, Tok::Comment);
            return State::Normal;
        }
        if let Some(b) = lang.block.filter(|b| at(cs, i, b.0)) {
            let (e, s2) = block_end(cs, i + b.0.chars().count(), b, 1, lang.rust);
            r.push(e - i, Tok::Comment);
            i = e;
            st = s2;
            if st != State::Normal {
                return st;
            }
            continue;
        }
        // Rust raw strings r"…", r#"…"#, br"…"
        if lang.rust && (c == 'r' || (c == 'b' && cs.get(i + 1) == Some(&'r'))) && !cs.get(i.wrapping_sub(1)).is_some_and(|&p| is_id(p)) {
            let s0 = if c == 'b' { i + 2 } else { i + 1 };
            let hashes = cs[s0.min(n)..].iter().take_while(|&&h| h == '#').count();
            if cs.get(s0 + hashes) == Some(&'"') {
                let (e, s2) = raw_end(cs, s0 + hashes + 1, hashes as u8);
                r.push(e - i, Tok::String);
                i = e;
                if s2 != State::Normal {
                    return s2;
                }
                continue;
            }
        }
        // Rust: 'x' and '\n' are chars, 'a a lifetime
        if lang.rust && c == '\'' {
            let close = if cs.get(i + 1) == Some(&'\\') {
                (i + 3..n.min(i + 12)).find(|&k| cs[k] == '\'')
            } else {
                Some(i + 2).filter(|&k| cs.get(k) == Some(&'\''))
            };
            match close {
                Some(k) => {
                    r.push(k + 1 - i, Tok::String);
                    i = k + 1;
                }
                None => {
                    let e = (i + 1..n).find(|&k| !is_id(cs[k])).unwrap_or(n);
                    r.push(e - i, Tok::Type);
                    i = e;
                }
            }
            continue;
        }
        if lang.quotes.contains(c) {
            let triple = lang.triple && cs.get(i + 1) == Some(&c) && cs.get(i + 2) == Some(&c);
            let from = i + if triple { 3 } else { 1 };
            let (e, s2) = str_end(cs, from, c, triple, true, lang.multiline.contains(c));
            r.push(e - i, Tok::String);
            i = e;
            if s2 != State::Normal {
                return s2;
            }
            continue;
        }
        if c.is_ascii_digit() && !cs.get(i.wrapping_sub(1)).is_some_and(|&p| is_id(p)) {
            let k = number_len(cs, i);
            r.push(k, Tok::Number);
            i += k;
            continue;
        }
        // a decorator (Python @name, Java @Override)
        if c == '@' && cs.get(i + 1).is_some_and(|&d| d.is_alphabetic()) {
            let e = (i + 1..n).find(|&k| !(is_id(cs[k]) || cs[k] == '.')).unwrap_or(n);
            r.push(e - i, Tok::Call);
            i = e;
            continue;
        }
        let pre = c == '#' && lang.keywords == C_KW && cs[..i].iter().all(|c| c.is_whitespace());
        if c.is_alphabetic() || c == '_' || (c == '$' && lang.keywords == crate::code::TS_KEYWORDS) || pre {
            let e = (i + 1..n).find(|&k| !(is_id(cs[k]) || (lang.keywords == RUBY_KW && cs[k] == '?'))).unwrap_or(n);
            let w: String = cs[i..e].iter().collect();
            let mut k = e;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let kw = if lang.any_case { lang.keywords.contains(&w.to_ascii_lowercase().as_str()) } else { lang.keywords.contains(&w.as_str()) };
            let (len, t) = if kw {
                (e - i, Tok::Keyword)
            } else if lang.types.contains(&w.as_str()) {
                (e - i, Tok::Type)
            } else if lang.rust && cs.get(e) == Some(&'!') && cs.get(e + 1) != Some(&'=') {
                (e + 1 - i, Tok::Call)
            } else if cs.get(k) == Some(&'(') {
                (e - i, Tok::Call)
            } else if lang.caps_types && w.starts_with(char::is_uppercase) {
                (e - i, Tok::Type)
            } else {
                (e - i, Tok::Plain)
            };
            r.push(len, t);
            i += len;
            continue;
        }
        r.push(1, Tok::Punct);
        i += 1;
    }
    st
}

const BASH_KW: &[&str] = crate::code::BASH_KEYWORDS;
const BASH_CMD_AFTER: &[&str] = crate::code::BASH_CMD_AFTER;

fn bash_line(state: State, cs: &[char], r: &mut Runs) -> State {
    let n = cs.len();
    let mut i = 0usize;
    if let State::Str { quote, .. } = state {
        let (e, st) = str_end(cs, 0, quote, false, quote == '"', true);
        r.push(e, Tok::String);
        if st != State::Normal {
            return st;
        }
        i = e;
    }
    let mut cmd_pos = i == 0;
    while i < n {
        let c = cs[i];
        if c.is_whitespace() {
            r.push(1, Tok::Plain);
            i += 1;
            continue;
        }
        if c == '#' && (i == 0 || cs[i - 1].is_whitespace() || matches!(cs[i - 1], ';' | '(' | '|' | '&')) {
            r.push(n - i, Tok::Comment);
            break;
        }
        if c == '\'' || c == '"' || c == '`' {
            let (e, st) = str_end(cs, i + 1, c, false, c == '"', true);
            r.push(e - i, Tok::String);
            cmd_pos = false;
            if st != State::Normal {
                return st;
            }
            i = e;
            continue;
        }
        if c == '$' {
            if cs.get(i + 1) == Some(&'(') {
                r.push(2, Tok::Keyword);
                i += 2;
                cmd_pos = true;
                continue;
            }
            if let Some((v, j)) = crate::code::bash_var(cs, i) {
                r.push(v.chars().count(), Tok::Number);
                i = j;
                cmd_pos = false;
                continue;
            }
        }
        if matches!(c, '|' | '&' | ';' | '(' | ')') {
            r.push(1, Tok::Keyword);
            cmd_pos = c != ')';
            i += 1;
            continue;
        }
        if matches!(c, '<' | '>') {
            r.push(1, Tok::Keyword);
            i += 1;
            continue;
        }
        if !crate::code::is_bash_word_char(c) {
            r.push(1, Tok::Punct);
            i += 1;
            continue;
        }
        let e = (i..n).find(|&k| !crate::code::is_bash_word_char(cs[k])).unwrap_or(n);
        let w: String = cs[i..e].iter().collect();
        if let Some(eq) = w.find('=').filter(|&p| cmd_pos && p > 0 && w[..p].chars().all(|c| c.is_alphanumeric() || c == '_')) {
            // X=1 cmd: the name, then the value; still a command after
            r.push(w[..eq].chars().count(), Tok::Number);
            r.push(w[eq..].chars().count(), Tok::Plain);
        } else if BASH_KW.contains(&w.as_str()) {
            r.push(e - i, Tok::Keyword);
            cmd_pos = BASH_CMD_AFTER.contains(&w.as_str());
        } else if cmd_pos {
            r.push(e - i, Tok::Call);
            cmd_pos = false;
        } else if w.starts_with('-') {
            r.push(e - i, Tok::Type);
        } else if w.chars().all(|c| c.is_ascii_digit()) {
            r.push(e - i, Tok::Number);
        } else {
            r.push(e - i, Tok::Plain);
        }
        i = e;
    }
    State::Normal
}

/// JSON, TOML, YAML: keys, strings, numbers, true/false/null.
fn data_line(lang: &Lang, state: State, cs: &[char], r: &mut Runs) -> State {
    let n = cs.len();
    let (mut i, st) = resume(lang, state, cs, r, true);
    if st != State::Normal {
        return st;
    }
    let lead = cs.iter().take_while(|c| c.is_whitespace()).count();
    // TOML [table] / [[array]]
    if lang.family == Family::Toml && i == 0 && cs.get(lead) == Some(&'[') {
        r.push(lead, Tok::Plain);
        let e = cs.iter().rposition(|&c| c == ']').map(|k| k + 1).unwrap_or(n);
        r.push(e - lead, Tok::Type);
        i = e;
    }
    // YAML `key:` and `- key:`, TOML `key =`: the key
    let mut key_done = lang.family == Family::Json || i > 0;
    while i < n {
        let c = cs[i];
        if c.is_whitespace() {
            r.push(1, Tok::Plain);
            i += 1;
            continue;
        }
        if lang.line_comments.iter().any(|p| at(cs, i, p)) && (i == 0 || cs[i - 1].is_whitespace() || lang.family == Family::Json) {
            r.push(n - i, Tok::Comment);
            return State::Normal;
        }
        if let Some(b) = lang.block.filter(|b| at(cs, i, b.0)) {
            let (e, s2) = block_end(cs, i + 2, b, 1, false);
            r.push(e - i, Tok::Comment);
            if s2 != State::Normal {
                return s2;
            }
            i = e;
            continue;
        }
        if lang.family == Family::Yaml && !key_done && c == '-' && cs.get(i + 1).is_none_or(|c| c.is_whitespace()) {
            r.push(1, Tok::Punct);
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            let triple = lang.triple && cs.get(i + 1) == Some(&c) && cs.get(i + 2) == Some(&c);
            let (e, s2) = str_end(cs, i + if triple { 3 } else { 1 }, c, triple, c == '"', false);
            let mut k = e;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let key = matches!(cs.get(k), Some(':') | Some('=')) && (lang.family == Family::Json || !key_done);
            r.push(e - i, if key { Tok::Call } else { Tok::String });
            key_done |= key;
            if s2 != State::Normal {
                return s2;
            }
            i = e;
            continue;
        }
        if !key_done && lang.family != Family::Json && (c.is_alphanumeric() || c == '_') {
            // a bare key up to `:` (YAML) or `=` (TOML)
            let sep = if lang.family == Family::Yaml { ':' } else { '=' };
            let e = (i..n).find(|&k| cs[k] == sep || (sep == '=' && cs[k] == ' ')).unwrap_or(n);
            let mut k = e;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let is_key = cs.get(k) == Some(&sep) && (sep != ':' || cs.get(k + 1).is_none_or(|c| c.is_whitespace()));
            if is_key {
                r.push(e - i, Tok::Call);
                key_done = true;
                i = e;
                continue;
            }
        }
        key_done = true;
        if c.is_ascii_digit() || (c == '-' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let k = 1 + (i + 1..n).take_while(|&k| cs[k].is_ascii_alphanumeric() || matches!(cs[k], '.' | '_' | '-' | ':' | '+')).count();
            r.push(k, Tok::Number);
            i += k;
            continue;
        }
        if c.is_alphabetic() || c == '~' {
            let e = (i..n).find(|&k| !(cs[k].is_alphanumeric() || cs[k] == '_' || cs[k] == '~')).unwrap_or(n);
            let w: String = cs[i..e].iter().collect();
            let t = if lang.keywords.contains(&w.to_ascii_lowercase().as_str()) { Tok::Keyword } else if lang.family == Family::Yaml { Tok::String } else { Tok::Plain };
            r.push(e - i, t);
            i = e;
            continue;
        }
        if lang.family == Family::Yaml && (c == '&' || c == '*') && cs.get(i + 1).is_some_and(|c| c.is_alphanumeric()) {
            let e = (i + 1..n).find(|&k| cs[k].is_whitespace()).unwrap_or(n);
            r.push(e - i, Tok::Type);
            i = e;
            continue;
        }
        r.push(1, Tok::Punct);
        i += 1;
    }
    State::Normal
}

fn css_line(lang: &Lang, state: State, cs: &[char], r: &mut Runs) -> State {
    let n = cs.len();
    let (mut i, st) = resume(lang, state, cs, r, true);
    if st != State::Normal {
        return st;
    }
    // a selector line opens a block (or lists selectors); a declaration
    // line has `prop: value`
    let selector = cs.contains(&'{') || cs.last() == Some(&',');
    while i < n {
        let c = cs[i];
        if at(cs, i, "/*") {
            let (e, s2) = block_end(cs, i + 2, ("/*", "*/"), 1, false);
            r.push(e - i, Tok::Comment);
            if s2 != State::Normal {
                return s2;
            }
            i = e;
            continue;
        }
        if at(cs, i, "//") && lang.names.contains(&"scss") {
            r.push(n - i, Tok::Comment);
            break;
        }
        if c == '"' || c == '\'' {
            let (e, _) = str_end(cs, i + 1, c, false, true, false);
            r.push(e - i, Tok::String);
            i = e;
            continue;
        }
        if c == '@' {
            let e = (i + 1..n).find(|&k| !(cs[k].is_alphanumeric() || cs[k] == '-')).unwrap_or(n);
            r.push(e - i, Tok::Keyword);
            i = e;
            continue;
        }
        let word = |k0: usize| (k0..n).find(|&k| !(cs[k].is_alphanumeric() || cs[k] == '-' || cs[k] == '_')).unwrap_or(n);
        if selector && (c == '.' || c == '#' || c == ':' || c.is_alphabetic() || c == '*' || c == '&') {
            let e = word(i + 1);
            r.push(e - i, Tok::Type);
            i = e;
            continue;
        }
        if c == '#' && cs.get(i + 1).is_some_and(|c| c.is_ascii_hexdigit()) {
            let e = word(i + 1);
            r.push(e - i, Tok::Number);
            i = e;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|c| c.is_ascii_digit())) || (c == '-' && cs.get(i + 1).is_some_and(|c| c.is_ascii_digit())) {
            let e = (i + 1..n).find(|&k| !(cs[k].is_alphanumeric() || cs[k] == '.' || cs[k] == '%')).unwrap_or(n);
            r.push(e - i, Tok::Number);
            i = e;
            continue;
        }
        if c.is_alphabetic() || c == '-' {
            let e = word(i);
            let mut k = e;
            while k < n && cs[k] == ' ' {
                k += 1;
            }
            let t = match cs.get(k) {
                Some(':') => Tok::Call,
                Some('(') => Tok::Call,
                _ if at(cs, i, "!important") => Tok::Keyword,
                _ => Tok::Plain,
            };
            r.push(e - i, t);
            i = e;
            continue;
        }
        r.push(1, if c.is_whitespace() { Tok::Plain } else { Tok::Punct });
        i += 1;
    }
    State::Normal
}

fn html_line(lang: &Lang, state: State, cs: &[char], r: &mut Runs) -> State {
    let n = cs.len();
    let (mut i, st) = resume(lang, state, cs, r, true);
    if st != State::Normal {
        return st;
    }
    let mut in_tag = false;
    while i < n {
        let c = cs[i];
        if at(cs, i, "<!--") {
            let (e, s2) = block_end(cs, i + 4, ("<!--", "-->"), 1, false);
            r.push(e - i, Tok::Comment);
            if s2 != State::Normal {
                return s2;
            }
            i = e;
            continue;
        }
        if c == '<' && cs.get(i + 1).is_some_and(|&d| d.is_alphabetic() || d == '/' || d == '!' || d == '?') {
            let s0 = i + 1 + usize::from(matches!(cs[i + 1], '/' | '!' | '?'));
            let e = (s0..n).find(|&k| !(cs[k].is_alphanumeric() || matches!(cs[k], '-' | ':' | '_' | '.'))).unwrap_or(n);
            r.push(s0 - i, Tok::Punct);
            r.push(e - s0, Tok::Keyword);
            in_tag = true;
            i = e;
            continue;
        }
        if in_tag {
            if c == '>' || at(cs, i, "/>") {
                let k = if c == '>' { 1 } else { 2 };
                r.push(k, Tok::Punct);
                in_tag = false;
                i += k;
                continue;
            }
            if c == '"' || c == '\'' {
                let (e, _) = str_end(cs, i + 1, c, false, false, false);
                r.push(e - i, Tok::String);
                i = e;
                continue;
            }
            if c.is_alphabetic() {
                let e = (i..n).find(|&k| !(cs[k].is_alphanumeric() || matches!(cs[k], '-' | ':' | '_' | '@' | '.'))).unwrap_or(n);
                r.push(e - i, Tok::Type);
                i = e;
                continue;
            }
            r.push(1, if c.is_whitespace() { Tok::Plain } else { Tok::Punct });
            i += 1;
            continue;
        }
        if c == '&' {
            if let Some(e) = (i + 1..n.min(i + 10)).find(|&k| cs[k] == ';') {
                r.push(e + 1 - i, Tok::Number);
                i = e + 1;
                continue;
            }
        }
        r.push(1, Tok::Plain);
        i += 1;
    }
    State::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens of `src` (lines from the normal state), as (text, token)
    /// without the whitespace runs.
    fn toks(tag: &str, src: &str) -> Vec<(String, Tok)> {
        let lang = lang_of(tag).expect("a known language");
        let mut st = State::Normal;
        let mut out = Vec::new();
        for l in src.split('\n') {
            let (runs, next) = line(lang, st, l);
            assert_eq!(runs.iter().map(|r| r.0).sum::<usize>(), l.chars().count(), "runs cover {l:?}");
            let cs: Vec<char> = l.chars().collect();
            let mut i = 0;
            for (n, t) in runs {
                let s: String = cs[i..i + n].iter().collect();
                i += n;
                if !s.trim().is_empty() {
                    out.push((s.trim().to_string(), t));
                }
            }
            st = next;
        }
        out
    }

    fn tok_of(tag: &str, src: &str, word: &str) -> Tok {
        toks(tag, src)
            .into_iter()
            .find(|(s, _)| s == word)
            .unwrap_or_else(|| panic!("{word:?} in {:?}", toks(tag, src)))
            .1
    }

    #[test]
    fn fence_tags_pick_a_language_or_none() {
        assert_eq!(lang_of("ts").unwrap().names[0], "ts");
        assert_eq!(lang_of("TypeScript").unwrap().names[0], "ts");
        assert_eq!(lang_of("rust title=x").unwrap().names[0], "rust");
        assert_eq!(lang_of("sh").unwrap().names[0], "bash");
        assert!(lang_of("").is_none());
        assert!(lang_of("brainfuck").is_none());
    }

    #[test]
    fn typescript_spans() {
        let src = "const x: Foo = await load(\"a\", 42); // hi";
        assert_eq!(tok_of("ts", src, "const"), Tok::Keyword);
        assert_eq!(tok_of("ts", src, "Foo"), Tok::Type);
        assert_eq!(tok_of("ts", src, "load"), Tok::Call);
        assert_eq!(tok_of("ts", src, "\"a\""), Tok::String);
        assert_eq!(tok_of("ts", src, "42"), Tok::Number);
        assert_eq!(tok_of("ts", src, "// hi"), Tok::Comment);
        assert_eq!(tok_of("ts", src, "x"), Tok::Plain);
    }

    #[test]
    fn states_carry_across_lines() {
        // a block comment and a template literal span lines
        let t = toks("ts", "/* a\nb */ let y = `one\ntwo` + 1");
        assert_eq!(t[0], ("/* a".into(), Tok::Comment));
        assert_eq!(t[1], ("b */".into(), Tok::Comment));
        assert_eq!(tok_of("ts", "/* a\nb */ let y = `one\ntwo` + 1", "let"), Tok::Keyword);
        assert!(t.contains(&("two`".into(), Tok::String)));
        // python triple quotes
        let p = toks("py", "s = \"\"\"doc\nstill\"\"\"\ndef f(): pass");
        assert!(p.contains(&("still\"\"\"".into(), Tok::String)));
        assert_eq!(tok_of("py", "s = \"\"\"doc\nstill\"\"\"\ndef f(): pass", "def"), Tok::Keyword);
        // an unclosed "..." in TypeScript ends at the line end
        assert_eq!(tok_of("ts", "\"open\nlet", "let"), Tok::Keyword);
    }

    #[test]
    fn rust_specifics() {
        let src = "fn f<'a>(s: &'a str) -> u8 { println!(\"{}\", 'x'); r#\"raw\"# }";
        assert_eq!(tok_of("rust", src, "fn"), Tok::Keyword);
        assert_eq!(tok_of("rust", src, "'a"), Tok::Type);
        assert_eq!(tok_of("rust", src, "u8"), Tok::Type);
        assert_eq!(tok_of("rust", src, "println!"), Tok::Call);
        assert_eq!(tok_of("rust", src, "'x'"), Tok::String);
        assert_eq!(tok_of("rust", src, "r#\"raw\"#"), Tok::String);
        // nested block comments
        let t = toks("rust", "/* a /* b */ c */ let");
        assert_eq!(t[0], ("/* a /* b */ c */".into(), Tok::Comment));
    }

    #[test]
    fn bash_json_yaml_toml_css_html_diff() {
        let sh = "FOO=1 grep -n \"$HOME\" x | wc -l # count";
        assert_eq!(tok_of("bash", sh, "FOO"), Tok::Number);
        assert_eq!(tok_of("bash", sh, "grep"), Tok::Call);
        assert_eq!(tok_of("bash", sh, "-n"), Tok::Type);
        assert_eq!(tok_of("bash", sh, "wc"), Tok::Call);
        assert_eq!(tok_of("bash", sh, "# count"), Tok::Comment);
        let js = "{\"name\": \"bise\", \"n\": 3, \"ok\": true}";
        assert_eq!(tok_of("json", js, "\"name\""), Tok::Call);
        assert_eq!(tok_of("json", js, "\"bise\""), Tok::String);
        assert_eq!(tok_of("json", js, "3"), Tok::Number);
        assert_eq!(tok_of("json", js, "true"), Tok::Keyword);
        let y = "- name: bise # the app\n  on: true";
        assert_eq!(tok_of("yaml", y, "name"), Tok::Call);
        assert_eq!(tok_of("yaml", y, "bise"), Tok::String);
        assert_eq!(tok_of("yaml", y, "# the app"), Tok::Comment);
        assert_eq!(tok_of("yaml", y, "true"), Tok::Keyword);
        let t = "[package]\nname = \"bise\"\nversion = 1";
        assert_eq!(tok_of("toml", t, "[package]"), Tok::Type);
        assert_eq!(tok_of("toml", t, "name"), Tok::Call);
        assert_eq!(tok_of("toml", t, "1"), Tok::Number);
        let c = ".card {\n  color: #fff;\n}";
        assert_eq!(tok_of("css", c, ".card"), Tok::Type);
        assert_eq!(tok_of("css", c, "color"), Tok::Call);
        assert_eq!(tok_of("css", c, "#fff"), Tok::Number);
        let h = "<a href=\"x\">hi</a> <!-- c -->";
        assert_eq!(tok_of("html", h, "a"), Tok::Keyword);
        assert_eq!(tok_of("html", h, "href"), Tok::Type);
        assert_eq!(tok_of("html", h, "\"x\""), Tok::String);
        assert_eq!(tok_of("html", h, "<!-- c -->"), Tok::Comment);
        assert_eq!(toks("diff", "+a\n-b\n c")[0].1, Tok::Added);
        assert_eq!(toks("diff", "+a\n-b\n c")[1].1, Tok::Removed);
    }

    #[test]
    fn sql_keywords_in_any_case() {
        assert_eq!(tok_of("sql", "SELECT id FROM t -- x", "SELECT"), Tok::Keyword);
        assert_eq!(tok_of("sql", "select id from t", "from"), Tok::Keyword);
    }
}
