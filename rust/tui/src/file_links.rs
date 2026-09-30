//! File links (BISE-264): a local path in the history (`rust/tui/src/links.rs`,
//! `./notes.md:12`, `/abs/file.rs:3:7`) is a link like a url (links.rs):
//! underlined, tagged, in OSC 8 as `file://…`, and a plain click opens it
//! in your editor, at the line when the text gives one.
//!
//! What is a path: a markdown link to a file (`[the guide](docs/guide.md)`),
//! an inline code span that is a path, a bare path in prose and in a
//! done tool row. Only a file that exists: a relative path resolves
//! against the folders of the feed in view ([`set_dirs`]: the agent's
//! worktree, then the workspace), then the TUI's cwd. The stat is
//! cached a few seconds: a row build stays cheap.
//!
//! The editor: `BISE_EDITOR`, then `$VISUAL`, then `$EDITOR`, else the
//! file's default app (`open`, `xdg-open`, or `BISE_OPEN`). A GUI editor
//! (code, cursor, zed, subl, idea…) runs detached with its own line
//! syntax; a terminal editor (vim, nvim, hx, nano, emacs…) runs in the
//! terminal panel (term.rs), never by suspending the TUI.

use ratatui::style::Style;
use ratatui::text::Span;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A file to open, at a line (and column) when given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) path: PathBuf,
    pub(crate) line: Option<u32>,
    pub(crate) col: Option<u32>,
}

// ---- where a relative path resolves ----

thread_local! {
    static DIRS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static STATS: RefCell<HashMap<PathBuf, (bool, Instant)>> = RefCell::new(HashMap::new());
}

/// How long a stat is trusted (a file the agent writes shows as a link
/// at the next row build after this).
const STAT_TTL: Duration = Duration::from_secs(5);

/// The folders a relative path of the feed in view resolves against, in
/// order (the TUI's cwd comes after them).
pub(crate) fn set_dirs(dirs: Vec<PathBuf>) {
    DIRS.with(|d| *d.borrow_mut() = dirs);
}

fn dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = DIRS.with(|d| d.borrow().clone());
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd);
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|d| !d.as_os_str().is_empty() && seen.insert(d.clone()));
    out
}

/// `p` is a regular file (cached [`STAT_TTL`]).
fn is_file(p: &Path) -> bool {
    let now = Instant::now();
    if let Some(hit) = STATS.with(|s| s.borrow().get(p).filter(|(_, at)| now.duration_since(*at) < STAT_TTL).map(|h| h.0)) {
        return hit;
    }
    let yes = p.is_file();
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() > 4096 {
            s.clear();
        }
        s.insert(p.to_path_buf(), (yes, now));
    });
    yes
}

#[cfg(test)]
pub(crate) fn forget_stats() {
    STATS.with(|s| s.borrow_mut().clear());
}

// ---- what is a path ----

fn num(s: &str) -> Option<u32> {
    (!s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten().filter(|n| *n > 0)
}

/// `path:12`, `path:12:3`, `path#L12`, `path#L12C3` (`#L12-L20`: 12):
/// the path and the line and column.
pub(crate) fn split_line(s: &str) -> (&str, Option<u32>, Option<u32>) {
    if let Some((p, frag)) = s.rsplit_once("#L") {
        let frag = frag.split('-').next().unwrap_or("");
        let (l, c) = frag.split_once('C').map_or((frag, None), |(l, c)| (l, Some(c)));
        if let Some(l) = num(l) {
            return (p, Some(l), c.and_then(num));
        }
    }
    if let Some((a, last)) = s.rsplit_once(':') {
        if let Some(n) = num(last) {
            if let Some((p, mid)) = a.rsplit_once(':') {
                if let Some(l) = num(mid) {
                    return (p, Some(l), Some(n));
                }
            }
            return (a, Some(n), None);
        }
    }
    (s, None, None)
}

/// Looks like a path, before any stat: no blank, not a url, a letter in
/// it, and a `/` or a `name.ext`.
fn path_like(p: &str) -> bool {
    if p.is_empty() || p.len() > 1024 || p.contains("://") || p.starts_with("//") {
        return false;
    }
    if p.chars().any(|c| c.is_whitespace() || c.is_control()) || !p.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    if p.contains('/') {
        return true;
    }
    match p.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && (1..=10).contains(&ext.len())
                && ext.starts_with(|c: char| c.is_ascii_alphabetic())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        }
        None => false,
    }
}

