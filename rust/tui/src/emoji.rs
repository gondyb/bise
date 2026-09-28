//! Emoji shortcodes in the composer, as in Slack and GitHub.
//!
//! `:cl` (a `:` that opens a word, then at least 2 letters) lists the
//! matching emojis in the composer popup; Tab/Enter inserts the emoji.
//! A known `:name:` typed in full becomes its emoji when the closing `:`
//! is typed. Pastes are never rewritten.
//!
//! The table is gemoji (github/gemoji `db/emoji.json`, MIT license),
//! embedded at build time as `emoji.tsv`: one emoji per line,
//! `emoji<TAB>names<TAB>tags<TAB>description`, names and tags separated
//! by spaces. No network at runtime.

use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct Emoji {
    pub(crate) glyph: &'static str,
    /// the shortcodes (`clap`, `+1`, `thumbsup`...)
    pub(crate) names: Vec<&'static str>,
    /// search words (`applause`, `praise`...), never shortcodes
    tags: Vec<&'static str>,
    pub(crate) desc: &'static str,
}

static DATA: &str = include_str!("emoji.tsv");

fn parse(text: &'static str) -> Vec<Emoji> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let glyph = f.next().filter(|g| !g.is_empty())?;
            let names: Vec<&str> = f.next()?.split(' ').filter(|n| !n.is_empty()).collect();
            let tags = f.next().unwrap_or("").split(' ').filter(|t| !t.is_empty()).collect();
            let desc = f.next().unwrap_or("");
            (!names.is_empty()).then_some(Emoji { glyph, names, tags, desc })
        })
        .collect()
}

pub(crate) fn all() -> &'static [Emoji] {
    static ALL: OnceLock<Vec<Emoji>> = OnceLock::new();
    ALL.get_or_init(|| parse(DATA))
}

/// The emoji of a shortcode (exact name, case-insensitive).
pub(crate) fn get(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    all().iter().find(|e| e.names.contains(&name.as_str())).map(|e| e.glyph)
}

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-')
}

/// Can a shortcode start after this char? Line start, a space, an
/// opening bracket or quote, or a non-ASCII symbol (an emoji: `👏:tada:`).
/// Never after a letter, a digit or `:`/`/`: `http://`, `12:30`, `a:b`,
/// `std::` stay text.
fn opens_after(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some('(' | '[' | '{' | '"') => true,
        Some(c) => !c.is_ascii() && !c.is_alphanumeric(),
    }
}

/// The `:word` that ends at the cursor (no length rule): the char index
/// of its `:` and the word typed so far.
fn raw_token(chars: &[char], cursor: usize) -> Option<(usize, String)> {
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && name_char(chars[start - 1]) {
        start -= 1;
    }
    if start == 0 || chars[start - 1] != ':' {
        return None;
    }
    let colon = start - 1;
    let prev = colon.checked_sub(1).map(|i| chars[i]);
    if !opens_after(prev) {
        return None;
    }
    Some((colon, chars[start..cursor].iter().collect()))
}

/// The `:word` of the popup: it ends at the cursor, opens a word, and has
/// at least 2 letters (`:)`, `:-D`, `:30` never open the list).
pub(crate) fn token(input: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let (start, q) = raw_token(&chars, cursor)?;
    (q.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2).then_some((start, q))
}

/// After a `:` typed at `cursor - 1`: when `:name:` ends there and the
/// name is known, the text with the emoji in its place, and the new cursor.
pub(crate) fn replace_typed(input: &str, cursor: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = input.chars().collect();
    if cursor == 0 || chars.get(cursor - 1) != Some(&':') {
        return None;
    }
    let (start, name) = raw_token(&chars, cursor - 1)?;
    let glyph = get(&name)?;
    Some(splice(&chars, start, cursor, glyph))
}

/// The composer once `glyph` replaces the token at `start..cursor`: the new
/// text and the new cursor (just after the emoji).
pub(crate) fn complete(input: &str, start: usize, cursor: usize, glyph: &str) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    splice(&chars, start, cursor.min(chars.len()), glyph)
}

fn splice(chars: &[char], start: usize, end: usize, glyph: &str) -> (String, usize) {
    // total: a stale range is clamped to the text, never sliced past it
    let end = end.min(chars.len());
    let start = start.min(end);
    let mut out: String = chars[..start].iter().collect();
    out.push_str(glyph);
    let cur = start + glyph.chars().count();
    out.extend(&chars[end..]);
    (out, cur)
}

/// Most entries the popup lists.
const MAX: usize = 50;

/// The emojis for a query, best first, each with the shortcode shown:
/// exact name, name prefix, name substring, tag prefix, then close
/// misspellings of a name or tag (`aplause` finds 👏 by its `applause`
/// tag). Shorter names first within a rank.
pub(crate) fn filter(query: &str) -> Vec<(&'static Emoji, &'static str)> {
    let q = query.to_ascii_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, usize, &'static Emoji, &'static str)> = Vec::new();
    for (i, e) in all().iter().enumerate() {
        let best = e
            .names
            .iter()
            .filter_map(|n| rank_name(&q, n).map(|r| (r, n.len(), *n)))
            .min();
        let best = best.or_else(|| {
            // a tag match shows the first shortcode
            let r = e.tags.iter().filter_map(|t| rank_tag(&q, t)).min()?;
            Some((r, e.names[0].len(), e.names[0]))
        });
        let best = best.or_else(|| {
            let close = e.names.iter().chain(e.tags.iter()).any(|w| close_to(&q, w));
            close.then(|| (5, e.names[0].len(), e.names[0]))
        });
        if let Some((r, len, name)) = best {
            hits.push((r, len, i, e, name));
        }
    }
    hits.sort_by_key(|h| (h.0, h.1, h.2));
    hits.into_iter().take(MAX).map(|h| (h.3, h.4)).collect()
}

