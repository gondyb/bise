#!/usr/bin/env bash
# Every switchboard check, from the repo root:
#   projects/switchboard/tests/run_all.sh          # deterministic (no model)
#   projects/switchboard/tests/run_all.sh --live   # + a real-model smoke test
set -euo pipefail
cd "$(dirname "$0")/../../.."
export PATH="$HOME/.bend/bin:$HOME/.cargo/bin:$PATH"
echo "== Bend laws (PROOF.bend)"
bend PROOF.bend | grep -E "PROOFS"
echo "== Rust: build, unit and scenario tests, clippy"
# an agent's shell points SB_CORE_BIN at its hub's (older) sb-core: the
# core tests must spawn this tree's
unset SB_CORE_BIN
log="$(mktemp -t sb-run-all)"
(cd rust && cargo build --offline -q) || exit 1
# every test binary's summary; a failure stops here with cargo's report
if ! (cd rust && cargo test --offline -q -p switchboard -p bend-tui) >"$log" 2>&1; then
  grep -E "^test .* FAILED|^failures:|panicked|test result" "$log" | head -40
  echo "FAILED: cargo test (full log: $log)"
  exit 1
fi
grep "test result" "$log"
rm -f "$log"
(cd rust && cargo clippy --offline -q --workspace --all-targets -- -D warnings)
echo "== E2E (real hub, REPLs, git; scripted provider)"
python3 -u projects/switchboard/tests/e2e.py
echo "== TUI under tmux"
for t in tui_tmux tui_help_tmux tui_term_tmux tui_composer_tmux tui_version_tmux tui_at_files_tmux tui_images_tmux tui_clear_tmux tui_archived_tmux tui_waits_tmux tui_undelivered_tmux tui_queue_tmux tui_onboarding_tmux; do
  python3 -u "projects/switchboard/tests/$t.py" | tail -1
done
if [ "${1:-}" = "--live" ]; then
  echo "== live model"
  python3 -u projects/switchboard/tests/live_smoke.py
fi
