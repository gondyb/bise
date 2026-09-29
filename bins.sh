#!/usr/bin/env bash
# bins.sh — the Bend native binaries (repl-live, repl-scripted, sb-core,
# harness-demo). They are NOT in git (BISE-114): each one is built from
# its sources into a cache keyed by the CONTENT of those sources, shared
# by every worktree and by versions.sh, then copied where it is run.
#
#   ./bins.sh [--src <dir>] <name>...      <dir>/<name> = the build of <dir>'s
#                                          sources (default <dir>: this repo)
#   ./bins.sh path [--src <dir>] <name>    build if needed, print the cache file
#   ./bins.sh key [--src <dir>] <name>     the cache key (hash of the sources)
#
# A hit is a copy (~0.3 s); a miss is a bend compile (sb-core ~15 s,
# the REPLs 1-2 min). Cache: $SB_BUILD_DIR/cache/<name>-<key> (default
# SB_BUILD_DIR=${XDG_STATE_HOME:-~/.local/state}/switchboard/build), the
# 12 newest of each name are kept. A failed compile keeps an existing
# <dir>/<name> (no toolchain: still runnable). A binary the EDR ate is
# rebuilt the same way: run the script again.
set -euo pipefail
REPO="$(cd "$(dirname "$0")" && pwd)"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}/switchboard"
CACHE="${SB_BUILD_DIR:-$STATE/build}/cache"
export PATH="$HOME/.bend/bin:$PATH"

say() { echo "bins: $*" >&2; }

# <name> -> "<main .bend> <source dirs...>"
recipe() {
  case "$1" in
    repl-live) echo "runtime/repl-live.bend runtime core vendor" ;;
    repl-scripted) echo "runtime/repl.bend runtime core vendor" ;;
    harness-demo) echo "runtime/demo.bend runtime core vendor" ;;
    sb-core) echo "hub/main.bend hub vendor" ;;
    *) say "unknown binary: $1 (repl-live repl-scripted sb-core harness-demo)"; exit 2 ;;
  esac
}

# the key: every .bend file under the source dirs, by content (never a
# date: a fresh checkout has fresh mtimes). The same key versions.sh
# always used, so its cache stays warm. Packages (0x…) are immutable.
key() {  # <src> <name>
  local r; r="$(recipe "$2")"
  set -- "$1" $r
  (cd "$1" && shift 2 && find "$@" -type f -name '*.bend' -print0 | sort -z | xargs -0 cat | shasum | cut -c1-12)
}

# build <name> of <src> into the cache if absent; print the cache file
cached() {  # <src> <name>
  local src="$1" name="$2" r k f main
  r="$(recipe "$name")"; main="${r%% *}"
  k="$(key "$src" "$name")"; f="$CACHE/$name-$k"
  if [ ! -x "$f" ]; then
    mkdir -p "$CACHE"
    say "$name: compiling $main (sb-core ~15 s, a REPL 1-2 min)..."
    local s=$SECONDS
    (cd "$src" && bend "$main" -o "$f.tmp.$$" >/dev/null) || { rm -f "$f.tmp.$$"; return 1; }
    mv "$f.tmp.$$" "$f"
    say "$name: built in $((SECONDS - s)) s ($f)"
    # keep the 12 newest of this name
    ls -t "$CACHE/$name-"* 2>/dev/null | grep -v '\.tmp\.' | tail -n +13 | while read -r old; do rm -f "$old"; done
  else
    touch "$f"
  fi
  echo "$f"
}

# <src>/<name> = the cache file (a new inode: a running binary keeps its own)
place() {  # <src> <name>
  local src="$1" name="$2" f out="$1/$2"
  if ! f="$(cached "$src" "$name")"; then
    if [ -x "$out" ]; then say "compiling $name failed: the existing $out is kept"; return 0; fi
    say "compiling $name failed"; return 1
  fi
  cmp -s "$f" "$out" && return 0
  cp "$f" "$out.tmp.$$" && mv -f "$out.tmp.$$" "$out"
}

cmd=place
case "${1:-}" in path|key) cmd="$1"; shift ;; ""|-h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;; esac
src="$REPO"
if [ "${1:-}" = --src ]; then src="$(cd "$2" && pwd)"; shift 2; fi
[ $# -gt 0 ] || { say "which binary? (repl-live repl-scripted sb-core harness-demo)"; exit 2; }
case "$cmd" in
  key) key "$src" "$1" ;;
  path) cached "$src" "$1" ;;
  place) for n in "$@"; do place "$src" "$n"; done ;;
esac
