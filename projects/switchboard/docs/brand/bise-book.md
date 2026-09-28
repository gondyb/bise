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
| [tui-screens.html](tui-screens.html) | every screen of the product, 32 mockups + the levels and symbol legends |
| [tui-live.html](tui-live.html) | an 80-second live simulation; you can type and send messages |
| [tui-onboarding.html](tui-onboarding.html) | the first launch, 6 steps (validated as is) |
| [tui-mockup.html](tui-mockup.html) | the first single-screen mockup (superseded by tui-screens) |
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
| site, subline | *built for engineers who think faster than they type.* | picked |
| inside the product (onboarding) | **your ideas. my hands. lots of them.** | picked |
| in reserve | you, but with way more hands. · your team is as big as your ideas. | former headlines |

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

**Background.** In the terminal we never paint the background: the user's
terminal background shows through (their exact color, transparency, blur).
We only set foreground colors, plus at most a light tint for the selection
and the card box.

**Two themes, light and dark,** chosen automatically from the terminal's
background (OSC 11 query), with a setting to force one. Every readable text
is ≥ 4.5:1 (WCAG AA) on white, our cream, black and a typical dark grey
`#282c34`.

| Role | Dark | Light | Use |
|---|---|---|---|
| text | `#ece6da` (15:1) | `#1b1917` (17.5:1) | everything you read |
| dim | `#a39c90` (6.9:1) | `#6b645a` (5.8:1) | secondary text, level 3, durations |
| faint | `#4a4540` | `#cfc8bd` | rails, borders, numbers; **never** for text you must read |
| accent | `#f4a6b0` pale pink (9.7:1) | `#b8416b` raspberry (5.2:1) | the `:*`, "needs you", the agent you talk to, `✓✓` read |
| error | `#ff5a52` | `#b3261e` | failures only |
| ok | `#b9d99a` | `#3f7a2a` | diff additions only |

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
| `›` | you (and the composer prompt) |
| `:*` (accent) | main, the agent you talk to by default |
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
| `∿` (pulsing) | working: a breeze |
| `…` | waiting on another agent |
| `?` (accent) | needs you (question or blocked) |
| `♡` | done |
| `✗` (error) | failed |
| `○` (dim) | idle |
| `–` (dim) | stopped |

**Marks**

| Glyph | Meaning |
|---|---|
| `·` → `✓` → `✓✓` | your message: sending → the agent got it → the model read it (`✓✓` in accent) |
| `•` (accent) | unread activity in an agent |
| worktree mark (BISE-84 picks it: width 1, in ≥ 4 of the audited fonts, not `⌥`) | the agent has its own worktree (no mark: the shared folder); was `⎇`, in almost no font |
| `⇄` | overlap: two agents changed the same file |
| `↻` (error) | a restart failed |
| `Δ` | a version is building or on trial (was `⧗`: in no font) |
| `▸` / `▾` | closed / open (progressive disclosure) |

**Decided by Gabriel (2026-09-28), after the glyph audit (BISE-03,
[glyph-audit.md](glyph-audit.md)):** keep the brand glyphs (`∿`, `♡`, `:*`) and
every glyph a fallback font draws at width 1; replace only the ones that break:
`✉` → `@` and `↪` → `»` (color-emoji risk), `⟳` → `≡` pulsing, `⧗` → `Δ`,
`⎇` → a glyph picked in BISE-84. `BISE_ASCII=1` switches every glyph to plain
ASCII (`~` working, `<3` done, `>` you, …) for terminals that draw them badly
(BISE-84). Terminals must use ambiguous width = narrow (the default).

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
7. **Honest.** No undo that can't undo, no silent failure, no hidden limit.

## 8. Layout

