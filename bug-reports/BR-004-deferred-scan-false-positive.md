# BR-004 — The deferred-request scan matches the ack text in ANY non-user message

- Severity: high (false-positive compaction AND harness restart)
- Component: runtime/main.bend (def_scan / def_note / deferred_of)

## What happens

At every turn end, deferred_of scans history to find self.compact /
self.reload requests: walk forward, reset at every user message, and
for every OTHER message check `String.contains(text, MARK)` where MARK
is the ack string constant in main.bend.

The scan does not check the message ROLE beyond "not user". Assistant
prose and TOOL RESULTS are scanned too. Any of these containing the
marker fires a real compaction and/or a harness restart at the turn
boundary:

- The model QUOTING the ack in its reply ("I filed the request — the
  tool said '<mark>'") — models echo tool outputs routinely.
- The model EXPLAINING the harness to the user (documentation, audits)
  and writing the marker verbatim.
- Any bash/tool output that prints the harness source containing the
  ack constants (sed/cat/rg of main.bend) — tool results enter history
  as tool messages.

## Live demonstration (this audit, 2026-09-27)

Auditing the interrupt/compaction path required printing the source of
main.bend (COMPACT_ACK / RELOAD_ACK constants). Those tool outputs are
in this session's history, after the last user message. At the end of
THIS turn the harness will therefore run an unsolicited compaction and
then restart itself — nobody asked for either. The session survives
(checkpoint + reload), but the conversation gets compacted and the
process restarts mid-work.

## Suggested fix

The ack rides a tool result of a self.* tool; the scan should only
trust those. Options, in increasing robustness:

1. Restrict def_note to tool-role messages (removes the assistant-prose
   false positive; bash outputs quoting the source remain a risk).
2. Restrict to tool results AND match a marker that cannot appear in
   prose or source (e.g. a sentinel with a nonce per session).
3. Track the deferred request in session STATE (not by scanning
   history text) — the checkpoint already round-trips session fields;
  the text-scan is the fragile seam.

## Resolution (fixed)

Both hardening layers, per the report's options 1+2: the scan reads
only TOOL-role messages (assistant prose is never scanned; user still
resets the segment), and the ack must stand alone — either the whole
tool text ends with it (the direct self.* shape prefixes it with
"tool X ok: ") or it is a whole line of the escape_nl'd text (a
program's result carries the acks on their own lines). A tool output
quoting the harness source keeps its quotes and indentation around
the constant and matches neither shape. Pinned by
`deferred_ignores_assistant_prose`, `deferred_ignores_source_quotes`,
`deferred_ignores_midtext_marker`, plus two positive shapes
(`deferred_reads_prefixed_ack`, `deferred_reads_program_acked_result`).
Option 3 (session-state tracking) was not needed.
