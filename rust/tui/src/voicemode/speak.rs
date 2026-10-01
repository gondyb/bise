//! What is said aloud of a message (owner: voice-tts; design §4, plan
//! §4.5). Pure.
//!
//! The first sentence or two (≤ [`MAX_WORDS`] words, ~12 s at 1×) are
//! said; the rest is shown. Never said: code blocks, tables, headings,
//! paths, file names, URLs, ids, hashes, flags. Numbers are rounded and in
//! words (English, or French when you speak it), key combos are said
//! (`ctrl+r`: "control R"). A list is said as how many, then up to 3 items
//! in ≤ 3 words. When something is left out, the last sentence is "the
//! rest is on screen.". Inline code is said only when it is one plain
//! word, a number or a key combo (a branch name, `ctrl+r`); else skipped.
//! Every said word keeps its byte range in the message (the thread's
//! lighting); the words bise adds have none.

use super::{Sentence, Spoken, Word};
use std::ops::Range;

/// The most words said of a message (~12 s at 1×), the closing line aside.
pub const MAX_WORDS: usize = 30;
/// A first sentence longer than this is cut (at a comma when it can).
const MAX_FIRST: usize = 40;
/// A list: how many, then this many items...
const LIST_ITEMS: usize = 3;
/// ...in this many words each.
const ITEM_WORDS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    pub fn of(language: Option<&str>) -> Lang {
        match language {
            Some(l) if l.trim().to_ascii_lowercase().starts_with("fr") => Lang::Fr,
            _ => Lang::En,
        }
    }
    fn pick(self, en: &'static str, fr: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }
}

/// The first sentence or two (~12 s), never code, paths, ids, hashes,
/// URLs or tables; numbers rounded and in words; lists as how many and
/// up to 3 items; "the rest is on screen" when there is more.
pub fn speakable(msg: &str, language: Option<&str>) -> Spoken {
    let lang = Lang::of(language);
    if msg.trim().is_empty() {
        return Spoken::default();
    }
    let mut chosen: Vec<Vec<SayWord>> = Vec::new();
    let mut count = 0;
    let mut more = false;
    let mut done = false;
    for block in blocks(msg) {
        match block {
            Block::Heading => {}
            Block::Shown => more = true,
            _ if done => more = true,
            Block::Prose(lines) => {
                for mut s in sentences(words_of(msg, &lines, lang)) {
                    if done {
                        more = true;
                        break;
                    }
                    if chosen.is_empty() {
                        if s.len() > MAX_FIRST {
                            cut_first(&mut s);
                            more = true;
                        }
                    } else if chosen.len() >= 2 || count + s.len() > MAX_WORDS {
                        more = true;
                        done = true;
                        break;
                    }
                    count += s.len();
                    chosen.push(s);
                    done = chosen.len() >= 2 || count >= MAX_WORDS;
                }
            }
            Block::List(items) => {
                let (summary, left_out) = list_summary(msg, &items, lang);
                let after_colon = chosen.last().and_then(|s| s.last()).is_some_and(|w| w.punct == ":");
                if chosen.is_empty() || (after_colon && count + summary.len() <= MAX_WORDS + 6) {
                    if let Some(w) = chosen.last_mut().and_then(|s| s.last_mut()) {
                        w.punct = ".".into();
                    }
                    count += summary.len();
                    chosen.push(summary);
                    more |= left_out;
                } else {
                    more = true;
                }
                done = true;
            }
        }
    }
    if chosen.is_empty() {
        chosen.push(added(lang.pick("it's on screen.", "c'est à l'écran.")));
        more = true;
    } else if more {
        chosen.push(added(lang.pick("the rest is on screen.", "la suite est à l'écran.")));
    }
    for s in &mut chosen {
        close(s);
    }
    Spoken { sentences: chosen.into_iter().map(sentence).collect(), more }
}

// ---- the message's blocks ----

#[derive(Clone, Debug, PartialEq)]
enum Block {
    /// a paragraph: its lines' text ranges
    Prose(Vec<Range<usize>>),
    /// a list: each top-level item's line ranges (nested items dropped)
    List(Vec<Vec<Range<usize>>>),
    /// code or a table: shown only
    Shown,
    /// a heading or a bold line: neither said nor "more"
    Heading,
}

