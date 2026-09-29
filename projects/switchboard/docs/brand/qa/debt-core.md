# debt-core · what makes the harness core hard to change

Scope: `core/*.bend` (4.9k lines), `runtime/*.bend` (7.2k), `PROOF.bend` /
`LAWS.bend` (6.5k), `rust/harness|jsrt|plugins|images` (4.2k), the root
scripts. Read at HEAD eaddea0 (private worktree), no edits.
Builds on BISE-88 (`docs/brand/qa/code-quality.md`): F1–F6 are fixed, F7–F9
are TUI (debt-tui's scope). **F10 is fixed** (`versions.sh` now has
`trap cleanup EXIT`, 0 `sb-build` worktrees left).

## Short verdict

The core is in good health for its size. Dead code is gone (a script
over every `def` in core/runtime finds 1 unused function, `Api.is_anthropic`,
and 4 used only by laws). The laws gate is fast (`bend PROOF.bend`: 17 s,
354 laws). The pure/IO split is real and followed. What makes changes
expensive is not mess, it is **three leftovers from earlier designs** and
**facts that have no single home** (tool names, env vars, build rules).

## Fix now, cheap (≤ 2 h each)

### 1. `run.sh` does not rebuild what changed (~1 h, high gain)

- The Rust staleness check looks at `rust/harness/src rust/tui/src
  rust/switchboard/src` only. `bend-harness` also depends on
  `rust/plugins` and `rust/images` (via tui), and on every `Cargo.toml` /
  `Cargo.lock`. Edit `rust/images/src/lib.rs` → `./run.sh` runs the old
  binary.
- `bend-jsrt` is built only **when missing** (`[ ! -x … ]`), never when
  stale. Commit 4282501 (images) and 73efdf1 changed `rust/jsrt/src/main.rs`:
  a tree that already had a jsrt kept the old engine silently.
- This is the "stale binary" class that run.sh's own comment says already
  bit once.
- Fix: one list of watched paths per binary (`rust/` minus `target`, per
  crate, plus manifests), and the same `find -newer` rule for jsrt. Even
  simpler: always call `cargo build` (a no-op build is ~1 s) and keep the
  mtime rule only for the 1–2 min Bend compiles.

### 2. The jsrt path is hard-coded to a dev folder (~1 h)

`runtime/main.bend:206` spawns `rust/jsrt/target/debug/bend-jsrt`;
`runtime/plugins.bend:61` falls back to `rust/target/debug/bend-harness`.
To satisfy this, `release.sh:41-42` copies the **release** jsrt into a
`target/debug` folder of the bundle, and `versions.sh:106-114` hard-links
it into the same fake path. Three files know one path. Fix: the harness
(Rust) resolves the paths once and passes them as env (`BEND_JSRT_BIN`,
`BEND_HARNESS_BIN` already exists), the runtime reads one variable.

### 3. Tool facts repeated as string lists (~2 h)

The set "tools whose args are structured JSON" (`run_typescript`,
`search_tool_functions`) is written twice: `core/api.bend:1088`
(`args_json`) and `runtime/provider-pure.bend` (`is_structured`), plus a
third time as schema choice (`api.bend:456 tool_schema`). The internal
rename `node_program → run_typescript` is written twice:
`core/session.bend:102 result_name` and `runtime/remote.bend:50 wire_name`.
Fix: one `core/tools.bend` (pure) with `is_structured`, `schema_of`,
`model_name_of`; the three call sites import it. Laws keep passing (same
values). See #5 for the full version of this.

### 4. Root folder clutter (~30 min)

At repo root: `test-bash-hang.bend`, `test-bg.bend`, `test-details.bend`
(real test suites, **not run** by `run_all.sh`, which runs `PROOF` only),
`test-parity.py`, `dbg/`, `HUB-2.0.31-PATCHES.md` (notes for a Bend version
long gone), `PLAN.md`, 5 `tool-desc-*.txt` + 2 `prompt-*.txt`. A newcomer
cannot tell product from scratch. Fix: `tests/bend/` for the 3 suites
(and add them to `run_all.sh` or say in their header why not),
`prompts/` for the txt files (run.sh and `P.read_or` paths change),
delete or archive the 2.0.31 note.

## Bigger refactors (half a day to 2 days)

### 5. Adding a tool touches ~8 places (1 day, high gain)

Tool identity lives in strings spread over 13 files (`rg -l
run_typescript core runtime …`). To add a model tool today you touch:
`tools-pure.bend` catalog, a `tool-desc-*.txt`, `api.bend` schema +
`args_json`, `provider-pure.bend is_structured`, `main.bend tool_of` (the
top-level dispatcher, a 6-deep `Bool.pick` chain), `main.bend
exec_program.call.*` (a **second** dispatcher for calls from inside a
program, a chain of 7 `.pick` functions in a different order),
`main-pure.bend is_code_call`, and the laws. The two dispatchers can drift
(e.g. `self.sleep` exists only in the program one, on purpose, but
nothing says so in one place).
Fix: one tool table (name, schema kind, structured?, code-call?, where it
may be called: model / program / both) in a pure module, and one
dispatcher `exec(tool, args)` used by both paths. Deletion test: the 7
`.pick` helpers and 3 string lists disappear.

### 6. The line "wire" between runtime and provider JSON (1–1.5 days, high gain)

