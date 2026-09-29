//! Scenario tests of the hub core: inputs in, effects out. No process,
//! no socket, no git (a fake `Env`).

use super::*;

struct FakeEnv {
    now: u64,
    git: bool,
    loss: Loss,
    dropped: Vec<String>,
}

impl FakeEnv {
    fn new() -> FakeEnv {
        FakeEnv {
            now: 1_000_000,
            git: true,
            loss: Loss::default(),
            dropped: Vec::new(),
        }
    }
}

impl Env for FakeEnv {
    fn now(&self) -> u64 {
        self.now
    }
    fn is_git(&self) -> bool {
        self.git
    }
    fn worktree_create(&mut self, name: &str, _with_changes: bool) -> Result<Workspace, String> {
        Ok(Workspace {
            mode: Mode::Worktree,
            path: format!("/state/worktrees/{}", name),
            branch: Some(format!("sb/{}", name)),
            base_commit: Some("abc".into()),
            dropped: false,
        })
    }
    fn worktree_loss(&mut self, _ws: &Workspace) -> Loss {
        self.loss.clone()
    }
    fn worktree_drop(
        &mut self,
        name: &str,
        _ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String> {
        self.dropped.push(name.to_string());
        Ok(loss
            .any()
            .then(|| format!("refs/switchboard/trash/{}/1", name)))
    }
    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        _snap: Option<&str>,
    ) -> Result<Workspace, String> {
        let mut ws = ws.clone();
        ws.dropped = false;
        let _ = name;
        Ok(ws)
    }
}

struct T {
    hub: Hub,
    env: FakeEnv,
    token: u64,
}

impl T {
    fn new() -> T {
        let mut t = T {
            hub: Hub::new("/w"),
            env: FakeEnv::new(),
            token: 0,
        };
        let fx = t.go(Input::Boot);
        assert!(fx.contains(&Effect::Spawn {
            agent: MAIN.into(),
            resume: true,
            crash_note: None
        }));
        t.go(Input::ReplReady { agent: MAIN.into() });
        t.go(Input::ClientHello { client: 1 });
        t
    }

    fn go(&mut self, input: Input) -> Vec<Effect> {
        self.hub.handle(input, &mut self.env)
    }

    fn req(&mut self, from: &str, req: AgentReq) -> (u64, Vec<Effect>) {
        self.token += 1;
        let tok = self.token;
        let fx = self.go(Input::Agent {
            token: tok,
            from: from.into(),
            req,
        });
        (tok, fx)
    }

    fn user(&mut self, focus: &str, text: &str) -> Vec<Effect> {
        self.go(Input::ClientInput {
            client: 1,
            focus: focus.into(),
            text: text.into(),
        })
    }

    /// A turn of `agent`: started, one assistant text, idle.
    fn turn(&mut self, agent: &str, text: &str) -> Vec<Effect> {
        self.go(Input::ReplLine {
            agent: agent.into(),
            line: "  obs: turn_started".into(),
        });
        self.go(Input::ReplLine {
            agent: agent.into(),
            line: format!("  obs: assistant: {}", text),
        });
        self.go(Input::ReplIdle {
            agent: agent.into(),
            leftover: false,
        })
    }

    fn spawn_task(&mut self, name: &str) {
        let (_, fx) = self.req(
            MAIN,
            AgentReq::Spawn {
                name: name.into(),
                brief: Brief {
                    objective: format!("objective of {}", name),
                    ..Brief::default()
                },
                worktree: false,
                with_changes: false,
            },
        );
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == name)),
            "{:?}",
            fx
        );
        let fx = self.go(Input::ReplReady { agent: name.into() });
        assert!(
            say_to(&fx, name).is_some(),
            "the brief starts the first turn: {:?}",
            fx
        );
        // the first turn ends: main gets the automatic reply
        self.go(Input::ReplLine {
            agent: name.into(),
            line: "  obs: turn_started".into(),
        });
    }

    fn status(&self, name: &str) -> Status {
        self.hub.st.agents[name].status()
    }
}

fn say_to(fx: &[Effect], agent: &str) -> Option<String> {
    fx.iter().find_map(|e| match e {
        Effect::Say { agent: a, text } if a == agent => Some(text.clone()),
        _ => None,
    })
}

fn steer_to(fx: &[Effect], agent: &str) -> Option<String> {
    fx.iter().find_map(|e| match e {
        Effect::Steer { agent: a, text } if a == agent => Some(text.clone()),
        _ => None,
    })
}

fn reply(fx: &[Effect], token: u64) -> Option<Value> {
    fx.iter().find_map(|e| match e {
        Effect::Reply { token: t, body } if *t == token => Some(body.clone()),
        _ => None,
    })
}

fn has_line(fx: &[Effect], agent: &str, needle: &str) -> bool {
    fx.iter().any(
        |e| matches!(e, Effect::Line { agent: a, line } if a == agent && line.contains(needle)),
    )
}

#[test]
fn the_user_talks_to_main_by_default() {
    let mut t = T::new();
    let fx = t.user(MAIN, "bonjour");
    assert_eq!(say_to(&fx, MAIN).as_deref(), Some("bonjour"));
    assert!(has_line(&fx, MAIN, "sb you : bonjour"));
    assert_eq!(t.status(MAIN), Status::Working);
    // main is busy: the next message steers the running turn
    let fx = t.user(MAIN, "et aussi");
    assert_eq!(steer_to(&fx, MAIN).as_deref(), Some("et aussi"));
}

#[test]
fn a_spawned_task_answers_main_automatically() {
    let mut t = T::new();
    t.spawn_task("auth-fix");
    assert_eq!(t.status("auth-fix"), Status::Working);
    let fx = t.go(Input::ReplLine {
        agent: "auth-fix".into(),
        line: "  obs: assistant: <think>x</think>corrigé".into(),
    });
    assert!(fx.is_empty() || !fx.iter().any(|e| matches!(e, Effect::Say { .. })));
    let fx = t.go(Input::ReplIdle {
        agent: "auth-fix".into(),
        leftover: false,
    });
    let to_main = say_to(&fx, MAIN).expect("main is woken by the automatic reply");
    assert!(
        to_main.contains("from=\"auth-fix\" relation=\"child\""),
        "{}",
        to_main
    );
    assert!(to_main.contains("auto=\"true\""), "{}", to_main);
    assert!(to_main.contains("corrigé"), "{}", to_main);
    // the board carries the automatic report
    let a = &t.hub.st.agents["auth-fix"];
    assert_eq!(
        a.last_report.as_ref().map(|r| r.summary.as_str()),
        Some("corrigé")
    );
    assert_eq!(t.status("auth-fix"), Status::Idle);
}

