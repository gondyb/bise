# the pages of the bise design system (site/design). format: see ds_site.py
GROUPS = [
    ("overview", ["overview/intro", "overview/laws", "overview/screen"]),
    ("foundations", ["foundations/color", "foundations/type", "foundations/space", "foundations/motion", "foundations/glyphs", "foundations/fallbacks"]),
    ("components", ["components/frame", "components/message", "components/levels", "components/chip", "components/tool-row", "components/fold",
                    "components/panel", "components/divider", "components/composer", "components/attachments", "components/key-bar",
                    "components/inbox", "components/inbox-item", "components/picker", "components/popup", "components/find", "components/palette", "components/flash"]),
    ("patterns", ["patterns/rest", "patterns/disclosure", "patterns/never-block", "patterns/one-key", "patterns/feedback", "patterns/room",
                  "patterns/errors", "patterns/zen", "patterns/spaces", "patterns/defaults"]),
    ("content", ["content/voice", "content/words", "content/templates"]),
    ("process", ["process/design", "process/checklist", "process/open"]),
]

P = {}
def page(pid, title, lead, body, src="", k=""):
    P[pid] = {"title": title, "lead": lead, "body": body, "src": src, "k": k}

# ------------------------------------------------------------------ overview
page("overview/intro", "introduction",
"how bise looks and behaves, and why. the laws, the foundations, every component of the terminal UI and the UX patterns behind them, with the reasons we chose them.",
r"""
bise is a terminal app: one thread where you talk to main, your team lead, while agents work behind it. everything on screen is text in cells, so this system is made of cells too: colors, glyphs, rows, keys and words.

this site is the layer above the [brand book](/book/). the book keeps every exact spec with its issue number. here you get the patterns and the reasons, so a new screen feels like the old ones without anyone having to ask. when the two disagree, the book wins and this site gets fixed.

## start here

::cards
the five laws -> #/overview/laws :: never block, calm by default, one key, show what happened, never lie
the screen -> #/overview/screen :: every part of the bise screen, named
color -> #/foundations/color :: color is attention: what each color may carry
components -> #/components/frame :: 18 components, each with its states and rules
patterns -> #/patterns/rest :: less at rest, progressive disclosure, feedback, drop order…
checklist -> #/process/checklist :: twelve questions before a new screen ships
::end

## how to read a component page

- **anatomy**: the parts, numbered, with what each one is for.
- **states**: the mocks have tabs. *rest*, *ctrl held*, *narrow*, *NO_COLOR*, *ASCII*. every mock is drawn in the real palette, on the real cells, and follows the site's light/dark switch.
- **rules**: what never changes.
- **do / don't**: the mistakes we already made once.

## the example

every mock tells the same small story, so you can compare them: you asked for three things (*signup is slow on mobile, dark mode, the 404 is sad*). main started three agents: **perf**, **dark-mode** and **sad-404**. perf is done. dark-mode works in its own worktree. sad-404 has a question for you: *the dog: a hat, or a scarf?*
""", k="start overview")

page("overview/laws", "the five laws",
"every rule on this site comes from one of these five. when two rules fight, the law higher on the list wins.",
r"""
## 1. never block

you can always type, send and move. nothing takes the keyboard away from you, nothing opens while you type, nothing waits for a click to let you go on.

::ex
::tab main works, you keep typing
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {>─} {d:58k · 22%} {r:─┤}
^{r:│}  {a:│}{>}{r:│}
^{r:│}  {a:│}   and the footer too{a:▌}{>}{r:│}
^{r:│}  {a:│}{>}{r:│}
^{r:│}      {t:tab} {d:queue}   {t:⏎} {d:steer}   {t:ctrl+c} {d:interrupt}{>}{r:│}
{r:╰}{>─}{r:╯}
::cap the composer is never locked during a turn: ⏎ steers the running turn, tab queues for after it.
::end

## 2. calm by default, loud only for you

at rest the screen says almost nothing. color, motion and position go to what needs you, and nothing else. a done agent is a small pink `✓`. an agent stuck on a question shows a pink `?` and an item in your inbox.

## 3. one key for the common case

the answer you give 9 times out of 10 is one key away, from anywhere. `ctrl+1` opens the first inbox item, `1` allows the command, and the next item opens by itself.

## 4. show what happened

every action leaves a trace you can see for a moment, then gets out of the way: a fold line after an answer, `✓ inbox clear` on the divider, the row you changed flashing `✓`.

## 5. never lie

no hidden limit, no fake progress, no key that doesn't work, no promise the product can't keep. without ctrl+digits from the terminal, the inbox says `click to open`. it never shows `ctrl+1`.

::note a law is a tie-breaker. "show what happened" never justifies a pop-up that blocks typing: law 1 wins.
""", k="principles never block calm one key feedback honest")

page("overview/screen", "the screen",
"one frame, one thread, one panel. every part of bise's main screen, named, so we all say the same word for the same thing.",
r"""
::ex w=100
::tab rest
{r:╭─} {b:bise} {a::*} {|66─}{r:┬}{>─} {d:~/acme · # 1 in the inbox} {r:─╮}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {a:│} signup is slow on mobile. and the 404 is sad {a:✓✓}{|66}{r:│}  {d:agents}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {ab::*} on it: perf, dark-mode and sad-404 started.{|66}{r:│}  {f:0} {g1} main {a::*} {d:@ 2}{|88}{d: 1m}{|93}{d:22%}{|99}{r:│}
{r:│}  {f:▸ 9 messages between 3 agents}{|66}{r:│}  {f:1} {a:✓} perf{|99}{r:│}
{r:│}{|66}{r:│}  {f:2} {g1} dark-mode{|88}{d: 3m}{|93}{d:12%}{|97}{d:ψ}{|99}{r:│}
{r:│}  {ch: ✉ dark-mode → main }{|66}{r:│}  {f:3} {a:?} sad-404{|93}{d:18%}{|99}{r:│}
{r:│}    {d:which gray for the borders?}{|66}{r:│}  {f:4} {g1} emoji-csv {a:•}{|88}{d:42s}{|93}{d:31%}{|99}{r:│}
{r:│}  {ab::*} i answered dark-mode: the gray in tokens.css.{|66}{r:│}  {f:5} {d:○} release{|93}{d: 8%}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {d:$} {d:runs the signup benchmark}{>63} {d:✓ 4.2s}{|66}{r:│}  {d:inbox}{|99}{r:│}
{r:│}  {a:✓} perf is done {f:·} signup 4.1 s → 0.9 s{|66}{r:│}  {f:1} {a:?} sad-404  {d:the dog: a h…}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│} {r:╭─} {f:inbox · 1 waiting for you} {>65─} {f:ctrl+1 open} {r:─╮}{|66}{r:│}{|99}{r:│}
{r:│} {r:│} {f:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>62} {f:4m}{|64}{r:│}{|66}{r:│}{|99}{r:│}
{r:│} {r:╰}{>65─}{r:╯}{|66}{r:│}{|99}{r:│}
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {|66─}{r:┴}{>─} {d:58k · 22%} {r:─┤}
^{r:│}  {a:│}{|99}{r:│}
^{r:│}  {a:│}   and give the sad dog a hat{a:▌}{|99}{r:│}
^{r:│}  {a:│}{|99}{r:│}
^{r:│}      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}   {t:ctrl+1} {d:inbox}{|99}{r:│}
{r:╰}{>─}{r:╯}
::tab ctrl held
{r:╭─} {b:bise} {a::*} {>─} {d:~/acme ·} {g3} {d:3 working · ✓ 1 done · # 1 in the inbox} {r:─╮}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {a:│} signup is slow on mobile. and the 404 is sad {a:✓✓}{|66}{r:│}  {d:agents}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {ab::*} on it: perf, dark-mode and sad-404 started.{|66}{r:│}  {f:0} {g1} main {a::*} {d:@ 2}{|88}{d: working}{|99}{r:│}
{r:│}  {f:▸} {a:ctrl+o} {f:expand}{|66}{r:│}  {f:1} {a:✓} perf{|88}{d:    done}{|99}{r:│}
{r:│}{|66}{r:│}  {f:2} {g1} dark-mode{|88}{d: working}{|97}{d:ψ}{|99}{r:│}
{r:│}  {ch: ✉ dark-mode → main }{|66}{r:│}  {f:3} {a:?} sad-404{|88}{a:asks you}{|99}{r:│}
{r:│}    {d:which gray for the borders?}{|66}{r:│}  {f:4} {g1} emoji-csv {a:•}{|88}{d: working}{|99}{r:│}
{r:│}  {ab::*} i answered dark-mode: the gray in tokens.css.{|66}{r:│}  {f:5} {d:○} release{|88}{d:    idle}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│}  {d:$} {d:runs the signup benchmark}{>63} {d:✓ 4.2s}{|66}{r:│}  {d:inbox}{|99}{r:│}
{r:│}  {a:✓} perf is done {f:·} signup 4.1 s → 0.9 s{|66}{r:│}  {a:1} {a:?} sad-404  {d:the dog: a h…}{|99}{r:│}
{r:│}{|66}{r:│}{|99}{r:│}
{r:│} {r:╭─} {f:inbox · 1 waiting for you} {>65─} {f:ctrl+1 open} {r:─╮}{|66}{r:│}{|99}{r:│}
{r:│} {r:│} {a:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>62} {f:4m}{|64}{r:│}{|66}{r:│}{|99}{r:│}
{r:│} {r:╰}{>65─}{r:╯}{|66}{r:│}{|99}{r:│}
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {d:working · 1m} {|66─}{r:┴}{>─} {d:58k / 262k tokens · 22%} {r:─┤}
^{r:│}  {a:│}{|99}{r:│}
^{r:│}  {a:│}   and give the sad dog a hat{a:▌}{|99}{r:│}
^{r:│}  {a:│}{|99}{r:│}
^{r:│}      {t:ctrl+c} {d:interrupt}   {t:ctrl+f} {d:find}   {t:ctrl+1} {d:open an inbox item}   {t:ctrl+s} {d:find agent}{|99}{r:│}
{r:╰}{>─}{r:╯}
::cap 100 columns. hold ctrl and every word comes back in its own place; let go and it goes.
::end

## anatomy

1. **the frame**: a thin rounded border around the whole app. its top border is the header: `bise :*`, the folder and the inbox count. → [frame & header](#/components/frame)
2. **the history**: main's thread, append-only, in a reading column. your messages carry a thin pink bar. → [your message](#/components/message), [three levels](#/components/levels)
3. **the agents panel**: one row per agent, its status glyph, its numbers. → [agents panel](#/components/panel)
4. **the inbox**: what needs you, in its own box above the divider. → [inbox](#/components/inbox)
5. **the divider**: who you talk to, with which model, effort and mode, and whether it works. → [divider](#/components/divider)
6. **the composer**: your space, raised, never locked. → [composer](#/components/composer)
7. **the key bar**: the 3-4 keys you can't guess. → [key bar](#/components/key-bar)
""", src="book §8", k="layout anatomy overview")

