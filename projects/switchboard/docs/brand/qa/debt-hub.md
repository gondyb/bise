# debt-hub · what makes the Switchboard hub hard to change

Read-only audit of HEAD eaddea0 (private worktree), with the
`codebase-design` and `code-quality` skills. Scope: `rust/switchboard`
(~7.9k lines), `hub/*.bend` (~4.4k lines), `sb-core`, the `sb` CLI,
`projects/switchboard/tests`. It builds on BISE-88
(`docs/brand/qa/code-quality.md`, F1–F10) and does not repeat it. F7
(tmux flakes) is covered here only as test design. F8/F9 are TUI
findings, left to debt-tui. Test speed is left to loop-speed-2.

The short version: each part is clean on its own (little dead code, small
pure helpers, good scenario tests). The cost is at the **seams between
the parts**. Three JSON/text protocols (Rust↔Bend, hub→TUI, hub lines)
have no single owner, so a small feature is written 5–10 times in two
languages, and the Rust side reads each field with a silent default.

## Fix now (cheap, ≤ 2 h each)

### 1. The journal replay drops the lines it cannot read, with no log (risk: lost state after a rollback)
`daemon.rs::run` reads the journal with
`.lines().filter_map(|l| serde_json::from_str(l).ok())` into the Rust
`Event` enum, then sends each event to sb-core. A line that the Rust
`Event` does not know is skipped, and nothing logs it. This happens
after a `/version` rollback to an older hub when the newer hub wrote a
new event kind. It also happens after a half-written last line. So the
state that sb-core rebuilds can miss events, and nobody sees it. The Rust
`Event` (model.rs, ~75 lines) has no other job. Its only role is a
round trip: sb-core's `journal` effect → `Event` → `to_string` → file,
and file → `Event` → JSON → sb-core. The decoder that matters is
`hub/codec.bend` (`C.ev_of`).
- **Fix:** keep journal lines as `serde_json::Value` on both paths. Log
  and count the lines that do not parse (at least `journal: N unreadable
  lines`). `Hub::replay(&[Value])`.
- **Cost:** 1–1.5 h. **Gain:** a real data-loss path closes, and one of
  the three copies of the event model goes away (Bend `Ev`, Rust
  `Event`, codec).

### 2. The `sb` usage lives in two texts that already drift
`cli.rs::USAGE` and `prompts.rs` (the agent system prompt) each
describe every command. They already differ: the prompt says
`--reply-to <id>` where the CLI says `m_<n>`. Also, `sb history`,
`sb version` and `sb restart` are in one text only. An agent learns the
CLI from the prompt, so any drift gives wrong calls.
- **Fix:** one table `(command, args, one-line doc, who)` in `cli.rs`.
  USAGE and the prompt list are both built from it.
- **Cost:** 1 h. **Gain:** a flag change is made in one place.

### 3. tmux tests: one shared "session" helper instead of 12 copies
There are 16 `tui_*_tmux.py` files. 12 of them set the module global
`tui_tmux.S = S` (hidden shared state). 17 files copy the same teardown:
`stop_hub`, `sleep(1)`, `SB_KEEP` on failure, `E.close()`. 9 files
write their own `while time.time() - t0 < N` poll loop, for example
`tui_waits_tmux.py`: "answer y" or "@t1 archived". These hand-written
loops are where the F4/F7-style flakes live.
- **Fix:** `with tui_session(cols, rows, env=…) as t:` (t.keys, t.typed,
  t.wait, t.wait_any([...]) → which one matched), with the teardown
  inside it and no module global. Move the tests one by one.
- **Cost:** 2–3 h. First coordinate with loop-speed-2, who is editing the
  tests. **Gain:** a new tmux test is about 20 lines of intent. A flake
  fix in `wait_any` fixes every test.

### 4. Small dead code and test hooks in production
- `hub/core.bend:370 set_turn` has no caller.
- `IForceRun` / `force_run` is a test-only input in the production
  dispatch (`core.bend:2317`, `in_tag`). This is acceptable, but put a
  `# tests only` note in `in_tag` too, or move the hook to a test
  entry point.
- **Cost:** 15 min (next sb-core rebuild).

## Bigger refactors (ranked by gain)

