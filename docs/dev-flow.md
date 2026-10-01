# The dev flow: PRs or straight to main

Status: design, nothing built. Goes with [pr-design.md](pr-design.md) (the
PR UI) and [pr-plan.md](pr-plan.md). The user's question (2026-10-01):
when should agents open pull requests rather than work on main? In a
repo several people share, PRs should be the main flow. But there is a
use case for what this repo does: no PR, everyone lands on main. What is
the best flow, and what changes in the agents' instructions besides the
UI?

## 1. The answer in short

Two flows, one per repo, picked once and saved:

- **PR flow** (the default as soon as the repo is shared): every task
  that changes code gets its own branch in a worktree, pushes it, opens
  a PR and owns it until it is merged. Nobody touches the default
  branch. You merge (or your team does).
- **Trunk flow** (a repo only you push to, like bise itself): the agents
  land small commits straight on main, one at a time, each one tested.
  No PR, no review step, unless you ask for one.

What sets the flow: the repo's rules first (a protected branch forces
PRs), then who commits there, then your answer to one question, asked
once. Your words always win for one task ("open a PR for this",
"just commit it").

## 2. How bise picks the flow

Checked when the hub starts in a repo, again when the remote changes:

| Signal | Flow |
|---|---|
| no remote | trunk (a PR is impossible) |
| the default branch is protected or has a ruleset that requires a PR (`gh api repos/{o}/{r}/rules/branches/{b}`; GitLab: protected branch, "allowed to push: no one") | PR, forced: a push to main would fail anyway |
| someone else (not a bot) committed on the default branch in the last 90 days (`git log --since=90.days --format=%ae origin/<b>`) | PR, suggested |
| the repo's AGENTS.md / CONTRIBUTING says "open a PR" | PR, suggested |
| none of the above (you alone) | trunk, suggested |

"Suggested" means main asks once, at the first task that changes code,
with the suggestion first (one question in your inbox, 2 options):

```
┃ ? main needs you · how should agents ship code here?
┃
┃   alice and 3 others committed on main this month.
┃
┃   1 a PR per task (you merge)          ← suggested
┃   2 straight to main, tested commits
```

