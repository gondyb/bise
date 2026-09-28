# `@` mentions: agents and files in one popup

Status: design (task `at-files`), then implemented in `rust/tui`.

## Goal

Typing `@` in the composer (line start or inline, Switchboard TUI and
single-agent TUI) opens one popup: the live agents first, then the files
and directories of the workspace. A file index built in the background
answers each keystroke in a few milliseconds, even on a large repo.
Picking a file puts a reference in the text that the model understands.

## What Codex and Vibe do

**Codex** (`codex-rs/file-search`, `tui/src/bottom_pane/mentions_v2`).

- Index: one `ignore::WalkBuilder` parallel walk (`hidden(false)`,
  `follow_links(true)`, `require_git(true)` so a `~/.gitignore` above the
  repo does not hide everything) fed into a `nucleo` session. The walk
  runs again for each search session (a session lives while the `@token`
  is not empty); no watcher.
- Ranking: `nucleo` with `Config::match_paths()` on the relative path,
  ties by path. The unified popup (`mentions_v2`) sorts by type first:
  plugins, skills, tasks (agents), then files and directories. The rows
  have a type tag (`File`, `Dir`, `Task`) in a color.
- Inserted text: the **bare relative path** (the `@query` token is
  replaced, no `@` kept), in double quotes when it has a space. Images
  are attached as images. Nothing else is sent: the model reads the file
  with its tools.

**Vibe** (`vibe/cli-rust/src/utils/{file_index,file_match}.rs`,
`vibe/app_server/_workspace.py`).

- Index: `git ls-files --cached --others --exclude-standard` (fallback:
  an `ignore` walk), kept fresh by `notify` (fs events; one git rebuild
  per burst of events). A `BTreeMap<path, is_dir>`.
- Ranking: a lexicographic tuple: exact directory, child of the typed
  directory, exact file name, exact stem, stem prefix, name prefix,
  extension, fuzzy score, shallow path. Empty query or `dir/`: the
  immediate children only. Dot files hidden unless the query starts with
  `.`. Scans at most 32 000 entries.
- Inserted text: `@path` (`@dir/` for a directory). On submit the server
  **inlines the content** of each mentioned file inside the workspace
  roots (max 8 files, 2000 lines, 50 KB each, else a truncation note), as
  resource blocks or a synthetic `read_file` call.

## Choices

### What is sent: the relative path, as text

Picking a file inserts its path relative to the workspace (the TUI's
working directory), without the `@`: `rust/tui/src/app.rs `. A directory
gets a trailing slash: `rust/tui/src/ `. A path with a space is quoted.

Why the Codex way and not the Vibe way:

- In Switchboard, `@name text` at the start of a line **routes** the
  message to agent `name` (`rust/switchboard/src/router.rs`). A kept
  `@README.md explain` would be sent to an agent called `README.md`. A
  bare path has no such conflict, at the start or inline.
- The agents have `bash` and `apply_patch`: they read a path when they
  need it. Inlining (Vibe) costs context on every mention, is a stale
  snapshot when several agents edit the same workspace, and needs a
  runtime change (Bend) plus limits and error paths.
- A relative path stays right in a task's worktree (same tree, other
  root), where an absolute path would point to the main workspace.

### Index: `ignore` walk in the background, rescan when stale

- `ignore::WalkBuilder` parallel walk from the working directory, with
  Codex's options (`hidden(false)`, `require_git(true)`, `.git` skipped),
  so `.gitignore`, `.ignore`, `.git/info/exclude` and the global excludes
  apply, and a non-git folder still works. No `git` subprocess.
- Files **and** directories, relative, `/`-separated, capped at 200 000
  entries. Each entry keeps its lowercase bytes, the offset of its name,
  its depth, and a 64-bit mask of the bytes it contains.
- The walk starts in a thread when the TUI starts, so the index is ready
  before the first `@`. The index is an `Arc<Vec<Entry>>` swapped under a
  mutex: readers never wait for a walk.
- Freshness: when the popup opens (the `@` token appears) and the index
  is older than 3 s, a new walk starts in the background (at most one at
  a time); the popup shows the current index at once and the new one on
  the next frame. No fs watcher: it saves a dependency (`notify`), a
  thread and the event-burst logic, and a walk is cheap (see numbers).

### Ranking: filename first, then the path

The query is split at its last `/`: `dir part` + `name part`
(`src/comp` = `src/` + `comp`). A candidate must:

1. contain every byte of the query (64-bit mask test: one AND per entry),
2. contain the dir part as a subsequence of its parent path,
3. contain the name part as a subsequence of its path.

Then a tier, best first: exact name, name prefix, name fuzzy (the name
part is a subsequence of the file name), path fuzzy. Inside a tier:
recently picked first (this session), then the `nucleo-matcher` score
(fuzzy tiers only, `match_paths` config), then shallower, then shorter,
then path order. Only the tiers needed to fill the visible top (50) are
scored: a 1-letter query on 80 000 entries does no fuzzy scoring at all.

Empty query (`@` alone) and `dir/`: the recent picks, then the immediate
children of the root (or of `dir/`), directories first. Dot files show
only when the query starts with `.` (Vibe).

### Popup

- Agents first (the current `@` agent filter, Switchboard only), then the
  files. An agent row keeps its status glyph; a file row has `▪` (dim),
  a directory row `▸` in the accent color and a trailing `/`.
- `@` opens the popup at the start of a word: line start or after a
  space, cursor inside the word, no space yet (`a@b` does not open it).
  The same rule as `$skill` (`skills::token`).
- Picking an agent inserts `@name ` (as today); at line start in
  Switchboard it still routes. Picking a file replaces the `@query` token
  with the path and a space.

## Latency (prototype, release build, Apple M-series)

`/tmp/at-files-proto` (same algorithm), per keystroke, whole index:

| repo | entries | walk | `m` | `main` | `main.rs` | `src/comp` | `file_index` | `zzzq` |
|---|---|---|---|---|---|---|---|---|
| `~/mistral/dashboard` | 84 629 | 190-280 ms | 1.2 ms | 2.4 ms | 3.3 ms | 1.8 ms | 0.4 ms | 0.1 ms |
| this repo | 208 | 3 ms | 20 µs | | | | 8 µs | |

Before the mask and the tiers, the same queries took 6-17 ms on
dashboard (every hit fuzzy-scored). The final numbers from the TUI code
are in the section below once it lands.

## Tests

- Unit: ranking (name over path, exact over prefix over fuzzy, dir part,
  recent boost, dot files), `.gitignore` respected (temp dir with a git
  repo), inline `@` detection (start, after a space, not mid-word, cursor
  position), completion text (quotes, trailing `/`).
- tmux: type `@` + a file query in the TUI, see agents and files, pick a
  file, check the composer text.
- Latency: an ignored test / bench that indexes a large repo and prints
  per-query times.
