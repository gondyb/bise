# core/session.bend — the Harness Core: the Step Protocol state machine.
#
# apply(session, command) -> Rejected{reason} | Accepted{obs, acts, next}
#
# - a Rejected result carries no session: a rejected command cannot
#   mutate state or consume an input id (true by type)
# - every Accepted result increments inputs by exactly one
# - the turn, steering, notifications, tool-call correlation, the null
#   iteration limit (ADR 0015), and ADR 0011 preflight compaction all
#   live here. The Core performs no IO.
import Base
import ./types.bend as T
import ./estimate.bend as E
import ./compaction.bend as C
import ./program.bend as P
import ./api.bend as Api

# ---- dispatch result (internal) ----
type DRes is Data:
  DR{obs: List<&2, T.Obs>, acts: List<&2, T.Act>,
     pending: Maybe<&2, T.Pending>, turn: Maybe<&2, T.Turn>,
     notifs: List<&2, String>}

# ---- small pure helpers ----

def inject_users(texts: List<&2, String>) -> List<&2, T.Msg>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.injected_user(h), inject_users(rest)}

def steer_obs(texts: List<&2, String>) -> List<&2, T.Obs>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.Steered{h}, steer_obs(rest)}

def notif_obs(texts: List<&2, String>) -> List<&2, T.Obs>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.NotifDelivered{h}, notif_obs(rest)}

# the role of the first message of a projected input
def head_role(input: List<&2, T.Msg>) -> Maybe<&2, T.Role>:
  match input:
    case Nil{}:
      None{}
    case Con{m, _}:
      match m:
        case T.Msg{role, _, _, _}:
          Some{role}

# the projected model input: system message + canonical history
def model_input(+cfg: T.Cfg, +history: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match cfg:
    case T.Cfg{_, _, _, system, _}:
      Con{T.sys(system), history}

def calls_count(calls: List<&2, T.Call>) -> Nat:
  match calls:
    case Nil{}:
      0n
    case Con{_, rest}:
      Nat.add(1n, calls_count(rest))

def tool_acts(+calls: List<&2, T.Call>, +base: U32) -> List<&2, T.Act>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      Con{T.ToolAct{base, c}, tool_acts(rest, U32.add(base, 1))}

# provider replies number calls per completion (1, 2, ...), so sequential
# turns would repeat ids on the wire; the Core renumbers calls to their
# action ids — ToolAct, PTools and history all agree, and the wire pairs
# each tool result with exactly one call
def renumber(+calls: List<&2, T.Call>, +base: U32) -> List<&2, T.Call>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match c:
        case T.Call{_, +name, +args}:
          Con{T.Call{base, name, args}, renumber(rest, U32.add(base, 1))}

def tool_start_obs(+calls: List<&2, T.Call>) -> List<&2, T.Obs>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      match c:
        case T.Call{cid, _, _}:
          Con{T.ToolStarted{cid}, tool_start_obs(rest)}

# node_program is the runtime's internal name for the live V8 engine
# (run_typescript renamed on the way in); the model called run_typescript,
# so every model-visible surface says run_typescript
def result_name.pick(is_np: Bool, +name: String) -> String:
  match is_np:
    case True{}:
      "run_typescript"
    case False{}:
      name

def result_name(+name: String) -> String:
  result_name.pick(String.eq(name, "node_program"), name)

def tool_result_msg(+name: String, +out: String, ok: Bool) -> T.Msg:
  match ok:
    case True{}:
      T.tool_res(String.append("tool ", String.append(result_name(name),
        String.append(" ok: ", out))))
    case False{}:
      T.tool_res(String.append("tool ", String.append(result_name(name),
        String.append(" failed: ", out))))

# one tool call taken out of the waiting list, with the remainder
type Taken is Data:
  Got{c: T.Call, rest: List<&2, T.Call>}

# ---- programmatic tool calling (D1) ----

def call_name(c: T.Call) -> String:
  match c:
    case T.Call{_, name, _}:
      name

def call_is_prog(c: T.Call) -> Bool:
  String.eq(call_name(c), "run_typescript")

def contains_run_program(calls: List<&2, T.Call>) -> Bool:
  match calls:
    case Nil{}:
      False{}
    case Con{c, rest}:
      Bool.or(call_is_prog(c), contains_run_program(rest))

def take_call.put(hit: Bool, +c: T.Call, +rest: List<&2, T.Call>,
    tail: Maybe<&2, Taken>) -> Maybe<&2, Taken>:
  match hit:
    case True{}:
      Some{Got{c, rest}}
    case False{}:
      tail

def take_call(waiting: List<&2, T.Call>, +id: U32) -> Maybe<&2, Taken>:
  match waiting:
    case Nil{}:
      None{}
    case +c <> +rest:
      match c:
        case T.Call{cid, _, _}:
          take_call.put(U32.is_eq(cid, id), c, rest, take_call(rest, id))

def add_steer(turn: Maybe<&2, T.Turn>, +text: String) -> Maybe<&2, T.Turn>:
  match turn:
    case None{}:
      None{}
    case Some{t}:
      match t:
        case T.Turn{nulls, steer, notifs}:
          Some{T.Turn{nulls, Con{text, steer}, notifs}}

def add_notif(turn: Maybe<&2, T.Turn>, +text: String) -> Maybe<&2, T.Turn>:
  match turn:
    case None{}:
      None{}
    case Some{t}:
      match t:
        case T.Turn{nulls, steer, notifs}:
          Some{T.Turn{nulls, steer, Con{text, notifs}}}

# ---- session field updates ----

def set_turn(+s: T.Session, turn: Maybe<&2, T.Turn>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, _, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending, queued, notifs}

def set_pending(+s: T.Session, p: Maybe<&2, T.Pending>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, p, queued, notifs}

def bump_inputs(+s: T.Session) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, Nat.add(inputs, 1n), actions, turn, pending, queued, notifs}