Saved in the repo's `.switchboard/config.toml` (`[flow] mode = "pr" |
"trunk"`), so it never asks again; `/flow` changes it. Forced: no
question, one line from main the first time ("main is protected here:
every agent opens a PR").

## 3. Which tasks get a branch

The flow decides where code goes. Many tasks write no code at all.

| Task | PR flow | Trunk flow |
|---|---|---|
| read-only: investigate, review, answer, plan, research | no branch, the shared folder | same |
| docs, notes, small config | a PR (it's still a change to a shared repo) | a commit on main from the shared folder |
| a code change | worktree `sb/<name>` from `origin/<default>`, a PR | small and alone: the shared folder, commits on main. Bigger, or another agent works in the same files, or the build must not see half-done edits: a worktree, then *land* (§5) |
| a long feature (several phases, like approvals) | a draft PR early, or one PR per phase stacked on each other | a local branch (`approvals`), then a review item for you before it lands |
| you say "open a PR" | — | a PR, even here |
| you say "just commit it" | refused if main is protected; else main asks once ("main is shared here, sure?") | — |

Rules that hold in both flows:

- **One concern per change.** A task that grows splits: a second PR (or
  commit series) for the second concern, stacked on the first if it
  depends on it.
- **Every commit passes the repo's checks** (`[flow] check`, e.g.
  `./gate.sh --quick`, `cargo test`, `pnpm test`), run by the agent
  before it pushes or lands. The CI is the second net, not the first.
- **Never rewrite what others have**: no `--force` on a branch you did
  not create (`--force-with-lease` on your own PR branch only, after a
  rebase), no `reset`, `stash`, `amend` or `rebase` in the shared folder.

## 4. PR flow, step by step

1. Main spawns the task with `--pr`: a worktree on `sb/<name>` from a
   fresh `origin/<default>` (not your local HEAD: it would carry your
   unpushed commits), plus the brief's "done when: its PR is open".
2. The agent works and commits small; runs the check.
3. It pushes its branch and opens the PR with `gh pr create` (or `glab mr
   create`): the repo's template if there is one, else what changed, why,
   and how it was tested, in the repo's style. Ready for review unless
   you said draft, or the task is long (draft until its last phase).
4. **It owns the PR until it is merged**: review comments, red checks,
   conflicts (`mergeStateStatus` DIRTY or BEHIND: it rebases on the base
   and pushes with `--force-with-lease`). It stays alive (idle costs
   nothing) instead of being archived when the PR opens.
5. It never merges, approves, closes, or writes on GitHub (comments,
   replies, resolving threads) unless you allowed it (pr-design §11 Q3).
6. Merged: the hub archives it and removes its worktree. Closed without
   merge: main tells you; the branch stays.

## 5. Trunk flow, step by step

What this repo does by hand today (a private `GIT_INDEX_FILE`, `git
commit-tree`, `git update-ref refs/heads/main <new> <old>`, then "sync
the shared tree"), made a command so no agent has to get it right alone:

**From the shared folder** (small change):

1. The agent edits, runs the check.
2. `sb land`: the hub commits only the files this agent changed (it
   tracks them, RFC 0001 §10.3) through a private index built from the
   current HEAD, moves main with a compare-and-swap (`update-ref <new>
   <old>`), then updates those paths in the shared index so `git status`
   stays clean (the "D / ??" lag we saw on 18b7443). Your own edits and
   other agents' stay where they are.
3. A file changed by two agents (an overlap, ⇄): `sb land` refuses and
   main asks who takes it.

**From a worktree** (bigger change):

1. The agent commits on its branch, runs the check.
2. `sb land`: the hub rebases the branch on the current main, runs the
   check again if main moved, then fast-forwards main. One land at a
   time (a queue in the hub: no two agents race for main). A conflict:
   back to the agent, with the files.
3. The worktree is then removed and the agent archived (or it keeps it
   for its next commit series).

**A long feature branch**: the agent says it's ready; the hub opens a
review item for you (`approvals is ready to land: 14 commits, +3,120
−410 · 1 land it · 2 show the diff · 3 not yet`), then lands it like
above. Today main does that by hand (`is-ancestor` then `update-ref`).

**Pushing**: in trunk flow, main lands locally. `[flow] push = true`
pushes main after every land (what this repo does now); else only when
you ask.

## 6. What changes in the agents' instructions

Today (`rust/switchboard/src/prompts.rs`): main has "use `--worktree`
ONLY when the user explicitly asks" and "never push, merge or run
destructive git commands unless the user asks"; a task in a worktree has
"you may commit on your branch; never push unless the user asks"; a task
in the shared folder has "do not revert changes you did not make". Each
repo's own habits (private index, `gate.sh`, push after landing) live in
briefs and in agents' memory, so every brief repeats them.

New: the hub writes a **Flow** section into both prompts from the repo's
config, so a brief no longer has to.

**Main's prompt**

- PR flow: "This repo ships through pull requests (base `main`). A task
  that changes code: spawn it with `--pr`. Read-only tasks: no branch.
  You never merge; the user does (the inbox asks them when a PR is
  approved with checks passing). GitHub's news about a PR go to the
  agent that owns it; you get a copy: escalate only product calls and
  checks still failing after 2 tries."
- Trunk flow: "This repo ships straight to `main`, through `sb land`.
  Small changes from the shared folder; a worktree (`--worktree`) when
  the change is big, risky, or another agent works in the same files. A
  long feature: a branch, then the user approves the land."
- Both: the worktree rule changes from "only when the user asks" to
  "when the flow says so, or the user asks"; "never push or merge" stays
  except what the flow does itself (`sb land`, the PR's own branch).
- The user's words win for one task: "open a PR", "just commit it".

**A task's prompt** (its place line, by flow and place)

- PR flow, worktree: "branch `sb/x` from `origin/main`. Commit small; run
  `<check>` before you push. Push only this branch. Open the PR with gh
  (the repo's template). You own it until it is merged: fix reviews and
  red checks, rebase when it conflicts (`--force-with-lease`, this
  branch only). Never merge, approve, close, or write on GitHub."
- Trunk flow, shared folder: "commit nothing by hand: run `<check>`, then
  `sb land "<message>"`. Never `git add -A`, stash, reset, rebase or
  amend here."
- Trunk flow, worktree: "commit on your branch; run `<check>`; `sb land`
  rebases it on main and moves main."
- Both: the commit message style (from the repo's AGENTS.md or the last
  50 commits: this repo writes long, detailed subject lines), and the
  check command.

**Approvals (auto mode)**

| Command | PR flow | Trunk flow |
|---|---|---|
| `git commit` in its worktree | runs | runs |
| `git push` of its own branch | runs | asks (no PR here) |
| `git push` to the default branch | always asks (hard rule) | runs after `sb land` if `push = true`, else asks |
| `gh pr create\|view\|checks\|diff` | runs | asks |
| `gh pr merge`, `review --approve`, `comment`, `gh api` writes | always ask | always ask |
| `sb land` | refused (no landing on main) | runs |

**The brief**: `--pr` adds "done when: its PR is open" (PR flow);
in trunk flow, "done when" ends with "landed on main". Briefs stop
repeating the git rules (private index, gate, push).

## 7. What changes in the UI

Most of it is in [pr-design.md](pr-design.md) (PR flow). For the trunk
flow, and the choice:

- **Sidebar**: nothing new in trunk flow (ψ while the agent has a
  worktree). Waiting to land: the row's glyph is today's `…` (waiting),
  and ctrl held says `waits to land · 2nd`.
- **Main's feed**: `✓ dark-mode landed 3 commits on main (a1b2c3)` (and
  `· pushed` when it pushed); a refused land: `dark-mode can't land:
  login.rs changed on main too. it's rebasing.`
- **Inbox**: the flow question (once per repo); a long branch ready to
  land; a "just commit it" on a shared repo.
- **Header, ctrl held**: the flow after the folder, `~/acme · PRs` or
  `~/acme · main`, so you know what an agent will do with your request.
- `/flow`: shows the flow, why (the signal), and switches it.

## 8. Recommendation

- PR flow is the default for any repo with someone else in it; trunk
  for a repo that is yours alone. Detected, asked once, saved; forced
  when the repo protects its branch.
- Build `sb land` first, even before the PR UI: it turns what this repo
  does by hand into one checked step and fixes the shared-index lag. It
  is phase 0b in the plan.
- The flow goes into the prompts from the config; briefs stop carrying
  git rules.

## 9. Open questions (for the user)

1. PR flow as soon as one other person committed in 90 days: right
   threshold? (Or any collaborator on the GitHub repo.)
2. Trunk flow: shared folder for small changes and a worktree only when
   needed (today), or a worktree for every code task (cleaner, but a
   cold build per worktree: minutes and GBs for Rust)?
3. Trunk flow: push main after every land (what this repo does), or only
   when you ask?
4. In PR flow, does the agent stay alive until the merge (it owns the
   PR) or archive at "PR open" and come back on the first review?
5. A long feature in PR flow: one draft PR that grows, or one PR per
   phase, stacked?
