# runtime/bash.bend — the bash tool.
#
# A command runs inside a generated /bin/sh wrapper: a sync window, then
# a background handoff with a file contract (slot dir, out, rc, stdin
# fifo). The contract text is pure and pinned by LAWS.bend; bash_exec
# is the one effect.
import Base
import ../core/text.bend as Tx
import ./tools.bend as X
import ./persist.bend as P
import ./mcp.bend as Mcp
import ../core/config.bend as Cf
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
import ./settings.bend as St
# ---- the bash tool, fully in Bend (hub snap runner) ----
# snap execvp's /bin/sh -c; the answer is the exit status on the first
# line, then stdout and stderr. Commands are flattened first (the wire
# already flattens call args — double safety).

def exec_of.pick.fail(+code: U32, +output: String) -> X.ExecRes:
  X.Ex{False{}, String.append("exit ",
    String.append(U32.show(code), String.append(": ", output)))}

# the handoff marker: the wrapper prints it as the very first line of
# its output, before the contract. The exit code is no longer the
# signal (a command that really exits 199 must report 199); only the
# wrapper can print this line first — a command's output is only
# cat'd after the marker would have been printed.
def HANDOFF_MARKER() -> String:
  "BEND-BG-HANDOFF-6f2c9e5b"

def exec_of.handoff.rest(sp: Maybe<&2, Nat>, +payload: String) -> X.ExecRes:
  match sp:
    case None{}:
      X.Ex{True{}, payload}
    case Some{+p}:
      X.Ex{True{}, String.drop(payload, Nat.add(p, 1n))}

def exec_of.handoff(+payload: String) -> X.ExecRes:
  # strip the marker line; keep the contract
  exec_of.handoff.rest(Tx.find(payload, "\n", 0n), payload)

def exec_of.pick.is_handoff(is_handoff: Bool, +code: U32,
    +output: String) -> X.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      exec_of.pick.fail(code, output)

# exit 0 without the marker is a plain success
def exec_of.pick.zero.ok(is_handoff: Bool, +output: String) -> X.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      X.Ex{True{}, output}

def exec_of.pick.zero(is_zero: Bool, +code: U32, +output: String) -> X.ExecRes:
  match is_zero:
    case True{}:
      exec_of.pick.zero.ok(
        String.starts_with(output, HANDOFF_MARKER()), output)
    case False{}:
      exec_of.pick.is_handoff(
        String.starts_with(output, HANDOFF_MARKER()), code, output)

def exec_of.pick(+code: U32, +output: String) -> X.ExecRes:
  exec_of.pick.zero(U32.is_eq(code, 0), code, output)

def exec_of.code.read(m: Maybe<&2, U32>, +output: String) -> X.ExecRes:
  match m:
    case Some{+c}:
      exec_of.pick(c, output)
    case None{}:
      exec_of.pick(1, output)

def exec_of.code(code_str: String, +output: String) -> X.ExecRes:
  exec_of.code.read(U32.read(code_str), output)

def exec_of.split(sp: Maybe<&2, Nat>, +out: String) -> X.ExecRes:
  match sp:
    case Some{+p}:
      exec_of.code(String.take(out, p), String.drop(out, Nat.add(p, 1n)))
    case None{}:
      exec_of.code(out, "")

def exec_of(+out: String) -> X.ExecRes:
  exec_of.split(Tx.find(out, "\n", 0n), out)

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

def bg_after.clamp2(over: Bool, +secs: U32) -> U32:
  match over:
    case True{}:
      3600
    case False{}:
      secs

def bg_after.clamp(+secs: U32) -> U32:
  bg_after.clamp2(U32.is_lt(3600, secs), secs)

def bg_after.env(+envv: String, +dflt: U32) -> U32:
  bg_after.read(U32.read(envv), dflt)

def bg_after.pick2(empty: Bool, +envv: String, +cfgt: String,
    +dflt: U32) -> U32:
  match empty:
    case True{}:
      Cf.cfg_u32(cfgt, "bg_after", dflt)
    case False{}:
      bg_after.env(envv, dflt)

def bg_after.or_cfg(+envv: String, +cfgt: String, +dflt: U32) -> U32:
  bg_after.pick2(String.is_empty(envv), envv, cfgt, dflt)

