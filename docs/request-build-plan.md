# The provider request, built once: the plan (option B)

Status: a plan, nothing implemented. Measured on 2026-09-30 by the
task `repl-cpu-2` against build 529a134 (the live one) and main
a57f628 (the file:line anchors below).

## 1. The short version

1. **Why.** A repl-live CPU spike (70-100% for 1-4 s) is one model
   call: the request is built before the stream starts. On a fresh
   process that costs ~0.3 s for a 780 KB context (openai-tools' session,
   ~235k tokens). The same call costs **~35 ms more for every call the
   process made before** (0.26 s at call 1, 2.6 s at call 60, request
   size and RSS flat, same profile). A restart on the saved session brings
   it back to 0.29 s. Live: openai-tools 2.8-4.2 s per call, main
   1.7-2.9 s. An idle agent costs 0.
2. **Cause of the growth (likely):** heap locality. A Bend `String` is a
   cons list, one node per character; the runtime's free chains are LIFO
   per size class, so the nodes of the strings each call allocates end
   up scattered over the heap, and every pass over them gets slower.
   Every call makes ~12 passes (copies, parses, prints) over the whole
   context, so the cost is (passes) x (context size) x (age factor).
3. **B** cuts the passes: the provider body is printed straight from the
   session's messages (no wire text, no parse, no JSON tree), then only
   the tail is printed on each call (the prefix is the same bytes the
   prompt cache already requires). Expected: ~200 ms -> ~20 ms of request
   build per call fresh, and most of the aging gone with the
   allocations. Cost: L overall, in 5 steps that each ship alone.
4. **The one hard constraint** is BISE-268: the body must stay byte for
   byte what it is today, so the provider's prompt cache keeps hitting.
   Each step is proven by a law "new body == old body" on fixtures, a fuzz
   over random histories, and a byte diff of real sessions' dumps.

## 2. What a call does today

The live loop (`runtime/main.bend:697-706`, `Rtp.EModel`) runs, for
every model call, on the whole context. Measured with a stage bench
(Appendix A) on openai-tools' session: 338 messages, 725k-char wire
request, 753k-char Anthropic body; fresh process, 3 runs.

| # | pass | where (main a57f628) | ms fresh |
|---|---|---|---|
| 1 | checkpoint: session -> text, written | `main.bend:651` save_rt -> `persist.bend:78` `K.to_text` (`checkpoint.bend:164`) | 39-41 |
| 2 | model input (pair calls) | `session.bend:60` model_input | 0 |
| 3 | wire text: `MSG`/`TOOL` lines, `escape_nl` per message | `main-pure.bend:107` -> `remote.bend:94` build_request, `:62-77` ser_msg(s) | 35 |
| 4 | context message swapped in for `END` (one walk + copy) | `main.bend:706` -> `remote.bend:159-183` with_context | 17 |
| 5 | image marker scan | `provider.bend:560` img_req -> `image.bend:27` has_image | 4 |
| 6 | wire text parsed back: split into lines, `unescape_nl` | `provider.bend:599` -> `wire.bend:332` parse_wire | 27-31 |
| 7 | unpaired-call guard, cache key, pairing | `wire.bend:1091`, `api.bend:233`, `wire.bend:1085` | 0 |
| 8 | JSON tree built, then printed (`quote_string` per text) | `provider.bend:610` -> `api.bend:266` -> `anthropic.bend:627` / `oai-chat.bend:306` / `oai-resp.bend:258`; `json.bend:1113,1244` | 53-59 |
| 9 | cache key field + `"stream":true` (two copies) | `api.bend:248` with_cache_key, `api.bend:154` wire_body | 3-5 |
| 10 | image splice (a scan when images) + UTF-8 encode (reversed list, then `String.reverse`) | `provider.bend:611` -> `image.bend:429`, `encoding.bend:115-123` | 22 |
| | **total** | | **~200** |

A real call in a fresh repl-live costs 0.26-0.35 s: the ~100 ms left are
the reply side (stream parse, usage lines, the transition, events),
not broken down yet (step 0). One plain walk of the 725k-char request
costs ~4 ms fresh (pass 5): the floor B aims at is a few walks.

With images, passes 5 and 10 grow (split, defang, splice per marker),
but with one marker the cost was the same as without (0.419 vs 0.418 s
per call).

## 3. The target design

