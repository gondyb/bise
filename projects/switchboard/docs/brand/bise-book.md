# the bise book

Brand book, product spec and implementation plan for **bise**, the terminal
UI of Switchboard. One document, three parts:

- **Part I, brand:** who we are, how we sound, how we look.
- **Part II, product spec:** what the terminal UI does, screen by screen.
- **Part III, implementation plan:** how main agents split the work into
  non-overlapping tasks. The issues themselves live in
  [bise-issues.md](bise-issues.md), one section per issue, with status and
  notes.

Status: design decided with Gabriel on 2026-09-28. Nothing implemented yet.
Claims that are not true yet are marked **⚠**. When this book and an older
doc disagree, this book wins.

Visual references (open them in a browser):

| File | What |
|---|---|
| [site/book/screens.html](site/book/screens.html) | every screen of the product, 32 mockups + the levels and symbol legends |
| [site/book/live.html](site/book/live.html) | an 80-second live simulation; you can type and send messages |
| [site/book/onboarding.html](site/book/onboarding.html) | the first launch, 6 steps (validated as is) |
| [tui-mockup.html](tui-mockup.html) | the first single-screen mockup (superseded by tui-screens) |
| [site/index.html](site/index.html) | the one static site: the landing page, and the book under `site/book/` (old `tui-*.html` paths redirect) |
| [board.html](board.html) | the brand exploration (3 art directions + the chosen one) |
| [tui-spec.md](tui-spec.md) | the design log (how we got here) |
| [../pitch.md](../pitch.md) | positioning and go-to-market |

---

# Part I — Brand

## 1. Name and story

- **Name: `bise`.** Always lowercase, like a command you type. Four letters,
  checked free on npm, crates.io, PyPI and Homebrew (**⚠** `bise.sh` and
  `bise.ai` are taken; domain, GitHub org and trademarks not checked).
- **The story, told once:** *bise* is French for a little kiss, and the name
  of a cold north wind. Light, quick, friendly. Working with bise should feel
  like that: a light touch, not a weight.
- **The mark: `:*`**, the ASCII kiss. The logo is `bise :*`. It works in any
  terminal and any font, and looks typed by a human.
- **Standalone brand.** bise is its own brand, unrelated to Mistral: no
  Mistral colors, no Mistral mention in the copy. (The code runs on Mistral's
  Unified Harness; what can ship under a separate brand is an open question,
  see [../pitch.md](../pitch.md).)

## 2. What we promise

**The problem, said plainly.** A day coding with agents feels empty. You
shipped more than ever and feel like you did nothing: progress bars, "v1 or
v2?", tab switching, reading what an agent did while you looked away. You feel
slow, useless, out of control, and got nothing back for the fun you gave away.
It's not you, it's the interruptions: every agent pulls you out of your head
every few minutes, and flow needs quiet to start. You never get there.

**bise preserves your flow.** Built for humans piloting hundreds of robots,
from a distance. (**⚠** "hundreds" is the image, not a tested number: no limit
in the product, but tokens, cost and provider speed are the real limit.)

- **Audience:** people with lots of ideas who move fast, and want the tech to
  disappear and keep up with them. Developers who already use a coding agent
  every day and have hit the wall at 2 or 3 agents.
- **The value is human:** flow, control, lightness. You talk to one agent;
  it runs as many as the work needs; you never wait, and you never have to
  manage them. Only the decisions that are really yours reach you.
- **Honesty rules:**
  - no number as a promise ("10x"): it sounds like a cap, and the product has
    none;
  - the real limits are tokens, cost and provider speed: say so;
  - never claim a feature that isn't built; mark it **⚠** in internal docs.

## 3. Taglines

| Where | Line | Status |
|---|---|---|
| site, headline | **kiss your backlog goodbye.** | picked (Gabriel) |
| site, subline | *ramble. interrupt. change your mind. i run the agents. you stay in flow.* | picked (replaces *built for engineers who think faster than they type.*, 2913f0a) |
| inside the product (onboarding) | **ideas in. little kisses out. also pull requests.** | picked (Gabriel; replaces "your ideas. my hands. lots of them.") |
| in reserve | you, but with way more hands. · your team is as big as your ideas. | former headlines |

**The name gloss**, outside the product only (site hero, README, launch
posts); inside, the welcome screen has the short one (§15):

> **bise** /beez/ · french, n. 1. a quick kiss on the cheek. 2. a brisk north
> wind. 3. a terminal where your agents ship while you think.

Rejected, don't bring them back: "all the agents. none of the overhead."
("overhead" is unclear), "all the agents. stay in flow.", "code like a team
of ten." (a number), "all your ideas. none of the juggling." (sounds like an
LLM).

## 4. Voice

- **No capital letter at the start of a word**, on every marketing piece, all
  website text and all UI chrome. It reads like a human typing.
  - Exceptions: proper nouns keep their capitals (Mistral, Claude Code,
    GitHub, Ghostty); acronyms stay in caps (API, MCP, PR), though "cli" in
    lowercase is fine; ALL CAPS to shout is allowed, extremely rarely.
  - Our own name stays lowercase: `bise`.
