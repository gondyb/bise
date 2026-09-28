# bend-harness

A Bend 2 port of the Unified Harness design: the same three-role
architecture (Harness Core / Harness Runtime / Client) with the Harness
Step Protocol between Core and Runtime, and ADR 0011's core-owned
context budgeting and compaction.

Sources this port follows:

- Notion: "Unified Harness - Internal capabilities & Plugins RFC"
  (D1-D15 capability decisions)
- Notion: "Unified Harness" (index)
- `~/mistral/dashboard/vibe_sdk/harness/` (Rust implementation)
- `docs/README.md` (architecture), `spec/step-protocol.ts`
  (protocol shapes), `docs/ADRs/0011-context-compaction-and-overflow-recovery.md`

## Design mapping (Unified Harness -> Bend)

| Unified Harness | bend-harness |
|---|---|
| Harness Core (pure Rust state machine, one session) | Pure Bend defs over a `Session` datatype. No IO in the Core. Provable in `LAWS.bend`/`PROOF.bend`. |
| Harness Runtime (effects, provider calls, tools, persistence) | Bend `IO` module: scripted model adapter, tool executors, scenario runner. |
| Step Protocol: Command -> apply -> Accepted{transition} / Rejected | `apply : Session -> Cmd -> Result` with `obs`, `acts`, `next : Session`. Rejections carry no `next`, so a rejected command cannot mutate state (true by type). |
| Actions (CompletionAction, ToolCallAction, compaction) | `AgentCall{action}`, `ToolAct{action, call}`, `CompactionCall{action}`. Model input is projected from the session by a pure function, never stored in two places. |
| Observations | `Obs` constructors mirroring the spec's set (steering, notifications, tool lifecycle, compaction lifecycle, turn outcome). |
| ADR 0011 core-owned budgeting + compaction | `estimate` (deterministic: 1 token per 4 chars of the projected input), fitted pending projection, oldest-first removal with tool-call pairing, middle-truncation of the lone latest user message, `<summary>` validation with one retry, replacement context (system, injected preamble, preserved recent user messages, injected summary), failure codes `request_too_large`, `invalid_compaction_summary`, `replacement_too_large`. Canonical history unchanged until a summary is accepted. Stable `compaction_id`, per-attempt `action_id`. |
| ADR 0015 null-turn iteration limit | `nulls` counter in turn state; a transition with no actions and no commits increments it; over the configured limit the turn fails. Bend's mandatory termination matches the harness's iteration-limit philosophy. |
| ADR 0005 steering is pending input | `mode: queue / steer` on user messages; steer queues inside the active turn, inserted at the next model-safe boundary with `steering_received` / `steered` observations. |
| D2 dynamic tool discovery | `search` over the tool registry: `best_match` (ranked substring) and `details` (exact names), invoked as a tool, results as text. |
| D1 programmatic tool calling (`run_typescript`) | Scoped out (Bend has no V8). Kept: one completion may carry several tool calls, emitted as independently executable actions correlated by `call_id`, with pending state resumed per result (the durable-effect-boundary idea survives; the TS isolate does not). |
| D8 subagents | Stretch goal; runtime `IO.fork`/`Chan` fits the spec's model-facing API. Cut if time. |
| D6/D7 skills & knowledge folders, D11-D15 plugins | Out of scope for this port; the Core's `Config` keeps the seams (system prompt, tool catalog) they would populate. |
| Checkpoints (ADR 0002) | `export`/`restore` conversion to an opaque `Checkpoint` value. In Bend the value itself is durable; the conversion layer mirrors the "separate persistence model" rule. |
| Model provider (Runtime HTTP calls) | Scripted pure adapter (no HTTP in Bend effects today). The Runtime boundary is real: the Core never calls it, it only emits `AgentCall`. |

## Modules

Core (pure, law-checked):

- `core/types.bend` — roles, messages, commands, actions, observations, config, session.
- `core/text.bend` — shared string helpers (newline, flatten, find, sh_quote).
- `core/estimate.bend` — deterministic model-input token estimate.
- `core/history.bend` — history queries: paired drop-oldest, middle-truncate.
- `core/compaction.bend` — ADR 0011: projection fit, summary validation, replacement.
- `core/discovery.bend` — search_tool_functions.
- `core/program.bend` — D1 JS-lite interpreter (scripted mode only; live mode runs run_typescript in bend-jsrt).
- `core/session.bend` — `apply`, the step-protocol state machine.
- `core/checkpoint.bend` — export/restore, the on-disk text form.
- `core/api.bend` — provider JSON (OpenAI-style and Anthropic) from/to the wire lines.
- `core/patch.bend` — apply_patch (V4A) parser and matcher.
- `core/config.bend` — the config.toml subset parser.
- `core/commands.bend` — user lines and /commands to protocol lines.
- `core/obs.bend` — observation rendering (one wire line each).

Runtime (IO):

- `runtime/main.bend` — the runtime loop (pure transition + IO run), tool dispatch, bend-jsrt, resume replay, steering/interrupt side-channels.
- `runtime/provider.bend` — provider table, HTTPS call, retry policy.
- `runtime/settings.bend` — the config file on disk.
- `runtime/bash.bend` — the bash tool and its background-handoff contract.
- `runtime/patch-tool.bend` — the apply_patch executor.
- `runtime/selftools.bend` — self.reload / self.compact and the deferred-request scan.
- `runtime/tools.bend` — tool catalog, descriptions, the tool execution result.
- `runtime/mcp.bend`, `runtime/skills.bend` — MCP connectors, skills.
- `runtime/remote.bend` — the provider request/reply line format.
- `runtime/persist.bend` — the session checkpoint file.
- `runtime/debug.bend` (+ `debug-pure.bend`) — the session debug log: events.jsonl, provider incident artifacts.
- `runtime/model.bend` — scripted provider replies.
- `runtime/repl-core.bend` — the REPL line server; `runtime/repl-live.bend` and `runtime/repl.bend` are its two entries.
- `runtime/demo.bend` — scripted scenarios (harness-demo).
- `LAWS.bend`, `PROOF.bend` — harness invariants.

## Plan / progress log

- [x] Read RFC, architecture doc, step protocol, ADR 0011, AGENTS.md.
- [x] Write PLAN.md (this file).
- [x] core/types.bend, core/estimate.bend, core/history.bend — check
- [x] core/compaction.bend (ADR 0011: fit, summary validation, replacement) — check
- [x] core/discovery.bend (D2: best_match / details) — check
- [x] core/session.bend (apply: turn loop, steering, notifications, ADR 0015 null limit, preflight compaction) — check
- [x] core/checkpoint.bend (ADR 0002: versioned wrapper) — check
- [x] runtime/model.bend (scripted provider adapter), runtime/tools.bend (echo, search_tool_functions, self.sleep) — check
- [x] runtime/main.bend — pure `transition` + single self-recursive fuel-bounded IO loop; 11 scenarios pass
- [x] scenarios: conversation, tools+discovery+failure, PROGRAMMATIC TOOL CALLING (success + failing call), steering, auto-compaction, malformed-summary retry, manual compaction, null-iteration limit, provider failure, mid-turn interrupt + late-result rejection, bounded sleep, checkpoint
- [x] LAWS/PROOF: 36 laws (11 original + 16 harness + 5 API + 4 permissive compaction/newline), `bend PROOF.bend` -> "All terms check."; a false law is rejected with expected/observed terms
- [x] native build: `bend runtime/main.bend -o harness-demo` (2.7 s, 368 KB); native output byte-identical to the interpreter run

Scenarios run: `bend runtime/main.bend` or `./harness-demo`.
Gate: `bend PROOF.bend`.
REPL: `bend runtime/repl.bend` (or `-o repl && ./repl`), then `nc 127.0.0.1 7700`,
or `./run.sh` (the Rust TUI; the session still lives in the harness,
not in the UI).

LIVE MODE (real provider + real filesystem):
- `node --experimental-strip-types live/bridge.ts` — the HTTP bridge
  (127.0.0.1:7701): the Runtime has no TLS, so the bridge performs the real
  api.mistral.ai call (MISTRAL_API_KEY, BEND_MODEL; default
  mistral-small-latest) and speaks a line protocol with the Runtime
  (runtime/remote.bend: MODEL/TOOL/MSG/CALL/END requests, OK/CALL/ERROR
  replies; tool results pair with assistant calls by order). Default
  model zai-glm-5-3 (verified on api.mistral.ai; GLM returns content as
  blocks, the bridge normalizes).
- `bend runtime/repl-live.bend -o repl-live && ./repl-live` then
  `nc 127.0.0.1 7702` — same REPL, live adapter (Rt.live=True): the
  EModel effect does the bridge round-trip, the EExec effect runs the
  bash tool. The UI connects with --port 7702.
