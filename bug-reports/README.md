# Bug reports

One file per bug: what the user saw, the reproduction, the root cause, the
fix and its commits. Next number: BR-009.

| id | bug | status |
|---|---|---|
| [BR-001](BR-001-orphaned-tool-call-on-interrupt.md) | an interrupt during a tool execution orphans the tool call in history | fixed |
| [BR-002](BR-002-stale-interrupt-flag-kills-next-turn.md) | a stale interrupt flag file kills the next turn | fixed |
| [BR-003](BR-003-interrupt-command-cannot-interrupt.md) | `/interrupt` cannot interrupt the running turn | fixed |
| [BR-004](BR-004-deferred-scan-false-positive.md) | the deferred-request scan matches the ack text in any non-user message | fixed |
| [BR-005](BR-005-interrupt-latency-bounds.md) | interrupt latency bounded only by the HTTP retry budget (design note) | fixed |
| [BR-006](BR-006-bash-hash-nat-overflow-crashes-repl.md) | a ~20k-char bash command crashes the REPL (Nat past 2^48-1) | fixed |
| [BR-007](BR-007-network-failure-reported-as-user-interrupt.md) | a network failure is reported as "interrupted by the user" | fixed |
| [BR-008](BR-008-apply-patch-relative-path-lands-in-version-dir.md) | apply_patch wrote relative paths in the version dir (files "vanish") | fixed (4355f84) |
