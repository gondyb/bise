# bise — issues

The work list for [the bise book](bise-book.md). One section per issue. An
implementer edits **only the sections of its own issues**: `status`, `owner`,
`commits`, `notes`. Everything else changes only through main.

Status values: `todo` · `in progress` · `blocked (why)` · `done`.

Index:

| ID | Title | Wave | Track | Depends on |
|---|---|---|---|---|
| BISE-01 | theme tokens and the two palettes | 0 | T | — |
| BISE-02 | theme auto-detection and `/theme` | 1 | T | 01 |
| BISE-03 | glyph audit | 0 | G | — |
| BISE-04 | hub line protocol v2 (levels, peer traffic) | 0 | H | — |
| BISE-10 | reading width | 1 | F | 01 |
| BISE-11 | scripts in full | 1 | F | 10 |
| BISE-12 | progressive disclosure | 1 | F | 11 |
| BISE-13 | entity glyphs in the feed | 1 | F | 12, 03 |
| BISE-14 | the three levels, folding, time marks | 2 | F | 04, 13 |
| BISE-15 | message marks `·` `✓` `✓✓` | 2 | F | 04, 14 |
| BISE-20 | agents panel | 1 | P | 01 |
| BISE-21 | header row | 1 | P | 20 |
| BISE-22 | status row, composer, first run | 1 | P | 21 |
| BISE-30 | cards as level 1 | 1 | C | 01 |
| BISE-31 | answered cards (fade in place) | 2 | C | 30, 14 |
| BISE-40 | remove undo (TUI) | 1 | K | — |
| BISE-41 | help and commands: words and case | 2 | K | 40, wave 1 done |
| BISE-42 | new keys | 2 | K | 12, 41 |
| BISE-50 | no undo in the hub, corrections by talking | 1 | M | — |
| BISE-51 | main's voice, summaries, "why" | 1 | M | 50, 04 |
| BISE-60 | onboarding flow | 2 | O | 01, 02 |
| BISE-61 | one-time hints | 2 | O | 60, 14, 20, 30 |
| BISE-70 | images UI | 2 | I | 22 |
| BISE-80 | vocabulary sweep | 3 | S | waves 1–2 |
| BISE-81 | lowercase and copy-deck sweep | 3 | S | 80 |
| BISE-82 | visual QA | 3 | S | 81 |
| BISE-83 | remove deprecated theme aliases | 3 | S | 81 |
| BISE-84 | glyph fallbacks and `BISE_ASCII` | 1 | T | 01, 03 |

---

## Wave 0 — foundations

### BISE-01 · theme tokens and the two palettes

- **status:** done · **owner:** bise-t-theme · **commits:** c1de22a
- **track:** T · **owns:** `rust/tui/src/theme.rs`
- **spec:** book §5, §6, contract C1 (§21)
- **do:**
  - Replace the OpenCode theme with role functions (`text()`, `dim()`,
    `faint()`, `accent()`, `error()`, `ok()`, `selection_bg()`,
    `card_tint()`, the syntax roles) backed by two palettes, dark and light
    (§5 table), switched by `set_mode(Mode)`; default dark.
  - Never paint the background: roles return foreground colors; the
    background is `Color::Reset`. Keep the old constants as
    `#[deprecated]` aliases mapped to the closest role (`BRAND` → accent,
    `WARN` → accent, `ERR` → error, `PANEL` / `ELEMENT` → `Color::Reset`, …)
    so no other file breaks.
  - Add the glyph constants of §6 (`G_YOU = "›"`, `G_MAIN = ":*"`,
    `G_WORKING = "∿"`, `G_DONE = "♡"`, …) and a `working_frame(tick)` that
    pulses `∿` (bright / dim) instead of the braille spinner.
  - Pick the **light syntax colors** (≥ 4.5:1 on white) and write them in the
    notes; main copies them into the book.
- **done when:** the TUI builds with no change in other files; `set_mode`
  switches every role; a unit test checks every readable role ≥ 4.5:1
  against white / `#f7f4ee` (light) and black / `#141211` / `#282c34` (dark).