- TOOL CATALOG SLIMMED TO BASH-ONLY FS ACCESS: read_file and write_file
  are removed (catalog: echo, search_tool_functions, self.sleep, bash);
  the model reaches the filesystem exclusively through bash (arg =
  command): the bridge runs the real shell command (30 s timeout) and
  returns stdout (stderr prefixed " [stderr] "); failures carry the
  exit code so the model can see why. Both bridges use one flat "arg"
  schema for every tool now. Removing the two tools shrank the catalog
  estimate, so the scripted compaction thresholds were retuned (auto
  235n -> 210n, retry 220n -> 195n); the auto/retry scenarios still
  trigger mid-turn as before. No permission gate in the lab — the real
  harness would gate bash behind permissions/sandbox policy (RFC
  D3/D5).
- Compaction threshold for real conversations: BEND_THRESHOLD env
  (approximate tokens, default 800000 — tuned for 1M-context models such
  as glm-5.3, compaction near 80% of the window) with the ADR 0011
  20000-token selection budget; the scripted scenarios keep their small
  tuned thresholds. The model itself is BEND_MODEL (bridge-side).
- Verified live: the model created /tmp/bend-bash-test.txt with printf,
  read it back with cat, and confirmed the content — all through bash
  (bridge log: tools=4, exec printf + cat).
- HISTORY CONVENTION FIX: canonical history is oldest-first (commits
  append), matching the ADR 0011 helpers and the provider message order;
  the live run exposed that commits used to prepend (reversed), which the
  scripted scenarios masked. Scenario thresholds retuned for the larger
  tool catalog (auto 235n, retry 220n).
RUST REBUILD (bridge + TUI — now the reference clients):
- `rust/` cargo workspace, two crates.
- `rust/bridge` (ureq 2 + rustls): exact port of live/bridge.ts, same
  line protocol on 127.0.0.1:7701 (MODEL/TOOL/MSG/CALL/END, OK/CALL/
  ERROR + END; EXEC + END for bash). Same env: MISTRAL_API_KEY,
  BEND_MODEL (default zai-glm-5-3). GLM block-content normalized the
  same way. Build: `cd rust && cargo build`, run
  `./rust/target/debug/bend-bridge`.
- `rust/tui` (ratatui 0.29 + crossterm 0.28): Vibe-CLI-style terminal UI
  over the REPL socket — header (model, host:port, connection state),
  per-turn feed with tool start/finish line merging, thinking dots,
  input box with history, status bar. `--host/--port` (default
  127.0.0.1:7702). Piped stdin -> line mode (scriptable): echoes the
  user line, prints the feed, waits for the turn's "--- idle" before
  reading the next command (120 s cap for live model calls; 5 s after
  quit). `quit` ends the client; the harness session survives.
- Verified end-to-end: TUI line mode -> repl-live (7702) -> Rust bridge
  -> GLM with a real tool round-trip (echo call, calls=1), correct
  UTF-8 accents, compaction display, session persistence across
  commands. Interactive ratatui mode needs a real TTY (not tested
  headless, same limitation as the Ink version).
- Gotchas found: the reader thread must accumulate raw BYTES and
  String::from_utf8_lossy per line (byte-as-char is Latin-1); tool-call
  -only assistant lines are "assistant:" after trim — skipped.
- The TS versions (live/bridge.ts, repl-tui/, repl-ui/) are DELETED:
  the Rust pair is the reference and the only client.
- `./run.sh` — convenience wrapper over the SINGLE EXECUTABLE
  rust/target/debug/bend-harness (auto-builds it and the Bend REPL
  binaries if missing, then execs it).
- SINGLE EXECUTABLE (rust/harness, bin bend-harness): one process tree
  per terminal — the HTTP bridge runs as a thread inside it, the Bend
  REPL (repl-live/repl-scripted) runs as a child process (found next to
  the exe or in cwd), and the ratatui TUI runs in the main thread. The
  child dies with the parent: no orphaned listeners. Flags:
  --scripted, --model NAME, --port N (force the REPL port), --verbose
  (bridge logs on stderr; off by default so they never garble the TUI).
  To make this possible the crates were restructured: bridge and tui are
  now lib+thin-bin (bend_bridge::serve(listener), bend_tui::run(host,
  port, is_live)).
- PARALLEL INSTANCES: all three ports were hardcoded (REPL listen 7700/
  7702, bridge connect 7701 in model_call + bash_exec). Now env-driven
  (IO.get_env, like BEND_THRESHOLD): BEND_REPL_PORT (defaults 7700
  scripted / 7702 live) and BEND_BRIDGE_PORT (default 7701). bend-harness
  picks free ports automatically, so N terminals = N independent
  sessions. The standalone binaries keep their defaults; one bridge can
  serve several REPLs (requests are stateless). Verified: two
  --scripted instances side by side with independent sessions; live
  GLM + bash round-trip through the single executable; zero leftover
  processes after exit.
- CODEX-STYLE INPUT (rust/tui): plain text is sent as an implicit
  "say <text>" — no prefix to type. Slash commands (client-side
  translation, popup filtered as you type, Tab completes, arrows
  navigate): /compact, /interrupt, /steer <t>, /notify <t>, /status
  (local), /clear (local), /help, /quit (/exit alias). Raw protocol
  words (say/steer/notify/compact/interrupt/quit/help) pass through
  unchanged. Keybindings: Enter send, Esc interrupt turn (or close
  popup), Ctrl+C clear input then quit, Ctrl+L clear feed, Up/Down
  history (popup navigation when open), Left/Right/Home/End/Ctrl+A/Ctrl+E
  cursor (UTF-8-safe char cursor), Backspace/Delete, Ctrl+W delete word.
  Line mode shares the same command translation, so pipes and the
  interactive UI behave alike. Verified via pty (script -q + stty size).
- FEED WRAPPING (rust/tui): the feed Paragraph wraps (ratatui Wrap,  trim: false) and the "render only what fits" logic counts WRAPPED
  rows (ceil(line width / inner width)) from the bottom, so a long
  assistant message spans several terminal rows and its end stays
  visible instead of being clipped at the right edge. The input box
  scrolls horizontally the same way: a sliding char window keeps the
  cursor visible for long inputs. pty-test.py drives the TUI under a
  pty with a set winsize (script -q is unreliable for this: the pty
  needs a size and Enter forwarding is flaky).
- NEW LAWS (16, total 27): commit convention (commits_append_at_tail —
  H.commit/H.last in core/history.bend pin oldest-first; the prepend
  regression a live run exposed and the scenarios masked breaks it
  instantly); wire protocol (wire_call_roundtrip pins the " : "
  separator, wire_reply_roundtrip pins the OK/CALL/END bottom-up
  parse — the contract the Rust bridge speaks); session rejections
  (orphan_tool_result_rejected, idle_interrupt_noop — both universal
  over cfg, closed by case-splitting in the proof); ADR 0011 budget
  (truncate_marks_the_cut, truncate_keeps_small_messages,
  preserve_recent_keeps_users with the H.all_user helper); obs wire
  format (obs_wire_format — show_obs moved to core/obs.bend, pure:
  every obs is one "  obs: " line); command language (7 laws, below).
  Gate: `bend PROOF.bend` -> "All terms check." — all closed by
  unfolding the definitions.
- SERVER-SIDE COMMAND LANGUAGE (core/commands.bend): the codex-style
  input interpretation moved from the Rust TUI INTO the harness — pure
  Bend, law-pinned, and every client gets it (nc included): plain text
  -> implicit "say <text>"; /compact /interrupt /steer <t> /notify <t>
  /help /quit //exit -> protocol words; unknown /x -> a Note warning
  emitted as an obs line; raw protocol words pass through verbatim.
  Laws: implicit_say, raw_words_pass_through, slash_compact_maps,
  slash_steer_maps, slash_steer_needs_arg, slash_quit_maps,
  unknown_slash_refused. The Rust TUI keeps only its own lifecycle and
  display (/quit /clear /status /help are client-local); everything
  else goes to the harness verbatim. GOTCHA: a Note must also emit
  "--- idle" or line-mode clients wait for a turn that never runs.

