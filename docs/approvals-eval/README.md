# approvals-eval: how design §3.2 and §5.1 were measured

Not product code: the prototype behind the numbers of
[approvals-design.md](../approvals-design.md) §3.2 (call rate) and §5.1
(parser test). The corpus is not committed (real paths and text).

1. `python3 extract.py` → `calls.jsonl`: every tool call in
   `~/.bise/hubs/*/agents/*/wire.log*` (the `assistant_message` events).
   Then write `bash_in.jsonl` (`{"i", "cmd"}`) and `bash_meta.jsonl`
   (`{"i", "hub", "agent", "cmd"}`) from its `bash` calls.
2. `parse.rs` is `src/main.rs` of a crate with `brush-parser = "0.4"` and
   `serde_json`: `bp < bash_in.jsonl > bash_parsed.jsonl` (the parts of
   each command).
3. `python3 tiers.py`: the tiers of design §3 and the cache simulation.
   `TMP_ROOT=0` treats `/tmp` as outside the roots; `READ_ANY=0` sends
   reads outside the roots to the checker. `ROOTS` maps each hub to its
   workspace.

On 2026-10-01: 5 131 bash calls, 57.9 % decided without a model, 6.9 %
checker calls with the per-repo pattern cache. Parser: 2 errors, both real
bash syntax errors (`bash -n` agrees); tree-sitter-bash on the same input:
4 (2 false).

## The product parser on the corpus (`approvals-parser`, 2026-10-01)

`rust/switchboard/src/approvals/` (`judge`) on the same logs, re-read by
`cargo run -p switchboard --release --example approvals_corpus --
harness-3abb2bd8=<workspace> dashboard-e95755e1=<workspace>` (`TMP_ROOT=1`:
`/tmp` as the agent's temp folder, like the prototype). 5 305 bash calls:

| | calls | share |
|---|---|---|
| tier 0, a card | 7 | 0.1 % |
| tier 1, allowed at once | 3 011 | 56.8 % |
| tier 3, deny once (inline code that writes) | 593 | 11.2 % |
| left for the checker | 1 694 | 31.9 % |
| **checker calls, cache per repo by pattern** | **459** | **8.7 %** |
| of which only a network tool keyed by exact text or an `rm -r` | 89 | 1.7 % |

The prototype (`tiers.py`) on the same 5 283 calls: 57.5 % decided, 7.1 %
checker calls. The product adds two rules the design text asks for and the
prototype did not apply: network tools cached by exact text (§4.4) and a
recursive delete inside the repo sent to the checker (§6.3). Without them:
7.0 %. With `TMP_ROOT=0`: see the run. Time (release, M-series, other gates
running): parse alone p50 10 µs, p99 61–65 µs; `judge` with no disk lookup
p50 16 µs, p99 91–106 µs; with the symlink lookups of written paths p99
~160 µs.
