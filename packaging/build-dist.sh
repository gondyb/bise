#!/usr/bin/env bash
# build-dist.sh — PROTOTYPE: build a self-contained, installable tarball
# of the harness + Switchboard for the current platform (tested on
# macOS arm64) from one git commit.
#
#   packaging/build-dist.sh [<rev>] [--out <dir>]
#
#   <rev>   a commit (default HEAD; uncommitted changes are NOT shipped)
#   --out   where the tarball lands (default /tmp/bise-dist)
#
# Output: <out>/bise-<id>-<os>-<arch>.tar.gz (+ .sha256), with
#   bise-<id>-<os>-<arch>/
#     install.sh           the installer (the same file curl | sh fetches)
#     app/                 the app root, exactly as versions.sh builds it:
#       bise                 Rust: TUI + Switchboard daemon + sb CLI
#       bend-harness         -> bise (the old name, kept one release)
#       sb                   -> bise (the agents' command: bise called as sb)
#       repl-live            native Bend REPL (runtime/repl-live.bend)
#       repl-scripted        native Bend REPL, no provider (runtime/repl.bend)
#       sb-core              native Bend hub core (hub/main.bend)
#       tool-desc-*.txt prompt-*.txt   read at run time (relative paths)
#       bend-jsrt            the V8 engine (RELEASE build; a commit before
#                            BISE-114: at rust/jsrt/target/debug/bend-jsrt,
#                            the path its runtime expects)
#       VERSION              id, commit, subject, built, bend_hash, macos,
#                            target
#       LICENSE NOTICE THIRD_PARTY_NOTICES   Apache-2.0 + what we ship of
#                            others (the commit's; the tree's before them)
#
# Every binary must run on the macOS target (rust/.cargo/config.toml,
# BISE-164): checked before packing (./bins.sh minos).
#
# It reuses versions.sh (its caches: cargo target dir, Bend compiles per
# source hash), so a commit already built by /version costs seconds.
# Nothing is published: the tarball stays on disk.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
# the same dirs as bise_home (rust/home): $BISE_HOME/dev, ~/.bise/dev once
# migrated, else today's ~/.local/state/switchboard (XDG_STATE_HOME unread)
if [ -n "${BISE_HOME:-}" ]; then STATE="$BISE_HOME/dev"
elif [ -e "$HOME/.bise/migrated.json" ]; then STATE="$HOME/.bise/dev"
else STATE="$HOME/.local/state/switchboard"; fi
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
name="bise-$id-$target"

# 1. the app root of this commit (cached by versions.sh)
say "versions.sh build $id..."
vdir="$("$REPO/scripts/versions.sh" build "$id")"
h="$(sed -n 's/^bend_hash=//p' "$vdir/VERSION")"

# 2. repl-scripted (versions.sh does not build it): bins.sh's cache,
#    same key as repl-live (content hash of runtime/ core/ vendor/)
scripted="$BUILD/cache/repl-scripted-$h"
if [ ! -x "$scripted" ]; then
  wt="/tmp/bise-dist-src-$id-$$"
  git worktree add -q --detach "$wt" "$commit"
  trap 'git -C "$REPO" worktree remove --force "$wt" 2>/dev/null || true' EXIT
  scripted="$("$REPO/scripts/bins.sh" path --src "$wt" repl-scripted)"
  git worktree remove --force "$wt"; trap - EXIT
fi

# 3. the V8 engine, release build: the version's own (versions.sh builds
#    it from the commit's rust/jsrt since BISE-133, VERSION has jsrt=),
#    else (an older version dir: its engine is the debug one) the tree's,
#    from bins.sh's cache: warn when the commit's rust/jsrt differs.
if grep -q '^jsrt=release' "$vdir/VERSION"; then
  js="$vdir/bend-jsrt"
else
  js="$("$REPO/scripts/bins.sh" path bend-jsrt)"
  if ! git diff --quiet "$commit" -- rust/jsrt/src rust/jsrt/Cargo.toml rust/jsrt/Cargo.lock; then
    say "WARNING: rust/jsrt differs between $id and the working tree; the shipped engine is the tree's"
  fi
fi

# 4. stage
stage="$(mktemp -d /tmp/bise-stage.XXXXXX)"
trap 'rm -rf "$stage"' EXIT
app="$stage/$name/app"
# where this commit's runtime looks for the engine: BEND_JSRT_BIN (set by
# the harness to app/bend-jsrt) since BISE-114, a fixed path before
jsrt_at=bend-jsrt
{ git show "$commit:bend/runtime/main.bend" 2>/dev/null || git show "$commit:runtime/main.bend"; } \
  | grep -c BEND_JSRT_BIN >/dev/null || jsrt_at=rust/jsrt/target/debug/bend-jsrt
