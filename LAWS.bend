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
import ./runtime/tools.bend as Xt
import ./runtime/main.bend as Rt
import ./runtime/mcp.bend as Mc
import ./runtime/skills.bend as Sk
import ./runtime/repl.bend as Rp
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J

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

# ---- multi-line message text (paste) ----
# The client escapes real newlines to keep every transport line
# single-line (socket, wire, checkpoint); the text restores at each
# parse, and the MODEL input carries the real newlines (ser_msg escapes
# instead of flattening - flattening turned a pasted block into one
# space-joined line).

# LAW: the say command line unescapes into the message text.
law say_multiline_unescapes:
  {Rp.plan_say(True{}, "say", "a\\nb")
    == Rp.PRun{Con{T.UserMsg{T.Queue{}, "a
b"}, Nil{}},
      Rp.script_for_entry("a
b")}
    : Rp.Plan}

# LAW: the request wire carries message text on one line, escaped.
law ser_msg_escapes_newlines:
  {Rm.ser_msg(T.Msg{T.RUser{}, "a
b", Nil{}, False{}})
    == "MSG user : a\\nb"
    : String}

# LAW: a multi-line message survives the checkpoint roundtrip.
def ml_flow.started(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(T.new_session(cfg), T.UserMsg{T.Queue{}, "a
b"}),
    T.new_session(cfg))

def ml_flow.done(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(ml_flow.started(cfg), T.CompletionOk{0, "done", Nil{}}),
    ml_flow.started(cfg))

law checkpoint_multiline_roundtrip:
  {K.restore(K.export(ml_flow.done(law_cfg()))) == ml_flow.done(law_cfg())
    : T.Session}

# ---- REPL chunk coalescing (the stub in PROOF) ----

# LAW: a chunk carrying several lines splits into separate commands,
# empty lines dropped - kernel write coalescing must not merge
# messages or swallow the commands after the first.
law chunk_lines_split:
  {Rp.cmd_lines("say a

say b")
    == Con{"say a", Con{"say b", Nil{}}}
    : List<&2, String>}

# LAW: a chunk of only blank lines yields no commands.
law chunk_lines_empty:
  {Rp.cmd_lines("\n\n") == Nil{} : List<&2, String>}

# LAW: the interrupt flag file is per-process (the Ctrl+C path).
law interrupt_path_is_per_port:
  {Rt.interrupt_path_of("7702") == "/tmp/bend-interrupt-7702.txt" : String}

# ---- multi-line tool args on the wire ----

# LAW: the wire arg encoding is reversible on newlines - tool args are
# commands and JSON text, and a lossy escape would corrupt them
# (escape_nl cannot tell an encoded newline from a literal backslash-n).
law wire_args_reversible:
  {Api.wire_decode(Api.wire_encode("one
two")) == "one
two" : String}

# LAW: a literal backslash-n in the arg survives the round-trip (the
# python -c one-liners are full of them).
law wire_args_keep_backslash_n:
  {Api.wire_decode(Api.wire_encode("a\\nb")) == "a\\nb" : String}

# LAW: a literal capital-N sentinel sequence survives too - encoding
# doubles its backslash, decoding restores it exactly.
law wire_args_keep_sentinel:
  {Api.wire_decode(Api.wire_encode("a\\Nb")) == "a\\Nb" : String}

# LAW: the reply wire carries a multi-line arg encoded, on one line.
law reply_encodes_multiline_args:
  {Api.reply_of(200, "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"one\\\\ntwo\\\"}\"}}]}}]}")
    == "OK 
CALL 1 bash : one\\Ntwo
END
" : String}

# LAW: the reply parse gives the arg its real newlines back - the
# heredoc the model wrote is the heredoc bash runs.
law parse_reply_decodes_multiline_args:
  {Rm.parse_reply("OK 
CALL 1 bash : one\\Ntwo
END
")
    == Rm.RROk{"", Con{T.Call{1, "bash", "one
two"}, Nil{}}}
    : Rm.RemoteRes}

# LAW: the request wire encodes multi-line args the same way (the
# session calls ride the wire to every provider round).
law wire_echo_encodes_multiline_args:
  {Rm.ser_call(T.Call{7, "bash", "a
b"})
    == "
  CALL 7 bash : a\\Nb" : String}

# LAW: the request parse decodes the args the history carries.
law api_wire_decodes_args:
  {Api.parse_wire("MODEL default
MSG assistant : 
  CALL 1 bash : one\\Ntwo
END
")
    == Api.WReq{Nil{}, Con{Api.WMsg{"assistant", "",
      Con{Api.WCall{"1", "bash", "one
two"}, Nil{}}}, Nil{}}, None{}}
    : Api.WReq}
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

# ---- steering commits into the durable history (vibe_sdk parity) ----
# vibe_sdk: defer_steer holds the message; state.context.extend(steering)
# commits it at the next agent call, and finish_turn commits it when a
# turn dies first. The Bend core now does both.

# a concrete cfg: the flows go through dispatch, whose fits comparison
# needs a concrete threshold to normalize
def law_cfg() -> T.Cfg:
  T.Cfg{600n, 20000n, 3n, "system prompt", Nil{}}

# the law flows: say hi (turn + first AgentCall pending), steer, then
# complete the call with a tool round -> the dispatch commits the steer
def steer_flow.started(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(T.new_session(cfg), T.UserMsg{T.Queue{}, "hi"}),
    T.new_session(cfg))

def steer_flow.steered(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.UserMsg{T.Steer{}, "penguins"}), steer_flow.started(cfg))

def steer_flow.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.steered(cfg),
      T.CompletionOk{0, "thinking", Con{T.Call{1, "bash", "ls"}, Nil{}}}),
    steer_flow.steered(cfg))

def steer_flow(+cfg: T.Cfg) -> T.Session:
  # the completion renumbers the calls (base = the consumed action ids),
  # so Call{1,...} waits as call id 2
  S.res_next(S.apply(steer_flow.called(cfg), T.ToolOk{2, "out"}),
    steer_flow.called(cfg))

# LAW: a steered user message enters the durable history at the next
# model-safe boundary -- every later model call and the checkpoint
# carry it.
law steering_lands_in_history:
  {S.history_has_user(S.sess_history(steer_flow(law_cfg())), "penguins")
    == True{} : Bool}

# LAW: a turn that dies before its next model call (an interrupt) still
# commits the steering: the client already showed the message, the next
# turn's model input must carry it.
law steering_survives_interrupt:
  {S.history_has_user(
      S.sess_history(S.res_next(
        S.apply(steer_flow.steered(law_cfg()), T.Interrupt{}),
        steer_flow.steered(law_cfg()))), "penguins") == True{} : Bool}

# LAW: a failed turn commits its pending steering the same way.
law steering_survives_fail:
  {S.history_has_user(
      S.sess_history(S.res_next(
        S.apply(steer_flow.steered(law_cfg()), T.FailTurn{"boom"}),
        steer_flow.steered(law_cfg()))), "penguins") == True{} : Bool}

# arrival order: two steers, committed in the order they arrived
def steer_order.one(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.UserMsg{T.Steer{}, "one"}), steer_flow.started(cfg))

def steer_order.two(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.one(cfg),
      T.UserMsg{T.Steer{}, "two"}), steer_order.one(cfg))

def steer_order.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.two(cfg),
      T.CompletionOk{0, "thinking", Con{T.Call{1, "bash", "ls"}, Nil{}}}),
    steer_order.two(cfg))

def steer_order(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.called(cfg), T.ToolOk{2, "out"}),
    steer_order.called(cfg))

# LAW: several steers commit in arrival order, not reversed.
law steering_arrival_order:
  {S.injected_user_texts(S.sess_history(steer_order(law_cfg())))
    == Con{"one", Con{"two", Nil{}}} : List<&2, String>}

# ---- mid-turn steering side-channel (ADR 0005, live) ----
# The client appends steering lines to /tmp/bend-steer-<port>.txt; the
# Runtime drains the file at every model/tool safe boundary (the socket
# is only read between turns). The pure pieces are pinned here.

# LAW: the side-channel path is per-process (the REPL port), so two
# parallel terminals never read each other's steering.
law steer_path_is_per_port:
  {Rt.steer_path_of("7702") == "/tmp/bend-steer-7702.txt" : String}

# LAW: blank lines in the side-channel never steer.
law steer_channel_skips_blank_lines:
  {Rt.drain_steer.skip_go(Con{"a", Con{"", Con{"b", Nil{}}}})
    == Con{"a", Con{"b", Nil{}}} : List<&2, String>}

# LAW: a drained mid-turn steer emits exactly the wire line the client
# shows (steering_received) — never a turn_started: draining must not
# start a new turn, it steers the running one.
law drain_emits_steering_received:
  {Rt.drain_steer.emit_of(
      S.apply(steer_flow.started(law_cfg()), T.UserMsg{T.Steer{}, "penguins"}))
    == Con{"  obs: steering_received: penguins", Nil{}} : List<&2, String>}

def drain_flow.acts(res: T.Res) -> List<&2, T.Act>:
  match res:
    case T.Accepted{_, acts, _}:
      acts
    case T.Rejected{_}:
      Nil{}

# LAW: mid-turn steering produces no acts: the drained line never
# reroutes the running turn's continuation (the drain may drop acts
# safely).
law drain_steering_is_act_free:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.UserMsg{T.Steer{}, "penguins"})) == Nil{} : List<&2, T.Act>}

# LAW: a rejected apply keeps the session (the drain's defensive arm).
law drain_next_of_rejected_keeps_session:
  for +s: T.Session
  {Rt.drain_steer.next_of(s, T.Rejected{"nope"}) == s : T.Session}

# ---- Agent Skills (the Vibe CLI design: progressive disclosure) ----
# The prompt carries name+description+path only; the `skill` tool loads
# the full SKILL.md on exact-name demand.

# LAW: the rules section is the vibe_sdk contract: proactive loading,
# exact names, and no re-reading the SKILL.md root.
law skills_rules_contract:
  {Bool.and(
     String.contains(Sk.skills_rules(), "## Available Skills"),
     Bool.and(
       String.contains(Sk.skills_rules(),
         "proactively call the `skill` tool with its exact name"),
       String.contains(Sk.skills_rules(),
         "Do not read a root `SKILL.md` path with `bash`"))) == True{} : Bool}

# LAW: XML escaping per vibe_sdk prompt.rs: &, <, >, ", '.
law skills_escape_xml:
  {Sk.escape_xml("a<b>&c\"d'e")
    == "a&lt;b&gt;&amp;c&quot;d&apos;e" : String}

# LAW: one index line renders exactly one vibe_sdk catalog entry.
law skills_entry_format:
  {Sk.skill_entry(Sk.Skill{"review", "Review code.", "/skills/review/SKILL.md"})
    == "  <skill>\n    <name>review</name>\n    <description>Review code.</description>\n    <path>/skills/review/SKILL.md</path>\n  </skill>" : String}

# LAW: the catalog section wraps the entries, deduped by name, first
# occurrence winning.
law skills_catalog_dedupes:
  {Sk.skills_catalog("one\td1\t/p1\none\td2\t/p2\ntwo\td3\t/p3")
    == "<available-skills>\n  <skill>\n    <name>one</name>\n    <description>d1</description>\n    <path>/p1</path>\n  </skill>\n  <skill>\n    <name>two</name>\n    <description>d3</description>\n    <path>/p3</path>\n  </skill>\n</available-skills>" : String}

# LAW: an empty index yields no section at all (the prompt stays clean).
law skills_catalog_empty_is_empty:
  {Sk.skills_catalog("") == "" : String}

# LAW: blank or malformed lines never become skills.
law skills_malformed_lines_skipped:
  {Sk.skill_ok_line("") == False{} : Bool}

# LAW: the args are the exact vibe_sdk schema: {"name": "..."}.
law skills_args_name_json:
  {Sk.args_name("{\"name\":\"code-quality\"}") == Some{"code-quality"}
    : Maybe<&2, String>}

# LAW: a missing or empty name is a usage error, never a lookup.
law skills_args_name_rejects_empty:
  {Sk.args_name("{}") == None{} : Maybe<&2, String>}

# LAW: a miss returns the vibe_sdk contract: the name, the guidance,
# and the available list.
law skills_miss_lists_available:
  {Bool.and(
     String.contains(Sk.not_available("bogus", "one\td1\t/p1"),
       "Skill \"bogus\" is not available"),
     String.contains(Sk.not_available("bogus", "one\td1\t/p1"),
       "Available skills: \"one\".")) == True{} : Bool}

# LAW: the live search surface carries the live catalog itself when
# the MCP index is absent (skill and self.* stay discoverable).
law merged_surface_carries_live_catalog:
  {Mc.merged_tools.live(Con{T.ToolDef{"skill", "d"}, Nil{}}, None{})
    == Con{T.ToolDef{"skill", "d"}, Nil{}} : List<&2, T.ToolDef>}

# LAW: an exact name finds the skill (first hit wins).
law skills_find_exact:
  {Sk.find_skill("one\td1\t/p1\ntwo\td2\t/p2", "two")
    == Some{Sk.Skill{"two", "d2", "/p2"}} : Maybe<&2, Sk.Skill>}

# LAW: the tool returns the CONTENT only — the YAML frontmatter
# never reaches the model (vibe_sdk ProjectSkill).
law skills_strip_frontmatter:
  {Sk.strip_frontmatter("---\nname: x\ndescription: y\n---\n\n# Body\nText")
    == "# Body\nText" : String}

# LAW: the prompt only gains the sections when skills exist.
law skills_prompt_gated:
  {Bool.and(
     String.eq(Sk.prompt_with("base", ""), "base"),
     String.contains(Sk.prompt_with("base", "one\td1\t/p1"),
       "<available-skills>")) == True{} : Bool}

# ---- queued messages (ADR 0005's sibling: Queue never supersedes) ----
# A Queue message applied mid-turn is HELD (queue_msg), and the idle
# boundary pops it into the next turn. Before the pop existed, the
# held list was a black hole: nothing consumed it.

def queue_flow.res(+cfg: T.Cfg) -> T.Res:
  S.apply(steer_flow.started(cfg), T.UserMsg{T.Queue{}, "penguins"})

def queue_flow.held(+cfg: T.Cfg) -> T.Session:
  S.res_next(queue_flow.res(cfg), steer_flow.started(cfg))

# LAW: a queued message mid-turn produces no acts — it never
# supersedes the running turn.
law queue_hold_is_act_free:
  {drain_flow.acts(queue_flow.res(law_cfg())) == Nil{} : List<&2, T.Act>}

# LAW: ... and lands in the held list (never in the history yet).
law queue_holds_mid_turn:
  {S.sess_queued(queue_flow.held(law_cfg()))
    == Con{"penguins", Nil{}} : List<&2, String>}

def queue_flow.done(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(queue_flow.held(cfg),
      T.CompletionOk{0, "done", Nil{}}), queue_flow.held(cfg))

# LAW: the turn's end keeps the queue intact.
law queue_survives_turn_end:
  {S.sess_queued(queue_flow.done(law_cfg()))
    == Con{"penguins", Nil{}} : List<&2, String>}

def queue_flow.tr(+cfg: T.Cfg) -> Rt.Trans:
  Rt.transition(Rt.Rt{queue_flow.done(cfg), Nil{}, Nil{}, False{}},
    Rt.JIdle{})

def queue_flow.job_of(t: Rt.Trans) -> Rt.Job:
  match t:
    case Rt.Tr{_, _, job}:
      job

def queue_flow.is_queue_cmd(job: Rt.Job) -> Bool:
  match job:
    case Rt.JCmd{T.UserMsg{T.Queue{}, "penguins"}, Nil{}}:
      True{}
    case _:
      False{}

def queue_flow.tail_of(t: Rt.Trans) -> List<&2, String>:
  match t:
    case Rt.Tr{_, +rt2, _}:
      match rt2:
        case Rt.Rt{+sess, _, _, _}:
          S.sess_queued(sess)

def queue_flow.next_of(t: Rt.Trans) -> T.Session:
  match t:
    case Rt.Tr{_, +rt2, _}:
      match rt2:
        case Rt.Rt{+sess, _, _, _}:
          sess

# LAW: the idle boundary pops the held head into the next turn's user
# message ...
law queue_pops_at_idle:
  {queue_flow.is_queue_cmd(queue_flow.job_of(queue_flow.tr(law_cfg())))
    == True{} : Bool}

# LAW: ... and the popped head is REMOVED from the held list (the tail
# stays queued for later turns; no re-delivery loop).
law queue_pop_removes_head:
  {queue_flow.tail_of(queue_flow.tr(law_cfg())) == Nil{}
    : List<&2, String>}

def queue_flow.delivered(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(queue_flow.next_of(queue_flow.tr(cfg)),
      T.UserMsg{T.Queue{}, "penguins"}),
    queue_flow.next_of(queue_flow.tr(cfg)))

# a PLAIN user text in the history (history_has_user matches only
# injected messages; a queued message starts a turn as a plain one)
def queue_flow.has_user(msgs: List<&2, T.Msg>, +text: String) -> Bool:
  match msgs:
    case Nil{}:
      False{}
    case Con{+m, +rest}:
      match m:
        case T.Msg{T.RUser{}, +t, _, False{}}:
          Bool.or(String.eq(t, text), queue_flow.has_user(rest, text))
        case _:
          queue_flow.has_user(rest, text)

# LAW: the popped message starts the next turn and reaches the
# durable history — the queue is a delivery mechanism, not a hold.
law queue_delivered_in_history:
  {queue_flow.has_user(S.sess_history(queue_flow.delivered(law_cfg())),
    "penguins") == True{} : Bool}

# ---- /reload (the self-improvement loop: restart on the same session) ----

# LAW: /reload maps onto the reload protocol word, sent verbatim.
law slash_reload_maps:
  {Cm.interpret("/reload") == Cm.Send{"reload"} : Cm.Interpretation}

# LAW: the raw word "reload" passes through untouched.
law reload_raw_word:
  {Cm.interpret("reload") == Cm.Send{"reload"} : Cm.Interpretation}

# LAW: a fresh session is between turns: the reload is allowed.
law reload_allowed_when_fresh:
  for +cfg: T.Cfg
  {S.reload_allowed(T.new_session(cfg)) == True{} : Bool}

# LAW: a running turn blocks the reload — the checkpoint only
# serializes turn boundaries (ADR 0002).
law reload_refused_while_turn:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.start_turn(text, T.new_session(cfg)))
    == False{} : Bool}

# LAW: a queued message at idle blocks the reload: it is not
# serialized, it would be lost.
law reload_refused_with_queue:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.queue_msg(T.new_session(cfg), text))
    == False{} : Bool}

# LAW: a held notification at idle blocks the reload: it is not
# serialized, it would be lost.
law reload_refused_with_notif:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.hold_notif(T.new_session(cfg), text))
    == False{} : Bool}

# ---- self.reload / self.compact: the agent files requests mid-turn ----
# Callable as tools (top-level or inside a run_typescript program),
# answered with an ack, executed at the turn boundary by the REPL.

# LAW: self.reload answers with the reload ack — visible to the model.
law self_reload_acks:
  {Rt.self_exec("self.reload") == Rt.Ex{True{}, Rt.RELOAD_ACK()}
    : Rt.ExecRes}

# LAW: self.compact answers with the compact ack.
law self_compact_acks:
  {Rt.self_exec("self.compact") == Rt.Ex{True{}, Rt.COMPACT_ACK()}
    : Rt.ExecRes}

# LAW: the routing recognizes exactly the two self.* tools; self.sleep
# stays a timer and the ordinary tools stay themselves.
law self_tool_routing_table:
  {Bool.and(Rt.is_self_tool("self.reload"),
     Bool.and(Rt.is_self_tool("self.compact"),
       Bool.and(Bool.not(Rt.is_self_tool("self.sleep")),
         Bool.not(Rt.is_self_tool("bash"))))) == True{} : Bool}

# LAW: the acks carry their scan markers — what deferred_of finds is
# exactly what the model read.
law self_acks_carry_markers:
  {Bool.and(String.contains(Rt.RELOAD_ACK(), "reload: scheduled"),
     String.contains(Rt.COMPACT_ACK(), "compact: scheduled"))
    == True{} : Bool}

# LAW: the deferred scan reads the turn that just ended — the tool
# results after the last user message.
law deferred_reads_reload_request:
  {Rt.deferred_of(Con{T.user("hi"), Con{T.tool_res(Rt.RELOAD_ACK()), Nil{}}})
    == Rt.Def{False{}, True{}} : Rt.Def}

law deferred_reads_compact_request:
  {Rt.deferred_of(Con{T.user("hi"), Con{T.tool_res(Rt.COMPACT_ACK()), Nil{}}})
    == Rt.Def{True{}, False{}} : Rt.Def}

law deferred_reads_both_requests:
  {Rt.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Rt.RELOAD_ACK()),
        Con{T.tool_res(Rt.COMPACT_ACK()), Nil{}}}})
    == Rt.Def{True{}, True{}} : Rt.Def}

# LAW: a request older than the last user message is spent — an old
# request can never fire twice.
law deferred_resets_at_user:
  {Rt.deferred_of(Con{T.tool_res(Rt.RELOAD_ACK()),
      Con{T.user("new turn"), Con{T.asst("hi", Nil{}), Nil{}}}})
    == Rt.Def{False{}, False{}} : Rt.Def}

# LAW: no history, no requests.
law deferred_empty_history:
  {Rt.deferred_of(Nil{}) == Rt.Def{False{}, False{}} : Rt.Def}

# LAW: no requests, no suffix on the program result.
law def_acks_suffix_empty_when_none:
  {Rt.def_acks.suffix(Rt.Def{False{}, False{}}) == "" : String}

# LAW: a reload filed inside a program rides its result — even when
# the program returns something else.
law def_acks_suffix_carries_reload:
  {String.contains(Rt.def_acks.suffix(Rt.Def{False{}, True{}}),
     "reload: scheduled") == True{} : Bool}

# LAW: both requests filed in one program both ride the result.
law def_acks_suffix_carries_both:
  {Bool.and(
     String.contains(Rt.def_acks.suffix(Rt.Def{True{}, True{}}),
       "compact: scheduled"),
     String.contains(Rt.def_acks.suffix(Rt.Def{True{}, True{}}),
       "reload: scheduled")) == True{} : Bool}

# LAW: the live catalog carries the skill loader and the two self
# tools after the four parity tools; catalog_short (the scripted
# scenarios) stays untouched.
law live_catalog_has_seven_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k")) == 7n : Nat}

law parity_catalog_keeps_four:
  {Xt.catalog_len(Xt.catalog_pure("s", "r", "b")) == 4n : Nat}

law live_catalog_skill_and_self_names:
  {Bool.and(String.eq(Xt.tool_name_at(4n, Xt.catalog_live("s", "r", "b", "k")),
      "skill"),
    Bool.and(String.eq(Xt.tool_name_at(5n, Xt.catalog_live("s", "r", "b", "k")),
      "self.reload"),
     String.eq(Xt.tool_name_at(6n, Xt.catalog_live("s", "r", "b", "k")),
      "self.compact"))) == True{} : Bool}

# LAW: the descriptions say what the tools do and when they act.
law self_reload_desc_says_when:
  {Bool.and(
     String.contains(Xt.tool_desc_at(5n, Xt.catalog_live("s", "r", "b", "k")),
       "Restart the harness process with the latest code"),
     String.contains(Xt.tool_desc_at(5n, Xt.catalog_live("s", "r", "b", "k")),
       "end of the current turn")) == True{} : Bool}

law self_compact_desc_says_when:
  {Bool.and(
     String.contains(Xt.tool_desc_at(6n, Xt.catalog_live("s", "r", "b", "k")),
       "Compact the conversation history"),
     String.contains(Xt.tool_desc_at(6n, Xt.catalog_live("s", "r", "b", "k")),
       "end of the current turn")) == True{} : Bool}

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
    == "{\"model\":\"zai-glm-5-3\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"bonjour\"}],\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"echo\",\"description\":\"Returns text.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]}}}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: GLM 5.3 runs at HIGH reasoning effort on every call — the vibe
# CLI parity (model thinking "high" maps to reasoning_effort "high"
# on the Mistral API, mistral.py _THINKING_TO_REASONING_EFFORT), and
# the compaction call carries it too.
law api_body_high_reasoning:
  {String.contains(Api.api_body("zai-glm-5-3",
      Api.parse_wire("MODEL default\nMSG user : bonjour\nEND\n")),
    "\"reasoning_effort\":\"high\"") == True{} : Bool}

# LAW: the way back — an assistant message whose history text carries
# marked reasoning is re-sent as the vibe CLI ThinkChunk shape: the
# thinking block first, the text block second.
law asst_think_roundtrip:
  {J.stringify(Api.asst_content_json(
      Api.unescape_nl("<think>2+2 = 4\\n</think>\\n4")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}]},{\"type\":\"text\",\"text\":\"4\"}]"
    : String}

# LAW: a thinking-only assistant message carries no text block.
law asst_think_only_no_text_block:
  {J.stringify(Api.asst_content_json(
      Api.unescape_nl("<think>R\\n</think>")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"R\"}]}]"
    : String}

# LAW: a plain assistant message is sent as a plain string, exactly as
# before — the conversion only fires on marked reasoning.
law asst_plain_content_stays_plain:
  {J.stringify(Api.asst_content_json("hello")) == "\"hello\"" : String}

# LAW: end to end on the request path — a wire carrying a marked
# assistant message produces an API body with the thinking block.
law api_body_returns_thinking_block:
  {String.contains(Api.api_body("m",
      Api.parse_wire("MODEL default\nMSG assistant : <think>2+2 = 4\\n</think>\\n4\nEND\n")),
    "\"type\":\"thinking\"") == True{} : Bool}

# LAW: GLM content blocks become separate lines; the wire carries them
# escaped, the client sees real newlines after unescape.
law glm_blocks_join_lines:
  {Api.unescape_nl(Api.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: GLM thinking blocks surface in the reply too -- the parts of the
# nested thinking array wrapped in think markers, ahead of the text
# blocks. The history the model sees back then carries its own
# reasoning; the common harness bug dropped it here, so every later
# hop of a multi-tool turn reasoned blind.
law glm_thinking_surfaces:
  {Api.unescape_nl(Api.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"closed\":true},{\"type\":\"text\",\"text\":\"4\"}]}}]}"))
    == "OK <think>2+2 = 4\n</think>\n4\nEND\n" : String}

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
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"salut\"}]}"
    : String}


# ---- the on-disk text form (ADR 0002) ----

# LAW: the text form of a checkpoint parses back into exactly that
# checkpoint — roles, injected flags, tool calls, counters and all.
law checkpoint_text_roundtrip:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})} : Maybe<&2, K.Checkpoint>}


# ---- call-id renumbering (core/session.bend) ----
# The wire pairs tool results with calls by id; provider replies number
# calls per completion, so sequential turns once echoed two "call_1" and
# the provider answered 400 invalid_args.

# LAW: calls renumber to sequential ids from the base, name and args
# untouched — ToolAct, PTools and history all agree on ids.
law renumber_is_sequential:
  {S.renumber(Con{T.Call{5, "echo", "hi"}, Con{T.Call{9, "bash", "ls"}, Nil{}}}, 42)
    == Con{T.Call{42, "echo", "hi"}, Con{T.Call{43, "bash", "ls"}, Nil{}}}
    : List<&2, T.Call>}

# LAW: renumbering drops and duplicates nothing.
law renumber_keeps_length:
  for +c1: T.Call
  for +c2: T.Call
  for +base: U32
  {S.calls_count(S.renumber(Con{c1, Con{c2, Nil{}}}, base)) == 2n : Nat}

# ---- the node_program rename never reaches the model ----
# The runtime renames run_typescript to node_program for routing; every
# model-visible surface translates it back (a leak once left the model
# reading a tool it never called).

# LAW: the wire echo of a past call says run_typescript.
law wire_echo_renames_node_program:
  {Rm.ser_call(T.Call{7, "node_program", "{\"code\": \"x\"}"})
    == "\n  CALL 7 run_typescript : {\"code\": \"x\"}" : String}

# LAW: ordinary names pass through the wire echo untouched.
law wire_echo_keeps_names:
  {Rm.ser_call(T.Call{7, "bash", "ls -la"}) == "\n  CALL 7 bash : ls -la"
    : String}

# LAW: tool results are attributed to the name the model called.
law result_text_renames_node_program:
  {S.tool_result_msg("node_program", "out", True{})
    == T.tool_res("tool run_typescript ok: out") : T.Msg}

law result_text_keeps_names:
  {S.tool_result_msg("bash", "out", False{})
    == T.tool_res("tool bash failed: out") : T.Msg}

# ---- structured tool arguments (core/api.bend) ----
# The schemas promise the model exact fields; args must pass through
# un-wrapped for the structured tools and wrapped for the flat ones.

# LAW: run_typescript args pass through as the JSON the model wrote.
law args_json_passes_structured:
  {Api.args_json("run_typescript", "{\"code\": \"x\"}") == "{\"code\": \"x\"}"
    : String}

# LAW: search args pass through too.
law args_json_passes_search:
  {Api.args_json("search_tool_functions", "{\"mode\": \"details\"}")
    == "{\"mode\": \"details\"}" : String}

# LAW: ordinary tools keep the flat {"arg": ...} wrapper.
law args_json_wraps_plain:
  {Api.args_json("bash", "ls -la") == "{\"arg\":\"ls -la\"}" : String}

# LAW: call_code extracts the code field from run_typescript args.
law call_code_extracts:
  {Api.call_code("{\"code\": \"async function main() { return 1; }\"}")
    == "async function main() { return 1; }" : String}

# LAW: args with no code field pass through — the engine fails
# readably instead of the runtime guessing.
law call_code_passthrough:
  {Api.call_code("just text") == "just text" : String}

# ---- the structured search (runtime/tools.bend) ----
# best_match returns ranked names without declarations, details returns
# declarations and NEVER a silent empty, the flat syntax stays for the
# scripted suite.

# LAW: best_match returns the exact function NAME, not its declaration.
law best_match_names_only:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"echo.thing", "a loud description"}, Nil{}})
    == "echo.thing" : String}

