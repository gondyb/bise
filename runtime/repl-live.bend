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
#   reload            restart the process with the latest code: only
#                     accepted between turns (reload_allowed); the parent
#                     recompiles, respawns on the same port and restores
#                     the checkpointed session (BEND_CONTINUE=1)
#   quit              close the connection; the session survives
#
# The whole loop follows the echo-server pattern from the demos: the
# per-line helpers take the loop continuation as a closure, and the
# accept loop is @unsafe (it runs until killed).
import Base
import ../core/types.bend as T
import ../core/compaction.bend as C
import ../core/commands.bend as Cm
import ../core/api.bend as Api
import ../core/session.bend as S
import ./persist.bend as P
import ./mcp.bend as Mcp
import ./skills.bend as Sk
import ./model.bend as M
import ./tools.bend as X
import ./main.bend as R
import ../core/config.bend as Cf

# ---- what one input line asks for ----

type Plan is Data:
  PRun{cmds: List<&2, T.Cmd>, script: List<&2, M.Reply>}
  PHelp{}
  PQuit{}
  PReload{}

# what the loops return: keep serving, or stop. The reload path
# unwinds to main so the process exits 0 — the parent then recompiles
# the latest source and respawns on the SAME session (--continue)
type Flow is Data:
  Go{sess: T.Session}
  Stop{}

def repl_help() -> List<&2, String>:
  Con{"bend-harness REPL. Commands:",
   Con{"  say <text>       start a turn (say tools ... / say prog: const s = search_tool_functions(best_match: catalog); return s;)",
   Con{"  steer <text>     steering input",
   Con{"  notify <text>    a notification",
   Con{"  compact          manual compaction",
   Con{"  interrupt        interrupt",
   Con{"  reload           restart with the latest code (between turns only)",
   Con{"  quit             close (the session survives)",
   Nil{}}}}}}}}}

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

