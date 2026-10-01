#!/usr/bin/env python3
"""bise design system site: builds site/design/index.html (one file, hash routes) from content.py + shell.html.
python3 docs/brand/design-system/make.py  -> writes the page, prints any mock line whose width is off."""
import html, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../../.."))
OUT = os.path.join(ROOT, "site/design/index.html")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from content import PAGES, GROUPS  # noqa

BAD = []

# ---------- inline markdown ----------
def inline(t):
    parts = re.split(r"(`[^`]+`)", t)
    out = []
    for p in parts:
        if len(p) > 1 and p.startswith("`") and p.endswith("`"):
            out.append("<code>" + html.escape(p[1:-1]) + "</code>")
        else:
            e = html.escape(p, quote=False)
            e = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", e)
            e = re.sub(r"(?<![\w*])\*([^*]+?)\*(?![\w*])", r"<i>\1</i>", e)
            e = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", lambda m: f'<a href="{m.group(2)}"' + (' target="_blank" rel="noopener"' if m.group(2).startswith("http") else "") + f">{m.group(1)}</a>", e)
            out.append(e)
    return "".join(out)

# ---------- terminal mock lines ----------
TOK = re.compile(r"\{(ab|a|b|d|f|e|r|t|hl|ok|u|db|sel|ch|eb)\:((?:[^{}])*)\}|\{(g5|g3|g1)\}")
PAD = re.compile(r"\{([|>])(\d*)([^}]?)\}")

def vis(s):
    s = PAD.sub("", s)
    s = re.sub(r"\{g5\}", "xxxxx", s); s = re.sub(r"\{g3\}", "xxx", s); s = re.sub(r"\{g1\}", "x", s)
    s = TOK.sub(lambda m: m.group(2) or "", s)
    return len(s)

def mark(s):
    def rep(m):
        if m.group(3):
            n = {"g5": 5, "g3": 3, "g1": 1}[m.group(3)]
            return f'<span class="gust" data-n="{n}">' + ("∿" if n == 1 else "≈∿~· "[:n]) + "</span>"
        return f'<span class="c-{m.group(1)}">' + html.escape(m.group(2), quote=False) + "</span>"
    out, last = [], 0
    for m in TOK.finditer(s):
        out.append(html.escape(s[last:m.start()], quote=False)); out.append(rep(m)); last = m.end()
    out.append(html.escape(s[last:], quote=False))
    return "".join(out)

def line(s, W, where):
    cls = ""
    if s.startswith("^^"): cls, s = " raised2", s[2:]
    elif s.startswith("^"): cls, s = " raised", s[1:]
    parts = PAD.split(s)  # seg, kind, n, ch, seg, ...
    segs = parts[0::4]; pads = [(parts[i], parts[i + 1], parts[i + 2]) for i in range(1, len(parts), 4)]
    out, col = [mark(segs[0])], vis(segs[0])
    for i, (kind, n, ch) in enumerate(pads):
        ch = ch or " "; nxt = segs[i + 1]; L = vis(nxt)
        target = int(n) if n else W
        want = target if kind == "|" else target - L
        k = max(0, want - col)
        if want - col < 0: BAD.append((where, s, "overflow"))
        fill = ch * k
        out.append(f'<span class="c-r">{fill}</span>' if ch in "─│━" else fill)
        out.append(mark(nxt)); col += k + L
    if PAD.search(s) and col != W and not s.endswith("{~}"):
        BAD.append((where, s, f"width {col} != {W}"))
    return f'<div class="ln{cls}">' + "".join(out) + "</div>"

def term(block, where):
    m = re.match(r"::ex\s*(.*)", block[0]); args = m.group(1)
    W = int(re.search(r"w=(\d+)", args).group(1)) if "w=" in args else 72
    tabs, cap, cur = [], "", None
    for l in block[1:]:
        if l.startswith("::tab"):
            label, _, cls = l[5:].strip().partition("|")
            tw = re.search(r"w=(\d+)", cls); cls = re.sub(r"w=\d+", "", cls)
            cur = [label.strip(), cls.strip(), [], int(tw.group(1)) if tw else None]; tabs.append(cur)
        elif l.startswith("::cap"):
            cap = l[5:].strip()
        else:
            if cur is None: cur = ["", "", [], None]; tabs.append(cur)
            cur[2].append(l)
    tab_html = ""
    if len(tabs) > 1 or tabs[0][0]:
        tab_html = '<div class="tabs">' + "".join(f'<button class="{"on" if i == 0 else ""}" data-i="{i}">{html.escape(t[0])}</button>' for i, t in enumerate(tabs)) + "</div>"
    screens = "".join(
        f'<div class="screen {t[1]}{" on" if i == 0 else ""}" style="--w:{t[3] or W}">' + "".join(line(x, t[3] or W, where) for x in t[2]) + "</div>"
        for i, t in enumerate(tabs))
    return f'<figure class="ex">{tab_html}<div class="scr">{screens}</div>' + (f"<figcaption>{inline(cap)}</figcaption>" if cap else "") + "</figure>"

