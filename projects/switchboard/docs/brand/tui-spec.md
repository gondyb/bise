# bise terminal UI: design spec v0

Design only, not implemented. Mockups: [tui-mockup.html](tui-mockup.html) (one
screen, interactive) and [tui-screens.html](tui-screens.html) (every feature of
today's TUI) and [tui-live.html](tui-live.html) (an 80-second live simulation
with a timeline; `?t=45&paused` opens it at 45 s).
Brand board: [board.html](board.html). Items marked **⚠** need a check or a
decision.

## Why change

The current TUI reuses the OpenCode dark theme as is (`rust/tui/src/theme.rs`:
primary `#fab283`, braille spinner, `┃` bars). It looks like OpenCode, not like
us. The features are right; the look and the reading comfort are not.

## Principles

1. **You stay in control, the screen stays calm.** Agents work quietly. Only
   what needs you gets color. A finished task arrives like a *bise*: a light
   touch, not an alarm.
2. **Everything is readable.** Prose never wraps wider than a novel line.
   Contrast is high enough for long reading.
3. **Progressive disclosure.** One line per step by default; details one
   keypress away. Scripts are the exception: always shown in full.
4. **A visual language of symbols.** Every entity and every status has one
   glyph. Color is for attention only.
5. **Few words to learn.** The user meets four things: *you*, *main*,
   *agents*, *cards*. The UI says "agent", never "task", "hands" or "hub"
   (Gabriel: avoid new vocabulary). **⚠** Today's UI and `/help` say "task";
   rename them.
6. **Lowercase chrome.** All UI labels lowercase (proper nouns and acronyms
   excepted), like the marketing.

## Must keep (Gabriel)

- **bash and TypeScript scripts are shown in full in the history**, never
  truncated or collapsed by default, with syntax colors.

## Visual hierarchy: what's for you, what isn't

Three levels, always the same, in main and inside an agent:

| Level | What | Look |
|---|---|---|
| 1 · needs you | a question or a blocker addressed to you | blush bar `┃` on the left, bold blush title with `?`, normal text; stays until answered, also in the card box |
| 2 · for you | what main or an agent says to you: replies, summaries, main answering on your behalf, reports on your requests | normal text, `:*` or the status glyph in front |
| 3 · between agents | messages agents send each other and to main | dim text under a faint rail, `✉ from → to  text`, one line each, disclosed with ▸ when long |

- Traffic between agents is **always in the history**, so what happened stays
  understandable. Main's level-2 line after it is the summary for you.
- Color is reserved for level 1 (and errors). Levels 2 and 3 differ by
  brightness and the rail, not by hue.
- No "quiet" mode (Gabriel): the levels and the folding of long runs already
  keep the screen calm.
- **⚠** Today the hub's messages to main reach the feed as plain turns; the
  TUI needs the sender, the recipient and the level for each line.

## Your message: sent, received, read

WhatsApp's double check, at the end of your own line (Gabriel):

| Mark | Meaning | Wire today |
|---|---|---|
| `·` (faint) | sending | the client sent it |
| `✓` (faint) | the agent got it, waiting for its turn | `steering_received` |
| `✓✓` (accent) | the model read it | `steered` |
| `✗ not delivered` (red, own line) | the agent is gone; `⏎` send again | **⚠** new |

- Replaces today's separate info lines ("steering received: …", "steering
  passed to the model: …"): less noise, and the answer to "did it get my
  message?" sits where you look.
- The same marks on level-3 lines for notifications
  (`notification_received` / `notification_delivered`), faint, so you can
  see an agent hasn't read main's message yet.
- A message sent at idle starts the turn at once: it goes straight to `✓✓`.

## History order (invariant)

- **The history is append-only, in arrival order.** No section per agent that
  gets updated later, no reordering, no line that moves. What changes over
  time (status, age, context fill, open cards) lives outside the history: the
  agents panel, the header counts, the card box.
- **Content is frozen; small status marks are not.** The only in-place
  change on an existing line is its status mark (`✓` → `✓✓` on your
  message). The line never moves and its text never changes.
- **Only the tail can grow.** A run of level-3 lines at the very bottom may
  fold and its count may go up while the run lasts. As soon as a level-1 or
  level-2 line is appended, the run is closed and frozen.
- Inside one agent's feed, a tool call and its result stay one item (the
  agent does nothing else in between), as today.