- **What builds the body:** a pure module `core/body.bend`, one printer
  per family (Chat, Anthropic, Responses), that prints the provider body
  as **UTF-8 text directly** from what the core already holds: the system
  prompt and tool catalog (`T.Cfg`), the paired history
  (`List<T.Msg>`, `model_input`), the context text, the image flags and
  the model facts. No wire text, no `WReq`, no `J.Json` tree: each message
  goes to its JSON with one escape (`quote_string` fused with the UTF-8
  encode).
- **What the loop carries:** `Rtp.EModel` carries the input (messages +
  mode: agent / compact / bare) instead of a request string;
  `main.bend:697-706` and `provider.bend:582-616` take it.
- **The prefix cache:** the runtime keeps the printed body of messages
  `0..k` (k = everything before the moving cache marks, i.e. all but the
  2 newest messages) with a key: family, model facts, system, tools,
  image-keep step, compaction count. Each call prints only messages
  `k+1..n` + the context + the closing, and sends prefix then tail
  (two sends, no copy of the prefix). A key change prints all once.
- **What stays:** the wire text and `parse_wire` stay for the scripted
  REPL, `BISE_ONESHOT` (it reads a wire request file), the Step Protocol
  scenarios and the laws, and as the **reference** the new printer is
  proven against. The checkpoint format does not change.

## 4. What must stay byte-identical (BISE-268) and how to prove it

The provider caches the longest byte-identical prefix of the body. Today
the laws `prefix_stable_oai` / `prefix_stable_anth` (LAWS.bend, section
"the prompt cache (BISE-268)") check it through `pc_body`: each request
of a session starts with the whole previous one up to its context
message, cache marks aside. B must not change **one byte** of any body,
in any family:

- the key order of every object, the escapes (`quote_string`: `\n`,
  `\"`, `\\`, control chars, non-ASCII as UTF-8 not `\u`), the blank
  text blocks Anthropic refuses, the `cache_control` marks on the 2
  newest messages and never on the context, the call ids (`call_<n>`),
  the tool-name mapping (BISE-293), thinking blocks and signatures,
  Responses' encrypted reasoning items (BISE-147), the cache key field
  first, `"stream":true` first, the image placeholders and the stepped
  image keep (`image_keep_stepped`), the unpaired-call answers.

Proof, per step:

1. **Equivalence laws** in LAWS.bend: for each family,
   `Body.print(st, cfg, input, ctx, facts) == Api.api_body_for(st, ...,
   W.pair_wire(W.parse_wire(Rm.with_context(ctx, Rm.build_request(cfg,
   input)))))` on fixtures that cover the list above (multi-line text,
   a literal backslash-n, CALL args with newlines, thinking, images kept
   and defanged, an unpaired call, empty and blank texts, non-ASCII, a
   tool description with newlines, the compact and bare modes).
2. **The prefix laws** rewritten over the new printer, plus one law for
   the prefix cache: `prefix(k) ++ tail(k) == print(all)` for every k at
   or before the marks.
3. **A fuzz**: random histories (the gate's `FUZZ_RUNS=2000` run),
   old path vs new path, compared byte for byte.
4. **Real sessions**: `BEND_WIRE_DUMP` from the old and the new binary
   on copies of the user's sessions (main: 32 images; marketing: 22;
   openai-tools: 758 KB), each family, through the fake provider
   (`tests/fake_provider.py`): `cmp` must be silent. Then one live call
   per family checks `cache_read` in the usage line stays at the
   previous call's input size.

## 5. The steps, in order

Gains: fresh = measured stage cost removed (Appendix A); aged = the same
x the age factor (~10 after 60 calls, profile unchanged), to be measured
with the 60-call repro (Appendix B) after each step.

| step | what | cost | risk | gain per call, fresh | aged (60 calls) |
|---|---|---|---|---|---|
| 0 | break down the ~100 ms outside the request build (reply side); add the stage timings to the debug log | S | none | - | - |
| 1 | UTF-8 encode in one pass (no reversed accumulator + `String.reverse`), and `"stream":true` + cache key printed in place (no 2 copies) | S | low: output identical, `encoding.bend` is vendored | ~15 ms (22+4 -> ~10) | ~0.15 s |
| 2 | the direct printer (`core/body.bend`), 3 families, behind the equivalence laws; the live path uses it, the wire path stays for the rest | L (~800-1200 lines incl. laws) | medium: byte identity over 3 families; mitigated by §4 1-4 | ~110 ms (passes 3, 4, 6 gone, 8 halved, 5 folded in) | ~1.1 s |
| 3 | the prefix cache: print only the tail; send prefix then tail (the HTTP layer takes a list of pieces) | M | medium: a stale prefix = a wrong body. Mitigated by the key (§3) and the law prefix ++ tail == all; `BEND_WIRE_DUMP` in e2e | ~50 ms (the remaining print of the whole body -> the tail) | ~0.5 s, and far fewer allocations (§6) |
| 4 | checkpoint by appending: write only the new messages, rewrite the file at compaction and at start | M | medium: crash consistency of a partial append (a torn last line must be dropped at load; the laws `checkpoint_*_roundtrip` extended) | ~35 ms | ~0.35 s |

