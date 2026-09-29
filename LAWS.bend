# LAWS.bend — the harness invariants.
# Open claims; PROOF.bend must close every one. `bend PROOF.bend` is the
# gate: it fails while any law is open or false.
import Base
import ./core/types.bend as T
import ./core/history.bend as H
import ./core/compaction.bend as C
import ./core/checkpoint.bend as K
import ./core/session.bend as S
import ./core/patch.bend as Pa
import ./core/config.bend as Cf
import ./core/commands.bend as Cm
import ./core/api.bend as Api
import ./core/wire.bend as W
import ./core/oai-chat.bend as Oai
import ./core/anthropic.bend as Anth
import ./core/image.bend as Im
import ./core/anth-stream.bend as As
import ./core/obs.bend as O
import ./runtime/remote.bend as Rm
import ./runtime/tools-pure.bend as Xt
import ./runtime/main-pure.bend as Rt
import ./runtime/bash-pure.bend as Sh
import ./runtime/provider-pure.bend as Pv
import ./runtime/selftools-pure.bend as Sf
import ./runtime/settings-pure.bend as St
import ./runtime/mcp-pure.bend as Mc
import ./runtime/skills-pure.bend as Sk
import ./runtime/repl-core-pure.bend as Rp
import ./vendor/json.bend as J
import ./runtime/debug-pure.bend as Dg
import ./runtime/usage-pure.bend as Us
import ./hub/model.bend as HM
import ./hub/cx.bend as HX
import ./hub/core.bend as HC
import ./hub/inv.bend as HI

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

# LAW: the compaction request is [system] ++ history ++ [prompt]: it
# ENDS with the user-role compaction prompt. An idle history ends with
# an assistant message, and providers without assistant prefill reject
# a request that does not end with a user message.
law compaction_request_ends_with_prompt:
  {C.project(T.Cfg{400n, 40n, 3n, "You are precise.", Nil{}},
      Con{T.user("bonjour"), Con{T.asst("réponse", Nil{}), Nil{}}}, "")
    == C.POk{Con{T.sys("You are precise."),
      Con{T.user("bonjour"), Con{T.asst("réponse", Nil{}),
      Con{T.injected_user(C.compaction_prompt("")), Nil{}}}}}}
    : C.PRes}

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
  {Pv.interrupt_path_of("7702") == "/tmp/bend-interrupt-7702.txt" : String}

# ---- multi-line tool args on the wire ----

# LAW: the wire arg encoding is reversible on newlines - tool args are
# commands and JSON text, and a lossy escape would corrupt them
# (escape_nl cannot tell an encoded newline from a literal backslash-n).
law wire_args_reversible:
  {W.wire_decode(W.wire_encode("one
two")) == "one
two" : String}

# LAW: a literal backslash-n in the arg survives the round-trip (the
# python -c one-liners are full of them).
law wire_args_keep_backslash_n:
  {W.wire_decode(W.wire_encode("a\\nb")) == "a\\nb" : String}

# LAW: a literal capital-N sentinel sequence survives too - encoding
# doubles its backslash, decoding restores it exactly.
law wire_args_keep_sentinel:
  {W.wire_decode(W.wire_encode("a\\Nb")) == "a\\Nb" : String}

# LAW: the reply wire carries a multi-line arg encoded, on one line.
law reply_encodes_multiline_args:
  {Oai.reply_of(200, "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"one\\\\ntwo\\\"}\"}}]}}]}")
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
  {W.parse_wire("MODEL default
MSG assistant : 
  CALL 1 bash : one\\Ntwo
END
")
    == W.WReq{Nil{}, Con{W.WMsg{"assistant", "",
      Con{W.WCall{"1", "bash", "one
two"}, Nil{}}}, Nil{}}, None{}}
    : W.WReq}
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

# LAW: the compaction preview stays on one wire line: a summary that
# starts with (or contains) newlines is trimmed and flattened.
law compaction_preview_one_line:
  {O.show_obs(T.CompactionDone{"
Le user
veut ça"})
    == "  obs: compaction_done: Le user veut ça..." : String}

# LAW: the same holds for the in-session form, where a newline travels
# escaped as backslash-n (chars 92, 110).
law compaction_preview_unescapes:
  {O.show_obs(T.CompactionDone{SCon{Chr{92}, SCon{Chr{110}, "Le user"}}})
    == "  obs: compaction_done: Le user..." : String}

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

# LAW: /steer is not a user command — steering is the Enter key while
# a turn runs (the client writes the steer side-channel).
law slash_steer_is_not_a_command:
  {Cm.interpret("/steer vas-y")
    == Cm.Note{"unknown command: /steer — try /help"}
    : Cm.Interpretation}

# LAW: /notify is not a user command — notifications come from the
# system and from background processes, never from the user.
law slash_notify_is_not_a_command:
  {Cm.interpret("/notify salut")
    == Cm.Note{"unknown command: /notify — try /help"}
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

# LAW: a queued message no longer blocks the reload: it is serialized
# (QUEUE lines, v2) and the restarted process pops it into the turn it
# owes, by itself.
law reload_allows_with_queue:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.queue_msg(T.new_session(cfg), text))
    == True{} : Bool}

# LAW: a held notification no longer blocks the reload either: it is
# serialized (NOTIF lines, v2).
law reload_allows_with_notif:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.hold_notif(T.new_session(cfg), text))
    == True{} : Bool}

# ---- self.reload / self.compact: the agent files requests mid-turn ----
# Callable as tools (top-level or inside a run_typescript program),
# answered with an ack, executed at the turn boundary by the REPL.

# LAW: self.reload answers with the reload ack — visible to the model.
law self_reload_acks:
  {Sf.self_exec("self.reload") == Xt.Ex{True{}, Sf.RELOAD_ACK()}
    : Xt.ExecRes}

# LAW: self.compact answers with the compact ack.
law self_compact_acks:
  {Sf.self_exec("self.compact") == Xt.Ex{True{}, Sf.COMPACT_ACK()}
    : Xt.ExecRes}

# LAW: the routing recognizes exactly the two self.* tools; self.sleep
# is a programmatic tool (inside run_typescript programs), the
# ordinary tools stay themselves.
law self_tool_routing_table:
  {Bool.and(Sf.is_self_tool("self.reload"),
     Bool.and(Sf.is_self_tool("self.compact"),
       Bool.and(Bool.not(Sf.is_self_tool("self.sleep")),
         Bool.not(Sf.is_self_tool("bash"))))) == True{} : Bool}


# LAW: self.sleep is programmatic now: the bounds are 0 < s < 20
# (a longer sleep stalls the program round for no benefit).
law sleep_program_bounds:
  {Bool.and(Rt.prog_sleep.bounds(1),
     Bool.and(Bool.not(Rt.prog_sleep.bounds(0)),
       Bool.not(Rt.prog_sleep.bounds(20)))) == True{} : Bool}

# LAW: the seconds parse from the bare argument string (self.sleep(19)
# in a program) when the JSON parse finds nothing.
law sleep_program_secs:
  {Rt.prog_sleep.secs(None{}, "19") == 19 : U32}

# LAW: the acks carry their scan markers — what deferred_of finds is
# exactly what the model read.
law self_acks_carry_markers:
  {Bool.and(String.contains(Sf.RELOAD_ACK(), "reload: scheduled"),
     String.contains(Sf.COMPACT_ACK(), "compact: scheduled"))
    == True{} : Bool}

# LAW: the deferred scan reads the turn that just ended — the tool
# results after the last user message.
law deferred_reads_reload_request:
  {Sf.deferred_of(Con{T.user("hi"), Con{T.tool_res(Sf.RELOAD_ACK()), Nil{}}})
    == Sf.Def{False{}, True{}} : Sf.Def}

law deferred_reads_compact_request:
  {Sf.deferred_of(Con{T.user("hi"), Con{T.tool_res(Sf.COMPACT_ACK()), Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

law deferred_reads_both_requests:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Sf.RELOAD_ACK()),
        Con{T.tool_res(Sf.COMPACT_ACK()), Nil{}}}})
    == Sf.Def{True{}, True{}} : Sf.Def}

# LAW: a request older than the last user message is spent — an old
# request can never fire twice.
law deferred_resets_at_user:
  {Sf.deferred_of(Con{T.tool_res(Sf.RELOAD_ACK()),
      Con{T.user("new turn"), Con{T.asst("hi", Nil{}), Nil{}}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: no history, no requests.
law deferred_empty_history:
  {Sf.deferred_of(Nil{}) == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: no requests, no suffix on the program result.
law def_acks_suffix_empty_when_none:
  {Sf.def_acks.suffix(Sf.Def{False{}, False{}}) == "" : String}

# LAW: a reload filed inside a program rides its result — even when
# the program returns something else.
law def_acks_suffix_carries_reload:
  {String.contains(Sf.def_acks.suffix(Sf.Def{False{}, True{}}),
     "reload: scheduled") == True{} : Bool}

# LAW: both requests filed in one program both ride the result.
law def_acks_suffix_carries_both:
  {Bool.and(
     String.contains(Sf.def_acks.suffix(Sf.Def{True{}, True{}}),
       "compact: scheduled"),
     String.contains(Sf.def_acks.suffix(Sf.Def{True{}, True{}}),
       "reload: scheduled")) == True{} : Bool}

# LAW: the live catalog carries the skill loader, the apply_patch
# editor and the two self tools after the four parity tools;
# catalog_short (the scripted scenarios) stays untouched.
# self.sleep left the catalog: it is a programmatic tool now
# (await self.sleep(n) inside a run_typescript program).
law live_catalog_has_seven_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k", "p")) == 7n : Nat}

law parity_catalog_keeps_three:
  {Xt.catalog_len(Xt.catalog_pure("s", "r", "b")) == 3n : Nat}

law live_catalog_skill_patch_and_self_names:
  {Bool.and(String.eq(Xt.tool_name_at(3n, Xt.catalog_live("s", "r", "b", "k", "p")),
      "skill"),
    Bool.and(String.eq(Xt.tool_name_at(4n, Xt.catalog_live("s", "r", "b", "k", "p")),
      "apply_patch"),
     Bool.and(String.eq(Xt.tool_name_at(5n, Xt.catalog_live("s", "r", "b", "k", "p")),
      "self.reload"),
      String.eq(Xt.tool_name_at(6n, Xt.catalog_live("s", "r", "b", "k", "p")),
       "self.compact")))) == True{} : Bool}

# LAW: apply_patch rides at position 4 with its description argument,
# and the fallback description names the format and the atomicity.
law apply_patch_desc_names_the_format:
  {Bool.and(
     String.eq(Xt.tool_desc_at(4n, Xt.catalog_live("s", "r", "b", "k", "p")), "p"),
     Bool.and(
       String.contains(Xt.patch_desc.read(None{}), "Codex V4A"),
       String.contains(Xt.patch_desc.read(None{}),
         "validated before anything is written"))) == True{} : Bool}

law self_reload_desc_says_when:
  {Bool.and(
     String.contains(Xt.tool_desc_at(5n, Xt.catalog_live("s", "r", "b", "k", "p")),
       "Restart the harness process with the latest code"),
     String.contains(Xt.tool_desc_at(5n, Xt.catalog_live("s", "r", "b", "k", "p")),
       "end of the current turn")) == True{} : Bool}

law self_compact_desc_says_when:
  {Bool.and(
     String.contains(Xt.tool_desc_at(6n, Xt.catalog_live("s", "r", "b", "k", "p")),
       "Compact the conversation history"),
     String.contains(Xt.tool_desc_at(6n, Xt.catalog_live("s", "r", "b", "k", "p")),
       "end of the current turn")) == True{} : Bool}

# ---- the provider API mapping (core/api.bend, formerly the Rust bridge) ----

# LAW: the wire request parses into exactly the tools and messages it
# was built from (oldest-first, calls attached to their message).
law api_wire_parses:
  {W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n")
    == W.WReq{Con{W.WTool{"echo", "Returns text."}, Nil{}},
      Con{W.WMsg{"user", "bonjour", Nil{}}, Nil{}}, None{}}
    : W.WReq}

# LAW: the API body carries the model, the messages, the tools with the
# flat arg schema, and tool_choice auto — byte-for-byte.
law api_body_maps:
  {Oai.api_body("zai-glm-5-3",
      W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"zai-glm-5-3\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"bonjour\"}],\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"echo\",\"description\":\"Returns text.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]}}}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: GLM 5.3 runs at HIGH reasoning effort on every call — the vibe
# CLI parity (model thinking "high" maps to reasoning_effort "high"
# on the Mistral API, mistral.py _THINKING_TO_REASONING_EFFORT), and
# the compaction call carries it too.
law api_body_high_reasoning:
  {String.contains(Oai.api_body("zai-glm-5-3",
      W.parse_wire("MODEL default\nMSG user : bonjour\nEND\n")),
    "\"reasoning_effort\":\"high\"") == True{} : Bool}

# LAW: the way back — an assistant message whose history text carries
# marked reasoning is re-sent as the vibe CLI ThinkChunk shape: the
# thinking block first, the text block second.
law asst_think_roundtrip:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>2+2 = 4\\n</think>\\n4")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}]},{\"type\":\"text\",\"text\":\"4\"}]"
    : String}

# LAW: a thinking-only assistant message carries no text block.
law asst_think_only_no_text_block:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>R\\n</think>")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"R\"}]}]"
    : String}

# LAW: a plain assistant message is sent as a plain string, exactly as
# before — the conversion only fires on marked reasoning.
law asst_plain_content_stays_plain:
  {J.stringify(Oai.asst_content_json("hello")) == "\"hello\"" : String}

# LAW: end to end on the request path — a wire carrying a marked
# assistant message produces an API body with the thinking block.
law api_body_returns_thinking_block:
  {String.contains(Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG assistant : <think>2+2 = 4\\n</think>\\n4\nEND\n")),
    "\"type\":\"thinking\"") == True{} : Bool}

# LAW: GLM content blocks become separate lines; the wire carries them
# escaped, the client sees real newlines after unescape.
law glm_blocks_join_lines:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: GLM thinking blocks surface in the reply too -- the parts of the
# nested thinking array wrapped in think markers, ahead of the text
# blocks. The history the model sees back then carries its own
# reasoning; the common harness bug dropped it here, so every later
# hop of a multi-tool turn reasoned blind.
law glm_thinking_surfaces:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"closed\":true},{\"type\":\"text\",\"text\":\"4\"}]}}]}"))
    == "OK <think>2+2 = 4\n</think>\n4\nEND\n" : String}

# LAW: a GLM thinking block with a signature carries it on the span's
# last line - the same signed transport the Anthropic adapter uses, so
# one extraction serves both providers.
law glm_thinking_signature_surfaces:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]}}]}"))
    == "OK <think>2+2 = 4
BENDSIG::sig</think>
4
END
" : String}

# LAW: a signed span replays as the ThinkChunk shape WITH the signature
# at block level (mistral.py _build_mistral_think_chunk: the metadata
# rides the block, the thinking array stays the nested text parts).
law glm_signed_think_signature_replays:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>2+2 = 4
BENDSIG::sig</think>
4")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]"
    : String}

# LAW: tool calls map onto the CALL reply lines the Core parses (1-based
# ids, the flat arg rehydrated from the arguments JSON string).
law api_tool_calls_map:
  {Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\": \\\"hi\\\"}\"}}]}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a provider error becomes the ERROR reply line, flattened and
# capped at 300 chars, with the status.
law api_error_maps:
  {Oai.reply_of(401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: {\"error\":\"bad key\"}\nEND\n" : String}

# ---- permissive compaction + newline transport ----

# LAW: an unclosed <summary> block still yields the summary (a
# truncated close tag no longer fails compaction).
law unclosed_summary_falls_back:
  {C.parse_summary("<summary>texte sans fermeture")
    == Some{"texte sans fermeture"} : Maybe<&2, String>}

# LAW: escaping keeps every wire line single-line, whatever the text.
law escape_keeps_lines_single:
  {String.contains(W.escape_nl("x\ny\nz"), "\n") == False{} : Bool}

# LAW: escaping then unescaping is the identity (concrete).
law escape_roundtrip:
  {W.unescape_nl(W.escape_nl("a\nb\n\nc")) == "a\nb\n\nc" : String}

# LAW: a compaction request (no TOOL lines) produces a body WITHOUT
# tools or tool_choice — the model cannot even offer a tool call.
law api_bare_body_omits_tools:
  {Oai.api_body("m", W.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"salut\"}]}"
    : String}


# ---- the on-disk text form (ADR 0002) ----

# LAW: the text form of a checkpoint parses back into exactly that
# checkpoint — roles, injected flags, tool calls, counters and all.
law checkpoint_text_roundtrip:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})} : Maybe<&2, K.Checkpoint>}



# LAW: the queue and the held notifications survive the text form too
# (QUEUE/NOTIF lines, wire-encoded) - the message the user queued
# mid-turn is there after a reload, so its turn starts by itself.
law checkpoint_text_roundtrip_queued:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.",
      Con{T.ToolDef{"echo", "Returns text."}, Nil{}}},
    Con{T.user("bonjour"), Nil{}}, 1n, 0, None{}, None{},
    Con{"next question", Nil{}}, Con{"note one", Nil{}}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.",
      Con{T.ToolDef{"echo", "Returns text."}, Nil{}}},
    Con{T.user("bonjour"), Nil{}}, 1n, 0, None{}, None{},
    Con{"next question", Nil{}}, Con{"note one", Nil{}}})}
    : Maybe<&2, K.Checkpoint>}

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

