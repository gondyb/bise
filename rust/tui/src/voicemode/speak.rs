//! What is said aloud of a message (owner: voice-tts; design §4, plan
//! §4.5). Pure.
//!
//! The whole message is said, sentence by sentence (plan §8 #4: you
//! can't always read, so nothing is left for the screen). Skipped
//! quietly: code blocks, tables, URLs, hashes, ids, flags, versions. A
//! path or a file name is said as its last word (`src/ui.rs`: "ui"), or
//! skipped when that is not a plain word. Headings are said. A list is
//! said whole: how many, then every item; short items in one sentence
//! ("two things: smaller and faster."). Numbers are rounded and in words,
//! key combos are said (`ctrl+r`: "control R"); numbers and every word
//! bise adds are in the message's language ([`language`]), else the
//! configured one. Inline code is said only when it is one plain word, a
//! number, a key combo or a path; else skipped. Every said word keeps its
//! byte range in the message (the thread's lighting); the words bise adds
//! have none.

use super::{Sentence, Spoken, Word};
use std::ops::Range;

/// A list whose items all have at most this many words...
const SHORT_ITEM: usize = 4;
/// ...and at most this many items is said in one sentence.
const SHORT_LIST: usize = 6;

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

/// The whole message as said, sentence by sentence: code, tables, URLs,
/// ids and hashes skipped quietly; numbers and added words in the
/// message's language (else `language`, the configured one). `more` is
/// always false: nothing is left for the screen.
pub fn speakable(msg: &str, language: Option<&str>) -> Spoken {
    if msg.trim().is_empty() {
        return Spoken::default();
    }
    let lang = Lang::of(detect(msg).or(language));
    let mut out: Vec<Vec<SayWord>> = Vec::new();
    for block in blocks(msg) {
        match block {
            Block::Shown => {}
            Block::Prose(lines) => out.extend(sentences(words_of(msg, &lines, lang))),
            Block::List(items) => say_list(msg, &items, lang, &mut out),
        }
    }
    out.retain(|s| !s.is_empty());
    if out.is_empty() {
        // only code or a table: one short line, not silence
        out.push(added(lang.pick("it's on screen.", "c'est à l'écran.")));
    }
    for s in &mut out {
        close(s);
    }
    Spoken { sentences: out.into_iter().map(sentence).collect(), more: false }
}

/// The language a text is in, when it is clear: "fr" or "en" (by their
/// small words, accents and elisions; code and URLs aside). None for a
/// text too short or too mixed to tell.
pub fn language(text: &str) -> Option<&'static str> {
    detect(text)
}

const FR_WORDS: &[&str] = &[
    "le", "la", "les", "des", "du", "de", "un", "une", "et", "est", "sont", "pas", "ne", "je", "tu", "il", "elle", "nous", "vous", "ils", "ce", "ça",
    "cette", "ces", "qui", "que", "dans", "pour", "sur", "avec", "mais", "ou", "où", "au", "aux", "à", "très", "aussi", "fait", "été", "mon", "ma",
    "mes", "ton", "ta", "tes", "son", "sa", "ses", "lance", "oui", "non", "merci", "voilà", "encore", "tout", "tous", "rien", "peux", "veux",
];
const EN_WORDS: &[&str] = &[
    "the", "is", "are", "was", "were", "and", "to", "of", "in", "it", "that", "this", "for", "with", "you", "i", "not", "have", "has", "be", "do",
    "does", "can", "will", "what", "my", "your", "we", "they", "now", "run", "yes", "no", "please", "thanks", "from", "but", "all", "just",
];

fn detect(text: &str) -> Option<&'static str> {
    let (mut fr, mut en) = (0u32, 0u32);
    let mut in_fence = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || t.starts_with('|') {
            continue;
        }
        for (i, part) in line.split('`').enumerate() {
            // odd parts are inline code
            if i % 2 == 1 {
                continue;
            }
            for raw in part.split_whitespace() {
                if raw.contains("://") || raw.contains('/') {
                    continue;
                }
                let w: String = raw.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
                if w.is_empty() {
                    continue;
                }
                let w = w.replace('’', "'");
                if ["l'", "d'", "c'", "j'", "n'", "qu'", "s'", "m'", "t'"].iter().any(|p| w.starts_with(p) && w.len() > p.len()) {
                    fr += 1;
                } else if w.ends_with("n't") || w.ends_with("'s") || w.ends_with("'re") || w.ends_with("'ll") || w.ends_with("'ve") || w == "i'm" {
                    en += 1;
                } else if FR_WORDS.contains(&w.as_str()) {
                    fr += 1;
                } else if EN_WORDS.contains(&w.as_str()) {
                    en += 1;
                } else if w.contains(['é', 'è', 'ê', 'à', 'ç', 'ù', 'û', 'ô', 'î', 'œ']) {
                    fr += 1;
                }
            }
        }
    }
    // a clear lead: more than the other, and more than a third of the hits
    match fr.cmp(&en) {
        std::cmp::Ordering::Greater if fr * 2 > en * 3 || en == 0 => Some("fr"),
        std::cmp::Ordering::Less if en * 2 > fr * 3 || fr == 0 => Some("en"),
        _ => None,
    }
}

// ---- the message's blocks ----

