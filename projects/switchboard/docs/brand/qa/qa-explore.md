# qa-explore: bise used from the inside

- **Build tested:** `51c081a` (worktree `/tmp/qa-explore-wt`, debug `bise` in
  `/tmp/qa-explore-target`). HEAD moved to `6586f6d` during the pass: bug I is
  fixed there. Every other bug was checked against the `51c081a` sources, and
  `6586f6d` does not touch them.
- **How:** a throwaway hub for each session: temp `HOME`, `BISE_HOME`,
  `XDG_STATE_HOME`, `SB_STATE_DIR`, the `BEND_*`/`SB_*`/`BISE_*` exports of the
  calling agent unset. The TUI ran in tmux (`capture-pane`, raw SGR mouse
  sequences for selections), mostly on the scripted fake provider
  (`tests/fake_provider.py`). A few live Mistral turns
  (`mistral/mistral-medium-latest`): one TUI session (main spawns a task) and
  one `bend_client.py` headless session (bash + skill). The bise CLI (`doctor`,
  `login`, `logout`, `auth list`, `models`) ran in empty temp HOMEs. For the
  live bend_client runs, `bend_client.RUN` pointed at a wrapper that runs the
  debug `bise` directly. `run.sh` cannot run in a temp HOME: rustup has no
  toolchain, and the `bend-jsrt` build fails.
- **Harness for the next pass:** `/tmp/qa/up.py NAME COLS ROWS [K=V…]` (tmux +
  fake provider + temp homes; `LIVE=1 BEND_PROVIDER_URL=-` for a live model) and
  `/tmp/qa/dump.py <state dir> [agent]` (the hub's lines through
  `tests/e2e.Client`).
- **Severity:** *medium* = wrong behavior a user or the model hits in a normal
  flow. *low* = wrong or misleading output, with a workaround. *cosmetic* =
  drawing only. No bug stopped a turn.

| # | severity | title |
|---|---|---|
| A | medium | A message that arrives while the agent is busy shows twice in its feed |
| I | medium | `skill` with an unknown name says "skills index unreadable" (**fixed in 6586f6d**) |
| J | medium | The runtime writes the session's skills/plugins index under `~/.bend-harness/run/<port>` in the bise layout (ignores `BEND_RUN_DIR`) |
| H | low | `/restart ` offers bise commits outside bise's source tree, then refuses them |
| B | low | `sb worktree` accepts a path that does not exist, and main can mark itself ψ |
| C | low | `bise doctor` on a fresh HOME says "old layout" and "migration not done" |
| G | low | `bise doctor` ignores the `[voice]` config (unknown provider, config warnings) |
| L | low | `sb spawn --help`/`-h` gives an error, and `sb` usage does not list `sb worktree` |
| P | low | `ctrl+r` does nothing, with no hint, when voice mode is off |
| E | cosmetic | `BISE_ASCII=1`: the card box, the card bar and the composer rail stay Unicode |
| F | cosmetic | The header ends with a lone ` · ` when the count segment is empty |
| K | cosmetic | The `✉︎` chip row is 1 column off in tmux (the right frame moves) |
| M | cosmetic | The panel gives a dropped agent's number to the next agent, listed last (3,5,6,7,4) |
| N | cosmetic | In an agent's view the divider shows two "working" timers that disagree |
| D | cosmetic | `bise models <no match>` prints nothing to say that no model matches |

Not a bise bug, noted for the prompts: in the live pass, main wrote
``sb spawn probe --objective "run `ls` in the workspace…"``. bash ran the
backticks, so the objective became "run README in the workspace". The spawn line
in the feed shows it (evidence `live-backticks.txt`). The `sb` usage supports
`-` (read the text from stdin), but main's prompt and tool description do not
mention it.

Evidence files: `/tmp/qa/ev/` (kept outside git; the excerpts below are the
relevant parts).

---

## A · A message that arrives while the agent is busy shows twice in its feed

- **Severity:** medium. Every busy hour doubles the lines. It hits the
  reports most (`✓ bench: …` twice). The model gets each message once.
- **Repro** (fake provider, `up.py qa2 120 40`), in main:
  ```
  [[bash: sb spawn talk --objective "{{bash: sb send main one && sb send main two && sb send main three}}"]]
  ```
  Same result with `sb send main '…' && sb report done '…'` from a task.