# ---- tool results carry the name the model called ----

# LAW: the wire echo of a past call carries its name as the model wrote it.
law wire_echo_keeps_names:
  {Rm.ser_call(T.Call{7, "bash", "ls -la"}) == "\n  CALL 7 bash : ls -la"
    : String}

# LAW: tool results are attributed to the name the model called.
law result_text_keeps_names:
  {S.tool_result_msg("bash", "out", False{})
    == T.tool_res("tool bash failed: out") : T.Msg}

# ---- structured tool arguments (core/api.bend) ----
# The schemas promise the model exact fields; args must pass through
# un-wrapped for the structured tools and wrapped for the flat ones.

# LAW: run_typescript args pass through as the JSON the model wrote.
law args_json_passes_structured:
  {Oai.args_json("run_typescript", "{\"code\": \"x\"}") == "{\"code\": \"x\"}"
    : String}

# LAW: search args pass through too.
law args_json_passes_search:
  {Oai.args_json("search_tool_functions", "{\"mode\": \"details\"}")
    == "{\"mode\": \"details\"}" : String}

# LAW: ordinary tools keep the flat {"arg": ...} wrapper.
law args_json_wraps_plain:
  {Oai.args_json("bash", "ls -la") == "{\"arg\":\"ls -la\"}" : String}

# LAW: call_code extracts the code field from run_typescript args.
law call_code_extracts:
  {W.call_code("{\"code\": \"async function main() { return 1; }\"}")
    == "async function main() { return 1; }" : String}

# LAW: args with no code field pass through — the engine fails
# readably instead of the runtime guessing.
law call_code_passthrough:
  {W.call_code("just text") == "just text" : String}

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

# LAW: results annotate with the outcome and a preview.
law tool_res_ann_ok:
  {Rt.res_ann(12, Xt.Ex{True{}, "hello"}) == "tool_result #12 ok : hello"
    : String}

law tool_res_ann_fail:
  {Rt.res_ann(12, Xt.Ex{False{}, "boom"}) == "tool_result #12 fail : boom"
    : String}

