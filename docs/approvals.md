# Tool-call approvals

Status: spec, not built. Task `approvals-spec`. To build before launch.

> 2026-10: the modes changed again: `yolo` (default) and `auto` only
> (`accept edits` = `auto` with the checker off), `shift+tab`, `confirm`
> cards in the user inbox, cheap tiers before a checker (Jev, else the
> `classify` role), `brush-parser`. [approvals-design.md](approvals-design.md)
> §14 lists what changes here; the hard rules, the wire and the memory
> format of this spec stand.

## Goal

Today every agent runs every tool call without asking. We want the least
annoying gate that still stops the dangerous calls:

- **auto** by default: a small model judges each tool call. Most calls run
  at once, without a word. Only the risky ones need the user.
- **Every approval request reaches the user wherever they are**: in main's
  view or in any agent's view. Never only in the asking agent's thread, or
  the agents stay stuck.

User's words: « je veux le truc le moins chiant possible, donc auto avec un
model de classification sur les tool calls ce serait le top, comme dans
vibe. Il faut absolument que tous les approvals soient remonté a main ou la
ou je suis (global) parce que sinon les agents vont toujours etre stuck. »

Non-goal: a sandbox. The gate reads the text of a call; it cannot see what a
program does once it runs. It stops mistakes and casual prompt injection.
Containment against a determined attacker needs an OS sandbox (later).

## 1. What Vibe does (read on this machine)

Source: the installed package `mistral-vibe 2.25.8`,
`~/.local/share/uv/tools/mistral-vibe/lib/python3.14/site-packages/`:
`mistralai_vibe_local_harness/vibe/_smart_approve.py` (the classifier),
`_local_actions.py` (the gate: `_classify_gate`, grants, deny-and-continue),
`_protected_paths.py`, `vibe/core/agents/models.py` (the modes),
`vibe/app_server/_runtime.py` (`ToolGate`, `_rust_tool_modes`),
`vibe/core/tools/builtins/bash.py` (allow/deny lists).

- **Modes** are agent profiles: `plan` (read-only), `accept-edits` (the
  default: edits run, the rest asks), `smart-approve` (the classifier),
  `auto-approve` / `--yolo` (`bypass_tool_permissions`: nothing asked).
  Each builtin tool gets a mode `allow | ask | deny | classify`.
