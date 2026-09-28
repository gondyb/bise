#!/usr/bin/env bash
# Convenience wrapper: everything starts from the single executable
# rust/target/debug/bend-harness (child Bend REPL + TUI). The provider
# (HTTPS to api.mistral.ai) and the bash tool run INSIDE the Bend REPL
# (hub HTTP + snap packages) — no bridge anymore.
#
#   ./run.sh                 # live session (opus-5.5 + bash)
#   ./run.sh --scripted      # scripted session
#   ./run.sh --model NAME    # provider model
#   ./run.sh --port N        # force the REPL port
#   ./run.sh --debug        # show the annotations (turns, idle)
#   ./run.sh --continue     # resume the most recent session
#                           # (by last activity, not a fixed file)
#   ./run.sh switchboard    # Switchboard: main + tasks, in the current folder
#   ./run.sh switchboard --stop  # stop the hub of the current folder
#   ./run.sh switchboard --dev   # test Switchboard next to the live one: build
#                                # isolated in /tmp/sb-dev, separate hub
#                                # (see sb-dev.sh: --no-tui, --stop, --status, --reset)
#   ./run.sh --resume ID    # resume a session by id
#                           # (a unique prefix is enough); /status in
#                           # the TUI shows the session id
#
# Several terminals = several ./run.sh: ports assigned automatically,
# independent sessions, each REPL dies with its terminal.

set -euo pipefail
# the dev switchboard builds elsewhere: never the live tree's binaries
if [ "${1:-}" = "switchboard" ] && [[ " $* " == *" --dev "* ]]; then
  shift
  exec "$(dirname "$0")/sb-dev.sh" "$@"
fi
# switchboard: the workspace is where the user launched from
export SB_LAUNCH_DIR="${SB_LAUNCH_DIR:-$PWD}"
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

# rebuild when the binary is missing OR stale (a source file is newer
# than it — a stale debug binary once showed a model the runtime no
# longer used)
# switchboard runs the RELEASE build: its TUI draws ~10x faster
# than the debug one (the sb shim of the agents points to it too)
profile=debug
if [ "${1:-}" = "switchboard" ]; then profile=release; fi
BIN="rust/target/$profile/bend-harness"
if [ ! -x "$BIN" ] \
   || [ -n "$(find rust/harness/src rust/tui/src rust/switchboard/src -newer "$BIN" -print -quit 2>/dev/null)" ]; then
  echo "bend-harness ($profile) missing or outdated — cargo build..." >&2
  if [ "$profile" = release ]; then
    (cd rust && cargo build --release -p bend-harness)
  else
    (cd rust && cargo build -p bend-harness)
  fi
fi

if [ ! -x rust/jsrt/target/debug/bend-jsrt ]; then
  echo "bend-jsrt missing — building the V8 engine (first time: a few minutes)...">&2
  (cd rust/jsrt && cargo build)
fi

# the Bend REPLs: rebuilt when absent OR older than any Bend source
# (runtime/, core/, vendor/, the tool descriptions). A failed rebuild keeps
# the existing binary when there is one (no toolchain: still runnable).
export PATH="$HOME/.bend/bin:$PATH"
bend_stale() {
  [ ! -x "$1" ] || [ -n "$(find runtime core vendor tool-desc-*.txt -newer "$1" -print -quit 2>/dev/null)" ]
}
build_repl() {
  local out="$1" src="$2"
  if bend_stale "$out"; then
    echo "$out missing or outdated — compiling with bend (1-2 min)..." >&2
    if ! bend "$src" -o "$out" >/dev/null; then
      if [ -x "$out" ]; then
        echo "compiling $out failed — existing binary kept" >&2
      else
        echo "compiling $out failed" >&2
        exit 1
      fi
    fi
  fi
}
build_repl repl-live runtime/repl-live.bend
build_repl repl-scripted runtime/repl.bend
# sb-core: the Switchboard hub's decisions (hub/*.bend), a child of the
# switchboard daemon
if [ ! -x sb-core ] || [ -n "$(find hub vendor -newer sb-core -print -quit 2>/dev/null)" ]; then
  echo "sb-core missing or outdated — compiling with bend..." >&2
  if ! bend hub/main.bend -o sb-core >/dev/null && [ ! -x sb-core ]; then
    echo "compiling sb-core failed" >&2
    exit 1
  fi
fi

exec "./$BIN" "$@"
