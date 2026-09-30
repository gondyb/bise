# BISE-88 · code quality pass — findings

Scope: `git diff 4282501..HEAD` in `rust/tui`, `rust/switchboard`,
`hub/*.bend` (55 files, +12.5k −1.4k lines), read with the
`code-quality` and `codebase-design` skills. Base: a60735a.

Budget (hard): ~1500 changed lines, 3 hours, one refactor per commit, no
behaviour or contract change (C1–C4, the hub line protocol, the journal),
bench not slower. Ranked by **risk × cost**: the cheap fixes of the
things that can hide a real bug come first.

| # | finding | risk | cost | status |
|---|---------|------|------|--------|
| F1 | `run_all.sh` hides Rust test results | high | ~15 lines | fixed ef2f8e1 |
| F2 | the tests inherit the agent's `SB_*` env (old sb-core) | high | ~15 lines | fixed 5b8b550 |
| F3 | `tui_version_tmux` depends on `~/.local/state`, starts a real build | medium | ~20 lines | fixed fd88a3a |
| F4 | `tui_composer_tmux` reads the reply from one snapshot | medium | ~3 lines | fixed 5be38a0 |
| F5 | dead code behind `allow(dead_code)` | low | ~60 lines | fixed c945f17 |
| F6 | `sb::key` is a 195-line match in a 1.4k-line `sb.rs` | medium | ~450 lines (move) | fixed f138fc3 |
| F7 | `tui_tmux`, `tui_at_files_tmux`, `tui_archived_tmux` flaky under load | medium | 1–2 h, unknown lines | listed |
| F8 | long functions in `feed.rs` / `render.rs` | medium | 300–600 lines + bench | listed |
| F9 | big modules: `onboarding.rs`, `sb/panel.rs`, `editor.rs` | low | 500+ lines each | listed |
| F10 | `versions.sh` leaks `/tmp/sb-build-*` worktrees | medium (disk) | ~3 lines + a cleanup | listed (for main) |

Used: ~690 changed lines of the ~1500 (the F6 move is 448 of them),
about 1 h 45 of the 3 h. Each fix is its own commit, gated (build,
`cargo test -p bend-tui`, clippy `--workspace --all-targets -D
warnings`; the test fixes by running the test), `run_all.sh` once at the
end. No `.bend` changed. Bench (`bench_long_feed`, 50k lines, release,
run by bise-f-feed): a60735a (before) steady 0.24 ms, PageUp frames
0.46 ms, windowed PageUp to the top 1575 ms; c945f17 (after) 0.24 /
0.43 / 1555 ms: not slower.

## F1 · `run_all.sh` hides Rust test results (high × cheap)

`cargo test -p switchboard 2>&1 | grep "test result" | head -1` prints the
first binary's summary only (the lib's unit tests); the scenario tests
(`core_tests`, …) come later and are cut. Whether a failure stops the
script depends on `pipefail` and on a SIGPIPE race between `grep` and
`head`. And the step never runs `bend-tui`'s tests (the whole TUI: 300+
tests), and clippy has no `--all-targets` (test code is not linted).

Fix: run `cargo test -p switchboard -p bend-tui`, keep its output in a
file, print every `test result` line, fail on a non-zero exit;
`clippy --workspace --all-targets -D warnings`; `unset SB_CORE_BIN` (F2).

## F2 · the tests inherit the agent's `SB_*` env (high × cheap)

An agent's shell carries `SB_CORE_BIN` (the **live** hub's sb-core, an old
version), `SB_SOCKET`, `SB_AGENT`, `SB_TASK`, `SB_PORT_OFFSET`.
`e2e.Env` copies `os.environ` into the throwaway hub's env, and
`tui_tmux.start_tui` forwards every `SB_*` variable to the TUI: the e2e
and tmux tests run the live hub's sb-core, not the tree's, and
`switchboard`'s `core_tests` (they spawn `core::core_bin()`) test the old
core. That is how "core_tests fail for no reason" in an agent's gate.

