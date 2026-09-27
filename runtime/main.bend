# runtime/main.bend — the Harness Runtime loop.
#
# Executes Core-emitted Actions and feeds normalized results back as
# commands (ADR 0006: the Core is driven only through the Step Protocol).
#
# The loop splits in two:
# - transition: a PURE function from (runtime state, job) to (effect, next
#   state, next job). It decides everything: applies commands, picks model
#   replies, runs pure tools.
# - run: the only IO cycle. It performs the effect (print, provider call,
#   tool exec) and recurses with less fuel, so the Runtime terminates
#   like the Core.
#
# Also here: tool dispatch (exec_pick), run_typescript through bend-jsrt,
# the --resume history replay, and the steering/interrupt side-channels.
# The tools themselves live in their own modules (bash, patch-tool,
# selftools, mcp, skills), the provider call in provider.bend. Live mode
# calls the real provider; scripted mode pops runtime/model.bend replies.
import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/obs.bend as O
import ../core/api.bend as Api
import ../core/compaction.bend as C
import ../core/text.bend as Tx
import ./model.bend as M
import ./tools.bend as X
import ./persist.bend as P
import ./remote.bend as Rem
import ./mcp.bend as Mcp
import ./skills.bend as Sk
import ./provider.bend as Pv
import ./bash.bend as Sh
import ./patch-tool.bend as Ptl
import ./selftools.bend as Sf
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


def rt_live(+rt: Rt) -> Bool:
  match rt:
    case Rt{_, _, _, +live}:
      live

# every tool is a Runtime effect: bash, the merged search (it reads
# the MCP index), remote MCP tools resolved by name, and unknown names
# (which fail cleanly with the name). self.sleep is NOT a model tool
# anymore: it is a programmatic tool, called inside a run_typescript
# program (exec_program.call.sleep below).
def trans.tool(+rt: Rt, +rest: List<&2, T.Act>, +call: T.Call) -> Trans:
  match call:
    case T.Call{_, _, _}:
      Tr{EExec{call, rest, rt_live(rt)}, rt, JIdle{}}

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

# the session's own queue drains at the idle boundary: a message the
# Core queued mid-turn (apply.user.busy: Queue never supersedes
# in-flight work) starts the next turn as soon as the current one
# ends — the same contract as a line the client queued between turns
# (rt.cmds takes priority; the tail stays queued for later turns)
def trans.idle.queued(q: List<&2, String>, +sess: T.Session,
    +script: List<&2, M.Reply>, +live: Bool) -> Trans:
  match q:
    case Nil{}:
      Tr{EPrints{Con{"--- idle", Nil{}}}, Rt{sess, script, Nil{}, live}, JDone{}}
    case Con{+text, +rest}:
      Tr{ENone{}, Rt{S.set_queued(sess, rest), script, Nil{}, live},
        JCmd{T.UserMsg{T.Queue{}, text}, Nil{}}}

def trans.idle(+rt: Rt) -> Trans:
  match rt:
    case Rt{+sess, +script, cmds, +live}:
      match cmds:
        case Nil{}:
          trans.idle.queued(S.sess_queued(sess), sess, script, live)
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

# ---- feed annotations for the TUI (live only) ----
# The Core's obs carry ids and outcomes; the runtime adds what the
# engineer actually needs: the tool name, the args, and a result
# preview. One line each, flattened and capped:
#   tool #<id> <name> : <args>
#   tool_result #<id> <ok|fail> : <out>
#   subtool <name> <ok|fail> : <out>          (inside run_typescript)

def ann_flat(+s: String) -> String:
  String.take(Tx.flatten(s), 200n)

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

