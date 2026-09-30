"""tmux `capture-pane -e -p` output (SGR escapes) -> a standalone HTML page
that looks like the terminal: default colors of the pass (dark / light),
truecolor, 256 colors, bold, dim, italic, underline, reverse.

python3 ansi2html.py capture.ansi out.html dark|light "title"
"""
import html
import re
import sys

DEFAULTS = {"dark": ("#141211", "#ece6da"), "light": ("#f7f4ee", "#1b1917")}
BASE16 = ["#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
          "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff"]


def c256(n):
    if n < 16:
        return BASE16[n]
    if n < 232:
        n -= 16
        v = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (v[n // 36], v[(n // 6) % 6], v[n % 6])
    g = 8 + (n - 232) * 10
    return "#%02x%02x%02x" % (g, g, g)


SGR = re.compile(r"\x1b\[([0-9;:]*)m")


def convert(text, mode):
    bg0, fg0 = DEFAULTS[mode]
    st = {"fg": None, "bg": None, "b": False, "d": False, "i": False, "u": False, "r": False}
    out = []

    def span(s):
        if not s:
            return
        fg, bg = st["fg"], st["bg"]
        if st["r"]:
            fg, bg = (bg or bg0), (fg or fg0)
        css = []
        if fg:
            css.append("color:%s" % fg)
        if bg:
            css.append("background:%s" % bg)
        if st["b"]:
            css.append("font-weight:700")
        if st["d"]:
            css.append("opacity:.6")
        if st["i"]:
            css.append("font-style:italic")
        if st["u"]:
            css.append("text-decoration:underline")
        e = html.escape(s)
        out.append('<span style="%s">%s</span>' % (";".join(css), e) if css else e)

    for line in text.split("\n"):
        pos = 0
        for m in SGR.finditer(line):
            span(line[pos:m.start()])
            pos = m.end()
            ps = [p for p in re.split(r"[;:]", m.group(1))] or ["0"]
            i = 0
            while i < len(ps):
                p = int(ps[i] or 0)
                if p == 0:
                    st.update(fg=None, bg=None, b=False, d=False, i=False, u=False, r=False)
                elif p == 1:
                    st["b"] = True
                elif p == 2:
                    st["d"] = True
                elif p == 3:
                    st["i"] = True
                elif p == 4:
                    st["u"] = True
                elif p == 7:
                    st["r"] = True
                elif p == 22:
                    st["b"] = st["d"] = False
                elif p == 23:
                    st["i"] = False
                elif p == 24:
                    st["u"] = False
                elif p == 27:
                    st["r"] = False
                elif 30 <= p <= 37:
                    st["fg"] = BASE16[p - 30]
                elif 90 <= p <= 97:
                    st["fg"] = BASE16[p - 90 + 8]
                elif 40 <= p <= 47:
                    st["bg"] = BASE16[p - 40]
                elif 100 <= p <= 107:
                    st["bg"] = BASE16[p - 100 + 8]
                elif p == 39:
                    st["fg"] = None
                elif p == 49:
                    st["bg"] = None
                elif p in (38, 48):
                    k = "fg" if p == 38 else "bg"
                    if i + 1 < len(ps) and ps[i + 1] == "2" and i + 4 < len(ps):
                        st[k] = "#%02x%02x%02x" % tuple(int(x or 0) for x in ps[i + 2:i + 5])
                        i += 4
                    elif i + 1 < len(ps) and ps[i + 1] == "5" and i + 2 < len(ps):
                        st[k] = c256(int(ps[i + 2] or 0))
                        i += 2
                i += 1
        span(line[pos:])
        out.append("\n")
    return "".join(out), bg0, fg0


def page(text, mode, title):
    body, bg0, fg0 = convert(text, mode)
    return """<!doctype html><meta charset="utf-8"><title>%s</title>
<style>body{margin:0;background:%s}pre{margin:0;padding:12px 14px;background:%s;color:%s;
font:13px/1.35 "JetBrains Mono",Menlo,monospace;white-space:pre;display:inline-block}</style>
<pre>%s</pre>""" % (html.escape(title), bg0, bg0, fg0, body)


if __name__ == "__main__":
    src, dst, mode, title = sys.argv[1:5]
    open(dst, "w").write(page(open(src).read(), mode, title))