# LAW: program sub-calls annotate with name and outcome.
law subtool_ann_format:
  {Rt.sub_ann("github_app.get_me", Xt.Ex{True{}, "{}"})
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

# ---- bash background handoff (runtime/bash.bend) ----
# A command that outlives the sync window must never block the turn: it
# detaches under a per-id directory the agent drives with ordinary bash.
# The wrapper is pinned byte for byte — output to a FILE (never a pipe,
# so no orphan can hold the read), the fifo held RDWR (no EOF), the
# reaper writing .rc, the exit-199 contract, the slot cleanup.

# LAW: the wrapper script, byte for byte.
law bg_wrapper_script:
  {Sh.bg_wrap("whoami", "/tmp/bend-bg-0", 30)
    == "BG=/tmp/bend-bg-0
mkdir -p $BG
i=0
f=0
while ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -d $BG/$i.slot ]; then i=$((i+1)); else f=$((f+1)); fi
  if [ $f -ge 20 ]; then echo \"background: cannot create a slot in /tmp/bend-bg-0 (disk full or not writable?)\"; rm -f \"$0\"; exit 1; fi
done
mkfifo $BG/$i.in 2>/dev/null
exec 9<>$BG/$i.in
cat > $BG/$i.cmd <<'XQ-3131618372-EOF'
whoami
XQ-3131618372-EOF
if [ $? -ne 0 ]; then echo \"background: could not write /tmp/bend-bg-0/$i.cmd (disk full?)\"; rm -f $BG/$i.in $BG/$i.cmd; rmdir $BG/$i.slot 2>/dev/null; rm -f \"$0\"; exit 1; fi
( ulimit -f 102400; ( /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 <&9 & C=$!; echo $C > $BG/$i.pid; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi; exit $R ) >/dev/null 2>&1 &
r=$!
e=0
while kill -0 $r 2>/dev/null; do
  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi
  e=$((e+1))
  if [ $e -ge 305 ]; then
    printf '%s\\n' \"BEND-BG-HANDOFF-6f2c9e5b\"
    printf '%s\\n' \"background: still running after 30s - id $i (dir /tmp/bend-bg-0)\"
    printf '%s\\n' \"  output: tail -n 50 /tmp/bend-bg-0/$i.out\"
    printf '%s\\n' \"  stdin (if it reads): printf '%s\\n' 'text' > /tmp/bend-bg-0/$i.in  (a command that reads stdin WAITS for input indefinitely: feed it or kill it)\"
    printf '%s\\n' \"  status: kill -0 $(cat /tmp/bend-bg-0/$i.pid) && echo RUNNING || cat /tmp/bend-bg-0/$i.rc 2>/dev/null\"
    printf '%s\\n' \"  kill: kill $(cat /tmp/bend-bg-0/$i.pid)\"
    if [ ! -s /tmp/bend-bg-0/$i.pid ]; then echo \"background: could not write /tmp/bend-bg-0/$i.pid (disk full?) - the command runs under the runner pid $r\"; fi
    rm -f \"$0\"
    exit 0
  fi
done
wait $r
s=$?
if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi
if [ ! -f /tmp/bend-bg-0/$i.out ]; then echo \"background: could not write /tmp/bend-bg-0/$i.out (disk full?)\"; fi
cat $BG/$i.out 2>/dev/null
if [ ! -s /tmp/bend-bg-0/$i.rc ]; then echo \"background: could not write /tmp/bend-bg-0/$i.rc (disk full?) - the output above may be incomplete\"; fi
rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc 2>/dev/null
rmdir $BG/$i.slot 2>/dev/null
rm -f \"$0\"
exit $s"
    : String}

# LAW: the sync window defaults# LAW: the sync window defaults to 30 seconds and clamps into [0, 3600].
law bg_after_defaults:
  {Sh.bg_after.read(None{}, 30) == 30 : U32}

law bg_after_clamps_high:
  {Sh.bg_after.clamp(4000) == 3600 : U32}

law bg_after_keeps_low:
  {Sh.bg_after.clamp(5) == 5 : U32}

# LAW: the handoff maps to a SUCCESSFUL result: the wrapper prints the
# handoff marker as its first line, exec_of strips it and keeps the
# contract — the turn continues and the agent decides.
law exec_of_background_maps_ok:
  {Sh.exec_of(String.append("0\n", String.append(Sh.HANDOFF_MARKER(),
      "\nbackground: still running")))
    == Xt.Ex{True{}, "background: still running"} : Xt.ExecRes}

# LAW: a command that REALLY exits 199 reports as a failure with its
# code — the handoff is detected by the marker, never by the code
# (once: exit 199 was silently swallowed as a handoff).
law exec_of_real_199_fails:
  {Sh.exec_of("199\nreal output")
    == Xt.Ex{False{}, "exit 199: real output"} : Xt.ExecRes}

# LAW: ordinary failures still report their exit code.
law exec_of_exit_still_fails:
  {Sh.exec_of.pick.zero(False{}, 1, "boom")
    == Xt.Ex{False{}, "exit 1: boom"} : Xt.ExecRes}


# ---- the background battery: solidity and agent ergonomics ----

# LAW: the bg dir is per-REPL, keyed by port — parallel terminals
# never share slots, and a /reload (same port) finds its slots again.
law bg_dir_default_root:
  {Sh.bg_dir_of("/tmp", "7702") == "/tmp/bend-bg-7702" : String}

law bg_dir_custom_root:
  {Sh.bg_dir_of("/var/bg", "3") == "/var/bg/bend-bg-3" : String}

# LAW: one slot = five files in one dir under one id.
law bg_slot_paths:
  {Bool.and(String.eq(Sh.bg_slot("/d", "4", ".out"), "/d/4.out"),
     Bool.and(String.eq(Sh.bg_slot("/d", "4", ".in"), "/d/4.in"),
       Bool.and(String.eq(Sh.bg_slot("/d", "4", ".pid"), "/d/4.pid"),
         Bool.and(String.eq(Sh.bg_slot("/d", "4", ".rc"), "/d/4.rc"),
           String.eq(Sh.bg_slot("/d", "4", ".cmd"), "/d/4.cmd")))))
    == True{} : Bool}

# LAW: the poll loop probes ten times per second of sync window.
# LAW: the poll loop probes fast first (5 x 20ms), then 100ms steps:
# a trivial command costs ~30ms of polling, the window stays ~after.
law bg_poll_iters_table:
  {Sh.bg_poll_iters(30) == 305 : U32}

# LAW: the headline names the window, the slot id and the dir — the
# agent can ls the dir right away.
law bg_headline_names_window_id_dir:
  {Sh.bg_headline("/tmp/bend-bg-0", "3", 30)
    == "background: still running after 30s - id 3 (dir /tmp/bend-bg-0)"
    : String}

# LAW: each contract line is a COMPLETE command on a CONCRETE path —
# the agent copy-pastes, it never resolves a variable.
law bg_tail_cmd_concrete:
  {Sh.bg_tail_cmd("/tmp/bend-bg-0", "3")
    == "  output: tail -n 50 /tmp/bend-bg-0/3.out" : String}

law bg_stdin_cmd_concrete:
  {Bool.and(
     String.eq(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "  stdin (if it reads): printf '%s\\n' 'text' > /tmp/bend-bg-0/3.in  (a command that reads stdin WAITS for input indefinitely: feed it or kill it)"),
     String.contains(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "feed it or kill it")) == True{} : Bool}

law bg_status_cmd_concrete:
  {Sh.bg_status_cmd("/tmp/bend-bg-0", "3")
    == "  status: kill -0 $(cat /tmp/bend-bg-0/3.pid) && echo RUNNING || cat /tmp/bend-bg-0/3.rc 2>/dev/null"
    : String}

law bg_kill_cmd_concrete:
  {Sh.bg_kill_cmd("/tmp/bend-bg-0", "3")
    == "  kill: kill $(cat /tmp/bend-bg-0/3.pid)" : String}

# ---- wrapper invariants (structure, not bytes) ----

# LAW: the fifo is held RDWR by the wrapper — never EOF for the
# reader, never blocks at open.
# LAW: every wait is total — the wrapper polls its own child, the
# runner ($!), and takes the exit code from `wait` (the runner exits
# with it), never from a file. Once: on a full disk the runner could
# not write the .rc, the wrapper waited for it forever, and a bash call
# hung 19 minutes.
law bg_wrapper_waits_on_runner:
  {Bool.and(
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5), "fi; exit $R ) >/dev/null 2>&1 &
r=$!"),
       String.contains(Sh.bg_wrap("x", "/d", 5), "while kill -0 $r 2>/dev/null; do")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5), "wait $r
s=$?"),
       Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5), "while [ ! -e $BG/$i.rc ]"))))
    == True{} : Bool}

# LAW: a slot file the runner could not write is named to the model —
# the call returns, and says why a file is missing.
law bg_wrapper_reports_unwritable_files:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5), Sh.bg_write_note("! -f", "/d", ".out", " (disk full?)")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5), "could not write /d/$i.rc (disk full?)"),
       String.contains(Sh.bg_wrap("x", "/d", 5), Sh.bg_cmd_fail("/d"))))
    == True{} : Bool}

law bg_wrapper_fifo_rdrw:
  {String.contains(Sh.bg_wrap("x", "/d", 5), "exec 9<>$BG/$i.in")
    == True{} : Bool}

# LAW: a detached runner is the command's real parent and records the
# TRUE exit code (a sibling cannot wait — that produced 127). The
# record is ownership-checked: the rc lands only if the runner's pid
# still owns the slot.
law bg_wrapper_runner_waits_and_records:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5),
       "C=$!; echo $C > $BG/$i.pid; wait $C; R=$?"),
     String.contains(Sh.bg_wrap("x", "/d", 5), "echo $R > $BG/$i.rc"))
    == True{} : Bool}

# LAW: output goes to a FILE, never a pipe — the orphan-holding-pipe
# bug class (minute-long hangs) is structurally gone.
law bg_wrapper_output_is_a_file:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5), "> $BG/$i.out 2>&1"),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5), " | ")))
    == True{} : Bool}

# LAW: the wrapper NEVER kills the command — its only kill is the -0
# liveness probe; ending a background command is the agent's decision.
law bg_wrapper_never_kills:
  {Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5), "kill $c")),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5), "kill -9")))
    == True{} : Bool}

# LAW: a command that finishes inside the window leaves no trace —
# every slot file is removed.
# LAW: a background command that ends frees its slot (pid, cmd and
# fifo are removed by the runner; out and rc stay readable) — the bg
# dir cannot grow without bound and slots are recycled.
law bg_wrapper_runner_frees_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5),
     "rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in") == True{} : Bool}

law bg_wrapper_cleans_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5),
     "rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc")
    == True{} : Bool}

# LAW: the handoff prints the marker as the wrapper's FIRST output
# line and exits 0 — the marker (not an exit code) is the signal, so a
# command that really exits 199 still reports 199.
law bg_wrapper_handoff_marker:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5),
       String.append("printf '%s\\n' \"", Sh.HANDOFF_MARKER())),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5), "exit 199")))
    == True{} : Bool}

# LAW: the wrapper carries a vanished-dir guard — an agent that cleans
# its bg dir with rm -rf gets a FAST failure with the dir name, never
# a hung tool.
law bg_rc_guard_names_the_dir:
  {String.contains(Sh.bg_rc_guard("/tmp/bend-bg-0"),
     "if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi")
    == True{} : Bool}

# LAW: the wrapper embeds both guards (the waits are all bounded by
# the dir check).
# LAW: slot allocation is an atomic mkdir CAS — two wrappers starting
# at the same instant can never take the same slot (once: concurrent
# calls collided on id 0 and corrupted each other's out/rc). It is
# bounded: a mkdir failing for another reason than an existing slot
# (full disk, read-only dir) ends the call after 20 tries (once: the
# loop retried forever).
law bg_wrapper_atomic_slot_allocation:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5),
       "while ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -d $BG/$i.slot ]; then i=$((i+1)); else f=$((f+1)); fi"),
     String.contains(Sh.bg_wrap("x", "/d", 5), Sh.bg_slot_fail("/d")))
    == True{} : Bool}

# LAW: the runner caps the process's file writes (ulimit -f 102400 =
# 50MiB) — an output-looping background command dies at the cap
# instead of filling the disk (once: a 6.4GB .out).
law bg_wrapper_output_capped:
  {String.contains(Sh.bg_wrap("x", "/d", 5), "( ulimit -f 102400; ( ")
    == True{} : Bool}

# LAW: the runner checks it still OWNS the slot before writing the rc
# and freeing the files — a recycled slot cannot be corrupted by the
# previous command's dying runner.
law bg_wrapper_runner_ownership_check:
  {String.contains(Sh.bg_wrap("x", "/d", 5),
     "if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then") == True{}
    : Bool}

# LAW: both exit paths delete the wrapper script ($0) — no more
# +1 leaked script per bash call (the byte law pins both occurrences).
law bg_wrapper_cleans_its_script:
  {String.contains(Sh.bg_wrap("x", "/d", 5), "rm -f \"$0\"") == True{}
    : Bool}

# LAW: multi-line commands stay intact — the heredoc and the runner
# subshell carry them verbatim (flatten once silently joined lines
# with spaces, restructuring scripts).
law bg_wrapper_multiline_command_intact:
  {Bool.and(
     String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5), "echo a\necho b"),
     Bool.not(String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5),
       "echo a echo b"))) == True{} : Bool}

