# LAWS.bend — the harness invariants.
# Open claims; PROOF.bend must close every one. `bend PROOF.bend` is the
# gate: it fails while any law is open or false.
import Base
import ./core/types.bend as T
import ./core/history.bend as H
import ./core/compaction.bend as C
import ./core/checkpoint.bend as K
import ./core/session.bend as S
import ./core/program.bend as P
import ./core/commands.bend as Cm
import ./core/api.bend as Api
import ./core/obs.bend as O
import ./runtime/remote.bend as Rm

# LAW: a checkpoint restores to exactly the session it exported.
law checkpoint_roundtrip:
  for +s: T.Session
  {K.restore(K.export(s)) == s : T.Session}

# LAW: a compaction reply that carries a tool call is malformed, whatever
# its text says (ADR 0011: "any tool call makes the response malformed").
law summary_rejects_tool_calls:
  for +t: String
  for +c: T.Call
  {C.valid_summary(t, Con{c, Nil{}}) == None{} : Maybe<&2, String>}

# LAW: an empty <summary></summary> block is malformed, not accepted.
law empty_block_malformed:
  {C.parse_summary("<summary></summary>") == None{} : Maybe<&2, String>}

# LAW: undelimited non-empty text is accepted as the provider
# compatibility fallback (ADR 0011).
law fallback_accepts_plain_text:
  {C.parse_summary("plain text") == Some{"plain text"} : Maybe<&2, String>}

# LAW: nothing can be dropped from an empty history.
law drop_one_empty:
  {H.drop_one(Nil{}) == None{} : Maybe<&2, List<&2, T.Msg>>}

# LAW: manual compaction instructions travel under their tag.
law manual_instructions_tagged:
  {String.contains(C.compaction_prompt("keep names"),
    "<extra_compaction_instructions>keep names</extra_compaction_instructions>")
    == True{} : Bool}

# LAW: the projected model input always leads with the system message,
# so every provider request keeps the prompt prefix.
law model_input_leads_with_system:
  for +cfg: T.Cfg
  for +h: List<&2, T.Msg>
  {S.head_role(S.model_input(cfg, h)) == Some{T.RSys{}} : Maybe<&2, T.Role>}

# LAW: replay consumes recorded results in order and completes the program
# (D1: a resumed program can never diverge from the original run).
law run_consumes_in_order:
  for +n1: String
  for +a1: String
  for +n2: String
  for +a2: String
  for +r1: String
  for +r2: String
  {P.run(Con{T.SConst{"x", T.ECall{n1, a1}},
      Con{T.SConst{"y", T.ECall{n2, a2}},
      Con{T.SRet{T.ERef{"x", Nil{}}}, Nil{}}}},
    Con{r1, Con{r2, Nil{}}})
    == P.PDone{r1} : P.PR}

# LAW: the program stops exactly at the missing tool result: the durable
# effect boundary.
law run_waits_at_the_missing_result:
  for +n1: String
  for +a1: String
  for +n2: String
  for +a2: String
  for +r1: String
  {P.run(Con{T.SConst{"x", T.ECall{n1, a1}},
      Con{T.SConst{"y", T.ECall{n2, a2}},
      Con{T.SRet{T.ERef{"x", Nil{}}}, Nil{}}}},
    Con{r1, Nil{}})
    == P.PWait{n2, a2} : P.PR}

# LAW: the parser reads the program source into the same statements the
# interpreter runs.
law parse_roundtrip:
  {P.parse("const me = github_app.get_me({}); return me.login;")
    == Some{Con{T.SConst{"me", T.ECall{"github_app.get_me", "{}"}},
      Con{T.SRet{T.ERef{"me", Con{"login", Nil{}}}}, Nil{}}}}
    : Maybe<&2, List<&2, T.Stmt>>}

# LAW: TypeScript type annotations are skipped, not parsed — the
# type-blind parser: models may write TS, the interpreter runs the JS.
law parse_ignores_types:
  {P.parse("const me: UserInfo = github_app.get_me({}); return me;")
    == Some{Con{T.SConst{"me", T.ECall{"github_app.get_me", "{}"}},
      Con{T.SRet{T.ERef{"me", Nil{}}}, Nil{}}}}
    : Maybe<&2, List<&2, T.Stmt>>}

