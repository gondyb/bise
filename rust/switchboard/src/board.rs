//! The state views every agent reads: the task board of main (RFC 0001
//! §8.2), the agent threads summary (RFC 0003 §9), the group roster of a
//! task, and the `sb list` answer. All built without an LLM, from the
//! state alone.

use crate::model::{Agent, Mode, MsgState, State, Status, MAIN, USER};
use crate::prompts::relation;
use crate::util::{age, clip, one_line};
use std::collections::BTreeMap;

fn ws_label(a: &Agent) -> String {
    match a.ws.mode {
        Mode::Worktree if a.ws.dropped => " [worktree supprimé]".to_string(),
        Mode::Worktree => format!(" [worktree {}]", a.ws.branch.clone().unwrap_or_default()),
        Mode::Shared => String::new(),
    }
}

/// One task board line.
pub fn task_line(a: &Agent, now: u64) -> String {
    let st = a.status();
    let mut s = format!(
        "{:<24} {:<9} {:>4}  \"{}\"{}",
        a.name,
        st.as_str(),
        age(a.created_ms, now),
        clip(&a.description(), 70),
        ws_label(a)
    );
    if let Some((_, note)) = &a.declared {
        if !note.is_empty() {
            s.push_str(&format!("  note: {}", clip(&one_line(note), 80)));
        }
    }
    if let Some(r) = &a.last_report {
        s.push_str(&format!(
            "  {} report {} ago: \"{}\"",
            if r.auto { "last turn" } else { r.kind.as_str() },
            age(r.at_ms, now),
            clip(&one_line(&r.summary), 160)
        ));
    }
    if st == Status::Failed {
        if let Some(f) = &a.failure {
            s.push_str(&format!("  failure: {}", clip(&one_line(f), 120)));
        }
    }
    s
}