fn blocks(msg: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut open = false;
    let mut fence: Option<&str> = None;
    let mut at = 0;
    for raw in msg.split_inclusive('\n') {
        let start = at;
        at += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let t = line.trim_start();
        let indent = line.len() - t.len();
        let tstart = start + indent;
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            out.push(Block::Shown);
            open = false;
            continue;
        }
        if t.is_empty() || is_rule(t) {
            open = false;
            continue;
        }
        if t.starts_with('|') {
            if out.last() != Some(&Block::Shown) {
                out.push(Block::Shown);
            }
            open = false;
            continue;
        }
        if is_heading(t) {
            out.push(Block::Heading);
            open = false;
            continue;
        }
        if let Some(off) = list_marker(t) {
            let item = tstart + off..start + line.len();
            match out.last_mut() {
                // a nested item: shown, not counted
                Some(Block::List(_)) if indent >= 2 => {}
                Some(Block::List(items)) => items.push(vec![item]),
                _ => out.push(Block::List(vec![vec![item]])),
            }
            open = true;
            continue;
        }
        let (t, tstart) = match t.strip_prefix('>') {
            Some(q) => {
                let q2 = q.trim_start();
                (q2, tstart + 1 + (q.len() - q2.len()))
            }
            None => (t, tstart),
        };
        let range = tstart..tstart + t.len();
        match out.last_mut() {
            Some(Block::List(items)) if open && indent >= 2 => {
                if let Some(it) = items.last_mut() {
                    it.push(range);
                }
            }
            Some(Block::Prose(lines)) if open => lines.push(range),
            _ => out.push(Block::Prose(vec![range])),
        }
        open = true;
    }
    out
}

fn is_rule(t: &str) -> bool {
    let c: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    c.len() >= 3 && (c.chars().all(|x| x == '-') || c.chars().all(|x| x == '*') || c.chars().all(|x| x == '_'))
}

/// `## title`, or a line that is all bold (`**What you see**`).
fn is_heading(t: &str) -> bool {
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        return true;
    }
    let body = t.trim_end().trim_end_matches(':');
    body.len() > 4 && body.starts_with("**") && body.ends_with("**") && !body[2..body.len() - 2].contains("**")
}

/// The byte offset of a list item's text after its marker.
fn list_marker(t: &str) -> Option<usize> {
    let b = t.as_bytes();
    let off = if b.len() >= 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
        2
    } else {
        let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
        if (1..=3).contains(&digits) && b.len() > digits + 1 && matches!(b[digits], b'.' | b')') && b[digits + 1] == b' ' {
            digits + 2
        } else {
            return None;
        }
    };
    let rest = &t[off..];
    let rest2 = rest.trim_start();
    let mut off = off + (rest.len() - rest2.len());
    for b in ["[ ] ", "[x] ", "[X] "] {
        if t[off..].starts_with(b) {
            off += b.len();
        }
    }
    Some(off)
}

// ---- words ----

/// A word to say: its text, where it is in the message, its punctuation.
#[derive(Clone, Debug, PartialEq)]
struct SayWord {
    text: String,
    src: Option<Range<usize>>,
    /// "" or one of . ? ! , ; :
    punct: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TokKind {
    Text,
    /// the inside of `…`
    Code,
    /// a link's URL, an image
    Skip,
}

#[derive(Clone, Debug, PartialEq)]
struct Tok {
    range: Range<usize>,
    kind: TokKind,
}

/// The message's tokens in `r`: words, code spans, links' text.
fn tokens(msg: &str, r: Range<usize>, out: &mut Vec<Tok>) {
    let s = &msg[r.clone()];
    let b = s.as_bytes();
    let base = r.start;
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i] == b'`' {
            let n = b[i..].iter().take_while(|c| **c == b'`').count();
            let fence = &s[i..i + n];
            if let Some(j) = s[i + n..].find(fence) {
                let inner = i + n..i + n + j;
                let trimmed = s[inner.clone()].trim();
                let lead = s[inner.clone()].len() - s[inner.clone()].trim_start().len();
                out.push(Tok { range: base + inner.start + lead..base + inner.start + lead + trimmed.len(), kind: TokKind::Code });
                i += n + j + n;
                continue;
            }
        }
        if b[i] == b'!' && b.get(i + 1) == Some(&b'[') {
            if let Some((_, end)) = link(s, i + 1) {
                out.push(Tok { range: base + i..base + end, kind: TokKind::Skip });
                i = end;
                continue;
            }
        }
        if b[i] == b'[' {
            if let Some((text, end)) = link(s, i) {
                tokens(msg, base + text.start..base + text.end, out);
                out.push(Tok { range: base + text.end..base + end, kind: TokKind::Skip });
                i = end;
                continue;
            }
        }
        let j = i + b[i..].iter().position(|c| c.is_ascii_whitespace() || *c == b'`').unwrap_or(b.len() - i);
        let j = if j == i { i + 1 } else { j };
        out.push(Tok { range: base + i..base + j, kind: TokKind::Text });
        i = j;
    }
}

/// `[text](url)` at `i`: the text's range and the end of the link.
fn link(s: &str, i: usize) -> Option<(Range<usize>, usize)> {
    let close = i + s[i..].find(']')?;
    if !s[close + 1..].starts_with('(') {
        return None;
    }
    let end = close + 1 + s[close + 1..].find(')')? + 1;
    Some((i + 1..close, end))
}

