# debt-tui · what makes rust/tui hard to change

Read-only audit of `rust/tui` at HEAD `eaddea0` (a private worktree; no edit, no commit).
Builds on BISE-88 (`projects/switchboard/docs/brand/qa/code-quality.md`, F1–F10): nothing
from there is repeated, only updated where the numbers moved.

Size: 27k lines, 351 tests. The biggest files: sb/panel.rs 1504, render.rs 1501,
onboarding.rs 1480, editor.rs 1407, feed_render_tests.rs 1371, sb.rs 1197, feed.rs 1106.
Churn over 30 days (183 commits): sb.rs 67, ui.rs 24, render.rs 23, sb/panel.rs 22, feed.rs 18.
(lib.rs shows 84, but that is the old split; lib.rs is now 99 lines.)

## The short version

One pattern causes most of the pain. **`App` is one big struct (47 `pub(crate)` fields,
19 files take `&mut App`), and it serves two clients at once**: the single-agent TUI and
Switchboard (`app.sb: Option<Sb>`). So:
- the state of "one feed" has no type of its own, and gets copied by hand (item 1);
- every feature asks "am I in Switchboard?" (item 2);
- the "what mode are we in" logic is written 3 times (item 3).

The cheap fixes (A–E) are small. They remove duplication that already hides small
inconsistencies. The bigger refactors (1–5) change how the code is shaped.

---

## Fix now, cheap (≤ half a day each)

### A. "Clear the feed" is written 4 times, and each copy resets different fields — 1 h
- `input.rs:407` (ctrl+l, single agent): events, cache, anchor, scroll, follow, unseen.
- `commands.rs:394` (`/clear`, single agent): the same 6 fields, copied.
- `sb.rs:369` `hub_reconnected`: the same fields + win, pending, interrupt_requested.
- `sb/feed.rs:154` `clear_feed`: + marks, loading, first_pos, **feed_sel**.
Only one copy clears `feed_sel` (the selection in the feed). Either the other copies have
a bug (the selection points at events that are gone), or there is a reason nobody wrote down.
**Fix**: one function (`feed::reset(app)`) plus the Switchboard-only extra.
**Gain**: a new per-feed field gets reset in one place. After item 1 this becomes `Feed::default()`.

### B. Four word-wrap functions — 1–2 h
`help::wrap` (help.rs:291), `ui::wrap_words` (ui.rs:325, the same loop without the long-word
cut), `hints::wrap` (hints.rs:197, adds `{accent}` spans), `onboarding::words_in` (onboarding.rs:744).
Plus `feed::wrap_line` and `code::wrap_code_line_hanging` for styled lines (those are legitimate).
**Fix**: one `text::wrap_words(&str, width) -> Vec<String>` with the long-word cut; hints and
onboarding style its output. **Gain**: one wide-character/emoji fix instead of four.

### C. Glob imports hide where names come from — 1–2 h, mechanical
`lib.rs` does `use theme::*; use wire::*; use render::*; use feed::*; use app::*; use commands::*;
use ui::*; use input::*; use run::*;`, then `app.rs`, `input.rs`, `run.rs`, `commands.rs` do
`use crate::*`, and `sb/*` do `use super::*`. Reading `push_event(...)` in `input.rs` does not
tell you where it lives. A rename or a new helper with a common name can shadow another silently.
**Fix**: explicit `use crate::feed::{push_event, …}`; clippy's `wildcard_imports` does it
file by file. **Gain**: local reasoning. Grep and IDE "go to definition" become reliable.

### D. Hidden global state for rendering — 2–3 h
- `render.rs:216-247`: thread-locals `MAIN_FEED` and `FEED_OWNER`. `ui.rs:368-370` sets them
  before each frame, and `render::ev_lines` / `feed.rs` (5 × `main: main_feed()`) read them.
  The feed cache is correct only because the cache is per view. That rule is a comment, not a type.
- `sb.rs:219` `static SB_MODE: AtomicBool`, set by `sb/client.rs:131`, read by
  `commands::popup_matches` to pick the command table.
- `attach::set_model` (a thread-local, set during draw in `ui.rs:84`).
**Fix**: a small `RenderCtx { main: bool, owner: &str }` passed to `ev_lines`/`event_rows`,
and kept in `EventRows` as the cache key. `popup_matches(input, sb: bool)`.
**Gain**: render functions become pure (tests stop calling `set_main_feed(true)` first,
render.rs:1261-1321). A cache bug cannot come from a forgotten "set before draw".

