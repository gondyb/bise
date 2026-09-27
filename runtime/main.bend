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
import ./tools-pure.bend as Xp
import ./persist.bend as P
import ./remote.bend as Rem
import ./mcp.bend as Mcp
import ./skills.bend as Sk
import ./provider.bend as Pv
import ./bash.bend as Sh
import ./patch-tool.bend as Ptl
import ./selftools.bend as Sf
import ./selftools-pure.bend as Sfp
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J
import ./main-pure.bend as Rtp

# where the loop's output goes: stdout, or a REPL socket. Sockets are
# affine handles, so Out is a Type: it is threaded, never copied.
type Out is Type:
  OStd{}
  ONet{sock: Socket}

# what the loop hands back when it stops
type RtEnd is Type:
  REnd{sess: T.Session, out: Out}

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

def emit_call_ann(out: Out, +call: T.Call, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{Rtp.call_ann(call), Nil{}})
    case False{}:
      IO.pure(Out, out)

def emit_res_ann(out: Out, +id: U32, +r: Xp.ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{Rtp.res_ann(id, r), Nil{}})
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
def res_val.go(results: List<&2, String>, +acc: List<&2, J.Json>)
    -> List<&2, J.Json>:
  match results:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case Con{+r, +rest}:
      res_val.go(rest, Con{J.JStr{r}, acc})

def res_val(results: List<&2, String>) -> J.Json:
  J.JArr{res_val.go(results, Nil{})}

def results_json(results: List<&2, String>) -> String:
  J.stringify(res_val(results))

# the request's args render as raw tool-arg text
def args_raw.str(+v: J.Json) -> String:
  match v:
    case J.JStr{+s}:
      s
    case _:
      J.stringify(v)

