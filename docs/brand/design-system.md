# the bise design system

How bise looks and behaves, and why. These are the rules behind the choices we
already made, written down so that a new screen feels like the old ones
without anyone having to ask.

The brand book (`bise-book.md`) stays the source for the exact specs: every
color value, every row, every key, with its issue number. This page is the
layer above it: the patterns, the reasons, and the checklist. When the two
disagree, the book wins and this page gets fixed.

Each rule has a short reason and a real example from the product.

---

## 1. the five laws

Every other rule on this page comes from one of these.

1. **never block.** you can always type, send and move. nothing takes the
   keyboard away from you, nothing opens while you type, nothing waits for a
   click to let you go on.
   *e.g.* the composer is never locked during a turn: `⏎` steers, `tab`
   queues. an inbox item never opens by itself while your draft has text.
2. **calm by default, loud only for you.** at rest the screen says almost
   nothing. color, motion and position go to what needs you, and nothing
   else.
   *e.g.* a done agent is a small pink `✓`. an agent stuck on a question shows
   a pink `?` and an item in your inbox.
3. **one key for the common case.** the answer you give 9 times out of 10
   is one key away, from anywhere.
   *e.g.* `ctrl+1` opens the first inbox item, `1` allows the command.
4. **show what happened.** every action leaves a trace you can see for a
   moment, then gets out of the way.
   *e.g.* after an answer, a fold line `✓ you answered sad-404: a hat` for
   2 s, then the next item opens.
5. **never lie.** no hidden limit, no fake progress, no key that doesn't
   work, no promise the product can't keep.
   *e.g.* without ctrl+digits from the terminal, the inbox says
   `click to open`. it never shows `ctrl+1`.

---

## 2. foundations

### 2.1 color is attention

The palette is in book §5. What matters here is what each color is for.

| role | what it may carry | never |
|---|---|---|
| text `#ece6da` | what you read: answers, names, your message | — |
| dim `#a39c90` | secondary text: the model, the agent's own work, durations | for something you must act on |
| faint `#857d72` | the quietest text: key labels, numbers, separators ` · ` | for a sentence you must read |
| rule `#4a4540` | lines: the frame, the panel's rule, borders | for text, ever |
| accent `#f4a6b0` | needs you, main's `:*`, the agent you talk to, done `✓`, read `✓✓`, your bar | for decoration |
| error `#ff5a52` | failures only: `✗`, failing checks, a broken key | for a warning or a default (yolo is dim) |
| ok `#b9d99a` | diff additions only | for success: success is the accent `✓` |
| raised `#1f1c1a` | your space: the composer pane, an open inbox item | for anything the agents own |

- **pink is never red.** "needs you" must not look like an error.
- **never green for done.** we tried it: it looks like a CI dashboard, and it
  isn't on brand. done is a small pink check.
- **one hue per screen region.** if two things on one row want color, one of
  them is wrong.
- every pair has a light theme, a 16-color form and a `NO_COLOR` form (bold
  replaces the accent). a design without them isn't finished.

### 2.2 type

- **the user's font.** we pick no font in the terminal. the site and the mocks
  use JetBrains Mono.
- **one size.** a terminal has one size, so importance comes from contrast,
  bold and room, never from size. level 2 gets a blank row above and below:
  that is its "bigger".
- **bold is rare.** speakers (`:*`, `@ name to you:`), titles of a screen, a
  key you must notice. never a whole paragraph.
- **lowercase everywhere**, except proper nouns (GitHub, Mistral), acronyms
  (PR, MCP) and keys as printed (`ctrl+s`).

### 2.3 space, in cells

- everything sits on whole cells. no half rows, no sub-cell gaps.
- **the reading column**: prose wraps at 88 columns, whatever the terminal's
  width. more width goes to the margin and the panel, never to longer lines.
- **one blank row** between history blocks. level 2 gets one above and below.
  two blank rows in a row is a bug.
- **text starts at fixed columns.** the frame's text at column 3, the
  composer's at column 7, the panel's rows at their own column. a new element
  lines up with one of them, never at a new column of its own.
- **numbers are right-aligned** in their column (`3m`, `42s`, `12%`). names
  are left-aligned and take the room that's left.

### 2.4 motion

- motion means **something is alive**: the working gust `∿`, the cursor, the
  `:*` pop, a 2-3 s flash. nothing bounces, nothing slides.