fn rank_name(q: &str, name: &str) -> Option<u8> {
    if name == q {
        Some(0)
    } else if name.starts_with(q) {
        Some(1)
    } else if name.contains(q) {
        Some(2)
    } else {
        None
    }
}

fn rank_tag(q: &str, tag: &str) -> Option<u8> {
    if tag == q {
        Some(3)
    } else if q.len() >= 3 && tag.starts_with(q) {
        Some(4)
    } else {
        None
    }
}

/// A misspelling of `word` or of its start: 1 edit for 3 to 5 letters,
/// 2 edits from 6 letters (insert, delete, change, swap two letters).
fn close_to(q: &str, word: &str) -> bool {
    let n = q.chars().count();
    if n < 3 {
        return false;
    }
    let max = if n >= 6 { 2 } else { 1 };
    let w: Vec<char> = word.chars().collect();
    let q: Vec<char> = q.chars().collect();
    // the whole word, or a start of it about as long as the query
    (n.saturating_sub(max)..=n + max)
        .filter(|&l| l <= w.len() && l > 0)
        .any(|l| edits(&q, &w[..l]) <= max)
}

/// Optimal string alignment distance (Levenshtein plus adjacent swaps).
fn edits(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs(q: &str) -> Vec<&'static str> {
        filter(q).into_iter().map(|(e, _)| e.glyph).collect()
    }

    #[test]
    fn table_loads_with_standard_names() {
        assert!(all().len() > 1500);
        assert_eq!(get("clap"), Some("👏"));
        assert_eq!(get("CLAP"), Some("👏"));
        assert_eq!(get("+1"), Some("👍"));
        assert_eq!(get("thumbsup"), Some("👍"));
        assert_eq!(get("heart"), Some("❤️"));
        assert_eq!(get("tada"), Some("🎉"));
        assert_eq!(get("aplause"), None);
    }

    #[test]
    fn token_opens_only_at_a_word_start_with_two_letters() {
        assert_eq!(token(":cl", 3), Some((0, "cl".into())));
        assert_eq!(token("bravo :cla", 10), Some((6, "cla".into())));
        assert_eq!(token("(:cl", 4), Some((1, "cl".into())));
        assert_eq!(token("👏:ta", 4), Some((1, "ta".into())));
        // one letter, no letter, smileys
        assert_eq!(token(":c", 2), None);
        assert_eq!(token("at :30", 6), None);
        assert_eq!(token(":-D", 3), None);
        assert_eq!(token(":)", 2), None);
        // URLs, times, labels, paths
        assert_eq!(token("http://ex", 9), None);
        assert_eq!(token("see https:", 10), None);
        assert_eq!(token("12:30", 5), None);
        assert_eq!(token("12:ab", 5), None);
        assert_eq!(token("note:todo", 9), None);
        assert_eq!(token("a: b", 4), None);
        assert_eq!(token("std::vec", 8), None);
        // the word must end at the cursor
        assert_eq!(token(":clap now", 9), None);
        assert_eq!(token(":clap", 3), Some((0, "cl".into())));
    }

    #[test]
    fn typed_closing_colon_replaces_a_known_name_only() {
        assert_eq!(replace_typed(":clap:", 6), Some(("👏".into(), 1)));
        assert_eq!(replace_typed("bravo :clap:", 12), Some(("bravo 👏".into(), 7)));
        assert_eq!(replace_typed("bravo :clap: tail", 12), Some(("bravo 👏 tail".into(), 7)));
        assert_eq!(replace_typed(":+1:", 4), Some(("👍".into(), 1)));
        assert_eq!(replace_typed("👏:tada:", 7), Some(("👏🎉".into(), 2)));
        // unknown names, misspellings, no opening word
        assert_eq!(replace_typed(":aplause:", 9), None);
        assert_eq!(replace_typed(":nope:", 6), None);
        assert_eq!(replace_typed("a:clap:", 7), None);
        assert_eq!(replace_typed("http://", 5), None);
        assert_eq!(replace_typed("12:30:", 6), None);
        assert_eq!(replace_typed("::", 2), None);
        // the cursor must be just after the colon
        assert_eq!(replace_typed(":clap:", 5), None);
    }

    #[test]
    fn complete_replaces_the_token() {
        assert_eq!(complete("bravo :cl", 6, 9, "👏"), ("bravo 👏".into(), 7));
        assert_eq!(complete(":cl tail", 0, 3, "👏"), ("👏 tail".into(), 1));
    }

    #[test]
    fn filter_ranks_exact_prefix_substring_tags_then_misspellings() {
        let cl = filter("cl");
        assert_eq!(cl[0].1, "cl"); // 🆑, the exact name
        let clap = cl.iter().position(|(e, _)| e.glyph == "👏").unwrap();
        assert!(clap < 8, "clap in the first page: {clap}");
        assert_eq!(glyphs("clap")[0], "👏");
        // a tag match
        assert!(glyphs("applause").contains(&"👏"));
        // misspellings of a tag or a name
        assert!(glyphs("aplause").contains(&"👏"));
        assert!(glyphs("aplau").contains(&"👏"));
        assert!(glyphs("tdaa").contains(&"🎉"));
        assert!(filter("zzzzqq").is_empty());
        assert!(filter("").is_empty());
        assert!(filter("e").len() <= MAX);
    }

    #[test]
    fn edits_counts_changes_and_swaps() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(edits(&c("aplause"), &c("applause")), 1);
        assert_eq!(edits(&c("tdaa"), &c("tada")), 1);
        assert_eq!(edits(&c("abc"), &c("abc")), 0);
        assert_eq!(edits(&c(""), &c("ab")), 2);
    }
}
