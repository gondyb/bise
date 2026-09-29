#!/usr/bin/env bash
# versions.sh — the Switchboard versions cache.
#
# A version is an immutable app root built from one git commit (or from
# the working tree, uncommitted changes included):
#   $SB_VERSIONS_DIR/<id>/  bend-harness, repl-live, tool-desc-*.txt,
#                           prompt-*.txt, sb-core (when hub/ exists),
#                           bend-jsrt (+ a hard link at the old path
#                           rust/jsrt/target/debug/bend-jsrt),
#                           VERSION (id, commit, subject, built, bend_hash)
# id = <short commit>, or <short commit>-dirty-<hash of the changes>.
# A hub runs FROM a version dir: rebuilding the tree never changes a
# running system, only an explicit switch does.
#
#   ./versions.sh build [<rev>|--tree]   # build (cached), print the version dir
#   ./versions.sh id [<rev>|--tree]      # the id a build would get
#   ./versions.sh list                   # built versions, newest first
#
# Defaults: SB_VERSIONS_DIR=~/.local/state/switchboard/versions; the
# cargo target dir and bins.sh's cache of the Bend binaries (one per
# source hash, a Bend compile is 1-2 min) live in
# SB_BUILD_DIR=~/.local/state/switchboard/build.
# Commits are built in a temporary git worktree under /tmp (removed after).

set -euo pipefail
cd "$(dirname "$0")"
REPO="$PWD"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}/switchboard"
VERSIONS="${SB_VERSIONS_DIR:-$STATE/versions}"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
export PATH="$HOME/.cargo/bin:$HOME/.bend/bin:$PATH"

say() { echo "versions: $*" >&2; }

# what to remove when the script exits, built or failed: a RETURN trap
# does not run when set -e exits, and the /tmp/sb-build-* worktrees leaked
CLEAN_TMP="" CLEAN_WT=""
cleanup() {
  if [ -n "$CLEAN_TMP" ]; then rm -rf "$CLEAN_TMP"; fi
  if [ -n "$CLEAN_WT" ]; then
    git worktree remove --force "$CLEAN_WT" 2>/dev/null || true
    git worktree prune 2>/dev/null || true
  fi
}
trap cleanup EXIT

# id of the working tree
tree_id() {
  local head dirty
  head="$(git rev-parse --short HEAD)"
  if [ -z "$(git status --porcelain)" ]; then echo "$head"; return; fi
  dirty="$( { git diff HEAD; git ls-files --others --exclude-standard -z \
              | xargs -0 shasum 2>/dev/null; } | shasum | cut -c1-8)"
  echo "$head-dirty-$dirty"
}

rev_id() { git rev-parse --short "$1^{commit}"; }

# build the source dir $1 into version $2 (subject/commit from $3)
build_from() {
  local src="$1" id="$2" rev="$3" vdir="$VERSIONS/$2"
  mkdir -p "$BUILD/cache" "$VERSIONS"
  local tmp="$vdir.tmp.$$"
  rm -rf "$tmp"; mkdir -p "$tmp/rust/jsrt/target/debug"
  CLEAN_TMP="$tmp"

  # release: the TUI draws ~10x faster than the debug build
  say "cargo build --release $id..."
  # one cargo target dir per SOURCE: cargo decides freshness by mtime,
  # and a commit checked out in a worktree (fresh mtimes) would otherwise
  # leave artifacts newer than the tree's edited files - the next --tree
  # build would ship stale Rust code. Worktrees are always freshly checked
  # out, so they can share theirs.
  local target="$BUILD/target-commits"
  [ "$src" = "$REPO" ] && target="$BUILD/target-tree"
  (cd "$src/rust" && CARGO_TARGET_DIR="$target" cargo build -q --release -p bend-harness)
  cp "$target/release/bend-harness" "$tmp/bend-harness"

  # the Bend binaries: bins.sh's cache (one per source hash, shared with
  # run.sh and the gate; a Bend compile is 1-2 min). This repo's bins.sh
  # builds any commit, old ones included (their committed binaries, up to
  # BISE-114, are never used: a version always compiles its own sources)
  local h; h="$("$REPO/bins.sh" key --src "$src" repl-live)"
  cp "$("$REPO/bins.sh" path --src "$src" repl-live)" "$tmp/repl-live"
  # sb-core: the hub's decisions in Bend (hub/*.bend), when the version has them
  if [ -f "$src/hub/main.bend" ]; then
    cp "$("$REPO/bins.sh" path --src "$src" sb-core)" "$tmp/sb-core"
  fi
  cp "$src"/tool-desc-*.txt "$src"/prompt-*.txt "$tmp/"

  # the V8 engine: rarely changes, 100 MB - a hard link of the live tree's
  local js="$REPO/rust/jsrt/target/debug/bend-jsrt"
  # a linked git worktree: the main checkout's engine
  [ -x "$js" ] || js="$(git rev-parse --path-format=absolute --git-common-dir)/../rust/jsrt/target/debug/bend-jsrt"
  if [ ! -x "$js" ]; then
    say "bend-jsrt missing — building the V8 engine..."
    (cd "$REPO/rust/jsrt" && CARGO_TARGET_DIR=target cargo build)
  fi
  # $tmp/bend-jsrt: the harness passes it to the runtime (BEND_JSRT_BIN);
  # the same file at the old path too: a runtime before BISE-114 (an old
  # commit) runs rust/jsrt/target/debug/bend-jsrt, relative to its root
  ln -f "$js" "$tmp/bend-jsrt" 2>/dev/null || cp "$js" "$tmp/bend-jsrt"
  ln -f "$tmp/bend-jsrt" "$tmp/rust/jsrt/target/debug/bend-jsrt"

  {
    echo "id=$id"
    echo "commit=$(git rev-parse "$rev")"
    echo "subject=$(git log -1 --format=%s "$rev")"
    echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "bend_hash=$h"
    echo "repo=$REPO"
  } > "$tmp/VERSION"
  rm -rf "$vdir"; mv "$tmp" "$vdir"
  CLEAN_TMP=""
  say "version $id built: $vdir"
}

built() { [ -x "$VERSIONS/$1/bend-harness" ] && [ -x "$VERSIONS/$1/repl-live" ] && [ -f "$VERSIONS/$1/VERSION" ]; }

build() {
  local what="${1:---tree}" id
  if [ "$what" = "--tree" ]; then
    id="$(tree_id)"
    built "$id" || build_from "$REPO" "$id" HEAD
  else
    id="$(rev_id "$what")"
    if ! built "$id"; then
      local wt="/tmp/sb-build-$id-$$"
      git worktree add -q --detach "$wt" "$id"
      CLEAN_WT="$wt"
      build_from "$wt" "$id" "$id"
    fi
  fi
  echo "$VERSIONS/$id"
}

case "${1:-}" in
  build) build "${2:---tree}" ;;
  id) if [ "${2:---tree}" = "--tree" ]; then tree_id; else rev_id "$2"; fi ;;
  list)
    [ -d "$VERSIONS" ] || exit 0
    for d in $(ls -t "$VERSIONS"); do
      [ -f "$VERSIONS/$d/VERSION" ] || continue
      printf '%s\t%s\t%s\n' "$d" "$(sed -n 's/^built=//p' "$VERSIONS/$d/VERSION")" \
        "$(sed -n 's/^subject=//p' "$VERSIONS/$d/VERSION")"
    done ;;
  *) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
