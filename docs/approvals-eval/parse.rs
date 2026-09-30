// Prototype: parse real agent bash commands with brush-parser and emit the
// simple commands (program + args after quote removal), redirections and the
// unreadable ("dynamic") parts. Tier logic lives in tiers.py.
use brush_parser::ast::*;
use brush_parser::word::{self, WordPiece, WordPieceWithSource};
use brush_parser::{Parser, ParserOptions};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

struct Out { parts: Vec<Value>, depth: usize }

fn opts() -> ParserOptions { ParserOptions::default() }

// Unquote a word; report dynamic pieces and collect nested command substitutions.
fn expand(w: &str, subs: &mut Vec<String>) -> (String, bool) {
    let pieces = match word::parse(w, &opts()) { Ok(p) => p, Err(_) => return (w.to_string(), true) };
    let mut s = String::new();
    let mut dynamic = false;
    fn walk(ps: &[WordPieceWithSource], s: &mut String, d: &mut bool, subs: &mut Vec<String>) {
        for p in ps {
            match &p.piece {
                WordPiece::Text(t) => { if t.contains('*') || t.contains('?') || t.contains('[') { /* glob: static enough */ } s.push_str(t) }
                WordPiece::SingleQuotedText(t) | WordPiece::AnsiCQuotedText(t) => s.push_str(t),
                WordPiece::EscapeSequence(t) => s.push_str(t.trim_start_matches('\\')),
                WordPiece::DoubleQuotedSequence(v) | WordPiece::GettextDoubleQuotedSequence(v) => walk(v, s, d, subs),
                WordPiece::TildeExpansion(_) => s.push_str("~"),
                WordPiece::ParameterExpansion(_) => { *d = true; s.push_str("$VAR") }
                WordPiece::CommandSubstitution(c) | WordPiece::BackquotedCommandSubstitution(c) => { *d = true; subs.push(c.clone()); s.push_str("$(…)") }
                WordPiece::ArithmeticExpression(_) => { *d = true; s.push_str("$((…))") }
            }
        }
    }
    walk(&pieces, &mut s, &mut dynamic, subs);
    (s, dynamic)
}