- **one animation per thing**, never two on one row.
- motion stops when the terminal loses focus, when nothing works, and under
  a reduce-motion setting (a static `∿` then).
- ≤ 10 frames a second, only the cells that change are redrawn.

### 2.5 glyphs

The full table is book §6. The rules behind it:

- **one glyph per thing, one thing per glyph.** `$` is bash, `ƒ` is a
  TypeScript call, `ψ` a worktree, `↑` a pull request. two meanings for one
  glyph is a bug (open: `@` is both a file key and the inbox count today, see
  §9).
- **one cell wide, in every common font.** each glyph went through the font
  audit (`glyph-audit.md`). a glyph that becomes a color emoji somewhere is
  out (`✉` → `@` in the panel).
- **the place tells two marks apart.** `✓` at the start of a row is done;
  `✓` `✓✓` at the end of your message is got it / read.
- **every glyph has an ASCII form** (`BISE_ASCII=1`), distinct from all the
  others, and a row in the `/help` legend.
- **a glyph needs no legend after a week.** if users ask what it means twice,
  change it (`⎇` → `ψ`, then the legend).

---

## 3. patterns

### 3.1 less at rest, more when you hold a key

The screen at rest shows the minimum needed to understand what's going on.
Holding a modifier shows the rest, in place, and letting go puts it back.

- **hold `ctrl`**: every ctrl key appears where it acts (`▸ ctrl+o expand`,
  the inbox numbers in accent), the state words appear (`working · 1m`,
  `asks you`), the long forms appear (`58k / 1M tokens · 22%`), and the key bar
  shows every ctrl key.
- **hold `⌥`**: the panel's numbers read `⌥0` `⌥1`.
- **hold `cmd`**: only once a cmd key has really reached bise.
- the held form **replaces** the rest form in the same cells. nothing moves,
  nothing pushes the layout.

> *rest:* `2 ∿ dark-mode     3m  12%  ψ`
> *held:* `2 ∿ dark-mode     working  ψ`

Why: the user said "on affiche trop d'informations". a key you hold is free
(it types nothing), so it's the cheapest way to see more.

### 3.2 progressive disclosure: one line, then everything

- every item in the history is **one line by default**: a tool call is
  `$ installs the deps ✓`, thinking is `∴ thought for 14s ▸`, a long message
  is its first 20 rows then `▸ 12 more lines`.
- `▸` means "there is more here". a click or `space` opens one, `ctrl+o` opens
  all of them, `▾` closes.
- **folded is never gone.** what's folded is all there, and search finds it.
- what you need to act on is **never** folded: an error shows its first line,
  a question shows in full.

### 3.3 three levels of attention

The same three levels everywhere (book §9).

| level | what | look |
|---|---|---|
| 1 · needs you | a question, an approval | the inbox, the heavy `┃` in accent, until you answer |
| 2 · for you | main talking to you, reports on your requests | text color, the speaker in bold, room around it |
| 3 · between agents | what agents say to each other | dim, a small chip, folded after a few |

A new kind of event is placed on one of these three levels before it gets a
look. If it doesn't fit, it probably shouldn't reach the screen.

### 3.4 the inbox: decisions, not news

- only what **needs you** goes in the inbox: a question main can't answer, an
  approval, a PR ready to merge. reports, done notes and FYI go to main's
  history.
- **it never blocks.** the inbox sits above the composer, in its own box. you
  keep typing to main while items wait.
- **its numbers are the same everywhere.** the row that says `1` in the box is
  `1` in the panel, and `ctrl+1` opens it.
- **open in place.** an item opens where its row was, on the raised tint, with
  what the agent did last. your draft is set aside, and it comes back when
  you're done.
- **answer with one key.** `1`-`9` picks, `⏎` takes the highlighted option,
  `esc` goes back to your draft. free text is always possible.
- **after the answer**, a fold line for 2 s, then the next item opens by
  itself. after the last one, `✓ inbox clear` on the divider.
- hard rules have **no "always"** option (push to main, secrets, a wipe).

### 3.5 keys

- **every key that's shown works.** we check what the terminal sends before
  showing a key. no ctrl+digits: `click to open`, or `/inbox`.
- **every mouse action has a key, every key-only action shows in help.**
  a click on a row = its number's key.
- **one key, one meaning, everywhere.** `esc` is always "back one step".
  `⏎` is always "do it". `ctrl+o` always opens what's folded.
- **digits pick.** in a list, a menu, an item: `1`-`9` takes that row at
  once.