# ------------------------------------------------------------------ foundations
SW = lambda name, hexd, hexl, use: f"""<div><div class="chip" style="background:{hexd};color:{'#141211' if name in ('text','accent','dim','ok','faint') else '#ece6da'}">Aa</div><div class="meta"><b>{name}</b><span>{hexd} · light {hexl}</span><span>{use}</span></div></div>"""

page("foundations/color", "color",
"color is attention. only what needs you and what failed get a hue. everything else is text, dim or faint.",
r"""
## the palette

| role | dark | light | contrast | carries | never |
|---|---|---|---|---|---|
| text | `#ece6da` | `#1b1917` | 15:1 | what you read: answers, names, your message | — |
| dim | `#a39c90` | `#6b645a` | 6.9:1 | the model, the agent's own work, durations | something you must act on |
| faint | `#857d72` | `#7d766c` | 4.6:1 | key labels, numbers, separators ` · ` | a sentence you must read |
| rule | `#4a4540` | `#cfc8bd` | 1.97:1 | lines: the frame, the panel's rule, borders | text, ever |
| accent | `#f4a6b0` | `#b8416b` | 9.7:1 | needs you, `:*`, the agent you talk to, done `✓`, read `✓✓`, your bar | decoration |
| error | `#ff5a52` | `#b3261e` | 6.1:1 | failures only: `✗`, failing checks, a wrong key | a warning, a default |
| ok | `#b9d99a` | `#3f7a2a` | — | diff additions only | success |

## surfaces

| surface | dark | light | for |
|---|---|---|---|
| ground | `#141211` | `#fdfbf7` | every cell: bise paints its own background |
| raised | `#1f1c1a` | `#f4f0e8` | your space: the composer pane |
| raised 2 | `#26221f` | `#efeae0` | an inbox item open in place, one step above |
| chip | `#231f1d` | `#efe9df` | the envelope chip of a message between agents |
| pill | `#3a2530` | `#f0d3dc` | a quote or image chip in your text |
| selection | `#33292c` | `#fdeef2` | selected text |

## in context

::ex
::tab dark
{a:?} {ab:sad-404 needs you}{>}
{d:  the dog: a hat, or a scarf?}{>}
{>}
{a:✓} {d:perf is done · signup 4.1 s → 0.9 s}{>}
{e:✗} {e:release failed} {d:· npm publish: 403 forbidden}{>}
{g1} {d:dark-mode}  {f:working · 3m}{>}
::tab NO_COLOR | nocolor
{a:?} {ab:sad-404 needs you}{>}
{d:  the dog: a hat, or a scarf?}{>}
{>}
{a:✓} {d:perf is done · signup 4.1 s → 0.9 s}{>}
{e:✗} {e:release failed} {d:· npm publish: 403 forbidden}{>}
{g1} {d:dark-mode}  {f:working · 3m}{>}
::cap three hues on screen, three meanings: pink needs you or is yours, red failed, the rest is grey. under NO_COLOR, bold takes the accent's job.
::end

## rules

- **pink is never red.** "needs you" must not look like an error.
- **never green for done.** done is a small pink check. green success reads like a CI dashboard, and it isn't us.
- **one hue per row.** if two things on one row want color, one of them is wrong.
- **every readable text is ≥ 4.5:1** on white, our cream, black and a typical dark grey.
- **the rule color is for lines.** it was the old faint text color: too dark to read, so text moved to faint.

::do
- color the one thing on screen that needs the user
- use dim for the agent's own work, so your words and main's stand out
- keep the error color for something that really broke
::dont
- color a default (yolo is dim, not red)
- use green for success or a passing check
- use the rule color for text, even a hint
::end
""", src="book §5", k="palette colors tokens contrast accent pink")

page("foundations/type", "typography",
"one font, the user's. one size. importance comes from contrast, weight and room.",
r"""
## one size

a terminal has one font size, so we never pretend otherwise in the product. what's for you reads "bigger" through contrast (text over dim), weight (the speaker in bold) and room (a blank row above and below).

::ex
{d:∴ thought for 14s ▸}{>}
{d:$} {d:runs the signup benchmark}{>} {d:✓ 4.2s}{>}
{>}
{ab::*} {b:perf is done.} signup went from 4.1 s to 0.9 s on a phone.{>}
   the hero image was 4.2 MB, it's 310 kB now.{>}
{>}
{d:± edit web/src/hero.tsx ✓ +3 −1 ▸}{>}
::cap level 2 (main to you) gets text color, a bold speaker and a blank row around it. the agent's own work stays dim.
::end

## rules

- **the user's font** in the terminal. JetBrains Mono on the site, the book and every mock.
- **bold is rare**: a speaker (`:*`, `@ name to you:`), a screen's title, a key you must notice. never a paragraph.
- **lowercase everywhere**, except proper nouns (GitHub, Mistral), acronyms (PR, MCP) and keys as printed (`ctrl+s`). our own name stays `bise`.
- **no italic, no underline** in the product, except a link: the text underlined in the accent.
- **a reading measure**: prose wraps at 88 columns at most. code can run to 100.
""", src="book §5, §9", k="font type size bold lowercase")

