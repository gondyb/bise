#!/usr/bin/env bash
# test-release-gh.sh — the GitHub Releases channel (BISE-217) end to end,
# public and private, against a local stand-in of GitHub: a python server
# with the release download URLs (`/<o>/<r>/releases/latest/download/`),
# the API (`/api/repos/<o>/<r>/releases/latest`, assets answered with a
# redirect to another host, as GitHub does) and bise.dev/install; a stub
# `gh` for the GitHub CLI. All under /tmp with `env -i` and fake HOMEs.
#
#   projects/switchboard/packaging/test-release-gh.sh
#
# Public: `curl .../install | sh` and `bise update` read the plain URLs,
# no gh, no token. Private (the download URLs answer 404, like GitHub
# without auth): no gh and no token -> a clear 'gh auth login' error,
# nothing installed; gh not logged in -> the same; gh logged in -> install,
# `update --check`, `update --background` (the daily check's command);
# GH_TOKEN and no gh -> install and update through the API; a bad token
# -> the error. The app is this tree's debug `bise` with stub REPLs and
# engine (no hub runs here: test-release.sh covers the hub).

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE" && git rev-parse --show-toplevel)"
W=/tmp/prg REL=/tmp/prg/rel PK=/tmp/prg/pack SRV=/tmp/prg/srv STUB=/tmp/prg/stub
TOKEN=tok-good

pass=0; fail=0
ok()  { pass=$((pass + 1)); echo "  ok   $*"; }
ko()  { fail=$((fail + 1)); echo "  FAIL $*"; }
check() { local what="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$what"; else ko "$what"; fi; }
has() { case "$1" in *"$2"*) return 0 ;; esac; return 1; }

[ -f "$W/srv.pid" ] && kill "$(cat "$W/srv.pid")" 2>/dev/null
rm -rf "$W"; mkdir -p "$REL" "$PK" "$SRV" "$STUB/bin"

os="$(uname -s | tr '[:upper:]' '[:lower:]')"; arch="$(uname -m)"; [ "$arch" = aarch64 ] && arch=arm64
target="$os-$arch"

echo "== the app: this tree's bise, stub REPLs and engine"
(cd "$REPO/rust" && cargo build -q -p bend-harness) || { echo "cargo build failed"; exit 1; }
src="$PK/src"; mkdir -p "$src"
cp "${CARGO_TARGET_DIR:-$REPO/rust/target}/debug/bise" "$src/bise"
for b in repl-live repl-scripted sb-core bend-jsrt; do printf '#!/bin/sh\nexit 1\n' > "$src/$b"; chmod 755 "$src/$b"; done
pack() {  # <id> <built>: a build-dist.sh-shaped archive of the app
  local d="$PK/bise-$1-$target"
  rm -rf "$d"; mkdir -p "$d"; cp -cR "$src" "$d/app" 2>/dev/null || cp -R "$src" "$d/app"
  printf 'id=%s\ncommit=%s\nsubject=release %s\nbuilt=%s\nmacos=14.0\ntarget=%s\nchannel=test\n' \
    "$1" "$(git -C "$REPO" rev-parse HEAD)" "$1" "$2" "$target" > "$d/app/VERSION"
  cp "$HERE/install.sh" "$d/install.sh"
  tar -C "$PK" -czf "$PK/bise-$1-$target.tar.gz" "bise-$1-$target"
  echo "$PK/bise-$1-$target.tar.gz"
}