- **Expected:** one `✉︎ talk → main` row for each message.
- **Actual:** "two" and "three" each show twice. For every message delivered in
  a turn later than its arrival, the hub emits `sb msg-in` twice, with the same
  id:
  ```
  pos 75 sb msg-in : talk m_10 : two        <- arrives while main is busy
  pos 78 --- idle
  pos 79 sb msg-in : talk m_10 : two        <- delivered for the next turn
  pos 80 sb msg-in : talk m_11 : three
  pos 85 --- idle
  pos 86 sb msg-in : talk m_11 : three
  pos 45/50  sb msg-in : bench m_6 : [report: done] p95 at 180 ms   (twice)
  ```
  On screen:
  ```
  ✉︎ talk → main
   one
   two
  :* ack: … id="m_9" …
  ✉︎ talk → main
   two
   three
  ```
- **Suspect:** `hub/core.bend`. `deliver_each` (≈l.575) emits the `msg-in`
  line each time a batch is handed to the agent. A batch handed over mid-turn
  and handed over again at `--- idle` gives two lines. Or `mark_delivered`
  does not stop the second pass. The TUI does not dedupe by `m_<id>` either
  (`rust/tui/src/sb.rs:708`).

## I · `skill` with an unknown name says "skills index unreadable" (fixed in 6586f6d)

- **Severity:** medium on 51c081a.
- **Repro** (live, fresh HOME, workdir with `.agents/skills/bend-harness-qa`):
  `BendSession.fresh()`, then `say('Call the skill tool with exactly {"name":"does-not-exist"} …')`.
- **Expected:** `Skill "does-not-exist" is not available. … Available skills: "bend-harness-qa".`
- **Actual:** `tool_result #6 fail : skills index unreadable: no skills are available`.
  The index was readable, and it held one skill.
- **Location:** `runtime/skills.bend exec_skill.live.found`: `None` (name not
  found) returned the "unreadable" text. `skills-pure.bend not_available()`
  was never called. `6586f6d` fixes this ("a name the index lacks is not an
  unreadable index").

## J · The session's skills/plugins index goes to `~/.bend-harness/run/<port>` in the bise layout

- **Severity:** medium. A fresh install gets a legacy `~/.bend-harness/`
  again next to `~/.bise/`. `BEND_RUN_DIR` (which the harness exports:
  `~/.bise/run`) has no effect on the Bend side. Two runtimes that share a
  HOME but not a `BISE_HOME` write to the same `run/<port>`.
- **Repro:** `HOME=$(mktemp -d)`, clean env, `bise --headless` (via
  `/tmp/qa/run-headless.sh`), wait for READY.
- **Expected:** everything under `~/.bise` (the REPL gets
  `BEND_RUN_DIR=$HOME/.bise/run`).
- **Actual:**
  ```
  $HOME/.bise/cache/skills-index.txt                      0 bytes
  $HOME/.bend-harness/run/52657/skills-index.txt          318 bytes (the workspace skill)
  $HOME/.bend-harness/run/52657/plugins/skills-index.txt  0 bytes
  ```
- **Location:** `runtime/plugins.bend:20-28`. `run_dir.of` is hardcoded to
  `$HOME/.bend-harness/run/<BEND_REPL_PORT>` and never reads `BEND_RUN_DIR`.
  It is still the same at `6586f6d`.

## H · `/restart ` offers bise commits outside bise's source tree, then refuses them

- **Severity:** low.
- **Repro:** start bise in a workspace that is not bise's sources (any
  `git init` folder). Type `/restart ` (with the space).
- **Expected:** outside the source tree, `/restart` only reloads (BISE-131),
  so the popup offers `current` only (or nothing).
- **Actual:** the popup lists `◉ tree [current] the working tree…`,
  `○ 51c081a BISE-136…`, `○ 2e2ff83 …`. Picking one and sending
  `/restart 5339aa5` gives:
  ```
  · /restart reloads bise on the version running now (this workspace is not bise's
    source tree): nothing to build; /version switches versions
  ```
  The command description in the popup also reads
  `/restart [current|<commit>]` in this workspace.