# env > config > default; the config file only feeds LIVE sessions
# (BEND_REPL_PORT is the live marker - the scripted suites stay
# hermetic against the user's config)
def bg_after.pick(port_empty: Bool, +envv: String) -> IO(U32):
  match port_empty:
    case True{}:
      IO.pure(U32, bg_after.clamp(bg_after.env(envv, 30)))
    case False{}:
      do IO<U32>:
        +cfgt : String <- St.read_cfg()
        IO.pure(U32, bg_after.clamp(bg_after.or_cfg(envv, cfgt, 30)))

def bg_after() -> IO(U32):
  do IO<U32>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_AFTER")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    bg_after.pick(String.is_empty(P.env_or(pr, "")), P.env_or(r, ""))

# the session's bg dir, pure: <root>/bend-bg-<port>. One dir per REPL
# process (keyed by port) so parallel terminals never share slots,
# and a /reload respawns on the same port and finds its slots again.
def bg_dir_of(+root: String, +port: String) -> String:
  String.append(root, String.append("/bend-bg-", port))

def bg_dir() -> IO(String):
  do IO<String>:
    root : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_ROOT")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +r : String <- IO.pure(String, P.env_or(root, "/tmp"))
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, bg_dir_of(r, p))

# ---- the agent-facing contract, pure ----
# Every driving line below is a COMPLETE shell command on a CONCRETE
# path: the agent copy-pastes it verbatim, no variables to resolve.
# The wrapper prints exactly these (its $i slot variable expanded at
# runtime), so what the laws pin is what the agent reads.

# one slot's file: <dir>/<id><ext>
def bg_slot(+bg: String, +id: String, +ext: String) -> String:
  String.append(bg, String.append("/", String.append(id, ext)))

def bg_headline(+bg: String, +id: String, +secs: U32) -> String:
  String.append("background: still running after ",
    String.append(U32.show(secs),
      String.append("s - id ",
        String.append(id, String.append(" (dir ", String.append(bg, ")"))))))

def bg_tail_cmd(+bg: String, +id: String) -> String:
  String.append("  output: tail -n 50 ", bg_slot(bg, id, ".out"))

def bg_stdin_cmd(+bg: String, +id: String) -> String:
  String.append("  stdin (if it reads): printf '%s\\n' 'text' > ",
    String.append(bg_slot(bg, id, ".in"),
      "  (a command that reads stdin WAITS for input indefinitely: feed it or kill it)"))

def bg_status_cmd(+bg: String, +id: String) -> String:
  String.append("  status: kill -0 $(cat ",
    String.append(bg_slot(bg, id, ".pid"),
      String.append(") && echo RUNNING || cat ",
        String.append(bg_slot(bg, id, ".rc"), " 2>/dev/null"))))

def bg_kill_cmd(+bg: String, +id: String) -> String:
  String.append("  kill: kill $(cat ",
    String.append(bg_slot(bg, id, ".pid"), ")"))

# the two guards: if the agent deletes the bg dir under a running
# wrapper (a legitimate cleanup), the waits must fail FAST with a
# clear message, never hang the tool
def bg_setup_guard(+bg: String) -> String:
  String.append("  if [ ! -d ", String.append(bg,
    String.append(" ]; then echo \"background: the bg dir vanished during setup: ",
      String.append(bg, "\"; exit 1; fi"))))

def bg_rc_guard(+bg: String) -> String:
  String.append("  if [ ! -d ", String.append(bg,
    String.append(" ]; then echo \"background: the bg dir vanished before the exit code was recorded: ",
      String.append(bg, "\"; exit 1; fi"))))

# 5 iterations at 20ms then 100ms steps: a trivial command costs
# ~30ms of polling instead of ~120ms, and the window stays ~after secs
def bg_poll_iters(+after: U32) -> U32:
  U32.add(U32.mul(after, 10), 5)

