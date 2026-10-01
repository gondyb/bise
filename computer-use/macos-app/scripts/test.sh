#!/usr/bin/env bash
# test.sh — the helper's checks.
#
#   scripts/test.sh            cu-check (pure logic) + the live run below
#   scripts/test.sh unit       cu-check only (no permission needed)
#
# The live run drives a helper over C5 with cu-client, like the broker:
# the dev host ("bise Computer Use dev.app" in $CU_DEV_DIR, default
# ~/.bise/cache/cu-apps-dev, made by `bundle.sh --devhost`; it needs
# Accessibility + Screen Recording, granted once). Targets: the tests' own
# app (cu test.app), TextEdit and Calculator, all started with open -g
# (never brought to the front; TextEdit and Calculator hidden, -j), and
# quit at the end when the test started them. Checked after every step:
# the front app and the real cursor did not change.
set -uo pipefail
PKG="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-all}"
cd "$PKG"
swift build --product cu-check >/dev/null && swift build --product cu-client >/dev/null || exit 1
bin="$(swift build --show-bin-path)"
"$bin/cu-check" || exit 1
[ "$mode" = unit ] && exit 0

DEV="${CU_DEV_DIR:-$HOME/.bise/cache/cu-apps-dev}"
scripts/bundle.sh --debug --devhost --testapp --out "$DEV" >/dev/null || exit 1
mkdir -p "$HOME/.bise/gate"
SOCK="$HOME/.bise/gate/cua-test-$$.sock"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/cu-apps-test.XXXXXX")"
started=()
cleanup() {
  pkill -f "cu-devhost --socket $SOCK" 2>/dev/null
  pkill -f "cu test.app/Contents/MacOS/cu-testapp" 2>/dev/null
  for app in "${started[@]:-}"; do [ -n "$app" ] && osascript -e "tell application id \"$app\" to quit saving no" >/dev/null 2>&1; done
  rm -rf "$WORK" "$SOCK"
}
trap cleanup EXIT

fails=0; n=0
ok() { n=$((n + 1)); echo "ok   $1"; }
ko() { n=$((n + 1)); fails=$((fails + 1)); echo "FAIL $1"; [ -n "${2:-}" ] && echo "     $2" | cut -c1-600; }

probe0="$("$bin/cu-client" --probe)"
# --no-takeover: your own clicks meanwhile must not pause the tester (the
# takeover path is driven by simulate_input below)
open -g -n --stderr "$WORK/helper.err" "$DEV/bise Computer Use dev.app" --args --socket "$SOCK" --no-takeover
for _ in $(seq 40); do [ -S "$SOCK" ] && break; sleep 0.25; done
[ -S "$SOCK" ] || { echo "the dev host did not start"; exit 1; }

id=0
# c '<op>' '<args json>' [agent]: one request; the reply's line in $R
c() {
  id=$((id + 1))
  R="$("$bin/cu-client" --socket "$SOCK" "{\"id\":$id,\"agent\":\"${3:-tester}\",\"op\":\"$1\",\"args\":$2}" | tail -n 1)"
}
ctl() { "$bin/cu-client" --socket "$SOCK" "$1" >/dev/null; }
# j '<python expr on r>': evaluate on the last reply
j() { python3 -c "import json,sys; r=json.loads(sys.argv[1]); print($1)" "$R" 2>/dev/null; }
expect() { # <name> <python bool expr>
  if [ "$(j "$2")" = True ]; then ok "$1"; else ko "$1" "$R"; fi
}
ours='test.bise.cu-testapp com.apple.TextEdit com.apple.calculator dev.bise.computer-use.dev'
steady() { # none of our apps came to the front; the cursor is where it was
  local p front; p="$("$bin/cu-client" --probe)"
  front="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["front"])' "$p")"
  case " $ours " in
    *" $front "*) ko "$1: $front came to the front" "$probe0 -> $p" ;;
    *) ok "$1: front app is not ours ($front)" ;;
  esac
  # the user may move the mouse meanwhile: a moved cursor is a warning
  # (a warp by the helper would put it on the element, every time)
  [ "${p#*cursor}" = "${probe0#*cursor}" ] || echo "     (cursor moved: $probe0 -> $p; you, or the helper?)"
  probe0="$p"
}

