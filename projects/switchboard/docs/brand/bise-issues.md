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

- **status:** done · **owner:** bise-t-theme · **commits:** 6fdba1f
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
  - **Where (main's correction):** not `term.rs` (the embedded shell). New
    `rust/tui/src/theme_detect.rs`; `lib.rs` +1 line (`mod theme_detect;`);
    `run.rs` `init_terminal` +2 lines (a comment + `crate::theme_detect::init();`)
    right after `enable_raw_mode()?` and before `EnterAlternateScreen`.
  - **API:** `Choice { Auto, Light, Dark }`, `Choice::parse(&str)` (light / dark
    / auto, any case), `apply(Choice) -> Mode` (for `/theme`, BISE-41),
    `detected() -> Option<Mode>` (what the terminal said at start), `init()`
    (once per process: a re-init after suspend keeps the `/theme` choice),
    `ENV = "BISE_THEME"`.
  - **How:** writes `ESC]11;?BEL` then DA1 `ESC[c` to `/dev/tty`, reads until
    the DA1 reply (every terminal answers it, so no 100 ms wait when OSC 11 is
    unsupported) or 100 ms. Waiting uses libc `select` declared by hand (no new
    dependency): **`poll` does not work on ttys on macOS** (returns at once,
    POLLNVAL). `/theme auto` at runtime reuses the start answer: querying
    while the UI reads keys would turn the reply into keystrokes.
  - **Threshold:** light when relative luminance > 0.184 (L* > 50), not 0.5:
    that is where our dark text reads better than our light text (mid-grey
    `#808080` → light, `#707070` → dark).
  - **By hand** (`cargo test -p bend-tui --lib probe_this_terminal -- --ignored`,
    writes `/tmp/bise-theme-probe.txt`): Ghostty dark `rgb:1616/1616/1616` →
    dark; Ghostty `--background=#f7f4ee` → light; tmux 3.5a inside Ghostty
    (dark and cream) → forwards the outer color, right mode; Terminal.app
    default profile `1e1e1e` → dark, white window → light; detached tmux (no
    client) answers only DA1 → dark in < 1 ms. All answers < 1 ms.
  - **Visible effect:** little until wave 1 moves off the deprecated aliases
    (they are fixed dark); the roles already switch.
  - **Gates** (private worktree, shared target): build, test --workspace (208 +
    97 + …, all ok), clippy clean, PROOF, e2e all PASS, tmux: tui, help, term,
    composer OK; `tui_version_tmux` fails **on HEAD 6159bb4 without my change
    too** (the BISE-20/21/22 chrome), so not from BISE-02.

### BISE-10 · reading width

- **status:** done · **owner:** bise-f-feed · **commits:** 90d76f2
- **track:** F · **owns:** `render.rs`, `feed.rs`, `code.rs`, `markdown.rs`,
  `feed_render_tests.rs`
- **spec:** book §11 (measure)
- **do:** prose (user lines, assistant text, reports, agent messages) wraps
  at `min(width − margins, 76)`; code (scripts, diffs, outputs) up to 100
  columns, longer lines wrap with a hanging indent and a faint `↪`. The
  extra width is left empty (the feed doesn't stretch lines).
- **done when:** render tests at widths 60, 100, 160 show prose ≤ 76 and code
  ≤ 100; the 50k-line bench (`sb/bench.rs`) is not slower.
- **notes:** `render::PROSE_MAX` 76 / `CODE_MAX` 100, `prose_width` / `code_width`, `ev_rows` (a tool follows the code measure, everything else the prose measure); rows never stretch (the user row's panel stopped at the measure, then went away in BISE-13). Code blocks lost the box, header and line numbers for a faint rail ` │ ` (mockup "inside an agent"); a long line wraps with a hanging indent and a faint wrap mark (`↪`, now `theme::G_WRAP` = `»`), marked soft so the copy joins it; the copy drops the rail. Open reasoning wraps behind its rail. Tests: `prose_wraps_at_76_and_code_at_100` (60/100/160), `long_code_lines_wrap_with_a_hanging_indent`, feedsel `code_rail_rows_copy_the_code_only`. Known: `wrap_line` keeps the blank after a word at the row end (as before; the copy relies on it), so a row can be measure + 1 with a trailing space. Markdown fences inside a message wrap at the prose measure. bench (`bench_long_feed`, 50k lines of main's transcript, release, 200×50), before (7d07b05) → after (d740568): replay 87.5 → 88.8 ms; first draw 1.0 → 0.8 ms; steady frame 0.32 → 0.20 ms; PageUp 0.86 → 0.76 ms; top 0.39 → 0.31 ms; PageDown 0.78 → 0.81 ms; 20 live lines 0.27 → 0.24 ms; resize 0.7 → 0.5 ms; windowed PageUp worst 1.51 → 1.52 ms, page ingest 2.8 → 1.9 ms. Not slower (noise level).

### BISE-11 · scripts in full

- **status:** done · **owner:** bise-f-feed · **commits:** 312daaf
- **track:** F · **owns:** as BISE-10
- **spec:** book §11 (scripts in full)
- **do:** bash and `run_typescript` sources are never folded
  (`CODE_FOLD_AT` / `CODE_FOLD_SHOW` in `render.rs` no longer apply to
  them); syntax colors from the theme roles. Other long code (e.g. a huge
  patch) still folds behind `▸`.
- **done when:** a 200-line script renders whole in a test; colors come from
  `theme::` roles.
- **notes:** bash and TypeScript sources never fold; syntax and diff colors come from the roles (`syntax_*()`, `text()`, `dim()`, `ok()`, `error()`), no diff bands (never paint the background). Tests: `a_long_script_renders_whole` (200-line bash and TypeScript), `bash_highlighting_classifies_tokens` on the roles. The 60/40 patch fold was replaced in BISE-12 by the edit line itself (a patch is hidden until opened, then whole).

### BISE-12 · progressive disclosure

- **status:** done · **owner:** bise-f-feed · **commits:** eb8c667
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
- **notes:** Forms: output `   ▸ output` (` · {k} failed` when the text has "N failed") / open `   ▾ output` + the text under the rail; a failure is one line with its reason (error color), `▸` when cut; edit `± edit {path} ✓ +a −d ▸` (several files: `{n} files`; failure: `✗ {reason}`), the diff only when opened; report in main (`[report: kind] …` msg-in) `♡ {from}: {summary} ▸ report` (✗ failed, ? blocked, · progress; `▸ report` only when there is more), open: the rest under the rail; brief inside an agent (msg-in starting `# Task \``) `◇ brief ▸`, open: the brief without its title. API: `feed::discloses`, `feed::toggle_event(events, cache, i)` (the click; input.rs now calls it: one hunk, main OK'd, bise-k-keys told), `feed::toggle_selected(app)` (selection head's event) and `feed::toggle_all_outputs(app)` (tools only: one closed → all open, else all close) for BISE-42, not bound. Contract: `Ev::AgentMsg` got an in-memory `open: bool` (main OK'd, C2 note; wire.rs + the 4 parse_hub_line constructors in sb.rs). Tests: one per form, `toggles_one_item_and_all_outputs`, `fit_chars_keeps_the_ellipsis_inside`. **Differs from the book:** the runtime's `tool_result` is a one-line preview (newlines flattened, ~250 chars), so there is no line count: the closed form is `▸ output`, not `▸ output · 42 lines` (needs the runtime to send the line count or the full output: for main). `result · 2 items` for TypeScript is not done (one word, `output`, everywhere).

### BISE-13 · entity glyphs in the feed

- **status:** done · **owner:** bise-f-feed · **commits:** d740568
- **track:** F · **owns:** as BISE-12
- **spec:** book §6 (entities), glyph audit (BISE-03)
- **do:** use the §6 glyphs in the feed: `›` you, `:*` main, `$` bash, `λ`
  TypeScript, `↳` sub-call, `±` edit, `∴` thinking (replaces `✦`), `⟳` / `≡`
  compaction, `▲` interrupt, `✗` failure; fallbacks from BISE-03. Replace
  the `◆ carte` line of `Ev::Card` by the level-1 look (accent bar, `?
  {name} needs you`).
- **done when:** render tests updated; screenshots of the feed match
  `tui-screens.html` screens "inside an agent" and "everything disclosed".
- **notes:** Glyphs through the theme constants: `›` you (dim, text from column 3; no bar, no panel background), `$ bash` / `λ typescript` (other tools: empty glyph column), working pulse while running, `✓` dim / `✗` error, `↳` sub-calls (a failed one gives its reason), `∴ thought for {s}s ▸` (no duration: `thought`), `≡` compaction and summary (dim), `▲` interrupt (dim), `✗` errors, `:*` on messages from main, `@` (`G_MSG`) on the others (their level look is BISE-14), a card is level 1 (accent `┃`, bold accent `? {name} needs you`, body in text; `♡ {name} is done: …`, `✗ {name} failed: …`). French leftovers fixed (`interrompu`, `raisonnement`, `tour`). Tests: `inside_an_agent_matches_the_mockup`, `everything_disclosed_matches_the_mockup` (rows at 100 columns), `a_card_is_level_one`, `feed_entities_use_the_book_glyphs`; updated sb.rs `hub_line_tests` and tmux waits (`tui_archived_tmux.py`, `tui_at_files_tmux.py`, `tui_tmux.py`: glyph-agnostic `" t1 m_"`, brief folded; the tui_tmux.py lines landed with ecb4bec). **Differs from the mockups:** (1) no `:*` in front of main's own replies in main's feed: the render path doesn't know whose feed it draws (needs a feed-owner flag from ui.rs → `ensure_rows`; proposed for BISE-14); (2) `♡ turn done · {duration}` is not drawn (TurnDone is still debug-only); (3) the elapsed shows only for live tools (replayed ones have none); (4) an open diff keeps its `~ path` header; (5) the mockup's blank lines between blocks follow today's gap rules. Visual check done on TestBackend rows, not yet in Ghostty dark/light (BISE-82). Not done here: `theme::glyph()` for BISE_ASCII (the draw-time `asciify` net covers it), `compacting_frame` pulse (the line is static history), wiring bise-i-images' attach.rs helpers (16bc2a2: chip_spans, sizes_line, result_spans, no_vision) → next render.rs work. help.rs still says `click ✦` (K, BISE-41).

### BISE-20 · agents panel

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
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
  - **Row:** ` N G name[ •][ ⎇][ ✉n] …… right ` — number faint (blank after 9),
    glyph from §6 via `G_*` (`∿`/`·` pulse through `working_frame`/`starting_frame`,
    `?` accent for blocked *or* an open question/blocked card of that agent, `✗`
    error, `○`/`–` dim, `:*` accent for main), name in text (accent + bold for
    the agent in view), right side dim flush right with 1 column of margin:
    working `12m · 21%` (turn duration · context), `you` (accent), `waiting`,
    `starting`, `done`, `idle` (`idle · 21%` when the fill is known), `failed`,
    `stopped`; main has no right side unless working/starting. Selected row on
    `selection_bg()`, with its objective (and declared note) dim under it
    (clickable too) — the only second row. Names are cut with `…` to keep the
    marks and the right side.
  - **Numbers** = the index Alt+N uses (live agents in the hub's order, main 0).
    They do not change while agents live, but dropping/archiving agent 2
    renumbers 3→2: making them truly stable needs `Goto` in `sb.rs` (K) and
    the panel to share one slot map — left as is, flagged to main.
  - **Differences with the mockup:** `waits {name}` shows `waiting` (the hub
    snapshot has no "waits on" field: H would need to add `waiting_on`); the
    queued-messages mark `✉n` (dim) kept from before; the panel no longer
    lists cards (mockup: agents only; the header counts them as needs you and
    the status row says `? n cards · ctrl+g` while the box is hidden); `+ {n}
    more` counts the agents below the window (a folded archived section counts
    its agents); no `· ctrl+k scroll` suffix (not in the copy deck).
  - **Archived (85160ab) restyled:** `▸ {n} archived` / `▾` dim (hint "click or
    /archived" dropped, copy deck), rows `  – name … 5h` (compact age, dim),
    report under the selected/focused one; clicks, `A`, `/archived`, read-only,
    `/restore` unchanged.
  - **Tests:** `panel::tests::panel_rows_at_28_and_40` (every state, title on
    one row at 28, flush right, no number after 9, no "task"), `panel_colors`,
    `overflow_ends_with_more`, click tests and archived tests updated.
  - `cards::ago` became unused (bise-c-cards keeps it with an allow, removes
    it later). New string: `+ {n} more` as in the copy deck; `idle · 21%`.

### BISE-21 · header row

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (header)
- **do:** one row on top: `bise :*` (`:*` accent) left; right, the non-zero
  counts `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done` (`? … needs
  you` in accent), or `no agents yet`. Under 70 columns (no panel) the
  counts shorten to `∿ 3 · ? 1 · ♡ 1`. Remove the old ` Switchboard ` title.
- **done when:** render test of the header at 60 and 120 columns.
- **notes:**
  - ` bise :*` (`bise` bold text, `:*` accent) left; right the non-zero counts
    `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done` (dim, `? … needs
    you` accent), 1 column of margin; under 70 columns `∿ 3 · … 1 · ? 1 · ♡ 1`;
    `no agents yet` (dim) when no live agent besides main. Counts leave out
    main and the archived; "needs you" wins over the status (same rule as the
    panel). The counts drop when they would not fit next to `bise :*`.
  - The header spans the whole width above feed and panel; the ` Switchboard `
    title is gone. `Sb::header()` is a method so `ui.rs` reaches it through
    `app.sb` without touching the re-exports of `sb.rs`.
  - **Mockup difference:** the 30-agent screen writes `? 2 need you`; the copy
    deck's `needs you` is used for every n.
  - **Tests:** `chrome_tests::header_at_60_and_120`, `header_colors`.
  - **Outside my files (main OK):** `app.rs` gets `feed_y`, `input.rs::feed_pos`
    subtracts it (and the drag-scroll edges), since the feed no longer starts at
    screen row 0; test `feed_clicks_land_on_the_row_under_the_header` (main and
    inside an agent).

### BISE-22 · status row, composer, first run

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (status row, composer, first run), §17
- **do:** status row starts with the agent in view in accent, the rest dim,
  lowercase (`main · idle · 210k / 1M tokens · 21%`); composer hints from
  the copy deck; the first-run text when there are no agents; the "inside an
  agent" line (`you're talking to {name} directly…`).
- **done when:** screenshots match `tui-screens.html` screens "first run",
  "inside an agent"; strings match §17 exactly.
- **notes:**
  - **Layout (Switchboard only; the single-agent TUI keeps its old layout):**
    header · feed | panel (the panel runs down to the blank row under the feed)
    · then full width: card box · status row · composer. No meta row (`◆ bend
    · model`), no padded prompt block, no separate hint row.
  - **Status row:** ` {name} · {state} · {40s while working} · {210k / 1M tokens
    · 21%} · {shared folder | ⎇ branch}` then notes: `read-only history ·
    /restore brings it back`, `preview of {name}`, `? {n} cards · ctrl+g`
    (accent, box hidden), `⧗ building {rev}`, `⧗ {rev} on trial`, `v {version}`,
    `○ hub disconnected · reconnecting…` (error). Name in accent, the rest dim.
    The scrolled-up row is now ` ↓ back to the bottom · end · {n} new lines`.
  - **Composer:** ` › ` (dim) then the text, continuation rows indented; the
    voice meter takes the `›` place; empty = the cursor only (archived: `{name}
    is archived: read-only`). Hints flush right on the composer's last row;
    when the text leaves no room, on the status row, else on the blank row
    under the feed. Hints: main / in an agent `⏎ send · @ agent · / commands`,
    during a turn `⏎ steer · ctrl+c interrupt` (copy deck); others lowercased:
    `y yes · n no · esc cancel`, card `alt+r answer with text · ctrl+x later ·
    ctrl+f full screen`, full card `alt+r answer · pgup/pgdn scroll · ctrl+f
    back`, selection `⏎ enter · space preview · D drop · esc close`, archived
    `/restore brings it back · esc back to main`, voice/popup hints lowercased.
  - **First run:** the three §17 lines, dim, wrapped ≤ 76, in main's feed while
    there are no agents and nothing visible in it yet.
  - **Inside an agent:** `you're talking to {name} directly. main isn't in the
    loop. esc back to main.` pinned (dim) on the first feed row, a blank row
    under it; previewing: `preview · {name} · ⏎ enter · esc close` (mockup
    string, not in the copy deck).
  - **Mockup differences:** the mockup's `· ? help` after the main hints is not
    in the copy deck: left out; the `.foot` top border is a blank row; no
    blinking cursor (a reversed cell, as before). New strings for the book:
    `{name} is archived: read-only`, `read-only history · /restore brings it
    back`, `? {n} cards · ctrl+g`, `↓ back to the bottom · end`, the lowercased
    hints above.
  - **Tests:** `chrome_tests::first_run_screen`, `inside_an_agent_screen`,
    `status_row_and_hints` (strings compared with §17 exactly). Updated with
    main's OK: `composer_wrap_tests.rs`, `voice_ui_tests.rs`, `sb/bench.rs`
    (layout), and the tmux tests `tui_tmux.py` (new helpers `wait_re`,
    `panel_row`, `in_view`), `tui_panel_click_tmux.py`, `tui_archived_tmux.py`,
    `tui_composer_tmux.py` (`composer()` reads the `› ` rows), and
    `wait_screen("bise :*")` in the others.
  - **Visual check:** tmux captures at 150×42 (dark palette); Ghostty / light
    terminal not done (light needs BISE-02).
  - **Gates** (private worktree HEAD 3c2b4ca + my files, shared
    `CARGO_TARGET_DIR`): cargo build ok; `cargo test --workspace` all green
    (bend-tui 199, switchboard 96); clippy `--workspace --all-targets` clean;
    `run_all.sh`: ALL PROOFS CHECK, e2e and every tmux test PASS (+
    `tui_panel_click_tmux` PASS). The shared tree did not compile its tests at
    that time (bise-f-feed WIP `Ev::AgentMsg.open`). `input.rs` committed from
    a private index: bise-f-feed's uncommitted hunk stays in the tree.

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

- **status:** done · **owner:** bise-k-keys · **commits:** 5ff22eb
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
- **notes:** Level 3 (`msg`, `msg-in`): one dim line under the faint rail, ` │ @ from      → to        text` (names cut to 9 and padded to 10; no `to` (msg-in inside an agent): the message id sits in the `to` column, no arrow). The line uses the **code measure** (100), not prose: it is a list row, and the mockup lines run past 76. `▸` when cut; it opens (a click, `feed::toggle_at`) to the whole text under the rail at the prose measure. Level 2 (`msg-you`): `@ name to you: …` in text (from main: `:* …`). `Ev::Answered`: ` :* docs asked: v1 or v2? i answered: v2 ▸ why`; the why under the rail when open. Folding: a run = consecutive visible level-3 lines. **Any** other visible line ends it (level 1/2, but also a tool, a thinking section, a notice, a time mark), so a closed run is frozen; more than 3 lines fold into ` │ ▸ {n} messages between {k} agents` (k = the distinct senders and receivers); the last run carries the working pulse `∿` (a live row redrawn per frame, like a running tool's line). Opening a fold shows its lines in place, in order; the first line of an open fold has two rows (the fold, the line), and `toggle_at(events, cache, i, row)` tells them apart (the click in input.rs now calls it). Append-only: `push_event` invalidates only the run before the new line (its count, its members when it reaches 4, its pulse when it closes). Time marks: `Ev::TimeMark`, ` · 14:31 ·` faint, gap before, pushed by `feed::pause_mark` from `run::ingest_line` when a **live** line comes 5 min or more after the previous wire line of that feed (local time via `date +%H:%M`, UTC fallback). bise-i-images' helpers wired: chips + a sizes line on your lines, `result · ▣ x.png 390×844` output labels (the text around it under `▸`), the no-vision line on `Ev::Err`; the `#[allow(dead_code)]` in attach.rs are gone. `toggle_selected`'s dead_code attr is gone (K bound it); `toggle_all_outputs` keeps its attr (unbound). **Contract note (C2, in memory only):** `Ev::AgentMsg.fold`, `Ev::Answered.open`, `Ev::TimeMark`; sb.rs constructors and hub_line_tests follow. **Tests:** `a_run_of_twelve_folds`, `a_level_two_line_closes_the_run` (and a main line, a card), `a_closed_run_never_changes` (cached and rebuilt rows), `arrival_order_holds_with_many_agents` (30 agents, 120 lines, lines for you in between), `time_marks_after_a_pause`, `level_two_and_answered_lines`, `the_first_line_of_an_open_fold_toggles_by_row`, `whats_for_you_matches_the_mockup`, `a_busy_hour_matches_the_mockup` (rows at 100 columns); hub_line_tests and `agent_message_wraps_behind_its_bar` updated. Tmux: `tui_tmux.py`, `tui_archived_tmux.py`, `tui_at_files_tmux.py` wait for `t1 +m_\d` (a regex: the names sit in columns now). **Bench** (50k lines, release, same transcript, worktree of HEAD vs HEAD+mine): replay 82.5 → 87.5 / 96.7 / 79.4 / 79.3 ms (noise: ±10 ms run to run), first draw 0.8 → 0.8, steady 0.27 → 0.21, PageUp 0.72 → 0.45, top 0.36 → 0.31, PageDown 0.85 → 0.48, live 0.23 → 0.24, resize 0.5 → 0.5, windowed PageUp worst 1.37 → 1.96 / 1.39 / 1.12 / 1.14, page ingest 1.7 → 1.2–1.3: not slower. **Differs from the mockups:** (1) main's replies still have no `:*` in main's feed (the render path doesn't know whose feed it draws: needs a feed-owner flag from ui.rs → `ensure_rows`, not in my files); (2) the mockup's `▾ 5 messages between 3 agents` lists 6 names: we count 6; (3) replayed history gets no time marks: the hub strips the transcript's timestamps from the lines it sends (keeping them on `history` lines would be a C2 change, for main); (4) a fold cut by a window trim or a page boundary may keep stale rows for its middle lines until they rebuild (trim_window/prepend_page rebuild only the first event: sb/feed.rs, not mine). **For K:** ctrl+t can use `feed::anything_closed` / `feed::set_everything(events, cache, open)` (folds and `▸ why` included; `input::is_open` treats `Answered` as always open). **Gates** (private worktree of HEAD + my hunks, shared target): build, clippy (`-D warnings` and `--all-targets`: 0 warnings), bend-tui 259 tests pass, e2e.py and every tmux test PASS except `tui_waits_tmux`; `cargo test --workspace` fails **only** on `switchboard core_tests::a_waiting_agent_says_who_it_waits_on` (`waiting_on` null): that's the switchboard crate, which my patch doesn't touch, and `tui_waits_tmux` asserts the same `waits t2` (BISE-23 / core, for main).

### BISE-15 · message marks `·` `✓` `✓✓`

- **status:** done · **owner:** bise-f-feed · **commits:** 9e44f32
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
- **notes:** `Ev::You(text, Mark)` with `Mark::{Sent, Received, Read}` (C3; no `None`: every line has a mark). Drawn after the last line of your message: `·` dim, `✓` faint, `✓✓` accent (through `theme::glyph`). `steering_received:` / `steered:` parse to an in-memory `Ev::MarkYou { text, mark, or }`: `push_event` moves the mark of your **last** line with the same words (whitespace-insensitive: the wire flattens line breaks; looks back 500 events), only upward, appends nothing; no info lines any more. A turn start (`Ev::Turn`) marks what you sent since the previous turn as read (a message at idle → `✓✓`). Your line is `·` when it appears (the hub's `sb you` echo, the solo client's local echo). Replayed history (REPL `--resume`): `you :` → read; `injected : X` marks your line X read, or stays the old `injected · X` info line when there is none (the REPL history has no `you :` line for a steering, and can't tell it from a notification). Hub history pages carry the same obs lines, so their marks come back the same way. `Hint::FirstSteer` (BISE-61) fires on the first `steered:` in view (sb.rs `ingest_for`). Also here (main's OK, m_828): **`:*` on main's replies in main's feed** — `render::set_main_feed` (ui.rs `draw_feed`, from `Sb::is_main_focus`); `EventRows.main` records the owner the rows were built for, and `ensure_rows` rebuilds on a mismatch. **Not done, needs a hub signal (for main):** `✗ not delivered: {name} stopped. ⏎ send again · esc drop` — the hub says nothing when a message goes to a stopped agent; a `sb undelivered : {name} : {text}` line (C2) would let `push_event` mark the line (a `Mark::Failed`) and draw the prompt. **Tests:** `steering_moves_the_mark_of_your_line` (`·` → `✓` → `✓✓`, no info line, the three colors), `a_mark_only_moves_up_and_finds_its_line`, `a_message_at_idle_is_read_when_its_turn_starts`, `a_replayed_history_restores_the_marks` (`you :`, `injected :` with and without its line, the live obs lines), `mains_replies_carry_its_glyph_in_its_feed_only`; mockup and multi-line tests updated (`✓✓` on your lines). **Gates** (worktree of b4808a2 + my hunks, `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace` green (the core `waiting_on` failure is gone with the right sb-core, per bise-h-hub), clippy `--all-targets` 0 warnings, `run_all.sh` rc 0 (all tmux PASS, `tui_waits` too); rebased on 07ce911 (K's ctrl+o): build + clippy clean. **Bench** (50k lines): replay 85.7 / 84.9 / 84.3 ms (base 82.5, noise ±10), steady 0.22–0.23, PageUp 0.46–0.50, PageDown 0.50–1.06, windowed PageUp worst 0.97–1.35, page ingest 1.3–1.9: not slower. **Differs from the mockups:** every line of yours carries its mark (the mockups show marks only in the steering screen); the hint says `✓✓` in the accent.

### BISE-31 · answered cards

- **status:** done (decided: the card fades in place, grey bar + `answered`; starts after F's BISE-14/15)
- **owner:** bise-c-cards · **commits:** f040d1d
- **track:** C · **owns:** `sb/cards.rs`, and the level-1 feed line style
  in `render.rs` **only after F's wave 2 is done**
- **spec:** book §12 (open question), §10 (status marks)
- **do:** once answered, the card's line in the history turns grey (dim bar, `answered`), the answer follows as a normal line; the box closes as today.
- **notes:**
  - **how:** `Ev::Card { text, closed }` (`closed` in memory only, empty:
    open) and a new in-memory `Ev::CardClosed { id, res }`, parsed from
    the hub's `sb card-closed : #N res` (was an info line). `push_event`
    finds the last `Ev::Card` `#N …` of that feed, sets `closed`,
    invalidates its rows and appends nothing (same pattern as F's
    `MarkYou`). A closed question/blocker/drop/overlap card: bar, glyph,
    title and body dim, not bold, title `? docs needs you · answered`.
    The answer follows as the hub's own route line (`you → @docs (answer
    to card #N) : v2`). The box closes as today (the snapshot drops it).
  - **words** (`render::closed_word`): `answered`, `answered via @x` →
    `answered by x`, `closed` (ctrl+x), `accepted` → `dropped`, `refused`
    → `kept`, `vue` → `seen`, `reprise` → `resumed`, `task stopped` →
    `agent stopped`.
  - **done / failed cards** are level-2 lines (`♡`, `✗`): no fade, and
    their close adds no line either.
  - **limit:** a card whose close arrives while the card itself is not in
    the loaded window (an older page, loaded later by scrolling up) stays
    as it was; the close shows as the old info line `card #N answered`.
  - **files outside `cards.rs`** (agreed with F, after BISE-15): `wire.rs`
    (the two variants), `sb.rs` (`parse_hub_line` card / card-closed arms
    + test helper), `feed.rs` (`push_event` arm, `is_notice`),
    `render.rs` (Card arm, `card_lines`, `closed_word`),
    `feed_render_tests.rs` (`an_answered_card_fades_in_place`).
  - **gates:** private worktree of ca45c51 + the patch, shared target:
    build, `cargo test --workspace`, clippy `--all-targets -D warnings`,
    `run_all.sh` all green (one switchboard test failed once in a first
    `run_all`, green on two reruns and the full rerun: the shared-target
    mixing). No .bend change.

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
  - **Help (`help.rs`):** every section, key and action lowercase, "agent"
    never "task" (sections: `talk to agents`, `agents (empty composer)`,
    `cards`, `feed`, …; keys written like §16: `ctrl+c`, `alt+r`,
    `option+←`, `esc`, `tab`). Kept capitals: `Ghostty`, `macOS U.S.`,
    the config path, and the keys `D` / `A` (they are shift+letter). New
    or changed rows: `⌥ + 0…9` "go to main (0) or to the agent with that
    number in the panel" (replaces the two `alt+1 … alt+9` / `alt+0`
    rows, top), `ctrl+f` (now top), `click ▸|space` in the feed,
    `ctrl+t` "open or close everything folded" (top), `ctrl+/|cmd+z`
    "undo your typing (only the composer: sent messages have no undo)".
    `D` says "drop the selected agent (stop it, archive its history)": it
    does **not** ask first today (book §16 says it does; the hub's
    `/drop` has no confirm). Headers `commands` and `essential keys ·
    every key: /shortcuts (tab here)`.
  - **Commands:** `SB_COMMANDS` (sb.rs) and `COMMANDS` (commands.rs)
    lowercase, "agent" (`/new` "start an agent", `/drop`, `/restore`,
    `/isolate`, `/rename`, `/archived`); "hub" left the `/restart` line
    ("rebuild and restart switchboard…"). **`/tasks` keeps its name** (the
    hub parses it): its line says "every agent: what it does, its last
    report, its questions". Renaming it `/agents` is a hub change (M).
  - **`/theme [auto|light|dark]`:** `theme_detect::choose` (BISE-62):
    switches and saves; says `theme: light.`; a failed save says `theme:
    light, for now: i couldn't save it ({err}).`; no argument says the
    mode in use; a wrong one `/theme takes auto, light or dark.` The feed
    cache is rebuilt (rows carry their colors). **`/welcome`:**
    `onboarding::run(app)`.
  - **No undo (§17):** `ctrl+z` and a typed `/cancel` push the info line
    `no undo: an agent may already have acted. say the change to main
    instead ("no, v1 for docs").` (sb.rs `NO_UNDO`); nothing reaches the
    hub, the draft stays. (BISE-40 had made ctrl+z silent.)
  - **New strings (for §17):** `theme: {mode}.` · `theme: {mode}. /theme
    auto, light or dark to change it.` · `theme: {mode}, for now: i
    couldn't save it ({err}).` · `/theme takes auto, light or dark.` ·
    the command lines above.
  - **Tests:** `help::tests::rows_read_lowercase_and_say_agent` (no
    capital but the proper nouns, no "task", no ctrl+z row, the ctrl+t
    text), `help_is_commands_and_essentials` (lowercase keys, `⌥ + 0…9`),
    `sb::…::theme_and_welcome_commands` (a fake chooser: tests never
    write the real `~/.bend-harness/tui.json`),
    `ctrl_z_and_cancel_say_no_undo`. **`tui_help_tmux.py`** assertions
    follow the lowercase strings (`essential keys · every key`,
    `commands`, `agents (empty composer)`, `talk to agents`,
    `ctrl+option+←`); it waits for `essential keys · every key` because
    the `/help` popup line also says "essential keys".
  - **Gates** (worktree of 45f5a36 + my files, shared target,
    `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace`,
    clippy `--workspace --all-targets -D warnings`, e2e and the 11 tmux
    tests of `run_all.sh`: green. No `.bend` change.

### BISE-42 · new keys

- **status:** done · **owner:** bise-k-keys · **commits:** 5ff22eb
- **track:** K · **owns:** `input.rs`, key arms of `sb.rs`
- **spec:** book §11 (keys), §16
- **do:** bind `space` on a selected feed item to `feed::toggle_selected`,
  and one key to `feed::toggle_all_outputs` (**⚠** pick it with main: free,
  not tmux's `ctrl+b`, not taken in `help.rs`).
- **done when:** keyprobe / input tests; no conflict with the composer.
- **notes:**
  - **No new key (main's call):** `ctrl+o` stays the shell. `ctrl+t` now
    opens or closes **everything folded**, one state: if any item that
    discloses (`feed::discloses`: thinking, an output, a diff, a report,
    a brief) is closed, all open; else all close. New thinking sections
    follow it (`show_thinking`). `input::toggle_everything` goes through
    `feed::toggle_event`, so `feed::toggle_all_outputs` stays unbound
    (F may drop it). Free ctrl keys found, if one is needed later:
    `ctrl+y`, `ctrl+s`, `ctrl+q`.
  - **User decision (bcfb8c2):** the key is now **`ctrl+o`** (like Claude
    Code), same one state, now through `feed::anything_closed` /
    `set_everything` (1a667af). `ctrl+t` is removed, with no alias. The
    `ctrl+o` shell in the agent's folder is removed (key arm, `Sb.shell`,
    `take_shell`, the run.rs block): the terminal panel (``ctrl+` `` /
    `ctrl+space`) is the one shell. Help rows, the solo hint row and book
    §11 / §16 are updated. `tui-screens.html` and `site/index.html` still
    say `ctrl+t` (main / marketing).
  - **`space`:** on the item selected in the feed (`app.feed_sel`, after
    a drag or a double click; a plain click already toggles and leaves no
    selection), with an empty composer: `feed::toggle_selected`. An agent
    selected in the panel keeps `space` for its preview (sb.rs arm first);
    with text in the composer, space is a space.
  - **Tests:** `input::keys_tests` (ctrl+t opens all then closes all,
    `show_thinking` follows; space toggles the selected item, types a
    space when the composer has text).

### BISE-60 · onboarding flow

- **status:** done · **owner:** bise-o-onboard · **commits:** 7d6ee92
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
  - **Where:** `onboarding.rs` (state `Onb`, pure `draw(f, &Onb, now_ms)`,
    `on_key`, the `show` loop). `run.rs`: `request_if_due` in `run_tui`,
    and the request taken at the top of `ui_loop` (under `crash::guarded`;
    hub lines keep being drained during it). `lib.rs`: `mod onboarding;`.
    **Switchboard only** (bise is the Switchboard UI; the single-agent
    client never shows it).
  - **For K (BISE-41):** `/welcome` calls `onboarding::run(app)`: it asks
    for a replay, the loop plays it at the next frame.
  - **Flag:** `$XDG_STATE_HOME/switchboard/onboarded`, else
    `~/.local/state/switchboard/onboarded` (per user, the root
    `switchboard::paths` uses). Written when it ends or is skipped.
    `SB_ONBOARDING=off` never shows it, `on` always does. `e2e.Env` sets
    `off` (main OK'd), so every other tmux test starts on the normal UI.
  - **Keys (decided with main):** the harness knows two providers
    (`runtime/provider-pure.bend`): claude, `ANTHROPIC_FOUNDRY_API_KEY`
    (opus-5.5 through the foundry proxy), and mistral, `MISTRAL_API_KEY`
    (every non-claude model, OpenAI-style API). Found in the env, else
    `~/.bend-harness/.env`, else `~/.vibe/.env` (the order `load_env_files`
    uses). Rows: each found key (the current model's first; another
    provider's key says `your model is {m}: set model in
    ~/.bend-harness/config.toml.`: the model is never changed here), `paste
    another key` (`paste a key` when none), `sign in with the browser`
    (dim, `not built yet.`, skipped by ↑↓). Paste: pick claude / mistral,
    paste masked (`•`), saved in `~/.bend-harness/.env` (created 600, an
    existing mode kept, other lines kept, an existing `KEY=` replaced only
    after `enter replaces it · esc keeps the old one`). Never logged, never
    in a feed. Unit tests use temp HOME / state dirs only.
  - **New strings (for §17):** `i couldn't read your terminal's background,
    so i picked {mode}.` (no OSC 11 answer, e.g. tmux) · `i found no key in
    your environment.` / `i found {n} keys in your environment.` · `paste a
    key` · `claude or mistral.` · `not built yet.` · `{name}, already set
    up. nothing to paste.` · `{name}. your model is {m}: set model in
    ~/.bend-harness/config.toml.` · `which key do you want to paste?` · `↑↓
    choose · enter ok · esc back` · `paste your {KEY}:` · `it goes in
    ~/.bend-harness/.env, only you can read it.` · `enter save · esc back` ·
    `✗ that doesn't look like a key: no spaces inside.` · `{KEY} is already
    in ~/.bend-harness/.env.` · `enter replaces it · esc keeps the old one`
    · `✓ saved in ~/.bend-harness/.env. i'll use it after a restart
    (switchboard --stop, then start me again).` (the hub loads the env
    files at its start) · `✗ couldn't save the key: {err}` · `· not a git
    repo` + ` git would be your safety net: git init here, and commit
    often.` · `another folder? start me there: cd into it, then run
    switchboard.` (the `o` key: a running hub can't change folder).
  - **Differences with the mockup:** no big font (the welcome line is
    bold); the `:*` pop is ` ·` → bold `:*` → `:*` (no scale in a
    terminal); the previews paint their own background (#141211 / #f7f4ee:
    the only painted ground, so the light preview reads on a dark
    terminal); the selected option is `▎` + the selection tint; `esc` in
    the paste sub-steps goes back to the options instead of skipping
    (`ctrl+c` still skips); step 6 is the normal UI (its hints: BISE-61).
  - **Not done here:** the theme picked on step 2 is not saved across
    launches (detection runs again; `/theme` has no store either). Needs a
    settings slot (e.g. `~/.bend-harness/tui.json`), owner to pick.
    Visual check done in tmux (text captures, dark); not in Ghostty light.
  - **Tests:** 13 unit tests in `onboarding.rs` (flag + env var, key
    lookup, model/provider, save 600/replace/keep mode, typing clock, each
    step's screen, esc / ctrl+c, narrow sizes); new
    `tests/tui_onboarding_tmux.py` (5 steps enter by enter with captures in
    `$SB_ONBOARDING_SHOTS`, the flag, a second launch without it, esc skips
    and marks seen), added to `run_all.sh` with `tui_waits_tmux` (main's
    ask). No existing assertion changed.

### BISE-61 · one-time hints

- **status:** done · **owner:** bise-o-onboard · **commits:** 6fe07f7
- **track:** O · **owns:** new `hints.rs`; hint calls in `sb.rs` event
  handling (not the key arms)
- **spec:** book §15 step 6, contract C4
- **do:** `hints::once(app, Hint::X)` with a store in the state directory;
  hints for the first agent, the first run of level 3, the first card
  (and **⚠** the first steer). A hint is a small accent-bordered note next
  to the thing; it goes away when used or after the next user message.
- **done when:** each hint shows once across restarts; tests of the store.
- **notes:**
  - **API (C4):** `hints::once(app, Hint::X)` asks for a hint; it waits
    until its thing is on screen and no other hint is up, then shows, and
    only then counts as seen (`hints.json` `{ "first_agent": true, … }`,
    next to the onboarding flag: `$XDG_STATE_HOME/switchboard/`, else
    `~/.local/state/switchboard/`; other keys kept). `hints::used(h)`: it
    goes away (up or waiting); `hints::user_message()`: the one up goes
    away. `SB_ONBOARDING=off` turns hints off too (every tmux test but
    `tui_onboarding_tmux`); under `cargo test` they are off unless a test
    gives a store (`use_store`).
  - **Where it points** is read from the drawn frame (so ui.rs / panel /
    feed stay as they are): the first agent: left of the panel, level with
    row `1` (arrow `→`); the first message between agents: under the last
    fold `▸ n messages between k agents` or `│ @ a → b` row (above it near
    the bottom); the first card: above the last `┃ ? name needs you` title
    (arrow `↓`). A hint up whose thing leaves the screen goes away (the
    card answered, the fold scrolled or gone). The box: rounded, accent
    border, `card_tint` ground (the mockup's tinted note), 36 columns of
    text, keys in accent.
  - **Hooks (sb.rs, event handling only):** `apply_state`: an agent that
    isn't main → FirstAgent; a card → FirstCard, no card → `used`;
    `ingest_for`: a live (`ready`) `sb msg` / `sb msg-in` line in the feed
    in view → FirstLevel3; `focus` on an agent → `used(FirstAgent)`;
    `handle_input`, a message sent → `user_message()`. `run.rs`:
    `hints::draw(app, f)` after the frame, before `asciify`. `lib.rs`:
    `mod hints;`.
  - **The steer hint** (⚠ proposed): `Hint::FirstSteer` (`✓ the agent got
    it · ✓✓ it read it.`, anchor: a `› … ✓✓` row) is there but not
    triggered: BISE-15 (track F) calls `once` where it sets `Mark::Read`
    and drops its `#[allow(dead_code)]`, if main keeps it. Told bise-f-feed.
  - **Differences with the mockup:** one hint at a time (a later one
    waits its turn, it is not lost); the note is drawn over the feed (it
    can cover a line of text, like the mockup's tip).
  - **Tests:** unit (store keeps other keys, off without a store, once →
    seen only when it comes up, across a restart, a never-shown one comes
    back; wrap + accent keys; the boxes' places; a TestBackend frame: no
    thing → nothing, a card title → the card hint above it and seen, the
    card gone → the hint goes, then the waiting level-3 one comes up).
    `tui_onboarding_tmux.py` goes on after the onboarding: spawn → the
    agent hint, ⌥1 → gone; the card hint, a message → gone; two agents
    talking → the level-3 hint; `hints.json` holds the three keys
    (captures 7–9 in `$SB_ONBOARDING_SHOTS`).

### BISE-70 · images UI

- **status:** done (the history screens wait for F's 3 call sites) ·
  **owner:** bise-i-images · **commits:** 16bc2a2
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
  - **Composer chips:** the text keeps `[Image #N]`; `editor::layout_input`
    makes it one cell (`InputCell.chip`, `w` = width of `▣ N`), `ui.rs`
    draws it `▣ N` in accent (reversed under the cursor, selection bg when
    selected). Atomic: `prev/next_grapheme` step over it, `move_to` never
    lands inside, `delete_back/forward` (all units) widen to the whole chip
    (`attach::chips / chip_around / chip_widen`). This touches `editor.rs`
    beyond its render path (the grapheme/delete/move functions): no other
    track owns them.
  - **Strip** (`attach::strip_lines`, drawn by `ui.rs` above the status row,
    both layouts): `attached · backspace on a chip removes it`, then one row
    per chip still in the text, `▣ 1 shots/login-mobile.png` and, flush
    right, dim, `1170×2532 · 310 kB` (+ ` → resized to fit 2048`). Sizes are
    what the model gets (after the downscale); resized = the stored bytes
    differ from the original (the store keeps them as is otherwise). Size
    text: decimal units (`999 B`, `310 kB`, `1.1 MB`). A long path keeps its
    end. `Attachment` gained `info: Info` (source, w, h, bytes, resized).
  - **Hint** while images are attached (sb view, not during a turn):
    `ctrl+v paste image · @ file` (from the mockup; **new string for §17**).
    The flash says `attached ▣ 1` (was `[Image #1]`).
  - **History, for track F** (render.rs, agreed with bise-f-feed):
    `attach::chip_spans(line, style)` (markers → accent `▣ login.png`;
    clipboard images, whose path is in the image store, → `▣ clipboard`),
    `attach::sizes_line(text)` (the dim size line under the user line; sizes
    read once from the stored copy beside the `.b64`, cached),
    `attach::result_spans(preview)` (`result · ▣ screenshot.png 390×844`),
    `attach::without_markers`, `attach::no_vision(err)` (the §17 line; the
    model is the agent in view, set by ui.rs each frame; matches provider
    wordings: image/vision + not supported / only supported / invalid
    content type…). They carry `#[allow(dead_code)]` until F wires them.
  - **Tests:** `attach::tests` (chips by char index, widen, strip text:
    sizes + resized, strip rows flush right + cut path, history chips /
    sizes / result line / missing store file, no-vision matching and the
    §17 line), `editor::tests::image_chips_are_atomic`.
    `tui_images_tmux.py`: composer waits now expect `▣ 1 ▣ 2 and ▣ 3`, the
    strip rows, backspace removing a chip whole; the feed check accepts
    `[Image #1 shots/red-blue.png]` or `▣ red-blue.png` (before/after F);
    its own `wait_composer` leaves out the new hint.
  - **Differences with the mockups:** the chip has no background tint (book
    §5: we never paint the background); the strip has no box border (plain
    rows at the composer's indent, like the card box region). The history
    screens (chips in your line, the size line, `result ·`, no-vision) show
    once F calls the helpers. Routing (`with both images`) is main's prompt,
    not here.
  - **Gates** (worktree of 1ee22b9 + my files, own target dir): build,
    clippy workspace all-targets clean, `cargo test --workspace` (two flaky
    failures under load, `switchboard::core` and `bend-plugins` bridge,
    pass alone), `PROOF.bend` OK, e2e OK, tmux images/composer/at-files/
    help/term/version/clear/archived OK. `tui_tmux.py` fails at HEAD on
    `# Task \`t1\`` (BISE-12 folded the brief; someone has a fix in the
    tree). **Found:** the committed `sb-core` is stale vs `hub/*.bend` at
    HEAD (6 `core::tests` fail with it, pass with a fresh
    `bend hub/main.bend`); and `/tmp/bise-gate-target` is shared by all
    tracks' gates, so one track's build overwrites another's binary; use one
    target dir per track.

---

## Wave 3 — sweeps

### BISE-80 · vocabulary sweep

- **status:** done (TUI side) · **owner:** bise-k-keys (TUI), bise-h-hub
  (hub) · **commits:** f812423 (TUI)
- **track:** S · **owns:** any user-visible string, one file at a time
- **do:** `rg -n -i 'task'` over user-visible strings of `rust/tui` and the
  hub's messages to the UI; "agent" everywhere (code identifiers may stay).
- **notes:**
  - **TUI:** `/agents` is the command in the TUI list, with the
    description agreed with bise-h-hub: `list the agents and what they do`
    (the hub keeps `/tasks` as a hidden alias, d4ae5b1). `tui_tmux.py`
    types `/agents`. Every other user-visible "task" had already gone in
    BISE-41 (help, command lines). What is left says "task" only in code:
    identifiers, the `# Task` brief parser in render.rs, and render.rs
    mapping the hub's `task stopped` to `agent stopped`.

### BISE-81 · lowercase and copy-deck sweep

- **status:** done (TUI side) · **owner:** bise-k-keys (TUI), bise-h-hub
  (hub) · **commits:** f812423, 574f904 (TUI)
- **track:** S
- **do:** every UI string lowercase (§4 exceptions); every string in §17
  matches exactly; list the strings not in §17 in the notes so main adds
  them.
- **notes:**
  - **TUI, lowercased (f812423):** voice notices and errors (`voice mode
    on. press ctrl+r to start recording.`, `voice mode off.`, `voice mode
    is off: /voice turns it on`, `no speech detected`, `voice
    transcription failed: {err}`, `voice transcription needs an API key:
    set {VAR}`, `no audio input device found.`, `audio backend is
    unavailable: {err}`, `the last words may be missing (the
    transcription did not finish in time).`, `no audio detected from the
    microphone — check your terminal has mic access.` + ` grant access in
    System Settings → Privacy & Security → Microphone.`); the interrupt
    lines (`… · ctrl+c again to quit`); `@{name} is archived: its history
    is read-only · /restore brings it back · esc → main`; the terminal
    panel (`terminal · ctrl+\` hide`, its hint `terminal: keys go to the
    shell · ctrl+\` hide · wheel/shift+pgup scroll · drag the border to
    resize`); the solo client's status and hint rows (`ask anything…`, `⏎
    steer · tab queue · ctrl+c interrupt · / commands · end bottom`); the
    help footer (`type to filter · tab switch · esc close`). **574f904:**
    two French leftovers: `nothing to steer with: type the text after
    steer`, `couldn't connect: {err}`.
  - **§17 check** (every backquoted string of §17 looked up in
    `rust/tui/src`): all found as written, except `♡ turn done ·
    {duration}` and the two `provider down` lines: the TUI has no such
    line yet (features to build, not a wording fix).
  - **Not in §17 (for main to add):** the strings above; from BISE-41/43:
    `theme: {mode}.`, `theme: {mode}. /theme auto, light or dark to change
    it.`, `theme: {mode}, for now: i couldn't save it ({err}).`, `/theme
    takes auto, light or dark.`, `drop {name}? its history stays in
    archived. y / n`, hint `y drop · n or esc keep`; the confirm hint `y
    yes · n no · esc cancel` and `answer y (yes) or n (no), then ⏎`;
    `display cleared — scroll up to see the earlier lines again`; the
    command descriptions of `/help`.
  - **Left as is:** `keyprobe` (a diagnostic, `bend-harness keyprobe`)
    still says `Option/Cmd`, `Ctrl+C`. `onboarding.rs` and `hints.rs` were
    already lowercase (bise-o-onboard). render.rs / markdown.rs had nothing
    to change outside bise-f-feed's table work.
  - **Gates** (each batch on HEAD + its files in a private worktree, shared
    target, `SB_CORE_BIN` = the worktree's): build, `cargo test
    --workspace`, clippy `--workspace --all-targets -D warnings`,
    `run_all.sh`. Its switchboard step failed twice (4 core tests) because
    another worktree was building in the shared target at the same time
    (bise-h-hub confirmed it). It passed after touching the sources, then
    e2e and every tmux test passed.

### BISE-82 · visual QA

- **status:** done · **owner:** bise-c-cards · **commits:** e0b7e3a
- **track:** S · **owns:** no code; screenshots under
  `projects/switchboard/docs/brand/qa/`
- **do:** every screen of `tui-screens.html`, reproduced in the real TUI, in
  Ghostty dark and a light terminal; a table screen × OK / differs (what).
- **notes:**
  - Done in tmux (not Ghostty: no Screen Recording permission) at
    `217b648`, dark / light (`BISE_THEME`) / `BISE_ASCII=1` / onboarding,
    with `qa/capture.py` (throwaway hub + fake provider, re-runnable);
    captures as `.ansi` + `.html`, PNG for the cited ones (headless
    Chrome). The report: `qa/visual-qa.md` (16 differences ranked, a
    table per screen, the colors vs §5).
  - Top differences: every report shows twice in main (card line + report
    line); Info / Warn lines wrap at column 1; a scrollbar on the feed; an
    agent's reply at column 1 in its own view; a message typed during a
    turn stays `·` after the answer; the `/` popup cuts its descriptions;
    compaction lines (`≡` running, raw `44 (manual)`, no `▸`); help key
    caps paint raw colors (told K); book §6 still says `✉` for `@`.
  - Light regressions found on `9c79318` (main's replies in the dark text
    color, popups in dark dim / faint / accent), fixed by BISE-83
    (`6a3a770`, `bb3f2f9`), confirmed at `217b648`.
  - n/a with the fake provider: main's own words (change your mind, sends
    work back, summaries), drop / overlap / restart cards, voice, images
    from tools, 30 agents, 50k lines, narrow tables.

### BISE-83 · remove deprecated theme aliases

- **status:** done · **owner:** bise-k-keys · **commits:** 6a3a770,
  bb3f2f9, da63fa7, 7d827f3
- **track:** S · **owns:** `theme.rs` and whatever still uses an alias
- **do:** remove the `#[deprecated]` aliases of BISE-01; every file uses the
  roles.
- **notes:**
  - **Aliases gone** (theme.rs, and lib.rs's `#![allow(deprecated)]`).
    Users moved to the roles: 6a3a770 (ui.rs, help.rs, commands.rs,
    sb/versions.rs, voice_ui_tests.rs), bb3f2f9 (markdown.rs, after
    bise-f-feed's BISE-87). Map: `BRAND`/`WARN`/`RECORDING` → `accent()`,
    `TEXT`/`ACCENT`/`HEAD` → `text()`, `DIM`/`INFO`/`TOOL` → `dim()`,
    `OK` → `ok()`, `ERR` → `error()`, `SELECTION` → `selection_bg()`,
    `ON_BRAND` → `on_accent()`, `BORDER_ACTIVE`/`FAINT` → `faint()`,
    `SYNTAX_*` → the syntax roles, `PANEL`/`ELEMENT`/`DIFF_*_BG` →
    `Color::Reset`. The solo client's braille spinner is `working_frame`
    (`∿`, `~` in ASCII mode). The alias test left with the aliases.
  - **Colors:** dark is unchanged by construction (each alias was the dark
    value of the role it maps to). Light changes, and that is the fix: the
    aliases were fixed to the dark palette, so assistant replies (markdown),
    popups, /help and /version drew dark colors on a light terminal
    (bise-c-cards' BISE-82 light pass saw it). They now read the light
    palette.
  - **da63fa7:** the /help key caps were raw `#eeeeee` on a painted
    `#3a3a44`; now accent + bold, no background (book §5). The Ghostty
    config lines use `syntax_string()`.
  - **Glyphs (7d827f3):** literals that have a §6 constant go through
    `theme::glyph()`: /version marks (`✗` `G_FAILED`, `○` `G_IDLE`), the
    solo `○ disconnected` (`G_IDLE`), the @ popup's folder mark
    (`G_CLOSED`). The rest are chrome with no §6 entity: `·` separators,
    `…`, `⏎ ← ↑ → ⌥ ⇧`, `● ◉ ◆ ✚ ◀ ▪ ×`, the direct-message `⇄`, the
    flash `✓`, and the `const` help rows. Those stay literals, and
    `asciify` (the C1 safety net) rewrites them in ASCII mode.
  - **ASCII check** (`BISE_ASCII=1`, tmux on a throwaway hub, the tree of
    7d827f3): start, `/` popup, /help and /shortcuts, /version, the @
    popup. No non-ASCII but letters and box drawing, except `§` inside
    commit subjects (text, like `é`).
  - **Gates:** each area ran build, `cargo test -p bend-tui` and clippy
    `--workspace --all-targets -D warnings` on its exact tree, with my own
    target (`/tmp/bise-k-target`, deleted after). Then the full gate on
    HEAD 7d827f3: `cargo test --workspace`, `run_all.sh` (PROOF, e2e, the
    13 tmux tests): green.

---

## Added by main

### BISE-23 · stable panel numbers, `waits {name}`

- **status:** done · **owner:** bise-p-chrome · **commits:** ecb4bec
- **track:** P · **owns:** `sb/panel.rs`; the `Nav::Goto` path and the
  agent fields of `apply_state` in `sb.rs`; `waiting_on` in the hub
  (`hub/view.bend`, `hub/main.bend`, `sb-core`, `core.rs` snapshot,
  `model.rs` Agent)
- **spec:** book §8 (agents panel: numbers never change while an agent
  lives; `waits docs`)
- **do:** (a) a slot map: each live agent keeps its number while it lives;
  Alt+N goes by that number. (b) the hub sends `waiting_on` (who the agent
  waits on) in the agent state; the panel shows `waits {name}`.
- **done when:** numbers survive a drop (test); `waits x` shows when an
  agent waits on another (throwaway hub); gates green.
- **notes:**
  - **Numbers:** `Sb::numbers()` (panel.rs) keeps a slot map in the panel's
    per-frame state (`PanelHits.slots`, no new `Sb` field): main 0; an agent
    keeps its number while it lives (stopped/failed too, archived frees it);
    a newcomer takes the smallest free number, so a fresh session numbers in
    creation order. `Nav::Goto` (sb.rs) uses `Sb::agent_numbered(n)`. Rows
    stay in the hub's order: after a drop and a newcomer, the panel can read
    `2 b, 3 c, 1 d` (the number is the key, not the rank).
  - **waiting_on:** in sb-core's view (`hub/view.bend` `waits_on`: the
    recipient of the message behind the agent's first waiter, i.e. its `sb
    wait` / `sb ask`; `view()` now takes the waiters, `hub/main.bend` passes
    `X.R.waiters`), read by `core.rs::load_view` into `model::Agent.waiting_on`
    and sent in the snapshot as `waiting_on` while the agent waits (null
    otherwise). No law touched; PROOF ALL PROOFS CHECK. sb-core rebuilt
    unstripped (`bend hub/main.bend -o`), read and run by the tests without
    being killed. The TUI reads it in `apply_state` (sb.rs, one field);
    the panel says `waits {name}` (plain `waiting` without it).
  - **Tests:** `panel::tests::numbers_survive_a_drop` (drop, Alt+3/1/0, a
    newcomer takes 1, a restored agent gets a free number), `waits docs` in
    `panel_rows_at_28_and_40`, core `a_waiting_agent_says_who_it_waits_on`,
    tmux `tui_waits_tmux.py` (throwaway hub: `3 … t3 • waits t2` while t2
    sleeps, drop t1 keeps `2 t2` / `3 t3`, Alt+2 → t2). Not added to
    `run_all.sh` (not mine): main may add it.
  - **Also fixed:** `tui_tmux.py` asserted the brief text in an agent's feed;
    since BISE-12 the brief is folded (`◇ brief ▸`): it checks `brief`.
    `tui_version_tmux.py` passes in my gate (HEAD 063285b + my files and at
    c51d8e6): not reproduced; the stale `SB_CORE_BIN` of the live hub's env
    may explain the failure seen elsewhere.
  - **Seen, not done:** a working agent's `12m` comes from the snapshot's
    `turn_ms`, refreshed only when the hub sends a new state: it can sit at
    `0s` for a while (the TUI could add the time since the snapshot).
  - **Gates** (private worktree, shared target): build, `cargo test
    --workspace` (switchboard 98), clippy `--workspace --all-targets` clean,
    PROOF ALL PROOFS CHECK, `run_all.sh` e2e + every tmux test PASS (+
    panel_click, waits). The working-tree `sb-core` is still the old one
    (HEAD has the new one): left alone.

### BISE-84 · glyph fallbacks and `BISE_ASCII`

- **status:** done · **owner:** bise-t-theme · **commits:** 1a9179d, 601bc3c (fix: 1a9179d had swept in BISE-60 hunks of run.rs)
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
  - **C1 amendment (OK'd by main):** `G_*` stay `&'static str` constants
    (the Unicode forms). New: `ascii_mode()` (`BISE_ASCII=1|true|yes`, read
    once; per thread under `cargo test`), `glyph(G_X) -> &str` (the ASCII
    form in ASCII mode, else `G_X`), the table `ASCII: &[(&str, &str)]`, and
    `asciify(&mut Buffer)`: a safety net after each draw (run.rs +1 line in the
    `terminal.draw` closure) that rewrites only the table's glyphs, one cell to
    one cell. It returns at once when the mode is off. Letters, accents, CJK,
    emoji, quotes, box drawing and block elements are never touched (tested).
    `working_frame` / `starting_frame` / new `compacting_frame` switch too.
  - **Replacements:** `G_MSG` `✉`→`@`, `G_COMPACTING` `⟳`→`≡` (pulse with
    `compacting_frame`, dim/faint), `G_BUILDING` `⧗`→`Δ`, new `G_WRAP` `»`
    (F switches `code.rs::G_WRAP` to it), **worktree mark `ψ`** (U+03C8).
  - **Worktree mark check:** fontTools cmap and advance = advance of `0` (the
    audit's method), in SF Mono, Menlo, JetBrains Mono and MesloLGS NF: 4 of 4
    fonts installed here (Fira Code and Cascadia are not installed). EAW A,
    `unicode-width` 1. It looks like a fork, and it is not `⌥`. Other passes: `Ψ`,
    `¥`, `Y`. Misses: `⋔ ⑂ ⅄ ᛘ ⎌` (0 fonts), `↱` (2), `⊢ ⊻` (1).
  - **ASCII forms (one cell each, `✓✓` two):** `›` > · `:*` :* · `◇` + · `∴` . ·
    `$` $ · `λ` \\ · `↳` - · `±` % · `@` @ · `▣` # · `?` ? · `≡` = · `▲` ^ · `»` > ·
    `·` . · `∿` ~ · `…` : · `♡` + · `✗` x · `○` o · `–` - · `✓` v · `✓✓` vv ·
    `•` * · `ψ` Y · `⇄` = · `↻` ! · `Δ` ^ · `▸` > · `▾` v. The net also covers
    the old glyphs (`✉ ⟳ ⧗ ⎇ ↪`), chrome/hints (`✦ ◀ ▶ ● ◉ ◆ ✚ ▪ × ⏎ → ← ↑ ↓
    ⇧ ⌥ — −`) and the braille spinner (`~`). In ASCII mode `…` and `·` in
    agent prose become `:` and `.`, `»` becomes `>` (accepted: table glyphs only).
  - **Tests:** every `G_*` is its documented width (1, `:*` and `✓✓` 2) in
    both modes and ASCII in ASCII mode; the table maps one cell to one cell,
    no duplicates, no letters but `λ ψ Δ`; every non-ASCII `G_*` has an ASCII
    form; `asciify` leaves `é ñ ü 漢字 👍 « “”` and box drawing alone.
  - **By hand:** TUI with `BISE_ASCII=1` on a throwaway hub (tmux, e2e env):
    start, spawn a task, preview, `/` popup, `/help`. No non-ASCII on screen
    except letters and box drawing.
  - **For BISE-83 (the sweep to `theme::glyph()`):** hard-coded glyph
    literals are in `sb/panel.rs` (`∿ ♡ ⎇ ▸ ▾ ⌥ …`), `sb/cards.rs` (`✗ ♡ ⇄ ↻ –`),
    `sb/versions.rs` (`✓ ✗ ● ○ ◉`), `ui.rs` (`✓ ● ○ ◆ ⇧ ← ↑ ↓ ⏎`), `render.rs`
    (`◀ → − └`), `sb.rs` (`◀ ⇄ ✚ ⏎ →`), `help.rs` (`✦ ▸ ⏎ → ↑ ↓ ←`),
    `commands.rs` (`▸ ▪`), `code.rs` (`↪`/G_WRAP, `−`), `attach.rs` (`× →`),
    `sb/client.rs` (`⏎`), `term.rs` (`↑`), `keyprobe.rs` (`→ …`) and the `·`
    separators in many hint strings. The net covers all of them today.
  - **Other tracks told:** P fixed its `sb/panel.rs` test that hard-coded `⎇`
    (lands with BISE-23). F got `G_WRAP` and `compacting_frame`. O knows about
    the run.rs line.
  - **Gates** (private worktree, shared target): build ok; clippy clean;
    PROOF ok; e2e all PASS; test --workspace: all ok except
    `panel_rows_at_28_and_40` (the `⎇` literal above, fixed by P). tmux: help,
    term, version, at_files, images, clear, archived OK; composer is flaky
    under load (1 fail, 1 pass with my change; passes on HEAD); `tui_tmux`
    fails on HEAD b5eb670 **without my change too** (the brief now folds,
    BISE-12).

### BISE-62 · the theme choice is saved

- **status:** done · **owner:** bise-o-onboard · **commits:** 8e56af4
- **track:** O · **owns:** the saved-choice part of `theme_detect.rs`
  (track T is done), `onboarding.rs` step 2
- **spec:** book §5 (a setting to force one), §15 step 2
- **do:** the theme picked in onboarding step 2 and by `/theme` is saved in
  `~/.bend-harness/tui.json` and reused at the next launch; precedence
  `BISE_THEME` > saved choice > auto-detect; tests with a temp HOME.
- **done when:** tests of the store and the precedence; gates green.
- **notes:**
  - `theme_detect.rs`: `"theme": "light" | "dark" | "auto"` in
    `~/.bend-harness/tui.json`, next to `/voice`'s `voice_mode_enabled`
    (other keys kept). `init()` uses `startup_choice(BISE_THEME, saved)`.
    Store: `settings_path`, `saved_from`, `with_theme`, `load_in(home)`,
    `save_in(home, choice)` (all take the home: tests use a temp dir).
  - **For K (/theme, BISE-41):** `theme_detect::choose(Choice::parse(arg)?)`
    → `(Mode, Result<(), String>)`: switches now and saves; on `Err` the
    mode is switched anyway, say it wasn't saved. Marked
    `#[allow(dead_code)]` until K called it (5ff22eb; dropped since). Told
    bise-k-keys.
  - Onboarding step 2: `enter` saves `auto` when the pick is what the
    terminal gave (dark with no answer), else the pick, so a kept default
    keeps following the terminal. `esc` / `ctrl+c` there don't save.
  - Tests: `the_choice_is_saved_next_to_the_other_settings`,
    `env_beats_the_saved_choice_beats_detection` (theme_detect), step 2's
    test saves in a temp HOME (onboarding).

### BISE-43 · `D` asks first

- **status:** done · **owner:** bise-k-keys · **commits:** e08966f
- **track:** K · **owns:** the `D` key arm and a `drop_ask` field in
  `sb.rs`, its init in `sb/client.rs`, the drop branch of `status_line` /
  `hint` in `sb/panel.rs`
- **spec:** book §16 (`D` drops the selected agent, asks first)
- **do:** TUI side only: `D` on an agent shows a one-line confirm in the
  status row; `y` sends the existing `/drop`, `n` or `esc` cancels; test.
- **done when:** nothing is dropped before `y`; tests; gates green.
- **notes:**
  - `D` on a live agent (never main, never an archived one) sets
    `Sb.drop_ask`; the status row says `drop {name}? its history stays in
    archived. y / n` (accent, `panel::drop_question`), the hint `y drop · n
    or esc keep` (**new string for §17**). `y` (no ctrl/alt/cmd) sends
    `/drop {name}` as before; `n` and `esc` keep the agent; any other key
    drops the question and does its usual job, so nothing gets stuck. The
    hub is unchanged (`/drop` typed still drops at once).
  - **Tests:** `sb::…::d_asks_before_dropping` (reads the hub end of a
    socket pair: nothing sent on `D`, `n`, `esc`; `/drop docs` on `y`; main
    is never asked about). `tui_tmux.py`: after `D` it waits for the
    question, then types `y` (the drop is still checked).
  - **Gates** (worktree of 682338a + exactly this commit, shared target,
    `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace`,
    clippy `--workspace --all-targets -D warnings`, e2e and the 11 tmux
    tests: green.
  - **Also (fix of 5ff22eb):** 0bb65b4. A `-U0` patch had put a test body
    inside a doc comment, so bend-tui tests did not build at 5ff22eb/7fe7cc7.
    Now each commit is the exact tree that was gated.

### BISE-85 · replayed history has time marks; stale sb-core in gates

- **status:** done · **owner:** bise-h-hub · **commits:** (this commit)
- **owns:** `daemon.rs` (the `history` page), `wire.rs` (`HistLine`,
  `parse_history`), the `history` arm of `dispatch` (sb.rs),
  `sb/feed.rs` `prepend_page` + `hhmm_at`
- **spec:** book §10 (time marks), §21 C2 amendment: history timestamp
- **do:** (1) find why `core_tests::a_waiting_agent_says_who_it_waits_on`
  and `tui_waits_tmux.py` fail at 8122fa8. (2) the hub sends each replayed
  history line's transcript time; the TUI draws `· hh:mm ·` marks in it.
- **notes:**
  - **(1) cause:** not the code. HEAD's `sb-core` blob (ecb4bec) has
    `waiting_on`; both tests pass in a clean worktree of HEAD. Two stale
    binaries made them fail: (a) an agent's shell inherits `SB_CORE_BIN`
    from the live hub, which runs version 2a43d58 (before 5d13667 stopped
    passing it to REPLs): `cargo test` runs the core tests against that
    old sb-core; (b) the shared tree's `sb-core` is the c03071e blob:
    ecb4bec committed the new binary through a private index, so the
    working file never changed (it shows `M sb-core`). `sbd` runs the
    sb-core of its app root, so the tmux tests run from the shared tree
    use that old file. Fix: gate in a worktree with `SB_CORE_BIN` set to
    its own sb-core (or unset); after committing an sb-core through a
    private index, copy it into the shared tree if the file there is still
    the previous blob.
  - **(2) wire:** a `history` page line is `{pos, line, ts?}` (`ts`: ms
    since the epoch, from the transcript stamp; missing if it does not
    parse). TUI: `wire::HistLine { pos, line, ts: Option<u64> }`, read by
    `wire::parse_history(&Value)`; `prepend_page` calls
    `feed::pause_mark(.., gap, || hhmm_at(ts))` before a line whose `ts`
    is `PAUSE_MS` or more after the previous one (the first line of a page
    gets none: its gap with the older page is not known). `hhmm_at`: local
    time via `date -r` (macOS) / `date -d @` (GNU), UTC fallback. The REPL's
    own `history ` lines (`--resume`) carry no time: no marks there.
  - **Tests:** `daemon::tests::history_lines_carry_their_time`,
    `sb::bench::replayed_history_gets_its_time_marks` (a mark before the
    line after a 5-minute pause, a line without `ts` still reads).

### BISE-86 · `✗ not delivered` (hub signal + mark)

- **status:** done · **owner:** bise-h-hub · **commits:** (this commit)
- **owns:** `hub/core.bend` (`undelivered.*`, `undelivered_msgs`,
  `stop_noted`, the error arms of `says.sent` / `route.sent`), `sb-core`;
  `wire.rs` (`Mark::Failed`, `Ev::Undelivered`), the `undelivered` arm of
  `parse_hub_line` and the ⏎/esc hook in `sb.rs` `key`, one `push_event`
  arm (feed.rs), `mark_span` + one `ev_lines` arm (render.rs)
- **spec:** book §13 (marks), §17 (copy deck), §21 C2 amendment
  `undelivered`
- **do:** the hub says when a message from the user cannot reach its agent;
  the TUI marks the line `✗` and asks `⏎ send again · esc drop`.
- **notes:**
  - **Hub:** `sb undelivered : {name} : {text}` (fields escaped like
    `answered`) when the user's send fails with `recipient_unavailable`
    (an agent whose worktree was dropped: a user message still revives a
    stopped or archived agent in the shared folder), in the feed where the
    user wrote it (`says.sent`: the agent's; `route.sent`: the focus); and
    for the user's messages still queued when an agent stops (`stop_noted`
    wraps `stop_task` at its three call sites: drop, drop with worktree,
    `sb stop`; the line goes to the message's `via` view, else the agent's).
    `stop_task` itself is unchanged, so its proofs are too. An agent's
    failed send gets its error only. PROOF: ALL PROOFS CHECK; sb-core
    rebuilt unstripped.
  - **TUI:** `Ev::Undelivered { name, text, open }`. `push_event` marks the
    newest matching `Ev::You` (`text`, or `@name text`) `Mark::Failed`,
    or puts `@name text` back marked when the feed does not have it (from
    main's view, a routed line shows only once delivered); only the newest
    question stays `open`. On an empty composer, ⏎ sends it again (`@name`
    from another view, echoed as your line) and esc drops it.
  - **Tests:** core `a_message_the_user_cannot_deliver_says_so` (own view,
    main's view, queued at drop, an agent's send), TUI
    `a_message_not_delivered_is_marked_and_asks`, tmux
    `tui_undelivered_tmux.py` (throwaway hub: `/new -w t1`, `/drop t1`,
    `@t1 …` → `✗` + the question, ⏎ again, esc), added to `run_all.sh`.

### BISE-87 · markdown tables

- **status:** done · **owner:** bise-f-feed · **commits:** de1b2d7
- **track:** F · **owns:** `markdown.rs`; the `md_lines` call sites in
  `render.rs`; `wrap_line` in `feed.rs`
- **spec:** book §11 (Markdown), the look from marketing (m_881; screen
  "markdown tables" in tui-screens.html, 8eca2bc)
- **do:** render GFM tables in messages: bold header, a faint rule, columns
  aligned by display width, `:---:` / `---:` honored, inline markdown in
  cells, fit to the width (shrink the widest columns, wrap inside a
  column), too many columns → one block per row; never inside a code
  fence; `BISE_ASCII=1` keeps it ASCII.
- **done when:** tests for alignment, wide chars, wrap, the narrow
  fallback, a half-streamed table; bench not slower; gates green.
- **notes:** `markdown::md_lines(text, prose, wide)` replaces `md_to_lines`:
  it wraps prose at `prose` itself and lays tables out at their natural
  width up to `wide` (the code measure, 100), so no later wrap cuts a table
  row (a reply's rows now use the code measure in `render::ev_rows`; its
  prose still wraps at 76). A table = a row with `|` then a delimiter row
  with as many cells; its body runs to a blank line or a line without `|`;
  never inside a fence. Cells split on `|` outside backticks (`\|` is a
  pipe); a short row gets blank cells, extra cells are dropped. Look (as
  marketing's spec): no frame, 2 spaces between columns, the header bold
  (following its column's alignment), one faint `─` segment per column
  (`-` under `BISE_ASCII=1`), cells with inline markdown in the text color.
  Too wide: the widest columns shrink to one cap, never under max(8, the
  longest word up to 12); cells wrap by words in their column, no wrap mark,
  then one blank line between the rows of that table. Still too wide: one
  block per row, the first cell as a bold title, then `  key  value` (keys
  dim, padded to the longest; the value wrapped at the prose measure under
  its column), a blank line between blocks. A header whose delimiter hasn't
  arrived yet stays a plain line (streaming). `feed::wrap_line` keeps a soft
  (continuation) row soft when it wraps it again, so pre-wrapped prose
  still copies as one line. Level 2, answered, report and brief bodies
  keep the prose measure for their tables. **Tests** (markdown.rs
  `table_tests`, TestBackend): `columns_align_as_the_delimiter_says`,
  `wide_chars_align_by_display_width` (CJK, emoji),
  `a_long_cell_wraps_inside_its_column` (and the blank lines, the code
  measure), `too_many_columns_turn_into_blocks`,
  `cells_keep_their_inline_markdown` (bold, code, `\|`, a pipe in code),
  `a_streamed_half_table_never_panics` (every prefix at widths 1–76),
  `no_table_inside_a_code_fence`, `column_widths_fit`. The ASCII rule isn't
  unit-tested: `ascii_mode` is read once per process. **Bench** (50k
  lines, release, the same machine, base and mine interleaved: the load
  average was 10–34 during the runs): in the quiet runs replay 88.5 → 86.2
  ms, steady 0.26 → 0.24, PageUp 0.46 → 0.89/0.45, PageDown 0.50 → 0.50,
  windowed PageUp worst 2.96 → 1.47 ms, total 1471 → 1453 ms: not slower.
  An earlier version wrapped a reply twice (md_lines, then ev_rows): the
  second pass is skipped now. **Gates** (worktree of b4cf2e7 + this change,
  the shared target): build, `cargo test --workspace`, clippy
  `--all-targets` 0 warnings, run_all.sh: every tmux test PASS after a
  fresh build (tui_term, tui_composer and a plugins bridge test failed once
  on artifacts mixed in the shared target / a full disk, and passed rebuilt).

### BISE-88 · code quality pass (last, with a budget)

- **status:** done · **owner:** bise-quality · **commits:** c1cdc16,
  5b8b550, ef2f8e1, 5be38a0, fd88a3a, f138fc3, c945f17
- **track:** S · **owns:** the code the bise work touched (`rust/tui`,
  `rust/switchboard`, `hub/*.bend`), one area at a time
- **when:** after BISE-80–83 and BISE-82's fixes, before main's final
  integration gate
- **do:** review what the bise waves added or changed (`git diff 4282501..HEAD`)
  with the `code-quality` and `codebase-design` skills: dead code, duplicated
  logic, modules that grew too big (`sb.rs`, `render.rs`, `feed.rs`), hidden
  state, `unwrap`/panics on input, tests that test nothing, missing tests on
  risky paths. Write the findings first (`docs/brand/qa/code-quality.md`,
  ranked by risk × cost), then fix from the top until the budget runs out.
- **budget (hard):** at most ~1500 changed lines in total and 3 hours of work;
  one refactor per commit, each gated; no behaviour change, no contract change
  (C1–C4, the hub line protocol, the journal), bench not slower. What doesn't
  fit the budget stays listed in the findings for main.
- **done when:** findings written; the fixes that fit are committed and gated
  (build, test --workspace, clippy --all-targets -D warnings, run_all.sh,
  PROOF if a `.bend` changed); the rest listed with a cost estimate.
- **notes:**
  - Findings: `docs/brand/qa/code-quality.md` (F1–F10, ranked by risk ×
    cost). Fixed, one commit each: F1 `run_all.sh` checks every test
    binary of `switchboard` **and `bend-tui`** (its tests never ran there),
    clippy `--workspace --all-targets`, `SB_CORE_BIN` unset (ef2f8e1); F2
    the throwaway hubs and TUIs drop the agent's `SB_CORE_BIN`,
    `SB_SOCKET`, `SB_AGENT`, `SB_TASK`, `SB_PORT_OFFSET` (5b8b550); F3
    `tui_version_tmux` on its own `XDG_STATE_HOME`, no real build
    (fd88a3a); F4 `tui_composer_tmux` waits for the reply (5be38a0); F5
    dead code and the module-wide allow in `theme.rs` (c945f17); F6
    `sb::key` moved to `sb/keys.rs` as is (f138fc3).
  - Listed with a cost (for main): F7 tmux flakes under load (1–2 h);
    F8 `feed::push_event` / `render::ev_lines` split (300–600 lines +
    bench, half a day); F9 `onboarding.rs`, `sb/panel.rs`, `editor.rs`
    (500+ lines each); F10 `versions.sh` leaks `/tmp/sb-build-*`
    worktrees (50 registered: an `EXIT` trap + `git worktree prune`).
  - Used ~690 changed lines of 1500, ~1 h 45 of 3 h. Gates: each commit
    build + `test -p bend-tui` (317) + clippy `--workspace --all-targets
    -D warnings`; `run_all.sh` on 4e050e8 green (PROOF, Rust 317 + 100,
    e2e, every tmux test); bench 50k not slower (a60735a 0.24 / 0.46 /
    1575 ms, c945f17 0.24 / 0.43 / 1555 ms, bise-f-feed's runs). No
    `.bend` changed.

### BISE-80/81 · hub side (vocabulary, lowercase)

- **status:** done · **owner:** bise-h-hub · **commits:** d4ae5b1 (Rust),
  (this commit) (hub/*.bend)
- **owns:** the hub's strings to the UI and the user (`rust/switchboard`,
  `hub/*.bend`); the TUI side is bise-k-keys'
- **notes:**
  - **/agents** is the user command (the board of every agent); `/tasks`
    stays an alias (router). Help row: `/agents  list the agents and what
    they do` (agreed with bise-k-keys, used verbatim in the TUI list).
  - **agent, never task** in what the user reads: /help rows, usage lines
    (`/drop <agent>`, `sb stop <agent> "<reason>"`…), `agent {name} created`
    (sb spawn), the spawn line `{who} → new agent @{name} …` (transcript.rs
    still reads `new task` and `nouvelle tâche` in old journals), `no agent
    named @{name}`, `the agent is mid-turn`, the card result `agent stopped`
    (the TUI maps the old `task stopped` too), notes to main (`the user
    created / dropped / restored the agent @x`, `agent @x crashed …`,
    `agent @x failed: …`), empty boards `no agents yet` (§17). Lowercase:
    `drop @x? … [y/N]`.
  - **Left as is:** identifiers, wire keys and journal fields
    (`task_created`, `sb tasks`, `SB_TASK`, `<task_board>`); model prompts
    (prompts.rs) and the brief's `# Task \`name\`` header (a model prompt,
    and the TUI's brief detection reads it).
  - **Strings not in §17** (main to add): the /help rows; `no agent named
    @{name} — did you mean @a, @b?`; `{who} → new agent @{name} (worktree
    sb/{name}) : {objective}`; `@{name} archived` + ` — worktree deleted` /
    ` — work saved (/restore)` / ` — worktree NOT deleted: {why}`; `drop
    @{name}? {n} changed files and {m} unpushed commits will be saved
    (/restore) [y/N]`; `drop of @{name} cancelled`; `main → @{name}
    stopped: {reason}`; `recipient_unavailable: @{name} is {status} (its
    worktree was deleted: /restore)`; `unknown command: {cmd} (see /help)`;
    `empty message for @{name}`; `--with-changes only works with -w`; the
    usage lines; `no agents yet` on /agents.

### BISE-89 · queued messages

- **status:** done · **owner:** bise-f-feed · **commits:** 87c0774
- **track:** F · **owns:** new `queue.rs`; the queue hunks in `app.rs`,
  `sb/feed.rs` (View), `sb.rs` (`ingest_for`, `send_input_to`), `run.rs`
  (solo loop), `input.rs` (tab, ↑), `ui.rs` (the rows above the
  composer), `sb/panel.rs` (the row count, the during-turn hint); new
  `tests/tui_queue_tmux.py`
- **spec:** book §13 (queued messages), §16, §17; after Codex
  (`bottom_pane/pending_input_preview.rs`, `chatwidget/input_flow.rs`,
  `input_restore.rs`); screen "queued messages" (marketing 484f543)
- **do:** a message queued for after the turn stays in the TUI, listed
  above the composer; ↑ pops it back to edit; the queue sends in order
  when the turn ends; one queue per agent; honest about persistence.
- **done when:** unit + tmux tests; gates green.
- **notes:** Codex lists queued inputs above the composer and pops the last
  one with shift+← / alt+↑; it sends one per turn. Here: `tab` during a
  turn queues the composer text **and its images** (nothing goes to the
  hub); rows above the composer: ` › text…` dim (one line, cut to the
  width), the 5 newest (`+ n more` above), then the faint `queued · sent
  when this turn ends · ↑ edit`. `↑` in an empty composer pops the newest
  (before the history); `tab` queues it again, `⏎` steers it now, clearing
  drops it. Codex's keys clash here (alt+↑ = previous agent, shift+← =
  selection). When the turn ends (`--- idle`), the oldest goes out as a
  normal message through the same path as ⏎ at idle (marks `·` → `✓✓`);
  the feed is marked pending at once, so the next waits for the turn that
  one starts. One queue per feed (`App.queued`, swapped with the View); an
  agent out of view still gets its queue (`Sb::send_input_to`); the solo
  client the same. Panel row: faint `· n queued`. During-turn hint: `tab
  queue · ⏎ steer · ctrl+c interrupt`. **Not persisted:** a TUI restart
  drops the queue (old lines never fire at an idle agent after a
  restart); the book says so. **Tests:** `queue::tests` (tab/↑/tab, the
  oldest at turn end one per turn, the rows above the composer, a queued
  image keeps its image), `chrome_tests` hint updated,
  `tui_queue_tmux.py` (throwaway hub, a slow `sleep 8` turn: nothing sent
  while queued, `· 2 queued`, ↑ edit + tab, sent in order after the turn,
  the queue empties), added to `run_all.sh`. **Gates** (own target
  `/tmp/bise-f-target`, worktree of 20f6732 + this change): build, `cargo
  test --workspace`, clippy `--all-targets` 0 warnings, run_all.sh green;
  rebased on e01177d (K's panel title): build + queue/chrome tests green.

### BISE-91 · chrome fixes from the visual QA

- **status:** done · **owner:** bise-k-keys · **commits:** 432d477,
  31ac636, e05cd9b, 97f3786 (shots)
- **track:** K · **owns:** `ui.rs` draw_popup, `sb/panel.rs` header counts
  + hint + panel title, `theme.rs` ASCII table + `ellipsis()`, `render.rs`
  truncate_chars / fit_chars (OK'd by bise-c-cards), `help.rs` key text,
  book §6 ASCII table
- **spec:** qa/visual-qa.md items 6, 11, 12, 14; book §6, §8
- **notes:**
  - **(6) 432d477:** the `/` popup and the `/version` picker are as wide as
    their widest line (at least 56 / 72, at most the prompt's width), so
    descriptions show whole.
  - **(11) 31ac636:** the hint row drops `alt+r answer with text · ctrl+x
    later · ctrl+f full screen` while the card box is shown (the box
    carries them); it shows the normal hint instead.
  - **(14) 31ac636:** the header fits its counts to the room left: every
    count with its word, else numbers only, then the least important go
    first ("needs you" stays, then working, waiting, done), always in the
    §8 order. Test `a_narrow_header_keeps_needs_you_first`.
  - **(12) e05cd9b:** each §6 entity has its own one-cell ASCII form:
    brief `&`, thinking `:`, sub-call `L`, wrap `}`, waiting `;`, done `*`,
    stopped `_`, unread `!`, overlap `/`, restart failed `(`, building `A`,
    closed / open `+` / `-` (the rest unchanged). A cut text ends with
    `...` in ASCII mode (`theme::ellipsis()`, used by `truncate_chars` /
    `fit_chars`), so `…` never reads `:` or `;` in prose. The panel title
    and the help keys say `alt + number` (source level: the cell net
    would give `M`). Box drawing stays. Book §6 has the table. Test
    `every_entity_has_its_own_ascii_form`.
  - **Shots (97f3786):** recaptured on e05cd9b with `qa/capture.py dark`
    and `ascii`: dark 04–09, 22, 24, 29 and ascii 02, 03, 29 (the others
    left as bise-c-cards made them). In that session only one agent is
    counted, so dark/29 still reads `? 1`: correct. The fit logic is
    covered by the test.
  - **Gates:** each commit ran build, `cargo test -p bend-tui` and clippy
    `--workspace --all-targets -D warnings` on its exact tree (own target
    `/tmp/bise-k-target`, deleted after). The full gate on e05cd9b:
    `cargo test --workspace`, `run_all.sh` (PROOF, e2e, the 13 tmux
    tests): green.

### BISE-92 · bise paints its own background

- **status:** done · **owner:** bise-o-onboard · **commits:** see git log (BISE-92)
- **track:** O (main's call) · **owns:** `theme.rs` (ground role, the paint
  pass), `theme_detect.rs` (OSC 11 set / restore), `crash.rs`
  `restore_terminal` (+1 call), `run.rs` (`draw_frame`), `onboarding.rs`
- **spec:** book §5 (the background rule, rewritten), §21 C1 (`bg()`)
- **do:** every cell of every frame gets the theme's ground; the terminal's
  own default background follows (OSC 11) and is always given back;
  `/theme` and the onboarding switch repaint at once; `BISE_ASCII` and tmux
  still fine.
- **done when:** no cell keeps a Reset background in any view (test); the
  restore sequence (test); tmux captures; gates green.
- **notes:**
  - **Ground:** `Palette.bg`, `theme::bg()` returns it: dark `#141211`,
    light `#fdfbf7` (a lighter cream than `#f7f4ee`: on `#f7f4ee` no light
    selection/card tint was both visible and ≥ 4.5:1 for dim and accent).
    Light tints moved to stay visible on it: selection `#fdeef2`, card
    `#f1eee6` (dark unchanged). Tests: every role ≥ 4.5:1 on the ground,
    each tint ≥ 1.08:1 from the ground and ≥ 1.03:1 from the other.
  - **The pass:** `theme::paint(buf)` after each frame (before `asciify`):
    a cell left at `Color::Reset` gets the ground (bg) or `text()` (fg), so
    no widget can leak the terminal's color, including the 4
    `bg(Color::Reset)` still in `markdown.rs` (111, 118) and `ui.rs` (516,
    730) and the embedded terminal's cells; those lines need no edit. The
    old `PANEL` / `ELEMENT` / `DIFF_*_BG` aliases are gone since BISE-83.
    `run::draw_frame(app, f)` is the one frame (view, hints, paint,
    asciify); the onboarding paints too (its previews use `palette.bg`).
  - **The terminal's background:** `theme_detect::sync_terminal_bg()` before
    each frame sends `OSC 11 ; #rrggbb` when the mode changed (start,
    `/theme`, the onboarding's switch); `restore_terminal_bg()` in
    `crash::restore_terminal` (exit, panic hook, the shell path) sends `OSC
    111`, then the color read at start (`OSC 11 ; rgb:…`) when we have it;
    the next frame sets it again. `BISE_TERM_BG=0` turns the OSC 11 part off.
    There is no suspend path to cover: in raw mode ctrl+z is a key (no
    undo), not SIGTSTP; SIGSTOP can't be caught.
  - `/theme` and the onboarding: the pass reads the mode each frame, so
    the ground changes at once; the onboarding clears the feed's color cache
    when the mode changed during it (`/theme` already did).
  - **Tests:** `run::paint_tests` (TestBackend, dark and light: main with
    agents, a card and level-3 lines, the card box, the `/` popup, `/help`,
    inside an agent: no Reset bg or fg cell); `theme` (ground contrast,
    tints, `paint` keeps set colors); `theme_detect` (set / restore
    sequences, the restore color parses back). `tui_onboarding_tmux`
    checks the dark ground in a `capture-pane -e`. Not checked by hand in
    Ghostty / Terminal.app (no screen here): the OSC 11 part is the one to
    look at there.

### BISE-93 · the composer block

- **status:** done · **owner:** bise-f-feed · **commits:** 2e317f5
- **track:** F · **owns:** `ui.rs` `draw_bise` (layout) and `draw_composer`,
  `ui::composer_block`; the composer tests and tmux readers
- **spec:** book §13 "the composer block" (marketing 393dbd3; the foot of
  every screen in site/book/screens.html)
- **do:** status row, queued list, images strip, then the composer with the
  bar on every row, blank bar rows around the text, text from column 3,
  growth then scrolling, hints on their own row; small-terminal fallbacks.
- **done when:** tests updated; gates green.
- **notes:** Bottom up: hints row (dim, flush right, 2 columns of margin),
  the composer block, the status row, the images strip, the queue. The bar
  ` │ ` on every row of the block: faint while empty, accent with text or
  while recording. Text from column 3, right margin 2, wrapped at
  min(width − 5, 73) (your message's width in the history); at least 2
  rows, up to min(12, 40% of the height), then it scrolls with the cursor
  row. Empty: the cursor at column 3 and the placeholder; recording: the
  meter at column 3. `ui::composer_block(h)`: < 24 drops the bottom blank
  row, < 18 the top one (1 text row allowed), < 14 the hints go on the
  status row. The solo client keeps its own layout. **Tests:**
  `composer_wrap_tests` (the block ends the screen: text, blank row,
  hints), chrome `first_run_screen`; tmux `tui_composer_tmux.composer()`
  reads the bar rows, `tui_queue_tmux` reads the block (it no longer imports
  tui_composer_tmux, whose import reset `tui_tmux.S`); the level-3 waits in
  `tui_tmux`, `tui_archived_tmux`, `tui_at_files_tmux` accept `→ name`
  (BISE-90 a5b841f shows the receiver, not the id). **Gates** (own target,
  worktree of 106f7ae + this change): build, `cargo test --workspace`,
  clippy 0 warnings, run_all.sh: every tmux test PASS (after the queue test
  fix).

### BISE-94 · onboarding layout and emphasis

- **status:** done · **owner:** bise-o-onboard · **commits:** see git log (BISE-94)
- **track:** O · **owns:** `onboarding.rs`, `tests/tui_onboarding_tmux.py`
- **spec:** book §15 'Layout' (marketing 393dbd3), mockup
  `site/book/onboarding.html`
- **do:** one centered content column, the block at 2/5 from the top,
  titles / notes / key lines emphasis, options `›`, small terminals.
- **done when:** unit tests + tmux test follow the layout; gates green.
- **notes:**
  - `column(area)`: 64 wide, centered; width − 8 when narrower, width − 4
    under 50 columns. Welcome and theme center their lines in it; model,
    folder and how-it-works are left-aligned in it. The block sits at 2/5
    of the free rows from the top; the dots stay 2 rows above the bottom.
    The theme previews stay centered on the whole screen (2 × 44 columns
    don't fit in 64), as in the mockup.
  - Emphasis: `title()` bold text color; notes dim; `keyline("{enter} ok ·
    {o} another folder")`: dim, keys in text color (never faint). 2 blank
    rows after the title and before the key line (`gap_of`: 1 under 22
    rows), 1 between options and lines. Options: `option()`: the selected
    `›` accent + its name bold, the others indented 2, sub-lines dim and
    indented 4, wrapped with their indent. The selection tint and the `▎`
    bar are gone.
  - Welcome: `hi, i'm bise` bold + `:*` accent bold (after the ` ·` pop);
    the gloss dim; the tagline text; `press enter ↵` dim with `enter` in
    text, typed. how-it-works: the title is `how it works, in three lines:`
    in bold; the key line `enter, and say what's on your mind.` comes last.
  - **Not done:** OSC 66 text sizing (`hi, i'm bise :*` at scale 2): main
    sends it later (marketing checks which terminals support it); bold
    meanwhile.
  - Tests: the unit tests read phrases across wraps (`flat`); the gloss is
    dim; the folder note on its own row when the path is long.
    `tui_onboarding_tmux` asserts on the joined rows too.

### BISE-90 · feed fixes from the visual QA

- **status:** done · **owner:** bise-c-cards · **commits:** a5b841f,
  7c032cd, 3510e00 (book §8), 1c13817 (QA shots); capture.py fix 557c52e
- **track:** C · **files:** `render.rs`, `feed.rs`, `wire.rs`, `ui.rs`
  (the scrollbar block + one line), `sb.rs` (`focus_name`), `usage.rs`,
  `feed_render_tests.rs`
- **spec:** BISE-82 list items 1, 2, 3, 4, 5, 7, 10 (main m_974), the
  user bar (main m_988, marketing m_1004), book §8 spacing (m_1074)
- **notes:**
  - **1 report + card:** `feed::merge_report_and_card`: blocked keeps the
    level-1 card (it fades when answered, BISE-31), done / failed keep the
    report line (`♡ bench: … ▸ report`); whichever comes second takes the
    first one's place (in place, near the tail) or is dropped. A card
    close with no card in the feed adds nothing (the BISE-31 info
    fallback is gone).
  - **2 hang:** `glyph_line` takes the width and hangs its rows
    (`hung_rows`): info (`· ✚ …`), warnings (`▲ …`), errors, done /
    failed card lines; an opened report too (marketing's bug a).
  - **3 scrollbar:** only while scrolled up from the bottom, faint `┃`
    thumb, no arrows, no track, never at the tail; book §8 note.
  - **4 reply inside an agent:** 1-space lead, so it starts at the glyph
    column (mockup "inside an agent").
  - **5 marks:** cause: the hub steered your message together with an
    agent's message and the `<task_status>` block, so the steered text is
    that block (your words on a later line) and never matched. An
    unmatched steering mark now raises the marks of what you sent since
    the turn started (`feed::mark_this_turn`). Verified in the QA
    recapture (dark/17 `·`, dark/18 `✓✓`).
  - **7 compaction:** `Ev::Compact` (unit) draws `≡ compacting`, its `≡`
    pulsing dim / faint while it runs (`Live::Compacting`, stops at the
    summary); `Ev::Compacted { text, open }` draws `≡ summary ▸`, the
    summary under the rail once opened (ctrl+o, clicks).
  - **10 `to` column:** a message the feed's owner received names it
    (`→ main`; inside an agent, its name) via `render::set_feed_owner`
    (set in `ui.rs` next to `set_main_feed`) and `Sb::focus_name`.
  - **user bar:** your messages: `│` accent at column 0 on every row, text
    from column 3, marks unchanged; `|` under `BISE_ASCII=1`.
  - **gates:** build, `cargo test -p bend-tui` (295), clippy
    `--workspace --all-targets -D warnings` on 44acc36 + the patches;
    `run_all.sh` green on the same tree (the tmux waits for `→ main` were
    already made tolerant by BISE-96).

### BISE-96 · bash and TypeScript: a box

- **status:** done · **owner:** bise-k-keys · **commits:** 5461388, d7394b8
  (shots)
- **track:** K · **owns:** new `toolbox.rs`; `render.rs` tool_lines and
  tool_head (split agreed with bise-c-cards); `feed.rs` build_rows /
  event_rows / refresh_live / push_event hooks for tools, tool_discloses
- **spec:** book §11 "Scripts: a box", §9 Emphasis;
  site/book/screens.html "bash and typescript: a box"
- **notes:**
  - A bash or TypeScript call is a rounded box at `code_width(width)` (it
    follows bise-f-feed's CODE_MAX of BISE-97). The title is in the top
    border: `╭─ $ bash ∿ 12s ─╮`, `✓ 0.9s`, `✗ 0.8s`. The border is text
    while running, faint when done, error when failed. The script shows
    in full (highlighted, wrapped with `»`), then a faint `├──┤`, then the
    output, dim. Closed: running and failed show `… n lines above` + the
    last 15; done shows the first 5, `▸ n more lines`, the last 9; 15
    lines or fewer show all. Opened (▸ via click, `space`, `ctrl+o`):
    everything. `tool_discloses` is "more than 15 result lines", so
    `toggle_event`, `set_everything` and `anything_closed` cover it.
  - TypeScript sub-calls (`Ev::Sub`) are output lines inside their box,
    shown in full before the result. A new Sub clears its box's cache, and
    the Sub itself draws no row of its own.
  - A running box redraws only its top border (the live head, 1 row), so
    the per-frame work is as before.
  - ASCII (`BISE_ASCII=1`): `+- $ bash ok 0.9s ---+`, `|`, `+---+`,
    `> n more lines`, `... n lines above`.
  - The separate `▸ output · n lines` line and the code rails are gone
    for bash/ts. Other tools stay one line, now dim (§9 Emphasis: the
    one-line tool calls). Edits keep their `±` line (edit_head is
    bise-c-cards').
  - **Tests:** `toolbox::tests` (title and width, head/tail fold, running
    and failed tail, border colors, sub-calls inside, ASCII box).
    `feed_render_tests` follow the box: the mockup tests compare rows
    without the box padding (`unbox`); the output/failure tests now check
    the box for bash and one line for other tools. `theme.rs` got a
    `cfg(test)` `set_ascii_for_tests`.
  - **Shots (d7394b8):** `qa/capture.py` dark, light and ascii on
    5461388 (temp HOME and XDG_STATE_HOME). Every screen that shows a
    bash/ts box was recaptured.
  - **Gates:** lite (build, `cargo test -p bend-tui`, clippy
    `--workspace --all-targets -D warnings`) on the committed tree after a
    rebase on HEAD. One `at_popup_tests` timing flake passed on rerun.
    Full gate on 5461388: `cargo test --workspace`, `run_all.sh` (PROOF,
    e2e, 14 tmux tests): green.

### BISE-98 · the frame (app look)

- **status:** done · **owner:** bise-f-feed · **commits:** a455133
- **track:** F · **owns:** `layout.rs`, `ui.rs` (draw_bise, the divider, the composer pane), `sb/panel.rs` (rule, header), scrollbar
- **spec:** book §8 "The frame" (exact layout), §13 "The composer pane"; mockups site/book/screens.html + live.html (marketing 9f000c8). User request on 805e538.
- **what:** a faint rounded frame on the terminal edge with `bise :*` and the summary in the top border; 2 blank columns inside; the panel behind a faint rule joined with `┬`/`┴`; a full-width divider `├─ you → main ─…─ state ─┤` replacing the status row; the composer bar at column 3, text at 5; the scrollbar thumb on the panel rule (or the right border); small-terminal tiers (no frame under 60×16). Lands after BISE-97, on its layout function. The key bar row itself is BISE-99.
- **notes (F):** `layout.rs`: `cols(w, h)` and `rows(w, h)` hold the numbers (FRAME_W 60 / FRAME_H 16, PAD 3, BARE 1, the panel tiers with their rule, PAD_TOP_FROM 20 / PAD_BOTTOM_FROM 24, MIN_TEXT 2 / MAX_TEXT 12). Framed: text from column 3 to F−4; panel text F−31..F−4 with the rule at F−33 (90–99: 24 wide, rule F−29), the history ends 3 columns left of the rule; the column is still 79 wide at most and centered from 83. The new `chrome.rs` draws the rounded frame (`+ - |` in ASCII), the title `bise :*` and the summary in the top edge (`Sb::title` / `Sb::summary`: `~/path · counts`, the path goes first, then the short counts), the rule joined with `┬` / `┴`, and the divider `├─ you → name ─…─ state ─┤` (`sb::status_state`: the old status row without the name; `↓ back to the bottom · end` while scrolled up, clickable; voice and flash notes). Under 60×16: header row on row 0, a plain divider with the same label, margins 1. The composer bar at column 3, text at 5, 1 blank row each side by height; the queued lines (` › ` under the bar) and the strip between the divider and the composer; the key bar row is `keybar::line(app, F−6)`, also on the solo screen. The scrollbar thumb (dim `┃`) is drawn on the panel rule, else on the frame's right edge (`draw_feed(.., bar)`). Placeholder: `what's on your mind?` / `talk to {name} directly` (§17 rows added). `ui::hint_text`, `composer_block` and the sb branch of `draw_status` are gone; `sb::hint` is test-only now, `term::HINT` and `attach::STRIP_HINT` carry `#[allow(dead_code)]`: bise-k-keys deletes the three. Tests: layout tiers/rows, `chrome::fit`, header/summary, first run and inside-an-agent screens (frame, divider, composer, key bar), composer wrap ends at the key bar. Tmux assertions changed: `tui_tmux` (`in_view` reads the divider; new `pane_rows` reads the composer from the bottom, the placeholder left out; key bar strings), `tui_composer_tmux.composer`, `tui_queue_tmux.composer_row` and the steer keys, `tui_onboarding_tmux` NORMAL. Bench (50k lines, release): windowed PageUp to the top 1525 / 1532 ms vs 1506 ms at HEAD (noise), frames ≤ 1.3 ms vs 1.85 ms, steady 0.26 ms. `tui_term_tmux` flaked twice under load in run_all (its `wait_screen("RED")` matches the typed command), passes alone; the final run_all is green.

### BISE-99 · the key bar and the tip

- **status:** done · **owner:** bise-k-keys · **commits:** a8ffe09, 16886fa
- **track:** K · **owns:** a pure `keybar` function (keys + tip → styled line for a given width and mode), `hints.rs` / `help.rs` for the tip texts; F places the row (BISE-98).
- **spec:** book §8 "The frame" (key bar line), §13 "The composer pane".
- **what:** the key bar from column 3: keys in text color, what they do dim, 3 spaces between pairs; default `⏎ send   @ agent   ⌥0-9 switch   / commands   ? help`, per-mode sets as today; on the right, ending at F−4, a dim tip, one per session (`tip · ctrl+o opens everything folded`), hidden while you type or when fewer than 3 columns separate it from the keys. ASCII forms per §6.
- **notes (K):** a8ffe09: new `rust/tui/src/keybar.rs`. Agreed with bise-f-feed: F calls `keybar::line(app, width) -> Line<'static>` on its row (width = the row, x = column 3, last column F−4); the pure part is `keybar::render(mode, width, typing, tip)`. `keybar::Mode` covers every set of the old hint row (terminal panel, voice, `@` file popup, images, drop ask, confirm, card full, selected, archived, steer, default, and the two sets without switchboard). `sb::key_mode` picks the switchboard modes in the order of `sb::hint`. Pairs that don't fit are dropped from the end. The tip shows in the default mode only, while the composer is empty, with 3 free columns. It is one of `help::TIPS`, chosen once per run; a test checks that each tip starts with a key of the help. ASCII: `enter`, `alt+0-9`, `left` / `right` / `up/down`, `...`, `tip: `. The module is `#[allow(dead_code)]` until BISE-98 calls it. 16886fa (after F's a455133 placed the row): `sb::hint`, `term::HINT` and `attach::STRIP_HINT` deleted, the allow dropped, the tests read `sb::key_mode`. Gates on 1daa2f7 + 16886fa: build, `test -p bend-tui` (317), clippy `-D warnings`; `run_all.sh` on d744bee + the same patch: PROOFS, e2e and every tmux test green but `tui_images_tmux`, a race of the test that is not from this patch (it reads the screen once `▣ red-blue.png` shows, and the fake provider's `ack:` echo with the raw `<image name=…>` marker may already be drawn); F's BISE-88 test patch covers it. QA: no capture of my own (16886fa draws nothing new): bise-c-cards' recapture on 1daa2f7 (dark, light, ascii) shows the key bar and the tip.

### BISE-97 · the global layout

- **status:** done · **owner:** bise-f-feed · **commits:** 3d9ec56
- **track:** F · **owns:** `layout.rs` (new), `ui.rs` `draw` / `draw_bise`,
  `sb/panel.rs` `split` / `draw_panel`, `render.rs` measures
- **spec:** book §8 "the reading column" and "spacing, in cells"
- **done when:** every block of the screen takes its x and width from one
  layout function; the width tiers; tests; gates green; bench not slower.
- **notes:** `layout::cols(width)`: margins 2 (1 under 64 columns), the
  feed area, the reading column (79 wide, centered when the feed area has
  ≥ 83 columns, else at the left margin; tables and code run to 103 from its
  x), the panel by tier (≥ 100: 28 wide, gap 3, names cut at 16; 90–99: 24,
  gap 2, cut at 12; < 90: none). `layout::margin_rows` (1 top and bottom
  from 30 rows) and `layout::header` (the header row 1 column before the
  margin, 1 blank row under it): the numbers are constants in layout.rs, so
  BISE-98 changes them in one place. The screen: header, then feed | blank
  | card | blank | status | queue | strip | composer | hints, all on the
  column; the panel has no left border (title, 1 blank row, rows); the
  scrollbar is the feed area's last column; the pinned banner wraps to 2
  rows. `PROSE_MAX` 79, `CODE_MAX` 103. Fixed on the way: main's `:*`
  reply was wrapped twice at 80 columns (one-word rows) and its rows under
  the first sat 1 column left of its text; the header overflowed a
  1-column buffer (fuzz). `sb/cards.rs` (agreed with bise-c-cards):
  `keys_hint` leaves room for the short scroll hint. **Tests:** layout
  unit tests, `mains_reply_wraps_once_under_its_text`,
  `prose_wraps_at_79_and_code_at_103`, panel / chrome / composer / bench
  updated. **Tmux assertions changed** (behaviour still checked): the
  composer readers (`tui_composer_tmux.composer()`,
  `tui_queue_tmux.composer_row()`) and `tui_tmux.in_view` find the rows at
  the column's x, not column 1. **Bench** (50k lines, release):
  windowed PageUp to the top 1467–1476 ms after vs 1532–1574 ms before,
  frames ≤ 0.51 ms. **Gates** (private worktree of HEAD + this change):
  build, clippy `--all-targets` 0 warnings, `cargo test -p bend-tui`,
  `run_all.sh` green. Two things outside this change: `cargo test -p
  switchboard` has 12 `core::tests` failing at HEAD (the crate is not
  touched here; run_all only greps the first result line); and
  `tui_version_tmux` fails when HEAD is already built in the shared
  `~/.local/state/switchboard/versions` (it switches with no "building"
  line): run it with `XDG_STATE_HOME` set to a temp dir.

### BISE-95 · level 2 reads bigger (emphasis)

- **status:** done · **owner:** bise-c-cards · **commits:** 1daa2f7
  (shots: a qa/ commit after this one)
- **track:** C · **owns:** `render.rs` l2_lines, answered_lines,
  report_lines, thinking_lines, the style of the `Ev::Assistant` arms
  (their widths and wrapping are bise-f-feed's, BISE-97); `feed.rs`
  wants_gap_before, new is_l2
- **spec:** book §9 "Emphasis" (marketing 82f1742);
  site/book/screens.html "what's for you reads bigger"
- **notes:**
  - The speaker of level 2 is bold: main's `:*` in accent bold (replies
    in main's feed, `:* …` messages, `:* docs asked … i answered …`);
    `@ name to you:` in text bold, the body plain text; a report's
    `name:` bold when done, failed or blocked (a progress line stays dim
    and not bold).
  - A blank row above and below every level-2 block (reply, answered,
    message to you, report), even between two of them and after the
    reply's thinking `∴` (it used to stick to it). The lines of a
    level-3 run still sit together.
  - Already dim before this issue, now tested: thinking `∴`, level 3.
    The one-line tool calls were made dim by bise-k-keys in BISE-96.
    Cards (level 1) unchanged.
  - Differs from the mockup: the book makes `@ name to you:` bold
    including the `@`; the mockup leaves the `@` plain. The book wins
    (text color either way). The mockup's `♡ auth-fix is done` line in
    "a wide terminal" is not bold; the book's rule (the speaker of level
    2 is bold) makes `auth-fix:` bold.
  - **Tests:** `render::emphasis_tests` (the speaker bold, the agent's
    own work dim, the blank rows around level 2).
  - **Gates:** on 4b98b7c + the patch: build, `cargo test -p bend-tui`,
    clippy `--workspace --all-targets -D warnings`, `run_all.sh` green
    (`tui_term_tmux` failed once: the shell's printf output was not
    there yet when it checked; it passed alone). Rebased cleanly on
    d744bee (BISE-98): build, `cargo test -p bend-tui` (317), clippy
    green on that exact tree.
  - **Shots:** `qa/capture.py` dark, light, ascii on 1daa2f7 (the new
    frame of BISE-98 and this emphasis), temp HOME and XDG_STATE_HOME.

### BISE-100 · done is a pink check, not a heart

- **status:** done · **owner:** bise-k-keys · **commits:** 92f12aa + 756c491 (code; 92f12aa's tree came from a stale base and undid f138fc3, fd88a3a, e71ec74, 7bca11e, b62d391: 756c491 restores them, its tree = f138fc3 + the seven BISE-100 files), QA shots cfbdbf1 (dark, light, ascii and the onboarding ones)
- **track:** K · **owns:** `theme.rs` (the done glyph), its uses in `sb/panel.rs`, `render.rs`, `feed.rs`, `sb/cards.rs`, the tests that read `♡`
- **spec:** book §6 (agent status table), §8, §9, §11, §17 (user decision, Gabriel 2026-09-29: the `♡` was not clear).
- **what:** every "done" in the product shows `✓` in accent (pink) instead of `♡`: the panel rows, the header / frame counts (`✓ 4 done`), reports (`✓ bench is done …`), `✓ turn done` when built. ASCII stays `*`. Your read marks (`✓` faint, `✓✓` accent, at the end of your lines) are unchanged: the place tells them apart. `♡` stays only outside the product (site, landing).
- **notes (bise-k-keys):** `theme::G_DONE` is `✓`, drawn through `theme::done_glyph()`: `*` under `BISE_ASCII=1`, because `glyph("✓")` gives `v`, the ASCII form of your read mark (same Unicode glyph, the place tells them apart). `♡ → *` moves to the replaced glyphs of the ASCII table, and a theme test checks that no other glyph reads `*`. The check is accent in: the panel rows (`sb/panel.rs` glyph), the header / frame counts (check accent, `4 done` dim), reports (`report_lines`), done cards in the feed, the card box title (accent check, plain title; the border stays faint) and the `/close` list, the onboarding theme preview. Tests: `♡` → `✓` in feed_render_tests / panel / cards, and the done card's glyph color is accent. No tmux test read `♡`. Gates: build, `cargo test -p bend-tui` 317, clippy `--all-targets -D warnings`; `run_all.sh` green on 4e050e8 (18 PASS, 317 + 100 Rust tests). `✓ turn done` is not built yet (§17).

### BISE-101 · a wider reading column (+15%)

- **status:** done · **owner:** bise-f-feed · **commits:** 7ba4146
- **track:** F · **owns:** `layout.rs` (COLUMN, CENTER_FROM), `render.rs` PROSE_MAX, the composer wrap width, tests
- **spec:** book §8 "The reading column", §11 "Measure" (user request, Gabriel 2026-09-29: the text in the history ~15% wider).
- **what:** the reading column goes from 79 (3 + 76) to 91 (3 + 88); it centers when the feed area is ≥ 95; prose wraps at min(width − margins, 88); your message in the composer wraps at the same width as in the history (as today, so it follows). Code and tables keep their widths (up to 100 / 103). Narrow tiers unchanged (column = min(91, width − 4)). Bench not slower.
- **notes (F):** `layout.rs` COLUMN 91, CENTER_FROM 95 (the tests: 160 centered at x0 18, 130 not centered, 133 centered; tables and code still capped at 103 from x0); `render.rs` PROSE_MAX 91 (3 + 88); the composer wraps at the column less its lead, so it follows. Mockup tests: the replies that fit in 88 stay on one row. Gates: build, clippy 0 warnings, cargo test --workspace (worktree), run_all.sh green on 4e050e8 (bise-k-keys' run, m_1236). Bench (50k, release): HEAD 1534 / 1549 ms windowed PageUp to the top vs 1555 ms before (c945f17), frames ≤ 0.45 ms, steady 0.24–0.29 ms. Follow-up (main, m_1239): `sb/cards.rs` READ_WIDTH 76 → 88, card prose follows the column (its own commit after BISE-102; run_all green).

### BISE-102 · the raised composer (tint + padding)

- **status:** done · **owner:** bise-f-feed · **commits:** dc3be09
- **track:** F · **owns:** `theme.rs` (new palette role `raised`, its fallbacks), `layout.rs` / `chrome.rs` / `ui.rs` (the composer pane rows and tiers, the tinted block)
- **spec:** book §5 (raised), §13 "The composer pane" (exact); mockups screens.html / live.html (marketing e71ec74, 7bca11e). User request 2026-09-29.
- **update (user, marketing 685220f):** the whole pane under the divider is raised (every cell inside the frame, H−2 up to the row under the divider); no blank ground row under the divider; 7 rows at rest; tiers per book §13. The rest of this entry is superseded where it differs.
- **what (first version):** the composer is a raised block (tint `#1f1c1a` / `#f4f0e8`) with a tinted row above and below the text, bar at x0, 1 tinted column, text, 2 tinted columns; 1 blank row under the divider; key bar right under the block; 8 rows at rest, the tiers by height/width; contrast tests on `raised`; after BISE-101.
- **notes (F):** `theme.rs`: a new palette role `raised` (#1f1c1a / #f4f0e8) and `theme::raised()` (under `NO_COLOR`: none, the ground); the contrast tests take it (text, dim, accent ≥ 4.5:1 on it, ≥ 1.08:1 against the ground). Not done: the 256-color (234 / 255) and "ground not ours" fallbacks: the TUI has no color-depth detection, and since BISE-92 it always paints its own ground, which `raised` is measured against. `layout.rs` `rows()`: `pad_top` (the tinted row under the divider, from 20 rows) and `pad_bottom` (the one between the text and the key bar, from 24 rows), `keys_in_divider` under 14 rows. `ui.rs` draw_bise: divider · tinted row · queue · strip · text · tinted row · key bar · edge (7 rows at rest; 6 at 20–23 rows; 4 at 16–19; bare below 16); every cell under the divider inside the frame gets the `raised` background (bare: the full width to the last row), the divider and the edges stay on the ground; the composer's bar, the queued ` › ` lines, the strip and the key bar start at x0 (the reading column's left edge). Under 14 rows the key bar takes the divider's right side (`chrome::divider_room`). Tests: `the_pane_under_the_divider_is_raised` (every cell's background per height, the divider and the edges not raised, the bar faint on the tint, keys in the divider under 14 rows), first-run screen (7 rows), layout rows. Gates in a worktree: build, clippy 0 warnings, cargo test -p bend-tui, run_all.sh green. Bench (50k, release) under a load average of 10–17 (run_alls of other tracks): this change 1659 / 2235 ms, the base 2476 ms, frames ≤ 0.67 ms; no slowdown seen, the load made the windowed totals noisy.

### BISE-103 · `esc back to main` in an agent's view

- **status:** done · **owner:** bise-k-keys · **commits:** 852dea4, QA shots 7f38785
- **track:** K · **owns:** `keybar.rs` (modes of an agent's view), tests
- **spec:** book §13 "The composer pane" (key bar in an agent's view). User request 2026-09-29.
- **what:** in an agent's view, `esc back to main` is the first pair of every key set, never dropped (pairs drop from the right, `/ commands` first); the tip is hidden there.
- **notes (bise-k-keys):** `keybar::render(mode, width, typing, agent, tip)` (new `agent` flag; `line()` sets it when switchboard runs and the focus isn't main). In an agent's view: idle `esc back to main   ⏎ send   @ agent   ⌥0-9 switch   / commands   ? help`; working the book's set `esc back to main   ⏎ steer   ctrl+c interrupt` (no `tab queue` there, as in §13); an archived agent puts `esc back to main` first too (before `/restore brings it back`). Narrow: `/ commands` drops first, then from the right; `esc back to main` stays (cut only when it alone doesn't fit). No tip in an agent's view. The other key sets (selection, `@` file popup, drop / confirm questions, card full screen, recording, terminal, images) keep their own keys: `esc` does something else there. Tests: `keybar` an_agents_view_starts_with_esc_back_to_main, in_an_agents_view_commands_drop_first_and_esc_never; no tmux test asserted an agent view's key bar. Gates: build, `cargo test -p bend-tui` 319, clippy `--all-targets -D warnings`; `run_all.sh` green on 852dea4 (18 PASS, 320 + 100 Rust tests).

### BISE-104 · the tip changes every 5 minutes

- **status:** done · **owner:** bise-k-keys · **commits:** 9771632
- **track:** K · **owns:** `keybar.rs`, `help.rs` TIPS, the timer (in the app state, redraw on change only)
- **spec:** book §8 "The frame" (key bar line). User request 2026-09-29.
- **what:** the tip on the right of the key bar changes every 5 minutes (was: one per session), going through `help::TIPS` in order (start where the last session stopped, saved with the hints if cheap, else start at a random one); never changes while you type; still hidden in an agent's view and when there is no room. No extra redraws beyond the change.
- **notes (bise-k-keys):** `keybar::TipClock` (pure: `start(saved, seed, now)`, `tick(now, typing)`) goes through `help::TIPS` in order; the tip changes 5 minutes after the last change, never while you type (it waits for an empty composer, then changes). It is read at each draw (the UI loop draws every 80 ms anyway): no timer, no redraw of its own. Saved in a file `tip` next to `hints.json` (the index shown; written at the start and at each change); a session starts at the next tip, else at a random one. `hints::store_path` became `pub(crate)` for it (so `SB_ONBOARDING=off` and tests don't save). Still hidden in an agent's view, while typing and without room. Gates (new rule): build, clippy `-p bend-tui --all-targets -D warnings`, `cargo test -p bend-tui keybar help hints` (23, test the_tip_changes_every_5_minutes_in_order). No QA shot: nothing new on screen.


### BISE-105 · the divider shows that the agent you view works

- **status:** done · **owner:** bise-divider · **commits:** the commit that marks it done (`git log --grep BISE-105`)
- **track:** F · **owns:** `chrome.rs` (the divider label), the turn start time from the feed/app state, tests
- **spec:** book §8 "The frame" (divider line). User request via marketing (m_1285, 2026-09-29).
- **what:** when the viewed agent (main or another) is working, the divider label reads `you → name ∿ working · 42s`: the indicator 1 space after the name (today's `∿`, pulsing as in the panel), `working · 42s` dim, seconds of the current turn, updated each second; idle: nothing after the name. Right side unchanged. The indicator glyph will change once the user picks among site/book/working.html (5f68a5c): keep it one constant.
- **notes:** `chrome::Working { mark, age }` passed to `draw_divider`; `sb::viewed_working` gives it for the focused agent when its status is `working` (mark = the panel's glyph for the tick, age = `short_age` of the hub's `turn_ms` + the time since that state came, `Agent::turn_age_ms`, so it moves each second on the existing 80 ms redraw loop). The working part only takes the room the state leaves (the right side is drawn exactly as before): full, then the mark alone, then nothing. Tests: `chrome::tests` (working with age, no age, idle, narrow widths 20-119). Gate: quick green (12 s). The mark became the gust in BISE-107, and the label's drop order became book §9 'Short on room' (`chrome::STEPS`, see BISE-107).

### BISE-106 · level 3 is an envelope chip

- **status:** done · **owner:** bise-chip · **commits:** 9a19579 (tui), the QA shots + this entry
- **track:** F · **owns:** `render.rs` (level-3 lines and folds), `theme.rs` (new role `chip`, ASCII / 16-color forms), tests, QA shots of level 3
- **spec:** book §5 (chip), §9 "Level 3 is an envelope chip" (user pick, marketing 346dacb, site/book/messages.html variant C).
- **what:** each agent-to-agent message = chip ` ✉︎ sender → receiver ` on the `chip` tint, then the dim text with a hanging indent, 2 lines max then `… ▸`; same pair stacks, new pair after a blank row; fold line unchanged; fallbacks per §9. Replaces today's ` │ @ from → to text` line.
- **notes:** `render::l3_lines` → `l3_lines_as(ChipForm, …)` (`Tinted` / `Bracketed`: NO_COLOR via `theme::chip_bg() == Reset`, or `BISE_ASCII=1`); `chip_spans`, `chip_names` (cap 12 / 8 / 6 by W = width − 2, then the receiver cut before the sender, envelope and arrow never), `l3_text_rows` (2 rows then `…` + ` ▸`; a word cut in its middle goes whole; open: every line, ` ▾` at the end), `G_ENVELOPE` = U+2709 U+FE0E (1 column, tested in the buffer), `envelope()` (`@` in ASCII). `l3_long` = several lines or wider than 2 × 30 columns (what may be cut at any W ≥ 34). `theme::Palette.chip` + `chip_bg()`; test `the_chip_reads_and_shows` (text 13.0 / dim 5.9 dark, dim 4.8 light, tint vs ground ≥ 1.08). Spacing: `feed::wants_gap_before` compares the pair (`l3_pair`, either way round, empty `to` = the feed's owner), also inside an open fold. Fold line: x0+2, no rail, cut from the right with `…`, the pulse kept (`fold_line(.., width)`). `hints` finds level 3 by its envelope + arrow. Tests: `render::chip_tests` (tint, bold sender, cut names, hanging indent, 2-row cap, the three widths + compact, receiver-first cut at every width 1-119, brackets for NO_COLOR and ASCII, same pair / new pair, no accent for main, fold line), the old level-3 tests moved to the chip. QA: dark/light/ascii shots showing level 3 recaptured (03, 04-09, 11, 15-18; PNGs of dark 03/04/15/17/18, light 15). Differences with the mockup: none in the layout; the ASCII chip is bracketed and untinted (the §9 ASCII form); no runtime check that the terminal draws ✉︎ 1 wide (the `@` fallback is ASCII mode). Gates: quick green (14 s) per commit, full at the end.

### BISE-107 · working = a gust blowing by

- **status:** done · **owner:** bise-divider · **commits:** the commit that marks it done (`git log --grep BISE-107`)
- **track:** F · **owns:** a small pure `gust` module (frame → cells, ASCII ramp), `chrome.rs` header + divider, `sb/panel.rs` status cell, the animation tick (≤ 10 fps, only those cells, stop when nothing works or focus is lost)
- **spec:** book §6 (working row), §9 "Working = a gust blowing by" (user pick, site/book/working.html variant I). Together with BISE-105 (the divider shows the viewed agent working).
- **notes:** new `rust/tui/src/gust.rs`: the motif is data (`Motif`: frame time, the 5-cell strip and the 3-cell short strip as frames × cells, the 1-cell breath, the still glyph, each glyph's tone and ASCII form); `MOTIF = W1` (user pick), another pick = a table swap. `Motion` (`Frame(n)` from a clock, 110 ms a frame, or `Still`) lives in `App::motion`, set by the draw loop before each draw: `Still` when the terminal lost the focus (crossterm focus reporting, enabled at start, disabled in `crash::restore_terminal`), when the last draw took > 60 ms (`DRAW_BUDGET`) or `BISE_REDUCE_MOTION` is set; still = one static `∿` (ASCII `~`) in text color everywhere; tests draw still, so their `∿` holds. Header: `gust::header_size` (≥ 90: 5 cells, 70–89: 3, < 70: breath) leads the working count (`Sb::summary/header` take the gust). Panel status cell and the `@` popup mark: the breath (`gust::cell`); main's `:*` does not move. Divider (BISE-105 reworked): `chrome::STEPS`, the book §9 'Short on room' order: the state whole or dropped first, then `working · `, 5 → 3 cells, → breath, the seconds, the name cut at 12 then 8; the gust stays. Only the gust cells change between frames (ratatui's diff), ≤ 9 changes a second, none when nothing works; the loop wakes every 80 ms as before (no extra redraws). Other `working_frame` users (render.rs spinner, toolbox, onboarding) unchanged. Tests: `gust::tests` (ramps, widths, tones, ASCII, still, motion rules, header sizes, ≤ 10 fps), `chrome::tests` (each drop step, widths 20-129). Gates: quick green (11 s) per commit, full at the end.

### BISE-108 · the composer with images and several rows

- **status:** done · **owner:** bise-composer-fix · **commits:** the commit that marks it done (`git log --grep BISE-108`)
- **track:** F · **owns:** `editor.rs` (the composer's wrap), `attach.rs` (the strip rows), `ui.rs` (the composer bar), `keybar.rs` (the images key set), tests
- **spec:** book §8 "The frame", §13 "The composer pane" (the bar on every row of the block), §14 Images; user screenshots on 6df3967 (1 image + 1 row, 2 images + 4 rows).
- **what:** (1) the strip showed the whole dropped path cut from the left (`…ar/folders/…/Screenshot … .png`): it shows the file name only (`attach::file_name`), cut at its end with `…` when short on room, the size flush right as before; `clipboard` unchanged. (2) the composer's bar ran only beside the text rows: it runs the whole block, the tinted row under the divider (unless the queue's `›` has that column), the strip rows, the text, the tinted row above the key bar, at x0 (text and strip at x0 + 2); accent as soon as there is text, an attached image or a recording, faint only when empty (`ui::composer_bar_color`). (3) the text wrapped in the middle of words (`le n` / `om complet`): `editor::layout_input` wraps at spaces like the history (a space stays at the end of its row, a space that does not fit takes its word along, a chip is a word; only a word longer than the row is cut); the cursor, clicks and ↑↓ use the same layout. (4) with an image the key bar showed only `ctrl+v paste image   @ file`: it keeps `⏎ send` first, then the image keys, then `⌥0-9 switch   / commands   ? help` (drop from the right; in an agent's view `esc back to main` first). (5) the divider's state and the frame: already right (state ends at F − 4, corners and sides drawn) at 60–260 columns; the screenshots were cropped at the terminal's right edge. Now covered by a test.
- **notes:** tests: `attach::tests::strip_names_the_file_never_its_path`, `composer_wrap_tests::{the_composer_wraps_at_word_boundaries, the_bar_runs_the_whole_block_in_accent_with_text_or_images (also the key bar and the strip at 200/120/80 columns), the_divider_state_never_runs_past_the_frame (60–260 columns)}`.