#[test]
fn the_board_of_main_follows_the_tasks() {
    let mut t = T::new();
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "docs".into(),
            brief: Brief {
                objective: "Doc API v2".into(),
                ..Brief::default()
            },
            worktree: false,
            with_changes: false,
        },
    );
    let ctx = fx
        .iter()
        .find_map(|e| match e {
            Effect::Context { agent, text } if agent == MAIN => Some(text.clone()),
            _ => None,
        })
        .expect("main's context is rewritten");
    assert!(
        ctx.contains("docs") && ctx.contains("Doc API v2"),
        "{}",
        ctx
    );
}

#[test]
fn steering_left_in_the_file_is_sent_again() {
    let mut t = T::new();
    t.user(MAIN, "premier");
    let fx = t.user(MAIN, "second");
    assert!(steer_to(&fx, MAIN).is_some());
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: assistant: ok".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: true,
    });
    assert_eq!(say_to(&fx, MAIN).as_deref(), Some("second"));
}

#[test]
fn ask_waits_for_the_reply() {
    let mut t = T::new();
    t.spawn_task("docs");
    // main is busy with its own turn: the question steers it
    t.user(MAIN, "go");
    let (tok, fx) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    assert!(reply(&fx, tok).is_none(), "the ask blocks");
    assert_eq!(t.status("docs"), Status::Waiting);
    let q = steer_to(&fx, MAIN).expect("delivered to main");
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    assert!(
        q.contains(&format!("id=\"m_{}\"", id)) && q.contains("expects_reply=\"true\""),
        "{}",
        q
    );
    let (tok2, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
        },
    );
    let r = reply(&fx, tok).expect("the wait ends");
    assert_eq!(r["type"], "reply");
    assert_eq!(r["message"], "v2");
    // the question's id (BISE-110)
    assert_eq!(r["asked"], format!("m_{}", id));
    assert_eq!(reply(&fx, tok2).unwrap()["delivery"], "delivered");
    assert_eq!(t.status("docs"), Status::Working);
    // main answered: no automatic reply at the end of its turn
    let fx = t.turn(MAIN, "réglé");
    assert!(
        say_to(&fx, "docs").is_none() && steer_to(&fx, "docs").is_none(),
        "{:?}",
        fx
    );
}

/// The agent state says who a waiting agent waits on (the panel's
/// `waits {name}`), and forgets it when the wait ends.
#[test]
fn a_waiting_agent_says_who_it_waits_on() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.user(MAIN, "go");
    let (tok, _) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    let agent = |t: &T, n: &str| {
        let snap = t.hub.snapshot(0);
        snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == n).cloned().unwrap()
    };
    let docs = agent(&t, "docs");
    assert_eq!(docs["status"], "waiting");
    assert_eq!(docs["waiting_on"], MAIN, "{}", docs);
    assert!(agent(&t, MAIN)["waiting_on"].is_null());
    let id = t.hub.st.msgs.values().find(|m| m.text == "v1 ou v2 ?").unwrap().id;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
        },
    );
    assert!(reply(&fx, tok).is_some(), "the wait ends");
    assert!(agent(&t, "docs")["waiting_on"].is_null());
}

#[test]
fn a_question_to_a_waiting_agent_ends_its_wait() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let (tok_a, _) = t.req(
        "a",
        AgentReq::Ask {
            to: "b".into(),
            text: "A?".into(),
            timeout_s: 20,
        },
    );
    let (tok_b, fx) = t.req(
        "b",
        AgentReq::Ask {
            to: "a".into(),
            text: "B?".into(),
            timeout_s: 20,
        },
    );
    let r = reply(&fx, tok_a).expect("a's wait ends: b asked it something");
    assert_eq!(r["type"], "incoming_request");
    assert_eq!(r["message"], "B?");
    assert!(reply(&fx, tok_b).is_none());
}

#[test]
fn waits_time_out() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, _) = t.req(
        "a",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "?".into(),
            timeout_s: 600,
        },
    );
    t.env.now += 24_000;
    assert!(reply(&t.go(Input::Tick), tok).is_none());
    t.env.now += 2_000;
    let r = reply(&t.go(Input::Tick), tok).expect("capped at 25 s");
    assert_eq!(r["error"], "timeout");
}

#[test]
fn main_learns_what_the_user_said_directly() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "  obs: assistant: brief lu".into(),
    });
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.turn(MAIN, "noté");
    t.go(Input::ClientFocus {
        client: 1,
        focus: "docs".into(),
    });
    let fx = t.user("docs", "utilise la v2");
    assert_eq!(say_to(&fx, "docs").as_deref(), Some("utilise la v2"));
    t.turn("docs", "ok, v2");
    let fx = t.go(Input::ClientFocus {
        client: 1,
        focus: MAIN.into(),
    });
    assert!(
        has_line(&fx, MAIN, "You talked to @docs (1 message)"),
        "{:?}",
        fx
    );
    // the note rides with the next message to main
    let fx = t.user(MAIN, "où en est la doc ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(s.starts_with("<switchboard_notes>"), "{}", s);
    assert!(s.contains("utilise la v2") && s.contains("ok, v2"), "{}", s);
    assert!(s.ends_with("où en est la doc ?"));
    assert!(t.hub.st.main_notes.is_empty());
}

#[test]
fn at_task_from_main_view_answers_there_and_notes_main() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.turn(MAIN, "noté");
    // the task reads it tagged, from the user via main's view
    let fx = t.user(MAIN, "@docs v1 ou v2 ?");
    let s = say_to(&fx, "docs").unwrap();
    assert_eq!(
        s,
        "<user_message via=\"main\">\nv1 ou v2 ?\n</user_message>"
    );
    assert!(has_line(&fx, "docs", "sb you : v1 ou v2 ?"), "{:?}", fx);
    // its end-of-turn answer comes back to main's view, main not woken
    let fx = t.turn("docs", "la v2");
    // C2 `msg-you`: an agent writing to the user
    assert!(has_line(&fx, MAIN, "sb msg-you : docs : la v2"), "{:?}", fx);
    assert!(say_to(&fx, MAIN).is_none() && steer_to(&fx, MAIN).is_none());
    assert!(t.hub.st.unanswered_for("docs").is_empty());
    // main's next turn carries the exchange
    let fx = t.user(MAIN, "et ensuite ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(
        s.contains("@docs answered the user (asked from @main's view: \"v1 ou v2 ?\"): \"la v2\""),
        "{}",
        s
    );
    // in the task's own view, it stays a plain user message
    t.turn(MAIN, "ok");
    let fx = t.user("docs", "@docs merci");
    assert_eq!(say_to(&fx, "docs").as_deref(), Some("merci"));
    let fx = t.turn("docs", "de rien");
    assert!(!has_line(&fx, MAIN, "@docs : de rien"));
}

#[test]
fn at_task_to_a_busy_task_steers_and_answers_at_turn_end() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs is still in its first turn
    let fx = t.user(MAIN, "@docs où en es-tu ?");
    let s = steer_to(&fx, "docs").unwrap();
    assert!(s.starts_with("<user_message via=\"main\">"), "{}", s);
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "  obs: assistant: à mi-chemin".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(has_line(&fx, MAIN, "sb msg-you : docs : à mi-chemin"), "{:?}", fx);
    // main still gets its own automatic reply to the brief
    assert!(t.hub.st.unanswered_for("docs").is_empty());
}

