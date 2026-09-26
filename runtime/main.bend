# runtime/main.bend — the Harness Runtime.
#
# Executes Core-emitted Actions and feeds normalized results back as
# commands (ADR 0006: the Core is driven only through the Step Protocol).
#
# The loop splits in two:
# - transition: a PURE function from (runtime state, job) to (effect, next
#   state, next job). It decides everything: applies commands, picks model
#   replies, runs pure tools, validates sleep bounds.
# - run: the only IO cycle. It performs the effect (print, sleep) and
#   recurses with less fuel, so the Runtime terminates like the Core.
#
# The provider adapter is scripted, which makes every scenario
# deterministic; a real adapter would replace runtime/model.bend only.
import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/checkpoint.bend as K
import ../core/obs.bend as O
import ../core/api.bend as Api
import ../core/compaction.bend as C
import ./model.bend as M
import ./tools.bend as X
import ./persist.bend as P
import ./remote.bend as Rem
import ./mcp.bend as Mcp
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http
import 0xbf477e663cf4acb1369a68e0f0fa713b/encoding/encoding.bend as Enc
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
import 0x16458a2db4f36577294543ec2a6d420c/json.bend as Jv

# ---- runtime state and loop vocabulary ----

type Rt is Data:
  Rt{sess: T.Session, script: List<&2, M.Reply>, cmds: List<&2, T.Cmd>,
     live: Bool}

type Job is Data:
  # apply one command, then execute the `after` acts
  JCmd{cmd: T.Cmd, after: List<&2, T.Act>}
  JActs{acts: List<&2, T.Act>}
  JIdle{}
  JDone{}

type Effect is Data:
  EPrints{lines: List<&2, String>}
  # the id and seconds ride along so the loop can annotate the feed
  ESleep{ms: U32, id: U32, secs: U32}
  # a real provider call: the hub HTTP client, in this process
  EModel{action: U32, request: String, after: List<&2, T.Act>}
  # a real tool execution (bash, the merged search, remote MCP calls).
  # The live flag keeps scripted scenarios hermetic: no MCP index reads.
  EExec{call: T.Call, after: List<&2, T.Act>, live: Bool}
  ENone{}

type Trans is Data:
  Tr{effect: Effect, rt: Rt, job: Job}

# where the loop's output goes: stdout, or a REPL socket. Sockets are
# affine handles, so Out is a Type: it is threaded, never copied.
type Out is Type:
  OStd{}
  ONet{sock: Socket}

# what the loop hands back when it stops
type RtEnd is Type:
  REnd{sess: T.Session, out: Out}

# ---- observation rendering (pure) ----