page("foundations/space", "space & layout",
"everything sits on whole cells. a few fixed columns, a reading column, one blank row between blocks.",
r"""
## the grid

| column / row | what starts there |
|---|---|
| column 0 and F−1 | the frame |
| column 3 | the frame's title, the history's text, the divider's label |
| column 7 | the composer's text and the key bar (x0 + 4: the composer is its own pane) |
| panel text | from the panel's rule + 2, 28 columns, to F−4 (wider from 165 columns) |
| row 0 | the header, in the frame's top border |
| H−2 | the key bar, then the frame's bottom |

## the reading column

prose wraps at 88 columns whatever the width. more width goes to the margin and the panel, never to longer lines. tables and code may run to 100.

## widths

| terminal | layout |
|---|---|
| ≥ 165 | the panel grows 1 column for every 5, up to 44 at 240 |
| 100–164 | the panel at 28 columns, its rule joining the frame |
| 90–99 | the panel at 24 columns |
| < 90 | no panel: the header keeps short counts |
| < 60 or < 16 rows | no frame: a header row, a plain rule for the divider |

## vertical rhythm

- **one blank row** between history blocks. level 2 gets one above and below. two blank rows in a row is a bug.
- the history starts on row 2 and ends one blank row above the inbox or the divider.
- the composer pane at rest: divider, bar row, text, bar row, key bar, frame. nothing under the frame.

::do
- line a new element up with an existing column
- right-align numbers in their column (`3m`, `42s`, `12%`)
- keep every column in place even when its cell is blank, so rows line up
::dont
- start text at a new column of its own
- use half rows or sub-cell gaps in a mock: the terminal can't
- let prose grow with the screen
::end
""", src="book §8", k="spacing grid columns layout width narrow")

page("foundations/motion", "motion",
"motion means something is alive. the gust while an agent works, the cursor, the :* pop, a flash for 2-3 seconds. nothing bounces, nothing slides.",
r"""
## the gust

an agent at work is a gust blowing by: 5 cells, 110 ms a frame, a 9-frame cycle. head `≈` and `∿` in text, `~` dim, `·` faint, then 5 empty frames.

::ex
::tab divider (5 cells)
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {>─}{r:─┤}
::tab short (3 cells)
{r:├─} {d:you →} {a:main} {f:·} {d:opus·hi} {f:·} {d:yolo} {f:·} {g3} {>─}{r:─┤}
::tab panel (1 cell breathes)
{f:2} {g1} dark-mode{|21}{d: 3m}{|26}{d:12%}{|31}{d:ψ}{>}
::cap the gust never disappears while the agent works: it's the last thing kept on a short divider.
::end

## timing

| motion | duration |
|---|---|
| the gust | 110 ms a frame, 9 frames |
| a mode switch (shift+tab) | the word in accent for 3 s |
| a fold line after an answer | 2 s |
| `✓ inbox clear`, `✓ copied` | 2 s |
| zen fades in | at once; comes back 5 s after your last key |

## rules

- **one animation per thing**, never two on one row.
- **≤ 10 frames a second**, only the cells that change are redrawn.
- **stop** when the terminal loses focus, when nothing works, and under a reduce-motion setting: a static `∿` then.
- **no motion to decorate.** a pop for `:*` at launch is the one flourish.
""", src="book §5, §8 working", k="animation gust working timing reduce motion")

page("foundations/glyphs", "glyphs",
"one glyph per thing, one thing per glyph. one cell wide, in every common font, with an ASCII form.",
r"""
## the set

| glyph | means | ASCII |
|---|---|---|
| `:*` | main, your team lead | `:*` |
| `│` | your message (thin bar, accent) | `\|` |
| `┃` | needs you (heavy bar, accent) | `\|` |
| `∿` | working: the gust | `~` |
| `?` | needs you | `?` |
| `✓` | done (accent); at the end of your message: got it | `*` / `v` |
| `✓✓` | read by the model | `vv` |
| `✗` | failed (error) | `x` |
| `○` | idle | `o` |
| `…` | waiting on another agent | `;` |
| `·` | starting, sending | `.` |
| `•` | unread (accent) | `!` |
| `ψ` | its own worktree | `Y` |
| `↑` | its pull request | `^` |
| `$` | a bash call | `$` |
| `ƒ` | a TypeScript call | `f` |
| `±` | a file edit | `%` |
| `∴` | thinking | `:` |
| `▸` `▾` | closed / open | `+` `-` |
| `✉︎` | a message between agents | `@` |
| `▣` `❝` | an image, a quote | `#` |

## rules

- **one cell wide in every font we audited.** a glyph that turns into a color emoji somewhere is out: `✉` became `@` in the panel, `⎇` became `ψ`.
- **the place tells two marks apart.** `✓` leading a row is done; `✓ ✓✓` at the end of your message is got it / read.
- **every glyph has a row in the `/help` legend**, and a test fails when one hasn't.
- **a glyph needs no legend after a week.** if people ask twice, change it.
- **two meanings for one glyph is a bug.** see [open questions](#/process/open) for the one we still have.
""", src="book §6", k="icons symbols ascii glyph audit")

page("foundations/fallbacks", "themes & fallbacks",
"a design isn't done until it works in light and dark, under NO_COLOR, in ASCII and on 16 colors.",
r"""
## the same row, five ways

::ex
::tab dark
{f:3} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>} {f:4m}
{f:2} {g1} dark-mode {d:ψ} {f:·} {d:dims the borders}{>}
{a:✓} {d:perf is done}{>}
::tab NO_COLOR | nocolor
{f:3} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>} {f:4m}
{f:2} {g1} dark-mode {d:ψ} {f:·} {d:dims the borders}{>}
{a:✓} {d:perf is done}{>}
::tab ASCII
{f:3} {a:?} sad-404 {f:.} the dog: a hat, or a scarf?{>} {f:4m}
{f:2} {d:~} dark-mode {d:Y} {f:.} {d:dims the borders}{>}
{a:*} {d:perf is done}{>}
::cap the light theme: use the switch at the top right. every mock on this site follows it.
::end

## the forms

| form | what changes |
|---|---|
| **light** | the palette's light column, picked from the terminal's background (OSC 11), or forced |
| **NO_COLOR** | no hue, no tint. the accent becomes bold, dim stays dim, chips get `[ ]` |
| **16 colors** | no tints: the raised pane and chips go, the bar alone marks your space |
| **ASCII** (`BISE_ASCII=1`) | every glyph in its ASCII form, boxes in `+ - \|`, `...` for a cut |
| **reduce motion** | a static `∿` everywhere |

::note bise paints its own background on every cell, so it reads whatever the terminal's theme. it gives the terminal's color back on exit, on a crash, and when a shell takes the screen.
""", src="book §5, §6", k="light dark no_color ascii 16 colors theme")

# ------------------------------------------------------------------ components
page("components/frame", "frame & header",
"bise draws itself like an app: a thin rounded frame on the terminal's edge, with the header in its top border.",
r"""
::ex
::tab rest
{r:╭─} {b:bise} {a::*} {>─} {d:~/acme · # 1 in the inbox} {r:─╮}
{r:│}{>}{r:│}
::tab ctrl held
{r:╭─} {b:bise} {a::*} {>─} {d:~/acme ·} {g3} {d:3 working · ✓ 1 done · # 1 in the inbox} {r:─╮}
{r:│}{>}{r:│}
::tab no panel (narrow)
{r:╭─} {b:bise} {a::*} {>─} {g1} {d:3 · ? 1 · ✓ 1 · # 1} {r:─╮}
{r:│}{>}{r:│}
::tab an agent's view
{r:╭─} {b:bise} {a::*} {f:·} {d:dims the borders to tokens.css} {>─} {d:~/acme} {r:─╮}
{r:│}{>}{r:│}
::end

## anatomy

1. `bise` bold and `:*` in accent, from column 3.
2. in an agent's view, its **role line**: what it's doing now, ≤ 60 characters, written after each of its turns.
3. on the right, dim: the folder and the inbox count. with no panel, the short counts (`∿ 3 · ? 1`), because the panel is the one that shows agents.

## rules

- the frame's lines are in the rule color and paint no background.
- short on room: the folder goes first, then long counts become short.
- under 60 columns or 16 rows: no frame, a header row instead.
""", src="book §8 the frame", k="header title border")