- FULL-BEND PROVIDER + BASH (hub.bend-lang.com packages, pinned by
  hash): the Rust bridge is DELETED (rust/bridge crate and
  live/bridge.ts). Everything the bridge did now runs inside the Bend
  REPL process:
  - core/api.bend (pure, law-pinned): parses the wire request
    (MODEL/TOOL/MSG/CALL) into WReq, builds the OpenAI-compatible JSON
    body (FIFO pairing of tool results to call_<n> ids, flat {"arg"}
    schema, GLM content-blocks normalization), and maps a provider
    response back into the OK/CALL/END reply remote.bend parses.
    5 new laws: api_wire_parses, api_body_maps, glm_blocks_flatten,
    api_tool_calls_map, api_error_maps.
  - EModel (runtime/main.bend): model_call reads MISTRAL_API_KEY +
    BEND_MODEL (default zai-glm-5-3), utf8-ENCODES the JSON body (the
    hub HTTP client speaks BYTES: one Char per octet — sending text
    with accents gives a 400 "error parsing the body"), POSTs via the
    hub HTTP client (hash 0xbf477e66..., HTTP/1.1 + DNS + TLS, 120 s
    timeout), and decodes the response with Http.text (utf8 decode).
  - EExec: bash runs via the hub snap runner (hash 0x9bfd9d5...,
    execvp): /bin/sh -c with the flattened command (multi-line commands
    would break snap's newline-joined argv wire — the harness protocol
    already flattens call args, and bash_exec flattens again for
    double safety). snap answers exit status on the first line, then
    stdout+stderr.
  - bend-harness (the single executable) no longer embeds any bridge:
    it spawns the REPL child (BEND_REPL_PORT) and runs the TUI.
    BEND_BRIDGE_PORT is gone. Build time ~9 s (TLS stack compiled in),
    binaries ~1 MB.
  - Verified: GLM call with French accents (correct UTF-8 both ways),
    bash tool round-trip, 13 scenarios, 32 laws, parallel instances,
    pty tests. Package install: `import 0x<hash>/file.bend as X` —
  - the compiler fetches and pins per-file hashes (cached in
    ~/.bend/lib). NOT-viable packages tried: several JSON libs fail to
    compile on bend 2.0.27; the working ones: JSON
    0x1f4d6c03... (production-grade, RFC 8259 + pointers), HTTP
    0xbf477e66..., snap 0x9bfd9d5....

- PERMISSIVE COMPACTION: live /compact could fail with
  invalid_compaction_summary when GLM answered the summary prompt with
  a tool call (the request offered the whole catalog with tool_choice
  auto) or an empty text. Fixes, at the right seams:
  1. Compaction requests carry NO tools (Rem.build_request.bare; the
     transition routes CompactionCall separately from AgentCall) and
     api_body OMITS the tools/tool_choice keys entirely — a tool-call
     answer is structurally impossible. Law: api_bare_body_omits_tools.
  2. Malformed summaries retry up to 3 attempts (was 1).
  3. An unclosed <summary> block takes the rest of the text as the
     summary instead of failing. Law: unclosed_summary_falls_back.
- NEWLINE TRANSPORT + MARKDOWN: the wire is line-oriented (one obs =
  one line, law-pinned), but markdown needs real newlines. Real
  newlines now travel ESCAPED as literal backslash-n:
  reply_of escapes the OK content (blocks joined newline-separated,
  no flattening), the Core stores the escaped form (wire-safe), and
  parse_wire UNESCAPES message contents so the API JSON carries real
  newlines again. api.bend has one self-recursive escape/unescape
  machine each (state in Data, fuel first — the json parser pattern).
  Laws: escape_keeps_lines_single, escape_roundtrip,
  glm_blocks_join_lines (updated).
