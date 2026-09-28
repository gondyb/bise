//! The `$` popup of the composer: pick a skill by name, as in Codex.
//!
//! The list is the skills index the Bend REPL writes at startup
//! (runtime/skills.bend: `name<TAB>description<TAB>path` per line, at
//! `$BEND_SKILLS_INDEX`, else `~/.bend-harness/skills-index.txt`). The
//! file is shared by every REPL, so it is close to, not exactly, what the
//! agent in focus sees. Picking inserts `$name ` in the text; the model
//! reads the mention (nothing loads the skill on the client side).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Skill {
    pub(crate) name: String,
    pub(crate) desc: String,
}

/// The index lines, first occurrence of a name kept (the scan visits
/// several skill folders), in file order.
pub(crate) fn parse_index(text: &str) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    for line in text.lines() {
        let mut f = line.split('\t');
        let (Some(name), Some(desc)) = (f.next(), f.next()) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        if out.iter().any(|s| s.name == name) {
            continue;
        }
        out.push(Skill {
            name: name.to_string(),
            desc: short(desc.trim()),
        });
    }
    out
}

/// The first sentence of a description.
fn short(desc: &str) -> String {
    match desc.find(". ") {
        Some(i) => desc[..=i].to_string(),
        None => desc.to_string(),
    }
}

fn index_path() -> Option<PathBuf> {
    match std::env::var("BEND_SKILLS_INDEX") {
        Ok(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => std::env::var("HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(".bend-harness/skills-index.txt")),
    }
}

type Cache = Option<(PathBuf, Option<SystemTime>, Vec<Skill>)>;
static CACHE: Mutex<Cache> = Mutex::new(None);

/// The skills of the index, re-read when the file changes.
pub(crate) fn index() -> Vec<Skill> {
    let Some(path) = index_path() else {
        return Vec::new();
    };
    let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, t, list)) = cache.as_ref() {
        if *p == path && *t == mtime {
            return list.clone();
        }
    }
    let list = std::fs::read_to_string(&path)
        .map(|t| parse_index(&t))
        .unwrap_or_default();
    *cache = Some((path, mtime, list.clone()));
    list
}

/// The `$word` that ends at the cursor: its start (char index of the
/// `$`) and the name typed so far. The `$` opens a word (line start or
/// after a space) and the word has no space yet.
pub(crate) fn token(input: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = chars[..cursor].iter().rposition(|c| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    // the cursor on or before the `$`: no token (and no slice past it)
    match chars[..cursor].get(start..) {
        Some(['$', rest @ ..]) => Some((start, rest.iter().collect())),
        _ => None,
    }
}

/// Case-insensitive prefix matches first, then substring matches.
pub(crate) fn filter<'a>(skills: &'a [Skill], query: &str) -> Vec<&'a Skill> {
    prefix_first(skills, query, |s| &s.name)
}

/// The items whose name matches `query`, case-insensitive: prefix
/// matches first, then substring matches, each group in input order
/// (the `$` skill popup, the `@` agent popup).
pub(crate) fn prefix_first<'a, T>(
    items: impl IntoIterator<Item = &'a T>,
    query: &str,
    name: impl Fn(&T) -> &str,
) -> Vec<&'a T> {
    let q = query.to_lowercase();
    // (is a prefix match, item) of every match
    let hits: Vec<(bool, &T)> = items
        .into_iter()
        .filter_map(|x| {
            let n = name(x).to_lowercase();
            n.contains(&q).then(|| (n.starts_with(&q), x))
        })
        .collect();
    let group = |prefix: bool| hits.iter().filter(move |h| h.0 == prefix).map(|h| h.1);
    group(true).chain(group(false)).collect()
}

/// The composer once `name` is picked for the token at `start..cursor`:
/// the new text and the new cursor (after `$name `).
pub(crate) fn complete(input: &str, start: usize, cursor: usize, name: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let cursor = cursor.min(chars.len());
    let head: String = chars[..start].iter().collect();
    let ins = format!("${} ", name);
    let mut tail: String = chars[cursor..].iter().collect();
    if tail.starts_with(' ') {
        tail.remove(0);
    }
    let at = start + ins.chars().count();
    (format!("{}{}{}", head, ins, tail), at)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "bend\tUse for Bend code. Loads the guide.\t/a/bend/SKILL.md
build-mcp-app\tBuild an MCP app\t/a/b/SKILL.md
grill-me\tInterview the user\t/a/g/SKILL.md
bend\tduplicate from another folder\t/b/bend/SKILL.md
broken line without tabs
mcp-builder\tGuide for MCP servers\t/a/m/SKILL.md
";

    fn names(v: Vec<&Skill>) -> Vec<&str> {
        v.into_iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn parses_the_index_first_name_wins() {
        let s = parse_index(INDEX);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].name, "bend");
        assert_eq!(s[0].desc, "Use for Bend code."); // first sentence
        assert_eq!(s[1].desc, "Build an MCP app");
    }

    #[test]
    fn token_is_the_dollar_word_at_the_cursor() {
        // the cursor on the `$` (← after typing it): none, no panic
        assert_eq!(token("$", 0), None);
        assert_eq!(token("run $be", 4), None);
        assert_eq!(token("$", 1), Some((0, String::new())));
        assert_eq!(token("$be", 3), Some((0, "be".into())));
        assert_eq!(token("use $gr", 7), Some((4, "gr".into())));
        assert_eq!(token("use $gr please", 7), Some((4, "gr".into())));
        assert_eq!(token("use $gr please", 14), None); // cursor after the word
        assert_eq!(token("cost 5$", 7), None); // not a word start
        assert_eq!(token("$bend ", 6), None); // done: a space follows
        assert_eq!(token("plain", 5), None);
    }

    #[test]
    fn filters_prefix_first() {
        let s = parse_index(INDEX);
        assert_eq!(names(filter(&s, "")).len(), 4);
        assert_eq!(names(filter(&s, "B")), ["bend", "build-mcp-app", "mcp-builder"]);
        assert_eq!(names(filter(&s, "mcp")), ["mcp-builder", "build-mcp-app"]);
        assert!(filter(&s, "zzz").is_empty());
    }

    #[test]
    fn completion_replaces_the_token() {
        assert_eq!(complete("$be", 0, 3, "bend"), ("$bend ".into(), 6));
        assert_eq!(complete("use $gr", 4, 7, "grill-me"), ("use $grill-me ".into(), 14));
        assert_eq!(
            complete("use $gr please", 4, 7, "grill-me"),
            ("use $grill-me please".into(), 14)
        );
        // the popup closes once picked
        let (t, c) = complete("$be", 0, 3, "bend");
        assert_eq!(token(&t, c), None);
    }
}
