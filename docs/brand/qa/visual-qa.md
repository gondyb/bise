# BISE-82 · visual QA

Every screen of [`tui-screens.html`](../tui-screens.html) and
[`tui-onboarding.html`](../tui-onboarding.html), reproduced in the real TUI
and compared with the mockups and the book (§5 colors, §6 glyphs, §8–§17).

- **Build:** HEAD `217b648` (includes BISE-83 part 1 and part 2, `6a3a770`
  and `bb3f2f9`). A first pass on `9c79318` found the light regressions
  listed at the end, since fixed.
- **How:** `qa/capture.py` runs a throwaway hub (the scripted fake provider
  of the e2e tests, `SB_DEV_ROOT=/tmp/bise-qa-*`) and the TUI in tmux at
  150×42, scripts the agents with `sb spawn` / `sb report` / `sb send` /
  `sb card`, and captures each screen with `tmux capture-pane -e` (colors
  kept). `qa/ansi2html.py` turns the captures into HTML (the terminal's
  default colors of the pass: dark `#141211` / `#ece6da`, light `#f7f4ee`
  / `#1b1917`), and headless Chrome turns the HTML into PNG (no Screen
  Recording permission needed). `qa/colors.py` lists every color of a
  pass with the §5 role it matches.
- **PNG:** only the shots cited below (`*.png`, ~135 KB each); every
  other shot is in `.html` (open in a browser) and `.ansi` (`cat` in a
  terminal). Regenerate a PNG with headless Chrome: `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --hide-scrollbars --screenshot=x.png --window-size=1220,790 file://…/x.html`.
- **Passes:** `shots/dark/`, `shots/light/` (`BISE_THEME=dark|light`),
  `shots/ascii/` (`BISE_ASCII=1`, dark), onboarding in dark and light
  (`SB_ONBOARDING=on`). Re-run:
  `python3 -u docs/brand/qa/capture.py dark|light|ascii [onboarding]`.
- **Limits:** the fake provider only runs scripts and answers `ack: …` /
  `done: …`, so main's own words (routing, summaries, corrections, "why")
  can't be reproduced; its raw `ack: <agent_message …>` lines in the
  captures are the fake provider, not the TUI. Screens that need a
  model's behavior, a microphone, a version switch or 30 live agents are
  marked **n/a**.

## Status after the fixes (recapture on `44acc36` + BISE-90 `7c032cd`)

`shots/dark/` and `shots/light/` were recaptured on that build (bise now
paints its own background, BISE-92). The ascii and onboarding shots are
still from `217b648` (ascii 02, 03, 29 recaptured by BISE-91).

| # | status |
|---|---|
| 1 report twice | **fixed** (BISE-90 a5b841f): blocked keeps the level-1 card, done and failed keep the report line (dark/02) |
| 2 info / warn wrap at column 1 | **fixed** (a5b841f, 7c032cd): spawn, provider error, opened reports hang under their text (dark/02, 30) |
| 3 feed scrollbar | **fixed** (a5b841f): only while scrolled up, a faint `┃` thumb, no arrows or track; none at the tail |
| 4 agent reply at column 1 | **fixed** (a5b841f): starts at the glyph column (dark/13) |
| 5 `·` after the answer | **fixed** (7c032cd): the hub steered it with an agent message + the status block; an unmatched steer now marks what you sent this turn (dark/17 `·`, dark/18 `✓✓`) |
| 6 `/` popup cuts | fixed by BISE-91 |
| 7 compaction | **fixed** (a5b841f): `≡ compacting` pulsing, then `≡ summary ▸` folded (dark/27, 28) |
| 8 help key caps | fixed by BISE-83 (`da63fa7`) |
| 9 `✉` in the book | fixed in the book (`4728293`) |
| 10 id in the `to` column | **fixed** (a5b841f): `@ talk-a    → main      …` (dark/03) |
| 11 hint row repeats the card keys | fixed by BISE-91 |
| 12 ASCII | fixed by BISE-91 |
| 13–16 | open (provider wording: runtime; narrow header: BISE-91; onboarding: BISE-94; brief capitals: M) |
| new | your messages have a thin accent bar `│` on every row (user decision, BISE-90) |

## Differences, most visible first