- TUI MARKDOWN (rust/tui): user and assistant messages render a
  pragmatic markdown subset — fenced code blocks (```), headers (#),
  bullet lists, blockquotes, inline **bold**, *italic* and `code`
  (inline_spans/md_to_lines). Events expand to multiple lines
  (ev_lines); the feed's bottom-fit counts wrapped rows over all
  expanded lines; line mode prints the same expansion. Compacted
  summaries unescape too.
- Verified live: markdown answer (title/list/code/italic each on its
  own line), tool output rendered as code lines, /compact twice in a
  row accepted, 36 laws, 13 scenarios, pty regression.

- CLEAN FEED (--debug): structural annotations — the turn separator
  ("── tour ──"), the turn-done line, and the idle marker — are
  debug-only now (Ev::Idle; ev_visible filters Turn/TurnDone/Idle/Raw
  from the feed unless --debug, in both interactive and line mode).
  Messages, tool lines, compaction results and errors always show.
  ./run.sh --debug (and bend-tui --debug) brings the annotations back.

- FEED SCROLLBACK (rust/tui): the feed renders ALL visible lines in one
  Paragraph with .scroll((start_row, 0)) — ratatui wraps first, so the
  scroll offset counts wrapped rows exactly. scroll = rows from the
  bottom (0 = follow the newest line). PgUp/PgDn page by half the
  viewport, the mouse wheel scrolls by 3 (mouse capture enabled
  around the TUI loop), End is contextual: scrolled up -> follow the
  bottom, at the bottom -> cursor to end of line. While scrolled up,
  incoming events grow the offset so the view stays pinned; sending a
  message jumps back to the bottom. A ratatui Scrollbar on the right
  edge (one column, reserved out of the text width) shows the position
  when the content exceeds the viewport. Verified via pty with
  forced full redraws (resize trick — differential frames don't show
  unchanged cells).

- SESSION PERSISTENCE (--continue): core/checkpoint.bend serializes the
  whole session (BEND-SESSION/TOOL/CFG/COUNT/MSG lines, calls, tool
  results) with a roundtrip law; runtime/persist.bend reads/writes it.
  The REPLs load the checkpoint at startup (BEND_CONTINUE=1 restores the
  messages), save after every turn, and greet with a
  "session_restored: N messages" observation. bend-harness --continue
  points BEND_SESSION_FILE at ~/.bend-harness/session-<repl>.txt; the
  TUI shows "session restauree . N messages". Verified live: GLM
  remembered a code word across full restarts.

- MISTRAL MCP CONNECTORS (zero context cost): runtime/mcp.bend. At live
  startup the REPL bootstraps GET api.mistral.ai/v1/connectors/bootstrap
  ?supports_mcp=true and writes ~/.bend-harness/mcp-index.txt (one line
  per tool: "<cid> <cname> <tool> : #<desc>", descriptions
  newline-flattened). The agent reaches the 359 remote tools through TWO
  static catalog tools, so no MCP schema ever enters the model context:
  - search_mcp_tools: the search_tool_functions protocol (best_match /
    details) over the index, via core/discovery
  - call_mcp_tool: "<connector>.<tool> <json args>" -> three JSON-RPC
    requests on ONE pooled connection (initialize,
    notifications/initialized, tools/call) against
    /v1/connectors-gateway/<cid>/mcp; the answer extracts
    result.content[].text and isError
  JSON parsing goes through the hub package 0x16458a2d (numbers as text):
  Bend's Nat is 48-bit and the bootstrap carries 2^53-1 schema maximums.
  Request bodies are still built with 0x1f4d6c03 (J.stringify/J.raw).
  bend-harness exports BEND_MCP_INDEX. Verified live end to end: the
  model found github_app.get_me with search_mcp_tools, called it via
  call_mcp_tool, and answered the GitHub login (gvergnaud).

- SCRIPTED COMPACTION FIX: trans.model.compact used to emit EModel (a
  real HTTP call) even in scripted scenarios, so the compaction scenarios
  silently hit the live API and broke their token budgets. Now scripted
  mode pops the script like every other model call; live mode sends the
  bare request. The 6-tool catalog raised the scenario thresholds
  (auto 291n, retry 276n: +81 tokens of tool descriptions) — the
  trigger points are unchanged. The later removal of the echo tool
  shrank the catalog to 5 tools (-9 tokens): auto 282n, retry 267n.
  Both compaction scenarios are green and
  deterministic again (retry shows candidate_discarded:
  invalid_compaction_summary then success, as designed).

- UNIFIED TOOL SURFACE (merged MCP): search_mcp_tools and call_mcp_tool
  are gone from the catalog. search_tool_functions is the single
  discovery engine: it searches the static catalog PLUS the MCP index
  (merged_search in mcp.bend). Any "<connector>.<tool>" the model calls
  — top-level or from a run_program program — resolves against the
  index at the routing seam (trans.tool) and speaks MCP by name.
  run_program joined the catalog (4 tools: search_tool_functions,
  self.sleep, bash, run_program), so live models can use D1. The EExec
  effect carries the live flag: scripted scenarios and the scripted REPL
  never read the MCP index (hermetic, deterministic — a machine-local
  index had leaked into the scripted battery and shifted the
  thresholds). A call with empty arguments defaults to a "{}" JSON
  body. Scenario thresholds: auto 290n, retry 275n (catalog = 153
  tokens). Verified live both ways: run_program with
  "call github_app.get_me; ret" AND a direct github_app.get_me tool
  call both answered the login.

- RUN_PROGRAM IN A V8 ISOLATE (the Vibe SDK core pattern): live models
  write REAL TypeScript/JavaScript. rust/jsrt (a standalone cargo project:
  deno_core raw v8 + deno_ast) runs each round in a FRESH isolate with
  heap limits (4-64MB) and a 10s watchdog; deno_ast transpiles TS
  in-process (types, interfaces, enums — no tsgo). Free identifiers become
  tool calls through a `with`-Proxy (dotted names compose:
  github_app.get_me). The first missing tool result prints a JSON request
  and exits 42; the runtime executes the tool through the same exec paths
  (bash, search, MCP) and RE-RUNS the isolate with the extended results —
  replay by re-execution, the same durable-boundary semantics as the
  in-house interpreter (which stays for scripted sessions and the laws).
  console.log goes to the tool's error output for debugging. The runtime
  renames live run_program completions to node_program so the Core treats
  them as ordinary tool calls. Verified live: interface User {...}, typed
  const, template literal, arrow + map, github_app.get_me, replay,
  returned the login (gvergnaud). run.sh builds bend-jsrt if missing
  (first build: a few minutes — v8).

Bend has no stdin effect, so the REPL is a line server: the session
persists across lines and across connections. Commands: say (with the
`tools` / `prog:` model modes), steer, notify, compact, interrupt, quit.
The loop uses the echo-server pattern: curried-closure continuations and
two @unsafe loops (per-connection recv, accept). The run loop's output is
parametrized (Out: stdout or socket) and returns the final session, so
the REPL threads one session through the whole conversation.

Deviation notes:
- run_typescript (D1): IMPLEMENTED in Bend itself (core/program.bend).
  Where the Unified Harness hosts a V8 isolate, here the orchestration
  language is Bend: `run_program` parses a tiny program
  ("call <tool> <args>; ...; ret"), the Core interprets it until the next
  missing tool result (the durable effect boundary), emits that ToolAct,
  and resumes by replaying the program with the recorded results. The
  interpreter is pure and total; 4 laws pin replay order, the wait
  boundary, parse roundtrip, and malformed-program refusal. run_program
  must be the sole call of its completion; a failing program call fails
  the program.
- Subagents (D8), plugins (D11-D15), skills/knowledge (D6/D7) out of scope;
  Config keeps the seams they would populate.
- The Runtime executes tool batches sequentially; the drain gives steering,
  notifications, and interrupts priority at model-safe boundaries.
- The interrupt scenario demonstrates the mid-turn interrupt plus the
  late-completion rejection, not the idle no-op.

## 2026-09-25 — vibe_sdk parity for run_typescript + search_tool_functions

The model-facing surface of the two direct tools is now EXACTLY the
vibe_sdk surface, and the live loop drives it end to end.

Exact copies (byte-verified against vibe_sdk/harness/core sources):
- tool-desc-run-typescript.txt (3619 chars, RUN_TYPESCRIPT_DESCRIPTION)
- tool-desc-search.txt (1087 chars, SEARCH_TOOL_FUNCTIONS_DESCRIPTION)
- prompt-tool-use.txt (tool_use_prompt) + prompt-current-time.txt
  (current_time_prompt) — the live system prompt is identity guidance
  + these two sections (repl-live live_cfg, IO-loaded; scenarios keep
  the short pure prompt so thresholds stay hermetic)
- per-tool parameter schemas in core/api.bend (generated from the
  schemars output, tool_fn_json dispatches by name): run_typescript
  {"code": string} required/deny_unknown_fields; search_tool_functions
  the full SearchRequest schema (mode/query/functions/functionNames/
  connectors, maxItems 10, minLength 1, defaults). The live path loads
  the full descriptions (X.catalog()); scenarios keep catalog_short.

Search execution now parses the structured SearchRequest (runtime/
tools.bend): best_match = word-scored ranking (score = number of query
words matched, ties keep catalog order), names only, cap 20, connectors
filter, moreCandidatesAvailable when >20; details = declarations for
exact names (functions ++ functionNames); all_connector_capabilities =
connector inventories. The flat 'best_match: query' syntax stays for
the scripted suite (J.parse falls back to it when args are not JSON).

Two wire bugs found and fixed during live e2e:
- Call ids: provider replies number calls per completion (1, 2, ...),
  so sequential turns repeated ids on the wire (both calls echoed as
  call_1, provider 400 invalid_args). apply.completion.tools now
  renumbers calls to their action ids (ToolAct, PTools and history all
  agree; take_call matching unchanged).
- node_program leaked into model context (history call echo and result
  text said node_program while the model called run_typescript).
  The internal rename stays for routing; result_name (core/session)
  and wire_name (runtime/remote) translate it back on every
  model-visible surface.

MCP index lines now carry the tool input schema (tool_line appends
" | input: <json>"), so details mode returns real declarations like
vibe_sdk — the model called github_app.list_issues with correct
owner/repo args on the first try after the change.

bend-jsrt (V8): supports the vibe format — `async function main()`
detected on the transpiled source (function main followed by '('),
Promise resolved through perform_microtask_checkpoint; errors print
{"error": "..."} and exit 43 (node_split parses the message so the
model reads it and retries); legacy `return`-body format still works.
BEND_WIRE_DUMP=<path> dumps the last provider request (debug).

Verified: bend PROOF.bend green (39 laws); harness-demo 13 scenarios
byte-identical to the baseline; live e2e: best_match → details →
run_typescript → github_app.list_issues with correct args, error
feedback loop (missing owner → fixed → missing repo → fixed), and a
multi-tool Promise.allSettled program, all over the real GLM provider.

## 2026-09-25 — the tool-calling feed, rebuilt for a human engineer

The old feed showed `outil #2 …` -> `outil #2 ok`: an id and an
outcome, nothing else. What an engineer actually needs, in order:
what tool with what args (wrong args are the #1 failure cause), the
error message on failure, how long a call took (a hung bash is the
most common live debugging case), and what a run_typescript program
actually did (its sub-calls were invisible — one program of 6 tool
calls collapsed into a single line).

Runtime annotations (live only; scripted runs and scenarios stay
byte-identical). The EExec loop emits around each tool execution:
  tool #<id> <name> : <args>              (flattened, capped 200)
  tool_code #<id> : <args>                (run_typescript only: FULL args,
                                          wire-encoded: \N newline, \R CR,
                                          backslash doubled — never capped)
  tool_result #<id> <ok|fail> : <out>    (flattened, capped 200)
exec_program emits one line per sub-call:
  subtool <name> <ok|fail> : <out>
ESleep carries (ms, id, secs) now so self.sleep annotates too.

RUN_TYPESCRIPT CODE BLOCK (the TUI reads tool_code): the merged tool
row renders the WHOLE program under the tool line, in a rounded box —
orange border while running, dim once ok, red on fail — with a
typescript header, a line-number gutter, and hand-rolled TS highlighting
(keywords purple, strings green, comments faint italic, numbers orange,
calls blue, types yellow; the OpenCode syntax palette). The box is
hard-clipped on the right, never word-wrapped (a wrapped box is not a
box). The old TS clients (repl-tui Ink, repl-ui, live/bridge.ts) are
DELETED: the Rust TUI is the only client.

The Out handle threads through the exec chain. Bend lessons paid for:
Type-kind values are ALSO use-once (no duplication, + is illegal on
Type), do-binds and pairs must be consumed exactly once, and mutual
recursion is impossible — exec_done / exec_program.step consume the
(Out & X) pair once and continue through a closure that
self-references the loop def (the serve-loop idiom); run and
exec_program are @unsafe for exactly that reason.

Core fix found by the e2e: tool_started carried the provider's
per-completion index while tool_finished carried the action id — they
only coincide on the first completion of a fresh session, so the TUI
could not pair events by id. tool_start_obs now renders the
renumbered calls; started/finished/annotation ids all agree. (This
changes the scenario feed: started ids are now action ids.)

TUI (ratatui): ToolData{id, name, args, state, result, started}
merged by id; render is  mark + name + elapsed + args preview, with
a result line (errors red):
  ● bash 3.2s — ls -la                 (running, elapsed ticks live)
  ✓ bash ok 0.3s — ls -la
    → 3 files
  ✗ bash ECHEC 1.2s — rm /protected
    → exit 1: permission denied
  ↳ github_app.list_issues ok {"issues":[]…     (program sub-calls)
Elapsed is measured client-side (Instant at event arrival), so no
clock effect is needed in Bend. Args previews are per-tool:
run_typescript shows the first line of main(), search shows
`mode "query"`, bash/mcp show the raw args (a naive JSON string
extractor tolerates the 200-char wire truncation). Line mode holds
running tools in a map and prints one merged line at completion.

Verified live (GLM, full discovery turn): search/bash/run_typescript
lines with names, args, elapsed, result previews; parallel calls;
sub-call lines under each program; a failed program attempt showing
the error in red. PROOF green (39 laws), 13 scenarios (started ids
re-baselined), rust build 0 warnings.

## 2026-09-25 — scrollback rebuilt: top-anchored + follow flag

The bottom-relative offset (rows from the bottom, 0 = follow) needed an
increment for every incoming event to keep a scrolled-up view pinned —
fragile accounting that drifted (tool enrichments add rows to existing
events, widths change, events merge). Replaced with the simple model:

- App: follow: bool, top: usize (offset from the FIRST row), max_top
  published by draw for the input handlers
- follow=true: draw pins top to max_top every frame (sticks to bottom)
- any scroll up (PgUp, wheel): follow=false — and since the view is
  TOP-anchored, new content appended below cannot move it, zero
  accounting needed
- PgDn/wheel down reaching max_top, End, sending a message, /clear or
  Ctrl+L: follow=true again
- scrollbar position = top

Also fixed (found by the pty test): the elapsed on FINISHED tools kept
ticking forever (an `echo` line read "ok 8.0s" growing to "ok 14s").
ToolData now freezes the elapsed at the finish merge; only the running
● line ticks live.

Verified with a pyte pty test on a live 6-tool turn: follow shows the
newest lines; after PgUp the view is byte-identical across 6 more
seconds of incoming content; End and PgDn-past-bottom return to the
bottom and re-follow; frozen elapseds. PROOF green (39 laws), 13
scenarios, rust 0 warnings.

## 2026-09-25 — feed hierarchy: gray tool blocks, blank separators

Tool calls now read as machinery between prose blocks: everything on a
tool line is DIM gray (name, args, elapsed, result preview) except the
status mark and label (● yellow / ✓ green + "ok", ✗ red + "ECHEC") and
error previews, which stay colored so failures stay scannable. Sub-call
lines are gray too. A blank line is inserted at every transition from a
tool block (Tool/Sub events) to a message (You/Assistant) in the draw
loop — the scrollback counts those rows automatically since they are
part of the rendered lines. Verified with a pyte pty run: white prose /
gray tool block / blank line / next message.

## 2026-09-25 — elapsed freeze verified; abandoned tools at turn end

Reported: finished tool timings kept incrementing. Verified on the
current binary with pyte pty runs: finished tools freeze (byte-
identical feeds 9s apart, including sub-call lines and previews). The
report traced to a stale bend-harness process started before the
freeze landed — a running process embeds the old code; restart
./run.sh to pick it up.

Two hardening fixes while re-checking:
- push_event: at turn end (TurnDone/Idle) any tool still shown as
  running was abandoned (interrupt or failed turn) — frozen as
  ✗ ECHEC with result "interrompu", so no line can ever tick forever.
- line mode (feed_line): abandoned held tools print one final
  "interrompu" line at turn end, and finished tools freeze their
  elapsed at print time.

Known semantics (not a display bug): Esc cannot cancel a blocking
tool exec mid-flight — the harness interrupt takes effect at the next
model-safe boundary, so during e.g. `sleep 30` the ● line keeps
ticking because the tool genuinely still runs; it freezes when the
turn actually ends.

## 2026-09-25 — 27 new laws pin the recent surface (and caught a live bug)

The laws covered the Core, the wire and compaction, but none of the
logic added for vibe_sdk parity. 27 laws now pin it — each one guards
a bug we actually shipped or a wire contract a client parses:

- call-id renumbering: sequential ids from the base, name/args
  untouched, length preserved (the duplicate call_1 wire 400)
- the node_program rename: wire echo, result text and the feed
  annotation all say run_typescript; other names pass through (the
  context leak)
- structured args: run_typescript/search args pass through un-wrapped,
  ordinary tools keep the flat wrapper, call_code extracts the code
  field (the root cause of the 400 invalid_args)
- structured search: best_match returns NAMES without declarations,
  two-word matches outrank one-word, cap 20 with
  moreCandidatesAvailable, connectors filter; details never comes back
  silently empty (missing message + section), functionNames alias, and
  the flat fallback for the scripted suite
- TUI annotations: tool/tool_result/subtool formats byte-for-byte,
  single-line, capped at 200; engine error payloads extract their
  message (exit-43 feedback loop)

The connectors law failed on first run and exposed a real bug the live
runs never surfaced: conn_keep conflated "no connectors given" (keep
everything) with "no connector matched" (drop) — its Nil base case
returned True for both, making the connectors filter a no-op that kept
every tool. Fixed by splitting conn_hit (any-match, False base) from
conn_keep (empty list = no filter). The law paid for itself before it
was even merged.

Gate: bend PROOF.bend — 66 laws green. Suite byte-identical, all
binaries rebuilt.

## 2026-09-25 — scrollback rebuilt on the codex transcript architecture

The old feed rendered EVERY event through markdown + ratatui word wrap
every 80ms frame and scrolled a Paragraph over the full history —
O(total) per frame, so long sessions lagged and scrolling felt broken.
Research: pulled openai/codex and studied transcript_view/ (layout
cache, Position::Reading anchor, tail_visible, the "Back to bottom"
follow control) plus the ratatui maintainer's guidance (store lines,
render only the visible range).

New rendering pipeline (rust/tui):
- EventRows cache: each event renders to wrapped rows ONCE (span-aware
  word wrap with unicode widths, hard-split for over-wide words);
  invalidated on mutation (enrichment, finish merges), width change,
  or for running tools (their elapsed ticks each frame). The block
  separator is part of the event's rows.
- Every frame: O(n) over cache row-counts for starts/total, then only
  the visible slice [top, top+h) is cloned into the Paragraph — no
  Paragraph wrap, no Paragraph scroll. O(visible) rendering.
- Back-to-bottom bar (codex follow control): the indicator row shows
  "↓ Bas (End) · N nouvelles lignes" whenever the tail is out of
  view; clicking it (mouse hit rect) returns to the bottom; the
  unseen counter tracks appended events while pinned and clears on
  every re-follow (End, PgDn to bottom, send, click, /clear, Ctrl+L).
- wrap width = feed area - 1 (scrollbar column), same as before.

Verified with pyte pty runs on live turns: pinned top row identical
across streaming content; the bar appears while pinned and reports
new activity; a click on the bar jumps back to the tail and clears
the bar and counter; line mode unaffected. 0 warnings.

## 2026-09-25 — the silent mid-turn stop: run fuel was 100 hops

Reported: the agent often stops mid-turn with no explanation. Root
cause: repl_plan gave the whole turn R.run(100n, ...) — every loop hop
(model call, obs emit, tool exec, commit) burns one unit, so ~5 hops
per round ≈ 20 model rounds. Longer agentic turns exhausted the fuel
mid-turn: run returned REnd silently, the REPL saved the session and
went idle — the turn stayed pending, nothing was shown. Compounding
it, the TUI hid EVERY turn_done in non-debug, so even legitimate
failures (null iteration limit after 3 empty model completions,
provider errors after their discard warning) ended invisibly.

Fixes:
- repl-live: fuel 100n -> 1000000n (effectively unbounded for a
  terminal session); if a session ever does return with the turn still
  active, repl_end_stalled emits "obs: turn_stalled: execution budget
  exhausted" before saving — never silent again
- exec_program fuel 50n -> 1000n (50 ≈ 12 program sub-calls before
  "program step budget exhausted")
- TUI: turn_done completed stays hidden, but "failed: X" renders as a
  red "tour échoué : X" line, "interrupted" as a warning; the
  turn_stalled obs parses as an error; null_iteration reads "réponse
  vide du modèle — nouvelle tentative" instead of "itération nulle
  comptée"

Verified live: a 25-command sequential bash turn (would have died
around command 20 on the old budget) completes fully with the final
summary. PROOF green (66 laws), suite byte-identical, all binaries
rebuilt, 0 warnings.

## 2026-09-25 — agent-loop robustness audit

Audited every seam of the loop for failure modes. Three real gaps found
and fixed; the rest verified already-safe:

FIXED — bash watchdog (runtime/main.bend): snap has NO timeout
(waitpid without deadline), so a blocking command (a bare `cat`, a
server, sleep 1000) hung the whole turn irrecoverably — and a killed
subshell leaves orphan children holding the stdout pipe. The command
now runs under a watchdog subshell: `( cmd ) & c=$!; ( sleep N;
pkill -9 -P $c; kill -9 $c ) & w=$!; ...; exit $s` — children killed
first (releases the pipe), BEND_BASH_TIMEOUT env (default 120s, max
3600), exit 137 mapped to "command exceeded the bash timeout and was
killed. Partial output: ...". (A set -m process-group kill was tried
and rejected: job-control notifications pollute every output.)

