//! The workspace file index behind the `@` popup: every file and folder
//! under the working directory that `.gitignore` keeps, walked in a
//! background thread, ranked per keystroke (file name before path).
//! Design: projects/switchboard/docs/at-mentions.md.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Entries past this many are dropped (a home folder, a huge monorepo).
const MAX_ENTRIES: usize = 200_000;
/// An index older than this is walked again when the popup opens.
const STALE: Duration = Duration::from_secs(3);
/// Recent picks kept for the boost.
const RECENT: usize = 32;

/// One file or folder, relative to the root, `/`-separated.
#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) path: String,
    pub(crate) dir: bool,
    lower: Vec<u8>,
    /// byte offset of the name in `path`
    name: usize,
    depth: u16,
    /// bit `b % 64` set for every byte `b` of `lower`
    mask: u64,
}

impl Entry {
    pub(crate) fn new(path: String, dir: bool) -> Entry {
        let lower = path.to_lowercase().into_bytes();
        let name = lower.iter().rposition(|&b| b == b'/').map(|i| i + 1).unwrap_or(0);
        let depth = path.matches('/').count().min(u16::MAX as usize) as u16;
        Entry { mask: mask(&lower), path, dir, lower, name, depth }
    }

    fn lname(&self) -> &[u8] {
        &self.lower[self.name..]
    }
}

fn mask(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |m, &b| m | 1u64 << (b % 64))
}

/// `q` is a subsequence of `hay` (memchr per byte).
fn subseq(mut hay: &[u8], q: &[u8]) -> bool {
    for &c in q {
        match memchr::memchr(c, hay) {
            Some(i) => hay = &hay[i + 1..],
            None => return false,
        }
    }
    true
}

/// Walk `root` as git sees it: `.gitignore`, `.ignore`, `.git/info/exclude`
/// and the global excludes apply (only inside a git repo, like git);
/// dot files are kept, `.git` is not. Files and folders, sorted.
pub(crate) fn walk(root: &Path, cap: usize) -> Vec<Entry> {
    let out = Mutex::new(Vec::new());
    ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(true)
        .filter_entry(|e| e.file_name() != ".git")
        .threads(4)
        .build_parallel()
        .run(|| {
            let out = &out;
            Box::new(move |e| {
                let Ok(e) = e else { return ignore::WalkState::Continue };
                let Some(rel) = e.path().strip_prefix(root).ok().and_then(|p| p.to_str()) else {
                    return ignore::WalkState::Continue;
                };
                if rel.is_empty() {
                    return ignore::WalkState::Continue;
                }
                let dir = e.file_type().is_some_and(|t| t.is_dir());
                let mut v = out.lock().unwrap_or_else(|e| e.into_inner());
                if v.len() >= cap {
                    return ignore::WalkState::Quit;
                }
                v.push((rel.replace('\\', "/"), dir));
                ignore::WalkState::Continue
            })
        });
    let mut v = out.into_inner().unwrap_or_else(|e| e.into_inner());
    v.sort();
    v.into_iter().map(|(p, d)| Entry::new(p, d)).collect()
}