#[test]
fn there_is_no_undo_a_route_stays_sent() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs never started its REPL again: simulate it down
    t.hub.force_run("docs", Run::Starting);
    let fx = t.user(MAIN, "@docs change de plan");
    assert!(has_line(&fx, MAIN, "you → @docs : change de plan"));
    // book §13: no undo, the user says the change to main instead
    let fx = t.user(MAIN, "/cancel");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").starts_with("no undo"))), "{:?}", fx);
    let fx = t.go(Input::ReplReady {
        agent: "docs".into(),
    });
    assert!(say_to(&fx, "docs").is_some_and(|s| s.contains("change de plan")), "{:?}", fx);
    let fx = t.user(MAIN, "@nope salut");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("no agent named @nope"))));
}

#[test]
fn dropping_a_worktree_with_work_asks_first() {
    let mut t = T::new();
    let fx = t.user(MAIN, "/new -w fix: corrige le bug");
    assert!(
        has_line(&fx, MAIN, "new agent @fix (worktree sb/fix)"),
        "{:?}",
        fx
    );
    t.go(Input::ReplReady {
        agent: "fix".into(),
    });
    t.go(Input::ReplIdle {
        agent: "fix".into(),
        leftover: false,
    });
    t.env.loss = Loss {
        dirty: 3,
        unpushed: 2,
    };
    let fx = t.user(MAIN, "/drop fix");
    let (id, text) = fx
        .iter()
        .find_map(|e| match e {
            Effect::ToClient { body, .. } if body["ev"] == "confirm" => Some((
                body["id"].as_u64().unwrap(),
                body["text"].as_str().unwrap().to_string(),
            )),
            _ => None,
        })
        .expect("a confirmation");
    assert!(
        text.contains("3 changed files and 2 unpushed commits"),
        "{}",
        text
    );
    let fx = t.go(Input::ClientConfirm {
        client: 1,
        id,
        yes: true,
    });
    assert!(fx.contains(&Effect::Kill {
        agent: "fix".into()
    }));
    assert_eq!(t.env.dropped, vec!["fix".to_string()]);
    assert_eq!(t.status("fix"), Status::Archived);
    assert!(t.hub.st.agents["fix"].snapshot_ref.is_some());
    // an archived worktree task is not revived by a message
    let fx = t.user(MAIN, "@fix encore");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("/restore"))));
    // restore brings it back
    let fx = t.user(MAIN, "/restore fix");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "fix")));
    assert!(!t.hub.st.agents["fix"].ws.dropped);
}

#[test]
fn main_cannot_drop_work_away() {
    let mut t = T::new();
    t.user(MAIN, "/new -w fix: x");
    t.go(Input::ReplReady {
        agent: "fix".into(),
    });
    t.go(Input::ReplIdle {
        agent: "fix".into(),
        leftover: false,
    });
    t.env.loss = Loss {
        dirty: 1,
        unpushed: 0,
    };
    let (tok, fx) = t.req(
        MAIN,
        AgentReq::Drop {
            agent: "fix".into(),
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["dropped"], false);
    let card = t
        .hub
        .st
        .cards
        .values()
        .find(|c| c.kind == "drop")
        .unwrap()
        .id;
    t.user(MAIN, &format!("/answer {} oui", card));
    assert_eq!(t.status("fix"), Status::Archived);
}

#[test]
fn an_escalated_question_is_answered_by_the_user() {
    let mut t = T::new();
    t.spawn_task("docs");
    let (tok, fx) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    assert!(say_to(&fx, MAIN).is_some());
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Card {
            text: "La doc : v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    assert!(
        has_line(&fx, MAIN, "sb card : #1 question @docs"),
        "{:?}",
        fx
    );
    // main ends its turn: no automatic reply, the question is handed over
    let fx = t.turn(MAIN, "J'ai demandé à l'utilisateur.");
    assert!(reply(&fx, tok).is_none());
    // the user answers the card
    let fx = t.user(MAIN, "/answer 1 v2");
    let r = reply(&fx, tok).expect("the task's wait ends with the user's answer");
    assert_eq!(r["from"], USER);
    assert_eq!(r["message"], "v2");
    assert!(t.hub.st.cards.is_empty());
}

#[test]
fn answering_in_the_task_view_answers_its_question() {
    let mut t = T::new();
    t.spawn_task("docs");
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    t.req(
        MAIN,
        AgentReq::Card {
            text: "v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.go(Input::ClientFocus {
        client: 1,
        focus: "docs".into(),
    });
    let fx = t.user("docs", "v2");
    let s = say_to(&fx, "docs").unwrap();
    assert!(
        s.contains("from=\"user\"") && s.contains(&format!("reply_to=\"m_{}\"", id)),
        "{}",
        s
    );
    assert!(t.hub.st.cards.is_empty());
}

#[test]
fn a_crashing_repl_restarts_then_fails() {
    let mut t = T::new();
    t.spawn_task("a");
    for i in 1..=5 {
        let fx = t.go(Input::ReplExited {
            agent: "a".into(),
            crashed: true,
            reason: "boom".into(),
        });
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "a")),
            "crash {}",
            i
        );
    }
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "boom".into(),
    });
    assert!(!fx.iter().any(|e| matches!(e, Effect::Spawn { .. })));
    assert_eq!(t.status("a"), Status::Failed);
    assert!(t.hub.st.cards.values().any(|c| c.kind == "failed"));
    // the user's message revives it
    let fx = t.user(MAIN, "@a réessaie");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "a")));
}

#[test]
fn peers_cannot_reach_an_archived_task_but_the_user_can() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.go(Input::ReplIdle {
        agent: "b".into(),
        leftover: false,
    });
    t.user(MAIN, "/drop b --force");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Send {
            to: "b".into(),
            text: "?".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    assert!(reply(&fx, tok).unwrap()["error"]
        .as_str()
        .unwrap()
        .starts_with("recipient_unavailable"));
    let fx = t.user(MAIN, "@b reviens");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "b")));
}

#[test]
fn worktrees_need_git() {
    let mut t = T::new();
    t.env.git = false;
    let fx = t.user(MAIN, "/new -w x: y");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("not a git repository"))));
    assert!(!t.hub.st.agents.contains_key("x"));
}

#[test]
fn shared_tasks_touching_one_file_open_a_card() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let patch = |f: &str| {
        format!("tool #1 apply_patch : {{\"arg\":\"*** Begin Patch\\n*** Update File: {}\\n@@\\n-x\\n+y\\n*** End Patch\"}}", f)
    };
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: patch("src/x.rs"),
    });
    assert!(t.hub.st.cards.is_empty());
    t.go(Input::ReplLine {
        agent: "b".into(),
        line: patch("src/x.rs"),
    });
    assert!(t
        .hub
        .st
        .cards
        .values()
        .any(|c| c.kind == "overlap" && c.text.contains("src/x.rs")));
    // a task that changed a file cannot be isolated anymore
    let fx = t.user(MAIN, "/isolate a");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("already changed files"))));
}

