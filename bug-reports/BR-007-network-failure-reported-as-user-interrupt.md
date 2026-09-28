# BR-007 — A network failure is reported as "interrupted by the user"

- Severity: high (the real cause is hidden; every message fails the same way)
- Component: runtime/provider.bend (retry loop), runtime/main.bend (wire
  log), switchboard hub (failed turns of tasks)

## What the user saw

Tailscale was off, so the Anthropic proxy was unreachable. Every message
to main failed about 1 s later with:

    turn failed: interrupted by the user

The user interrupted nothing. The live main transcript shows it five
times in a row (`turn_started`, then `turn_done: failed: interrupted by
the user` ~1 s later, no retry line).

## Reproduction (throwaway REPL, no VPN touched)

`BEND_PROVIDER_URL=http://127.0.0.1:1/v1/messages`, two messages
through `bend_client.py`:

    === turn 1 151.4s                      (flag file does not exist yet)
       obs: provider_retry: 1/10 · connect 61 Connection refused · retry in 1s
       ... 9 retries ...
       obs: turn_done: failed: provider failed after 10 attempts: connect 61 Connection refused
    === turn 2 0.0s                        (flag file exists, EMPTY)
       obs: provider_retry: 1/10 · connect 61 Connection refused · retry in 1s
       obs: turn_done: failed: interrupted by the user

## Root cause

1. `model_call.step.retry.flag` called `.interrupted(String.is_empty(c), …)`:
   the test was inverted. The interrupt flag file is cleared by writing
   `""` (every turn end does it, BR-002), so from the second turn on, an
   existing EMPTY file counted as an interrupt. Any retryable failure
   (connect, DNS, timeout) stopped after one attempt, and the text
   replaced the real error. A steer or a new message played no part:
   only Ctrl+C / `/interrupt` / `sb interrupt` write the flag.
2. The retry notes (`provider_retry`) went to the socket only. The
   Switchboard hub reads the REPL's output from the wire log
   (`BEND_WIRE_LOG`), so in Switchboard no retry was ever visible.
3. The hub ignored `turn_done: failed`: a task whose turn failed went
   quiet; main and the user were not told.

## Fix

- `Pvp.interrupt_requested`: only a flag WITH content is an interrupt
  (law `interrupt_flag_needs_content`).
- Classified causes with the host and a hint (`Pvp.net_cause`,
  `Pvp.status_hint`): `cannot reach <host> (connect 61 Connection
  refused) — check your network or VPN`, `cannot resolve <host> (dns) —
  …`, `no answer from <host> after 120 s (timeout)`, `provider 401: … —
  check the API key`. A final HTTP error fails the turn at once with its
  status and hint.
- An interrupt during the retries keeps the last error (law
  `interrupted_keeps_last_error`): `stopped retrying (interrupted by the
  user) after 5 failed attempts; last error: …`. The pause before a
  retry is slept in 1 s slices, so Ctrl+C acts within ~1 s.
- Retries exhausted: `provider failed after 10 attempts: <cause>` (law
  `exhausted_keeps_last_error`).
- The provider's notes are appended to the wire log too
  (`Pv.wire_log`, shared with `emit`).
- Hub: a task's `turn_done: failed: …` becomes a `turn_failed` report to
  its parent (board + message). Main's own failures show in main's view.
  A stop the user asked for is not reported.
- TUI: `model call failed (attempt 2/10): <cause> · retry 3/10 in 4s`;
  the last French texts ("tour interrompu") are now English.

## What the user sees now (throwaway dev hub, provider at 127.0.0.1:1)

    ▲ model call failed (attempt 1/10): cannot reach 127.0.0.1:1 (connect 61 Connection refused) — check your network or VPN · retry 2/10 in 1s
    ▲ model call failed (attempt 2/10): … · retry 3/10 in 2s
    ✗ turn failed: provider failed after 10 attempts: cannot reach 127.0.0.1:1 (connect 61 Connection refused) — check your network or VPN

With Ctrl+C during the retries:

    ✗ turn failed: stopped retrying (interrupted by the user) after 5 failed attempts; last error: cannot reach 127.0.0.1:1 (connect 61 Connection refused) — check your network or VPN

A task whose turn fails reports it to main (journal + main's feed):

    sb msg-in : vpn-probe-say m_5 : [report: turn_failed] my turn failed: provider failed after 10 attempts: cannot reach 127.0.0.1:1 (connect 61 Connection refused) — check your network or VPN — a new message retries it

## Found on the way

`versions.sh build --tree` could ship stale Rust code: the release build
shared its cargo target with the commit builds in /tmp worktrees, and a
newer artifact made the tree's edited files look "fresh". Fixed by
live-versions in 4349775 (one target per source).

## Commits

d560d03 (provider code + laws, swept into a concurrent commit), 64a829e
(proofs), fbde735 (wire log), f4d5772 (hub report), b00a793 (TUI texts).