/// What a token says.
#[derive(Clone, Debug, PartialEq)]
enum Said {
    /// its words, and the number it says (units after it agree with it)
    Words(Vec<String>, Option<f64>),
    /// only punctuation for the word before ("—" is a comma)
    Punct(&'static str),
    Skip,
}

const MARKS_LEAD: &[char] = &['*', '_', '~', '(', '[', '"', '“', '‘', '\'', '«', '¿', '¡'];
const MARKS_TRAIL: &[char] = &['*', '_', '~', ')', ']', '"', '”', '’', '\'', '»'];
const PUNCT: &[char] = &['.', ',', ';', ':', '!', '?', '…'];

/// The token without its marks: the word's range, whether `~` led it,
/// and its punctuation (one of . ? ! , ; : or "").
fn strip(msg: &str, r: Range<usize>) -> (Range<usize>, bool, &'static str) {
    let t = &msg[r.clone()];
    let lead_len = t.len() - t.trim_start_matches(MARKS_LEAD).len();
    let about = t[..lead_len].ends_with('~');
    let mut core = &t[lead_len..];
    let mut punct = String::new();
    while let Some(c) = core.chars().last() {
        if PUNCT.contains(&c) {
            punct.insert(0, c);
        } else if !MARKS_TRAIL.contains(&c) {
            break;
        }
        core = &core[..core.len() - c.len_utf8()];
    }
    let p = if punct.contains('?') {
        "?"
    } else if punct.contains('!') {
        "!"
    } else if punct.contains('.') || punct.contains('…') {
        "."
    } else {
        match punct.chars().last() {
            Some(',') => ",",
            Some(';') => ";",
            Some(':') => ":",
            _ => "",
        }
    };
    let start = r.start + lead_len;
    (start..start + core.len(), about, p)
}

/// Words that dangle when the thing after them is not said ("committed
/// as ~~46c4da5~~;" says "committed;").
const DANGLING: &[&str] = &[
    "a", "an", "and", "or", "et", "ou", "as", "at", "by", "for", "from", "in", "into", "of", "on", "the", "to", "via", "with", "à", "au", "aux", "dans", "de",
    "des", "du", "en", "la", "le", "les", "par", "pour", "sur", "un", "une", "vers",
];

/// Punctuation for the last word (a dangling word before a skipped
/// thing goes).
fn attach(out: &mut Vec<SayWord>, punct: &str, after_skip: bool) {
    if after_skip && out.len() > 1 {
        if let Some(w) = out.last() {
            if w.punct.is_empty() && DANGLING.contains(&w.text.to_lowercase().as_str()) {
                out.pop();
            }
        }
    }
    if let Some(w) = out.last_mut() {
        if !punct.is_empty() && !matches!(w.punct.as_str(), "." | "?" | "!") {
            w.punct = punct.to_string();
        }
    }
}

/// The said words of some lines, in order.
fn words_of(msg: &str, lines: &[Range<usize>], lang: Lang) -> Vec<SayWord> {
    let mut toks = Vec::new();
    for l in lines {
        tokens(msg, l.clone(), &mut toks);
    }
    let mut out: Vec<SayWord> = Vec::new();
    let mut last_num: Option<f64> = None;
    let mut skipped = false;
    for tok in toks {
        let (core, about, punct) = match tok.kind {
            TokKind::Skip => (tok.range.start..tok.range.start, false, ""),
            TokKind::Code => (tok.range.clone(), false, ""),
            TokKind::Text => strip(msg, tok.range.clone()),
        };
        let c = &msg[core.clone()];
        let said = match tok.kind {
            TokKind::Skip => Said::Skip,
            TokKind::Code if c.contains(char::is_whitespace) => Said::Skip,
            _ if c.is_empty() => Said::Punct(""),
            _ => match last_num.and_then(|v| unit_words(c, v, lang)) {
                Some(u) => Said::Words(u, None),
                None => classify(c, about, lang),
            },
        };
        match said {
            Said::Words(words, num) => {
                // "committed as <hash> on main": the "as" dangles too
                let dangles = |w: &str| DANGLING.contains(&w.to_lowercase().as_str());
                if skipped
                    && out.len() > 1
                    && words.first().is_some_and(|w| dangles(w))
                    && out.last().is_some_and(|w| w.punct.is_empty() && dangles(&w.text))
                {
                    out.pop();
                }
                // "e.g." does not end the sentence
                let punct = if punct == "." && abbreviation(&c.to_lowercase(), lang).is_some() { "" } else { punct };
                let n = words.len();
                for (i, w) in words.into_iter().enumerate() {
                    out.push(SayWord { text: w, src: Some(core.clone()), punct: if i + 1 == n { punct.to_string() } else { String::new() } });
                }
                last_num = num;
                skipped = false;
            }
            Said::Punct(p) => {
                attach(&mut out, if p.is_empty() { punct } else { p }, skipped);
                last_num = None;
            }
            Said::Skip => {
                skipped = true;
                attach(&mut out, punct, false);
                if !punct.is_empty() {
                    attach(&mut out, punct, true);
                    skipped = false;
                }
                last_num = None;
            }
        }
    }
    if skipped {
        attach(&mut out, "", true);
    }
    out
}

/// The words split at their sentence ends; the last ends with "." too.
fn sentences(words: Vec<SayWord>) -> Vec<Vec<SayWord>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for w in words {
        let end = matches!(w.punct.as_str(), "." | "?" | "!");
        cur.push(w);
        if end {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The sentence's end: "." unless it asks or exclaims.
fn close(s: &mut [SayWord]) {
    if let Some(w) = s.last_mut() {
        if !matches!(w.punct.as_str(), "." | "?" | "!") {
            w.punct = ".".into();
        }
    }
}

/// A first sentence too long to say whole: cut at a comma between 12
/// and 30 words, else at 30.
fn cut_first(s: &mut Vec<SayWord>) {
    let at = (8..MAX_WORDS).rev().find(|i| matches!(s[*i].punct.as_str(), "," | ";" | ":")).map(|i| i + 1).unwrap_or(MAX_WORDS);
    s.truncate(at);
    if let Some(w) = s.last_mut() {
        w.punct = ".".into();
    }
}

/// "three things: inbox strip, key bar and opening items." and whether
/// it leaves anything out.
fn list_summary(msg: &str, items: &[Vec<Range<usize>>], lang: Lang) -> (Vec<SayWord>, bool) {
    let n = items.len() as u64;
    let mut out: Vec<SayWord> = Vec::new();
    let count = if lang == Lang::Fr && n == 1 { vec!["une".to_string()] } else { cardinal(n, lang) };
    for w in count {
        out.push(SayWord { text: w, src: None, punct: String::new() });
    }
    let thing = match (lang, n) {
        (Lang::En, 1) => "thing",
        (Lang::En, _) => "things",
        (Lang::Fr, 1) => "chose",
        (Lang::Fr, _) => "choses",
    };
    out.push(SayWord { text: thing.into(), src: None, punct: ":".into() });
    let mut left_out = items.len() > LIST_ITEMS;
    let mut named: Vec<Vec<SayWord>> = Vec::new();
    for item in items.iter().take(LIST_ITEMS) {
        let first = &item[0];
        let label = bold_label(msg, first.clone());
        let mut words = words_of(msg, &[label.clone().unwrap_or(first.clone())], lang);
        if label.is_some() || words.len() > ITEM_WORDS || item.len() > 1 {
            left_out = true;
        }
        words.truncate(ITEM_WORDS);
        for w in &mut words {
            w.punct.clear();
        }
        if !words.is_empty() {
            named.push(words);
        }
    }
    let k = named.len();
    for (i, mut words) in named.into_iter().enumerate() {
        if i > 0 && i + 1 == k {
            out.push(SayWord { text: lang.pick("and", "et").into(), src: None, punct: String::new() });
        }
        if i + 2 < k {
            if let Some(w) = words.last_mut() {
                w.punct = ",".into();
            }
        }
        out.extend(words);
    }
    if let Some(w) = out.last_mut() {
        w.punct = ".".into();
    }
    (out, left_out)
}

/// An item's `**label:**` (or `**label**`) at its start.
fn bold_label(msg: &str, r: Range<usize>) -> Option<Range<usize>> {
    let t = &msg[r.clone()];
    let inner = t.strip_prefix("**")?;
    let end = inner.find("**")?;
    (end > 0).then(|| r.start + 2..r.start + 2 + end)
}

fn added(text: &str) -> Vec<SayWord> {
    let parts: Vec<&str> = text.split(' ').collect();
    let n = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let (t, punct) = if i + 1 == n { (p.trim_end_matches('.'), ".") } else { (p, "") };
            SayWord { text: t.into(), src: None, punct: punct.into() }
        })
        .collect()
}

fn sentence(words: Vec<SayWord>) -> Sentence {
    let mut say = String::new();
    let mut out = Vec::with_capacity(words.len());
    for w in words {
        if !say.is_empty() {
            say.push(' ');
        }
        let start = say.len();
        say.push_str(&w.text);
        say.push_str(&w.punct);
        out.push(Word { say: start..say.len(), src: w.src });
    }
    Sentence { say, words: out }
}

// ---- what a token says ----

fn classify(c: &str, about: bool, lang: Lang) -> Said {
    let lower = c.to_lowercase();
    if !c.chars().any(char::is_alphanumeric) {
        return match c {
            "&" => Said::Words(vec![lang.pick("and", "et").into()], None),
            "≥" | ">=" => Said::Words(words(lang.pick("at least", "au moins")), None),
            "≤" | "<=" => Said::Words(words(lang.pick("at most", "au plus")), None),
            "—" | "–" | "-" | "--" => Said::Punct(","),
            _ => Said::Skip,
        };
    }
    if let Some(w) = abbreviation(&lower, lang) {
        return Said::Words(words(w), None);
    }
    if lower.contains("://") || lower.starts_with("www.") {
        return Said::Skip;
    }
    if c.contains(['_', '#', '=', '<', '>', '{', '}', '|', '\\', '@', '`', '^']) || c.contains("::") || c.contains("()") {
        return Said::Skip;
    }
    if c.contains('/') {
        // "on/off", "and/or": short plain words
        let parts: Vec<&str> = c.split('/').collect();
        if parts.len() == 2 && parts.iter().all(|p| !p.is_empty() && p.len() <= 8 && p.chars().all(char::is_alphabetic)) {
            return Said::Words(vec![parts[0].into(), lang.pick("or", "ou").into(), parts[1].into()], None);
        }
        return Said::Skip;
    }
    if c.contains('+') && !c.starts_with('+') {
        return keys(c, lang).map(|w| Said::Words(w, None)).unwrap_or(Said::Skip);
    }
    let first = c.chars().next().unwrap_or(' ');
    let second = c.chars().nth(1).unwrap_or(' ');
    if first.is_ascii_digit() || ("$€£-+≈".contains(first) && second.is_ascii_digit()) {
        return match number(c, about, lang) {
            Some((w, v)) => Said::Words(w, Some(v)),
            None => Said::Skip,
        };
    }
    if first == '-' || first == '.' || first == '~' || first == '$' {
        return Said::Skip;
    }
    // a file name, a domain, a version: letters around a dot
    let cs: Vec<char> = c.chars().collect();
    if cs.windows(3).any(|w| w[1] == '.' && w[0].is_alphanumeric() && w[2].is_alphanumeric()) {
        return Said::Skip;
    }
    // ids and hashes: letters and digits mixed
    if c.chars().any(|x| x.is_ascii_digit()) {
        return Said::Skip;
    }
    match lower.as_str() {
        "ctrl" => return Said::Words(vec![lang.pick("control", "contrôle").into()], None),
        "cmd" => return Said::Words(vec![lang.pick("command", "commande").into()], None),
        "esc" => return Said::Words(vec![lang.pick("escape", "échap").into()], None),
        _ => {}
    }
    let w: String = c.chars().filter(|x| x.is_alphanumeric() || matches!(x, '-' | '\'' | '’' | '.')).collect();
    if w.is_empty() || !w.chars().any(char::is_alphabetic) {
        Said::Skip
    } else {
        Said::Words(vec![w], None)
    }
}

fn words(s: &str) -> Vec<String> {
    s.split(' ').map(str::to_string).collect()
}

fn abbreviation(lower: &str, lang: Lang) -> Option<&'static str> {
    Some(match lower {
        "e.g" | "eg" => lang.pick("for example", "par exemple"),
        "i.e" | "ie" => lang.pick("that is", "c'est-à-dire"),
        "etc" => lang.pick("and so on", "et cetera"),
        "vs" => "versus",
        "approx" => lang.pick("about", "environ"),
        _ => return None,
    })
}