- Views are filters over the same stream, never a regrouping: entering an
  agent shows its own thread; **⚠** a possible filter "only lines about
  @name" keeps the arrival order.

Staying calm with dozens of agents, without breaking the order:

1. **Fold runs of level 3.** A run longer than 3 lines shows as one dim line
   `▸ 47 messages between 30 agents`; ▸ opens it in place, in order.
2. **Fixed name column.** `✉ from → to  text`, names padded or truncated to 10
   columns, so the eye scans down one column.
3. **Time marks after a pause.** A faint `· 14:31 ·` only after 5 minutes
   without a line (**⚠** threshold to tune).
4. **Main summarizes bursts** at level 2 ("the 12 endpoint agents agreed on
   one error format").
5. **The panel scrolls, the history doesn't grow sideways.** With more
   agents than rows, the panel shows the ones that need you first, then the
   working ones, then `+ 21 more`.

## Line width (measure)

- **Prose** (user messages, agent text, reports, cards): wrap at
  `min(terminal width − margins, 76)` columns. Extra width goes to the left
  margin and the task panel, not to longer lines.
- **Code** (scripts, diffs, tool output): up to 100 columns; longer lines wrap
  with a hanging indent and a faint `↪` marker.
- **⚠** 76 is a starting value (novel lines are ~60–75 characters); tune after
  trying it.

## Readability

- The terminal font is the user's choice; we control contrast, spacing and
  weight.
- All text meant to be read: contrast ≥ 4.5:1 on the background (WCAG AA).
  The very faint color is only for rails and decoration, never for text.
- One blank line between blocks. One bold level. No italic for long text.
- Marketing screenshots: JetBrains Mono (free) or a similar clean mono.

## Themes: light and dark

Decided (Gabriel): two themes, light and dark, nothing more for now.

- **The background stays the terminal's own.** ratatui can paint any
  background (`Color::Rgb` on every cell), but that fights the user's
  terminal (their exact color, transparency, blur). We draw on
  `Color::Reset` and only set foreground colors, plus at most a light tint
  for the selection and the card box. (Today `theme.rs` paints some panels
  `#141414`; that goes.)
- **Pick the theme automatically.** At start, ask the terminal for its
  background color (OSC 11; Ghostty, iTerm2, kitty, WezTerm, Terminal.app,
  Alacritty answer it) and choose light or dark by its brightness. A setting
  forces one. **⚠** Fallback when the terminal doesn't answer (e.g. some tmux
  setups): dark.
- Both palettes keep every readable text ≥ 4.5:1, checked against white,
  our cream, black and a typical dark grey (`#282c34`).

## Palette (dark)

