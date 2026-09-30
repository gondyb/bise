# Approvals: design (3 global modes)

Status: design, not built. Task `approvals`, 2026-10; updated with the
user's answers to the 8 open decisions. Plan and costs:
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

The user's answers to the 8 decisions (second round):

1. A real **bash parser**: it splits a command into its simple commands and
   finds each one's program and arguments. It spots safe reads, matches the
   saved "always allow" rules (per repo, prefix rules like `cargo test`,
   `npm run *`), and spots the plain bash writes. A command no rule allows
   is a card, and the card offers "always allow this here" (§2.3).
2. Settled: in `accept edits`, a bash edit the parser cannot read is
   denied once with the "use `edit`" hint (or `apply_patch`: the one tool
   that is on); a card only if
   the agent repeats it.
3. Find why agents avoid `apply_patch` (§2.4). Settled (third round): one
   edit tool per request, never both, chosen by the provider: OpenAI →
   `apply_patch` only; every other provider → Vibe's `edit` only, as is
   (§2.5). The prompts and the hint name the one that is on.
4. **No time limit.** An agent waits as long as needed. No
   `approvals_timeout` option.
5. Writable roots in `accept edits`: the current folder and everything
   under it, plus `~/.bise` (from anywhere). No `/tmp`: each agent gets its
   own temp dir `~/.bise/tmp/<agent-id>` (§4).
6. "always allow" is per repo.
7. Network in `auto`: judged by the model.
8. Switching to `yolo` while cards wait: the cards stay until answered.

## 2. The problem: editing through bash

The agents have `bash`, `run_typescript`, `apply_patch`, `skill` and
`search_tool_functions`. `apply_patch` (V4A) exists since 2026-09-27
(fdfedd9), but the models edit mostly through bash: `sed -i`,
`cat > f <<EOF`, `python3 - <<EOF` scripts (Opus's favorite), `perl -pi`.
A rough count over every thread (`sb history --role tool`, a word search
that over-counts bash): `python3 - <<` 3 214 hits, `cat >` 2 403, `sed -i`
957, against 358 `Begin Patch`.

To say "this bash call only edits files in the repo" is not always possible
from the text: `python3 - <<EOF`, `make fmt`, `cargo run`, `git apply`,
`npm run x`, a script written then run. Each can edit, delete, or do
something else.

### 2.1 The options (first round)

| option | how `accept edits` decides | pros | cons | cost |
|---|---|---|---|---|
| (a) edit tools | only an edit tool call counts as an edit | exact: the paths are in the arguments; predictable | models drift to bash | steering ~0.5 day; `edit` tool ~1.5 days |
| (b) parse bash | read each simple command: safe reads, saved rules, plain writes | no model call, fast; what Vibe and Claude Code do for their rules | opaque commands (scripts, `python -c`, build tools) stay opaque | ~3 days with tree-sitter |
| (c) classifier | the `classify` model says "only an in-repo edit?" | handles python scripts | 0.6–0.7 s per call; can be fooled; the mode becomes a guess | ~3 days, shared with `auto` |
| (e) OS sandbox (what Codex relies on) | bash runs in Seatbelt / Landlock, writes only in the repo | real containment | changes the mode's meaning; breaks caches and worktrees; per-OS | 1–2 weeks |

### 2.2 The choice: (a) + (b), the classifier only in `auto`

The user chose (a) + (b): an edit tool each provider's models know (one per request), and a real bash
parser. The classifier stays out of `accept edits`, so that mode stays a
rule the user can predict. Codex, for the record, does not parse bash to
find edits: it parses only to allowlist safe commands and relies on its
sandbox for the rest.

### 2.3 The bash parser (copied from Vibe)

Vibe's parts worth copying (`vibe/core/tools/builtins/_shell_permission_analysis.py`,
`bash.py`, `vibe/core/tools/arity.py`, `_shell_command_policy.py`):

- **A real grammar.** Vibe parses with tree-sitter-bash, not with regexes.
  We use the same grammar from Rust (`tree-sitter` + `tree-sitter-bash`
  crates, a small C build). A hand splitter gets heredocs, quotes and
  subshells wrong.