page("components/message", "your message",
"what you said, marked by a thin pink bar on every line. its marks at the end say whether it arrived and was read.",
r"""
::ex
::tab sent
{a:│} signup is slow on mobile. and dark mode. and the 404 is sad {f:·}{>}
::tab got it
{a:│} signup is slow on mobile. and dark mode. and the 404 is sad {d:✓}{>}
::tab read
{a:│} signup is slow on mobile. and dark mode. and the 404 is sad {a:✓✓}{>}
::tab long
{a:│} here is the full list of things i noticed on the signup page, in{>}
{a:│} the order i'd fix them:{>}
{a:│}  1. the hero image is 4.2 MB{>}
{a:│} {f:▸ 12 more lines} {a:✓✓}{>}
::end

## anatomy

1. the thin `│` bar in accent, on every wrapped line. the heavy `┃` belongs to things that need you.
2. your text, in the text color.
3. the mark: `·` sending, `✓` the agent got it, `✓✓` (accent) the model read it.
4. past 20 lines: `▸ n more lines`, opened by a click, `space` or `ctrl+o`.

## rules

- thin bar = you, heavy bar = needs you. you tell at a glance what you said from what they said.
- a quote or an image is one chip in your text (`❝ 1`, `▣ 1`), never a separate block above it.
""", src="book §9", k="user message bar read marks")

page("components/levels", "three levels",
"everything in the history sits on one of three levels: it needs you, it's for you, or it's between agents.",
r"""
::ex
{a:┃} {ab:? sad-404 needs you}{>}
{a:┃} the dog: a hat, or a scarf?{>}
{>}
{ab::*} i answered dark-mode: the gray in tokens.css, like everywhere.{>}
{>}
{ch: ✉ dark-mode → main }{>}
  {d:which gray for the borders?}{>}
{ch: ✉ main → dark-mode }{>}
  {d:the one in tokens.css.}{>}
::cap top to bottom: level 1, level 2, level 3.
::end

| level | what | look |
|---|---|---|
| **1 · needs you** | a question, an approval | the inbox; the heavy `┃` and a bold accent title, until you answer |
| **2 · for you** | main talking to you, reports on what you asked | text color, the speaker in bold, a blank row above and below |
| **3 · between agents** | what agents tell each other | dim, an envelope chip, folded after a few |

## rules

- levels 2 and 3 differ by brightness and the chip, never by hue.
- a new kind of event gets a level **before** it gets a look. if it fits none, it probably shouldn't reach the screen.
- main answering for you is level 2, and it says why: `▸ why`.
""", src="book §9", k="levels history attention level 1 2 3")

page("components/chip", "agent messages",
"when agents talk to each other, it looks like a message: a small tinted chip says who writes to whom, the text goes under it, dim.",
r"""
::ex
::tab one
{ch: ✉ dark-mode → main }{>}
  {d:which gray for the borders? the mock says #2a2623, tokens.css}{>}
  {d:says #4a4540.} {f:… ▸}{>}
::tab a run
{ch: ✉ perf → main }{>}
  {d:the hero image is 4.2 MB. ok to convert it to webp?}{>}
  {d:also: the font loads twice.}{>}
{ch: ✉ main → perf }{>}
  {d:yes to both.}{>}
::tab folded
{f:▸ 9 messages between 3 agents}{>}
::tab NO_COLOR | nocolor
{t:[✉ dark-mode → main]}{>}
  {d:which gray for the borders?}{>}
::end

## rules

- the quietest thing on screen: the chip's tint is its only background, **no accent inside**, main included.
- the text sits under the chip, never beside it, so every text starts on the same column.
- at most 2 rows, then `… ▸`. the same pair stacks with no blank row; a new pair gets one.
- short on room, the receiver's name is cut before the sender's. never the arrow, never the envelope.
""", src="book §9 level 3", k="level 3 envelope chip agents talking")

page("components/tool-row", "tool rows",
"each bash or TypeScript call is one dim row: its glyph, what the model says it's doing, in your language, and its state.",
r"""
::ex
::tab rows
{d:$} {d:installs the deps}{>} {d:✓ 3.1s}
{d:ƒ} {t:reads the open issues on GitHub}{>} {g1} {d:12s}
{e:$} {d:runs the tests}{>} {e:✗ exit 1 · 0.8s}
   {d:FAIL signup.spec.ts › keeps the email after a reload…}{>}
{d:$ ▸ 6 commands · weighs the hero image}{>} {d:✓ 3.2s}
::tab one opened
{r:╭─} {d:$ weighs the hero image ✓ 0.1s} {>─}{r:─╮}
{r:│}  ls -la public/hero.png{>}{r:│}
{r:├}{>─}{r:┤}
{r:│}  {d:-rw-r--r--  1 gab  staff  4.2M  hero.png}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab ASCII
{d:$} {d:installs the deps}{>} {d:ok 3.1s}
{d:f} {t:reads the open issues on GitHub}{>} {d:~ 12s}
{e:$} {d:runs the tests}{>} {e:x exit 1}
{d:> 6 commands · weighs the hero image}{>} {d:ok 3.2s}
::end

## anatomy

1. `$` bash or `ƒ` TypeScript, in the lead column (error color when it failed).
2. the model's own **description**, one short line in your language. running, it's in the text color.
3. the state, right-aligned: `∿ 12s` running, `✓ 0.3s` done (dim, never green), `✗ exit 1 · 0.8s` failed.
4. a failure adds one row: the first error line, cut with `…`.

## rules

- 4 done calls or more in a run fold into `▸ n commands`. failed and running calls keep their rows.
- a click or `space` opens one call as a box (15 rows); `ctrl+o` opens every one.
- a failure never opens by itself: its first error line is enough to decide.
""", src="book §11 calls", k="bash typescript tool call description command")

page("components/fold", "folds",
"▸ means there is more here. one line by default, everything one key away. folded is never gone.",
r"""
::ex
::tab closed
{d:∴ thought for 14s ▸}{>}
{d:± edit web/src/hero.tsx ✓ +3 −1 ▸}{>}
{f:▸ 9 messages between 3 agents}{>}
{a:│} {f:▸ 12 more lines} {a:✓✓}{>}
::tab ctrl held
{d:∴ thought for 14s} {a:ctrl+o} {f:expand}{>}
{d:± edit web/src/hero.tsx ✓ +3 −1} {a:ctrl+o} {f:expand}{>}
{f:▸} {a:ctrl+o} {f:expand}{>}
{a:│} {f:▸} {a:ctrl+o} {f:expand} {a:✓✓}{>}
::end

## rules

- `▸` closed, `▾` open. a click or `space` toggles one, `ctrl+o` opens or closes all.
- the fold row says what's inside and how much: `▸ 12 more lines`, `▸ 6 commands`.
- search (`ctrl+f`) finds text in a folded part and opens it.
- what you must act on is never folded.
""", src="book §11", k="fold expand ctrl+o disclosure more lines")

page("components/panel", "agents panel",
"one row per agent: its number, its status glyph, its name, and on the right what changes: the turn time, the context, its worktree.",
r"""
::ex w=34
::tab rest
{d:agents}{>}
{>}
{f:0} {g1} main {a::*} {d:@ 2}{|21}{d: 1m}{|26}{d:22%}{>}
{f:1} {a:✓} perf{>}
{f:2} {g1} dark-mode{|21}{d: 3m}{|26}{d:12%}{|31}{d:ψ}{>}
{f:3} {a:?} sad-404{|26}{d:18%}{>}
{f:4} {g1} emoji-csv {a:•}{|21}{d:42s}{|26}{d:31%}{>}
{f:5} {d:○} release{|26}{d: 8%}{>}
::tab ctrl held
{d:agents}{>}
{>}
{f:0} {g1} main {a::*} {d:@ 2}{|21}{d: working}{>}
{f:1} {a:✓} perf{|21}{d:    done}{>}
{f:2} {g1} dark-mode{|21}{d: working}{|31}{d:ψ}{>}
{f:3} {a:?} sad-404{|21}{a:asks you}{>}
{f:4} {g1} emoji-csv {a:•}{|21}{d: working}{>}
{f:5} {d:○} release{|21}{d:    idle}{>}
::tab ⌥ held
{d:agents} {f:·} {d:⌥↑↓ select}{>}
{>}
{a:⌥0} {g1} main {a::*} {d:@ 2}{|21}{d: 1m}{|26}{d:22%}{>}
{a:⌥1} {a:✓} perf{>}
{a:⌥2} {g1} dark-mode{|21}{d: 3m}{|26}{d:12%}{|31}{d:ψ}{>}
{a:⌥3} {a:?} sad-404{|26}{d:18%}{>}
::tab with a PR
{f:2} {g1} dark-mode{|21}{d: 3m}{|26}{d:12%}{|31}{d:↑}{>}
{f:6} {a:✓} login-fix{|26}{d: 9%}{|31}{e:↑}{>}
::cap with a PR: ↑ takes ψ's column (a PR implies a branch). red only when its checks fail. designed, not built yet.
::end

## anatomy

1. **number**, faint: `⌥` + it opens the agent. it never changes while the agent lives.
2. **status glyph**: the breathing gust, `✓`, `?`, `✗`, `○`, `…`.
3. **name**, then its marks: main's `:*` and `@ n`, the pink `•` unread.
4. **turn time**, 3 columns, right-aligned, only while it works.
5. **context %**, 3 columns, right-aligned.
6. **ψ** far right when it works in its own worktree.

## rules

- **no state words at rest**: the glyph says them. holding ctrl writes them in the time and % columns.
- **every column stays in place** even when blank, so the rows line up.
- a name takes the room its row leaves and is cut with `…` only there.
""", src="book §8 agents panel", k="sidebar agents rows worktree context")

