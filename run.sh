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

# the Rust binary: cargo decides what is stale (every crate, every
# Cargo.toml / Cargo.lock; a no-op build is ~0.2 s). A stale debug binary
# once showed a model the runtime no longer used.
# switchboard runs the RELEASE build: its TUI draws ~10x faster
# than the debug one (the sb shim of the agents points to it too)
profile=debug
if [ "${1:-}" = "switchboard" ]; then profile=release; fi
# (an agent's CARGO_TARGET_DIR is honoured: the binary run is the one built)
BIN="${CARGO_TARGET_DIR:-$PWD/rust/target}/$profile/bend-harness"
[ -x "$BIN" ] || echo "bend-harness ($profile) missing — cargo build (first time: a few minutes)..." >&2
if [ "$profile" = release ]; then
  (cd rust && cargo build -q --release -p bend-harness)
else
  (cd rust && cargo build -q -p bend-harness)
fi

# the V8 engine (its own cargo workspace): built when missing OR older
# than its sources (rust/jsrt, rust/images). The harness hands its path
# to the runtime (BEND_JSRT_BIN).
JS=rust/jsrt/target/debug/bend-jsrt
if [ ! -x "$JS" ]; then
  echo "bend-jsrt missing — building the V8 engine (first time: a few minutes)...">&2
  (cd rust/jsrt && CARGO_TARGET_DIR=target cargo build)
elif [ -n "$(find rust/jsrt/src rust/jsrt/Cargo.toml rust/jsrt/Cargo.lock rust/images/src rust/images/Cargo.toml -newer "$JS" -print -quit 2>/dev/null)" ]; then
  echo "bend-jsrt outdated — cargo build (rust/jsrt)...">&2
  (cd rust/jsrt && CARGO_TARGET_DIR=target cargo build) \
    || echo "building bend-jsrt failed — the existing engine is kept" >&2
fi

# the Bend binaries (not in git): ./bins.sh copies each one from a cache
# keyed by the content of its sources, and compiles it on a miss (sb-core
# ~15 s, a REPL 1-2 min). A failed compile keeps the existing binary.
./bins.sh repl-live repl-scripted sb-core

exec "$BIN" "$@"
