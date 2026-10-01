# Pull requests: plan for the build

Status: design done, not started. Design: [pr-design.md](pr-design.md).
Mock (local only): `http://localhost:4747/content/pr-support.html`.
Nothing is built until the user picks (design §11) and says go.

Depends on designer's "less on screen" spec (approved; ui-simplify builds
it: the right-aligned columns, the gust on the divider, the ctrl-held
words). Phase 1's UI lands after it; the PR mark goes in ψ's column, so
it needs no new column.

## 1. Phases

Costs are agent-days of build, gate included (`cargo test`, the TUI
tmux tests). Each phase ships on its own and is useful without the next.

| # | What | Where | Cost |
|---|---|---|---|
| 0 | **Conventions only.** `sb spawn --pr` (implies `--worktree`, base `origin/HEAD` after a fetch, adds "done when: its PR is open" and the PR rules to the brief); main's prompt: a PR request is a worktree request; the agent prompt's PR rules (design §5.3); approvals tier rules (push to its own branch, `gh pr create\|view\|checks\|diff` allowed in auto; merge, approve, comment, `gh api` writes, push to the default branch always ask). No UI. | switchboard (cli, prompts, worktree), approvals/tiers | 1 |
| 0b | **The flow** ([dev-flow.md](dev-flow.md)). `[flow] mode = "pr" \| "trunk"`, `check`, `push` in the repo's config; detection (protected branch, other committers, AGENTS.md) and the one-time question; the Flow section written into main's and the tasks' prompts from the config (the worktree rule becomes "when the flow says so, or the user asks"); `sb land` (shared folder: the agent's files through a private index, compare-and-swap on main, the shared index synced; worktree: rebase, check, fast-forward; one land at a time); the approvals rows per flow; `/flow`; the feed's "landed" lines. | switchboard (config, cli, prompts, worktree, approvals/tiers), tui (`/flow`, feed lines) | 2.5 |
| 1 | **See the PRs.** `Forge` trait + GitHub through `gh api graphql` (one query per repo per tick, aliases per branch; cadence and back-off of design §7); `PrSnapshot` on the hub's agent and in the TUI snapshot; the sidebar cell, the held line, the divider link (OSC 8, `links.rs`), the header's held count; feed lines for opened / merged / closed; merged → archive + remove the worktree with no backup (fixes RFC 0002 §5.1's squash case); `bise doctor` line; legend row `↑`, ASCII `#`. | switchboard (new `forge/` module, daemon tick, model, journal), tui (panel, chrome, theme) | 3 |
| 2 | **Act on the news.** Reviews, comments and failing checks to the owning agent (`@ github → <agent>`, main copied), with the details query (threads, authors, `authorAssociation` filter, `gh run view --log-failed` tail); main's prompt for escalation; the 2-tries cap on a failing check → an inbox item; `/prs`. | switchboard (router, prompts), tui (`/prs`) | 2.5 |
| 3 | **Ready to merge.** Inbox kind `merge` opened by the hub (design §6.3; variant A), `gh pr merge` with an allowed method, withdraw on change; first-PR tips (gh logged in / not). | switchboard (cards), tui (item view options) | 1.5 |
| 4 | **Edge cases.** PR from the shared folder (the agent's files to a branch through a private index, then a worktree on it, design §5.2); your own branch's PR in `/prs`; a branch based on another agent's branch (stacked PRs: base = that branch, retarget after its merge). | switchboard (worktree) | 2 |
| 5 | **GitLab.** `Forge` for GitLab through `glab api graphql` (MRs by `sourceBranches`, approvals, pipeline, merge); `!88` and "pipeline" in the words; remote detection. Needs a GitLab repo to test on. | switchboard (forge), tui (words) | 2 |

Total: about 14.5 agent-days; 0 + 0b + 1 (6.5 days) already answer the
user's ask (several branches, their PR state on screen, and the right
flow per repo). 0b is useful alone, in this repo first: it replaces the
private-index landing every brief repeats today.

Running costs: no model tokens for polling (the hub, not a model, asks
GitHub). GitHub: 1 point a query, at most ~240 queries an hour with 10
PRs (5 % of the 5,000). Tokens: one message per event to the owning
agent, plus main's one line.

## 2. How to split it (parallel agents)

- Phase 0 alone first (small, prompts and approvals: one agent).
- Phase 1 in two agents with a frozen contract (`PrSnapshot` in the TUI
  snapshot, design §10): **pr-hub** (forge, tick, journal, events,
  merged cleanup) and **pr-tui** (cell, held line, divider, legend),
  pr-tui against a fake snapshot.
- Phases 2 and 3: one agent each, after 1 (both touch the router and the
  cards; 3 starts after 2's router change lands).
- 4 and 5 when the user asks.

## 3. Tests

- A fake `gh` on `PATH` (a script reading fixtures by query) for the hub:
  open → changes requested → checks fail → pass → approved → merged;
  `gh` missing; 401; rate limit (back-off); a PR on an unknown branch.
- Panel snapshots (`panel.rs` tests): the cell's 4 states, draft, held
  line cut with `…` at 24 and 31 columns, ASCII `P`, NO_COLOR.
- tmux test (like `tui_approvals_tmux`): a PR goes from dim to red to
  pink, the inbox item, `1` calls the fake `gh pr merge`, the row turns
  ✓ and leaves.
- One manual run on a real repo (gh logged in): open a draft PR from an
  agent, review it on GitHub, watch the cell.

## 4. Risks

- **Prompt injection through comments** on public repos: the author
  filter and quoting (design §6.2) are in phase 2, not later.
- **Your identity**: commits, PRs and (if allowed) replies are yours.
  The rules of design §5.3 are prompt rules plus approvals cards in auto;
  in yolo only the prompt holds them (same as `git push` today).
- **A loop** of agent fixes on red CI: capped at 2 tries per check.
- **Stale state** offline: the cell goes faint, ctrl says how old.
- **gh output changes**: we use GraphQL fields, not `gh pr view`'s text.

## 5. Open questions

The user's, in design §11 (the glyph, merge from the inbox, replies on
GitHub, PR = worktree, the base, draft, auto-archive, routing, the PR
body line). For the build only:

- Does `gh api graphql` with 20+ aliases stay at cost 1? (measured with 2;
  check with 25 before phase 1 freezes the query).
- GitLab's "changes requested" field across versions (phase 5).
- Where the poller lives: the daemon's tick thread or its own thread
  (`gh` takes ~0.5 s; it must not block the tick).