def plan_reload(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PReload{}
    case False{}:
      PHelp{}

def plan_quit(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PQuit{}
    case False{}:
      plan_reload(String.eq(op, "reload"), op)

def plan_interrupt(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Interrupt{}, Nil{}}, Nil{}}
    case False{}:
      plan_quit(String.eq(op, "quit"), op)

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
      PRun{Con{T.UserMsg{T.Steer{}, Api.unescape_nl(rest)}, Nil{}},
        script_for_entry(Api.unescape_nl(rest))}
    case False{}:
      plan_notify(String.eq(op, "notify"), op, rest)

def plan_say(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      # the client escapes real newlines to keep the socket line
      # single-line; the message text carries the real newlines
      PRun{Con{T.UserMsg{T.Queue{}, Api.unescape_nl(rest)}, Nil{}},
        script_for_entry(Api.unescape_nl(rest))}
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
# socket and session). Everything returns Flow: Go keeps serving,
# Stop unwinds to main (the reload path — the process exits 0 and the
# parent respawns the latest code on the same session).

def repl_out(out: R.Out, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match out:
    case R.OStd{}:
      IO.pure(Flow, Go{sess})
    case R.ONet{s2}:
      next(s2)(sess)

def repl_end(e: R.RtEnd, next: Socket -> T.Session -> IO(Flow))
    -> IO(Flow):
  match e:
    case R.REnd{sess2, out2}:
      repl_out(out2, sess2, next)

# ---- /reload and the deferred self.reload: gated, then a clean exit ----

# refused: turn/pending/queued/notifs would be lost — the checkpoint
# only serializes turn boundaries (ADR 0002); say so, keep serving
def repl_reload.busy(out: R.Out, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    out2 : R.Out <- R.emit(out,
      Con{"  obs: reload refused: not between turns — finish or interrupt the pending turn first",
        Con{"--- idle", Nil{}}})
    repl_out(out2, sess, next)

# accepted: ack, checkpoint, close — Stop unwinds so the process exits
# 0; the parent recompiles the latest source and respawns with
# BEND_CONTINUE=1 (same port, same session file). Background command
# slots live in BEND_BG_ROOT on disk: they keep running across the
# reload and stay reachable from the new process.
def repl_reload.halt(out: R.Out, +sess: T.Session) -> IO(Flow):
  match out:
    case R.OStd{}:
      do IO<Flow>:
        P.save(sess)
        IO.pure(Flow, Stop{})
    case R.ONet{s2}:
      do IO<Flow>:
        Socket.close(s2)
        P.save(sess)
        IO.pure(Flow, Stop{})

def repl_reload(out: R.Out, ok: Bool, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match ok:
    case True{}:
      do IO<Flow>:
        out2 : R.Out <- R.emit(out,
          Con{"  obs: reload: restarting the harness with the latest code",
            Con{"  obs: reload: the session is preserved — background commands keep running",
              Con{"--- idle", Nil{}}}})
        repl_reload.halt(out2, sess)
    case False{}:
      repl_reload.busy(out, sess, next)

# ---- deferred self.reload / self.compact (the agent's own requests) ----
# A tool result acked "compact: scheduled" / "reload: scheduled" is a
# request the agent filed mid-turn. It executes HERE, at the turn
# boundary: compact first (the checkpoint then carries the compacted
# session into the restarted process), then reload through the same
# gated path as the /reload command. The request set is computed ONCE
# — the compaction turn itself is never re-scanned, so a failing
# compaction cannot loop.

def repl_deferred.reload(+sess: T.Session, out: R.Out,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    +ok : Bool <- IO.pure(Bool, S.reload_allowed(sess))
    f : Flow <- repl_reload(out, ok, sess, next)
    IO.pure(Flow, f)

# after the deferred compaction: save, then the reload if it was also
# requested, else keep serving
def repl_deferred.compact.after.pick(reload: Bool, +sess3: T.Session,
    out3: R.Out, next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match reload:
    case True{}:
      do IO<Flow>:
        u : Unit <- P.save(sess3)
        f : Flow <- repl_deferred.reload(sess3, out3, next)
        IO.pure(Flow, f)
    case False{}:
      do IO<Flow>:
        u : Unit <- P.save(sess3)
        f : Flow <- repl_out(out3, sess3, next)
        IO.pure(Flow, f)

def repl_deferred.compact.after(reload: Bool, e: R.RtEnd,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match e:
    case R.REnd{sess3, out3}:
      repl_deferred.compact.after.pick(reload, sess3, out3, next)

def repl_deferred.compact(reload: Bool, +sess2: T.Session, out2: R.Out,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    e : R.RtEnd <- R.run(1000000n, out2,
      R.Rt{sess2, Nil{}, Con{T.Compact{""}, Nil{}}, True{}}, R.JIdle{},
      R.transition(R.Rt{sess2, Nil{}, Con{T.Compact{""}, Nil{}}, True{}},
        R.JIdle{}))
    repl_deferred.compact.after(reload, e, next)

def repl_deferred.reload.only(reload: Bool, +sess2: T.Session, out2: R.Out,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match reload:
    case True{}:
      repl_deferred.reload(sess2, out2, next)
    case False{}:
      repl_out(out2, sess2, next)

def repl_deferred.compact.only(compact: Bool, reload: Bool, +sess2: T.Session,
    out2: R.Out, next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match compact:
    case True{}:
      repl_deferred.compact(reload, sess2, out2, next)
    case False{}:
      repl_deferred.reload.only(reload, sess2, out2, next)

def repl_deferred(d: R.Def, +sess2: T.Session, out2: R.Out,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match d:
    case R.Def{+compact, +reload}:
      repl_deferred.compact.only(compact, reload, sess2, out2, next)

def repl_end_save.then(+sess2: T.Session, out2: R.Out,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    # BR-002: the turn is over - a stale interrupt flag would kill the
    # NEXT turn at its first boundary. The runtime is the authority on
    # turn boundaries: clear the flag now; a flag written later
    # belongs to a later turn (or to no turn at all).
    +ipath : String <- R.interrupt_path()
    uc : Unit <- P.write_file(ipath, "")
    P.save(sess2)
    # the turn asked for nothing: keep serving; else run its requests
    +d : R.Def <- IO.pure(R.Def, R.deferred_of(P.history_of(sess2)))
    f : Flow <- repl_deferred(d, sess2, out2, next)
    IO.pure(Flow, f)

def repl_end_save(e: R.RtEnd, next: Socket -> T.Session -> IO(Flow))
    -> IO(Flow):
  match e:
    case R.REnd{sess2, out2}:
      repl_end_save.then(sess2, out2, next)

# a stalled session (fuel exhausted mid-turn) must say so, never die
# silently: the turn is still pending and the user must know
def turn_active(s: T.Session) -> Bool:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          False{}
        case Some{_}:
          True{}

def repl_end_stalled.stalled(active: Bool, e: R.RtEnd,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match active:
    case True{}:
      match e:
        case R.REnd{+sess2, out2}:
          do IO<Flow>:
            out3 : R.Out <- R.emit(out2,
              Con{"  obs: turn_stalled: execution budget exhausted",
                Con{"--- idle", Nil{}}})
            P.save(sess2)
            repl_out(out3, sess2, next)
    case False{}:
      repl_end_save(e, next)

def repl_end_stalled(e: R.RtEnd,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match e:
    case R.REnd{+sess2, _}:
      repl_end_stalled.stalled(turn_active(sess2), e, next)

def repl_plan(s: Socket, plan: Plan, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match plan:
    case PQuit{}:
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess})
    case PHelp{}:
      do IO<Flow>:
        out2 : R.Out <- R.emit(R.ONet{s}, repl_help())
        repl_out(out2, sess, next)
    case PReload{}:
      do IO<Flow>:
        +ok : Bool <- IO.pure(Bool, S.reload_allowed(sess))
        f : Flow <- repl_reload(R.ONet{s}, ok, sess, next)
        IO.pure(Flow, f)
    case PRun{+cmds, +script}:
      do IO<Flow>:
        # 1M hops: effectively unbounded for a terminal session (the
        # old 100 died mid-turn around ~20 model rounds, silently)
        e : R.RtEnd <- R.run(1000000n, R.ONet{s},
          R.Rt{sess, script, cmds, True{}}, R.JIdle{},
          R.transition(R.Rt{sess, script, cmds, True{}}, R.JIdle{}))
        repl_end_stalled(e, next)

def repl_interpret(s: Socket, i: Cm.Interpretation, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match i:
    case Cm.Send{+line}:
      repl_plan(s, plan_of(op_of(line)), sess, next)
    case Cm.Note{+warning}:
      # a local warning: no protocol effect, the session is untouched;
      # the idle marker keeps the client contract (batch + idle)
      do IO<Flow>:
        out2 : R.Out <- R.emit(R.ONet{s},
          Con{String.append("  obs: ", warning), Con{"--- idle", Nil{}}})
        repl_out(out2, sess, next)

def repl_cmd(s: Socket, +line: String, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  repl_interpret(s, Cm.interpret(line), sess, next)

# a chunk may carry SEVERAL lines (kernel coalescing while a turn or a
# compaction ran); each line becomes its own command, run in order
def repl_nl.get(m: Maybe<&2, Char>) -> Char:
  match m:
    case Some{+c}:
      c
    case None{}:
      Char.from_u32(10)

def repl_nl() -> Char:
  repl_nl.get(String.get("\n", 0n))

def cmd_lines.keep(empty: Bool, +l: String, +rest: List<&2, String>)
    -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{l, rest}

def cmd_lines.go(lines: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      cmd_lines.keep(String.is_empty(String.trim(l)), String.trim(l),
        cmd_lines.go(rest))

def cmd_lines(+data: String) -> List<&2, String>:
  cmd_lines.go(String.split(data, repl_nl()))

# @unsafe: the continuation closure self-references the batch loop
@unsafe
def repl_lines.go(+lines: List<&2, String>, s: Socket, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match lines:
    case Nil{}:
      next(s)(sess)
    case Con{+l, +rest}:
      do IO<Flow>:
        f : Flow <- repl_cmd(s, l, sess,
          s2 => sess2 => repl_lines.go(rest, s2, sess2, next))
        IO.pure(Flow, f)

def repl_data(closed: Bool, s: Socket, +data: String, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match closed:
    case True{}:
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess})
    case False{}:
      repl_lines.go(cmd_lines(data), s, sess, next)

def repl_after(rm: Socket & Result<&1, &1, U32 & String, String>,
    +sess: T.Session, next: Socket -> T.Session -> IO(Flow))
    -> IO(Flow):
  match rm:
    case (s, Fail{_}):
      do IO<Flow>:
        Socket.close(s)
        IO.pure(Flow, Go{sess})
    case (s, Done{+data}):
      repl_data(String.is_empty(data), s, data, sess, next)

@unsafe
def repl_conn(sock: Socket, sess: T.Session) -> IO(Flow):
  do IO<Flow>:
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

# the restored queue restarts the turn chain by itself (checkpoint v2):
# the runtime idle boundary pops the queued head into a fresh turn,
# so the message the user queued before the reload runs without anyone
# re-typing it; further queued messages chain the same way inside the
# one run loop entry. Turn/pending are still boundary-empty at a reload,
# so only the queue and the held notifications resume here.
def resume_queued.run(s: Socket, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  do IO<Flow>:
    e : R.RtEnd <- R.run(1000000n, R.ONet{s},
      R.Rt{sess, Nil{}, Nil{}, True{}}, R.JIdle{},
      R.transition(R.Rt{sess, Nil{}, Nil{}, True{}}, R.JIdle{}))
    f : Flow <- repl_end_stalled(e, next)
    IO.pure(Flow, f)

def resume_queued(empty: Bool, s: Socket, +sess: T.Session,
    next: Socket -> T.Session -> IO(Flow)) -> IO(Flow):
  match empty:
    case True{}:
      next(s)(sess)
    case False{}:
      resume_queued.run(s, sess, next)

def greet_conn(out: R.Out, +sess: T.Session) -> IO(Flow):
  match out:
    case R.OStd{}:
      IO.pure(Flow, Go{sess})
    case R.ONet{s2}:
      resume_queued(List.is_empty(&2, String, S.sess_queued(sess)), s2,
        sess, s3 => sess3 => repl_conn(s3, sess3))



# the connection loop ended: Go keeps serving (checkpoint + accept the
# next client), Stop unwinds to main (the reload exit)
def serve_after.cont(f: Flow, l2: Listener,
    next: Listener -> Maybe<&2, Nat> -> T.Session -> IO(Flow))
    -> IO(Flow):
  match f:
    case Go{+sess2}:
      do IO<Flow>:
        P.save(sess2)
        next(l2)(None{})(sess2)
    case Stop{}:
      IO.pure(Flow, Stop{})

def serve_after(am: Listener & Result<&1, &1, U32 & String, Socket>,
    +greet: Maybe<&2, Nat>, +sess: T.Session,
    next: Listener -> Maybe<&2, Nat> -> T.Session -> IO(Flow))
    -> IO(Flow):
  match am:
    case (l2, Fail{_}):
      do IO<Flow>:
        u : Unit <- IO.print("accept failed")
        IO.pure(Flow, Stop{})
    case (l2, Done{s}):
      do IO<Flow>:
        out2 : R.Out <- greet_out(greet, s)
        f : Flow <- greet_conn(out2, sess)
        serve_after.cont(f, l2, next)

@unsafe
def serve(l: Listener, greet: Maybe<&2, Nat>, sess: T.Session) -> IO(Flow):
  do IO<Flow>:
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

def env_of(+s: String) -> Result<&1, &1, U32 & String, String>:
  Done{s}

# the threshold answers the same precedence as everything else:
# BEND_THRESHOLD env > config.toml threshold > 800000
def thr_of.pick(empty: Bool, +envv: String, +cfgt: String) -> Nat:
  match empty:
    case True{}:
      U32.to_nat(Cf.cfg_u32(cfgt, "threshold", 800000))
    case False{}:
      thr_of(env_of(envv))

def thr_of_cfg(+envv: String, +cfgt: String) -> Nat:
  thr_of.pick(String.is_empty(envv), envv, cfgt)

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

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), then the skills sections
# (rules before the <available-skills> catalog, the vibe_sdk order);
# both only when the skills index is non-empty
def live_prompt.go(+base: String, +content: String) -> String:
  Sk.prompt_with(base, content)

def live_prompt() -> IO(String):
  do IO<String>:
    +tu : String <- prompt_read("prompt-tool-use.txt")
    +ct : String <- prompt_read("prompt-current-time.txt")
    +content : String <- Sk.prompt_content()
    IO.pure(String, live_prompt.go(prompt_join(tu, ct), content))

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

def flow_note.body(stopped: Bool) -> IO(Unit):
  match stopped:
    case True{}:
      do IO<Unit>:
        u2 : Unit <- IO.print("reload-exit")
        IO.pure(Unit, Unit{})
    case False{}:
      IO.pure(Unit, Unit{})

def flow_note(f: Flow) -> IO(Unit):
  match f:
    case Go{_}:
      flow_note.body(False{})
    case Stop{}:
      flow_note.body(True{})

def main() -> IO(Unit):
  do IO<Unit>:
    # the config template lands on disk on first run, before anything
    # reads it
    ut : Unit <- R.write_cfg_template()
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_THRESHOLD")
    +cfgt : String <- R.read_cfg()
    +threshold : Nat <- IO.pure(Nat, thr_of_cfg(R.env_or(env, ""), cfgt))
    +p : U32 <- repl_port()
    l : Listener <- IO.try(Listener, TCP.listen(p))
    IO.print(String.append("bend-harness LIVE REPL on 127.0.0.1:",
      String.append(U32.show(p),
        String.append(" (real model, FS tools, bash) — compaction threshold: ",
          String.append(Nat.show(threshold), " tokens (BEND_THRESHOLD or ~/.bend-harness/config.toml to change)")))))
    +cfg : T.Cfg <- live_cfg(threshold)
    +ld : P.Load <- P.load(T.new_session(cfg))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, R.env_or(kr, ""))
    u : Unit <- Mcp.bootstrap(key)
    u4 : Unit <- Sk.skills_scan()
    u3 : Unit <- R.bash_cleanup(U32.show(p))
    f : Flow <- serve(l, ld_greet(ld), ld_sess(ld))
    # Stop (a /reload) lands here: main returns, the process exits 0,
    # and the parent respawns the latest code on the same session.
    # The marker line lets the parent tell a reload from a crash.
    u2 : Unit <- flow_note(f)
    IO.pure(Unit, Unit{})