page("components/divider", "divider",
"the rule between the history and your space. it says who you talk to, with which model, effort and mode, and whether it works.",
r"""
::ex
::tab working
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {>─} {d:58k · 22%} {r:─┤}
::tab idle
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {>─} {d:18k · 2%} {r:─┤}
::tab ctrl held | w=90
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {d:working · 1m} {>─} {d:58k / 262k tokens · 22%} {r:─┤}
::tab mode switched
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {a:auto} {>─} {d:18k · 2%} {r:─┤}
::tab an agent
{r:├─} {d:you →} {a:dark-mode} {f:·} {d:sonnet 5.5} {f:·} {d:low} {f:·} {d:yolo} {f:·} {d:ψ sb/dark-mode} {f:·} {g5} {>─}{r:─┤}
::tab narrow
{r:├─} {d:you →} {a:main} {f:·} {d:opus·hi} {f:·} {d:yolo} {f:·} {g3} {>─}{r:─┤}
::end

## anatomy

1. `you →` dim, the agent's name in accent.
2. model and effort, dim, ` · ` faint between them.
3. the approvals mode, dim (accent for 3 s after shift+tab).
4. the gust, only while it works.
5. on the right, the context, short: `58k · 22%`.

## rules

- the divider is also where short notes land: `✓ inbox clear`, `✓ copied 9 chars`, an open item's `you → ? sad-404 · your answer`.
- short on room: the long context falls back to the short one, then the context goes, then the branch name, then `opus 5.5 · high` becomes `opus·hi`. the mode goes last. the gust stays.
""", src="book §8 divider", k="status model effort mode context tokens")

page("components/composer", "composer",
"your space: a raised pane with your pink bar. it's never locked, and markdown shows as you type it.",
r"""
::ex
::tab empty
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {>─} {d:18k · 2%} {r:─┤}
^{r:│}  {f:│}{>}{r:│}
^{r:│}  {f:│}   {d:what's on your mind?}{>}{r:│}
^{r:│}  {f:│}{>}{r:│}
^{r:│}      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab typing
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {>─} {d:18k · 2%} {r:─┤}
^{r:│}  {a:│}{>}{r:│}
^{r:│}  {a:│}   make the 404 page **funnier**, and give the dog a hat{a:▌}{>}{r:│}
^{r:│}  {a:│}{>}{r:│}
^{r:│}      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab main works
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {g5} {>─} {d:58k · 22%} {r:─┤}
^{r:│}  {a:│}{>}{r:│}
^{r:│}  {a:│}   and the footer too{a:▌}{>}{r:│}
^{r:│}  {a:│}{>}{r:│}
^{r:│}      {t:tab} {d:queue}   {t:⏎} {d:steer}   {t:ctrl+c} {d:interrupt}{>}{r:│}
{r:╰}{>─}{r:╯}
::end

## rules

- **raised, edge to edge** inside the frame: it's the one place where you type.
- the bar is faint while empty, accent with text, an image or a recording.
- **never locked**: during a turn `⏎` steers it, `tab` queues for after it.
- **markdown is colored, never hidden**: every mark stays, the message sent is the text typed.
- text starts at column 7, one column right of the history's: the pane is its own space.
""", src="book §8 composer, §13", k="input prompt typing steer queue")

page("components/attachments", "attachments box",
"what comes with your message, but isn't your text, sits in its own small box above it. so you never think it's part of what you typed.",
r"""
::ex
^{r:│}  {r:╭─} {f:attached} {>69─}{r:─╮}{>}{r:│}
^{r:│}  {r:│} {a:❝ 1} {d:“la licence du repo,”}{>68}{r:│}{>}{r:│}
^{r:│}  {r:│} {a:▣ 2} {d:404.png} {f:1440×900}{>68}{r:│}{>}{r:│}
^{r:│}  {r:╰─} {f:backspace on a chip removes it} {>69─}{r:─╯}{>}{r:│}
^{r:│}  {a:│}   tu recommandes quoi pour {a:❝ 1} ? et regarde {a:▣ 2}{a:▌}{>}{r:│}
^{r:│}      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}{>}{r:│}
::end

## rules

- a quote or an image is a chip **inline** in your text; the box lists what each chip is.
- the box has its own border and title, so it reads as a tray, not as lines of your message.
- backspace on a chip removes it from both places.
""", src="book §13 the composer block", k="images quotes chips attachments paste")

page("components/key-bar", "key bar",
"the 3 or 4 keys you can't guess, under the composer. holding ctrl shows every other one.",
r"""
::ex
::tab rest
^      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}   {t:ctrl+1} {d:inbox}{>}
::tab main works
^      {t:tab} {d:queue}   {t:⏎} {d:steer}   {t:ctrl+c} {d:interrupt}{>}
::tab an agent's view
^      {t:esc} {d:back to main}   {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}{>}
::tab ctrl held
^      {t:ctrl+c} {d:interrupt}   {t:ctrl+f} {d:find}   {t:ctrl+1} {d:open an inbox item}{>}
::tab an item open
^      {t:1-2} {d:answer}   {t:←→} {d:choose}   {t:↑↓} {d:other items}   {t:esc} {d:back to your message}{>}
::end

## rules

- the key in text color, what it does dim, 3 spaces between pairs.
- **only keys you can't guess.** `⏎ send` and `? help` went: everybody knows them.
- **only keys that work here.** `ctrl+1 inbox` shows only while the inbox has items, and only when the terminal sends ctrl+digits; otherwise `/inbox`.
- the bar follows the mode: steering, an agent's view, an open item each have their own.
""", src="book §8, §16", k="shortcuts hints keys bottom bar")

page("components/inbox", "inbox",
"what needs you, in its own box above the divider. it never blocks: you keep typing to main while items wait.",
r"""
::ex
::tab one item
{r:╭─} {f:inbox · 1 waiting for you} {>─} {f:ctrl+1 open} {r:─╮}
{r:│} {f:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab four items
{r:╭─} {f:inbox · 4 waiting for you} {>─} {f:ctrl+1-3 open} {r:─╮}
{r:│} {f:1} {a:?} t3 {f:·} $ npm publish --access public{>69} {f:2m}{>}{r:│}
{r:│} {f:2} {a:?} api-v2 {f:·} $ git push origin main{>69} {f:3m}{>}{r:│}
{r:│} {f:3} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:│} {f:+ 1 more · ↑ #409 ready to merge}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab ctrl held
{r:╭─} {f:inbox · 4 waiting for you} {>─} {f:ctrl+1-3 open} {r:─╮}
{r:│} {a:1} {a:?} t3 {f:·} $ npm publish --access public{>69} {f:2m}{>}{r:│}
{r:│} {a:2} {a:?} api-v2 {f:·} $ git push origin main{>69} {f:3m}{>}{r:│}
{r:│} {a:3} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:│} {f:+ 1 more · ↑ #409 ready to merge}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab no ctrl+digits
{r:╭─} {f:inbox · 1 waiting for you} {>─} {f:click to open} {r:─╮}
{r:│} {f:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:╰}{>─}{r:╯}
::end

## what goes in

| kind | answers |
|---|---|
| an approval | `1` allow · `2` always allow … here · `3` no |
| a hard rule (push to main, secrets, a wipe) | `1` allow · `3` no: no "always" |
| a sandbox rerun | `1` run it again without the sandbox · `2` always here · `3` no |
| a question | its choices, or free text |
| ready to merge *(designed)* | `1` merge it · `2` not yet |

reports, done notes and FYI stay out: they're main's thread.

## rules

- **its own box**, rounded, in the rule color, on the ground. no box when there's nothing.
- **numbered the same everywhere**: the row that says `1` here says `1` in the panel, and `ctrl+1` opens it.
- approvals first, then questions, then merges. 3 rows, then `+ n more`.
- the title's right side only names a key that works: `ctrl+1-3 open`, `click to open`, or `/inbox opens it`.
""", src="book §12", k="cards approvals questions inbox ctrl+1")