def res_ann(+id: U32, +r: X.ExecRes) -> String:
  match r:
    case X.Ex{+ok, +out}:
      String.append("tool_result #", String.append(U32.show(id),
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

def emit_call_ann(out: Out, +call: T.Call, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{call_ann(call), Nil{}})
    case False{}:
      IO.pure(Out, out)

def emit_res_ann(out: Out, +id: U32, +r: X.ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{res_ann(id, r), Nil{}})
    case False{}:
      IO.pure(Out, out)

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
    IO.pure(NR, node_split(Tx.find(out, "\n", 0n), out))

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

def exec_program.decide(r: X.ExecRes, +results: List<&2, String>)
    -> EpNext:
  match r:
    case X.Ex{+ok, +out}:
      match ok:
        case True{}:
          EpRe{List.append(&2, String, results, Con{out, Nil{}})}
        case False{}:
          EpFail{String.append("program call failed: ", out)}

def exec_program.call.node(is_node: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_node:
    case True{}:
      IO.pure(X.ExecRes, X.Ex{False{},
        "run_typescript inside a program is not supported"})
    case False{}:
      Sh.exec_mcp(name, args, live)

# ---- self.sleep: a programmatic tool (called inside a program) ----
# await self.sleep(20) inside a run_typescript program. The seconds
# come as a bare number (self.sleep(20)) or as {seconds: 20}; the HOST
# sleeps between the replay rounds (the V8 watchdog only bounds the
# program own CPU), and the recorded result replays by position, so
# the wait happens exactly once.
def prog_sleep.unread(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+u}:
      u
    case None{}:
      0

def prog_sleep.field(f: Jv.Val) -> U32:
  match f:
    case Jv.Num{+s}:
      prog_sleep.unread(U32.read(s))
    case _:
      0

def prog_sleep.parsed.num(v: Jv.Val) -> U32:
  match v:
    case Jv.Num{+s}:
      prog_sleep.field(v)
    case _:
      0

def prog_sleep.parsed.get(m: Maybe<&2, Jv.Val>, +v: Jv.Val) -> U32:
  match m:
    case Some{+f}:
      prog_sleep.field(f)
    case None{}:
      prog_sleep.parsed.num(v)

def prog_sleep.parsed(+v: Jv.Val) -> U32:
  prog_sleep.parsed.get(Jv.get(v, "seconds"), v)

def prog_sleep.secs.none(+args: String) -> U32:
  prog_sleep.unread(U32.read(args))

def prog_sleep.secs(m: Maybe<&2, Jv.Val>, +args: String) -> U32:
  match m:
    case Some{+v}:
      prog_sleep.parsed(v)
    case None{}:
      prog_sleep.secs.none(args)

def prog_sleep.bounds(+secs: U32) -> Bool:
  Bool.and(U32.is_lt(0, secs), U32.is_lt(secs, 20))

def exec_program.call.sleep.valid.pick(valid: Bool, +secs: U32)
    -> IO(X.ExecRes):
  match valid:
    case True{}:
      do IO<X.ExecRes>:
        u : Unit <- IO.sleep(U32.mul(secs, 1000))
        IO.pure(X.ExecRes, X.Ex{True{},
          String.append("slept ", String.append(U32.show(secs), "s"))})
    case False{}:
      IO.pure(X.ExecRes, X.Ex{False{}, "sleep requires 0 < seconds < 20"})

def exec_program.call.sleep.valid(+secs: U32) -> IO(X.ExecRes):
  exec_program.call.sleep.valid.pick(prog_sleep.bounds(secs), secs)

def exec_program.call.sleep(is_sleep: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_sleep:
    case True{}:
      do IO<X.ExecRes>:
        +pr : Maybe<&2, Jv.Val> <- IO.pure(Maybe<&2, Jv.Val>, Jv.parse(String.trim(args)))
        +secs : U32 <- IO.pure(U32, prog_sleep.secs(pr, args))
        exec_program.call.sleep.valid(secs)
    case False{}:
      exec_program.call.node(String.eq(name, "node_program"), name, args,
        live)

def exec_program.call.self(is_self: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_self:
    case True{}:
      IO.pure(X.ExecRes, Sf.self_exec(name))
    case False{}:
      exec_program.call.sleep(String.eq(name, "self.sleep"), name, args,
        live)

def exec_program.call.skill(is_skill: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_skill:
    case True{}:
      do IO<X.ExecRes>:
        r : Mcp.MR <- Sk.exec_skill(args, live)
        IO.pure(X.ExecRes, Sh.exec_mr(r))
    case False{}:
      exec_program.call.self(Sf.is_self_tool(name), name, args, live)

def exec_program.call.search(is_search: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_search:
    case True{}:
      Sh.exec_search(args, live)
    case False{}:
      exec_program.call.skill(String.eq(name, "skill"), name, args, live)

def exec_program.call.patch(is_patch: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_patch:
    case True{}:
      Ptl.exec_patch(args, live)
    case False{}:
      exec_program.call.search(String.eq(name, "search_tool_functions"),
        name, args, live)

def exec_program.call.bash(is_bash: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_bash:
    case True{}:
      Sh.bash_exec(args)
    case False{}:
      exec_program.call.patch(String.eq(name, "apply_patch"),
        name, args, live)

# the sub-call line renders in the TUI feed (live only): one line per
# tool the program called, with the outcome. Out is Type-kind: every
# emit writes the same connection, so the emitter param is shared and
# the returned handle is discarded here.
def sub_ann(+name: String, +r: X.ExecRes) -> String:
  match r:
    case X.Ex{+ok, +out}:
      String.append("subtool ", String.append(name,
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

def emit_sub(out: Out, +name: String, +r: X.ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{sub_ann(name, r), Nil{}})
    case False{}:
      IO.pure(Out, out)

# a self.* request filed from inside the program joins the program's
# request set (it will ride the program's result)
def exec_program.call.self_req(is_self: Bool, +name: String, +d: Sf.Def)
    -> Sf.Def:
  match is_self:
    case True{}:
      Sf.def_merge(d, String.eq(name, "self.compact"),
        String.eq(name, "self.reload"))
    case False{}:
      d

def exec_program.call(out: Out, +name: String, +args: String, +live: Bool,
    +d: Sf.Def) -> IO(Out & (Sf.Def & X.ExecRes)):
  do IO<Out & (Sf.Def & X.ExecRes)>:
    +r : X.ExecRes <- exec_program.call.bash(String.eq(name, "bash"),
      name, args, live)
    +d2 : Sf.Def <- IO.pure(Sf.Def,
      exec_program.call.self_req(Sf.is_self_tool(name), name, d))
    out2 : Out <- emit_sub(out, name, r, live)
    IO.pure(Out & (Sf.Def & X.ExecRes), (out2, (d2, r)))

# consume the call pair once: the handle rides beside the next step
def exec_program.nr.done(pr: Out & (Sf.Def & X.ExecRes),
    +results: List<&2, String>) -> Out & (Sf.Def & EpNext):
  match pr:
    case (out2, (d2, r)):
      (out2, (d2, exec_program.decide(r, results)))

def exec_program.nr(out: Out, nr: NR, +prog: String,
    +results: List<&2, String>, +live: Bool, +d: Sf.Def)
    -> IO(Out & (Sf.Def & EpNext)):
  match nr:
    case NRDone{+o}:
      IO.pure(Out & (Sf.Def & EpNext), (out, (d, EpDone{o})))
    case NRErr{+why}:
      IO.pure(Out & (Sf.Def & EpNext), (out, (d, EpFail{why})))
    case NRReq{+name, +args}:
      do IO<Out & (Sf.Def & EpNext)>:
        pr : Out & (Sf.Def & X.ExecRes) <- exec_program.call(out, name, args,
          live, d)
        IO.pure(Out & (Sf.Def & EpNext), exec_program.nr.done(pr, results))

def exec_program.step(pr: Out & (Sf.Def & EpNext), k: Nat, +prog: String,
    +results: List<&2, String>, +live: Bool,
    next: Nat -> Out -> Sf.Def -> EpNext -> IO(Out & X.ExecRes))
    -> IO(Out & X.ExecRes):
  match pr:
    case (out2, (d2, nx)):
      next(k)(out2)(d2)(nx)

# the program's final value carries the requests it filed: the model
# reads its own acks back in the tool result, and the turn-boundary
# scan finds the markers in the history
def exec_program.done(d: Sf.Def, +o: String) -> X.ExecRes:
  X.Ex{True{}, String.append(o, Sf.def_acks.suffix(d))}

def exec_program.fail(d: Sf.Def, +why: String) -> X.ExecRes:
  X.Ex{False{}, String.append(why, Sf.def_acks.suffix(d))}

@unsafe
def exec_program.go(fuel: Nat, out: Out, d: Sf.Def, act: EpAct, +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & X.ExecRes):
  match fuel:
    case 0n:
      IO.pure(Out & X.ExecRes,
        (out, exec_program.fail(d, "program step budget exhausted")))
    case 1n+k:
      match act:
        case EpRun{}:
          do IO<Out & X.ExecRes>:
            pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
            +port : String <- IO.pure(String, P.env_or(pr, "0"))
            nr : NR <- node_run(prog, results, port)
            por : Out & (Sf.Def & EpNext) <- exec_program.nr(out, nr, prog,
              results, live, d)
            exec_program.step(por, k, prog, results, live,
              k2 => o2 => d2 => nx => exec_program.go(k2, o2, d2,
                EpNext{nx}, prog, results, live))
        case EpNext{+nx}:
          match nx:
            case EpDone{+o}:
              IO.pure(Out & X.ExecRes, (out, exec_program.done(d, o)))
            case EpFail{+why}:
              IO.pure(Out & X.ExecRes, (out, exec_program.fail(d, why)))
            case EpRe{+rs}:
              exec_program.go(k, out, d, EpRun{}, prog, rs, live)

# @unsafe: the loop recurses through the step continuation closure
@unsafe
def exec_program(fuel: Nat, pr0: Out & (Sf.Def & EpAct), +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & X.ExecRes):
  match pr0:
    case (out, (d, act)):
      exec_program.go(fuel, out, d, act, prog, results, live)

# the skill loader: the vibe_sdk exact-name contract, executed by
# reading the SKILL.md the catalog points at
def exec_pick.patch(is_patch: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_patch:
    case True{}:
      Ptl.exec_patch(args, live)
    case False{}:
      Sh.exec_mcp(name, args, live)

def exec_pick.skill(is_skill: Bool, +name: String, +args: String,
    +live: Bool) -> IO(X.ExecRes):
  match is_skill:
    case True{}:
      do IO<X.ExecRes>:
        r : Mcp.MR <- Sk.exec_skill(args, live)
        IO.pure(X.ExecRes, Sh.exec_mr(r))
    case False{}:
      exec_pick.patch(String.eq(name, "apply_patch"), name, args, live)

def exec_pick.not_bash(is_search: Bool, is_self: Bool, is_skill: Bool,
    +name: String, +args: String, +live: Bool) -> IO(X.ExecRes):
  match is_search:
    case True{}:
      Sh.exec_search(args, live)
    case False{}:
      match is_self:
        case True{}:
          IO.pure(X.ExecRes, Sf.self_exec(name))
        case False{}:
          exec_pick.skill(is_skill, name, args, live)

def exec_pick(is_bash: Bool, is_search: Bool, is_self: Bool,
    is_skill: Bool, +name: String, +args: String, +live: Bool)
    -> IO(X.ExecRes):
  match is_bash:
    case True{}:
      Sh.bash_exec(args)
    case False{}:
      exec_pick.not_bash(is_search, is_self, is_skill, name, args, live)

def call_name(c: T.Call) -> String:
  match c:
    case T.Call{_, +name, _}:
      name

def exec_pick.call(is_bash: Bool, is_search: Bool, is_skill: Bool,
    +call: T.Call, +live: Bool) -> IO(X.ExecRes):
  match call:
    case T.Call{_, +name, +args}:
      exec_pick(is_bash, is_search, Sf.is_self_tool(name), is_skill, name,
        args, live)

def exec_tool.node_args(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & X.ExecRes):
  match call:
    case T.Call{_, _, +args}:
      exec_program(1000n, (out, (Sf.def_none(), EpRun{})),
        Api.call_code(args), Nil{}, live)

def exec_tool.plain(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & X.ExecRes):
  do IO<Out & X.ExecRes>:
    r : X.ExecRes <- exec_pick.call(String.eq(call_name(call), "bash"),
      String.eq(call_name(call), "search_tool_functions"),
      String.eq(call_name(call), "skill"), call, live)
    IO.pure(Out & X.ExecRes, (out, r))

def exec_tool.is_node(out: Out, is_node: Bool, +call: T.Call, +live: Bool)
    -> IO(Out & X.ExecRes):
  match is_node:
    case True{}:
      exec_tool.node_args(out, call, live)
    case False{}:
      exec_tool.plain(out, call, live)

def exec_tool(out: Out, +call: T.Call, +live: Bool) -> IO(Out & X.ExecRes):
  exec_tool.is_node(out, String.eq(call_name(call), "node_program"),
    call, live)

def call_id(c: T.Call) -> U32:
  match c:
    case T.Call{id, _, _}:
      id

def call_args(c: T.Call) -> String:
  match c:
    case T.Call{_, _, args}:
      args

# the source line of a code tool (run_typescript, bash, apply_patch -
# the TUI renders the patch as a diff): the FULL args,
# wire-encoded (newline as \N, backslash doubled - the same reversible
# encoding the provider wire uses), never flattened or capped, so the
# TUI renders the whole program or script; the tool # preview line
# stays capped at 200
def code_ann(+call: T.Call) -> String:
  String.append("tool_code #", String.append(U32.show(call_id(call)),
    String.append(" : ", Api.wire_encode(call_args(call)))))

def is_code_call.wire(+w: String) -> Bool:
  Bool.or(String.eq(w, "run_typescript"),
    Bool.or(String.eq(w, "bash"), String.eq(w, "apply_patch")))

def is_code_call(+name: String) -> Bool:
  is_code_call.wire(Rem.wire_name(name))

def emit_code_ann.pick(is_code: Bool, out: Out, +call: T.Call) -> IO(Out):
  match is_code:
    case True{}:
      emit(out, Con{code_ann(call), Nil{}})
    case False{}:
      IO.pure(Out, out)

def emit_code_ann(out: Out, +call: T.Call, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit_code_ann.pick(is_code_call(call_name(call)), out, call)
    case False{}:
      IO.pure(Out, out)

# ---- --resume / --continue: replay the restored history to the client ----
# A client's feed starts empty: a fresh TUI after --resume, and after a
# /reload respawn too. The REPL replays the restored history as the SAME
# wire lines a live turn prints, each prefixed "history ", so the client
# renders it with its live code path (tool blocks included). Two lines
# have no live form: "you : <text>" (a user message, typed client-side
# in a live turn) and "injected : <text>" (steering / notifications).
# Tool results carry no call id: they follow their assistant message in
# call order, so the walk keeps the calls still waiting (pend). A call
# that never got its result (interrupt, crash) replays as failed.
# One self-recursive def, fuel first, the state in Data; the output
# accumulates reversed.

type Rp is Data:
  Rp{out: List<&2, String>, ms: List<&2, T.Msg>, pend: List<&2, T.Call>}

def hist(+l: String) -> String:
  String.append("history ", l)

def rev_onto(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      rev_onto(t, Con{h, acc})

def fin_label(ok: Bool) -> String:
  match ok:
    case True{}:
      " ok"
    case False{}:
      " failed"

def replay.fin(+id: U32, ok: Bool) -> String:
  hist(String.append("  obs: tool_finished #",
    String.append(U32.show(id), fin_label(ok))))

def replay.code(is_code: Bool, +c: T.Call) -> List<&2, String>:
  match is_code:
    case True{}:
      Con{hist(code_ann(c)), Nil{}}
    case False{}:
      Nil{}

def replay.call(+c: T.Call) -> List<&2, String>:
  Con{hist(String.append("  obs: tool_started #", U32.show(call_id(c)))),
    Con{hist(call_ann(c)), replay.code(is_code_call(call_name(c)), c)}}

def replay.calls(cs: List<&2, T.Call>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, t}:
      List.append(&2, String, replay.call(c), replay.calls(t))

def replay.abandon(cs: List<&2, T.Call>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, t}:
      Con{replay.fin(call_id(c), False{}), replay.abandon(t)}

# a committed result: "tool <name> ok: <out>" / "tool <name> failed: <out>"
# (session.tool_result_msg), the out newline-escaped
def replay.res.lines(+id: U32, +ok: Bool, +out: String) -> List<&2, String>:
  Con{hist(res_ann(id, X.Ex{ok, Api.unescape_nl(out)})),
    Con{replay.fin(id, ok), Nil{}}}

def replay.res.fail(is_fail: Bool, +id: U32, +text: String, +pfail: String)
    -> List<&2, String>:
  match is_fail:
    case True{}:
      replay.res.lines(id, False{}, String.drop(text, String.length(pfail)))
    case False{}:
      replay.res.lines(id, True{}, text)

def replay.res.ok(is_ok: Bool, +id: U32, +text: String, +pok: String,
    +pfail: String) -> List<&2, String>:
  match is_ok:
    case True{}:
      replay.res.lines(id, True{}, String.drop(text, String.length(pok)))
    case False{}:
      replay.res.fail(String.starts_with(text, pfail), id, text, pfail)

def replay.res(+c: T.Call, +text: String) -> List<&2, String>:
  match c:
    case T.Call{+id, +name, _}:
      +base = String.append("tool ", S.result_name(name))
      replay.res.ok(String.starts_with(text, String.append(base, " ok: ")),
        id, text, String.append(base, " ok: "),
        String.append(base, " failed: "))

def replay.summary_of(m: Maybe<&2, String>, +text: String) -> String:
  match m:
    case Some{+s}:
      s
    case None{}:
      text

def replay.injected.other(is_preamble: Bool, +text: String)
    -> List<&2, String>:
  match is_preamble:
    case True{}:
      Nil{}
    case False{}:
      Con{hist(String.append("injected : ", Api.escape_nl(text))), Nil{}}

def replay.injected.summary(is_summary: Bool, +text: String)
    -> List<&2, String>:
  match is_summary:
    case True{}:
      Con{hist(O.show_obs(T.CompactionDone{
        replay.summary_of(C.parse_summary(text), text)})), Nil{}}
    case False{}:
      replay.injected.other(
        String.starts_with(text, "The earlier conversation was compacted."),
        text)

def replay.user(inj: Bool, +text: String, +rest: List<&2, T.Msg>,
    +pend: List<&2, T.Call>, +out: List<&2, String>) -> Rp:
  match inj:
    case False{}:
      Rp{Con{hist(String.append("you : ", Api.escape_nl(text))), out},
        rest, pend}
    case True{}:
      Rp{rev_onto(replay.injected.summary(
          String.starts_with(text, "Summary of the earlier conversation:"),
          text), out), rest, pend}

def replay.asst(empty: Bool, +text: String) -> List<&2, String>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      # the restored text may carry real newlines: escaped, the line stays
      # one wire line (live replies arrive escaped already)
      Con{hist(O.show_obs(T.Assistant{Api.escape_nl(text)})), Nil{}}

def replay.tool(pend: List<&2, T.Call>, +text: String,
    +rest: List<&2, T.Msg>, +out: List<&2, String>) -> Rp:
  match pend:
    case Nil{}:
      Rp{out, rest, Nil{}}
    case Con{+c, +t}:
      Rp{rev_onto(replay.res(c, text), out), rest, t}

def replay.msg(+m: T.Msg, +rest: List<&2, T.Msg>, +pend: List<&2, T.Call>,
    +out: List<&2, String>) -> Rp:
  match m:
    case T.Msg{+role, +text, +calls, +inj}:
      match role:
        case T.RUser{}:
          replay.user(inj, text, rest, pend, out)
        case T.RAsst{}:
          # a new assistant message: calls still waiting never got a result
          Rp{rev_onto(List.append(&2, String, replay.abandon(pend),
              List.append(&2, String, replay.asst(String.is_empty(text), text),
                replay.calls(calls))), out), rest, calls}
        case T.RTool{}:
          replay.tool(pend, text, rest, out)
        case T.RSys{}:
          Rp{out, rest, pend}

def replay.step(st: Rp) -> Rp:
  match st:
    case Rp{+out, +ms, +pend}:
      match ms:
        case Nil{}:
          Rp{rev_onto(replay.abandon(pend), out), Nil{}, Nil{}}
        case Con{+m, +rest}:
          replay.msg(m, rest, pend, out)

def replay.go(fuel: Nat, st: Rp) -> Rp:
  match fuel:
    case 0n:
      st
    case 1n+k:
      replay.go(k, replay.step(st))

def replay.out(st: Rp) -> List<&2, String>:
  match st:
    case Rp{+out, _, _}:
      List.reverse(&2, String, out)

# one step per message, one more to flush the calls left waiting
def replay(+history: List<&2, T.Msg>) -> List<&2, String>:
  replay.out(replay.go(Nat.add(List.length(&2, T.Msg, history), 1n),
    Rp{Nil{}, history, Nil{}}))


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

# ---- mid-turn steering side-channel (ADR 0005, live) ----
# The REPL is single-threaded: the socket is only read BETWEEN turns, so
# a "steer" line sent during a turn waits in the kernel buffer until the
# turn ends — the Core's mid-turn steering was never exercised live (an
# idle steer even starts a NEW turn, the exact opposite of ADR 0005).
# Codex-style fix: the client appends steering lines to a per-process
# file, and the Runtime drains it at the two safe boundaries — right
# before a model result or a tool result is applied. There the Core is
# mid-turn, so apply routes Steer to steer_session; the completion is
# then applied on a session that already holds the steering, and
# dispatch injects it into the next model input.

def steer_path_of(+port: String) -> String:
  String.append("/tmp/bend-steer-", String.append(port, ".txt"))

def steer_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, steer_path_of(p))

# ---- the harness-info line: what every client must know ----
# The REPL is the single source of truth. Both clients (the TUI via
# the bend-harness parent, and bend_client.py) read this line from the
# REPL's output instead of recomputing anything: the resolved model,
# the compaction threshold, and the two side-channel paths. One line,
# space-separated key=value pairs, printed BEFORE the "REPL on" banner
# (the parent waits for the banner, so the info is always there).
def info_line(+model: String, +threshold: String, +steer: String,
    +interrupt: String) -> String:
  String.append("harness-info model=",
    String.append(model,
      String.append(" threshold=",
        String.append(threshold,
          String.append(" steer=",
            String.append(steer,
              String.append(" interrupt=", interrupt)))))))

def harness_info(+model: String, +threshold: String) -> IO(String):
  do IO<String>:
    +sp : String <- steer_path()
    +ip : String <- Pv.interrupt_path()
    IO.pure(String, info_line(model, threshold, sp, ip))

# the feed lines for one drained steering line's apply result; mid-turn
# Steer is always Accepted (apply.user.busy), so the Rejected arm is
# the defensive one
def drain_steer.emit_of(res: T.Res) -> List<&2, String>:
  match res:
    case T.Rejected{+reason}:
      Con{String.append("  obs: steering_rejected: ", reason), Nil{}}
    case T.Accepted{+obs, _, _}:
      obs_lines(obs)

def drain_steer.next_of(+sess: T.Session, res: T.Res) -> T.Session:
  match res:
    case T.Rejected{_}:
      sess
    case T.Accepted{_, _, +next}:
      next

# the drained content becomes steering lines; blank lines never steer
def drain_steer.push(empty: Bool, +l: String, +rest: List<&2, String>)
    -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{l, rest}

def drain_steer.skip_go(lines: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      drain_steer.push(String.is_empty(l), l, drain_steer.skip_go(rest))

# apply the drained lines one by one. Mid-turn Steer produces no acts
# (apply.user.busy), so the continuation work in `after` is untouched.
# Structural recursion on the shrinking line list; out and rt are
# merely threaded.
# apply the drained lines one by one. Mid-turn Steer produces no acts
# (apply.user.busy), so the continuation work in `after` is untouched.
# Structural recursion on the shrinking line list; out and the session
# fields are merely threaded.
def drain_steer.one(steers: List<&2, String>, out: Out, +sess: T.Session,
    +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>, live: Bool)
    -> IO(Out & Rt):
  match steers:
    case Nil{}:
      do IO<Out & Rt>:
        IO.pure(Out & Rt, (out, Rt{sess, script, cmds, live}))
    case Con{+l, +rest}:
      do IO<Out & Rt>:
        +res : T.Res <- IO.pure(T.Res,
          S.apply(sess, T.UserMsg{T.Steer{}, l}))
        out2 : Out <- emit(out, drain_steer.emit_of(res))
        drain_steer.one(rest, out2, drain_steer.next_of(sess, res),
          script, cmds, live)

# destructure the runtime state once, then fold the steering lines in
def drain_steer.step(out: Out, +rt: Rt, +steers: List<&2, String>)
    -> IO(Out & Rt):
  match rt:
    case Rt{+sess, +script, +cmds, +live}:
      drain_steer.one(steers, out, sess, script, cmds, live)

# read-then-truncate: content that arrives after the read stays in the
# file for the NEXT boundary, never lost, never double-applied
def drain_steer.some(empty: Bool, +path: String, out: Out, +rt: Rt,
    +c: String) -> IO(Out & Rt):
  match empty:
    case True{}:
      do IO<Out & Rt>:
        IO.pure(Out & Rt, (out, rt))
    case False{}:
      do IO<Out & Rt>:
        u : Unit <- P.write_file(path, "")
        drain_steer.step(out, rt, drain_steer.skip_go(String.split(c, Tx.newline())))

def drain_steer.pick(m: Maybe<&2, String>, +path: String, out: Out, +rt: Rt)
    -> IO(Out & Rt):
  match m:
    case None{}:
      do IO<Out & Rt>:
        IO.pure(Out & Rt, (out, rt))
    case Some{+c}:
      drain_steer.some(String.is_empty(c), path, out, rt, c)

# ---- the interrupt side-channel (Ctrl+C in the TUI) ----
# Same shape as the steering side-channel: the socket is only read
# between turns, so a mid-turn interrupt would sit in the kernel buffer
# until the turn ends. The TUI writes a flag file instead; the runtime
# drains it right after the steering file at the same safe boundaries.
# The Core kills the turn (CandidateDiscarded + TurnDone interrupted),
# commits any steering the turn was holding, and the in-flight result
# is then rejected cleanly ("no pending completion").

def drain_interrupt.some(empty: Bool, +ipath: String, out: Out, +rt: Rt,
    +sess: T.Session, +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>,
    +live: Bool) -> IO(Out & Rt):
  match empty:
    case True{}:
      do IO<Out & Rt>:
        IO.pure(Out & Rt, (out, rt))
    case False{}:
      do IO<Out & Rt>:
        u : Unit <- P.write_file(ipath, "")
        +res : T.Res <- IO.pure(T.Res, S.apply(sess, T.Interrupt{}))
        out2 : Out <- emit(out, drain_steer.emit_of(res))
        IO.pure(Out & Rt,
          (out2, Rt{drain_steer.next_of(sess, res), script, cmds, live}))

def drain_interrupt.pick(m: Maybe<&2, String>, +ipath: String, out: Out,
    +rt: Rt, +sess: T.Session, +script: List<&2, M.Reply>,
    +cmds: List<&2, T.Cmd>, live: Bool) -> IO(Out & Rt):
  match m:
    case None{}:
      do IO<Out & Rt>:
        IO.pure(Out & Rt, (out, rt))
    case Some{+c}:
      drain_interrupt.some(String.is_empty(c), ipath, out, rt, sess,
        script, cmds, live)

# consumes the drained pair once and runs the interrupt check
def drain_interrupt(pr: Out & Rt) -> IO(Out & Rt):
  match pr:
    case (out2, +rt2):
      match rt2:
        case Rt{+sess, +script, +cmds, +live}:
          do IO<Out & Rt>:
            +ipath : String <- Pv.interrupt_path()
            m : Maybe<&2, String> <- P.read_file(ipath)
            drain_interrupt.pick(m, ipath, out2, rt2, sess, script, cmds,
              live)

def drain_steer.file(+path: String, out: Out, +rt: Rt) -> IO(Out & Rt):
  do IO<Out & Rt>:
    m : Maybe<&2, String> <- P.read_file(path)
    pr : Out & Rt <- drain_steer.pick(m, path, out, rt)
    drain_interrupt(pr)

# the entry point: scripted runs (live=False) never touch the file, so
# the scenarios and harness-demo stay hermetic
def drain_steer(out: Out, +rt: Rt) -> IO(Out & Rt):
  match rt:
    case Rt{_, _, _, +live}:
      match live:
        case True{}:
          do IO<Out & Rt>:
            path : String <- steer_path()
            drain_steer.file(path, out, rt)
        case False{}:
          do IO<Out & Rt>:
            IO.pure(Out & Rt, (out, rt))

# consumes the exec pair once and hands (handle, cmd) to the loop's
# continuation; the closure passed by run captures its +params and
# self-references run, so no mutual recursion is needed
def exec_done.drained(pd: Out & Rt, k: Nat, +cmd: T.Cmd,
    next: Nat -> Out -> Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out5, +rt3):
      next(k)(out5)(rt3)(cmd)

def exec_done(pr: Out & X.ExecRes, k: Nat, +call: T.Call,
    +after: List<&2, T.Act>, +rt2: Rt, +live: Bool,
    next: Nat -> Out -> Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pr:
    case (out3, +r):
      do IO<RtEnd>:
        out4 : Out <- emit_res_ann(out3, call_id(call), r, live)
        +cmd : T.Cmd <- IO.pure(T.Cmd, X.exec_cmd(r, call_id(call)))
        pd : Out & Rt <- drain_steer(out4, rt2)
        exec_done.drained(pd, k, cmd, next)

# the same boundary for a model result: the completion is applied on the
# drained state, so steering that arrived during the call is already in
# the turn and lands in the next model input
def model_done(pd: Out & Rt, k: Nat, +res: Rem.RemoteRes, +action: U32,
    +after: List<&2, T.Act>,
    next: Nat -> Out -> Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out3, +rt3):
      do IO<RtEnd>:
        +cmd : T.Cmd <- IO.pure(T.Cmd, rename_cmd(Rem.remote_cmd(res, action)))
        next(k)(out3)(rt3)(cmd)

# The IO loop. The pure transition is computed as an argument and matched
# as a parameter, so `run` is a single self-recursive def; every hop burns
# one unit of fuel, so the Runtime terminates like the Core. The loop
# returns the final session and output handle, so callers (the scenarios,
# the REPL) can continue from where it stopped.
# @unsafe: the EExec branch recurses through the exec_done continuation,
# and the EModel branch through model_done (both carry the drained state)
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
                    case EModel{+action, +request, +after}:
                      # the real provider call (the hub HTTP client);
                      # the side channel is drained before the result is
                      # applied, so steering sent during the call enters
                      # the turn at this safe boundary
                      do IO<RtEnd>:
                        res : Rem.RemoteRes <- Pv.model_call(request)
                        pd : Out & Rt <- drain_steer(out, rt2)
                        model_done(pd, k, res, action, after,
                          k2 => o3 => +r3 => +c =>
                            run(k2, o3, r3, JCmd{c, after},
                              transition(r3, JCmd{c, after})))
                    case EExec{+call, +after, +live}:
                      # the real tool: bash, the merged search, or a
                      # remote MCP call resolved by name. The annotations
                      # carry name/args/result to the TUI (live only);
                      # program sub-calls annotate inside exec_program.
                      do IO<RtEnd>:
                        out1 : Out <- emit_call_ann(out, call, live)
                        out2 : Out <- emit_code_ann(out1, call, live)
                        pr : Out & X.ExecRes <- exec_tool(out2, call, live)
                        exec_done(pr, k, call, after, rt2, live,
                          k2 => o4 => +r3 => +c =>
                            run(k2, o4, r3, JCmd{c, after},
                              transition(r3, JCmd{c, after})))

# ---- scenarios (ADR 0006: the Core tested through the Step Protocol) ----

def sys_prompt() -> String:
  "You are a precise assistant running on the Bend Unified Harness."

def cfg(threshold: Nat, max_nulls: Nat) -> T.Cfg:
  T.Cfg{threshold, 40n, max_nulls, sys_prompt(), X.catalog_short()}