### E. Agent status is a string — 1–2 h
`Agent.status: String`, compared as text 26 times in `sb/panel.rs` (+5 in `sb/mention.rs`):
`"working"`, `"waiting"`, `"archived"`, … One typo compiles fine.
**Fix**: `enum Status` parsed once in `apply_state`, `Other(String)` for new hub words.
**Gain**: a new status (the hub adds them) shows every `match` that must handle it.

(F7, tmux flakes, is still open: 45 `time.sleep` calls across the 14 `tui_*tmux*.py`. It is
the same F4 pattern. Replace a sleep with `wait_screen(...)` when a test flakes; no big project.)

---

## Bigger refactors

### 1. A feed has no type: 12 fields copied between `App` and `View` — ~1 day. Top priority.
The feed in focus lives in `App` fields. The other feeds live in `sb::feed::View`.
Switching focus runs `swap_feed` (sb/feed.rs:72-85): **12 hand-written `mem::swap`** calls
(events, cache, win, follow, anchor, scroll, unseen, tail_visible, pending,
interrupt_requested, last_line_at, queued). `with_feed` swaps a background feed in, runs
the code, then swaps it back.
- Adding a per-feed field takes **4 edits in 2 files**: the `App` field, the `View` field,
  `View::new`, `swap_feed`. BISE-89 (queued messages, 87c0774) made exactly those 4 edits.
  If you forget the swap, nothing fails: the field just leaks from one agent's view to another's.
- `events` and `cache` are two parallel `Vec`s that must stay the same length and in sync.
  `push_event(&mut events, &mut cache, ev)` takes both (29 call sites in 9 files). There are
  **18 manual `cache[i] = None`** after a change to an event, one of them outside feed.rs
  (`sb/keys.rs:61`). If you forget one, the screen shows a stale line.
- `Ev` mixes data with view state: 5 variants carry `open` / `fold` flags "in memory only".
**Fix**: `struct Feed { events, cache, win, follow, anchor, scroll, unseen, tail_visible,
pending, interrupt_requested, last_line_at, queued }` with methods `push`, `update(i, f)`
(invalidates the cache itself), `reset`. `App.feed: Feed`; `View = { feed: Feed, ed: Editor }`;
`swap_feed` becomes one `mem::swap`. Mostly a mechanical rename (`app.events` → `app.feed.events`,
~300 sites), then move the cache invalidation behind `update`.
**Gain**: the biggest one. It also makes F8 (split `push_event`, 168 lines) easy, because the
rules become methods on `Feed` with their own tests. Run the 50k-line bench before and after.

### 2. Two clients inside one `App` — 1–2 days, and a product decision first
`app.sb: Option<Sb>` is branched on **134 times** (`if let Some(sb) = app.sb…`,
`sb.is_some()`, …). As a result the code has:
- two layouts: `ui::draw` (single agent, ui.rs:50-124) and `draw_bise` (171 lines). Each
  composer feature is written twice: the queue height and the images strip are at
  ui.rs:81-83 and again at ui.rs:148-150;
- two command tables (`commands::COMMANDS`, `sb::SB_COMMANDS`, with 5 entries in both:
  /voice /plugins /help /shortcuts /quit) and two `handle_input` (commands.rs:369, sb.rs:617);
- `keybar::Mode::Solo/SoloSteer`, `help::Scope::Solo/Sb`, and `ctrl+l` with one branch per client.
- In 30 days, 10 commits touched both `ui.rs` and `sb/panel.rs`.
**Decision for the user**: is the single-agent TUI still a product? If yes, it can be
"Switchboard with one agent and no panel": one layout, one command table (with a `scope`
field, like help.rs), and `Option<Sb>` turns into a few precise questions (`has_panel()`,
`focus_name()`). If no, delete it (about −600 lines).
**Gain**: every new composer or feed feature is written once.

### 3. The "mode" is computed 3 times: keys, key bar, help — ~1 day
- `input::on_key` (181 lines) runs a priority chain: help → term → voice → `sb::key`
  (193 lines: drop question, not-delivered question, panel, cards) → `at_nav` → its own
  match → `composer_key`.
