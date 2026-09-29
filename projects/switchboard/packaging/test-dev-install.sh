#!/usr/bin/env bash
# test-dev-install.sh — the dev channel (install.sh --dev, BISE-129) in a
# CLEAN fake HOME: the real ~/.bise, ~/.local/state/switchboard and live
# hubs are never touched.
#
#   projects/switchboard/packaging/test-dev-install.sh <version dir> [<version dir>]
#
# Two built versions (versions.sh build: dirs of ~/.bise/dev/versions),
# cloned into the fake HOME; a second one defaults to the first. Checks:
# the launcher follows the dev repo's hub (no hub: the newest version;
# then hub.root; then versions.json 'current', as a restart writes it),
# `bise` in the repo attaches to its running hub (no second hub), `bise`
# in a non-git folder with spaces opens a hub there, --version, doctor's
# PATH line, uninstall. Needs tmux (the TUI needs a terminal).

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
V1="$(cd "${1:?usage: test-dev-install.sh <version dir> [<version dir>]}" && pwd -P)"
V2="$(cd "${2:-$1}" && pwd -P)"
T=/tmp/bd-home
REPO=/tmp/bd-repo
WS="/tmp/bd ws/my proj"
TM="tmux -L bd-test"

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }

# a clean environment: fake HOME, system PATH + the bin dir
E() { env -i HOME="$T" PATH="$T/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin" SHELL=/bin/zsh \
        TERM=xterm-256color USER="${USER:-me}" LANG=en_US.UTF-8 "$@"; }
sbd_pids() { ps -axo pid=,command= | grep -F " sbd --workspace $1" | grep -v grep | awk '{print $1}'; }
cleanup() {
  $TM kill-server 2>/dev/null
  for p in $(ps -axo pid=,command= | grep -F -e "$T/" -e "sbd --workspace $REPO" -e "sbd --workspace $WS" | grep -v grep | awk '{print $1}'); do
    kill "$p" 2>/dev/null
  done
}
cleanup
rm -rf "$T" "$REPO" "/tmp/bd ws"
mkdir -p "$T/.bise/dev/versions" "$WS" "$REPO"
echo '{"version":1}' > "$T/.bise/migrated.json"   # the ~/.bise layout
# APFS clones: 0 bytes, ~1 s
cp -cR "$V1" "$T/.bise/dev/versions/v1"
if [ "$V2" != "$V1" ]; then cp -cR "$V2" "$T/.bise/dev/versions/v2"; else cp -cR "$V1" "$T/.bise/dev/versions/v2"; fi
touch "$T/.bise/dev/versions/v2"   # the newest
R1="$(cd "$T/.bise/dev/versions/v1" && pwd -P)"; R2="$(cd "$T/.bise/dev/versions/v2" && pwd -P)"
(cd "$REPO" && git init -q && mkdir scripts && printf '#!/bin/sh\n' > scripts/versions.sh && chmod +x scripts/versions.sh)
PREPO="$(cd "$REPO" && pwd -P)"   # /tmp is /private/tmp
trap cleanup EXIT

echo "== install --dev"
if E sh "$HERE/install.sh" --dev --repo "$REPO" --bin-dir "$T/.local/bin" 2>"$T/install.err"; then
  ok "install.sh --dev"
else
  ko "install.sh --dev"; cat "$T/install.err"; exit 1
fi
L="$T/.bise/dev/bin/bise"
[ "$(readlink "$T/.local/bin/bise")" = "$L" ] && ok "~/.local/bin/bise -> the dev launcher" || ko "link: $(readlink "$T/.local/bin/bise")"
[ "$(E bash -c 'command -v bise')" = "$T/.local/bin/bise" ] && ok "bise on PATH" || ko "bise not on PATH"
[ "$(E bise --launcher-root)" = "$R2" ] && ok "no hub yet: the newest version" || ko "no hub: $(E bise --launcher-root)"
E bise --version | grep -qF "dev channel: the version the hub of $PREPO runs" && ok "--version names the channel" || ko "--version: $(E bise --version 2>&1)"

