#!/usr/bin/env bash
# The agents' gate (projects/switchboard/docs/loop-speed.md), run in the
# foreground, never `sleep N; tail` in a loop:
#   gate.sh [quick]   what the tree's changes touch, ~5-10 s warm: clippy -D
#                     warnings (next to the tests, in $target/clippy), the
#                     tests of the changed crates and of the crates using them; if a .bend file of core/ hub/ vendor/
#                     or LAWS/PROOF changed: PROOF.bend (4 shards in parallel)
#                     and, for hub/ vendor/, a quick sb-core for the tests
#                     (-O1, cached by content; ./sb-core is not touched).
#                     The bend part runs next to cargo.
#   gate.sh full      everything: cargo build, run_all.sh (it puts ./repl-live
#                     ./repl-scripted ./sb-core in place with bins.sh: a copy
#                     from the cache, a compile when their sources changed;
#                     never commit them) with FUZZ_RUNS=2000
#                     (~60 s warm; e2e + tmux tests in parallel, SB_TEST_JOBS),
#                     then no binary built needs a macOS newer than the
#                     target (./bins.sh minos, BISE-164).
#   gate.sh wait <bg .out file | pid>
#                     the bash tool put a gate in the background: block until
#                     it ends (at most 25 s), then show its end and exit code.
# "Changed" = the tree vs GATE_BASE (default HEAD), untracked files included.
# CARGO_TARGET_DIR is honoured (the e2e/tmux tests run its bise);
# the bend results are cached in $CARGO_TARGET_DIR/gate-cache.
set -uo pipefail
mode="${1:-quick}"
if [ "$mode" = wait ]; then
  arg="${2:?usage: gate.sh wait <bg .out file | pid>}"
  if [ -f "$arg" ]; then pid="$(cat "${arg%.out}.pid" 2>/dev/null)"; rcf="${arg%.out}.rc"; else pid="$arg"; rcf=; fi
  for _ in $(seq 25); do { [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; } || break; sleep 1; done
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then echo "still running (pid $pid): gate.sh wait $arg"; exit 0; fi
  [ -f "$arg" ] && tail -n 25 "$arg"
  [ -n "$rcf" ] && [ -f "$rcf" ] && echo "exit code: $(cat "$rcf")"
  exit 0
fi
case "$mode" in quick|full) ;; *) echo "usage: gate.sh [quick|full|wait <file|pid>]" >&2; exit 2 ;; esac
cd "$(dirname "$0")/../../.."
root="$PWD"
export PATH="$HOME/.bend/bin:$HOME/.cargo/bin:$PATH"
unset SB_CORE_BIN
# the oldest macOS the binaries run on (rust/.cargo/config.toml, BISE-164):
# for the quick sb-core's cc too
export MACOSX_DEPLOYMENT_TARGET; MACOSX_DEPLOYMENT_TARGET="$(./bins.sh macos-target)"
base="${GATE_BASE:-HEAD}"
changed="$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)"
hub_changed=0 bend_changed=0
printf '%s\n' "$changed" | grep -qE '^(hub|vendor)/' && hub_changed=1
printf '%s\n' "$changed" | grep -qE '^((core|hub|vendor)/.*|LAWS|PROOF)\.bend$' && bend_changed=1
# ./sb-core (not in git, BISE-114): the build of this tree's hub/ vendor/,
# from bins.sh's cache shared by every worktree (a copy; ~15 s on a miss).
# quick with a hub/ change: the tests run a -O1 build instead (below)
if [ $hub_changed = 0 ] || [ "$mode" = full ]; then
  ./bins.sh sb-core || { echo "FAIL sb-core build"; exit 1; }
fi

if [ "$mode" = full ]; then
  s=$SECONDS
  export FUZZ_RUNS="${FUZZ_RUNS:-2000}"
  projects/switchboard/tests/run_all.sh; rc=$?
  # every binary built runs on the macOS target (BISE-164): the Bend ones,
  # bise, and the engine when this tree has one
  bins=(./repl-live ./repl-scripted ./sb-core "${CARGO_TARGET_DIR:-rust/target}/debug/bise")
  for f in ./harness-demo rust/jsrt/target/debug/bend-jsrt; do [ -e "$f" ] && bins+=("$f"); done
  if [ $rc = 0 ]; then ./bins.sh minos "${bins[@]}" || rc=1; fi
  [ $rc = 0 ] && echo "GATE full GREEN ($((SECONDS - s))s)" || echo "GATE full FAILED"
  exit $rc
fi