echo "== a stand-in GitHub"
cat > "$W/srv.py" <<'EOF'
import http.server, json, os, sys
REL, SRV, TOKEN = sys.argv[1], sys.argv[2], sys.argv[3]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, code, body=b"", ctype="application/octet-stream", loc=None):
        self.send_response(code)
        if loc: self.send_header("Location", loc)
        self.send_header("Content-Type", ctype); self.send_header("Content-Length", str(len(body)))
        self.end_headers(); self.wfile.write(body)
    def file(self, name):
        p = os.path.join(REL, os.path.basename(name))
        if not os.path.isfile(p): return self.send(404, b"Not Found")
        self.send(200, open(p, "rb").read())
    def do_GET(self):
        auth = self.headers.get("Authorization", "")
        with open(os.path.join(SRV, "log"), "a") as f: f.write(f"{self.path} auth={'yes' if auth else 'no'}\n")
        private = open(os.path.join(SRV, "mode")).read().strip() == "private"
        p = self.path.split("?")[0].split("/")[1:]
        port = self.server.server_address[1]
        if p[:2] == ["site", "install"]: return self.file("install.sh")   # bise.dev/install
        if p[:5] == ["o", "r", "releases", "latest", "download"] and len(p) == 6:
            return self.send(404, b"Not Found") if private else self.file(p[5])
        if p[:1] == ["api"]:
            if auth != "Bearer " + TOKEN: return self.send(404, b'{"message":"Not Found"}', "application/json")
            if p[1:] == ["repos", "o", "r", "releases", "latest"]:
                assets = [{"name": n, "label": None, "url": f"http://127.0.0.1:{port}/api/repos/o/r/releases/assets/{n}"} for n in sorted(os.listdir(REL))]
                return self.send(200, json.dumps({"tag_name": "vX", "body": None, "assets": assets}).encode(), "application/json")
            if p[1:6] == ["repos", "o", "r", "releases", "assets"] and len(p) == 7:
                if self.headers.get("Accept") != "application/octet-stream": return self.send(200, b'{"name":"json"}', "application/json")
                return self.send(302, loc=f"http://localhost:{port}/s3/{p[6]}")
        if p[:1] == ["s3"]:   # another host: GitHub's storage refuses an Authorization header
            return self.send(400, b"auth sent to storage") if auth else self.file(p[1])
        self.send(404, b"Not Found")
s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
open(os.path.join(SRV, "port"), "w").write(str(s.server_address[1]))
s.serve_forever()
EOF
echo public > "$SRV/mode"
python3 "$W/srv.py" "$REL" "$SRV" "$TOKEN" 2>"$W/srv.err" &
srv=$!; echo "$srv" > "$W/srv.pid"
for _ in $(seq 50); do [ -s "$SRV/port" ] && break; sleep 0.1; done
PORT="$(cat "$SRV/port" 2>/dev/null)"; [ -n "$PORT" ] || { echo "server did not start"; cat "$W/srv.err"; exit 1; }
trap 'kill "$srv" 2>/dev/null' EXIT
CH="http://127.0.0.1:$PORT/o/r/releases/latest/download" SITE="http://127.0.0.1:$PORT/site/install"
# the GitHub CLI: logged in when $STUB/authed exists (like `gh auth login`)
cat > "$STUB/bin/gh" <<EOF
#!/bin/sh
echo "gh \$*" >> "$SRV/gh.log"
[ -f "$STUB/authed" ] || { echo "To get started with GitHub CLI, please run:  gh auth login" >&2; exit 4; }
[ "\$1 \$2" = "release download" ] || exit 2
shift 2; name="" out="" repo=""
while [ \$# -gt 0 ]; do case "\$1" in -p) name="\$2"; shift ;; -O) out="\$2"; shift ;; -R) repo="\$2"; shift ;; esac; shift; done
case "\$repo" in */o/r) ;; *) echo "release not found" >&2; exit 1 ;; esac
cp "$REL/\$name" "\$out" 2>/dev/null || { echo "no assets match the file pattern" >&2; exit 1; }
EOF
chmod 755 "$STUB/bin/gh"
publish() { "$HERE/make-release.sh" --out "$REL" --url "$CH" --version "$2" "$1" >/dev/null 2>&1; }
t1="$(pack r1 2026-01-01T00:00:01Z)" t2="$(pack r2 2026-01-02T00:00:00Z)" t3="$(pack r3 2026-01-03T00:00:00Z)" t4="$(pack r4 2026-01-04T00:00:00Z)"
publish "$t1" 0.0.1 && ok "release r1 on the stand-in ($CH)" || { ko "make-release.sh"; exit 1; }

# a clean environment: fake HOME, system PATH (+ the stub gh: E_GH=1)
E() {
  local p=/usr/bin:/bin:/usr/sbin:/sbin; [ "${E_GH:-0}" = 1 ] && p="$STUB/bin:$p"
  env -i HOME="$H" PATH="$p" SHELL=/bin/zsh TERM=dumb USER="${USER:-me}" LANG=en_US.UTF-8 BISE_NO_UPDATE=1 "$@"
}
install_line() { (cd "$W" && E sh -c "curl -fsSL '$SITE' | sh" 2>&1); }
cur() { readlink "$H/.local/share/bise/current" 2>/dev/null; }
B() { E "$H/.local/bin/bise" "$@" 2>&1; }