# LAW: malformed programs are refused whole.
law parse_rejects_garbage:
  {P.parse("frobnicate") == None{} : Maybe<&2, List<&2, T.Stmt>>}

# LAW: a branch not taken requests none of its calls — control flow is
# respected across the durable boundary, so replay stays consistent.
law branch_skips_untaken_calls:
  {P.run(Con{T.SConst{"c", T.ECall{"t.c", "{}"}},
      Con{T.SIf{T.CTruth{T.ERef{"c", Con{"ok", Nil{}}}},
        Con{T.SConst{"a", T.ECall{"t.a", "{}"}}, Nil{}},
        Con{T.SConst{"b", T.ECall{"t.b", "{}"}}, Nil{}}},
      Con{T.SRet{T.EStr{"z"}}, Nil{}}}},
    Con{"{\"ok\":true}", Nil{}})
    == P.PWait{"t.a", "{}"} : P.PR}

# ---- the commit convention ----

# LAW: a commit lands at the tail of the history (canonical order is
# oldest-first). A prepend regression — which a live run once exposed
# and the scripted scenarios masked — breaks this immediately.
law commits_append_at_tail:
  {H.last(H.commit(Con{T.user("one"), Con{T.user("two"), Nil{}}},
      T.user("three")))
    == Some{T.user("three")} : Maybe<&2, T.Msg>}

# ---- the wire protocol (runtime/remote.bend, spoken by the bridge) ----

# LAW: a serialized call parses back into the same call — the " : "
# separator and the id format are contract, not convention.
law wire_call_roundtrip:
  {Rm.parse_call("7 echo : hi there") == T.Call{7, "echo", "hi there"}
    : T.Call}

# LAW: a reply parses bottom-up into the completion the Core expects:
# OK text, CALL lines, END.
law wire_reply_roundtrip:
  {Rm.parse_reply("OK hello\nCALL 1 echo : hi\nEND\n")
    == Rm.RROk{"hello", Con{T.Call{1, "echo", "hi"}, Nil{}}}
    : Rm.RemoteRes}

# ---- session-level rejections (the Core accepts nothing uninvited) ----

# LAW: a tool result with no tools pending is rejected — a late or
# orphan result can never touch the session.
law orphan_tool_result_rejected:
  for +cfg: T.Cfg
  {S.apply(T.new_session(cfg), T.ToolOk{0, "out"})
    == T.Rejected{"no pending tool result"} : T.Res}

# LAW: interrupting an idle session is an accepted no-op: the session
# is untouched (only the turn counter moves).
law idle_interrupt_noop:
  for +cfg: T.Cfg
  {S.apply(T.new_session(cfg), T.Interrupt{})
    == T.Accepted{Nil{}, Nil{}, S.bump_inputs(T.new_session(cfg))} : T.Res}

# ---- ADR 0011 selection budget ----

# LAW: the middle-truncation keeps the head, marks the cut, and stays
# within the token budget (4 chars per token).
law truncate_marks_the_cut:
  {H.truncate("abcdefghij", 2n) == "ab[...]" : String}

# LAW: a message that already fits the budget passes through untouched.
law truncate_keeps_small_messages:
  {H.truncate("abc", 2n) == "abc" : String}

# LAW: the compaction selection keeps only non-injected user messages.
law preserve_recent_keeps_users:
  {H.all_user(H.preserve_recent(100n,
      Con{T.asst("hi", Nil{}),
        Con{T.user("u1"),
          Con{T.tool_res("res"),
            Con{T.user("u2"), Nil{}}}}}))
    == True{} : Bool}

# ---- the observation wire format (every client parses this) ----

# LAW: an observation renders as exactly one line with the obs prefix.
law obs_wire_format:
  {O.show_obs(T.Assistant{"bonjour"}) == "  obs: assistant: bonjour"
    : String}

# ---- the command language (core/commands.bend, server-side) ----

# LAW: plain text is an implicit "say" — the reason no client ever
# needs a command prefix.
law implicit_say:
  {Cm.interpret("bonjour le harness") == Cm.Send{"say bonjour le harness"}
    : Cm.Interpretation}

