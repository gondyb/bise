#!/usr/bin/env python3
"""An MCP tool call with non-ASCII arguments reaches the server whole.

runtime/mcp.bend posted its JSON-RPC body as a plain string; Http counts
Content-Length in the string's units, so each non-ASCII char made it one
byte or more short: the plugins bridge read a cut JSON, answered 400, and
the agent saw "mcp call failed: non-200 http status" (launch, computer use:
getByText(/25\\.0 s · paused/)). call_body now UTF-8-encodes the body.

The probe (tests/mcp_utf8_probe.bend, compiled native) POSTs call_body with
'·', 'é', '↖' and an emoji to a server that checks Content-Length against
the bytes and parses the JSON; it must answer 200 and see the text intact.

python3 -u tests/mcp_utf8.py
"""
import http.server, json, os, shutil, subprocess, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
SEEN = []


class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(n)
        try:
            msg = json.loads(raw.decode("utf-8"))
            SEEN.append(msg)
            code, out = 200, b"{}"
        except Exception as e:  # a cut body: what the bridge answered 400 to
            SEEN.append({"error": str(e), "len": n})
            code, out = 400, str(e).encode()
        self.send_response(code)
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


def main():
    bend = shutil.which("bend") or os.path.expanduser("~/.bend/bin/bend")
    if not os.path.exists(bend):
        print("SKIP mcp_utf8: no bend compiler")
        return 0
    tmp = tempfile.mkdtemp(prefix="sb-mcp-utf8-")
    exe = os.path.join(tmp, "probe")
    c = subprocess.run([bend, "mcp_utf8_probe.bend", "-o", exe], cwd=HERE, capture_output=True, text=True)
    if not os.path.exists(exe):
        print("FAIL mcp_utf8: the probe did not compile\n" + c.stdout[-2000:] + c.stderr[-2000:])
        return 1
    srv = http.server.HTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    env = dict(os.environ, PROBE_URL="http://127.0.0.1:%d/" % srv.server_address[1])
    out = subprocess.run([exe], env=env, capture_output=True, text=True, timeout=30).stdout.strip()
    srv.shutdown()
    shutil.rmtree(tmp, ignore_errors=True)
    fails = []
    if out != "200":
        fails.append("status %r (want 200), server saw %r" % (out, SEEN))
    args = SEEN[0].get("params", {}).get("arguments", {}) if SEEN else {}
    if args.get("locator", {}).get("text_re") != "25\\.0 s · paused" or args.get("note") != "élan ↖ 🙂":
        fails.append("arguments changed on the way: %r" % (args,))
    for f in fails:
        print("FAIL " + f)
    print("PASS mcp_utf8" if not fails else "FAIL mcp_utf8")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