#[test]
fn a_done_report_updates_the_board_and_wakes_main() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Report {
            kind: "done".into(),
            summary: "fini".into(),
            decisions: vec!["API v2".into()],
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    let s = say_to(&fx, MAIN).expect("main is woken");
    assert!(
        s.contains("[report: done] fini") && s.contains("- API v2"),
        "{}",
        s
    );
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert_eq!(t.status("a"), Status::Done);
    // a new turn clears the declared status
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: "  obs: turn_started".into(),
    });
    assert_eq!(t.status("a"), Status::Working);
}

#[test]
fn only_main_controls_tasks() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Stop {
            agent: "main".into(),
            reason: "x".into(),
        },
    );
    assert!(reply(&fx, tok).unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("reserved for main"));
}

#[test]
fn journal_replay_rebuilds_the_same_state() {
    let mut t = T::new();
    t.spawn_task("a");
    let fx = t.user(MAIN, "/new -w b: autre");
    let mut journal: Vec<Value> = Vec::new();
    // collect every journal event the hub emitted so far, by replaying
    // the scenario with a recorder
    let _ = fx;
    let mut t2 = T::new();
    let mut record = |fx: Vec<Effect>| {
        for e in fx {
            if let Effect::Journal(ev) = e {
                journal.push(ev);
            }
        }
    };
    let (_, fx) = t2.req(
        MAIN,
        AgentReq::Spawn {
            name: "a".into(),
            brief: Brief {
                objective: "objective of a".into(),
                ..Brief::default()
            },
            worktree: false,
            with_changes: false,
        },
    );
    record(fx);
    record(t2.user(MAIN, "/new -w b: autre"));
    let mut h = Hub::new("/w");
    assert!(h.replay(&journal).is_empty(), "sb-core knows every kind it wrote");
    assert_eq!(h.st.order, t2.hub.st.order);
    assert_eq!(h.st.msgs, t2.hub.st.msgs);
    assert_eq!(h.st.agents["b"].ws, t2.hub.st.agents["b"].ws);
}

/// After a /version rollback, the journal can hold an event kind this
/// sb-core does not know: it is not applied, and replay names it (the
/// daemon logs it) instead of losing it in silence.
#[test]
fn replay_returns_the_events_sb_core_does_not_know() {
    let mut h = Hub::new("/w");
    let known = json!({"type": "main_note", "text": "hello", "at_ms": 1});
    let newer = json!({"type": "from_a_newer_hub", "name": "a"});
    let skipped = h.replay(&[known, newer.clone()]);
    assert_eq!(skipped, vec![newer]);
    assert_eq!(h.st.main_notes.len(), 1, "the known event is applied");
}

#[test]
fn a_report_answers_the_parent_and_no_auto_reply_repeats_it() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Report {
            kind: "done".into(),
            summary: "fait".into(),
            decisions: vec![],
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    let s = say_to(&fx, MAIN).unwrap();
    assert!(
        s.contains("reply_to=\"m_"),
        "the report replies to the brief: {}",
        s
    );
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: "  obs: assistant: fait, fichier écrit".into(),
    });
    // main is busy with the report: nothing else may reach it at a's idle
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert!(
        steer_to(&fx, MAIN).is_none() && say_to(&fx, MAIN).is_none(),
        "no duplicate: {:?}",
        fx
    );
}

#[test]
fn steering_the_model_never_read_starts_a_new_turn() {
    let mut t = T::new();
    t.user(MAIN, "premier");
    t.user(MAIN, "question tardive");
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steering_received: question tardive".into(),
    });
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: assistant: réponse au premier".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: false,
    });
    let s = say_to(&fx, MAIN).expect("a new turn");
    assert!(s.contains("not answered them yet"), "{}", s);
    // when the model did read it, nothing happens
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: turn_started".into(),
    });
    t.user(MAIN, "encore");
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steering_received: encore".into(),
    });
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steered: encore".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: false,
    });
    assert!(say_to(&fx, MAIN).is_none(), "{:?}", fx);
}

#[test]
fn main_hears_when_a_task_crashes_and_when_it_fails() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    t.turn(MAIN, "vu");
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "bend: out of memory".into(),
    });
    let s = say_to(&fx, MAIN).expect("main is woken");
    assert!(s.contains("from=\"switchboard\" relation=\"hub\""), "{}", s);
    assert!(
        s.contains("agent @a crashed (bend: out of memory)") && s.contains("attempt 1/5"),
        "{}",
        s
    );
    for _ in 2..=5 {
        t.go(Input::ReplExited {
            agent: "a".into(),
            crashed: true,
            reason: "boom".into(),
        });
    }
    t.turn(MAIN, "vu");
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "boom".into(),
    });
    let s = say_to(&fx, MAIN).expect("main is woken again");
    assert!(s.contains("agent @a failed"), "{}", s);
}

#[test]
fn many_task_messages_always_reach_main() {
    let mut t = T::new();
    t.spawn_task("a");
    for i in 0..30 {
        t.turn(MAIN, "ok");
        let (_, fx) = t.req(
            "a",
            AgentReq::Send {
                to: MAIN.into(),
                text: format!("progress {}", i),
                expect_reply: false,
                reply_to: None,
                queued: false,
                why: String::new(),
            },
        );
        assert!(say_to(&fx, MAIN).is_some(), "message {} held: {:?}", i, fx);
    }
}

#[test]
fn a_user_message_to_main_starts_with_the_task_status() {
    let mut t = T::new();
    let fx = t.user(MAIN, "salut");
    assert_eq!(
        say_to(&fx, MAIN).as_deref(),
        Some("salut"),
        "no task: no status"
    );
    t.turn(MAIN, "ok");
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "tool #3 bash : cargo test --all".into(),
    });
    t.turn(MAIN, "ok");
    let fx = t.user(MAIN, "où en est-on ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(s.starts_with("<task_status>\ndocs working"), "{}", s);
    assert!(s.contains("now: bash `cargo test --all`"), "{}", s);
    assert!(s.ends_with("où en est-on ?"), "{}", s);
    // a task's message to main carries no status block
    let (_, fx) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "fini".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let s = steer_to(&fx, MAIN).or_else(|| say_to(&fx, MAIN)).unwrap();
    assert!(!s.contains("<task_status>"), "{}", s);
}

#[test]
fn sb_tasks_details_every_task() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "tool #3 bash : npm run build".into(),
    });
    t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let (tok, fx) = t.req(MAIN, AgentReq::Tasks);
    let text = reply(&fx, tok).unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.starts_with("## docs — working — shared workspace"),
        "{}",
        text
    );
    assert!(text.contains("objective: objective of docs"), "{}", text);
    assert!(
        text.contains("last activity (0s ago): bash `npm run build`"),
        "{}",
        text
    );
    assert!(text.contains("waits for a reply from main"), "{}", text);
}

