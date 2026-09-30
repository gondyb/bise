# Approvals: plan for the build (start here)

Status: design done, not started. Design:
[approvals-design.md](approvals-design.md) (the 3 modes, the options, the
card, what changes). Reference for the classifier, the hard rules and the
wire: [approvals.md](approvals.md) (d22c024). Nothing is built until the
user says go.

## 1. The user's decisions (firm, 2026-10)

- 3 global modes: `yolo` (default), `accept edits`, `auto`.
- `shift+tab` cycles them; the last pick is remembered (config.toml).
- What needs the user: a `confirm` card in the user inbox (BISE-299).
- `auto` runs on the `classify` role (BISE-298).

## 2. The choice for `accept edits` (design §2)

| option | 1 line | cost |
|---|---|---|
| (a) edit tools | only `apply_patch` counts as an edit: exact, but models drift to bash | steering ~0.5 day; a new `edit` tool +1.5 days |
| (b) parse bash | recognizing every bash edit is impossible; recognizing safe reads is easy | ~2 days |
| (c) classifier | handles scripts, but slow, can be fooled, makes the mode a guess | ~3 days |
| (e) OS sandbox | real containment, but changes the mode's meaning and breaks caches/worktrees | 1–2 weeks |
| **(d) mix (reco)** | `apply_patch` is the only edit; a conservative check lets safe reads and `sb` run; a bash write is denied once with "use apply_patch", then a card; the classifier only in `auto` | the phases below |

## 3. Open decisions for the user

The 7 decisions of the previous plan: 4 (in-project edits skip the model)
and 6 (the model) are settled by the new design; 1 is settled by designer
(`yolo` checks nothing, or the name lies). Still open:

1. `accept edits` asks for every command that is not a read or `sb`
   (`cargo test`, `git commit`…). OK as the user said, or also let the
   classifier allow safe commands there (then it is close to `auto`)?
2. Bash edits in `accept edits`: denied once with the "use apply_patch"
   hint, then a card on repeat. OK?
3. Add an `edit` tool (exact string replace, the format Anthropic models
   know) now, or only if the data shows Opus keeps editing with python?
4. Away: no timeout by default (the agent waits, costs nothing). OK, or a
   default `approvals_timeout` (say 30 min, then deny)?
5. Writable roots for `accept edits`: the agent's own working tree only,
   or also `/tmp`?
6. "always allow … here": per repo only; global rules by hand in
   `~/.bise/approvals.toml` or `/approvals`. Enough?
7. Network (`curl` GET, `npm install`) in `auto`: judged by the model, not
   a hard rule. OK?
8. Switching to `yolo` while cards wait: the cards stay until answered
   (reco), or `yolo` allows them all at once?

## 4. Phases

Ids from HEAD's tracker at launch (next free today: BISE-301).

| phase | what | where | cost |
|---|---|---|---|
| 1. modes + gate | mode in the hub, `approvals` key in config.toml, `shift+tab` cycle + outdent on backspace, the key-bar indicator, flash, first-run tip, `/approvals`; runtime gate (wire, gate file, pause, interrupt, mode file so `yolo` costs nothing); `confirm` answers go to the gate; the waiting-agent signals; `accept edits` rules (in-repo `apply_patch`, safe reads, `sb`, the bash-write hint); tool-desc + prompt steering | `bend/runtime/main.bend` + pure laws, `bend/hub/core.bend`, `rust/switchboard`, `rust/tui` (`input.rs`, `mdlive.rs`, key bar, cards), `rust/catalog` | ~4 days, 2–3 agents in parallel |
| 2. auto | hard rules H1–H10, the classifier on the `classify` role (one-shot path, 6 s, strict JSON), deny-and-continue, failure mode + notice, the "auto-confirm" row in `/setup` roles, its cost in `/usage` | `rust/switchboard` (approvals.rs), `rust/tui/src/onboarding` | ~4 days |
| 3. comfort | "always allow … here" (`~/.bise/approvals.toml`), grouped cards, terminal notification when unfocused, `bise approvals` CLI | `rust/switchboard`, `rust/tui`, `rust/harness` | ~2 days |
| 4. proof | eval (~150 labeled calls on 2–3 small models: share reaching the user, latency, cost); e2e with the fake provider (card in main's and another view, allow / no / ctrl+c); the steering measured (bash edits vs `apply_patch`, before/after) | `tests` | ~2 days |

Phase 1 alone gives `yolo` and `accept edits`. Phase 2 can start with
phase 1 (the rules core is pure). Launch criterion for `auto`: phase 4
green, and on our own transcripts fewer than 1 call in 50 reaches the user.

## 5. When it lands

- Landing / README: a line about the modes (designer asked to be told).
- Onboarding: the one-time tip "you're in yolo: agents run commands without
  asking. ⇧⇥ changes it."