### 5. One `sb` command = ~10 edits in 4 files and 2 languages (shallow round trip)
The path of one command:
`cli.rs::build` (argv → JSON) → daemon → `AgentReq::from_json`
(core.rs, 110 lines: JSON → enum, validation) → `Hub::agent_req`
(core.rs, 80 lines: enum → **almost the same JSON**) → Bend `cmd_tag`
(a `Bool.pick` string chain) → `Cmd` type → `req.go` / `req.main.go` →
the handler, which reads the fields again (`C.get_s(q, "to")`). Add
`cli.rs::render`, USAGE, the prompt, and core_tests.
Evidence: 0d40c77 (sb close/rename/restore/isolate) touched **10 files,
+320 lines**: core.bend, cli.rs, core.rs, core_tests, daemon.rs,
prompts.rs, e2e.py, IMPLEMENTATION.md, the RFC, sb-core. Today
`isolate` appears in 7 source files. The deletion test says `AgentReq`
earns its keep only for `list` / `tasks` (answered in Rust) and for 4
enum validations. Everything else passes through.
The input side (`Input` enum → `json!({"t": …})` → Bend `In` + `in_tag`
+ `dispatch.go`) has the same shape: 5 places per input.
- **Fix:** forward the CLI's `req` JSON to sb-core as it is. Keep in
  Rust only what Rust answers (`list`, `tasks`, `inspect`, `history`,
  `version`). Move the validation (modes, statuses, report kinds) into
  Bend next to its decision, or into `cli.rs`, the edge. Replace the
  `Bool.pick` string chains (`cmd_tag`, `in_tag`, `main.bend` tags)
  with one match on the string.
- **Cost:** 4–6 h + PROOF + an sb-core rebuild. **Gain:** a new command
  = cli.rs (+ its table row, see #2) + its Bend handler + a test.

### 6. Three protocols with no owner, all read with silent defaults
- **Bend → Rust view.** `hub/view.bend` writes about 30 JSON keys.
  `core.rs::load_view` reads them one by one with
  `unwrap_or("")` / `unwrap_or(false)` into the mirror `model::State`.
  If a key is renamed or misspelled on one side, the value becomes ""
  or false: no error, no failing test.
  - Bend field reads: 101. Rust `jstr` / `as_*` in core.rs: 89.
  - Evidence: ecb4bec ("waits {name}") touched main.bend, view.bend,
    core.rs, model.rs, core_tests, sb-core, then the TUI.
- **Hub → TUI snapshot.** `core.rs::snapshot` builds `json!({...})`.
  `rust/tui/src/sb.rs::apply_state` reads the same keys by hand, again
  with defaults.
- **Hub lines (`sb msg : from → to m_<n> : text`).**
  - Formatted: in Bend (`feed_main`, …) and in Rust (`core.rs::line`,
    `join_fields`, `FIELD_SEP`).
  - Parsed three times: `tui::sb::parse_hub_line`,
    `transcript::readable` (`strip_prefix("sb msg : ")`) and
    `cli::render`.
  - Evidence: BISE-110 (eaddea0 + the TUI commit) changed one field and
    touched core.bend, cli.rs, core_tests and the TUI parser, plus an
    "old lines still parse" compatibility branch.
- **Fix:** a `switchboard::protocol` module that owns the types.
  - A `HubLine` enum with `format` / `parse`, used by the TUI,
    transcript, cli and the tests.
  - A `Snapshot` struct (serde, both ends).
  - A `View` struct with `#[derive(Deserialize)]` and no silent
    defaults on required keys. A missing key panics in the tests, not
    in the user's panel.
  - Keep the Bend side as it is, but pin it: a core_tests test that
    decodes every `view` and `line` sb-core emits in the scenarios with
    the strict types.
- **Cost:** 6–8 h (the snapshot part touches rust/tui, so do it after
  the TUI work settles). **Gain:** a field rename breaks the build or a
  test, not the UI. core_tests can assert on `HubLine::Msg{..}` instead
  of strings (see #9).

### 7. `daemon::Shell`: 20 mutable fields, of which 10 describe one REPL
`repls, gens, next_gen, pids, bins, ports, switching, switch_spawned,
restored, resume_turn` are 10 maps and sets keyed by agent dir, and
together they describe **one REPL's lifecycle**. The rules ("an exit of
an old generation is ignored", "switch_spawned ⊆ switching", "restored
lines are not news") are spread over `spawn_on` (~120 lines),
`on_repl_line`, the `Msg::ReplGone` arm of `run` (~310 lines),
`switch_idle_repls` and `versions.rs`. Nothing ties them together, so
each restart/switch fix must find every map. Version switching churned:
17 commits in 4 weeks on switch.rs / versions.rs / versions.sh.
- **Fix:** `struct ReplSlot { gen, pid, port, bin, conn: Option<Repl>,
  phase: Running|Switching{pending: Vec<String>, spawned: bool}|…,
  restored, resume }` in one `BTreeMap<String, ReplSlot>`, with
  transition methods (`on_spawned`, `on_exit(gen)`, `on_line(gen)`)
  that are pure and unit-tested. Also move `run()`'s startup (journal
  read, buffer rebuild) out of the event loop into functions.
- **Cost:** 4–6 h + e2e `t_restart_keeps_everything` + the version tmux
  test. **Gain:** restart/switch bugs become local, and the lifecycle
  gets its first unit tests (today only e2e covers it).

### 8. `hub/core.bend`: 2401 lines, 321 defs, one file
Sends, waits, cards, drops/restores/isolates/renames, routing, pump,
tick and the dispatch all live in one file, and each feature touches it
(it is in 12 of the last 20 hub commits). A recurring trap: eager
`Bool.pick` runs both branches, which gave 4 perf fixes in a month
(6884f99, 285fe29, 1450540, ccc8a67). The string-tag chains are still
`Bool.pick`.
- **Fix:** split along the sections that already exist
  (`# ---- waits ----` …) into `hub/send.bend`, `waits.bend`,
  `cards.bend`, `tasks.bend`, `dispatch.bend`. It is a move, no logic
  change, checked by PROOF. Write down the `Bool.pick` vs `pick_lazy`
  rule in one line at the top of `cx.bend`.
- **Cost:** 3–4 h (a move, then PROOF + a rebuild). **Gain:** smaller
  review surface and fewer merge conflicts on the hottest file.

### 9. core_tests pin exact text
Of 199 asserts, 46 are `contains("…")` and many match the whole line,
e.g. `starts_with("sb msg : main → docs m_") && ends_with(" : use v2")`,
or `"sb answered : docs : and the title? : keep it : "`. Any change to
a line's format edits N tests (BISE-110, BISE-80/81 "task" → "agent":
core_tests changed in both). The scenario design itself is good: inputs
in, effects out, a fake `Env`.
- **Fix:** after #6, assert on parsed `HubLine` values. Keep one golden
  test per line kind for the exact text.
