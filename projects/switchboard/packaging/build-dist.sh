#!/usr/bin/env bash
# build-dist.sh — PROTOTYPE: build a self-contained, installable tarball
# of the harness + Switchboard for the current platform (tested on
# macOS arm64) from one git commit.
#
#   projects/switchboard/packaging/build-dist.sh [<rev>] [--out <dir>]
#
#   <rev>   a commit (default HEAD; uncommitted changes are NOT shipped)
#   --out   where the tarball lands (default /tmp/bise-dist)
#
# Output: <out>/bend-harness-<id>-<os>-<arch>.tar.gz (+ .sha256), with
#   bend-harness-<id>-<os>-<arch>/
#     install.sh           the installer (the same file curl | sh fetches)
#     app/                 the app root, exactly as versions.sh builds it:
#       bend-harness         Rust: TUI + Switchboard daemon + sb CLI
#       repl-live            native Bend REPL (runtime/repl-live.bend)
#       repl-scripted        native Bend REPL, no provider (runtime/repl.bend)
#       sb-core              native Bend hub core (hub/main.bend)
#       tool-desc-*.txt prompt-*.txt   read at run time (relative paths)
#       rust/jsrt/target/debug/bend-jsrt   the V8 engine (RELEASE build,
#                            at the debug path runtime/main.bend expects)
#       VERSION              id, commit, subject, built, bend_hash, target
#
# It reuses versions.sh (its caches: cargo target dir, Bend compiles per
# source hash), so a commit already built by /version costs seconds.
# Nothing is published: the tarball stays on disk.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}/switchboard"
BUILD="${SB_BUILD_DIR:-$STATE/build}"
export PATH="$HOME/.cargo/bin:$HOME/.bend/bin:$PATH"

rev=HEAD
out=/tmp/bise-dist
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) rev="$1" ;;
  esac
  shift
done

say() { echo "build-dist: $*" >&2; }

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

cd "$REPO"
commit="$(git rev-parse "$rev^{commit}")"
id="$(git rev-parse --short "$commit")"
name="bend-harness-$id-$target"

# 1. the app root of this commit (cached by versions.sh)
say "versions.sh build $id..."
vdir="$("$REPO/versions.sh" build "$id")"
h="$(sed -n 's/^bend_hash=//p' "$vdir/VERSION")"

# 2. repl-scripted (versions.sh does not build it): same Bend sources,
#    same cache key scheme (content hash of runtime/ core/ vendor/)
scripted="$BUILD/cache/repl-scripted-$h"
if [ ! -x "$scripted" ]; then
  wt="/tmp/bise-dist-src-$id-$$"
  git worktree add -q --detach "$wt" "$commit"
  trap 'git -C "$REPO" worktree remove --force "$wt" 2>/dev/null || true' EXIT
  say "bend runtime/repl.bend (1-2 min)..."
  (cd "$wt" && bend runtime/repl.bend -o "$scripted.tmp" >/dev/null)
  mv "$scripted.tmp" "$scripted"
  git worktree remove --force "$wt"; trap - EXIT
fi

# 3. the V8 engine, release build. It is NOT versioned yet (versions.sh
#    hard-links the live tree's debug build): warn when the commit's
#    rust/jsrt differs from the tree the engine was built from.
js="$REPO/rust/jsrt/target/release/bend-jsrt"
if [ ! -x "$js" ]; then
  say "bend-jsrt release missing — cargo build --release (long, once)..."
  (cd "$REPO/rust/jsrt" && cargo build --release)
fi
if ! git diff --quiet "$commit" -- rust/jsrt/src rust/jsrt/Cargo.toml rust/jsrt/Cargo.lock; then
  say "WARNING: rust/jsrt differs between $id and the working tree; the shipped engine is the tree's"
fi

# 4. stage
stage="$(mktemp -d /tmp/bise-stage.XXXXXX)"
trap 'rm -rf "$stage"' EXIT
app="$stage/$name/app"
mkdir -p "$app/rust/jsrt/target/debug"
for f in bend-harness repl-live sb-core; do cp "$vdir/$f" "$app/$f"; done
cp "$scripted" "$app/repl-scripted"
cp "$vdir"/tool-desc-*.txt "$vdir"/prompt-*.txt "$app/"
cp "$js" "$app/rust/jsrt/target/debug/bend-jsrt"
# VERSION: the dev repo path means nothing on the user's machine (the
# daemon would look for versions.sh there); the target says what it runs on
grep -v '^repo=' "$vdir/VERSION" > "$app/VERSION"
echo "target=$target" >> "$app/VERSION"
echo "channel=${BISE_CHANNEL:-dev}" >> "$app/VERSION"
cp "$HERE/install.sh" "$stage/$name/install.sh"
chmod +x "$stage/$name/install.sh"

# 5. sign, NEVER strip. The endpoint security agent (CrowdStrike) on the
#    dev Macs deleted a stripped sb-core within a second of its creation
#    (ad-hoc signed or not); unstripped + ad-hoc signed binaries are left
#    alone. Stripping would save ~25 MB (mostly V8): only reconsider it
#    with Developer ID signing + notarization (docs/packaging.md §6).
#    Ad-hoc (-) by default; BISE_SIGN_ID=<Developer ID> adds the hardened
#    runtime + timestamp (entitlements still to add: JIT for bend-jsrt,
#    audio-input for bend-harness).
if [ "$os" = darwin ]; then
  for b in bend-harness repl-live repl-scripted sb-core rust/jsrt/target/debug/bend-jsrt; do
    codesign -s "${BISE_SIGN_ID:--}" --force ${BISE_SIGN_ID:+--options runtime --timestamp} "$app/$b" 2>/dev/null
  done
fi

# 6. verify: a missing piece must fail here, not on the user's machine
#    (wait a moment: a quarantine by security software is not instant)
sleep 2
for f in bend-harness repl-live repl-scripted sb-core rust/jsrt/target/debug/bend-jsrt \
         tool-desc-bash.txt prompt-tool-use.txt VERSION; do
  [ -e "$app/$f" ] || { say "INCOMPLETE: app/$f missing"; exit 1; }
done

# 7. pack
mkdir -p "$out"
tarball="$out/$name.tar.gz"
tar -C "$stage" -czf "$tarball.tmp" "$name"
mv "$tarball.tmp" "$tarball"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
say "app root: $(du -sh "$app" | cut -f1), tarball: $(du -h "$tarball" | cut -f1)"
echo "$tarball"
