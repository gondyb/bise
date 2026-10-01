"""paper art direction, for images that load no web fonts (GitHub READMEs, og.png sources).

text_path() turns Newsreader / Caveat words into SVG paths (harfbuzz shapes them, kerning
included), so the display words look the same everywhere. The rest: the paper tokens, a grain
tile, a pencil frame drawn as a path (no filter: cheap to repaint in animated SVGs), the cloud.
The fonts are fetched once from github.com/google/fonts into ~/.cache/bise-brand-fonts."""
import base64, math, os, random, struct, urllib.request, zlib

CACHE = os.path.expanduser("~/.cache/bise-brand-fonts")
FONTS = {
    "serif": "ofl/newsreader/Newsreader%5Bopsz,wght%5D.ttf",
    "serif-italic": "ofl/newsreader/Newsreader-Italic%5Bopsz,wght%5D.ttf",
    "hand": "ofl/caveat/Caveat%5Bwght%5D.ttf",
}

# the paper tokens (site/content/landing-paper.html). 'in_*' are the screens' own colors on that paper.
PAPER = {
    # graphite: the cloud and every pencil mark are grey, no blue (the user, after the first round)
    "light": dict(paper="#f2ede2", ink="#1d1a17", dim="#5a5349", kick="#8a8174", pen="#c8264a", graphite="#6b645a",
                  card="#faf7f0", foot="#f3eee4", frame="#1d1a17a8", shadow="#1d1a170c", hl="#f4a6b099",
                  hatch="#8a8174", gust="#8a8174", face="#1d1a17", cheek="#ef9aae", cheek_a=".55", grain=(90, 77, 64), grain_a=20),
    "dark":  dict(paper="#171513", ink="#ece6da", dim="#b8b0a3", kick="#8a8276", pen="#f4a6b0", graphite="#b8b0a3",
                  card="#1f1c19", foot="#24201d", frame="#ece6da80", shadow="#00000038", hl="#f4a6b047",
                  hatch="#8a8276", gust="#8a8276", face="#ece6da", cheek="#f4a6b0", cheek_a=".45", grain=(255, 242, 230), grain_a=11),
}

_fonts = {}
def _font(face, **var):
    import uharfbuzz as hb
    key = (face, tuple(sorted(var.items())))
    if key not in _fonts:
        os.makedirs(CACHE, exist_ok=True)
        p = os.path.join(CACHE, os.path.basename(FONTS[face]))
        if not os.path.exists(p):
            urllib.request.urlretrieve("https://raw.githubusercontent.com/google/fonts/main/" + FONTS[face], p)
        f = hb.Font(hb.Face(open(p, "rb").read()))
        if var: f.set_variations(var)
        _fonts[key] = f
    return _fonts[key]

class _Pen:
    def __init__(s, k, ox, oy): s.k, s.ox, s.oy, s.d = k, ox, oy, []
    def _p(s, x, y): return f"{s.ox + x * s.k:.1f} {s.oy - y * s.k:.1f}"
    def moveTo(s, p): s.d.append("M" + s._p(*p))
    def lineTo(s, p): s.d.append("L" + s._p(*p))
    def qCurveTo(s, *pts):
        # TrueType: implied on-curve points between consecutive off-curve ones
        *offs, end = pts
        for i, a in enumerate(offs):
            b = end if i == len(offs) - 1 else ((a[0] + offs[i + 1][0]) / 2, (a[1] + offs[i + 1][1]) / 2)
            s.d.append("Q" + s._p(*a) + " " + s._p(*b))
    def curveTo(s, a, b, c): s.d.append("C" + s._p(*a) + " " + s._p(*b) + " " + s._p(*c))
    def closePath(s): s.d.append("Z")
    def endPath(s): pass

def measure(text, face="serif", size=20, track=0.0, **var):
    return text_path(text, face, size, track=track, **var)[1]

def text_path(text, face="serif", size=20, x=0.0, y=0.0, track=0.0, anchor="start", **var):
    """(path d, width): `text` set at baseline y. track: letter-spacing in em. anchor: start|middle|end."""
    import uharfbuzz as hb
    f = _font(face, **var)
    upem = f.face.upem
    buf = hb.Buffer(); buf.add_str(text); buf.guess_segment_properties()
    hb.shape(f, buf, {"kern": True, "liga": True})
    k = size / upem
    width = sum(p.x_advance for p in buf.glyph_positions) * k + track * size * (len(buf.glyph_infos) - 1)
    x0 = x - (width / 2 if anchor == "middle" else width if anchor == "end" else 0)
    pen_x, d = 0.0, []
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        p = _Pen(k, x0 + (pen_x + pos.x_offset) * k, y - pos.y_offset * k)
        f.draw_glyph_with_pen(info.codepoint, p)
        d += p.d
        pen_x += pos.x_advance + track * upem
    return "".join(d), width

# ---------- the paper ----------
def _png(w, h, rows):
    raw = b"".join(b"\0" + r for r in rows)
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")

_grain = {}
def grain(mode, n=96, seed=3):
    """a small tile of pencil grain (a PNG data URI): sparse specks, one color, a few alpha levels."""
    if mode in _grain: return _grain[mode]
    t = PAPER[mode]; r, g, b = t["grain"]; a = t["grain_a"]; rnd = random.Random(seed)
    rows = []
    for _ in range(n):
        row = bytearray()
        for _ in range(n):
            v = rnd.random()
            al = 0 if v < .55 else a // 2 if v < .85 else a if v < .97 else a * 2
            row += bytes((r, g, b, al))
        rows.append(bytes(row))
    _grain[mode] = "data:image/png;base64," + base64.b64encode(_png(n, n, rows)).decode()
    return _grain[mode]