def queue_msg(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending,
        List.append(&2, String, queued, Con{text, Nil{}}), notifs}

def steer_session(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, add_steer(turn, text), pending,
        queued, notifs}

def notif_turn(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, add_notif(turn, text), pending,
        queued, notifs}

def hold_notif(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending, queued,
        Con{text, notifs}}

def start_turn(+text: String, +s: T.Session) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, _, _, +queued, +notifs}:
      T.Sess{cfg, List.append(&2, T.Msg, history, Con{T.user(text), Nil{}}),
        Nat.add(inputs, 1n), actions,
        Some{T.Turn{0n, Nil{}, Nil{}}}, None{}, queued, notifs}

# ---- compaction start (shared by automatic preflight and manual command) ----

def start_compaction.too_large(auto: Bool, turn: Maybe<&2, T.Turn>,
    +steer: List<&2, String>, +notifs: List<&2, String>) -> DRes:
  match auto:
    case True{}:
      # in-turn: the active turn fails
      DR{Con{T.CompactionFailed{"compaction_request_too_large"},
          Con{T.TurnDone{"failed: compaction_request_too_large"},
            List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))}},
        Nil{}, None{}, None{}, Nil{}}
    case False{}:
      # manual between-turn compaction returns idle
      DR{Con{T.CompactionFailed{"compaction_request_too_large"},
          List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))},
        Nil{}, None{}, turn, Nil{}}

def start_compaction.project(+auto: Bool, +extra: String, +actions: U32,
    turn: Maybe<&2, T.Turn>, +steer: List<&2, String>,
    +notifs: List<&2, String>, p: C.PRes) -> DRes:
  match p:
    case C.POk{+input}:
      # ADR 0011: the first request uses the compaction id as its action id
      DR{Con{T.CompactionStarted{actions, auto},
          List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))},
        Con{T.CompactionCall{actions, input}, Nil{}},
        Some{T.PComp{actions, 1n, actions, extra, auto}},
        turn, Nil{}}
    case C.PBig{}:
      start_compaction.too_large(auto, turn, steer, notifs)

def start_compaction(auto: Bool, +extra: String, +cfg: T.Cfg,
    +history: List<&2, T.Msg>, +actions: U32, turn: Maybe<&2, T.Turn>,
    +steer: List<&2, String>, +notifs: List<&2, String>) -> DRes:
  start_compaction.project(auto, extra, actions, turn, steer, notifs,
    C.project(cfg, history, extra))

# ---- dispatch: the seam before every agent completion ----
# Steering and notifications enter model context here (model-safe
# boundary), then ADR 0011 preflight budgets the complete input.

def dispatch.pick(fits: Bool, +cfg: T.Cfg, +history: List<&2, T.Msg>,
    +actions: U32, +nulls: Nat, +steer: List<&2, String>,
    +notifs: List<&2, String>) -> DRes:
  match fits:
    case True{}:
      DR{List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs)),
        Con{T.AgentCall{actions, model_input(cfg, history)}, Nil{}},
        Some{T.PAgent{actions}},
        Some{T.Turn{nulls, Nil{}, Nil{}}}, Nil{}}
    case False{}:
      # oversized input starts compaction before the agent Action
      start_compaction(True{}, "", cfg, history, actions,
        Some{T.Turn{nulls, Nil{}, Nil{}}}, steer, notifs)

