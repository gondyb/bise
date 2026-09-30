# Approvals: design (3 global modes)

Status: design, not built. Task `approvals`, 2026-10. Plan and costs:
[approvals-plan.md](approvals-plan.md). The older spec
[approvals.md](approvals.md) (d22c024) stays the reference for the
classifier, the hard rules and the wire; this page says what changes.

## 1. The user's decisions (firm)

- 3 modes, **global**: one mode for main and every agent, never per agent.
  - `yolo`: every call runs. Nothing asks. No exceptions (not even the hard
    rules: designer's rule, "if a card fires in yolo, the name lies").
  - `accept edits`: file edits run without asking. Everything else asks.
  - `auto`: a small model judges each call; the safe ones run, the risky
    ones ask.
- Default: `yolo` (today's behavior).
- `shift+tab` cycles the mode, like Claude Code, Codex and Vibe.
- The pick is remembered across restarts and sessions.
- What needs the user is a `confirm` card in the **user inbox**
  (BISE-299: `open_confirm` in `bend/hub/core.bend`, not called yet). Only
  the user answers user-inbox cards.
- `auto` runs on the model role `classify` (BISE-298: `[roles] classify`,
  `BISE_CLASSIFY_MODEL`; unset = the `small` role). Its row in `/setup`
  roles ("auto-confirm") ships with this feature.

## 2. The problem: editing through bash

The agents have `bash`, `run_typescript`, `apply_patch`, `skill` and
`search_tool_functions`. `apply_patch` (V4A) exists, but the models edit
mostly through bash: `sed -i`, `cat > f <<EOF`, `python3 - <<EOF` scripts
(Opus's favorite), `perl -pi`. A rough count over every thread
(`sb history --role tool`, hits include reads of these strings): `apply_patch`
376, `sed -i` 956, `cat >` 2 397, `python3 - <<` 3 213. So most edits today
are bash calls.

To say "this bash call only edits files in the repo" is not possible from the
text: `python3 - <<EOF`, `make fmt`, `cargo run`, `git apply`, `npm run x`,
a script written then run. Every one can edit, delete, or do something else.

### 2.1 The options

| option | how `accept edits` decides | pros | cons | cost |
|---|---|---|---|---|
| (a) edit tools | only `apply_patch` (or a new `edit`/`write_file`) counts as an edit; bash never does | exact: the paths are in the patch headers (`core/patch.bend`, `wire.rs` already read them); predictable | models drift to bash; a bash edit becomes a card | steering only: ~0.5 day. A new `edit` (str_replace) tool: +1.5 days |
| (b) parse bash (Codex's allowlist) | recognize bash edits (`sed -i`, `>`, heredocs) | no model call, fast | recognizing *every* edit is impossible (scripts, `python -c`, build tools); only the conservative direction works: "this is a known safe read" | read allowlist + splitter: ~2 days |
| (c) classifier | the `classify` model says "only an in-repo edit?" | handles python scripts and heredocs | 0.6–0.7 s and ~$0.0003 per call; can be fooled; down with its provider; the boundary is a guess, so `accept edits` is no longer predictable | ~3 days (spec BISE-231), shared with `auto` |
| (e) OS sandbox (what Codex really does) | bash runs in Seatbelt (macOS) / Landlock (Linux), writes allowed only in the repo | the only real containment; any edit form is fine | changes the mode's meaning ("anything that stays in the repo"); breaks caches (`~/.cargo`, `~/.npm`), worktrees (writes to the main `.git`), the `sb` socket; per-OS work; `sandbox-exec` is deprecated | 1–2 weeks |
| (d) mix | (a) + the conservative half of (b); (c) only in `auto` | see below | see below | the plan |

Codex does not parse bash to find edits: it parses only to allowlist known
safe commands, and relies on its sandbox for the rest. Vibe's
`accept-edits` does (a): its `edit`/`write_file` tools run, `bash` asks
except for its read-only allowlist (it splits commands with tree-sitter,
`_shell_permission_analysis.py`). Claude Code's `accept edits` is the same:
Edit/Write run, Bash asks.

### 2.2 Recommendation: (d)

1. **`apply_patch` is the edit path.** Only an `apply_patch` call counts as
   an edit. We never try to recognize an edit inside bash.
2. **A conservative static check** (the safe half of (b)): it recognizes
   only calls that are clearly safe: `sb …`, read-only commands on paths in
   the repo (`ls cat rg grep head tail wc git status/log/diff/show…`, the
   spec's fast-path list §4.2). If it is not sure, the call is "not safe".
   A wrong "not safe" costs a card, never a silent run.
3. **Steering, so models use `apply_patch`:**
   - the bash tool description: "edit files with `apply_patch`, not with
     `sed -i`, redirections, heredocs or scripts";
   - one line in the agent prompt;
   - in `accept edits`, a bash call that *looks* like a write (`sed -i`,
     `>`/`>>` to a file, `tee`, a heredoc into a file or into
     `python`/`perl`/`node`) is **denied and continued** the first time,
     with no card: "accept edits: bash edits ask the user. Use apply_patch:
     it runs without asking. If bash is really needed, repeat the call and
     the user will be asked." The model retries with `apply_patch` in the
     same turn. This check can be loose: a mistake costs one retry.
4. **The classifier is only for `auto`.** `accept edits` stays a fixed
   rule the user can predict.
5. Later, if the data shows Anthropic models keep writing python edits: add
   an `edit` tool (exact string replace, the format they are trained on).
   Not in the first build.

## 3. What each mode does

Gated calls: `bash` (top level and `bash()` in programs), `apply_patch`,
every connector call (`tools.<group>.<fn>`, one gate per call). Never
gated: `search_tool_functions`, `skill`, `self.*`, the `run_typescript`
wrapper (its isolate has no files and no network; its tool calls are
gated one by one).

| call | `yolo` | `accept edits` | `auto` |
|---|---|---|---|
| `sb …` (every part of the command) | runs | runs | runs |
| safe read (fast path, spec §4.2) | runs | runs | runs |
| `apply_patch`, every path an in-repo edit (§4) | runs | runs | runs |
| `apply_patch` outside the repo or on a protected path | runs | card | card |
| bash that looks like a write | runs | denied once with the `apply_patch` hint, card on repeat | classifier |
| any other bash (`cargo test`, `git commit`, `curl`…) | runs | card | classifier |
| connector call | runs | card | classifier |
| hard-rule hit (spec §4.1: push to main, secrets, shared-tree wipes…) | runs | card, no "always" | card, no "always" |
| a rule the user saved ("always allow … here") | runs | runs | runs (hard rules still ask) |

Plain words: `accept edits` asks for every command that is not a read or
`sb`. With 5 agents that is many cards; "always allow cargo test here"
cuts it down. The user asked for this mode; the doc says the cost.

### 3.1 `auto` in detail

The spec's pipeline (approvals.md §4), unchanged: hard rules → fast path →
saved rules → turn cache → classifier. The classifier input stays stripped
(tool, arguments, the user's own words, the brief as untrusted context, the
roots), the prompt is Vibe's adapted, strict JSON, policy re-applied in
code.

**On doubt:**

| classifier says | result |
|---|---|
| safe, or medium risk in the repo | runs |
| risky, not asked by the user | denied and continued: the agent gets the reason and "if no safer way exists, repeat the call and the user will be asked" (one dim line in the thread, no card) |
| risky, but the user asked for it | card |
| a repeat of a denied call, 3 denials in a row, 20 in a turn | card |
| error: timeout (6 s), bad JSON, no key, provider down | card, reason "couldn't check this call". Never a silent run. After 3 errors in a row: one notice in main's feed, and 2 minutes where every non-fast-path call is a card |

Model: the `classify` role (BISE-298). Default = the `small` role, so no new
provider sees the data. Measured on `mistral-small-latest`: 0.6–0.7 s,
~2.1 k input tokens (cached), ~40 output tokens per call.

## 4. What counts as an edit

An `apply_patch` call is an in-repo edit when **every** path in its headers
(Add, Update, Delete, Move):

- resolves (after `..`, `~`, symlinks of the parent directory) inside the
  **agent's own working tree**: the shared workspace for the agents that
  work there, its private worktree for an agent in a worktree. Not another
  agent's worktree, not another repo, not `/tmp` (open question 5);
- is not a protected path: `.git/` (hooks, config, index), `.envrc`,
  `.bise/` and `.bend-harness/` if present, and the spec's H2 list for
  paths outside the repo.

A delete is an edit. Git recovers a tracked file; an untracked file
deleted by a patch is lost (said in `/help`).

## 5. The mode: switch, show, remember

- **Where it lives**: the hub holds the live mode and applies it to every
  agent. A switch applies to each agent's next gated call; a call already
  waiting on a card stays a card.
- **Remembered**: `approvals = "yolo" | "accept-edits" | "auto"` in
  `~/.bise/config.toml`. `shift+tab` writes it (same writer as
  `bise config set`). Absent = `yolo`. Every session, every repo, after a
  restart: the last pick. `bise config get/set approvals` works.
  `BISE_APPROVALS` wins for that session and is never written; a `shift+tab`
  then switches this session only and the flash says so.
- **Indicator** (designer): at the right end of the key bar, always on, dim,
  with its key: `⇧⇥ yolo` (ASCII `shift+tab yolo`). The same dim for all
  three modes; never the error color, never accent at rest. Zen fades it
  like the rest of the chrome. On a narrow screen the key bar drops its
  other hints first.
- **Switch flash**: for 3 s the key bar becomes one line, the mode word in
  accent and a dim sentence:
  - `yolo · everything runs, nothing asks`
  - `accept edits · file edits run, everything else asks you`
  - `auto · a small model runs the safe calls and asks you about the risky ones`
- **First launch**: a one-time tip (the BISE-61 box): "you're in yolo:
  agents run commands without asking. ⇧⇥ changes it."
- `/approvals`: the mode and the saved rules; `/approvals yolo|accept
  edits|auto` switches it (for a user without `shift+tab`).

### 5.1 The `shift+tab` clash (settled with designer)

Today `shift+tab` outdents a markdown list item in the composer (BISE-276,
`rust/tui/src/input.rs` + `mdlive.rs`) and moves up in the palette, help
and popups.

- Palette, help, popups: keep `shift+tab` while they are open (they own the
  keys, like `tab`).
- Composer: `shift+tab` **always** cycles the mode, with no context rule
  (a context rule means your list outdents when you wanted to switch mode).
- Outdent moves to **backspace at the start of a list item's text**: one
  level out; at the top level it removes the bullet (Notes, Notion, Google
  Docs). `tab` still indents.
- Cycle order: `yolo → accept edits → auto → yolo`.

## 6. The confirm card

A `confirm` card in the user inbox, in the same card box as the other
cards, one at a time, with a counter. Designer's look:

```
┃ ? api-v2 wants to run                                   1/3
┃   $ git push origin main --force
┃   it pushes to main and rewrites its history.
┃   1 allow   2 always allow git push here   3 no
┃   or type why not, then ⏎
```

- `?` in accent (it needs you), `$` in accent (the bash mark), the command
  in text color, the reason dim, the keys like the other cards (digit
  accent, label dim).
- Reason line: in `auto`, the classifier's own words, one sentence (~80
  chars). In `accept edits`: "accept edits: commands ask first." `yolo`
  never shows a card.
- Hard rule: no option 2; the reason says why it always asks: "it rewrites
  main. this one always asks."
- Edit outside the repo: "? api-v2 wants to edit a file outside the repo",
  the path, the first 3 diff lines dim, then "▸ 12 more lines" (ctrl+o).
- A connector call: "? api-v2 wants to call gmail.send_email", the
  arguments cut to 3 lines.
- Keys: `1`/`2`/`3` on an empty composer; typed text + ⏎ = no, with the text
  as the note to the agent. The box never opens by itself while you type
  (its row pulses once); it may open by itself on an empty, idle composer.
- "always allow X here" stores the command prefix (`git push`, `cargo
  test`: program + subcommand words, no flags) for this repo in
  `~/.bise/approvals.toml` (spec §5, path moved from `~/.bend-harness`).
  For a connector: the whole tool. The file is a protected path.
- Identical calls from several agents (same tool, arguments, repo) are one
  card, "? 3 agents want to run"; one answer answers all.
- Once answered, it folds to one line in the feed of the agent in view and
  in main's: `✓ you allowed api-v2: git push origin main --force` /
  `✗ you said no to api-v2: git push… · 'use a branch'`.
- Only the user answers: `open_confirm` puts the card in the user inbox;
  agents' `sb close`/answers are refused (RFC 0003). Main sees "api-v2 waits
  for you, card #12" in its board and can tell you; it cannot answer.

Hub change: today `answer.kind` for `KConfirm` sends the answer as a message
to the agent (BISE-299's placeholder). It must instead write the verdict to
the waiting call's gate (allow / deny + note), save the rule on "always",
and fold the card.

## 7. The waiting agent

- Its turn is paused inside the gate: the call has not run, no model call,
  no tokens.
- Status `waiting on you` (the existing `waiting_on`, set to the user): the
  panel row `? api-v2 · waiting for you 1m` with `?` in accent; `sb list`
  and main's board say it too. Its tool row:
  `$ git push origin main --force   ? waiting for you`.
- Messages to it queue as usual; it reads them after the call.
- ctrl+c on it, or `sb interrupt`, ends the wait: the call does not run,
  its result is "interrupted by the user", the card closes.
- The other agents keep working.
- The wire (spec §3, unchanged): the runtime prints `gate <n> <json>`, then
  polls a gate file (20 ms, then 100 ms) and the interrupt file; the hub
  answers `<n> <nonce> allow|deny <reason>`. A hub restart keeps the card
  (journaled) and the REPL keeps waiting.
- In `yolo` the runtime does not ask at all: the hub writes the mode next to
  the gate file, the runtime reads it per call (cached on mtime). `yolo`
  costs no latency.

## 8. `sb`, background jobs, timeouts

- **`sb` commands** run in every mode, with no card: they are how agents
  work together, and they reach only the hub, which already refuses what
  needs the user. In a chain (`sb report … && git push`), each part is
  judged; the chain runs only if every part may run.
- **Background jobs** (`cmd &`, `nohup`, a dev server): judged once, when
  launched. What the process does later is not seen by the gate. In
  `accept edits` a background launch is a command, so it asks.
- **`run_typescript` programs**: each tool call inside is gated on its own;
  a card pauses the program at that call.
- **Timeouts**: none by default. A waiting agent costs nothing, and an
  auto-deny teaches agents to work around the gate. `approvals_timeout = N`
  (minutes) is an option: an unanswered card then denies with "the user
  did not answer in N min; do something else or ask later".
- **You are away**: a terminal notification (OSC 9/777, else the bell) when a
  card opens and the terminal is not focused. TUI closed, hub running: the
  cards stay; the next `bise` shows them.
- **Headless** (no hub: the scripted tests, the bench, a bare `repl-live`):
  no gate, unless `BISE_APPROVALS` is set; then a card is a denial.

## 9. Security limits (plainly)

- **Not a sandbox.** The gate reads the text of a call. It cannot see what a
  program does once it runs: `cargo test`, `make`, a script, a background
  job can do anything the user can. Only an OS sandbox contains that
  (option (e), later).
- **The classifier can be fooled.** Its input includes arguments the agent
  wrote, maybe after reading a hostile file or page. A crafted command can
  get "low risk" from a small model. The defenses (no tool results or file
  contents in its input, arguments marked as data, authorization only from
  the user's own words, strict JSON, policy re-applied in code, hard rules
  first) reduce this; they do not remove it. `auto` stops mistakes and
  casual prompt injection, not a determined attacker.
- **`accept edits` is predictable but not tight**: an in-repo edit can
  change a file that runs later (`Makefile`, `package.json` scripts, a test).
  The next command that runs it asks, but its card shows `make`, not what
  the Makefile now does.
- **Obfuscation**: `base64 -d | sh`, aliases, symlinks, `python -c`. Some are
  hard rules (pipe to a shell); the rest the classifier may miss.
- **Answering its own card**: agents cannot answer user-inbox cards; the gate
  file sits in the hub state dir (protected) and each answer carries a nonce
  the model never sees. An agent that already runs arbitrary code could
  forge one; reading answers from the REPL socket closes this (checked in
  the build).
- **`yolo`** checks nothing, and it is the default. The key bar says it at
  all times.

## 10. Changes from approvals.md (d22c024)

| approvals.md | now |
|---|---|
| modes `auto` / `ask` / `yolo` | `yolo` / `accept edits` / `auto`; `ask` is gone (`accept edits` replaces it) |
| default `auto` | default `yolo`, the last pick remembered |
| switch: `/approvals`, config, env | `shift+tab` (writes config.toml), `/approvals` kept, env for one session |
| status bar shows the mode only when not `auto`, `yolo` in the error color | always shown, dim, `⇧⇥ <mode>` at the right of the key bar |
| card kind `approval` in the shared card list | `confirm` card in the user inbox (BISE-299) |
| keys `1/2/3` + `alt+1/2/3`, `alt+r` note | `1/2/3` on an empty composer; type + ⏎ = no with a note (cards round 2) |
| `approvals_model`, default `small_model` | the `classify` role (BISE-298), default the `small` role |
| memory in `~/.bend-harness/approvals.toml` | `~/.bise/approvals.toml` |
| in-project `apply_patch` skips the model (decision 4) | settled: it is what `accept edits` means |
| bash edits: fast path does not know them | `accept edits`: denied once with the `apply_patch` hint, then a card |
| paths `runtime/`, `hub/`, `LAWS.bend` | `bend/runtime/`, `bend/hub/`, `bend/LAWS.bend` |
| ids BISE-230..239 | taken at launch from HEAD's tracker (next free today: BISE-301) |

Unchanged: the gate wire (§3), the hard rules H1–H10 (§4.1), the fast path
(§4.2), the classifier (§4.3–4.5), the memory format (§5), the security
notes (§8).