# the pid-wait also breaks on an existing .rc: the runner frees the
# slot (rm pid/cmd/in) as soon as the command ends, so for a fast
# command the pid file is EPHEMERAL — the wrapper must not wait for a
# file that already came and went. An empty pid then makes kill -0
# fail cleanly and the rc-wait completes the sync path.
# snap's wire joins argv with newlines, so a multi-line -c script is
# impossible; the wrapper goes to a file (the node_run pattern) and sh
# runs the file.
#
# The file is named by a hash of its OWN content: two different
# commands never share a wrapper file, so an orphaned wrapper (a
# process that died mid-command, still sleeping in its poll loop)
# can never re-read ANOTHER command's bytes when it wakes up. Re-running
# the same command rewrites identical bytes, which is safe: the orphan
# just continues its own script.
# a positional weighted sum: each char's code times (pos^2 + 1). No
# mod, no overflow (14 chars x 1.1M x ~200 << 2^48), deterministic,
# and different orders of the same chars hash apart. Fuel-first (the
# codebase idiom): the checker can normalize it in closed laws.
def bg_script_hash.go(fuel: Nat, +s: String, +pos: Nat, acc: Nat) -> Nat:
  match fuel:
    case 0n:
      acc
    case 1n+k:
      match s:
        case SNil{}:
          acc
        case SCon{Chr{+c}, t}:
          bg_script_hash.go(k, t, Nat.add(pos, 1n),
            Nat.add(acc,
              Nat.mul(U32.to_nat(c), Nat.add(Nat.mul(pos, pos), 1n))))

def bg_script_hash(+s: String) -> Nat:
  bg_script_hash.go(Nat.mul(String.length(s), 2n), s, 1n, 0n)

# the .cmd heredoc sentinel derives from the command's own hash: a
# command containing the sentinel would have to contain its own hash
def bg_sentinel(+command: String) -> String:
  String.append("XQ-", String.append(Nat.show(bg_script_hash(command)),
    "-EOF"))

def bash_script_path(+port: String, +script: String) -> String:
  String.append("/tmp/bend-sh-", String.append(port,
    String.append("-", String.append(Nat.show(bg_script_hash(script)), ".sh"))))

@unsafe
def bash_exec(command: String) -> IO(X.ExecRes):
  do IO<X.ExecRes>:
    +secs : U32 <- bg_after()
    +bg : String <- bg_dir()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    # NO flatten: real newlines stay — the heredoc and the runner
    # subshell both accept multi-line commands (flatten silently
    # restructured them)
    +script : String <- IO.pure(String,
      bg_wrap(String.trim(command), bg, secs))
    +path : String <- IO.pure(String, bash_script_path(port, script))
    w : Unit <- P.write_file(path, script)
    out : String <- Snap.run(Con{"/bin/sh", Con{path, Nil{}}})
    IO.pure(X.ExecRes, exec_of(out))

# boot cleanup: the wrappers of a previous process on this port cannot be
# running (the parent waits for the old REPL to die before respawning), so
# stale /tmp/bend-sh-<port>-*.sh files are safe to remove - the wrapper
# leak (one file per bash call) would otherwise grow without bound.
def bash_cleanup(+port: String) -> IO(Unit):
  do IO<Unit>:
    out : String <- Snap.run(Con{"/bin/sh", Con{"-c",
      Con{String.append("rm -f /tmp/bend-sh-",
        String.append(port, "-*.sh")), Nil{}}}})
    IO.pure(Unit, Unit{})

def exec_mr(m: Mcp.MR) -> X.ExecRes:
  match m:
    case Mcp.MR{+ok, +out}:
      X.Ex{ok, out}

def exec_search(+args: String, +live: Bool) -> IO(X.ExecRes):
  do IO<X.ExecRes>:
    r : Mcp.MR <- Mcp.merged_search(args, live)
    IO.pure(X.ExecRes, exec_mr(r))

def exec_mcp.key(+name: String, +args: String, +key: String)
    -> IO(X.ExecRes):
  do IO<X.ExecRes>:
    r : Mcp.MR <- Mcp.mcp_call(name, args, key)
    IO.pure(X.ExecRes, exec_mr(r))

def exec_mcp.scripted(+name: String) -> X.ExecRes:
  X.Ex{False{},
    String.append("unknown tool: ", String.append(name,
      " — remote MCP tools need a live session"))}