page("components/inbox-item", "inbox item",
"an item opens where its row was, with what the agent did last. one key answers it, and the next one opens by itself.",
r"""
::ex
::tab open
{r:╭─} {f:inbox · 2 waiting for you} {>─} {f:ctrl+1-2 open} {r:─╮}
^^{r:│} {a:┃}{>}{r:│}
^^{r:│} {a:┃} {ab:?} {b:t3 wants to run}{>} {d:1 of 2 · 2m} {r:│}
^^{r:│} {a:┃}   $ npm publish --access public{>}{r:│}
^^{r:│} {a:┃}{>}{r:│}
^^{r:│} {a:┃}   {d:it publishes the package: everyone can install it.}{>}{r:│}
^^{r:│} {a:┃}   {d:t3 is shipping 2.5.0 · its last step: ✓ npm run build}{>}{r:│}
^^{r:│} {a:┃}{>}{r:│}
^^{r:│} {a:┃}   {a:1} {d:allow}   {a:2} {d:always allow npm publish * here}   {a:3} {d:no}{>}{r:│}
^^{r:│} {a:┃}   {f:or type why not, ⏎ says no}{>}{r:│}
^^{r:│} {a:┃}{>}{r:│}
{r:│} {f:2} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:╰}{>─}{r:╯}
{r:├─} {d:you →} {a:? t3} {f:·} {d:your answer} {>─}{r:─┤}
::tab option chosen
{r:╭─} {f:inbox · 2 waiting for you} {>─} {f:ctrl+1-2 open} {r:─╮}
^^{r:│} {a:┃} {ab:?} {b:t3 wants to run}{>} {d:1 of 2 · 2m} {r:│}
^^{r:│} {a:┃}   $ npm publish --access public{>}{r:│}
^^{r:│} {a:┃}{>}{r:│}
^^{r:│} {a:┃}   {hl: 1 allow }  {a:2} {d:always allow npm publish * here}   {a:3} {d:no}{>}{r:│}
^^{r:│} {a:┃}   {f:or type why not, ⏎ says no}{>}{r:│}
{r:│} {f:2} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>69} {f:4m}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab right after
{r:╭─} {f:inbox · 1 waiting for you} {>─} {f:ctrl+1 open} {r:─╮}
{r:│} {a:✓} {d:you allowed t3: npm publish --access public}{>}{r:│}
^^{r:│} {a:┃} {ab:?} {b:sad-404 asks}{>} {d:1 of 1 · 4m} {r:│}
^^{r:│} {a:┃}   the dog: a hat, or a scarf?{>}{r:│}
^^{r:│} {a:┃}{>}{r:│}
^^{r:│} {a:┃}   {a:1} {d:a hat}   {a:2} {d:a scarf}{>}{r:│}
^^{r:│} {a:┃}   {f:or type your answer, ⏎ sends it}{>}{r:│}
{r:╰}{>─}{r:╯}
::tab the last one
{r:├─} {a:✓} inbox clear {>─}{r:─┤}
^{r:│}  {a:│}{>}{r:│}
^{r:│}  {a:│}   and give the sad dog a hat{a:▌}{>}{r:│}
::cap after the last answer, the box goes, your draft comes back and the divider says ✓ inbox clear for 2 s.
::end

## keys

| key | does |
|---|---|
| `ctrl+1`…`9`, a click | open item N, in place |
| `1`–`9` | pick that option (empty composer) |
| `←` `→` | highlight an option; `⏎` picks it |
| `↑` `↓` | the previous / next item |
| type, then `⏎` | answer in words (for an approval: a no, with your note) |
| `ctrl+o` | full screen, the other items as tabs |
| `esc` | back to your message, the cursor where it was |

## rules

- the open item is **raised one step** above the composer, with the heavy `┃` on every row.
- **your draft is set aside, never lost**: the divider says `you → ? t3 · your answer` while you answer.
- after an answer, a fold line for 2 s, then **the next item opens by itself**: `1, 1, 3, 1` clears four.
- past half the feed, the item shows its start and `… n more lines · ctrl+o full screen`.
""", src="book §12", k="approval card answer open allow deny")

page("components/picker", "pickers",
"every full-screen choice looks the same: a question, a filter, what works first, enter forward, esc back.",
r"""
::ex w=78
::tab /models
{b:which model does what?}{>}
{d:each role picks a provider, then a model. one provider can serve several.}{>}
{>}
{a:›} main         Mistral  {d:mistral-large-latest · high}{>}
  agents       {d:same as main · Mistral · mistral-large-latest}{>}
  small jobs   {d:auto · Mistral · ministral-8b-latest}{>}
  voice        Mistral  {d:voxtral-mini-latest}{>}
{>}
{t:↑↓} {d:choose} {f:·} {t:enter} {d:change} {f:·} {t:esc} {d:back}{>}
::tab step 1
{b:agents: which provider?}{>}
{d:now: same as main · Mistral · mistral-large-latest}{>}
{>}
{a:›} same as main   {d:Mistral · mistral-large-latest}{>}
  Mistral        {a:✓} ready {f:· main, small jobs use it}{>}
  Anthropic      {a:✓} ready{>}
  OpenAI         {d:not set up}{>}
  {d:more providers…}{>}
::tab step 2
{b:agents · Anthropic: which model?}{>}
{d:type to filter, or a model id that isn't listed.}{>}
{a:›} {d:son}{a:▌}{>}
{>}
{a:›} claude-sonnet-5-5   {a:recommended}{>}
  claude-sonnet-4-6{>}
::end

## rules

- **a bold question as the title**, one dim line saying what it changes.
- **what works comes first**: ready providers before the rest, `same as main` / `auto` first, with what they resolve to.
- **the current one** says `now` (dim). **one** `recommended` at most, in accent.
- `enter` forward, `esc` back one step, digits pick at once. a step with one choice is skipped.
- after the last step you land where you started, the row flashing `✓`.
""", src="book §8 /models, /provider", k="menu models provider roles picker list")

page("components/popup", "popups",
"/, @ and $ open a small list right above the composer. tab completes, ⏎ runs, esc closes. typing never stops.",
r"""
::ex
{r:╭─}{>─}{r:─╮}
{r:│} {a:›} {b:/models}    {d:which model does what}{>}{r:│}
{r:│}   /model     {d:the model of the agent in view}{>}{r:│}
{r:│}   /theme     {d:light · dark · auto}{>}{r:│}
{r:╰}{>─}{r:╯}
^{r:│}  {a:│}   /mo{a:▌}{>}{r:│}
::end

## rules

- `/` commands, `@` a file, `$` a skill. the list filters as you type.
- `tab` completes, `⏎` runs once nothing is left to pick, `esc` closes and keeps your text.
- the arguments of a command are listed the same way (`/theme` → light · dark · auto).
""", src="book §16", k="slash commands autocomplete file skills")

page("components/find", "find",
"ctrl+f (or cmd+f) finds text in the history, in a small box over its top-right corner, like an editor's find.",
r"""
::ex
{>45}{r:╭─} {f:find} {>─}{r:─╮}
{>45}{r:│} {a:›} hero{a:▌}{>66}{f:2/5}{>}{r:│}
{>45}{r:╰}{>─}{r:╯}
the hero image was 4.2 MB, the {sel:hero} loads twice.{>}
::end

## rules

- 3 rows, its top border on the history's first row, left of the panel's rule.
- `⏎` next, `shift+⏎` previous, `esc` closes. it searches folded text too, and opens it.
- it never covers the line it found: the history scrolls.
""", src="book §16 ctrl+f", k="search find ctrl+f cmd+f")

