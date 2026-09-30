#!/usr/bin/env python3
"""Serve site/ and store page annotations.

python3 serve.py [port]   (default 4747)
GET  /__notes/<page>  -> notes/<page>.json
POST /__notes/<page>  -> writes notes/<page>.json and notes/<page>.md
"""
import datetime, http.server, json, os, sys

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTES = os.path.join(SITE, "content", "notes")
MARK = {"love": "♡", "meh": "~", "no": "✗"}


def to_md(page, notes):
    out = [f"# notes · {page} · {datetime.datetime.now():%Y-%m-%d %H:%M}", ""]
    for n in notes:
        mark = MARK.get(n.get("verdict") or "", "·")
        quote = " ".join((n.get("quote") or "").split())[:160]
        line = f"- {mark} [{n.get('key','')}] \"{quote}\""
        if n.get("text"):
            line += f"\n  → {n['text']}"
        if n.get("reply"):
            line += f"\n  ↳ marketing: {n['reply']}"
        out.append(line)
    return "\n".join(out) + "\n"


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=SITE, **k)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def _page(self):
        raw = self.path.split("?")[0][len("/__notes/"):]
        return "".join(c for c in raw if c.isalnum() or c in "-_") or "page"

    def do_GET(self):
        if not self.path.startswith("/__notes/"):
            return super().do_GET()
        path = os.path.join(NOTES, self._page() + ".json")
        data = open(path, "rb").read() if os.path.exists(path) else b"[]"
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if not self.path.startswith("/__notes/"):
            return self.send_error(404)
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        try:
            notes = json.loads(body)
        except ValueError:
            return self.send_error(400)
        os.makedirs(NOTES, exist_ok=True)
        page = self._page()
        with open(os.path.join(NOTES, page + ".json"), "w") as f:
            json.dump(notes, f, indent=1, ensure_ascii=False)
        with open(os.path.join(NOTES, page + ".md"), "w") as f:
            f.write(to_md(page, notes))
        self.send_response(204)
        self.end_headers()

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4747
    print(f"http://127.0.0.1:{port}/content/")
    http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
