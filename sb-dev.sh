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
# Layout ($SB_DEV_ROOT, default /tmp/sb-dev):
#   target/            cargo target dir of the dev builds (incremental)
#   cache/             repl-live per Bend-source hash (a Bend compile is 1-2 min)
#   versions/<id>/     one self-contained app root per build:
#                      bend-harness, repl-live, tool-desc-*.txt, prompt-*.txt,
#                      rust/jsrt/target/debug/bend-jsrt, VERSION
#   current            -> versions/<id>, the version the dev hub runs
#   ws/                the dev workspace (a small git repo; SB_DEV_WS overrides)
#   state/             the dev hub state (SB_STATE_DIR): hub.sock, journal...
#
# A version id is <short commit>, plus -dirty-<hash of the changes> for
# an uncommitted tree: the same tree gives the same id (cached).

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
    *) echo "sb-dev : argument inconnu : $a" >&2; exit 1 ;;
  esac
done

say() { echo "sb-dev: $*" >&2; }

hub_pid() {
  local p
  p="$(cat "$SB_STATE_DIR/hub.pid" 2>/dev/null || true)"
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then echo "$p"; fi
}

stop_hub() {
  [ -n "$(hub_pid)" ] || return 0
  local exe="$ROOT/current/bend-harness"
  [ -x "$exe" ] || exe="$REPO/rust/target/debug/bend-harness"
  "$exe" switchboard --stop --workspace "$WS" || true
}

# the version id of the current tree
version_id() {
  local head dirty
  head="$(git rev-parse --short HEAD)"
  if [ -z "$(git status --porcelain)" ]; then echo "$head"; return; fi
  dirty="$( { git diff HEAD; git ls-files --others --exclude-standard -z \
              | xargs -0 shasum 2>/dev/null; } | shasum | cut -c1-8)"
  echo "$head-dirty-$dirty"
}

# hash of everything repl-live is compiled from
bend_hash() {
  find runtime core -type f -name '*.bend' -print0 | sort -z | xargs -0 cat | shasum | cut -c1-12
}

build_version() {
  local id="$1" vdir="$ROOT/versions/$1"
  if [ -x "$vdir/bend-harness" ] && [ -x "$vdir/repl-live" ]; then
    say "version $id déjà construite"
    return 0
  fi
  mkdir -p "$ROOT/cache" "$ROOT/versions"
  local tmp="$vdir.tmp.$$"
  rm -rf "$tmp"; mkdir -p "$tmp/rust/jsrt/target/debug"

  say "cargo build (target $ROOT/target)..."
  (cd rust && CARGO_TARGET_DIR="$ROOT/target" cargo build -q -p bend-harness)
  cp "$ROOT/target/debug/bend-harness" "$tmp/bend-harness"

  local h; h="$(bend_hash)"
  if [ ! -x "$ROOT/cache/repl-live-$h" ]; then
    say "bend runtime/repl-live.bend (1-2 min)..."
    bend runtime/repl-live.bend -o "$ROOT/cache/repl-live-$h.tmp" >/dev/null
    mv "$ROOT/cache/repl-live-$h.tmp" "$ROOT/cache/repl-live-$h"
  fi
  cp "$ROOT/cache/repl-live-$h" "$tmp/repl-live"

  cp tool-desc-*.txt prompt-*.txt "$tmp/"
  # the V8 engine: rarely changes, 100 MB - a hard link when possible
  if [ ! -x rust/jsrt/target/debug/bend-jsrt ]; then
    say "bend-jsrt absent — build du moteur V8..."
    (cd rust/jsrt && cargo build)
  fi
  ln -f rust/jsrt/target/debug/bend-jsrt "$tmp/rust/jsrt/target/debug/bend-jsrt" 2>/dev/null \
    || cp rust/jsrt/target/debug/bend-jsrt "$tmp/rust/jsrt/target/debug/bend-jsrt"

  {
    echo "id=$id"
    echo "commit=$(git rev-parse HEAD)"
    echo "subject=$(git log -1 --format=%s)"
    echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "bend_hash=$h"
  } > "$tmp/VERSION"
  rm -rf "$vdir"; mv "$tmp" "$vdir"
  say "version $id construite : $vdir"
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
  say "le hub n'a pas démarré (voir $SB_STATE_DIR/hub.err)"; exit 1
}

case "$action" in
  stop) stop_hub; exit 0 ;;
  status)
    echo "root:      $ROOT"
    echo "workspace: $WS"
    echo "state:     $SB_STATE_DIR"
    p="$(hub_pid)"
    if [ -n "$p" ]; then echo "hub:       pid $p"; else echo "hub:       arrêté"; fi
    if [ -f "$ROOT/current/VERSION" ]; then sed 's/^/version:   /' "$ROOT/current/VERSION"; fi
    exit 0 ;;
  reset)
    stop_hub
    rm -rf "$WS" "$SB_STATE_DIR" "$ROOT/current"
    say "workspace et état de dev effacés (versions et cache gardés)"
    exit 0 ;;
esac

id="$(version_id)"
build_version "$id"
running="$(basename "$(readlink "$ROOT/current" 2>/dev/null || echo none)")"
if [ -n "$(hub_pid)" ] && [ "$running" != "$id" ]; then
  say "le hub de dev tourne sur $running — redémarrage sur $id (journal rejoué, sessions reprises)"
  stop_hub
fi
ln -sfn "versions/$id" "$ROOT/current"
init_ws
[ -n "$(hub_pid)" ] || start_hub
say "hub de dev pid $(hub_pid), version $id, workspace $WS"
if [ "$tui" = 1 ]; then
  cd "$ROOT/current"
  exec ./bend-harness switchboard --workspace "$WS" ${tui_args[@]+"${tui_args[@]}"}
fi
