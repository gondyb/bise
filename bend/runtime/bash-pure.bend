# runtime/bash-pure.bend - the pure half of runtime/bash.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/text.bend as Tx
import ./tools-pure.bend as Xp

# ---- the bash tool, fully in Bend (runtime/proc.bend) ----
# Proc.run runs /bin/sh on the wrapper; the answer is the exit status on
# the first line, then stdout and stderr. Commands are flattened first (the wire
# already flattens call args — double safety).

def exec_of.pick.fail(+code: U32, +output: String) -> Xp.ExecRes:
  Xp.Ex{False{}, String.append("exit ",
    String.append(U32.show(code), String.append(": ", output)))}

# the handoff marker: the wrapper prints it as the very first line of
# its output, before the contract. The exit code is no longer the
# signal (a command that really exits 199 must report 199); only the
# wrapper can print this line first — a command's output is only
# cat'd after the marker would have been printed.
def HANDOFF_MARKER() -> String:
  "BEND-BG-HANDOFF-6f2c9e5b"

def exec_of.handoff.rest(sp: Maybe<&2, Nat>, +payload: String) -> Xp.ExecRes:
  match sp:
    case None{}:
      Xp.Ex{True{}, payload}
    case Some{+p}:
      Xp.Ex{True{}, String.drop(payload, Nat.add(p, 1n))}

def exec_of.handoff(+payload: String) -> Xp.ExecRes:
  # strip the marker line; keep the contract
  exec_of.handoff.rest(Tx.find(payload, "\n", 0n), payload)

def exec_of.pick.is_handoff(is_handoff: Bool, +code: U32,
    +output: String) -> Xp.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      exec_of.pick.fail(code, output)

# exit 0 without the marker is a plain success
def exec_of.pick.zero.ok(is_handoff: Bool, +output: String) -> Xp.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      Xp.Ex{True{}, output}

def exec_of.pick.zero(is_zero: Bool, +code: U32, +output: String) -> Xp.ExecRes:
  match is_zero:
    case True{}:
      exec_of.pick.zero.ok(
        String.starts_with(output, HANDOFF_MARKER()), output)
    case False{}:
      exec_of.pick.is_handoff(
        String.starts_with(output, HANDOFF_MARKER()), code, output)

def exec_of.pick(+code: U32, +output: String) -> Xp.ExecRes:
  exec_of.pick.zero(U32.is_eq(code, 0), code, output)

def exec_of.code.read(m: Maybe<&2, U32>, +output: String) -> Xp.ExecRes:
  match m:
    case Some{+c}:
      exec_of.pick(c, output)
    case None{}:
      exec_of.pick(1, output)

def exec_of.code(code_str: String, +output: String) -> Xp.ExecRes:
  exec_of.code.read(U32.read(code_str), output)

def exec_of.split(sp: Maybe<&2, Nat>, +out: String) -> Xp.ExecRes:
  match sp:
    case Some{+p}:
      exec_of.code(String.take(out, p), String.drop(out, Nat.add(p, 1n)))
    case None{}:
      exec_of.code(out, "")

def exec_of(+out: String) -> Xp.ExecRes:
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
#     cannot wait: 127); it records the pid, waits, writes the exit
#     code to <id>.rc and exits with it — the wrapper polls the RUNNER
#     (its own child, $!) and takes the code from `wait`, never from a
#     file: a file the runner could not write (full disk, read-only
#     dir) can never hang the wrapper
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

# the session's bg dir, pure: <root>/bend-bg-<port>. One dir per REPL
# process (keyed by port) so parallel terminals never share slots,
# and a /reload respawns on the same port and finds its slots again.
def bg_dir_of(+root: String, +port: String) -> String:
  String.append(root, String.append("/bend-bg-", port))

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

# the vanished-dir guard: if the agent deletes the bg dir under a
# running wrapper (a legitimate cleanup), the tool fails with a clear
# message instead of cat'ing files that are gone
def bg_rc_guard(+bg: String) -> String:
  String.append("if [ ! -d ", String.append(bg,
    String.append(" ]; then echo \"background: the bg dir vanished before the exit code was recorded: ",
      String.append(bg, "\"; rm -f \"$0\"; exit 1; fi"))))

# slot allocation is bounded: mkdir failing on an EXISTING slot moves to
# the next id; failing for any other reason (full disk, read-only or
# uncreatable bg dir) 20 times ends the call with this message (once:
# the loop retried forever)
def bg_slot_fail(+bg: String) -> String:
  String.append("  if [ $f -ge 20 ]; then echo \"background: cannot create a slot in ",
    String.append(bg, " (disk full or not writable?)\"; rm -f \"$0\"; exit 1; fi"))