- **the key bar shows 3-4 keys**, the ones you can't guess. the obvious ones
  (`⏎ send`, `? help`) go. holding ctrl shows the rest.
- **a shortcut leaves the system alone.** we don't take a key the OS or the
  terminal already uses (`ctrl+←→` on macOS); when we must (cmd+f), the
  book says how to free it per terminal.

### 3.6 feedback: show it, then fold it

| what happened | where it shows | for how long |
|---|---|---|
| you switched a mode (shift+tab) | the mode word in accent on the divider, its explanation on the key bar | 3 s |
| you answered an item | a fold line in the box | 2 s, then the next item |
| a setting changed (`/models`) | the row flashes `✓` | 1 flash |
| something you can copy | `✓ copied 9 chars` on the divider | 2 s |
| your message | `·` sending → `✓` got it → `✓✓` read | stays |
| a first-time explanation | a tip over what it explains | once, ever |

- a flash lives **where your eyes already are**: the divider, the row you
  touched. never a pop-up in a corner.
- a flash is a sentence of state (`✓ inbox clear`), never a cheer.

### 3.7 short on room: a written drop order

Every line that can run out of room has its order written in the book, and
we apply it one step at a time:

1. the explanation words go first (`working · `, `in the inbox`),
2. then long forms become short forms (`opus 5.5 · high` → `opus·hi`,
   `∿ 3 working` → `∿ 3`),
