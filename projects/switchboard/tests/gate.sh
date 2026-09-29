#!/usr/bin/env bash
# The agents' gate (projects/switchboard/docs/loop-speed.md), run in the
# foreground, never `sleep N; tail` in a loop:
#   gate.sh [quick]   what the tree's changes touch: build, clippy, the tests
#                     of the changed crates and of the crates using them;
#                     PROOF + sb-core rebuild if hub/*.bend changed. ~5-20 s warm.
#   gate.sh full      everything: run_all.sh with FUZZ_RUNS=2000 (~60 s warm);
#                     e2e + tmux tests in parallel (SB_TEST_JOBS, default 4).
#   gate.sh wait <bg .out file | pid>
#                     the bash tool put a gate in the background: block until
#                     it ends (at most 25 s), then show its end and exit code.
# "Changed" = the tree vs GATE_BASE (default HEAD), untracked files included.
# CARGO_TARGET_DIR is honoured (the e2e/tmux tests run its bend-harness).
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
cd "$(dirname "$0")/../../.."
export PATH="$HOME/.bend/bin:$HOME/.cargo/bin:$PATH"
unset SB_CORE_BIN
# sb-core (hub/*.bend) at the tree's root: the core tests spawn it
base="${GATE_BASE:-HEAD}"
changed="$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)"
if printf '%s\n' "$changed" | grep -qE '^(hub|vendor)/'; then
  echo "hub/ changed: PROOF + sb-core rebuild (1-2 min)"
  bend PROOF.bend | grep -q "ALL PROOFS CHECK" || { bend PROOF.bend | tail -20; echo "FAIL PROOF"; exit 1; }
  bend hub/main.bend -o sb-core >/dev/null || { echo "FAIL sb-core build"; exit 1; }
elif [ ! -x sb-core ]; then
  main_tree="$(git worktree list --porcelain | sed -n '1s/^worktree //p')"
  if [ -x "$main_tree/sb-core" ] && git diff --quiet "$(git -C "$main_tree" rev-parse HEAD)" -- hub vendor; then
    cp "$main_tree/sb-core" sb-core
  else
    echo "no sb-core: building it (1-2 min)"; bend hub/main.bend -o sb-core >/dev/null || { echo "FAIL sb-core build"; exit 1; }
  fi
fi
if [ "$mode" = full ]; then
  export FUZZ_RUNS="${FUZZ_RUNS:-2000}"
  s=$SECONDS
  projects/switchboard/tests/run_all.sh; rc=$?
  [ $rc = 0 ] && echo "GATE full GREEN ($((SECONDS - s))s)" || echo "GATE full FAILED"
  exit $rc
fi
[ "$mode" = quick ] || { echo "usage: gate.sh [quick|full|wait <file|pid>]" >&2; exit 2; }
# the crates to test: changed ones and the crates depending on them
pkgs=""
add() { case " $pkgs " in *" $1 "*) ;; *) pkgs="$pkgs $1" ;; esac; }
for f in $changed; do
  case "$f" in
    rust/Cargo.toml|rust/Cargo.lock) add bend-plugins; add bend-images; add bend-tui; add switchboard; add bend-harness ;;
    rust/plugins/*) add bend-plugins; add bend-tui; add bend-harness ;;
    rust/images/*) add bend-images; add bend-tui; add bend-harness ;;
    rust/tui/*) add bend-tui; add bend-harness ;;
    rust/switchboard/*) add switchboard; add bend-harness ;;
    rust/harness/*) add bend-harness ;;
    hub/*|vendor/*) add switchboard ;;
  esac
done
t0=$SECONDS
out="$(mktemp -d -t sb-gate)"
step() {  # <name> <cmd...>: one line when green, the failures and the log when red
  local n=$1 s=$SECONDS; shift
  if "$@" >"$out/$n.log" 2>&1; then echo "ok   $n ($((SECONDS - s))s)"
  else
    echo "FAIL $n ($((SECONDS - s))s)"
    grep -E "^test .* FAILED|panicked|^error|^warning|^failures:" "$out/$n.log" | head -30
    echo "log: $out/$n.log"; exit 1
  fi
}
step build bash -c 'cd rust && cargo build --offline -q'
step clippy bash -c 'cd rust && cargo clippy --offline -q --workspace --all-targets -- -D warnings'
if [ -n "$pkgs" ]; then
  args=""; for p in $pkgs; do args="$args -p $p"; done
  step "test$(echo "$pkgs" | tr ' ' '_')" bash -c "cd rust && cargo test --offline -q $args"
  grep "test result" "$out"/test*.log | grep -v " 0 passed" | sed 's/^.*test result/  test result/'
else
  echo "no Rust or hub change vs $base: no tests run"
fi
rm -rf "$out"
echo "GATE quick GREEN ($((SECONDS - t0))s):${pkgs:- nothing to test} (full: gate.sh full)"
