#!/usr/bin/env bash
# bundle.sh — make "bise Computer Use.app" (dev.bise.computer-use,
# LSUIElement, macOS 14+) from this Swift package.
#
#   scripts/bundle.sh [--out <dir>] [--debug] [--testapp]
#
#   --out      where the .app lands (default: <package>/build)
#   --debug    a debug build (default: release)
#   --testapp  also bundle the tests' app ("cu test.app", test.bise.cu-testapp;
#              not dev.bise.*: the helper refuses bise's own apps)
#   --devhost  dev only, instead of the helper: "bise Computer Use dev.app"
#              (dev.bise.computer-use.dev) in <out>, made once and never
#              rebuilt, + the helper's code as libCUHelperDylib.dylib next
#              to it (replaced each time). Grant it Accessibility / Screen
#              Recording once; the grants hold across rebuilds.
#
# Signing (docs/packaging.md §6): ad-hoc by default; BISE_SIGN_ID=<Developer
# ID Application identity> signs with the hardened runtime and a secure
# timestamp. BISE_NOTARY_PROFILE=<notarytool keychain profile> (with
# BISE_SIGN_ID) then notarizes and staples the .app. Never commit a
# certificate or a credential: both come from the keychain.
#
# An ad-hoc signature changes with every build, and TCC keys an ad-hoc app
# on it: after a rebuild, macOS asks for Accessibility / Screen Recording
# again. A Developer ID signature keeps the grants across versions.
# Prints the .app's path.
set -euo pipefail
PKG="$(cd "$(dirname "$0")/.." && pwd)"
out="$PKG/build"
config=release
testapp=0
devhost=0
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --debug) config=debug ;;
    --testapp) testapp=1 ;;
    --devhost) devhost=1 ;;
    -h|--help) sed -n '2,27p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "bundle.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done
say() { echo "bundle.sh: $*" >&2; }

version="$(sed -n 's/^let helperVersion = "\(.*\)"/\1/p' "$PKG/Sources/CUHelper/Engine.swift")"
products=(BiseComputerUse)
[ "$devhost" = 1 ] && products=(CUHelperDylib cu-devhost)
[ "$testapp" = 1 ] && products+=(cu-testapp)
for p in "${products[@]}"; do
  (cd "$PKG" && swift build -c "$config" --product "$p" >&2)
done
bin="$(cd "$PKG" && swift build -c "$config" --show-bin-path)"

sign() { # <bundle>
  if [ -n "${BISE_SIGN_ID:-}" ]; then
    codesign --force --options runtime --timestamp -s "$BISE_SIGN_ID" "$1"
  else
    codesign --force -s - "$1"
  fi
  codesign --verify --strict "$1"
}

# <name> <bundle id> <executable> <exe name> <LSUIElement> <icon: 1|0>
make_app() {
  local app="$out/$1.app"
  rm -rf "$app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$bin/$3" "$app/Contents/MacOS/$4"
  local icon=""
  if [ "$6" = 1 ]; then
    local set; set="$(mktemp -d "${TMPDIR:-/tmp}/cu-icon.XXXXXX")/AppIcon.iconset"
    "$bin/$3" --write-iconset "$set"
    iconutil -c icns -o "$app/Contents/Resources/AppIcon.icns" "$set"
    rm -rf "$(dirname "$set")"
    icon="<key>CFBundleIconFile</key><string>AppIcon</string>"
  fi
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$1</string>
  <key>CFBundleDisplayName</key><string>$1</string>
  <key>CFBundleIdentifier</key><string>$2</string>
  <key>CFBundleExecutable</key><string>$4</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><$5/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAccessibilityUsageDescription</key><string>bise agents read and use the apps you allow, without taking your cursor.</string>
  <key>NSScreenCaptureUsageDescription</key><string>bise agents see the one window they work in, never your whole screen.</string>
  $icon
</dict>
</plist>
EOF
  sign "$app"
  echo "$app"
}

mkdir -p "$out"
[ "$testapp" = 1 ] && make_app "cu test" test.bise.cu-testapp cu-testapp cu-testapp false 0 >/dev/null
if [ "$devhost" = 1 ]; then
  host="$out/bise Computer Use dev.app"
  # made once: a new host binary = a new code hash = the grants are lost
  [ -d "$host" ] || make_app "bise Computer Use dev" dev.bise.computer-use.dev cu-devhost cu-devhost true 0 >/dev/null
  cp "$bin/libCUHelperDylib.dylib" "$out/libCUHelperDylib.dylib.new"
  mv -f "$out/libCUHelperDylib.dylib.new" "$out/libCUHelperDylib.dylib"
  say "$host (dev host) + libCUHelperDylib.dylib ($version, $config)"
  echo "$host"
  exit 0
fi
helper="$(make_app "bise Computer Use" dev.bise.computer-use BiseComputerUse bise-computer-use true 1)"

if [ -n "${BISE_NOTARY_PROFILE:-}" ]; then
  [ -n "${BISE_SIGN_ID:-}" ] || { say "BISE_NOTARY_PROFILE needs BISE_SIGN_ID (a Developer ID)"; exit 1; }
  zip="$(mktemp -d "${TMPDIR:-/tmp}/cu-notary.XXXXXX")/helper.zip"
  ditto -c -k --keepParent "$helper" "$zip"
  say "notarizing (notarytool, profile $BISE_NOTARY_PROFILE)..."
  xcrun notarytool submit "$zip" --keychain-profile "$BISE_NOTARY_PROFILE" --wait >&2
  xcrun stapler staple "$helper" >&2
  rm -rf "$(dirname "$zip")"
fi
say "$helper ($version, $config, $( [ -n "${BISE_SIGN_ID:-}" ] && echo "Developer ID" || echo ad-hoc ))"
echo "$helper"