3. then whole details go (the branch's name, the model tag),
4. **names are cut last**, with `…`, never in the middle of a glyph or an
   arrow,
5. the things that say "it's alive" or "needs you" are never dropped (the
   gust while working, `?`).

Every new line comes with its drop order, tested at 80 and 150 columns.

### 3.8 pickers and menus

All full-screen choices look the same (`/models`, `/provider`, the first run):

- a bold title as a question (`which provider?`), one dim line saying what it
  changes, a filter line `› type to filter`.
- **what works comes first**: ready providers before the others, `same as
  main` / `auto` first, with **what they resolve to** written next to them
  (`auto · Mistral · mistral-small-latest`).
- **the current one** says `now` (dim). **one recommended** option at most, in
  accent.
- `enter` goes forward, `esc` goes back one step, digits pick at once.
- the step you can skip is skipped (one model: no model step).
- after the last step you land back where you started, the row flashing `✓`.

### 3.9 errors

One line, in this order: **what happened, then how to fix it**, with the
command that fixes it.

> `✗ turn stopped: no OpenRouter key yet. /provider sets it up.`
> `✗ i can't hear you. allow the microphone for your terminal: System Settings › Privacy & Security › Microphone.`

- `✗` in error color for what broke. `?` in accent when it needs your choice
  (no credit: you decide).
- the provider's own words go dim under it, not instead of it.
- **nothing is lost** on a failure: the clip, the draft, the queue are kept,
  and the line says so (`your recording is kept: ctrl+r retry`).

### 3.10 zen: the screen gets out of your way

While you type, everything that isn't your text or the history you read fades
45 % toward the background: the panel, the counts, the key bar, the frame.
It comes back 5 s after your last key, or at once on `⏎`, `esc`, a shortcut,
the mouse, or anything that needs you. Nothing moves, only the brightness
changes.

### 3.11 your space and theirs

- **your space is raised**: the composer pane, an open inbox item. everything
  you type or answer happens on the raised tint, with your accent bar `│`.
- **their space is the ground**: the history, the panel, the agents' work.
- **you never type in their space** and they never draw in yours.
- things that aren't your message (attachments, queued messages) sit in their
  own box, outside the text, so you never think they are part of it.

### 3.12 defaults that decide for you

- **bise has opinions**, so you don't need settings. first run asks one thing
  (which model does the work); every other role says `same as main` or `auto`.
- a default **always says what it resolves to**. never a bare `auto`.
- **safe for the user, honest about it**: what leaves the machine is named
  the first time (`auto sends commands to Jev (TypeSafe) to check them.`).
- **remembered**: a pick is saved and survives restarts; never asked twice.

---

## 4. components

Each component's exact spec is in the book. Here is what each one is for and
its one rule.

| component | for | its rule |
|---|---|---|
| frame `╭─ bise :* ─╮` | says "this is an app" | the header lives in its top border: the folder and the inbox count at rest |
| history | what happened, in order | append-only, never reordered, never rewritten |
| your message `│` | what you said | thin accent bar on every line; marks at the end |
| level-3 chip `✉ a → b` | agents talking | the quietest thing on screen, no accent inside |
| tool row `$` `ƒ` | a call, in one line | the model's own description, in your language; state glyph at the end |
| fold `▸ n more` | the rest of something | one row, dim, opens in place |
| panel row | one agent at a glance | `N glyph name … time  %  ψ`, numbers right-aligned, ψ far right |
| divider `├─ you → main ─┤` | who you talk to and how | `you → name · model · effort · mode · gust`, context short on the right |
| composer | your words | raised, accent bar, never locked, markdown shown as typed |
| key bar | the keys you can't guess | 3-4 pairs, key in text, label dim, 3 spaces apart |
| inbox box | decisions waiting | its own border, numbered rows, never over your draft |
| inbox item, open | one decision | raised, `┃` accent, one key answers, draft set aside |
| picker | a choice with steps | title as a question, what works first, esc back |
| find box | search the history | small, top-right, never covers what it found |
| palette (ctrl+s) | jump to an agent by name | live agents first, archived dim |
| flash | what just happened | where your eyes are, 2-3 s, a sentence of state |
| tip | the first time only | over what it explains, once ever |

---

## 5. words in the UI

The voice is book §4. The UI patterns:

- **bise says "i"**, the user is **"you"**. `i'll work in ~/acme`.
- **four nouns**: you, main, agents, the inbox. never task, hub, sub-agent,
  orchestrator. what waits in the inbox keeps its own word: a question, an
  approval.
- **state words are short and the same everywhere**: `working`, `idle`,
  `done`, `waiting`, `asks you`, `failed`, `starting`.
- **titles are questions** in pickers (`which model?`), **sentences of state**
  in flashes (`✓ inbox clear`), **verbs** in keys (`open`, `find`, `copy`).
- **no final period on a title or a key label.** a full sentence keeps it.
- **numbers before units, no fluff**: `58k · 22%`, `3m`, `42s`.
- **the command that fixes it** ends the error line (`/provider sets it up.`).
- **nothing LLM-y**: no "seamless", no "X, not Y", no "never deleted", no
  "let's". say the moment, not the feeling.

---

## 6. every screen in three forms, every width

A design is done when it exists in all of these:

- **dark and light**, both ≥ 4.5:1 for text you read.
- **`NO_COLOR`**: bold for the accent, no tints, `[ ]` around chips.
- **`BISE_ASCII=1`**: every glyph in its ASCII form, `+ - |` boxes.
- **80 columns** (no panel) and **150 columns** (panel), plus the drop order
  between them.
- **ctrl up and ctrl held.**
- **empty, one, many**: no item, one item, more than fit (`+ 3 more`).
- **the moment after**: what the screen says once you've acted.

---

## 7. how we design a change

1. **start from the user's words**, quoted in the issue. the design answers
   them, nothing more.
2. **mock it on the real geometry**: a local page (`site/content/*.html`,
   never deployed) drawn with the TUI's real columns, colors and rows. not a
   sketch: a screenshot-level mock.
3. **variants only where we hesitate**, the pick clearly marked, with one
   line of why.
4. **the user picks.** nothing is built before.
5. **the builder sends real captures** (tmux, 80 and 150, ctrl up and held).
   the designer signs off, or lists the changes.
6. **the book gets the final spec**, with the user's words and the issue
   number.

---

## 8. checklist for a new screen

- [ ] does it block typing, sending or switching agents? it must not.
- [ ] what is the one key for the common case?
- [ ] which level is it (needs you / for you / between agents)?
- [ ] what does it show at rest, what only with ctrl held?
- [ ] is any color used for something that doesn't need the user?
- [ ] does any glyph mean two things? is it one cell in every font?
- [ ] does every shown key work in this terminal? what's the fallback?
- [ ] what shows right after the user acts, and for how long?
- [ ] the drop order at 80 columns, written down
- [ ] dark, light, `NO_COLOR`, ASCII
- [ ] every word: lowercase, short, one of the four nouns, no LLM tells
- [ ] on failure: one line, what happened, the command that fixes it, nothing
      lost

---

## 9. open questions

- **the inbox's mark.** the panel shows `@ 2` on main's row and the header
  `# 1 in the inbox`. `@` is also the file key on the key bar and `#` a PR's
  number. proposal: bring back `✉︎` (text form, one cell) for the inbox only,
  in its own issue.
- **light theme syntax colors** are still to pick (BISE-01).