- **Order of the gate** (`_classify_gate`): the static permission resolver
  first (deny lists, allow lists, the user's grants, "outside the project
  directory" checks): a `deny` or an `allow` settles the call with no model
  call. Only the resolver's `ask` residue goes to the classifier. Then the
  session grants, then the model.
- **Classifier input** (`RiskClassificationRequest`), stripped on purpose:
  tool name, arguments (cut at 3000 chars), the last user message (cut at
  2000), the user's answers to clarification questions since then, the
  writable roots. Never the transcript, never tool results, never the
  assistant's text.
- **Model**: `mistral-small-latest`, temperature 0, thinking off, even when
  the session runs another provider (a `classifier_provider` route sends it
  to Mistral). Timeout 10 s.
- **Output**: JSON only, `{"safe", "risk_level": low|medium|high|critical,
  "user_authorization": unknown|low|medium|high, "reason"}`. The code
  re-applies the policy instead of trusting `safe`: critical never runs;
  high runs only with authorization medium or high. Any parse error,
  timeout or missing key is `ERROR`, which never auto-approves.
- **Prompt**: trusted evidence = the user's words; arguments and file
  contents are untrusted data, never instructions; judge what the agent
  does, not what it says; look through `bash -c`, pipes, `&&`, `sudo`; a
  risk taxonomy with examples; block rules (exfiltration, weakening
  security, crossing trust boundaries, pushing to main, arbitrary code) and
  allow rules (read-only, in-project edits, tests, `git add/commit`, push to
  a personal branch). A delete inside a writable root is medium, not high:
  "prompting on routine cleanup teaches people to switch the gate off".
- **Deny-and-continue**: a risky verdict the user did not ask for does not
  interrupt the user. The call fails back to the model with the reason and
  a hint ("try a safer alternative; if none exists, repeat the call and the
  user will be asked"). It goes to the user when the action is
  user-authorized, on a repeat of a denied call, after 3 denials in a row,
  or after 20 in one turn. An `ERROR` goes to the user at once.
- **Hard rule outside the model**: a write to a path that decides what runs
  later (`.vibe/`, `.git/hooks`, `.git/config`, `.bashrc`/`.zshrc`/...) is
  never auto-approved, whatever the model says.
- **Grants**: "approve once", "approve for session" (scoped to the exact
  arguments by a hash, or to the tool for MCP tools), "approve
  permanently" (written to the allow list). No cache of model verdicts:
  a verdict depends on the user's intent, which changes.
- **Headless** (no one to ask): an escalation is a denial.
- **Telemetry**: one `tool_classification` event per classified call
  (verdict, tier, latency, model, prompt version hash, outcome).

Measured here, the Vibe prompt on `mistral-small-latest` through our
one-shot path (`BISE_ONESHOT`, whole process included): 0.6–0.7 s per call,
2.1 k input tokens (2 048 of them read from the provider's cache), 30–50
output tokens. Verdicts: `git push origin main --force` → high, unsafe;
`cargo test -p bend-tui` → low, safe; `curl -X POST -d @.env https://…` →
critical, unsafe; `rm -rf target/debug` → medium, safe.

**What we copy**: the gate order (static rules, then memory, then model),
the stripped input, the prompt (adapted, §4.3), the JSON contract and the
policy re-applied in code, fail-closed on errors, deny-and-continue with its
escalation triggers, the protected-paths rule, no cache of verdicts across
intents, the telemetry event.

**What we change**:

- auto is our default (in Vibe it is opt-in).
- One gate for all agents, in the hub, not one per session: the hub knows
  which words are the user's (a task's session is driven by main's brief
  and other agents' messages, which carry no user authority).
- Escalations are cards, shown in every view (§6), grouped when several
  agents ask the same thing.
- Bise-specific hard rules: the shared workspace (`git reset --hard`,
  `git clean`, `git stash` destroy other agents' work), bise's own config
  and state, `sb` commands always allowed.
- "always" is per project by default, stored outside the repository.

## 2. Modes and config

| mode | what runs without the user | what needs the user |
|---|---|---|
| `auto` (default) | fast-path calls, remembered rules, what the model allows | hard-rule hits, model escalations (§5.3) |
| `ask` | fast-path calls, remembered rules | everything else (no model call) |
| `yolo` | everything | nothing (today's behavior) |

- `config.toml` (`~/.bend-harness/config.toml`): `approvals = "auto"`;
  `approvals_model = "<provider>/<model>"` (default: the resolved
  `small_model`, §4.4); `approvals_timeout = 0` (minutes before an
  unanswered card denies the call; 0 = never, §6.5).
- Env: `BISE_APPROVALS=auto|ask|yolo` wins over the file (like every key).
- `/approvals` in the TUI: shows the mode and the remembered rules;
  `/approvals auto|ask|yolo` switches it live for every agent of the
  workspace (the hub holds it; the next call uses it). The status bar
  shows the mode only when it is not `auto` (`yolo` in the error color).
- `bise approvals` (CLI): lists and removes remembered rules.
- The config template gets the three keys with a one-line comment each.
- Main and every task are gated the same way.

## 3. Where the gate sits

The Bend runtime executes the tools (`runtime/main.bend`: `exec_tool` for
the model's calls, `exec_program.call` for the calls a `run_typescript`
program makes). The hub (`sbd`, `rust/switchboard`) owns the user, the
cards and the one-shot small-model path. So:

- **The runtime asks, the hub decides.** Before a gated call runs, the
  runtime prints one wire line and waits for the hub's answer. The runtime
  holds no rules: one place decides (Rust, tested), one place asks the user.
- **Gated**: `bash`, `apply_patch`, every connector/MCP call (at top level
  and inside `run_typescript` programs, one gate per call), `bash()` inside
  programs. **Not gated**: `skill`, `search_tool_functions`, `self.*`,
  `self.sleep`, the `run_typescript` wrapper itself (its isolate has no
  file system and no network; only its tool calls act, and they are gated).
- **Wire**: runtime → hub: `gate <n> <json>` with
  `{"tool", "args", "nonce"}` (`n` counts per session, `nonce` is random
  and never shown to the model). Hub → runtime: one line appended to the
  gate file (next to the steering file, `repl.json` names it):
  `<n> <nonce> allow` or `<n> <nonce> deny <reason>`. The runtime ignores a
  line whose nonce does not match. It prints `gate-done <n>` when it
  resumes.
- **Waiting**: the runtime polls the gate file (every 20 ms for the first
  200 ms, then every 100 ms) and the interrupt file. An interrupt (ctrl+c
  on the agent, `sb interrupt`) ends the wait: the call does not run, its
  result is "interrupted by the user", the hub closes the card.
- **A denied call** returns a failed tool result with the reason, so the
  model sees it and adapts.
- **No hub** (a bare `repl-live`, the scripted tests, the bench): no gate,
  today's behavior, unless `BISE_APPROVALS` is set; then an escalation is a
  denial (no one to ask), like Vibe's headless mode.
- **Hub restart**: the REPL keeps waiting. The new hub adopts it, finds the
  `gate` line with no `gate-done` in `wire.log`, and decides again (the
  card is journaled, so it comes back as it was).

## 4. The decision

For each `gate` line, in this order; the first step that settles wins:

1. **Mode** `yolo`: allow.
2. **Hard rules** (§4.1): a hit → the user (a card). The model and the
   remembered rules cannot allow it.
3. **Fast path** (§4.2): allow, no model call.
4. **Remembered rules** (§5): allow.
5. Mode `ask`: the user.
6. **Turn cache**: the same call (tool + arguments) with the same user
   intent in the same turn → the same verdict.
7. **Classifier** (§4.3): allow, deny-and-continue, or the user (§4.5).

### 4.1 Hard rules (outside the model)

Pure Rust (`rust/switchboard/src/approvals.rs`), table-tested. Bash
commands are split on `&&`, `||`, `;`, `|` and newlines; each part is
tokenized (shell quoting); `bash -c`, `sh -c`, `env X=…`, `sudo`, `time`,
`nohup` wrappers are looked through. Paths are resolved against the agent's
working directory (`BEND_WORKDIR`) and `~`. **Writable roots**: the agent's
workspace or worktree, `/tmp`, `$TMPDIR`.

A hit on any rule sends the call to the user:

- H1 **Write outside the writable roots**: `apply_patch` on such a path;
  `rm mv cp tee touch mkdir ln chmod chown truncate`, `>`/`>>`
  redirections, `sed -i`, `git -C <outside>` with such a target.
- H2 **Protected paths** (write, even inside the roots): `.git/hooks`,
  `.git/config`, shell startup files, `~/.bend-harness/` (config, auth,
  `approvals.toml`), the hub state dir (`~/.local/state/switchboard/`:
  gate files, journal), `~/.ssh`, `~/.config/git`, crontab, launch agents.
  Vibe's normalisation of `..`, `//`, `\` is copied.
- H3 **Secrets read**: `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.netrc`,
  `~/.bend-harness/auth.json`, `security find-*-password`, browser profiles.
- H4 **Git to others**: `git push` to `main`, `master`, `release/*` or the
  remote's default branch; any force (`-f`, `--force`,
  `--force-with-lease`, a `+refspec`); `git push --delete`; `gh pr merge`,
  `gh release`, `gh repo delete`.
- H5 **Shared workspace wipes**: `git reset --hard`, `git checkout -- .` /
  `git restore .`, `git clean -f…`, `git stash` (without `list`/`show`),
  when the agent works in the shared workspace (other agents' uncommitted
  work lives there). In a private worktree they go to the classifier.
- H6 **Big deletes**: a recursive delete of `/`, `~`, a path outside the
  roots, the workspace root itself, or `.git`.
- H7 **Pipe to a shell**: `curl … | sh`, `| bash`, `base64 -d | sh`,
  `eval "$(curl …)"`.
- H8 **Privilege and persistence**: `sudo`, `su`, `doas`, `launchctl`,
  `crontab`, `systemctl`, `chmod +s`.
- H9 **Local data out**: `curl` with `-d @file`, `-F`, `-T`,
  `--data-binary @`; `scp`, `rsync`, `sftp` to a remote host; `nc` fed
  from a file.
- H10 **Publishing**: `npm publish`, `cargo publish`, `docker push`,
  `twine upload`, `gh release create`.

Plain network access (`curl` GET, `npm install`, `git fetch`, web search)
is **not** a hard rule: it is too common, and blocking it everywhere is
the fastest way to make people switch to `yolo`. The classifier judges it.

These rules read text. A symlink, an alias, a script written then run, or
`python -c` can hide an action. The classifier sees the same text and
catches some of these (its prompt says to look through wrappers and
evaluate written payloads); the rest is what a sandbox is for.

### 4.2 Fast path (no model)

Allowed at once, when no part of the command has `$(…)`, backticks, a
redirection outside the roots, or a hard-rule hit:

- every part is `sb …` (the agents talk through `sb` all the time);
- every part is a read-only command on paths inside the roots: `ls cat
  head tail wc rg grep find` (without `-exec`/`-delete`) `tree file stat
  du df pwd echo which date diff sort uniq cut jq`, `git status|log|diff|
  show|branch|rev-parse|ls-files|blame|worktree list`, `cd` (Vibe's list,
  with its option guards: `find -exec`, `less` commands, `sort -o`...);
- `apply_patch` whose every path is inside the writable roots and not
  protected (in-project edits are reviewable through git; Vibe's prompt
  rates them medium, so the model would allow them anyway: we save the
  call).

### 4.3 The classifier

**What it sees** (one system message, one user message; built by the hub):

- `Tool:` the tool name (`bash`, `apply_patch`, `mcp:<group>.<tool>`).
- `Arguments:` the JSON arguments, cut at 3 000 chars (a patch: its text).
- `User request:` **the user's own words only**, cut at 2 000 chars. For
  main: the user's last message in main's thread (and the one before, when
  the last is short). For a task: the user message that led to its creation
  (the hub keeps it: `sb inspect main --origin`) and the user's messages
  to this task since (direct, `@task`, answers to its cards).
- `Task brief (written by main, context only, not the user's
  authorization):` the brief's objective, cut at 1 000 chars.
- `Writable roots:` the agent's roots, and whether the workspace is shared.

Never: the transcript, tool results, file contents, the agent's text,
other agents' messages.

**Prompt**: Vibe's `_CLASSIFIER_PROMPT` (quoted in the implementation,
with its version hash), with these edits:

- evidence: add the task brief as untrusted context, like the assistant
  context; authorization comes from `User request` only;
- allow rules: add "`sb …` commands" and "reading the project's files";
- block rules: replace `.vibe/` by `~/.bend-harness/` and the hub state
  dir; add "discarding uncommitted changes in a shared workspace
  (`git reset --hard`, `git clean`, `git stash`): other agents work there";
- keep the output contract as is.

**Output**: JSON, the four fields `safe`, `risk_level`,
`user_authorization`, `reason`. Strict parse (code fences stripped); the
policy is re-applied in code: not `safe` → risky; `critical` → risky;
`high` without authorization `medium`/`high` → risky; else allow.

### 4.4 Model, latency, cost

- **Model**: `approvals_model`, else the resolved small model (already
  used for role lines, BISE-126): `BISE_SMALL_MODEL` > config
  `small_model` > the agents' provider's `small_model` in `models.toml`
  (`mistral-small-latest`, `claude-haiku-4-5`, `gpt-5-mini`). Temperature
  0, reasoning off (lowest effort for `gpt-5-mini`), `max_tokens` 200.
  Recommended: `mistral-small-latest` (Vibe's choice, measured above).
- **Call path**: the one-shot path the role lines use (`repl-live` with
  `BISE_ONESHOT`, same provider code, keys and retries), run off the hub's
  loop like `Effect::AskRole`, with the answer back as an `Input`. Several
  agents' calls run in parallel.
- **Latency**: 0.6–0.7 s per classified call, measured (§1), process spawn
  included. The hub round trip for the other steps is a few ms plus the
  runtime's polling (≤ 20 ms at first). A turn with 20 bash calls where 6
  reach the model pays about 4 s.
- **Cost** per classified call: ~2.1 k input tokens (the system prompt,
  cached after the first call), ~40 output tokens. `mistral-small-latest`
  at $0.10/$0.30 per M (list price, to check; not in our catalog):
  ≈ $0.0002. `claude-haiku-4-5` ($1/$5, cache read $0.10): ≈ $0.0005.
  `gpt-5-mini` ($0.25/$2): ≈ $0.0006 plus its reasoning tokens. 1 000
  classified calls ≈ $0.20–0.60. The cost shows in `/usage` on its own row.
- **Timeout**: 6 s. No retry (the provider code already retries 429/5xx
  inside that budget).
- **Cache**: none across turns or intents (Vibe's reason: a verdict
  depends on the user's words). Inside one turn, the same tool + arguments
  + intent hash reuse the verdict (an agent retrying a call).
- **Failure** (timeout, bad JSON, no key, provider down): the call goes to
  the user (a card), reason "couldn't check this call (<why>)". Never an
  allow. After 3 failures in a row, main's feed gets one notice ("the
  approvals model fails: <why>; risky-or-unknown calls go to you"), and
  the hub skips the model for 2 minutes (every non-fast-path call becomes a
  card, like `ask`), then tries again.

### 4.5 What a verdict does

- **allow**: the call runs. Nothing new in the thread (§7).
- **risky, not authorized**: deny-and-continue (Vibe's policy). The call
  fails back to the agent: "blocked by approvals: <reason>. If a safer way
  exists, use it. If not, repeat the call and the user will be asked." The
  thread shows one dim line (§7). No card.
- **the user** (a card) when: a hard rule hit; the model failed; the risky
  call is user-authorized (`user_authorization` medium or high: the user
  asked for it, only they can confirm); a repeat of a call denied this
  session; 3 denials in a row for this agent; 20 in its turn.

## 5. "Always" memory

- **File**: `~/.bend-harness/approvals.toml`, next to `config.toml`. One
  file, owned by the user, outside every repository. Not in the repo: a
  cloned repository must not ship its own permissions. The file is a
  protected path (H2): an agent cannot write it without a card.
- **Written** by the hub when the user picks "always" on a card; edited
  by hand, by `/approvals`, or by `bise approvals`.
- **Scope**: `project` (default: the git common root, so the worktrees of
  a repository share it) or `global` (by hand or `/approvals`; the card
  offers "always here" only).
- **Rule shapes**:

```toml
# ~/.bend-harness/approvals.toml: what bise runs without asking you.
# Written when you pick "always here" on a card; edit freely.

[[allow]]
project = "/Users/me/lab/api"     # absent: every project
tool = "bash"
prefix = "cargo test"             # argv prefix of one command part
added = "2026-10-12T14:03:00Z"
from = "card #12, api-v2"

[[allow]]
project = "/Users/me/lab/api"
tool = "mcp:gmail.send_email"     # a connector tool: the whole tool

[[allow]]
tool = "apply_patch"
path = "/Users/me/notes/"         # a directory outside the roots
```

- **Bash prefix**: the program and its subcommand words, without flags or
  operands: `cargo test`, `npm install`, `git push`, `docker compose up`.
  A chain is allowed only when every part is (fast path or a rule). The
  card says what "always" will store ("always here: cargo test").
- **Hard rules win over memory**: `git push` remembered still asks for a
  push to main or a force push. A card for a hard-rule hit has no "always"
  choice.
- **Read** on each gate (the file is small; reread when its mtime changes).

## 6. Routing: every approval reaches the user

### 6.1 A card, global

An approval is an attention card of a new kind, `approval`, in the hub
(`hub/core.bend` `open_card`, the sb-core journal). The hub already sends
every open card to every client in its state snapshot, whatever the view,
and the TUI's card box already works in every view. So an approval shows in
main's view, in the asker's view, and in any other agent's view, the same.

- New: the box **opens by itself** when an approval card arrives (the other
  kinds wait for ctrl+g). It does not take the keyboard: the composer keeps
  its text, and the card's keys are digits on an empty composer or alt+digit
  (§7).
- Approval cards sort first in the box (before questions): each one holds
  an agent in the middle of a turn.
- Only the user answers. `/answer` on an approval card is accepted from a
  TUI client only; `sb close` and every agent request refuse it (RFC 0003:
  no agent message carries the user's authority). Main sees the pending
  approvals in its board ("api-v2 waits for your approval, card #12") and
  may tell the user; it cannot answer.

### 6.2 What the waiting agent does

- Its turn is paused inside the gate. No model call, no tokens.
- Hub status: `waiting on you` (the existing `waiting_on`, set to the
  user), in the panel, `sb list`, and main's board.
- Messages to it queue as usual (steering file); it reads them after the
  call.
- ctrl+c on it, or main's `sb interrupt`, cancels the wait (§3).
- Other agents keep working.

### 6.3 Many at once

- One card at a time in the box, oldest first, with a counter `1/5`.
- Identical requests (same tool and arguments, same project) from several
  agents are one card, "3 agents want to run". One answer answers them all.
- No "allow all" key in v1 (designer: it is the key that makes approvals
  pointless).
- After an answer the next card slides in; the counter drops.

### 6.4 Answers

- **allow once**: the hub writes `allow` to the gate file(s), closes the
  card ("allowed"), notes it in main's feed.
- **always here**: the same, plus the rule in `approvals.toml`.
- **deny**: `deny <reason>` with reason "the user said no"; with a note
  (alt+r), the note goes to the agent as the reason.
- The answer is also a user message for the intent (§4.3): a later
  classification of the same agent sees "the user allowed: <call>".

### 6.5 When the user is away

- The card waits. By default nothing is ever allowed without the user and
  nothing times out: the agent stays paused, the others go on.
- `approvals_timeout = N` (minutes): an unanswered card denies with the
  reason "the user did not answer in N min; do something else or ask
  later", so the agent can go on with other work.
- A terminal notification when an approval card opens and the terminal
  window is not focused (focus events): OSC 9 / OSC 777 (iTerm2, WezTerm,
  Ghostty, kitty), else the bell. ux-notes question 16.
- TUI closed, hub running: the cards stay open (journaled); the next
  `bise` opens the box on the first one.

## 7. UI (designer's call, m_1918)

The card, in the same box as the other cards, one at a time:

```
┃ ? api-v2 wants to run                                          1/5
┃   git push origin main --force
┃   pushes to main and rewrites history
┃   1 allow once   2 always here   3 deny   alt+r deny with a note
```

- Title: `? <agent> wants to run` (bash, programs), `wants to edit <path>`
  (patch), `wants to call <tool>` (connector). `?` and the name in bold
  accent, like the other cards; `1/5` dim, on the right. Grouped:
  `? 3 agents want to run`.
- Preview: the command in the code colors, 6 rows at most, then
  `▸ 14 more lines · ctrl+f full screen`. A patch: its first 6 diff lines,
  then `▸ 3 files · +42 −7`. A long line wraps; it is never cut inside.
- Reason: the classifier's line (or the hard rule's), dim, as a plain
  fact, no "because": `pushes to main and rewrites history`. One line, cut
  with `…`.
- Keys: `1` / `2` / `3` when the composer is empty, `alt+1` / `alt+2` /
  `alt+3` always (even while typing); `alt+r` = deny with the composer's
  text as a note to the agent. No letters (y/a/n): they collide with
  typing, and digits already pick a card's numbered choices. `/help` says
  what "always here" stores (this command prefix, this repository). A
  hard-rule card shows `1 allow once   3 deny`.
- In a view that is not the asker's: the same box (an approval blocks an
  agent: level 1 everywhere), with a faint hint on the counter row:
  `⌥3 opens api-v2`.
- Signals: panel row `3 ? api-v2 · waiting for you 1m`, `?` in accent; the
  divider's right side `? 2 need you` in accent, before the working gust;
  the header count adds `? 2`. In the asker's thread its tool box shows
  `waiting for you` dim in the border, where `∿` would be.
- Auto-allowed calls: nothing new (the tool box already shows the call;
  optional: `auto` faint in the box border, visible with ctrl+o).
- Auto-denied calls (deny-and-continue): one dim level-3 line under the
  call: `✗ not run · rewrites history · say "run it" to allow` (▸ the full
  reason). The agent is told; it is not a card. "run it" works because the
  user's message becomes authorization for the next classification.

## 8. Security caveats

- **Prompt injection into the classifier**: the arguments are written by
  the agent, which may have read a hostile file or web page. Defenses: the
  classifier sees no tool result and no file content; the prompt marks
  arguments as data, never instructions; authorization comes only from the
  user's own words, which the hub picks (never from agent messages or the
  brief); the output is strict JSON and the policy is re-applied in code
  (a `safe: true` with `critical` does not run); hard rules run before and
  whatever the model says. A crafted command can still fool a small model
  into "low risk"; the hard rules cover the worst families (secrets out,
  pushes, wipes, privilege, persistence).
- **Faking the user's intent**: an agent can write "the user said push"
  in a message to another agent. The intent is built from messages the hub
  knows came from a TUI client, so this does nothing.
- **Answering its own card**: agents cannot answer approval cards (§6.1).
  The gate file is in the hub state dir (a protected path: writing it is a
  hard-rule card) and each answer carries a nonce the model never sees.
  An agent that already runs arbitrary code could still read the wire log
  and forge an answer: it would first need a call that runs arbitrary
  code, which is what the gate judges. Reading mid-turn answers from the
  REPL socket instead of a file would close this; to check in BISE-233.
- **Obfuscation**: `base64 -d | sh`, a script written then run, `python -c`,
  aliases, symlinks. Partly caught (H7, the prompt's "evaluate the
  payload"), not fully. Not a sandbox (see Goal).
- **Data to the classifier's provider**: the command text and the user's
  words go to the small model's provider. By default it is the agents'
  provider's small model, so no new party sees the data; a different
  `approvals_model` sends it elsewhere (said in the config comment).
- **Denial of wallet**: a runaway agent doing 1 000 classified calls costs
  $0.20–0.60 (§4.4).
- **`yolo`** removes every check, hard rules included. The status bar says
  so in the error color.

## 9. Plan (ids to confirm: the last used is BISE-218)

| id | task | where | depends on |
|---|---|---|---|
| BISE-230 | Rules core: modes, config keys, hard rules H1–H10, fast path, bash splitting and path resolution, the verdict policy, deny-and-continue counters, the memory file (read, match, write, prefix). Pure, table tests (Vibe's cases + ours). | `rust/switchboard/src/approvals.rs` | — |
| BISE-231 | Classifier: request builder (user words, brief, roots), the adapted prompt + version hash, one-shot call off the hub loop (`Effect::Classify` → `Input::Verdict`), 6 s timeout, strict parse, turn cache, failure mode and notice, cost in `/usage`, the `approval` event in the session log. | `rust/switchboard` (`approvals.rs`, `core.rs`, `daemon.rs`) | 230 |
| BISE-232 | Runtime gate: `gate` line before gated calls (`exec_tool`, `exec_program.call`), gate file polling with the interrupt, nonce check, `gate-done`, failed result on deny; laws for the pure parts. No hub: today's behavior. | `runtime/main.bend`, `runtime/*-pure.bend`, `LAWS.bend` | — |
| BISE-233 | Hub plumbing: parse `gate` lines, the gate file (named in `repl.json`), the decision pipeline §4, `waiting_on = you`, interrupt cancels, adoption after a hub restart, check the socket option (§8). | `rust/switchboard/src/daemon*`, `core.rs` | 230, 232 |
| BISE-234 | Card kind `approval` in sb-core: open/close, grouping identical requests, answers from clients only (`sb close` and agents refused), journal events, the board line for main, `approvals_timeout`. | `hub/core.bend`, `hub/model.bend`, `hub/codec.bend` | 233 |
| BISE-235 | TUI card: the look §7, keys 1/2/3 + alt+1/2/3 + alt+r, preview and ctrl+f, counter, grouped title, the box opens by itself, hint in other views, sort first. | `rust/tui/src/sb/cards.rs`, `sb/keys.rs`, `help.rs` | 234 |
| BISE-236 | TUI signals: panel row, divider `? n need you`, header count, `waiting for you` in the tool box border, the auto-denied line, the optional `auto` tag. | `rust/tui/src/sb/panel.rs`, `toolbox.rs`, `render.rs` | 233 |
| BISE-237 | Settings: `approvals`, `approvals_model`, `approvals_timeout` in the config template and the catalog setup, `BISE_APPROVALS`, `/approvals` (show, switch, list, remove), `bise approvals` CLI, the mode in the status bar, the onboarding line ("i ask you before risky commands"). | `runtime/settings-pure.bend`, `rust/catalog`, `rust/tui/src/commands.rs`, `rust/harness` | 230 |
| BISE-238 | Away: terminal notification on a new approval card when unfocused (OSC 9/777, bell). | `rust/tui` | 235 |
| BISE-239 | Eval and e2e: ~150 labeled calls (Vibe's prompt examples + real calls from our transcripts) run through the classifier on mistral-small, haiku-4-5, gpt-5-mini: share of calls that reach the model, the user, latency, cost; one e2e with the fake provider: a task hits a risky call, the card shows in main's view and another task's view, the user allows, the task resumes; same with deny and with ctrl+c. | `tests` | 231–235 |

Order: 230 and 232 in parallel, then 231 and 233, then 234, then 235–238
in parallel, 239 last (its eval part can start after 231). Launch
criterion: 239 green, and on our own transcripts fewer than 1 call in 50
reaches the user in `auto`.

## 10. Open questions for the user

1. **`yolo`**: no checks at all (like today), or keep the hard rules H2
   and H4–H6 even in `yolo`?
2. **Hard rules and "always"**: a hard-rule card has no "always" (a
   `git push origin main` asks every time). OK, or allow "always" on some
   of them (H4 push to main in a solo repository)?
3. **Away**: no timeout by default (the agent waits forever). OK, or a
   default `approvals_timeout` (say 30 min) that denies and lets the agent
   go on?
4. **In-project edits** go through without the model (fast path). OK, or
   send patches to the classifier too (+0.6 s per edit)?
5. **Network**: plain network access is judged by the model, not a hard
   rule. OK?
6. **Model**: the agents' provider's small model by default (no new party
   sees the data), or always `mistral-small-latest` like Vibe (cheapest,
   needs a Mistral key)?
7. **Global rules**: "always" on a card is per project only; global rules
   are set by hand or `/approvals`. Enough?