def obs_lines(os: List<&2, T.Obs>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case Con{o, rest}:
      Con{O.show_obs(o), obs_lines(rest)}

# ---- the pure transition ----

def trans.tool.bounds(+rt: Rt, +rest: List<&2, T.Act>, +id: U32, +secs: U32,
    valid: Bool) -> Trans:
  match valid:
    case True{}:
      Tr{ESleep{U32.mul(secs, 1000), id, secs}, rt,
        JCmd{T.ToolOk{id,
          String.append("slept ", String.append(U32.show(secs), "s"))}, rest}}
    case False{}:
      Tr{ENone{}, rt,
        JCmd{T.ToolFail{id, "sleep requires 0 < seconds < 60"}, rest}}

def trans.tool.seconds(+rt: Rt, +rest: List<&2, T.Act>, +id: U32,
    s: Maybe<&2, U32>) -> Trans:
  match s:
    case None{}:
      Tr{ENone{}, rt,
        JCmd{T.ToolFail{id, "sleep needs a number of seconds"}, rest}}
    case Some{+secs}:
      trans.tool.bounds(rt, rest, id, secs,
        Bool.and(U32.is_lt(0, secs), U32.is_lt(secs, 60)))

def rt_live(+rt: Rt) -> Bool:
  match rt:
    case Rt{_, _, _, +live}:
      live

# every tool except the sleep timer is a Runtime effect: bash, the
# merged search (it reads the MCP index), remote MCP tools resolved by
# name, and unknown names (which fail cleanly with the name)
def trans.tool.is_sleep(+rt: Rt, +rest: List<&2, T.Act>, +id: U32,
    +args: String, +call: T.Call, sleep: Bool, live: Bool) -> Trans:
  match sleep:
    case True{}:
      trans.tool.seconds(rt, rest, id, U32.read(args))
    case False{}:
      Tr{EExec{call, rest, live}, rt, JIdle{}}

def trans.tool(+rt: Rt, +rest: List<&2, T.Act>, +call: T.Call) -> Trans:
  match call:
    case T.Call{+id, +name, +args}:
      trans.tool.is_sleep(rt, rest, id, args, call,
        String.eq(name, "self.sleep"), rt_live(rt))

# live only: run_typescript flows to the runtime as node_program, so the
# V8 engine (bend-jsrt) executes it instead of the in-house interpreter;
# scripted sessions keep the Core's law-checked interpreter
def rename_call.go(hit: Bool, +id: U32, +name: String, +args: String)
    -> T.Call:
  match hit:
    case True{}:
      T.Call{id, "node_program", args}
    case False{}:
      T.Call{id, name, args}

def rename_call(live: Bool, +c: T.Call) -> T.Call:
  match live:
    case True{}:
      match c:
        case T.Call{+id, +name, +args}:
          rename_call.go(String.eq(name, "run_typescript"), id, name, args)
    case False{}:
      c

def rename_calls(+live: Bool, +calls: List<&2, T.Call>)
    -> List<&2, T.Call>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      Con{rename_call(live, c), rename_calls(live, rest)}

def trans.model.ok(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +text: String, +calls: List<&2, T.Call>, +script2: List<&2, M.Reply>)
    -> Trans:
  match rt:
    case Rt{+sess, _, +cmds, +live}:
      Tr{ENone{}, Rt{sess, script2, cmds, live},
        JCmd{T.CompletionOk{action, text, rename_calls(live, calls)},
          rest}}

def trans.model.fail(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +script2: List<&2, M.Reply>) -> Trans:
  match rt:
    case Rt{+sess, _, +cmds, +live}:
      Tr{ENone{}, Rt{sess, script2, cmds, live},
        JCmd{T.CompletionFail{action, "scripted provider failure"}, rest}}

def trans.model.reply(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +reply: M.Reply, +script2: List<&2, M.Reply>) -> Trans:
  match reply:
    case M.ROk{text, calls}:
      trans.model.ok(rt, rest, action, text, calls, script2)
    case M.RFail{}:
      trans.model.fail(rt, rest, action, script2)

def trans.model.step(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    st: M.Stepped) -> Trans:
  match st:
    case M.Stepped{reply, script2}:
      trans.model.reply(rt, rest, action, reply, script2)

def trans.model.live(+sess: T.Session, +rt: Rt, +rest: List<&2, T.Act>,
    +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match sess:
    case T.Sess{+cfg, _, _, _, _, _, _, _}:
      Tr{EModel{action, Rem.build_request(cfg, input), rest}, rt, JIdle{}}

def script_of(+rt: Rt) -> List<&2, M.Reply>:
  match rt:
    case Rt{_, +script, _, _}:
      script

# compaction calls carry no tool catalog: a tool call cannot even be
# offered, so the summary cannot be malformed by one. Live mode sends the
# bare request to the real provider; scripted mode pops the script like
# every other model call, so the compaction scenarios stay deterministic.
def trans.model.compact.live(+sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match sess:
    case T.Sess{+cfg, _, _, _, _, _, _, _}:
      Tr{EModel{action, Rem.build_request.bare(cfg, input), rest}, rt, JIdle{}}

def trans.model.compact.script(+rt: Rt, +rest: List<&2, T.Act>,
    +action: U32) -> Trans:
  trans.model.step(rt, rest, action, M.model_next(script_of(rt)))

def trans.model.compact.pick(live: Bool, +sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match live:
    case True{}:
      trans.model.compact.live(sess, rt, rest, action, input)
    case False{}:
      trans.model.compact.script(rt, rest, action)

def trans.model.compact(+rt: Rt, +rest: List<&2, T.Act>,
    +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match rt:
    case Rt{+sess, _, _, +live}:
      trans.model.compact.pick(live, sess, rt, rest, action, input)

def trans.model.pick(live: Bool, +sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>)
    -> Trans:
  match live:
    case True{}:
      trans.model.live(sess, rt, rest, action, input)
    case False{}:
      trans.model.step(rt, rest, action, M.model_next(script_of(rt)))

def trans.model(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +input: List<&2, T.Msg>) -> Trans:
  match rt:
    case Rt{+sess, _, _, +live}:
      trans.model.pick(live, sess, rt, rest, action, input)

def trans.act(+rt: Rt, +rest: List<&2, T.Act>, +a: T.Act) -> Trans:
  match a:
    case T.AgentCall{action, input}:
      trans.model(rt, rest, action, input)
    case T.CompactionCall{action, input}:
      trans.model.compact(rt, rest, action, input)
    case T.ToolAct{_, call}:
      trans.tool(rt, rest, call)

def trans.idle(+rt: Rt) -> Trans:
  match rt:
    case Rt{+sess, +script, cmds, +live}:
      match cmds:
        case Nil{}:
          Tr{EPrints{Con{"--- idle", Nil{}}}, rt, JDone{}}
        case Con{c, rest}:
          Tr{ENone{}, Rt{sess, script, rest, live}, JCmd{c, Nil{}}}

def trans.acts(+rt: Rt, +acts: List<&2, T.Act>) -> Trans:
  match acts:
    case Nil{}:
      trans.idle(rt)
    case Con{a, rest}:
      trans.act(rt, rest, a)

def trans.res.accepted(+rt: Rt, +after: List<&2, T.Act>, +obs: List<&2, T.Obs>,
    +acts: List<&2, T.Act>, +next: T.Session) -> Trans:
  match rt:
    case Rt{_, +script, +cmds, +live}:
      Tr{EPrints{obs_lines(obs)}, Rt{next, script, cmds, live},
        JActs{List.append(&2, T.Act, acts, after)}}

def trans.res(+rt: Rt, +after: List<&2, T.Act>, res: T.Res) -> Trans:
  match res:
    case T.Rejected{reason}:
      Tr{EPrints{Con{String.append("core rejected: ", reason), Nil{}}},
        rt, JActs{after}}
    case T.Accepted{obs, acts, next}:
      trans.res.accepted(rt, after, obs, acts, next)

def trans.cmd(+rt: Rt, +after: List<&2, T.Act>, +cmd: T.Cmd) -> Trans:
  match rt:
    case Rt{+sess, _, _, _}:
      trans.res(rt, after, S.apply(sess, cmd))

def merge_acts(+acts: List<&2, T.Act>, +cont: Job) -> Job:
  match cont:
    case JCmd{cmd, after}:
      JCmd{cmd, List.append(&2, T.Act, acts, after)}
    case JActs{as0}:
      JActs{List.append(&2, T.Act, acts, as0)}
    case JIdle{}:
      JActs{acts}
    case JDone{}:
      JActs{acts}

def trans.drain.ok(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    +obs: List<&2, T.Obs>, +acts: List<&2, T.Act>, +next: T.Session) -> Trans:
  match rt:
    case Rt{_, +script, _, +live}:
      Tr{EPrints{obs_lines(obs)}, Rt{next, script, rest_cmds, live},
        merge_acts(acts, cont)}

def trans.drain.rejected(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    +reason: String) -> Trans:
  match rt:
    case Rt{+sess, +script, _, +live}:
      Tr{EPrints{Con{String.append("core rejected: ", reason), Nil{}}},
        Rt{sess, script, rest_cmds, live}, cont}

def trans.drain.res(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    res: T.Res) -> Trans:
  match res:
    case T.Rejected{reason}:
      # consumed with a rejection line; the pending work continues
      trans.drain.rejected(rt, cont, rest_cmds, reason)
    case T.Accepted{obs, acts, next}:
      trans.drain.ok(rt, cont, rest_cmds, obs, acts, next)

def trans.job(+rt: Rt, +cont: Job) -> Trans:
  match cont:
    case JCmd{cmd, after}:
      trans.cmd(rt, after, cmd)
    case JActs{acts}:
      trans.acts(rt, acts)
    case JIdle{}:
      trans.idle(rt)
    case JDone{}:
      Tr{ENone{}, rt, JDone{}}

def trans.drain(+rt: Rt, +cont: Job) -> Trans:
  match rt:
    case Rt{+sess, +script, cmds, _}:
      match cmds:
        case Nil{}:
          trans.job(rt, cont)
        case Con{+c, rest}:
          # only steering, notifications, and interrupts interleave with
          # in-flight work; queued messages and compaction wait for an
          # idle boundary (the Core would sort them out anyway, but the
          # client contract sends them between turns)
          match c:
            case T.UserMsg{T.Steer{}, _}:
              trans.drain.res(rt, cont, rest, S.apply(sess, c))
            case T.Notify{_}:
              trans.drain.res(rt, cont, rest, S.apply(sess, c))
            case T.Interrupt{}:
              trans.drain.res(rt, cont, rest, S.apply(sess, c))
            case _:
              trans.job(rt, cont)

def transition(rt: Rt, job: Job) -> Trans:
  match job:
    case JCmd{cmd, after}:
      trans.cmd(rt, after, cmd)
    case JActs{acts}:
      trans.drain(rt, JActs{acts})
    case JIdle{}:
      trans.idle(rt)
    case JDone{}:
      Tr{ENone{}, rt, JDone{}}

# send one batch of lines to the loop's output; a socket send hands the
# socket back, so the new Out is returned for the next hop
# ---- the IO loop ----

def print_lines(lines: List<&2, String>) -> IO(Unit):
  match lines:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case Con{l, rest}:
      do IO<Unit>:
        IO.print(l)
        print_lines(rest)

def emit.pair(sm: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket:
  match sm:
    case (s2, _):
      s2

def emit(out: Out, lines: List<&2, String>) -> IO(Out):
  match out:
    case OStd{}:
      do IO<Out>:
        print_lines(lines)
        IO.pure(Out, OStd{})
    case ONet{sock}:
      do IO<Out>:
        sm : Socket & Result<&1, &1, U32 & String, Unit> <- TCP.send(sock,
          String.append(String.join(lines, "\n"), "\n"))
        s2 : Socket <- IO.pure(Socket, emit.pair(sm))
        IO.pure(Out, ONet{s2})

# ---- live-effect helpers ----
# the bridge round-trip: connect, send the request, read the reply

# the unified tool-execution result: ok carries the output, failure
# carries the reason (exit status, stderr) so the model can see it
type ExecRes is Data:
  Ex{ok: Bool, out: String}

# a giant output (a catted binary, a full log) must never flow into the
# history and blow up the next provider request. The limit is
# parameterized so the law can prove the logic at a small scale (a
# 20001-char literal would overflow the proof checker's stack).
def cap_out_limit() -> Nat:
  20000n

def cap_out_at.pick(fits: Bool, +out: String, +n: Nat) -> String:
  match fits:
    case True{}:
      out
    case False{}:
      String.append(String.take(out, n),
        "  [output truncated at 20000 chars]")

def cap_out_at(+n: Nat, +out: String) -> String:
  cap_out_at.pick(Nat.is_le(String.length(out), n), out, n)

def cap_out(+out: String) -> String:
  cap_out_at(cap_out_limit(), out)

def exec_cmd.pick(ok: Bool, +out: String, +id: U32) -> T.Cmd:
  match ok:
    case True{}:
      T.ToolOk{id, cap_out(out)}
    case False{}:
      T.ToolFail{id, cap_out(out)}

def exec_cmd(r: ExecRes, +id: U32) -> T.Cmd:
  match r:
    case Ex{+ok, +out}:
      exec_cmd.pick(ok, out, id)

# ---- feed annotations for the TUI (live only) ----
# The Core's obs carry ids and outcomes; the runtime adds what the
# engineer actually needs: the tool name, the args, and a result
# preview. One line each, flattened and capped:
#   tool #<id> <name> : <args>
#   tool_result #<id> <ok|fail> : <out>
#   subtool <name> <ok|fail> : <out>          (inside run_typescript)

def ann_flat(+s: String) -> String:
  String.take(Api.flatten(s), 200n)

def ok_label(ok: Bool) -> String:
  match ok:
    case True{}:
      "ok"
    case False{}:
      "fail"

def call_ann(+call: T.Call) -> String:
  match call:
    case T.Call{+id, +name, +args}:
      String.append("tool #", String.append(U32.show(id),
        String.append(" ", String.append(Rem.wire_name(name),
          String.append(" : ", ann_flat(args))))))

def res_ann(+id: U32, +r: ExecRes) -> String:
  match r:
    case Ex{+ok, +out}:
      String.append("tool_result #", String.append(U32.show(id),
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

def sleep_ann(+id: U32, +secs: U32) -> String:
  String.append("tool #", String.append(U32.show(id),
    String.append(" self.sleep : ",
      String.append(U32.show(secs), "s"))))

def sleep_res_ann(+id: U32, +secs: U32) -> String:
  String.append("tool_result #", String.append(U32.show(id),
    String.append(" ok : slept ",
      String.append(U32.show(secs), "s"))))

def emit_call_ann(out: Out, +call: T.Call, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{call_ann(call), Nil{}})
    case False{}:
      IO.pure(Out, out)

def emit_res_ann(out: Out, +id: U32, +r: ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{res_ann(id, r), Nil{}})
    case False{}:
      IO.pure(Out, out)

# the sleep result is known upfront: both lines go out together
def emit_sleep_anns(out: Out, +id: U32, +secs: U32, live: Bool) -> IO(Out):
  match live:
    case True{}:
      do IO<Out>:
        out2 : Out <- emit(out, Con{sleep_ann(id, secs), Nil{}})
        emit(out2, Con{sleep_res_ann(id, secs), Nil{}})
    case False{}:
      IO.pure(Out, out)

# ---- the live provider call, fully in Bend (hub HTTP client) ----
# The wire request (MODEL/TOOL/MSG/CALL lines) becomes the
# OpenAI-compatible JSON in core/api.bend, is POSTed to api.mistral.ai
# by the hub HTTP client, and the reply maps back through core/api.bend
# into the OK/CALL/END string remote.bend parses. No bridge process.

def env_or(r: Result<&1, &1, U32 & String, String>, +dflt: String) -> String:
  match r:
    case Done{+s}:
      s
    case Fail{_}:
      dflt

def api_err(e: Http.Err) -> String:
  match e:
    case Http.ErrUrl{}:
      "bad url"
    case Http.ErrDns{}:
      "dns"
    case Http.ErrConnect{+c, +w}:
      String.append("connect ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrTls{+c, +w}:
      String.append("tls ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrRead{+c, +w}:
      String.append("read ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrWrite{+c, +w}:
      String.append("write ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrTimeout{}:
      "timeout"
    case Http.ErrRedirect{}:
      "too many redirects"
    case Http.ErrBad{}:
      "bad response"

# BEND_WIRE_DUMP=<path>: write the last provider request body (debug)
def dump_wire.write(+path: String, +body: String) -> IO(Unit):
  do IO<Unit>:
    u : Unit <- P.write_file(path, body)
    IO.pure(Unit, Unit{})

def dump_wire.skip(empty: Bool, +path: String, +body: String) -> IO(Unit):
  match empty:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      dump_wire.write(path, body)

def dump_wire(+path: String, +body: String) -> IO(Unit):
  dump_wire.skip(String.is_empty(path), path, body)

def model_call.err(e: Http.Err) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    IO.pure(Rem.RemoteRes,
      Rem.RRErr{String.append("provider unreachable: ", api_err(e))})

def model_call.reply(+res: Http.Res) -> String:
  match res:
    case Http.Res{+status, _, _}:
      Api.reply_of(status, Http.text(res))

def model_call.res(res: Http.Res) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    reply : String <- IO.pure(String, model_call.reply(res))
    IO.pure(Rem.RemoteRes, Rem.parse_reply(reply))

def model_call.ok(r: Result<&2, &2, Http.Err, Http.Res>) -> IO(Rem.RemoteRes):
  match r:
    case Fail{+e}:
      model_call.err(e)
    case Done{+res}:
      model_call.res(res)

# transient failures get another chance: connection loss, DNS, timeouts
# and server-side statuses (5xx, 429) retry with a pause; everything else
# (4xx auth/validation) fails the candidate immediately. The outcome is
# classified PURELY into an Attempt; the retry goes through a
# continuation closure that self-references the loop (the exec_done
# idiom — mutual recursion between defs is not expressible).
type Attempt is Data:
  AOk{res: Http.Res}
  AFail{e: Http.Err}
  ARetry{after_ms: U32, why: String}

def retryable_err(e: Http.Err) -> Bool:
  match e:
    case Http.ErrConnect{_, _}:
      True{}
    case Http.ErrDns{}:
      True{}
    case Http.ErrTimeout{}:
      True{}
    case _:
      False{}

def retryable_status(+s: U32) -> Bool:
  Bool.or(U32.is_le(500, s), U32.is_eq(s, 429))

def attempt_res.status_of(+res: Http.Res) -> U32:
  match res:
    case Http.Res{+status, _, _}:
      status

def attempt_ok(retryable: Bool, +res: Http.Res) -> Attempt:
  match retryable:
    case True{}:
      ARetry{1500, String.append("provider ",
        String.append(U32.show(attempt_res.status_of(res)), " (transient)"))}
    case False{}:
      AOk{res}

def attempt_res(+res: Http.Res) -> Attempt:
  attempt_ok(retryable_status(attempt_res.status_of(res)), res)

def attempt_fail(retryable: Bool, +e: Http.Err) -> Attempt:
  match retryable:
    case True{}:
      ARetry{1500, api_err(e)}
    case False{}:
      AFail{e}

def attempt_fail2(+e: Http.Err) -> Attempt:
  attempt_fail(retryable_err(e), e)

def model_call.attempt_result(r: Result<&2, &2, Http.Err, Http.Res>)
    -> Attempt:
  match r:
    case Done{+res}:
      attempt_res(res)
    case Fail{+e}:
      attempt_fail2(e)

def model_call.step.retry(can: Bool, +k: Nat, +after_ms: U32, +why: String,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match can:
    case True{}:
      do IO<Rem.RemoteRes>:
        IO.sleep(after_ms)
        next(k)
    case False{}:
      IO.pure(Rem.RemoteRes, Rem.RRErr{why})

def model_call.step(a: Attempt, +k: Nat,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match a:
    case AOk{+res}:
      model_call.res(res)
    case AFail{+e}:
      model_call.err(e)
    case ARetry{+after_ms, +why}:
      model_call.step.retry(Nat.is_lt(0n, k), k, after_ms, why, next)

@unsafe
def model_call.try(fuel: Nat, +request: String, +key: String)
    -> IO(Rem.RemoteRes):
  match fuel:
    case 0n:
      IO.pure(Rem.RemoteRes,
        Rem.RRErr{"provider unreachable: retries exhausted"})
    case 1n+k:
      do IO<Rem.RemoteRes>:
        mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
        +model : String <- IO.pure(String, env_or(mr, "zai-glm-5-3"))
        +body : String <- IO.pure(String,
          Enc.utf8.encode(Api.api_body(model, Api.parse_wire(request))))
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        dump_dbg : Unit <- dump_wire(env_or(dr, ""), body)
        hdrs : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
          Http.set(Http.set(Http.empty(), "Authorization",
              String.append("Bearer ", key)),
            "Content-Type", "application/json"))
        r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch.with("POST",
          "https://api.mistral.ai/v1/chat/completions", hdrs, body, 120000)
        +a : Attempt <- IO.pure(Attempt, model_call.attempt_result(r))
        model_call.step(a, k,
          k2 => model_call.try(k2, request, key))

# three attempts total: the first try plus two retries
def model_call.go(+request: String, +key: String) -> IO(Rem.RemoteRes):
  model_call.try(3n, request, key)

def model_call.key(empty: Bool, +request: String, +key: String)
    -> IO(Rem.RemoteRes):
  match empty:
    case True{}:
      IO.pure(Rem.RemoteRes, Rem.RRErr{"MISTRAL_API_KEY is not set"})
    case False{}:
      model_call.go(request, key)

@unsafe
def model_call(+request: String) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, env_or(kr, ""))
    model_call.key(String.is_empty(key), request, key)

# ---- the bash tool, fully in Bend (hub snap runner) ----
# snap execvp's /bin/sh -c; the answer is the exit status on the first
# line, then stdout and stderr. Commands are flattened first (the wire
# already flattens call args — double safety).

def exec_of.pick.fail(+code: U32, +output: String) -> ExecRes:
  Ex{False{}, String.append("exit ",
    String.append(U32.show(code), String.append(": ", output)))}

def exec_of.pick.bg(is_bg: Bool, +code: U32, +output: String) -> ExecRes:
  match is_bg:
    case True{}:
      # the background handoff: the contract printed by the wrapper
      Ex{True{}, output}
    case False{}:
      exec_of.pick.fail(code, output)

def exec_of.pick.zero(is_zero: Bool, +code: U32, +output: String) -> ExecRes:
  match is_zero:
    case True{}:
      Ex{True{}, output}
    case False{}:
      exec_of.pick.bg(U32.is_eq(code, 199), code, output)

def exec_of.pick(+code: U32, +output: String) -> ExecRes:
  exec_of.pick.zero(U32.is_eq(code, 0), code, output)

def exec_of.code.read(m: Maybe<&2, U32>, +output: String) -> ExecRes:
  match m:
    case Some{+c}:
      exec_of.pick(c, output)
    case None{}:
      exec_of.pick(1, output)

def exec_of.code(code_str: String, +output: String) -> ExecRes:
  exec_of.code.read(U32.read(code_str), output)

def exec_of.split(sp: Maybe<&2, Nat>, +out: String) -> ExecRes:
  match sp:
    case Some{+p}:
      exec_of.code(String.take(out, p), String.drop(out, Nat.add(p, 1n)))
    case None{}:
      exec_of.code(out, "")

def exec_of(+out: String) -> ExecRes:
  exec_of.split(C.find(out, "\n", 0n), out)

# ---- bash: a sync window, then a background handoff ----
# A command that outlives the sync window never blocks the turn: it keeps
# running detached under a per-id directory that the agent drives with
# ORDINARY bash — tail the .out, write the .in fifo, read the .rc, kill
# the .pid. The command's output is a FILE, never a pipe, so no orphan
# child can ever hold the tool's read open (the bug class that once made
# every command cost the full timeout is structurally gone).
#
# Wrapper shape (bg_wrap below):
#   - id: the first free slot (no .pid) in the session's bg dir
#   - stdin: a fifo held RDWR (never EOF, never blocks at open) — the
#     agent feeds it with printf > <id>.in whenever the command reads
#   - a detached RUNNER subshell is the command's real parent (a sibling
#     cannot wait: 127); it records the pid, waits, and writes the exit
#     code to <id>.rc — the wrapper polls the pid and reads the rc
#   - within BEND_BG_AFTER seconds (default 30): the tool returns the
#     output and the exit code, and cleans the slot
#   - past the window: the wrapper prints the driving contract and
#     exits 199, which exec_of maps to a SUCCESSFUL result — the agent
#     keeps thinking and interacts through further bash calls

def bg_after.read(m: Maybe<&2, U32>, +dflt: U32) -> U32:
  match m:
    case Some{+secs}:
      secs
    case None{}:
      dflt

def bg_after.or(r: Result<&1, &1, U32 & String, String>, +dflt: U32) -> U32:
  match r:
    case Done{+v}:
      bg_after.read(U32.read(v), dflt)
    case Fail{_}:
      dflt

def bg_after.clamp2(over: Bool, +secs: U32) -> U32:
  match over:
    case True{}:
      3600
    case False{}:
      secs

def bg_after.clamp(+secs: U32) -> U32:
  bg_after.clamp2(U32.is_lt(3600, secs), secs)

def bg_after() -> IO(U32):
  do IO<U32>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_AFTER")
    +raw : U32 <- IO.pure(U32, bg_after.or(r, 30))
    IO.pure(U32, bg_after.clamp(raw))

def bg_root.or(r: Result<&1, &1, U32 & String, String>, +dflt: String) -> String:
  match r:
    case Done{+v}:
      v
    case Fail{_}:
      dflt

def bg_dir() -> IO(String):
  do IO<String>:
    root : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_ROOT")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +r : String <- IO.pure(String, bg_root.or(root, "/tmp"))
    +p : String <- IO.pure(String, env_or(pr, "0"))
    IO.pure(String,
      String.append(r, String.append("/bend-bg-", p)))

def bg_wrap.seg(+a: String, +b: String) -> String:
  String.append(a, String.append("\n", b))

def bg_wrap(+command: String, +bg: String, +after: U32) -> String:
  +niter = U32.mul(after, 10)
  +secs = U32.show(after)
  bg_wrap.seg(String.append("BG=", bg),
  bg_wrap.seg("mkdir -p $BG",
  bg_wrap.seg("i=0",
  bg_wrap.seg("while [ -e $BG/$i.pid ]; do i=$((i+1)); done",
  bg_wrap.seg("mkfifo $BG/$i.in 2>/dev/null",
  bg_wrap.seg("exec 9<>$BG/$i.in",
  bg_wrap.seg("cat > $BG/$i.cmd <<'XQ7NDEOF'",
  bg_wrap.seg(command,
  bg_wrap.seg("XQ7NDEOF",
  bg_wrap.seg(String.append("( ( ", String.append(command,
      " ) > $BG/$i.out 2>&1 <&9 & echo $! > $BG/$i.pid; wait $!; echo $? > $BG/$i.rc ) >/dev/null 2>&1 &")),
  bg_wrap.seg("while [ ! -s $BG/$i.pid ]; do sleep 0.05; done",
  bg_wrap.seg("c=$(cat $BG/$i.pid)",
  bg_wrap.seg("e=0",
  bg_wrap.seg("while kill -0 $c 2>/dev/null; do",
  bg_wrap.seg("  sleep 0.1",
  bg_wrap.seg("  e=$((e+1))",
  bg_wrap.seg(String.append("  if [ $e -ge ",
    String.append(U32.show(niter), " ]; then")),
  bg_wrap.seg(String.append("    echo \"background: still running after ", String.append(secs, "s - id $i (dir $BG)\"")),
  bg_wrap.seg("    echo \"  output: tail -n 50 $BG/$i.out\"",
  bg_wrap.seg("    echo \"  stdin (if it reads): printf '%s\\n' 'text' > $BG/$i.in\"",
  bg_wrap.seg("    echo \"  status: kill -0 $(cat $BG/$i.pid) && echo RUNNING || cat $BG/$i.rc 2>/dev/null\"",
  bg_wrap.seg("    echo \"  kill: kill $(cat $BG/$i.pid)\"",
  bg_wrap.seg("    exit 199",
  bg_wrap.seg("  fi",
  bg_wrap.seg("done",
  bg_wrap.seg("while [ ! -e $BG/$i.rc ]; do sleep 0.05; done",
  bg_wrap.seg("s=$(cat $BG/$i.rc)",
  bg_wrap.seg("cat $BG/$i.out",
  bg_wrap.seg("rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc",
  "exit $s")))))))))))))))))))))))))))))

# snap's wire joins argv with newlines, so a multi-line -c script is
# impossible; the wrapper goes to a file (the node_run pattern) and sh
# runs the file
def bash_script_path(+port: String) -> String:
  String.append("/tmp/bend-sh-", String.append(port, ".sh"))

@unsafe
def bash_exec(command: String) -> IO(ExecRes):
  do IO<ExecRes>:
    +secs : U32 <- bg_after()
    +bg : String <- bg_dir()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, env_or(pr, "0"))
    +script : String <- IO.pure(String,
      bg_wrap(Api.flatten(command), bg, secs))
    w : Unit <- P.write_file(bash_script_path(port), script)
    out : String <- Snap.run(Con{"/bin/sh",
      Con{bash_script_path(port), Nil{}}})
    IO.pure(ExecRes, exec_of(out))

def exec_mr(m: Mcp.MR) -> ExecRes:
  match m:
    case Mcp.MR{+ok, +out}:
      Ex{ok, out}

def exec_search(+args: String, +live: Bool) -> IO(ExecRes):
  do IO<ExecRes>:
    r : Mcp.MR <- Mcp.merged_search(args, live)
    IO.pure(ExecRes, exec_mr(r))

def exec_mcp.key(+name: String, +args: String, +key: String)
    -> IO(ExecRes):
  do IO<ExecRes>:
    r : Mcp.MR <- Mcp.mcp_call(name, args, key)
    IO.pure(ExecRes, exec_mr(r))

def exec_mcp.scripted(+name: String) -> ExecRes:
  Ex{False{},
    String.append("unknown tool: ", String.append(name,
      " — remote MCP tools need a live session"))}

def exec_mcp.live(+name: String, +args: String) -> IO(ExecRes):
  do IO<ExecRes>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, env_or(kr, ""))
    exec_mcp.key(name, args, key)

def exec_mcp(+name: String, +args: String, live: Bool) -> IO(ExecRes):
  match live:
    case True{}:
      exec_mcp.live(name, args)
    case False{}:
      IO.pure(ExecRes, exec_mcp.scripted(name))

# ---- bend-jsrt: run_typescript in a V8 isolate (the Vibe SDK core pattern) ----
# The model writes real TypeScript/JavaScript; rust/jsrt (deno_core v8 +
# deno_ast) runs it in a FRESH isolate per round with heap limits and a
# watchdog. Tool calls print a request and exit 42; the runtime executes
# the tool through the same exec paths (bash, search, MCP) and RE-RUNS
# the program with the extended results — replay by re-execution, the
# same durable-boundary semantics as the in-house interpreter.

type NR is Data:
  NRDone{out: String}
  NRReq{name: String, args: String}
  NRErr{why: String}

# the recorded results travel as a JSON array of strings
def res_val.go(results: List<&2, String>, +acc: List<&2, Jv.Val>)
    -> List<&2, Jv.Val>:
  match results:
    case Nil{}:
      List.reverse(&2, Jv.Val, acc)
    case Con{+r, +rest}:
      res_val.go(rest, Con{Jv.Str{r}, acc})

def res_val(results: List<&2, String>) -> Jv.Val:
  Jv.Arr{res_val.go(results, Nil{})}

def results_json(results: List<&2, String>) -> String:
  Jv.encode(res_val(results))

# the request's args render as raw tool-arg text
def args_raw.str(+v: Jv.Val) -> String:
  match v:
    case Jv.Str{+s}:
      s
    case _:
      Jv.encode(v)

def args_raw(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case None{}:
      "{}"
    case Some{+v}:
      args_raw.str(v)

# parse the snap output: exit-code line, then stdout
def node_req.tool_str(v: Jv.Val) -> String:
  match v:
    case Jv.Str{+s}:
      s
    case _:
      "?"

def node_req.tool(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case None{}:
      "?"
    case Some{+v}:
      node_req.tool_str(v)

def node_req.ok(+v: Jv.Val) -> NR:
  NRReq{node_req.tool(Jv.get(v, "tool")), args_raw(Jv.get(v, "args"))}

def node_req.res(r: Maybe<&2, Jv.Val>) -> NR:
  match r:
    case Some{+v}:
      node_req.ok(v)
    case None{}:
      NRErr{"bad tool request from the program"}

def node_req(+body: String) -> NR:
  node_req.res(Jv.parse(String.trim(body)))

# exit 43: the engine printed {"error": "..."} — hand the message back
# to the model so it can read the error and retry
def node_err.res(r: Maybe<&2, Jv.Val>) -> String:
  match r:
    case Some{+v}:
      node_req.tool(Jv.get(v, "error"))
    case None{}:
      "program failed"

def node_err(+body: String) -> String:
  node_err.res(Jv.parse(String.trim(body)))

def node_split.zero(z: Bool, +code: U32, +body: String) -> NR:
  match z:
    case True{}:
      NRDone{String.trim(body)}
    case False{}:
      NRErr{String.append("program error (exit ",
        String.append(U32.show(code), ")"))}

def node_split.pick43(is43: Bool, +code: U32, +body: String) -> NR:
  match is43:
    case True{}:
      NRErr{node_err(body)}
    case False{}:
      node_split.zero(U32.is_eq(code, 0), code, body)

def node_split.pick42(is42: Bool, +code: U32, +body: String) -> NR:
  match is42:
    case True{}:
      node_req(body)
    case False{}:
      node_split.pick43(U32.is_eq(code, 43), code, body)

def node_split.pick(+code: U32, +body: String) -> NR:
  node_split.pick42(U32.is_eq(code, 42), code, body)

def node_split.code(c: U32, +body: String) -> NR:
  node_split.pick(c, body)

def node_split.read(m: Maybe<&2, U32>, +code: String, +body: String) -> NR:
  match m:
    case Some{+c}:
      node_split.code(c, body)
    case None{}:
      NRErr{"unreadable exit status"}

def node_split.code_str(+code: String, +body: String) -> NR:
  node_split.read(U32.read(code), code, body)

def node_split.some(+p: Nat, +out: String) -> NR:
  node_split.code_str(String.take(out, p),
    String.drop(out, Nat.add(p, 1n)))

def node_split.none(+out: String) -> NR:
  node_split.code_str(out, "")

def node_split.sp(sp: Maybe<&2, Nat>, +out: String) -> NR:
  match sp:
    case Some{+p}:
      node_split.some(p, out)
    case None{}:
      node_split.none(out)

def node_split(sp: Maybe<&2, Nat>, +out: String) -> NR:
  node_split.sp(sp, out)

def node_cmd(+path: String, +res_path: String, +err_path: String) -> String:
  String.append("rust/jsrt/target/debug/bend-jsrt ",
    String.append(path,
      String.append(" ", String.append(res_path,
        String.append(" 2>", err_path)))))

def node_run.go(+prog: String, +results: List<&2, String>, +path: String,
    +res_path: String, +err_path: String) -> IO(NR):
  do IO<NR>:
    u : Unit <- P.write_file(path, prog)
    u2 : Unit <- P.write_file(res_path, results_json(results))
    +out : String <- Snap.run(Con{"/bin/sh", Con{"-c",
      Con{node_cmd(path, res_path, err_path), Nil{}}}})
    IO.pure(NR, node_split(C.find(out, "\n", 0n), out))

def node_run(+prog: String, +results: List<&2, String>, +port: String)
    -> IO(NR):
  node_run.go(prog, results,
    String.append("/tmp/bend-prog-", String.append(port, ".ts")),
    String.append("/tmp/bend-res-", String.append(port, ".json")),
    String.append("/tmp/bend-prog-", String.append(port, ".err")))

# run the program to completion: request -> execute -> record -> re-run
# (one machine, bottom-up; pending decisions ride action constructors)
type EpNext is Data:
  EpDone{out: String}
  EpFail{why: String}
  EpRe{results: List<&2, String>}

type EpAct is Data:
  EpRun{}
  EpNext{nx: EpNext}

def exec_program.decide(r: ExecRes, +results: List<&2, String>)
    -> EpNext:
  match r:
    case Ex{+ok, +out}:
      match ok:
        case True{}:
          EpRe{List.append(&2, String, results, Con{out, Nil{}})}
        case False{}:
          EpFail{String.append("program call failed: ", out)}

def exec_program.call.node(is_node: Bool, +name: String, +args: String,
    +live: Bool) -> IO(ExecRes):
  match is_node:
    case True{}:
      IO.pure(ExecRes, Ex{False{},
        "run_typescript inside a program is not supported"})
    case False{}:
      exec_mcp(name, args, live)

def exec_program.call.search(is_search: Bool, +name: String, +args: String,
    +live: Bool) -> IO(ExecRes):
  match is_search:
    case True{}:
      exec_search(args, live)
    case False{}:
      exec_program.call.node(String.eq(name, "node_program"), name, args,
        live)

def exec_program.call.bash(is_bash: Bool, +name: String, +args: String,
    +live: Bool) -> IO(ExecRes):
  match is_bash:
    case True{}:
      bash_exec(args)
    case False{}:
      exec_program.call.search(String.eq(name, "search_tool_functions"),
        name, args, live)

# the sub-call line renders in the TUI feed (live only): one line per
# tool the program called, with the outcome. Out is Type-kind: every
# emit writes the same connection, so the emitter param is shared and
# the returned handle is discarded here.
def sub_ann(+name: String, +r: ExecRes) -> String:
  match r:
    case Ex{+ok, +out}:
      String.append("subtool ", String.append(name,
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

def emit_sub(out: Out, +name: String, +r: ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{sub_ann(name, r), Nil{}})
    case False{}:
      IO.pure(Out, out)

def exec_program.call(out: Out, +name: String, +args: String, +live: Bool)
    -> IO(Out & ExecRes):
  do IO<Out & ExecRes>:
    +r : ExecRes <- exec_program.call.bash(String.eq(name, "bash"),
      name, args, live)
    out2 : Out <- emit_sub(out, name, r, live)
    IO.pure(Out & ExecRes, (out2, r))

# consume the call pair once: the handle rides beside the next step
def exec_program.nr.done(pr: Out & ExecRes,
    +results: List<&2, String>) -> Out & EpNext:
  match pr:
    case (out2, r):
      (out2, exec_program.decide(r, results))

def exec_program.nr(out: Out, nr: NR, +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & EpNext):
  match nr:
    case NRDone{+o}:
      IO.pure(Out & EpNext, (out, EpDone{o}))
    case NRErr{+why}:
      IO.pure(Out & EpNext, (out, EpFail{why}))
    case NRReq{+name, +args}:
      do IO<Out & EpNext>:
        pr : Out & ExecRes <- exec_program.call(out, name, args, live)
        IO.pure(Out & EpNext, exec_program.nr.done(pr, results))

def exec_program.step(pr: Out & EpNext, k: Nat, +prog: String,
    +results: List<&2, String>, +live: Bool,
    next: Nat -> Out -> EpNext -> IO(Out & ExecRes))
    -> IO(Out & ExecRes):
  match pr:
    case (out2, nx):
      next(k)(out2)(nx)

# @unsafe: the loop recurses through the step continuation closure
@unsafe
def exec_program(fuel: Nat, out: Out, act: EpAct, +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & ExecRes):
  match fuel:
    case 0n:
      IO.pure(Out & ExecRes,
        (out, Ex{False{}, "program step budget exhausted"}))
    case 1n+k:
      match act:
        case EpRun{}:
          do IO<Out & ExecRes>:
            pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
            +port : String <- IO.pure(String, env_or(pr, "0"))
            nr : NR <- node_run(prog, results, port)
            por : Out & EpNext <- exec_program.nr(out, nr, prog, results, live)
            exec_program.step(por, k, prog, results, live,
              k2 => o2 => nx => exec_program(k2, o2, EpNext{nx},
                prog, results, live))
        case EpNext{+nx}:
          match nx:
            case EpDone{+o}:
              IO.pure(Out & ExecRes, (out, Ex{True{}, o}))
            case EpFail{+why}:
              IO.pure(Out & ExecRes, (out, Ex{False{}, why}))
            case EpRe{+rs}:
              exec_program(k, out, EpRun{}, prog, rs, live)

def exec_pick(is_bash: Bool, is_search: Bool, +name: String,
    +args: String, +live: Bool) -> IO(ExecRes):
  match is_bash:
    case True{}:
      bash_exec(args)
    case False{}:
      match is_search:
        case True{}:
          exec_search(args, live)
        case False{}:
          exec_mcp(name, args, live)

def call_name(c: T.Call) -> String:
  match c:
    case T.Call{_, +name, _}:
      name

def exec_pick.call(is_bash: Bool, is_search: Bool, +call: T.Call,
    +live: Bool) -> IO(ExecRes):
  match call:
    case T.Call{_, +name, +args}:
      exec_pick(is_bash, is_search, name, args, live)

def exec_tool.node_args(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & ExecRes):
  match call:
    case T.Call{_, _, +args}:
      exec_program(1000n, out, EpRun{}, Api.call_code(args), Nil{}, live)

def exec_tool.plain(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & ExecRes):
  do IO<Out & ExecRes>:
    r : ExecRes <- exec_pick.call(String.eq(call_name(call), "bash"),
      String.eq(call_name(call), "search_tool_functions"), call, live)
    IO.pure(Out & ExecRes, (out, r))

def exec_tool.is_node(out: Out, is_node: Bool, +call: T.Call, +live: Bool)
    -> IO(Out & ExecRes):
  match is_node:
    case True{}:
      exec_tool.node_args(out, call, live)
    case False{}:
      exec_tool.plain(out, call, live)

def exec_tool(out: Out, +call: T.Call, +live: Bool) -> IO(Out & ExecRes):
  exec_tool.is_node(out, String.eq(call_name(call), "node_program"),
    call, live)

def call_id(c: T.Call) -> U32:
  match c:
    case T.Call{id, _, _}:
      id

# live completions: run_typescript becomes node_program so the V8 engine
# executes it (the Core's law-checked interpreter stays for scripted runs)
def rename_cmd.ok(+action: U32, +text: String, calls: List<&2, T.Call>)
    -> T.Cmd:
  T.CompletionOk{action, text, rename_calls(True{}, calls)}

def rename_cmd(cmd: T.Cmd) -> T.Cmd:
  match cmd:
    case T.CompletionOk{+action, +text, calls}:
      rename_cmd.ok(action, text, calls)
    case _:
      cmd

# consumes the exec pair once and hands (handle, cmd) to the loop's
# continuation; the closure passed by run captures its +params and
# self-references run, so no mutual recursion is needed
def exec_done(pr: Out & ExecRes, k: Nat, +call: T.Call,
    +after: List<&2, T.Act>, +rt2: Rt, +live: Bool,
    next: Nat -> Out -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pr:
    case (out3, +r):
      do IO<RtEnd>:
        out4 : Out <- emit_res_ann(out3, call_id(call), r, live)
        +cmd : T.Cmd <- IO.pure(T.Cmd, exec_cmd(r, call_id(call)))
        next(k)(out4)(cmd)

# The IO loop. The pure transition is computed as an argument and matched
# as a parameter, so `run` is a single self-recursive def; every hop burns
# one unit of fuel, so the Runtime terminates like the Core. The loop
# returns the final session and output handle, so callers (the scenarios,
# the REPL) can continue from where it stopped.
# @unsafe: the EExec branch recurses through the exec_done continuation
@unsafe
def run(fuel: Nat, out: Out, +rt: Rt, +job: Job, tr: Trans) -> IO(RtEnd):
  match fuel:
    case 0n:
      match rt:
        case Rt{+sess, _, _, _}:
          IO.pure(RtEnd, REnd{sess, out})
    case 1n+k:
      match rt:
        case Rt{+sess, _, _, _}:
          match job:
            case JDone{}:
              IO.pure(RtEnd, REnd{sess, out})
            case _:
              match tr:
                case Tr{effect, +rt2, +job2}:
                  match effect:
                    case ENone{}:
                      run(k, out, rt2, job2, transition(rt2, job2))
                    case EPrints{lines}:
                      do IO<RtEnd>:
                        out2 : Out <- emit(out, lines)
                        run(k, out2, rt2, job2, transition(rt2, job2))
                    case ESleep{+ms, +id, +secs}:
                      # the sleep timer, annotated for the live feed
                      do IO<RtEnd>:
                        out2 : Out <- emit_sleep_anns(out, id, secs,
                          rt_live(rt))
                        IO.sleep(ms)
                        run(k, out2, rt2, job2, transition(rt2, job2))
                    case EModel{+action, +request, +after}:
                      # the real provider call (the hub HTTP client)
                      do IO<RtEnd>:
                        res : Rem.RemoteRes <- model_call(request)
                        +cmd : T.Cmd <- IO.pure(T.Cmd,
                          rename_cmd(Rem.remote_cmd(res, action)))
                        run(k, out, rt2, JCmd{cmd, after},
                          transition(rt2, JCmd{cmd, after}))
                    case EExec{+call, +after, +live}:
                      # the real tool: bash, the merged search, or a
                      # remote MCP call resolved by name. The annotations
                      # carry name/args/result to the TUI (live only);
                      # program sub-calls annotate inside exec_program.
                      do IO<RtEnd>:
                        out2 : Out <- emit_call_ann(out, call, live)
                        pr : Out & ExecRes <- exec_tool(out2, call, live)
                        exec_done(pr, k, call, after, rt2, live,
                          k2 => o4 => +c => run(k2, o4, rt2, JCmd{c, after},
                            transition(rt2, JCmd{c, after})))

# ---- scenarios (ADR 0006: the Core tested through the Step Protocol) ----

def sys_prompt() -> String:
  "You are a precise assistant running on the Bend Unified Harness."

def cfg(threshold: Nat, max_nulls: Nat) -> T.Cfg:
  T.Cfg{threshold, 40n, max_nulls, sys_prompt(), X.catalog_short()}

def say(text: String) -> T.Cmd:
  T.UserMsg{T.Queue{}, text}

def reply(text: String) -> M.Reply:
  M.ROk{text, Nil{}}

def call(id: U32, name: String, args: String) -> M.Reply:
  M.ROk{"", Con{T.Call{id, name, args}, Nil{}}}

def scenario(name: String, +c: T.Cfg, +cmds: List<&2, T.Cmd>,
    +script: List<&2, M.Reply>) -> IO(Unit):
  do IO<Unit>:
    IO.print(String.append("\n=== ", String.append(name, " ===")))
    e : RtEnd <- run(500n, OStd{},
      Rt{T.new_session(c), script, cmds, False{}},
      JIdle{},
      transition(Rt{T.new_session(c), script, cmds, False{}}, JIdle{}))
    IO.pure(Unit, Unit{})

def s_conversation() -> IO(Unit):
  scenario("conversation",
    cfg(600n, 3n),
    Con{say("Hello!"), Nil{}},
    Con{reply("Hi! How can I help?"), Nil{}})

def s_tools() -> IO(Unit):
  scenario("tools + discovery + a failing tool",
    cfg(600n, 3n),
    Con{say("Find the search tool, search the catalog again, and also try a bogus tool."), Nil{}},
    Con{call(1, "search_tool_functions", "best_match: catalog"),
      Con{call(2, "search_tool_functions", "details: search_tool_functions"),
      Con{call(3, "bogus_tool", "anything"),
      Con{reply("Searched twice and reported the bogus tool."), Nil{}}}}})

def s_program() -> IO(Unit):
  scenario("programmatic tool calling (D1)",
    cfg(600n, 3n),
    Con{say("Orchestrate: search the catalog and look up a tool's details, then report."), Nil{}},
    Con{call(1, "run_typescript",
        "const s = search_tool_functions(best_match: catalog); const d = search_tool_functions(details: bash); return s;"),
      Con{reply("The program searched the catalog and looked up the details."), Nil{}}})

def s_program_fail() -> IO(Unit):
  scenario("a failing program call fails the program",
    cfg(600n, 3n),
    Con{say("Run a program that calls a missing tool."), Nil{}},
    Con{call(1, "run_typescript", "const s = no_such_tool(oops); return s;"),
      Con{reply("The program failed on the missing tool as expected."), Nil{}}})

def s_steering() -> IO(Unit):
  scenario("steering mid-turn (ADR 0005)",
    cfg(600n, 3n),
    Con{say("Do something."),
      Con{T.UserMsg{T.Steer{}, "Also mention penguins."}, Nil{}}},
    Con{call(1, "search_tool_functions", "best_match: catalog"),
      Con{reply("Done, and penguins are lovely."), Nil{}}})

def s_auto_compaction() -> IO(Unit):
  scenario("automatic in-turn compaction (ADR 0011)",
    cfg(304n, 3n),
    Con{say("First: let us talk about the history of the harbour, the cranes, and the ferries that cross every morning."),
      Con{say("Second: the warehouses along the quay stored grain, timber, and later container freight for decades."),
        Con{say("Third: the fishermen mended nets on the pier and told the same stories every single evening there."),
          Con{say("Fourth: now summarize everything about the harbour we discussed so far."), Nil{}}}}},
    Con{reply("The harbour has cranes, ferries, warehouses, and fishermen."),
      Con{reply("Grain and container freight shaped the warehouses."),
        Con{reply("The fishermen's stories repeated every evening."),
          Con{reply("<summary>The user asked about the harbour: cranes, ferries, warehouses with grain and freight, and fishermen telling stories.</summary>"),
            Con{reply("The harbour summary: cranes and ferries, warehouses, fishermen."), Nil{}}}}}})

def s_retry() -> IO(Unit):
  scenario("malformed summary retry, then success (ADR 0011)",
    cfg(289n, 3n),
    Con{say("First: the observatory on the hill opened its dome at dusk and tracked the moving planets all night."),
      Con{say("Second: the astronomers logged each transit in leather notebooks, arguing gently about the numbers."),
        Con{say("Third: now compact the observatory conversation."), Nil{}}}},
    Con{reply("The observatory tracked the moving planets from dusk until dawn, and the dome closed only when the sky paled at dawn."),
      Con{reply("The astronomers logged each transit carefully in leather notebooks, and they argued gently about the numbers until dawn."),
        Con{reply(""),
          Con{reply("<summary>The user asked about the hill observatory: it opened at dusk, tracked planets, and the astronomers logged transits while arguing gently.</summary>"),
            Con{reply("The observatory summary is ready."), Nil{}}}}}})

def s_manual_compaction() -> IO(Unit):
  scenario("manual between-turn compaction (ADR 0011)",
    cfg(600n, 3n),
    Con{say("Tell me about the lighthouse on the eastern cliff, its lamp, and its keeper who climbed every evening."),
      Con{T.Compact{"Keep the keeper's name if mentioned."}, Nil{}}},
    Con{reply("The lighthouse lamp turned slowly and the keeper climbed each evening."),
      Con{reply("<summary>The user asked about the eastern cliff lighthouse: its rotating lamp and the keeper's evening climbs.</summary>"),
      Nil{}}})

def s_null_limit() -> IO(Unit):
  scenario("null iteration limit (ADR 0015)",
    cfg(600n, 1n),
    Con{say("Say something."), Nil{}},
    Con{reply(""), Con{reply(""), Nil{}}})

def s_provider_failure() -> IO(Unit):
  scenario("provider failure fails the turn",
    cfg(600n, 3n),
    Con{say("Hello?"), Nil{}},
    Con{M.RFail{}, Nil{}})

def s_interrupt_noop() -> IO(Unit):
  scenario("interrupt mid-turn; the late completion is rejected",
    cfg(600n, 3n),
    Con{say("Hi."),
      Con{T.Interrupt{}, Nil{}}},
    Con{reply("Hello!"), Nil{}})

def s_sleep() -> IO(Unit):
  scenario("bounded sleep tool (D9 bis)",
    cfg(600n, 3n),
    Con{say("Sleep a moment, then tell me the time you slept."), Nil{}},
    Con{call(1, "self.sleep", "1"),
      Con{reply("I slept one second."), Nil{}}})

def s_checkpoint() -> IO(Unit):
  do IO<Unit>:
    IO.print("\n=== checkpoint roundtrip ===")
    IO.print(String.append("restore(export(s)) is the session by proof; version = ",
      U32.show(K.version(K.export(T.new_session(cfg(600n, 3n)))))))

def main() -> IO(Unit):
  do IO<Unit>:
    s_conversation()
    s_tools()
    s_program()
    s_program_fail()
    s_steering()
    s_auto_compaction()
    s_retry()
    s_manual_compaction()
    s_null_limit()
    s_provider_failure()
    s_interrupt_noop()
    s_sleep()
    s_checkpoint()
