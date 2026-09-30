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
