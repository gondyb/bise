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

- `core/types.bend` — roles, messages, commands, actions, observations, config, session.
- `core/estimate.bend` — deterministic model-input token estimate.
- `core/history.bend` — history queries: latest non-injected user, paired drop-oldest, middle-truncate.
- `core/compaction.bend` — ADR 0011: projection fit, summary validation, replacement.
- `core/discovery.bend` — search_tool_functions.
- `core/program.bend` — D1: programmatic tool calling in Bend (parser + replay interpreter).
- `core/session.bend` — `apply`, the step-protocol state machine.
- `core/checkpoint.bend` — export/restore.
- `runtime/model.bend` — scripted provider adapter.
- `runtime/tools.bend` — tool registry + executors.
- `runtime/main.bend` — scenario runner: applies commands, executes actions, prints observations.
- `scenarios/*.bend` — acceptance scenarios (core tested through the step protocol, per ADR 0006).
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
or the terminal UI: `node --experimental-strip-types repl-ui/repl.ts`
(zero-dependency TypeScript client over the same socket; colors, prompt,
history; the session still lives in the harness, not in the UI).

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
- The TS versions remain: live/bridge.ts and repl-tui/ (Ink) as
  alternates; the Rust pair is the reference.
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
  tool_result #<id> <ok|fail> : <out>    (flattened, capped 200)
exec_program emits one line per sub-call:
  subtool <name> <ok|fail> : <out>
ESleep carries (ms, id, secs) now so self.sleep annotates too.

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