/// The entries matching `query`, best first, at most `limit`.
///
/// The query splits at its last `/` into a folder part (a subsequence
/// of the parent path) and a name part. Tiers: exact name, name prefix,
/// name fuzzy, path fuzzy (no folder part only). In a tier: recent
/// picks first, then the fuzzy score, then shallower, shorter, path.
/// Empty name part (`""`, `src/`): the children of that folder only,
/// folders first. Dot files only when the name part starts with `.`.
pub(crate) fn rank(entries: &[Entry], query: &str, recent: &[String], limit: usize) -> Vec<usize> {
    let q = query.to_lowercase();
    let q = q.as_bytes();
    let (qdir, qname) = match q.iter().rposition(|&b| b == b'/') {
        Some(i) => (&q[..=i], &q[i + 1..]),
        None => (&q[..0], q),
    };
    let dots = qname.first() == Some(&b'.');
    let visible = |e: &Entry| dots || e.lname().first() != Some(&b'.');
    let is_recent = |e: &Entry| recent.contains(&e.path);
    // `rust/tui/` names a folder of the index: the search stays inside it
    let scoped = !qdir.is_empty() && is_folder(entries, &qdir[..qdir.len() - 1]);
    if qname.is_empty() {
        return children(entries, qdir, scoped, &visible, &is_recent, limit);
    }
    let qm = mask(q) & !(1u64 << b'/');
    let mut tiers: [Vec<usize>; 4] = Default::default();
    for (i, e) in entries.iter().enumerate() {
        if qm & !e.mask != 0 || !visible(e) {
            continue;
        }
        if scoped && !e.lower.starts_with(qdir) {
            continue;
        }
        let parent = &e.lower[..e.name];
        let dir_ok = qdir.is_empty() || scoped || subseq(parent, qdir);
        let n = e.lname();
        let tier = if dir_ok && n == qname {
            0
        } else if dir_ok && n.starts_with(qname) {
            1
        } else if dir_ok && subseq(n, qname) {
            2
        } else if scoped && subseq(&e.lower[qdir.len()..], qname) {
            3
        } else if !scoped && subseq(&e.lower, if qdir.is_empty() { qname } else { q }) {
            // the whole query along the path (`src/tu` finds `src/tui/`)
            3
        } else {
            continue;
        };
        tiers[tier].push(i);
    }
    let name_query = std::str::from_utf8(qname).unwrap_or("");
    let pat = Pattern::new(name_query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut buf = Vec::new();
    let mut out = Vec::new();
    for (t, ids) in tiers.iter().enumerate() {
        let want = limit.saturating_sub(out.len());
        if want == 0 {
            break;
        }
        // (recent, score, -depth, -len): bigger is better
        let mut v: Vec<((bool, u32, i32, i32), usize)> = ids
            .iter()
            .map(|&i| {
                let e = &entries[i];
                let score = match t {
                    2 => pat.score(Utf32Str::new(&e.path[e.name..], &mut buf), &mut matcher),
                    3 => pat.score(Utf32Str::new(&e.path, &mut buf), &mut matcher),
                    _ => Some(0),
                };
                ((is_recent(e), score.unwrap_or(0), -(e.depth as i32), -(e.path.len() as i32)), i)
            })
            .collect();
        let cmp = |a: &((bool, u32, i32, i32), usize), b: &((bool, u32, i32, i32), usize)| {
            b.0.cmp(&a.0).then(a.1.cmp(&b.1))
        };
        if v.len() > want {
            v.select_nth_unstable_by(want - 1, cmp);
            v.truncate(want);
        }
        v.sort_by(cmp);
        out.extend(v.into_iter().map(|(_, i)| i));
    }
    out
}

/// `lower` (lowercase, no trailing `/`) is a folder of the index.
fn is_folder(entries: &[Entry], lower: &[u8]) -> bool {
    entries.iter().any(|e| e.dir && e.lower == lower)
}

/// The children of folder `dir` (`""`: the root; a folder query like
/// `src/` also matches `rust/tui/src/` unless `scoped`: `dir` is a folder
/// of the index, only its own children), recent then folders first.
fn children(
    entries: &[Entry],
    dir: &[u8],
    scoped: bool,
    visible: &dyn Fn(&Entry) -> bool,
    is_recent: &dyn Fn(&Entry) -> bool,
    limit: usize,
) -> Vec<usize> {
    fn parent(e: &Entry) -> &[u8] {
        &e.lower[..e.name]
    }
    let mut v: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| visible(e))
        .filter(|(_, e)| {
            let p = parent(e);
            if dir.is_empty() || scoped {
                p == dir
            } else {
                p.ends_with(dir) && (p.len() == dir.len() || p[p.len() - dir.len() - 1] == b'/')
            }
        })
        .map(|(i, _)| i)
        .collect();
    // exact folder path first, then the shallowest folders named so
    v.sort_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        (!is_recent(x), parent(x).len() != dir.len(), x.depth, !x.dir, &x.lower)
            .cmp(&(!is_recent(y), parent(y).len() != dir.len(), y.depth, !y.dir, &y.lower))
    });
    v.truncate(limit);
    v
}

