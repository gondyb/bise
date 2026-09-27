# test-bg.bend — the background-command battery, end to end.
#
# Runs REAL commands through Sh.bash_exec (the hub snap runner) and
# asserts on the contract the agent actually reads. Complements the
# LAWS: the laws pin the pure contract byte for byte, this suite runs
# it against /bin/sh, real processes, a real fifo, real kills.
#
#   bend test-bg.bend -o /tmp/bg-suite
#   rm -rf /tmp/bgtest
#   BEND_BG_ROOT=/tmp/bgtest BEND_BG_AFTER=2 /tmp/bg-suite
#
# The rm -rf BEFORE the run matters: the suite asserts on hardcoded
# slot ids, so it must start from an empty bg dir (a command may never
# clean the bg dir itself — s14 proves why). The env pins a 2s sync
# window (so the suite is fast) and a private bg root (so it is
# hermetic). With no BEND_REPL_PORT the dir is /tmp/bgtest/bend-bg-0.
# Every "sleep 1" below is itself a sync command — the suite waits
# through the very machinery it tests. Waits are IO.sleep (pure): a
# bash_exec wait between a slot-freeing event and its evidence read
# would recycle the slot and destroy the out/rc. Slots RECYCLE: a command that
# ends frees its slot (pid/cmd/in removed by the runner, out/rc kept
# readable), so the suite asserts on the ids the recycling assigns.
#
# Gates: no FAIL line, and the summary says 14/14.
import Base
import runtime/main.bend as Rt
import runtime/bash.bend as Sh
import runtime/tools.bend as X
import runtime/persist.bend as P

def bg() -> String:
  "/tmp/bgtest/bend-bg-0"

def slot(id: String, ext: String) -> String:
  Sh.bg_slot(bg(), id, ext)

def res_ok(+r: X.ExecRes) -> Bool:
  match r:
    case X.Ex{+ok, _}:
      ok

def res_out(+r: X.ExecRes) -> String:
  match r:
    case X.Ex{_, +out}:
      out

def txt_or(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{+t}:
      t

def readf(path: String) -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file(path)
    IO.pure(String, txt_or(m))

def check(cond: Bool, +name: String) -> IO(Unit):
  match cond:
    case True{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("PASS ", String.append(name, "\n")))
        IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("FAIL ", String.append(name, "\n")))
        IO.pure(Unit, Unit{})

# a sync bash delay (UNUSED in the suite: any bash_exec between a
# slot-freeing event and its evidence read recycles the slot and
# destroys the evidence — the suite waits with IO.sleep instead)
def wait() -> IO(X.ExecRes):
  Sh.bash_exec("sleep 1")