c permissions '{}'
if [ "$(j 'r["result"]["accessibility"]')" != True ]; then
  echo "skip: the dev host has no Accessibility permission. Grant it once:"
  echo "  System Settings > Privacy & Security > Accessibility > + > $DEV/bise Computer Use dev.app"
  echo "  (and Screen Recording); or send {\"id\":1,\"op\":\"request\",\"what\":\"accessibility\"}"
  exit 2
fi
shots="$(j 'r["result"]["screen_recording"]')"

# --- the tests' app -------------------------------------------------------
open -g "$DEV/cu test.app"
T='"target":"app:test.bise.cu-testapp"'
for _ in $(seq 40); do c snapshot "{$T}"; [ "$(j 'r["ok"]')" = True ] && break; sleep 0.25; done
expect "snapshot: C2 header" 'r["result"]["text"].startswith("# cu test · cu-testapp") or r["result"]["text"].startswith("# cu test · cu test")'
expect "snapshot: the button, with a ref" '"- button \"Add one\" [e" in r["result"]["text"]'
expect "snapshot: refs counted" 'r["result"]["refs"] == r["result"]["text"].count("[e")'
expect "snapshot: password masked" '"value=\"•••\"" in r["result"]["text"] and "hunter2" not in r["result"]["text"]'
expect "snapshot: checkbox, combobox roles" '"- checkbox \"Remember me\"" in r["result"]["text"] and "- combobox" in r["result"]["text"]'
c snapshot "{$T,\"max_nodes\":5}"
expect "snapshot: max_nodes cuts, truncated" 'r["result"]["truncated"] and r["result"]["refs"] == 5'
steady snapshot

status() { c act "{$T,\"action\":\"read\",\"locator\":{\"role\":\"text\",\"text_re\":\"^status: \"}}"; j 'r["result"]["changed"]'; }

c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
expect "click by locator" 'r["ok"] and r["result"]["summary"] == "clicked \"Add one\" · cu-testapp" or r["result"]["summary"].startswith("clicked \"Add one\" · ")'
expect "click: changed shows the status line" '"status: clicked 1" in r["result"]["changed"]'
[ "$(status)" = "status: clicked 1" ] && ok "read: the status says clicked 1" || ko "read after click" "$(status)"
steady click

ref="$(c snapshot "{$T}"; j 'r["result"]["text"].split("Add one\" [")[1].split("]")[0]')"
c act "{$T,\"action\":\"click\",\"ref\":\"$ref\"}"
expect "click by ref ($ref)" 'r["ok"] and "status: clicked 2" in r["result"]["changed"]'

c act "{$T,\"action\":\"fill\",\"locator\":{\"role\":\"textbox\",\"name\":\"Your name\"},\"value\":\"Ada\"}"
expect "fill" 'r["ok"] and "value=\"Ada\"" in r["result"]["changed"]'
c act "{$T,\"action\":\"press\",\"locator\":{\"role\":\"textbox\",\"name\":\"Your name\"},\"keys\":\"Enter\"}"
expect "press Enter (posted to the pid, app in the background)" 'r["ok"]'
[ "$(status)" = "status: name Ada" ] && ok "press: the app got Enter" || ko "press: the app did not get Enter (needs_front?)" "$(status)"
c act "{$T,\"action\":\"type\",\"locator\":{\"role\":\"textbox\",\"name\":\"Your name\"},\"text\":\" Lovelace\"}"
expect "type at the caret" 'r["ok"] and "Lovelace" in r["result"]["changed"]'
steady "fill/press/type"

c act "{$T,\"action\":\"check\",\"locator\":{\"role\":\"checkbox\",\"name\":\"Remember me\"}}"
expect "check" 'r["ok"] and r["result"]["summary"].startswith("checked \"Remember me\"")'
[ "$(status)" = "status: remember on" ] && ok "check: the app saw it" || ko "check: status" "$(status)"
c act "{$T,\"action\":\"check\",\"locator\":{\"role\":\"checkbox\",\"name\":\"Remember me\"},\"value\":false}"
expect "uncheck (value false)" 'r["ok"] and r["result"]["summary"].startswith("unchecked")'