FIXED — tool output cap: results entered history unbounded; a catted
binary blew up the next provider request. cap_out at exec_cmd (the
single choke point into the Core): 20000 chars + "[output truncated at
20000 chars]". Covers bash, MCP, run_typescript, search.

FIXED — provider retries: any transient failure (connect/DNS/timeout,
5xx, 429) killed the whole turn instantly. model_call now classifies
each attempt purely into AOk/AFail/ARetry and retries twice with a
1.5s pause (the retry goes through a continuation closure
self-referencing model_call.try — mutual recursion is not expressible;
the exec_done idiom). 4xx fails immediately, as before.

VERIFIED-SAFE (no change): malformed 200 JSON degrades to an empty
completion (null-iteration retry, now visible); jsrt
crash/missing-binary maps to readable exit codes; MCP timeout 20s;
client disconnect mid-turn keeps the turn alive (emit errors
swallowed); session save is best effort; "core rejected" lines are
visible; the fuel is 1M hops with a loud stall warning.

Live verification: sleep-1000 killed at 3s (exit 137, message read and
relayed by the model, no orphan processes); 60000-char output
truncated at 20000 (model reported truncation and the cut);
normal commands clean; PROOF green (66 laws), suite byte-identical.

Testing footnote: pkill -f uses ERE — "a\|b" matches nothing (literal
backslash-pipe); use "a|b". A stale repl-live survived several test
rounds on a busy port because of this, sending tests to the wrong
binary.