/// `ctrl+r`, `cmd+shift+p`, `ctrl+1-9`: said; None when a part is not a key.
fn keys(c: &str, lang: Lang) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut has_modifier = false;
    for part in c.split('+') {
        let p = part.to_lowercase();
        let w = match p.as_str() {
            "ctrl" | "control" | "⌃" => {
                has_modifier = true;
                lang.pick("control", "contrôle")
            }
            "cmd" | "command" | "⌘" => {
                has_modifier = true;
                lang.pick("command", "commande")
            }
            "alt" | "opt" | "option" | "⌥" => {
                has_modifier = true;
                "option"
            }
            "shift" | "⇧" => {
                has_modifier = true;
                "shift"
            }
            "esc" | "escape" => lang.pick("escape", "échap"),
            "tab" => "tab",
            "space" => lang.pick("space", "espace"),
            "enter" | "return" | "⏎" => lang.pick("enter", "entrée"),
            _ if part.chars().count() == 1 && part.chars().all(|x| x.is_ascii_alphabetic()) => {
                out.push(part.to_uppercase());
                continue;
            }
            _ if part.starts_with(|x: char| x.is_ascii_digit()) => {
                out.extend(number(part, false, lang)?.0);
                continue;
            }
            _ if part.len() >= 2 && part.chars().all(|x| x.is_ascii_alphabetic()) => {
                out.push(part.to_string());
                continue;
            }
            _ => return None,
        };
        out.push(w.to_string());
    }
    has_modifier.then_some(out)
}

