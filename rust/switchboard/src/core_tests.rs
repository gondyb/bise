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
        },
    );
    let r = reply(&fx, tok).expect("the wait ends");
    assert_eq!(r["type"], "reply");
    assert_eq!(r["message"], "v2");
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
        has_line(&fx, MAIN, "Tu as parlé à @docs (1 message)"),
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
    assert!(has_line(&fx, MAIN, "sb msg-in : @docs : la v2"), "{:?}", fx);
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
    assert!(has_line(&fx, MAIN, "@docs : à mi-chemin"), "{:?}", fx);
    // main still gets its own automatic reply to the brief
    assert!(t.hub.st.unanswered_for("docs").is_empty());
}

#[test]
fn an_explicit_route_can_be_cancelled_before_delivery() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs never started its REPL again: simulate it down
    t.hub.st.agents.get_mut("docs").unwrap().run = Run::Starting;
    let fx = t.user(MAIN, "@docs change de plan");
    assert!(has_line(&fx, MAIN, "toi → @docs : change de plan"));
    let fx = t.user(MAIN, "/cancel");
    assert!(has_line(&fx, MAIN, "routage vers @docs annulé"), "{:?}", fx);
    let fx = t.go(Input::ReplReady {
        agent: "docs".into(),
    });
    assert!(say_to(&fx, "docs").is_none());
    let fx = t.user(MAIN, "@nope salut");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("aucune tâche nommée @nope"))));
}

#[test]
fn dropping_a_worktree_with_work_asks_first() {
    let mut t = T::new();
    let fx = t.user(MAIN, "/new -w fix: corrige le bug");
    assert!(
        has_line(&fx, MAIN, "nouvelle tâche @fix (worktree sb/fix)"),
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
        text.contains("3 fichiers modifiés et 2 commits non poussés"),
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
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("pas un dépôt git"))));
    assert!(t.hub.st.agents.get("x").is_none());
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
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("déjà modifié"))));
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
        .contains("réservé à main"));
}

#[test]
fn journal_replay_rebuilds_the_same_state() {
    let mut t = T::new();
    t.spawn_task("a");
    let fx = t.user(MAIN, "/new -w b: autre");
    let mut journal: Vec<Event> = Vec::new();
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
    h.replay(&journal);
    assert_eq!(h.st.order, t2.hub.st.order);
    assert_eq!(h.st.msgs, t2.hub.st.msgs);
    assert_eq!(h.st.agents["b"].ws, t2.hub.st.agents["b"].ws);
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
        s.contains("Task @a crashed (bend: out of memory)") && s.contains("attempt 1/5"),
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
    assert!(s.contains("Task @a failed"), "{}", s);
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
        },
    );
    assert!(t.hub.st.cards.contains_key(&card));
    let snap = t.hub.snapshot(t.env.now);
    let note = snap["cards"][0]["note"].as_str().unwrap_or("");
    assert!(note.contains("@main a écrit à @docs"), "{}", snap);
    // the reply to the question closes it
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
        },
    );
    assert!(t.hub.st.cards.is_empty());
    assert!(
        has_line(&fx, MAIN, &format!("#{} répondue via @main", card)),
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
    assert!(has_line(&fx, MAIN, &format!("#{} classée", card)), "{:?}", fx);
}
