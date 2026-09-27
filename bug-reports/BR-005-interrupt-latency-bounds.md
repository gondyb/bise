# BR-005 — Interrupt latency is bounded only by the HTTP retry budget (design note)

- Severity: low (design observation, answers "why a safe boundary")
- Component: runtime/main.bend (model_call / drain_steer)

## Why the interrupt cannot preempt the work in flight

The REPL is single-threaded Bend with blocking IO. The two blocking
operations of a turn are:

- model_call: one HTTPS fetch, 120s timeout, 3 attempts total with
  1.5s pauses -> up to ~6 minutes of unobservable blocking per model
  action (worst case: a hung provider).
- tools: bash hands off to background after BEND_BG_AFTER (30s default);
  run_typescript has a 10s V8 watchdog.

There is no thread, no signal, no async IO: the process physically
cannot observe the interrupt flag until the blocking call returns.
"Safe boundary" is not a policy choice — it is the first moment the
loop can look. The drain points are correct (after the call, before the
result is applied, so nothing half-applied).

## What can still improve

- The retry loop drains only after ALL attempts: a Ctrl+C during
  attempt 1 waits for attempts 2 and 3 too. model_call.step.retry could
  check the interrupt flag between attempts (a cheap file read every
  1.5s pause) and bail out early — the completion_fail path then fails
  the turn cleanly. That cuts worst-case Ctrl+C latency from ~6 min to
  ~2 min (one attempt).
- The TUI already frees the composer locally on Ctrl+C, so the UI feels
  immediate; only the runtime catches up. The status row could say how
  the catch-up is bounded (one in-flight call).

## Not bugs (checked and fine)

- Compaction core (project/fit/replace, summary validation, retry,
  protected latest user message) — law-pinned, no issue found.
- Deferred compaction runs between turns, saves after, cannot loop
  (scan computed once; next user message resets it).
- Interrupt during AUTO compaction kills the turn; the pre-compaction
  history survives and the next dispatch re-triggers compaction —
  self-healing.
- Steering drained in the same pass as the interrupt is committed by
  finish_turn (law steering_survives_interrupt) — order is correct.

## Resolution (fixed)

The improvement the report suggested: `model_call.step.retry` reads
the interrupt flag between attempts; when set, it consumes the flag
and bails out with "interrupted by the user" — the completion_fail
path fails the turn cleanly, cutting the worst-case Ctrl+C latency
from three attempts to one.