c act "{$T,\"action\":\"select\",\"locator\":{\"role\":\"combobox\"},\"value\":\"Large\"}"
expect "select in a pop-up" 'r["ok"] and r["result"]["summary"].startswith("selected \"Large\"")'
[ "$(status)" = "status: size Large" ] && ok "select: the app saw it" || ko "select: status" "$(status)"
steady "check/select"

c act "{$T,\"action\":\"scroll\",\"locator\":{\"role\":\"table\"},\"direction\":\"down\"}"
expect "scroll the list (the scroll bar moved)" 'r["ok"] and "+ - scrollbar" in r["result"]["changed"]'
c act "{$T,\"action\":\"hover\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
expect "hover" 'r["ok"]'
steady "scroll/hover"

c act "{$T,\"action\":\"wait\",\"text\":\"size Large\"}"
expect "wait for text" 'r["ok"]'
c act "{$T,\"action\":\"wait\",\"text\":\"never there\",\"timeout_ms\":300}"
expect "wait: timeout" 'not r["ok"] and r["error"]["code"] == "timeout"'
c act "{$T,\"action\":\"read\"}"
expect "read the window" 'r["ok"] and "status: size Large" in r["result"]["changed"] and "hunter2" not in r["result"]["changed"]'

# errors
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Nope\"},\"timeout_ms\":200}"
expect "not_found + summary" 'r["error"]["code"] == "not_found" and r["error"]["summary"] == "couldn'"'"'t find \"Nope\""'
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\"}}"
expect "ambiguous lists candidates" 'r["error"]["code"] == "ambiguous" and len(r["error"]["candidates"]) >= 2 and r["error"]["candidates"][0].startswith("- button")'
c act "{$T,\"action\":\"click\",\"ref\":\"e999999\"}"
expect "stale_ref" 'r["error"]["code"] == "stale_ref"'
c act "{$T,\"action\":\"goto\",\"url\":\"https://example.com\"}"
expect "goto → bad_args" 'r["error"]["code"] == "bad_args" and r["error"]["message"] == "goto works on tabs only"'
c act '{"target":"app:com.mitchellh.ghostty","action":"click","ref":"e1"}'
expect "refused: a terminal" 'r["error"]["code"] == "refused"'
c snapshot '{"target":"app:com.apple.keychainaccess"}'
expect "refused: Keychain Access" 'r["error"]["code"] == "refused"'
c snapshot '{"target":"app:com.example.not-running"}'
expect "not running → not_found" 'r["error"]["code"] == "not_found"'
c snapshot "{$T,\"window\":\"no such window\"}"
expect "window: not_found with the titles" 'r["error"]["code"] == "not_found" and "- window \"cu test\"" in r["error"]["candidates"]'
c snapshot "{$T,\"window\":\"cu te\"}"
expect "window: unique substring" 'r["ok"]'

# apps
c apps '{}'
expect "apps lists the test app with its window" 'any(a["target"] == "app:test.bise.cu-testapp" and a["windows"][0]["title"] == "cu test" for a in r["result"])'
expect "apps hides refused apps" 'not any(a["target"] in ("app:com.mitchellh.ghostty", "app:com.apple.Terminal") for a in r["result"])'

# the cursor overlay: shown over the window while driving, gone at release
c diag '{}'
expect "overlay: the agent's cursor exists" 'any(o["agent"] == "tester" for o in r["result"]["overlays"])'
ctl '{"release":"tester"}'
sleep 0.3
c diag '{}'
expect "release hides the cursor" 'not any(o["agent"] == "tester" and o["visible"] for o in r["result"]["overlays"])'