| # | what | where (shot) | likely owner |
|---|---|---|---|
| 1 | **Every report shows twice in main.** An agent's `sb report` gives a card line (`┃ ? docs needs you` + body, `♡ bench is done: …`, `✗ deploy failed: …`) **and** a report line (`? docs: … ▸ report`, `♡ bench: …`, `✗ deploy: …`) right after it. The mockup "reports in main" has one line per report. | dark/02, 03 | hub (card + `msg-in` report for the same event) / F render.rs: pick one |
| 2 | **Info and warning lines wrap at column 1.** The spawn line `· ✚ main → new agent @deploy : …` and the provider error `▲ model call failed …` continue at column 1, not under their text (every other kind hangs its rows). | dark/02, 03, 30 | F, render.rs (Info / Warn rows) |
| 3 | **A scrollbar on the feed** (`▲ ║ █ ▼`, right edge of the feed, every screen with a long history). No mockup has one; the book says calm. | all | F / P (ui.rs, feed) |
| 4 | **An agent's reply inside its own view starts at column 1** with no glyph or indent (`done: tool bash ok: …`), while its brief, tools and outputs sit at column 3. In main the same reply has `:* ` and the indent. | dark/13, 14 | F, render.rs (assistant lines, agent view) |
| 5 | **A message typed during a turn stays `·`** after main has answered it (`› also check the logs ·` then `:* ack: also check the logs`); it turns `✓✓` only a few seconds later, at the next event. Mockup: `✓` when received, `✓✓` when read. | dark/17, 18 (✓✓ by 22) | F (BISE-15 marks: the queued-message path) |
| 6 | **The `/` popup cuts its descriptions** (`turn voice mode (ctrl+r speech-to-text) on o…`) while the terminal has 90 free columns; the `/version` picker too. Mockup: one line each, whole. | dark/22, 24 | K, commands.rs (popup width) |
| 7 | **Compaction:** the running line is `≡ compaction 44 (manual)` (should be `⟳`, and `44 (manual)` is raw), the summary is `≡ summary ......` with no `▸`. Mockup: one line while it runs, then the summary behind `▸`. | dark/27, 28 | F, render.rs (Compact / Compacted) + the wire text |
| 8 | **Help key caps paint a background** with raw colors (fg `#eeeeee` on bg `#3a3a44`, help.rs:251-252), not theme roles, in both themes (a dark chip on the cream light terminal). Book §5: never paint the background. | dark/23, light/23 | K (BISE-83, next commit, told) |
| 9 | **Level-3 glyph is `@`, the book says `✉`** (§6, §9 table, copy deck). The code chose `@` (theme.rs, "was ✉"); the mockups and the onboarding preview use `@`. | dark/03, onboarding-3 | main: update the book |
| 10 | **A message to main shows its id in the `to` column** (`@ talk-a      m_13      logout is fine`) instead of `→ main`. Mockup: `✉ from → to  text`. | dark/03 | F, render.rs (BISE-14 choice) |
| 11 | **Hint row repeats the card keys** (`alt+r answer with text · ctrl+x later · ctrl+f full screen`) right under the card box that already shows them. | dark/04–09 | K / P, ui.rs hint_text |
| 12 | **ASCII mode:** `+` stands for `♡` done, `◇` brief and `✚` spawn (three meanings); the ellipsis `…` becomes `:` (`No such file or di: >`); box drawing stays Unicode (`┃` card bar, `│` rails, `║ █` scrollbar); `⌥ + number` reads `M + number`. | ascii/02, 03, 29 | K / T, theme.rs ASCII table |
| 13 | **Provider error wording:** `▲ model call failed (attempt 2/10): cannot reach 127.0.0.1:… — check your network or VPN · retry 3/10 in 2s` vs the deck's `✗ the model provider answered {code}. retrying in {s}s ({i} of {n}).` (the deck marks it not built). | dark/30 | runtime (provider) + F |
| 14 | **Narrow terminal:** the header keeps only `? 1` of the counts (mockup keeps them). | dark/29 | P, panel.rs header |
| 15 | **Onboarding, theme step:** with `BISE_THEME` set it says "i couldn't read your terminal's background, so i picked dark." (mockup: "your terminal looks dark, so i picked dark."). Model step: the wrapped detail `~/.bend-harness/config.toml.` loses its indent. Folder step: a long path wraps mid-phrase (`a git` / `repo ✓`). | onboarding-3, -4, -5 | O, onboarding.rs |
| 16 | **Brief text is capitalized** (`Objective: …`, `No end criterion: this is a long-running task. Stay available.`) inside a lowercase UI. | dark/14 | M, prompts.rs (model-facing text: maybe keep) |

## Screen by screen