# LAW: the .cmd heredoc sentinel derives from the command's own hash —
# a command cannot contain its own sentinel without containing its
# own hash.
law bg_sentinel_derives_from_command:
  {Bool.and(
     String.eq(Sh.bg_sentinel("whoami"), Sh.bg_sentinel("whoami")),
     Bool.not(String.eq(Sh.bg_sentinel("whoami"),
       Sh.bg_sentinel("whoamiX")))) == True{} : Bool}

law bg_wrapper_has_the_guard:
  {String.contains(Sh.bg_wrap("x", "/d", 5), Sh.bg_rc_guard("/d"))
    == True{} : Bool}

# LAW: the wrapper filename derives from the script's own content:
# re-running the same command rewrites IDENTICAL bytes (an orphaned
# wrapper just continues its own script), and the hash is pinned so
# the naming cannot silently degrade.
law bg_script_hash_deterministic:
  {Sh.bg_script_hash("ab") == 1294271946 : U32}

# LAW: the hash is a U32 - code points past 2^20 included (the former
# Nat sum overflowed the runtime's 2^48-1 and killed the REPL).
law bg_script_hash_high_code_points:
  {Sh.bg_script_hash("é—􏿿") == 1694011805 : U32}

# LAW: different scripts hash apart — two commands never share a
# wrapper file, so an orphan can never re-read another command's
# bytes when it wakes from a poll sleep.
law bg_script_hash_distinguishes:
  {Bool.not(String.eq(
      U32.show(Sh.bg_script_hash("ab")),
      U32.show(Sh.bg_script_hash("cd"))))
    == True{} : Bool}

law bash_script_path_per_command:
  {Bool.and(String.eq(Sh.bash_script_path("7702", "a b"),
      Sh.bash_script_path("7702", "a b")),
     Bool.not(String.eq(Sh.bash_script_path("7702", "a b"),
       Sh.bash_script_path("7702", "a c")))) == True{} : Bool}

# ---- exec_of: the tool result the model actually reads ----

# LAW: exit 0 is a success carrying the output.
law exec_of_success_is_ok:
  {Sh.exec_of("0\nhello") == Xt.Ex{True{}, "hello"} : Xt.ExecRes}

# LAW: an ordinary failure reports its code and output.
law exec_of_failure_carries_code:
  {Sh.exec_of("3\nbad") == Xt.Ex{False{}, "exit 3: bad"} : Xt.ExecRes}

# LAW: a garbage status line fails closed as exit 1 (never a success).
law exec_of_garbage_fails_closed:
  {Sh.exec_of("not-a-code\nx") == Xt.Ex{False{}, "exit 1: x"} : Xt.ExecRes}

# LAW: a result with no status line at all fails closed too.
law exec_of_no_newline_fails_closed:
  {Sh.exec_of("plain") == Xt.Ex{False{}, "exit 1: "} : Xt.ExecRes}


# ---- the tool output cap (runtime/tools.bend) ----

# LAW: outputs that fit pass through untouched.
law cap_out_keeps_small:
  {Xt.cap_out("hello") == "hello" : String}

# LAW: the cap limit is 20000.
law cap_out_limit_is_20000:
  {Xt.cap_out_limit() == 20000n : Nat}

# LAW: outputs that fit pass through untouched.
law cap_out_at_keeps_small:
  {Xt.cap_out_at(5n, "12345") == "12345" : String}

# LAW: an oversized output is cut at the limit with the marker.
law cap_out_at_truncates:
  {Xt.cap_out_at(5n, "12345678")
    == "12345  [output truncated at 20000 chars; 3 more chars discarded]"
    : String}

# ---- the provider retry classification (runtime/provider.bend) ----

# LAW: transient statuses retry; client errors and success do not.
law retryable_status_table:
  {Bool.and(
      Bool.and(Pv.retryable_status(500), Pv.retryable_status(503)),
      Bool.and(Pv.retryable_status(429),
        Bool.and(Bool.not(Pv.retryable_status(200)),
          Bool.not(Pv.retryable_status(404)))))
    == True{} : Bool}

# LAW: connection loss, DNS and timeout retry; TLS and URL errors do
# not (retrying a TLS failure or a bad URL only wastes the turn).
law retryable_err_table:
  {Bool.and(
      Bool.and(Pv.retryable_err(Pv.NConnect{1, "refused"}),
        Pv.retryable_err(Pv.NDns{})),
      Bool.and(Pv.retryable_err(Pv.NTimeout{}),
        Bool.and(Bool.not(Pv.retryable_err(Pv.NTls{1, "handshake"})),
          Bool.not(Pv.retryable_err(Pv.NUrl{}))))
    )
    == True{} : Bool}

# LAW: a transient network failure classifies as a retry with the
# failure text carried for the final error if retries run out.
law attempt_timeout_retries:
  {Pv.attempt_err("api.example.com", Pv.NTimeout{})
    == Pv.ARetry{0, "no answer from api.example.com (timeout)"}
    : Pv.Attempt}

# LAW: a TLS failure classifies as final — no retry.
law attempt_tls_fails:
  {Pv.attempt_err("api.example.com", Pv.NTls{1, "handshake"})
    == Pv.AFail{"TLS handshake with api.example.com failed (tls 1 handshake) — check your network or VPN"}
    : Pv.Attempt}

# LAW: the other transient statuses retry too - 529 (Anthropic
# overloaded), 408, 425 - while auth and validation errors do not.
law retryable_status_transient:
  {Bool.and(
      Bool.and(Pv.retryable_status(529), Pv.retryable_status(408)),
      Bool.and(Pv.retryable_status(425),
        Bool.and(Bool.not(Pv.retryable_status(400)),
          Bool.not(Pv.retryable_status(401)))))
    == True{} : Bool}

# LAW: a connection cut mid-response (read, write) and a garbled
# response retry: the request never produced an answer.
law retryable_err_midstream:
  {Bool.and(Pv.retryable_err(Pv.NRead{54, "reset"}),
      Bool.and(Pv.retryable_err(Pv.NWrite{32, "broken pipe"}),
        Pv.retryable_err(Pv.NBad{})))
    == True{} : Bool}

# LAW: ten attempts in total.
law provider_max_attempts:
  {Pv.max_attempts() == 10n : Nat}

# LAW: one read may wait long enough for the longest non-streaming
# reply (32k tokens took 255 s on opus-5.5): 10 minutes.
law provider_step_timeout_covers_long_replies:
  {Pv.step_timeout_ms() == 600000 : U32}

# LAW: the backoff doubles from 1s and caps at 30s; a server hint
# (Retry-After) wins over it.
law provider_backoff_schedule:
  {Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 0n), 1000),
      Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 3n), 8000),
        Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 8n), 30000),
          U32.is_eq(Pv.retry_delay_ms(5000, 2n), 5000))))
    == True{} : Bool}

# LAW: Retry-After is read in seconds, capped at 60s; garbage is no hint.
law provider_retry_after:
  {Bool.and(U32.is_eq(Pv.retry_after_ms(" 12 "), 12000),
      Bool.and(U32.is_eq(Pv.retry_after_ms("600"), 60000),
        U32.is_eq(Pv.retry_after_ms("soon"), 0)))
    == True{} : Bool}

# LAW: a transient status carries the server's Retry-After into the retry.
law provider_529_retries_with_hint:
  {Pv.attempt_status(529, "7", "ERROR provider 529: busy\nEND\n")
    == Pv.ARetry{7000, "provider 529 (transient)"} : Pv.Attempt}

# LAW: a 200 that carries an error body (the proxy's overload answer)
# is a retry, never a final reply.
law provider_200_error_body_retries:
  {Pv.attempt_status(200, "",
      Api.reply_of_for(Api.Anthropic{}, 200,
        "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"))
    == Pv.ARetry{0, "provider 200: Overloaded"} : Pv.Attempt}

# LAW: a good response is the reply itself.
law provider_ok_passes_through:
  {Pv.attempt_status(200, "", "OK hi\nEND\n")
    == Pv.AOk{"OK hi\nEND\n"} : Pv.Attempt}

# LAW: a client error is final: the turn fails on the status and the
# body the style mapper kept - retrying it only wastes the user's time.
law provider_client_error_is_final:
  {Pv.attempt_status(400, "", "ERROR provider 400: bad\nEND\n")
    == Pv.AFail{"provider 400: bad"} : Pv.Attempt}

# LAW: an auth error says what to check.
law provider_auth_error_hint:
  {Pv.attempt_status(401, "", "ERROR provider 401: bad key\nEND\n")
    == Pv.AFail{"provider 401: bad key — check the API key"} : Pv.Attempt}

# LAW (BR-007): a refused connection names the host and the fix; it
# retries (a VPN coming back ends the outage).
law attempt_connect_names_host:
  {Pv.attempt_err(Pv.url_host("https://foundry.example.net/anthropic/v1/messages"),
      Pv.NConnect{61, "Connection refused"})
    == Pv.ARetry{0, "cannot reach foundry.example.net (connect 61 Connection refused) — check your network or VPN"}
    : Pv.Attempt}

# LAW (BR-007): the cleared interrupt flag is an EMPTY file - it is not
# an interrupt; neither is a missing file. Only content is.
law interrupt_flag_needs_content:
  {Bool.and(Bool.not(Pv.interrupt_flag(None{})),
      Bool.and(Bool.not(Pv.interrupt_flag(Some{""})),
        Bool.and(Bool.not(Pv.interrupt_flag(Some{"\n"})),
          Pv.interrupt_flag(Some{"1"}))))
    == True{} : Bool}

# LAW (BR-007): an interrupted retry loop keeps the error that made it
# retry - for every error text.
law interrupted_keeps_last_error:
  for +n: Nat
  for +why: String
  exs head: String
  {Pv.interrupted_text(n, why) == String.append(head, why) : String}

# LAW: retries exhausted: the final error ends with the last cause.
law exhausted_keeps_last_error:
  for +n: Nat
  for +why: String
  exs head: String
  {Pv.exhausted_text(n, why) == String.append(head, why) : String}

# LAW: a pause is slept in 1 s slices, the flag read before each one; an
# interrupt empties what is left.
law retry_wait_slices:
  {Bool.and(Nat.is_eq(Pv.wait_slices(8000), 9n),
      Bool.and(U32.is_eq(Pv.slice_ms(False{}, 2500), 1000),
        Bool.and(U32.is_eq(Pv.left_after(False{}, 2500), 1500),
          Bool.and(U32.is_eq(Pv.slice_ms(True{}, 2500), 0),
            U32.is_eq(Pv.left_after(True{}, 2500), 0)))))
    == True{} : Bool}

# LAW: every retry shows the user where the call stands.
law provider_retry_line:
  {Pv.retry_line(0n, "provider 529 (transient)", 1000)
    == "  obs: provider_retry: 1/10 · provider 529 (transient) · retry in 1s"
    : String}

# LAW: the final error says how many attempts ran and why the last failed.
law provider_exhausted_text:
  {Pv.exhausted_text(10n, "timeout")
    == "provider failed after 10 attempts: timeout" : String}

