#!/usr/bin/env bash
# test-install.sh — install a tarball from build-dist.sh into a CLEAN
# fake HOME and check it runs. Everything happens under /tmp: the real
# ~/.bend-harness, ~/.local/state/switchboard and live hubs are never touched.
#
#   projects/switchboard/packaging/test-install.sh <tarball>
#
# Checks: install (from the extracted bundle, like curl | sh would),
# the command on PATH in a new login shell, --version, init (key file),
# a single-agent session start (--headless, scripted and live), the
# Switchboard hub start / sb list / stop on a throwaway git workspace,
# a reinstall (idempotent), then uninstall (data kept).

set -uo pipefail
tarball="${1:?usage: test-install.sh <tarball>}"
tarball="$(cd "$(dirname "$tarball")" && pwd)/$(basename "$tarball")"
T=/tmp/pk-home
WS=/tmp/pk-ws
DL=/tmp/pk-dl
WORK=/tmp/pk-work

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }

# a clean environment: fake HOME, system PATH only, zsh as the shell
E() { env -i HOME="$T" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh TERM=dumb \
        USER="${USER:-me}" LANG=en_US.UTF-8 "$@"; }

# leftovers of an earlier run (a hub of the fake HOME only)
for p in $(ps -axo pid=,command= | grep -F "$T/.local/share/" | grep -v grep | awk '{print $1}'); do kill "$p" 2>/dev/null; done
rm -rf "$T" "$WS" "$DL" "$WORK"; mkdir -p "$T" "$DL" "$WORK"

echo "== install"
tar -C "$DL" -xzf "$tarball"
bundle="$(ls -d "$DL"/*/)"
E sh "$bundle/install.sh" 2>&1 | sed 's/^/     /'
BIN="$T/.local/bin/bend-harness"
check "launcher linked in ~/.local/bin" test -x "$BIN"
check "PATH line in ~/.zshrc" grep -q 'added by the bend-harness installer' "$T/.zshrc"
found="$(E /bin/zsh -ic 'command -v bend-harness' 2>/dev/null | tail -n 1)"
[ "$found" = "$BIN" ] && ok "new shell finds bend-harness ($found)" || ko "new shell finds bend-harness (got '$found')"

echo "== --version"
v="$(cd "$WORK" && E /bin/zsh -ic 'bend-harness --version' 2>&1 | tail -n 1)"
echo "     $v"
case "$v" in "bend-harness "*darwin-arm64*) ok "--version" ;; *) ko "--version" ;; esac

echo "== init (non-interactive)"
(cd "$WORK" && E MISTRAL_API_KEY=test-key-not-real "$BIN" init) 2>&1 | sed 's/^/     /'
check "~/.bend-harness/.env holds the key" grep -q '^MISTRAL_API_KEY=test-key-not-real' "$T/.bend-harness/.env"
[ "$(stat -f %Lp "$T/.bend-harness/.env")" = 600 ] && ok ".env is mode 600" || ko ".env is mode 600"
check "config.toml sets a public model" grep -q '^model = "mistral' "$T/.bend-harness/config.toml"

# a headless session: READY on stdout, then close stdin to end it
session() {
  local label="$1"; shift
  local d; d="$(mktemp -d /tmp/pk-sess.XXXXXX)"
  mkfifo "$d/in"
  (cd "$WORK" && E "$BIN" --headless "$@" < "$d/in" > "$d/out" 2> "$d/err") &
  local pid=$!
  exec 3> "$d/in"
  local i=0
  while [ $i -lt 300 ] && ! grep -q '^READY' "$d/out" 2>/dev/null; do
    kill -0 $pid 2>/dev/null || break; sleep 0.1; i=$((i + 1))
  done
  if grep -q '^READY' "$d/out"; then
    ok "$label: $(grep '^READY' "$d/out" | cut -c1-90)…"
  else
    ko "$label: no READY"; sed 's/^/     /' "$d/err" | tail -n 5
  fi
  exec 3>&-
  i=0; while kill -0 $pid 2>/dev/null && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
  kill -0 $pid 2>/dev/null && { ko "$label: still running after stdin closed"; kill $pid; } || ok "$label: exits when stdin closes"
  rm -rf "$d"
}
echo "== single-agent session"
session "scripted session" --scripted
session "live session (model from config.toml)" 
check "sessions dir created in ~/.bend-harness" test -d "$T/.bend-harness/sessions"

echo "== Switchboard hub on a throwaway workspace"
mkdir -p "$WS" && (cd "$WS" && git init -q && echo x > README.md && git add README.md \
  && git -c user.name=t -c user.email=t@t commit -qm init)
(cd "$WS" && E "$BIN" sbd --workspace "$WS" </dev/null >/dev/null 2>"$DL/hub.err" &)
state=""; i=0
while [ $i -lt 150 ]; do
  state="$(ls -d "$T"/.local/state/switchboard/pk-ws-* 2>/dev/null | head -n 1)"
  [ -n "$state" ] && [ -S "$state/hub.sock" ] && break
  sleep 0.1; i=$((i + 1))
done
if [ -n "$state" ] && [ -S "$state/hub.sock" ]; then
  ok "hub started (state $state)"
  root="$(cat "$state/hub.root" 2>/dev/null)"
  case "$root" in "$(cd "$T" && pwd -P)/.local/share/bend-harness/versions/"*) ok "hub runs from the installed version ($root)" ;; *) ko "hub root: $root" ;; esac
  sleep 2
  out="$(E SB_SOCKET="$state/hub.sock" SB_AGENT=main "$state/bin/sb" list 2>&1)"
  echo "$out" | sed 's/^/     /' | head -n 5
  echo "$out" | grep -q main && ok "sb list (through the agents' shim) answers" || ko "sb list"
  ps -axo command= | grep -F "$root/repl-live" | grep -v grep >/dev/null && ok "main agent's repl-live runs from the version dir" || ko "main agent's repl-live not running"
  (cd "$WS" && E "$BIN" switchboard --stop --workspace "$WS") 2>&1 | sed 's/^/     /'
  i=0; while [ -e "$state/hub.sock" ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
  pid="$(cat "$state/hub.pid" 2>/dev/null)"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then ko "hub still running after --stop"; else ok "hub stopped"; fi
  ps -axo command= | grep -F "$root/repl-live" | grep -v grep >/dev/null && ko "agent REPLs left behind" || ok "no agent REPL left"
else
  ko "hub did not start"; tail -n 5 "$DL/hub.err"
fi

echo "== reinstall (same version: idempotent)"
E sh "$bundle/install.sh" 2>&1 | sed 's/^/     /'
[ "$(grep -c 'added by the bend-harness installer' "$T/.zshrc")" = 1 ] && ok "one PATH line only" || ko "PATH line duplicated"

echo "== uninstall"
E "$BIN" uninstall 2>&1 | sed 's/^/     /'
check "prefix removed" test ! -e "$T/.local/share/bend-harness"
check "command link removed" test ! -e "$T/.local/bin/bend-harness"
check "PATH line removed" sh -c "! grep -q 'bend-harness installer' '$T/.zshrc'"
check "user data kept (~/.bend-harness/.env)" test -f "$T/.bend-harness/.env"

echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
