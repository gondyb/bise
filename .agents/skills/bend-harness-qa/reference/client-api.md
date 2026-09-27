# bend_client.py — full API and the wire contract

All methods on `BendSession` (import from the repo root).

## Constructors

| Method | What it does |
|---|---|
| `BendSession.fresh(bg_after=None)` | Spawn `repl-live` on a private port, fresh session file `/tmp/bend-sessions/session-<port>.txt`. `bg_after=N` sets `BEND_BG_AFTER` (bash handoff window in seconds). |
| `BendSession.resume(session_file, bg_after=None)` | Same, but restores the given checkpoint file (`BEND_CONTINUE=1`). |

Both wait for the `REPL on` banner in `/tmp/bend-client-<port>.log` and drain the resume greeting. They raise `RuntimeError` if the REPL dies at startup (read that log).

## Driving the session

| Method | Wire | Notes |
|---|---|---|
| `say(text, timeout=900, verbose=False)` | plain text (implicit `say`) | Blocks until `--- idle`. Returns the list of raw lines of the whole turn. `verbose=True` prints them. |
| `steer(text, timeout=900)` | `steer <text>` | SOCKET steer — read at idle, so it starts the NEXT turn. Only use between turns. |
| `steer_midturn(text)` | file `/tmp/bend-steer-<port>.txt` | TRUE mid-turn steering (ADR 0005): the runtime drains the file at the next model/tool safe boundary. Use while a turn runs (say in a thread). Returns immediately. |
| `notify(text, timeout=900)` | `notify <text>` | Inject a notification. |
| `compact(timeout=900)` | `compact` | Manual compaction. |
| `send(line, timeout=900)` | raw | Any protocol line, blocks to idle. |
| `close()` | `quit` | Detach; the REPL exits, the checkpoint stays on disk. |

## Reading the conversation

| Method | Notes |
|---|---|
| `last_assistant(lines=None)` | Last `obs: assistant:` text, `\n` unescaped. Pass the lines of a specific turn or omit for the latest. |
| `assistant_texts(lines=None)` | All assistant texts of the turn. |
| `unescape(text)` | `\\n` → newline. |

## The wire (one line per event)

Lines the server sends; the ones you assert on:

```
  obs: turn_started
  obs: steering_received: <text>     <- side-channel steer landed mid-turn
  obs: steered: <text>               <- injected into the model input
  obs: notification_received: / notification_delivered:
  obs: assistant: <text>             <- newlines escaped as \n
  obs: tool_started #<id>
  tool #<id> <name> : <args>
  tool_result #<id> ok|fail : <preview>
  obs: tool_finished #<id> ok|fail
  obs: tool_result_committed #<id>
  subtool <name> ok|fail : <preview>   <- a tool called from run_typescript
  obs: compaction_started #<n> (auto|manual)
  obs: candidate_discarded: <cause>
  obs: context_compaction_failed: <cause>
  obs: compaction_done: <summary...>
  obs: null_iteration
  obs: turn_done: completed|failed|...
  --- idle                           <- end of the turn batch
```

Every batch ends with `--- idle`; unsolicited lines (the resume greeting `obs: session_restored: N messages`) do NOT.

## Threading pattern for mid-turn actions

```python
result = {}
def turn():
    result["lines"] = s.say("...long instruction...", timeout=420)
t = threading.Thread(target=turn); t.start()
# wait for evidence the turn is in flight (client log), then:
time.sleep(2)
s.steer_midturn("...")
t.join()
```

## CLI of bend_client.py

```
python3 -u bend_client.py --message "hello" [--continue] [--bg-after 3]
```

## Session files and side files

- Client-spawned checkpoints: `/tmp/bend-sessions/session-<port>.txt` (one per fresh port).
- Binary-spawned (TUI/line mode): `~/.bend-harness/sessions/<id>.txt`, id `YYYYMMDD-HHMMSS-<pid>`.
- Steering side-channel: `/tmp/bend-steer-<port>.txt` (truncated by the runtime at each drain).
- Background command slots: `/tmp/bend-bg-<port>/<id>.{pid,out,rc,in}` (default root `/tmp`, `BEND_BG_ROOT` overrides).
- Bash wrapper scripts: `/tmp/bend-sh-<port>-*.sh` (cleaned at REPL startup).
- Client log: `/tmp/bend-client-<port>.log`.