def dispatch.thresh(+threshold: Nat, +cfg: T.Cfg, +history: List<&2, T.Msg>,
    +actions: U32, turn: Maybe<&2, T.Turn>, +sess_notifs: List<&2, String>)
    -> DRes:
  match turn:
    case None{}:
      DR{Nil{}, Nil{}, None{}, None{}, Nil{}}
    case Some{t}:
      match t:
        case T.Turn{+nulls, +steer, +tnotifs}:
          +all_notifs = List.append(&2, String, sess_notifs, tnotifs)
          +history2 = List.append(&2, T.Msg,
            List.append(&2, T.Msg, history, inject_users(steer)),
            inject_users(all_notifs))
          dispatch.pick(Nat.is_lt(E.input_tokens(cfg, history2), threshold),
            cfg, history2, actions, nulls, steer, all_notifs)

def dispatch.turn(+cfg: T.Cfg, +history: List<&2, T.Msg>, +actions: U32,
    turn: Maybe<&2, T.Turn>, +sess_notifs: List<&2, String>) -> DRes:
  match cfg:
    case T.Cfg{+threshold, _, _, _, _}:
      dispatch.thresh(threshold, cfg, history, actions, turn, sess_notifs)

def dispatch(+s: T.Session) -> DRes:
  match s:
    case T.Sess{+cfg, +history, _, +actions, +turn, _, _, +notifs}:
      dispatch.turn(cfg, history, actions, turn, notifs)

# wrap a dispatch result as an Accepted apply result; the emitted action id
# is consumed, so the next action id moves on by one
def dispatch_result(+d: DRes, +pre: List<&2, T.Obs>, +s: T.Session) -> T.Res:
  match d:
    case DR{obs, acts, pending, turn, notifs}:
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, _, _, +queued, _}:
          T.Accepted{List.append(&2, T.Obs, pre, obs), acts,
            T.Sess{cfg, history, inputs, U32.add(actions, 1), turn, pending,
              queued, notifs}}

# ---- completion result handlers ----

def apply.completion.too_large(auto: Bool, +code: String, +s: T.Session) -> T.Res:
  match auto:
    case True{}:
      T.Accepted{Con{T.CompactionFailed{code},
          Con{T.TurnDone{String.append("failed: ", code)}, Nil{}}},
        Nil{}, bump_inputs(set_turn(set_pending(s, None{}), None{}))}
    case False{}:
      T.Accepted{Con{T.CompactionFailed{code}, Nil{}}, Nil{},
        bump_inputs(set_pending(s, None{}))}

def apply.completion.resume(auto: Bool, +s: T.Session, +summary: String) -> T.Res:
  match auto:
    case True{}:
      # the turn continues on the replacement context; the first agent
      # Action after compaction uses the full new input
      d = dispatch(s)
      dispatch_result(d, Con{T.CompactionDone{summary}, Nil{}}, bump_inputs(s))
    case False{}:
      T.Accepted{Con{T.CompactionDone{summary}, Nil{}}, Nil{},
        bump_inputs(set_pending(s, None{}))}

def apply.completion.accepted(r: C.RRes, +auto: Bool, +summary: String,
    +s: T.Session) -> T.Res:
  match r:
    case C.ROk{+h2}:
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          apply.completion.resume(auto,
            T.Sess{cfg, h2, inputs, actions, turn, None{}, queued, notifs},
            summary)
    case C.RBig{}:
      apply.completion.too_large(auto, "compaction_replacement_too_large", s)

def apply.completion.malformed.project(p: C.PRes, +action: U32,
    +compaction: U32, +extra: String, +auto: Bool, +s: T.Session) -> T.Res:
  match p:
    case C.POk{+input}:
      match s:
        case T.Sess{+cfg, +history, +inputs, _, +turn, _, +queued, +notifs}:
          # one format retry: new action id derived from the stable
          # compaction id, attempt 2, same fitted projection
          T.Accepted{Con{T.CandidateDiscarded{"invalid_compaction_summary"},
              Nil{}},
            Con{T.CompactionCall{action, input}, Nil{}},
            T.Sess{cfg, history, Nat.add(inputs, 1n), U32.add(action, 1),
              turn, Some{T.PComp{compaction, 2n, action, extra, auto}},
              queued, notifs}}
    case C.PBig{}:
      apply.completion.too_large(auto, "compaction_request_too_large", s)