# LAW: raw protocol words pass through verbatim, never re-interpreted.
law raw_words_pass_through:
  {Cm.interpret("say tools demo") == Cm.Send{"say tools demo"}
    : Cm.Interpretation}

# LAW: /compact maps onto the manual compaction protocol word.
law slash_compact_maps:
  {Cm.interpret("/compact") == Cm.Send{"compact"} : Cm.Interpretation}

# LAW: /steer carries its argument to the steering word.
law slash_steer_maps:
  {Cm.interpret("/steer vas-y") == Cm.Send{"steer vas-y"}
    : Cm.Interpretation}

# LAW: /steer without an argument is refused, not sent.
law slash_steer_needs_arg:
  {Cm.interpret("/steer") == Cm.Note{"/steer needs a message"}
    : Cm.Interpretation}

# LAW: /quit (and /exit) map onto the connection-closing word.
law slash_quit_maps:
  {Cm.interpret("/exit") == Cm.Send{"quit"} : Cm.Interpretation}

# LAW: an unknown /command is refused with the name it was given.
law unknown_slash_refused:
  {Cm.interpret("/frobnicate")
    == Cm.Note{"unknown command: /frobnicate — try /help"}
    : Cm.Interpretation}

# ---- the provider API mapping (core/api.bend, formerly the Rust bridge) ----

# LAW: the wire request parses into exactly the tools and messages it
# was built from (oldest-first, calls attached to their message).
law api_wire_parses:
  {Api.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n")
    == Api.WReq{Con{Api.WTool{"echo", "Returns text."}, Nil{}},
      Con{Api.WMsg{"user", "bonjour", Nil{}}, Nil{}}, None{}}
    : Api.WReq}

# LAW: the API body carries the model, the messages, the tools with the
# flat arg schema, and tool_choice auto — byte-for-byte.
law api_body_maps:
  {Api.api_body("zai-glm-5-3",
      Api.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"zai-glm-5-3\",\"messages\":[{\"role\":\"user\",\"content\":\"bonjour\"}],\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"echo\",\"description\":\"Returns text.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]}}}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: GLM content blocks become separate lines (the thinking blocks
# carry no top-level text field, so only text blocks surface); the wire
# carries them escaped, the client sees real newlines after unescape.
law glm_blocks_join_lines:
  {Api.unescape_nl(Api.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: tool calls map onto the CALL reply lines the Core parses (1-based
# ids, the flat arg rehydrated from the arguments JSON string).
law api_tool_calls_map:
  {Api.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\": \\\"hi\\\"}\"}}]}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a provider error becomes the ERROR reply line, flattened and
# capped at 300 chars, with the status.
law api_error_maps:
  {Api.reply_of(401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: {\"error\":\"bad key\"}\nEND\n" : String}

# ---- permissive compaction + newline transport ----

# LAW: an unclosed <summary> block still yields the summary (a
# truncated close tag no longer fails compaction).
law unclosed_summary_falls_back:
  {C.parse_summary("<summary>texte sans fermeture")
    == Some{"texte sans fermeture"} : Maybe<&2, String>}

# LAW: escaping keeps every wire line single-line, whatever the text.
law escape_keeps_lines_single:
  {String.contains(Api.escape_nl("x\ny\nz"), "\n") == False{} : Bool}

# LAW: escaping then unescaping is the identity (concrete).
law escape_roundtrip:
  {Api.unescape_nl(Api.escape_nl("a\nb\n\nc")) == "a\nb\n\nc" : String}

# LAW: a compaction request (no TOOL lines) produces a body WITHOUT
# tools or tool_choice — the model cannot even offer a tool call.
law api_bare_body_omits_tools:
  {Api.api_body("m", Api.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"messages\":[{\"role\":\"user\",\"content\":\"salut\"}]}"
    : String}


# ---- the on-disk text form (ADR 0002) ----

# LAW: the text form of a checkpoint parses back into exactly that
# checkpoint — roles, injected flags, tool calls, counters and all.
law checkpoint_text_roundtrip:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})} : Maybe<&2, K.Checkpoint>}