def args_raw(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      "{}"
    case Some{+v}:
      args_raw.str(v)

def node_req.ok(+v: J.Json) -> NR:
  NRReq{Rtp.node_req.tool(J.get(v, "tool")), args_raw(J.get(v, "args"))}

def node_req.res(r: Maybe<&2, J.Json>) -> NR:
  match r:
    case Some{+v}:
      node_req.ok(v)
    case None{}:
      NRErr{"bad tool request from the program"}

def node_req(+body: String) -> NR:
  node_req.res(Rtp.json_m(J.parse(String.trim(body))))

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
      NRErr{Rtp.node_err(body)}
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

def exec_program.decide(r: Xp.ExecRes, +results: List<&2, String>)
    -> EpNext:
  match r:
    case Xp.Ex{+ok, +out}:
      match ok:
        case True{}:
          EpRe{List.append(&2, String, results, Con{out, Nil{}})}
        case False{}:
          EpFail{String.append("program call failed: ", out)}

def exec_program.call.node(is_node: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_node:
    case True{}:
      IO.pure(Xp.ExecRes, Xp.Ex{False{},
        "run_typescript inside a program is not supported"})
    case False{}:
      Sh.exec_mcp(name, args, live)

def exec_program.call.sleep.valid.pick(valid: Bool, +secs: U32)
    -> IO(Xp.ExecRes):
  match valid:
    case True{}:
      do IO<Xp.ExecRes>:
        u : Unit <- IO.sleep(U32.mul(secs, 1000))
        IO.pure(Xp.ExecRes, Xp.Ex{True{},
          String.append("slept ", String.append(U32.show(secs), "s"))})
    case False{}:
      IO.pure(Xp.ExecRes, Xp.Ex{False{}, "sleep requires 0 < seconds < 20"})

def exec_program.call.sleep.valid(+secs: U32) -> IO(Xp.ExecRes):
  exec_program.call.sleep.valid.pick(Rtp.prog_sleep.bounds(secs), secs)

def exec_program.call.sleep(is_sleep: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_sleep:
    case True{}:
      do IO<Xp.ExecRes>:
        +pr : Maybe<&2, J.Json> <- IO.pure(Maybe<&2, J.Json>,
          Rtp.json_m(J.parse(String.trim(args))))
        +secs : U32 <- IO.pure(U32, Rtp.prog_sleep.secs(pr, args))
        exec_program.call.sleep.valid(secs)
    case False{}:
      exec_program.call.node(String.eq(name, "node_program"), name, args,
        live)

def exec_program.call.self(is_self: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_self:
    case True{}:
      IO.pure(Xp.ExecRes, Sfp.self_exec(name))
    case False{}:
      exec_program.call.sleep(String.eq(name, "self.sleep"), name, args,
        live)

def exec_program.call.skill(is_skill: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_skill:
    case True{}:
      do IO<Xp.ExecRes>:
        r : Mcp.MR <- Sk.exec_skill(args, live)
        IO.pure(Xp.ExecRes, Sh.exec_mr(r))
    case False{}:
      exec_program.call.self(Sfp.is_self_tool(name), name, args, live)

def exec_program.call.search(is_search: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_search:
    case True{}:
      Sh.exec_search(args, live)
    case False{}:
      exec_program.call.skill(String.eq(name, "skill"), name, args, live)

def exec_program.call.patch(is_patch: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_patch:
    case True{}:
      Ptl.exec_patch(args, live)
    case False{}:
      exec_program.call.search(String.eq(name, "search_tool_functions"),
        name, args, live)

def exec_program.call.bash(is_bash: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_bash:
    case True{}:
      Sh.bash_exec(args)
    case False{}:
      exec_program.call.patch(String.eq(name, "apply_patch"),
        name, args, live)

def emit_sub(out: Out, +name: String, +r: Xp.ExecRes, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit(out, Con{Rtp.sub_ann(name, r), Nil{}})
    case False{}:
      IO.pure(Out, out)

# a self.* request filed from inside the program joins the program's
# request set (it will ride the program's result)
def exec_program.call.self_req(is_self: Bool, +name: String, +d: Sfp.Def)
    -> Sfp.Def:
  match is_self:
    case True{}:
      Sfp.def_merge(d, String.eq(name, "self.compact"),
        String.eq(name, "self.reload"))
    case False{}:
      d

def exec_program.call(out: Out, +name: String, +args: String, +live: Bool,
    +d: Sfp.Def) -> IO(Out & (Sfp.Def & Xp.ExecRes)):
  do IO<Out & (Sfp.Def & Xp.ExecRes)>:
    +r : Xp.ExecRes <- exec_program.call.bash(String.eq(name, "bash"),
      name, args, live)
    +d2 : Sfp.Def <- IO.pure(Sfp.Def,
      exec_program.call.self_req(Sfp.is_self_tool(name), name, d))
    out2 : Out <- emit_sub(out, name, r, live)
    IO.pure(Out & (Sfp.Def & Xp.ExecRes), (out2, (d2, r)))

# consume the call pair once: the handle rides beside the next step
def exec_program.nr.done(pr: Out & (Sfp.Def & Xp.ExecRes),
    +results: List<&2, String>) -> Out & (Sfp.Def & EpNext):
  match pr:
    case (out2, (d2, r)):
      (out2, (d2, exec_program.decide(r, results)))

def exec_program.nr(out: Out, nr: NR, +prog: String,
    +results: List<&2, String>, +live: Bool, +d: Sfp.Def)
    -> IO(Out & (Sfp.Def & EpNext)):
  match nr:
    case NRDone{+o}:
      IO.pure(Out & (Sfp.Def & EpNext), (out, (d, EpDone{o})))
    case NRErr{+why}:
      IO.pure(Out & (Sfp.Def & EpNext), (out, (d, EpFail{why})))
    case NRReq{+name, +args}:
      do IO<Out & (Sfp.Def & EpNext)>:
        pr : Out & (Sfp.Def & Xp.ExecRes) <- exec_program.call(out, name, args,
          live, d)
        IO.pure(Out & (Sfp.Def & EpNext), exec_program.nr.done(pr, results))

def exec_program.step(pr: Out & (Sfp.Def & EpNext), k: Nat, +prog: String,
    +results: List<&2, String>, +live: Bool,
    next: Nat -> Out -> Sfp.Def -> EpNext -> IO(Out & Xp.ExecRes))
    -> IO(Out & Xp.ExecRes):
  match pr:
    case (out2, (d2, nx)):
      next(k)(out2)(d2)(nx)

# the program's final value carries the requests it filed: the model
# reads its own acks back in the tool result, and the turn-boundary
# scan finds the markers in the history
def exec_program.done(d: Sfp.Def, +o: String) -> Xp.ExecRes:
  Xp.Ex{True{}, String.append(o, Sfp.def_acks.suffix(d))}

def exec_program.fail(d: Sfp.Def, +why: String) -> Xp.ExecRes:
  Xp.Ex{False{}, String.append(why, Sfp.def_acks.suffix(d))}

@unsafe
def exec_program.go(fuel: Nat, out: Out, d: Sfp.Def, act: EpAct, +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & Xp.ExecRes):
  match fuel:
    case 0n:
      IO.pure(Out & Xp.ExecRes,
        (out, exec_program.fail(d, "program step budget exhausted")))
    case 1n+k:
      match act:
        case EpRun{}:
          do IO<Out & Xp.ExecRes>:
            pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
            +port : String <- IO.pure(String, P.env_or(pr, "0"))
            nr : NR <- node_run(prog, results, port)
            por : Out & (Sfp.Def & EpNext) <- exec_program.nr(out, nr, prog,
              results, live, d)
            exec_program.step(por, k, prog, results, live,
              k2 => o2 => d2 => nx => exec_program.go(k2, o2, d2,
                EpNext{nx}, prog, results, live))
        case EpNext{+nx}:
          match nx:
            case EpDone{+o}:
              IO.pure(Out & Xp.ExecRes, (out, exec_program.done(d, o)))
            case EpFail{+why}:
              IO.pure(Out & Xp.ExecRes, (out, exec_program.fail(d, why)))
            case EpRe{+rs}:
              exec_program.go(k, out, d, EpRun{}, prog, rs, live)

# @unsafe: the loop recurses through the step continuation closure
@unsafe
def exec_program(fuel: Nat, pr0: Out & (Sfp.Def & EpAct), +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Out & Xp.ExecRes):
  match pr0:
    case (out, (d, act)):
      exec_program.go(fuel, out, d, act, prog, results, live)

# the skill loader: the vibe_sdk exact-name contract, executed by
# reading the SKILL.md the catalog points at
def exec_pick.patch(is_patch: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_patch:
    case True{}:
      Ptl.exec_patch(args, live)
    case False{}:
      Sh.exec_mcp(name, args, live)

def exec_pick.skill(is_skill: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_skill:
    case True{}:
      do IO<Xp.ExecRes>:
        r : Mcp.MR <- Sk.exec_skill(args, live)
        IO.pure(Xp.ExecRes, Sh.exec_mr(r))
    case False{}:
      exec_pick.patch(String.eq(name, "apply_patch"), name, args, live)

def exec_pick.not_bash(is_search: Bool, is_self: Bool, is_skill: Bool,
    +name: String, +args: String, +live: Bool) -> IO(Xp.ExecRes):
  match is_search:
    case True{}:
      Sh.exec_search(args, live)
    case False{}:
      match is_self:
        case True{}:
          IO.pure(Xp.ExecRes, Sfp.self_exec(name))
        case False{}:
          exec_pick.skill(is_skill, name, args, live)

def exec_pick(is_bash: Bool, is_search: Bool, is_self: Bool,
    is_skill: Bool, +name: String, +args: String, +live: Bool)
    -> IO(Xp.ExecRes):
  match is_bash:
    case True{}:
      Sh.bash_exec(args)
    case False{}:
      exec_pick.not_bash(is_search, is_self, is_skill, name, args, live)

def exec_pick.call(is_bash: Bool, is_search: Bool, is_skill: Bool,
    +call: T.Call, +live: Bool) -> IO(Xp.ExecRes):
  match call:
    case T.Call{_, +name, +args}:
      exec_pick(is_bash, is_search, Sfp.is_self_tool(name), is_skill, name,
        args, live)

def exec_tool.node_args(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & Xp.ExecRes):
  match call:
    case T.Call{_, _, +args}:
      exec_program(1000n, (out, (Sfp.def_none(), EpRun{})),
        Api.call_code(args), Nil{}, live)

def exec_tool.plain(out: Out, +call: T.Call, +live: Bool)
    -> IO(Out & Xp.ExecRes):
  do IO<Out & Xp.ExecRes>:
    r : Xp.ExecRes <- exec_pick.call(String.eq(Rtp.call_name(call), "bash"),
      String.eq(Rtp.call_name(call), "search_tool_functions"),
      String.eq(Rtp.call_name(call), "skill"), call, live)
    IO.pure(Out & Xp.ExecRes, (out, r))

def exec_tool.is_node(out: Out, is_node: Bool, +call: T.Call, +live: Bool)
    -> IO(Out & Xp.ExecRes):
  match is_node:
    case True{}:
      exec_tool.node_args(out, call, live)
    case False{}:
      exec_tool.plain(out, call, live)

def exec_tool(out: Out, +call: T.Call, +live: Bool) -> IO(Out & Xp.ExecRes):
  exec_tool.is_node(out, String.eq(Rtp.call_name(call), "node_program"),
    call, live)

def emit_code_ann.pick(is_code: Bool, out: Out, +call: T.Call) -> IO(Out):
  match is_code:
    case True{}:
      emit(out, Con{Rtp.code_ann(call), Nil{}})
    case False{}:
      IO.pure(Out, out)

def emit_code_ann(out: Out, +call: T.Call, live: Bool) -> IO(Out):
  match live:
    case True{}:
      emit_code_ann.pick(Rtp.is_code_call(Rtp.call_name(call)), out, call)
    case False{}:
      IO.pure(Out, out)

# live completions: run_typescript becomes node_program so the V8 engine
# executes it (the Core's law-checked interpreter stays for scripted runs)
def rename_cmd.ok(+action: U32, +text: String, calls: List<&2, T.Call>)
    -> T.Cmd:
  T.CompletionOk{action, text, Rtp.rename_calls(True{}, calls)}

def rename_cmd(cmd: T.Cmd) -> T.Cmd:
  match cmd:
    case T.CompletionOk{+action, +text, calls}:
      rename_cmd.ok(action, text, calls)
    case _:
      cmd

def steer_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, Rtp.steer_path_of(p))

def harness_info(+model: String, +threshold: String) -> IO(String):
  do IO<String>:
    +sp : String <- steer_path()
    +ip : String <- Pv.interrupt_path()
    IO.pure(String, Rtp.info_line(model, threshold, sp, ip))

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
    -> IO(Out & Rtp.Rt):
  match steers:
    case Nil{}:
      do IO<Out & Rtp.Rt>:
        IO.pure(Out & Rtp.Rt, (out, Rtp.Rt{sess, script, cmds, live}))
    case Con{+l, +rest}:
      do IO<Out & Rtp.Rt>:
        +res : T.Res <- IO.pure(T.Res,
          S.apply(sess, T.UserMsg{T.Steer{}, l}))
        out2 : Out <- emit(out, Rtp.drain_steer.emit_of(res))
        drain_steer.one(rest, out2, Rtp.drain_steer.next_of(sess, res),
          script, cmds, live)

# destructure the runtime state once, then fold the steering lines in
def drain_steer.step(out: Out, +rt: Rtp.Rt, +steers: List<&2, String>)
    -> IO(Out & Rtp.Rt):
  match rt:
    case Rtp.Rt{+sess, +script, +cmds, +live}:
      drain_steer.one(steers, out, sess, script, cmds, live)

# read-then-truncate: content that arrives after the read stays in the
# file for the NEXT boundary, never lost, never double-applied
def drain_steer.some(empty: Bool, +path: String, out: Out, +rt: Rtp.Rt,
    +c: String) -> IO(Out & Rtp.Rt):
  match empty:
    case True{}:
      do IO<Out & Rtp.Rt>:
        IO.pure(Out & Rtp.Rt, (out, rt))
    case False{}:
      do IO<Out & Rtp.Rt>:
        u : Unit <- P.write_file(path, "")
        drain_steer.step(out, rt, Rtp.drain_steer.skip_go(String.split(c, Tx.newline())))

def drain_steer.pick(m: Maybe<&2, String>, +path: String, out: Out, +rt: Rtp.Rt)
    -> IO(Out & Rtp.Rt):
  match m:
    case None{}:
      do IO<Out & Rtp.Rt>:
        IO.pure(Out & Rtp.Rt, (out, rt))
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

def drain_interrupt.some(empty: Bool, +ipath: String, out: Out, +rt: Rtp.Rt,
    +sess: T.Session, +script: List<&2, M.Reply>, +cmds: List<&2, T.Cmd>,
    +live: Bool) -> IO(Out & Rtp.Rt):
  match empty:
    case True{}:
      do IO<Out & Rtp.Rt>:
        IO.pure(Out & Rtp.Rt, (out, rt))
    case False{}:
      do IO<Out & Rtp.Rt>:
        u : Unit <- P.write_file(ipath, "")
        +res : T.Res <- IO.pure(T.Res, S.apply(sess, T.Interrupt{}))
        out2 : Out <- emit(out, Rtp.drain_steer.emit_of(res))
        IO.pure(Out & Rtp.Rt,
          (out2, Rtp.Rt{Rtp.drain_steer.next_of(sess, res), script, cmds, live}))

def drain_interrupt.pick(m: Maybe<&2, String>, +ipath: String, out: Out,
    +rt: Rtp.Rt, +sess: T.Session, +script: List<&2, M.Reply>,
    +cmds: List<&2, T.Cmd>, live: Bool) -> IO(Out & Rtp.Rt):
  match m:
    case None{}:
      do IO<Out & Rtp.Rt>:
        IO.pure(Out & Rtp.Rt, (out, rt))
    case Some{+c}:
      drain_interrupt.some(String.is_empty(c), ipath, out, rt, sess,
        script, cmds, live)

# consumes the drained pair once and runs the interrupt check
def drain_interrupt(pr: Out & Rtp.Rt) -> IO(Out & Rtp.Rt):
  match pr:
    case (out2, +rt2):
      match rt2:
        case Rtp.Rt{+sess, +script, +cmds, +live}:
          do IO<Out & Rtp.Rt>:
            +ipath : String <- Pv.interrupt_path()
            m : Maybe<&2, String> <- P.read_file(ipath)
            drain_interrupt.pick(m, ipath, out2, rt2, sess, script, cmds,
              live)

def drain_steer.file(+path: String, out: Out, +rt: Rtp.Rt) -> IO(Out & Rtp.Rt):
  do IO<Out & Rtp.Rt>:
    m : Maybe<&2, String> <- P.read_file(path)
    pr : Out & Rtp.Rt <- drain_steer.pick(m, path, out, rt)
    drain_interrupt(pr)

# the entry point: scripted runs (live=False) never touch the file, so
# the scenarios and harness-demo stay hermetic
def drain_steer(out: Out, +rt: Rtp.Rt) -> IO(Out & Rtp.Rt):
  match rt:
    case Rtp.Rt{_, _, _, +live}:
      match live:
        case True{}:
          do IO<Out & Rtp.Rt>:
            path : String <- steer_path()
            drain_steer.file(path, out, rt)
        case False{}:
          do IO<Out & Rtp.Rt>:
            IO.pure(Out & Rtp.Rt, (out, rt))

# consumes the exec pair once and hands (handle, cmd) to the loop's
# continuation; the closure passed by run captures its +params and
# self-references run, so no mutual recursion is needed
def exec_done.drained(pd: Out & Rtp.Rt, k: Nat, +cmd: T.Cmd,
    next: Nat -> Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out5, +rt3):
      next(k)(out5)(rt3)(cmd)

def exec_done(pr: Out & Xp.ExecRes, k: Nat, +call: T.Call,
    +after: List<&2, T.Act>, +rt2: Rtp.Rt, +live: Bool,
    next: Nat -> Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pr:
    case (out3, +r):
      do IO<RtEnd>:
        out4 : Out <- emit_res_ann(out3, Rtp.call_id(call), r, live)
        +cmd : T.Cmd <- IO.pure(T.Cmd, X.exec_cmd(r, Rtp.call_id(call)))
        pd : Out & Rtp.Rt <- drain_steer(out4, rt2)
        exec_done.drained(pd, k, cmd, next)

# the same boundary for a model result: the completion is applied on the
# drained state, so steering that arrived during the call is already in
# the turn and lands in the next model input
def model_done(pd: Out & Rtp.Rt, k: Nat, +res: Rem.RemoteRes, +action: U32,
    +after: List<&2, T.Act>,
    next: Nat -> Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out3, +rt3):
      do IO<RtEnd>:
        +cmd : T.Cmd <- IO.pure(T.Cmd, rename_cmd(Rem.remote_cmd(res, action)))
        next(k)(out3)(rt3)(cmd)

# the provider call carries the loop's output (its retry notes reach
# the client while the call waits): Out -> Pv.Note and back
def note_of(out: Out) -> Pv.Note:
  match out:
    case OStd{}:
      Pv.NStd{}
    case ONet{sock}:
      Pv.NNet{sock}

def out_of(n: Pv.Note) -> Out:
  match n:
    case Pv.NStd{}:
      OStd{}
    case Pv.NNet{sock}:
      ONet{sock}

# the checkpoint before every provider call: the history is coherent
# there (every tool result is in, the completion is not asked yet), so
# a process that dies during the call - or anywhere later in the hop -
# restarts on everything the turn had done so far (the checkpoint text
# carries no turn and no pending: the restored session is idle)
def save_rt(+rt: Rtp.Rt) -> IO(Unit):
  match rt:
    case Rtp.Rt{+sess, _, _, _}:
      P.save(sess)

def model_back(nr: Pv.Note & Rem.RemoteRes, +rt2: Rtp.Rt, k: Nat, +action: U32,
    +after: List<&2, T.Act>,
    next: Nat -> Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match nr:
    case (n, +res):
      do IO<RtEnd>:
        pd : Out & Rtp.Rt <- drain_steer(out_of(n), rt2)
        model_done(pd, k, res, action, after, next)

# The IO loop. The pure transition is computed as an argument and matched
# as a parameter, so `run` is a single self-recursive def; every hop burns
# one unit of fuel, so the Runtime terminates like the Core. The loop
# returns the final session and output handle, so callers (the scenarios,
# the REPL) can continue from where it stopped.
# @unsafe: the EExec branch recurses through the exec_done continuation,
# and the EModel branch through model_done (both carry the drained state)
@unsafe
def run(fuel: Nat, out: Out, +rt: Rtp.Rt, +job: Rtp.Job, tr: Rtp.Trans) -> IO(RtEnd):
  match fuel:
    case 0n:
      match rt:
        case Rtp.Rt{+sess, _, _, _}:
          IO.pure(RtEnd, REnd{sess, out})
    case 1n+k:
      match rt:
        case Rtp.Rt{+sess, _, _, _}:
          match job:
            case Rtp.JDone{}:
              IO.pure(RtEnd, REnd{sess, out})
            case _:
              match tr:
                case Rtp.Tr{effect, +rt2, +job2}:
                  match effect:
                    case Rtp.ENone{}:
                      run(k, out, rt2, job2, Rtp.transition(rt2, job2))
                    case Rtp.EPrints{lines}:
                      do IO<RtEnd>:
                        out2 : Out <- emit(out, lines)
                        run(k, out2, rt2, job2, Rtp.transition(rt2, job2))
                    case Rtp.EModel{+action, +request, +after}:
                      # the real provider call (the hub HTTP client);
                      # the side channel is drained before the result is
                      # applied, so steering sent during the call enters
                      # the turn at this safe boundary
                      do IO<RtEnd>:
                        sv : Unit <- save_rt(rt2)
                        nr : Pv.Note & Rem.RemoteRes <- Pv.model_call(
                          note_of(out), request)
                        model_back(nr, rt2, k, action, after,
                          k2 => o3 => +r3 => +c =>
                            run(k2, o3, r3, Rtp.JCmd{c, after},
                              Rtp.transition(r3, Rtp.JCmd{c, after})))
                    case Rtp.EExec{+call, +after, +live}:
                      # the real tool: bash, the merged search, or a
                      # remote MCP call resolved by name. The annotations
                      # carry name/args/result to the TUI (live only);
                      # program sub-calls annotate inside exec_program.
                      do IO<RtEnd>:
                        out1 : Out <- emit_call_ann(out, call, live)
                        out2 : Out <- emit_code_ann(out1, call, live)
                        pr : Out & Xp.ExecRes <- exec_tool(out2, call, live)
                        exec_done(pr, k, call, after, rt2, live,
                          k2 => o4 => +r3 => +c =>
                            run(k2, o4, r3, Rtp.JCmd{c, after},
                              Rtp.transition(r3, Rtp.JCmd{c, after})))

# ---- scenarios (ADR 0006: the Core tested through the Step Protocol) ----

def sys_prompt() -> String:
  "You are a precise assistant running on the Bend Unified Harness."

def cfg(threshold: Nat, max_nulls: Nat) -> T.Cfg:
  T.Cfg{threshold, 40n, max_nulls, sys_prompt(), X.catalog_short()}