- **Suspect:** the `/restart` argument completion (`rust/tui/src/commands.rs`)
  uses the `versions` list whatever the hub's `restart_plan`
  (`rust/switchboard/src/daemon/versions.rs:49`, dev=false → `Refuse`).

## B · `sb worktree` accepts a path that does not exist, and main can mark itself

- **Severity:** low.
- **Repro:** in main: `[[bash: sb worktree /does/not/exist; echo rc=$?]]`.
- **Expected:** refused (not a directory, or not a git worktree), and refused
  for main, which works in the workspace.
- **Actual:** `the hub knows you work in /does/not/exist`, `rc=0`. The panel
  shows `0 ○ main :* ψ`, and main's divider `ψ exist`. The flag stays across a
  TUI restart. A relative path is refused (`an absolute path or none`), so
  some validation exists.
- **Suspect:** the `sb worktree` handler (hub, BISE-136): it checks only for an
  absolute path.

## C · `bise doctor` on a fresh HOME reports an "old layout" that does not exist

- **Severity:** low. This is the first command of a new user.
- **Repro:** `HOME=$(mktemp -d) bise doctor` (the folder is empty).
- **Actual:**
  ```
  ! home      old layout: /tmp/qa/h1/.bend-harness + ~/.local/state/switchboard — fix: start `bise` once: it moves the state to ~/.bise
  ! migration not done: state still in ~/.bend-harness and ~/.local/state/switchboard — fix: …
  ```
  Neither folder exists. `bise auth list` says `keys: ~/.bend-harness/auth.json`
  too, and `bise login mistral` writes `~/.bend-harness/auth.json` on a fresh
  HOME. The next `bise` start copies it to `~/.bise`.
- **Expected:** "~/.bise does not exist yet — start `bise` once", with no
  migration warning when there is nothing to migrate. A fresh login should go
  straight to `~/.bise/auth.json`.
- **Suspect:** `rust/home` (`Layout::Legacy` when `migrated.json` is absent,
  even with no legacy files) and `rust/harness/src/doctor.rs home_check` /
  `migration`.

## G · `bise doctor` ignores the `[voice]` config

- **Severity:** low.
- **Repro:** config.toml with
  `[voice]\nmodel = "nosuch/whisper"\nlanguage = 42\nvocabulary = [1, "x"]\nbogus = true`.
- **Actual:** `bise models` shows `voice nosuch/whisper (config; unknown provider 'nosuch')`
  and 3 `warning: config.toml: voice.*` lines. `bise doctor` has no voice line
  and no config line: every row is ✓.
- **Expected:** doctor shows the voice model with its key status, and the
  config warnings.
- **Suspect:** `rust/harness/src/doctor.rs` (no voice/config check).

## L · `sb` help gaps

- **Severity:** low.
- `sb spawn --help` → `unknown option: --help`. `sb spawn -h` →
  `sb spawn: --objective is required`.
- The `sb` usage (the text printed by `sb` alone) does not list `sb worktree <path>|none` (BISE-136).
- **Suspect:** `rust/switchboard/src/cli.rs`.

## P · `ctrl+r` does nothing, with no hint, when voice mode is off

- **Severity:** low.
- **Repro:** a fresh TUI, press `ctrl+r`.
- **Actual:** nothing: no line, no hint. The `/voice` description says
  "(ctrl+r speech-to-text)". After `/voice`, ctrl+r records (`▁` meter).
- **Expected:** a one-line hint ("voice mode is off: /voice turns it on").
- **Suspect:** `rust/tui/src/voice*`.

## E · ASCII mode: the card box and the rails stay Unicode

- **Severity:** cosmetic. Visual QA #12 is still partly open.
- **Repro:** `BISE_ASCII=1 NO_COLOR=1`, `sb card "pick one\n1. alpha\n2. beta"`, ctrl+g.
- **Actual:**
  ```
  |  ┎ ? main needs you . 1 of 2 . 23s ───────────────────────────╮  |
  |  ┃ pick one                                                   │  |
  |  ┖ alt+r answer with text . ctrl+x later . ctrl+f full screen ╯  |
  |   ┃ ? docs needs you                                             |
  |  │    what's on your mind?                                       |
  ```
  The frame is ASCII (`+ - |`), but the card box, the `┃` card bar in the
  feed, the popup box `┃` and the composer's `│` rail are not.