/// A unit after a number ("529 ms"), agreeing with it.
fn unit_words(c: &str, value: f64, lang: Lang) -> Option<Vec<String>> {
    let one = match lang {
        Lang::En => value == 1.0,
        Lang::Fr => value < 2.0,
    };
    let (en1, en2, fr1, fr2) = match c {
        "ms" => ("millisecond", "milliseconds", "milliseconde", "millisecondes"),
        "s" | "sec" | "secs" => ("second", "seconds", "seconde", "secondes"),
        "min" | "mins" => ("minute", "minutes", "minute", "minutes"),
        "h" | "hr" | "hrs" => ("hour", "hours", "heure", "heures"),
        "%" => ("percent", "percent", "pour cent", "pour cent"),
        "x" | "×" => ("times", "times", "fois", "fois"),
        "kB" | "KB" => ("kilobyte", "kilobytes", "kilo-octet", "kilo-octets"),
        "MB" => ("megabyte", "megabytes", "mégaoctet", "mégaoctets"),
        "GB" => ("gigabyte", "gigabytes", "gigaoctet", "gigaoctets"),
        "TB" => ("terabyte", "terabytes", "téraoctet", "téraoctets"),
        "kHz" => ("kilohertz", "kilohertz", "kilohertz", "kilohertz"),
        "Hz" => ("hertz", "hertz", "hertz", "hertz"),
        "px" => ("pixel", "pixels", "pixel", "pixels"),
        "fps" => ("frame a second", "frames a second", "image par seconde", "images par seconde"),
        _ => return None,
    };
    let w = match (lang, one) {
        (Lang::En, true) => en1,
        (Lang::En, false) => en2,
        (Lang::Fr, true) => fr1,
        (Lang::Fr, false) => fr2,
    };
    Some(words(w))
}