- `keybar::mode` (keybar.rs:131) plus `sb::key_mode` (sb/panel.rs:702) run *another*
  priority chain (term → recording → transcribing → file popup → images → drop-ask →
  confirm → card → selected → archived → steer → default) to show which keys work.
- `help::ROWS` holds the key names a third time, and `keybar` has its own strings
  (`("ctrl+o", "open/close all")` vs `"open or close everything folded"`).
- 9 commits in 30 days touched `input.rs` and `sb.rs`/`sb/keys.rs` together.
Nothing checks that the key bar shows the keys that actually work in the current state.
**Fix**: one `mode(app) -> Mode`, computed once per key and once per frame. Key dispatch
does `match mode` and hands off to one small handler per mode. The key bar reads its pairs
from the same `Mode`. **Gain**: a new overlay or question is one enum variant plus one
handler, and the key bar cannot drift from the real keys.

### 4. The hub line protocol is a string format, parsed by hand — ~1 day, across crates
`sb::parse_hub_line` (sb.rs:727) splits `" : "`, un-escapes `" \: "`, and guesses whether
the last word is an id (`is_msg_id`, added by BISE-110 so that older lines still parse).
`wire::parse_line` is another 163 lines of the same kind. Changing one feed line means
editing `hub/core.bend`, `switchboard/cli.rs` and `tui/sb.rs` together. For example,
BISE-86 (c441322) touched core.bend, cli.rs, core_tests.rs and sb.rs.
**Fix**: the hub adds the fields as JSON next to the display text (`{"kind":"msg","from",
"to","id","text"}`). The TUI decodes with serde and keeps the text parser only for old
transcripts. **Owned by debt-hub, see `/tmp/debt-hub.md`**: it covers the whole seam
(the snapshot JSON read field by field in `sb.rs::apply_state`, and the hub lines parsed in
`sb::parse_hub_line` *and* `switchboard transcript::readable`, formatted in `hub/core.bend`,
with no shared type). Only the TUI side is listed here.
**Gain**: a new field in a feed line stops being a three-file migration with guesses.

### 5. Big modules (F9 update) — lower priority than it looks
The line counts include the tests. The production parts are smaller: sb/panel.rs 677,
onboarding.rs 564, sb/cards.rs 555, editor.rs 1004, **render.rs 1172**, feed.rs 1106,
ui.rs 870. The two that really need a split are render.rs (`ev_lines`, one arm per `Ev`
variant; `Ev` has 25) and feed.rs. Items 1 and 3 cut both naturally. Leave onboarding and
panel alone until a change hurts there.
Long functions still in place: `sb::key` 193, `code::highlight_bash` 184, `input::on_key` 181,
`ui::draw_bise` 171, `feed::push_event` 168, `wire::parse_line` 163, `input::on_mouse` 123,
`ui::draw_feed` 117.

---

## Suggested order

| # | what | cost | gain |
|---|------|------|------|
| 1 | A: one feed reset | 1 h | fixes a possible stale selection; prepares for item 1 |
| 2 | C: explicit imports | 1–2 h | every later diff is easier to read |
| 3 | D: RenderCtx instead of thread-locals | 2–3 h | pure render, honest cache key |
| 4 | **1: `struct Feed`** (+ cache invalidated inside it) | ~1 day + bench | the largest gain; unlocks F8 |
| 5 | B + E: one wrap, status enum | 2–3 h | small, can be done at any time |
| 6 | 3: one `Mode` for keys and key bar | ~1 day | key bar cannot drift from the real keys |
| 7 | 2: ask the user about the single-agent client, then merge or delete | 1–2 days | features written once |
| 8 | 4: structured hub lines (with debt-hub) | ~1 day | no more three-file format changes |
| 9 | F8 split of `push_event` / `ev_lines`, then item 5 | ½–1 day | easy after item 1 |

Do the cheap items (A, C, D) before item 1: they shrink its diff. Do item 1 before items 3 and 2,
because both touch the same fields. Run each refactor as one commit, gated by the
`bend-tui` tests plus the 50k-line bench (F8 numbers: 0.24 ms steady, 0.43 ms PageUp).
Ask the other agents editing `rust/tui` to pause while the item-1 rename lands.
