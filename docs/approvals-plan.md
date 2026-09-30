# Approvals: plan for the build (start here)

Status: design done, not started. Design:
[approvals-design.md](approvals-design.md) (the 3 modes, the bash parser,
the `edit` tool, the card, what changes). Reference for the classifier, the
hard rules and the wire: [approvals.md](approvals.md) (d22c024). Nothing is
built until the user says go.

## 1. The user's decisions (firm, 2026-10)

- 3 global modes: `yolo` (default), `accept edits`, `auto`.
- `shift+tab` cycles them; the last pick is remembered (config.toml).
- What needs the user: a `confirm` card in the user inbox (BISE-299).
- `auto` runs on the `classify` role (BISE-298).
- Second round (design §1):
  1. a real bash parser (tree-sitter-bash, Vibe's analysis copied) for safe
     reads, saved "always allow" rules per repo (`cargo test *`), and plain
     bash writes;
  2. in `accept edits`, a bash edit the parser cannot read is denied once
     with the "use `edit` / `apply_patch`" hint; a card only on repeat;
  3. a simple `edit` tool (exact string replace) in phase 1, with steering;
  4. no time limit: an agent waits as long as needed;
  5. roots: the current folder and below, `~/.bise` from anywhere, a
     per-agent temp dir `~/.bise/tmp/<agent-id>` (`TMPDIR`), no `/tmp`;
  6. "always allow" is per repo;
  7. network in `auto`: judged by the model;
  8. switching to `yolo` leaves the waiting cards open.

## 2. The choice for `accept edits` (design §2)

| option | 1 line | cost |
|---|---|---|
| (a) edit tools | `edit` + `apply_patch` count as edits: exact | `edit` ~1.5 days, steering ~0.5 day |
| (b) bash parser | reads each simple command: safe reads, saved rules, plain writes; opaque commands stay opaque | ~3 days |
| (c) classifier | handles scripts, but slow, can be fooled, makes the mode a guess | only in `auto` |
| (e) OS sandbox | real containment, changes the mode's meaning | later, 1–2 weeks |
| **chosen: (a) + (b)** | the classifier only in `auto` | phase 1 |

Why the agents avoid `apply_patch` (design §2.4): it is 3 days old and the
long threads kept their habit; no prompt line asks for it and the bash
description invites heredocs; V4A is OpenAI's format, while Anthropic
models know exact string replace (so Opus writes python `replace`
scripts); a small change costs more in V4A than `sed -i`. Not failures:
~2 % of patches fail.

## 3. Open decisions for the user

1. `~/.bise` is a root, except 3 paths i recommend keeping protected: the
   hub state (`hubs/`: an agent could forge a card's answer), the saved
   rules (`approvals.toml`: it could allow itself everything) and the keys
   (`auth.json`). OK?
2. In `accept edits`, plain bash writes the parser can read (`sed -i`,
   `cat > f <<EOF`, `mkdir`, `rm` inside the roots) run like edits.
   That is how i read "spot the obvious bash writes". OK?
3. `edit` and `apply_patch` shown to every model (reco), or one per family
   (Anthropic `edit`, OpenAI `apply_patch`)?
4. The parser is tree-sitter-bash from Rust: a C grammar compiled into the
   hub (Vibe uses the same one). OK, or a hand-written parser (smaller,
   wrong on some heredoc and quoting cases)?

## 4. Phases

Ids from HEAD's tracker at launch (next free today: BISE-301).

| phase | what | where | cost |
|---|---|---|---|
| 1a. modes + gate | mode in the hub, `approvals` key in config.toml, `shift+tab` cycle + outdent on backspace, the key-bar indicator, flash, first-run tip, `/approvals`; runtime gate (wire, gate file, pause, interrupt, mode file so `yolo` costs nothing); `confirm` answers go to the gate; the waiting-agent signals | `bend/runtime/main.bend` + pure laws, `bend/hub/core.bend`, `rust/switchboard`, `rust/tui` (`input.rs`, `mdlive.rs`, key bar, cards), `rust/catalog` | ~3 days |
| 1b. parser + rules | tree-sitter-bash analysis (parts, wrappers, unreadable parts, redirections), safe reads with option guards, plain writes, roots and protected paths, the arity table, saved rules per repo in `~/.bise/approvals.toml` and "always allow … here" on the card; table tests with Vibe's cases | `rust/switchboard/src/approvals/` | ~3 days |
| 1c. `edit` + steering | the `edit` tool (pure core + laws, TUI diff), tool order, the prompt line, the bash description, the deny-once hint | `bend/core/edit.bend`, `bend/runtime`, `prompts/`, `rust/tui` | ~2 days |
| 1d. temp dir | `~/.bise/tmp/<agent-id>`, `TMPDIR`/`TMP`/`TEMP` in the tool env, delete on drop and on hub start | `rust/switchboard` (`tools_env.rs`, drop, sweep) | ~0.5 day |
| 2. auto | hard rules H1–H10, the classifier on the `classify` role (one-shot path, 6 s, strict JSON), deny-and-continue, failure mode + notice, the "auto-confirm" row in `/setup` roles, its cost in `/usage` | `rust/switchboard`, `rust/tui/src/onboarding` | ~4 days |
| 3. comfort | grouped cards, terminal notification when unfocused, `bise approvals` CLI | `rust/switchboard`, `rust/tui`, `rust/harness` | ~1.5 days |
| 4. proof | eval (~150 labeled calls on 2–3 small models: share reaching the user, latency, cost); e2e with the fake provider (card in main's and another view, allow / no / ctrl+c); the edit share by tool, before/after, per model | `tests` | ~2 days |

Phase 1 (1a–1d, ~8.5 days, 4 agents in parallel: about 3 days of wall
time) gives `yolo` and `accept edits`. Phase 2 can start with 1b (the
rules core is pure). Launch criterion for `auto`: phase 4 green, and on
our own transcripts fewer than 1 call in 50 reaches the user. Total
~16 agent-days.

## 5. When it lands

- Landing / README: a line about the modes (designer asked to be told).
- Onboarding: the one-time tip "you're in yolo: agents run commands without
  asking. ⇧⇥ changes it."
