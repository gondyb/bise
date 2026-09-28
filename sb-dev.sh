#!/usr/bin/env bash
# sb-dev.sh — a throwaway Switchboard next to the live one.
#
# Builds the CURRENT tree (uncommitted changes included) into a
# separate place, then runs an isolated hub on it: its own workspace,
# state dir, socket, journal and agents. The live tree's binaries
# (rust/target, ./repl-live) and the live hub are never touched.
#
#   ./run.sh switchboard --dev            # build, (re)start the dev hub, open its TUI
#   ./run.sh switchboard --dev --no-tui   # same, without the TUI (scripts, agents)
#   ./run.sh switchboard --dev --stop     # stop the dev hub
#   ./run.sh switchboard --dev --status   # dev hub pid, version, paths
#   ./run.sh switchboard --dev --reset    # stop + wipe the dev workspace and state
#
# The build is a version of the versions cache (versions.sh build --tree:
# ~/.local/state/switchboard/versions/<id>, shared with the live hub's
# version selector). Layout ($SB_DEV_ROOT, default /tmp/sb-dev):
#   current            -> the version dir the dev hub runs
#   ws/                the dev workspace (a small git repo; SB_DEV_WS overrides)
#   state/             the dev hub state (SB_STATE_DIR): hub.sock, journal...

set -euo pipefail
cd "$(dirname "$0")"
REPO="$PWD"
ROOT="${SB_DEV_ROOT:-/tmp/sb-dev}"
WS="${SB_DEV_WS:-$ROOT/ws}"
export SB_STATE_DIR="$ROOT/state"
export PATH="$HOME/.cargo/bin:$HOME/.bend/bin:$PATH"

action=run
tui=1
tui_args=()
for a in "$@"; do
  case "$a" in
    --dev) ;;
    --no-tui) tui=0 ;;
    --stop) action=stop ;;
    --status) action=status ;;
    --reset) action=reset ;;
    --debug) tui_args+=(--debug) ;;
    *) echo "sb-dev: unknown argument: $a" >&2; exit 1 ;;
  esac
done

say() { echo "sb-dev: $*" >&2; }

hub_pid() {
  local p
  p="$(cat "$SB_STATE_DIR/hub.pid" 2>/dev/null || true)"
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then echo "$p"; fi
}

stop_hub() {  # [--keep-agents]
  [ -n "$(hub_pid)" ] || return 0
  local exe="$ROOT/current/bend-harness"
  [ -x "$exe" ] || exe="$REPO/rust/target/debug/bend-harness"
  "$exe" switchboard --stop "$@" --workspace "$WS" || true
  for _ in $(seq 50); do [ -z "$(hub_pid)" ] && return 0; sleep 0.1; done
}

init_ws() {
  [ -d "$WS/.git" ] && return 0
  mkdir -p "$WS"
  (cd "$WS" && git init -q && echo "# sb-dev test workspace" > README.md \
     && git add README.md && git -c user.name=sb-dev -c user.email=sb-dev@localhost commit -qm init)
}

# start the hub detached (its own session: it outlives this script and the TUI)
start_hub() {
  mkdir -p "$SB_STATE_DIR"
  (cd "$ROOT/current" && exec python3 -c 'import os,sys; os.setsid(); os.execv(sys.argv[1], sys.argv[1:])' \
     "$ROOT/current/bend-harness" sbd --workspace "$WS" \
     </dev/null >/dev/null 2>>"$SB_STATE_DIR/hub.err" &)
  for _ in $(seq 150); do
    [ -S "$SB_STATE_DIR/hub.sock" ] && [ -n "$(hub_pid)" ] && return 0
    sleep 0.1
  done
  say "the hub did not start (see $SB_STATE_DIR/hub.err)"; exit 1
}

case "$action" in
  stop) stop_hub; exit 0 ;;
  status)
    echo "root:      $ROOT"
    echo "workspace: $WS"
    echo "state:     $SB_STATE_DIR"
    p="$(hub_pid)"
    if [ -n "$p" ]; then echo "hub:       pid $p"; else echo "hub:       stopped"; fi
    if [ -f "$ROOT/current/VERSION" ]; then sed 's/^/version:   /' "$ROOT/current/VERSION"; fi
    exit 0 ;;
  reset)
    stop_hub
    rm -rf "$WS" "$SB_STATE_DIR" "$ROOT/current"
    say "dev workspace and state wiped (versions and cache kept)"
    exit 0 ;;
esac

vdir="$(./versions.sh build --tree)"
id="$(basename "$vdir")"
running="$(readlink "$ROOT/current" 2>/dev/null || echo none)"
if [ -n "$(hub_pid)" ] && [ "$running" != "$vdir" ]; then
  # a hot switch: the agents' REPLs keep running (their turns too); the
  # new hub adopts them, and each moves to the new binary at its next
  # idle, same session
  say "switching the dev hub: $(basename "$running") -> $id (agents kept)"
  stop_hub --keep-agents
fi
mkdir -p "$ROOT"; ln -sfn "$vdir" "$ROOT/current"
init_ws
[ -n "$(hub_pid)" ] || start_hub
say "dev hub pid $(hub_pid), version $id, workspace $WS"
if [ "$tui" = 1 ]; then
  cd "$ROOT/current"
  exec ./bend-harness switchboard --workspace "$WS" ${tui_args[@]+"${tui_args[@]}"}
fi