/// The existing file `p` names: `~/` is your home, a relative path the
/// first folder of [`set_dirs`] (then the cwd) that has it.
fn resolve(p: &str) -> Option<PathBuf> {
    if let Some(rest) = p.strip_prefix("~/") {
        let home = std::env::var_os("HOME")?;
        let f = PathBuf::from(home).join(rest);
        return is_file(&f).then_some(f);
    }
    let path = Path::new(p);
    if path.is_absolute() {
        return is_file(path).then(|| path.to_path_buf());
    }
    dirs().into_iter().map(|d| d.join(path)).find(|f| is_file(f))
}

/// The file `s` names (`path`, `path:12`, `path:12:3`, `path#L12`), if
/// it exists.
pub(crate) fn target(s: &str) -> Option<Target> {
    let (p, line, col) = split_line(s);
    if !path_like(p) {
        return None;
    }
    resolve(p).map(|path| Target { path, line, col })
}

/// A char that may start a bare path.
fn starts_path(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '/' | '~' | '_')
}

/// A char that ends a bare path.
fn ends_path(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | ',' | ';' | '*')
}

/// A bare path to an existing file at `cs[i]` (not inside a word): its
/// length in chars and its url. Trailing punctuation stays out.
pub(crate) fn bare_at(cs: &[char], i: usize) -> Option<(usize, String)> {
    let c = *cs.get(i)?;
    if !starts_path(c) {
        return None;
    }
    if i > 0 && (cs[i - 1].is_alphanumeric() || matches!(cs[i - 1], '/' | '.' | '_' | '-' | '~' | '@' | ':' | '%' | '+' | '#' | '=' | '\\')) {
        return None;
    }
    let mut end = i;
    while end < cs.len() && !ends_path(cs[end]) {
        end += 1;
    }
    while end > i && matches!(cs[end - 1], '.' | ':' | '!' | '?') {
        end -= 1;
    }
    if end == i {
        return None;
    }
    let tok: String = cs[i..end].iter().collect();
    target(&tok).map(|t| (end - i, url_of(&t)))
}

/// `text` as spans in `st`, each bare path to a file a link (a tool row).
pub(crate) fn plain_spans(text: &str, st: Style) -> Vec<Span<'static>> {
    let cs: Vec<char> = text.chars().collect();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < cs.len() {
        if let Some((n, url)) = bare_at(&cs, i) {
            if !plain.is_empty() {
                out.push(Span::styled(std::mem::take(&mut plain), st));
            }
            let tag = crate::links::add(&url);
            let fg = st.fg.unwrap_or(crate::theme::dim());
            out.push(Span::styled(cs[i..i + n].iter().collect::<String>(), crate::links::link_style(st, fg, tag)));
            i += n;
            continue;
        }
        plain.push(cs[i]);
        i += 1;
    }
    if !plain.is_empty() {
        out.push(Span::styled(plain, st));
    }
    out
}

// ---- the url of a file ----