# the .cmd write can fail too (full disk): never run a truncated command
def bg_cmd_fail(+bg: String) -> String:
  String.append("if [ $? -ne 0 ]; then echo \"background: could not write ",
    String.append(bg_slot(bg, "$i", ".cmd"),
      " (disk full?)\"; rm -f $BG/$i.in $BG/$i.cmd; rmdir $BG/$i.slot 2>/dev/null; rm -f \"$0\"; exit 1; fi"))

# a slot file the runner could not write: the call still returns, and
# says why a file is missing (the output may be cut short)
def bg_write_note(+test: String, +bg: String, +ext: String, +why: String)
    -> String:
  String.append("if [ ", String.append(test, String.append(" ",
    String.append(bg_slot(bg, "$i", ext),
      String.append(" ]; then echo \"background: could not write ",
        String.append(bg_slot(bg, "$i", ext),
          String.append(why, "\"; fi")))))))

# 5 iterations at 20ms then 100ms steps: a trivial command costs
# ~30ms of polling instead of ~120ms, and the window stays ~after secs
def bg_poll_iters(+after: U32) -> U32:
  U32.add(U32.mul(after, 10), 5)

# Every wait is TOTAL: the wrapper polls its own child, the runner
# ($!), and reads the exit code from `wait`, so it never waits for a
# file. (Once: on a full disk the runner could not write the .rc, the
# wrapper waited for it forever, and a bash call hung 19 minutes.) The
# .pid is still written for the agent's status/kill lines.
# The wrapper goes to a file (the node_run pattern) and sh runs the
# file: the command never travels as an argument.
#
# The file is named by a hash of its OWN content: two different
# commands never share a wrapper file, so an orphaned wrapper (a
# process that died mid-command, still sleeping in its poll loop)
# can never re-read ANOTHER command's bytes when it wakes up. Re-running
# the same command rewrites identical bytes, which is safe: the orphan
# just continues its own script.
# FNV-1a over the code points, in U32: the arithmetic wraps mod 2^32
# by construction, so no script length can overflow. (The former
# positional sum c * (pos^2 + 1) was a Nat growing as length^3: it hit
# the runtime's 2^48-1 ceiling - "bend: a Nat past the largest
# immediate" - on a ~20k-char ASCII script, far sooner with accented
# text, and every long heredoc the model wrote killed the REPL.) The
# hash stays a U32 end to end: a large Nat is also slow to normalize in
# the laws. Fuel-first (the codebase idiom).
def bg_script_hash.go(fuel: Nat, +s: String, +h: U32) -> U32:
  match fuel:
    case 0n:
      h
    case 1n+k:
      match s:
        case SNil{}:
          h
        case SCon{Chr{+c}, t}:
          bg_script_hash.go(k, t, U32.mul(U32.xor(h, c), 16777619))

def bg_script_hash(+s: String) -> U32:
  bg_script_hash.go(Nat.add(String.length(s), 1n), s, 2166136261)

# the .cmd heredoc sentinel derives from the command's own hash: a
# command containing the sentinel would have to contain its own hash
def bg_sentinel(+command: String) -> String:
  String.append("XQ-", String.append(U32.show(bg_script_hash(command)),
    "-EOF"))

def bash_script_path(+port: String, +script: String) -> String:
  String.append("/tmp/bend-sh-", String.append(port,
    String.append("-", String.append(U32.show(bg_script_hash(script)), ".sh"))))

# ---- BEND_WORKDIR: where the command runs ----
# A switchboard agent runs its REPL from the app root (the runtime's
# relative paths live there) but works in its own directory: the
# workspace, or a task's git worktree. The command then starts with a
# cd into it; a missing directory fails the command instead of running
# it somewhere else. Empty: the REPL's own directory, unchanged.
def with_workdir.pick(empty: Bool, +dir: String, +command: String)
    -> String:
  match empty:
    case True{}:
      command
    case False{}:
      String.append("cd ", String.append(Tx.sh_quote(dir),
        String.append(" || exit 1\n", command)))

def with_workdir(+dir: String, +command: String) -> String:
  with_workdir.pick(String.is_empty(dir), dir, command)