page("components/palette", "agent palette",
"type part of an agent's name to jump to it. live agents first, archived ones dim.",
r"""
::ex
{r:╭─} {f:find agent} {>─}{r:─╮}
{r:│} {a:›} dar{a:▌}{>}{r:│}
{r:│}{>}{r:│}
{r:│} {a:›} {g1} {b:dar}k-mode {f:·} {d:dims the borders to tokens.css}{>}{r:│}
{r:│}   {d:–} {d:dar}{d:t-sass} {f:· archived}{>}{r:│}
{r:╰}{>─}{r:╯}
::end

## rules

- `ctrl+s` everywhere; `cmd+k` once the terminal has passed a cmd key. `/switch` works too.
- `↑↓` choose, `⏎` opens the agent's view, `esc` closes.
- the part that matches is bold; an archived agent opens read-only.
""", src="book §16 switch agents", k="switch agents cmd+k ctrl+s palette")

page("components/flash", "flashes & tips",
"a flash says what just happened, where your eyes already are, for 2-3 seconds. a tip explains something the first time, once.",
r"""
::ex
::tab mode switch
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {a:auto} {>─} {d:18k · 2%} {r:─┤}
^      {a:auto} {f:·} {d:safe calls run, risky ones ask you}{>}
::tab copied
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {a:✓} copied 9 chars {>─}{r:─┤}
::tab inbox clear
{r:├─} {a:✓} inbox clear {>─}{r:─┤}
::tab first-time tip
{>34}{r:╭─}{>─}{r:─╮}
{>34}{r:│} {d:you're in yolo: everything runs.}{>}{r:│}
{>34}{r:│} {t:⇧⇥} {d:changes it.}{>}{r:│}
{>34}{r:╰}{>─}{r:╯}
{r:├─} {d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {a:yolo} {>─} {d:18k · 2%} {r:─┤}
::end

## rules

- **where your eyes are**: the divider, the row you touched, the key bar. never a pop-up in a corner.
- **a sentence of state**, not a cheer: `✓ inbox clear`, never "all done!".
- a tip shows **once, ever**, over the thing it explains, and goes on the next key.
""", src="book §12, approvals", k="toast notification flash tip hint")

# ------------------------------------------------------------------ patterns
page("patterns/rest", "less at rest, more when you hold a key",
"at rest the screen shows what changes and what needs you. holding a modifier brings every word back, in place, and letting go puts it away.",
r"""
::ex w=34
::tab rest
{f:2} {g1} dark-mode{|21}{d: 3m}{|26}{d:12%}{|31}{d:ψ}{>}
{f:3} {a:?} sad-404{|26}{d:18%}{>}
::tab ctrl held
{f:2} {g1} dark-mode{|21}{d: working}{|31}{d:ψ}{>}
{f:3} {a:?} sad-404{|21}{a:asks you}{>}
::end

## the modifiers

| hold | shows |
|---|---|
| `ctrl` | every ctrl key where it acts (`▸ ctrl+o expand`, the inbox numbers in accent), the state words, the long forms (`58k / 262k tokens`), the full header, the full key bar |
| `⌥` | the panel's numbers as `⌥0` `⌥1`, `⌥↑↓ select` |
| `cmd` | the cmd keys, once a cmd key has really reached bise |

## rules

- **the held form replaces the rest form in the same cells.** nothing moves, nothing pushes.
- **the glyph already says it**, so the word waits for ctrl.
- **a key you hold types nothing**: it's the cheapest way to see more.

::note why: "on affiche trop d'informations" (the user). the glyphs said the state, and the words said it again.
""", k="ctrl held hints hidden reveal modifier")

page("patterns/disclosure", "progressive disclosure",
"one line by default, details one key away. folded is never gone, and what you must act on is never folded.",
r"""
| item | by default | opened |
|---|---|---|
| thinking | `∴ thought for 14s ▸` | the full text |
| a bash / TypeScript call | one row with the model's description | its box: script and output |
| 4+ calls in a run | `▸ 6 commands · …` | the rows |
| a file edit | `± edit hero.tsx ✓ +3 −1 ▸` | the diff |
| agents talking | `▸ 9 messages between 3 agents` | the chips |
| your long message | 20 rows, `▸ 12 more lines` | the whole message |
| a report | `✓ perf is done ▸ report` | the report |

## rules

- `space` or a click opens one, `ctrl+o` opens all, `▾` closes.
- a failure shows its first error line, a question shows in full: **you never open something to know you must act**.
- search finds what's folded and opens it.
""", src="book §11", k="fold expand ctrl+o collapse details")

page("patterns/never-block", "never block",
"you can always type, send and move. nothing takes the keyboard away, nothing opens while you type.",
r"""
## where it shows

- **the composer is never locked.** during a turn, `⏎` steers it, `tab` queues for after it.
- **the inbox never takes the composer.** it's a box above it. an item opens only when you ask (`ctrl+N`, a click), never while your draft has text.
- **an open item sets your draft aside**, and `esc` gives it back, cursor in place.
- **talk to any agent, anytime.** `⌥` + its number, then back to main. nobody has to stop.
- **restart whenever.** the agents, your thread, your draft and your queue come back.

::do
- let the user answer later: an item waits in the inbox, the agent waits for it
- keep the thread in sight while answering: open in place, not on a new screen
::dont
- open a modal that eats the next keys
- auto-focus a new item while the user types
- block sending because an item waits
::end
""", k="blocking modal focus typing composer locked")

page("patterns/one-key", "one key for the common case",
"the answer you give 9 times out of 10 is one key away, from anywhere. digits pick, the same digit everywhere.",
r"""
## rules

- **digits pick.** in an item, a menu, a picker: `1`–`9` takes that option at once.
- **one number, one thing, everywhere.** the inbox row `1` is `1` in the panel, and `ctrl+1` opens it from anywhere.
- **the next one comes to you.** after an answer the next item opens by itself: `1, 1, 3, 1` clears four.
- **one key, one meaning.** `esc` is always back one step. `⏎` is always "do it". `ctrl+o` always opens what's folded.
- **every key that's shown works.** we check what the terminal sends; no ctrl+digits → `click to open`, `/inbox`.
- **leave the system's keys alone.** macOS keeps ctrl+arrows for Spaces, so we don't build on them. when we must take one (cmd+f), the book says how to free it per terminal.
- **every key has a mouse twin**, and every click a key.
""", src="book §16", k="shortcuts keys digits ctrl+1 keyboard")

page("patterns/feedback", "show it, then fold it",
"every action leaves a trace you can see for a moment, where your eyes already are, then gets out of the way.",
r"""
| what happened | where it shows | how long |
|---|---|---|
| you switched mode | the mode word in accent on the divider, its words on the key bar | 3 s |
| you answered an item | a fold line in the box, then the next item | 2 s |
| the last item | `✓ inbox clear` on the divider, your draft back | 2 s |
| a setting changed | the row flashes `✓` | 1 flash |
| you copied | `✓ copied 9 chars` on the divider | 2 s |
| your message | `·` → `✓` → `✓✓` at its end | stays |
| an agent finished | its glyph turns `✓`, one line in main's thread | stays |

## rules

- the same fold line lands in main's thread, so the history keeps the decision.
- a flash is a state, never a cheer. no exclamation marks.
""", k="feedback toast flash confirmation after action")

page("patterns/room", "short on room: a written drop order",
"every line that can run out of room has its order written down, and we apply it one step at a time.",
r"""
::ex
::tab 80 columns | w=80
{r:├─} {d:you →} {a:dark-mode} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {d:ψ sb/dark-mode} {f:·} {g5} {>─} {d:58k} {r:─┤}
::tab 58 | w=58
{r:├─} {d:you →} {a:dark-mode} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo} {f:·} {d:ψ} {f:·} {g5} {>─}{r:─┤}
::tab 48 | w=48
{r:├─} {d:you →} {a:dark-mode} {f:·} {d:opus·hi} {f:·} {d:yolo} {f:·} {d:ψ} {f:·} {g3} {>─}{r:─┤}
::tab 32 | w=32
{r:├─} {d:you →} {a:dark-mode} {f:·} {d:yolo} {g1} {>─}{r:─┤}
::cap the same divider as the room shrinks. the gust is the last thing to go: never, while it works.
::end

## the order

1. explanation words first (`working · `, `in the inbox`),
2. long forms become short (`opus 5.5 · high` → `opus·hi`, `∿ 3 working` → `∿ 3`),
3. whole details go (a branch's name, the model tag),
4. **names are cut last**, with `…`, never through a glyph or an arrow,
5. what says "alive" or "needs you" is never dropped: the gust while working, `?`.

::note every new line ships with its drop order, tested at 80 and 150 columns.
""", src="book §8 short on room", k="truncation narrow width responsive cut ellipsis")