mkdir -p "$app/$(dirname "$jsrt_at")"
# the command (BISE-165): bise, or the bend-harness of a version dir
# built before the rename; the old name stays as a link one release
if [ -e "$vdir/bise" ]; then cp "$vdir/bise" "$app/bise"; else cp "$vdir/bend-harness" "$app/bise"; fi
ln -s bise "$app/bend-harness"
# sb: the agents' command, the same binary called by that name
ln -s bise "$app/sb"
for f in repl-live sb-core; do cp "$vdir/$f" "$app/$f"; done
cp "$scripted" "$app/repl-scripted"
# the prompts: prompts/ since the root cleanup, at the top of an older version
if [ -d "$vdir/prompts" ]; then cp -R "$vdir/prompts" "$app/prompts"; pr=prompts/
else cp "$vdir"/tool-desc-*.txt "$vdir"/prompt-*.txt "$app/"; pr=""; fi
cp "$js" "$app/$jsrt_at"
# VERSION: the dev repo path means nothing on the user's machine (the
# daemon would look for versions.sh there); the target says what it runs on
grep -v '^repo=' "$vdir/VERSION" > "$app/VERSION"
echo "target=$target" >> "$app/VERSION"
echo "channel=${BISE_CHANNEL:-dev}" >> "$app/VERSION"
# the license and the third-party notices (Apache-2.0 4(a)/(d)): the
# commit's; a commit before them gets the working tree's
for f in LICENSE NOTICE THIRD_PARTY_NOTICES; do
  git show "$commit:$f" > "$app/$f" 2>/dev/null || cp "$REPO/$f" "$app/$f"
done
cp "$HERE/install.sh" "$stage/$name/install.sh"
chmod +x "$stage/$name/install.sh"

# 5. sign, NEVER strip. The endpoint security agent (CrowdStrike) on the
#    dev Macs deleted a stripped sb-core within a second of its creation
#    (ad-hoc signed or not); unstripped + ad-hoc signed binaries are left
#    alone. Stripping would save ~25 MB (mostly V8): only reconsider it
#    with Developer ID signing + notarization (docs/packaging.md §6).
#    Ad-hoc (-) by default; BISE_SIGN_ID=<Developer ID> adds the hardened
#    runtime + timestamp (entitlements still to add: JIT for bend-jsrt,
#    audio-input for bise).
if [ "$os" = darwin ]; then
  for b in bise repl-live repl-scripted sb-core "$jsrt_at"; do
    codesign -s "${BISE_SIGN_ID:--}" --force ${BISE_SIGN_ID:+--options runtime --timestamp} "$app/$b" 2>/dev/null
  done
fi

# 5b. computer use's helper (macOS, docs/computer-use-design.md §5.2):
#     app/bise Computer Use.app from the commit's computer-use/macos-app.
#     Its bundle.sh signs it (BISE_SIGN_ID: Developer ID, hardened runtime,
#     timestamp; else ad-hoc) and, with BISE_NOTARY_PROFILE (a notarytool
#     keychain profile), notarizes and staples it. Cached by the folder's
#     tree hash and the signing identity.
helper=""
if [ "$os" = darwin ] && git cat-file -e "$commit:computer-use/macos-app/Package.swift" 2>/dev/null; then
  hkey="$(git rev-parse "$commit:computer-use/macos-app")-$(printf %s "${BISE_SIGN_ID:-adhoc}${BISE_NOTARY_PROFILE:+-notarized}" | shasum | cut -c1-8)"
  hcache="$BUILD/cache/cu-helper-$hkey"
  if [ ! -d "$hcache/bise Computer Use.app" ]; then
    say "building the computer-use helper ($hkey)..."
    hsrc="$(mktemp -d /tmp/bise-cu-helper.XXXXXX)"
    git archive "$commit" computer-use/macos-app | tar -x -C "$hsrc"
    rm -rf "$hcache.tmp" && mkdir -p "$hcache.tmp"
    "$hsrc/computer-use/macos-app/scripts/bundle.sh" --out "$hcache.tmp" >/dev/null \\n      || { rm -rf "$hsrc" "$hcache.tmp"; say "the computer-use helper did not build"; exit 1; }
    rm -rf "$hsrc" "$hcache" && mv "$hcache.tmp" "$hcache"
  fi
  helper="$app/bise Computer Use.app"
  ditto "$hcache/bise Computer Use.app" "$helper"
fi

# 6. verify: a missing piece must fail here, not on the user's machine
#    (wait a moment: a quarantine by security software is not instant)
sleep 2
for f in bise bend-harness sb repl-live repl-scripted sb-core "$jsrt_at" \
         "${pr}tool-desc-bash.txt" "${pr}prompt-tool-use.txt" "${pr}prompt-tone.txt" VERSION \
         LICENSE NOTICE THIRD_PARTY_NOTICES; do
  [ -e "$app/$f" ] || { say "INCOMPLETE: app/$f missing"; exit 1; }
done
# ... and no binary needs a macOS newer than the target (a version dir
# built before BISE-164 has Bend binaries for the build machine's macOS:
# remove it and run again)
"$REPO/scripts/bins.sh" minos "$app/bise" "$app/repl-live" "$app/repl-scripted" \
  "$app/sb-core" "$app/$jsrt_at" >&2 \
  || { say "a binary needs a newer macOS than $("$REPO/scripts/bins.sh" macos-target): rm -rf $vdir and run again"; exit 1; }
# ... and the helper is whole: signature, macOS target
if [ -n "$helper" ]; then
  codesign --verify --strict "$helper" || { say "INCOMPLETE: $helper: bad signature"; exit 1; }
  "$REPO/scripts/bins.sh" minos "$helper/Contents/MacOS/bise-computer-use" >&2 \\n    || { say "the computer-use helper needs a newer macOS than the target"; exit 1; }
fi

# 7. pack
mkdir -p "$out"
tarball="$out/$name.tar.gz"
tar -C "$stage" -czf "$tarball.tmp" "$name"
mv "$tarball.tmp" "$tarball"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
say "app root: $(du -sh "$app" | cut -f1), tarball: $(du -h "$tarball" | cut -f1)"
echo "$tarball"