#[test]
fn any_number_of_tasks_work_at_once() {
    let mut t = T::new();
    for n in ["a", "b", "c", "d", "e", "f", "g"] {
        t.spawn_task(n);
        assert_eq!(t.status(n), Status::Working, "{} starts right away", n);
    }
}

#[test]
fn agents_talk_as_long_as_they_want() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let mut last: Option<u64> = None;
    for i in 0..60 {
        let (from, to) = if i % 2 == 0 { ("a", "b") } else { ("b", "a") };
        t.go(Input::ReplIdle {
            agent: to.into(),
            leftover: false,
        });
        let (tok, fx) = t.req(
            from,
            AgentReq::Send {
                to: to.into(),
                text: i.to_string(),
                expect_reply: false,
                reply_to: last,
                queued: false,
                why: String::new(),
            },
        );
        assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "message {}", i);
        assert!(
            say_to(&fx, to).is_some(),
            "message {} delivered: {:?}",
            i,
            fx
        );
        last = t.hub.st.msgs.values().last().map(|m| m.id);
    }
    assert!(t.hub.st.cards.is_empty());
}

fn send(t: &mut T, from: &str, to: &str, text: &str, queued: bool) -> (u64, Vec<Effect>) {
    t.req(
        from,
        AgentReq::Send {
            to: to.into(),
            text: text.into(),
            expect_reply: false,
            reply_to: None,
            queued,
            why: String::new(),
        },
    )
}

#[test]
fn a_queued_message_waits_for_the_end_of_the_turn() {
    let mut t = T::new();
    t.spawn_task("a");
    // `a` is busy: the queued message is neither steered nor said
    let (tok, fx) = send(&mut t, MAIN, "a", "later", true);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "queued");
    assert!(
        steer_to(&fx, "a").is_none() && say_to(&fx, "a").is_none(),
        "{:?}",
        fx
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "later")
        .unwrap()
        .id;
    assert!(
        t.hub.st.msgs[&id].queued,
        "the mode is in the journal event"
    );
    // a steer message meanwhile goes in at once, alone
    let (tok, fx) = send(&mut t, MAIN, "a", "now", false);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "delivered");
    let s = steer_to(&fx, "a").expect("steered");
    assert!(s.contains("now") && !s.contains("later"), "{}", s);
    // the turn ends: the queued message starts a new turn
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    let s = say_to(&fx, "a").expect("a new turn");
    assert!(
        s.contains("later") && s.contains(&format!("m_{}", id)),
        "{}",
        s
    );
    assert_eq!(t.hub.st.msg_state[&id], MsgState::Delivered);
}

#[test]
fn a_queued_message_to_an_idle_agent_is_delivered_at_once() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    let (tok, fx) = send(&mut t, MAIN, "a", "go", true);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "delivered");
    assert!(say_to(&fx, "a").unwrap().contains("go"));
}

#[test]
fn a_queued_message_does_not_end_a_wait() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, _) = send(&mut t, "a", MAIN, "question", false);
    let q = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "question")
        .unwrap()
        .id;
    let (wtok, _) = t.req(
        "a",
        AgentReq::Wait {
            msg: q,
            timeout_s: 20,
        },
    );
    let _ = tok;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "a".into(),
            text: "answer".into(),
            expect_reply: false,
            reply_to: Some(q),
            queued: true,
            why: String::new(),
        },
    );
    assert!(
        reply(&fx, wtok).is_none() && steer_to(&fx, "a").is_none(),
        "{:?}",
        fx
    );
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "a").unwrap().contains("answer"));
}

/// Opens a question card for a question of `docs` to main; returns
/// (message id, card id).
fn docs_question_card(t: &mut T) -> (u64, u64) {
    t.spawn_task("docs");
    t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    t.req(
        MAIN,
        AgentReq::Card {
            text: "v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    let card = *t.hub.st.cards.keys().next().unwrap();
    (id, card)
}

#[test]
fn main_replying_to_the_question_closes_its_card() {
    let mut t = T::new();
    let (id, card) = docs_question_card(&mut t);
    // a plain message from main does not close it: the view says so
    t.env.now += 10;
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "patiente".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    assert!(t.hub.st.cards.contains_key(&card));
    let snap = t.hub.snapshot(t.env.now);
    let note = snap["cards"][0]["note"].as_str().unwrap_or("");
    assert!(note.contains("@main wrote to @docs"), "{}", snap);
    // the reply to the question closes it
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
        },
    );
    assert!(t.hub.st.cards.is_empty());
    assert!(
        has_line(&fx, MAIN, &format!("#{} answered via @main", card)),
        "{:?}",
        fx
    );
}

#[test]
fn the_user_closes_a_card_without_answering() {
    let mut t = T::new();
    let (_, card) = docs_question_card(&mut t);
    let fx = t.user(MAIN, &format!("/close {}", card));
    assert!(t.hub.st.cards.is_empty());
    assert!(say_to(&fx, "docs").is_none());
    assert!(has_line(&fx, MAIN, &format!("#{} closed", card)), "{:?}", fx);
}

/// docs asks main (a plain send, not an ask), main answers, docs waits
/// afterwards: (question id, reply id, the fx of main's reply).
fn question_then_reply(t: &mut T, queued: bool) -> (u64, u64) {
    t.spawn_task("docs");
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let q = t.hub.st.msgs.values().find(|m| m.text == "v1 ou v2 ?").unwrap().id;
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(q),
            queued,
            why: String::new(),
        },
    );
    let r = t.hub.st.msgs.values().find(|m| m.text == "v2").unwrap().id;
    (q, r)
}

fn delivered_events(fx: &[Effect], id: u64) -> usize {
    fx.iter()
        .filter(|e| {
            matches!(e, Effect::Journal(ev) if ev["type"] == "message_state" && ev["id"] == id && ev["state"] == "delivered")
        })
        .count()
}

/// Found by the law delivered_once (L3): a reply already delivered (here
/// steered into docs' turn) is read again by `sb wait`, never delivered a
/// second time.
#[test]
fn waiting_for_a_delivered_reply_does_not_deliver_it_twice() {
    let mut t = T::new();
    let (q, r) = question_then_reply(&mut t, false);
    assert!(matches!(t.hub.st.msg_state.get(&r), Some(MsgState::Delivered)));
    let (tok, fx) = t.req("docs", AgentReq::Wait { msg: q, timeout_s: 20 });
    assert_eq!(reply(&fx, tok).expect("the wait returns the reply")["message"], "v2");
    assert_eq!(delivered_events(&fx, r), 0, "{:?}", fx);
}

