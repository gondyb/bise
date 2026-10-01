# Pull requests: design

Status: design, nothing built. Plan: [pr-plan.md](pr-plan.md). Mock
(local only, gitignored): `http://localhost:4747/content/pr-support.html`
(generator in pr-designer's temp folder; the frames use designer's
`ui2.py` geometry, copied from a real 150-column tmux capture). Drawn on
designer's "less on screen" spec, approved by the user (being built by
ui-simplify), and the inbox's `ctrl+1-9` (BISE-302). Extends [RFC 0002](rfc-0002-worktrees.md) (worktrees) and the inbox
of the book (§12).

## 1. What the user asked

Several branches at once, with agents coding on other branches. Each
agent's PR state from GitHub in the UI: open, approved, changes requested,
merged. GitHub now, GitLab later.

## 2. The answer in one paragraph

An agent that ships code works on its own branch (a hub worktree,
`sb/<name>`) and opens its PR itself with `gh`, as you would. The hub
finds the PR by its branch (nothing to declare) and follows it by polling
GitHub through `gh api graphql`, with your gh login: no new sign-in, bise
never reads the token. In the sidebar every worktree is a small box
(§4.1): the PR's `↑` sits in its top border, dim, red when checks fail,
never pink (pink stays on the inbox item). Ctrl held: its number in the
border and its state on one lid line in the box. The divider of the
agent you view links the PR. GitHub's news (a review,
red checks) go to the agent that owns the PR, main reads them and says it
in one line; main escalates only what is yours (a product call in a
review, checks that still fail after two tries). Approved with checks
passing opens
an inbox item, "ready to merge", and `1` merges with your gh login. No
agent ever merges. Merged: the cell goes, the agent is archived, its
worktree removed.

## 3. How others do it