After steps 1-4: request build ~200 ms -> ~15-25 ms fresh (one walk of
the prefix to send, the tail printed). A fresh call would then cost ~0.1 s
plus the reply side (step 0 tells).

Order: 0 and 1 first (small, independent). 2 is the core of B and the
only large piece. 3 only makes sense on top of 2. 4 is independent of
2-3 and can go any time.

## 6. Does B reduce the aging?

Yes, in two ways, both to be measured with Appendix B:

- **Fewer passes.** The age factor multiplies each pass over scattered
  nodes, so ~12 passes -> ~3 cuts the aged cost by about the same ratio
  (2.6 s -> ~0.7 s at call 60 after step 2, estimated).
- **Far fewer allocations.** Today each call allocates and frees ~10
  copies of the context (~7M nodes); that churn is what scatters the free
  chains. With the prefix cache, the prefix is allocated once and stays
  where it was, and a call allocates only its tail (a few KB). The
  scattering should then grow very slowly: the aged cost should stay
  near the fresh one (estimate, not measured).

What B does not fix: other big long-lived strings walked often (the
history's message texts are walked once when a message is printed, then
never again with the prefix cache).

## 7. Option C, for the Bend team (3 lines)

The runtime's per-class LIFO free chains (`heap_alloc`/`heap_free`,
`heap_hand` parking to banks) scatter cons cells: a 780 KB `String`
walked ~12x per call gets ~10x slower after 60 calls with flat RSS
(repro: Appendix B, 60 calls, same session, then restart). Fix options:
address-ordered or periodically sorted free chains, a bump arena for
short-lived allocations per IO step, or a compact string (rope / byte
buffer) in Base; the repro and the stage bench are the acceptance tests.

## Appendix A: the stage bench

`bend/bench_stages.bend` (not committed; recreate from this):
it loads a session like repl-live (`P.load` with `BEND_CONTINUE=1`,
`BEND_SESSION_FILE`), then times each pass of §2 with `IO.now()` and
forces it with `String.length`, 3 times in a row. Build with
`bend bench_stages.bend -o bench_stages`, run with
`env -i PATH=$PATH HOME=<tmp> BEND_CONTINUE=1 BEND_SESSION_FILE=<copy of
a session.resume.txt> CTX_TEXT="$(cat <context.txt>)" ./bench_stages`.
The passes it calls: `K.to_text(K.export(sess))`,
`S.model_input(cfg, P.history_of(sess))`, `Rem.build_request`,
`Rem.with_context`, `Im.has_image`, `W.parse_wire`,
`W.wire_unpaired`, `Api.session_key`, `W.pair_wire`,
`Api.api_body_for(Api.Anthropic{}, ...)`, `Api.with_cache_key` +
`Api.wire_body`, `Im.img_splice` + `Enc.utf8.encode`.

## Appendix B: the 60-call repro

A copy of a live repl-live binary, a copy of an agent's
`session.resume.txt` (tool names with a dot renamed for a fake at
HEAD: `self.compact` -> `self_compact`), temp HOME, the fake provider
of the binary's commit (`git show <sha>:tests/fake_provider.py`)
behind a temp `config.toml` with `[providers.foundry] base_url =
"http://127.0.0.1:<port>/v1"`, `BEND_MODEL=foundry/claude-opus-5-5`,
`BEND_CONTINUE=1`, the agent's context.txt and role.md. Send one line
`[[bash: echo 0]] ... [[bash: echo 59]]` on the REPL port, and read the
process's CPU time (`ps -o time=`) at each new line of `$FAKE_LOG`.
Measured (build 529a134): 0.26, 0.30, 0.33 ... 1.26 (call 30) ... 2.6 s
(call 60); RSS 101-102 MB throughout; a second turn keeps growing;
restarted on the saved session after 40 calls: 0.29-0.37 s.