def exec_mcp.live(+name: String, +args: String) -> IO(X.ExecRes):
  do IO<X.ExecRes>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    exec_mcp.key(name, args, key)

def exec_mcp(+name: String, +args: String, live: Bool) -> IO(X.ExecRes):
  match live:
    case True{}:
      exec_mcp.live(name, args)
    case False{}:
      IO.pure(X.ExecRes, exec_mcp.scripted(name))

# ---- the wrapper: one line per generated shell statement ----
def bg_wrap.seg(+a: String, +b: String) -> String:
  String.append(a, String.append("\n", b))

# the contract lines carry the LITERAL dir (not $BG): at runtime the
# agent reads a fully concrete path, with only the slot id resolved.
# printf, not echo: printf's %s argument is printed verbatim.
def bg_wrap.echo(+line: String, +rest: String) -> String:
  bg_wrap.seg(String.append("    printf '%s\\n' \"", String.append(line, "\"")), rest)

def bg_wrap(+command: String, +bg: String, +after: U32) -> String:
  +niter = bg_poll_iters(after)
  +sent = bg_sentinel(command)
  bg_wrap.seg(String.append("BG=", bg),
  bg_wrap.seg("mkdir -p $BG",
  bg_wrap.seg("i=0",
  bg_wrap.seg("while ! mkdir $BG/$i.slot 2>/dev/null; do i=$((i+1)); done",
  bg_wrap.seg("mkfifo $BG/$i.in 2>/dev/null",
  bg_wrap.seg("exec 9<>$BG/$i.in",
  bg_wrap.seg(String.append("cat > $BG/$i.cmd <<'", String.append(sent, "'")),
  bg_wrap.seg(command,
  bg_wrap.seg(sent,
    # the command runs FROM THE .cmd FILE, never inline: a heredoc in an
  # inline command would swallow the wrapper own following lines
  # (polling, handoff, result print) as its body - the wrapper then
  # exits before printing anything and the tool returns empty.
  bg_wrap.seg(String.append("( ulimit -f 102400; ( /bin/sh $BG/$i.cmd", String.append("",
      " ) > $BG/$i.out 2>&1 <&9 & C=$!; echo $C > $BG/$i.pid; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi ) >/dev/null 2>&1 &")),
  bg_wrap.seg("while [ ! -s $BG/$i.pid ] && [ ! -e $BG/$i.rc ]; do",
  bg_wrap.seg(bg_setup_guard(bg),
  bg_wrap.seg("  sleep 0.05",
  bg_wrap.seg("done",
  bg_wrap.seg("c=$(cat $BG/$i.pid 2>/dev/null)",
  bg_wrap.seg("e=0",
  bg_wrap.seg("while kill -0 $c 2>/dev/null; do",
  bg_wrap.seg("  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi",
  bg_wrap.seg("  e=$((e+1))",
  bg_wrap.seg(String.append("  if [ $e -ge ",
    String.append(U32.show(niter), " ]; then")),
  bg_wrap.seg(String.append("    printf '%s\\n' \"",
    String.append(HANDOFF_MARKER(), "\"")),
  bg_wrap.echo(bg_headline(bg, "$i", after),
  bg_wrap.echo(bg_tail_cmd(bg, "$i"),
  bg_wrap.echo(bg_stdin_cmd(bg, "$i"),
  bg_wrap.echo(bg_status_cmd(bg, "$i"),
  bg_wrap.echo(bg_kill_cmd(bg, "$i"),
  bg_wrap.seg("    rm -f \"$0\"",
  bg_wrap.seg("    exit 0",
  bg_wrap.seg("  fi",
  bg_wrap.seg("done",
  bg_wrap.seg("while [ ! -e $BG/$i.rc ]; do",
  bg_wrap.seg(bg_rc_guard(bg),
  bg_wrap.seg("  sleep 0.05",
  bg_wrap.seg("done",
  bg_wrap.seg("s=$(cat $BG/$i.rc)",
  bg_wrap.seg("cat $BG/$i.out",
  bg_wrap.seg("rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc",
  bg_wrap.seg("rmdir $BG/$i.slot 2>/dev/null",
  bg_wrap.seg("rm -f \"$0\"",
  "exit $s")))))))))))))))))))))))))))))))))))))))