- **notes:**
  - **API (C1, as built):** `Mode { Dark, Light }`, `set_mode(Mode)`, `mode()`;
    roles `text() dim() faint() accent() error() ok() on_accent() bg()
    selection_bg() card_tint() syntax_keyword() syntax_string()
    syntax_comment() syntax_number() syntax_call() syntax_type()`; the raw
    palettes `DARK` / `LIGHT` / `palette_of(Mode)` / `palette()` (for tests).
    `bg()` is always `Color::Reset`. Extra roles vs the book: `on_accent`
    (text on an accent chip / selected popup row: `#1b1917` dark, white
    light), `syntax_type`.
  - **Glyphs §6:** `G_YOU G_MAIN G_BRIEF G_THINK G_BASH G_TS G_SUBCALL
    G_PATCH G_MSG G_IMAGE G_CARD G_COMPACTING G_SUMMARY G_INTERRUPTED`,
    status `G_STARTING G_WORKING G_WAITING G_NEEDS_YOU G_DONE G_FAILED G_IDLE
    G_STOPPED`, marks `G_SENDING G_RECEIVED G_READ G_UNREAD G_WORKTREE
    G_OVERLAP G_RESTART_FAILED G_BUILDING G_CLOSED G_OPEN`.
    `working_frame(tick) -> (&str, Color)`: `∿` text/dim, one phase every 4
    ticks; `starting_frame(tick)`: `·` dim/faint. `SPLIT` kept (not deprecated).
  - **Light syntax colors (for the book §5):** keyword `#8a3fb0`, string
    `#44782a`, comment `#726b60`, number `#9a4a0c`, call `#1f63a8`, type
    `#7a5c00` (all ≥ 4.5:1 on white and `#f7f4ee`). Dark type: `#e8cf9a`.
  - **Book correction:** the dark comment `#857e74` is only 3.5:1 on
    `#282c34`; lifted to `#99928a` (4.56:1). Tints: selection `#33292c` dark /
    `#faeef0` light, card `#211d1b` dark / `#f3eee6` light (text, dim and
    accent ≥ 4.5:1 on them; on_accent ≥ 4.5:1 on accent).
  - **Contrast test:** `theme::tests` (7 tests) — every readable role ≥ 4.5:1
    on white/`#f7f4ee` (light) and black/`#141211`/`#282c34` (dark), the
    tints, faint quieter than dim, `set_mode` switches every role, the pulse,
    the aliases. All pass.
  - **Deprecated aliases:** fixed to the **dark** palette (they do not follow
    `set_mode`): `BRAND WARN RECORDING` → accent, `ERR` → error, `OK` → ok,
    `TEXT` and also `ACCENT` (markdown headings) and `HEAD` (emphasis, bash
    options, code types) → text, `DIM TOOL INFO` → dim, `FAINT BORDER_ACTIVE`
    → faint, `SELECTION` → selection_bg, `ON_BRAND` → on_accent, `SYNTAX_*` →
    syntax dark, `PANEL ELEMENT DIFF_ADD_BG DIFF_DEL_BG` → `Color::Reset`.
    `SPINNER`/`spinner_frame` (braille) and the old `GLYPH_*` keep their values
    and are deprecated too (BISE-13 moves the feed to §6). So the TUI already
    wears the dark bise colors, with no panel/code/diff backgrounds.
  - **Warnings:** one line outside my file — `#![allow(deprecated)]` at the
    top of `rust/tui/src/lib.rs` (~250 use sites through `use theme::*`);
    BISE-83 removes it. theme.rs has `#![allow(dead_code)]` until wave 1 uses
    the roles/glyphs. Note: the crate-wide allow also hides other
    deprecations until BISE-83.
  - **For wave 1:** call the role functions, never the aliases (they stay
    dark in light mode). Mode is a process-wide atomic; under `cargo test` it
    is per thread, so a test may `set_mode(Mode::Light)` without repainting
    its neighbours. BISE-02 only needs to call `set_mode`.
  - **Gates:** cargo build ok; bend-tui tests 180 ok; clippy --workspace
    --all-targets clean; PROOF ALL PROOFS CHECK. At commit time the shared
    tree had 6 failing `switchboard core::tests` and 2 failing tmux tests
    (`tui_tmux` `◀ t1 m_`, `tui_composer_tmux`) from BISE-04 in progress; in a
    clean worktree (HEAD + my two files) all tmux tests and e2e pass.
    Visual check in Ghostty/light terminal not done (light is reachable only
    after BISE-02).

### BISE-03 · glyph audit

- **status:** done · **owner:** bise-g-glyphs · **commits:** see `git log -- projects/switchboard/docs/brand/glyph-audit.md`
- **track:** G · **owns:** new `projects/switchboard/docs/brand/glyph-audit.md`
- **spec:** book §6
- **do:** check every glyph of §6 (and `↪ ▸ ▾ ┃ │ ─`) for width 1 and
  presence in SF Mono, Menlo, JetBrains Mono, Fira Code, Cascadia Code, and in
  Ghostty, iTerm2, Terminal.app, kitty, WezTerm (what can be checked on this
  Mac; mark the rest "not checked"). Use `unicode-width` for the width and a
  screenshot per terminal. Propose a fallback for each glyph that fails
  (e.g. `∿` → `~`, `♡` → `<3`?).
- **done when:** the audit file has one table glyph × font/terminal, and a
  fallback list; main updates §6 if a glyph changes.
- **notes:**
  - Width: every glyph is 1 cell in `unicode-width` 0.2.0/0.1.14 and
    measured 1 cell in Ghostty 1.3.1 and Terminal.app (cursor-position probe).
  - Fonts: 17 of 33 glyphs are missing from at least one font. `∿ ⧗` are in
    none; `⟳ ⎇` only in Fira; `✉ ⇄ ↻ ♡` only in Menlo/Meslo. `✉ ↪` can
    become color emoji.
  - Proposed §6 changes (main/Gabriel decide): `∿`→`~`, `⧗`→`Δ`,
    `⟳`/`≡`→`Σ` (pulsing/still), `⎇`→`⌥`, `✉`→`@`, `⇄`→`↔`, `↻`→`!`,
    `♡`→`✓` (brand call), `∴`→`≈`, `↳`→`└`, `▣`→`■`, `◇`→`◊`, `✗`→`×`,
    `↪`→`»`; keep `▸ ▾`.
  - Not checked: iTerm2 (did not run the probe), kitty and WezTerm (not
    installed), screenshots (no Screen Recording permission).
  - The user's Ghostty asks for FiraCode, which is not installed, so it
    uses its built-in JetBrains Mono.

### BISE-04 · hub line protocol v2 (levels, peer traffic)