- **Who speaks.** Inside the product, bise speaks as **"i"**; **"you"** is
  always the user. ("say it and keep talking. the work runs in the
  background, i'm always here.") The site talks to the reader as "you", so
  "kiss your backlog goodbye." is a site line, never an in-product line.
  Why it works: an English idiom everyone knows, turned: the backlog goes,
  and the kiss gives the name its meaning for English speakers.
- **Human, short, concrete.** Casual, warm, a bit cheeky. Short sentences.
  Show the moment instead of naming the feeling. Avoid polished LLM patterns
  ("X. None of the Y.", "seamless", "unleash", "supercharge").
- **Few words to learn.** The user meets four things: **you, main, agents,
  cards.** Never say "task", "hands", "hub", "sub-agent", "orchestrator" in
  the UI. (Internal docs and code may keep "task".)

## 5. Visual identity

**Art direction: "terminal brut, with the soul of the bise."** Raw terminal
credibility (monospace, calm, the product is the visual) plus one warm human
touch (the pale pink accent, the `:*`, a heart for "done").

**Background.** bise paints its own background (BISE-92, Gabriel's call;
it replaces "the terminal's background shows through"): every cell gets the
theme's ground (dark `#141211`, light `#fdfbf7`), so the text reads whatever
the terminal's colors, its transparency, or a wrong theme pick. On that
ground, only two tints: the selection and the card box. Where the terminal
supports it (OSC 11), its own default background is set to the same ground
so the padding around the grid matches; it is always given back (OSC 111,
then the color read at start) on exit, on a crash and when a shell takes
the terminal. A terminal that ignores OSC 11 just keeps its padding color.

**Two themes, light and dark,** chosen automatically from the terminal's
background (OSC 11 query), with a setting to force one. Every readable text
is ≥ 4.5:1 (WCAG AA) on white, our cream, black and a typical dark grey
`#282c34`.

| Role | Dark | Light | Use |
|---|---|---|---|
| text | `#ece6da` (15:1) | `#1b1917` (17.5:1) | everything you read |
| dim | `#a39c90` (6.9:1) | `#6b645a` (5.8:1) | secondary text, level 3, durations |
| faint | `#4a4540` | `#cfc8bd` | rails, borders, numbers; **never** for text you must read |
| accent | `#f4a6b0` pale pink (9.7:1) | `#b8416b` raspberry (5.2:1) | the `:*`, "needs you", the agent you talk to, `✓✓` read, the bar on your messages |
| error | `#ff5a52` | `#b3261e` | failures only |
| ok | `#b9d99a` | `#3f7a2a` | diff additions only |
| ground (`bg`) | `#141211` | `#fdfbf7` | every cell (BISE-92); tints on it: selection `#33292c` / `#fdeef2`, card `#211d1b` / `#f1eee6` |
| raised | `#1f1c1a` (1.10:1 on the ground; text 13.6, dim 6.2, accent 8.8, error 5.5) | `#f4f0e8` (1.10:1; text 15.4, dim 5.1, accent 4.6, error 5.8) | the composer pane: everything under the divider (BISE-102, user request). Ground not ours (`BISE_TERM_BG=0`, or OSC 11 answers another color): the ground mixed 5% toward the text color. Not truecolor: 234 / 255. 16 colors or `NO_COLOR`: no tint, the bar alone |
| chip | `#231f1d` (text 13:1, dim 5.9:1) | `#efe9df` (dim ≈ 4.8:1) | the level-3 message chip only (BISE-106). 16 colors / `NO_COLOR`: no tint |

- **Color means attention.** Only "needs you" and errors get a hue. Everything
  else is text, dim or faint. The accent is pink, not red, so "needs you"
  never looks like an error.
- **Syntax colors** (scripts, diffs): soft versions of the usual palette. Dark:
  keyword `#d7a6f0`, string `#b9d99a`, comment `#857e74`, number `#f0b27a`,
  call `#8fc4f0`. **⚠** Light syntax colors: to pick (issue BISE-01).
- **Marketing type:** JetBrains Mono (free) for screenshots and the site. In
  the terminal the font is the user's.
- **Motion:** only the working pulse `∿`, the blinking cursor, the typed
  welcome, the `:*` pop. Nothing bounces, nothing slides.

## 6. Symbol language

One glyph per entity and per status. Color only for attention.

**Entities**

| Glyph | Meaning |
|---|---|
| `›` | the composer prompt, and queued messages above it |
| `│` (accent) | your message in the history: a thin bar in column 1, on every wrapped line (heavy `┃` is for cards). ASCII: `|` |
| `:*` (accent) | main, the agent you talk to by default; it never moves: while main works, the panel's breathing gust sits 1 space after its name (`0 :* main ≈`, BISE-116) |
| `◇` | an agent's brief |
| `∴` (dim) | thinking |
| `$` | a bash call |
| `λ` | a TypeScript call |
| `↳` | a sub-call inside a TypeScript run |
| `±` | a file edit (patch) |
| `@` | a message between agents, or an agent writing to you (was `✉`: can turn into a color emoji) |
| `▣` (accent chip) | an image |
| `?` (accent) | a card: a decision that needs you |
| `≡` (dim; pulsing while running) | compaction running / its summary (was `⟳`: in almost no font) |
| `▲` (dim) | turn interrupted |

**Agent status**

| Glyph | Status |
|---|---|
| `·` (dim, pulsing) | starting |
| `∿` → a gust blowing by (`≈∿~·`), animated; one breathing cell in the panel | working: a breeze (user pick, site/book/working.html variant I; BISE-107) |
| `…` | waiting on another agent |
| `?` (accent) | needs you (question or blocked) |
| `✓` (accent) | done (Gabriel, 2026-09-29: the `✓` was not clear; a small pink check reads "finished"). It leads a panel row or a report; your read marks `✓` / `✓✓` sit at the end of your own lines, so the place tells them apart |
| `✗` (error) | failed |
| `○` (dim) | idle |
| `–` (dim) | stopped |

**Marks**

| Glyph | Meaning |
|---|---|
| `·` → `✓` → `✓✓` | your message: sending → the agent got it → the model read it (`✓✓` in accent) |
| `•` (accent) | unread activity in an agent |
| `ψ` (picked in BISE-84: width 1, in every installed audited font) | the agent has its own worktree (no mark: the shared folder); was `⎇`, in almost no font |
| `⇄` | overlap: two agents changed the same file |
| `↻` (error) | a restart failed |
| `Δ` | a version is building or on trial (was `⧗`: in no font) |
| `▸` / `▾` | closed / open (progressive disclosure) |

**Decided by Gabriel (2026-09-28), after the glyph audit (BISE-03,
[glyph-audit.md](glyph-audit.md)):** keep the brand glyphs (`∿`, `:*`; `♡` only outside the product since BISE-100) and
every glyph a fallback font draws at width 1; replace only the ones that break:
`✉` → `@` and `↪` → `»` (color-emoji risk), `⟳` → `≡` pulsing, `⧗` → `Δ`,
`⎇` → `ψ` (BISE-84). `BISE_ASCII=1` switches every glyph to plain
ASCII (`~` working, `<3` done, `>` you, …) for terminals that draw them badly
(BISE-84). Terminals must use ambiguous width = narrow (the default).

**ASCII forms** (`BISE_ASCII=1`, BISE-84, made distinct in BISE-91): one
cell each, and no two entities share one.

| Glyph | ASCII | | Glyph | ASCII | | Glyph | ASCII |
|---|---|---|---|---|---|---|---|
| `›` you | `>` | | `▲` interrupted | `^` | | `✓` got it | `v` |
| `:*` main | `:*` | | `»` wrap | `}` | | `✓✓` read | `vv` |
| `◇` brief | `&` | | `·` starting, sending | `.` | | `•` unread | `!` |
| `∴` thinking | `:` | | `∿` working | `~` | | `ψ` worktree | `Y` |
| `λ` TypeScript | `\` | | `…` waiting | `;` | | `⇄` overlap | `/` |
| `↳` sub-call | `L` | | `✓` done | `*` | | `↻` restart failed | `(` |
| `±` edit | `%` | | `✗` failed | `x` | | `Δ` building | `A` |
| `▣` image | `#` | | `○` idle | `o` | | `▸` / `▾` | `+` / `-` |
| `≡` compaction | `=` | | `–` stopped | `_` | | `$` `@` `?` | themselves |

A cut text ends with `...` (not the one-cell `;`), and the panel title and
the help keys say `alt + number` instead of `⌥ + number`. Chrome glyphs
outside §6 (`⏎ ← → ↑ ↓ ⇧ ● ◉ ◆ ✚ ◀ ▪ ×`) keep their table forms through
`theme::asciify`. Box drawing stays (it is drawn by every font).

---

# Part II — Product spec (the terminal UI)

## 7. Principles

1. **You stay in control, the screen stays calm.** Agents work quietly. Only
   what needs you gets color. A finished agent arrives like a bise: a light
   touch, not an alarm.
2. **Everything is readable.** Prose never wraps wider than a novel line.
   Contrast ≥ 4.5:1 for all text you read.
3. **Progressive disclosure.** One line per step by default, details one key
   away. Exception: bash and TypeScript scripts are always shown in full.
4. **One symbol per thing** (§6).
5. **The history never lies.** Append-only, in arrival order (§10).
6. **No new vocabulary** (§4): you, main, agents, cards.
7. **Honest.** No silent failure, no hidden limit, nothing we pretend to
   have. (Don't talk about undo in user-facing copy: it doesn't exist for the
   user, so mentioning it is an artifact. §13 keeps it for implementers.)

## 8. Layout

```
 bise :*                              ∿ 3 working · ? 1 needs you · ✓ 1 done    ← header
                                                    │ agents · ⌥ + number
  │  the login breaks on safari                     │ 0 :* main
  :* on it: auth-fix takes it.                      │ 1 ∿ auth-fix     12m · 21%
    │ @ docs      → main      v1 or v2?             │ 2 ∿ release       3m · 8%
    │ @ main      → docs      v2, the brief says so │ 3 ? docs              you
  :* docs asked v1 or v2; the brief says v2,        │ 4 … api-v2     waits docs
     so i answered. ▸ why                           │ 5 ✓ bench            done
  ┃ ? docs needs you                                │
  ┃ the brief says "keep old clients working"…      │
 ┌ card box (when a card is open) ──────────────────┐
 main · idle · 210k / 1M tokens · 21%                                             ← status row
 › _                                          ⏎ send · @ agent · / commands       ← composer
```

- **Header:** `bise :*` on the left; live counts on the right, only the
  non-zero ones: `∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done`.
  **⚠** No cost in $ until the usage work lands.
- **Feed** on the left, prose ≤ 88 columns; extra width goes to the margin
  and the panel, never to longer lines.
- **Scrollbar:** only while you are scrolled up from the bottom: faint, one
  column, no arrows. Never at the tail (BISE-90).
- **Agents panel** on the right (hidden under 70 columns; the header keeps the
  counts). Title `agents · ⌥ + number` (fits 26 columns on one line). One
  row per agent: its number (faint; 0 main, 1–9 the first nine agents, blank
  after), status glyph, name, and on the right the age and context fill
  (`12m · 21%`), or `you` (accent) / `done` / `waits docs` / `starting`.
  Marks `•` unread and `⎇` worktree after the name. Numbers never change
  while an agent lives (creation order). With more agents than rows, it
  scrolls and ends with `+ 21 more`. Archived agents: keep what landed in
  85160ab (a dim folded `▸ {n} archived` row at the bottom, click / `A` /
  `/archived` to open, read-only history, `/restore`); restyle only.
- **Card box** above the status row when a card is open (§12).
- **Status row:** the name of the agent you talk to **in accent** (`main`,
  `auth-fix`), then dim: state, context (`210k / 1M tokens · 21%`), `shared
  folder` or `⎇ branch`, and transient notes (`preview of auth-fix`).
- **Composer:** `› ` prompt; key hints on the right, dim, lowercase.
- **First run** (no agents yet), in the feed, dim:
  ```
  what's on your mind?

  say it and keep talking. the work runs in the background, i'm always here.

  try: "fix the flaky login test, and draft the release note"
  ```

**The reading column** (user request, marketing 82f1742). The history is a 91-column column (3 for the lead + 88 of text; widened from 79 by Gabriel, 2026-09-29: +15%), centered in the space left of the panel: F = terminal width − panel (30) − 1; x0 = floor((F − 91) / 2) when F ≥ 95, else column 1. Tables and code start at x0 and may run to 103 columns (capped at F − 1), extending right, never re-centered. The status row, the queue, the images strip, the composer block and its hints use the same x0 and width (hints right-aligned to x0 + 91). The agents panel stays flush right. Under 70 columns the panel hides and F = width.

**Spacing, in cells** (user request, marketing 0e6e803). The rule is the landing demo, translated to whole cells.
- **Outer margins:** 2 columns left and right, 1 row top and bottom; under 30 rows the top and bottom rows go.
- **Header:** its own row: `bise :*` bold at the left margin, the summary (`∿ 3 working · ✓ 4 done`) flush right; then 1 blank row.
- **History:** the reading column above (91 wide, centered in the feed area = everything left of the panel).
- **Between feed and panel:** 3 blank columns. No vertical rule: whitespace and alignment do the job.
- **Scrollbar:** no arrows, no track: only a faint `┃` thumb in the last column of the feed area, and only while you are away from the bottom (the status row says `↓ back to the bottom`).
- **Agents panel:** 28 columns, flush right at the right margin, first row level with the history's first row; title `agents · ⌥ + number` dim, then 1 blank row; rows `N glyph name` with the age right-aligned; a name takes all the room its row leaves and is cut with `…` only there (no fixed cap; BISE-109, user request).
- **Between history blocks:** 1 blank row (§10), and 1 above and below level 2 (§9).
- **Card box:** the reading column's x and width, 1 blank row above; heavy bar `┃` in the column's first cell, text from its 4th; title, body, 1 blank row, then the choices and keys row.
- **Bottom stack:** 1 blank row under the history, then the status row, queue, strip, composer block (§13), hints row, 1 bottom margin row, all on the reading column's x and width (the hints end at its right edge).
- **Narrow terminals:** ≥ 100 columns as above; 90–99: panel 24 wide, gap 2; < 90: no panel (the header summary grows to `∿ 3 working · ⌥ + number`), margins 2, column = min(91, width − 4) (at 80: 76 wide, 73 of text); at 60: margins 1, column 58 (55 of text). The column is centered only when the feed area has at least 95 columns, else it starts at the left margin.
- **What the demo does that a terminal can't:** its own font (the terminal's is the user's), line height (1.2 vs 1.6), sub-cell gaps (the site's 10–18 px become 0 or 1 whole row: we take 1), 1 px rules (a terminal rule is a full cell), fade and slide motion. bise does paint its theme background (§5, BISE-92), but no rounded panels. The demo is the reference for rhythm and proportions, not for exact pixels.

**The frame** (user request on 805e538, marketing 9f000c8; replaces, in "Spacing, in cells" above, the outer margins, the header row, "no vertical rule" and the bottom stack's status and hints rows; the reading column, the panel widths and the narrow tiers stay). bise draws itself like an app: a thin faint frame on the edge of the terminal, with "bise :*" and the summary in its top border. Inside, 2 blank columns on each side. The agents panel sits behind a faint rule that joins the frame. A full-width divider separates the history from the composer pane. The lines are faint so the text stays in front: the frame is a shape, not a decoration. Under 60 columns or 16 rows the frame goes, the divider stays.

Exact layout (terminal F columns × H rows, 0-based; all lines faint; ASCII: `+ - |`):

Frame (when F ≥ 60 and H ≥ 16):
- rounded frame on the terminal edge: row 0, row H−1, column 0, column F−1 (╭ ╮ ╰ ╯ ─ │).
- row 0 = the header. `╭─ bise :* ───…─── ~/acme · ∿ 3 working · ? 1 needs you ─╮`: title from column 3 ("bise" bold text, ":*" accent), 1 space around it and the summary; the summary dim (its glyphs keep their colors), ending at column F−4. Not enough room: drop the path first, then use the short counts (∿ 3 · ? 1 · ✓ 2).
- row 1 blank. The history starts on row 2 and ends 1 blank row above the divider.
- inside the frame: 2 blank columns each side. Text starts at column 3.
- panel, F ≥ 100: a rule │ at column F−33, joined with ┬ on row 0 and ┴ on the divider; panel text from F−31 to F−4 (28 columns); the history ends at column F−36. F 90–99: panel 24 wide (rule at F−29). F < 90: no panel, no rule.
- scrollbar: a dim ┃ thumb drawn on the panel rule (on the frame's right border if there is no panel), only while you are scrolled away from the bottom.
Composer pane (bottom up):
- H−1 frame bottom. H−2 the key bar. The composer: 1 bar row, the text, 1 bar row (the same above and under, BISE-111). [1 blank row] [the attachments] [queued lines, BISE-89]. The divider.
- divider: a full-width rule `├─ you → main ─────…───── idle · 18k / 1M tokens · 2% ─┤` joining the frame. Label from column 3: "you →" dim, the agent name accent. The state is dim and ends at F−4. This replaces the status row (recording, typing, tokens: same text as today). When the agent you're viewing works, the label shows it right after the name: `you → marketing ∿ working · 42s` (the indicator 1 space after the name in its own color, then `working · 42s` dim, the seconds of the current turn); idle: nothing after the name (BISE-105, user request; the indicator glyph is today's `∿` until the user picks one of the variations in site/book/working.html).
- composer: bar │ at column 3 on every row of the composer (the blank bar rows around the text too; faint while empty, accent with text, an image or recording), text from column 6 (x0 + 3, the history's text column; BISE-108, user request), wrapped at word boundaries like your message in the history. At least 1 row (an empty composer sits centered between its bar rows, BISE-111), growing to min(12, 40% of H), then scrolling. Empty: cursor then a dim placeholder "what's on your mind?" (to an agent: "talk to auth-fix directly").
- key bar, from column 3: keys in text color, what they do dim, 3 spaces between pairs; default `⏎ send   @ agent   ⌥0-9 switch   / commands   ? help`; per-mode sets as today. On the right, ending at F−4: a dim tip (e.g. "tip · ctrl+o opens everything folded") that changes every 5 minutes, going through the tips in order, so you meet more of them over time (user request, 2026-09-29); it never changes while you type, hidden while you type or when there are fewer than 3 columns between it and the keys.
- height: divider + blank + 1 + blank + key bar + frame = 6 rows at rest (BISE-111, user request: symmetric and half the padding of BISE-108), the same as today's block + bottom margin. The frame gives back 1 row at the top (border+blank instead of margin+header+blank). It costs 2 columns (3 each side instead of 2).
Small terminals: H < 24: drop the blank row under the text. H < 20: also the one above. H < 16 or F < 60: no frame. Then a header row on row 0, the divider is a plain ─ rule, margins of 1, the key bar stays.
The frame and the rules paint no background of their own: the theme ground (§5, BISE-92) stays everywhere; light theme = same tokens.

## 9. Three levels: what's for you, what isn't

The same three levels everywhere, in main and inside an agent.

| Level | What | Look |
|---|---|---|
| **1 · needs you** | a question or a blocker addressed to you | accent bar `┃` on the left, bold accent title `? docs needs you`, normal body; stays until answered; also in the card box |
| **2 · for you** | what main or an agent says to you: replies, summaries, reports on your requests, main answering on your behalf | normal text, with `:*`, a status glyph (`✓` `✗`) or `@ name to you:` in front |
| **3 · between agents** | messages agents send each other and to main | dim text under a faint rail: `@ from → to  text`, names padded to 10 columns, one line each, `▸` when long |

- **Your messages** carry a thin accent bar `│` on the left, on every wrapped line, text at column 3; marks `·` `✓` `✓✓` at the end. Thin bar = you, heavy bar `┃` = needs you, so you can tell at a glance what you said from what the agents said (decided by Gabriel, 2026-09-28).

- Traffic between agents is **always in the history** (including between two
  agents that aren't main), so what happened stays understandable. Main's
  level-2 line after a burst is the summary for you.
- Levels 2 and 3 differ by brightness and the rail, never by hue.
- Main deciding for you is level 2 and says why on demand:
  `:* docs asked v1 or v2; the brief says v2, so i answered. ▸ why`.
- No "quiet" mode: the levels and the folding (§10) already keep it calm.

**Emphasis** (user request, marketing 82f1742). A terminal has one font size, so what's for you reads bigger through contrast and room: level 2 in text color with its speaker in bold (`:*` accent bold, `@ name to you:` text bold) and a blank row above and below, even between two level-2 blocks; the agent's own work (thinking `∴`, the one-line tool calls, level 3) is dim; cards (level 1) unchanged. OSC 66 text sizing is never used in the history (§15 may use it for the welcome line only, where detected).

**Level 3 is an envelope chip** (user pick, site/book/messages.html variant C, marketing 346dacb; BISE-106). Between agents, a message looks like a message: a small tinted chip with an envelope says who writes to whom (✉ auth-fix → release, the sender in bold), and the text follows, dim. You can follow the conversation at a glance, and it never shouts.
- One message = one line group at x0, flush with the text (the chip's first tinted cell at x0, `✉︎` at x0+1, the sender at x0+3; BISE-109, marketing e567233): the chip, 1 ground space, the text. Chip = tinted cells ` ✉︎ sender → receiver ` (1 tinted column each side; palette role `chip`, §5). `✉︎` = U+2709 U+FE0E (text presentation, 1 column; if a terminal still draws it 2 wide, fall back to `@`). Envelope dim, sender bold text, `→` faint, receiver dim. Names cut at 24 with `…` (BISE-109, user request: room for real names; was 12). `main` is a plain name here (no `:*`, no accent): it's level 3.
- 16 colors / `NO_COLOR`: no tint, the chip reads `[✉ sender → receiver]`. ASCII: `[@ sender > receiver]`.
- Text: dim, wraps under its own first column (hanging indent after the chip); at most 2 lines, then `… ▸` opens it whole. Chip + text < 30 columns wide: the text goes on the next row at x0+2.
- Spacing: messages of the same pair stack with no blank row; a new pair after a blank row. The fold line `▸ n messages between k agents` stays dim at x0, no chip. Levels 1 and 2 unchanged.
- **Short on room (the chip)** (W = reading width at x0): a name is cut only when the row really lacks room (BISE-109). W ≥ 60: names up to 24, then `…`; the text starts after the chip if at least 30 columns are left, else on the next row at x0+2. 40 ≤ W < 60: names up to 16; the text always on the next row at x0+2 (hanging), still ≤ 2 lines then `… ▸`. W < 40: the chip without inner padding and spaces, `✉auth-fix→release`, still tinted, at x0, names up to 10; text on the next row at x0. Never cut the arrow or the envelope; cut the receiver before the sender. 16 colors / `NO_COLOR` and ASCII forms follow the same cut rules. The fold line is cut from the right with `…` when narrow. Level 3 stays the quietest thing on screen: the chip tint is its only background, no accent anywhere in it (main included).

**A box that only sends** (user request, BISE-110). A bash box whose whole script is one `sb send`, `sb ask` or `sb report` (any flags, after at most one `cd <dir> &&`) says nothing the chip under it doesn't: it hides, if and only if the command succeeded and the message it sent (matched by the id in its output: `sent m_12`, `reported (m_12)`, an ask's `reply from docs (m_13, answers m_12)`, the question and the reply both) is drawn below it in the same feed. Anything else in the script (`;`, a pipe, a redirection, `$( )`, a second command), a failure, or no such message drawn (a task's own sends: only main's feed draws them): the box stays. A hidden box takes no row and does not split a run of level 3 (main's sends to one agent stack, and fold after 3 like any run, §10); ctrl+o shows it again with everything folded. The hub's `msg` line carries the id for this (`msg : main → docs m_12 : text`).

**Working = a gust blowing by** (user pick, site/book/working.html variant I; BISE-107).
- Header and divider: 5 cells. A gust crosses left to right, 110 ms a frame, 9-frame cycle: head `≈` (text), tail `∿` (text) `~` (dim) `·` (faint), then 5 empty frames. Cell k at frame i = ramp[(i − k) mod 9], ramp = `≈ ∿ ~ · _ _ _ _ _` (`_` = space).
- Panel status (1 cell): the gust breathes in place: `· ~ ∿ ≈ ∿ ~`, 110 ms a frame, same colors. main's `:*` in the panel never moves; while main works, its row gets the same breathing cell 1 space after the name: ` 0 :* main ≈      42s` (BISE-116); idle main: no cell.
- Divider (BISE-105): `you → marketing <5-cell gust> working · 42s` (1 space around the gust; `working · 42s` dim). Idle: nothing after the name. Header: `<gust> 3 working · ? 1 needs you · ✓ 2 done`.
- ASCII: ramp `. - ~ =` (head `=`), same motion.
- Cost: redraw only those cells; ≤ 10 fps; stop when no agent works or the terminal loses focus.
- **Short on room (the gust):** divider label, full: `you → marketing ≈∿~·  working · 42s` on the left, the state (tokens, %) on the right. Not enough room: drop in this order, one step at a time, until it fits with ≥ 3 columns between left and right: (1) the right-side state; (2) the word `working · ` (keep `42s`); (3) the gust 5 cells → 3 cells (same ramp, cycle 7: `≈ ∿ ~ · _ _ _`); (4) the gust → the 1-cell breathing form (`· ~ ∿ ≈ ∿ ~`); (5) the seconds; (6) last, cut the agent name at 20, then at 12, with `…` (BISE-109; was 12 then 8). The gust never disappears while the agent works: it's the last thing kept after `you → name`. Header: F ≥ 90: `<5-cell gust> 3 working · ? 1 needs you · ✓ 2 done`; 70–89: 3-cell gust + short counts `3 · ? 1 · ✓ 2`; < 70: 1-cell breathing + short counts. Panel: always the 1-cell breathing form. No motion (the terminal loses focus, the redraw budget is hit, or a reduce-motion env is set): a static `∿` in text color everywhere (`BISE_ASCII=1` keeps the motion with `. - ~ =`).

## 10. The history (invariant)

- **Append-only, in arrival order.** No section per agent that gets updated
  later, no reordering, no line that moves. What changes over time (status,
  age, context, open cards) lives outside the history: panel, header, card
  box.
- **Content is frozen; small status marks are not.** The only in-place
  change on an existing line is its status mark (`✓` → `✓✓` on your message;
  a card's answered state, see §12).
- **Only the tail can grow.** A run of level-3 lines longer than 3 folds into
  one dim line `▸ 47 messages between 30 agents`; `▸` opens it in place, in
  order. Only the last run, at the bottom, can still grow (a small pulsing
  `∿` shows it's live). As soon as a level-1 or level-2 line is appended, the
  run is closed and frozen.
- **Time marks** after a pause: a faint `· 14:31 ·` after 5 minutes without a
  line (**⚠** threshold to tune).
- Views are filters of the same stream (entering an agent shows its thread),
  never a regrouping.

## 11. Reading

- **Measure.** Prose wraps at `min(width − margins, 88)`. Code (scripts,
  diffs, outputs) up to 100 columns; longer lines wrap with a hanging indent
  and a faint `↪`.
- **Scripts in full.** bash and TypeScript scripts are always shown whole,
  with syntax colors. (Today `render.rs` folds code over 60 lines to 40:
  that goes for scripts.)
- **Progressive disclosure**

| Item | Default | Disclosed |
|---|---|---|
| thinking | `∴ thought for 14s ▸` | the full text |
| bash / TypeScript script | **always in full** | — |
| bash / TypeScript output | inside its box, 15 rows (see *Scripts: a box*) | the full output |
| other tool results | `▸ output · 42 lines · 1 failed` | the full output |
| sub-calls | `↳ github.search_issues ✓` | — |
| file edit | `± edit web/src/auth/session.ts ✓ +3 −1 ▸` | the diff |
| report in main | `✓ bench is done. p95 at 180 ms ▸ report` | the report |
| brief (inside an agent) | `◇ brief ▸` | the brief |
| a run of level 3 | `▸ 12 messages between 8 agents` | the messages, in order |
| a card | open while it needs you | see §12 |

  Keys: click or `space` on the selected item toggles it; `ctrl+o` opens or
  closes everything folded, one state (like Claude Code; `ctrl+t` is gone).
- **Scripts: a box** (user request, marketing 82f1742). Each bash or typescript call is a box, rounded (`╭─╮ │ ╰─╯`), the width of the reading column, content at 2 columns of inner margin, its title in the top border (`╭─ $ bash ∿ 12s ─…╮`, `λ typescript ✓ 1.1s`, `$ bash ✗ exit 1 · 0.8s`). Border: text color while running (with the pulsing `∿`; not the accent), faint when done, error when failed. Inside: the script in full (never capped), a faint rule `├──┤`, then the output, dim, 15 rows at most while closed: running = the last 15 as they stream, with a faint `… n lines above`; done ok = the first 5, a dim `▸ n more lines`, the last 9; failed = the last 15 with `… n lines above`; 15 lines or fewer: all, no fold. `▸` or `ctrl+o` opens the whole output in place and closes back to the same 15. TypeScript sub-calls `↳ github.search_issues ✓ 0.8s` are output lines inside the box. ASCII: `+- $ bash ok 0.9s ---+`, `|`, `+---+`, `> 27 more lines`. The box replaces the separate folded `▸ output · n lines` line and the code/output left rails.
- **Failures**: a failing bash/typescript call is a box with an error border (above); other failing tools (edit, read, web) stay one line in error color with the reason, `▸` for the full error.
- **Markdown** in messages: headers, lists, quotes, code fences, inline
  bold/italic/code, and GFM tables (BISE-87): no frame, columns 2 spaces
  apart, bold header over one faint `─` per column, aligned by display
  width (`---:`, `:---:`); up to the code measure, then the widest column
  shrinks and wraps inside it (a blank line between rows once one wraps);
  too many columns: one `title` + `  key  value` block per row.

## 12. Cards

- A card is level 1: accent bar, `? <agent> needs you`, the question, then the
  choices when the agent gives some (`1 v1  2 v2`), and the keys, dim:
  `alt+r answer with text · ctrl+x later · ctrl+f full screen`.
- **Kinds** reuse the glyphs: `?` question and blocked (accent), `✗` failed,
  `↻` restart failed (error), `–` drop confirmation, `⇄` overlap, `✓` done.
  Sorted by what blocks an agent first, then the oldest.
- **Keep `ctrl+f`** (today): full screen and scrollable (`pgup` / `pgdn`);
  `ctrl+f` or `esc` brings it back.
- Other keys as today: `ctrl+g` show/hide the box, `ctrl+n` / `ctrl+p` next /
  previous, `alt+r` answer with the composer text (empty: acknowledge a done
  card), `ctrl+x` close without answering, `y` / `n` / `esc` on a
  confirmation.
- **Answered cards fade in place** (decided by Gabriel, 2026-09-28): once
  answered, the card in the history turns grey (dim bar, `answered`), your
  answer follows as a normal line. BISE-31.

## 13. Talking to agents

- **To main:** `⏎`. Main says who takes what: `:* on it: auth-fix takes the
  safari bug, release takes the note.` New agents appear in the panel.
- **To an agent directly from main:** `@name text` (popup with the agents).
  Main is not in the loop; the agent's reply to you is level 2:
  `@ auth-fix to you: got it, i'll check logout after the login fix.`
- **Inside an agent:** `⏎` on a selected agent or `⌥ + number`. One dim line
  says: `you're talking to auth-fix directly. main isn't in the loop. esc back
  to main.` `@main` goes back up.
- **Steering and marks.** While an agent works, `⏎` steers its turn. Your
  line ends with a mark: `·` sending, `✓` the agent got it
  (`steering_received`), `✓✓` in accent the model read it (`steered`). This
  replaces today's info lines "steering received: …" and "steering passed to
  the model: …". A message at idle goes straight to `✓✓`. If the agent is
  gone: `✗ not delivered: auth-fix stopped. ⏎ send again · esc drop` (**⚠**
  new).
- **Queued messages** (BISE-89, after Codex). During a turn, `tab` keeps
  the composer text for after the turn instead of steering: it stays in
  the TUI, **nothing goes to the hub** until it leaves the queue. The queue
  shows just above the composer, one dim line each (` › text…`, cut to the
  width), newest last, then a faint `queued · sent when this turn ends · ↑
  edit`. `↑` in an empty composer pops the newest back to edit (the
  history comes after the queue); `tab` queues it again, `⏎` steers it
  now, clearing the composer drops it. When the turn ends, the oldest goes
  out as a normal message (marks `·` → `✓✓`) and starts the next turn; the
  next one waits for that turn to end. One queue per agent (main and each
  agent), kept even out of view; the panel row shows `· {n} queued`. A
  restart of the TUI drops the queue (old lines never fire at an idle
  agent after a restart).
- **No undo.** Agents may already have acted, so an undo promises too much.
  To change something, you say it ("no, v1 for docs"). Main sends the agent an
  explicit correction (`the user changed their mind: use v1, not v2.`) and
  confirms in one line (`:* told docs: v1, you changed your mind.`). Today's
  `ctrl+z` / `/cancel` go away.
- `ctrl+c` interrupts the turn of the agent in view; again (or at idle)
  quits; the agents keep running.

### The composer block (layout; user request, marketing 393dbd3)

Bottom of the screen, top to bottom:

1. the status row (1 row);
2. the queued messages (BISE-89) and their faint hint, if any: they are the composer's pending texts, so they sit right above it;
3. the images strip, if any (the composer's attachments, §14);
4. the composer: a bar `│` in column 1 on every row of the block, faint while the composer is empty, accent as soon as there is text (the same bar your message keeps in the history, so a sent message just moves up unchanged); 1 blank row (bar only) above the text and 1 below; the text from column 3, at least 2 rows, growing one row per wrapped row up to min(12, 40% of the terminal height), then scrolling with the cursor row in view; right margin 2 columns; the text wraps at the same width as your message in the history (§11), so the composer shows how it will read. Empty: the cursor at column 3 and the dim placeholder. Recording: the bar in accent, the meter glyph at column 3 of the first text row, the text after it. `›` leaves the composer (it stays for the queued lines).
5. the key hints: their own last row, dim, flush right with 2 columns of margin (never on the text row).

Rows: 1 status + 1 + 2 text + 1 + 1 hints = 6 at minimum (+ the blank row under the feed). Small terminals: height < 24 drops the bottom blank row; < 18 also the top one, and the minimum goes to 1 text row; < 14 the hints go back on the status row.

**The composer pane** (framed, raised; user request, marketing e71ec74; supersedes the list above for the status row and the hints, see §8 "The frame" for the frame rows). The bottom of the frame belongs to you. The divider says who you talk to (you → main, the name in blush) and, on the right, what they're doing. Everything under the divider is slightly raised, like the bottom of an app: a tinted row, your text behind the same thin bar your messages keep in the history (faint while empty, blush once you type), at least 2 rows that grow with you, then a tinted row and the key bar: keys in the text color, what they do dim, a tip on the right when you're idle. When you talk to an agent directly, the key bar starts with `esc back to main`, so the way home is always in sight. Empty, the composer asks: what's on your mind?

Exact (BISE-102, BISE-103):
- **The raised pane** (palette role `raised`, §5; user request on marketing 685220f, replaces the raised block of e71ec74): every cell from the row under the divider to the row above the frame's bottom border (H−2), columns 1..F−2 (the whole inside of the frame, the margin columns included). No frame (H < 16 or F < 60): full width, from the row under the divider down to H−1. The divider row itself stays on the ground (its label and state too).
- **On the tint, top down** (H ≥ 20; BISE-108 two sections, BISE-111 symmetric padding, user requests): [queued lines] · [the attachments section: its title and one row per image at x0+3 (file names, §14), no bar, then 1 blank tinted row] · the body: the bar `│` at x0 on each of its rows (faint empty, accent with text, an image or recording): 1 blank bar row, the text from x0+3 (the history's text column, wrapped at word boundaries), ≥ 1 row, grows to min(12, 40% of H), then scrolls, 1 blank bar row (always as many above the text as under it) · the key bar (from x0; in an agent's view `esc back to main` first) · frame bottom. No blank ground row under the divider (the blank bar row does that job). Height at rest: divider + 1 + 1 + 1 + key bar + frame = 6 rows. H 16–19: drop both bar rows together and the blank row under the attachments (4). H < 16 or F < 60: no frame, same as 16–19. H < 14: the key bar goes into the divider's right side instead of the state.
- **Placeholder:** dim, `what's on your mind?` (to an agent: `talk to auth-fix directly`).
- **Key bar in an agent's view:** `esc back to main` is always the first pair, from x0, on every key set of that view (idle; working: `esc back to main   ⏎ steer   ctrl+c interrupt`). `esc` in the text color like every key, `back to main` dim. Never dropped for lack of room: pairs drop from the right, `/ commands` first. The right-side tip is hidden in an agent's view. The divider says `you → auth-fix`.

## 14. Images

Built on the technical work of the `screenshots` task
([../images.md](../images.md)): sources are a dragged file (its path is
pasted), `ctrl+v` (clipboard image) and an image picked in the `@` popup.

- **Chip:** an image is one atomic accent chip in the text: `▣ 1` in the
  composer, `▣ login.png` in the history. Deleting the chip drops the image.
  (The label underneath can stay `[Image #1]`.)
- **Strip above the composer** (the attachments section, §13) while
  images are attached, the file name only (never the path; cut at its end
  with `…`; BISE-108; the divider's flash too: `✓ attached ▣ 1 login-mobile.png`): `▣ 1 login-mobile.png · 1170×2532 · 310 kB`, `▣ 2 clipboard · 2048×1536 ·
  1.1 MB → resized to fit 2048`, and `backspace on a chip removes it`.
- **History:** your line keeps the chips; one dim line under it gives each
  image's size. No picture drawn in the terminal for now (**later**: kitty /
  iTerm2 image protocols).
- **Routing:** main says it passes the images on (`layout-fix takes it, with
  both images.`). **⚠** To check: the image marker travels in the brief.
- **Tool results:** `result · ▣ screenshot.png 390×844`.
- **Model without vision:** `✗ glm-5 can't read images. pick a model that can
  (/model), or describe the screen in words.` (**⚠** today the provider's raw
  error shows.)

## 15. Onboarding (first launch)

Validated as is: [site/book/onboarding.html](site/book/onboarding.html). Inspired by
Vibe's (`vibe/setup/onboarding`). Six steps, `enter` to go on:

1. **Welcome**, centered. Typed at ~70 ms per character: `hi, i'm bise`, then
   `:*` pops in accent (scale 0.4 → 1.5 → 1, 0.9 s), then, right under the
   name, the short gloss fades in (faint): `bise /beez/ · french: a kiss on
   the cheek. also a north wind.`, then `ideas in. little kisses out. also
   pull requests.` (dim) is typed, then `press enter ↵` (faint).
2. **Theme.** `your terminal looks dark, so i picked dark.` / `you can change
   it any time with /theme.` When no detection was needed, it says why:
   `BISE_THEME is set to light, so i picked it.` or `you picked light last
   time, so i kept it.` (the saved choice); no answer from the terminal:
   `i couldn't read your terminal's background, so i picked dark.` Two live previews side by side (the same four
   lines of a bise feed); `←→` switches, `enter` keeps.
3. **Model.** `which model should do the work?` A key found in the
   environment first: `1 · use ANTHROPIC_API_KEY  found` / `claude, already
   set up. nothing to paste.`; `2 · paste another key`; `3 · sign in with the
   browser` (**⚠** not built). `↑↓` chooses.
4. **Folder, and one honest thing.** `i'll work in ~/lab/app · a git repo ✓`
   / `all your agents share this folder and know about each other. no
   worktrees to merge.` / `one honest thing: agents run commands here without
   asking you. git is your safety net, so commit often.` When an approval
   mode ships (probably before release), this becomes: `agents ask you before
   risky commands (push, deleting outside this folder). change it in
   /settings.` `o` picks another folder.
5. **How it works, in three lines,** appearing one by one:
   `› you talk to me. i start agents for the work, in the background.`
   `∿ they show up on the right. ⌥ + number to look inside, esc to come back.`
   `? when someone needs you, you get a card. the rest can wait.`
   `enter, and say what's on your mind.`
6. **The real first run, with one-time hints.** No tour. Each hint shows once,
   next to the thing, the first time it happens, and goes away when used:
   - first agent: `new: your agents. they work in the background. ⌥ 1 to look
     inside, esc to come back. →`
   - first run of level 3: `agents talk to each other. it stays dim: you can
     ignore it, or ▸ to read.`
   - first card: `a card: someone needs you. type your answer, alt+r sends
     it. ↓`
   - (**⚠** proposed) first steer: `✓ the agent got it · ✓✓ it read it.`

The onboarding runs once per user (a flag in the state directory); `/welcome`
replays it (**⚠** proposed command).

**Layout.** One content column for all steps: 64 columns (terminal width − 8 when narrower), horizontally centered. Welcome and theme center their lines inside it; model, folder and how-it-works are left-aligned inside it. Vertically, the block sits a bit above the middle: 2/5 of the free rows above it, 3/5 below; the step dots stay 2 rows above the bottom. **Emphasis** (a terminal has one font size, so "size" is weight, color and space): each step's first line is its title, bold, text color; then 2 blank rows; the body in text color, notes dim, 1 blank row between options or lines; then 2 blank rows and the key line. Key lines are read, so they are dim, never faint (§5), with the keys themselves in text color: `enter ok · o another folder`. Options: the selected one `›` accent + name bold, the others indented 2, their sub-line dim and indented 2 more. Welcome: `hi, i'm bise` bold + `:*` accent bold; the gloss dim (not faint: it is read); 2 blank rows; the tagline in text color; 2 blank rows; `press enter ↵` dim with `enter` in text. Where the terminal supports text sizing (kitty ≥ 0.40, OSC 66), `hi, i'm bise :*` is drawn at scale 2; elsewhere bold. Small terminals: height < 22 turns every 2 blank rows into 1; width < 50 makes the column width − 4.

## 16. Keys (final)

| Keys | Action | Change |
|---|---|---|
| `⏎` | send to the agent in view; during a turn, steer | — |
| `tab` | during a turn: queue the message for after it | shown above the composer (BISE-89) |
| `↑` in an empty composer | edit the newest queued message (then the history) | new (BISE-89) |
| `@name …` | direct message from main | — |
| `ctrl+c` | interrupt; again (or idle) quit, agents keep running | — |
| `⌥ + 0…9` | go to main / agent N | now shown in the panel |
| `ctrl+k` / `ctrl+j`, `alt+↓` / `alt+↑` | select next / previous agent | — |
| `⏎` on a selected agent | enter it | — |
| `space` | preview the selected agent; in the feed, toggle the selected item | feed toggle new |
| `D` | drop the selected agent (asks first) | — |
| `esc` | close selection; in an agent, back to main | — |
| `ctrl+g`, `ctrl+n` / `ctrl+p`, `alt+r`, `ctrl+f`, `ctrl+x`, `y` / `n` | cards | hints on the card |
| `ctrl+o` | open or close everything folded (thinking, outputs, diffs, reports, runs, `▸ why`) | was `ctrl+t` (removed, no alias); the `ctrl+o` shell is gone: the terminal panel is the one shell |
| `ctrl+r` | voice | — |
| ``ctrl+` `` | terminal panel | — |
| `ctrl+v` | paste an image | from the images work |
| `ctrl+z` | ~~cancel the last route~~ | **removed** |

## 17. Copy deck

Every string the UI shows, lowercase. Issues must use these exact strings.

| Where | Text |
|---|---|
| header, no agents | `no agents yet` |
| header counts | `∿ {n} working · … {n} waiting · ? {n} needs you · ✓ {n} done` |
| panel title | `agents · ⌥ + number` |
| panel, more rows | `+ {n} more` |
| panel, archived | `▸ {n} archived` |
| first run | `what's on your mind?` / `say it and keep talking. the work runs in the background, i'm always here.` / `try: "fix the flaky login test, and draft the release note"` |
| inside an agent | `you're talking to {name} directly. main isn't in the loop. esc back to main.` |
| composer hints, main | `⏎ send · @ agent · / commands` |
| composer hints, during a turn | `tab queue · ⏎ steer · ctrl+c interrupt` |
| composer placeholder (BISE-98) | `what's on your mind?` (to main) · `talk to {name} directly` (inside an agent) · `{name} is archived: read-only` |
| divider (BISE-98) | `you → {name}` · on the right the old status row: `idle · 18k / 1M tokens · 2%`, or `↓ back to the bottom · end · {n} new lines` while scrolled up |
| run of level 3 | `▸ {n} messages between {k} agents` |
| thinking | `∴ thought for {s}s` |
| output | `▸ output · {n} lines` (+ ` · {k} failed` when known) |
| edit | `± edit {path} ✓ +{a} −{d}` |
| turn done (inside an agent) | `✓ turn done · {duration}` |
| card title | `? {name} needs you` |
| card keys | `alt+r answer with text · ctrl+x later · ctrl+f full screen` |
| direct reply | `@ {name} to you: {text}` |
| not delivered | `✗ not delivered: {name} stopped. ⏎ send again · esc drop` |
| no vision | `✗ {model} can't read images. pick a model that can (/model), or describe the screen in words.` |
| provider down | `✗ the model provider answered {code}. retrying in {s}s ({i} of {n}).` / `{names} are waiting on it; nothing is lost.` |
| hub lost | `○ hub disconnected · reconnecting…` |
| images strip | `attached · backspace on a chip removes it` |
| no undo (ctrl+z, or /cancel typed) | `no undo: an agent may already have acted. say the change to main instead ("no, v1 for docs").` |

| drop asks first (D) | `drop {name}? its history stays in archived. y / n` · hint `y drop · n or esc keep` |
| yes/no confirm | `answer y (yes) or n (no), then ⏎` · hint `y yes · n no · esc cancel` |
| archived agent in view | `@{name} is archived: its history is read-only · /restore brings it back · esc → main` |
| /theme | `theme: {mode}.` / `theme: {mode}. /theme auto, light or dark to change it.` / `theme: {mode}, for now: i couldn't save it ({err}).` / `/theme takes auto, light or dark.` |
| /clear, ctrl+l | `display cleared — scroll up to see the earlier lines again` |
| interrupt | `… · ctrl+c again to quit` |
| terminal panel | title ``terminal · ctrl+` hide`` · hint ``terminal: keys go to the shell · ctrl+` hide · wheel/shift+pgup scroll · drag the border to resize`` |
| help footer | `type to filter · tab switch · esc close` |
| steer with nothing | `nothing to steer with: type the text after steer` |
| queued messages | ` › {text}…` (one per line, dim) · hint `queued · sent when this turn ends · ↑ edit` · panel row `· {n} queued` |
| connect failed | `couldn't connect: {err}` |
| voice | `voice mode on. press ctrl+r to start recording.` / `voice mode off.` / `voice mode is off: /voice turns it on` / `no speech detected` / `voice transcription failed: {err}` / `voice transcription needs an API key: set {VAR}` / `no audio input device found.` / `audio backend is unavailable: {err}` / `the last words may be missing (the transcription did not finish in time).` / `no audio detected from the microphone — check your terminal has mic access.` (+ ` grant access in System Settings → Privacy & Security → Microphone.`) |
| command descriptions | as in `/help` (lowercase, "agent"); `/agents`: `list the agents and what they do` |

Not built yet (a feature, not wording): `✓ turn done · {duration}` and the two `provider down` lines.

Onboarding strings: §15.

---

# Part III — Implementation plan

## 18. How main agents use this plan

1. Read Part I and II once, then [bise-issues.md](bise-issues.md).
2. Work **wave by wave**. Inside a wave, issues on different **tracks** can
   run in parallel: their files don't overlap (§20). Issues on the same track
   run one after the other, by one implementer.
3. Spawn **one implementer per track**, not per issue. Give it the list of its
   issues for the wave, this book and the tracker. It works them in order.
4. Before starting a track, check `sb list`: a task already working in the
   same files must land first. Don't interrupt it. (On 2026-09-28 the two
   known ones have landed: `archived-sidebar` in 85160ab, `screenshots` in
   4282501.)
5. The implementer updates **only its issue sections** in the tracker:
   `status`, `owner`, `commits`, `notes` (what was done, what was learned,
   what the next issue should know).
6. An issue is done when its "done when" list is true, the gates pass (§19),
   and its tracker section says `done` with the commits.
7. When an issue changes a contract (§21), stop and tell main: other tracks
   depend on it.

## 19. Rules for every implementer

- **Shared folder.** Never `git stash`, `git clean`, `git reset --hard`,
  `git checkout -- <paths>`, `git restore` on paths that aren't yours. Commit
  only your paths (`git add <paths>`, check `git diff --cached`). No push.
- **Stay in your files** (the "owns" list of the issue). If you need a change
  elsewhere, write it in your issue's notes and tell main; don't edit.
- **Gates** before `done`: `cargo build`, `cargo test --workspace`,
  `cargo clippy` without new warnings, `projects/switchboard/tests/run_all.sh`;
  `bend PROOF.bend` if a Bend file changed.
- **Visual check:** run the TUI in Ghostty (dark) and once in a light
  terminal; compare with the mockup named in the issue. Put what differs in
  the notes.
- **Strings** come from the copy deck (§17). A new string: add it to your
  notes, main updates the book.

## 20. Tracks and file ownership

| Track | Owns | Issues |
|---|---|---|
| **T · theme** | `rust/tui/src/theme.rs`; then `term.rs` + new `theme_detect.rs` | BISE-01, BISE-02 |
| **G · glyph audit** | no code; writes `docs/brand/glyph-audit.md` | BISE-03 |
| **H · hub protocol** | `rust/switchboard/src/core.rs`, `daemon.rs`, `transcript.rs`, `core_tests.rs`; in the TUI `wire.rs` (the `Ev` enum) and `sb.rs::parse_hub_line` | BISE-04 |
| **F · feed** | `render.rs`, `feed.rs`, `code.rs`, `markdown.rs`, `feedsel.rs`, `feed_render_tests.rs`; later `wire.rs` (steering lines) | BISE-10 … BISE-15 |
| **P · chrome** | `sb/panel.rs`, `ui.rs` | BISE-20, BISE-21, BISE-22 |
| **C · cards** | `sb/cards.rs` | BISE-30, BISE-31 |
| **K · keys & help** | `help.rs`, `commands.rs`, `input.rs`, the key arms of `sb.rs` | BISE-40, BISE-41, BISE-42 |
| **M · main's behavior** | `rust/switchboard/src/prompts.rs`, `router.rs` | BISE-50, BISE-51 |
| **O · onboarding** | new `onboarding.rs`, `hints.rs`; the entry in `run.rs`; hint hooks in `sb.rs` (event handling only) | BISE-60, BISE-61 |
| **I · images UI** | `attach.rs`, the composer chip drawing (`editor.rs` render path), the strip in `ui.rs` | BISE-70 |
| **S · sweeps** | any file, one sweep at a time, when no track is active in it | BISE-80 … BISE-83 |

Files touched by two tracks are never touched in the same wave: `wire.rs`
(H in wave 0, F in wave 2), `sb.rs` (H wave 0, K wave 1 key arm, O wave 2
hooks), `ui.rs` (P wave 1, I wave 2).

## 21. Contracts (frozen in wave 0)

**C1 · theme tokens** (BISE-01). `theme.rs` exposes roles, not colors:
`text()`, `dim()`, `faint()`, `accent()`, `error()`, `ok()`, `selection_bg()`,
`card_tint()`, `bg()` (the painted ground, BISE-92; every cell left at
`Color::Reset` gets it through the frame pass `theme::paint`), syntax roles,
and `set_mode(Mode::Light | Mode::Dark)`. The old
constants (`BRAND`, `ACCENT`, `INFO`, `WARN`, `HEAD`, `PANEL`, …) stay as
deprecated aliases until BISE-83, so no track breaks. Glyph constants for §6
(`G_YOU`, `G_MAIN`, `G_WORKING`, …) live there too.
Amendment (BISE-84, accepted by main): `G_*` stay `&'static str` constants
(Unicode, with the §6 fallbacks applied); `theme::glyph(G_X)` returns the
ASCII form when `BISE_ASCII=1` (`theme::ascii_mode()`); as a safety net,
`theme::asciify(buf)` rewrites the drawn buffer's cells that hold a table
glyph (only those) after each draw, only in ASCII mode. ASCII forms are one
cell wide (`+` done, `:*` stays). Hard-coded glyph literals migrate to
`glyph()` in BISE-83.

**C2 · hub line protocol v2** (BISE-04). Today the hub writes synthetic lines
`sb <kind> : <text>` into an agent's feed (`you`, `msg-in`, `card`,
`card-closed`, `route`, `spawn`, `direct`, `warn`). v2 keeps the same shape
and adds structured kinds; the TUI maps each to one level:

| kind | text | level | Ev |
|---|---|---|---|
| `msg-in` | `{from} {m_id} : {text}` (what the feed owner receives; v1) | 3 | `Ev::AgentMsg { from, to: "", text, level: 3, id: "m_3" }` |
| `msg` | `{from} → {to} : {text}` | 3 | `Ev::AgentMsg { from, to, text, level: 3, id: "" }` |
| `msg-you` | `{from} : {text}` (an agent writing to the user) | 2 | `Ev::AgentMsg { from, to: "you", text, level: 2, id: "" }` |
| `answered` | `{agent} : {question} : {answer} : {why}` (main answered for you) | 2 | `Ev::Answered { … }` |
| `route` / `spawn` | as today | 2 | as today |
| `card` | as today | 1 | as today |

- The hub also feeds `msg` lines for **messages between two other agents**
  into main's feed (today main only sees messages to main).
- Old kinds keep working (a v1 transcript still renders).
- Amendment (BISE-04, accepted by main): `Ev::AgentMsg` carries `id` (the message id, `m_3` for `msg-in`, empty otherwise). A ` : ` inside a field of `answered` is escaped as ` \: `. `sb send --why <text>` fills the `why` of `answered`.
- C2 amendment: history timestamp (BISE-85, accepted by main). A `history`
  page line is `{pos, line, ts?}`: `ts` is when the hub's transcript wrote
  the line (ms since the epoch), optional; a line without it still parses.
  The TUI reads it into `wire::HistLine { pos, line, ts: Option<u64> }`
  (`wire::parse_history`) and puts `Ev::TimeMark("hh:mm")` (local time)
  before a replayed line that comes 5 minutes or more after the one before
  it, as for live lines (§10).
- C2 amendment: `undelivered` (BISE-86, accepted by main). When a message
  from the user cannot reach its agent (stopped, dropped, archived: the
  send fails with `recipient_unavailable`, or it was still queued when the
  agent stopped), the hub writes `sb undelivered : {name} : {text}` in the
  feed where the user wrote it (the agent's own, or the `via` view; fields
  escaped like `answered`). The `recipient_unavailable` notice stays. TUI:
  `Ev::Undelivered { name, text, open }`; your matching line (the text, or
  `@name text`) gets `Mark::Failed` (`✗`, error color), or comes back
  marked when the feed does not have it; the line reads
  `✗ not delivered: {name} stopped. ⏎ send again · esc drop` (§13, §17)
  while `open`: on an empty composer ⏎ sends it again, esc drops it.
- **⚠** Volume: with 30 agents this is many lines; the TUI folds them (§10),
  the hub must not drop them.

**C3 · steering marks** (BISE-15). The REPL's wire lines
`steering_received: <text>` and `steered: <text>` stop being `Ev::Info`; they
set a mark on the last `Ev::You` with the same text: `Mark::Received`,
`Mark::Read`.

**C4 · one-time hints** (BISE-61). A small store in the state directory
(`hints.json`: `{ "first_agent": true, … }`) and one call
`hints::once(app, Hint::FirstAgent)`.

## 22. Waves

```
wave 0  foundations      BISE-01 theme tokens ─┐   BISE-03 glyph audit   BISE-04 hub protocol v2
                                                │                              │
wave 1  parallel tracks  T: BISE-02 detection  F: BISE-10 → 11 → 12 → 13    P: BISE-20 → 21 → 22
                         C: BISE-30            K: BISE-40                    M: BISE-50 → 51
wave 2  builds on 1      F: BISE-14 (needs 04, 13) → BISE-15 (needs 04)
                         O: BISE-60 → 61 (61 needs 14, 20, 30)
                         I: BISE-70 (needs 22)
                         K: BISE-41 → 42 (needs 12 for the new keys)
                         C: BISE-31 (needs the open question answered)
wave 3  sweeps           BISE-80 vocabulary · BISE-81 lowercase + copy deck
                         BISE-82 visual QA · BISE-83 remove deprecated theme aliases
```

A good first day: wave 0 with three implementers (T, G, H), then wave 1 with
up to six (T, F, P, C, K, M).

## 23. Later (not scheduled)

- Draw images in the terminal (kitty / iTerm2 protocols).
- An approval mode for risky commands (then onboarding step 4 changes, §15).
- Cost in $ in the header.
- `✓` / `✓✓` on level-3 notifications (`notification_received` /
  `notification_delivered`).
- Renaming the binary and commands to `bise` (a product decision for main).
- A "filter this agent" view that keeps arrival order.