page("patterns/errors", "errors",
"one line: what happened, then how to fix it, with the command that fixes it. nothing is lost.",
r"""
::ex
{e:✗} turn stopped: no OpenRouter key yet. {t:/provider} sets it up.{>}
{>}
{e:✗} Mistral says the voice key is wrong. {t:/provider} fixes it.{>}
  {d:401 · Unauthorized}{>}
  {f:your recording is kept: ctrl+r retry}{>}
{>}
{a:?} your Mistral account has no credit yet. add some here:{>}
  {u:console.mistral.ai/billing}{>}
::end

## rules

- `✗` in error color for what broke; `?` in accent when it's your call (no credit: you decide).
- **the fix ends the line**: a command (`/provider sets it up.`), a key, a place.
- the provider's own words go **dim under it**, never instead of it.
- **nothing is lost**: the draft, the clip, the queue are kept, and the line says so.
- bise says "i": `i couldn't reach Mistral`, not "an error occurred".
""", src="book §8 voice, providers", k="error failure message fix")

page("patterns/zen", "zen while you type",
"start typing and everything else fades: the agents, the counts, the chatter. just you and your words. send it and it all comes back.",
r"""
## in and out

- **in**: a key that changes your text (a character, backspace, a paste).
- **holds** while you edit or move inside the composer.
- **out**: 5 s after your last key, or at once on `⏎`, `esc`, a shortcut, the mouse, focus lost, or anything that needs you.

## the look

the chrome goes 45 % of the way to the background: the header, the frame, the panel, the divider's right side, the key bar. the history you read and your text stay as they are. nothing moves, only brightness.

::note why: "parfois j'ai quand même besoin de lire pour pouvoir écrire. mais j'ai pas besoin de voir tout ce qui se passe ailleurs" (the user).
""", src="book §8 zen", k="focus typing fade zen mode")

page("patterns/spaces", "your space, their space",
"your space is raised and carries your pink bar. their space is the ground. you never type in theirs, they never draw in yours.",
r"""
| yours (raised) | theirs (ground) |
|---|---|
| the composer pane | the history |
| an inbox item, open | the agents panel |
| the attachments box, above your text | the agents' tool rows and chips |

## rules

- everything you type or answer happens on the raised tint, with the `│` bar.
- what isn't your text (attachments, queued messages) sits in its own box, so you never think it's part of it.
- the raised tint fills edge to edge inside the frame; the frame's lines stay on the ground.
""", k="raised surface composer ownership")

page("patterns/defaults", "defaults that decide",
"bise has opinions, so you don't need settings. every default says what it resolves to, and every pick is remembered.",
r"""
## rules

- **the first run asks one thing**: which model does the work. every other role says `same as main` or `auto`.
- **never a bare `auto`**: `auto · Mistral · mistral-small-latest`.
- **name what leaves the machine** the first time: `auto sends commands to Jev (TypeSafe) to check them.`
- **remembered**: a pick is saved and survives restarts. never asked twice.
- **one recommended** option at most, and only when we'd really pick it.

::do
- show what a default resolves to, on the same row
- ask once, at the moment it matters
::dont
- add a setting to avoid a decision
- tag something "recommended" because it's ours
::end
""", src="book §8 /models, approvals", k="settings defaults auto same as main config")

# ------------------------------------------------------------------ content
page("content/voice", "voice in the UI",
"bise speaks as i, the user is you. human, short, concrete. lowercase.",
r"""
## rules

- **bise says "i"**: `i'll work in ~/acme`, `i couldn't reach Mistral`.
- **lowercase everywhere**, except proper nouns, acronyms and keys as printed.
- **short sentences.** show the moment, not the feeling.
- **no final period on a title or a key label.** a full sentence keeps it.
- **nothing LLM-y**: no "seamless", no "X, not Y", no "never deleted", no "let's", no exclamation marks.
- **the user's language** for what models write to you (a tool row's description, main's answers). the chrome stays English.

::do
- `✓ inbox clear`
- `i start an agent when a job needs one.`
- `mistral says this key is wrong. copy it again from <link>.`
::dont
- `All done! 🎉`
- `Your agents work seamlessly together.`
- `An error occurred while processing your request.`
::end
""", src="book §4", k="tone voice copy writing")

page("content/words", "vocabulary",
"four nouns to learn: you, main, agents, the inbox. and the same short state words everywhere.",
r"""
## the nouns

| say | never say |
|---|---|
| you | the user, operator |
| main, your team lead | orchestrator, hub, coordinator |
| an agent | task, sub-agent, worker, hand |
| the inbox | cards, notifications |
| a question, an approval | a card, a ticket |
| a worktree | a sandbox, a clone |

## the state words

`working` · `idle` · `done` · `waiting` · `asks you` · `failed` · `starting` · `stopped`

the same word for the same state in the panel, the header, the help and the site.

## units

numbers before units, no fluff: `58k · 22%`, `3m`, `42s`, `4.2 MB`, `1 of 2`.
""", src="book §4", k="words nouns vocabulary glossary terms")

page("content/templates", "message templates",
"the shapes we reuse, so new strings sound like the old ones.",
r"""
| kind | template | example |
|---|---|---|
| error | `✗ <what happened>. <command> <fixes it>.` | `✗ voice needs a key. /voice setup picks one.` |
| your call | `? <what>. <where to do it>:` | `? your Mistral account has no credit yet. add some here:` |
| flash | `✓ <state>` | `✓ inbox clear`, `✓ copied 9 chars` |
| mode | `<mode> · <what it means>` | `auto · safe calls run, risky ones ask you` |
| tip | `<the fact>. <key> <does what>.` | `you're in yolo: everything runs. ⇧⇥ changes it.` |
| picker title | a question | `which provider?`, `how hard should it think?` |
| picker line | what it changes | `you can change it any time with /model.` |
| fold | `▸ <n> <things>` | `▸ 12 more lines`, `▸ 6 commands` |
| item head | `? <agent> <wants to / asks>` | `? t3 wants to run` |
| answered | `✓ you <verb> <agent>: <what>` | `✓ you allowed t3: npm publish --access public` |
| hard rule | `<what it does>. this one always asks.` | `it rewrites main. this one always asks.` |
""", src="book §17 copy deck", k="copy strings templates error flash tip")

# ------------------------------------------------------------------ process
page("process/design", "how we design a change",
"from the user's words to a signed-off capture. nothing is built before he picks.",
r"""
1. **start from the user's words**, quoted in the issue. the design answers them, nothing more.
2. **mock it on the real geometry**: a local page drawn with the TUI's real columns, colors and rows. a screenshot-level mock, not a sketch.
3. **variants only where we hesitate**, the pick clearly marked, with one line of why.
4. **the user picks.** nothing is built before.
5. **the builder sends real captures**: tmux, 80 and 150 columns, ctrl up and held. the designer signs off, or lists the changes.
6. **the book gets the final spec**, with the user's words and the issue number. this site follows.
""", k="process workflow mock review sign off")

page("process/checklist", "checklist",
"twelve questions before a new screen ships.",
r"""
1. does it block typing, sending or switching agents? it must not.
2. what is the one key for the common case?
3. which level is it: needs you, for you, between agents?
4. what does it show at rest, and what only with ctrl held?
5. is any color used for something that doesn't need the user?
6. does any glyph mean two things? is it one cell in every font?
7. does every shown key work in this terminal? what's the fallback?
8. what shows right after the user acts, and for how long?
9. what's the drop order at 80 columns?
10. dark, light, NO_COLOR, ASCII: all drawn?
11. every word: lowercase, short, one of the four nouns, no LLM tells?
12. on failure: one line, what happened, the command that fixes it, nothing lost?
""", k="checklist review ship")

page("process/open", "open questions",
"what we haven't settled yet.",
r"""
## the inbox's mark

the panel shows `@ 2` on main's row and the header says `# 1 in the inbox`. `@` is also the file key on the key bar, and `#` a pull request's number. proposal: bring back `✉︎` (its text form, one cell) for the inbox only, in its own issue.

## light theme syntax colors

the dark syntax colors are set; the light ones are still to pick.

## pull requests

PR support is designed (the `↑` mark, the ready-to-merge item), not built yet. its pages here say *designed* until it ships.
""", k="open questions todo undecided")

PAGES = P
