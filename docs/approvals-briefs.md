# Approvals: one brief per parallel agent (phases 0 and 1)

Main spawns these as written (`sb spawn <name> --objective … `, the brief
as the first message). Design: [approvals-design.md](approvals-design.md);
plan and the one branch: [approvals-plan.md](approvals-plan.md) §4–5.

## Rules for every agent (paste into each brief)

- Read first: the design sections named in your brief, `docs/loop-speed.md`
  §0, and your part's contracts below.
- One branch for everything: the local branch `approvals` (plan §5).
  Work in your own worktree (`tests/gate.sh new <your name>`, then
  `git checkout --detach approvals`) and commit **onto
  `refs/heads/approvals`** with a private index built from the branch's
  current tip, then a compare-and-swap:
  `old=$(git rev-parse refs/heads/approvals); GIT_INDEX_FILE=$TMPDIR/approvals.idx git read-tree $old;`
  add your files only; `new=$(git commit-tree $(git write-tree) -p $old -F msg.txt)`;
  `git update-ref refs/heads/approvals $new $old`. If it fails, someone
  committed first: rebuild from the new tip and retry. Then
  `git checkout --detach approvals` to build on everyone's work.
  **Never commit to main, never push, never reset/amend/rebase, no other
  branch.**
- `tests/gate.sh` (quick) per commit, `gate.sh full` once on the last
  commit, in the foreground. End with `gate.sh done <name>`.
- Before running a hub or a REPL for tests: unset `BEND_SESSION_FILE
  BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT`, use a temp `HOME`. Never
  touch the live hub. Never print a key.
- Load the `bend` skill before editing a `.bend` file; pure code in the
  `*-pure.bend` / `bend/core` files with laws in `bend/LAWS.bend`.
- Ask designer about any UI text or look the design does not already fix.
- Take the tracker ids at launch from HEAD's `docs/brand/bise-issues.md`
  (or the current tracker), one per part.
- Done = the brief's "done when" list, the gate green, a report with the
  commits on `approvals`, what you did not do, and a 3-line "how to try it".

## Contracts between the parts (fixed now so the parts can run in parallel)

```rust
// rust/switchboard/src/approvals/mod.rs  (owner: 1b; 1a, 1d, 1e use it)
pub enum Mode { Yolo, Auto }
pub enum Checker { Jev, Model, Off }
pub struct Call { pub tool: String, pub args: serde_json::Value,   // bash: {"arg": cmd}; edit/write_file: {"file_path"}; apply_patch: the patch
                  pub agent: String, pub cwd: PathBuf, pub repo: PathBuf, pub tmp: PathBuf }
pub enum Verdict {
    Allow { tier: u8 },                                   // tiers 0–4 of design §3
    DenyOnce { hint: String },                            // tier 3 (not with the sandbox)
    Check { parts: Vec<Part>, keys: Vec<CacheKey> },      // tier 5: 1d decides
    Card { reason: String, always: Option<String> },      // tier 0 (always = None) or checker off
}
pub fn judge(call: &Call, rules: &Rules, cache: &Cache, sandboxed: bool) -> Verdict;   // pure
// 1d: pure state + effectful call
pub fn checker_state(call: &Call, parts: &[Part], task: &str) -> CheckerState;
pub async fn check(state: &CheckerState, how: Checker) -> Result<Decision, CheckErr>; // Decision { allow: bool, reason: String, scores: Vec<(String, f32)> }
```

- The gate wire stays approvals.md §3 (`gate <n> <json>` from the runtime,
  the answer `<n> <nonce> allow|deny <reason>` in the gate file).
- Until a part lands, its users stub it: 1a's gate calls a `judge` that
  returns `Card` for everything but `sb`; 1b's `Check` goes to a `check`
  that returns "couldn't check"; 1e is off unless `BISE_SANDBOX=1`.

---

## Phase 0 · `approvals-tmp`: the agent's temp folder (done first)

