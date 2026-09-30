# Approvals: plan for the build (start here)

Status: spec done ([approvals.md](approvals.md), d22c024), not started.
The user postponed the build (2026-09-29). This page is the entry point
for the next session: confirm the 7 decisions, then launch the tasks.

## 1. Decisions to confirm (main's recos, not yet approved)

| # | question (spec §10) | reco |
|---|---|---|
| 1 | `yolo`: no checks at all, or keep some hard rules? | no checks: yolo is an explicit choice |
| 2 | "always" on hard-rule cards? | only for H4 (`git push` to main) per project; the other hard rules ask every time |
| 3 | user away: wait forever or deny after a timeout? | wait (no default timeout): an auto-deny makes agents work around it |
| 4 | in-project edits skip the classifier (fast path)? | yes, git is the safety net |
| 5 | network judged by the model, not a hard rule? | yes |
| 6 | classifier model | the agents' provider's small model (no new party sees the data); `mistral-small-latest` as an option |
| 7 | "always" per project only, global rules by hand or `/approvals`? | yes |

## 2. Tasks

Full table in [approvals.md §9](approvals.md). Two fixes before launch:

- **Ids:** the spec says BISE-230..239 "to confirm". Take the next free
  numbers from HEAD's tracker (BISE-221 was the last used on 2026-09-29).
- **Paths moved** by the root cleanup (005c14b): `runtime/` → `bend/runtime/`,
  `hub/` → `bend/hub/`, `LAWS.bend` → `bend/LAWS.bend`.
- **Memory file:** the spec puts it in `~/.bend-harness/approvals.toml`;
  use the bise home (`~/.bise/`) unless there is a reason not to.

- **UI changes to approvals.md §7** (designer, cards round 2, mocks 1b6627d):
  approvals are cards in the same strip + box as every card (see the cards
  proposal, all-screens top 14). (a) no alt+1/2/3 (⌥0-9 switches agents):
  digits 1/2/3 answer on an empty composer while the card is open;
  (b) the note is "type + ⏎" in the open card (⏎ with text = deny with that
  note), alt+r stays as a shortcut from the composer. An approval never
  opens by itself while you type (its row pulses once); it may open by
  itself when the composer is empty and idle.

Order: rules core + runtime gate in parallel → classifier + hub plumbing →
card kind in sb-core → TUI card, TUI signals, settings, away notice in
parallel → eval + e2e last (its eval part can start after the classifier).

Launch criterion: the eval + e2e task green, and on our own transcripts
fewer than 1 call in 50 reaches the user in `auto`.

## 3. When it lands

- Onboarding: add the line "i ask you before risky commands" (spec: settings task).
- Landing / README: say it (designer).