| Tool | Branch / isolation | Opens the PR | Follows the PR | Auth |
|---|---|---|---|---|
| gh CLI | — | `gh pr create` | `gh pr status`, `gh pr checks`, `gh api graphql` | its own login (keychain), or `GH_TOKEN` / `GITHUB_TOKEN` |
| glab | — | `glab mr create` | `glab mr view -F json`, `glab api graphql` | its own login, or `GITLAB_TOKEN` |
| Conductor | one worktree per workspace | a button | status per workspace in the sidebar (checks running, merged; a "status matrix" in experimental) | gh |
| Vibe Kanban | one worktree per task attempt | "Create PR" (title and body from the task), then "Push" | polls; a merged PR moves the task to Done | gh |
| Claude Code | `claude --worktree <name>` (also from a PR: `--worktree "#1234"`) | the agent runs `gh pr create` | the desktop app shows the PR in the footer (closed/merged PRs were a bug, #24834) | gh |
| Codex | worktrees in the app; cloud tasks | a button (cloud), gh (local) | the sidebar shows review feedback; `@codex review` on GitHub | gh |

What we take: one branch per agent, gh's login, polling, the PR mark
next to the agent, "merged moves it to done". What we leave: buttons
(bise is a terminal you talk to: you ask, main routes), a separate PR
pane (the feed and the inbox already exist), webhooks (§7).

## 4. Where the PR shows

Rule (designer, book §5): color means attention. Dim when nothing is for
you, red only for failing checks, pink only on the inbox item itself
(never on `↑`), never green.

| Place | At rest | Ctrl held |
|---|---|---|
| **sidebar** | **§4.1, the worktree boxes** (this replaces the earlier "cell in ψ's column": every PR has a branch, so every PR agent sits in a box) | §4.1 |
| **divider** (the agent you view) | the approved order model · effort · mode · gust, it has room, so both, the branch first, after the mode, before the gust (the gust is the status, it stays last): `you → dark-mode · sonnet · high · yolo · ψ sb/dark-mode · ↑ #412 · ∿∿∿` (the number is a link), the context short on the right `31k · 15%`. Short on room (designer): the branch name goes first (`ψ · ↑ #412`), then ψ, then the number (`↑`), the mode last | `… · ψ sb/dark-mode · ↑ #412 changes asked · checks pass · ∿∿∿ working · 3m`, right `31k / 200k tokens · 15%` |
| **header** | nothing new (the folder and the inbox count) | `↑ 3 PRs` with the other counts |
| **main's feed** | one level-2 line per event: opened, changes asked (and who is on it), checks fail, back in review, merged / closed (dim) | — |
| **inbox** | only: ready to merge (§6.3), a review question main can't answer, checks that still fail after two tries | — |
| **`/prs`** | a list in the feed: every open PR of this repo's agents, plus the one on your own branch | — |

*(Replaced by §4.1: the earlier "↑ in ψ's column of the row, pink when
an inbox item waits, a faint line under the row when held" is gone. Do
not build it.)*

**The glyph.** `↑` (U+2191): in SF Mono, JetBrains Mono and Menlo
(checked with fontTools on this Mac); nothing in the panel or the divider
uses it (it shows only in key hints, always next to a word). Designer's
first idea `⇡` / `⇣` is missing from SF Mono and JetBrains Mono (a
fallback font draws it). ASCII (`BISE_ASCII=1`): `P` (designer: `#` is
`▣`'s, `^` is `▲`'s, and every ASCII form must be distinct). It joins §6
("marks") and the legend: `↑ a pull request (dim open, faint draft, red
checks fail)`.
The user keeps ψ for worktrees.

**Variants in the mock**: the cell A `↑` (my pick), B `⇡ ⇣`, C the
number `#412` in ψ's place (4 columns), D nothing at rest. Ctrl held: A a
line under the row (my pick), B in the state column (short words only).
Ready to merge: A an inbox item with a merge option (my pick), B one line
from main, you merge on GitHub.

### 4.1 The sidebar: one box per worktree (the user's pick, designer's rules)

Several agents can share a worktree and its branch ([dev-flow.md](dev-flow.md)
§3.1). The user picked the grouped sidebar, with a clear line between an
agent's row and a git line. **Git lives in borders, agents live in
rows.** This replaces the `↑`-in-ψ's-column cell above for agents in a
worktree; agents in the shared folder never had one.

```
 agents
  0 ∿ main :* ✉ 2   1m  22%
  1 ○ sad-404           18%

╭─ ψ sb/dark-mode ────────── ↑ ─
│ 2 ∿ dark-mode     3m  12%
╰ 4 ∿ i18n •       42s  31%

╭─ ψ sb/login-fix ────────── ↑ ─      (↑ red: checks fail)
╰ 3 ∿ login-fix     5m   9%

 inbox
  1 ? sad-404  #409 is approv…

 ▸ 2 archived
```

1. **Order**, top to bottom: the rows in your folder (main first), then
   one box per worktree, ordered by its first agent's number, then the
   inbox section, then `▸ n archived`. One blank row between blocks. The
   inbox section stays a plain dim title with rows, never a box: boxes
   mean git only. Numbers never change; the order follows the blocks.
2. **Colors**: the border's lines, the rail `│` and the closing `╰` in
   the rule color; ψ and the branch dim; `↑` follows the PR rules (dim
   open, faint draft, red when checks fail, never pink: pink stays on
   the inbox item; never green). In trunk flow a box waiting to land
   shows `…` in its border (the border is git only, so it can't read as
   an agent waiting), and its held lid says `waits to land · 2nd`.
3. **Short on room** (the 24-column panel at 90-99 columns): the branch
   name is cut with `…` first; ψ and `↑` stay. A border never wraps.
4. **Every worktree gets a box**, even with one agent: the rule stays
   simple. ψ leaves the rows inside (the box says it), so names get 2
   columns back.
5. **Ctrl held**: the border adds the PR number (`↑ #412`); one lid line
   inside the box, under the border and before any agent, dim, no
   glyph, at the border's text column: `changes asked · checks pass`,
   `no PR yet · 2 commits`, in trunk flow `waits to land · 2nd`. Red only
   on the words `checks fail`. The agent rows get their state words as
   everywhere.
6. **Long lists**: a box never splits across the panel's scroll; if it
   doesn't fit, it goes under `+ n more`, whole.

Mock: the first section of pr-support.html (at rest, ctrl held, 24
columns, trunk flow). When it ships, designer updates bise.dev/design
(its agents panel shows ψ in the rows today).

## 5. How an agent gets a branch and opens a PR

When agents open PRs at all (PR flow) or land straight on main (trunk
flow), which tasks get a branch, and what that changes in the agents'
instructions: [dev-flow.md](dev-flow.md).

### 5.1 A PR request is a worktree request

RFC 0002 says main never picks a worktree on its own. New rule: **a
request that asks for a PR is a request for a worktree** ("fix X, then
open a PR", "one PR each"). Main spawns with `--worktree --pr`; nothing
else changes in RFC 0002 (`/new -w`, `/isolate`, drop, restore).

- Branch: `sb/<name>` (`[worktree] branch_prefix`).
- Base: for a PR task, the remote's default branch after a fetch
  (`origin/HEAD`), not your local `HEAD`: a PR from `HEAD` would carry
  your unpushed commits. New key `[pr] base = "origin/HEAD"` (open
  question 5).
- `--pr` adds to the brief: "done when: its PR is open", the repo's PR
  conventions (template, title style, if any), and the rules of §5.3.

### 5.2 An agent already in your folder

When you ask for a PR from an agent that worked in the shared checkout,
`/isolate` refuses (it has changed files). Main offers: "login-fix
changed 3 files in your folder. i put them on a branch, sb/login-fix,
without touching your folder, and it opens the PR from there?" The hub
does what the docs commits of this repo do: a private `GIT_INDEX_FILE`
from `HEAD`, the agent's files (the hub knows them, RFC 0001 §10.3), a
commit, a branch, then a worktree on that branch; the agent continues
there. Your folder keeps the files until you say otherwise. Phase 4.

### 5.3 Opening and updating

The agent runs, in its worktree: `git push -u origin sb/<name>`, then
`gh pr create --fill --base <default>` (or `glab mr create`). It knows
these tools; no `sb pr open` wrapper. Rules in its prompt:

- push only its own branch; never main, never `--force` to a branch it
  didn't create;
- never merge, approve, close, or post on GitHub (comments, review
  replies, resolving threads) unless you allowed it (open question 3);
- after a fix, push and say so in one report.

**Approvals** (auto): `git push` to the agent's own branch from its
worktree, `gh pr create|view|checks|diff` run without a card (a tier
rule: the branch is the agent's `ws.branch`). `gh pr merge`, `gh pr
review --approve`, `gh pr comment`, `gh api -X POST|PATCH|DELETE` and any
push to the default branch always ask (the hard rule that already covers
`git push origin main`). Yolo: as today, but the prompt rules above
stand.

### 5.4 How the hub links a PR to an agent

By branch: each tick, the hub asks for the PRs whose head is one of the
branches it knows: hub worktrees (`ws.branch`), private worktrees the
agent told it about (`sb worktree <path>`: the hub reads that worktree's
branch), and your shared folder's current branch (shown as "you" in
`/prs`, nowhere else). A PR on another branch: `sb pr link <url>` (the
agent or main), rare.

## 6. What happens to GitHub's news

### 6.1 Who gets what

| Event | Goes to | Then |
|---|---|---|
| PR opened (seen first) | main (level 3) | main: `:* dark-mode opened #412: …` |
| review: changes requested, or new comments | **the owning agent**, main copied. A PR belongs to its branch, and several agents may share it ([dev-flow.md](dev-flow.md) §3.1): "owning" = the agent that pushed the commit the review is about, else the one that opened the PR, else main picks | the agent fixes, pushes, reports; main: one line |
| checks fail | the owning agent (failing check names + the log's last 60 lines, `gh run view --log-failed`) | same; after 2 tries on the same check, main asks you (inbox) |
| approved, checks pass | the hub opens an inbox item (§6.3) | the item is pink in the inbox; the box's `↑` stays dim |
| merged | main | `✓ perf's #401 merged · perf archived, its worktree removed` |
| closed without merge | main | dim line; the agent and its branch stay |
| owning agent archived or dropped | main | main restores it (`/restore`) or answers itself |

Why straight to the agent (and not through main, the brief's question):
the agent owns the code and the context; going through main costs a
model turn and adds latency for nothing. Main still reads every event
(copy, level 3) and steps in when needed. The alternative (main routes
everything) is open question 8.

### 6.2 What main does with review comments

- Comments the brief or the code settles: the agent fixes them; main
  says it once.
- A product call or a disagreement ("settings or the header?"): main
  asks you (inbox, quoted, with file and line); your answer goes to the
  agent.
- Main never answers a reviewer on GitHub by itself.
- **Comments are untrusted text** (a public repo: anyone can comment).
  The hub passes on only comments from authors with write access
  (`authorAssociation` OWNER / MEMBER / COLLABORATOR) and bots the repo
  lists (`[pr] trusted_bots`); others are counted, not quoted ("2
  comments from outside the team, ▸"). Always quoted, never as
  instructions.

### 6.3 Ready to merge

Approved (review decision `APPROVED`, or no review required) and checks
passing: the hub opens an inbox item, kind `merge`, for you (never main's:
BISE-299 rules). `1 squash and merge` (a method the repo allows:
`squashMergeAllowed`, else merge, else rebase), `2 open it on GitHub`,
`3 not yet`. `1` runs `gh pr merge <n> --<method>` with your login. The
item withdraws itself when the PR changes (new commits, a review, merged
elsewhere). "not yet" stays quiet until the PR changes again. Variant B:
no item, one line from main (open question 2).

### 6.4 After the merge

The agent is archived and its worktree removed (local branch deleted;
the remote branch is GitHub's business, RFC 0002 §5.3). RFC 0002 §5.1's
known case (a squash merge leaves the local commits "unpushed") is fixed:
when the PR is merged and its head commit is the branch's tip, nothing is
lost, no backup, no confirmation. `/restore` still works.

## 7. Polling, not webhooks

Webhooks need a public URL: bise runs on your laptop. `gh webhook
forward` is a preview extension that needs admin rights on the repo. A
GitHub App relay would need a bise cloud. So: polling, cheap.

- **One GraphQL query per repo per tick**, an alias per branch:
  `pullRequests(headRefName:, first:1, orderBy: UPDATED_AT)` →
  `number url state isDraft reviewDecision mergeStateStatus headRefOid
  updatedAt` and the last commit's `statusCheckRollup { state }`. Measured
  on this repo: cost 1 point (of 5,000 an hour), 0.47 s.
- Details only on change (`updatedAt` moved): the reviews, the review
  threads (unresolved, with path / line / author / association), the
  failing checks. A second query, only then.
- **Cadence**: 15 s while checks run or for 5 min after the agent's push
  (the hub sees the branch tip move locally, free); 60 s otherwise; 5 min
  when no client is attached; stop at merged / closed. 10 open PRs at the
  fast rate: 240 queries an hour, under 5 % of the budget.
- `gh` errors (offline, rate limit, 401): back off (×2, max 10 min), the
  cell keeps its last state, faint; ctrl held says `state from 12m ago`.
  Never an inbox item for that.

## 8. Auth: no new login

The hub shells out to `gh api graphql` (and `gh pr merge`). gh uses its
own login (keychain), or `GH_TOKEN` / `GITHUB_TOKEN` when set, and knows
GitHub Enterprise hosts and proxies. bise never reads, stores or prints a
token. No gh, or gh logged out: main says it once ("i can't follow it:
gh isn't logged in. `gh auth login` once"), the cell stays off, `bise
doctor` has a line. A direct HTTPS client (token from the env, no gh) is
possible later; not needed now.

Identity, plainly: the agent's commits and PRs are yours (your git
config, your gh login). The PR body ends with one line, "opened by bise
(agent dark-mode)" (open question 9).

## 9. GitLab later: the provider seam

One small interface in the hub, one implementation per forge:

```rust
trait Forge {
    /// The PRs (MRs) whose head is one of `branches`: one call.
    fn fetch(&self, repo: &RepoRef, branches: &[String]) -> Result<Vec<PrSnapshot>, ForgeError>;
    /// Reviews, comments, failing checks since `since` (only on change).
    fn activity(&self, pr: &PrRef, since: &str) -> Result<Vec<PrEvent>, ForgeError>;
    fn merge(&self, pr: &PrRef, method: MergeMethod) -> Result<(), ForgeError>;
}

struct PrSnapshot {
    number: u64, url: String, branch: String, head_oid: String,
    state: PrState,        // Draft | Open | Merged | Closed
    review: Review,        // None | Pending | Approved | ChangesRequested
    checks: Checks,        // None | Running | Pass | Fail(Vec<String>)
    updated_at: String,
}
```

- **Which forge**: the push remote's URL (`git remote get-url origin`):
  `github.com` or a host `gh auth status --hostname` knows → GitHub;
  `gitlab.com` or a host glab knows → GitLab; else nothing.
- **GitHub**: `gh api graphql` (§7).
- **GitLab**: `glab api graphql`, `project(fullPath:) { mergeRequests(
  sourceBranches: [...]) { nodes { iid webUrl state draft approved
  detailedMergeStatus headPipeline { status } diffHeadSha } } }`: one
  query for every branch. "Changes requested" from `detailedMergeStatus`
  / the reviewers' state (to check against the user's GitLab version).
  Merge: `glab mr merge --squash`.
- **On screen**: the same arrow and colors; GitLab's notation `!88`,
  "pipeline" for checks. You can say PR or MR; main understands both.

The TUI only sees `PrSnapshot` (in the agent's snapshot): it never knows
the forge.

## 10. Data and events

- `Agent` (hub): `pr: Option<PrSnapshot>` (runtime), `pr_number:
  Option<u64>` (journal, so a restart knows the link), `ci_tries: u8`.
- Hub events: `pr_seen`, `pr_changed { from, to }`, `pr_merged`,
  `pr_closed`, `pr_unreachable` (gh errors, for doctor).
- The TUI's `Agent`: `pr: { number, url, state, review, checks,
  stale_ms }`; the needs-you color comes from the inbox, as today.
- Inbox kind `merge` (§6.3), opened by the hub, answered by you only.
- Messages to agents: from `github` (a pseudo sender, like a peer:
  `@ github → dark-mode`), folded like any traffic between agents.

## 11. Open questions (for the user)

1. The cell: `↑` (A), `⇡ ⇣` (B), `#412` (C) or nothing at rest (D)?
2. Ready to merge: an inbox item that merges (A) or only a line, you
   merge on GitHub (B)?
3. May agents reply on GitHub to review comments in your name ("fixed in
   a1b2c3")? Default: no.
4. A PR request = its own worktree automatically (changes RFC 0002's
   "only when the user asks")? Recommended: yes.
5. Base of a PR branch: the remote's default branch (fresh) or your local
   `HEAD`? Recommended: the remote's.
6. Draft or ready PRs by default? Recommended: ready (you asked for a
   PR); draft when you say "draft".
7. After a merge: archive the agent and remove its worktree on its own?
   Recommended: yes (`/restore` brings it back).
8. Reviews and red checks: straight to the agent, main copied (my pick),
   or through main?
9. A line "opened by bise (agent dark-mode)" at the end of the PR body?