| Role | Hex | Use |
|---|---|---|
| background | `#141211` | warm near-black (or the terminal's own background, **⚠** to decide) |
| text | `#ece6da` | cream, all readable text |
| dim | `#a39c90` | secondary text, durations, labels |
| faint | `#4a4540` | rails, borders, line numbers only |
| accent | `#f4a6b0` | pale pink (Gabriel: less like an error): the `:*` mark, "needs you", focus; 9.7:1 |
| error | `#ff5a52` | failures only |
| ok | `#b9d99a` | diff additions only |

Syntax colors stay, but softer (see the mockup).

## Palette (light)

| Role | Hex | Contrast on white |
|---|---|---|
| text | `#1b1917` | 17.5:1 |
| dim | `#6b645a` | 5.8:1 |
| faint | `#cfc8bd` | rails and borders only |
| accent | `#b8416b` | 5.2:1 (a raspberry pink, darker so it stays readable, pink not red) |
| error | `#b3261e` | 6.5:1 |
| ok | `#3f7a2a` | 5.2:1 |

**⚠** Not mocked yet; the light syntax colors are still to pick.

## Symbols

Entities:

| Glyph | Entity |
|---|---|
| `›` | you (also the composer prompt) |
| `:*` | bise, main (the ASCII kiss, blush) |
| `◇` | a task brief |
| `∴` | thinking |
| `$` | bash call |
| `λ` | TypeScript call |
| `±` | file edit (patch) |
| `✉` | message between agents |
| `?` | card: a decision that needs you |
| `↳` | a sub-call inside a TypeScript run, or a steer |
| `⟳` / `≡` | compaction running / its summary |
| `▲` (dim) | turn interrupted |

Task status:

| Glyph | Status |
|---|---|
| `∿` (pulsing) | working: a breeze |
| `…` | waiting on another task |
| `?` (blush) | needs you |
| `♡` | done |
| `✗` (red) | failed |
| `○` (dim) | idle |
| `·` (dim, pulsing) | starting |
| `–` (dim) | stopped |

Marks next to an agent: `•` unread activity, `⎇` own worktree (no mark: the
shared folder), context fill as `21%`. Card kinds reuse the status glyphs, plus
`⇄` overlap (two agents changed the same file), `↻` failed restart, `–` drop
confirmation. Versions: `⧗` building / on trial.

**⚠** Check that `∿ ∴ ◇ ± ✉ ♡ λ` exist and are single-width in common
terminal fonts (SF Mono, Menlo, JetBrains Mono, Fira Code, Cascadia) and in
Ghostty, iTerm2, Terminal.app, kitty, WezTerm. Fallbacks to pick where not.

## Progressive disclosure

| Item | Default | Disclosed |
|---|---|---|
| thinking | one line: `∴ thought for 14s ▸` | full text |
| bash / TypeScript script | **always in full** | n/a |
| tool output / result | one line: `▸ output · 42 lines · 1 failed` | full output |
| file edit | one line: `± edit path ✓ +3 −1 ▸` | the diff |
| task report (in main) | one line: `♡ bench is done. <summary> ▸ report` | full report |
| brief (in checkout) | one line: `◇ brief ▸` | full brief |
| card | open while it needs you | collapses to one line once answered |

Keys (**⚠** to align with the existing feed selection in `feedsel.rs`): move
over items, `space` toggles one item, one key toggles all thinking, one key
toggles all outputs.

## Layout

- Header: `bise :*` on the left; on the right a live count
  `∿ 2 working · ? 1 needs you · ♡ 1 done`. **⚠** A cost in $ is not
  available today (only tokens per agent); add it once the usage work lands.
- Feed on the left (prose ≤ 76 columns), task panel on the right titled
  `agents`, one line per agent: its shortcut, glyph, name, age or state,
  context fill. The panel title says how to switch once, `agents · ⌥ + number` (short
  on purpose: it must fit the narrowest panel, 26 columns, on one line),
  and each row starts with its number in the faint color (0 main, 1…9 the
  first nine agents, blank after), so switching is discoverable without
  `/help`.
  **⚠** `⌥` is macOS; show `alt+1` on Linux and Windows.
- Main answering an agent on your behalf is shown in the feed, with `▸ why`.
- **No undo** (Gabriel, 2026-09-28). Agents may already have acted, so an
  undo promises too much, and it is one more concept. To change something,
  you say it ("no, v1 for docs"); main sends the agent an explicit correction
  ("the user changed their mind: …") and confirms in one line. Today's
  `ctrl+z` (cancel a route not yet delivered) goes away.
- Checkout view: a one-line banner "you are talking to auth-fix directly. main
  is not in the loop. esc back to main."
- Composer at the bottom: `› ` prompt, key hints on the right, dim. The
  status row above it starts with the name of the agent you talk to
  (`main`, `auth-fix`) in the accent color (Gabriel), so you always see who
  gets your message; the rest of the row stays dim.

## Images

Follows the technical plan of the `screenshots` task
([../images.md](../images.md)): an image is an atomic placeholder in the
composer; sources are a dragged file, `ctrl+v` (clipboard) and the `@` popup.

- **Chip.** In the composer and the history, an image is a blush chip
  `▣ 1` / `▣ login.png`, not `[Image #1]`. **⚠** Cosmetic only; the text
  label underneath can stay what the implementation uses.
- **Strip above the composer** while images are attached: number, source
  (file name or "clipboard"), size in pixels and kB, and "resized to fit
  2048" when it applies. Backspace on a chip removes the image.
- **History.** Your line keeps the chips with the file name; `▸` opens a
  preview. **⚠** A real thumbnail needs the kitty or iTerm2 image protocols,
  not planned in images.md.
- **Routing.** Main says it passes the images on ("layout-fix takes it, with
  both images"). **⚠** To check: the image marker must travel in the brief.
- **Tool results.** A script that returns an image shows `result · ▣
  screenshot.png 390×844 ▸`.
- **Model without vision.** Plain words and a way out: "glm-5 can't read
  images. pick a model that can (/model), or describe the screen in words."
  **⚠** Today the provider's raw error shows.

## Onboarding (first launch)

Mockup: [tui-onboarding.html](tui-onboarding.html). Inspired by Vibe's
(`vibe/setup/onboarding`: typed welcome, live theme preview, one choice per
screen). Six steps, `enter` to go on:

1. **Welcome.** Typed: "hi, i'm bise", then the `:*` pops in blush, then
   "your ideas. my hands. lots of them." (Gabriel) and "press enter ↵". Voice rule: in the
   product bise speaks as "i"; "you" is always the user. So the site tagline
   "you, but with way more hands." is not used inside the product.
2. **Theme.** "your terminal looks dark, so i picked dark." Two live
   previews side by side; ←→ switches, `/theme` changes it later.
3. **Model.** "which model should do the work?" A key found in the
   environment comes first ("use ANTHROPIC_API_KEY · found"). **⚠** The list
   depends on the providers bise ships with; browser sign-in is not built.
4. **Folder, and one honest thing.** "i'll work in ~/lab/app · a git repo ✓.
   all your agents share this folder." Then: "agents run commands here without
   asking you. git is your safety net." **⚠** No approval mode yet; it will
   probably ship before release. Then the copy becomes: "agents ask you before
   risky commands (push, deleting outside this folder). change it in
   /settings."
5. **How it works, in three lines.** `›` you talk, `∿` agents on the right
   (⌥ + number, esc), `?` a card when someone needs you.
6. **The real first run, with just-in-time hints.** No tour: each hint shows
   once, next to the thing, the first time it happens (first agent, first
   messages between agents, first card), and goes away when you use it.

## Coverage of today's features

Checked against `rust/tui` on 2026-09-28 (`help.rs` rows, `sb/panel.rs`,
`sb/cards.rs`, `wire.rs` events, `commands.rs`). Every item has a screen in
[tui-screens.html](tui-screens.html):

first run · routing · main answering for you · you change your mind · reports in main ·
main sending work back · question card · every card kind (question, blocked,
failed, restart, drop, overlap, done) · full-screen card · the agents panel in
every state (starting, working, waiting, needs you, blocked, done, failed,
idle, stopped, unread, worktree, archived) · preview (space) · checkout ·
everything disclosed · failing tool · steer and interrupt · `@agent` messages
and the `@` popup · agent-to-agent messages · compaction · provider and hub
errors · `/version` build and trial · slash commands · voice · terminal panel
· help · drop confirmation · narrow terminal · long history.

Not built today, shown with **⚠** in the screens: files in the `@` popup (at-files task in progress), one key to open
every output, cost in $. Not mocked: the text editor keys and selection (no
visual change), emoji completion, plugins (in progress).

## Decisions and backlog

Decided with Gabriel. Nothing is sent to implementation yet: the task list
is written once every open question below is settled.

To implement:

1. Replace the OpenCode theme with two bise palettes, light and dark, on the
   terminal's own background, chosen by querying it (OSC 11).
2. The glyph set for entities and statuses (tables above).
3. Prose wraps at 76 columns, code at 100 with a hanging indent.
4. bash and TypeScript scripts always shown in full, with syntax colors.
5. Progressive disclosure: thinking, outputs, diffs, reports, brief behind ▸;
   keys to open one item, all thinking, all outputs.
6. Say "agent" everywhere in the UI and `/help`, never "task".
7. Agents panel: title `agents · ⌥ + number`, a number per row.
8. The 3 levels (needs you / for you / between agents); the hub tags each
   feed line with its sender, recipient and level.
9. Append-only history: fold runs longer than 3, only the tail run grows,
    time marks after a pause, fixed name column.
10. No undo: remove `ctrl+z`; main's prompt sends explicit corrections.
11. First-run copy: "what's on your mind? / say it and keep talking. the work
    runs in the background, i'm always here."
12. Header: `bise :*` and the live counts.
13. Lowercase chrome (proper nouns and acronyms excepted).
14. Onboarding: the 6 steps above, and the one-time hints (stored, so each
    shows once per user).
15. Images: chips, the attachment strip, readable error for models without
    vision.
16. Message marks `·` `✓` `✓✓` on your lines (and faint on notifications),
    replacing the steering info lines.

Open questions:

- A card once answered: fade it in place (gray, "answered"), or leave it
  as it was and let the answer follow below?
- Glyph coverage in common fonts and terminals; `alt+` labels off macOS.
- Cost in $ in the header (needs the usage work).
