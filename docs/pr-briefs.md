# PRs and the dev flow: one brief per agent

Main spawns these when the user says go. Design:
[pr-design.md](pr-design.md) (the PR UI, §4.1 the worktree boxes),
[dev-flow.md](dev-flow.md) (the flows, places, `sb land`, the prompts),
plan: [pr-plan.md](pr-plan.md). Mock (local):
`http://localhost:4747/content/pr-support.html`. The user's answers:
dev-flow §0 and §9; designer signs off the UI.

## Order (waves)

| Wave | Agents | Why this order |
|---|---|---|
| 1 | **flow-hub** (places + `sb land` + flow config) | everything else sits on places: a PR belongs to a place's branch |
| 2 | **flow-prompts** (prompts + approvals rows + detection + `/flow`), **pr-hub** (forge, polling, snapshot), **pr-tui** (boxes, ↑, divider, held lines) | in parallel once wave 1's contract (§Contracts) is on main; pr-tui against a fake snapshot |
| 3 | **pr-news** (reviews / checks to agents, escalation, `/prs`), then **pr-merge** (ready-to-merge inbox item) | both touch the router and the inbox; pr-merge after inbox-redesign has landed |
| later | GitLab, PR from the shared folder, stacked PRs | when the user asks |

## Rules for every agent (paste into each brief)

- This repo is trunk flow until wave 1 lands: work in your own worktree
  (`tests/gate.sh new <name>`), land on main with a private index built
  from the current HEAD and a compare-and-swap `update-ref`, a patch of
  your own hunks only, never `git add` of whole files; never stash,
  reset, amend or rebase the shared folder. After wave 1: `sb land`.
- `tests/gate.sh` (quick) per commit, `gate.sh full` once at the end,
  in the foreground; `gate.sh done <name>`.
- Never touch the live hub; tests use a temp `HOME` and a fake `gh` on
  `PATH` (fixtures by query). Never print a token; bise never reads one
  (`gh` does).
- Any UI text or look the design doesn't fix: ask designer.
- Done = the brief's list, the gate green, a report with the commits,
  what you didn't do, and 3 lines "how to try it".

## Contracts (frozen by wave 1)

```rust
// switchboard: a place, shared by agents (dev-flow §3.1)
struct Place { id: String, kind: PlaceKind /* Shared | Worktree */, path: String,
               branch: Option<String>, base: Option<String>, agents: Vec<String>,
               pr: Option<PrSnapshot> }
// the TUI snapshot gets `places: Vec<PlaceView>`; an Agent gets `place: String`
struct PlaceView { id: String, branch: Option<String>, agents: Vec<String>,
                   pr: Option<PrView>, lid: Option<String> /* the held line */ }
struct PrView { number: u64, url: String, state: PrState, review: Review,
                checks: Checks, stale_ms: Option<u64> }
```

`PrSnapshot`, `PrState`, `Review`, `Checks`: pr-design §9. The hub fills
`pr` from wave 2 on; until then it is always `None`.

## flow-hub (wave 1, ~3 days)

