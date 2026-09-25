# runtime/repl.bend — a small interactive REPL over TCP.
#
# Bend has no stdin effect, so the REPL is a line server: the session
# persists across lines and across connections (until the process dies).
#
#   bend runtime/repl-live.bend -o repl-live && ./repl-live
#   nc 127.0.0.1 7702
#
# The provider call and the bash tool run INSIDE this process: the hub
# HTTP client does the TLS call to api.mistral.ai (core/api.bend maps
# the wire request to the API JSON and the reply back), and the hub snap
# runner execvp's /bin/sh -c. No bridge process anymore.
#
# Commands (one per line):
#   say <text>        start a turn. "say tools ..." makes the model call
#                     search_tool_functions; "say prog: <program>"
#                     makes it call run_typescript (D1).
#   steer <text>      steering input (at idle the Core starts a turn with it)
#   notify <text>     a notification
#   compact           manual between-turn compaction
#   interrupt         interrupt (an accepted no-op while idle)
#   quit              close the connection; the session survives
#
# The whole loop follows the echo-server pattern from the demos: the
# per-line helpers take the loop continuation as a closure, and the
# accept loop is @unsafe (it runs until killed).
import Base
import ../core/types.bend as T
import ../core/compaction.bend as C
import ../core/commands.bend as Cm
import ./persist.bend as P
import ./mcp.bend as Mcp
import ./model.bend as M
import ./tools.bend as X
import ./main.bend as R

# ---- what one input line asks for ----

type Plan is Data:
  PRun{cmds: List<&2, T.Cmd>, script: List<&2, M.Reply>}
  PHelp{}
  PQuit{}

def repl_help() -> List<&2, String>:
  Con{"bend-harness REPL. Commands:",
   Con{"  say <text>       start a turn (say tools ... / say prog: const s = search_tool_functions(best_match: catalog); return s;)",
   Con{"  steer <text>     steering input",
   Con{"  notify <text>    a notification",
   Con{"  compact          manual compaction",
   Con{"  interrupt        interrupt",
   Con{"  quit             close (the session survives)",
   Nil{}}}}}}}}

# the scripted model's plan for a turn
def script_for.prog(is_prog: Bool, +text: String) -> List<&2, M.Reply>:
  match is_prog:
    case True{}:
      Con{M.ROk{"", Con{T.Call{1, "run_typescript",
          String.trim(String.drop(text, 5n))}, Nil{}}},
        Con{M.ROk{"Program complete.", Nil{}}, Nil{}}}
    case False{}:
      Con{M.ROk{String.append("You said: ", text), Nil{}}, Nil{}}

def script_for(is_tools: Bool, is_prog: Bool, +text: String)
    -> List<&2, M.Reply>:
  match is_tools:
    case True{}:
      Con{M.ROk{"", Con{T.Call{1, "search_tool_functions", "best_match: catalog"},
            Nil{}}},
        Con{M.ROk{"I searched the catalog.", Nil{}}, Nil{}}}
    case False{}:
      script_for.prog(is_prog, text)

def script_for_entry(+text: String) -> List<&2, M.Reply>:
  script_for(String.starts_with(text, "tools"),
    String.starts_with(text, "prog:"), text)

# ---- command dispatch ----

type CmdLine is Data:
  CL{op: String, rest: String}

def op_of.found(sp: Maybe<&2, Nat>, +line: String) -> CmdLine:
  match sp:
    case None{}:
      CL{line, ""}
    case Some{+p}:
      CL{String.take(line, p), String.drop(line, Nat.add(p, 1n))}

def op_of(+line: String) -> CmdLine:
  op_of.found(C.find(line, " ", 0n), line)

def plan_quit(eq: Bool) -> Plan:
  match eq:
    case True{}:
      PQuit{}
    case False{}:
      PHelp{}

def plan_interrupt(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Interrupt{}, Nil{}}, Nil{}}
    case False{}:
      plan_quit(String.eq(op, "quit"))

def plan_compact(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Compact{""}, Nil{}},
        Con{M.ROk{"<summary>The REPL conversation so far, compacted on request.</summary>", Nil{}},
          Nil{}}}
    case False{}:
      plan_interrupt(String.eq(op, "interrupt"), op)

def plan_notify(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Notify{rest}, Nil{}}, Nil{}}
    case False{}:
      plan_compact(String.eq(op, "compact"), op)

def plan_steer(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.UserMsg{T.Steer{}, rest}, Nil{}}, script_for_entry(rest)}
    case False{}:
      plan_notify(String.eq(op, "notify"), op, rest)