# ---- apply_patch: the parser and the strict matcher (core/patch.bend) ----
# Codex V4A. The parser is pinned byte-for-byte on concrete patches
# (the kind classifier makes every branch of the mode loop reachable
# and deterministic); the matcher is pinned on hits, misses, and the
# sequential cursor.

# LAW: an Add File section accumulates '+' lines; each line keeps a
# trailing newline (Codex pushes '\n' per line).
law patch_parse_add:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: a.txt\n+one\n+two\n*** End Patch")
    == Some{Con{Pa.PAdd{"a.txt", "one\ntwo\n"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Add File section with zero lines creates an EMPTY file, not
# a lone newline.
law patch_parse_add_empty:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: e.txt\n*** End Patch")
    == Some{Con{Pa.PAdd{"e.txt", ""}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: a Delete File section is one action.
law patch_parse_delete:
  {Pa.parse_patch("*** Begin Patch\n*** Delete File: x\n*** End Patch")
    == Some{Con{Pa.PDelete{"x"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Update File section carries its chunks with exact
# old/new line lists and lengths; the @@ hint line is dropped.
law patch_parse_update:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u.txt\n@@ fn foo\n ctx\n-del\n+add\n*** End Patch")
    == Some{Con{Pa.PUpdate{"u.txt",
      Con{Pa.PChunk{Con{"ctx", Con{"del", Nil{}}}, Con{"ctx", Con{"add", Nil{}}}, 2n, 2n},
        Nil{}}}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: content may open a chunk without @@ (Codex creates the chunk
# implicitly); a lone @@ chunk is skipped, never an action.
law patch_parse_implicit_chunk:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u\n@@\n@@\n a\n-b\n+c\n*** End Patch")
    == Some{Con{Pa.PUpdate{"u",
      Con{Pa.PChunk{Con{"a", Con{"b", Nil{}}}, Con{"a", Con{"c", Nil{}}}, 2n, 2n},
        Nil{}}}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: a multi-section patch parses to its actions in order (Add,
# Update, Delete here - the list carries them reversed by the fold,
# Delete last-added first-shown).
law patch_parse_multi:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** Update File: u\n@@\n a\n-b\n+B\n*** Delete File: d\n*** End Patch")
    == Some{Con{Pa.PAdd{"n", "hi\n"}, Con{Pa.PUpdate{"u", Con{Pa.PChunk{Con{"a", Con{"b", Nil{}}}, Con{"a", Con{"B", Nil{}}}, 2n, 2n}, Nil{}}}, Con{Pa.PDelete{"d"}, Nil{}}}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

law patch_rejects_missing_end:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: a\n+x") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the first line must be '*** Begin Patch'.
law patch_rejects_bad_first:
  {Pa.parse_patch("hello\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Update File section with no chunk is malformed (Codex:
# "Update file hunk for path is empty").
law patch_rejects_empty_update:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: garbage at the section level is rejected loudly.
law patch_rejects_garbage_section:
  {Pa.parse_patch("*** Begin Patch\nwhat is this\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: non-empty content after '*** End Patch' is rejected (Codex:
# the last line must be End Patch).
law patch_rejects_after_end:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** End Patch\njunk") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the whole patch is trimmed first - a trailing newline after
# End Patch is fine.
law patch_tolerates_trailing_newline:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** End Patch\n")
    == Some{Con{Pa.PAdd{"n", "hi\n"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the classifier is the parser's spine - every kind pinned.
law patch_kind_add_line:
  {Pa.p_kind_of("+x") == Pa.KAdd{} : Pa.PKind}

law patch_kind_del_line:
  {Pa.p_kind_of("-x") == Pa.KDel{} : Pa.PKind}

law patch_kind_ctx_line:
  {Pa.p_kind_of(" x") == Pa.KCtx{} : Pa.PKind}

law patch_kind_empty_line:
  {Pa.p_kind_of("") == Pa.KEmpty{} : Pa.PKind}

law patch_kind_begin:
  {Pa.p_kind_of("*** Begin Patch ") == Pa.KBegin{} : Pa.PKind}

law patch_kind_end:
  {Pa.p_kind_of("*** End Patch") == Pa.KEnd{} : Pa.PKind}

law patch_kind_add_header:
  {Pa.p_kind_of("*** Add File: x") == Pa.KAddHdr{} : Pa.PKind}

law patch_kind_delete_header:
  {Pa.p_kind_of("*** Delete File: x") == Pa.KDelHdr{} : Pa.PKind}

law patch_kind_update_header:
  {Pa.p_kind_of("*** Update File: x") == Pa.KUpdHdr{} : Pa.PKind}

law patch_kind_chunk_head:
  {Pa.p_kind_of("@@ fn foo") == Pa.KHead{} : Pa.PKind}

law patch_kind_garbage:
  {Pa.p_kind_of("what") == Pa.KGarbage{} : Pa.PKind}

# LAW: a chunk's lines replace exactly; the file's trailing newline
# survives (the split/join roundtrip is byte-exact).
law patch_apply_hit:
  {Pa.apply_update("a\nb\nc\n", Con{Pa.PChunk{Con{"b", Nil{}}, Con{"B", Nil{}}, 1n, 1n}, Nil{}})
    == Pa.POut{True{}, "a\nB\nc\n", 1n} : Pa.POut}

# LAW: a miss fails loudly with the chunk's index (0-based).
law patch_apply_miss:
  {Pa.apply_update("a\nb\nc\n", Con{Pa.PChunk{Con{"x", Nil{}}, Con{"X", Nil{}}, 1n, 1n}, Nil{}})
    == Pa.POut{False{}, "", 0n} : Pa.POut}

# LAW: chunks apply in order - the second matches AFTER the first's
# splice (the done/todo cursor is strict).
law patch_apply_sequential:
  {Pa.apply_update("f\na\nf\na\n",
    Con{Pa.PChunk{Con{"f", Nil{}}, Con{"F", Nil{}}, 1n, 1n},
     Con{Pa.PChunk{Con{"f", Nil{}}, Con{"G", Nil{}}, 1n, 1n}, Nil{}}})
    == Pa.POut{True{}, "F\na\nG\na\n", 2n} : Pa.POut}

# LAW: the cursor never rewinds - the second chunk may NOT match a
# line the first chunk already consumed.
law patch_apply_sequential_miss:
  {Pa.apply_update("a\nf\n",
    Con{Pa.PChunk{Con{"a", Nil{}}, Con{"A", Nil{}}, 1n, 1n},
     Con{Pa.PChunk{Con{"a", Nil{}}, Con{"B", Nil{}}, 1n, 1n}, Nil{}}})
    == Pa.POut{False{}, "", 1n} : Pa.POut}

# LAW: a chunk with no old lines is a pure insertion at the cursor.
law patch_apply_insert:
  {Pa.apply_update("a\n", Con{Pa.PChunk{Nil{}, Con{"X", Nil{}}, 0n, 1n}, Nil{}})
    == Pa.POut{True{}, "X\na\n", 1n} : Pa.POut}

# LAW: the failed chunk's first line is the error anchor.
law patch_anchor_names_the_chunk:
  {Pa.p_chunk_anchor(
    Con{Pa.PChunk{Con{"one", Nil{}}, Nil{}, 1n, 0n},
     Con{Pa.PChunk{Con{"two", Nil{}}, Nil{}, 1n, 0n}, Nil{}}}, 1n) == "two" : String}

# LAW: the validate loop reads ahead by action path - every action
# shape carries one.
law patch_path_of_actions:
  {Bool.and(String.eq(Pa.p_path_of(Pa.PAdd{"a", "x"}), "a"),
    Bool.and(String.eq(Pa.p_path_of(Pa.PDelete{"d"}), "d"),
     String.eq(Pa.p_path_of(Pa.PUpdate{"u", Nil{}}), "u"))) == True{} : Bool}

# LAW: the Begin marker matches by prefix - the live model appends
# a stray ' ***' to it (observed end-to-end with GLM).
law patch_kind_begin_prefix:
  {Pa.p_kind_of("*** Begin Patch ***") == Pa.KBegin{} : Pa.PKind}

# LAW: section headers parse without the leading '***' too - safe,
# because a content line inside a chunk always carries a ' ', '+' or
# '-' prefix, so a bare 'Add File:' line can only be a header.
law patch_kind_bare_add_header:
  {Pa.p_kind_of("Add File: x") == Pa.KAddHdr{} : Pa.PKind}

law patch_kind_bare_delete_header:
  {Pa.p_kind_of("Delete File: x") == Pa.KDelHdr{} : Pa.PKind}

law patch_kind_bare_update_header:
  {Pa.p_kind_of("Update File: x") == Pa.KUpdHdr{} : Pa.PKind}

# LAW: the exact variant GLM emitted live (trailing ' ***' on Begin,
# bare section headers) parses to the same actions the canonical
# form gives.
law patch_parse_glm_variant:
  {Pa.parse_patch("*** Begin Patch ***\nUpdate File: ap-e2e/a.txt\n@@ \n one\n-two\n+TWO\nAdd File: ap-e2e/sub/new.txt\n+hello\n+patched\nDelete File: ap-e2e/del.txt\n*** End Patch")
    == Some{Con{Pa.PUpdate{"ap-e2e/a.txt",
      Con{Pa.PChunk{Con{"one", Con{"two", Nil{}}}, Con{"one", Con{"TWO", Nil{}}}, 2n, 2n},
        Nil{}}},
      Con{Pa.PAdd{"ap-e2e/sub/new.txt", "hello\npatched\n"},
      Con{Pa.PDelete{"ap-e2e/del.txt"}, Nil{}}}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# ---- BR-001: an interrupt must not orphan the calls the turn holds ----

# LAW: interrupting a turn that waits on tool calls answers every
# waiting call with a failed "interrupted" result BEFORE the turn
# dies - the answer stays adjacent to its assistant call (the steering
# the turn held is committed after, by finish_turn).
law interrupt_closes_waiting_calls:
  {S.sess_history(S.res_next(S.apply(steer_flow.called(law_cfg()), T.Interrupt{}),
      steer_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("thinking", Con{T.Call{2, "bash", "ls"}, Nil{}}),
      Con{T.tool_res("tool bash failed: interrupted"),
      Con{T.Msg{T.RUser{}, "penguins", Nil{}, True{}}, Nil{}}}}}
    : List<&2, T.Msg>}

# LAW: an agent pending (nothing in history yet) answers nothing on an
# interrupt.
law interrupt_closes_agent_pending:
  {S.interrupt_closes(Some{T.PAgent{9}}) == Nil{}
    : List<&2, T.Msg>}

# ---- run_typescript: one engine, an ordinary tool (BISE-118) ----
# The Core runs no program: a run_typescript call goes out as a tool act
# like bash; the runtime runs it in V8 (bend-jsrt), live and scripted
# alike, and the program's one result comes back as the tool result.
# The program's own tool calls stay inside the runtime (record and
# re-run), the Core never sees them.

def ts_args() -> String:
  "{\"code\": \"return 1\"}"

def ts_flow.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Nil{}}}),
    steer_flow.started(cfg))

# LAW: the call goes out: one tool act, name and args as the model wrote
# them (the completion renumbers Call{1} to 2, like any tool call).
law run_typescript_goes_out_as_a_tool:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Nil{}}}))
    == Con{T.ToolAct{2, T.Call{2, "run_typescript", ts_args()}}, Nil{}}
    : List<&2, T.Act>}

# LAW: a program batches with other calls in one completion (the live
# behavior; the old scripted core refused "run_typescript must be the
# only call").
law run_typescript_batches_with_other_calls:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Con{T.Call{2, "bash", "ls"}, Nil{}}}}))
    == Con{T.ToolAct{2, T.Call{2, "run_typescript", ts_args()}},
      Con{T.ToolAct{3, T.Call{3, "bash", "ls"}}, Nil{}}}
    : List<&2, T.Act>}

# LAW: the result comes back: the program's value is the run_typescript
# tool result, right after its call.
law run_typescript_result_comes_back:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.ToolOk{2, "1"}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript ok: 1"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: a program that fails (a throw, a failed inner call, a bad
# program) comes back as a failed run_typescript result.
law run_typescript_failure_comes_back:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.ToolFail{2, "program error"}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript failed: program error"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: an interrupt answers the running program like any waiting call.
law run_typescript_interrupted:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.Interrupt{}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript failed: interrupted"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: the runtime runs run_typescript on the V8 engine (bend-jsrt).
# The dispatch has no mode: live and scripted sessions run the same code.
law run_typescript_runs_on_jsrt:
  {Rt.tool_of("run_typescript") == Rt.TNode{} : Rt.Tool}

# LAW: the old routing name node_program is no tool of its own any more
# (an unknown name goes to the MCP lookup, which fails it by name).
law node_program_is_not_a_tool:
  {Rt.tool_of("node_program") == Rt.TMcp{} : Rt.Tool}

# LAW: a session saved before BISE-118 holds live programs under the old
# routing name node_program: they load as run_typescript.
law checkpoint_loads_node_program_as_run_typescript:
  {K.fcall("  CALL 7 node_program : x")
    == T.Call{7, "run_typescript", "x"} : T.Call}

# LAW: every other call name loads untouched.
law checkpoint_keeps_call_names:
  {K.fcall("  CALL 7 bash : ls") == T.Call{7, "bash", "ls"} : T.Call}

# ---- BR-004: the deferred scan trusts only standalone tool acks ----

# LAW: assistant prose quoting the ack never fires the deferred
# request (the model echoes tool outputs routinely).
# BR-004 helper: an assistant message that quotes the compact ack
def prose_quoting_ack() -> T.Msg:
  T.asst(String.append("the tool said '",
    String.append(Sf.COMPACT_ACK(), "' - filing it")), Nil{})

law deferred_ignores_assistant_prose:
  {Sf.deferred_of(Con{T.user("hi"), Con{prose_quoting_ack(), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool output QUOTING the harness source never fires - the
# quotes and indentation around the constant break the whole-text
# and whole-line shapes.
law deferred_ignores_source_quotes:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("  \"",
          String.append(Sf.COMPACT_ACK(), "\""))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool result that merely CONTAINS the marker mid-text never
# fires - the ack must end the text or stand on its own line.
law deferred_ignores_midtext_marker:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("prefix ",
          String.append(Sf.COMPACT_ACK(), " suffix"))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: the real direct shape fires - the self.* tool result prefixes
# the ack with "tool X ok: ", so the ack ends the text.
law deferred_reads_prefixed_ack:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("tool self.compact ok: ",
          Sf.COMPACT_ACK())), Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# LAW: the real program shape fires - the acks ride their own lines of
# the escaped tool text (a literal backslash-n line boundary).
law deferred_reads_program_acked_result:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{S.tool_result_msg("run_typescript",
          String.append("prog out\n", Sf.COMPACT_ACK()), True{}),
        Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# ---- the provider adapters (core/api.bend + runtime/provider.bend) ----
# The vibe CLI structure: one mapping per api_style, a provider table
# in the runtime. The OpenAI mapping above stays exactly as the GLM
# laws pinned it; the Anthropic mapping below is the Messages shape
# the foundry proxy serves (probed live: thinking blocks are optional,
# tool_use without history thinking is accepted, a thinking block
# without its signature is rejected). A SIGNED history reasoning
# replays its thinking block verbatim with the signature (the
# reasoning continuity across tool calls); an unsigned one stays
# stripped.

# LAW: the alias resolves and the style follows the model name.
law provider_resolves_alias:
  {Pv.resolve_model("opus-5.5") == "claude-opus-5-5" : String}

law provider_opus_is_anthropic:
  {Pv.model_style("claude-opus-5-5") == Api.Anthropic{} : Api.Style}

law provider_glm_is_openai:
  {Pv.model_style("zai-glm-5-3") == Api.OpenAI{} : Api.Style}

# LAW: the provider table carries the right api_base and key env.
law provider_anthropic_base:
  {Pv.model_url(Api.Anthropic{})
    == "https://foundry-proxy.cheetah-koi.ts.net/anthropic/v1/messages"
    : String}

law provider_openai_base:
  {Pv.model_url(Api.OpenAI{})
    == "https://api.mistral.ai/v1/chat/completions" : String}

law provider_anthropic_key_env:
  {Pv.model_key_env(Api.Anthropic{}) == "ANTHROPIC_FOUNDRY_API_KEY"
    : String}

law provider_openai_key_env:
  {Pv.model_key_env(Api.OpenAI{}) == "MISTRAL_API_KEY" : String}

# LAW: the style dispatch routes the body and the reply to the right
# adapter - the OpenAI side byte-identical to api_body/reply_of.
law style_routes_body:
  {Api.api_body_for(Api.OpenAI{}, "m",
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    == Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    : String}

law style_routes_reply:
  {Api.reply_of_for(Api.OpenAI{}, 401, "{\"error\":\"bad key\"}")
    == Oai.reply_of(401, "{\"error\":\"bad key\"}") : String}

# LAW: an Anthropic body maps a simple wire byte-for-byte: the system
# prompt becomes the top-level system field, the user message a text
# block, the thinking config the vibe CLI sends for opus-5.5 (adaptive,
# summarized, effort high, the adaptive max_tokens budget).
law api_body_anth_maps:
  {Api.api_body_for(Api.Anthropic{}, "claude-opus-5-5",
      W.parse_wire("MODEL default\nMSG system : You are precise.\nMSG user : bonjour\nEND\n"))
    == "{\"system\":[{\"type\":\"text\",\"text\":\"You are precise.\"}],\"model\":\"claude-opus-5-5\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\"}]}]}"
    : String}

# LAW: an Anthropic compaction call (no TOOL lines) omits tools and
# tool_choice - the model cannot even offer a tool call.
law api_body_anth_bare_omits_tools:
  {Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\"}]}]}"
    : String}

# LAW: the Anthropic tools carry the {name, description, input_schema}
# shape and tool_choice is the auto object.
law api_body_anth_tools_shape:
  {Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\"}]}],\"tools\":[{\"name\":\"echo\",\"description\":\"Returns text.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]}}],\"tool_choice\":{\"type\":\"auto\"}}"
    : String}

# LAW: an assistant call pairs with its tool result exactly as the
# OpenAI adapter pairs them - the tool_use id and the tool_use_id of
# the result agree, and the result rides a USER message.
law anth_tool_result_pairs:
  {Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\nMSG tool : tool echo ok: out\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"thinking\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"tool echo ok: out\"}]}]}"
    : String}

# LAW: consecutive tool results merge into ONE user message - the
# vibe mapper's shape (the ids pair FIFO).
law anth_tool_results_merge:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"}")
    == False{} : Bool}

# LAW: parallel calls pair with their results in order - the first
# CALL line takes the first tool result (the calls of one message
# were once reversed, so each result went to the other call).
law oai_parallel_results_pair_in_order:
  {String.contains(Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"role\":\"tool\",\"content\":\"out1\",\"tool_call_id\":\"call_7\"},{\"role\":\"tool\",\"content\":\"out2\",\"tool_call_id\":\"call_8\"}")
    == True{} : Bool}

# LAW: the same pairing on the Anthropic adapter.
law anth_parallel_results_pair_in_order:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\"},{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"}")
    == True{} : Bool}

# LAW: the marked history reasoning is STRIPPED on the way back - a
# thinking block without its signature is rejected by the Messages
# API (probed), so no \"type\":\"thinking\" block ever rides an
# assistant message.
law anth_unsigned_thinking_stays_stripped:
  {Bool.not(String.contains(Api.api_body_for(Api.Anthropic{}, "m",
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4</think>\\n4
END
")),
       "type\":\"thinking\""))
    == True{} : Bool}

# LAW: a SIGNED marked reasoning replays its thinking block verbatim
# with the signature at block level - the reasoning continuity the
# Messages API needs across tool calls; the model no longer re-thinks
# after every tool result.
law anth_signed_thinking_replays:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m",
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4\\nBENDSIG::sig</think>\\n4
END
")),
       "{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"}")
    == True{} : Bool}

# LAW: EVERY signed span of one message replays as its own thinking
# block - a reply with two thinking blocks leaves no marker in the
# text (a span riding as plain text is refused as reasoning
# extraction, probed).
law anth_multi_span_replays:
  {Api.api_body_for(Api.Anthropic{}, "m",
       W.parse_wire("MODEL default
MSG assistant : <think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"a\",\"signature\":\"s1\"},{\"type\":\"thinking\",\"thinking\":\"b\",\"signature\":\"s2\"},{\"type\":\"text\",\"text\":\"ok\"}]}]}"
    : String}

# LAW: a long tool run keeps every message - 20 tool results still
# reach the body (the walk fuel covers the three hops of a tool message).
law anth_long_tool_run_keeps_messages:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 1 echo : hi\nMSG tool : out1\nMSG assistant : x\n  CALL 2 echo : hi\nMSG tool : out2\nMSG assistant : x\n  CALL 3 echo : hi\nMSG tool : out3\nMSG assistant : x\n  CALL 4 echo : hi\nMSG tool : out4\nMSG assistant : x\n  CALL 5 echo : hi\nMSG tool : out5\nMSG assistant : x\n  CALL 6 echo : hi\nMSG tool : out6\nMSG assistant : x\n  CALL 7 echo : hi\nMSG tool : out7\nMSG assistant : x\n  CALL 8 echo : hi\nMSG tool : out8\nMSG assistant : x\n  CALL 9 echo : hi\nMSG tool : out9\nMSG assistant : x\n  CALL 10 echo : hi\nMSG tool : out10\nMSG assistant : x\n  CALL 11 echo : hi\nMSG tool : out11\nMSG assistant : x\n  CALL 12 echo : hi\nMSG tool : out12\nMSG assistant : x\n  CALL 13 echo : hi\nMSG tool : out13\nMSG assistant : x\n  CALL 14 echo : hi\nMSG tool : out14\nMSG assistant : x\n  CALL 15 echo : hi\nMSG tool : out15\nMSG assistant : x\n  CALL 16 echo : hi\nMSG tool : out16\nMSG assistant : x\n  CALL 17 echo : hi\nMSG tool : out17\nMSG assistant : x\n  CALL 18 echo : hi\nMSG tool : out18\nMSG assistant : x\n  CALL 19 echo : hi\nMSG tool : out19\nMSG assistant : x\n  CALL 20 echo : hi\nMSG tool : out20\nEND\n")),
    "\"content\":\"out20\"")
    == True{} : Bool}

# LAW: a refusal (empty content, stop_reason "refusal") surfaces as a
# provider error - never an empty reply retried to the null limit.
law anth_refusal_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[],\"stop_reason\":\"refusal\",\"stop_details\":{\"explanation\":\"blocked\"}}")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# comment header preserved below
# LAW: Anthropic text blocks become separate lines (the GLM parity).
law anth_blocks_join_lines:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: Anthropic thinking blocks surface in the reply wrapped in the
# think markers - the same transport the GLM adapter uses, so the TUI
# and the history are unchanged.
law anth_thinking_surfaces:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]}"))
    == "OK <think>2+2 = 4
BENDSIG::sig</think>
4
END
" : String}

# comment header preserved below
# LAW: tool_use blocks map onto the CALL reply lines the Core parses
# (1-based ids, the flat arg rehydrated from the input object).
law anth_tool_calls_map:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a structured input rides as its JSON on the CALL line.
law anth_structured_call_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"run_typescript\",\"input\":{\"code\":\"x()\"}}]}")
    == "OK \nCALL 1 run_typescript : {\"code\":\"x()\"}\nEND\n" : String}

# LAW: a 200 that carries an error body surfaces as an error.
law anth_error_body_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a non-200 still maps to the flattened ERROR line with the status.
law anth_status_error_maps:
  {Api.reply_of_for(Api.Anthropic{}, 401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: {\"error\":\"bad key\"}\nEND\n" : String}

# LAW: the Messages API tool-name pattern has no dot - the self.*
# tools ride as self_* on the way out and come back as their harness
# names (the proxy rejects "self.reload" with invalid_request_error).
law anth_tool_name_roundtrip:
  {Bool.and(
     String.eq(Anth.anth_tool_name_out("self.reload"), "self_reload"),
     Bool.and(
       String.eq(Anth.anth_tool_name_in("self_reload"), "self.reload"),
       String.eq(Anth.anth_tool_name_out("bash"), "bash")))
    == True{} : Bool}

# ---- the config file (core/config.bend) ----
# A pragmatic TOML subset: key = value lines, [section] headers, #
# comments. Precedence everywhere: env var > config > default.

# LAW: flat keys parse byte-for-byte - strings quoted, numbers bare.
law config_parse_flat:
  {Cf.parse_toml("model = \"opus-5.5\"\nthreshold = 800000\n")
    == Con{Cf.KV{"model", "opus-5.5"}, Con{Cf.KV{"threshold", "800000"},
      Nil{}}} : List<&2, Cf.KV>}

# LAW: comments and blank lines never reach the pairs.
law config_skips_comments_blanks:
  {Cf.parse_toml("# a comment\n\n  \nmodel = \"m\"\n")
    == Con{Cf.KV{"model", "m"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a key inside a section carries the section prefix.
law config_sections_prefix:
  {Cf.parse_toml("[harness]\nbg_after = 5\n")
    == Con{Cf.KV{"harness.bg_after", "5"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a quoted value may carry '=' itself (the pair splits on the
# FIRST one only).
law config_value_keeps_equals:
  {Cf.parse_toml("cmd = \"a=b\"\n")
    == Con{Cf.KV{"cmd", "a=b"}, Nil{}} : List<&2, Cf.KV>}

# LAW: the lookups answer the exact key - Some when present, the
# default when not.
law config_lookup_found:
  {Cf.cfg_get("model = \"opus-5.5\"\n", "model") == Some{"opus-5.5"}
    : Maybe<&2, String>}

law config_lookup_missing:
  {Cf.cfg_or(Cf.cfg_get("model = \"m\"\n", "threshold"), "800000")
    == "800000" : String}

# LAW: numbers ride as U32.
law config_u32_parses:
  {Cf.cfg_u32("bg_after = 45\n", "bg_after", 30) == 45 : U32}

law config_u32_defaults:
  {Cf.cfg_u32("model = \"m\"\n", "bg_after", 30) == 30 : U32}

# LAW: the precedence - the environment variable wins, then the
# config file, then the default.
law config_env_wins:
  {Cf.env_cfg_or("zai-glm-5-3",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "zai-glm-5-3" : String}

law config_env_empty_falls_to_file:
  {Cf.env_cfg_or("",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "opus-5.5" : String}

law config_env_empty_no_file:
  {Cf.env_cfg_or("", Cf.cfg_get("", "model"), "claude-opus-5-5")
    == "claude-opus-5-5" : String}

# LAW: the template the runtime writes parses - its own keys are
# what the lookups find.
law config_template_parses:
  {Cf.cfg_get(St.CONFIG_TEMPLATE(), "model") == Some{"opus-5.5"}
    : Maybe<&2, String>}

law config_template_threshold:
  {Cf.cfg_u32(St.CONFIG_TEMPLATE(), "threshold", 800000) == 800000
    : U32}

# ---- the harness-info line: the single source of truth for clients ----
# The TUI (through the bend-harness parent) and bend_client.py read
# this line instead of recomputing the model, the threshold or the
# side-channel paths. Its format is a contract with both parsers.
law info_line_format:
  {Rt.info_line("claude-opus-5-5", "800000", "/tmp/bend-steer-7.txt",
      "/tmp/bend-interrupt-7.txt")
    == "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt"
    : String}

# LAW: the announced side-channel paths are the ones the runtime
# drains - the same builders, per port.
law info_paths_are_the_drained_ones:
  {Bool.and(String.eq(Rt.steer_path_of("7"), "/tmp/bend-steer-7.txt"),
     String.eq(Pv.interrupt_path_of("7"), "/tmp/bend-interrupt-7.txt"))
    == True{} : Bool}

# ---- --resume: the restored history replays as live wire lines ----

# LAW: a restored history replays as the lines a live turn prints, each
# prefixed "history ": the compaction summary (its preamble skipped), the
# user message, each tool call with its code and its result (paired in
# call order, no id in the result), the assistant text on ONE wire line
# (a real newline escaped as backslash-n, chars 92 110, as live replies
# arrive); a call left without a result (interrupt, crash) replays as
# failed.
law resume_replays_history:
  {Rt.replay(Con{T.injected_user("The earlier conversation was compacted. A summary replaces it; continue from the preserved user messages."),
      Con{T.injected_user("Summary of the earlier conversation:
<summary>court</summary>"),
      Con{T.user("salut"),
      Con{T.asst("", Con{T.Call{7, "bash", "ls -la"}, Nil{}}),
      Con{T.tool_res("tool bash failed: boom"),
      Con{T.asst("<think>hmm</think>
fini", Nil{}),
      Con{T.asst("", Con{T.Call{9, "bash", "pwd"}, Nil{}}),
      Nil{}}}}}}}})
    == Con{"history   obs: compaction_done: court...",
      Con{"history you : salut",
      Con{"history   obs: tool_started #7",
      Con{"history tool #7 bash : ls -la",
      Con{"history tool_code #7 : ls -la",
      Con{"history tool_result #7 fail : boom",
      Con{"history   obs: tool_finished #7 failed",
      Con{String.append("history   obs: assistant: <think>hmm</think>",
        SCon{Chr{92}, SCon{Chr{110}, "fini"}}),
      Con{"history   obs: tool_started #9",
      Con{"history tool #9 bash : pwd",
      Con{"history tool_code #9 : pwd",
      Con{"history   obs: tool_finished #9 failed",
      Nil{}}}}}}}}}}}}}
    : List<&2, String>}

# ---- the Anthropic stream (core/anth-stream.bend) ----

# LAW: a streamed request is the same body with "stream": true first.
law anth_stream_body:
  {As.stream_body("{\"model\":\"m\"}") == "{\"stream\":true,\"model\":\"m\"}" : String}

# LAW: text and tool input stream in pieces; the reply is rebuilt whole -
# pings, padding and the empty first input piece change nothing.
law anth_sse_tool_call:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}      \n\nevent: ping\ndata: {\"type\": \"ping\"}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll list\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" /tmp.\"}}   \n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"bash\",\"input\":{},\"caller\":{\"type\":\"direct\"}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"arg\\\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\": \\\"ls -la /tmp\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" | head\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":72}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "OK I'll list /tmp.\nCALL 1 bash : ls -la /tmp | head\nEND\n" : String}

# LAW: a streamed reply maps to exactly what the same message sent whole
# maps to - signed thinking included (the signature streams as its own
# delta and must survive for the replay).
law anth_sse_matches_whole_reply:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"2+2\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\" = 4\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"4\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == Api.reply_of_for(Api.Anthropic{}, 200, "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}],\"stop_reason\":\"end_turn\"}") : String}

# LAW: an error event mid-stream is the reply: the 200 error body the
# retry policy already retries.
law anth_sse_error_event:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a streamed reply prints the usage a whole reply would: the input
# side from message_start, output_tokens from the last message_delta.
law anth_sse_usage_line:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"content\":[],\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":2,\"cache_read_input_tokens\":30,\"output_tokens\":1}}}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":72}}

")
    == "  obs: usage: model=m in=42 out=72 cache_read=30 cache_write=2" : String}

# LAW: a stream with no message_start (an error event) prints no usage.
law anth_sse_no_usage:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: error
data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}

") == "" : String}

# LAW: a streamed refusal surfaces like a whole one.
law anth_sse_refusal:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"type\":\"refusal\",\"explanation\":\"blocked\"}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# LAW: a 200 that is a whole JSON error body (the proxy's overload
# answer, even to a streamed request) still maps to the provider error
# the retry policy retries.
law anth_stream_json_error_body:
  {As.reply_any("{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: only a non-empty flag is a Ctrl+C - the runtime clears the flag to
# "" at every turn end, and that must never read as an interrupt.
law interrupt_flag_semantics:
  {Bool.and(Pv.interrupt_requested("1"),
      Bool.and(Bool.not(Pv.interrupt_requested("")),
        Bool.not(Pv.interrupt_requested("\n"))))
    == True{} : Bool}

# LAW: a streamed read waits 90 s at most (events come every few seconds).
law provider_stream_step_timeout:
  {Pv.stream_step_ms() == 90000 : U32}

# LAW: a debug event is one JSON object on one line: ts, src, kind, then
# the fields in order; strings are JSON-escaped (a newline stays \\n).
law debug_event_line:
  {Dg.ev_line(7n, "a\"b", Con{Dg.FStr{"k", "x\ny"}, Con{Dg.FNat{"n", 3n}, Nil{}}})
    == "{\"ts\":7,\"src\":\"repl\",\"kind\":\"a\\\"b\",\"k\":\"x\\ny\",\"n\":3}" : String}

# LAW: only obs and tool annotations become breadcrumbs, one line each.
law debug_crumbs_filter:
  {Dg.crumbs(Con{"  obs: turn_started", Con{"hello", Con{"tool #3 bash : ls", Nil{}}}}, 5n)
    == "{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"obs: turn_started\"}\n{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"tool #3 bash : ls\"}\n" : String}

# LAW: a first attempt that works is not logged; a later one that works
# is a recovery; a failure, a retry or a 200 carrying an ERROR reply is
# an incident.
law debug_plan_ok_first:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 0n) == Dg.PSkip{} : Dg.Plan}

law debug_plan_recovered:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 2n) == Dg.PRecovered{} : Dg.Plan}

law debug_plan_error_reply:
  {Dg.plan(Pv.AOk{"ERROR provider refusal: x"}, 0n)
    == Dg.PFailed{"error_reply", "ERROR provider refusal: x", 0} : Dg.Plan}

law debug_plan_retry:
  {Dg.plan(Pv.ARetry{5000, "provider 529 (transient)"}, 1n)
    == Dg.PFailed{"retry", "provider 529 (transient)", 5000} : Dg.Plan}

# LAW: an artifact is cut at the limit, and says how long it was.
law debug_clip:
  {Bool.and(String.eq(Dg.clip("abcdef", 3n), "abc\n[... truncated: 6 chars in total]"),
      String.eq(Dg.clip("abc", 3n), "abc"))
    == True{} : Bool}

# LAW: the reply artifact names the attempt 1-based, like the retry notes.
law debug_reply_name:
  {Dg.reply_name(12n, 0n) == "reply-12-1.txt" : String}

# LAW: the clock probe answers "<status>\n<epoch ms>": the offset is
# epoch - mono; an unreadable answer gives no offset (0).
law debug_clock_anchor:
  {Dg.anchor(Con{"0", Con{"1500", Nil{}}}, 500n) == 1000n : Nat}

law debug_clock_anchor_failed:
  {Dg.anchor(Con{"127", Con{"sh: perl: not found", Nil{}}}, 500n) == 0n : Nat}

# LAW: an event is stamped mono + the stored offset, mono without one.
law debug_stamp:
  {Bool.and(Nat.is_eq(Dg.stamp(Some{"1000\n"}, 5n), 1005n),
      Nat.is_eq(Dg.stamp(None{}, 5n), 5n))
    == True{} : Bool}

# ---- switchboard hooks (BEND_WORKDIR, BEND_CONTEXT_FILE, BEND_EXTRA_PROMPT) ----

# LAW: without BEND_WORKDIR the command is unchanged.
law workdir_empty_keeps_command:
  {Sh.with_workdir("", "ls -la") == "ls -la" : String}

# LAW: with BEND_WORKDIR the command first enters the (quoted) directory,
# and fails instead of running elsewhere when it cannot.
law workdir_cds_first:
  {Sh.with_workdir("/tmp/a b", "ls") == "cd '/tmp/a b' || exit 1\nls" : String}

# LAW: an empty context changes nothing.
law context_empty_keeps_request:
  {Rm.with_context("", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n" : String}

# LAW: a context becomes the LAST user message of an agent request,
# newlines escaped like every wire message.
law context_is_last_user_message:
  {Rm.with_context("board\nline 2", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nMSG user : board\\nline 2\nEND\n" : String}

# LAW: a compaction request (no tool catalog) never carries the context.
law context_skips_compaction:
  {Rm.with_context("board", "MODEL default\nMSG user : hi\n\nEND\n")
    == "MODEL default\nMSG user : hi\n\nEND\n" : String}

# LAW: the extra prompt ends the system prompt; an empty one adds nothing.
law extra_prompt_appends:
  {Rp.prompt_extra("base", "role") == "base\n\nrole" : String}

law extra_prompt_empty_keeps:
  {Rp.prompt_extra("base", "  ") == "base" : String}

# LAW: a drained steering line carries escaped newlines back as real ones.
law steer_line_unescapes:
  {Rt.steer_cmd("a\\nb") == T.UserMsg{T.Steer{}, "a\nb"} : T.Cmd}

# ---- Switchboard hub (hub/*.bend, the sb-core process) ----
# The laws quantify over EVERY hub state, EVERY input (any JSON line the
# daemon may send) and EVERY sequence of inputs: no test could enumerate
# them. HC.step is the decision of one input, HC.commit what sb-core keeps
# of it, HC.run its loop over a list of inputs.

# LAW: after any input, the hub's state is exactly the replay of the
# journal events it emitted over the state before (the journal is a
# faithful log: a restart that replays it rebuilds the same state).
law hub_step_replays:
  for +h: HC.Hub
  for +i: J.Json
  {HX.Core.d(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))
    == HM.apply_all(HX.journal(HX.Core.fx(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))), HC.Hub.d(h)) : HM.St}

# LAW: over any sequence of inputs (git answers still missing included),
# the final state is the replay of the whole journal over the first one.
law hub_run_replays:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  {HC.Hub.d(HC.RunR.hub(HC.run(ins, h))) == HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)) : HM.St}

# LAW: every message the hub ever sends, over any sequence of inputs,
# takes the next free id of the state it is sent in: no id is reused, so
# no message overwrites (loses) another.
law msg_ids_fresh:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Fresh(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW: no message the hub ever sends is both an automatic reply and a
# question (RFC 0003 §7): automatic replies can never start a chain.
law auto_never_asks:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Quiet(HC.RunR.journal(HC.run(ins, h)))

# LAW: after a tick, every wait still pending ends later than now: a wait
# past its deadline always gets its timeout (with the 25 s cap of `sb
# wait`, no set of agents waiting on each other blocks longer).
law tick_clears_late_waiters:
  for -o: HM.St
  for +cx: HX.Cx<o>
  HC.Later(HC.waiters_of_cx(o, HC.tick(o, cx)), HX.R.now(HX.Cx.r(o, cx)))

# LAW: stopping or dropping a task leaves no wait of it behind (each got
# its `stopped` error), whatever the state.
law stop_leaves_no_waiter:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  HC.NoneOf(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), n)

# LAW (L2, no message lost or doubled): a new hub keeps each message as
# ONE record — its text and its single delivery state; no state exists
# without its message (the state is a field of the record).
law init_unique:
  for +w: String
  HI.Unique(HM.init(w))

# LAW (L2): from a state where each message is one record, any sequence
# of inputs keeps it so: no two records ever share an id.
law msgs_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.Unique(HC.Hub.d(h))
  HI.Unique(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L2): over any sequence of inputs, the messages of the first state
# start, in order, the messages of the last: a message once sent is never
# removed (its record, and so its state, stays).
law msgs_kept:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HI.Kept(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L3, delivered at most once, RFC 0003 §6.1): over any sequence of
# inputs, a message becomes Delivered only while it is Queued, and it goes
# back to Queued only as an explicit steer_leftover (steering the runtime
# never read). So a message is delivered once, and again only after an
# explicit steer_leftover.
law delivered_once:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Deliv(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW (L12): stopping or dropping a task, in any state where each message
# is one record (every state the hub reaches: msgs_unique), leaves no
# message queued for it (each got recipient_unavailable).
law stop_leaves_no_queue:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  for u: HI.Unique(HX.d(o, cx))
  {HM.queued_for(HX.d(o, HC.stop_task(o, cx, n, life, why)), n) == Nil{} : List<&2, HM.Msg>}

# LAW (L12): stopping or dropping a task, in any state where no two open
# cards share an id (every state the hub reaches: cards_unique), leaves no
# open card of it.
law stop_leaves_no_card:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  for u: HI.UniqueCards(HX.d(o, cx))
  {HC.card_ids(HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), n, Nil{}) == Nil{} : List<&2, Nat>}

# LAW (L12 support): a new hub has no two open cards with one id...
law init_cards_unique:
  for +w: String
  HI.UniqueCards(HM.init(w))

# ...and any sequence of inputs keeps it so (each card takes the next
# free id).
law cards_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.UniqueCards(HC.Hub.d(h))
  HI.UniqueCards(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L4 support): a new hub keeps the agents' names apart...
law init_names_apart:
  for +w: String
  HI.NamesApart(HM.init(w))

# ...and any sequence of inputs keeps them apart: no name or old name of an
# agent is ever a name or old name of another one (a message sent to an
# agent can only be for it).
law names_apart:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.NamesApart(HC.Hub.d(h))
  HI.NamesApart(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))


# ---- images (docs/images.md, core/image.bend) ----

# LAW: a text splits into its text parts and its image markers, in order.
law image_split_marker:
  {Im.img_split("a <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\"> b")
    == Con{Im.IText{"a "}, Con{Im.IImg{"[Image #1]", "p.png", "image/png", "/s/x.b64"},
      Con{Im.IText{" b"}, Nil{}}}}
    : List<&2, Im.IPart>}

# LAW: a marker that does not name a .b64 file is no image.
law image_split_refuses_bad_marker:
  {Im.img_paths("<image name=\"x\" path=\"p\" mime=\"image/png\" b64=\"/etc/passwd\">")
    == Nil{} : List<&2, String>}

# LAW: OpenAI style - a user message with a marker becomes content
# parts, the Codex frame around a data-url placeholder.
law api_body_image_user_parts:
  {Api.api_body_for(Api.OpenAI{}, "m", W.parse_wire("MODEL default
MSG user : see <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"see \"},{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},{\"type\":\"image_url\",\"image_url\":{\"url\":\"data:image/png;base64,@@BENDIMG:/s/x.b64@@\"}},{\"type\":\"text\",\"text\":\"</image>\"}]}]}"
    : String}

# LAW: Anthropic - a tool result with a marker carries image blocks.
law api_body_image_anth_tool_result:
  {Api.api_body_for(Api.Anthropic{}, "m", W.parse_wire("MODEL default
MSG user : go
MSG assistant : 
  CALL 1 run_typescript : {}
MSG tool : <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"run_typescript\",\"input\":{}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_1\",\"content\":[{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":\"image/png\",\"data\":\"@@BENDIMG:/s/x.b64@@\"}},{\"type\":\"text\",\"text\":\"</image>\"}]}]}]}"
    : String}

# LAW: only the last `cap` loaded images are kept.
law image_keep_last:
  {Im.img_keep(Con{True{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}, 2n)
    == Con{False{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}
    : List<&2, Bool>}

# LAW: a marker that is not kept becomes plain text; the others stay.
law image_defang_unkept:
  {Im.img_defang("<image name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">",
      Con{False{}, Con{True{}, Nil{}}})
    == "<image-unavailable name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">"
    : String}

# LAW: the splice puts each image's data at its placeholder (an unknown
# one becomes empty).
law image_splice_data:
  {Im.img_splice("{\"d\":\"@@BENDIMG:/s/1.b64@@\",\"e\":\"@@BENDIMG:/s/9.b64@@\"}",
      Con{Im.IL{"/s/1.b64", "QUJD"}, Nil{}})
    == "{\"d\":\"QUJD\",\"e\":\"\"}" : String}