```
 bise :*                              ∿ 3 working · ? 1 needs you · ♡ 1 done    ← header
                                                    │ agents · ⌥ + number
  › the login breaks on safari                      │ 0 :* main
  :* on it: auth-fix takes it.                      │ 1 ∿ auth-fix     12m · 21%
    │ ✉ docs      → main      v1 or v2?             │ 2 ∿ release       3m · 8%
    │ ✉ main      → docs      v2, the brief says so │ 3 ? docs              you
  :* docs asked v1 or v2; the brief says v2,        │ 4 … api-v2     waits docs
     so i answered. ▸ why                           │ 5 ♡ bench            done
  ┃ ? docs needs you                                │
  ┃ the brief says "keep old clients working"…      │
 ┌ card box (when a card is open) ──────────────────┐
 main · idle · 210k / 1M tokens · 21%                                             ← status row
 › _                                          ⏎ send · @ agent · / commands       ← composer
```

- **Header:** `bise :*` on the left; live counts on the right, only the
  non-zero ones: `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done`.
  **⚠** No cost in $ until the usage work lands.
- **Feed** on the left, prose ≤ 76 columns; extra width goes to the margin
  and the panel, never to longer lines.
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

## 9. Three levels: what's for you, what isn't

The same three levels everywhere, in main and inside an agent.

| Level | What | Look |
|---|---|---|
| **1 · needs you** | a question or a blocker addressed to you | accent bar `┃` on the left, bold accent title `? docs needs you`, normal body; stays until answered; also in the card box |
| **2 · for you** | what main or an agent says to you: replies, summaries, reports on your requests, main answering on your behalf | normal text, with `:*`, a status glyph (`♡` `✗`) or `✉ name to you:` in front |
| **3 · between agents** | messages agents send each other and to main | dim text under a faint rail: `✉ from → to  text`, names padded to 10 columns, one line each, `▸` when long |