def plan_say(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.UserMsg{T.Queue{}, rest}, Nil{}}, script_for_entry(rest)}
    case False{}:
      plan_steer(String.eq(op, "steer"), op, rest)

def plan_of(cl: CmdLine) -> Plan:
  match cl:
    case CL{+op, +rest}:
      plan_say(String.eq(op, "say"), op, rest)

# ---- the connection loop ----
# Each helper takes the loop continuation as a curried closure (the
# echo-server pattern: closures cannot match, so extraction happens in
# ordinary defs, and the closure only re-enters the loop with the new
# socket and session).

def repl_out(out: R.Out, +sess: T.Session,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  match out:
    case R.OStd{}:
      IO.pure(T.Session, sess)
    case R.ONet{s2}:
      next(s2)(sess)

def repl_end(e: R.RtEnd, next: Socket -> T.Session -> IO(T.Session))
    -> IO(T.Session):
  match e:
    case R.REnd{sess2, out2}:
      repl_out(out2, sess2, next)

def repl_end_save.then(+sess2: T.Session, out2: R.Out,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  do IO<T.Session>:
    P.save(sess2)
    repl_out(out2, sess2, next)

def repl_end_save(e: R.RtEnd, next: Socket -> T.Session -> IO(T.Session))
    -> IO(T.Session):
  match e:
    case R.REnd{sess2, out2}:
      repl_end_save.then(sess2, out2, next)

def repl_plan(s: Socket, plan: Plan, +sess: T.Session,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  match plan:
    case PQuit{}:
      do IO<T.Session>:
        Socket.close(s)
        IO.pure(T.Session, sess)
    case PHelp{}:
      do IO<T.Session>:
        out2 : R.Out <- R.emit(R.ONet{s}, repl_help())
        repl_out(out2, sess, next)
    case PRun{+cmds, +script}:
      do IO<T.Session>:
        e : R.RtEnd <- R.run(100n, R.ONet{s},
          R.Rt{sess, script, cmds, True{}}, R.JIdle{},
          R.transition(R.Rt{sess, script, cmds, True{}}, R.JIdle{}))
        repl_end_save(e, next)

def repl_interpret(s: Socket, i: Cm.Interpretation, +sess: T.Session,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  match i:
    case Cm.Send{+line}:
      repl_plan(s, plan_of(op_of(line)), sess, next)
    case Cm.Note{+warning}:
      # a local warning: no protocol effect, the session is untouched;
      # the idle marker keeps the client contract (batch + idle)
      do IO<T.Session>:
        out2 : R.Out <- R.emit(R.ONet{s},
          Con{String.append("  obs: ", warning), Con{"--- idle", Nil{}}})
        repl_out(out2, sess, next)

def repl_cmd(s: Socket, +line: String, +sess: T.Session,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  repl_interpret(s, Cm.interpret(line), sess, next)

def repl_data(closed: Bool, s: Socket, +data: String, +sess: T.Session,
    next: Socket -> T.Session -> IO(T.Session)) -> IO(T.Session):
  match closed:
    case True{}:
      do IO<T.Session>:
        Socket.close(s)
        IO.pure(T.Session, sess)
    case False{}:
      repl_cmd(s, String.trim(data), sess, next)

def repl_after(rm: Socket & Result<&1, &1, U32 & String, String>,
    +sess: T.Session, next: Socket -> T.Session -> IO(T.Session))
    -> IO(T.Session):
  match rm:
    case (s, Fail{_}):
      do IO<T.Session>:
        Socket.close(s)
        IO.pure(T.Session, sess)
    case (s, Done{+data}):
      repl_data(String.is_empty(data), s, data, sess, next)

@unsafe
def repl_conn(sock: Socket, sess: T.Session) -> IO(T.Session):
  do IO<T.Session>:
    rm : Socket & Result<&1, &1, U32 & String, String> <- TCP.recv(sock, 4096)
    repl_after(rm, sess, s2 => sess2 => repl_conn(s2, sess2))

# ---- the accept loop: one connection at a time, session survives ----

# the --continue greeting: sent on the first connection of a restored
# session, then never again
def greet_lines(+n: Nat) -> List<&2, String>:
  # no idle marker: the greeting is unsolicited, not a turn's end — an
  # idle here would make clients give up waiting for the first turn
  Con{String.append("  obs: session_restored: ",
      String.append(Nat.show(n), " messages")), Nil{}}

def greet_out(greet: Maybe<&2, Nat>, s: Socket) -> IO(R.Out):
  match greet:
    case Some{+n}:
      R.emit(R.ONet{s}, greet_lines(n))
    case None{}:
      IO.pure(R.Out, R.ONet{s})

def greet_conn(out: R.Out, +sess: T.Session) -> IO(T.Session):
  match out:
    case R.OStd{}:
      IO.pure(T.Session, sess)
    case R.ONet{s2}:
      repl_conn(s2, sess)



def serve_after(am: Listener & Result<&1, &1, U32 & String, Socket>,
    +greet: Maybe<&2, Nat>, +sess: T.Session,
    next: Listener -> Maybe<&2, Nat> -> T.Session -> IO(Unit))
    -> IO(Unit):
  match am:
    case (l2, Fail{_}):
      IO.print("accept failed")
    case (l2, Done{s}):
      do IO<Unit>:
        out2 : R.Out <- greet_out(greet, s)
        +sess2 : T.Session <- greet_conn(out2, sess)
        P.save(sess2)
        next(l2)(None{})(sess2)

@unsafe
def serve(l: Listener, greet: Maybe<&2, Nat>, sess: T.Session) -> IO(Unit):
  do IO<Unit>:
    am : Listener & Result<&1, &1, U32 & String, Socket> <- TCP.accept(l)
    serve_after(am, greet, sess, l2 => g2 => s2 => serve(l2, g2, s2))

# the compaction threshold for real conversations: BEND_THRESHOLD env
# (approximate tokens). Default 800000, tuned for 1M-context models
# (e.g. glm-5.3): compaction starts near 80% of the window, with the
# ADR 0011 selection budget (20000) for preserved user messages
def thr_num(m: Maybe<&2, U32>) -> Nat:
  match m:
    case Some{+u}:
      U32.to_nat(u)
    case None{}:
      800000n

def thr_of(r: Result<&1, &1, U32 & String, String>) -> Nat:
  match r:
    case Fail{_}:
      800000n
    case Done{+s}:
      thr_num(U32.read(s))

# the REPL listen port: BEND_REPL_PORT env (default 7702) so several
# harness instances can run side by side
def repl_port.num(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+u}:
      u
    case None{}:
      7702

def repl_port.pick(r: Result<&1, &1, U32 & String, String>) -> U32:
  match r:
    case Fail{_}:
      7702
    case Done{+s}:
      repl_port.num(U32.read(s))

def repl_port() -> IO(U32):
  do IO<U32>:
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_REPL_PORT")
    IO.pure(U32, repl_port.pick(env))

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), loaded from prompt-*.txt
def prompt_read(+path: String) -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file(path)
    IO.pure(String, X.desc_or(m, ""))

def prompt_join(+tu: String, +ct: String) -> String:
  String.append(
    "You are a precise assistant running on the Bend Unified Harness. Discover additional tools with `search_tool_functions`, and call them using `run_typescript`.",
    String.append("\n\n", String.append(tu, String.append("\n\n", ct))))

def live_prompt() -> IO(String):
  do IO<String>:
    +tu : String <- prompt_read("prompt-tool-use.txt")
    +ct : String <- prompt_read("prompt-current-time.txt")
    IO.pure(String, prompt_join(tu, ct))

def live_cfg(+threshold: Nat) -> IO(T.Cfg):
  do IO<T.Cfg>:
    +p : String <- live_prompt()
    +cat : List<&2, T.ToolDef> <- X.catalog()
    IO.pure(T.Cfg, T.Cfg{threshold, 20000n, 3n, p, cat})

def ld_sess(ld: P.Load) -> T.Session:
  match ld:
    case P.LD{+sess, _}:
      sess

def ld_greet(ld: P.Load) -> Maybe<&2, Nat>:
  match ld:
    case P.LD{_, +greet}:
      greet

def main() -> IO(Unit):
  do IO<Unit>:
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_THRESHOLD")
    +threshold : Nat <- IO.pure(Nat, thr_of(env))
    +p : U32 <- repl_port()
    l : Listener <- IO.try(Listener, TCP.listen(p))
    IO.print(String.append("bend-harness LIVE REPL on 127.0.0.1:",
      String.append(U32.show(p),
        String.append(" (real model, FS tools, bash) — compaction threshold: ",
          String.append(Nat.show(threshold), " tokens (BEND_THRESHOLD to change)")))))
    +cfg : T.Cfg <- live_cfg(threshold)
    +ld : P.Load <- P.load(T.new_session(cfg))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, R.env_or(kr, ""))
    u : Unit <- Mcp.bootstrap(key)
    serve(l, ld_greet(ld), ld_sess(ld))