- **status:** done · **owner:** bise-h-protocol · **commits:** ae4437f
- **track:** H · **owns:** `rust/switchboard/src/core.rs`, `daemon.rs`,
  `transcript.rs`, `core_tests.rs`; `rust/tui/src/wire.rs` (the `Ev` enum
  only); `rust/tui/src/sb.rs` (`parse_hub_line` only)
- **spec:** book §9, contract C2 (§21)
- **do:**
  - Hub: write `sb msg : {from} → {to} : {text}` for every message between
    agents, **including between two agents that aren't main**, into main's
    feed; `sb msg-you : {from} : {text}` when an agent writes to the user;
    `sb answered : {agent} : {question} : {answer} : {why}` when main answers
    an agent's question (from `sb send --reply-to` by main on a question the
    task asked). Keep the old kinds.
  - TUI: extend `Ev::AgentMsg` to `{ from, to, text, level }` and add
    `Ev::Answered { agent, question, answer, why }`; `parse_hub_line` maps v1
    and v2 kinds. Rendering stays as today (the F track restyles in
    BISE-14): map the new variants to the current look so nothing breaks.
  - `transcript.rs`: the new kinds round-trip.
- **done when:** `core_tests.rs` has a test per new kind (including peer
  traffic reaching main's feed); an old transcript still renders; the TUI
  shows peer messages in main (old look is fine).
- **notes:**
  - **Wire format as built** (C2 + one amendment main accepted):
    `sb msg : {from} → {to} : {text}` (level 3) goes into main's feed at
    send time for every agent message main does **not** receive: between
    two tasks and main → task (incl. briefs, reports between tasks, auto
    replies). What main receives stays v1 `sb msg-in : {from} m_<n> :
    {text}`, written when it is delivered (no duplicate). `sb msg-you : {from} :
    {text}` (level 2) is a task's end-of-turn reply to `@task …` typed in
    another view (was `msg-in : @task : …`). `sb answered : {agent} :
    {question} : {answer} : {why}` (level 2) goes into main's feed instead of the
    `msg` line when main replies (`--reply-to`, or the implicit open
    question) to a message where the task asked main (`expect_reply`),
    including main's auto reply. Inside an `answered` field, `" : "` is escaped as
    `" \: "` (core.rs `field_escape`; `parse_hub_line` and transcript.rs
    undo it; a literal `" \: "` in a text would read back as `" : "`).
    `why` comes from a new `sb send --why "<reason>"` (cli.rs, taken with
    main's OK); empty when absent. **C2 amendment:** `Ev::AgentMsg` has
    an extra `id: String` (`m_3` from `msg-in`, empty otherwise) so the
    current look `◀ t1 m_3` and the tmux tests stay. Book §21 to update.
  - **Ev mapping:** `msg` → `AgentMsg{from,to,text,level:3,id:""}`;
    `msg-in` → `{from, to:"" (= the feed owner), level 3, id}`; old
    `msg-in : @x : …` and `msg-you` → `{to:"you", level 2}`; `answered` →
    `Ev::Answered{agent,question,answer,why}`. Other kinds are unchanged.
  - **Bend:** hub/core.bend: `send` now = `send_why(…, "")`, which adds
    `feed_main` after a successful send; `req_send` passes `q.why`;
    `auto_reply` writes `msg-you`; `for_msg_of` moved up; new fx field
    `fields` on `line` (the daemon joins and escapes them). This only adds lines, with no
    state change, so no law was touched; PROOF: ALL PROOFS CHECK. sb-core rebuilt
    (not stripped).
  - **Outside my list (main's OK):** render.rs (AgentMsg arm + helper
    `agent_msg_rows`, Answered arm, one test), cli.rs (`--why`).
    daemon.rs unchanged.
  - **For M (BISE-51):** prompts.rs still documents `sb send` without
    `--why`; main should use `--why` when it answers for the user.
  - **For F (BISE-14):** main's feed now gets many level-3 lines: the
    brief of each new task shows as `main → t1` (long), and main's own
    sends also show as its bash tool call (a duplicate to fold).
    Current look: head `from → to`, `from to you` (level 2), `from m_3`
    (msg-in); `answered` is drawn as a message block "main answered @x".
  - **Checked:** core_tests (peer traffic in main's feed, main → task,
    one line for what main receives, answered with/without why and the
    escape, a reply to a non-question stays `msg`, msg-you); transcript
    test (v2 kinds + v1); TUI tests (v2 parse, v1 parse + draw of an old
    main feed, peer/answered draw); throwaway hub (e2e.Env + tmux,
    `/tmp/bise_h_peer_tmux.py`): `◀ t1 → t2 / peer-hello-from-t1` in
    main's feed. Gates green: cargo build, test --workspace, clippy
    (0 warnings), run_all.sh, PROOF.
  - **Gotcha:** a task's shell inherits `SB_CORE_BIN` (the live hub's
    versioned sb-core); unset it (and SB_SOCKET…) before `cargo test`,
    or the hub tests run against the live binary.

---

## Wave 1 — parallel tracks

### BISE-02 · theme auto-detection and `/theme`

- **status:** todo · **owner:** — · **commits:** —
- **track:** T · **owns:** `rust/tui/src/term.rs`, new `theme_detect.rs`;
  the `/theme` entry in `commands.rs` **is done by K in BISE-41** (write the
  function it calls here)
- **spec:** book §5
- **do:** at start, before the alternate screen, query the terminal
  background with OSC 11 (`\x1b]11;?\x07`), read the `rgb:` reply with a
  short timeout (~100 ms), pick light if the luminance > 0.5, else dark; call
  `theme::set_mode`. An env var `BISE_THEME=light|dark|auto` forces it
  (**⚠** name to confirm with main). No answer (tmux without passthrough,
  dumb terminal): dark. Expose `theme_detect::apply(choice)` for `/theme`.
- **done when:** tested by hand in Ghostty dark, a light Terminal.app profile,
  and tmux; unit test of the reply parser (`rgb:ffff/ffff/ffff`, 2- and
  4-digit hex, garbage).
- **notes:**

### BISE-10 · reading width

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** `render.rs`, `feed.rs`, `code.rs`, `markdown.rs`,
  `feed_render_tests.rs`
- **spec:** book §11 (measure)
- **do:** prose (user lines, assistant text, reports, agent messages) wraps
  at `min(width − margins, 76)`; code (scripts, diffs, outputs) up to 100
  columns, longer lines wrap with a hanging indent and a faint `↪`. The
  extra width is left empty (the feed doesn't stretch lines).
- **done when:** render tests at widths 60, 100, 160 show prose ≤ 76 and code
  ≤ 100; the 50k-line bench (`sb/bench.rs`) is not slower.
- **notes:**

### BISE-11 · scripts in full

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** as BISE-10
- **spec:** book §11 (scripts in full)
- **do:** bash and `run_typescript` sources are never folded
  (`CODE_FOLD_AT` / `CODE_FOLD_SHOW` in `render.rs` no longer apply to
  them); syntax colors from the theme roles. Other long code (e.g. a huge
  patch) still folds behind `▸`.
- **done when:** a 200-line script renders whole in a test; colors come from
  `theme::` roles.
- **notes:**

### BISE-12 · progressive disclosure

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** as BISE-10 + `feedsel.rs`
- **spec:** book §11 (table)
- **do:** one-line collapsed forms and their disclosed forms for: tool
  outputs / results (`▸ output · 42 lines · 1 failed`), file edits
  (`± edit path ✓ +3 −1 ▸`), reports in main (`♡ … ▸ report`), the brief
  inside an agent (`◇ brief ▸`). Thinking keeps today's behavior. Toggle one
  item on click and on `space` when the feed selection is on it (expose
  `feed::toggle_selected(app)`), and all outputs (`feed::toggle_all_outputs`).
  **Don't bind keys** (K does it in BISE-42).
- **done when:** render tests for each collapsed / open form; the two
  functions exist and are tested.
- **notes:**

### BISE-13 · entity glyphs in the feed

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** as BISE-12
- **spec:** book §6 (entities), glyph audit (BISE-03)
- **do:** use the §6 glyphs in the feed: `›` you, `:*` main, `$` bash, `λ`
  TypeScript, `↳` sub-call, `±` edit, `∴` thinking (replaces `✦`), `⟳` / `≡`
  compaction, `▲` interrupt, `✗` failure; fallbacks from BISE-03. Replace
  the `◆ carte` line of `Ev::Card` by the level-1 look (accent bar, `?
  {name} needs you`).
- **done when:** render tests updated; screenshots of the feed match
  `tui-screens.html` screens "inside an agent" and "everything disclosed".
- **notes:**

### BISE-20 · agents panel

- **status:** todo · **owner:** — · **commits:** —
- **track:** P · **owns:** `sb/panel.rs`
- **spec:** book §8 (agents panel), §6 (status)
- **already there:** the archived row from 85160ab (`▸ {n} archived`, `A`,
  `/archived`, read-only history); keep its behavior, restyle it.
- **do:** title `agents · ⌥ + number`; each row: number (faint, 0–9, blank
  after), status glyph (§6, `∿` pulsing), name, then right-aligned age and
  context fill, or `you` (accent) / `done` / `waits {name}` / `starting`;
  marks `•` and `⎇`; `+ {n} more` when it overflows.
  Numbers follow creation order and never change while an agent lives. The
  word "task" disappears from the panel.
- **done when:** tests for the row layout at panel widths 28 and 40; no
  wrap of the title at 28.
- **notes:**

### BISE-21 · header row

- **status:** todo · **owner:** — · **commits:** —
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (header)
- **do:** one row on top: `bise :*` (`:*` accent) left; right, the non-zero
  counts `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done` (`? … needs
  you` in accent), or `no agents yet`. Under 70 columns (no panel) the
  counts shorten to `∿ 3 · ? 1 · ♡ 1`. Remove the old ` Switchboard ` title.
- **done when:** render test of the header at 60 and 120 columns.
- **notes:**

### BISE-22 · status row, composer, first run

- **status:** todo · **owner:** — · **commits:** —
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (status row, composer, first run), §17
- **do:** status row starts with the agent in view in accent, the rest dim,
  lowercase (`main · idle · 210k / 1M tokens · 21%`); composer hints from
  the copy deck; the first-run text when there are no agents; the "inside an
  agent" line (`you're talking to {name} directly…`).
- **done when:** screenshots match `tui-screens.html` screens "first run",
  "inside an agent"; strings match §17 exactly.
- **notes:**

### BISE-30 · cards as level 1

- **status:** done · **owner:** bise-c-cards · **commits:** b5c4e52
- **track:** C · **owns:** `sb/cards.rs`
- **spec:** book §12, §9 (level 1)
- **do:** the card box uses the level-1 look (accent border / bar, `?
  {name} needs you`, body wrapped at 76, choices, dim keys `alt+r answer
  with text · ctrl+x later · ctrl+f full screen`); kind glyphs `?` `✗` `↻`
  `–` `⇄` `♡` (`kind_look`); keep `ctrl+f` full screen and all current keys;
  lowercase.
- **done when:** tests for each kind; screenshots match "cards: a question",
  "cards: every kind", "a card, full screen".
- **notes:**
  - **look:** a box with the kind's color on the border, the left side a
    heavy bar `┃` (corners `┎` `┖`, right side rounded `╮` `╯`). Title
    `{glyph} {title}` bold in the kind's color, then dim `· 1 of 3 · 2m`
    (`1 of 3` only with 2+ cards; full screen: `· full screen · ctrl+f
    back`). Text in `text()`, wrapped at 76; the hub's note dim italic
    (the `ⓘ` is gone). Keys dim at the bottom left, scroll state dim at
    the bottom right: `▾ {n} more lines · pgdn`, then `end · pgup`
    (short form `▾ {n}` when narrow). The scrollbar, the 70% cap, the
    wheel/arrows, `ctrl+f`, `/close` completion and every key are kept.
  - **kinds (`kind_look`, theme `G_*`):** question `? {name} needs you`
    and blocked `? {name} is blocked` (accent); failed `✗ {name} failed`
    and restart `↻ restart failed` (error); drop `– drop {name}?`,
    overlap `⇄ overlap`, done `♡ {name} is done` (glyph `text()`, border
    `faint()`). Order unchanged: question, blocked, failed/restart,
    drop, overlap, done, then oldest.
  - **choices:** the hub has no field for them, so the TUI reads them
    from the text: a run of 2–9 last lines `1. x` / `1) x` / `1 - x`
    numbered from 1 becomes `1 x   2 y` (number accent bold) under the
    text; answer by typing the number then `alt+r`. Not for done/overlap
    (a done summary keeps its numbered list). A real `choices` field in
    the card snapshot (hub + C2) would be cleaner: later, if wanted.
  - **keys line:** `alt+r answer with text · ctrl+x later · ctrl+f full
    screen`, + `ctrl+n next` with 2+ cards; `alt+r got it` for done and
    overlap; full screen drops `ctrl+f full screen` (it is in the title).
    What does not fit is cut from the right.
  - **vs the mockups:** the keys sit on the bottom border, not on the
    choices row (they stay visible when the text scrolls); the question
    box spans the feed width (text still wraps at 76); blocked says
    `is blocked` (every-kind mockup), not `needs you`; `drop` says `alt+r
    answer with text` (the hub's drop card is answered yes/no by text,
    not `y`/`n`); failed/restart/drop/overlap have no `▸ report` link.
  - **not mine, seen:** the hint row under the composer
    (`hint_text`, ui/render) still says `Alt+R answer … Ctrl+N/P card …`
    in capitals (for K/S). `Card` now has `Default` (bise-p-chrome's panel
    test uses it). `ago()` stays `#[allow(dead_code)]` until BISE-20
    drops its last caller; then remove it.
  - **gates:** run in a private worktree of 7d07b05 + cards.rs (shared
    tree has other tracks' WIP), `CARGO_TARGET_DIR=/tmp/bise-gate-target`,
    `SB_CORE_BIN` = the worktree's sb-core: build, `cargo test
    --workspace`, clippy `--all-targets` (no warnings), `run_all.sh` all
    green. No .bend change.

### BISE-40 · remove undo (TUI)

- **status:** done · **owner:** bise-k-keys · **commits:** 1965b91
- **track:** K · **owns:** `sb.rs` (the `ctrl+z` key arm only), `help.rs`
- **spec:** book §13 (no undo), §16
- **do:** remove the `ctrl+z` → `/cancel` binding and its help row. The hub
  side is BISE-50.
- **done when:** `ctrl+z` does nothing in the switchboard client (the
  composer's own undo, `cmd+z` / `ctrl+/`, is untouched); help tests pass.
- **notes:**
  - Removed in `sb.rs`: the `ctrl+z` → `/cancel` key arm and the `/cancel`
    entry of `SB_COMMANDS`; in `help.rs`: the `Ctrl+Z` row. `ctrl+z` now
    falls through to the composer, where `editor::action` maps it to
    nothing (undo is `cmd+z` / `ctrl+/` only), so it does nothing. The
    terminal is raw, so no SIGTSTP either.
  - New test `sb::nav_key_tests::ctrl_z_does_nothing` (the key is not
    consumed, the editor has no action for it, the draft is untouched,
    `/cancel` is not in the command list).
  - No tmux test asserted on `/cancel` or `Ctrl+Z`: no assertion changed.
  - Hub side (`UserCmd::Cancel`) is BISE-50 (bise-m-main); until it lands a
    typed `/cancel` still reaches the hub as input.
  - Gates in a private worktree of HEAD + my files (shared tree had other
    tracks' work): cargo build, cargo test --workspace (with
    `SB_CORE_BIN` unset), cargo clippy --workspace --all-targets
    -D warnings, e2e.py and the 9 tmux tests of run_all.sh: all green. No
    `.bend` file changed.

### BISE-50 · no undo in the hub, corrections by talking

- **status:** done · **owner:** bise-m-main · **commits:** 1ba9ff7, c03071e
- **track:** M · **owns:** `rust/switchboard/src/router.rs`, `prompts.rs`
- **spec:** book §13 (no undo)
- **do:** remove `/cancel` (`UserCmd::Cancel`) and its tests; in main's
  prompt: when the user changes their mind, send the agent an explicit
  correction (`the user changed their mind: …`) and confirm to the user in
  one line (`told {name}: …, you changed your mind.`).
- **done when:** router tests updated; a live check on a throwaway hub
  (`SB_DEV_ROOT=/tmp/…`): "no, v1 for docs" after main answered v2 produces
  the correction message and the one-line confirmation.
- **notes:**
  - **Ownership change (main's OK):** removing `Cancel` touched H's files
    after BISE-04 landed: `core.rs` (match arm, `ClientView.last_route`,
    the `routed` fx arm, the HELP line), `core_tests.rs` (the cancel test
    becomes `there_is_no_undo_a_route_stays_sent`), `hub/core.bend`
    (`ICancel`, `cancel.*`, the `"cancel"` input, the `routed` fx that only
    fed `last_route`), `sb-core` rebuilt (not stripped). The TUI side
    (`sb.rs` `/cancel` command entry, `ctrl+z`) is K's (BISE-40).
  - `/cancel` and `/undo` now parse to `Invalid(router::NO_UNDO)`: the
    notice `no undo: an agent may already have acted. say the change to
    main instead ("no, v1 for docs").` (**new string**, for §17).
  - Kept on purpose: `M.Cancelled{}` message state in `model.bend` and the
    `"cancelled"` codec: old journals may hold it. Nothing produces it now.
  - Main's prompt: a "There is no undo" rule: explicit correction
    `sb send <task> "the user changed their mind: <new>, not <old>."`, then
    one line to the user `told <task>: <new>, you changed your mind.`;
    never offer an undo. Test `main_corrects_by_talking_never_by_undo`.
  - PROOF: ALL PROOFS CHECK (no law mentioned cancel).
  - **Live check** (real model, throwaway hub `/tmp/m-cq84g8tt`): docs
    asked main `v1 or v2?`, main answered `v2` (`sb answered : docs : v1 or
    v2? : v2 : `). Then `no, v1 for docs` →
    `sb send docs "the user changed their mind: v1 for docs, not v2. Update
    docs.txt … then report done."` and main said: `Told \`docs\` you changed
    your mind: you want v1, not v2. It's working on \`docs.txt\` now and
    hasn't reported back yet.` docs.txt ended as `version: v1`. Works;
    the confirmation is capitalized and two sentences, not the one-line
    `told docs: v1, you changed your mind.` (voice is BISE-51).
  - **Gates** (private worktree of HEAD + my files, shared
    `/tmp/bise-gate-target`, built from the worktree): cargo build, test
    --workspace, clippy --workspace --all-targets (0 warnings),
    run_all.sh (e2e + tmux all PASS), PROOF: all green.
  - **Gotcha, sb-core:** my first local rebuild of `sb-core` was flagged
    (likely the endpoint security): any process that read it (git add,
    cmp, git status) got SIGKILLed. Removed it, put HEAD's back, rebuilt
    once cleanly into /tmp (`bend hub/main.bend -o …`, unstripped),
    readable, committed alone in c03071e. If `git status` dies with
    exit 137, look for a flagged binary in the tree.

### BISE-51 · main's voice, summaries, "why"

- **status:** done · **owner:** bise-m-main · **commits:** c9bf346
- **track:** M · **owns:** `prompts.rs`
- **spec:** book §4 (voice), §9 (summaries, why), §13
- **do:** main's prompt: speak as "i", lowercase, short, human (§4); say who
  takes what when routing (`on it: auth-fix takes …`); after a burst of
  agent traffic, one summary line for the user; when answering an agent on
  the user's behalf, use `sb send --reply-to` so the hub emits `answered`
  (BISE-04) with a one-sentence `why`; never promise an undo.
- **done when:** 5 scripted conversations on a throwaway hub read like the
  mockups (`tui-live.html`); paste them in the notes. **⚠** Model behavior,
  not a guarantee: note what fails.
- **notes:**
  - Main's prompt gets a "how you talk to the user" block: "i" / "you",
    lowercase starts (proper nouns and acronyms keep capitals), 1–3 short
    sentences, no filler; "agents" and "cards" to the user, never task /
    sub-agent / hub / orchestrator; routing in one line `on it: auth-fix
    takes …, release takes …`; one summary line per burst of agent
    traffic; answering for the user = `sb send <agent> --reply-to <id>
    --why "<one sentence>" "<answer>"` then `docs asked v1 or v2; the
    brief says v2, so i answered.`; same style in the user's language.
    `--why` is also listed in main's own commands (not in `SB_COMMANDS`,
    tasks don't need it). Test `main_speaks_as_i_routes_summarizes_and_says_why`.
  - **5 scripted conversations** (real model, throwaway hubs
    `/tmp/m-_uxk1x0p`, `/tmp/m-viohgygj`, `/tmp/m-wc8nlg3f`; script
    `/tmp/m-live/live.py`, main's visible lines only):
    1. routing — you: `two things: make test_login.sh not flaky (it
       should never sleep), and draft a 3-line release note for v0.2 in
       NOTES.md` → main: `on it: login-flaky makes test_login.sh stable
       with no sleeps, and release-note writes the 3-line v0.2 note in
       NOTES.md.` then, per report: `login-flaky fixed it: a random 0–2 s
       sleep before \`echo ok\` was causing the flakiness. it removed the
       sleep, and the test passed 20 out of 20 runs. nothing is committed.
       release-note is still writing the v0.2 note.` / `release-note wrote
       the v0.2 note in NOTES.md, so both jobs are done. …`
    2. status — you: `what's going on?` → `both jobs are done and nothing
       needs you. test_login.sh no longer sleeps and passed 20 out of 20
       runs, and NOTES.md has the 3-line v0.2 note. nothing is committed
       yet, so tell me if you want a commit.`
    3. answering for you — docs asks `v1 or v2?`, the user said v2 →
       main: `sb send docs --reply-to m_3 --why "the user said they want
       v2 and told me to answer this myself" "v2"` (hub: `sb answered :
       docs : v1 or v2? : v2 : the user said …`), then `docs asked whether
       to document v1 or v2. you'd already said v2, so i answered v2.`
    4. correction — you: `no, v1 for docs` → `sb send docs "the user
       changed their mind: document v1, not v2. rewrite docs.txt … then
       report done."` and `told docs to switch to v1 instead of v2 because
       you changed your mind. it'll rewrite docs.txt to \`version: v1\`
       once it finishes its current turn.` docs.txt ends `version: v1`.
    5. agents talking to each other — schema + client, client asks
       schema directly (hub: `sb msg : client → schema : what type is
       org_id?`, `sb msg : schema → client : string`) → main: `on it:
       schema writes schema.json and answers questions about it, and
       client asks schema for the type and writes client.txt without
       reading any files.` … `Correction to my last line: both agents are
       already done. … I checked both files and they're right, so nothing
       needs you.`
  - **What fails (model, not code):** the confirmations follow the idea
    but not the exact shape (`told docs to switch to v1 instead of v2
    because you changed your mind.` vs `told docs: v1, you changed your
    mind.`); a capital slips in now and then (`Correction to…`, `I
    checked`); a summary is one line per main turn, and each report
    wakes main, so a burst of N reports can still give N lines (the
    folding of BISE-14 has to carry that); main sometimes speaks from a
    stale picture (`client is asking it now` when both were done) and
    corrects itself next turn; main closes the "done" cards itself and
    once said it did so "without reading what it asked first" (card
    closing is not in this issue). Visible text never said "task".
  - **Gates** (private worktree of HEAD + prompts.rs, shared
    `/tmp/bise-gate-target`, `SB_CORE_BIN` set to the worktree's): build,
    test --workspace, clippy (0 warnings), PROOF, run_all.sh green. The
    first tmux run failed (`'Switchboard' not on screen`) from artifact
    mixing in the shared target; after touching the sources and
    rebuilding, all tmux tests PASS.

---

## Wave 2 — builds on wave 1

### BISE-14 · the three levels, folding, time marks

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** as BISE-12
- **spec:** book §9, §10
- **do:** render `Ev::AgentMsg` by level: level 3 dim under a faint rail
  `✉ from → to  text` with names padded to 10 columns; level 2 `✉ name to
  you: …`; `Ev::Answered` as a main line with `▸ why`. Fold runs of level 3
  longer than 3 into `▸ {n} messages between {k} agents` (open in place, in
  order); only the last run may grow (pulsing `∿`); a level-1/2 line closes
  it. Time mark `· hh:mm ·` after 5 minutes without a line. The feed stays
  append-only: no regrouping.
- **done when:** render tests: a run of 12 folds, a level-2 line closes the
  run, a closed run never changes, time marks; the 50k-line bench is not
  slower; screenshots match "what's for you, what isn't" and "a busy hour, 30
  agents".
- **notes:**

### BISE-15 · message marks `·` `✓` `✓✓`

- **status:** todo · **owner:** — · **commits:** —
- **track:** F · **owns:** as BISE-12 + `wire.rs` (steering lines only)
- **spec:** book §13 (steering and marks), contract C3
- **do:** `steering_received:` / `steered:` set a mark on the last `Ev::You`
  with the same text instead of an info line; draw `·` (sent, not yet
  received), `✓` faint, `✓✓` accent at the end of the user line; a message
  at idle shows `✓✓` when its turn starts. `✗ not delivered: {name}
  stopped. ⏎ send again · esc drop` when the hub reports the agent gone
  (**⚠** needs a hub signal; if missing, write it in the notes for main).
- **done when:** tests for the three marks and a replayed history (marks
  restored from `injected :` lines).
- **notes:**

### BISE-31 · answered cards

- **status:** todo (decided: the card fades in place, grey bar + `answered`; starts after F's BISE-14/15)
- **owner:** — · **commits:** —
- **track:** C · **owns:** `sb/cards.rs`, and the level-1 feed line style
  in `render.rs` **only after F's wave 2 is done**
- **spec:** book §12 (open question), §10 (status marks)
- **do:** once answered, the card's line in the history turns grey (dim bar, `answered`), the answer follows as a normal line; the box closes as today.
- **notes:**

### BISE-41 · help and commands: words and case

- **status:** todo · **owner:** — · **commits:** —
- **track:** K · **owns:** `help.rs`, `commands.rs`
- **spec:** book §4, §16, §17
- **do:** every help row and command description lowercase, "agent" never
  "task"; rows for `⌥ + number` (shown in the panel), `ctrl+f`, the new keys
  of BISE-42; add `/theme` (calls BISE-02's `theme_detect::apply`) and
  **⚠** `/welcome` (replays the onboarding, BISE-60).
- **done when:** help tests updated; `/help` and `/shortcuts` read like §16.
- **notes:**

### BISE-42 · new keys

- **status:** todo · **owner:** — · **commits:** —
- **track:** K · **owns:** `input.rs`, key arms of `sb.rs`
- **spec:** book §11 (keys), §16
- **do:** bind `space` on a selected feed item to `feed::toggle_selected`,
  and one key to `feed::toggle_all_outputs` (**⚠** pick it with main: free,
  not tmux's `ctrl+b`, not taken in `help.rs`).
- **done when:** keyprobe / input tests; no conflict with the composer.
- **notes:**

### BISE-60 · onboarding flow

- **status:** todo · **owner:** — · **commits:** —
- **track:** O · **owns:** new `rust/tui/src/onboarding.rs`, its entry in
  `run.rs`
- **spec:** book §15, mockup `tui-onboarding.html` (validated)
- **do:** the six steps, exactly as the mockup: typed welcome and `:*` pop,
  theme with two live previews (uses `theme::set_mode`), model (detect keys
  in the environment; **⚠** which providers: ask main), folder + the honest
  line, the three lines, then the normal UI. Runs once per user (flag in the
  state directory); `esc` / `ctrl+c` skip it and mark it seen.
- **done when:** a first launch with an empty state directory shows it; a
  second launch doesn't; screenshots match the mockup step by step.
- **notes:**

### BISE-61 · one-time hints

- **status:** todo · **owner:** — · **commits:** —
- **track:** O · **owns:** new `hints.rs`; hint calls in `sb.rs` event
  handling (not the key arms)
- **spec:** book §15 step 6, contract C4
- **do:** `hints::once(app, Hint::X)` with a store in the state directory;
  hints for the first agent, the first run of level 3, the first card
  (and **⚠** the first steer). A hint is a small accent-bordered note next
  to the thing; it goes away when used or after the next user message.
- **done when:** each hint shows once across restarts; tests of the store.
- **notes:**

### BISE-70 · images UI

- **status:** todo · **owner:** — · **commits:** —
- **track:** I · **owns:** `attach.rs`, the chip drawing in the composer
  render path, the strip in `ui.rs`
- **spec:** book §14, `../images.md`
- **already there:** the image plumbing from 4282501 (`[Image #N]`, `@`
  pick, dropped path, `ctrl+v`, markers through the hub).
- **do:** chips `▣ 1` in the composer and `▣ name` in the history (atomic,
  accent); the strip above the composer; the size line under a user line;
  `result · ▣ name WxH` for tool results; the readable no-vision error
  (match the provider error, then the §17 string).
- **done when:** screenshots match the three image screens of
  `tui-screens.html`; tests for the strip text (sizes, "resized to fit").
- **notes:**

---

## Wave 3 — sweeps

### BISE-80 · vocabulary sweep

- **status:** todo · **owner:** — · **commits:** —
- **track:** S · **owns:** any user-visible string, one file at a time
- **do:** `rg -n -i 'task'` over user-visible strings of `rust/tui` and the
  hub's messages to the UI; "agent" everywhere (code identifiers may stay).
- **notes:**

### BISE-81 · lowercase and copy-deck sweep

- **status:** todo · **owner:** — · **commits:** —
- **track:** S
- **do:** every UI string lowercase (§4 exceptions); every string in §17
  matches exactly; list the strings not in §17 in the notes so main adds
  them.
- **notes:**

### BISE-82 · visual QA

- **status:** todo · **owner:** — · **commits:** —
- **track:** S · **owns:** no code; screenshots under
  `projects/switchboard/docs/brand/qa/`
- **do:** every screen of `tui-screens.html`, reproduced in the real TUI, in
  Ghostty dark and a light terminal; a table screen × OK / differs (what).
- **notes:**

### BISE-83 · remove deprecated theme aliases

- **status:** todo · **owner:** — · **commits:** —
- **track:** S · **owns:** `theme.rs` and whatever still uses an alias
- **do:** remove the `#[deprecated]` aliases of BISE-01; every file uses the
  roles.
- **notes:**

---

## Added by main

### BISE-84 · glyph fallbacks and `BISE_ASCII`

- **status:** todo · **owner:** — · **commits:** —
- **track:** T · **owns:** `rust/tui/src/theme.rs` (the `G_*` constants)
- **spec:** book §6 (decision after the glyph audit), glyph-audit.md
- **do:** apply the §6 decision in the `G_*` constants: `✉` → `@`, `↪` → `»`,
  `⟳` → `≡` (pulsing while running), `⧗` → `Δ`; pick the worktree mark
  (width 1, in ≥ 4 of the audited fonts, not `⌥`, check it with the audit's
  method) and write it in the notes (main updates §6). Add `BISE_ASCII=1`:
  every `G_*` glyph (and `working_frame`) returns a plain-ASCII form
  (`~` working, `<3` done, `>` you, `*` main mark stays `:*`, …; table in the
  notes). Glyphs must go through functions or a table so the switch is
  global.
- **done when:** a unit test checks every glyph is width 1 (or its documented
  width) in both modes; the TUI runs with `BISE_ASCII=1` on a throwaway hub
  and shows no non-ASCII glyph except box-drawing lines.
- **notes:**