/// Threads between agents that the user is not part of, most recent
/// first (RFC 0003 §9).
pub fn agent_threads(st: &State, limit: usize) -> Vec<String> {
    let mut threads: BTreeMap<u64, Vec<&crate::model::Msg>> = BTreeMap::new();
    for m in st.msgs.values() {
        threads.entry(m.thread).or_default().push(m);
    }
    let mut out: Vec<(u64, String)> = Vec::new();
    for (t, ms) in threads {
        if ms.iter().any(|m| m.from == USER || m.to == USER || m.plain) {
            continue;
        }
        let parties: Vec<&str> = {
            let mut p: Vec<&str> = Vec::new();
            for m in &ms {
                for x in [m.from.as_str(), m.to.as_str()] {
                    if !p.contains(&x) {
                        p.push(x);
                    }
                }
            }
            p
        };
        if parties.contains(&MAIN) && parties.len() == 2 {
            // main's own conversations are already in its thread
            continue;
        }
        let open = ms
            .iter()
            .any(|m| m.expect_reply && !st.settled.contains(&m.id));
        let last = ms.last().map(|m| m.created_ms).unwrap_or(0);
        out.push((
            last,
            format!(
                "t_{} {}  {} message{}  {}  \"{}\"",
                t,
                parties.join(" ↔ "),
                ms.len(),
                if ms.len() > 1 { "s" } else { "" },
                if open { "ouvert" } else { "répondu" },
                clip(&one_line(&ms[0].text), 60)
            ),
        ));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().take(limit).map(|(_, s)| s).collect()
}

/// The block appended to every model request of main.
pub fn main_context(st: &State, now: u64) -> String {
    let mut s = String::from(
        "<switchboard_state>\nLive state injected by the Switchboard hub before this call (not a user message).\n<task_board>\n",
    );
    let tasks: Vec<&Agent> = st.tasks().filter(|a| a.status() != Status::Archived).collect();
    if tasks.is_empty() {
        s.push_str("(no task)\n");
    }
    for a in &tasks {
        s.push_str(&task_line(a, now));
        s.push('\n');
    }
    let archived = st.tasks().filter(|a| a.status() == Status::Archived).count();
    if archived > 0 {
        s.push_str(&format!("({} archived task{})\n", archived, if archived > 1 { "s" } else { "" }));
    }
    s.push_str("</task_board>\n");
    let threads = agent_threads(st, 8);
    if !threads.is_empty() {
        s.push_str("<agent_threads>\n");
        for t in threads {
            s.push_str(&t);
            s.push('\n');
        }
        s.push_str("</agent_threads>\n");
    }
    let cards: Vec<String> = st
        .open_cards()
        .map(|c| {
            let for_msg = c.for_msg.map(|m| format!(" (answers m_{})", m)).unwrap_or_default();
            format!("#{} {} @{}: \"{}\"{}", c.id, c.kind, c.agent, clip(&one_line(&c.text), 100), for_msg)
        })
        .collect();
    if !cards.is_empty() {
        s.push_str("<open_cards>\n");
        for c in cards {
            s.push_str(&c);
            s.push('\n');
        }
        s.push_str("</open_cards>\n");
    }
    let waiting: Vec<String> = st
        .unanswered_for(MAIN)
        .iter()
        .map(|m| format!("m_{} from {}: \"{}\"", m.id, m.from, clip(&one_line(&m.text), 80)))
        .collect();
    if !waiting.is_empty() {
        s.push_str("<questions_for_you>\n");
        for w in waiting {
            s.push_str(&w);
            s.push('\n');
        }
        s.push_str("</questions_for_you>\n");
    }
    s.push_str("</switchboard_state>");
    s
}

/// The group roster, as `name` sees it (`sb list`, and the context of a
/// task).
pub fn roster(st: &State, viewer: &str, now: u64) -> Vec<String> {
    let viewer_parent = st.agents.get(viewer).and_then(|a| a.parent.clone());
    st.order
        .iter()
        .filter_map(|n| st.agents.get(n))
        .filter(|a| a.status() != Status::Archived)
        .map(|a| {
            let rel = if a.name == viewer {
                "self"
            } else {
                match relation(&a.name, viewer, a.parent.as_deref(), viewer_parent.as_deref()) {
                    "parent" => "parent",
                    "child" => "child",
                    _ => "peer",
                }
            };
            let note = a
                .declared
                .as_ref()
                .filter(|(_, n)| !n.is_empty())
                .map(|(_, n)| format!(" — {}", clip(&one_line(n), 60)))
                .unwrap_or_default();
            format!(
                "{:<24} {:<6} {:<9} {:>4}  {}{}",
                a.name,
                rel,
                a.status().as_str(),
                age(if a.is_main { now } else { a.created_ms }, now),
                clip(&a.description(), 70),
                note
            )
        })
        .collect()
}

/// The block appended to every model request of a task.
pub fn task_context(st: &State, name: &str, now: u64) -> String {
    let mut s = format!(
        "<switchboard_state>\nLive state injected by the Switchboard hub before this call (not a user message). You are `{}`.\n<group>\n",
        name
    );
    for l in roster(st, name, now) {
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str("</group>\n");
    let waiting: Vec<String> = st
        .unanswered_for(name)
        .iter()
        .map(|m| format!("m_{} from {}: \"{}\"", m.id, m.from, clip(&one_line(&m.text), 80)))
        .collect();
    if !waiting.is_empty() {
        s.push_str("<questions_for_you>\n");
        for w in waiting {
            s.push_str(&w);
            s.push('\n');
        }
        s.push_str("</questions_for_you>\n");
    }
    s.push_str("</switchboard_state>");
    s
}

/// Queued messages, for the board shown to the user.
pub fn queued_count(st: &State, name: &str) -> usize {
    st.msgs
        .values()
        .filter(|m| m.to == name && matches!(st.msg_state.get(&m.id), Some(MsgState::Queued { .. })))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Brief, Event, Msg, Workspace};

    fn state() -> State {
        let mut st = State::new("/w");
        for (n, obj) in [("auth-fix", "Corriger le login Safari"), ("docs", "Doc API v2")] {
            st.apply(&Event::TaskCreated {
                name: n.into(),
                parent: MAIN.into(),
                brief: Brief {
                    objective: obj.into(),
                    ..Brief::default()
                },
                ws: Workspace {
                    mode: Mode::Shared,
                    path: "/w".into(),
                    branch: None,
                    base_commit: None,
                    dropped: false,
                },
                at_ms: 0,
            });
        }
        st
    }

    #[test]
    fn the_board_lists_every_live_task() {
        let st = state();
        let c = main_context(&st, 12 * 60_000);
        assert!(c.contains("auth-fix"), "{}", c);
        assert!(c.contains("\"Doc API v2\""), "{}", c);
        assert!(c.contains("12m"), "{}", c);
        assert!(c.starts_with("<switchboard_state>") && c.ends_with("</switchboard_state>"));
    }

    #[test]
    fn peer_threads_are_summarized_for_main() {
        let mut st = state();
        st.apply(&Event::MessageSent {
            msg: Msg {
                id: 1,
                thread: 1,
                from: "docs".into(),
                to: "auth-fix".into(),
                reply_to: None,
                expect_reply: true,
                auto: false,
                text: "v1 ou v2 ?".into(),
                created_ms: 1,
                plain: false,
            },
        });
        let t = agent_threads(&st, 8);
        assert_eq!(t.len(), 1);
        assert!(t[0].contains("docs ↔ auth-fix") && t[0].contains("ouvert"), "{}", t[0]);
    }

    #[test]
    fn a_task_sees_its_relations() {
        let st = state();
        let r = roster(&st, "docs", 0);
        assert!(r.iter().any(|l| l.starts_with("main") && l.contains("parent")), "{:?}", r);
        assert!(r.iter().any(|l| l.starts_with("auth-fix") && l.contains("peer")), "{:?}", r);
        assert!(r.iter().any(|l| l.starts_with("docs") && l.contains("self")), "{:?}", r);
    }
}