// ---- numbers ----

/// A number token in words: `529`, `1,234`, `0.8-1.6`, `45%`, `$0.003`,
/// `16:28`, `3rd`, `2s`, `~12`, `2026`; and its value. None: not one.
pub fn number(c: &str, about: bool, lang: Lang) -> Option<(Vec<String>, f64)> {
    let mut s = c;
    let mut about = about;
    let mut pre: Vec<String> = Vec::new();
    let mut currency: Option<(&str, &str, &str, &str)> = None;
    if let Some(r) = s.strip_prefix('≈') {
        about = true;
        s = r;
    }
    if let Some(r) = s.strip_prefix('-') {
        pre.push(lang.pick("minus", "moins").into());
        s = r;
    } else if let Some(r) = s.strip_prefix('+') {
        pre.push("plus".into());
        s = r;
    }
    for (sym, cur) in [('$', ("dollar", "dollars", "dollar", "dollars")), ('€', ("euro", "euros", "euro", "euros")), ('£', ("pound", "pounds", "livre", "livres"))] {
        if let Some(r) = s.strip_prefix(sym) {
            currency = Some(cur);
            s = r;
        }
    }
    // a time
    if let Some((h, m)) = s.split_once(':') {
        let (h, m): (u64, u64) = (h.parse().ok()?, m.parse().ok()?);
        if m.to_string().len() > 2 || h > 24 || m > 59 {
            return None;
        }
        return Some((time_words(h, m, lang), h as f64));
    }
    // a range: 1-3, 0.8-1.6
    if let Some(i) = s.find(['-', '–']).filter(|i| *i > 0) {
        let (a, b) = (&s[..i], &s[i + s[i..].chars().next()?.len_utf8()..]);
        let (aw, _) = number(a, false, lang)?;
        let (bw, bv) = number(b, false, lang)?;
        let mut out = pre;
        if about {
            out.push(lang.pick("about", "environ").into());
        }
        out.extend(aw);
        out.push(lang.pick("to", "à").into());
        out.extend(bw);
        return Some((out, bv));
    }
    // digits, then a suffix
    let digits_end = s.char_indices().find(|(_, ch)| !(ch.is_ascii_digit() || *ch == ',' || *ch == '.' || *ch == ' ')).map(|(i, _)| i).unwrap_or(s.len());
    let (num, suffix) = s.split_at(digits_end);
    let num = num.trim_end_matches(['.', ',']);
    if num.is_empty() || num.contains(' ') {
        return None;
    }
    let value = parse_value(num, lang)?;
    // ordinals
    let ordinal = matches!((lang, suffix), (Lang::En, "st" | "nd" | "rd" | "th") | (Lang::Fr, "e" | "è" | "ème" | "eme" | "er" | "re" | "ère"));
    if ordinal && value.fract() == 0.0 {
        let fem = matches!(suffix, "re" | "ère");
        let mut out = pre;
        out.extend(ordinal_words(value as u64, fem, lang));
        return Some((out, value));
    }
    let (mult, unit) = match suffix {
        "k" | "K" => (1e3, ""),
        "M" => (1e6, ""),
        _ => (1.0, suffix),
    };
    let value = value * mult;
    let year = unit.is_empty() && currency.is_none() && pre.is_empty() && num.len() == 4 && num.chars().all(|x| x.is_ascii_digit()) && (1900.0..2100.0).contains(&value);
    let (mut w, rounded) = if year { (year_words(value as u64, lang), false) } else { value_words(value, lang) };
    let mut out = pre;
    if about || rounded {
        out.push(lang.pick("about", "environ").into());
    }
    out.append(&mut w);
    if let Some((en1, en2, fr1, fr2)) = currency {
        let one = value == 1.0;
        out.push(match (lang, one) {
            (Lang::En, true) => en1,
            (Lang::En, false) => en2,
            (Lang::Fr, true) => fr1,
            (Lang::Fr, false) => fr2,
        }
        .into());
    }
    if !unit.is_empty() {
        out.extend(unit_words(unit, value, lang)?);
    }
    Some((out, value))
}

