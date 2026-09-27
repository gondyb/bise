# BR-002 — A stale interrupt flag file kills the NEXT turn at its first boundary

- Severity: medium (small trigger window, confusing blast radius)
- Component: runtime/main.bend (drain_interrupt), rust/tui (Ctrl+C handler)

## What happens

The TUI writes `/tmp/bend-interrupt-<port>.txt` on Ctrl+C while a turn is
pending. The runtime drains it ONLY at the two in-turn boundaries
(EModel: right after a model call returns; EExec: right after a tool
returns) — drain_interrupt.some is the only place the file is cleared.

Race: if the flag is written AFTER the last drain of a turn (the pure
transition hops between the final model result and the "--- idle"
emission — a sub-second window), the turn completes normally, the TUI
clears its local `pending` on idle, but the flag file stays "1" on disk.

Blast radius: the NEXT turn is killed at its FIRST boundary. Its first
model call runs (seconds of latency + tokens spent), then its reply is
discarded: obs `candidate_discarded: interrupt` + `turn_done:
interrupted`, plus a red `core rejected: no pending completion` line.
The user interrupted nothing — the previous turn's ghost did.

## Suggested fix

The runtime is the authority on turn boundaries: clear the interrupt
flag when a turn ends (e.g. in repl_end_save.then, or wherever "---
idle" is emitted — a best-effort truncate, any interrupt arriving later
belongs to no turn). Defense in depth: the TUI can also truncate the
file when it receives "--- idle" with pending=false.

The steer side-channel has the same never-cleared-at-idle shape, but a
stale steer line merely steers the next turn (arguably correct), so the
fix is only required for the interrupt flag.

## Resolution (fixed)

Both defenses from the report are in: the runtime truncates the flag
in `repl_end_save.then` (runtime/repl-live.bend) as the turn ends —
the runtime is the authority on turn boundaries; and the TUI truncates
the file when it receives "--- idle" (rust/tui). A flag written later
belongs to no turn: the TUI only writes it mid-turn (Ctrl+C or
/interrupt while a turn is pending).