echo "== the repo's hub"
HUB="$(sed -n "s/^HUB='\(.*\)'$/\1/p" "$L")"
state="$T/.bise/hubs/$HUB"
(cd "$REPO" && E BISE_DEV_VERSION=v1 "$T/.local/bin/bise" sbd --workspace "$REPO" </dev/null >/dev/null 2>"$T/hub.err" &)
i=0; while [ ! -S "$state/hub.sock" ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
if [ -S "$state/hub.sock" ]; then ok "hub of the repo started ($state)"; else ko "hub did not start in $state"; tail -5 "$T/hub.err"; exit 1; fi
[ "$(cat "$state/hub.root")" = "$R1" ] && ok "it runs v1 (BISE_DEV_VERSION)" || ko "hub.root: $(cat "$state/hub.root")"
[ "$(E bise --launcher-root)" = "$R1" ] && ok "the launcher follows hub.root" || ko "hub.root not followed: $(E bise --launcher-root)"
printf '{"current":"%s","good":"%s"}' "$R2" "$R1" > "$state/versions.json"
[ "$(E bise --launcher-root)" = "$R2" ] && ok "a restart (versions.json current) moves bise" || ko "current not followed: $(E bise --launcher-root)"
printf '{"current":"%s","good":"%s"}' "$R1" "$R1" > "$state/versions.json"
[ "$(cd "$REPO" && E bise switchboard --state-dir)" = "$state" ] && ok "bise in the repo names its hub's folder" || ko "state dir: $(cd "$REPO" && E bise switchboard --state-dir)"

echo "== bise in the repo attaches"
pid="$(cat "$state/hub.pid")"
$TM new-session -d -s repo -x 120 -y 40 -c "$REPO" "env -i HOME='$T' PATH='$T/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin' TERM=xterm-256color LANG=en_US.UTF-8 bise; sleep 30"
i=0; while [ $i -lt 100 ]; do
  scr="$($TM capture-pane -p -t repo 2>/dev/null)"
  echo "$scr" | grep -q -i -e main -e 'api key' -e bise && break
  sleep 0.1; i=$((i + 1))
done
sleep 1
n="$(sbd_pids "$REPO" | wc -l | tr -d ' ')"
[ "$n" = 1 ] && [ "$(cat "$state/hub.pid")" = "$pid" ] && kill -0 "$pid" && ok "one hub, the same (pid $pid)" || ko "hubs for the repo: $n, pid $(cat "$state/hub.pid") (was $pid)"
$TM capture-pane -p -t repo | grep -q -i -e 'hub did not start' -e 'not found' && ko "TUI error: $($TM capture-pane -p -t repo | head -5)" || ok "TUI up: $($TM capture-pane -p -t repo | grep -v '^ *$' | head -1 | cut -c1-60)"

echo "== bise in a non-git folder with spaces"
$TM new-session -d -s ws -x 120 -y 40 -c "$WS" "env -i HOME='$T' PATH='$T/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin' TERM=xterm-256color LANG=en_US.UTF-8 bise; sleep 30"
ws_state=""
i=0; while [ $i -lt 150 ]; do
  ws_state="$(ls -d "$T"/.bise/hubs/my-proj-* 2>/dev/null | head -n 1)"
  [ -n "$ws_state" ] && [ -S "$ws_state/hub.sock" ] && break
  sleep 0.1; i=$((i + 1))
done
if [ -n "$ws_state" ] && [ -S "$ws_state/hub.sock" ]; then ok "a hub for '$WS' ($ws_state)"; else ko "no hub for '$WS'"; $TM capture-pane -p -t ws | head -8; fi
[ "$(cat "$ws_state/hub.root" 2>/dev/null)" = "$R1" ] && ok "it runs the repo hub's current version" || ko "hub.root: $(cat "$ws_state/hub.root" 2>/dev/null)"
[ "$(sbd_pids "$(cd "$WS" && pwd -P)" | wc -l | tr -d ' ')" = 1 ] && ok "one hub for it" || ko "hubs for '$WS': $(sbd_pids "$(cd "$WS" && pwd -P)" | wc -l)"

echo "== doctor"
if [ -x "$R1/bise" ] && "$R1/bise" help 2>&1 | grep -q doctor; then
  out="$(cd "$REPO" && E bise doctor 2>&1)"
  echo "$out" | grep -q '^✓ PATH .*launcher that runs this bise' && ok "doctor: PATH is the launcher" || ko "doctor PATH: $(echo "$out" | grep PATH)"
  echo "$out" | grep -qF "running for $PREPO" && ok "doctor: the repo's hub is running" || ko "doctor hubs: $(echo "$out" | grep hubs)"
else
  echo "  skip doctor (v1 has none)"
fi

echo "== stop, uninstall"
$TM kill-server 2>/dev/null
(cd "$REPO" && E bise switchboard --stop >/dev/null 2>&1)
(cd "$WS" && E bise switchboard --stop >/dev/null 2>&1)
sleep 0.5
[ -z "$(sbd_pids "$REPO")" ] && ok "hubs stopped" || ko "a hub still runs"
E sh "$HERE/install.sh" --uninstall --dev --bin-dir "$T/.local/bin" 2>/dev/null
[ ! -e "$T/.local/bin/bise" ] && [ ! -e "$L" ] && ok "uninstall --dev: link and launcher gone" || ko "uninstall left files"
[ -d "$R1" ] && ok "versions kept" || ko "versions removed"

echo "dev channel: $pass ok, $fail failed"
[ "$fail" = 0 ]