/// The `@word` that ends at the cursor: its start (char index of the
/// `@`) and the query typed so far. The `@` opens a word (line start or
/// after a space) and the word has no space yet, or it is an open quote
/// (`@"docs/my notes/`, a folder with a space) not closed yet. None when
/// the cursor is on or before the `@`.
pub(crate) fn token(input: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let before = &chars[..cursor.min(chars.len())];
    if let Some(q) = before.iter().rposition(|&c| c == '"') {
        let opens = q >= 1 && before[q - 1] == '@' && (q == 1 || before[q - 2].is_whitespace());
        if opens && !before[q + 1..].contains(&'\n') {
            return Some((q - 1, before[q + 1..].iter().collect()));
        }
    }
    let start = before
        .iter()
        .rposition(|c| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    match before.get(start..) {
        Some(['@', rest @ ..]) => Some((start, rest.iter().collect())),
        _ => None,
    }
}

/// The token text that browses folder `path`: `@path/`, `@"path/` when
/// the path holds a space (the popup lists the folder's entries).
pub(crate) fn browse(path: &str) -> String {
    if path.is_empty() {
        "@".to_string()
    } else if path.contains(char::is_whitespace) {
        format!("@\"{path}/")
    } else {
        format!("@{path}/")
    }
}

/// One folder up from a query that browses a folder (`rust/tui/` →
/// `rust`, `rust/` → the root `""`); None when the query does not end
/// with `/`.
pub(crate) fn parent_query(query: &str) -> Option<&str> {
    let q = query.strip_suffix('/')?;
    Some(q.rfind('/').map_or("", |i| &q[..i]))
}

/// What a picked entry inserts: the relative path (a folder with a
/// trailing `/`), quoted when it holds a space.
pub(crate) fn reference(path: &str, dir: bool) -> String {
    let p = if dir { format!("{path}/") } else { path.to_string() };
    if p.contains(char::is_whitespace) && !p.contains('"') {
        format!("\"{p}\"")
    } else {
        p
    }
}

/// The composer once `ins` replaces the token at `start..cursor`: the
/// new text and the cursor (after `ins` and one space).
pub(crate) fn complete(input: &str, start: usize, cursor: usize, ins: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars[..start].iter().collect();
    let mut tail: String = chars[cursor..].iter().collect();
    if tail.starts_with(' ') {
        tail.remove(0);
    }
    let at = start + ins.chars().count() + 1;
    (format!("{head}{ins} {tail}"), at)
}

/// The composer once `tok` replaces the token at `start..cursor`, the
/// popup still open: the new text and the cursor right after `tok`.
pub(crate) fn replace_token(input: &str, start: usize, cursor: usize, tok: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars[..start].iter().collect();
    let tail: String = chars[cursor..].iter().collect();
    (format!("{head}{tok}{tail}"), start + tok.chars().count())
}

/// The shared index of the process: the working directory walked in the
/// background, re-walked when stale, plus the recent picks.
struct State {
    root: PathBuf,
    entries: Arc<Vec<Entry>>,
    built: Option<Instant>,
    walking: bool,
    recent: Vec<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start indexing `root` in the background (the TUI calls it at startup).
pub(crate) fn start(root: PathBuf) {
    *state() = Some(State {
        root,
        entries: Arc::new(Vec::new()),
        built: None,
        walking: false,
        recent: Vec::new(),
    });
    refresh();
}

/// Walk again in the background when the index is stale and no walk runs.
pub(crate) fn refresh() {
    let root = {
        let mut g = state();
        let Some(s) = g.as_mut() else { return };
        if s.walking || s.built.is_some_and(|t| t.elapsed() < STALE) {
            return;
        }
        s.walking = true;
        s.root.clone()
    };
    let _ = std::thread::Builder::new().name("file-index".into()).spawn(move || {
        let entries = Arc::new(walk(&root, MAX_ENTRIES));
        if let Some(s) = state().as_mut().filter(|s| s.root == root) {
            s.entries = entries;
            s.built = Some(Instant::now());
            s.walking = false;
        }
    });
}

/// A search hit: the relative path and whether it is a folder.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) path: String,
    pub(crate) dir: bool,
}

/// The best `limit` entries of `root` for `query` (refreshes a stale
/// index in the background; this call never waits for a walk). A new
/// root starts a new index: its popup shows the files a frame later.
pub(crate) fn search(root: &Path, query: &str, limit: usize) -> Vec<Hit> {
    if state().as_ref().is_none_or(|s| s.root != root) {
        start(root.to_path_buf());
    }
    refresh();
    let (entries, recent) = match state().as_ref() {
        Some(s) => (s.entries.clone(), s.recent.clone()),
        None => return Vec::new(),
    };
    rank(&entries, query, &recent, limit)
        .into_iter()
        .map(|i| Hit { path: entries[i].path.clone(), dir: entries[i].dir })
        .collect()
}

/// Remember a picked path (boosted in the next searches).
pub(crate) fn picked(path: &str) {
    if let Some(s) = state().as_mut() {
        s.recent.retain(|p| p != path);
        s.recent.insert(0, path.to_string());
        s.recent.truncate(RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(paths: &[&str]) -> Vec<Entry> {
        paths
            .iter()
            .map(|p| match p.strip_suffix('/') {
                Some(d) => Entry::new(d.to_string(), true),
                None => Entry::new(p.to_string(), false),
            })
            .collect()
    }

    fn top<'a>(e: &'a [Entry], q: &str, recent: &[&str]) -> Vec<&'a str> {
        let recent: Vec<String> = recent.iter().map(|s| s.to_string()).collect();
        rank(e, q, &recent, 10).into_iter().map(|i| e[i].path.as_str()).collect()
    }

    const TREE: &[&str] = &[
        "Makefile",
        "README.md",
        ".gitignore",
        "rust/",
        "rust/tui/",
        "rust/tui/src/",
        "rust/tui/src/app.rs",
        "rust/tui/src/sb/",
        "rust/tui/src/sb/mention.rs",
        "rust/tui/src/main.rs",
        "hub/main.bend",
        "docs/main-notes/readme.txt",
        "projects/switchboard/docs/at-mentions.md",
    ];

    #[test]
    fn file_name_beats_path() {
        let e = index(TREE);
        // exact name, then name prefix, then the path-only match
        assert_eq!(top(&e, "main.rs", &[]), ["rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "main", &[])[..2], ["hub/main.bend", "rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "mention", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
        // "sbmen" only matches along a path: the tighter match first
        assert_eq!(top(&e, "sbmen", &[])[0], "rust/tui/src/sb/mention.rs");
        // case-insensitive; same tier: the shallower first
        assert_eq!(top(&e, "readme", &[])[..2], ["README.md", "docs/main-notes/readme.txt"]);
        assert!(top(&e, "zzz", &[]).is_empty());
    }

    #[test]
    fn exact_beats_prefix_beats_fuzzy() {
        let e = index(&["a/src.rs", "b/src/", "c/sourcery.rs", "d/xsrc.rs"]);
        assert_eq!(top(&e, "src", &[]), ["b/src", "a/src.rs", "c/sourcery.rs", "d/xsrc.rs"]);
    }

    #[test]
    fn folder_part_filters_the_parent_path() {
        let e = index(TREE);
        // the folder part is fuzzy too ("sb/" in "projects/switchboard/docs/")
        assert_eq!(top(&e, "sb/me", &[]), ["rust/tui/src/sb/mention.rs", "projects/switchboard/docs/at-mentions.md"]);
        assert_eq!(top(&e, "docs/me", &[])[0], "projects/switchboard/docs/at-mentions.md");
        assert_eq!(top(&e, "tui/src/m", &[]), ["rust/tui/src/main.rs", "rust/tui/src/sb/mention.rs"]);
        // folder then empty name: the children of that folder, folders first
        assert_eq!(top(&e, "rust/tui/src/", &[]), ["rust/tui/src/sb", "rust/tui/src/app.rs", "rust/tui/src/main.rs"]);
        assert_eq!(top(&e, "src/", &[]), ["rust/tui/src/sb", "rust/tui/src/app.rs", "rust/tui/src/main.rs"]);
    }

    #[test]
    fn empty_query_lists_the_root_dot_files_hidden() {
        let e = index(TREE);
        assert_eq!(top(&e, "", &[]), ["rust", "Makefile", "README.md"]);
        assert_eq!(top(&e, ".git", &[]), [".gitignore"]);
        assert!(!top(&e, "gitig", &[]).contains(&".gitignore"));
    }

    #[test]
    fn recent_picks_come_first_in_their_tier() {
        let e = index(TREE);
        assert_eq!(top(&e, "main", &["rust/tui/src/main.rs"])[..2], ["rust/tui/src/main.rs", "hub/main.bend"]);
        assert_eq!(top(&e, "", &["README.md"])[0], "README.md");
        // a recent path-only match does not jump over a name match
        assert_eq!(top(&e, "mention", &["projects/switchboard/docs/at-mentions.md"])[0], "rust/tui/src/sb/mention.rs");
    }

    #[test]
    fn walk_respects_gitignore() {
        let root = std::env::temp_dir().join(format!("at-files-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["src", "target/debug", "node_modules/x", ".git"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for (f, body) in [
            (".gitignore", "target/\nnode_modules\n*.log\n"),
            ("src/lib.rs", ""),
            ("src/app.log", ""),
            ("target/debug/bin", ""),
            ("node_modules/x/i.js", ""),
            (".env.example", ""),
            (".git/HEAD", "ref: refs/heads/main\n"),
        ] {
            std::fs::write(root.join(f), body).unwrap();
        }
        let got: Vec<(String, bool)> = walk(&root, 100).into_iter().map(|e| (e.path, e.dir)).collect();
        let _ = std::fs::remove_dir_all(&root);
        let want = [(".env.example", false), (".gitignore", false), ("src", true), ("src/lib.rs", false)];
        assert_eq!(got, want.map(|(p, d)| (p.to_string(), d)));
    }

    #[test]
    fn token_is_the_at_word_at_the_cursor() {
        assert_eq!(token("@", 1), Some((0, String::new())));
        assert_eq!(token("@ma", 3), Some((0, "ma".into())));
        assert_eq!(token("look at @src/ma", 15), Some((8, "src/ma".into())));
        assert_eq!(token("look at @src/ma now", 15), Some((8, "src/ma".into())));
        assert_eq!(token("look at @src/ma now", 19), None); // cursor past the word
        assert_eq!(token("mail a@b.c", 10), None); // mid-word @
        assert_eq!(token("@main ", 6), None); // done: a space follows
        assert_eq!(token("line\n@ap", 8), Some((5, "ap".into())));
    }

    #[test]
    fn token_is_total_and_reads_an_open_quote() {
        // the cursor on the `@` (← after typing it): no token, no panic
        assert_eq!(token("@", 0), None);
        assert_eq!(token("see @ru", 4), None);
        assert_eq!(token("", 5), None);
        // an open quote: a folder with a space being browsed
        assert_eq!(token("@\"docs/my notes/", 16), Some((0, "docs/my notes/".into())));
        assert_eq!(token("see @\"docs/my notes/a", 21), Some((4, "docs/my notes/a".into())));
        // a closed quote ends it; a quote mid-word is not an opener
        assert_eq!(token("@\"docs/my notes/a.md\" ", 22), None);
        assert_eq!(token("say \"@x", 7), None);
    }

    #[test]
    fn browse_and_parent_query() {
        assert_eq!(browse("rust/tui"), "@rust/tui/");
        assert_eq!(browse("docs/my notes"), "@\"docs/my notes/");
        assert_eq!(browse(""), "@");
        assert_eq!(parent_query("rust/tui/"), Some("rust"));
        assert_eq!(parent_query("rust/"), Some(""));
        assert_eq!(parent_query("rust/tu"), None);
        assert_eq!(parent_query(""), None);
        assert_eq!(replace_token("see @ru now", 4, 7, "@rust/"), ("see @rust/ now".into(), 10));
        assert_eq!(replace_token("@", 5, 9, "@x/"), ("@@x/".into(), 4)); // out of range: clamped, no panic
    }

    #[test]
    fn a_folder_path_scopes_the_search() {
        let e = index(&[
            "rust/",
            "rust/tui/",
            "rust/tui/src/",
            "rust/tui/src/files.rs",
            "rust/tui/src/main.rs",
            "rust/tui/Cargo.toml",
            "rust/other/",
            "rust/other/tui/",
            "rust/other/tui/files.rs",
            "vendor/rust/tui/x.rs",
        ]);
        // an exact folder: its own children only (not vendor/rust/tui/)
        assert_eq!(top(&e, "rust/tui/", &[]), ["rust/tui/src", "rust/tui/Cargo.toml"]);
        // a name inside it: its descendants only
        assert_eq!(top(&e, "rust/tui/fi", &[]), ["rust/tui/src/files.rs"]);
        // not a folder path: fuzzy as before, and the whole query along
        // the path finds folders (`src/fi`, `tu/sr`)
        assert_eq!(top(&e, "tui/", &[]), ["rust/tui/src", "rust/tui/Cargo.toml", "rust/other/tui/files.rs", "vendor/rust/tui/x.rs"]);
        assert_eq!(top(&e, "rust/tu", &[])[..2], ["rust/tui", "rust/other/tui"]);
        assert!(top(&e, "tu/sr", &[]).contains(&"rust/tui/src"));
    }

    #[test]
    fn completion_inserts_the_reference() {
        assert_eq!(reference("src/app.rs", false), "src/app.rs");
        assert_eq!(reference("rust/tui", true), "rust/tui/");
        assert_eq!(reference("docs/my notes.md", false), "\"docs/my notes.md\"");
        assert_eq!(complete("see @ap", 4, 7, "src/app.rs"), ("see src/app.rs ".into(), 15));
        assert_eq!(complete("see @ap now", 4, 7, "src/app.rs"), ("see src/app.rs now".into(), 15));
        let (t, c) = complete("@ma", 0, 3, "hub/main.bend");
        assert_eq!((t.as_str(), c), ("hub/main.bend ", 14));
        assert_eq!(token(&t, c), None); // popup closes
    }

    /// Per-keystroke latency on a big repo (release):
    /// `AT_FILES_BENCH=~/mistral/dashboard cargo test --release -p bend-tui files::tests::bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench() {
        let root = std::env::var("AT_FILES_BENCH").unwrap_or_else(|_| ".".into());
        let t = Instant::now();
        let e = walk(Path::new(&root), MAX_ENTRIES);
        println!("walk {root}: {} entries in {:.1?}", e.len(), t.elapsed());
        let mut worst = Duration::ZERO;
        for q in ["", "m", "ma", "mai", "main", "main.rs", "src/", "src/comp", "README", "c", "co", "com", "comp", "compose", "composer", "fidx", "zzzq"] {
            let t = Instant::now();
            for _ in 0..10 {
                rank(&e, q, &[], 50);
            }
            let d = t.elapsed() / 10;
            worst = worst.max(d);
            println!("{:>12} {:>9.2?}", format!("{q:?}"), d);
        }
        println!("worst {worst:.2?}");
    }
}
