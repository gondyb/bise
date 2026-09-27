# BR-003 — /interrupt cannot interrupt the running turn (advertised as if it could)

- Severity: medium (contract / UX)
- Component: rust/tui COMMANDS list, core/commands.bend, runtime/repl-live.bend (plan_interrupt)

## What happens

The TUI advertises `/interrupt — interrompre le tour en cours`. The
actual path: the TUI sends the line on the socket (there is NO
side-channel special-case for /interrupt in handle_input — only /steer
has one). The runtime only reads the socket BETWEEN turns, so:

- sent mid-turn: the line sits in the kernel buffer until the turn
  ends, then `apply.interrupt` runs with turn=None — an accepted no-op.
  The turn the user wanted to kill runs to completion.
- sent at idle: also a no-op.

The only working mid-turn interrupt is Ctrl+C (the flag file). The
command is a trap: the user believes they interrupted; nothing happens.

## Secondary wart on the working path

After a mid-model-call interrupt lands at the EModel boundary, the
in-flight completion still applies and is rejected — by design (the
comment says "rejected cleanly") — but the TUI renders
`core rejected: no pending completion` as a red error line. It is normal
interrupt plumbing, not an error.

## Suggested fix

In the TUI's handle_input, treat `/interrupt` while `pending` exactly
like Ctrl+C (write the flag file, free the composer locally). At idle,
/`interrupt` can stay a no-op but should say so. Also consider dimming
the post-interrupt rejection line in the TUI (expected consequence,
not a failure).

## Resolution (fixed)

`/interrupt` while a turn is pending now goes through the same flag
file as Ctrl+C (shared `write_interrupt_flag` in rust/tui): the
composer frees immediately, the turn dies at its next safe boundary.
At idle it answers "aucun tour en cours à interrompre" locally instead
of sending a no-op line to the harness. The post-interrupt
"core rejected: no pending completion" line renders as a dim info
note ("réponse en vol ignorée (tour interrompu)"), not a red error.
