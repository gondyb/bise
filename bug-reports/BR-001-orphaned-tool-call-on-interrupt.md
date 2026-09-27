# BR-001 — Interrupt during a tool execution orphans the tool call in history

- Severity: medium
- Component: core/session.bend (apply.interrupt / finish_turn), runtime/main.bend (drain_interrupt at the EExec boundary)
- Found: 2026-09-27, audit of interrupt + compaction

## What happens

When Ctrl+C (the interrupt flag file) is drained at the EExec boundary —
i.e. the interrupt lands while a tool is running — the turn is killed
AFTER the tool call was committed to history but BEFORE the tool result
is applied:

1. The assistant completion carrying the calls was accepted earlier:
   history holds `asst(text, calls)`, pending is `PTools{waiting}`.
2. The tool runs (exec_tool); exec_done drains the side channels.
3. drain_interrupt applies `T.Interrupt` -> `apply.interrupt` ->
   `finish_turn`: turn=None, pending=None, history unchanged.
4. The tool result then applies -> `apply.tool` with pending=None ->
   `T.Rejected{"no pending tool result"}`.

Result: durable history contains an assistant message with `tool_calls`
that no tool message ever answers. Same shape when the interrupt lands
during a run_typescript sub-call (pending `PProg`).

## Impact

- The next provider request carries the unanswered call. Tested live
  against api.mistral.ai (model zai-glm-5-3): the request is ACCEPTED
  (HTTP 200), so the turn does NOT fail — severity is not high.
- But the model reads its own tool call with no result: it can assume
  the tool succeeded, re-issue it, or get confused about state. The
  orphan survives until compaction drops it (drop_one works from the
  oldest side; the orphan is recent, so it lingers).
- The TUI shows the tool as completed (the result preview was emitted
  before the drain) while the runtime discarded it from history — the
  display and the history disagree.

## Suggested fix

`apply.interrupt` knows the pending state. When pending is
`PTools{waiting}` (or `PProg`), synthesize one tool result message per
waiting call ("interrupted", ok=false) before `finish_turn`, so the
wire contract (every call answered) survives the interrupt. The
pinned laws (`idle_interrupt_noop`, `steering_survives_interrupt`) use
PAgent/None pendings and are unaffected; a new law should pin the
PTools case.

## Resolution (fixed)

`apply.interrupt` now synthesizes one failed tool result ("interrupted",
ok=false) per waiting call before `finish_turn` — `interrupt_closes` in
core/session.bend. The answers ride the history BEFORE the steering is
committed, so each answer stays adjacent to its assistant call. PProg
is answered as one "tool run_typescript failed: interrupted"; PAgent
and PComp answer nothing (nothing model-visible is held). Pinned
byte-exact by `interrupt_closes_waiting_calls` (the full resulting
history, adjacency included), `interrupt_closes_prog_pending`,
`interrupt_closes_agent_pending`. apply.fail was left as-is: FailTurn
never fires while tools are pending on the live paths.