#[derive(Clone, Debug, PartialEq)]
enum Block {
    /// a paragraph: its lines' text ranges
    Prose(Vec<Range<usize>>),
    /// a list: its items in order, nested ones too
    List(Vec<Item>),
    /// code or a table: shown only
    Shown,
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    /// its lines' text ranges (the marker aside)
    lines: Vec<Range<usize>>,
    /// a nested item (not counted)
    nested: bool,
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
        if let Some(off) = heading(t) {
            // said as its own sentence
            out.push(Block::Prose(one(tstart + off..start + line.len())));
            open = false;
            continue;
        }
        if is_bold_line(t) {
            // `**What you see**`: its own sentence too
            out.push(Block::Prose(one(tstart..start + line.len())));
            open = false;
            continue;
        }
        if let Some(off) = list_marker(t) {
            let lines = one(tstart + off..start + line.len());
            match out.last_mut() {
                Some(Block::List(items)) => items.push(Item { lines, nested: indent >= 2 }),
                _ => out.push(Block::List(vec![Item { lines, nested: false }])),
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
                    it.lines.push(range);
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

/// One line's range, as a block's lines.
fn one(r: Range<usize>) -> Vec<Range<usize>> {
    std::iter::once(r).collect()
}

/// `## title`: the offset of its text.
fn heading(t: &str) -> Option<usize> {
    let hashes = t.chars().take_while(|c| *c == '#').count();
    ((1..=6).contains(&hashes) && t[hashes..].starts_with(' ')).then(|| hashes + 1)
}

/// A line that is all bold (`**What you see**`), a heading in effect.
fn is_bold_line(t: &str) -> bool {
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
                // "committed as c5bfd13;": the dangling "as" goes, the
                // ";" stays on the word before it
                skipped = punct.is_empty();
                attach(&mut out, punct, !punct.is_empty());
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

/// A list, said whole: how many, then every item, nested ones too. Short
/// plain items go in one sentence ("two things: smaller and faster."),
/// joined to the sentence that brings them in when it ends with a colon
/// ("Two choices: smaller and faster."); else each item is its own
/// sentence(s), after "four things.".
fn say_list(msg: &str, items: &[Item], lang: Lang, out: &mut Vec<Vec<SayWord>>) {
    let said: Vec<(bool, Vec<SayWord>)> =
        items.iter().map(|it| (it.nested, words_of(msg, &it.lines, lang))).filter(|(_, w)| !w.is_empty()).collect();
    let n = said.iter().filter(|(nested, _)| !nested).count() as u64;
    if said.is_empty() {
        return;
    }
    let after_colon = out.last().and_then(|s| s.last()).is_some_and(|w| w.punct == ":");
    let ends = |w: &SayWord| matches!(w.punct.as_str(), "." | "?" | "!");
    let short = said.len() <= SHORT_LIST
        && said.iter().all(|(nested, w)| !nested && w.len() <= SHORT_ITEM && !w[..w.len() - 1].iter().any(ends));
    if short {
        let mut s = match out.pop() {
            Some(s) if after_colon => s,
            other => {
                out.extend(other);
                if n >= 2 {
                    how_many(n, lang, ":")
                } else {
                    Vec::new()
                }
            }
        };
        let k = said.len();
        for (i, (_, mut words)) in said.into_iter().enumerate() {
            for w in &mut words {
                w.punct.clear();
            }
            if i > 0 && i + 1 == k {
                s.push(SayWord { text: lang.pick("and", "et").into(), src: None, punct: String::new() });
            }
            if i + 2 < k {
                if let Some(w) = words.last_mut() {
                    w.punct = ",".into();
                }
            }
            s.extend(words);
        }
        out.push(s);
        return;
    }
    if !after_colon && n >= 2 {
        out.push(how_many(n, lang, "."));
    }
    for (_, words) in said {
        out.extend(sentences(words));
    }
}

/// "four things" (added words), then `punct`.
fn how_many(n: u64, lang: Lang, punct: &str) -> Vec<SayWord> {
    let mut out: Vec<SayWord> = cardinal(n, lang).into_iter().map(|w| SayWord { text: w, src: None, punct: String::new() }).collect();
    let thing = match (lang, n) {
        (Lang::En, 1) => "thing",
        (Lang::En, _) => "things",
        (Lang::Fr, 1) => "chose",
        (Lang::Fr, _) => "choses",
    };
    out.push(SayWord { text: thing.into(), src: None, punct: punct.into() });
    out
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
        // a path: its last word
        let last = c.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        return file_word(last).or_else(|| plain_word(last)).map(|w| Said::Words(vec![w], None)).unwrap_or(Said::Skip);
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
    // a file name, a domain, a version: letters around a dot (a file
    // name says its stem)
    let cs: Vec<char> = c.chars().collect();
    if cs.windows(3).any(|w| w[1] == '.' && w[0].is_alphanumeric() && w[2].is_alphanumeric()) {
        return file_word(c).map(|w| Said::Words(vec![w], None)).unwrap_or(Said::Skip);
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

/// The extensions that make `name.ext` a file name (else a domain or a
/// version: skipped).
const FILE_EXTS: &[&str] = &[
    "rs", "md", "toml", "ts", "tsx", "js", "jsx", "mjs", "json", "py", "sh", "yaml", "yml", "txt", "lock", "go", "c", "h", "cpp", "swift", "html",
    "css", "aiff", "wav", "png", "jpg", "svg", "log", "sql", "bend", "kt", "java", "rb", "zsh", "plist", "mp3", "pdf", "csv",
];

/// A file name's stem when it is a plain word (`speak.rs`: "speak",
/// `README.md`: "README"); None for anything else.
fn file_word(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if !FILE_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
        return None;
    }
    // "Cargo.lock", "voice.test.ts": the first part
    plain_word(stem.split('.').next().unwrap_or(""))
}

/// A word that says itself: letters, inner hyphens (no digits, no `_`).
fn plain_word(w: &str) -> Option<String> {
    let ok = w.chars().count() >= 2
        && w.chars().all(|c| c.is_alphabetic() || c == '-')
        && !w.starts_with('-')
        && !w.ends_with('-');
    ok.then(|| w.to_string())
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