# LAW: a two-word match ranks above a one-word match.
law best_match_ranks_words:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing\nother.thing" : String}

# LAW: best_match caps at 20 names and flags that more exist (the
# a21.zz tool must NOT appear).
law best_match_caps_and_flags:
  {Bool.and(
      String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "moreCandidatesAvailable: true"),
      Bool.not(String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "a21.zz")))
    == True{} : Bool}

# LAW: details on a missing name never comes back silently empty — the
# empty message and the missing list (a silent "" once left the model
# retrying blind).
law details_missing_is_loud:
  {Xt.search_reply("{\"mode\": \"details\", \"functions\": [\"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "No exact tool function details loaded. Call best_match/all_connector_capabilities first, then request exact returned function names.\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: details with a found and a missing name shows both parts.
law details_show_found_and_missing:
  {Xt.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"echo.thing\", \"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: the deprecated functionNames alias resolves like functions.
law details_alias_resolves:
  {Xt.search_reply("{\"mode\": \"details\", \"functionNames\": [\"echo.thing\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: the flat scripted syntax still resolves (the suite's hermetic
# path — non-JSON args fall back).
law flat_details_still_works:
  {Xt.search_reply("details: echo.thing",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: a connectors filter restricts best_match to that connector.
law best_match_connectors_filter:
  {Xt.search_reply(
      "{\"mode\": \"best_match\", \"query\": \"thing\", \"connectors\": [\"echo\"]}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing" : String}

# ---- the TUI feed annotations (runtime/main.bend) ----
# One line each, single-line, capped — every client parses these.

# LAW: the tool annotation carries the id and the model-facing name.
law tool_ann_format:
  {Rt.call_ann(T.Call{12, "bash", "ls -la"}) == "tool #12 bash : ls -la"
    : String}

# LAW: node_program is translated on the feed too.
law tool_ann_renames_node_program:
  {Rt.call_ann(T.Call{3, "node_program", "{\"code\": \"x\"}"})
    == "tool #3 run_typescript : {\"code\": \"x\"}" : String}

# LAW: results annotate with the outcome and a preview.
law tool_res_ann_ok:
  {Rt.res_ann(12, Rt.Ex{True{}, "hello"}) == "tool_result #12 ok : hello"
    : String}

law tool_res_ann_fail:
  {Rt.res_ann(12, Rt.Ex{False{}, "boom"}) == "tool_result #12 fail : boom"
    : String}

# LAW: program sub-calls annotate with name and outcome.
law subtool_ann_format:
  {Rt.sub_ann("github_app.get_me", Rt.Ex{True{}, "{}"})
    == "subtool github_app.get_me ok : {}" : String}

# LAW: annotations stay single-line — one wire line each.
law ann_flat_single_line:
  {Bool.not(String.contains(Rt.ann_flat("a\nb"), "\n")) == True{} : Bool}

# LAW: annotations cap at 200 chars (a 210-char payload trims to 200).
law ann_flat_caps_at_200:
  {String.length(Rt.ann_flat("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")) == 200n : Nat}

# LAW: an engine error payload hands its message back — the model
# reads exit-43 errors and retries.
law node_err_extracts_message:
  {Rt.node_err("{\"error\": \"missing required parameter: owner\"}")
    == "missing required parameter: owner" : String}



# ---- MCP connector parity (vibe connector_registry) ----

# LAW: the published alias is a clean identifier (vibe_sdk requires a
# TypeScript identifier for tools.<alias>.<function>): everything
# outside [a-zA-Z0-9_-] flattens to "_", edges strip, "" is "unnamed".
law mcp_alias_normalizes:
  {Bool.and(String.eq(Mc.normalize_name("github app!"), "github_app"),
     Bool.and(String.eq(Mc.normalize_name("--x.y--"), "x_y"),
       Bool.and(String.eq(Mc.normalize_name("ok-name_1"), "ok-name_1"),
         String.eq(Mc.normalize_name(""), "unnamed")))) == True{} : Bool}

# LAW: colliding normalized aliases get _2, _3... (vibe dedupe).
law mcp_alias_dedupes:
  {Bool.and(
     String.eq(Mc.dedupe("github", Nil{}), "github"),
     Bool.and(
       String.eq(Mc.dedupe("github", Con{"github", Nil{}}), "github_2"),
       String.eq(Mc.dedupe("github",
         Con{"github", Con{"github_2", Nil{}}}), "github_3")))
    == True{} : Bool}

# LAW: the bootstrap hits the same endpoint as the vibe CLI, with the
# same params: the web_search builtin is exposed as an MCP tool and
# auth-actionable connectors are included (ready ones index only).
law mcp_bootstrap_url_parity:
  {Bool.and(
     String.contains(Mc.bootstrap_url(),
       "/v1/connectors/bootstrap?supports_mcp=true"),
     Bool.and(
       String.contains(Mc.bootstrap_url(),
         "include_auth_actionable_connectors=true"),
       String.contains(Mc.bootstrap_url(), "builtin_connectors=web_search")))
    == True{} : Bool}

# ---- bash background handoff (runtime/main.bend) ----
# A command that outlives the sync window must never block the turn: it
# detaches under a per-id directory the agent drives with ordinary bash.
# The wrapper is pinned byte for byte — output to a FILE (never a pipe,
# so no orphan can hold the read), the fifo held RDWR (no EOF), the
# reaper writing .rc, the exit-199 contract, the slot cleanup.

# LAW: the wrapper script, byte for byte.
law bg_wrapper_script:
  {Rt.bg_wrap("whoami", "/tmp/bend-bg-0", 30)
    == "BG=/tmp/bend-bg-0\nmkdir -p $BG\ni=0\nwhile ! mkdir $BG/$i.slot 2>/dev/null; do i=$((i+1)); done\nmkfifo $BG/$i.in 2>/dev/null\nexec 9<>$BG/$i.in\ncat > $BG/$i.cmd <<'XQ-10236-EOF'\nwhoami\nXQ-10236-EOF\n( ulimit -f 102400; ( whoami ) > $BG/$i.out 2>&1 <&9 & C=$!; echo $C > $BG/$i.pid; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi ) >/dev/null 2>&1 &\nwhile [ ! -s $BG/$i.pid ] && [ ! -e $BG/$i.rc ]; do\n  if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished during setup: /tmp/bend-bg-0\"; exit 1; fi\n  sleep 0.05\ndone\nc=$(cat $BG/$i.pid 2>/dev/null)\ne=0\nwhile kill -0 $c 2>/dev/null; do\n  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi\n  e=$((e+1))\n  if [ $e -ge 305 ]; then\n    printf '%s\\n' \"BEND-BG-HANDOFF-6f2c9e5b\"\n    printf '%s\\n' \"background: still running after 30s - id $i (dir /tmp/bend-bg-0)\"\n    printf '%s\\n' \"  output: tail -n 50 /tmp/bend-bg-0/$i.out\"\n    printf '%s\\n' \"  stdin (if it reads): printf '%s\\n' 'text' > /tmp/bend-bg-0/$i.in  (a command that reads stdin WAITS for input indefinitely: feed it or kill it)\"\n    printf '%s\\n' \"  status: kill -0 $(cat /tmp/bend-bg-0/$i.pid) && echo RUNNING || cat /tmp/bend-bg-0/$i.rc 2>/dev/null\"\n    printf '%s\\n' \"  kill: kill $(cat /tmp/bend-bg-0/$i.pid)\"\n    rm -f \"$0\"\n    exit 0\n  fi\ndone\nwhile [ ! -e $BG/$i.rc ]; do\n  if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; exit 1; fi\n  sleep 0.05\ndone\ns=$(cat $BG/$i.rc)\ncat $BG/$i.out\nrm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc\nrmdir $BG/$i.slot 2>/dev/null\nrm -f \"$0\"\nexit $s"
    : String}

# LAW: the sync window defaults# LAW: the sync window defaults to 30 seconds and clamps into [0, 3600].
law bg_after_defaults:
  {Rt.bg_after.read(None{}, 30) == 30 : U32}

law bg_after_clamps_high:
  {Rt.bg_after.clamp(4000) == 3600 : U32}

law bg_after_keeps_low:
  {Rt.bg_after.clamp(5) == 5 : U32}

# LAW: the handoff maps to a SUCCESSFUL result: the wrapper prints the
# handoff marker as its first line, exec_of strips it and keeps the
# contract — the turn continues and the agent decides.
law exec_of_background_maps_ok:
  {Rt.exec_of(String.append("0\n", String.append(Rt.HANDOFF_MARKER(),
      "\nbackground: still running")))
    == Rt.Ex{True{}, "background: still running"} : Rt.ExecRes}

# LAW: a command that REALLY exits 199 reports as a failure with its
# code — the handoff is detected by the marker, never by the code
# (once: exit 199 was silently swallowed as a handoff).
law exec_of_real_199_fails:
  {Rt.exec_of("199\nreal output")
    == Rt.Ex{False{}, "exit 199: real output"} : Rt.ExecRes}

# LAW: ordinary failures still report their exit code.
law exec_of_exit_still_fails:
  {Rt.exec_of.pick.zero(False{}, 1, "boom")
    == Rt.Ex{False{}, "exit 1: boom"} : Rt.ExecRes}


# ---- the background battery: solidity and agent ergonomics ----

# LAW: the bg dir is per-REPL, keyed by port — parallel terminals
# never share slots, and a /reload (same port) finds its slots again.
law bg_dir_default_root:
  {Rt.bg_dir_of("/tmp", "7702") == "/tmp/bend-bg-7702" : String}

law bg_dir_custom_root:
  {Rt.bg_dir_of("/var/bg", "3") == "/var/bg/bend-bg-3" : String}

# LAW: one slot = five files in one dir under one id.
law bg_slot_paths:
  {Bool.and(String.eq(Rt.bg_slot("/d", "4", ".out"), "/d/4.out"),
     Bool.and(String.eq(Rt.bg_slot("/d", "4", ".in"), "/d/4.in"),
       Bool.and(String.eq(Rt.bg_slot("/d", "4", ".pid"), "/d/4.pid"),
         Bool.and(String.eq(Rt.bg_slot("/d", "4", ".rc"), "/d/4.rc"),
           String.eq(Rt.bg_slot("/d", "4", ".cmd"), "/d/4.cmd")))))
    == True{} : Bool}

# LAW: the poll loop probes ten times per second of sync window.
# LAW: the poll loop probes fast first (5 x 20ms), then 100ms steps:
# a trivial command costs ~30ms of polling, the window stays ~after.
law bg_poll_iters_table:
  {Rt.bg_poll_iters(30) == 305 : U32}

# LAW: the headline names the window, the slot id and the dir — the
# agent can ls the dir right away.
law bg_headline_names_window_id_dir:
  {Rt.bg_headline("/tmp/bend-bg-0", "3", 30)
    == "background: still running after 30s - id 3 (dir /tmp/bend-bg-0)"
    : String}

# LAW: each contract line is a COMPLETE command on a CONCRETE path —
# the agent copy-pastes, it never resolves a variable.
law bg_tail_cmd_concrete:
  {Rt.bg_tail_cmd("/tmp/bend-bg-0", "3")
    == "  output: tail -n 50 /tmp/bend-bg-0/3.out" : String}

law bg_stdin_cmd_concrete:
  {Bool.and(
     String.eq(Rt.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "  stdin (if it reads): printf '%s\\n' 'text' > /tmp/bend-bg-0/3.in  (a command that reads stdin WAITS for input indefinitely: feed it or kill it)"),
     String.contains(Rt.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "feed it or kill it")) == True{} : Bool}

law bg_status_cmd_concrete:
  {Rt.bg_status_cmd("/tmp/bend-bg-0", "3")
    == "  status: kill -0 $(cat /tmp/bend-bg-0/3.pid) && echo RUNNING || cat /tmp/bend-bg-0/3.rc 2>/dev/null"
    : String}

law bg_kill_cmd_concrete:
  {Rt.bg_kill_cmd("/tmp/bend-bg-0", "3")
    == "  kill: kill $(cat /tmp/bend-bg-0/3.pid)" : String}

# ---- wrapper invariants (structure, not bytes) ----

# LAW: the fifo is held RDWR by the wrapper — never EOF for the
# reader, never blocks at open.
# LAW: the pid-wait also breaks on an existing rc — the runner frees
# the slot when a fast command ends, so the pid file can be ephemeral
# and the wrapper must never wait for a file that came and went.
law bg_wrapper_pid_wait_breaks_on_rc:
  {String.contains(Rt.bg_wrap("x", "/d", 5),
     "while [ ! -s $BG/$i.pid ] && [ ! -e $BG/$i.rc ]; do") == True{}
    : Bool}

law bg_wrapper_fifo_rdrw:
  {String.contains(Rt.bg_wrap("x", "/d", 5), "exec 9<>$BG/$i.in")
    == True{} : Bool}

# LAW: a detached runner is the command's real parent and records the
# TRUE exit code (a sibling cannot wait — that produced 127). The
# record is ownership-checked: the rc lands only if the runner's pid
# still owns the slot.
law bg_wrapper_runner_waits_and_records:
  {Bool.and(
     String.contains(Rt.bg_wrap("x", "/d", 5),
       "C=$!; echo $C > $BG/$i.pid; wait $C; R=$?"),
     String.contains(Rt.bg_wrap("x", "/d", 5), "echo $R > $BG/$i.rc"))
    == True{} : Bool}

# LAW: output goes to a FILE, never a pipe — the orphan-holding-pipe
# bug class (minute-long hangs) is structurally gone.
law bg_wrapper_output_is_a_file:
  {Bool.and(String.contains(Rt.bg_wrap("x", "/d", 5), "> $BG/$i.out 2>&1"),
     Bool.not(String.contains(Rt.bg_wrap("x", "/d", 5), " | ")))
    == True{} : Bool}

# LAW: the wrapper NEVER kills the command — its only kill is the -0
# liveness probe; ending a background command is the agent's decision.
law bg_wrapper_never_kills:
  {Bool.and(Bool.not(String.contains(Rt.bg_wrap("x", "/d", 5), "kill $c")),
     Bool.not(String.contains(Rt.bg_wrap("x", "/d", 5), "kill -9")))
    == True{} : Bool}

# LAW: a command that finishes inside the window leaves no trace —
# every slot file is removed.
# LAW: a background command that ends frees its slot (pid, cmd and
# fifo are removed by the runner; out and rc stay readable) — the bg
# dir cannot grow without bound and slots are recycled.
law bg_wrapper_runner_frees_slot:
  {String.contains(Rt.bg_wrap("x", "/d", 5),
     "rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in") == True{} : Bool}

law bg_wrapper_cleans_slot:
  {String.contains(Rt.bg_wrap("x", "/d", 5),
     "rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc")
    == True{} : Bool}

# LAW: the handoff prints the marker as the wrapper's FIRST output
# line and exits 0 — the marker (not an exit code) is the signal, so a
# command that really exits 199 still reports 199.
law bg_wrapper_handoff_marker:
  {Bool.and(
     String.contains(Rt.bg_wrap("x", "/d", 5),
       String.append("printf '%s\\n' \"", Rt.HANDOFF_MARKER())),
     Bool.not(String.contains(Rt.bg_wrap("x", "/d", 5), "exit 199")))
    == True{} : Bool}

# LAW: both waits carry a vanished-dir guard — an agent that cleans
# its bg dir with rm -rf gets a FAST failure with the dir name, never
# a hung tool.
law bg_setup_guard_names_the_dir:
  {String.contains(Rt.bg_setup_guard("/tmp/bend-bg-0"),
     "the bg dir vanished during setup: /tmp/bend-bg-0") == True{} : Bool}

law bg_rc_guard_names_the_dir:
  {String.contains(Rt.bg_rc_guard("/tmp/bend-bg-0"),
     "if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; exit 1; fi")
    == True{} : Bool}

# LAW: the wrapper embeds both guards (the waits are all bounded by
# the dir check).
# LAW: slot allocation is an atomic mkdir CAS — two wrappers starting
# at the same instant can never take the same slot (once: concurrent
# calls collided on id 0 and corrupted each other's out/rc).
law bg_wrapper_atomic_slot_allocation:
  {String.contains(Rt.bg_wrap("x", "/d", 5),
     "while ! mkdir $BG/$i.slot 2>/dev/null; do i=$((i+1)); done")
    == True{} : Bool}

# LAW: the runner caps the process's file writes (ulimit -f 102400 =
# 50MiB) — an output-looping background command dies at the cap
# instead of filling the disk (once: a 6.4GB .out).
law bg_wrapper_output_capped:
  {String.contains(Rt.bg_wrap("x", "/d", 5), "( ulimit -f 102400; ( ")
    == True{} : Bool}

# LAW: the runner checks it still OWNS the slot before writing the rc
# and freeing the files — a recycled slot cannot be corrupted by the
# previous command's dying runner.
law bg_wrapper_runner_ownership_check:
  {String.contains(Rt.bg_wrap("x", "/d", 5),
     "if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then") == True{}
    : Bool}

# LAW: both exit paths delete the wrapper script ($0) — no more
# +1 leaked script per bash call (the byte law pins both occurrences).
law bg_wrapper_cleans_its_script:
  {String.contains(Rt.bg_wrap("x", "/d", 5), "rm -f \"$0\"") == True{}
    : Bool}

# LAW: multi-line commands stay intact — the heredoc and the runner
# subshell carry them verbatim (flatten once silently joined lines
# with spaces, restructuring scripts).
law bg_wrapper_multiline_command_intact:
  {Bool.and(
     String.contains(Rt.bg_wrap("echo a\necho b", "/d", 5), "echo a\necho b"),
     Bool.not(String.contains(Rt.bg_wrap("echo a\necho b", "/d", 5),
       "echo a echo b"))) == True{} : Bool}

# LAW: the .cmd heredoc sentinel derives from the command's own hash —
# a command cannot contain its own sentinel without containing its
# own hash.
law bg_sentinel_derives_from_command:
  {Bool.and(
     String.eq(Rt.bg_sentinel("whoami"), Rt.bg_sentinel("whoami")),
     Bool.not(String.eq(Rt.bg_sentinel("whoami"),
       Rt.bg_sentinel("whoamiX")))) == True{} : Bool}

law bg_wrapper_has_both_guards:
  {Bool.and(String.contains(Rt.bg_wrap("x", "/d", 5), Rt.bg_setup_guard("/d")),
     String.contains(Rt.bg_wrap("x", "/d", 5), Rt.bg_rc_guard("/d")))
    == True{} : Bool}

# LAW: the wrapper filename derives from the script's own content:
# re-running the same command rewrites IDENTICAL bytes (an orphaned
# wrapper just continues its own script), and the hash is pinned so
# the naming cannot silently degrade.
law bg_script_hash_deterministic:
  {Rt.bg_script_hash("ab") == 684n : Nat}

# LAW: different scripts hash apart — two commands never share a
# wrapper file, so an orphan can never re-read another command's
# bytes when it wakes from a poll sleep.
law bg_script_hash_distinguishes:
  {Bool.not(String.eq(
      Nat.show(Rt.bg_script_hash("ab")),
      Nat.show(Rt.bg_script_hash("cd"))))
    == True{} : Bool}

law bash_script_path_per_command:
  {Bool.and(String.eq(Rt.bash_script_path("7702", "a b"),
      Rt.bash_script_path("7702", "a b")),
     Bool.not(String.eq(Rt.bash_script_path("7702", "a b"),
       Rt.bash_script_path("7702", "a c")))) == True{} : Bool}

# ---- exec_of: the tool result the model actually reads ----

# LAW: exit 0 is a success carrying the output.
law exec_of_success_is_ok:
  {Rt.exec_of("0\nhello") == Rt.Ex{True{}, "hello"} : Rt.ExecRes}

# LAW: an ordinary failure reports its code and output.
law exec_of_failure_carries_code:
  {Rt.exec_of("3\nbad") == Rt.Ex{False{}, "exit 3: bad"} : Rt.ExecRes}

# LAW: a garbage status line fails closed as exit 1 (never a success).
law exec_of_garbage_fails_closed:
  {Rt.exec_of("not-a-code\nx") == Rt.Ex{False{}, "exit 1: x"} : Rt.ExecRes}

# LAW: a result with no status line at all fails closed too.
law exec_of_no_newline_fails_closed:
  {Rt.exec_of("plain") == Rt.Ex{False{}, "exit 1: "} : Rt.ExecRes}


# ---- the tool output cap (runtime/main.bend) ----

# LAW: outputs that fit pass through untouched.
law cap_out_keeps_small:
  {Rt.cap_out("hello") == "hello" : String}

# LAW: the cap limit is 20000.
law cap_out_limit_is_20000:
  {Rt.cap_out_limit() == 20000n : Nat}

# LAW: outputs that fit pass through untouched.
law cap_out_at_keeps_small:
  {Rt.cap_out_at(5n, "12345") == "12345" : String}

# LAW: an oversized output is cut at the limit with the marker.
law cap_out_at_truncates:
  {Rt.cap_out_at(5n, "12345678")
    == "12345  [output truncated at 20000 chars; 3 more chars discarded]"
    : String}

# ---- the provider retry classification (runtime/main.bend) ----

# LAW: transient statuses retry; client errors and success do not.
law retryable_status_table:
  {Bool.and(
      Bool.and(Rt.retryable_status(500), Rt.retryable_status(503)),
      Bool.and(Rt.retryable_status(429),
        Bool.and(Bool.not(Rt.retryable_status(200)),
          Bool.not(Rt.retryable_status(404)))))
    == True{} : Bool}

# LAW: connection loss, DNS and timeout retry; TLS and URL errors do
# not (retrying a TLS failure or a bad URL only wastes the turn).
law retryable_err_table:
  {Bool.and(
      Bool.and(Rt.retryable_err(Http.ErrConnect{1, "refused"}),
        Rt.retryable_err(Http.ErrDns{})),
      Bool.and(Rt.retryable_err(Http.ErrTimeout{}),
        Bool.and(Bool.not(Rt.retryable_err(Http.ErrTls{1, "handshake"})),
          Bool.not(Rt.retryable_err(Http.ErrUrl{}))))
    )
    == True{} : Bool}

# LAW: a transient network failure classifies as a retry with the
# failure text carried for the final error if retries run out.
law attempt_timeout_retries:
  {Rt.attempt_fail2(Http.ErrTimeout{})
    == Rt.ARetry{1500, Rt.api_err(Http.ErrTimeout{})} : Rt.Attempt}

# LAW: a TLS failure classifies as final — no retry.
law attempt_tls_fails:
  {Rt.attempt_fail2(Http.ErrTls{1, "handshake"})
    == Rt.AFail{Http.ErrTls{1, "handshake"}} : Rt.Attempt}