/// "1,234" "3.5"; in French "3,5" is three and a half ("1,234": thousands).
fn parse_value(num: &str, lang: Lang) -> Option<f64> {
    let groups = |s: &str| {
        let parts: Vec<&str> = s.split(',').collect();
        parts.len() > 1 && parts[0].len() <= 3 && !parts[0].is_empty() && parts[1..].iter().all(|p| p.len() == 3)
    };
    let int_dec = |s: &str| -> String {
        if s.contains(',') && !groups(s.split('.').next().unwrap_or("")) {
            // a decimal comma (French), or junk
            if lang == Lang::Fr && !s.contains('.') && s.matches(',').count() == 1 {
                return s.replace(',', ".");
            }
            return "x".into();
        }
        s.replace(',', "")
    };
    let n = int_dec(num);
    if n.matches('.').count() > 1 {
        return None;
    }
    n.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The words of a value, rounded; true when rounding changed it (said
/// with "about").
fn value_words(v: f64, lang: Lang) -> (Vec<String>, bool) {
    let point = lang.pick("point", "virgule");
    if v.fract() != 0.0 {
        if v >= 10.0 {
            return value_words(v.round(), lang);
        }
        if v < 0.1 {
            return (words(lang.pick("almost zero", "presque zéro")), false);
        }
        let d = (v * 10.0).round() / 10.0;
        if d.fract() == 0.0 {
            return (cardinal(d as u64, lang), false);
        }
        let mut out = cardinal(d.trunc() as u64, lang);
        out.push(point.into());
        out.extend(cardinal(((d.fract() * 10.0).round() as u64) % 10, lang));
        return (out, false);
    }
    let n = v as u64;
    if n < 1000 {
        return (cardinal(n, lang), false);
    }
    // two significant digits
    let mag = 10u64.pow((n as f64).log10().floor() as u32 - 1);
    let r = ((n as f64 / mag as f64).round() as u64) * mag;
    let rounded = r != n;
    for (scale, en, fr1, fr2) in [(1_000_000_000u64, "billion", "milliard", "milliards"), (1_000_000, "million", "million", "millions")] {
        if r >= scale {
            let m = r as f64 / scale as f64;
            let mut out = if m < 10.0 && m.fract() != 0.0 {
                let mut o = cardinal(m.trunc() as u64, lang);
                o.push(point.into());
                o.extend(cardinal(((m.fract() * 10.0).round() as u64) % 10, lang));
                o
            } else {
                cardinal(m.round() as u64, lang)
            };
            out.push(match lang {
                Lang::En => en,
                Lang::Fr if m < 2.0 => fr1,
                Lang::Fr => fr2,
            }
            .into());
            return (out, rounded);
        }
    }
    (cardinal(r, lang), rounded)
}

pub fn cardinal(n: u64, lang: Lang) -> Vec<String> {
    let s = match lang {
        Lang::En => en(n),
        Lang::Fr => fr(n),
    };
    words(&s)
}

const EN_ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen",
    "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const EN_TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn en(n: u64) -> String {
    let tail = |big: u64, word: &str, n: u64| {
        let r = n % big;
        format!("{} {}{}", en(n / big), word, if r > 0 { format!(" {}", en(r)) } else { String::new() })
    };
    match n {
        0..=19 => EN_ONES[n as usize].into(),
        20..=99 => {
            let t = EN_TENS[(n / 10) as usize];
            if n.is_multiple_of(10) {
                t.into()
            } else {
                format!("{}-{}", t, EN_ONES[(n % 10) as usize])
            }
        }
        100..=999 => tail(100, "hundred", n),
        1_000..=999_999 => tail(1_000, "thousand", n),
        1_000_000..=999_999_999 => tail(1_000_000, "million", n),
        _ => tail(1_000_000_000, "billion", n),
    }
}

const FR_ONES: [&str; 20] = [
    "zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze",
    "quinze", "seize", "dix-sept", "dix-huit", "dix-neuf",
];
const FR_TENS: [&str; 7] = ["", "", "vingt", "trente", "quarante", "cinquante", "soixante"];

fn fr(n: u64) -> String {
    match n {
        0..=19 => FR_ONES[n as usize].into(),
        20..=69 => {
            let (t, u) = (FR_TENS[(n / 10) as usize], n % 10);
            match u {
                0 => t.into(),
                1 => format!("{} et un", t),
                _ => format!("{}-{}", t, FR_ONES[u as usize]),
            }
        }
        70..=79 if n == 71 => "soixante et onze".into(),
        70..=79 => format!("soixante-{}", FR_ONES[(n - 60) as usize]),
        80 => "quatre-vingts".into(),
        81..=99 => format!("quatre-vingt-{}", FR_ONES[(n - 80) as usize]),
        100..=999 => {
            let (h, r) = (n / 100, n % 100);
            let hs = if h == 1 { "cent".to_string() } else { format!("{} cent", fr(h)) };
            match (r, h) {
                (0, 1) => hs,
                (0, _) => hs + "s",
                _ => format!("{} {}", hs, fr(r)),
            }
        }
        1_000..=999_999 => {
            let (th, r) = (n / 1000, n % 1000);
            let ts = if th == 1 { "mille".to_string() } else { format!("{} mille", no_plural(fr(th))) };
            if r == 0 {
                ts
            } else {
                format!("{} {}", ts, fr(r))
            }
        }
        _ => {
            let (scale, one, many) = if n >= 1_000_000_000 { (1_000_000_000, "milliard", "milliards") } else { (1_000_000, "million", "millions") };
            let (m, r) = (n / scale, n % scale);
            let head = format!("{} {}", fr(m), if m == 1 { one } else { many });
            if r == 0 {
                head
            } else {
                format!("{} {}", head, fr(r))
            }
        }
    }
}

/// "quatre-vingts mille" is "quatre-vingt mille", "deux cents mille" is
/// "deux cent mille".
fn no_plural(s: String) -> String {
    if s.ends_with("vingts") || s.ends_with("cents") {
        s[..s.len() - 1].to_string()
    } else {
        s
    }
}

fn year_words(y: u64, lang: Lang) -> Vec<String> {
    if lang == Lang::Fr || (2000..2010).contains(&y) {
        return cardinal(y, lang);
    }
    let (hi, lo) = (y / 100, y % 100);
    let mut out = cardinal(hi, lang);
    match lo {
        0 => out.push("hundred".into()),
        1..=9 => {
            out.push("oh".into());
            out.extend(cardinal(lo, lang));
        }
        _ => out.extend(cardinal(lo, lang)),
    }
    out
}

fn time_words(h: u64, m: u64, lang: Lang) -> Vec<String> {
    match lang {
        Lang::En => {
            let mut out = cardinal(h, lang);
            match m {
                0 => out.push("hundred".into()),
                1..=9 => {
                    out.push("oh".into());
                    out.extend(cardinal(m, lang));
                }
                _ => out.extend(cardinal(m, lang)),
            }
            out
        }
        Lang::Fr => {
            let mut out = if h == 1 { vec!["une".to_string()] } else { cardinal(h, lang) };
            out.push(if h < 2 { "heure" } else { "heures" }.into());
            if m > 0 {
                out.extend(cardinal(m, lang));
            }
            out
        }
    }
}

fn ordinal_words(n: u64, feminine: bool, lang: Lang) -> Vec<String> {
    let mut out = cardinal(n, lang);
    let Some(last) = out.pop() else { return out };
    let (head, seg) = match last.rfind('-') {
        Some(i) => (last[..=i].to_string(), last[i + 1..].to_string()),
        None => (String::new(), last.clone()),
    };
    let seg = match lang {
        Lang::En => match seg.as_str() {
            "one" => "first".into(),
            "two" => "second".into(),
            "three" => "third".into(),
            "five" => "fifth".into(),
            "eight" => "eighth".into(),
            "nine" => "ninth".into(),
            "twelve" => "twelfth".into(),
            s if s.ends_with('y') => format!("{}ieth", &s[..s.len() - 1]),
            s => format!("{}th", s),
        },
        Lang::Fr if n == 1 => (if feminine { "première" } else { "premier" }).into(),
        Lang::Fr => match seg.as_str() {
            "un" => "unième".into(),
            "cinq" => "cinquième".into(),
            "neuf" => "neuvième".into(),
            s if s.ends_with("vingts") || s.ends_with("cents") => format!("{}ième", &s[..s.len() - 1]),
            s if s.ends_with('e') => format!("{}ième", &s[..s.len() - 1]),
            s => format!("{}ième", s),
        },
    };
    out.push(head + &seg);
    out
}

#[cfg(test)]
#[path = "speak_tests.rs"]
mod tests;