`runtime/remote.bend` says it itself: "A leftover of the removed HTTP
bridge". The runtime serializes the model input to `MODEL/TOOL/MSG/CALL`
text lines, then `core/api.bend` parses that text **back** in the same
process to build JSON; the reply does the same detour (`OK/CALL/END` →
`parse_reply`). Cost today: escaping in both directions (`escape_nl` is
used 11×, `unescape_nl` 9×, `wire_encode/decode` 9×), ~600 lines between
`remote.bend` and the parse half of `api.bend`, 40 laws pinning the text
format, and a class of bugs (a newline or `" : "` in content). Commit
9af2dc2 spent a whole CPU fix on rescanning this request string.
Fix: `Api.api_body` takes the typed `T.Msg` list + tool defs directly,
the provider reply becomes a `RemoteRes` directly. Keep the text form
only if something outside reads it (the debug dump: `BEND_WIRE_DUMP` can
dump the JSON instead). Do it after #5 (the tool table feeds the schema).

### 7. Two engines for `run_typescript` (decision first, then 1–2 days)

**Done (BISE-118, option (a)):** one engine, V8; `core/program.bend`, the rename and `result_name`/`wire_name` are gone.

Live sessions run V8 (`bend-jsrt`, via the `node_program` rename in
`main-pure.bend:68-90` and `main.bend:501`); scripted sessions and all
laws run `core/program.bend`, a 1341-line JS-lite interpreter (its `px`
function alone is 283 lines, the longest in the codebase). So the tests
check an engine users never run, and every `run_typescript` feature
(content blocks, images in 4282501, 73efdf1 parity) is done for V8 only,
widening the gap. The rename itself is the source of the duplicate
`result_name`/`wire_name` in #3.
Options: (a) scripted mode also uses jsrt, `program.bend` and its laws go
(−1.3k lines Bend, −~16 laws; the laws then pin the effect *protocol*,
not the interpreter); (b) keep it, but name it "scripted JS-lite" and stop
calling both `run_typescript`. Needs main/user to choose; (a) is simpler.

### 8. Env vars read everywhere, each with its own default (½ day)

17 runtime files call `IO.get_env` for 20 `BEND_*` variables.
`BEND_REPL_PORT` alone is read at 11 places in 6 files
(`main.bend:406,516`, `bash.bend:34,40,50`, `skills.bend:84`,
`plugins.bend:28,72`, `provider.bend:203`, `repl-core.bend:451`), and
`bash.bend:20` uses it as a hidden "am I live?" flag. `BEND_WORKDIR`,
`BEND_CONTINUE`, `BEND_BG_ROOT` are similar. A change of default or name
means a grep and hope. Fix: read the env once at startup into a
`RtEnv` record (next to `settings.bend`, which already does
env > file > default for 3 keys) and pass it down; the "live" flag
becomes a field, not a port check. Pure functions become testable with
any env.

### 9. Built binaries committed to git (decision + 1 h)

`repl-live` (2.9 MB), `repl-scripted` (2.8 MB), `sb-core` (2.2 MB) are
Mach-O files in the repo, rebuilt in 29 / 29 / 20 commits (14 commit
messages in 30 days say "rebuilt"). The pack is 34 MB mostly from them;
two agents who both touch `.bend` get a binary merge conflict; a commit
can carry a binary that does not match its sources ("rebuilt
unstripped" is a manual step). run.sh already rebuilds them from source
when stale. Fix: stop tracking them (`.gitignore`), let run.sh /
versions.sh build them (versions.sh already caches by hash). Risk: a
machine with no `bend` can't run — only if that case matters.

### 10. `rust/harness/src/main.rs::main` is 458 lines (½ day, low urgency)

`main` (lines 314–772) handles the `sb`, `sbd`, `sbswitch`, TUI, headless
and reload modes, crash notes and REPL restarts in one function. Fine
today (25 changes in 30 days, no bug found), but each new mode reopens
it. Fix: one function per mode, `main` becomes the match. Pure move.

## Checked, nothing to fix

- Dead code: 1 unused def (`Api.is_anthropic`, delete it), 4 law-only.
- `PROOF.bend`: 461 of its lines are `{==}` (proof by unfolding): cheap to
  keep. The 5 long hub proofs (up to 1352 lines, `cards_unique`) are
  debt-hub's scope.
- The Bend `.pick`/`.go` helper style (144 of 1451 defs) is how Bend
  branches on a value with linear args; it is noise, not debt. #5 removes
  the worst chains.
- `rust/plugins`, `rust/images`: small, own tests, clear seams. The image
  marker format is written in 3 places (core/image.bend, rust/images,
  rust/tui) but the Rust side shares `bend_images::marker`; only the Bend
  parser is separate, pinned by laws — acceptable.

## Suggested order

| order | item | cost | gain |
|---|---|---|---|
| 1 | #1 run.sh staleness (and jsrt) | 1 h | no more "old binary" sessions |
| 2 | #2 one jsrt/harness path, passed as env | 1 h | bundle/versions stop faking `target/debug` |
| 3 | #4 root cleanup, run the 3 Bend suites | 30 min | clear repo, suites actually gate |
| 4 | #3 then #5 tool table + one dispatcher | 2 h + 1 day | new tool = 2 places, not 8 |
| 5 | #8 one `RtEnv` | ½ day | env defaults in one place |
| 6 | #7 decide one `run_typescript` engine | decision + 1–2 days | tests check what users run, −1.3k lines |
| 7 | #6 remove the line wire | 1–1.5 days | −~600 lines, no escaping bugs, faster model calls |
| 8 | #9 untrack binaries | decision + 1 h | no binary conflicts, smaller repo |
| 9 | #10 split harness `main` | ½ day | only when a new mode lands |
