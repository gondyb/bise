#!/usr/bin/env python3
"""Render the extracted system prompts as annotatable pages (local only, content/ is not deployed).
python3 content/prompts.py ~/Desktop/bise-prompts"""
import html, os, re, sys

SRC = os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else "~/Desktop/bise-prompts")
OUT = os.path.dirname(os.path.abspath(__file__))

def inline(t):
    parts = re.split(r"(`[^`]+`)", t)
    out = []
    for p in parts:
        if p.startswith("`") and p.endswith("`") and len(p) > 1:
            out.append("<code>" + html.escape(p[1:-1]) + "</code>")
        else:
            e = html.escape(p)
            e = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", e)
            e = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", r'<a href="\2">\1</a>', e)
            out.append(e)
    return "".join(out)

def render(md):
    lines, out, i = md.split("\n"), [], 0
    para = []
    def flush():
        if para:
            out.append("<p>" + "<br>".join(inline(l) for l in para) + "</p>")
            para.clear()
    while i < len(lines):
        l = lines[i]
        if l.lstrip().startswith("```"):
            flush(); ind = len(l) - len(l.lstrip()); lang = l.strip()[3:]; body = []; i += 1
            while i < len(lines) and not lines[i].lstrip().startswith("```"):
                body.append(lines[i][ind:] if lines[i][:ind].strip() == "" else lines[i]); i += 1
            out.append(f'<pre data-lang="{html.escape(lang)}"><code>' + html.escape("\n".join(body)) + "</code></pre>")
            i += 1; continue
        m = re.match(r"^(#{1,6})\s+(.*)$", l)
        if m:
            flush(); n = len(m.group(1)); out.append(f"<h{n}>{inline(m.group(2))}</h{n}>"); i += 1; continue
        if l.strip() == "---":
            flush(); out.append("<hr>"); i += 1; continue
        m = re.match(r"^(\s*)([-*]|\d+\.)\s+(.*)$", l)
        if m:
            flush(); ind = len(m.group(1)); mark = m.group(2); text = [m.group(3)]; i += 1
            while i < len(lines) and lines[i].strip() and not re.match(r"^\s*([-*]|\d+\.)\s+", lines[i]) and not lines[i].lstrip().startswith("```") and not lines[i].startswith("#"):
                text.append(lines[i].strip()); i += 1
            out.append(f'<li style="margin-left:{2 + ind}ch"><span class="mk">{html.escape(mark)}</span>' + "<br>".join(inline(t) for t in text) + "</li>")
            continue
        if not l.strip():
            flush(); i += 1; continue
        para.append(l); i += 1
    flush()
    return "\n".join(out)

CSS = """:root{--bg:#141211;--text:#ece6da;--dim:#8f887d;--faint:#3d3935;--acc:#f4a6b0;--raised:#221e1c}
body{margin:0;background:var(--bg);color:var(--text);font:14px/1.7 "JetBrains Mono",Menlo,monospace}
main{max-width:92ch;margin:0 auto;padding:40px 28px 160px}
.top{color:var(--dim);margin-bottom:28px}.top a{color:var(--acc)}
h1{font-size:22px;margin:36px 0 12px}h2{font-size:18px;margin:34px 0 10px;color:var(--acc)}h3{font-size:15px;margin:26px 0 8px}h4{font-size:14px}
p{margin:0 0 12px}li{list-style:none;margin-bottom:6px;text-indent:-2ch;padding-left:0}.mk{color:var(--dim);display:inline-block;min-width:2ch;text-indent:0}
code{background:var(--raised);padding:1px 4px;border-radius:4px;font-size:.95em}
pre{background:var(--raised);padding:14px 16px;border-radius:8px;overflow:auto;white-space:pre-wrap;word-break:break-word}pre code{background:none;padding:0}
hr{border:0;border-top:1px solid var(--faint);margin:28px 0}a{color:var(--acc)}"""

for name, page, title in [("main-system-prompt.md", "prompt-main", "system prompt · main"),
                          ("task-system-prompt.md", "prompt-task", "system prompt · a task")]:
    md = open(os.path.join(SRC, name), encoding="utf-8").read()
    other = "prompt-task" if page == "prompt-main" else "prompt-main"
    doc = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>{title}</title><style>{CSS}</style></head>
<body data-page="{page}"><main>
<div class="top">internal · local only · notes on every block (hover, then write) · <a href="{other}.html">{other.replace('prompt-', 'the other: ')}</a></div>
{render(md)}
</main><script src="/annotate.js"></script></body></html>
"""
    open(os.path.join(OUT, page + ".html"), "w", encoding="utf-8").write(doc)
    print(page, len(md), "chars")