echo "== public: plain downloads, no gh, no token"
H=$W/home-pub; mkdir -p "$H"
out="$(install_line)"
check "curl bise.dev/install | sh installs r1" test "$(cur)" = versions/r1
check "the channel is recorded" test "$(cat "$H/.local/share/bise/dist-url")" = "$CH"
publish "$t2" 0.0.2
out="$(B update --check)"; has "$out" "r2 is available" && ok "update --check: r2 available" || ko "update --check: $out"
out="$(B update)"; check "bise update: current -> r2" test "$(cur)" = versions/r2
check "no gh, no API call" sh -c "[ ! -e '$SRV/gh.log' ] && ! grep -q '^/api' '$SRV/log'"

echo "== private: the download URLs answer 404"
echo private > "$SRV/mode"
H=$W/home-priv; mkdir -p "$H"
out="$(install_line)"
has "$out" "gh auth login" && ok "no gh, no token: the error says gh auth login" || ko "no gh: $out"
printf '%s\n' "$out" | tail -n 2 | sed 's/^/     /'
check "nothing installed" test ! -e "$H/.local/share/bise"
out="$(E_GH=1 install_line)"
has "$out" "gh auth login" && ok "gh not logged in: the same error" || ko "gh not logged in: $out"
check "gh was asked" grep -q 'release download -R 127.0.0.1:'"$PORT"'/o/r -p latest.json' "$SRV/gh.log"
touch "$STUB/authed"
out="$(E_GH=1 install_line)"
check "gh logged in: installs r2 (the latest)" test "$(cur)" = versions/r2
publish "$t3" 0.0.3
rm "$STUB/authed"
out="$(E_GH=1 B update --check)"
has "$out" "gh auth login" && ok "update --check, gh logged out: the error says gh auth login" || ko "update, logged out: $out"
printf '%s\n' "$out" | sed 's/^/     /'
touch "$STUB/authed"
out="$(E_GH=1 B update --check)"; has "$out" "r3 is available" && ok "update --check via gh: r3 available" || ko "update --check via gh: $out"
E_GH=1 E "$H/.local/bin/bise" update --background 2>"$W/bg.log"
check "update --background via gh: current -> r3" test "$(cur)" = versions/r3
check "... and it logs 'updated to r3'" grep -q 'updated to r3' "$W/bg.log"
rm "$STUB/authed"

echo "== private: GH_TOKEN, no gh (the API)"
H=$W/home-tok; mkdir -p "$H"
: > "$SRV/log"
out="$(cd "$W" && E GH_TOKEN="$TOKEN" BISE_GITHUB_API="http://127.0.0.1:$PORT/api" sh -c "curl -fsSL '$SITE' | sh" 2>&1)"
check "installs r3 through the API" test "$(cur)" = versions/r3
check "the asset came from the storage host, without the token" grep -q '^/s3/bise-r3-.* auth=no' "$SRV/log"
check "the token is never in a URL" sh -c "! grep -q '$TOKEN' '$SRV/log'"
publish "$t4" 0.0.4
out="$(E GH_TOKEN=bad BISE_GITHUB_API="http://127.0.0.1:$PORT/api" "$H/.local/bin/bise" update 2>&1)"
has "$out" "gh auth login" && ok "a bad token: the error says gh auth login" || ko "bad token: $out"
out="$(E GITHUB_TOKEN="$TOKEN" BISE_GITHUB_API="http://127.0.0.1:$PORT/api" "$H/.local/bin/bise" update 2>&1)"
check "GITHUB_TOKEN: bise update -> r4" test "$(cur)" = versions/r4

echo "== public again: the private installs update with no auth"
echo public > "$SRV/mode"
H=$W/home-priv
out="$(B update)"; check "the gh install updates to r4 by plain curl" test "$(cur)" = versions/r4

echo "== $pass passed, $fail failed"
[ "$fail" = 0 ] && rm -rf "$W"
[ "$fail" = 0 ]