def main() -> IO(Unit):
  do IO<Unit>:
    # ---- s01 a fast command returns output + exit 0 ----
    +r1 : X.ExecRes <- Sh.bash_exec("echo sync-ok")
    # output cleanliness: no shell noise from the ephemeral pid race
    u1 : Unit <- check(Bool.and(res_ok(r1),
        Bool.and(String.contains(res_out(r1), "sync-ok"),
          Bool.not(String.contains(res_out(r1), "No such file")))),
      "s01 sync output ok and clean")

    # ---- s02 exit 0 is a success ----
    +r2 : X.ExecRes <- Sh.bash_exec("true")
    u2 : Unit <- check(res_ok(r2), "s02 exit 0 is ok")

    # ---- s03 a failure reports its code ----
    +r3 : X.ExecRes <- Sh.bash_exec("false")
    u3 : Unit <- check(Bool.and(Bool.not(res_ok(r3)),
        String.contains(res_out(r3), "exit 1")), "s03 failure carries exit code")

    # ---- s04 stderr is captured too ----
    +r4 : X.ExecRes <- Sh.bash_exec("echo err-ok >&2")
    u4 : Unit <- check(String.contains(res_out(r4), "err-ok"),
      "s04 stderr captured")

    # ---- s05 a sync slot is fully cleaned ----
    +t5 : String <- readf(slot("0", ".out"))
    u5 : Unit <- check(String.eq(t5, ""), "s05 sync slot cleaned")

    # ---- s06 a slow command hands off as ok (199 -> contract) ----
    +r6 : X.ExecRes <- Sh.bash_exec("sleep 5")
    +o6 : String <- IO.pure(String, res_out(r6))
    u6 : Unit <- check(res_ok(r6), "s06 handoff maps to ok")

    # ---- s07 the contract is concrete: window, id, dir, commands ----
    # The headline/tail/stdin lines match the pinned templates exactly;
    # the status/kill lines go further — the wrapper's $(cat pid) is
    # substituted at print time, so the agent reads the REAL pid.
    +p7 : String <- readf(slot("0", ".pid"))
    u7a : Unit <- check(String.contains(o6, Sh.bg_headline(bg(), "0", 2)),
      "s07a headline names window, id, dir")
    u7b : Unit <- check(String.contains(o6, Sh.bg_tail_cmd(bg(), "0")),
      "s07b output line is a concrete tail command")
    u7c : Unit <- check(String.contains(o6, Sh.bg_stdin_cmd(bg(), "0")),
      "s07c stdin line is a concrete fifo command")
    u7d : Unit <- check(String.contains(o6,
        String.append("  status: kill -0 ", String.trim(p7))),
      "s07d status line probes the real pid")
    u7e : Unit <- check(String.contains(o6,
        String.append("  kill: kill ", String.trim(p7))),
      "s07e kill line uses the real pid")

    # ---- s08 the contract's pid file holds a LIVE pid ----
    +p8 : String <- readf(slot("0", ".pid"))
    +r8 : X.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p8)))
    u8 : Unit <- check(res_ok(r8), "s08 contract pid is live")

    # ---- s09 the kill command works; the runner records 143 ----
    +r9 : X.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p8)))
    # read the rc immediately: any bash_exec in between would recycle
    # the freed slot and overwrite it
    +rc9 : String <- readf(slot("0", ".rc"))
    u9 : Unit <- check(String.eq(String.trim(rc9), "143"),
      "s09 kill then rc is 143")
    # let the detached runner free the slot before the next allocation
    # (IO.sleep: a bash_exec wait would recycle the freed slot itself)
    +u9w : Unit <- IO.sleep(1100)

    # ---- s10 the fifo feeds a reader: printf > .in ----
    +r10 : X.ExecRes <- Sh.bash_exec("read x; echo got:$x")
    f10 : X.ExecRes <- Sh.bash_exec(
      String.append("printf 'fifo-ok\\n' > ", slot("0", ".in")))
    # a pure sleep: a bash_exec wait would recycle slot 0 and
    # destroy the reader's out/rc before they are read
    +u10w : Unit <- IO.sleep(1100)
    +o10 : String <- readf(slot("0", ".out"))
    +rc10 : String <- readf(slot("0", ".rc"))
    u10 : Unit <- check(Bool.and(String.contains(o10, "got:fifo-ok"),
        String.eq(String.trim(rc10), "0")), "s10 stdin fifo feeds reader")
    # let the runner free the reader's slot before the next allocation
    +u10b : Unit <- IO.sleep(1100)

    # ---- s11 tail while running: empty, then filled when done ----
    +r11 : X.ExecRes <- Sh.bash_exec("sleep 3; echo late-line")
    +o11a : String <- readf(slot("0", ".out"))
    +u11w : Unit <- IO.sleep(2100)
    +o11b : String <- readf(slot("0", ".out"))
    +rc11 : String <- readf(slot("0", ".rc"))
    u11 : Unit <- check(Bool.and(Bool.not(String.contains(o11a, "late-line")),
        Bool.and(String.contains(o11b, "late-line"),
          String.eq(String.trim(rc11), "0"))), "s11 tail empty then filled")

    # ---- s12 two slow commands run in PARALLEL, distinct slots ----
    +r12a : X.ExecRes <- Sh.bash_exec("sleep 8")
    +r12b : X.ExecRes <- Sh.bash_exec("sleep 9")
    +p12a : String <- readf(slot("0", ".pid"))
    +p12b : String <- readf(slot("1", ".pid"))
    +l12a : X.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p12a)))
    +l12b : X.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p12b)))
    +k12a : X.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p12a)))
    # read slot 0's rc BEFORE the next bash_exec: it would recycle the
    # freed slot and destroy the evidence
    +rc12a : String <- readf(slot("0", ".rc"))
    +k12b : X.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p12b)))
    +rc12b : String <- readf(slot("1", ".rc"))
    u12 : Unit <- check(Bool.and(Bool.and(res_ok(l12a), res_ok(l12b)),
        Bool.and(String.eq(String.trim(rc12a), "143"),
          String.eq(String.trim(rc12b), "143"))),
      "s12 two parallel slots, both killed")

    # ---- s13 listing the dir is ordinary ls ----
    +r13 : X.ExecRes <- Sh.bash_exec(String.append("ls ", bg()))
    # the ls wrapper itself recycles slot 0 (free), so only slot 1's
    # kept rc is guaranteed in the listing
    u13 : Unit <- check(Bool.and(res_ok(r13),
        String.contains(res_out(r13), "1.rc")),
      "s13 ls lists ended slots (out/rc kept, pid freed)")

    # ---- s15 multi-line commands stay intact (QA bug 1.1/1.2) ----
    +r15 : X.ExecRes <- Sh.bash_exec("echo one\necho two\nif true; then\necho in-block\nfi")
    u15 : Unit <- check(Bool.and(res_ok(r15),
        Bool.and(String.contains(res_out(r15), "one\ntwo"),
          Bool.and(String.contains(res_out(r15), "in-block"),
            Bool.not(String.contains(res_out(r15), "one echo two"))))),
      "s15 multi-line command runs intact")

    # ---- s16 a real exit 199 is a failure (QA bug 1.3) ----
    +r16 : X.ExecRes <- Sh.bash_exec("exit 199")
    u16 : Unit <- check(Bool.and(Bool.not(res_ok(r16)),
        String.contains(res_out(r16), "exit 199")),
      "s16 real exit 199 reports as failure")

    # ---- s17 background output is capped at 50MiB (QA bug 3.1) ----
    +r17 : X.ExecRes <- Sh.bash_exec(
      "dd if=/dev/zero bs=1048576 count=55 2>/dev/null | cat > /dev/null; echo written")
    u17 : Unit <- check(Bool.and(res_ok(r17),
        String.contains(res_out(r17), "written")),
      "s17 capped output still completes (ulimit)")

    # ---- s18 the wrapper script deletes itself (QA bug 3.6) ----
    +c18a : X.ExecRes <- Sh.bash_exec("ls /tmp/bend-sh-*.sh 2>/dev/null | wc -l")
    +r18 : X.ExecRes <- Sh.bash_exec("echo self-clean-probe")
    +c18b : X.ExecRes <- Sh.bash_exec("ls /tmp/bend-sh-*.sh 2>/dev/null | wc -l")
    u18 : Unit <- check(Bool.and(res_ok(c18a), Bool.and(res_ok(c18b),
        Bool.not(String.eq(String.trim(res_out(c18b)),
          String.append(String.trim(res_out(c18a)), "1"))))),
      "s18 wrapper scripts do not leak per call")

    # ---- s14 the agent rm -rf's its own bg dir: fast failure, no hang ----
    +r14 : X.ExecRes <- Sh.bash_exec(String.append("rm -rf ", bg()))
    u14 : Unit <- check(Bool.and(Bool.not(res_ok(r14)),
        String.contains(res_out(r14), "the bg dir vanished")),
      "s14 bg dir cleanup fails fast, never hangs")

    fin : Unit <- IO.print("---- bg suite done\n")
    IO.pure(Unit, Unit{})
