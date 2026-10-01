# The dev flow: PRs or straight to main

Status: design, nothing built. Goes with [pr-design.md](pr-design.md) (the
PR UI) and [pr-plan.md](pr-plan.md). The user's question (2026-10-01):
when should agents open pull requests rather than work on main? In a
repo several people share, PRs should be the main flow. But there is a
use case for what this repo does: no PR, everyone lands on main. What is
the best flow, and what changes in the agents' instructions besides the
UI?

## 0. The user's answers (2026-10-01)

- **Q1 yes**: PR flow as soon as someone else committed in 90 days.
- **Q2: main decides** whether a task needs a worktree, not a fixed rule.
  And **places are shared, not owned**: several agents may work in the
  same worktree, and in PR flow on the same branch. An agent is never
  locked to one branch. Isolation stays, and it must show on screen
  (§3.1).
- **Q4 yes**: in PR flow the agents stay alive until the merge.
- **Q5: the user chooses** one PR or a PR per phase; bise doesn't
  prescribe. Main asks when it matters and the user hasn't said.
- **Q3 yes**: in trunk flow, main is pushed after every land
  (`[flow] push = true` by default).

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

## 3. Where each task works (main decides)

The flow says where code ends up (a PR, or main). Main decides where
each task works, the way it decides who takes what: it knows the files,
the other agents and how big the change is. Many tasks write no code at
all.