/// Found by the law delivered_once (L3): a queued-mode reply still queued
/// is never handed to `sb wait`; it arrives as the next turn.
#[test]
fn a_queued_reply_is_not_handed_to_a_wait() {
    let mut t = T::new();
    let (q, r) = question_then_reply(&mut t, true);
    assert!(matches!(t.hub.st.msg_state.get(&r), Some(MsgState::Queued { .. })));
    let (tok, fx) = t.req("docs", AgentReq::Wait { msg: q, timeout_s: 20 });
    assert!(reply(&fx, tok).is_none(), "the wait keeps waiting: {:?}", fx);
    assert_eq!(delivered_events(&fx, r), 0);
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "docs").unwrap_or_default().contains("v2"), "{:?}", fx);
    assert_eq!(delivered_events(&fx, r), 1);
}

fn err_of(fx: &[Effect], tok: u64) -> String {
    reply(fx, tok).unwrap()["error"].as_str().unwrap_or("").to_string()
}

#[test]
fn main_closes_a_card_with_a_note_and_tasks_cannot() {
    let mut t = T::new();
    let (_, card) = docs_question_card(&mut t);
    let (tok, fx) = t.req("docs", AgentReq::Close { card, note: "done".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    assert!(t.hub.st.cards.contains_key(&card));
    let (tok, fx) = t.req(MAIN, AgentReq::Close { card: 99, note: String::new() });
    assert!(err_of(&fx, tok).contains("no open card #99"));
    let (tok, fx) = t.req(MAIN, AgentReq::Close { card, note: "handled".into() });
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    assert!(t.hub.st.cards.is_empty());
    assert!(say_to(&fx, "docs").is_none());
    assert!(has_line(&fx, MAIN, &format!("#{} main: handled", card)), "{:?}", fx);
}

#[test]
fn main_renames_a_task_and_tasks_cannot() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let ren = |a: &str, n: &str| AgentReq::Rename { agent: a.into(), new_name: n.into() };
    let (tok, fx) = t.req("a", ren("b", "c"));
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, ren("a", "b"));
    assert!(err_of(&fx, tok).contains("invalid or taken name"));
    let (tok, fx) = t.req(MAIN, ren("a", "Bad Name"));
    assert!(err_of(&fx, tok).contains("invalid or taken name"));
    let (tok, fx) = t.req(MAIN, ren("main", "boss"));
    assert!(err_of(&fx, tok).contains("no agent named"));
    let (tok, fx) = t.req(MAIN, ren("a", "alpha"));
    assert_eq!(reply(&fx, tok).unwrap()["name"], "alpha");
    assert!(t.hub.st.agents.contains_key("alpha"));
    // the old name still works
    let (tok, fx) = t.req(MAIN, ren("a", "alpha2"));
    assert_eq!(reply(&fx, tok).unwrap()["name"], "alpha2");
}

#[test]
fn main_restores_and_isolates_a_task_and_tasks_cannot() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.go(Input::ReplIdle { agent: "b".into(), leftover: false });
    t.user(MAIN, "/drop b --force");
    assert_eq!(t.status("b"), Status::Archived);
    let (tok, fx) = t.req("a", AgentReq::Restore { agent: "b".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    assert_eq!(t.status("b"), Status::Archived);
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "b".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "b");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "b")));
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "b".into() });
    assert!(err_of(&fx, tok).contains("active"), "{:?}", reply(&fx, tok));
    // isolate: a task cannot, main can (a has changed nothing yet)
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    let (tok, fx) = t.req("b", AgentReq::Isolate { agent: "a".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, AgentReq::Isolate { agent: "a".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "a", "{:?}", fx);
    assert!(t.hub.st.agents["a"].ws.mode == crate::model::Mode::Worktree);
    let (tok, fx) = t.req(MAIN, AgentReq::Isolate { agent: "a".into() });
    assert!(err_of(&fx, tok).contains("already"), "{:?}", reply(&fx, tok));
}

/// Found while stating the law L4 (no message stuck in a queue): a message
/// queued for a task that is renamed before it is delivered reaches the
/// task under its new name.
#[test]
fn a_message_queued_before_a_rename_reaches_the_new_name() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.hub.force_run("docs", Run::Starting);
    t.user(MAIN, "@docs change de plan");
    let id = t.hub.st.msgs.values().find(|m| m.text == "change de plan").unwrap().id;
    assert!(matches!(t.hub.st.msg_state.get(&id), Some(MsgState::Queued { .. })));
    t.user(MAIN, "/rename docs api");
    assert!(t.hub.st.agents.contains_key("api"));
    let fx = t.go(Input::ReplReady { agent: "api".into() });
    assert!(say_to(&fx, "api").unwrap_or_default().contains("change de plan"), "{:?}", fx);
    assert!(matches!(t.hub.st.msg_state.get(&id), Some(MsgState::Delivered)));
}

/// Found by the L4 proof (no message stuck in a queue), also in the Rust
/// origin: a failed task whose REPL is idle keeps its queued mail; a
/// /restore gives it that mail as a new turn.
#[test]
fn a_restored_idle_task_gets_its_queued_mail() {
    let mut t = T::new();
    t.spawn_task("docs");
    // a queued-mode message waits for the end of docs' turn
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "plus tard".into(),
            expect_reply: false,
            reply_to: None,
            queued: true,
            why: String::new(),
        },
    );
    t.req(
        "docs",
        AgentReq::Report {
            kind: "failed".into(),
            summary: "bloqué".into(),
            decisions: vec![],
        },
    );
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "docs").is_none(), "a failed task gets no turn");
    let fx = t.user(MAIN, "/restore docs");
    assert!(say_to(&fx, "docs").unwrap_or_default().contains("plus tard"), "{:?}", fx);
}

// BR-007: a task whose turn fails (network down, provider error) must
// not go quiet: its parent gets the real cause as a report
#[test]
fn a_failed_turn_of_a_task_reaches_main() {
    let mut t = T::new();
    t.spawn_task("net");
    let fx = t.go(Input::ReplLine {
        agent: "net".into(),
        line: "  obs: turn_done: failed: provider failed after 10 attempts: cannot reach api.example.com (connect 61 Connection refused) — check your network or VPN".into(),
    });
    let said = say_to(&fx, MAIN).unwrap_or_default();
    assert!(
        said.contains("[report: turn_failed]") && said.contains("cannot reach api.example.com"),
        "{:?}",
        fx
    );
}

#[test]
fn failed_turn_report_skips_main_and_user_stops() {
    assert!(failed_turn_report("net", "failed: provider 401: bad key").is_some());
    assert!(failed_turn_report("main", "failed: provider 401: bad key").is_none());
    assert!(failed_turn_report("net", "completed").is_none());
    assert!(failed_turn_report("net", "interrupted").is_none());
    assert!(failed_turn_report(
        "net",
        "failed: stopped retrying (interrupted by the user) after 2 failed attempts; last error: x"
    )
    .is_none());
}

// ---- BISE-04: hub line protocol v2 (contract C2) ----

fn send_v2(t: &mut T, from: &str, to: &str, text: &str, expect: bool, reply_to: Option<u64>, why: &str) -> Vec<Effect> {
    let (tok, fx) = t.req(
        from,
        AgentReq::Send {
            to: to.into(),
            text: text.into(),
            expect_reply: expect,
            reply_to,
            queued: false,
            why: why.into(),
        },
    );
    let r = reply(&fx, tok).expect("send answers");
    assert_eq!(r["ok"], true, "{}", r);
    fx
}

