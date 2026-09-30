# Approvals: plan for the build (start here)

Status: design done, not started. Design:
[approvals-design.md](approvals-design.md) (2 modes, the tiers, the
checker, the parser, the sandbox comparison, the card). One brief per
parallel agent: [approvals-briefs.md](approvals-briefs.md). Reference for
the hard rules and the wire: [approvals.md](approvals.md) (d22c024).
Nothing is built until the user says go, and nothing lands on main before
his review (§5).

## 1. The user's decisions (firm)

- 2 global modes: `yolo` (default) and `auto`. `accept edits` is merged
  into `auto`: it is `auto` with the checker off.
- `shift+tab` toggles them; the last pick is remembered (config.toml).
- What needs the user: a `confirm` card in the user inbox (BISE-299).
- `auto` calls the checker **rarely**: cheap tiers first, the checker only
  for what is left, its allow verdicts cached (design §3, §4.4).
- Settled before and kept: a real bash parser (safe reads, per-repo
  "always allow" rules, plain writes); plain writes the parser reads run
  like edits; a bash edit the parser cannot read is denied once with the
  hint, then a card; one edit toolset per request (OpenAI → `apply_patch`;
  the rest → Vibe's `edit` + `write_file`, as is); no time limit; roots =
  the current folder and below + `~/.bise` minus `hubs/`,
  `approvals.toml`, `auth.json`; "always allow" per repo; network judged by
  the checker; `yolo` leaves waiting cards open; the phases run in
  parallel.
- New: the agent's temp folder lives in its session folder
  (`~/.bise/hubs/<hub>/agents/<agent>/tmp`, told in its prompt), the
  harness's own `/tmp` files move there too, and this ships first
  (design §7.1).
- The user's answers (2026-10-01): **Q1 the sandbox on macOS now: yes.
  Q2 Jev by default: yes**, as a role row, `checker`, in `/models`
  (role → provider → model; TypeSafe or OpenRouter for Jev, a chat provider
  as the fallback, or off; a TypeSafe key through `/provider`). **Q3 reads
  anywhere but the secret paths in `auto`: yes. Q4 local git (`add`,
  `commit`, `apply`, the private-index plumbing) at once: yes.**
- Depends on **roles-menu** (option A): the `approvals` branch starts from
  main after it lands, and the `checker` row builds on its role table.
- New: the build lands on **one local branch, `approvals`** (from main,
  never pushed unless the user asks), every agent committing onto it;
  main merges it after the user's test and go (§5).

## 2. The picks

| what | pick | why (design) |
|---|---|---|
| the checker | **Jev** (TypeSafe, `jev-1.13`) in the `checker` role of `/models`, through TypeSafe or OpenRouter; a chat model can take the role; or off | 3 yes/no questions in one pass, probabilities, ~$0.00005 a check, 0 dangerous commands allowed on a public 113-command test; fails closed (§4) |
| "Jev" | the user's word was right: TypeSafe AI's System One model, released 2026-09-15/18, on OpenRouter as `typesafe/jev-1.13` | §4.1 |
| the parser | **`brush-parser` 0.4** (pure Rust, MIT) | 5 131 real commands: 0 false errors (tree-sitter-bash: 2), 13 µs p50; no C; ~30 new crates (§5.1) |
| call rate | **6.9 % of bash calls** (~140 a day at today's pace) with the parser path; **~1–2 %** with the macOS sandbox | measured on 2.5 days of real threads (§3.2, §6.3) |
| cost | Jev ≈ **$0.007 a day**; the chat fallback ≈ $0.04 a day; the agents' own tokens unchanged | §4.6 |
| the sandbox | Seatbelt on macOS in phase 1 (settled), the parser path as the fallback | §6.5 |

## 3. Open questions for the user

None. The 4 of the last round are answered (§1).

## 4. Phases

Ids from HEAD's tracker at launch (the last used today: BISE-301).

| phase | what | where | cost |
|---|---|---|---|
| 0. temp folder | `tmp/` and `run/` in the agent's session folder; `TMPDIR`/`TMP`/`TEMP`/`TMUX_TMPDIR`; the harness's `/tmp` files moved (bg slots, sh wrappers, steer, interrupt, `run_typescript` files, plugins script); the prompt line; delete on drop and on hub start | `rust/switchboard` (`tools_env.rs`, `paths.rs`, drop, sweep), `bend/runtime` (`bash*.bend`, `main.bend`, `plugins.bend`), `bend/LAWS.bend`, `prompts/` | ~1 day |
| 1a. modes + gate | the mode in the hub, `approvals` in config.toml, `shift+tab` toggle + outdent on backspace, key-bar indicator, flash, tips, `/approvals`; the runtime gate (wire, gate file, pause, interrupt, mode file so `yolo` costs nothing); `confirm` answers to the gate; the waiting-agent signals; `checking…` after 250 ms | `bend/runtime/main.bend` + laws, `bend/hub/core.bend`, `rust/switchboard`, `rust/tui`, `rust/catalog` | ~3 days |
| 1b. parser + tiers + rules | `brush-parser` behind `approvals/parse.rs`; the analysis (parts, wrappers, unreadable parts, redirections); tiers 0–3; roots and protected paths (with the `tmp/` carve-out); the arity table; saved rules in `~/.bise/approvals.toml` and "always allow … here"; the risk classes of §6.3; the corpus test | `rust/switchboard/src/approvals/` | ~3 days |
| 1c. edit tools + steering | Vibe's `edit` and `write_file` as is (pure core + laws, TUI diff); one toolset per request by provider in `catalog_live`; tool order; prompt lines; the bash description; the deny-once hint text | `bend/core/edit.bend`, `bend/runtime`, `prompts/`, `rust/tui` | ~2.5 days |
| 1d. checker | the `checker` role row in `/models` on roles-menu's role table (TypeSafe / OpenRouter / a chat provider / off; TypeSafe in `/provider`); the Jev client (both routes), the state builder, the 3 questions and thresholds, the chat fallback, the per-repo cache, fail-closed + the 3-errors notice, the reason words, `/usage` cost | `rust/switchboard/src/approvals/checker*.rs`, `rust/catalog`, `rust/tui/src/onboarding` | ~3 days |
| 1e. sandbox (macOS) | the per-agent Seatbelt profile, `sandbox-exec` around the bash call in `auto`, the git common dir, the cache allowlist, network rule, denial detection and the rerun flow | `rust/switchboard/src/approvals/sandbox.rs`, `bend/runtime/bash.bend` | ~3.5 days |
| 2. integration + proof | e2e with the fake provider (card in main's view and another, allow / always / no / ctrl+c, `yolo` costs nothing); the checker eval (~150 labeled calls: Jev vs the `classify` role: share reaching the user, dangerous allowed, latency, cost); the edit-tool share per model, before/after; the call rate re-measured | `tests`, `docs/approvals-eval/` | ~2 days |
| 3. the user's review | a local build from `approvals`, the test script (§5), fixes, then main merges `approvals` into main on his go | — | ~0.5 day + fixes |
| later | grouped cards, terminal notification when unfocused, `bise approvals` CLI; the Linux sandbox in `sb/ports` (~4 days) | | ~1.5 + 4 days |

Phase 0: ~1 day, done first (it helps the rest). Phase 1 (1a–1e, ~15 agent-days, 5 agents in
parallel): about 3.5 days of wall time. The parts meet on contracts
written in the briefs (the `Verdict` type, the checker's `check(state)`,
the gate wire), with stubs until the real part lands. Launch criterion
for `auto`: phase 2 green, 0 dangerous commands allowed on the eval, and on
our own threads fewer than 1 call in 50 reaches the user. Total ~21
agent-days with the sandbox, ~17 without.

## 5. One branch, review and merge (the user: "one branch with everything in it, even if it is huge and several agents work on it; it can be my local branch")

- One local branch, **`approvals`**, created by main from main **after
  roles-menu lands** (main sends its SHA). Never pushed unless the user asks. Everything lands there:
  phase 0, the 5 phase-1 parts, phase 2.
- Each agent works in its own worktree (`tests/gate.sh new <name>`,
  detached on the tip of `approvals`) and commits **onto
  `refs/heads/approvals`** without checking it out:
  ```sh
  old=$(git rev-parse refs/heads/approvals)
  export GIT_INDEX_FILE=$TMPDIR/approvals.idx
  git diff $base -- <your files> > $TMPDIR/mine.patch   # base = the tip your worktree is on
  git read-tree $old                      # the branch's CURRENT tip
  git apply --cached $TMPDIR/mine.patch   # your hunks on top of everyone's
  new=$(git commit-tree $(git write-tree) -p $old -F msg.txt)
  git update-ref refs/heads/approvals $new $old   # compare-and-swap
  ```
  Your changes go as a patch, never as `git add` of whole files: a file
  another agent changed since your base would lose their hunks. If
  `update-ref` fails, another agent committed first: redo from
  `old=$(git rev-parse …)`. If `git apply --cached` fails, the same lines
  moved: bring your worktree to the tip (`git diff > $TMPDIR/wip.patch;
  git checkout -f --detach approvals; git apply --3way $TMPDIR/wip.patch`),
  fix, retry. After each commit, `git checkout --detach approvals` (your
  worktree now holds exactly the tip) to keep building on everyone's work.
  (Tried on a scratch repo: two worktrees, two patches on one file, both
  kept; a stale `update-ref` is refused.)
- **No agent commits to main, pushes, resets, amends or rebases.** No
  per-part branches, no per-part merges.
- Review of `approvals`: main builds it (`versions.sh` on the branch's
  head) so the user can run it next to his daily bise on a real repo, and
  gives him the test script:
  1. start in `yolo`: nothing asks, the key bar says `⇧⇥ yolo`;
  2. `shift+tab` → `auto`, the flash, the tip naming what leaves the
     machine; restart bise: still `auto`;
  3. ask an agent to read and edit files in the repo: no card; `ls`,
     `rg`, `git status`, `git commit` on a private index: no card;
  4. ask for `cargo test` (or the repo's test command): `checking…`, then
     it runs; again: no check (cached);
  5. ask it to write outside the repo (`~/Desktop/x.txt`): a card; "no"
     with a note: the agent gets the note;
  6. ask for `git push --force` to main: a card without "always";
  7. a card, then ctrl+c on the agent: the call does not run;
  8. "always allow … here" on `npm run build` (or similar): the next one
     runs without a card; `/approvals` lists the rule;
  9. `/models` → `checker` → `off`: a command asks; → a chat provider and
     model: that model checks; → `TypeSafe`: back to Jev;
  10. two agents at once, one waiting on a card: the other keeps working.
- main merges `approvals` into main only after the user says go.

## 6. The spawn list (once roles-menu has landed and `approvals` exists)

| wave | agent | brief (approvals-briefs.md) | starts |
|---|---|---|---|
| 1 | `approvals-tmp` | "Phase 0 · approvals-tmp" | at once |
| 1 | `approvals-parser` | "1b · approvals-parser" | at once (new files only) |
| 1 | `approvals-checker` | "1d · approvals-checker" | at once (needs roles-menu, landed) |
| 1 | `approvals-edit` | "1c · approvals-edit" | at once |
| 1 | `approvals-modes` | "1a · approvals-modes" | at once: TUI and hub first; its runtime gate after `approvals-tmp`'s commit (the gate and interrupt files move to `run/`) |
| 2 | `approvals-sandbox` | "1e · approvals-sandbox" | after `approvals-tmp` reports done (needs `tmp/`, the carve-out, `TMUX_TMPDIR`, and edits `bash.bend` after it) |
| 3 | (main) | phase 2 integration + proof, then the user's review (§5) | after all six report done |

## 7. When it lands

- Landing / README: a line about the 2 modes (designer asked to be told).
- Onboarding: the one-time tip "you're in yolo: agents run commands without
  asking. ⇧⇥ changes it."