# ---- the REPL's private environment stays in the REPL ----
# The hub (and the harness) hand the REPL its session file, wire log,
# context file, port...: they belong to THIS process. A command that
# inherits them and starts another REPL (a scripted test run from an
# agent's shell) makes that REPL write its turns into the live agent's
# wire log and session (once: the scripted plan's "Program complete."
# and "scripted provider failure" landed in a live agent's transcript).
# The command runs without them; SB_SOCKET/SB_AGENT stay (the sb CLI
# needs them), and a harness drops those itself for its child.
def bg_unset_private() -> String:
  "unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT BEND_EXTRA_PROMPT BEND_CONTINUE BEND_CRASH_NOTE BEND_DEBUG_DIR"

# ---- slot allocation never recycles a slot that still holds results ----
# A background command that ends frees its .slot but KEEPS its .out and
# .rc for the agent to read. A slot is free only once those are gone:
# recycling it earlier truncated the job's output (the next command's
# own > <id>.out), and a next command that cat'd <id>.out after printing
# anything read its own output file back into itself until ulimit -f
# stopped it: a 100 MiB tool result that killed the REPL ("bend: memory
# fault", three agents in one hour, each cat'ing the .out of a finished
# background cp).
def bg_slot_loop() -> String:
  "while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ! mkdir $BG/$i.slot 2>/dev/null; do"

def bg_slot_skip() -> String:
  "  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi"

# ---- what the REPL reads back is bounded ----
# The REPL turns the whole output into a String before it caps what the
# model sees (Xp.cap_out, 20000 chars); a 100 MiB output crashes it
# (measured: 75 MB passes, 100 MiB is "bend: memory fault"). The
# wrapper hands back at most bg_out_max() bytes, after a line that says
# how big the output was.
def bg_out_max() -> String:
  "1048576"

def bg_cat_out(+bg: String) -> String:
  String.append("n=$(wc -c < $BG/$i.out 2>/dev/null); n=$((${n:-0}+0)); if [ $n -gt ",
    String.append(bg_out_max(),
      String.append(" ]; then echo \"[bash: the output is $n bytes; only its first ",
        String.append(bg_out_max(),
          String.append(" follow. The rest is lost: redirect a big output to a file and read it in parts]\"; head -c ",
            String.append(bg_out_max(), " $BG/$i.out; echo; else cat $BG/$i.out 2>/dev/null; fi"))))))

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
  bg_wrap.seg(bg_unset_private(),
  bg_wrap.seg("mkdir -p $BG",
  bg_wrap.seg("i=0",
  bg_wrap.seg("f=0",
  bg_wrap.seg(bg_slot_loop(),
  bg_wrap.seg(bg_slot_skip(),
  bg_wrap.seg(bg_slot_fail(bg),
  bg_wrap.seg("done",
  bg_wrap.seg("mkfifo $BG/$i.in 2>/dev/null",
  bg_wrap.seg("exec 9<>$BG/$i.in",
  bg_wrap.seg(String.append("cat > $BG/$i.cmd <<'", String.append(sent, "'")),
  bg_wrap.seg(command,
  bg_wrap.seg(sent,
  bg_wrap.seg(bg_cmd_fail(bg),
    # the command runs FROM THE .cmd FILE, never inline: a heredoc in an
  # inline command would swallow the wrapper own following lines
  # (polling, handoff, result print) as its body - the wrapper then
  # exits before printing anything and the tool returns empty.
  bg_wrap.seg(String.append("( ulimit -f 102400; ( /bin/sh $BG/$i.cmd", String.append("",
      " ) > $BG/$i.out 2>&1 <&9 & C=$!; echo $C > $BG/$i.pid; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi; exit $R ) >/dev/null 2>&1 &")),
  bg_wrap.seg("r=$!",
  bg_wrap.seg("e=0",
  bg_wrap.seg("while kill -0 $r 2>/dev/null; do",
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
  bg_wrap.seg(String.append("    ", bg_write_note("! -s", bg, ".pid",
    " (disk full?) - the command runs under the runner pid $r")),
  bg_wrap.seg("    rm -f \"$0\"",
  bg_wrap.seg("    exit 0",
  bg_wrap.seg("  fi",
  bg_wrap.seg("done",
  bg_wrap.seg("wait $r",
  bg_wrap.seg("s=$?",
  bg_wrap.seg(bg_rc_guard(bg),
  bg_wrap.seg(bg_write_note("! -f", bg, ".out", " (disk full?)"),
  bg_wrap.seg(bg_cat_out(bg),
  bg_wrap.seg(bg_write_note("! -s", bg, ".rc",
    " (disk full?) - the output above may be incomplete"),
  bg_wrap.seg("rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc 2>/dev/null",
  bg_wrap.seg("rmdir $BG/$i.slot 2>/dev/null",
  bg_wrap.seg("rm -f \"$0\"",
  "exit $s"))))))))))))))))))))))))))))))))))))))))))