Objective: places in the hub and `sb land`, the base for PR and trunk
flows (dev-flow §3.1, §5).
- A place table replacing one `ws` per agent (journal migration: each
  agent's `ws` becomes a place of its own). `sb spawn --place
  new|<agent>|<branch>`, `sb move <agent> <place>` (only an agent that
  changed nothing yet; `/isolate` becomes `sb move <agent> new`). Drop
  and restore counted per place (RFC 0002 §5 rules): the worktree goes
  with its last agent.
- `sb land --here "<msg>"`: the agent's own files (the hub's per-agent
  file list) through a private index from the place's branch tip, CAS
  `update-ref`, the place's index synced for those paths. `sb land`:
  from a worktree, rebase on main, run `[flow] check`, fast-forward main;
  from the shared folder, the same as `--here` onto main. One land at a
  time per target ref (a queue). Push after the land when `[flow] push`
  (default true in trunk flow); a failed push fetches, rebases, retries
  once, then reports.
- `.switchboard/config.toml` `[flow] mode, check, push` read (detection
  and the question are flow-prompts').
- The TUI snapshot carries `places` (contract above).

Done when: the contract on main; `sb land` lands this repo's own work
(dogfood it on your last commit); tests for the queue, the CAS race, an
overlap refused, a shared worktree with 2 agents, drop of a shared
place.

## flow-prompts (wave 2, ~1.5 days)

Objective: the Flow section in main's and the tasks' prompts, from the
config (dev-flow §6).
- Replace "use `--worktree` ONLY when the user explicitly asks" by the
  placement hints; the per-flow place lines for tasks; the commit style
  and check command.
- Detection (dev-flow §2: protected branch via `gh api`, other
  committers in 90 days, AGENTS.md) and the one-time question as an
  inbox item; `/flow` (show, why, switch).
- Approvals rows per flow (dev-flow §6 table) in `approvals/tiers`.
- Main's feed lines for lands (`✓ dark-mode landed 3 commits on main
  (e4f5a6b) · pushed`).

Done when: prompts tests (`prompts.rs`), approvals tests per flow, the
question asked once and saved, `/flow`.

## pr-hub (wave 2, ~2.5 days)

Objective: follow the PRs (pr-design §7-§10).
- `forge/` module: the `Forge` trait, GitHub through `gh api graphql`
  (one query per repo per tick, an alias per place branch; check that 25
  aliases still cost 1 point before freezing it), cadence and back-off,
  never on the tick thread.
- `PrSnapshot` on each place; events `pr_seen / pr_changed / pr_merged
  / pr_closed / pr_unreachable`; the journal keeps the number.
- Merged: archive the place's agents, remove the worktree, no backup
  when the PR's head is the branch tip (RFC 0002 §5.1's squash case).
- `bise doctor`: gh present, logged in, the repo's forge.

Done when: fake-gh tests for the whole PR life (open → changes → red →
green → approved → merged), offline, 401, rate limit.

## pr-tui (wave 2, ~2 days)

Objective: the UI of pr-design §4 and §4.1 on designer's quiet layout.
- The worktree boxes (§4.1's 6 rules), `↑` in the border, the held
  number and lid line, the 24-column cut, a box never split by the
  scroll.
- The divider: `… · yolo · ψ sb/dark-mode · ↑ #412 · ∿∿∿` (the number
  an OSC 8 link), its short-on-room order (branch name, ψ, number, mode
  last); `with i18n` for a shared place.
- The header's held `↑ 2 PRs` and the flow (`lands via PRs` / `lands
  on main`); the legend row `↑`, ASCII `P` (`#` and `^` are taken).
- Not to build: the earlier "↑ in ψ's column of a row" (pr-design §4
  marks it replaced by §4.1).
- After the build: tmux captures for designer at 150 and 90 columns
  (the 24-column panel), ctrl up and held: one shared box with a PR,
  checks failing, a draft, no PR yet, trunk waiting to land, and a
  ready-to-merge item open.

Done when: panel tests at 24, 31 and 44 columns, NO_COLOR, ASCII; a tmux
capture at 150 and 80 columns, ctrl up and held, signed off by designer.

## pr-news (wave 3, ~2.5 days)

Objective: GitHub's news reach the right agent (pr-design §6).
- Reviews, comments (only authors with write access and the repo's
  trusted bots, quoted), failing checks with the log's tail, to the
  agent of the place (the one that pushed the commit, else the one that
  opened the PR, else main picks), main copied.
- The 2-tries cap on a failing check → an inbox question; a product
  call → main escalates.
- Main's feed lines (opened, changes asked, checks fail, back in review,
  merged, closed). `/prs`.

## pr-merge (wave 3, after inbox-redesign, ~1.5 days)

Objective: the "ready to merge" inbox item (pr-design §6.3), on the
inbox as inbox-redesign leaves it.
- Opened by the hub when approved with checks passing; `gh pr merge` with
  a method the repo allows; withdrawn when the PR changes; "not yet"
  quiet until the next change.
- The first-PR lines (gh logged in / not).