| Task | PR flow | Trunk flow |
|---|---|---|
| read-only: investigate, review, answer, plan, research | the shared folder, no branch | same |
| docs, notes, small config | a branch and a PR (it's still a change to a shared repo) | a commit on main from the shared folder |
| a code change | a branch from `origin/<default>` in a worktree, a PR. A new one, or the place of agents already on that change | the shared folder when it's small and nobody else edits those files; else a worktree (new or shared), then *land* (§5) |
| a change next to another agent's (the same feature, a test for it, a review fix) | main may put it **in the same worktree, on the same branch**: one PR, several agents | same: one worktree, several agents, one land |
| a long feature (several phases) | one PR or a PR per phase: **you choose**; main asks if you haven't said | a local branch, then a review item for you before it lands |

What main weighs to isolate (in its prompt as hints, not rules): two
agents in the same files, a build or tests that must not see another
agent's half-done edits, a change long enough to be reviewed on its own,
a risky change you may throw away. What it weighs to share a place: the
work belongs in the same PR, or one agent needs the other's code right
now.

### 3.1 Places are shared, not owned

A **place** is where agents work: the shared folder, or a worktree (a
folder and its branch). Today each agent has its own `ws` (RFC 0002);
here a place is its own thing in the hub, and agents join it:

- `sb spawn x --place new` (a new worktree, branch `sb/x`), `--place
  dark-mode` (the place of agent dark-mode, or a branch name), or
  nothing (the shared folder). `sb move <agent> <place>` moves an agent
  that has changed nothing yet (today's `/isolate`, generalized).
- A place lives as long as one agent is in it or its PR is open. The
  worktree is removed when the last agent leaves and nothing is lost
  (RFC 0002's drop rules, counted per place, not per agent).
- Several agents in one worktree have the shared folder's rules there:
  each commits only its own files (§5, `sb land`), overlaps (⇄) are
  flagged, nobody rewrites the branch while another works on it.
- **A PR belongs to the branch, not to an agent.** Every agent in that
  place sees it; GitHub's news go to the agent that pushed the commit the
  review is about, else the one that opened the PR, else main picks
  (pr-design §6.1).
- **On screen** (pr-design §4.1; the user picked the grouped boxes,
  then the lighter option A, BISE-306): a worktree is a box only when 2
  or more live agents share it. Its top border, in the line color,
  carries the git facts: `╭─ ψ sb/dark-mode ──────── ↑ ─`; its agents
  sit inside behind a rail `│`, the last one closing it with `╰`. An
  agent alone in its worktree is a plain row with its git state as the
  mark in the last column (`↑` its PR, `ψ` no PR yet, `…` its land
  waits), after your folder's rows. Ctrl held: a box adds the PR number
  (`↑ #412`) and one dim lid line (`changes asked · checks pass`, `no PR
  yet · 2 commits`, in trunk flow `waits to land · 2nd`); a solo row
  gets the same words on a line under it (`#415 · checks fail`).
  Numbers never change; the rows' order follows the blocks. The divider
  of an agent in a shared place says who else is there: `ψ sb/dark-mode
  with i18n`.
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

1. Main spawns the task with `--pr`, in a new place (a worktree on
   `sb/<name>` from a fresh `origin/<default>`, not your local HEAD: it
   would carry your unpushed commits) or in the place of agents already
   on that change; the brief says "done when: its PR is open" or "your
   part is on the branch".
2. The agent works and commits small; runs the check.
3. It pushes its branch and opens the PR with `gh pr create` (or `glab mr
   create`): the repo's template if there is one, else what changed, why,
   and how it was tested, in the repo's style. Ready for review unless
   you said draft, or the task is long (draft until its last phase).
4. **The agents of the branch stay alive until it is merged** (the
   user's Q4): review comments, red checks, conflicts (`mergeStateStatus`
   DIRTY or BEHIND: one of them rebases on the base and pushes with
   `--force-with-lease`, the hub holding the branch's other lands
   meanwhile). Idle costs nothing.
5. It never merges, approves, closes, or writes on GitHub (comments,
   replies, resolving threads) unless you allowed it (pr-design §11 Q3).
6. Merged: the hub archives the branch's agents and removes the
   worktree (an agent that also works elsewhere just leaves this place).
   Closed without merge: main tells you; the branch stays.

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

**From a worktree** (bigger change, one agent or several):

1. Each agent commits its own files on the place's branch with `sb land
   --here` (the same private-index commit as above, on the worktree's
   branch instead of main: two agents in one worktree never commit each
   other's half-done files). It runs the check.
2. `sb land` (when the place's work is done: main or the last agent
   says so): the hub rebases the branch on the current main, runs the
   check again if main moved, then fast-forwards main. One land at a
   time (a queue in the hub: no two places race for main). A conflict:
   back to the place's agents, with the files.
3. The worktree is removed when its last agent leaves.

In PR flow the same `sb land --here` commits on the branch, then the
hub pushes it (one push at a time per branch): several agents on one
branch never race each other's pushes.

**A long feature branch**: the agent says it's ready; the hub opens a
review item for you (`approvals is ready to land: 14 commits, +3,120
−410 · 1 land it · 2 show the diff · 3 not yet`), then lands it like
above. Today main does that by hand (`is-ancestor` then `update-ref`).

**Pushing** (the user's Q3): in trunk flow, the hub pushes main after
every land, `[flow] push = true` by default (what this repo does now).
`push = false` keeps the lands local until you ask. A push that fails
(offline, main moved on the remote): the hub fetches, rebases the lands
not pushed yet, tries again; still failing, main says it once.

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

- Places (the user's Q2): "You decide where each task works: the shared
  folder, a new worktree, or the worktree of agents already on that
  change (`--place new|<agent>`). Isolate when two agents would edit the
  same files, when a build must not see another's half-done edits, or
  when the change will be reviewed or thrown away on its own. Share a
  place when the work belongs in the same PR or needs the other agent's
  code now. Say it in your routing line: `dark-mode takes a worktree;
  i18n joins it`."
- PR flow: "This repo ships through pull requests (base `main`). A task
  that changes code ends in a PR: `--pr` (a new branch) or in the place
  of the PR it belongs to. One PR or one per phase: the user chooses;
  ask when they haven't said. Read-only tasks: no branch.
  You never merge; the user does (the inbox asks them when a PR is
  approved with checks passing). GitHub's news about a PR go to the
  agent that owns it; you get a copy: escalate only product calls and
  checks still failing after 2 tries."
- Trunk flow: "This repo ships straight to `main`, through `sb land`. A
  long feature: a branch, then the user approves the land."
- Both: "use `--worktree` ONLY when the user explicitly asks" goes
  (main decides, above); "never push or merge" stays except what the
  flow does itself (`sb land`, the PR's own branch).
- The user's words win for one task: "open a PR", "just commit it".

**A task's prompt** (its place line, by flow and place)

- PR flow, worktree: "branch `sb/x` from `origin/main`, shared with
  <agents> (or: yours alone for now; others may join). Commit only your
  files, with `sb land --here`; run `<check>` first. Open the PR with gh
  (the repo's template) if nobody has; the hub pushes. You stay until it
  is merged: fix the reviews and red checks sent to you, rebase when it
  conflicts (`--force-with-lease`, this branch only, after telling the
  branch's other agents). Never merge, approve, close, or write on
  GitHub."
- Trunk flow, shared folder: "commit nothing by hand: run `<check>`, then
  `sb land "<message>"`. Never `git add -A`, stash, reset, rebase or
  amend here."
- Trunk flow, worktree: "commit your files on the branch with `sb land
  --here`; run `<check>`; when the place's work is done, `sb land`
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

- **Sidebar, places** (picked: A, then the lighter option A of
  sidebar-wt, BISE-306): a box only for a worktree several agents
  share, git facts in its border; alone, an agent is a row with its mark
  (§3.1).
- **Sidebar, trunk flow**: a land waiting shows `…` (in a box's border,
  or as a solo row's mark); ctrl held, its words say `waits to land ·
  2nd`.
- **Main's feed**: `✓ dark-mode landed 3 commits on main (a1b2c3)` (and
  `· pushed` when it pushed); a refused land: `dark-mode can't land:
  login.rs changed on main too. it's rebasing.`
- **Inbox**: the flow question (once per repo); a long branch ready to
  land; a "just commit it" on a shared repo.
- **Header, ctrl held**: what happens to the work, after the folder:
  `~/acme · lands via PRs` or `~/acme · lands on main` (designer: "PRs"
  alone reads like a count next to `↑ 3 PRs`).
- `/flow`: shows the flow in the same words, why (the signal), and
  switches it.

## 8. Recommendation

- PR flow is the default for any repo with someone else in it; trunk
  for a repo that is yours alone. Detected, asked once, saved; forced
  when the repo protects its branch.
- Build `sb land` first, even before the PR UI: it turns what this repo
  does by hand into one checked step and fixes the shared-index lag. It
  is phase 0b in the plan.
- The flow goes into the prompts from the config; briefs stop carrying
  git rules.

- Main decides the places; a place (a worktree and its branch) can hold
  several agents, and a PR belongs to the branch. That changes the hub's
  model (a place table instead of one `ws` per agent): phase 0b.

## 9. Open questions (for the user)

Answered: Q1, Q2, Q3, Q4, Q5 (§0); Q6, the sidebar: A, grouped, with
git in borders and agents in rows (§3.1).
Answered later (2026-10-01): **Q7, who rebases a shared branch that
conflicts: the agent the conflict's files belong to**, the branch's other
lands held meanwhile; main picks when the files are several agents'.