# takeover, stop, resume
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
"$bin/cu-client" --socket "$SOCK" "{\"id\":900,\"op\":\"simulate_input\",\"args\":{$T}}" --wait-event paused 3 > "$WORK/ev" 2>&1
grep -q '"event":"paused"' "$WORK/ev" && grep -q '"target":"app:test.bise.cu-testapp"' "$WORK/ev" && ok "takeover: paused event" || ko "takeover: no paused event" "$(cat "$WORK/ev")"
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
expect "paused: the next act fails" 'r["error"]["code"] == "paused" and r["error"]["summary"].startswith("you took the wheel")'
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}" other
expect "another agent is not paused" 'r["ok"]'
"$bin/cu-client" --socket "$SOCK" '{"resume":"tester"}' --wait-event resumed 3 > "$WORK/ev" 2>&1
grep -q '"event":"resumed"' "$WORK/ev" && ok "resume: resumed event" || ko "resume: no event" "$(cat "$WORK/ev")"
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
expect "resumed: acts again" 'r["ok"]'
ctl '{"stop":"tester"}'
c act "{$T,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"Add one\"}}"
expect "stopped: act fails" 'r["error"]["code"] == "stopped"'
c snapshot "{$T}"
expect "stopped: snapshot fails" 'r["error"]["code"] == "stopped"'
ctl '{"resume":"tester"}'
c snapshot "{$T}"
expect "resume after stop" 'r["ok"]'
steady "takeover/stop"

# screenshots: the window is behind the user's windows (covered)
if [ "$shots" = True ]; then
  c screenshot "{$T}"
  expect "screenshot: a JPEG of the window" 'r["ok"] and r["result"]["mime"] == "image/jpeg" and r["result"]["data"].startswith("/9j/") and 0 < r["result"]["width"] <= 1280'
  w="$(j 'r["result"]["width"]')"
  c screenshot "{$T,\"max_width\":200}"
  expect "screenshot: max_width" 'r["ok"] and r["result"]["width"] == 200'
  c snapshot "{$T}"
  ref="$(j 'r["result"]["text"].split("Add one\" [")[1].split("]")[0]')"
  c screenshot "{$T,\"ref\":\"$ref\"}"
  expect "screenshot: one element (crop)" "r['ok'] and r['result']['width'] < $w"
  steady screenshot
else
  echo "skip: screenshots (no Screen Recording permission)"
fi

# --- TextEdit (hidden) ----------------------------------------------------
pgrep -xq TextEdit || started+=(com.apple.TextEdit)
note="cu-note-$$.txt"
printf 'hello from the test\n' > "$WORK/$note"
open -g -j -a TextEdit "$WORK/$note"
E="\"target\":\"app:com.apple.TextEdit\",\"window\":\"$note\""
for _ in $(seq 40); do c snapshot "{$E}"; [ "$(j 'r["ok"]')" = True ] && break; sleep 0.25; done
expect "TextEdit: snapshot (hidden app)" 'r["ok"] and "hello from the test" in r["result"]["text"]'
c act "{$E,\"action\":\"type\",\"locator\":{\"role\":\"textbox\"},\"text\":\"typed by cu-apps\"}"
expect "TextEdit: type" 'r["ok"] and "typed by cu-apps" in r["result"]["changed"]'
c act "{$E,\"action\":\"press\",\"locator\":{\"role\":\"textbox\"},\"keys\":\"Meta+A\"}"
expect "TextEdit: press Meta+A" 'r["ok"]'
steady TextEdit
# our note only (TextEdit may be the user's): close it, saved (it is ours)
c act "{$E,\"action\":\"close\"}"
expect "TextEdit: close our window" "r['ok'] and r['result']['summary'] == 'closed \"$note\" · TextEdit'"

# --- Calculator (hidden) --------------------------------------------------
pgrep -xq Calculator || started+=(com.apple.calculator)
open -g -j -a Calculator
K='"target":"app:com.apple.calculator"'
for _ in $(seq 40); do c snapshot "{$K}"; [ "$(j 'r["ok"]')" = True ] && break; sleep 0.25; done
expect "Calculator: snapshot" 'r["ok"] and "- button" in r["result"]["text"]'
echo "$R" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["text"])' > "$WORK/calc.txt"
key() { c act "{$K,\"action\":\"click\",\"locator\":{\"role\":\"button\",\"name\":\"$1\",\"exact\":true}}"; }
key "Clear"; key "All Clear"
key 7; expect "Calculator: click 7" 'r["ok"]'
key "Add"; key 3; key "Equals"
expect "Calculator: = shows 10" '"10" in r["result"]["changed"]'
steady Calculator
[ "$fails" = 0 ] || { echo "Calculator tree:"; head -40 "$WORK/calc.txt"; }

echo "test.sh: $((n - fails))/$n passed"
[ "$fails" = 0 ]
