#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["uharfbuzz>=0.40", "websocket-client"]
# ///
"""The X header (1500x500) in the paper art direction: light paper, the north-wind cloud blowing,
'bise :*' (Newsreader + the Caveat kiss, its * level with the colon), a few seeds in the wind.
Run: uv run docs/brand/x-header/paper.py -> header-paper.svg, header-paper.png, header-paper@2x.png
(the PNGs are rendered by headless Chrome, so the pencil filter looks like it does on the site).
X covers the bottom-left with the avatar and crops the top and bottom on phones: the name and the
cloud sit in the middle band, right of center."""
import os, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "readme"))
import ink

W, H, M = 1500, 500, "light"
t = ink.PAPER[M]
tp = ink.text_path

# the name, centered a little right of the middle
NS, KS = 150, 138
_, w_bise = tp("bise", "serif", NS, track=-.045, wght=600, opsz=72)
w_sp = ink.measure(" ", "serif", NS, wght=600, opsz=72) * .45
_, w_col = tp(":", "hand", KS, wght=700); _, w_star = tp("*", "hand", KS, wght=700)
w_name = w_bise + w_sp + w_col + w_star
x0, yb = 560 - w_name / 2, 300
d_bise, _ = tp("bise", "serif", NS, x=x0, y=yb, track=-.045, wght=600, opsz=72)
xk = x0 + w_bise + w_sp
d_col, _ = tp(":", "hand", KS, x=xk, y=yb, wght=700)
d_star, _ = tp("*", "hand", KS, x=xk + w_col - KS * .02, y=yb + KS * .3, wght=700)

# the cloud, right of the name, blowing outward (viewBox 0 0 260 120)
cs = 1.55
cx, cy = xk + w_col + w_star + 50, 120

# seeds carried by the wind, after the cloud
seeds = [(1210, 150, 22, 0), (1290, 215, 18, 40), (1345, 140, 16, 120), (1400, 250, 20, 200), (1255, 300, 14, 80),
         (1440, 185, 15, 300), (300, 150, 14, 30), (230, 330, 16, 90), (980, 380, 14, 160)]
seed_svg = "".join(f'<text x="{x}" y="{y}" font-size="{s}" fill="{t["kick"]}" opacity=".55" transform="rotate({r} {x} {y})" '
                   f'font-family="JetBrains Mono, ui-monospace, monospace">*</text>' for x, y, s, r in seeds)

svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">'
       f'<defs>{ink.paper_defs(M)}{ink.cloud_defs(M)}</defs>'
       f'<rect width="{W}" height="{H}" fill="{t["paper"]}"/><rect width="{W}" height="{H}" fill="url(#paper)"/>'
       f'{seed_svg}'
       f'<path d="{d_bise}" fill="{t["ink"]}"/><g fill="{t["pen"]}"><path d="{d_col}"/><path d="{d_star}"/></g>'
       f'<g transform="translate({cx:.1f} {cy}) scale({cs})">{ink.cloud(M)}</g>'
       '</svg>' + chr(10))
out = os.path.join(HERE, "header-paper.svg")
open(out, "w").write(svg)

chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
def render(page, scale, out, tmp):
    """one screenshot through Chrome DevTools (the --screenshot flag hangs at device scale 2 on the pencil filter)."""
    import base64, json, time, urllib.request, websocket
    port = 9334 + scale
    p = subprocess.Popen([chrome, "--headless=new", f"--remote-debugging-port={port}", f"--user-data-dir={tmp}/prof{scale}", "--hide-scrollbars",
                          f"--window-size={W},{H}", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try: tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")); break
            except Exception: time.sleep(.2)
        ws = websocket.create_connection([t for t in tabs if t["type"] == "page"][0]["webSocketDebuggerUrl"], suppress_origin=True)
        n = [0]
        def call(m, **pa):
            n[0] += 1; ws.send(json.dumps({"id": n[0], "method": m, "params": pa}))
            while True:
                r = json.loads(ws.recv())
                if r.get("id") == n[0]: return r.get("result", r)
        call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=scale, mobile=False)
        call("Page.enable"); call("Page.navigate", url="file://" + page); time.sleep(2)
        open(out, "wb").write(base64.b64decode(call("Page.captureScreenshot", format="png")["data"]))
    finally:
        p.kill()

with tempfile.TemporaryDirectory() as tmp:
    page = os.path.join(tmp, "p.html")
    open(page, "w").write(f'<!doctype html><body style="margin:0">{svg}</body>')
    for scale, name in [(1, "header-paper.png"), (2, "header-paper@2x.png")]:
        render(page, scale, os.path.join(HERE, name), tmp)
print("ok")