fn lines_of<'a>(fx: &'a [Effect], agent: &str) -> Vec<&'a str> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Line { agent: a, line } if a == agent => Some(line.as_str()),
            _ => None,
        })
        .collect()
}

/// Traffic between two tasks reaches main's feed as a `msg` line (level
/// 3), text whole: main no longer only sees what is sent to main.
#[test]
fn peer_traffic_reaches_mains_feed() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let fx = send_v2(&mut t, "a", "b", "can you check logout?\nafter the fix", false, None, "");
    assert!(
        lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : a → b m_") && l.ends_with(" : can you check logout?\\nafter the fix")),
        "{:?}",
        fx
    );
    // the recipient's own feed still reads it as `msg-in`
    assert!(has_line(&fx, "b", "sb msg-in : a m_"), "{:?}", fx);
}

/// Main writing to a task shows in main's feed too; a message TO main
/// shows once, as the `msg-in` of its delivery (no `msg` duplicate).
#[test]
fn main_traffic_in_mains_feed_once() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    let fx = send_v2(&mut t, MAIN, "docs", "use v2", false, None, "");
    assert!(lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : main → docs m_") && l.ends_with(" : use v2")), "{:?}", fx);
    t.turn(MAIN, "ok");
    let fx = send_v2(&mut t, "docs", MAIN, "done", false, None, "");
    let main = lines_of(&fx, MAIN);
    assert!(!main.iter().any(|l| l.starts_with("sb msg : ")), "{:?}", main);
    assert!(main.iter().any(|l| l.starts_with("sb msg-in : docs m_") && l.ends_with(" : done")), "{:?}", main);
}

/// qa-explore bug A: a message steered into a turn that never read it
/// (`leftover`) goes again as a new turn, but its `msg-in` line was
/// written when it was steered: the feed shows it once.
#[test]
fn a_steer_leftover_shows_once_in_the_feed() {
    let mut t = T::new();
    t.spawn_task("talk");
    t.user(MAIN, "go");
    let fx = send_v2(&mut t, "talk", MAIN, "two", false, None, "");
    assert!(steer_to(&fx, MAIN).is_some(), "{:?}", fx);
    let first: Vec<_> = lines_of(&fx, MAIN).into_iter().filter(|l| l.starts_with("sb msg-in : talk m_")).collect();
    assert_eq!(first.len(), 1, "{:?}", fx);
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: true,
    });
    assert!(say_to(&fx, MAIN).is_some_and(|s| s.contains("two")), "sent again: {:?}", fx);
    assert!(!has_line(&fx, MAIN, "sb msg-in : "), "no second msg-in line: {:?}", fx);
}

/// Main answering a task's question: `answered : agent : question :
/// answer : why` in main's feed (level 2), instead of the `msg` line;
/// a " : " inside a field is escaped.
#[test]
fn main_answering_a_question_is_answered() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn(MAIN, "ok");
    let (tok, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 or v2 : which one?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    t.turn(MAIN, "thinking");
    let q = t.hub.st.msgs.values().find(|m| m.from == "docs" && m.to == MAIN).map(|m| m.id).expect("the question");
    let _ = tok;
    let fx = send_v2(&mut t, MAIN, "docs", "v2", false, Some(q), "the brief says v2");
    let main = lines_of(&fx, MAIN);
    assert!(
        main.contains(&"sb answered : docs : v1 or v2 \\: which one? : v2 : the brief says v2"),
        "{:?}",
        main
    );
    assert!(!main.iter().any(|l| l.starts_with("sb msg : ")), "{:?}", main);
    // without --why the field is there, empty
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "and the title?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    let q2 = t.hub.st.msgs.values().filter(|m| m.from == "docs" && m.to == MAIN).map(|m| m.id).max().unwrap();
    let fx = send_v2(&mut t, MAIN, "docs", "keep it", false, Some(q2), "");
    assert!(lines_of(&fx, MAIN).contains(&"sb answered : docs : and the title? : keep it : "), "{:?}", fx);
}

/// A reply of main to a task message that did not ask anything stays a
/// plain `msg` line.
#[test]
fn main_replying_to_a_non_question_is_msg() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn(MAIN, "ok");
    send_v2(&mut t, "docs", MAIN, "fyi: done", false, None, "");
    t.turn(MAIN, "noted");
    let id = t.hub.st.msgs.values().filter(|m| m.from == "docs").map(|m| m.id).max().unwrap();
    let fx = send_v2(&mut t, MAIN, "docs", "thanks", false, Some(id), "");
    assert!(lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : main → docs m_") && l.ends_with(" : thanks")), "{:?}", fx);
}

#[test]
fn fields_escape_the_separator() {
    assert_eq!(field_escape("a : b"), "a \\: b");
    assert_eq!(join_fields(&["docs".into(), "x : y".into(), "z".into(), "".into()]), "docs : x \\: y : z : ");
}

/// A message from the user that cannot reach its agent (C2 amendment,
/// BISE-86): `sb undelivered : {name} : {text}` in the feed where the
/// user wrote it, from the agent's own view or from another one (` : `
/// inside the text escaped); a message queued when the agent stops says
/// it too; an agent's message to it does not.
#[test]
fn a_message_the_user_cannot_deliver_says_so() {
    let mut t = T::new();
    t.user(MAIN, "/new -w fix: corrige le bug");
    // not ready yet: the user's message waits in the queue
    let fx = t.user("fix", "d'abord : les tests");
    assert!(lines_of(&fx, "fix").iter().all(|l| !l.starts_with("sb undelivered")), "{:?}", fx);
    let fx = t.user(MAIN, "/drop fix");
    assert_eq!(t.status("fix"), Status::Archived, "{:?}", fx);
    assert!(
        lines_of(&fx, "fix").contains(&"sb undelivered : fix : d'abord \\: les tests"),
        "{:?}",
        fx
    );
    // the worktree is gone: nothing revives it
    let fx = t.user("fix", "encore");
    assert!(lines_of(&fx, "fix").contains(&"sb undelivered : fix : encore"), "{:?}", fx);
    // from main's view: the line goes to main's feed
    let fx = t.user(MAIN, "@fix et là ?");
    assert!(lines_of(&fx, MAIN).contains(&"sb undelivered : fix : et là ?"), "{:?}", fx);
    assert!(lines_of(&fx, "fix").is_empty(), "{:?}", fx);
    // an agent sending to it gets its error, no undelivered line
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "fix".into(),
            text: "hello".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
        },
    );
    assert!(fx.iter().all(|e| !matches!(e, Effect::Line { line, .. } if line.starts_with("sb undelivered"))), "{:?}", fx);
}

fn ask_role(fx: &[Effect]) -> Option<(String, String, String)> {
    fx.iter().find_map(|e| match e {
        Effect::AskRole { dir, key, request } => Some((dir.clone(), key.clone(), request.clone())),
        _ => None,
    })
}

