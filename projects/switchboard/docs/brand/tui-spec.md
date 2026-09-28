# bise terminal UI: design spec v0

Design only, not implemented. Mockups: [tui-mockup.html](tui-mockup.html) (one
screen, interactive) and [tui-screens.html](tui-screens.html) (every feature of
today's TUI, 26 screens).
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

## Palette (dark)

| Role | Hex | Use |
|---|---|---|
| background | `#141211` | warm near-black (or the terminal's own background, **⚠** to decide) |
| text | `#ece6da` | cream, all readable text |
| dim | `#a39c90` | secondary text, durations, labels |
| faint | `#4a4540` | rails, borders, line numbers only |
| accent | `#f2766b` | blush: the `:*` mark, "needs you", focus |
| error | `#ff5a52` | failures only |
| ok | `#b9d99a` | diff additions only |

Syntax colors stay, but softer (see the mockup). **⚠** A light theme is not
designed yet.

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
  context fill. The shortcut is always shown, in the faint color, with a space (`⌥ 0` main, `⌥ 1`…`⌥ 9` the first
  nine agents, blank after) so switching is discoverable without `/help`.
  **⚠** `⌥` is macOS; show `alt+1` on Linux and Windows.
- Main answering a task on your behalf is shown in the feed, with its undo
  (`ctrl+z`).
- Checkout view: a one-line banner "you are talking to auth-fix directly. main
  is not in the loop. esc back to main."
- Composer at the bottom: `› ` prompt, key hints on the right, dim.

## Coverage of today's features

Checked against `rust/tui` on 2026-09-28 (`help.rs` rows, `sb/panel.rs`,
`sb/cards.rs`, `wire.rs` events, `commands.rs`). Every item has a screen in
[tui-screens.html](tui-screens.html):

first run · routing and route undo · main answering for you · reports in main ·
main sending work back · question card · every card kind (question, blocked,
failed, restart, drop, overlap, done) · full-screen card · the agents panel in
every state (starting, working, waiting, needs you, blocked, done, failed,
idle, stopped, unread, worktree, archived) · preview (space) · checkout ·
everything disclosed · failing tool · steer and interrupt · `@agent` messages
and the `@` popup · agent-to-agent messages · compaction · provider and hub
errors · `/version` build and trial · slash commands · voice · terminal panel
· help · drop confirmation · narrow terminal · long history.

Not built today, shown with **⚠** in the screens: undo of an answer main gave
for you, files in the `@` popup (at-files task in progress), one key to open
every output, cost in $. Not mocked: the text editor keys and selection (no
visual change), emoji completion, plugins (in progress).