- **Suspect:** the card box and composer rail drawing in `rust/tui/src`
  (render/cards/ui) do not go through the `theme.rs` ASCII table.

## F · The header ends with a lone separator

- **Severity:** cosmetic.
- **Repro:** `/drop docs`, then `/restore docs` (only idle agents left).
- **Actual:** `+- bise :* ------- /…/sb-e2e-28pb_ixj/ws .  -+` (Unicode mode:
  `· `).
- **Expected:** no separator when the count segment is empty (or the text "no
  agents yet"). The header does say "no agents yet" right after the drop.
- **Suspect:** `chrome::share_room` / the header counts in `rust/tui/src/panel.rs` or `ui.rs`.

## K · The `✉︎` chip row is 1 column off in tmux

- **Severity:** cosmetic (it depends on the terminal).
- **Repro:** any level-3 message (`sb send main hi` from a task), in tmux 120×40.
- **Actual:** on the chip row the feed's right border `│` is 1 column to the
  right of the other rows, and the panel's contents move too (e.g.
  `   │  #2 ✓ bench …` shifted). `✉︎` is U+2709 + VS15. tmux draws it 1 wide;
  the TUI counts it differently.
- **Suspect:** the chip glyph width in `rust/tui/src/render.rs` / `theme.rs`.
  Use a glyph without a variation selector, or measure it the way the terminal
  does.

## M · The panel reuses a dropped agent's number and lists it last

- **Severity:** cosmetic.
- **Repro:** agents 1–7, drop `wt1` (#4), spawn another.
- **Actual:** `3 ○ talk2`, `5 ○ place`, `6 ○ fixer`, `7 ○ docs-2`,
  `4 · abcdefghijk…`. The new agent gets ⌥4, the old wt1's key, and sits at the
  bottom.
- **Expected:** a new number (8), or rows kept in number order.
- **Suspect:** the panel numbering (`rust/tui/src/panel.rs`).

## N · In an agent's view, the divider shows two "working" timers

- **Severity:** cosmetic.
- **Repro:** a task that ran `sb status working --note …` and then a long
  command. Open it (⌥4).
- **Actual:** `├─ you → abcdefghijklmnopqrstuvwx ∿≈ working · 32s ───── working · 0s · 15 / 128k tokens · 0% …`
- **Expected:** one status and one timer.
- **Suspect:** the divider label and right side in `rust/tui/src/ui.rs`: the
  task status timer vs the REPL turn timer.

## D · `bise models <filter>` with no match is silent

- **Severity:** cosmetic.
- `bise models zzz` prints the model/agent/voice summary and the config path,
  with no "no provider or model matches 'zzz'" line (rc 0).

---

## Checked, no bug found

- Zen: the chrome fades on typing (frame `#322e2b` vs `#4a4540` after 5 s) and
  comes back.
- Drafts per agent: kept across ctrl+c and a TUI restart.
- `Esc` on text: the draft goes to the history. `Esc` on the `/` popup clears
  the composer. Both are as coded, though surprising.
- Selection → `❝ 1` quote chip: sent as `<selection from="main">`, and the
  history row shows `❝ line2 · main · 1 line`.
- Slash argument completion for `/restore` and `/drop`.
- `/restart` reload in a non-bise workspace: hub restarted, REPL adopted.
- `/new -w`: worktree `sb/fixer`, ψ.
- `sb spawn --worktree`: ψ in the panel and on the divider.
- Role line in the header.
- Card answer with alt+r.
- `/drop` closes the agent's card. `/restore` works.
- Renamed agents keep working with their old `SB_AGENT`.
- `sb` error paths: unknown recipient, self-send, bad name, a duplicate name
  becomes `docs-2`, drop main, close of an unknown card.
- Narrow terminals: 60 and 34 columns.
- `NO_COLOR`.
- `bise login`/`logout`: auth.json is 0600; exit codes are 1 on errors.
- `tests/mcp_bootstrap.py`: PASS.
- Live Mistral: bash round trip; main spawns a task that reports done.