# ---------- page body ----------
def body(src, where):
    lines = src.strip("\n").split("\n"); out = []; i = 0; toc = []
    while i < len(lines):
        l = lines[i]
        if not l.strip(): i += 1; continue
        if l.startswith("::ex"):
            blk = [l]; i += 1
            while not lines[i].startswith("::end"): blk.append(lines[i]); i += 1
            i += 1; out.append(term(blk, where)); continue
        if l.startswith("::do"):
            do, dont, cur = [], [], None; i += 1
            while not lines[i].startswith("::end"):
                x = lines[i]
                if x.startswith("::dont"): cur = dont
                elif x.startswith("- "): (cur if cur is not None else do).append(x[2:])
                elif x.strip() and (cur if cur is not None else do): (cur if cur is not None else do)[-1] += " " + x.strip()
                i += 1
            i += 1
            out.append('<div class="dd"><div class="do"><div class="h"><span class="c-a">✓</span> do</div><ul>' + "".join(f"<li>{inline(x)}</li>" for x in do) +
                       '</ul></div><div class="dont"><div class="h"><span class="c-e">✗</span> don\'t</div><ul>' + "".join(f"<li>{inline(x)}</li>" for x in dont) + "</ul></div></div>")
            continue
        if l.startswith("::note"):
            out.append(f'<div class="note">{inline(l[6:].strip())}</div>'); i += 1; continue
        if l.startswith("::cards"):
            i += 1; cards = []
            while not lines[i].startswith("::end"):
                if lines[i].strip():
                    t, _, d = lines[i].partition("::"); h, _, href = t.partition("->")
                    cards.append(f'<a class="card" href="{href.strip()}"><b>{inline(h.strip())}</b><span>{inline(d.strip())}</span></a>')
                i += 1
            i += 1; out.append('<div class="cards">' + "".join(cards) + "</div>"); continue
        if l.startswith("## "):
            t = l[3:].strip(); sid = re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")
            toc.append((sid, t)); out.append(f'<h2 id="{where}--{sid}">{inline(t)}</h2>'); i += 1; continue
        if l.startswith("### "):
            out.append(f"<h3>{inline(l[4:])}</h3>"); i += 1; continue
        if l.startswith("|"):
            rows = []
            while i < len(lines) and lines[i].startswith("|"): rows.append(lines[i]); i += 1
            c = lambda r: [x.strip() for x in r.strip().strip("|").split("|")]
            out.append('<div class="tw"><table><tr>' + "".join(f"<th>{inline(x)}</th>" for x in c(rows[0])) + "</tr>" +
                       "".join("<tr>" + "".join(f"<td>{inline(x)}</td>" for x in c(r)) + "</tr>" for r in rows[2:]) + "</table></div>")
            continue
        if re.match(r"^(- |\d+\. )", l):
            ordered = l[0].isdigit(); items = []
            while i < len(lines) and (re.match(r"^(- |\d+\. )", lines[i]) or (lines[i].startswith("  ") and items)):
                x = lines[i]
                if re.match(r"^(- |\d+\. )", x): items.append(re.sub(r"^(- |\d+\. )", "", x))
                else: items[-1] += " " + x.strip()
                i += 1
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(x)}</li>" for x in items) + f"</{tag}>"); continue
        p = []
        while i < len(lines) and lines[i].strip() and not re.match(r"^(- |\d+\. |\||##|::)", lines[i]): p.append(lines[i].strip()); i += 1
        out.append(f"<p>{inline(' '.join(p))}</p>")
    return "\n".join(out), toc

def build():
    arts, nav = [], []
    order = []
    for g, items in GROUPS:
        nav.append(f'<div class="grp">{g}</div>')
        for pid in items:
            p = PAGES[pid]; order.append(pid)
            nav.append(f'<a href="#/{pid}" data-id="{pid}" data-k="{html.escape((p["title"] + " " + p.get("k", "")).lower())}">{html.escape(p["title"])}</a>')
    for n, pid in enumerate(order):
        p = PAGES[pid]; g = next(g for g, it in GROUPS if pid in it)
        b, toc = body(p["body"], pid.replace("/", "-"))
        prv = order[n - 1] if n else None; nxt = order[n + 1] if n + 1 < len(order) else None
        pn = '<div class="pn">' + (f'<a href="#/{prv}"><span>previous</span>{html.escape(PAGES[prv]["title"])}</a>' if prv else "<i></i>") + \
             (f'<a class="r" href="#/{nxt}"><span>next</span>{html.escape(PAGES[nxt]["title"])}</a>' if nxt else "") + "</div>"
        tochtml = "".join(f'<a href="#/{pid}--{s}" data-t="{pid.replace("/", "-")}--{s}">{html.escape(t)}</a>' for s, t in toc)
        src = f'<a class="src" href="https://github.com/gvergnaud/bise/blob/main/docs/brand/bise-book.md" target="_blank" rel="noopener">{html.escape(p["src"])} ↗</a>' if p.get("src") else ""
        arts.append(f'<article data-id="{pid}" hidden><div class="crumb">{g}{src}</div><h1>{inline(p["title"])}</h1>'
                    f'<p class="lead">{inline(p["lead"])}</p>{b}{pn}<template class="toc">{tochtml}</template></article>')
    tpl = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "shell.html"), encoding="utf-8").read()
    doc = tpl.replace("@@NAV@@", "\n".join(nav)).replace("@@ARTICLES@@", "\n".join(arts)).replace("@@FIRST@@", order[0])
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    # the dense reference for agents, served as text at bise.dev/design.md
    import shutil; shutil.copy(os.path.join(ROOT, "docs/brand/design-system.md"), os.path.join(ROOT, "site/design.md"))
    open(OUT, "w", encoding="utf-8").write(doc)
    print(len(order), "pages", len(doc), "bytes")
    for w, s, why in BAD: print("BAD", w, why, "|", s)

if __name__ == "__main__":
    build()
