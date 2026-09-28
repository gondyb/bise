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
(cd rust && cargo build --offline -q && cargo test --offline -q -p switchboard 2>&1 | grep "test result" | head -1 \
  && cargo clippy --offline -q -p switchboard -p bend-tui -p bend-harness -- -D warnings)
echo "== E2E (real hub, REPLs, git; scripted provider)"
python3 -u projects/switchboard/tests/e2e.py
echo "== TUI under tmux"
for t in tui_tmux tui_help_tmux tui_term_tmux tui_composer_tmux tui_version_tmux tui_at_files_tmux tui_images_tmux; do
  python3 -u "projects/switchboard/tests/$t.py" | tail -1
done
if [ "${1:-}" = "--live" ]; then
  echo "== live model"
  python3 -u projects/switchboard/tests/live_smoke.py
fi