fn role_of(t: &T, name: &str) -> String {
    let snap = t.hub.snapshot(t.env.now);
    let a = snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == name).unwrap().clone();
    a["role"].as_str().unwrap().to_string()
}

/// BISE-126: a task's role line starts as its objective's first
/// sentence; a turn that changed its report asks once; a turn that
/// changed nothing, main, and a turn during a call do not.
#[test]
fn a_role_line_is_asked_once_per_changed_turn() {
    let mut t = T::new();
    t.spawn_task("docs");
    assert_eq!(role_of(&t, "docs"), "objective of docs");
    assert_eq!(role_of(&t, MAIN), "");
    // main's turns never ask
    assert!(ask_role(&t.turn(MAIN, "hello")).is_none());
    // the task's first turn ends with a report: one call
    let fx = t.turn("docs", "drafted the outline");
    let (dir, key, request) = ask_role(&fx).expect("a call after the turn");
    assert!(request.contains("objective of docs") && request.contains("drafted the outline"), "{}", request);
    // a turn that ends while the call runs: no second call now...
    let fx = t.turn("docs", "wrote chapter one");
    assert!(ask_role(&fx).is_none(), "one call in flight at most: {:?}", fx);
    // ...but one when the first call is over (the inputs changed since)
    let fx = t.go(Input::RoleLine { dir: dir.clone(), key, line: Some("drafting the docs outline".into()) });
    assert_eq!(role_of(&t, "docs"), "drafting the docs outline");
    assert!(fx.contains(&Effect::State), "the views get the new line: {:?}", fx);
    let (_, key2, request2) = ask_role(&fx).expect("the pending look");
    assert!(request2.contains("wrote chapter one"), "{}", request2);
    assert!(request2.contains("drafting the docs outline"), "the line now is in the prompt");
    t.go(Input::RoleLine { dir: dir.clone(), key: key2, line: Some("writing chapter one".into()) });
    // nothing changed since: no call
    t.go(Input::ReplLine { agent: "docs".into(), line: "  obs: turn_started".into() });
    let fx = t.go(Input::ReplIdle { agent: "docs".into(), leftover: false });
    assert!(ask_role(&fx).is_none(), "{:?}", fx);
    // a failed call keeps the old line and waits before the next one
    let fx = t.turn("docs", "wrote chapter two");
    let (_, key3, _) = ask_role(&fx).unwrap();
    t.go(Input::RoleLine { dir: dir.clone(), key: key3, line: None });
    assert_eq!(role_of(&t, "docs"), "writing chapter one");
    assert!(ask_role(&t.turn("docs", "wrote chapter three")).is_none());
    t.env.now += ROLE_RETRY_MS;
    assert!(ask_role(&t.turn("docs", "wrote chapter four")).is_some());
}

/// A line saved by an earlier hub comes back, and its key spares a call.
#[test]
fn a_saved_role_line_is_kept() {
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.turn("docs", "drafted the outline");
    let (dir, key, _) = ask_role(&fx).unwrap();
    let mut t2 = T::new();
    t2.spawn_task("docs");
    t2.hub.load_role(&dir, "drafting the outline".into(), key);
    assert_eq!(role_of(&t2, "docs"), "drafting the outline");
    assert!(ask_role(&t2.turn("docs", "drafted the outline")).is_none());
}

/// BISE-136: `sb worktree <path>` (gate.sh new) puts the private worktree
/// in the snapshot and `sb tasks`; `none` takes it back. Before any
/// `sb worktree`, a bash call that starts in a linked worktree (a `.git`
/// file) is the fallback; after one, the fallback no longer guesses.
#[test]
fn an_agent_says_where_it_works() {
    let mut t = T::new();
    t.spawn_task("docs");
    let place = |t: &T| {
        let snap = t.hub.snapshot(0);
        snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == "docs").unwrap()["place"].clone()
    };
    assert!(place(&t).is_null());
    // the fallback: a linked worktree has a .git file
    let wt = std::env::temp_dir().join(format!("sb-place-{}", std::process::id()));
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(wt.join(".git"), "gitdir: /x").unwrap();
    let wt = wt.to_string_lossy().to_string();
    let bash = |t: &mut T, cmd: &str| {
        t.go(Input::ReplLine {
            agent: "docs".into(),
            line: format!("tool #1 bash : {}", json!({"arg": cmd})),
        })
    };
    bash(&mut t, "cd /tmp && ls");
    assert!(place(&t).is_null(), "not a linked worktree");
    let fx = bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&t), json!(wt));
    assert!(fx.contains(&Effect::State), "the views learn it");
    // told: the fallback stops guessing
    let (tok, fx) = t.req("docs", AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.contains(&Effect::Reply { token: tok, body: json!({"ok": true, "path": "/tmp/docs-wt"}) }), "{:?}", fx);
    assert_eq!(place(&t), json!("/tmp/docs-wt"));
    let (_, fx) = t.req(MAIN, AgentReq::Tasks);
    assert!(format!("{:?}", fx).contains("private worktree /tmp/docs-wt"), "{:?}", fx);
    bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&t), json!("/tmp/docs-wt"));
    // none: its own workspace again
    t.req("docs", AgentReq::Worktree { path: String::new() });
    assert!(place(&t).is_null());
    // the CLI's JSON: an absolute path or none
    let r = |p: &str| AgentReq::from_json(&json!({"cmd": "worktree", "path": p}));
    assert_eq!(r("none"), Ok(AgentReq::Worktree { path: String::new() }));
    assert_eq!(r("/tmp/x-wt/"), Ok(AgentReq::Worktree { path: "/tmp/x-wt".into() }));
    assert!(r("x-wt").is_err());
    // qa-explore B: main works in the workspace, it cannot mark itself
    let (tok, fx) = t.req(MAIN, AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Reply { token, body } if *token == tok && body["ok"] == false)), "{:?}", fx);
    let _ = std::fs::remove_dir_all(&wt);
}


/// BISE-135: `/model` and `/reasoning` choose for the agent in view; the
/// daemon checks and writes (Effect::Choose), nothing goes to a REPL.
#[test]
fn model_and_reasoning_choose_for_the_agent_in_view() {
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.user("docs", "/model anthropic/claude-sonnet-4-5 default");
    let chosen: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Choose { agent, model, effort, default, .. } => {
                Some((agent.clone(), model.clone(), effort.clone(), *default))
            }
            _ => None,
        })
        .collect();
    assert_eq!(chosen, [("docs".to_string(), Some("anthropic/claude-sonnet-4-5".to_string()), None, true)]);
    let fx = t.user(MAIN, "/reasoning low");
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Choose { agent, effort: Some(x), model: None, .. } if agent == MAIN && x == "low")),
        "{:?}",
        fx
    );
    assert!(!fx.iter().any(|e| matches!(e, Effect::Passthrough { .. } | Effect::Say { .. })));
}