fn list(o: &mut Out, l: &CompoundList, ctx: &str) { for CompoundListItem(ao, sep) in &l.0 {
    let bg = matches!(sep, SeparatorOperator::Async);
    and_or(o, ao, if bg { "background" } else { ctx });
} }
fn and_or(o: &mut Out, ao: &AndOrList, ctx: &str) {
    pipeline(o, &ao.first, ctx);
    for x in &ao.additional { match x { AndOr::And(p) | AndOr::Or(p) => pipeline(o, p, ctx) } }
}
fn pipeline(o: &mut Out, p: &Pipeline, ctx: &str) {
    let piped = p.seq.len() > 1;
    for (i, c) in p.seq.iter().enumerate() { command(o, c, ctx, if piped && i > 0 { "piped" } else { "" }) }
}
fn redirs(rl: &Option<RedirectList>, subs: &mut Vec<String>, dynamic: &mut bool) -> Vec<Value> {
    rl.as_ref().map(|r| r.0.iter().map(|x| redir(x, subs, dynamic)).collect()).unwrap_or_default()
}
fn redir(r: &IoRedirect, subs: &mut Vec<String>, dynamic: &mut bool) -> Value {
    match r {
        IoRedirect::File(fd, k, t) => {
            let (target, kind) = match t {
                IoFileRedirectTarget::Filename(w) => { let (s, d) = expand(&w.value, subs); if d { *dynamic = true } (s, "file") }
                IoFileRedirectTarget::Fd(n) => (n.to_string(), "fd"),
                IoFileRedirectTarget::Duplicate(w) => (w.value.clone(), "dup"),
                IoFileRedirectTarget::ProcessSubstitution(_, sc) => { *dynamic = true; subs.push(sc.list.to_string()); ("<(…)".into(), "procsub") }
            };
            json!({"op": k.to_string(), "fd": fd, "target": target, "kind": kind})
        }
        IoRedirect::HereDocument(fd, h) => json!({"op": "<<", "fd": fd, "kind": "heredoc", "doc_len": h.doc.value.len(), "doc": h.doc.value.chars().take(4000).collect::<String>()}),
        IoRedirect::HereString(fd, w) => { let (s, d) = expand(&w.value, subs); if d { *dynamic = true } json!({"op": "<<<", "fd": fd, "kind": "herestring", "target": s}) }
        IoRedirect::OutputAndError(w, append) => { let (s, d) = expand(&w.value, subs); if d { *dynamic = true } json!({"op": if *append {"&>>"} else {"&>"}, "kind": "file", "target": s}) }
    }
}
fn command(o: &mut Out, c: &Command, ctx: &str, pipe: &str) {
    match c {
        Command::Simple(sc) => simple(o, sc, ctx, pipe),
        Command::Compound(cc, rl) => {
            let mut subs = vec![]; let mut d = false;
            let rs = redirs(rl, &mut subs, &mut d);
            if !rs.is_empty() { o.parts.push(json!({"words": ["<compound>"], "redirs": rs, "dynamic": d, "ctx": ctx})) }
            compound(o, cc, ctx);
            nested(o, subs, ctx);
        }
        Command::Function(f) => { o.parts.push(json!({"words": ["<function>"], "dynamic": false, "ctx": ctx})); compound(o, &f.body.0, "function") }
        Command::ExtendedTest(_, _) => o.parts.push(json!({"words": ["[["], "dynamic": false, "ctx": ctx})),
    }
}
fn compound(o: &mut Out, cc: &CompoundCommand, ctx: &str) {
    match cc {
        CompoundCommand::BraceGroup(b) => list(o, &b.list, ctx),
        CompoundCommand::Subshell(s) => list(o, &s.list, ctx),
        CompoundCommand::ForClause(f) => {
            let mut subs = vec![];
            for w in f.values.iter().flatten() { expand(&w.value, &mut subs); }
            nested(o, subs, ctx);
            list(o, &f.body.list, "loop")
        }
        CompoundCommand::IfClause(i) => { list(o, &i.condition, ctx); list(o, &i.then, ctx);
            for e in i.elses.iter().flatten() { if let Some(c) = &e.condition { list(o, c, ctx) } list(o, &e.body, ctx) } }
        CompoundCommand::WhileClause(w) | CompoundCommand::UntilClause(w) => { list(o, &w.0, "loop"); list(o, &w.1.list, "loop") }
        CompoundCommand::CaseClause(c) => for it in &c.cases { if let Some(l) = &it.cmd { list(o, l, ctx) } },
        CompoundCommand::Coprocess(_) => o.parts.push(json!({"words": ["coproc"], "dynamic": true, "ctx": ctx})),
        CompoundCommand::Arithmetic(_) | CompoundCommand::ArithmeticForClause(_) => o.parts.push(json!({"words": ["(("], "dynamic": true, "ctx": ctx})),
    }
}
fn nested(o: &mut Out, subs: Vec<String>, ctx: &str) {
    if o.depth > 4 { return }
    for s in subs { o.depth += 1; parse_into(o, &s, &format!("{ctx}subst")); o.depth -= 1 }
}
fn simple(o: &mut Out, sc: &SimpleCommand, ctx: &str, pipe: &str) {
    let mut words = vec![]; let mut dyn_flags = vec![]; let mut assigns = vec![]; let mut rs = vec![];
    let mut subs = vec![]; let mut rdyn = false;
    let mut item = |it: &CommandPrefixOrSuffixItem, words: &mut Vec<String>, dyn_flags: &mut Vec<bool>, assigns: &mut Vec<String>, rs: &mut Vec<Value>, subs: &mut Vec<String>| match it {
        CommandPrefixOrSuffixItem::Word(w) => { let (s, d) = expand(&w.value, subs); words.push(s); dyn_flags.push(d) }
        CommandPrefixOrSuffixItem::AssignmentWord(a, _) => { let v = a.value.to_string(); expand(&v, subs); assigns.push(a.name.to_string()) }
        CommandPrefixOrSuffixItem::IoRedirect(r) => rs.push(redir(r, subs, &mut rdyn)),
        CommandPrefixOrSuffixItem::ProcessSubstitution(_, s) => { subs.push(s.list.to_string()); words.push("<(…)".into()); dyn_flags.push(true) }
    };
    if let Some(p) = &sc.prefix { for it in &p.0 { item(it, &mut words, &mut dyn_flags, &mut assigns, &mut rs, &mut subs) } }
    if let Some(w) = &sc.word_or_name { let (s, d) = expand(&w.value, &mut subs); words.push(s); dyn_flags.push(d) }
    if let Some(p) = &sc.suffix { for it in &p.0 { item(it, &mut words, &mut dyn_flags, &mut assigns, &mut rs, &mut subs) } }
    // `bash -c '<script>'` / `sh -c`: parse the string again
    if words.len() >= 3 && matches!(words[0].as_str(), "bash" | "sh" | "zsh") && words[1] == "-c" && !dyn_flags[2] && o.depth < 4 {
        let inner = words[2].clone();
        o.depth += 1; parse_into(o, &inner, &format!("{ctx}bash-c")); o.depth -= 1;
    }
    o.parts.push(json!({"words": words, "dyn": dyn_flags, "assigns": assigns, "redirs": rs, "rdyn": rdyn, "ctx": ctx, "pipe": pipe}));
    nested(o, subs, ctx);
}
fn parse_into(o: &mut Out, src: &str, ctx: &str) -> Result<(), String> {
    let mut p = Parser::new(std::io::Cursor::new(src), &opts());
    match p.parse_program() {
        Ok(prog) => { for cc in &prog.complete_commands { list(o, cc, ctx) } Ok(()) }
        Err(e) => { o.parts.push(json!({"words": ["<unparsed>"], "ctx": ctx, "err": e.to_string()})); Err(e.to_string()) }
    }
}
fn main() {
    let stdin = std::io::stdin(); let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let cmd = v["cmd"].as_str().unwrap_or("");
        let t = std::time::Instant::now();
        let mut o = Out { parts: vec![], depth: 0 };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let mut o2 = Out { parts: vec![], depth: 0 }; let r = parse_into(&mut o2, cmd, ""); (o2.parts, r) }));
        let (ok, err) = match r { Ok((parts, r)) => { o.parts = parts; (r.is_ok(), r.err()) } Err(_) => (false, Some("panic".into())) };
        let us = t.elapsed().as_micros();
        writeln!(out, "{}", json!({"i": v["i"], "ok": ok, "err": err, "us": us, "parts": o.parts})).unwrap();
    }
}
