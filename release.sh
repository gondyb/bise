#!/usr/bin/env bash
# Builds the distributable bundle: dist/bend-harness-<os>-<arch>/
#
# Everything the CLI needs lives in the bundle, and the parent chdirs
# to its own directory at startup - so the bundle runs from anywhere:
#   dist/bend-harness-darwin-arm64/bend-harness --continue
#
# Requirements at RUNTIME: a MISTRAL_API_KEY in the env.
# Optional: bend on the PATH unlocks the /reload self-recompile
# (without it, a reload still restarts the process and keeps the
# session - it just keeps the shipped binaries).

set -euo pipefail
cd "$(dirname "$0")"

OS=$(uname -s | tr "[:upper:]" "[:lower:]")
ARCH=$(uname -m)
DIST="dist/bend-harness-$OS-$ARCH"

echo "== release: $DIST"
mkdir -p "$DIST"

# 1. the Rust parent/TUI, release mode
echo "== cargo release (bend-harness)"
(cd rust && cargo build --release -p bend-harness)

# 2. the Bend REPLs, native binaries (self-contained, no bend needed at
#    runtime): bins.sh's cache (compiled when the sources changed)
echo "== bend native binaries"
for b in repl-live repl-scripted harness-demo; do
  cp "$(./bins.sh path "$b")" "$DIST/$b"
done

# 3. the V8 engine (bend-jsrt), release mode - skipped when already built
#    (the release link is >50MiB: build it outside any ulimit -f)
if [ ! -x rust/jsrt/target/release/bend-jsrt ]; then
  echo "== jsrt release build (long)"
  (cd rust/jsrt && CARGO_TARGET_DIR=target cargo build --release)
fi
# next to bend-harness: the harness hands its path to the runtime
# (BEND_JSRT_BIN); the old rust/jsrt/target/debug path is gone
rm -rf "$DIST/rust"
cp rust/jsrt/target/release/bend-jsrt "$DIST/bend-jsrt"

# 4. the sources (the reload recompiles them) + the text assets
echo "== sources and assets"
rm -rf "$DIST/runtime" "$DIST/core"
cp -R runtime core "$DIST/"
cp LAWS.bend PROOF.bend tool-desc-*.txt prompt-*.txt "$DIST/"

# 5. the parent binary itself, and strip everything shippable
echo "== assemble + strip"
cp rust/target/release/bend-harness "$DIST/bend-harness"
strip "$DIST/bend-harness" "$DIST/repl-live" "$DIST/repl-scripted" "$DIST/harness-demo" 2>/dev/null || true
strip -x "$DIST/bend-jsrt" 2>/dev/null || true
# a stable ad-hoc signature per artifact: EDR ML scoring is less
# suspicious of signed binaries, and the identifier is allowlistable
codesign -s - --force "$DIST/bend-harness" "$DIST/repl-live" \
  "$DIST/repl-scripted" "$DIST/harness-demo" 2>/dev/null || true

# 6. a tiny README in the bundle
cat > "$DIST/README.txt" <<EOF
bend-harness ($OS-$ARCH)

Launch from anywhere:
  ./bend-harness            new session
  ./bend-harness --continue     resume the latest session
  ./bend-harness --resume <id>  resume by session id prefix

MISTRAL_API_KEY must be set in the environment.
Sessions live in ~/.bend-harness/sessions/.
EOF

# 7. VERIFY the bundle: a killed build step (an EDR quarantine, a racing
#    concurrent build, a full disk) must FAIL LOUDLY, not ship a broken
#    bundle that dies on "REPL Bend introuvable" at the user's machine.
echo "== verify"
for f in bend-harness repl-live repl-scripted harness-demo \
          bend-jsrt runtime/repl-live.bend LAWS.bend; do
  if [ ! -e "$DIST/$f" ]; then
    echo "RELEASE INCOMPLETE: $DIST/$f is missing (build step failed or the file was removed post-build)" >&2
    exit 1
  fi
done

echo "== done:"
du -sh "$DIST"