/// `file:///abs/path`, then `#L12` or `#L12C3`: `%`, `#`, `?` and blanks
/// percent-encoded.
pub(crate) fn url_of(t: &Target) -> String {
    let mut u = String::from("file://");
    for ch in t.path.to_string_lossy().chars() {
        match ch {
            '%' | '#' | '?' => u.push_str(&format!("%{:02X}", ch as u32)),
            c if c.is_whitespace() || c.is_control() => {
                let mut b = [0u8; 4];
                for x in c.encode_utf8(&mut b).bytes() {
                    u.push_str(&format!("%{:02X}", x));
                }
            }
            c => u.push(c),
        }
    }
    match (t.line, t.col) {
        (Some(l), Some(c)) => u.push_str(&format!("#L{}C{}", l, c)),
        (Some(l), None) => u.push_str(&format!("#L{}", l)),
        _ => {}
    }
    u
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(x) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(x);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The file of a `file://` url (any host part dropped), its `#L12`
/// line kept.
pub(crate) fn target_of_url(url: &str) -> Option<Target> {
    if url.len() < 7 || !url[..7].eq_ignore_ascii_case("file://") {
        return None;
    }
    let rest = &url[7..];
    let rest = &rest[rest.find('/')?..];
    let (p, frag) = rest.split_once('#').unwrap_or((rest, ""));
    let (line, col) = match frag.strip_prefix('L') {
        Some(f) => {
            let f = f.split('-').next().unwrap_or("");
            let (l, c) = f.split_once('C').map_or((f, None), |(l, c)| (l, Some(c)));
            (num(l), c.and_then(num))
        }
        None => (None, None),
    };
    Some(Target { path: PathBuf::from(percent_decode(p)), line, col })
}

/// The url without its `#L12` (OSC 8: a terminal opens the file itself).
pub(crate) fn without_line(url: &str) -> &str {
    if target_of_url(url).is_some() {
        url.split('#').next().unwrap_or(url)
    } else {
        url
    }
}

/// A copy keeps a file link's label alone when it names the file.
pub(crate) fn label_names_file(label: &str, url: &str) -> bool {
    target_of_url(url)
        .and_then(|t| t.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .is_some_and(|n| label.contains(&n))
}

// ---- the editor ----

/// How a click opens a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Launch {
    /// a GUI editor, detached: the program and its arguments
    Gui(Vec<String>),
    /// a terminal editor, in the terminal panel
    Term(Vec<String>),
    /// no editor set: the file's default app (no line)
    Default(PathBuf),
}

/// The editor command: the first non-blank of `BISE_EDITOR`, `VISUAL`,
/// `EDITOR`.
pub(crate) fn editor_choice(bise: Option<&str>, visual: Option<&str>, editor: Option<&str>) -> Option<String> {
    [bise, visual, editor].into_iter().flatten().map(str::trim).find(|s| !s.is_empty()).map(str::to_string)
}

/// `s` split into words like a shell would (quotes and backslashes; no
/// expansion).
pub(crate) fn shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut any = false;
    let mut quote: Option<char> = None;
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => {
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                any = true;
            }
            (None, '\\') => {
                if let Some(n) = it.next() {
                    cur.push(n);
                    any = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The command that opens `t` with `editor` (a command line, `$EDITOR`
/// style: `code --wait`, `emacs -nw`), at its line in that editor's
/// syntax. An editor bise does not know runs in the terminal panel,
/// given the file alone.
pub(crate) fn launch(editor: Option<&str>, t: &Target) -> Launch {
    let words = editor.map(shell_words).unwrap_or_default();
    let Some(prog) = words.first() else { return Launch::Default(t.path.clone()) };
    let name = Path::new(prog).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name = name.trim_end_matches(".exe");
    let file = t.path.to_string_lossy().into_owned();
    let at = |sep: &str| match (t.line, t.col) {
        (Some(l), Some(c)) => format!("{}:{}{}{}", file, l, sep, c),
        (Some(l), None) => format!("{}:{}", file, l),
        _ => file.clone(),
    };
    let with = |extra: Vec<String>| words.iter().cloned().chain(extra).collect::<Vec<String>>();
    let plus = |col_sep: Option<&str>| -> Vec<String> {
        match (t.line, t.col, col_sep) {
            (Some(l), Some(c), Some(s)) => vec![format!("+{}{}{}", l, s, c), file.clone()],
            (Some(l), _, _) => vec![format!("+{}", l), file.clone()],
            _ => vec![file.clone()],
        }
    };
    match name {
        "code" | "code-insiders" | "cursor" | "windsurf" | "codium" | "vscodium" => match t.line {
            Some(_) => Launch::Gui(with(vec!["-g".into(), at(":")])),
            None => Launch::Gui(with(vec![file.clone()])),
        },
        "zed" | "zeditor" | "subl" | "sublime_text" => Launch::Gui(with(vec![at(":")])),
        "idea" | "webstorm" | "pycharm" | "goland" | "clion" | "rubymine" | "phpstorm" | "rustrover" | "rider"
        | "datagrip" | "studio" | "fleet" => {
            let mut extra = Vec::new();
            if let Some(l) = t.line {
                extra.extend(["--line".to_string(), l.to_string()]);
                if let Some(c) = t.col {
                    extra.extend(["--column".to_string(), c.to_string()]);
                }
            }
            extra.push(file.clone());
            Launch::Gui(with(extra))
        }
        "mate" => match t.line {
            Some(l) => Launch::Gui(with(vec!["-l".into(), l.to_string(), file.clone()])),
            None => Launch::Gui(with(vec![file.clone()])),
        },
        "gvim" | "mvim" => Launch::Gui(with(plus(None))),
        "hx" | "helix" => Launch::Term(with(vec![at(":")])),
        "nano" | "pico" => Launch::Term(with(plus(Some(",")))),
        "emacs" | "emacsclient" | "micro" | "kak" | "mg" => Launch::Term(with(plus(Some(":")))),
        "vi" | "vim" | "nvim" | "view" | "joe" | "ne" | "jed" => Launch::Term(with(plus(None))),
        _ => Launch::Term(with(vec![file.clone()])),
    }
}

/// The editor from the environment (a test sets its own).
fn editor_from_env() -> Option<String> {
    #[cfg(test)]
    {
        EDITOR.with(|e| e.borrow().clone())
    }
    #[cfg(not(test))]
    {
        let v = |k: &str| std::env::var(k).ok();
        editor_choice(v("BISE_EDITOR").as_deref(), v("VISUAL").as_deref(), v("EDITOR").as_deref())
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static EDITOR: RefCell<Option<String>> = const { RefCell::new(None) };
    /// what a test's click launched (never a real editor)
    pub(crate) static LAUNCHED: RefCell<Vec<Launch>> = const { RefCell::new(Vec::new()) };
}

/// The file with its line, for the status row: `links.rs:12`.
fn short(t: &Target) -> String {
    let n = t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| t.path.to_string_lossy().into_owned());
    match t.line {
        Some(l) => format!("{}:{}", n, l),
        None => n,
    }
}

/// Opens `t` (a click on a file link): the note for the status row.
pub(crate) fn open(app: &mut crate::App, t: &Target) -> String {
    let how = launch(editor_from_env().as_deref(), t);
    #[cfg(test)]
    LAUNCHED.with(|l| l.borrow_mut().push(how.clone()));
    match how {
        Launch::Default(p) => {
            let url = url_of(&Target { path: p, line: None, col: None });
            if crate::links::open(&url) {
                format!("opening {}", short(t))
            } else {
                format!("could not open {}", short(t))
            }
        }
        Launch::Gui(argv) => {
            let prog = Path::new(&argv[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match spawn_detached(&argv) {
                Ok(()) => format!("opening {} in {}", short(t), prog),
                Err(e) => format!("could not start {}: {}", prog, e),
            }
        }
        Launch::Term(argv) => {
            let prog = Path::new(&argv[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            #[cfg(test)]
            let run: Result<(), String> = Ok(());
            #[cfg(not(test))]
            let run = {
                let cwd = crate::term::cwd(app);
                app.term.run(&cwd, &argv)
            };
            let _ = &app;
            match run {
                Ok(()) => format!("{} opens {} in the terminal panel · ctrl+` hides it", prog, short(t)),
                Err(e) => e,
            }
        }
    }
}

/// Starts a GUI editor, no terminal attached; a thread reaps it.
fn spawn_detached(argv: &[String]) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = argv;
        Ok(())
    }
    #[cfg(not(test))]
    {
        let mut child = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