Objective: every agent gets a temp folder in its session folder, is told
its path, and the harness stops writing to `/tmp`. Useful now in `yolo`.

Read: design §7.1, §6.2 (tmux), plan §5 (the one branch).

Do:
1. At spawn the hub creates `~/.bise/hubs/<hub>/agents/<agent>/tmp/` and
   `…/run/`; `TMPDIR`, `TMP`, `TEMP`, `TMUX_TMPDIR` point to `tmp/` in the
   agent's tool env (`rust/switchboard/src/tools_env.rs`, `paths.rs`).
2. Move the harness's `/tmp` files: background slots `/tmp/bend-bg-<port>/`
   → `tmp/bg/`; `/tmp/bend-sh-<port>-*.sh`, `/tmp/bend-steer-<port>.txt`,
   `/tmp/bend-interrupt-<port>.txt`, `/tmp/bend-prog-<port>.ts|.err`,
   `/tmp/bend-res-<port>.json`, `/tmp/bend-plugins-start-<port>.sh` →
   `run/` (`bend/runtime/bash*.bend`, `main.bend`, `plugins.bend`,
   `provider*.bend`; update the laws in `bend/LAWS.bend` ~171, ~414,
   ~1273–1392). The runtime gets the folder from the hub (an env var);
   headless runs (tests, bench, bare `repl-live`) fall back to
   `$TMPDIR`, then `/tmp`.
3. One prompt line (in `prompts/`, next to the other env lines): "Your temp
   folder is `<path>` (`$TMPDIR`): use it for scratch files, never `/tmp`.
   It is deleted when you are dropped." main's own prompt too (main writes
   its briefs there).
4. Delete `tmp/` on drop, and at hub start the `tmp/` of agents that no
   longer exist (`sweep.rs`). Never delete `run/` of a live agent.

Done when: a new agent's `echo $TMPDIR` is its `tmp/`; `mktemp`, python
`tempfile`, `tmux -L x` land there; `ls /tmp/bend-*` stays empty through a
session with a background job, a steer and an interrupt; drop removes
`tmp/`; laws and e2e green. ~1 day. On `approvals`, like every part.

---

## 1a · `approvals-modes`: the modes, the gate, the card flow

Objective: `yolo`/`auto` as a global mode with `shift+tab`, and the gate
that pauses a call until a verdict, with the `confirm` card in the user
inbox.

Read: design §3.1, §8, §9, §10, §11; approvals.md §3 (the wire).

Do:
1. The mode in the hub, `approvals = "yolo" | "auto"` in config.toml
   (same writer as `bise config set`), `BISE_APPROVALS` for one session;
   `/approvals` (mode, checker, rules; `/approvals yolo|auto`,
   `/approvals checker jev|model|off`).