Fix: `e2e.Env` drops these variables (the throwaway hub picks the
tree's `sb-core`, and gives its agents their own); `run_all.sh` unsets
`SB_CORE_BIN` before cargo.

## F3 · `tui_version_tmux` depends on `~/.local/state` (medium × cheap)

The last step presses Enter on HEAD and waits for `version <head>:
building`. When HEAD is already built in
`~/.local/state/switchboard/versions` the hub switches instead and the
test fails. When it is not, the test starts a real `versions.sh build`
(release cargo build, repl-live) in the user's state dir, which outlives
the test.

Fix: the TUI (and the hub it starts) gets a temp `XDG_STATE_HOME`; its
`switchboard/build` is a plain file, so `versions.sh` fails at its first
`mkdir` and the test waits for `build of <head> failed`: the picker and
the "building" answer are still checked, nothing is built.

## F4 · `tui_composer_tmux` reads the reply from one snapshot (medium × trivial)

It waits for `first message` (the user's own line, on screen at once),
then `find("ack: first message")` reads **one** screen: under load the
fake's reply is not there yet. Fix: `wait_screen("ack: first message")`
before `find`.

## F5 · dead code behind `allow(dead_code)` (low × cheap)

- `theme.rs` has a crate-module `#![allow(dead_code)]` ("the roles land
  before their users, wave 1"): the waves are done; it now hides one dead
  function, `compacting_frame`.
- `sb/cards.rs`: `ago` (`#[allow(dead_code)]`, "the panel stops using it
  with BISE-20"): unused.
- `feed.rs`: `toggle_all_outputs` / `set_all_outputs` ("bound by
  BISE-42"): BISE-42 went through `feed::toggle_event` instead and said
  "F may drop it"; only a test calls them.

Fix: delete them and the allows (the test keeps its one-item toggle).

## F6 · `sb::key` in a 1.4k-line `sb.rs` (medium × medium)

`sb.rs` holds the hub client state, the hub-line parser, the commands,
the state ingest and the keys. `key` is one ~195-line match: the drop
question, the not-delivered question, the panel navigation, the cards,
esc. Nothing wrong in it, but every key change reopens the whole file.

Fix: move `key`, `Nav`, `nav_key` (and their tests) to `sb/keys.rs`, no
code change (a pure move: ~450 changed lines).

## F7 · flaky tmux tests under load (listed)

`tui_tmux`, `tui_at_files_tmux`, `tui_archived_tmux` pass alone and fail
now and then in `run_all.sh` (a loaded machine). Likely the same pattern
as F4 (a fixed `sleep` or one snapshot after a key); each needs a run
under load to find the step. **Cost:** 1–2 h, small patches.

## F8 · long functions in `feed.rs` / `render.rs` (listed)

`feed::push_event` (~170 lines: the twins, the marks, the folds, the
cache), `feed::wrap_line` (~70), `feed::move_anchor` (~65),
`render::ev_lines` (~105, one arm per event kind). They are the hot path
of the 50k-line bench and the most tested code of the crate
(`feed_render_tests.rs`, 1.3k lines); splitting `push_event` into one
pure step per rule is the right next move. **Cost:** 300–600 changed
lines, a bench run before and after, half a day.

## F9 · big modules (listed)

`onboarding.rs` (1480 lines: steps, env-file writing, drawing),
`sb/panel.rs` (1431), `editor.rs` (1378). Each would split along its
seams (onboarding: the `.env` writer is the only effectful part and
deserves its own module and tests). **Cost:** 500+ lines each, not in
this budget.

## F10 · `versions.sh` leaks `/tmp/sb-build-*` worktrees (listed, for main)

`build <rev>` checks the commit out in `/tmp/sb-build-<id>-<pid>` and
removes it with a `RETURN` trap; under `set -e` a failed build exits the
shell and a `RETURN` trap does not run on exit. 50 such worktrees are
registered on this repo (`git worktree list`), each a full checkout, on a
disk with ~10 GB free. Fix: an `EXIT` trap (or `trap … RETURN EXIT`),
then `git worktree remove --force` the stale ones and `git worktree
prune`. **Cost:** ~3 lines + the cleanup; outside BISE-88's scope
(`versions.sh`, other agents' build dirs), so main decides.

## Checked, nothing to fix

- `unwrap`/panics on input: the new byte slices on external text
  (`theme_detect::parse_osc11`, `has_da1`, `onboarding::tilde`,
  `onboarding::typed`, `editor`'s chip labels) all cut at an ASCII match
  or a `char_indices` boundary; no `unwrap` on hub or terminal input
  outside tests.
- `hub/*.bend` (`view.waits_on`, the undelivered notices): small,
  covered by PROOF and `core_tests`; `waits_on` is O(agents × waiters),
  fine at our sizes.
- Tests with no assert: only the "never panics" tests (fuzz, narrow
  screens), on purpose.