def apply.completion.malformed(retry: Bool, +compaction: U32, +extra: String,
    +auto: Bool, +s: T.Session) -> T.Res:
  match retry:
    case True{}:
      match s:
        case T.Sess{+cfg, +history, _, +actions, _, _, _, _}:
          apply.completion.malformed.project(
            C.project(cfg, history, extra), U32.add(actions, 1),
            compaction, extra, auto, s)
    case False{}:
      apply.completion.too_large(auto, "invalid_compaction_summary", s)

def apply.completion.reply(v: Maybe<&2, String>, +auto: Bool,
    +compaction: U32, +attempt: Nat, +extra: String, +s: T.Session) -> T.Res:
  match v:
    case Some{+summary}:
      match s:
        case T.Sess{+cfg, +history, _, _, _, _, _, _}:
          apply.completion.accepted(C.replace(cfg, history, summary), auto,
            summary, s)
    case None{}:
      apply.completion.malformed(Nat.is_lt(attempt, 3n), compaction, extra,
        auto, s)

def apply.completion.comp(matched: Bool, +text: String,
    +calls: List<&2, T.Call>, +compaction: U32, +attempt: Nat,
    +extra: String, +auto: Bool, +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      apply.completion.reply(C.valid_summary(text, calls), auto, compaction,
        attempt, extra, s)

def cfg_max_nulls(+cfg: T.Cfg) -> Nat:
  match cfg:
    case T.Cfg{_, _, max_nulls, _, _}:
      max_nulls

def apply.completion.tools(+text: String, +calls: List<&2, T.Call>,
    +s: T.Session) -> T.Res:
  # commit the assistant message, emit one Action per tool call, and wait
  # for every call id before the next dispatch
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      # the started obs carries the same action ids as the ToolActs (and
      # as the finished obs), so every consumer can pair by id
      T.Accepted{Con{T.Assistant{text},
          tool_start_obs(renumber(calls, U32.add(actions, 1)))},
        tool_acts(renumber(calls, U32.add(actions, 1)), U32.add(actions, 1)),
        T.Sess{cfg, List.append(&2, T.Msg, history,
            Con{T.asst(text, renumber(calls, U32.add(actions, 1))), Nil{}}),
          Nat.add(inputs, 1n),
          U32.add(actions, U32.from_nat(Nat.add(calls_count(calls), 1n))),
          turn, Some{T.PTools{renumber(calls, U32.add(actions, 1))}}, queued, notifs}}

# a program that failed to parse, or finished with no calls: the assistant
# message and the failed result commit together, then the turn continues
def apply.prog.fail.body(+id: U32, +text: String, +c: T.Call, +why: String,
    +s: T.Session) -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      +s2 : T.Session = T.Sess{cfg,
        List.append(&2, T.Msg, List.append(&2, T.Msg, history,
          Con{T.asst(text, Con{c, Nil{}}), Nil{}}),
          Con{tool_result_msg("run_typescript", why, False{}), Nil{}}),
        Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}
      d = dispatch(s2)
      dispatch_result(d,
        Con{T.Assistant{text}, Con{T.ToolStarted{id},
        Con{T.ToolFinished{id, False{}},
        Con{T.ToolCommitted{id}, Nil{}}}}}, s2)

def apply.prog.fail(+text: String, +c: T.Call, +why: String,
    +s: T.Session) -> T.Res:
  match c:
    case T.Call{+id, _, _}:
      apply.prog.fail.body(id, text, c, why, s)

# the program finished: its result is the run_program tool result
def apply.prog.done.body(+id: U32, +text: String, +c: T.Call, +out: String,
    +s: T.Session) -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      +s2 : T.Session = T.Sess{cfg,
        List.append(&2, T.Msg, List.append(&2, T.Msg, history,
          Con{T.asst(text, Con{c, Nil{}}), Nil{}}),
          Con{tool_result_msg("run_typescript", out, True{}), Nil{}}),
        Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}
      d = dispatch(s2)
      dispatch_result(d,
        Con{T.Assistant{text}, Con{T.ToolStarted{id},
        Con{T.ToolFinished{id, True{}},
        Con{T.ToolCommitted{id}, Nil{}}}}}, s2)

def apply.prog.done(+text: String, +c: T.Call, +out: String,
    +s: T.Session) -> T.Res:
  match c:
    case T.Call{+id, _, _}:
      apply.prog.done.body(id, text, c, out, s)

# the program stopped at its first call: emit it and keep the program
def apply.prog.first.wait2(+id: U32, +name: String, +args: String,
    +text: String, +c: T.Call, +prog: List<&2, T.Stmt>, +s: T.Session)
    -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      +fresh = U32.add(actions, 1)
      T.Accepted{
        Con{T.Assistant{text}, Con{T.ToolStarted{id},
          Con{T.ToolStarted{fresh}, Nil{}}}},
        Con{T.ToolAct{fresh, T.Call{fresh, name, args}}, Nil{}},
        T.Sess{cfg, List.append(&2, T.Msg, history,
          Con{T.asst(text, Con{c, Nil{}}), Nil{}}),
          Nat.add(inputs, 1n), U32.add(fresh, 1), turn,
          Some{T.PProg{prog, Nil{}, id, fresh}}, queued, notifs}}

def apply.prog.first.wait(+name: String, +args: String, +text: String,
    +c: T.Call, +prog: List<&2, T.Stmt>, +s: T.Session) -> T.Res:
  match c:
    case T.Call{+id, _, _}:
      apply.prog.first.wait2(id, name, args, text, c, prog, s)

# the first interpreter verdict: wait on a call, or finish immediately
def apply.prog.first(pr: P.PR, +text: String, +c: T.Call,
    +prog: List<&2, T.Stmt>, +s: T.Session) -> T.Res:
  match pr:
    case P.PDone{+out}:
      apply.prog.done(text, c, out, s)
    case P.PWait{+name, +args}:
      apply.prog.first.wait(name, args, text, c, prog, s)
    case P.PFail{+why}:
      apply.prog.fail(text, c, why, s)

def apply.prog.start.parsed(p: Maybe<&2, List<&2, T.Stmt>>, +text: String,
    +c: T.Call, +s: T.Session) -> T.Res:
  match p:
    case None{}:
      apply.prog.fail(text, c, "bad program", s)
    case Some{+prog}:
      apply.prog.first(P.run(prog, Nil{}), text, c, prog, s)

# the model asked to run a program: parse it and interpret to the first
# missing tool result
def apply.prog.start(+text: String, +c: T.Call, +s: T.Session) -> T.Res:
  match c:
    case T.Call{_, _, +args}:
      apply.prog.start.parsed(P.parse(Api.call_code(args)), text, c, s)

# a tool result arrived for a program in flight: replay with the record
def apply.prog.resume.pfail(+call: U32, +run: U32, +why: String,
    +s: T.Session) -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      +s2 : T.Session = T.Sess{cfg,
        List.append(&2, T.Msg, history,
          Con{tool_result_msg("run_typescript", why, False{}), Nil{}}),
        Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}
      d = dispatch(s2)
      dispatch_result(d,
        Con{T.ToolFinished{call, True{}},
        Con{T.ToolCommitted{call},
        Con{T.ToolFinished{run, False{}},
        Con{T.ToolCommitted{run}, Nil{}}}}}, s2)

def apply.prog.resume(pr: P.PR, +call: U32, +out: String,
    +prog: List<&2, T.Stmt>, +results: List<&2, String>, +run: U32,
    +s: T.Session) -> T.Res:
  match pr:
    case P.PFail{+why}:
      apply.prog.resume.pfail(call, run, why, s)
    case P.PWait{+name, +args}:
      # still orchestral: emit the next call and keep the program pending
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          +fresh = U32.add(actions, 1)
          T.Accepted{
            Con{T.ToolFinished{call, True{}},
              Con{T.ToolCommitted{call},
              Con{T.ToolStarted{fresh}, Nil{}}}},
            Con{T.ToolAct{fresh, T.Call{fresh, name, args}}, Nil{}},
            T.Sess{cfg, history, Nat.add(inputs, 1n), U32.add(fresh, 1),
              turn, Some{T.PProg{prog, results, run, fresh}}, queued, notifs}}
    case P.PDone{+out2}:
      # the program returned: commit its output as the run_program result
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          +s2 : T.Session = T.Sess{cfg,
            List.append(&2, T.Msg, history,
              Con{tool_result_msg("run_typescript", out2, True{}), Nil{}}),
            Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}
          d = dispatch(s2)
          dispatch_result(d,
            Con{T.ToolFinished{call, True{}},
            Con{T.ToolCommitted{call},
            Con{T.ToolFinished{run, True{}},
            Con{T.ToolCommitted{run}, Nil{}}}}}, s2)

def apply.prog.matched(matched: Bool, +call: U32, +out: String,
    +prog: List<&2, T.Stmt>, +results: List<&2, String>, +run: U32,
    +wait: U32, +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      # a late result cannot complete the current program step
      T.Rejected{"stale tool call id"}
    case True{}:
      +results2 = List.append(&2, String, results, Con{out, Nil{}})
      apply.prog.resume(P.run(prog, results2), call, out, prog, results2,
        run, s)

def apply.prog.mismatch(matched: Bool, +call: U32, +run: U32,
    +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale tool call id"}
    case True{}:
      # a failed program call fails the program
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          +s2 : T.Session = T.Sess{cfg,
            List.append(&2, T.Msg, history,
              Con{tool_result_msg("run_typescript", "a program call failed",
                False{}), Nil{}}),
            Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}
          d = dispatch(s2)
          dispatch_result(d,
            Con{T.ToolFinished{call, False{}},
            Con{T.ToolCommitted{call},
            Con{T.ToolFinished{run, False{}},
            Con{T.ToolCommitted{run}, Nil{}}}}}, s2)

def apply.prog.result(ok: Bool, +call: U32, +out: String,
    +prog: List<&2, T.Stmt>, +results: List<&2, String>, +run: U32,
    +wait: U32, +s: T.Session) -> T.Res:
  match ok:
    case True{}:
      apply.prog.matched(U32.is_eq(call, wait), call, out, prog, results,
        run, wait, s)
    case False{}:
      apply.prog.mismatch(U32.is_eq(call, wait), call, run, s)

# a completion carries either an ordinary tool batch, or a single program
def apply.completion.sole.pick(is_prog: Bool, +c: T.Call, +text: String,
    +s: T.Session) -> T.Res:
  match is_prog:
    case True{}:
      apply.prog.start(text, c, s)
    case False{}:
      apply.completion.tools(text, Con{c, Nil{}}, s)

def apply.completion.sole(+c: T.Call, +text: String, +s: T.Session) -> T.Res:
  apply.completion.sole.pick(call_is_prog(c), c, text, s)

def apply.completion.multi.pick(has_prog: Bool,
    +calls: List<&2, T.Call>, +text: String, +s: T.Session) -> T.Res:
  match has_prog:
    case True{}:
      T.Rejected{"run_typescript must be the only call"}
    case False{}:
      apply.completion.tools(text, calls, s)

def apply.completion.multi(+calls: List<&2, T.Call>, +text: String,
    +s: T.Session) -> T.Res:
  apply.completion.multi.pick(contains_run_program(calls), calls, text, s)

def apply.completion.null2(over: Bool, +nulls: Nat, +s: T.Session) -> T.Res:
  match over:
    case True{}:
      T.Accepted{Con{T.NullIteration{},
          Con{T.TurnDone{"failed: null iteration limit"}, Nil{}}},
        Nil{}, bump_inputs(set_turn(set_pending(s, None{}), None{}))}
    case False{}:
      # retry within the limit
      +s1 = bump_inputs(set_turn(s, Some{T.Turn{Nat.add(nulls, 1n), Nil{}, Nil{}}}))
      d = dispatch(s1)
      dispatch_result(d, Con{T.NullIteration{}, Nil{}}, s1)

def apply.completion.empty(+s: T.Session) -> T.Res:
  match s:
    case T.Sess{+cfg, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          T.Rejected{"no active turn"}
        case Some{t}:
          match t:
            case T.Turn{+nulls, _, _}:
              apply.completion.null2(
                Bool.not(Nat.is_le(Nat.add(nulls, 1n), cfg_max_nulls(cfg))),
                nulls, s)

def apply.completion.plain2(empty: Bool, +text: String, +s: T.Session) -> T.Res:
  match empty:
    case False{}:
      # a final completion: commit and end the turn
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, _, _, +queued, +notifs}:
          T.Accepted{Con{T.Assistant{text},
              Con{T.TurnDone{"completed"}, Nil{}}},
            Nil{},
            T.Sess{cfg, List.append(&2, T.Msg, history,
              Con{T.asst(text, Nil{}), Nil{}}),
              Nat.add(inputs, 1n), actions, None{}, None{}, queued, notifs}}
    case True{}:
      # ADR 0015: an empty candidate commits nothing and emits no action
      apply.completion.empty(s)

def apply.completion.agent(matched: Bool, +text: String,
    +calls: List<&2, T.Call>, +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      match calls:
        case Nil{}:
          apply.completion.plain2(String.is_empty(String.trim(text)), text, s)
        case Con{c, Nil{}}:
          apply.completion.sole(c, text, s)
        case Con{_, Con{_, _}}:
          apply.completion.multi(calls, text, s)

def apply.completion.pending(action: U32, +text: String,
    +calls: List<&2, T.Call>, +s: T.Session, +p: T.Pending) -> T.Res:
  match p:
    case T.PAgent{a}:
      apply.completion.agent(U32.is_eq(a, action), text, calls, s)
    case T.PTools{_}:
      T.Rejected{"completion while tools pending"}
    case T.PComp{+compaction, +attempt, a, +extra, +auto}:
      apply.completion.comp(U32.is_eq(a, action), text, calls, compaction,
        attempt, extra, auto, s)
    case T.PProg{_, _, _, _}:
      T.Rejected{"program pending, not a completion"}

def apply.completion(action: U32, +text: String, +calls: List<&2, T.Call>,
    +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending completion"}
        case Some{p}:
          apply.completion.pending(action, text, calls, s, p)

# ---- completion failure ----

def apply.completion_fail.agent(matched: Bool, +error: String,
    +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      # the candidate is discarded without entering model context; an
      # ordinary provider failure fails the active turn
      T.Accepted{Con{T.CandidateDiscarded{error},
          Con{T.TurnDone{String.append("failed: ", error)}, Nil{}}},
        Nil{}, bump_inputs(set_turn(set_pending(s, None{}), None{}))}

def apply.completion_fail.comp(matched: Bool, +error: String, +auto: Bool,
    +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      apply.completion.too_large(auto, String.append("provider failure: ", error), s)

def apply.completion_fail.pending(action: U32, +error: String,
    +s: T.Session, +p: T.Pending) -> T.Res:
  match p:
    case T.PAgent{a}:
      apply.completion_fail.agent(U32.is_eq(a, action), error, s)
    case T.PTools{_}:
      T.Rejected{"completion while tools pending"}
    case T.PComp{+compaction, +attempt, a, +extra, +auto}:
      apply.completion_fail.comp(U32.is_eq(a, action), error, auto, s)
    case T.PProg{_, _, _, _}:
      T.Rejected{"program pending, not a completion"}

def apply.completion_fail(action: U32, +error: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending completion"}
        case Some{p}:
          apply.completion_fail.pending(action, error, s, p)

# ---- tool results ----

def apply.tool.done(+rest: List<&2, T.Call>, +call: U32, ok: Bool,
    +s1: T.Session) -> T.Res:
  match rest:
    case Nil{}:
      # every call answered: the next dispatch decides what happens next
      d = dispatch(s1)
      dispatch_result(d,
        Con{T.ToolFinished{call, ok}, Con{T.ToolCommitted{call}, Nil{}}},
        bump_inputs(s1))
    case Con{_, _}:
      T.Accepted{Con{T.ToolFinished{call, ok}, Con{T.ToolCommitted{call}, Nil{}}},
        Nil{}, bump_inputs(s1)}

def apply.tool.commit(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +c: T.Call, +rest: List<&2, T.Call>) -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      match c:
        case T.Call{_, +name, _}:
          apply.tool.done(rest, call, ok,
            T.Sess{cfg, List.append(&2, T.Msg, history,
              Con{tool_result_msg(name, out, ok), Nil{}}), inputs,
              actions, turn, Some{T.PTools{rest}}, queued, notifs})

def apply.tool.found(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +found: Taken) -> T.Res:
  match found:
    case Got{c, rest}:
      apply.tool.commit(call, out, ok, s, c, rest)

def apply.tool.take(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +waiting: List<&2, T.Call>, taken: Maybe<&2, Taken>) -> T.Res:
  match taken:
    case None{}:
      # a late result cannot complete the current work
      T.Rejected{"unknown tool call id"}
    case Some{found}:
      apply.tool.found(call, out, ok, s, found)

def apply.tool.pending(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +p: T.Pending) -> T.Res:
  match p:
    case T.PTools{+waiting}:
      apply.tool.take(call, out, ok, s, waiting, take_call(waiting, call))
    case T.PAgent{_}:
      T.Rejected{"completion pending, not tools"}
    case T.PComp{_, _, _, _, _}:
      T.Rejected{"compaction pending, not tools"}
    case T.PProg{+prog, +results, +run, +wait}:
      apply.prog.result(ok, call, out, prog, results, run, wait, s)

def apply.tool(+call: U32, +out: String, +ok: Bool, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending tool result"}
        case Some{p}:
          apply.tool.pending(call, out, ok, s, p)

# ---- user messages ----

def apply.user.start(+text: String, +s: T.Session) -> T.Res:
  +s1 = start_turn(text, s)
  d = dispatch(s1)
  dispatch_result(d, Con{T.TurnStarted{}, Nil{}}, s1)

def apply.user.busy(mode: T.Mode, +text: String, +s: T.Session) -> T.Res:
  match mode:
    case T.Queue{}:
      # a queued message does not supersede in-flight work; it starts the
      # next turn
      T.Accepted{Nil{}, Nil{}, bump_inputs(queue_msg(s, text))}
    case T.Steer{}:
      # ADR 0005: steering is pending input, inserted at the next
      # model-safe boundary
      T.Accepted{Con{T.SteerReceived{text}, Nil{}}, Nil{},
        bump_inputs(steer_session(s, text))}

def apply.user.idle(mode: T.Mode, +text: String, +s: T.Session,
    turn: Maybe<&2, T.Turn>) -> T.Res:
  match turn:
    case None{}:
      # idle: a user message starts a turn
      apply.user.start(text, s)
    case Some{t}:
      apply.user.busy(mode, text, s)

def apply.user.state(mode: T.Mode, +text: String, +s: T.Session,
    turn: Maybe<&2, T.Turn>, pending: Maybe<&2, T.Pending>) -> T.Res:
  match pending:
    case None{}:
      apply.user.idle(mode, text, s, turn)
    case Some{p}:
      apply.user.busy(mode, text, s)

def apply.user(mode: T.Mode, +text: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      apply.user.state(mode, text, s, turn, pending)

# ---- notifications ----

def apply.notify.idle(pending: Maybe<&2, T.Pending>, +text: String,
    +s: T.Session) -> T.Res:
  match pending:
    case None{}:
      # idle: the notification enters model context immediately
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          T.Accepted{Con{T.NotifReceived{text},
              Con{T.NotifDelivered{text}, Nil{}}},
            Nil{},
            T.Sess{cfg,
              List.append(&2, T.Msg, history,
                Con{T.injected_user(
                  String.append("[notification] ", text)), Nil{}}),
              Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}}
    case Some{p}:
      # held while a between-turn compaction is pending; delivered at the
      # next dispatch
      T.Accepted{Con{T.NotifReceived{text}, Nil{}}, Nil{},
        bump_inputs(hold_notif(s, text))}

def apply.notify(+text: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      match turn:
        case Some{t}:
          # received during a turn: enters model context at the next
          # model-safe boundary
          T.Accepted{Con{T.NotifReceived{text}, Nil{}}, Nil{},
            bump_inputs(notif_turn(s, text))}
        case None{}:
          apply.notify.idle(pending, text, s)

# ---- manual compaction ----

def apply.compact.idle(pending: Maybe<&2, T.Pending>, +extra: String,
    +s: T.Session) -> T.Res:
  match pending:
    case Some{p}:
      T.Rejected{"compaction already pending"}
    case None{}:
      match s:
        case T.Sess{+cfg, +history, _, +actions, _, _, _, +notifs}:
          +d = start_compaction(False{}, extra, cfg, history, actions, None{},
            Nil{}, notifs)
          dispatch_result(d, Nil{}, bump_inputs(s))

def apply.compact(+extra: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      match turn:
        case Some{t}:
          T.Rejected{"session busy: compact between turns"}
        case None{}:
          apply.compact.idle(pending, extra, s)

# ---- interrupt and fail_turn ----

def apply.interrupt(+s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case Some{t}:
          T.Accepted{Con{T.CandidateDiscarded{"interrupt"},
              Con{T.TurnDone{"interrupted"}, Nil{}}},
            Nil{}, bump_inputs(set_turn(set_pending(s, None{}), None{}))}
        case None{}:
          # interrupting an inactive session is an accepted idempotent no-op
          T.Accepted{Nil{}, Nil{}, bump_inputs(s)}

def apply.fail(+error: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case Some{t}:
          T.Accepted{Con{T.TurnDone{String.append("failed: ", error)}, Nil{}},
            Nil{}, bump_inputs(set_turn(set_pending(s, None{}), None{}))}
        case None{}:
          T.Rejected{"no active turn"}

# ---- the step protocol entry point ----

def apply.cmd(cmd: T.Cmd, +s: T.Session) -> T.Res:
  match cmd:
    case T.UserMsg{mode, text}:
      apply.user(mode, text, s)
    case T.Notify{text}:
      apply.notify(text, s)
    case T.Compact{extra}:
      apply.compact(extra, s)
    case T.CompletionOk{action, text, calls}:
      apply.completion(action, text, calls, s)
    case T.CompletionFail{action, error}:
      apply.completion_fail(action, error, s)
    case T.ToolOk{call, out}:
      apply.tool(call, out, True{}, s)
    case T.ToolFail{call, error}:
      apply.tool(call, error, False{}, s)
    case T.Interrupt{}:
      apply.interrupt(s)
    case T.FailTurn{error}:
      apply.fail(error, s)

def apply(s: T.Session, cmd: T.Cmd) -> T.Res:
  apply.cmd(cmd, s)