2. TUI: `shift+tab` toggles in the composer; list outdent moves to
   backspace at the start of an item's text (BISE-276 code in `input.rs`,
   `mdlive.rs`); the key-bar indicator `⇧⇥ yolo`; the 3-second flash
   (designer's lines, design §8); the one-time tips (design §8).
3. The runtime gate (`bend/runtime/main.bend` + pure laws): in `auto`,
   before each gated call, `gate <n> <json>`, then wait on the gate file
   and the interrupt file; the mode file next to the gate file so `yolo`
   never asks (no latency).
4. The hub: on a gate line, call `judge` (stub until 1b), `check` (stub
   until 1d); `Card` → `open_confirm` (`bend/hub/core.bend`, BISE-299) with
   the reason, the "always" pattern, the parts already allowed shown dim;
   the answer writes the verdict to the gate (allow / deny + note), saves
   the rule on "always", folds the card (design §9). Identical calls from
   several agents: one card.
5. The waiting agent: `waiting on you` status, the `?` row, ctrl+c /
   `sb interrupt` ends the wait; the `checking…` state after 250 ms on the
   tool row while `check` runs.

Done when: the plan §5 test script steps 1, 2, 6, 7, 10 pass with stubs;
e2e with the fake provider (a card in main's view and another view;
allow / always / no with a note / ctrl+c); `yolo` adds no latency
(measured). ~3 days.

---

## 1b · `approvals-parser`: the parser, the tiers, the saved rules

Objective: `judge()` of the contracts, pure and fast, on `brush-parser`.

Read: design §3, §5, §6.3 (risk classes), §7; the prototype in
`docs/approvals-eval/` (its tiers and numbers are your baseline); Vibe's
`_shell_permission_analysis.py`, `arity.py`, `_shell_command_policy.py`.

Do:
1. `approvals/parse.rs`: `brush-parser = "=0.4.0"` wrapped; out: parts
   (program, args unquoted, assignments, redirections, unreadable flags,
   heredoc bodies as text), through lists, pipes, subshells, groups,
   `if`/`for`/`while`/`case`, `$(…)` bodies, `bash -c` strings; wrappers
   (`env`, `time`, `nohup`, `timeout`, `nice`, `command`, `exec`,
   `xargs`, `git -C`); `cd` moves the base.
2. Tiers 0–4 (design §3): hard rules, `sb`, safe reads with guards, plain
   writes, local git, builtins, saved rules, deny-once detection, the cache
   lookup; the roots with symlinks and `..` resolved, the protected paths,
   the `tmp/` carve-out; the edit tools' paths.
3. The arity table and the "always" pattern; `~/.bise/approvals.toml`
   read/write (per git common root), exact text for guarded or unreadable
   parts; the risk classes of design §6.3 (used by 1e).
4. Tests: a table of Vibe's cases + ours; the corpus test: a script that
   re-extracts the bash calls from `~/.bise/hubs/*/agents/*/wire.log`
   (never committed), runs `judge`, prints the tier shares; it must stay
   within 1 point of design §3.2 (57.9 % decided without a model, 6.9 %
   checker calls with the cache).

Done when: `judge` covers every row of design §3's table with tests; the
corpus run is reported with its numbers; p99 under 0.1 ms. ~3 days.

---

## 1c · `approvals-edit`: Vibe's `edit` and `write_file`, one toolset per request

Objective: non-OpenAI requests get Vibe's `edit` + `write_file` as is,
OpenAI requests keep `apply_patch`, and the prompts steer edits to them.

Read: design §2.1, §2.2 (the tools' exact text); Vibe 2.25.8
`vibe/core/tools/builtins/edit.py`, `write_file.py`, `prompts/edit.md`,
`prompts/write_file.md`.

Do:
1. `edit` and `write_file`: names, schemas, descriptions, errors, results
   copied exactly; atomic write; a pure core in `bend/core/edit.bend` with
   laws; the runtime tool; the TUI shows both as a diff like `apply_patch`;
   the activity line "edit <file>" / "write <file>".
2. `catalog_live` (`bend/runtime/tools-pure.bend`): one toolset by the
   agent's provider; a provider switch rebuilds the catalog and the prompt
   lines; old sessions replay their past calls.
3. Steering: tool order (edit tools before `bash`); the system prompt
   line; the bash description without the heredoc invitation and with
   "not for editing files: use `edit`" (or `apply_patch`); the deny-once
   hint text for 1a/1b, naming the tool that is on.
4. The gate reads `file_path` (for 1b): export it in the gate JSON.

Done when: an Anthropic, a Mistral and an OpenAI session each list only
their toolset; the error strings match Vibe's byte for byte (a test per
error); laws green; a short before/after count of edit methods on 3 small
tasks per provider. ~2.5 days.

---

## 1d · `approvals-checker`: Jev, the fallback, the cache

Objective: `check()` of the contracts: Jev when a route exists, the
`classify` role otherwise, off on request; allow verdicts cached per repo;
fails closed.

Read: design §4 (all), §9 (reason words), §12; the sources listed in §4.1.

Do:
1. Routes: OpenRouter (`typesafe/jev-1.13`, the user's OpenRouter key)
   and TypeSafe's API (`TYPESAFE_API_KEY` or `bise login typesafe`, stored
   in `auth.json`). Request = state + the 3 `noul` questions of §4.3; allow
   at `contained` ≥ 0.9, `serves_task` ≥ 0.8, `secrets` ≤ 0.1. Timeout 5 s.
2. The state builder (pure): the parts, the script run (cut 4 000
   chars), the user's words behind the task (cut 2 000), paths; nothing
   else. A unit test that no tool result or file content can enter it.
3. The fallback on the `classify` role (BISE-298): approvals.md §4.3's
   prompt, strict JSON, the same rule. A checker error is a card, never
   the other checker.
4. The cache (design §4.4): per repo, per hub session, allow only;
   pattern keys for plain parts, exact text for the rest and for network
   and publish tools; cleared on "no", on a checker change, on restart.
5. The reason words from the scores (design §9), scores to the debug log;
   the 3-errors notice and the 2-minute cool-down; `approvals_classifier`
   in config.toml; the cost in `/usage`; the "auto-confirm" row in
   `/setup` roles.
6. A small eval (`docs/approvals-eval/`): 40 labeled commands from our
   threads (20 fine, 20 risky), Jev vs `mistral-small-latest`: dangerous
   allowed, share to the user, latency, cost. Phase 2 grows it to ~150.

Done when: both routes and the fallback work against a fake server in
tests and once against the real ones (no key printed); the eval table is
in the report. ~3 days.

---

## 1e · `approvals-sandbox`: Seatbelt on macOS (only if the user says yes to plan Q1)

Objective: in `auto` on macOS, every bash call runs under a per-agent
Seatbelt profile: writes only in the roots, network only when allowed;
a denial becomes a rerun decision.

Read: design §6 (all), §7, §7.1; Codex `sandboxing/src/seatbelt.rs`,
`seatbelt_base_policy.sbpl`, `denial.rs`, `core/src/tools/orchestrator.rs`.

Do:
1. The profile (pure generator + tests): `(allow default)`, deny writes,
   allow the roots (the current folder, the repo's git common dir with
   `hooks/` and `config` denied, `~/.bise` minus `hubs/`,
   `approvals.toml`, `auth.json`, plus the agent's `tmp/`), `/dev/null`,
   `/dev/fd`, ttys, the cache allowlist (`~/.cargo/registry`,
   `~/.cargo/git`, `~/.npm`, `~/Library/pnpm`, `~/.cache`,
   `~/Library/Caches`); network denied except loopback and the hub socket.
   Written by the hub next to the gate file, per agent, rewritten when a
   root changes.
2. The runtime (`bend/runtime/bash.bend`): in `auto` with a profile,
   `Proc.run(["sandbox-exec", "-f", profile, "/bin/sh", path])`;
   background jobs too. A part that 1b marks "network by name" and that
   the checker allowed runs with a network-open profile.
3. Denial: exit code + "Operation not permitted" (Codex's heuristic) →
   the result says it was stopped by the sandbox and why; the rerun
   without the sandbox goes to the checker (state: the command + the
   denied path), then a card "it needs to write outside the repo: <path>"
   that says the command will run a second time.
4. Tests: the table of design §6.2 as tests (write in/out, cargo build,
   python tempfile, `sb`, tmux with `TMUX_TMPDIR`, a worktree commit, the
   `tmp/` carve-out); the +12 ms cost measured; `sandbox-exec` missing →
   the parser path, said once in main's feed.

Done when: the plan §5 test script steps 3–5 pass with the sandbox on;
bash edit scripts inside the repo run without a card; a write to
`~/Desktop` is stopped and becomes a card. ~3.5 days.