- **Cost:** 2 h after #6. **Gain:** a wording change = 1 test, not 10.

### 10. `sb-core` is a 2.2 MB binary in git, rebuilt by hand (decide with main)
Each hub change commits a new binary: 20 commits so far, and a binary
cannot be merged. The gate must rebuild it (1–2 min) or copy the main
tree's copy when `hub/` did not change. A forgotten rebuild means the
Rust tests run an old core; F2 in BISE-88 was the same bug class.
`versions.sh` already has the right mechanism: a cache keyed by the hash
of `hub/`, `$BUILD/cache/sb-core-$hc`.
- **Fix:** one `sb-core-for <tree>` helper with that content-hash cache,
  used by gate.sh, run_all.sh, `core::core_bin()` (dev) and versions.sh.
  Stop tracking `sb-core`, after checking that packaging/install.sh and
  build-dist.sh build it.
- **Cost:** 2–3 h + packaging check. **Gain:** no binary in diffs, no
  stale core, and a hub commit is source only. This touches
  loop-speed-2's area (gate.sh), so ask them first.

## Suggested order
1. #1 journal (a real risk, 1 h) → #4 dead code (at the next rebuild) →
   #2 one usage table (1 h).
2. #3 tmux session helper, once loop-speed-2 is done with the tests.
3. #6 protocol types (hub lines first, then view, then snapshot with
   debt-tui) → #9 tests on parsed lines.
4. #5 pass-through `req` (much easier once #6 exists).
5. #7 ReplSlot, then #8 core.bend split (a pure move, any quiet day).
6. #10 sb-core cache: a decision for main and the user (packaging).

Total: fix-now ≈ 5–6 h; refactors ≈ 25–30 h, which can be done one at a
time, each with its own gate.

## Checked, fine
- No unused `pub fn` in `rust/switchboard`. Only one dead Bend def.
- `board.rs`, `router.rs`, `transcript.rs`, `wire.rs`, `worktree.rs`:
  pure or edge-only, small, tested.
- The `Env` trait is a real seam (git in the daemon, a fake in the
  tests), and the scenario tests use it well.
- A newer sb-core's unknown effects are skipped, with a log
  (`client_effect`).
