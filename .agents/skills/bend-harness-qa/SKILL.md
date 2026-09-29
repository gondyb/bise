---
name: bend-harness-qa
description: Drive a live bend-harness session programmatically with bend_client.py to test features end to end, reproduce and investigate bugs, and write precise bug reports. Use when asked to test, verify, QA, benchmark, or investigate the Bend harness.
---

# Bend Harness QA

You control a REAL live harness session (GLM model + bash + MCP tools + skills) from Python: the same wire protocol the TUI speaks, but scripted and reproducible. This is how you test a new feature end to end, reproduce a user-reported bug, or investigate weird behavior.

Repo: `/Users/gabrielvergnaud/lab/bend-lab/harness` (all relative paths below are inside it).

## Golden rules

1. ALWAYS run with `python3 -u` (unbuffered stdout) — without it a long turn looks dead.
2. One TCP connection per session: the REPL serves one client at a time. Sequential `BendSession.fresh()` calls are fine (each picks a private port); always `s.close()` when done (the REPL dies with it, the session checkpoint stays).
3. A turn ends at the `--- idle` line. `say()` blocks until then (default timeout 900s — pass a shorter one for smoke tests).
4. Bash tool calls slower than `BEND_BG_AFTER` seconds (default 30) hand off to a background slot instead of blocking. Use `fresh(bg_after=N)` to shrink the window in tests.

## Minimal run

```python
import sys; sys.path.insert(0, "/Users/gabrielvergnaud/lab/bend-lab/harness")
from bend_client import BendSession

s = BendSession.fresh()          # or BendSession.resume(path)
try:
    lines = s.say("reply with exactly OK", timeout=120)
    assert "OK" in s.last_assistant(lines)
    print("PASS")
finally:
    s.close()
```

Run it: `python3 -u /tmp/mytest.py`. The full turn transcript is in `lines` (print it when debugging); the client log is `/tmp/bend-client-<port>.log`.

For anything beyond the basics, read `reference/client-api.md` (full client API + the wire events) and copy `scripts/e2e_template.py` as a starting point.

## E2E recipes

- **Feature smoke test**: one turn, instruct precisely, then assert on the obs lines (tool started/finished, tool_result) AND `last_assistant()`. Print PASS/FAIL, never eyeball.
- **Mid-turn steering**: run `say()` in a thread, wait for the tool call to appear in the log, then `s.steer_midturn(text)` — it writes the file side-channel `/tmp/bend-steer-<port>.txt`. Assert: an obs `steering_received:` mid-turn, an obs `steered:` (injected into the next model input), and the final answer honoring the steered text. (`s.steer()` sends on the SOCKET — it is only read at idle and starts a NEW turn; that is NOT mid-turn steering.)
- **Background commands**: `BendSession.fresh(bg_after=2)`, ask the agent to run `sleep 5 && echo done`. Assert the tool result carries the handoff contract (slot id, tail/stdin/status/kill commands) and that a second bash call can read the slot's `.out`/`.rc` under `/tmp/bend-bg-<port>/`.
- **Skills**: ask the model to call the `skill` tool with exact args `{"name":"<exact>"}`. Assert the result starts with the SKILL.md CONTENT (frontmatter stripped), and that a bogus name returns "Skill ... is not available" plus the available list.
- **Sessions / CLI flags** (not client-driven): `./run.sh --headless [--scripted]` (what bend_client.py runs: one READY line, lives until stdin closes; there is no single-agent TUI any more, BISE-113). Sessions live in `~/.bend-harness/sessions/<id>.txt`; `--continue` = latest mtime, `--resume <id-or-unique-prefix>`, errors list the available ids.

## Before you report a bug

Separate a harness bug from an environment/model flake — run the deterministic gates from the repo root:

```sh
~/.bend/bin/bend PROOF.bend          # laws (NEVER `bend update`: pin 2.0.27)
bend runtime/demo.bend -o harness-demo && ./harness-demo > /tmp/demo-a.txt && ./harness-demo > /tmp/demo-b.txt && diff /tmp/demo-a.txt /tmp/demo-b.txt   # deterministic
~/.bend/bin/bend test-bg.bend -o /tmp/bg-suite && rm -rf /tmp/bgtest && BEND_BG_ROOT=/tmp/bgtest BEND_BG_AFTER=2 /tmp/bg-suite
```

A live-model failure with all gates green is a live-integration bug or a model flake — re-run the e2e once before reporting.

## Bug report format

Write reports as markdown (scratchpad or the file the user asked for), one report per bug:

1. **Title** — one line, the symptom.
2. **Environment** — commit hash (`git log --oneline -1`), mode (live/scripted), env vars (BEND_BG_AFTER, port).
3. **Repro** — the SMALLEST python script that triggers it (must be runnable with `python3 -u`), or the exact pipe command.
4. **Expected / Observed** — two short lines, not prose.
5. **Transcript excerpt** — the relevant obs lines only, not the whole turn.
6. **Hypothesis** — the suspected module (`runtime/main.bend`, `runtime/bash.bend`, `runtime/provider.bend`, `core/session.bend`, the TUI...) and what the gates say.
7. **Severity** — blocks-turn / degrades / cosmetic.

## Investigation pointers

- Session checkpoints: `/tmp/bend-sessions/session-<port>.txt` (client-spawned) or `~/.bend-harness/sessions/` (binary-spawned). Format: the checkpoint text protocol (core/checkpoint.bend).
- The harness can test ITSELF: via `say()`, ask the live agent to exercise its own tools (bash, background slots, skills, programmatic calling) and return a structured report — it knows its own contract. That is the QA-round pattern; parse its final answer for findings.
- Tool routing: `runtime/main.bend` (`exec_pick` top-level, `exec_program.call` from the sandbox). Pure contracts are pinned in `LAWS.bend`, closed by `PROOF.bend`.
- MCP index `~/.bend-harness/mcp-index.txt`, skills index `~/.bend-harness/skills-index.txt`.

## Gotchas

- No `timeout` command on macOS (GNU coreutils absent) — use the client timeout args.
- Assistant text escapes newlines as literal `\n` on the wire — read it via `last_assistant()`/`unescape()`.
- The client log shows `REPL on` when the REPL is ready; a TCP probe would steal the resume greeting — never probe, wait for the banner.
- A client-spawned session is LIVE (real model, real bash): commands the agent runs are real commands on this machine.
