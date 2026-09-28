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

The bug was fixed twice, in parallel: on `switchboard` (f56a746: the
positional sum bounded by a cycling weight and a fold mod the largest
prime under 2^32) and on `main` (5551368: FNV-1a in U32). The merge of
`switchboard` into `main` keeps ONE fix, main's: `bg_script_hash` is
FNV-1a over the code points in U32, so the arithmetic wraps mod 2^32 by
construction and no script length can overflow. The hash stays a U32
end to end (`U32.show` in the file name and the heredoc sentinel), and
a U32 normalizes fast in the laws (the switchboard fix had to avoid
`Nat.mod` on large values because the checker's Nat is unary).

Laws: `bg_script_hash_deterministic` ("ab" = 1294271946),
`bg_script_hash_high_code_points` (code points past 2^20),
`bg_script_hash_distinguishes`. The switchboard laws
`bg_hash_fold_keeps_small` and `bg_hash_pos_cycles` went with their
code.

Verification (5551368): a 42k-char accented script reproduced the crash
before; a 72k-char accented heredoc through `Sh.bash_exec` exits 0
after.

The live REPLs get the fix only after they are rebuilt and restarted.