- **Output**: the list of simple commands, each with its program, its
  arguments after quote removal, its env assignments and its redirections
  (target, operator), found through `&&`, `||`, `;`, `|`, `( … )`,
  `{ …; }`, `if`/`for`/`while` bodies, and the string of `bash -c` /
  `sh -c` (parsed again). Wrappers are looked through: `env X=1`, `time`,
  `nohup`, `timeout N`, `nice`; `sudo` is a hard rule. `cd <dir>` moves the
  base directory for the paths of the parts after it.
- **Unreadable parts** (Vibe's "dynamic nodes"): command substitution
  `$(…)` and backticks, variable expansion in a program name or a path,
  process substitution, `eval`, brace and arithmetic expansion. A part with
  one of these is never allowed by the read list or by a rule's `*`; its
  card offers "always allow" for the exact text only (Vibe's
  `invalidates_scope`).
- **Harmless redirections**: `2>&1` and `>/dev/null` touch no file (Vibe's
  `_file_redirect_reason`); any other `>`/`>>` names a file.

What the parser gives, per simple command:

1. **Safe read**: the program is in Vibe's read-only list (`cat head tail
   ls wc grep rg find stat file diff sort uniq cut tr jq pwd which date
   basename dirname readlink du shasum tree echo`, `git status/log/diff/
   show/branch/rev-parse/ls-files/blame`, `sb …`), no option guardrail hit
   (Vibe's list: `find -exec/-delete/-fprint`, `sort -o`, `git diff
   --output`, `git log --ext-diff`...), and every path argument inside the
   roots (a read outside the roots asks, as in Vibe: `cat ~/.ssh/id_rsa`
   is a read).
2. **Saved rule match**: the command's text matches a rule of this repo
   (below).
3. **Plain write**: a known write whose every target is a static path:
   `>`/`>>` redirection (`cat > f <<EOF`, `echo x >> f`, `printf`), `tee f`,
   `sed -i`, `mkdir touch cp mv rm ln truncate`. Inside the roots it is an
   edit (§4).
4. **Opaque**: everything else, including an interpreter fed inline code
   (`python3 - <<EOF`, `python -c`, `perl -e`, `node -e`), scripts, build
   and test tools.

A command runs without a card only when **every** part is a safe read, a
saved-rule match, or (in `accept edits`) a plain write inside the roots.

**Saved rules ("always allow … here")**, Vibe's and Claude Code's prefix
rules:

- The card proposes a pattern from the command's **arity** (Vibe's `ARITY`
  table, copied: `cargo 2`, `cargo run 3`, `npm run 3`, `git 2`,
  `git stash 3`, `docker compose 3`, `uv run 3`, `make 2`...): the first
  N words, then `*`. `cargo test -p x` → `cargo test *`; `npm run build`
  → `npm run build *`; an unknown program → its name + `*`. The user can
  widen it by hand in the file (`npm run *`).
- A command with an option guardrail, or an unreadable part, gets its exact
  text as the rule (Vibe: `git log *` earned by `git log $REF` would cover
  `git log --ext-diff`).
- A chain with several unallowed parts: one card, and "always" stores one
  rule per part ("always allow cargo test, git commit here").
- Match: the part's text equals the pattern, or starts with the pattern's
  words then a space (Vibe's `_matches_pattern`). Env assignments before
  the program are ignored (`GIT_INDEX_FILE=x git commit` matches
  `git commit *`).
- Stored per repo (the git common root, so a repo's worktrees share them)
  in `~/.bise/approvals.toml` (spec §5 format, `prefix` → `pattern`).
- Hard rules win: `git push *` saved still asks for a push to main or a
  force push.

### 2.4 Why the agents avoid `apply_patch`

What the code and the threads show:

1. **It is new.** It landed on 2026-09-27. The long threads (designer,
   bend-hub, main) built their habit before, and a model repeats the edit
   method it already used in its own thread.
2. **Nothing outside its own description asks for it.** The system prompt
   (`prompts/prompt-tool-use.txt`) talks only about `run_typescript`. The
   bash description invites the other way: "the command may span multiple
   lines (scripts, heredocs, …)". "This is the preferred tool for file
   edits" sits only inside `apply_patch`'s own description, last in the
   tool list.
3. **The format.** V4A is OpenAI's patch format (GPT-5 and Codex models
   know it). Anthropic models are trained on an exact string-replace tool
   (`str_replace`, Claude Code's `Edit`). With no such tool, Opus writes the
   closest thing it knows: a python script with `s.replace(old, new)`.
   This task did it too, once.
4. **The cost of a small change.** A one-line change in V4A needs the
   `*** Begin Patch` frame, an `@@` line, and context lines with an exact
   leading space; `sed -i` or a replace is shorter. A new file needs a `+`
   on every line, where `cat > f <<EOF` needs none.
5. **Not failures.** Only 7 "chunk not found" errors show against ~358
   patches (~2 %): the tool works when used.

### 2.5 One edit tool per request (settled) and the steering

The user's decision: **exactly one edit tool per request, never both,
chosen by the provider.**

- **The OpenAI provider** → `apply_patch` only (V4A, the format OpenAI
  models are trained on).
- **Every other provider** (Anthropic, Mistral, OpenRouter, Google, the
  rest) → Vibe's `edit` only, **as is**: same name, schema, description and
  behavior, copied exactly from Vibe 2.25.8
  (`vibe/core/tools/builtins/edit.py`, `prompts/edit.md`).
- The prompts, the bash description and the deny-once hint name the one
  tool that is on in that request, never the other.

Vibe's `edit`, copied exactly:

- Name: `edit`.
- Parameters:
  - `file_path` (string, required): "The absolute path to the file to modify"
  - `old_string` (string, required): "The text to replace"
  - `new_string` (string, required): "The text to replace it with (must be
    different from old_string)"
  - `replace_all` (boolean, default false): "Replace all occurrences of
    old_string (default false)"
- Description (verbatim): "Exact string replacement in a file. You must
  `read_file` first. When editing text from `read_file` output, never
  include any part of the line-number prefix in `old_string` or
  `new_string`. If `old_string` is not found or matches multiple
  locations, provide more context to make it unique, or use `replace_all`.
  If an edit fails, re-read the file before retrying."
- Behavior and errors (Vibe's words): an empty path → "File path cannot be
  empty"; empty `old_string` → "old_string cannot be empty. Use write_file
  to create new files."; same strings → "No changes to make — old_string
  and new_string are identical"; missing file → "File does not exist:
  <path>"; not found → "String to replace not found in file.\nString:
  <old_string>"; several matches without `replace_all` → "Found N matches of
  the string to replace, but replace_all is false. To replace all
  occurrences, set replace_all to true. To replace only one occurrence,
  please provide more context to uniquely identify the instance.\nString:
  <old_string>"; not text → "Cannot edit <path>: file is not valid text
  (…)". Success → "The file has been updated successfully." (or "… All
  occurrences were successfully replaced"). The write is atomic.
- Ours around it: a pure core in Bend with laws (`bend/core/edit.bend`), the
  TUI shows it as a diff like `apply_patch`, the gate reads `file_path`
  (§4). The hub's activity line reads "edit <file>".

**Two gaps in "as is"** (said plainly, not changed): the description names
`read_file` and `write_file`, and bise has neither. The models will read
with `cat`/`rg` in bash, which is fine (a safe read). But a non-OpenAI model
has **no tool to create a file**: `edit` refuses an empty `old_string`, and
`apply_patch` is gone for it. It will create files with bash
(`cat > f <<EOF`): a plain write the parser reads, so it runs in `accept
edits` if open question 2 is yes, else a card. Open question 5 in the plan:
copy Vibe's `write_file` as is too (name `write_file`, `file_path` +
`content`, "Create a new file. Errors if the file already exists — use
`edit` to modify existing files. Prefer editing existing files over
creating new ones. Do not proactively create documentation or README
files."). That would close the gap and make `edit`'s own error message
true.

The tool is picked when the tool catalog is built (`catalog_live` in
`bend/runtime/tools-pure.bend`), from the agent's provider; a provider
switch (`/model`, a reload) rebuilds the catalog and the prompt lines. An old session replays its past
`apply_patch` or `edit` calls as history; only the new catalog changes.

Steering (phase 1):

- Tool list order: the edit tool before `bash`.
- System prompt, one line naming the tool that is on: "Edit files with
  `edit`." (or "with `apply_patch`."). "Don't edit files through bash (`sed -i`, redirections,
  heredocs, python or perl scripts): those need the user in `accept
  edits`, and tool edits show as diffs."
- Bash description: drop the heredocs invitation; add "not for editing
  files: use `edit`." (or `apply_patch`: the one that is on).
- In `accept edits`, an opaque bash command that looks like an edit (an
  interpreter fed inline code that writes, `perl -pi`) is denied once with
  the hint (settled), naming the tool that is on: "accept edits: this bash
  call needs the user. Use `edit`: it runs without asking. If bash is really
  needed, repeat the call and the user will be asked." Plain writes the
  parser can read do not need this (open question 2).
- Measured in phase 4: the share of edits made by the edit tool, before and
  after, per model.

## 3. What each mode does

Gated calls: `bash` (top level and `bash()` in programs), `edit`,
`apply_patch`, every connector call (`tools.<group>.<fn>`, one gate per call). Never
gated: `search_tool_functions`, `skill`, `self.*`, the `run_typescript`
wrapper (its isolate has no files and no network; its tool calls are
gated one by one).

| call (every part of a bash chain, §2.3) | `yolo` | `accept edits` | `auto` |
|---|---|---|---|
| `sb …` | runs | runs | runs |
| safe read inside the roots (§2.3) | runs | runs | runs |
| the edit tool (`edit` or `apply_patch`, one per request), every path inside the roots (§4) | runs | runs | runs |
| plain bash write (`sed -i`, `>`, `tee`, `mkdir`, `rm`…), every target inside the roots | runs | runs | runs |
| a saved rule of this repo ("always allow cargo test here") | runs | runs | runs (hard rules still ask) |
| the edit tool or a plain write outside the roots or on a protected path | runs | card | card |
| opaque bash that looks like an edit (`python3 - <<EOF`, `perl -pi`) | runs | denied once with the `edit` hint, card on repeat | classifier |
| any other bash (`cargo test`, `git commit`, `curl`…) | runs | card, with "always allow … here" | classifier |
| connector call | runs | card, with "always allow this tool here" | classifier |
| hard-rule hit (spec §4.1: push to main, secrets, shared-tree wipes…) | runs | card, no "always" | card, no "always" |

Plain words: `accept edits` asks for every command that is not a read, an
edit, `sb`, or a saved rule. The first hour in a repo gives many cards
(`cargo test`, `git commit`…); each "always allow … here" removes one kind
for good in that repo.

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

An edit is a call of the request's edit tool (`edit` or `apply_patch`), or a plain bash write the parser
can read (§2.3), when **every** path it writes (after `..`, `~`, and
symlinks of the parent directory):

- is inside the **roots**:
  - the agent's current folder and everything under it: the shared
    workspace for the agents that work there, its private worktree for an
    agent in a worktree;
  - `~/.bise`, from any folder (the user's choice), except the protected
    paths below;
  - the agent's own temp dir `~/.bise/tmp/<agent-id>` (part of `~/.bise`);
- and is not protected: `.git/` (hooks, config, index), `.envrc`, and in
  `~/.bise`: the hub state (`~/.bise/hubs/`: journal, socket, gate files),
  `approvals.toml` (the saved rules), `auth.json` (the keys). See open
  question 1 in the plan: the user said "all of `~/.bise`", these three are
  the exceptions i recommend.

Not a root: `/tmp`, another agent's worktree, another repo, the rest of `~`.

**The agent's temp dir.** Each agent gets `~/.bise/tmp/<agent-id>`: the hub
creates it at spawn and sets `TMPDIR`, `TMP` and `TEMP` to it in the
agent's tool env (`rust/switchboard/src/tools_env.rs`). Tools that honor
`TMPDIR` (`mktemp`, cargo, python's `tempfile`) write there. A literal
`/tmp/...` path is outside the roots: a card in `accept edits`; the prompt
says "use `$TMPDIR`, not `/tmp`". The hub deletes the dir when the agent is
dropped, and at start it deletes the dirs of agents that no longer exist.
`<agent-id>` is the agent's unique id, not its name (names come back).

A delete is an edit. Git recovers a tracked file; an untracked file deleted
by an edit is lost (said in `/help`). A recursive delete of a root itself
(`rm -rf .`, `rm -rf ~/.bise`) is a hard rule (H6).

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
- A chain shows only the parts that need you; the parts already allowed
  (reads, `sb`, saved rules) stay dim above them.
- Hard rule: no option 2; the reason says why it always asks: "it rewrites
  main. this one always asks."
- Edit outside the repo: "? api-v2 wants to edit a file outside the repo",
  the path, the first 3 diff lines dim, then "▸ 12 more lines" (ctrl+o).
- A connector call: "? api-v2 wants to call gmail.send_email", the
  arguments cut to 3 lines.
- Keys: `1`/`2`/`3` on an empty composer; typed text + ⏎ = no, with the text
  as the note to the agent. The box never opens by itself while you type
  (its row pulses once); it may open by itself on an empty, idle composer.
- "always allow X here" stores the parser's pattern (§2.3: `cargo test *`,
  `npm run build *`, or the exact text for a guarded or unreadable
  command) for this repo in `~/.bise/approvals.toml` (spec §5 format, path
  moved from `~/.bend-harness`). The card names the pattern it stores. For
  a connector: the whole tool. The file is a protected path.
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
  launched (the parser looks through `nohup` and `&`). What the process does later is not seen by the gate. In
  `accept edits` a background launch is a command, so it asks.
- **`run_typescript` programs**: each tool call inside is gated on its own;
  a card pauses the program at that call.
- **No time limit** (the user's decision). A card waits as long as needed:
  a waiting agent costs nothing, and an auto-deny teaches agents to work
  around the gate. There is no `approvals_timeout` option.
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
- **The parser reads text, not effects.** A saved `cargo test *` runs
  whatever the tests do; `make *` runs whatever the Makefile says. An
  unreadable part (`$(…)`, a variable as a path) is never matched by a
  `*`, but a readable command can still do more than its name says.
- **`~/.bise` as a root.** It holds the hub state, the saved rules and the
  keys. If an agent could edit them without a card, it could forge a card's
  answer, grant itself "always allow *", or read and send the keys. So
  `hubs/`, `approvals.toml` and `auth.json` stay protected (§4), even though
  the rest of `~/.bise` is a root.
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
| in-project `apply_patch` skips the model (decision 4) | settled: it is what `accept edits` means, with `edit` and plain bash writes |
| bash split on `&&`/`;`/`|` by hand, a small tokenizer | a real parser (tree-sitter-bash, Vibe's analysis), used for reads, saved rules and plain writes |
| "always" stores program + subcommand words | Vibe's arity table + `*`; exact text for guarded or unreadable commands |
| writable roots: the workspace/worktree, `/tmp`, `$TMPDIR` | the current folder and below, `~/.bise` (except hub state, saved rules, keys), a per-agent temp dir `~/.bise/tmp/<agent-id>`; no `/tmp` |
| `approvals_timeout` option | no time limit, no option |
| tools: `bash`, `apply_patch` for every model | one edit tool per request, by provider: OpenAI → `apply_patch`; every other provider → Vibe's `edit` as is; the prompts name only that one |
| opaque bash edits | `accept edits`: denied once with the `edit` hint, then a card |
| paths `runtime/`, `hub/`, `LAWS.bend` | `bend/runtime/`, `bend/hub/`, `bend/LAWS.bend` |
| ids BISE-230..239 | taken at launch from HEAD's tracker (next free today: BISE-301) |

Unchanged: the gate wire (§3), the hard rules H1–H10 (§4.1), the fast path
(§4.2), the classifier (§4.3–4.5), the memory format (§5), the security
notes (§8).
