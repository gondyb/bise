# history search: `sb history` and `sb show` (BISE-233)

The user asks "the divider thing you did two weeks ago": the agent must
find it, even when it happened before its last compaction, or in a task
that is archived since. Two commands, open to every agent:

```
sb history "<words>" [--agent <a>] [--role user|assistant|message|tool|hub]
           [--since 2w] [--until 2026-09-30] [--archived|--live] [--limit n] [--page n]
sb show <agent>#<pos> [--context <n>]
```

## What is searched

`<hub state>/agents/<dir>/transcript.log`, one per agent that ever
existed in this hub: main, the tasks, archived tasks, and dirs that no
agent holds any more (shown by their dir name, as archived). The hub
appends every line of a feed there and never rewrites it: a compaction
or a restart of the REPL does not touch it. So a search reads the raw
messages from before any compaction checkpoint. The compaction summaries
(`obs: compaction_done`) are indexed too, as `hub` lines.

An entry and its role:

| role      | transcript lines |
|-----------|------------------|
| user      | `sb you : ` (the user's messages) |
| assistant | `obs: assistant: ` (the text, without the thinking) |
| message   | `sb msg`, `sb msg-in`, `sb msg-you`, `sb answered` (agent messages, reports) |
| tool      | `tool #n` (the call) and `tool_result #n` (the head of its result) |
| hub       | the other `sb ` lines (spawns, cards, infos), compaction summaries |

Not searched: the thinking, `tool_code` (the same text as the call),
`ev:`/`obs:` protocol lines, and the journal (its messages are in the
threads already; the old `sb history` searched the caller's thread and
the journal only).

A hub serves one workspace: the threads of another workspace (or of an
old hub state, `*.moved-*`) are not searched.

## Query and ranking

Every term must appear (AND); `"a phrase"` is one term. Case and the
accents of latin letters do not matter (`facon` finds `façon`). A tool
entry is searchable on its first 1000 chars, any other on its first 4000.

Rank: user and assistant entries first, then messages, tools, hub lines;
newest first inside each tier. A message is in the thread of its sender
and of its receiver: one hit (the same body counts once).

## Output, always bounded

```
$ sb history "divider gust"
70 hits for "divider gust" (page 1/7; by thread: main 24, bise-divider 17, bise-chip 4, marketing 4, bise-zen 3, bise-zen-read 3, +11 more)
main#15632 · 6h ago · assistant: …the divider label now carries the gust (346dacb)…
bise-divider#2210 · 7h ago · archived · assistant: Done: the gust is drawn by …
...
-- next: sb history "divider gust" --page 2 | open a hit with its neighbors: sb show <agent>#<pos> | narrow: --agent <a> --role ...
```

- a hit is `<agent>#<pos>`: the line number in that transcript, stable
  forever (the file only grows); `sb inspect <agent> --around #<pos>`
  takes the same positions;
- 10 hits per page (`--limit` up to 20), one line each (240 chars, from
  a little before the first term), the answer at most 6000 chars;
- the header counts the hits per thread: the next query knows which
  `--agent` to take.

```
$ sb show main#15632 --context 2
main's thread, #15632 (2026-09-30 14:03 UTC, assistant):
#15620 · 6h ago · user: …
#15626 · 6h ago · tool: bash : git log --oneline -3
>> #15632 · 6h ago · assistant: <the entry, up to 3500 chars, newlines kept>
#15640 · 6h ago · message main → bise-divider : …
#15641 · 6h ago · hub: …
-- earlier: sb show main#15611 | later: sb show main#15650 | agents: bise-divider (sb history "<words>" --agent <a>) | commits: 346dacb (git show <hash>)
```

`sb show` follows the entry: the agents it names, the message ids
(`m_<n>`: `sb history m_<n>`), the commits (`git show`). Neighbors are
cut to fit the 6000 chars; the whole entry: `sb inspect <agent> --at`.

## The index

In the hub, in memory (`rust/switchboard/src/search.rs`, `Index`). Each
thread keeps the byte offset it has read and the number of lines; a
search first reads only the bytes written since (whole lines only), for
every transcript (a `read_dir` and a `stat` each). A document is its
position, time, role, byte offset and folded text; the text shown is
read back from the file at the offset, for the hits of the page only.
It is built at the first search after the hub starts, and dies with it.

Measured on the live hub state (read-only, release build, 2026-09-30,
machine loaded by ~10 agents), `SB_SEARCH_BENCH=<state>/agents cargo test
-p switchboard --release --lib bench_real -- --ignored --nocapture`:

| what | measured |
|------|---------:|
| transcripts | 164 threads, 63 MB, 36 h of work |
| entries indexed | 42 500 (tool 86 %) |
| folded text in memory | 10.1 MB |
| cold build (first search after a hub start) | 130-340 ms |
| refresh, nothing new | 3.8 ms |
| search, 1-2 terms | 0.8-2 ms (`the`, 10 600 hits: 2-4 ms) |
| answer | 1.6-3.3 KB |

Why not SQLite FTS5: a search is a linear `contains` over 10 MB, a few
ms; FTS5 would add a C dependency to the hub (a new warm seed for every
task, a build-time cost) for a gain we cannot measure yet. When it
grows (this hub makes ~7 MB of index per day at its busiest; a month is
~200 MB and ~5 s of cold build): keep the index on disk
(`agents/<dir>/search.idx`, append-only, same offsets) so a hub start
does not rebuild it, and forget the text of old archived threads,
keeping only a trigram filter. The API (`Index::refresh/search/show`)
does not change.

The search runs on the hub's thread (a slow one, > 200 ms, is logged in
`hub.err`): at today's size the cold build is the only noticeable
pause.

## For the agents

main's role and the tasks' role say: when the user (or the brief)
refers to past work, search with `sb history` before asking or
guessing, then `sb show` a hit; quote what you found. `sb help` and the
command list of every role prompt carry both commands.