def paper_defs(mode, pid="paper"):
    return (f'<pattern id="{pid}" width="96" height="96" patternUnits="userSpaceOnUse">'
            f'<image href="{grain(mode)}" width="96" height="96"/></pattern>')

def _noise(seed):
    rnd = random.Random(seed); ph = [rnd.uniform(0, 6.28) for _ in range(6)]
    return lambda u: (math.sin(u * 2.1 + ph[0]) * .5 + math.sin(u * 5.3 + ph[1]) * .3 + math.sin(u * 11.7 + ph[2]) * .2)

def wobble_rect(x, y, w, h, r=10, amp=1.1, seed=7, step=14):
    """a rounded rect drawn by hand: the pencil frame of a figure, as a plain path."""
    pts, nz = [], _noise(seed)
    def arc(cx, cy, a0):
        for i in range(1, 5): a = a0 + i * math.pi / 2 / 4; pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    def side(x0, y0, x1, y1):
        L = math.hypot(x1 - x0, y1 - y0); n = max(2, int(L / step))
        for i in range(n): t = i / n; pts.append((x0 + (x1 - x0) * t, y0 + (y1 - y0) * t))
    side(x + r, y, x + w - r, y); arc(x + w - r, y + r, -math.pi / 2)
    side(x + w, y + r, x + w, y + h - r); arc(x + w - r, y + h - r, 0)
    side(x + w - r, y + h, x + r, y + h); arc(x + r, y + h - r, math.pi / 2)
    side(x, y + h - r, x, y + r); arc(x + r, y + r, math.pi)
    per = sum(math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]) for i in range(len(pts)))
    out, acc = [], 0.0
    for i, (px, py) in enumerate(pts):
        if i: acc += math.hypot(px - pts[i - 1][0], py - pts[i - 1][1])
        u = acc / per * 2 * math.pi * 3
        out.append((px + nz(u) * amp, py + nz(u + 40) * amp))
    out.append(out[0])
    d = f"M{out[0][0]:.1f} {out[0][1]:.1f}"
    for i in range(1, len(out)):  # smooth: through the midpoints
        mx, my = (out[i - 1][0] + out[i][0]) / 2, (out[i - 1][1] + out[i][1]) / 2
        d += f" Q{out[i - 1][0]:.1f} {out[i - 1][1]:.1f} {mx:.1f} {my:.1f}"
    return d + " Z"

def figure(mode, w, h, body, x=0, y=0, r=10, seed=7):
    """a screen as a figure: a card a bit lighter than the paper, a flat offset shadow, a pencil frame on top."""
    t = PAPER[mode]
    return (f'<g transform="translate({x} {y})"><rect x="3" y="3" width="{w}" height="{h}" rx="{r}" fill="{t["shadow"]}"/>'
            f'<rect width="{w}" height="{h}" rx="{r}" fill="{t["card"]}"/>{body}'
            f'<path d="{wobble_rect(1.5, 1.5, w - 3, h - 3, r, seed=seed)}" fill="none" stroke="{t["frame"]}" stroke-width="1.6" stroke-linejoin="round"/></g>')

PENCIL_FILTER = ('<filter id="pencil" x="-10%" y="-10%" width="120%" height="120%"><feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" seed="4" result="n"/>'
                 '<feDisplacementMap in="SourceGraphic" in2="n" scale="2.2" result="d"/><feTurbulence type="fractalNoise" baseFrequency="1.8" seed="9" result="g"/>'
                 '<feColorMatrix in="g" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.1 1.3" result="m"/><feComposite in="d" in2="m" operator="in"/></filter>')

def cloud_defs(mode):
    t = PAPER[mode]
    return (PENCIL_FILTER + f'<pattern id="hatchB" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(-35)">'
            f'<line x1="0" y1="0" x2="0" y2="5" stroke="{t["hatch"]}" stroke-width="1.5" opacity=".7"/></pattern>')

def cloud(mode, asleep=False):
    """the north wind, a colored-pencil cloud blowing (viewBox 0 0 220 120). gusts: class 'gust'."""
    t = PAPER[mode]
    eyes = '<path d="M70 66 q6 5 12 0"/><path d="M104 66 q6 5 12 0"/>'
    mouth = '<path d="M150 46 l8 0 l-8 8 l8 0"/>' if asleep else '<ellipse cx="95" cy="80" rx="4" ry="4.6"/>'
    # the gusts start at the cloud's right edge and blow outward: they never cross its face
    wind = '' if asleep else ('<g class="gust"><path d="M182 74 C196 68,208 80,226 72 S246 66,254 70"/>'
                              '<path d="M180 86 C196 90,210 98,232 92"/><path d="M174 60 C188 52,200 56,216 48"/></g>')
    return (f'<g filter="url(#pencil)" fill="none" stroke-linecap="round" stroke-linejoin="round">'
            f'<path d="M40 92 C16 92,10 66,32 60 C26 36,56 26,70 42 C78 18,118 16,126 40 C142 26,170 36,164 58 C188 60,188 92,164 94 Z" fill="url(#hatchB)" stroke="{t["graphite"]}" stroke-width="2.8"/>'
            f'<g stroke="{t["face"]}" stroke-width="2">{eyes}</g><circle cx="66" cy="78" r="6" fill="{t["cheek"]}" opacity="{t["cheek_a"]}"/><circle cx="122" cy="78" r="6" fill="{t["cheek"]}" opacity="{t["cheek_a"]}"/>'
            f'<g stroke="{t["graphite"] if asleep else t["face"]}" stroke-width="1.8">{mouth}</g>'
            f'<g stroke="{t["gust"]}" stroke-width="2">{wind}</g></g>')