- Traffic between agents is **always in the history** (including between two
  agents that aren't main), so what happened stays understandable. Main's
  level-2 line after a burst is the summary for you.
- Levels 2 and 3 differ by brightness and the rail, never by hue.
- Main deciding for you is level 2 and says why on demand:
  `:* docs asked v1 or v2; the brief says v2, so i answered. ▸ why`.
- No "quiet" mode: the levels and the folding (§10) already keep it calm.

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

- **Measure.** Prose wraps at `min(width − margins, 76)`. Code (scripts,
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
| tool output / result | `▸ output · 42 lines · 1 failed` | the full output |
| sub-calls | `↳ github.search_issues ✓` | — |
| file edit | `± edit web/src/auth/session.ts ✓ +3 −1 ▸` | the diff |
| report in main | `♡ bench is done. p95 at 180 ms ▸ report` | the report |
| brief (inside an agent) | `◇ brief ▸` | the brief |
| a run of level 3 | `▸ 12 messages between 8 agents` | the messages, in order |
| a card | open while it needs you | see §12 |

  Keys: click or `space` on the selected item toggles it; `ctrl+t` opens or
  closes all thinking (today); **⚠** one new key opens or closes all outputs
  (to pick, not `ctrl+b`: tmux).
- **Failures** stay one line in error color with the reason; `▸` for the
  full error: `$ bash ✗ exit 1 · 0.8s` then `error[E0425]: … ▸ 18 lines`.

## 12. Cards

- A card is level 1: accent bar, `? <agent> needs you`, the question, then the
  choices when the agent gives some (`1 v1  2 v2`), and the keys, dim:
  `alt+r answer with text · ctrl+x later · ctrl+f full screen`.
- **Kinds** reuse the glyphs: `?` question and blocked (accent), `✗` failed,
  `↻` restart failed (error), `–` drop confirmation, `⇄` overlap, `♡` done.
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
  `✉ auth-fix to you: got it, i'll check logout after the login fix.`
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
- **No undo.** Agents may already have acted, so an undo promises too much.
  To change something, you say it ("no, v1 for docs"). Main sends the agent an
  explicit correction (`the user changed their mind: use v1, not v2.`) and
  confirms in one line (`:* told docs: v1, you changed your mind.`). Today's
  `ctrl+z` / `/cancel` go away.
- `ctrl+c` interrupts the turn of the agent in view; again (or at idle)
  quits; the agents keep running.

## 14. Images

Built on the technical work of the `screenshots` task
([../images.md](../images.md)): sources are a dragged file (its path is
pasted), `ctrl+v` (clipboard image) and an image picked in the `@` popup.

- **Chip:** an image is one atomic accent chip in the text: `▣ 1` in the
  composer, `▣ login.png` in the history. Deleting the chip drops the image.
  (The label underneath can stay `[Image #1]`.)
- **Strip above the composer** while images are attached: `▣ 1
  shots/login-mobile.png · 1170×2532 · 310 kB`, `▣ 2 clipboard · 2048×1536 ·
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

Validated as is: [tui-onboarding.html](tui-onboarding.html). Inspired by
Vibe's (`vibe/setup/onboarding`). Six steps, `enter` to go on:

1. **Welcome**, centered. Typed at ~70 ms per character: `hi, i'm bise`, then
   `:*` pops in accent (scale 0.4 → 1.5 → 1, 0.9 s), then `your ideas. my
   hands. lots of them.` (dim), then `press enter ↵` (faint).
2. **Theme.** `your terminal looks dark, so i picked dark.` / `you can change
   it any time with /theme.` Two live previews side by side (the same four
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

## 16. Keys (final)

| Keys | Action | Change |
|---|---|---|
| `⏎` | send to the agent in view; during a turn, steer | — |
| `@name …` | direct message from main | — |
| `ctrl+c` | interrupt; again (or idle) quit, agents keep running | — |
| `⌥ + 0…9` | go to main / agent N | now shown in the panel |
| `ctrl+k` / `ctrl+j`, `alt+↓` / `alt+↑` | select next / previous agent | — |
| `⏎` on a selected agent | enter it | — |
| `space` | preview the selected agent; in the feed, toggle the selected item | feed toggle new |
| `D` | drop the selected agent (asks first) | — |
| `esc` | close selection; in an agent, back to main | — |
| `ctrl+g`, `ctrl+n` / `ctrl+p`, `alt+r`, `ctrl+f`, `ctrl+x`, `y` / `n` | cards | hints on the card |
| `ctrl+t` | all thinking | — |
| **⚠** new key | all outputs | to pick |
| `ctrl+r` | voice | — |
| ``ctrl+` `` | terminal panel | — |
| `ctrl+v` | paste an image | from the images work |
| `ctrl+z` | ~~cancel the last route~~ | **removed** |

## 17. Copy deck

Every string the UI shows, lowercase. Issues must use these exact strings.

| Where | Text |
|---|---|
| header, no agents | `no agents yet` |
| header counts | `∿ {n} working · … {n} waiting · ? {n} needs you · ♡ {n} done` |
| panel title | `agents · ⌥ + number` |
| panel, more rows | `+ {n} more` |
| panel, archived | `▸ {n} archived` |
| first run | `what's on your mind?` / `say it and keep talking. the work runs in the background, i'm always here.` / `try: "fix the flaky login test, and draft the release note"` |
| inside an agent | `you're talking to {name} directly. main isn't in the loop. esc back to main.` |
| composer hints, main | `⏎ send · @ agent · / commands` |
| composer hints, during a turn | `⏎ steer · ctrl+c interrupt` |
| run of level 3 | `▸ {n} messages between {k} agents` |
| thinking | `∴ thought for {s}s` |
| output | `▸ output · {n} lines` (+ ` · {k} failed` when known) |
| edit | `± edit {path} ✓ +{a} −{d}` |
| turn done (inside an agent) | `♡ turn done · {duration}` |
| card title | `? {name} needs you` |
| card keys | `alt+r answer with text · ctrl+x later · ctrl+f full screen` |
| direct reply | `✉ {name} to you: {text}` |
| not delivered | `✗ not delivered: {name} stopped. ⏎ send again · esc drop` |
| no vision | `✗ {model} can't read images. pick a model that can (/model), or describe the screen in words.` |
| provider down | `✗ the model provider answered {code}. retrying in {s}s ({i} of {n}).` / `{names} are waiting on it; nothing is lost.` |
| hub lost | `○ hub disconnected · reconnecting…` |
| images strip | `attached · backspace on a chip removes it` |
| no undo (ctrl+z, or /cancel typed) | `no undo: an agent may already have acted. say the change to main instead ("no, v1 for docs").` |

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
`card_tint()`, syntax roles, and `set_mode(Mode::Light | Mode::Dark)`. The old
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