## 2026-09-25 — every quick bash command cost the full 120s timeout

Reported: simple bash commands took ~2 minutes. Reproduced with a
stopwatch: tool_started at 1.9s, tool_result at 122.0s — exactly
BEND_BASH_TIMEOUT. Root cause: the watchdog's own sleeper. The sleeper
subshell `( sleep N; ... )` inherited the RESULT pipe; when a quick
command finished, the script killed the sleeper subshell, but its
child `sleep 120` became an orphan still holding the pipe open —
snap reads to EOF, so every command waited out the full deadline.

Fix (runtime/main.bend, bash_watchdog):
- the sleeper's output is redirected to /dev/null — it can never hold
  the result pipe
- after the command: `pkill -9 -P $w` then `kill -9 $w` — no orphan
  sleep lingers at all

Bend to the max — 13 new laws (66 -> 79), each pinning a surface that
broke or nearly broke:
- bash_watchdog_script: the watchdog byte-for-byte, sleeper redirect
  and children-first kills included — the 2-minute regression can
  never come back silently
- bash_timeout defaults/clamps; exit 137 maps to the readable message
- cap_out: limit 20000 pinned, logic proven at small scale via the
  parameterized cap_out_at (a 20001-char literal overflows the proof
  checker's stack — deep string recursion)
- retryable_status/retryable_err tables; attempt classification
  (timeout -> ARetry with why, TLS -> AFail)

Verified live with timings: quick command 0.05s (was 120s); blocking
sleep-1000 killed at 3.09s with the readable message; 50000-char
output capped. PROOF green (79 laws), suite byte-identical, all
binaries rebuilt.

## 2026-09-25 — Enter steers, Tab queues (codex composer semantics)

The Core already had both paths (ADR 0005: T.Steer — pending input
injected at the next model-safe boundary; T.Queue — held for the next
turn; both proven by the scenarios), but the TUI sent plain text, so
Enter during a running turn always QUEUED — steering required typing
"steer ..." by hand.

Now (rust/tui): Enter while the agent works sends "steer <text>" (the
input joins the running turn); at idle Enter starts a turn as before;
/commands pass through untouched. Tab while the agent works sends
"say <text>" (queued for after the turn; the wrapper also neutralizes
text that would read as a protocol word); with the slash popup open
Tab still completes. The feed echoes the typed text without the
transport prefix, and the status bar hint follows the turn state.

Live pty verification on a slow 8s bash turn: Enter mid-turn delivered
the steering into the same answer ("penguins" in the final message);
Tab mid-turn queued and ran right after the turn ended; idle hints
restored after.

## 2026-09-26 — Background bash: 30s sync window, file-based contract

Bash commands no longer block the turn forever. Each call runs under a
generated wrapper (runtime/main.bend, bg_wrap) written to
/tmp/bend-sh-<port>.sh — snap joins argv with newlines, so a multi-line
`sh -c` script must be passed as a file, not as one -c argument.

Wrapper contract, per call, in $BEND_BG_ROOT/bend-bg-<port>/<id>:
- .in  — fifo opened RDWR by the wrapper (`exec 9<>`): never EOF, never
  blocks the opener; the agent feeds it with printf '%s\n' 'text' > <id>.in
- .out — output file (NOT a pipe: the orphan-holding-pipe class of bug,
  the 120s hangs, is structurally impossible)
- .pid — the command's real pid, written by a detached RUNNER subshell
  `( ( CMD ) >out 2>&1 <&9 & echo $! >pid; wait $!; echo $? >rc ) &`
  (a sibling cannot wait — that produced exit 127)
- .rc  — real exit code written by the runner when the command ends

Sync window: the wrapper polls kill -0 every 0.1s up to BEND_BG_AFTER
(default 30s, clamped [0,3600]). Done in time: output + exit code
returned, slot cleaned. Otherwise: exit 199 + a printed contract with
concrete paths, real pid, ready-to-copy commands. exec_of maps 199 to
Ex{True, contract} — the turn continues, the process lives until the
agent decides (tail the output, feed stdin, kill, or leave it).

No new tools: the agent drives everything with plain bash against the
contract paths; the bash tool description (tool-desc-bash.txt) documents
the protocol. Zero tool: fast sync returns directly; the watchdog/kill
137 timeout is gone — a command that runs for hours is a background
slot, not a killed turn.

Laws (80): bg_wrapper_script byte-for-byte against bg_wrap, defaults
and clamps of bg_after, exec_of_background_maps_ok, and normal exit
codes still fail. Verified end-to-end with dbg binaries: sync ~0.04s
and slot cleaned; handoff at 2s with real pid; .rc holds the true code
(0, 143 after kill); stdin via fifo reaches the reader; listing = ls.
Suite byte-identical to baseline; PROOF green.

## 2026-09-26 — /reload: the self-improvement loop

Run the harness on its own next version without losing the session:

  /reload  (typed in the TUI, between turns)

Server side (Bend): /reload maps to the raw word "reload" in
core/commands.bend. Both REPLs grew a Flow return for their loops
(Go keeps serving, Stop unwinds to main): the connection loop, the
accept loop and main now return Flow instead of looping forever. On
reload the plan is gated by S.reload_allowed (core/session.bend):
turn, pending, queued and notifications must all be empty — exactly
the state the checkpoint serializes (ADR 0002), so the restart cannot
lose session state. Accepted: ack lines, checkpoint, socket close,
"reload-exit" marker on stdout (the log), process exits 0. Refused
(a stalled turn, a queued message, a held notification): a note, and
the session keeps serving. Gating is between turns structurally too:
the REPL only reads commands when no turn is running.

Parent side (rust/harness): the run loop became spawn -> wait banner
-> TUI, and when the TUI returns, the child state decides: exited
with the reload-exit marker and code 0 -> recompile the checked-out
source (runtime/repl-live.bend / runtime/repl.bend) into the REPL
binary, set BEND_CONTINUE=1, respawn on the same port, reconnect the
TUI (the fresh process restores the checkpoint and greets with the
message count). Compile failure: keep the previous binary, warn on
stderr, respawn anyway — the session survives a broken edit. Any
other child death: report and die with it. Child alive: the user
quit the TUI; die together (the old invariant). Reloads are capped
at 10 in a row.

TUI (rust/tui): the event drain treats a channel Disconnected as a
stop (the old UI stayed "connected" over a dead socket); /reload
joins the slash popup. Background command slots live on disk keyed
by port: detached processes survive the reload and stay reachable
from the new process.

Laws (86): slash_reload_maps, reload_raw_word,
reload_allowed_when_fresh, reload_refused_while_turn,
reload_refused_with_queue, reload_refused_with_notif.

Verified: nc on repl-scripted (acks, clean exit 0, reload-exit
marker, checkpoint, then BEND_CONTINUE restore with "session
restored: 2 messages" and a working turn); full pty loop through
bend-harness --scripted (reload -> recompile -> respawn ->
reconnect -> turn on the restored session, exit 0); the live path
too (reload -> recompile of repl-live -> respawn with MCP bootstrap
banner -> reconnect). Suite byte-identical; PROOF green; rust 0
warnings.

## 2026-09-26 — The background battery: 25 laws + an 18-check e2e suite

The battery found and fixed three real bugs before they could bite an
agent; every contract line is now a pinned, law-checked pure def.

Laws (111 total). The contract generation moved from inline strings in
bg_wrap to pure defs — bg_dir_of, bg_slot, bg_headline, bg_tail_cmd,
bg_stdin_cmd, bg_status_cmd, bg_kill_cmd, bg_poll_iters, plus the two
vanished-dir guards and bg_script_hash. LAWS now pin, byte for byte
and line by line: the wrapper script; the per-port bg dir; the five
slot files; ten probes per window second; every contract line as a
COMPLETE command on a CONCRETE path; the fifo held RDWR; the detached
runner writing the TRUE exit code; output as a file, never a pipe;
the wrapper NEVER killing (only kill -0 probes); the slot cleanup; the
199 sentinel; both vanished-dir guards; exec_of on success, failure
with code, garbage status, missing newline.

test-bg.bend (new, top level) runs REAL commands through bash_exec
under BEND_BG_ROOT=/tmp/bgtest BEND_BG_AFTER=2 (rm -rf the root
first — a command may never clean the bg dir itself, s14 proves why):
sync output/exit-0/exit-code/stderr, slot cleanup, handoff-as-ok, the
concrete contract (headline/tail/stdin exact, status/kill with the
REAL pid), live pid, kill -> rc 143, the fifo feeding a reader, tail
empty-then-filled, two parallel slots both killed, ls listing, and
rm -rf of the bg dir failing fast instead of hanging.

Bugs the battery caught and fixed:
1. rm -rf of the bg dir hung the tool forever (both waits were
   unbounded). Both waits now carry a dir guard: fast failure with
   the dir name, never a hang.
2. The wrapper script lived at a FIXED path (/tmp/bend-sh-<port>.sh):
   an orphaned wrapper (a process that died mid-command, sleeping in
   its poll loop) would wake up reading the NEXT command's bytes and
   execute garbage. The file is now named by a hash of its own
   content (positional weighted sum, fuel-first so the checker can
   normalize it): different commands never share a file, and rewrites
   of the same command are byte-identical, which an orphan survives.
3. sh-mode echo INTERPRETS backslash escapes: the contract's stdin
   line arrived split in two with its \n eaten — an agent copying it
   would have deadlocked a fifo reader (printf without a newline; read
   waits for one). The contract now prints with printf '%s\n' "..."
   whose %s argument is verbatim.

Checker notes: laws on bg_script_hash must use SHORT literals (a
6-char string overflows the checker's machine stack while
normalizing; "ab" checks in 2s) and the hash is fuel-first recursion
(idiom of escape_nl.go) — without fuel the checker diverges.

Gates: PROOF 111 laws green in ~2s; suite byte-identical; battery
18/18 twice in a row, zero orphaned processes; rust 0 warnings.

## 2026-09-26 — self.reload() / self.compact(): the agent drives its own harness

The sandbox now exposes the harness commands as callable tools — a
free identifier in run_typescript, exactly like self.sleep:

  const r = self.reload(); return r;   // restart on the latest code
  self.compact(); ...                  // compact the conversation

Both only make sense BETWEEN turns, so the tool result is an ACK the
model reads ("reload: scheduled - ..."), and the request executes at
the turn boundary: the REPL scans the finished turn's tool results
(deferred_of, pure, law-pinned; a user message resets the segment, so
an old request can never fire twice), then compact first (the
checkpoint carries the compacted session into the restarted process),
then reload through the exact /reload path (gate, acks, reload-exit
marker, parent recompile + respawn).

The live catalog carries both tools after the four parity tools, with
descriptions that say what they do and WHEN they act ("end of the
current turn"). catalog_short keeps the scripted scenarios
byte-identical.

Two execution paths, one protocol: a top-level tool call puts the ack
straight in the tool result (history). From inside a V8 program the
ack is the subtool's result, invisible to the scan — so exec_program
threads a Def accumulator through its loop and SUFFIXES the acks to
the program's final value: the model reads its own requests back, and
the markers reach the history whatever the program returns. In the
scripted in-house interpreter (test-only) the Core owns the program
state; there the request surfaces when the program returns the ack
(`return r;`), which the natural usage does.

Laws (128): the acks verbatim, the routing table (self.sleep stays a
timer), the markers riding the acks, the deferred scan (reads the
finished turn, resets at user messages, both requests, empty history),
the catalog (6 live tools, parity 4, names at 4/5, descriptions say
what and when), and the ack suffix (empty when none, carries reload,
carries both).

Verified end to end on repl-scripted over nc: self.compact() in a
sandbox program -> turn -> compaction_started/done; self.reload() ->
turn -> reload acks -> exit 0 with the reload-exit marker (the parent
then recompiles and respawns). The V8 live path verified through
exec_program directly: a program that files self.reload() and returns
a constant still yields a result suffixed with the ack.

## 2026-09-26 — Merge reconciliation: slot recycling meets the battery

The merge of self-reload-compact with the parallel "stuff" commit
mixed two wrapper evolutions. Reconciled deliberately:

- KEPT from "stuff": the runner frees a finished slot (rm pid/cmd/in;
  out/rc stay readable) — bounded dir growth, slot recycling; and the
  boot cleanup of stale wrapper files (bash_cleanup in repl-live).
- RESTORED from the battery: the fail-fast vanished-dir setup guard.
  The mkdir-recreate variant has an infinite-loop window exactly in
  the rm -rf-as-command case (the pid file can be deleted before the
  runner writes it; the recreated dir never sees it); the
  between-commands case "stuff" targets is already covered by the
  wrapper's own mkdir -p.

The recycling exposed two real bugs, both fixed and law-pinned:
1. The ephemeral pid race: for a fast command the runner writes and
   REMOVES the pid within milliseconds; the wrapper's pid-wait could
   wait forever for a file that came and went. The wait now also
   breaks on an existing rc (bg_wrapper_pid_wait_breaks_on_rc).
2. Shell noise: the same race made `c=$(cat pid)` leak "No such file"
   into fast commands' output. Now 2>/dev/null; s01 asserts clean
   output.

The battery was retaught the recycling semantics: slots RECYCLE, so
evidence (out/rc) must be read with IO.sleep waits (a bash_exec wait
recycles the freed slot and destroys it) and immediately after the
freeing event, before any other bash_exec. 18/18 twice; PROOF green
(131 laws); demo byte-identical; the "stuff" dbg tests still pass.

## 2026-09-26 — Steering commits into the history (vibe_sdk parity)

The user was right: steering did not really work. The Core HELD the
steered text in the turn and projected it into the next model call's
input, but NEVER committed it into the durable history — after that one
call the message vanished from every later projection, from the
checkpoint, and from what compaction could preserve. vibe_sdk commits
it (state.context.extend(steering)) and also at finish_turn.

Fixed, law-pinned (134 laws):
- dispatch now commits the injected steering and notifications into
  the session history at the model-safe boundary (DRes carries the
  history; dispatch_result applies it).
- finish_turn (vibe_sdk finish_turn): a turn that dies before its next
  model call — interrupt, provider failure, iteration cap, failed
  compaction, or a final completion — commits what the turn was still
  holding. No Steered observation: the model never received it.
- arrival order: add_steer/add_notif append (vibe_sdk pushes to a
  Vec); multiple steers inject in the order they arrived, not
  reversed.
- an in-turn auto-compaction no longer consumes the steering into the
  compaction request: the turn carries it (vibe_sdk keeps it pending)
  and the first agent call after the compaction commits it.
- a null-iteration retry keeps the active turn's pending steering.

Laws: steering_lands_in_history (a full say/steer/completion/tool
chain), steering_survives_interrupt, steering_survives_fail,
steering_arrival_order — via apply chains over a concrete cfg (a
symbolic threshold blocks the dispatch's fits comparison and the chain
cannot normalize).

Architectural note: the live REPL reads commands only between turns,
so a mid-turn Enter-steer is applied right after the turn ends and
starts the next turn (the Core's mid-turn defer path is exercised by
scenarios that batch commands, and by the laws). The commit fix
matters there too: everything the turn held lands in the history.

## 2026-09-26 — Toolchain incident: bend 2.0.29 breaks the hub packages

Mid-session the compiler was updated to 2.0.29 (bend update, another
agent): it rejects numeric dotted def names (resolve.3 in the hub DNS
package) and its Base removed Nat.read.fit (the hub JSON package calls
it). Every build broke. Restored the coherent 2.0.27 toolchain
(binary + bend2 Base + effs from the v2.0.27 release tarball; the hub
cache left pristine). PIN: build with bend 2.0.27 until the hub
packages are republished for 2.0.29. (2.0.29 kept at /tmp/bend-2.0.29.bak.)

## 2026-09-27 — The live agent QA'd its own bash tool; 9 root causes fixed

New: bend_client.py — drive a live harness session programmatically
(launch repl-live on a private port, speak the line protocol, read the
obs stream until idle; say/steer/notify/compact; --continue resumes
the latest session). The QA mission ran the LIVE agent (GLM) against
its own bash tool with BEND_BG_AFTER=3; it returned a 15-bug report
(four CRITICAL) with reproductions and benchmarks. Fixes, all
law-pinned (141 laws):

- Multi-line commands run as written: Api.flatten is gone from
  bash_exec (it silently joined lines with spaces — the heredoc and
  the runner subshell both carry real newlines).
- The heredoc sentinel derives from the command's own hash
  (bg_sentinel); a command can no longer truncate the wrapper.
- The handoff is detected by a first-line marker (BEND-BG-HANDOFF-…),
  never by an exit code: a command that REALLY exits 199 now reports
  "exit 199" (once silently swallowed as a handoff). The wrapper exits
  0 on handoff; exec_of strips the marker and keeps the contract.
- Slot allocation is an atomic mkdir CAS ($i.slot): concurrent
  wrappers can no longer collide on the same id (once: 4 parallel
  calls took ids [0,1,0,1] and overwrote each other).
- The runner is ownership-checked: it writes .rc and frees the slot
  only if its pid still owns it — a recycled slot cannot be corrupted
  by a dying runner (that also fixed the "rm -rf the bg dir poisons
  the next command" cascade).
- The runner caps file writes: ulimit -f 102400 (50MiB) — an
  output-looping background command dies at the cap instead of
  filling the disk (the QA run grew a .out to 6.4GB; the host had
  178MB free at the worst moment).
- Both exit paths delete the wrapper script (rm -f "$0"): no more
  +1 leaked script per call (1,037 had accumulated).
- The contract's stdin line warns that a stdin-reading background
  command waits for input until fed or killed (documented semantics —
  the fifo never EOFs while the harness holds it RDWR).
- The truncation marker counts discarded chars; the poll loop probes
  at 20ms for the first 5 iterations (a trivial call costs ~35ms of
  overhead instead of ~150ms), then 100ms.

Battery: 22 checks green (added: multi-line intact, real 199 fails,
ulimit cap, no script leak). Demo byte-identical; PROOF green.

## 2026-09-27 — MCP connector parity with the vibe CLI

The bootstrap now hits the same endpoint with the same params as the
vibe CLI's ConnectorRegistry: supports_mcp=true&
include_auth_actionable_connectors=true&builtin_connectors=web_search
(extracted into bootstrap_url, law-pinned). The web_search builtin
rides along as an MCP-callable connector (5 tools: web_search,
open_url, news_search, weather_search, finance_search).

Published aliases are now clean identifiers (vibe's _normalize_name):
anything outside [a-zA-Z0-9_-] flattens to "_", the edges strip, ""
becomes "unnamed" — vibe_sdk requires a TypeScript identifier for
tools.<alias>.<function>. Colliding normalized aliases dedupe with
_2, _3 suffixes (vibe's disambiguation), computed against the aliases
BEFORE the current connector (the first version consed the current
alias into its own seen list: every alias got _2; caught live).

The flow, end to end, verified live: bootstrap at startup writes the
index (only ready connectors index their tools); search_tool_functions
(best_match) ranks over the merged catalog+index (360 tools), details
returns the input schema; run_typescript calls <connector>.<tool> and
the runtime resolves the connector id from the index and drives the
MCP session on /v1/connectors-gateway/<cid>/mcp (initialize,
notifications/initialized, tools/call). Nothing enters the context
window except what the agent explicitly searches for and reads.

Laws (144): mcp_alias_normalizes, mcp_alias_dedupes,
mcp_bootstrap_url_parity. Live check: 14 connectors / 360 tools
indexed; best_match found web_search.web_search; details returned the
schema; github_app.get_me returned gvergnaud.

## 2026-09-27 — feed breathing + solid user panel + prompt spacing

The TUI was dense: blocks touched each other at the message/message
transition, notices glued to the block above them, and the prompt sat
flush against the status row. Three changes in rust/tui/src/lib.rs:

- build_rows now delegates the blank-line decision to wants_gap_before
  (pure): one blank row whenever the content kind switches (message /
  tool block / notice), computed against the previous VISIBLE event.
  Two exceptions stay: a reply never detaches from its thinking
  section, and the first event of the feed starts flush at the top.
- the user block paints its panel background the full column
  (pad_line_bg sets the line base style to bg PANEL, then fills the
  remainder), so a user message reads as a solid OpenCode-style panel
  instead of a 3-cell strip under the bar.
- draw: one blank row between the feed and the status row, one between
  the prompt and the hint row, and one blank line inside the prompt
  between the typed text and the meta row (input_h = composer + 4).

Also: the standalone bend-tui binary entry (rust/tui/src/main.rs) had
not followed the run() signature (session_id) and no longer compiled;
it now parses --session and passes it through.

Verified with a pyte pty run against a fake REPL (two turns, tool
block, markdown, code fence): the user panel spans the full width
(bg 141414 x0..x108), a blank row at every transition, the meta row
one line under the input, and the hint row clear of the prompt. Note
for future pty checks: start the child with start_new_session=True —
crossterm sizes the terminal from /dev/tty (the controlling terminal),
not from stdin, so a plain Popen measures the WRONG pty.

## 2026-09-27 — glyph vocabulary + feed margins + roomy composer

The history told its story in words (tool "ok", turn "terminé",
warning "!"), the feed ran edge-to-edge, and the composer sat one row
under the status line. Changes in rust/tui/src/lib.rs:

- a glyph vocabulary now carries status, not text: ✦ reasoning
  (collapsed: duration only, "thought for Ns" is gone), ✓ ok (green),
  ✗ fail (red), ▲ warning, · notice, ⟳ compaction, ≡ summary, ↳
  preview/sub-result. Tool lines read "✓ bash 0.2s · <args>" — the
  ok/échec words are gone; the state is the glyph plus its color.
- an expanded thinking section shows its reasoning under a faint rail
  (│, FAINT 0x4a4a4a — dimmer than textMuted) instead of a bare
  3-space indent.
- the feed keeps one column of margin on each edge: the text starts at
  x+1 and stops one column short of the scrollbar (feed_w = width - 3),
  so history never touches a screen edge and the scrollbar gets its own
  gutter.
- the composer is fully separated from the history: blank row, status
  row, blank row, prompt. The prompt block itself gained air
  (Padding 3,2,2,1 — two blank rows above the typed text), and its
  inner width is now computed exactly (width - 6, border + paddings) so
  the composer wrap and the row estimate agree.
- Ev::TurnDone dropped its String payload (always "tour terminé"; the
  other outcomes already map to Err/Warn) — it renders as "└─ ✓".
- the hint row speaks glyphs and fits 100 cols ("⏎ envoyer · Maj+⏎
  nouvelle ligne · / commandes · Ctrl+T raisonnement · Ctrl+C
  quitter"); the old one truncated at width 100.

Follow-ups in the same pass (same session): the prompt meta row is
glyph-driven too — "◆ bend · zai-glm-5-3 · ⇧⏎ ligne · ●", the
connection is a quiet ● when up and a loud red "○ déconnecté" when
down (the Ctrl+J fallback note moved to the hint row, which still
fits 100 cols); /help documents the vocabulary (one line, 93 cols);
debug mode brackets a turn with " ── tour ──…" and " └─ ✓".

Verified under tmux (100x40) against a fake wire server: thinking ✦ +
rail via Ctrl+T, ✓ green / ✗ red tool lines, ↳ previews, the pinned
"↓ Bas (End)" bar, PageUp/End re-stick, composer wrap flush against
the ┃ border, Ctrl+J newline with the "⏎ envoyer · ⇧⏎ ligne" meta
variant, and the hint row intact at the right edge.

## 2026-09-28 — Session debug log: every error leaves a trail

Every session now keeps `~/.bend-harness/sessions/<id>.debug/` next to
its checkpoint (the parent creates it and passes `BEND_DEBUG_DIR`; the
scripted suites never set it, so they stay byte-identical).

```
<id>.debug/
├── events.jsonl          one JSON object per line, src = parent | repl
├── clock                 the REPL's clock offset (epoch ms - IO.now())
├── req-<call>.json       request body of a call that failed (once per call)
├── reply-<call>-<n>.txt  raw reply / SSE text of failed attempt n (256 KB max)
└── crash-<ts>/           one per REPL crash
    ├── stderr.txt        the dead generation's stderr ("bend: ...")
    ├── stdout.txt        its stdout (banner, mcp bootstrap) - overwritten on respawn
    ├── session.txt       the checkpoint it restarts from
    └── env.txt           the BEND_* environment
```

Events:
- parent (rust/harness/src/debuglog.rs): `harness_start` (args, repl
  binary + mtime, port), `repl_spawn` (generation, pid, cause
  start|reload|crash), `repl_ready`, `repl_reload`,
  `reload_recompile_failed`, `repl_crash` (exit status, last `bend:`
  line, uptime, crashes in a row, snapshot path), `repl_start_failed`,
  `crash_loop_stop`, `harness_exit` (+ TUI error), `panic` (message,
  location, backtrace; the hook chains to ratatui's restore).
- REPL: `repl_start` (clock anchor), `sent` (every obs line and tool
  annotation sent to the client, 2000 chars max: the breadcrumbs before
  a crash), `provider_attempt_failed` (call, attempt, verdict
  retry|fail|error_reply, why, status, retry-after, model, url, sizes,
  artifact names), `provider_recovered`.

`IO.now()` is monotonic (ms since boot) and Bend has no wall clock: at
startup the REPL probes epoch ms once (`perl` Time::HiRes, `date +%s`
fallback), stores the offset in `clock`, and stamps events mono +
offset, so both sources sort on one time scale. 11 laws pin the line
format, the breadcrumb filter, the incident policy, the clip and the
clock (315 total). events.jsonl rotates to events.1.jsonl past 8 MB at
startup.

Investigating: `tail -50 <id>.debug/events.jsonl` (or `jq -c 'select(.kind
!= "sent")'` for the incidents only), then the artifacts it names.