| screen (tui-screens.html) | result | shots |
|---|---|---|
| first run | OK | 01 |
| you ask, main starts agents | differs: #1, #2 (main's words n/a) | 02 |
| what's for you, what isn't | differs: #1, #9, #10 (levels 1/2/3 look right: accent bar, text, dim rail) | 03 |
| you change your mind | n/a (model behavior) | — |
| a busy hour, 30 agents | partly: level-3 lines OK; the fold `▸ 4 messages between 2 agents` OK when the run is long enough (light/04); 30 agents n/a | 03, light/04 |
| reports in main | differs: #1 | 02, 03 |
| main sends work back | n/a (model behavior) | — |
| cards: a question | OK (`┎ ? main needs you · 1 of 4 · 7s`, text at 76, `1 v1   2 v2`, dim keys); #11 | 04 |
| cards: every kind | OK for question, blocked, failed, done; drop, overlap, restart n/a | 05–09 |
| a card, full screen | OK (`· full screen · ctrl+f back`, pgup/pgdn) | 10 |
| the agents panel, every state | OK for needs you, done, failed, working, idle, unread `•`, numbers; waiting, stopped, worktree `⎇` not reached | 02, 03, 11 |
| preview an agent | OK-ish: `preview · docs · ⏎ enter · esc close` on top; the panel shows 2 preview rows under the agent | 12 |
| inside an agent (checkout) | differs: #4; the header line and brief `▸` OK | 13 |
| everything disclosed | OK (ctrl+o opens brief and output) | 14 |
| markdown tables | OK (bold header, one faint rule per column, `:---:` / `---:` honored) | 15 |
| a table in a narrow terminal / that can't fit | not captured at those widths (the table layout has unit tests, BISE-87) | — |
| a failing tool | OK: `$ bash ✗` in error, the script, the reason (3 rows: the script shows in full since BISE-11) | 16 |
| your message: sent, received, read | differs: #5 | 17, 18 |
| a turn in progress: steer, interrupt | OK (`$ bash ∿ 3.7s`, the steer line waiting) | 17 |
| talk to an agent from main | OK (`· → you → @bench : …`, `@ bench to you: …`) | 19 |
| images: attaching | OK (strip `▣ 1 <path> 1×1 · 66 B`, `look at this ▣ 1`, hint `ctrl+v paste image · @ file`) | 20 |
| images in the history | OK (`▣ shot.png 1×1` under the message) | 21 |
| images from tools, models without eyes | n/a (needs a tool returning an image / a model without vision) | — |
| agents talk to each other | OK (level 3 inside and in main); #9, #10 | 03 |
| compaction | differs: #7 | 27, 28 |
| provider or hub errors | differs: #2, #13 | 30 |
| versions: build, try, roll back | OK for the picker (`◉ tree [current]`, commits); build/trial n/a | 24 |
| slash commands | differs: #6 | 22 |
| voice | n/a (microphone) | — |
| the terminal panel | OK (`┌ terminal · ctrl+\` hide`, hint row) | 25 |
| help | differs: #8 | 23 |
| drop an agent | OK (`drop docs? its history stays in archived. y / n`) | 26 |
| a narrow terminal | differs: #14; panel hidden OK | 29 |
| a long history | n/a (50k lines; covered by the tmux paging test) | — |

| onboarding (tui-onboarding.html) | result | shots |
|---|---|---|
| welcome (typed) | OK | onboarding-1, -2 |
| theme (live preview, ←→) | differs: #15 (wording with a forced theme) | onboarding-3 |
| model | OK (lists the keys found; env dependent); #15 indent | onboarding-4 |
| folder | OK; #15 path wrap | onboarding-5 |
| how it works | OK | onboarding-6 |
| first run + one-time hints | first run OK; hints not triggered by this script (n/a) | onboarding-7, -8 |

## Colors (book §5)

`qa/colors.py` over every capture at `217b648`: every foreground is a §5
role (text, dim, faint, accent, error, ok, the syntax roles, on_accent) in
both themes; the only backgrounds are `selection_bg` (`#33292c` dark,
`#faeef0` light) and the accent popup selection, plus #8 (help key caps).

**Fixed during this QA** (seen on `9c79318`, gone at `217b648`, told
bise-k-keys): in light, every reply of main was drawn in the dark text
color `#ece6da` (markdown.rs aliases: near-invisible on a light terminal);
the `/` popup, `/version` and `/help` used the dark dim / faint / accent.
