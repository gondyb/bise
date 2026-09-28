#!/usr/bin/env bash
# relaunch-live.sh — relaunch the live Switchboard of this repo on the
# current code, cleanly: check the tree, back up the state, stop the
# running hub, start the new one (release build, journal replayed,
# agent sessions resumed).
#
#   ./relaunch-live.sh
set -euo pipefail
cd "$(dirname "$0")"

# 1. the tree must be clean (the build takes the working tree as is);
#    the Bend binaries rebuilt by run.sh do not count
dirty=$(git status --porcelain --untracked-files=no -- . ':!repl-live' ':!repl-scripted' ':!sb-core')
if [ -n "$dirty" ]; then
  echo "working tree not clean, relaunch cancelled:" >&2
  echo "$dirty" >&2
  exit 1
fi

# 2. back up the hub state (journal, sessions, transcripts)
state=$(ls -d "${XDG_STATE_HOME:-$HOME/.local/state}"/switchboard/harness-switchboard-* 2>/dev/null | head -1 || true)
if [ -n "$state" ]; then
  backup="/tmp/sb-live-backup-$(date +%Y%m%d-%H%M%S)"
  cp -R "$state" "$backup"
  echo "state backed up: $backup"
fi

# 3. stop the running hub (and its agents: the old hub cannot keep them)
./run.sh switchboard --stop || true
for _ in $(seq 1 20); do
  pid=$(cat "$state/hub.pid" 2>/dev/null || true)
  if [ -z "$pid" ] || ! kill -0 "$pid" 2>/dev/null; then break; fi
  sleep 0.5
done

# 4. start the new one
exec ./run.sh switchboard
