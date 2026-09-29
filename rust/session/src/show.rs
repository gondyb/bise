//! `bise session show` (BISE-199): a session log as text for people.
//! transcript = every event in order (what happened); context = what the
//! model sees now (the rebuilt context, compaction applied).
use crate::project::parts_text;
use crate::reader::{Event, Log};
use crate::state::State;
use crate::types::*;
use std::fmt::Write;
use std::path::Path;

fn indent(t: &str, pad: &str) -> String {
    t.lines().map(|l| format!("{pad}{l}")).collect::<Vec<_>>().join("\n")
}

fn parts(p: &[Part], blobs: &Path) -> String {
    let mut o = String::new();
    for part in p {
        match part {
            Part::Thinking { text, .. } => {
                let _ = writeln!(o, "(thinking)\n{}", indent(text, "  "));
            }
            Part::Image { name, path, image, .. } => {
                let _ = write!(o, "[image {name}: {path}, {} bytes]", image.bytes);
            }
            other => o.push_str(&parts_text(std::slice::from_ref(other), blobs).unwrap_or_else(|e| format!("[{e}]"))),
        }
    }
    o
}

/// an enum as the log writes it ("hub_state")
fn name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

fn time(e: &Event) -> &str {
    e.at.get(11..19).unwrap_or("")
}

/// One event as a few lines; None for events only the UI needs.
fn event(e: &Event, blobs: &Path) -> Option<String> {
    let p = e.payload.as_ref()?;
    let t = time(e);
    Some(match p {
        Payload::SessionStart(s) => {
            let mut o = format!("{t} session {} in {}", s.session, s.cwd);
            if let Some(a) = &s.agent {
                let _ = write!(o, " (agent {} of {})", a.name, a.hub);
            }
            if let Some(m) = &s.migrated_from {
                let _ = write!(o, "\n   migrated from {}", m.path);
            }
            o
        }
        Payload::ProcessOpened(p) => format!("{t} opened by {} (pid {}{})", p.writer, p.pid, if p.resume { ", resumed" } else { "" }),
        Payload::ModelSet(m) => format!("{t} model {}{}", m.model.model, m.model.effort.as_ref().map(|e| format!(" ({e})")).unwrap_or_default()),
        Payload::TurnStarted { .. } => format!("── turn {} ──", e.turn.unwrap_or(0)),
        Payload::TurnEnded(t2) if !matches!(t2.outcome, Outcome::Done) => format!("{t} turn ended: {}", name(&t2.outcome)),
        Payload::UserMessage(m) => format!("{t} › you\n{}", indent(&parts(&m.content, blobs), "  ")),
        Payload::AgentMessage(m) => format!("{t} ✉ {} ({})\n{}", m.from, name(&m.relation), indent(&parts(&m.content, blobs), "  ")),
        Payload::ContextInjected(m) => format!("{t} · injected ({})\n{}", name(&m.kind), indent(&parts(&m.content, blobs), "  ")),
        Payload::AssistantMessage(m) => {
            let mut o = format!("{t} ◆ {}\n{}", m.model, indent(&parts(&m.parts, blobs), "  "));
            for c in &m.calls {
                let _ = write!(o, "\n  → {} {}: {}", c.id, c.name, c.args.lines().next().unwrap_or(""));
            }
            o
        }
        Payload::ToolResult(r) => {
            let text = parts(&r.content, blobs);
            let n = text.lines().count();
            let head: Vec<&str> = text.lines().take(8).collect();
            let mut o = format!("{t} ← {} {}\n{}", r.call, if r.ok { "ok" } else { "failed" }, indent(&head.join("\n"), "  "));
            if n > 8 {
                let _ = write!(o, "\n  … {} more lines", n - 8);
            }
            o
        }
        Payload::Usage(u) => format!("   usage: in {} out {} cache {}", u.input, u.output, u.cache_read.unwrap_or(0)),
        Payload::RequestFailed(r) => format!("{t} ✗ request failed ({}): {}", r.attempt, r.error.message),
        Payload::Interrupted(i) => format!("{t} interrupted by {} during {}", name(&i.by), name(&i.during)),
        Payload::InputQueued(q) => format!("{t} queued: {}", parts(&q.content, blobs)),
        Payload::CompactionDone(c) => format!("{t} compaction {}: replaces {}..{}\n{}", c.id, c.replaces.from, c.replaces.to, indent(&parts(&c.summary, blobs), "  ")),
        Payload::CompactionFailed(c) => format!("{t} compaction {} failed: {}", c.id, c.error.message),
        Payload::TitleSet { title, .. } => format!("{t} title: {title}"),
        _ => return None,
    })
}

/// Every event, in order.
pub fn transcript(log: &Log, blobs: &Path) -> String {
    let mut o = String::new();
    for e in &log.events {
        match event(e, blobs) {
            Some(s) => {
                o.push_str(&s);
                o.push('\n');
            }
            None if e.payload.is_none() => {
                let _ = writeln!(o, "{} [{} v{}: from a newer bise]", time(e), e.typ, e.v);
            }
            None => {}
        }
    }
    let skipped = log.bad_lines.len() + usize::from(log.torn.is_some());
    if skipped > 0 {
        let _ = writeln!(o, "[{skipped} unreadable line(s) skipped]");
    }
    o
}

/// What the model sees now: the context events, in order.
pub fn context(log: &Log, st: &State, blobs: &Path) -> String {
    let mut o = String::new();
    for s in &st.context {
        if let Some(line) = log.by_seq(*s).and_then(|e| event(e, blobs)) {
            o.push_str(&line);
            o.push('\n');
        }
    }
    o
}
