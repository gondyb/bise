#!/usr/bin/env bash
# build.sh — the zip the Chrome Web Store (and Edge Add-ons) take.
#
#   computer-use/store/build.sh [out-dir]      default out-dir: $TMPDIR/cu-store
#
# The store package is computer-use/extension minus what a store refuses
# or doesn't need: the dev "key" (the Store assigns its own ID and rejects
# a manifest with "key"; the host's allowed_origins lists both IDs) and
# test/. Prints the zip's path. Checks: icons present (128 px required by
# the Store), no unused permission the review would question.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
EXT="$HERE/../extension"
OUT="${1:-${TMPDIR:-/tmp}/cu-store}"
command -v jq >/dev/null || { echo "build.sh: needs jq" >&2; exit 1; }

version=$(jq -r .version "$EXT/manifest.json")
stage="$OUT/stage"
rm -rf "$stage"
mkdir -p "$stage"
(cd "$EXT" && find . -type f ! -path './test/*' ! -name '.*' -print0 | xargs -0 -I{} rsync -R {} "$stage/")
jq 'del(.key)' "$EXT/manifest.json" > "$stage/manifest.json"

warn=0
if [ "$(jq -r '.icons["128"] // empty' "$stage/manifest.json")" = "" ]; then
  echo "build.sh: no 128 px icon in manifest.json (the Store requires one; designer: icons/128.png, 48, 16)" >&2
  warn=1
fi

zip="$OUT/bise-computer-use-$version.zip"
rm -f "$zip"
(cd "$stage" && zip -q -r -X "$zip" .)
echo "$zip"
exit "$warn"