# ---- quick
t0=$SECONDS
cache="${CARGO_TARGET_DIR:-$root/rust/target}/gate-cache"
out="$(mktemp -d -t sb-gate)"
mkdir -p "$cache"
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$out"' EXIT
fail() {  # <name> <log>: the failures, the log kept
  echo "FAIL $1"
  grep -E "^test .* FAILED|panicked|^error|^warning|^failures:|^Location|^Error" "$2" | head -30
  cp "$2" "/tmp/sb-gate-$1.log"; echo "log: /tmp/sb-gate-$1.log"; exit 1
}
hash_of() {  # <dirs/files...>: one hash of the .bend files' names and contents
  (cd "$root" && find "$@" -name '*.bend' -type f 2>/dev/null | sort | while read -r f; do echo "$f"; cat "$f"; done) | shasum | cut -c1-16
}
# the bend part, in the background (it takes longer than cargo)
proof_job() {
  local h; h="$(hash_of core hub vendor LAWS.bend PROOF.bend)"
  [ -f "$cache/proof-ok-$h" ] && { echo "ok   PROOF (cached)"; return 0; }
  local d="$out/proof" n=4 s=$SECONDS i pids=()
  mkdir -p "$d"
  python3 projects/switchboard/tests/proof_shards.py "$root" $n "$d" || { echo "FAIL PROOF (split)"; return 1; }
  for i in $(seq 0 $((n - 1))); do bend "$d/P$i.bend" --check-only >"$d/P$i.log" 2>&1 & pids+=($!); done
  wait "${pids[@]}"
  for i in $(seq 0 $((n - 1))); do
    grep -q "ALL PROOFS CHECK" "$d/P$i.log" && continue
    grep -qx "Error: $(cat "$d/P$i.expect") TODOs found." "$d/P$i.log" && continue
    echo "FAIL PROOF ($((SECONDS - s))s): shard $i of $n"; grep -v '^- ' "$d/P$i.log" | head -20
    echo "(rerun whole: bend PROOF.bend)"; return 1
  done
  touch "$cache/proof-ok-$h"; echo "ok   PROOF ($((SECONDS - s))s, $n shards)"
}
sbcore_job() {  # a quick sb-core (-O1: half the compile of bend -o's -O3)
  local h s=$SECONDS; h="$(hash_of hub vendor)"
  [ -x "$cache/sb-core-$h" ] && { echo "ok   sb-core (cached)"; return 0; }
  bend hub/main.bend -o "$out/sb-core.c" >"$out/sb-core.log" 2>&1 \
    && cc -std=c11 -O1 -fno-inline "$out/sb-core.c" -lpthread -lm -o "$out/sb-core" >>"$out/sb-core.log" 2>&1 \
    || { echo "FAIL sb-core build"; tail -20 "$out/sb-core.log"; return 1; }
  rm -f "$cache"/sb-core-*; mv "$out/sb-core" "$cache/sb-core-$h"
  echo "ok   sb-core ($((SECONDS - s))s, quick -O1 build for the tests)"
}
[ $bend_changed = 1 ] && { proof_job >"$out/proof.res" 2>&1; echo $? >"$out/proof.rc"; } &
if [ $hub_changed = 1 ]; then
  { sbcore_job >"$out/sbcore.res" 2>&1; echo $? >"$out/sbcore.rc"; } &
  sbcore_pid=$!
  export SB_CORE_BIN="$cache/sb-core-$(hash_of hub vendor)"
fi
# the crates to test: changed ones and the crates depending on them
pkgs=""
add() { case " $pkgs " in *" $1 "*) ;; *) pkgs="$pkgs $1" ;; esac; }
for f in $changed; do
  case "$f" in
    rust/Cargo.toml|rust/Cargo.lock|rust/.cargo/*) add bise-home; add bise-catalog; add bend-plugins; add bend-images; add bend-tui; add switchboard; add bend-harness ;;
    rust/home/*) add bise-home; add bend-plugins; add bend-images; add bend-tui; add switchboard; add bise-catalog; add bend-harness ;;
    rust/catalog/*) add bise-catalog; add bend-harness ;;
    rust/plugins/*) add bend-plugins; add bend-tui; add bend-harness ;;
    rust/images/*) add bend-images; add bend-tui; add bend-harness ;;
    rust/tui/*) add bend-tui; add bend-harness ;;
    rust/switchboard/*) add switchboard; add bend-harness ;;
    rust/harness/*) add bend-harness ;;
    hub/*|vendor/*) add switchboard ;;
  esac
done
step() {  # <name> <cmd...>: one line when green, the failures and the log when red
  local n=$1 s=$SECONDS; shift
  if "$@" >"$out/$n.log" 2>&1; then echo "ok   $n ($((SECONDS - s))s)"; else fail "$n" "$out/$n.log"; fi
}
# no `cargo build`: clippy checks every target, the tests compile and link
# the changed crates; the binary itself is built by gate.sh full.
# clippy runs next to the tests in its own target dir ($target/clippy: its
# own cargo lock; ~300 MB, 30-40 s the first time)
target="${CARGO_TARGET_DIR:-$root/rust/target}"
{ s=$SECONDS
  if (cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir "$target/clippy" -- -D warnings) >"$out/clippy.log" 2>&1
  then echo "ok   clippy ($((SECONDS - s))s)" >"$out/clippy.res"; echo 0 >"$out/clippy.rc"
  else echo 1 >"$out/clippy.rc"; fi; } &
clippy_pid=$!
if [ -n "$pkgs" ]; then
  args=""; for p in $pkgs; do args="$args -p $p"; done
  if [ -n "${sbcore_pid:-}" ]; then
    step test-build bash -c "cd rust && cargo test --offline -q --no-run $args"
    wait "$sbcore_pid"; cat "$out/sbcore.res"; [ "$(cat "$out/sbcore.rc")" = 0 ] || exit 1
  fi
  step "test$(echo "$pkgs" | tr ' ' '_')" bash -c "cd rust && cargo test --offline -q $args"
  grep "test result" "$out"/test_*.log | grep -v " 0 passed" | sed 's/^.*test result/  test result/'
else
  echo "no Rust or hub change vs $base: no Rust tests run"
fi
wait "$clippy_pid"
[ "$(cat "$out/clippy.rc")" = 0 ] || fail clippy "$out/clippy.log"
cat "$out/clippy.res"
if [ $bend_changed = 1 ]; then
  wait; cat "$out/proof.res"; [ "$(cat "$out/proof.rc")" = 0 ] || exit 1
fi
echo "GATE quick GREEN ($((SECONDS - t0))s):${pkgs:- no Rust tests}$([ $bend_changed = 1 ] && echo ' + PROOF') (full: gate.sh full)"
