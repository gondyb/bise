# BR-006 — A bash command of ~20k chars crashes the REPL (Nat past 2^48-1)

- Severity: high (the REPL process dies, exit 1; the turn is lost)
- Component: runtime/bash-pure.bend (bg_script_hash, lines 198-212 before
  the fix), called by bg_sentinel (bg_wrap) and bash_script_path
  (runtime/bash.bend:71)

## What happens

Every bash tool call hashes the command (bg_sentinel, for the .cmd heredoc
sentinel) and the whole wrapper script (bash_script_path, for the
/tmp/bend-sh-<port>-<hash>.sh file name). The hash was an unbounded Nat:

    acc += char * (pos^2 + 1)

It grows like avg_char * n^3 / 3. The native runtime only has immediate
Nats up to 2^48-1 and aborts past it:

    bend: a Nat past the largest immediate 2^48-1     (exit 1)

The comment assumed "14 chars x 1.1M x ~200", i.e. short commands. With
ASCII text the limit is passed at about 20 000 chars (20 571 chars of
'a'). The wrapper adds ~2k chars, so a command of ~18k chars is enough;
non-ASCII chars lower the limit.

The crash happens after the transcript logs `tool_code`, before the
command runs: the file the command writes is never created.

## Live occurrence (2026-09-28)

Task bend-hub crashed twice (attempts 1/5 and 2/5) on one bash call each
time: `cat > rust/switchboard/src/core_next.rs <<'EOF' …` with 31 269 and
31 509 chars. Hash of the commands alone: 6.78e14 and 6.68e14, over
2^48-1 = 2.81e14. No numeric literal in the content was involved (the
model's guess "a big number in my output" was wrong).

## Minimal repro

    import Base
    import <repo>/runtime/bash-pure.bend as Sh
    def rep(n: Nat, +s: String) -> String:
      match n:
        case 0n:
          ""
        case 1n+k:
          String.append(s, rep(k, s))
    def main() -> String:
      Sh.bg_sentinel(rep(2500n, "aaaaaaaaaa"))    # 25 000 chars

`bend repro.bend -o repro && ./repro` → `bend: a Nat past the largest
immediate 2^48-1`, exit 1. (Interpreted `bend repro.bend` is too slow to
reach it quickly; use the native build.)

## Workaround (before the fix is live)

Write big files with the apply_patch tool (Add File), which does not go
through bash, or split the bash write into chunks under ~15k chars
(`cat > f <<'EOF'` then `cat >> f <<'EOF'`).

## Resolution (fixed)

bg_script_hash keeps its formula, bounded two ways:

- the position weight cycles in 1..4096 (`bg_hash_next_pos`,
  `BG_HASH_CYCLE`);
- the sum folds mod 4294967291, the largest prime under 2^32, once it
  reaches it (`bg_hash_fold`, `BG_HASH_MOD`).

One step adds at most 1.1M x (4096^2 + 1) < 2^45 to a value under 2^32,
so the result stays far below 2^48. Inputs under 4096 chars with a sum
under P hash exactly as before: "ab" = 684 and the pinned wrapper
sentinel `XQ-10236-EOF` do not change.

Why not a rolling hash `(acc*31 + c) mod P`: the checker's Nat is unary.
`Nat.mod`/`Nat.is_lt` on a value of ~1e9 does not finish (PROOF.bend
passed 9 GB RSS). Short law inputs already reach that size after 6 chars.
The fold only compares a small sum (cost ~ its value) and never divides
it.

Verification:
- `bend PROOF.bend --check-only`: ALL PROOFS CHECK (8.3 s, same as
  before).
- New laws: `bg_hash_fold_keeps_small` and `bg_hash_pos_cycles`.
- The repro (native) now prints `XQ-3063375497-EOF`, exit 0.
- 1M chars with emoji hash in 0.3 s, and the results match a Python
  reference.

A law on a 20k-char input is not practical: the checker normalizes it
too slowly.

The live REPLs get the fix only after they are rebuilt and restarted.
