# Research — the bise session format (JSONL event log)

Status: proposal, no code changed. To discuss with the user; the open
decisions are in §12. Read at HEAD `6586f6d`.

The goal, in the user's words: a JSONL file that is a log of structured
events, typed by one precise discriminated union, so we can add new event
types later and stay backward compatible.

What "done" means for the format: **a session can resume from its log with
no loss of information.** The model gets the same context, byte for byte
(prompt cache hits survive a restart), and the UI can show the full past,
including what compaction removed from the context.

---

## 1. Summary

- One session = one folder `~/.bise/sessions/<session-id>/` with an
  append-only `events.jsonl`. Images and big outputs go to a shared
  content-addressed store `~/.bise/blobs/`.
- Each line is one event: an envelope (`seq`, `at`, `type`, `v`, optional
  `turn`, optional `must`) and a `data` payload. `type` + `v` select the
  payload shape (a TypeScript discriminated union, §4).
- Lines are never rewritten. Undo, compaction, and edits are new events
  that point back at older `seq` numbers.
- A reader skips event types it does not know, **unless** the event has
  `"must": true`: then it opens the session read-only ("written by a newer
  bise"). A reader keeps unknown fields. A writer never touches old lines,
  so what a newer bise wrote survives an older one.
- A `checkpoint` event holds the whole resumable state. It is written after
  each compaction and at the start of each file segment, so a resume reads
  one checkpoint plus the events after it, not the whole history.
- Today's `.txt` checkpoints are converted once, checked by a round trip,
  and kept on disk.

---

## 2. What exists today in bise

### 2.1 The session checkpoint (`BEND-SESSION 2`)

`runtime/persist.bend` writes the whole session to `BEND_SESSION_FILE`
after every connection, with `File.open(path, "w")` then one write
(`persist.bend:47-59`). `core/checkpoint.bend` defines the text:

```
BEND-SESSION 2
TOOL <name> : <description>                 one line per tool
CFG <threshold> <select_budget> <max_nulls> <system prompt, newlines escaped>
COUNT <inputs> <actions>
QUEUE <wire-encoded text>                   queued user messages (v2)
NOTIF <wire-encoded text>                   held notifications (v2)
MSG <injected True|False> <user|assistant|system|tool> : <text, escaped>
  CALL <id> <name> : <wire-encoded args>    tool calls of an assistant MSG
```

Where the files are:

- solo sessions: `~/.bise/sessions/<yyyymmdd-hhmmss-pid>.txt` (45 files
  here, the biggest 1.8 MB), plus `.debug/` folders;
- hub agents: `<hub dir>/agents/<name>/session.txt`, next to `repl.json`,
  `role.md`, `context.txt`, `transcript.log`, `wire.log`, `wire.offset`,
  `repl.log`, `repl.err`, `choice.toml` (the `/model` and `/reasoning`
  choice, `rust/catalog/src/lib.rs:990-1001`) and `role.json`
  (`daemon.rs:1183-1193`).

What the checkpoint loses or risks:

| Gap | Detail |
|---|---|
| Not crash safe | `"w"` truncates, then writes: a crash in between leaves an empty or half file. `role.json` already uses tmp + rename; the checkpoint does not. |
| No history of its own | Each save replaces the last one. After a compaction the old messages are gone from disk (only `transcript.log`, a human log, keeps them). |
| Thinking is text | A thinking block is `<think>…\nBENDSIG::<signature></think>` inside the assistant text (`core/wire.bend:484-620`). It works, but a reader must parse markers to find it. |
| Images are text | An image is a marker `<image name=… path=… mime=… b64="/abs/…/images/<hash>.b64">` in the text (`core/image.bend:1-11`). The absolute path breaks if `~/.bise` moves. |
| Tool results by position | A `tool` MSG has no call id; it pairs with the calls of the assistant MSG before it, in order. |
| Not in the file | model and effort (in `choice.toml`), usage and cost (only in `wire.log`: `usage: model=… in=… out=… cache_read=… cache_write=…`), errors, retries, interruptions, turn boundaries, timestamps, the bise version that wrote it. |
| Permissions | `-rw-r--r--`: other users of the Mac can read every conversation. |

### 2.2 The hub journal (already JSONL)

`<hub dir>/journal.jsonl` is a flat JSONL log of hub events:
`task_created`, `message_sent`, `message_state`, `message_settled`,
`card_opened`, `card_closed`, `declared`, `reported`, `lifecycle`,
`renamed`, `main_note`, `main_notes_flushed` (2.7 MB here). Its rules are
already the ones this proposal wants: the Rust side does not decode events,
so an unknown kind still reaches `sb-core`, which answers `skipped`
(`hub/main.bend:11-13`); a line that is not a JSON object is counted and
logged, never dropped in silence (`daemon.rs:199-215`). It has no `seq`, no
format version, and no fsync. The hub journal stays the owner of messages
and cards (§5.8); the session log records what entered an agent's context.

### 2.3 Other state (not part of a session)

`~/.bise/drafts/*.json` (unsent input, written by tmp + rename; stale
`*.tmp` files are left behind), `images/<hash>.{png,b64}`, `crashes/`,
`prefs.json`, `config.toml`, `auth.json`. Sessions reference images; the
rest stays out of the log.

---

## 3. What the other tools do (read on this Mac, 2026-10)

**Claude Code** (`~/.claude/projects/<cwd-slug>/<session-uuid>.jsonl`,
files `0600`):

- One line per *content block*: one assistant reply with thinking + text +
  2 tool calls is 4 lines sharing `message.id`, chained by `parentUuid`.
- Every line repeats a large envelope: `cwd`, `gitBranch`, `version`,
  `entrypoint`, `userType`, `slug`, and both `sessionId` and `session_id`.
- Thinking blocks keep `thinking` + `signature`.
- Side records are rewritten over and over: in one 4,224-line file,
  `permission-mode`, `mode`, `last-prompt` and `ai-title` appear 207 times
  each.
- Compaction: a `system/compact_boundary` line with `compactMetadata`
  (`trigger`, `preTokens`, `postTokens`, `preservedSegment` head/anchor/tail
  uuids, `preservedMessages`) and a `logicalParentUuid`, then a user line
  with `isCompactSummary: true`.
- Interruptions are fake user messages: `[Request interrupted by user]`,
  `[Request interrupted by user for tool use]`.
- Tool results carry both the model-visible content and a UI-only
  `toolUseResult` object.
- Also in the log: `file-history-snapshot`, `file-history-delta`,
  `attachment` (hook results, reminders), `queue-operation`, `pr-link`.
- `~/.claude/history.jsonl`: one line per prompt typed
  (`display`, `pastedContents`, `project`, `sessionId`, `timestamp`).

**Codex CLI** (`~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<uuid>.jsonl`,
`archived_sessions/`, plus SQLite indexes):

- Every line is `{timestamp, type, payload}` with `payload.type` as a
  second tag. Top types: `session_meta` (once), `turn_context` (per turn:
  `model`, `effort`, `cwd`, policies, `turn_id`), `response_item` (the raw
  provider items: `message`, `reasoning` with `encrypted_content`,
  `function_call`, `function_call_output`, …), `event_msg` (UI events),
  `compacted`, `world_state`.
- The same fact is often stored twice: `response_item/reasoning` and
  `event_msg/agent_reasoning`, `response_item/message` and
  `event_msg/agent_message`.
- `event_msg/token_count` is the most frequent line of all (1.79 M lines
  over all rollouts here, more than all messages together).
- Compaction: `compacted {message, replacement_history}` — the new context
  in full, so a resume does not need what came before.
- Undo: `event_msg/thread_rolled_back {num_turns}`. Abort:
  `event_msg/turn_aborted`.
- `~/.codex/history.jsonl`: `{session_id, ts, text}` per prompt.

**Vibe** (`vibe/core/session/session_logger.py`):

- A folder per session with `messages.jsonl` (one LLM message per line:
  `role`, `content`, `reasoning_content`, `reasoning_payloads`,
  `tool_calls`, `images`, `injected`, `context_boundary: "compaction"`, …)
  and a metadata JSON (stats, tools, system prompt, title, model).
- Appends when the old messages are unchanged (checked with a fingerprint
  of the last message), else rewrites the whole file (tmp + fsync +
  `os.replace`). Directory `0700`.
- It is a snapshot of messages, not a log of events: errors, retries,
  model changes, and interruptions are not in it.
- The model reads with `extra="ignore"`, so a rewrite drops fields it
  does not know.
- `session_migration.py` converts the old one-file JSON format to folders
  and deletes the old file.

---

## 4. The event union (TypeScript)

This is the normative definition. `seq`, `type`, and `v` decide
everything; every other field is data.

```ts
// ---------- envelope: the same on every line ----------
type Seq = number;          // 1, 2, 3… per session, no gaps, never reused
type Iso = string;          // "2026-10-01T09:14:03.120Z" (UTC, ms)

interface Envelope {
  seq: Seq;
  at: Iso;                  // when the fact happened (writer clock)
  turn?: number;            // the turn it belongs to, when it belongs to one
  must?: true;              // a reader that does not know (type, v) must not
                            // rebuild the context from this file (§6.2)
}

// one variant = { type, v, data }; `v` changes only on a breaking change
type Ev<T extends string, V extends number, D> =
  Envelope & { type: T; v: V; data: D };

// ---------- shared pieces ----------
type BlobRef = {            // a file in ~/.bise/blobs/sha256/<2>/<62>
  sha256: string;
  bytes: number;
  mime: string;
};

type Text = { text: string } | { blob: BlobRef };   // big text goes to a blob

type Part =
  | { kind: "text"; text: string }
  | { kind: "image"; image: BlobRef; name?: string }   // name: "[Image #1]"
  | { kind: "file"; file: BlobRef; name: string };     // future attachments

type AssistantPart =
  | { kind: "text"; text: string }
  | { kind: "thinking"; text: string;
      signature?: string;          // provider signature, stored as given
      provider?: string }          // who signed it ("anthropic", …)
  | { kind: "redacted_thinking"; data: string; provider?: string };

type ToolCall = {
  id: string;               // the id the model sees ("call_12")
  name: string;
  args: string;             // the exact bytes the model produced
};

type ToolDef = { name: string; description: string; schema?: unknown };

type ModelRef = { model: string; effort?: string };  // "anthropic/claude-…", "high"

type ErrorInfo = {
  kind: "http" | "network" | "timeout" | "parse" | "tool" | "internal" | "other";
  status?: number;
  message: string;          // no headers, no keys (§8)
};

// ---------- session and file ----------
type SessionStart = Ev<"session_start", 1, {
  session: string;          // session id (§5.1)
  format: 1;                // format major (§6.1)
  created_by: string;       // "bise 0.9.3 (51c081a)"
  cwd: string;
  agent?: {                 // absent for a solo session
    hub: string;            // hub folder name, "harness-3abb2bd8"
    name: string;           // "session-format"
    parent?: string;        // "main"
  };
  forked_from?: { session: string; seq: Seq };
  migrated_from?: { path: string; format: string; sha256: string };
}>;

type SegmentStart = Ev<"segment_start", 1, {   // first line of segment 2, 3…
  session: string;
  format: 1;
  index: number;            // 2, 3…
  prev: { file: string; last_seq: Seq; sha256: string };
}>;

type ProcessOpened = Ev<"process_opened", 1, {  // each time a process opens the log
  writer: string;           // "bise 0.9.4 (a1b2c3d)"
  pid: number;
  resume: boolean;
}>;

type SessionClosed = Ev<"session_closed", 1, {
  reason: "user" | "task_done" | "task_dropped" | "other";
}>;

type TitleSet = Ev<"title_set", 1, { title: string; source: "auto" | "user" }>;

// ---------- configuration: what the model sees besides messages ----------
type ContextSet = Ev<"context_set", 1, {        // must: true
  system?: Text;            // absent = unchanged
  tools?: ToolDef[];        // the full list, absent = unchanged
}>;

type LimitsSet = Ev<"limits_set", 1, {
  compact_threshold?: number;   // tokens
  select_budget?: number;
  max_nulls?: number;
}>;

type ModelSet = Ev<"model_set", 1, ModelRef & {
  provider?: string;
  context_window?: number;
  source: "config" | "env" | "user" | "agent" | "fallback" | "migration" | "other";
}>;

// ---------- turns ----------
type TurnStarted = Ev<"turn_started", 1, {
  cause: "user" | "queue" | "notification" | "agent_message" | "resume" | "other";
}>;

type TurnEnded = Ev<"turn_ended", 1, {
  outcome: "done" | "interrupted" | "failed" | "crashed" | "other";
  error?: ErrorInfo;
}>;

// ---------- what enters the model context ----------
type UserMessage = Ev<"user_message", 1, {      // must: true
  content: Part[];
  delivery: "prompt" | "steer";   // steer = typed while a turn runs
  from_queue?: Seq;               // the input_queued it consumes
}>;

type ContextInjected = Ev<"context_injected", 1, {   // must: true
  // a user-role message the model sees but the user did not type
  kind: "preamble" | "notification" | "hub_state" | "resume_note"
      | "skill" | "reminder" | "other";
  content: Part[];
  from_queue?: Seq;
}>;

type AgentMessage = Ev<"agent_message", 1, {     // must: true
  // a message from another agent or the hub, as the model saw it
  hub_msg: string;          // "m_1689": joins the hub journal
  thread?: string;
  from: string;
  relation: "parent" | "child" | "peer" | "user" | "hub";
  expects_reply: boolean;
  reply_to?: string;
  content: Part[];          // the exact text given to the model
  from_queue?: Seq;
}>;

type AssistantMessage = Ev<"assistant_message", 1, {   // must: true
  req: number;              // request number in this session (joins usage, errors)
  model: string;            // the model that answered
  parts: AssistantPart[];
  calls: ToolCall[];
  stop?: "end" | "tool_use" | "max_tokens" | "other";
  response_id?: string;     // provider id, when given
}>;

type ToolResult = Ev<"tool_result", 1, {        // must: true
  call: string;             // ToolCall.id
  ok: boolean;
  content: Part[];          // what the model sees; big text → a blob part
  ms?: number;
  exit?: number;            // bash exit code, when there is one
}>;

// ---------- facts that do not enter the context ----------
type Usage = Ev<"usage", 1, {
  req: number;
  model: string;
  input: number;
  output: number;
  cache_read?: number;
  cache_write?: number;
  reasoning?: number;
  cost?: { usd: number; prices: string };  // prices: price table id, "2026-10-01"
}>;

type RequestFailed = Ev<"request_failed", 1, {
  req: number;
  attempt: number;          // 1, 2…
  error: ErrorInfo;
  retry_in_ms?: number;     // absent = no retry
}>;

type ResponseDiscarded = Ev<"response_discarded", 1, {
  req: number;
  cause: "empty" | "interrupted" | "invalid" | "other";
  partial?: AssistantPart[];      // what streamed before the cut, for the UI
}>;

type Interrupted = Ev<"interrupted", 1, {
  by: "user" | "restart" | "agent" | "other";
  during: "request" | "tool" | "compaction" | "idle";
  pending_calls?: string[];       // tool calls with no result yet
}>;

type ToolStarted = Ev<"tool_started", 1, { call: string }>;  // UI and crash repair

// ---------- queue: inputs that wait for the next turn ----------
type InputQueued = Ev<"input_queued", 1, {      // must: true
  kind: "user" | "notification" | "agent_message";
  content: Part[];
  agent?: AgentMessage["data"];   // when kind = agent_message
}>;

type InputDropped = Ev<"input_dropped", 1, {    // must: true
  queued: Seq;
  reason: "user" | "stale" | "other";
}>;

// ---------- compaction ----------
type CompactionStarted = Ev<"compaction_started", 1, {
  id: number;
  trigger: "auto" | "user" | "agent";
  tokens_before?: number;
  extra?: string;           // user instructions for the summary
}>;

type CompactionDone = Ev<"compaction_done", 1, {  // must: true
  id: number;
  summary: Part[];          // becomes an injected message
  replaces: { from: Seq; to: Seq };  // context events it removes
  kept: Seq[];              // context events inside the range kept as they were
  tokens_after?: number;
}>;

type CompactionFailed = Ev<"compaction_failed", 1, {
  id: number;
  attempt: number;
  error: ErrorInfo;
}>;

// ---------- history edits ----------
type Rewound = Ev<"rewound", 1, {               // must: true
  to: Seq;                  // the context is what it was right after `to`
  reason: "user_undo" | "user_edit" | "restore" | "other";
}>;

// ---------- full state, for fast resume ----------
type Checkpoint = Ev<"checkpoint", 1, {         // must: true
  upto: Seq;                // state after this seq (usually seq - 1)
  context: Seq[];           // the context events, in order (§7)
  system: Text;
  tools: ToolDef[];
  model: ModelRef & { provider?: string };
  limits: LimitsSet["data"];
  queue: Seq[];             // input_queued not yet consumed or dropped
  counters: { req: number; turn: number; compaction: number; inputs: number; actions: number };
  usage_total: { input: number; output: number; cache_read: number; cache_write: number; usd?: number };
}>;

// ---------- the union ----------
type Event =
  | SessionStart | SegmentStart | ProcessOpened | SessionClosed | TitleSet
  | ContextSet | LimitsSet | ModelSet
  | TurnStarted | TurnEnded
  | UserMessage | ContextInjected | AgentMessage | AssistantMessage | ToolResult
  | Usage | RequestFailed | ResponseDiscarded | Interrupted | ToolStarted
  | InputQueued | InputDropped
  | CompactionStarted | CompactionDone | CompactionFailed
  | Rewound | Checkpoint;

// what a reader holds for a line it cannot type (§6)
type UnknownEvent = Envelope & { type: string; v: number; data: unknown };
```

Which events are "context events" (they build the model's message list):
`user_message`, `context_injected`, `agent_message`, `assistant_message`,
`tool_result`, and the summary of a `compaction_done`. Every other event is
state (config, model, queue) or a record for the UI and for debugging.

Which events carry `must: true`: every event whose loss would change the
model context or the queue — the ones marked `// must: true` above. A new
type gets `must` only if an old reader skipping it would send the model a
wrong context.

---

## 5. Design choices, one by one

### 5.1 Files and ids

```
~/.bise/sessions/<session-id>/
  events.jsonl            the current segment
  events.000001.jsonl     older segments (after a rotation, §8.3)
  lock                    the writer's lock (flock + pid)
~/.bise/blobs/sha256/ab/cdef…   images, big tool outputs, big prompts
~/.bise/hubs/<hub>/agents/<name>/session   one line: the session id
```

- Session id: `s-20261001-091403-7f3a9c` (UTC time + 6 random hex). It
  sorts by time, it is short, and it is not a path. (Open decision 2.)
- A hub agent points at its session by id. `/restart` and a worktree move do
  not move the log. The hub journal gets a `session_bound {name, session}`
  event when an agent gets a session, so the hub can list them.
- Blobs are written once, by content hash, and shared between sessions (the
  same screenshot pasted twice is one file). This replaces the absolute
  `.b64` paths in image markers: a reference is a hash, so `~/.bise` can
  move.

### 5.2 The envelope is small

Claude Code repeats ~15 fields on every line; here the session id, cwd,
agent name, and bise version are written once (`session_start`,
`process_opened`), because a file is one session. `seq` is the only id: it
is short, ordered, and makes gaps visible. `at` is for people and for
durations; order is `seq`, never `at` (clocks jump).

Why `data` is nested instead of flat: envelope fields can grow (a future
`host` or `cause`) without colliding with a payload field, and "keep
unknown fields" is simple — a reader keeps `data` as raw JSON.

### 5.3 Messages

- **One line per message**, not per content block (Claude Code) and not a
  message plus a UI echo (Codex). `assistant_message` holds all its parts
  and calls, in order.
- **Thinking** is a part with its signature as the provider gave it.
  Today's `<think>…BENDSIG::</think>` text becomes structured on write and
  is rebuilt the same way on read, until the Core itself uses parts.
  Encrypted or redacted reasoning is stored as given (`redacted_thinking`).
- **Tool calls** keep `args` as the exact string the model wrote (not
  parsed JSON): the next request must replay the same bytes.
- **Tool results** carry `call`, so they no longer pair by position.
- **Images** are `image` parts with a `BlobRef`. The `.png` stays in
  `~/.bise/images/` for the UI; the blob is the bytes the model got.
- **Injected context** (the role preamble, `<switchboard_state>`, the
  resume note, notifications) is its own type, `context_injected`, with a
  `kind`. Today it is `MSG True user`. The UI can hide it; the model gets
  it.
- **Inter-agent messages** are `agent_message`: the hub fields
  (`hub_msg`, `from`, `relation`, `expects_reply`) plus the exact text the
  model got. We store the rendered text, not only the fields: a later
  change of the `<agent_message>` wrapper must not change past context.

### 5.4 Model, effort, provider

`model_set` is written at session start and at every `/model`,
`/reasoning`, or fallback change, with its `source`. Each
`assistant_message` and `usage` also names the model that really answered.
This replaces reading `choice.toml` for the past. (`choice.toml` can stay
as the live control file; the log records each change.)

### 5.5 Compaction

Compaction is two or three events: `compaction_started`, then
`compaction_done` (or `compaction_failed`, one per attempt). `done` says
which range of context events it replaces (`from`..`to`), which ones inside
that range it keeps word for word (`kept`: today the recent user messages),
and the summary. The old events stay in the file: the UI can show them, and
a bug in a summary can be seen. A `checkpoint` follows right after, so a
resume starts there (Codex's `replacement_history` idea, without
duplicating messages: the checkpoint lists `seq`s).

### 5.6 Interruptions, errors, retries

- `interrupted` records who stopped what; `turn_ended {outcome:
  "interrupted"}` closes the turn. No fake user message (Claude Code
  writes `[Request interrupted by user]` as if the user typed it). If the
  model must be told, that is a `context_injected {kind: "resume_note"}`,
  visible as such.
- Each failed request attempt is a `request_failed` with the retry delay.
  A partial answer that was dropped is a `response_discarded` with the
  streamed parts, for the UI only.
- A crash is found at resume time (a turn with no `turn_ended`) and closed
  then (§7, step 5).

### 5.7 Usage and cost

One `usage` line per request, joined to the answer by `req`. Tokens are
facts and always stored. Cost is optional and names the price table it came
from, because prices change. No per-stream-chunk counters (Codex's
`token_count` is its most frequent line).

### 5.8 Agents, messages, cards

The hub journal stays the source of truth for who sent what, delivery
state, and cards (`card_opened` / `card_closed`). The session log does not
copy them: it has what the agent's model saw (`agent_message`,
`context_injected {kind: "hub_state"}`), with `hub_msg` to join the two
logs. What an agent sends goes through its tool calls (`sb send …` in
`bash`), so it is already in the log as a `tool_result`.

### 5.9 Forks, undo, restores

- **Fork**: a new session whose `session_start.forked_from` is
  `{session, seq}`, followed by a `checkpoint` that copies the parent's
  state at that `seq` (context events are copied too, so the fork is
  self-contained). The parent file is not touched.
- **Undo / edit a past message**: a `rewound {to}` event. The context is
  again what it was after `to`; the events between stay in the file,
  hidden. The log stays linear: no `parentUuid` tree to walk.
- **Restore after a binary rollback** (`/version back`): nothing special;
  §6 decides what the old binary may do with the file.

---

## 6. Compatibility rules

### 6.1 Format major

`session_start.format` (and `segment_start.format`) is the format major,
`1` for this proposal. It changes only if the envelope or the line rules
change. A reader that does not know the major does not open the file
(message: "session written by a newer bise, update to resume it").

### 6.2 Readers

1. A line that is not valid JSON, or not an object: skip it, count it, log
   the line numbers (the hub journal's rule, `daemon.rs:199-215`). Never
   stop at the first bad line.
2. A `(type, v)` the reader does not know:
   - without `must`: skip it, keep its raw line (the UI can show "1 event
     from a newer version");
   - with `must: true`: read the rest for display, but open the session
     **read-only** — no resume, no append.
3. An unknown field inside a known payload: keep it (raw `data`), ignore
   it.
4. A missing optional field: use the default the type documents.
5. An unknown value in a known enum: every enum has `"other"`; treat an
   unknown value as `"other"`.
6. `seq` goes down or repeats: a writer bug. Keep the first, report the
   rest.

### 6.3 Writers

1. Append only. Never rewrite, reorder, or delete a line. (This is what
   keeps unknown fields and events written by a newer bise.)
2. A writer that read an unknown `must` event does not append.
3. Adding an optional field to a payload: no version change.
4. Adding a type: no version change; add `must: true` only if skipping it
   would give the model a wrong context.
5. Changing the meaning or the shape of a field: new `v` for that type.
   A writer may write both `v1` and `v2` of the same fact for a while if
   old readers still matter (open decision 9).
6. Type names are `snake_case`, never reused, never renamed. The union in
   §4 is the registry; a removed type stays listed as "retired".

### 6.4 Old binary on a new file

An older bise that knows format 1 reads everything it knows, skips the
rest, and appends safely unless it met a `must` event it does not know.
So adding `title_set` or a new `usage` field never locks anyone out; adding
a new kind of context message does, visibly, and only for that session.

---

## 7. Resume algorithm

Input: a session id. Output: the in-memory `T.Session` (`core/types.bend`)
plus the UI history.

1. Take the lock (`lock`, `flock`). If another live pid holds it, open
   read-only.
2. Open the last segment. Check its first line (`session_start` or
   `segment_start`) and its `format`.
3. **Repair the tail.** If the file does not end with `\n`, or its last
   line is not valid JSON: copy those bytes to `events.torn-<time>` and
   truncate the file to the last `\n`. (The event was not fsynced, so no
   one relied on it.)
4. **Rebuild the state.** Find the last `checkpoint`; start from it (the
   first segment's start if there is none). For each event after it, in
   `seq` order:
   - `context_set`, `limits_set`, `model_set`: update the config;
   - context events: append to the context list; a `from_queue` removes
     that item from the queue;
   - `input_queued` / `input_dropped`: update the queue;
   - `compaction_done`: remove the `replaces` range except `kept`, insert
     the summary (as an injected message) at the start of the range;
   - `rewound {to}`: set the context, queue, and config back to their
     state after `to` (replay from the checkpoint up to `to`; if `to` is
     before the checkpoint, read the older segment);
   - `usage`: add to the totals; `req`, `turn`, `compaction` counters:
     take the maximum seen;
   - unknown types: §6.2.
   A `checkpoint` lists context events by `seq`: load those lines (they
   are in this segment or an older one; a segment keeps an index at its
   end, open decision 7).
5. **Close what the crash left open.** If the last `turn_started` has no
   `turn_ended`:
   - for each `ToolCall` of the last `assistant_message` with no
     `tool_result`: write `tool_result {ok: false, content: "interrupted
     by a restart"}` (the provider needs a result for every call);
   - write `interrupted {by: "restart"}` and `turn_ended {outcome:
     "crashed"}`;
   - if it is a hub agent, write `context_injected {kind: "resume_note"}`
     with today's `RESUME_TEXT` (`daemon.rs:196`) and start a turn.
   A `compaction_started` with no end gets a `compaction_failed`.
6. Write `process_opened {resume: true}`. fsync.
7. Convert to `T.Session`: config → `T.Cfg`; context events → `T.Msg`
   list (parts back to today's text with markers; thinking back to
   `<think>…BENDSIG::</think>`; tool results in call order); counters →
   `inputs`, `actions`; queue → `queued` and `notifs`.
8. Check: the projected request (system + tools + messages) is byte-equal
   to what the last request sent, when the log has it (a hash in `usage`,
   open decision 8). A mismatch is logged: it means a lost cache, not a
   broken session.

Cost: one checkpoint plus the events after it. A session is never read in
full to resume, however long it is.

---

## 8. Crash safety, size, secrets

### 8.1 Writing a line

- One writer per session (the lock). The file is opened with `O_APPEND`;
  each event is serialized in full, then written with one `write` call
  that ends with `\n`.
- A blob is written before the event that refers to it: temp file, fsync,
  rename into `blobs/`. A dangling reference cannot happen; an orphan blob
  can (cleaned by the retention job).
- **fsync** after: `user_message`, `agent_message`, `input_queued`,
  `turn_ended`, `compaction_done`, `checkpoint`, `process_opened`, and
  before the hub is told a message was delivered. Not after each tool
  result or usage line (the next fsync covers them; a crash loses at most
  the facts of the current step, which the repair in §7 step 5 closes).
  (Open decision 4.)
- Files `0600`, folders `0700`.

### 8.2 Line size

A line stays under 256 KiB. Bigger text (a 2 MB `cat`, a long system
prompt) goes to a blob and the event holds `{blob}`. Images always go to a
blob. (Open decision 5 for the threshold.)

### 8.3 Rotation

When `events.jsonl` passes 32 MiB, at a turn boundary: rename it
`events.<n>.jsonl`, start a new `events.jsonl` with `segment_start` (name,
last `seq`, and sha256 of the previous segment) and a `checkpoint`. `seq`
continues. A resume reads only the last segment, unless a `rewound` goes
further back. Old segments can be gzipped later without changing the
format. (Open decision 7.)

### 8.4 Secrets are never stored

- Never written: API keys, `auth.json` content, HTTP headers, the raw
  provider request or response bodies, the process environment.
- `ErrorInfo.message` is built by bise from the status and the provider's
  error text, never from a header dump.
- Tool output can hold a secret the model printed (`cat .env`). At write
  time, the writer replaces the values of the keys bise knows (from
  `auth.json`, `.env` files) and well-known key shapes with
  `«redacted:<name>»`. The model saw the secret; the disk does not keep
  it. (Open decision 10: redact or not, and what the model sees on
  resume.)

---

## 9. Examples

Made-up content; one line per event (wrapped here only if noted).

### 9.1 A short solo session: one question, one tool call

```jsonl
{"seq":1,"at":"2026-10-01T09:14:03.120Z","type":"session_start","v":1,"data":{"session":"s-20261001-091403-7f3a9c","format":1,"created_by":"bise 0.9.4 (a1b2c3d)","cwd":"/Users/ada/code/shop"}}
{"seq":2,"at":"2026-10-01T09:14:03.121Z","type":"process_opened","v":1,"data":{"writer":"bise 0.9.4 (a1b2c3d)","pid":4242,"resume":false}}
{"seq":3,"at":"2026-10-01T09:14:03.125Z","type":"context_set","v":1,"must":true,"data":{"system":{"blob":{"sha256":"9f2c…","bytes":14210,"mime":"text/plain"}},"tools":[{"name":"bash","description":"Run a shell command."}]}}
{"seq":4,"at":"2026-10-01T09:14:03.125Z","type":"limits_set","v":1,"data":{"compact_threshold":800000,"select_budget":20000,"max_nulls":3}}
{"seq":5,"at":"2026-10-01T09:14:03.126Z","type":"model_set","v":1,"data":{"model":"anthropic/claude-sonnet-x","effort":"medium","provider":"anthropic","context_window":1000000,"source":"config"}}
{"seq":6,"at":"2026-10-01T09:14:10.004Z","turn":1,"type":"turn_started","v":1,"data":{"cause":"user"}}
{"seq":7,"at":"2026-10-01T09:14:10.004Z","turn":1,"type":"user_message","v":1,"must":true,"data":{"content":[{"kind":"text","text":"How many tests are in this repo?"}],"delivery":"prompt"}}
{"seq":8,"at":"2026-10-01T09:14:13.410Z","turn":1,"type":"assistant_message","v":1,"must":true,"data":{"req":1,"model":"anthropic/claude-sonnet-x","parts":[{"kind":"thinking","text":"Count the test files.","signature":"EqQBCkYI…","provider":"anthropic"}],"calls":[{"id":"call_1","name":"bash","args":"rg -c '^def test_' tests | wc -l"}],"stop":"tool_use"}}
{"seq":9,"at":"2026-10-01T09:14:13.411Z","turn":1,"type":"usage","v":1,"data":{"req":1,"model":"anthropic/claude-sonnet-x","input":12400,"output":230,"cache_read":5200,"cache_write":7200}}
{"seq":10,"at":"2026-10-01T09:14:13.420Z","turn":1,"type":"tool_started","v":1,"data":{"call":"call_1"}}
{"seq":11,"at":"2026-10-01T09:14:13.530Z","turn":1,"type":"tool_result","v":1,"must":true,"data":{"call":"call_1","ok":true,"content":[{"kind":"text","text":"37\n"}],"ms":110,"exit":0}}
{"seq":12,"at":"2026-10-01T09:14:15.002Z","turn":1,"type":"assistant_message","v":1,"must":true,"data":{"req":2,"model":"anthropic/claude-sonnet-x","parts":[{"kind":"text","text":"There are 37 test files."}],"calls":[],"stop":"end"}}
{"seq":13,"at":"2026-10-01T09:14:15.003Z","turn":1,"type":"usage","v":1,"data":{"req":2,"model":"anthropic/claude-sonnet-x","input":12690,"output":12,"cache_read":12400}}
{"seq":14,"at":"2026-10-01T09:14:15.004Z","turn":1,"type":"turn_ended","v":1,"data":{"outcome":"done"}}
```

### 9.2 Later in a longer session: a crash, a resume, a model change, a compaction

```jsonl
{"seq":80,"at":"2026-10-01T10:02:00.000Z","turn":9,"type":"turn_started","v":1,"data":{"cause":"user"}}
{"seq":81,"at":"2026-10-01T10:02:00.001Z","turn":9,"type":"user_message","v":1,"must":true,"data":{"content":[{"kind":"text","text":"Run the full test suite."},{"kind":"image","image":{"sha256":"0455…","bytes":88120,"mime":"image/png"},"name":"[Image #1]"}],"delivery":"prompt"}}
{"seq":82,"at":"2026-10-01T10:02:04.300Z","turn":9,"type":"request_failed","v":1,"data":{"req":31,"attempt":1,"error":{"kind":"http","status":529,"message":"overloaded"},"retry_in_ms":2000}}
{"seq":83,"at":"2026-10-01T10:02:09.900Z","turn":9,"type":"assistant_message","v":1,"must":true,"data":{"req":31,"model":"anthropic/claude-sonnet-x","parts":[],"calls":[{"id":"call_40","name":"bash","args":"make test"}],"stop":"tool_use"}}
{"seq":84,"at":"2026-10-01T10:02:09.910Z","turn":9,"type":"tool_started","v":1,"data":{"call":"call_40"}}
```

The machine sleeps and bise is killed here. At the next start, the resume
(§7 step 5) appends:

```jsonl
{"seq":85,"at":"2026-10-01T11:30:00.000Z","type":"process_opened","v":1,"data":{"writer":"bise 0.9.4 (a1b2c3d)","pid":5150,"resume":true}}
{"seq":86,"at":"2026-10-01T11:30:00.001Z","turn":9,"type":"tool_result","v":1,"must":true,"data":{"call":"call_40","ok":false,"content":[{"kind":"text","text":"interrupted by a restart"}]}}
{"seq":87,"at":"2026-10-01T11:30:00.001Z","turn":9,"type":"interrupted","v":1,"data":{"by":"restart","during":"tool","pending_calls":["call_40"]}}
{"seq":88,"at":"2026-10-01T11:30:00.002Z","turn":9,"type":"turn_ended","v":1,"data":{"outcome":"crashed"}}
{"seq":89,"at":"2026-10-01T11:31:12.000Z","type":"model_set","v":1,"data":{"model":"mistral/devstral-y","effort":"high","provider":"mistral","context_window":256000,"source":"user"}}
{"seq":90,"at":"2026-10-01T11:31:12.001Z","type":"limits_set","v":1,"data":{"compact_threshold":200000}}
{"seq":91,"at":"2026-10-01T11:31:12.002Z","type":"compaction_started","v":1,"data":{"id":3,"trigger":"auto","tokens_before":231400}}
{"seq":92,"at":"2026-10-01T11:31:40.500Z","type":"compaction_done","v":1,"must":true,"data":{"id":3,"summary":[{"kind":"text","text":"Summary: the user works on the checkout flow; tests…"}],"replaces":{"from":7,"to":88},"kept":[81],"tokens_after":9800}}
{"seq":93,"at":"2026-10-01T11:31:40.501Z","type":"checkpoint","v":1,"must":true,"data":{"upto":92,"context":[92,81],"system":{"blob":{"sha256":"9f2c…","bytes":14210,"mime":"text/plain"}},"tools":[{"name":"bash","description":"Run a shell command."}],"model":{"model":"mistral/devstral-y","effort":"high","provider":"mistral"},"limits":{"compact_threshold":200000,"select_budget":20000,"max_nulls":3},"queue":[],"counters":{"req":31,"turn":9,"compaction":3,"inputs":40,"actions":88},"usage_total":{"input":913000,"output":21400,"cache_read":702000,"cache_write":88000}}}
```

In `checkpoint.context`, `92` stands for the summary of that compaction.

### 9.3 A hub agent: a brief, a queued message, an event from the future

```jsonl
{"seq":1,"at":"2026-10-01T12:00:00.000Z","type":"session_start","v":1,"data":{"session":"s-20261001-120000-a01b2c","format":1,"created_by":"bise 0.9.4 (a1b2c3d)","cwd":"/Users/ada/code/shop","agent":{"hub":"shop-5d21e0aa","name":"fix-login","parent":"main"}}}
{"seq":6,"at":"2026-10-01T12:00:00.300Z","turn":1,"type":"context_injected","v":1,"must":true,"data":{"kind":"preamble","content":[{"kind":"text","text":"# Your role: task `fix-login` …"}]}}
{"seq":7,"at":"2026-10-01T12:00:00.301Z","turn":1,"type":"agent_message","v":1,"must":true,"data":{"hub_msg":"m_12","thread":"t_12","from":"main","relation":"parent","expects_reply":true,"content":[{"kind":"text","text":"<agent_message from=\"main\" relation=\"parent\" id=\"m_12\" …># Task `fix-login` …</agent_message>"}]}}
{"seq":30,"at":"2026-10-01T12:03:10.000Z","turn":1,"type":"input_queued","v":1,"must":true,"data":{"kind":"agent_message","content":[{"kind":"text","text":"<agent_message from=\"qa\" relation=\"peer\" id=\"m_15\" …>login still fails on Safari</agent_message>"}],"agent":{"hub_msg":"m_15","from":"qa","relation":"peer","expects_reply":false,"content":[]}}}
{"seq":31,"at":"2026-10-01T12:03:11.000Z","turn":1,"type":"turn_ended","v":1,"data":{"outcome":"done"}}
{"seq":32,"at":"2026-10-01T12:03:11.001Z","turn":2,"type":"turn_started","v":1,"data":{"cause":"queue"}}
{"seq":33,"at":"2026-10-01T12:03:11.002Z","turn":2,"type":"agent_message","v":1,"must":true,"data":{"hub_msg":"m_15","from":"qa","relation":"peer","expects_reply":false,"content":[{"kind":"text","text":"<agent_message from=\"qa\" …>login still fails on Safari</agent_message>"}],"from_queue":30}}
{"seq":34,"at":"2026-10-01T12:03:11.050Z","turn":2,"type":"plan_updated","v":1,"data":{"steps":["repro on Safari","fix cookie flag"]}}
```

Line 34 was written by a newer bise. An older one skips it (no `must`),
keeps it, and still resumes and appends. Had it been
`{"type":"voice_message","must":true,…}`, the older bise would show the
session read-only.

---

## 10. Migration from today's `.txt`

### 10.1 Mapping

| `.txt` line | Events |
|---|---|
| file itself | `session_start {migrated_from: {path, format: "BEND-SESSION 2", sha256}}`, `process_opened` |
| `TOOL`, `CFG` | `context_set {system, tools}`, `limits_set` |
| (none) | `model_set {source: "migration"}` from the agent's `choice.toml`, else from the config of that start |
| `MSG False user` | `user_message {delivery: "prompt"}` |
| `MSG True user` | `context_injected {kind}`: `preamble` if it starts with the role header, `hub_state` for `<switchboard_state>`, `notification` otherwise; an `<agent_message …>` text becomes `agent_message` (fields parsed from the tag) |
| `MSG … system` | `context_injected {kind: "other"}` (should not happen outside the CFG) |
| `MSG False assistant` + `CALL` | `assistant_message`: text split into `text` / `thinking` parts at `<think>…BENDSIG::…</think>`, calls with `call_<id>`; `req` numbered in order; `model` = the migration model |
| `MSG False tool` | `tool_result` paired by position with the calls of the assistant message before it; `ok: true` (the `.txt` does not say) |
| image marker in any text | `image` part; the `.b64` file is copied into `blobs/`; a missing file keeps the marker as text |
| `QUEUE`, `NOTIF` | `input_queued {kind: "user" / "notification"}` |
| `COUNT` | counters of the final `checkpoint` |

A compaction summary in the `.txt` is an injected user message: it becomes
`context_injected {kind: "other"}` — the replaced messages were never on
disk, so there is nothing to point at.

### 10.2 Steps

1. **When:** at first resume of a `.txt` session (lazy), plus a
   `bise sessions migrate` command for all of them. No big-bang migration
   at startup.
2. **Check:** rebuild the `T.Session` from the new log (§7 step 7) and
   print it with `K.to_text`. It must be byte-equal to the `.txt`. If not,
   keep using the `.txt` for that session and log why.
3. **Keep:** the `.txt` is never deleted; `~/.bise/migrated.json` gets
   `{sessions: {<txt path>: <session id>}}`. (Vibe deletes the old file;
   we do not.)
4. **Rollback safety:** `/version back` can start a bise that only knows
   `.txt`. During a transition period the writer also writes the `.txt`
   checkpoint (tmp + rename, `0600`) after each turn. Old binaries keep
   working; the new binary reads the JSONL first. (Open decision 6: how
   long.)
5. **Hub agents:** `agents/<name>/session.txt` → a session folder +
   `agents/<name>/session` (the id). The hub keeps passing
   `BEND_SESSION_FILE` to old REPLs during the transition.
6. **Human logs:** `transcript.log`, `wire.log`, `context.txt` stay for
   now; later they can be rendered from the JSONL on demand (open decision
   11).

### 10.3 What must be checked in Bend before building

- Append mode and fsync in `File` (today `persist.bend` uses `"w"` only).
- JSON writing of big strings without quadratic appends (`vendor/json.bend`;
  images already avoid it with the `@@BENDIMG:` placeholder).
- The writer probably belongs on the Rust side (the harness already sees
  every `obs:` line on the wire), with the Bend REPL only reading
  checkpoints. This is an implementation choice for the next step, not a
  format question.

---

## 11. Comparison

| | Claude Code | Codex CLI | Vibe | bise today | This proposal |
|---|---|---|---|---|---|
| Layout | `projects/<cwd>/<uuid>.jsonl` | `sessions/Y/M/D/rollout-….jsonl` + SQLite | folder: `messages.jsonl` + meta JSON | one `.txt` rewritten each turn | folder per session: `events.jsonl` + segments; shared blobs |
| Line = | one content block | one item or one UI event | one message | (not lines) | one event |
| Tag | `type` (+ `subtype`) | `type` + `payload.type` | `role` | line prefix | `type` + `v` |
| Envelope | ~15 fields, repeated | `timestamp`, `type` | none | none | `seq`, `at`, `turn?`, `must?` |
| Append only | yes | yes | append or full rewrite | full rewrite | yes |
| Thinking | block + `signature` | `reasoning` + `encrypted_content` | `reasoning_content`, `reasoning_payloads` | text + `BENDSIG::` line | part + `signature` |
| Model changes | `effort` on each line | `turn_context` per turn | metadata field | `choice.toml`, not in session | `model_set` + model per answer |
| Compaction | boundary + summary message, uuid ranges | `compacted` with full replacement history | `context_boundary` mark | old messages gone | `compaction_done` with `seq` range + `checkpoint` |
| Interruptions | fake user text | `turn_aborted` | — | not stored | `interrupted` + `turn_ended` |
| Errors / retries | — | `error` events | — | not stored | `request_failed`, `response_discarded` |
| Usage | in each assistant line | `token_count`, very frequent | stats in metadata | `wire.log` only | one `usage` per request |
| Undo / fork | `parentUuid` tree | `thread_rolled_back` | rewrite | — | `rewound`, `forked_from` |
| Unknown data | — | — | dropped on rewrite (`extra="ignore"`) | parse failure | skip + keep; `must` → read-only |
| Crash safety | — | — | tmp + fsync + replace | none (`"w"`) | append, fsync at key points, tail repair |
| Permissions | `0600` | dir `0755` | dir `0700` | `0644` | `0600` / `0700` |

(— = not seen in the files or code read.)

**Copy:**

- Codex: `{type, payload}` split; one context record per turn/change;
  compaction that makes resume independent of the past.
- Claude Code: thinking stored with its signature; compaction metadata
  (trigger, tokens before/after, what was kept); `0600` files.
- Vibe: the `injected` flag; a folder per session; fsync; an explicit
  migration step.
- bise's own hub journal: never drop a bad line in silence; let unknown
  kinds through.

**Avoid:**

- Claude Code: the big envelope on every line; one line per content block;
  side records rewritten hundreds of times; interruptions as fake user
  text; a parent tree that every reader must walk.
- Codex: the same fact stored twice (model item + UI event); a usage line
  per stream update.
- Vibe: rewriting the whole file; dropping unknown fields on rewrite;
  deleting the old file after migration.
- bise today: truncate-then-write; absolute paths inside messages;
  results paired by position; world-readable files.

---

## 12. Open decisions

1. **Where logs live.** `~/.bise/sessions/<id>/` for all sessions, with
   hub agents pointing to them by id (proposed), or inside
   `hubs/<hub>/agents/<name>/` so a hub folder holds everything?
2. **Session id shape.** `s-<utc time>-<6 hex>` (proposed), a UUIDv7, or a
   ULID?
3. **Linear log + `rewound`** (proposed) or a `parent` pointer on each
   event (Claude Code's tree, allows branches in one file)?
4. **fsync policy.** At the key points of §8.1 (proposed), after every
   event (simplest, slower on big tool outputs), or never (the hub
   journal today)?
5. **Blob threshold** for text: 256 KiB per line (proposed), or lower
   (64 KiB) to keep files easy to read with `jq` and `less`?
6. **Dual-write of the `.txt`** during the transition: how many releases,
   or until a date? Or no dual-write, and `/version back` refuses to go
   before the JSONL release?
7. **Rotation size and index.** 32 MiB per segment (proposed)? Do we add a
   small index (seq → byte offset) at segment end, or scan?
8. **Request hash.** Store a hash of each provider request in `usage`, to
   prove a resume rebuilds the same bytes (and see cache misses)?
9. **Payload versions.** Keep a `v` per type (proposed), or only the file
   `format` and additive changes forever?
10. **Secret redaction** in tool output: redact known key values at write
    time (proposed)? On resume, the model then sees `«redacted:…»` where
    it saw the key — is that acceptable?
11. **Human logs.** Keep writing `transcript.log` / `wire.log` /
    `context.txt`, or render them from the JSONL with a `bise session
    show` command and stop writing them?
12. **Hub journal.** Move it to the same envelope (`seq`, `at`, `v`,
    `must`, `format` header) now, later, or never?
13. **Prompt history.** Add `~/.bise/history.jsonl` (the prompts typed,
    for ↑ in the input box, like Claude Code and Codex), or keep drafts
    only?
14. **UI-only facts** (`tool_started`, `response_discarded.partial`,
    stream timings): store them in the session log (proposed), or in a
    separate debug file to keep the log smaller?
