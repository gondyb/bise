#!/usr/bin/env bash
# relaunch-live.sh — relaunch the live Switchboard of this repo on a
# MANAGED VERSION of a commit (the working tree may be dirty: other tasks
# work in it; the version is built from the commit, in a temporary git
# worktree, by versions.sh).
#
#   ./relaunch-live.sh              # the last commit (HEAD), not the tree
#   ./relaunch-live.sh a0d3c54      # a given commit
#   ./relaunch-live.sh --no-tui ... # do not open the TUI afterwards
#
# Steps: build the version (cached), back up the hub state (journal,
# sessions, transcripts), stop the running hub keeping its agents (a hub
# with adopt support leaves the REPLs running; the new hub adopts them and
# moves each to the new binary at its next idle, same session), start the
# hub FROM the version's app root, record it as current and last good
# (versions.json: /version then works fully), open the TUI.
#
# SB_LIVE_WS overrides the workspace (default: this repo), SB_STATE_DIR
# the state dir (default: the workspace's) - for tests on a throwaway hub.
set -euo pipefail
cd "$(dirname "$0")"
REPO="$PWD"
WS="${SB_LIVE_WS:-$REPO}"

tui=1
rev=HEAD
for a in "$@"; do
  case "$a" in
    --no-tui) tui=0 ;;
    -*) echo "relaunch-live: unknown option $a" >&2; exit 1 ;;
    *) rev="$a" ;;
  esac
done

# 1. the version (built from the commit, never from the working tree)
vdir="$(./versions.sh build "$rev")"
id="$(basename "$vdir")"
exe="$vdir/bend-harness"
echo "version: $id ($vdir)"

# the hub state of the workspace
if [ -n "${SB_STATE_DIR:-}" ]; then
  state="$SB_STATE_DIR"
else
  # paths.rs: <name>-<fnv1a of the canonical workspace path, low 32 bits>
  state="${XDG_STATE_HOME:-$HOME/.local/state}/switchboard/$(python3 - "$WS" <<'PY'
import os, re, sys
ws = os.path.realpath(sys.argv[1])
h = 0xcbf29ce484222325
for b in ws.encode():
    h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
base = re.sub(r"[^A-Za-z0-9_-]", "-", os.path.basename(ws) or "root")[:32]
print("%s-%08x" % (base, h & 0xFFFFFFFF))
PY
)"
fi
[ -n "$state" ] || { echo "relaunch-live: state dir not found for $WS" >&2; exit 1; }
mkdir -p "$state"

# 2. back up the hub state (journal, sessions, transcripts)
if [ -f "$state/journal.jsonl" ]; then
  backup="/tmp/sb-live-backup-$(date +%Y%m%d-%H%M%S)"
  rsync -a --exclude hub.sock "$state/" "$backup/"
  echo "state backed up: $backup"
fi

hub_pid() {
  local p; p="$(cat "$state/hub.pid" 2>/dev/null || true)"
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then echo "$p"; fi
}

# where the running hub runs from (a version dir, or the dev tree)
prev=""
if [ -n "$(hub_pid)" ]; then
  prev="$(cat "$state/hub.root" 2>/dev/null || true)"
  # 3. stop it, agents kept (an older hub ignores keep_agents: they restart)
  "$exe" switchboard --stop --keep-agents --workspace "$WS" || true
  for _ in $(seq 1 40); do [ -z "$(hub_pid)" ] && break; sleep 0.25; done
  if [ -n "$(hub_pid)" ]; then
    echo "relaunch-live: the hub did not stop" >&2; exit 1
  fi
fi

# 4. start the hub from the version's app root, detached (own session)
rm -f "$state/hub.sock"
(cd "$vdir" && exec python3 -c 'import os,sys; os.setsid(); os.execv(sys.argv[1], sys.argv[1:])' \
   "$exe" sbd --workspace "$WS" </dev/null >/dev/null 2>>"$state/hub.err" &)
for _ in $(seq 1 100); do
  [ -S "$state/hub.sock" ] && [ -n "$(hub_pid)" ] && break
  sleep 0.1
done
if [ -z "$(hub_pid)" ]; then
  echo "relaunch-live: the hub of $id did not start (see $state/hub.err)" >&2
  exit 1
fi
echo "hub pid $(hub_pid), version $id"

# 5. the live is now a managed version: current and last good
python3 - "$state/versions.json" "$vdir" "$prev" <<'PY'
import json, os, sys
path, vdir, prev = sys.argv[1:4]
try:
    st = json.load(open(path))
except Exception:
    st = {}
if prev and os.path.realpath(prev) != os.path.realpath(vdir):
    st["previous"] = prev
st["current"] = vdir
st["good"] = vdir
st["probation_until"] = 0
tmp = path + ".tmp"
json.dump(st, open(tmp, "w"))
os.replace(tmp, path)
PY

# 6. the TUI of that version
if [ "$tui" = 1 ]; then
  cd "$vdir"
  exec "$exe" switchboard --workspace "$WS"
fi
