#!/usr/bin/env python3
"""Builds the README's animated SVGs (dark + light). Run: python3 make.py
The SVGs are shown on GitHub as <img>: no JS, no web fonts. CSS keyframes only."""
from html import escape as E
import os
HERE = os.path.dirname(os.path.abspath(__file__))
PAL = {
  "dark":  dict(bg="#141211", text="#ece6da", dim="#8f887d", faint="#3d3935", wind="#3f3a36", acc="#f4a6b0", raised="#1f1c1a", chip="#2a2522", line="#2c2825"),
  "light": dict(bg="#f7f4ee", text="#1b1917", dim="#77706a", faint="#d6cfc4", wind="#ddd5c8", acc="#b8416b", raised="#efe9df", chip="#e4ddd2", line="#e2dbcf"),
}
MONO = "ui-monospace, SFMono-Regular, 'JetBrains Mono', Menlo, Consolas, 'Liberation Mono', monospace"
BASE_CSS = f"text{{font-family:{MONO};}} @media (prefers-reduced-motion: reduce){{*{{animation:none!important}}}}"

def svg(w, h, body, css, label):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-label="{E(label)}">'
            f'<style>{BASE_CSS}{css}</style>{body}</svg>\n')

class TL:
    """Timeline: each element fades in at t and stays until the loop fades out."""
    def __init__(s, T): s.T, s.css, s.n = T, [], 0
    def show(s, t, out=None, dur=0.35):
        s.n += 1; k = f"k{s.n}"; T = s.T; out = out if out is not None else T - 0.8
        p = lambda x: f"{max(0, min(100, x / T * 100)):.2f}%"
        s.css.append(f"@keyframes {k}{{0%,{p(t)}{{opacity:0}}{p(t+dur)},{p(out)}{{opacity:1}}{p(out+dur)},100%{{opacity:0}}}}"
                     f".{k}{{opacity:0;animation:{k} {T}s linear infinite}}")
        return k
    def reveal(s, t, t2, w):
        """a clip rect that grows from 0 to w between t and t2 (typing), gone at the send."""
        s.n += 1; k = f"r{s.n}"; T = s.T
        p = lambda x: f"{x / T * 100:.2f}%"
        s.css.append(f"@keyframes {k}{{0%,{p(t)}{{width:0}}{p(t2)},100%{{width:{w}px}}}}.{k}{{animation:{k} {T}s steps(40,end) infinite}}")
        return k

# ---------- hero ----------
def hero(c):
    W, H = 960, 340
    rows, css = [], []
    pat = ["~    ∿      ·     ~   ", "  ·    ~       ∿    ·  ", "∿     ·   ~        ~   ", "   ~      ·    ∿     · "]
    for i in range(9):
        y = 30 + i * 34
        t = pat[i % 4] * 6
        speed = 40 + (i * 13) % 30
        css.append(f"@keyframes w{i}{{from{{transform:translateX(0)}}to{{transform:translateX(-{W}px)}}}}.w{i}{{animation:w{i} {speed}s linear infinite}}")
        rows.append(f'<text class="w{i}" x="0" y="{y}" fill="{c["wind"]}" font-size="15" textLength="{2*W}" lengthAdjust="spacing" xml:space="preserve">{E(t+t)}</text>')
    kisses = []
    for i, (x, y, d) in enumerate([(150, 70, 0), (800, 95, 2.2), (690, 280, 4.1), (230, 262, 5.6), (860, 230, 7.3)]):
        css.append(f"@keyframes q{i}{{0%,{d/9*100:.1f}%{{opacity:0;transform:translateY(0)}}{(d+0.6)/9*100:.1f}%{{opacity:.9}}{(d+2.2)/9*100:.1f}%,100%{{opacity:0;transform:translateY(-14px)}}}}.q{i}{{opacity:0;animation:q{i} 9s ease-out infinite}}")
        kisses.append(f'<text class="q{i}" x="{x}" y="{y}" fill="{c["acc"]}" font-size="16">:*</text>')
    css.append("@keyframes kiss{0%,86%,100%{opacity:1}90%{opacity:.25}94%{opacity:1}}.kiss{animation:kiss 5s infinite}")
    body = (f'<rect width="{W}" height="{H}" rx="18" fill="{c["bg"]}"/><g>{"".join(rows)}</g>{"".join(kisses)}'
            f'<rect x="250" y="92" width="460" height="160" rx="14" fill="{c["bg"]}" opacity=".82"/>'
            f'<text x="480" y="170" text-anchor="middle" font-size="76" font-weight="700" fill="{c["text"]}" xml:space="preserve">bise <tspan class="kiss" fill="{c["acc"]}">:*</tspan></text>'
            f'<text x="480" y="212" text-anchor="middle" font-size="22" fill="{c["text"]}">multi-agent coding, made human.</text>'
            f'<text x="480" y="242" text-anchor="middle" font-size="14" fill="{c["dim"]}">meet your team lead. you stay in flow, i run the agents.</text>')
    return svg(W, H, body, "".join(css), "bise :* multi-agent coding, made human.")

# ---------- demo ----------
# the same story as the landing's flow demo (site/index.html, run()): five ideas in a row,
# the team syncs, you change your mind, one card, everything ships.
def demo(c):
    W, T = 960, 31
    tl = TL(T); out = []; feed = []; marks = []  # marks: (time, baseline) of each feed row, for the scroll
    fx, lh = 40, 29
    acc = lambda t: f'<tspan fill="{c["acc"]}">{t}</tspan>'
    y = [76]
    def line(t, html, gap=0, color="text", size=14):
        y[0] += gap
        k = tl.show(t); marks.append((t, y[0])); feed.append(f'<text class="{k}" x="{fx}" y="{y[0]}" font-size="{size}" fill="{c[color]}" xml:space="preserve">{html}</text>'); y[0] += lh; return k
    def you(t, s, gap=14):
        y[0] += gap
        k = tl.show(t); r = tl.show(t + 0.55, dur=0.1); marks.append((t, y[0]))
        feed.append(f'<g class="{k}"><rect x="{fx-16}" y="{y[0]-15}" width="3" height="19" fill="{c["acc"]}"/>'
                   f'<text x="{fx}" y="{y[0]}" font-size="14" font-weight="700" fill="{c["text"]}" xml:space="preserve">{E(s)} <tspan font-weight="400" fill="{c["faint"]}">✓</tspan></text></g>'
                   f'<text class="{r}" x="{fx + int((len(s) + sum(ord(ch) > 0xFFFF for ch in s) + 1) * 8.45)}" y="{y[0]}" font-size="14" fill="{c["acc"]}">✓✓</text>')
        y[0] += lh
    main = lambda t, s: line(t, f'{acc(":*")} {E(s)}')
    msgs = []  # (typing start, typing end, send, text): drawn in the composer once its y is known
    def say(t, t2, send, text, gap=14):
        msgs.append((t, t2, send, text)); you(send, text, gap)
    say(0.8, 2.6, 2.8, "signup is slow on mobile. can you look?", gap=0)
    main(3.4, "on it. perf is profiling it on a mid-range phone.")
    say(4.0, 5.2, 5.4, "oh and dark mode. people keep asking")
    main(5.9, "dark-mode started. settings page first, then the rest.")
    say(6.3, 7.3, 7.5, "and the 404 page is so sad. make it less sad")
    main(7.9, "cheering it up. sad-404 started.")
    say(8.2, 9.0, 9.2, "wait also the csv export crashes on emoji 😭")
    main(9.6, "emoji-csv is on it. it's always unicode.")
    say(9.9, 10.6, 10.8, "OK LAST ONE. release notes for all of this")
    main(11.3, "release waits for the others, then writes them. go get a coffee.")
    line(12.8, "▸ 9 messages between 5 agents", gap=14, color="dim", size=13)
    main(13.6, "dark-mode asked which gray. i said the one in tokens.css.")
    say(14.4, 15.8, 16.0, "actually keep the sad dog on the 404. just give it a hat")
    main(16.6, "told sad-404. the dog keeps its job. now with a hat.")
    # the card: it asks, you press 2, it folds to one answered line
    y[0] += 18; cy0 = y[0]
    k = tl.show(17.8, 19.9, dur=0.2); a2 = tl.show(19.9, dur=0.2)
    marks.append((17.8, cy0 + 44))
    feed.append(f'<g class="{k}"><rect x="{fx-16}" y="{cy0-16}" width="600" height="68" rx="4" fill="{c["raised"]}"/><rect x="{fx-16}" y="{cy0-16}" width="3" height="68" fill="{c["acc"]}"/>'
               f'<text x="{fx}" y="{cy0}" font-size="14" font-weight="700" fill="{c["acc"]}">? perf needs you</text>'
               f'<text x="{fx}" y="{cy0+21}" font-size="14" fill="{c["text"]}">the hero image is 4.2 MB. compress it, or lazy-load it?</text>'
               f'<text x="{fx}" y="{cy0+42}" font-size="13" fill="{c["dim"]}" xml:space="preserve">1 · compress   2 · both   alt+r answer with text</text></g>'
               f'<text class="{a2}" x="{fx}" y="{cy0}" font-size="14" fill="{c["dim"]}">{acc("✓")} perf · you said both</text>')
    y[0] = cy0 + lh
    done = [(21.0, "emoji-csv done", "🦄 exports fine now. the file was read as latin-1."),
            (22.2, "perf done", "signup: 4.1 s → 0.9 s on a mid-range phone."),
            (23.4, "sad-404 and dark-mode done", "the dog has a hat. settings is dark."),
            (24.8, "release done", "2.5 notes drafted. five things, zero tabs.")]
    for i, (t, a, b) in enumerate(done):
        line(t, f'{acc("✓")} {a} <tspan fill="{c["dim"]}">· {E(b)}</tspan>', gap=14 if i == 0 else 0)
    say(25.4, 26.2, 26.4, "you're the best")
    main(27.0, ":*")
    # the frame, sized to the story
    H = 600; top = H - 150; view = top - 14  # the feed shows rows 49..view, then scrolls
    css_scroll, cur, frames = [], 0, ["0%{transform:translateY(0)}"]
    pc = lambda x: f"{x / T * 100:.2f}%"
    for t, yb in sorted(marks):
        need = max(0, yb + 8 - view)
        if need > cur:
            frames.append(f"{pc(t)}{{transform:translateY(-{cur}px);animation-timing-function:ease-out}}{pc(t + 0.45)}{{transform:translateY(-{need}px)}}")
            cur = need
    frames.append(f"{pc(T - 0.2)}{{transform:translateY(-{cur}px)}}100%{{transform:translateY(0)}}")
    css_scroll = f"@keyframes scr{{{''.join(frames)}}}.scr{{animation:scr {T}s linear infinite}}"
    out.append(f'<clipPath id="feedclip"><rect x="0" y="49" width="679" height="{top - 49}"/></clipPath>'
               f'<g clip-path="url(#feedclip)"><g class="scr">{"".join(feed)}</g></g>')
    out.insert(0, f'<rect width="{W}" height="{H}" rx="14" fill="{c["bg"]}" stroke="{c["line"]}"/>'
               f'<text x="24" y="32" font-size="14" fill="{c["text"]}" font-weight="700">bise {acc(":*")}</text>'
               f'<text x="{W-24}" y="32" text-anchor="end" font-size="13" fill="{c["dim"]}">~/acme</text>'
               f'<line x1="0" y1="48" x2="{W}" y2="48" stroke="{c["line"]}"/>'
               f'<line x1="680" y1="48" x2="680" y2="{top}" stroke="{c["line"]}"/>'
               f'<text x="700" y="76" font-size="12" fill="{c["dim"]}">agents · ⌥ + number</text>'
               f'<text x="700" y="102" font-size="14" fill="{c["dim"]}">0 {acc(":*")} <tspan fill="{c["text"]}">main</tspan></text>')
    # the panel: each agent's ∿ turns into a ✓ when it ships
    for n, (name, t0, t1) in enumerate([("perf", 3.5, 22.2), ("dark-mode", 6.0, 23.4), ("sad-404", 8.0, 23.4),
                                         ("emoji-csv", 9.7, 21.0), ("release", 11.4, 24.8)], 1):
        yy = 102 + n * 24; k = tl.show(t0)
        out.append(f'<text class="{k}" x="700" y="{yy}" font-size="14" fill="{c["dim"]}" xml:space="preserve">{n} <tspan class="g" fill="{c["acc"]}">∿</tspan> <tspan fill="{c["text"]}">{name}</tspan></text>')
        d = tl.show(t1)
        out.append(f'<rect class="{d}" x="709" y="{yy-15}" width="17" height="20" fill="{c["bg"]}"/><text class="{d}" x="713" y="{yy}" font-size="14" fill="{c["acc"]}">✓</text>')
    # the composer
    out.append(f'<rect x="1" y="{top}" width="{W-2}" height="{H-top-1}" fill="{c["raised"]}"/>'
               f'<path d="M1 {top} H{W-1}" stroke="{c["faint"]}"/>'
               f'<text x="24" y="{top+26}" font-size="13" fill="{c["dim"]}">you → <tspan fill="{c["acc"]}" font-weight="700">main</tspan></text>'
               f'<text x="24" y="{top+102}" font-size="12" fill="{c["faint"]}">⏎ send · @ agent · ⌥0-9 switch · / commands · ? help</text>')
    for n, (t0, t1) in [(1, (3.5, 6.0)), (2, (6.0, 8.0)), (3, (8.0, 9.7)), (4, (9.7, 11.4)), (5, (11.4, 21.0)),
                        (4, (21.0, 22.2)), (3, (22.2, 23.4)), (1, (23.4, 24.8))]:
        k = tl.show(t0, t1, dur=0.15)
        out.append(f'<text class="{k}" x="{W-24}" y="{top+26}" text-anchor="end" font-size="13" fill="{c["dim"]}"><tspan class="g" fill="{c["acc"]}">∿</tspan> {n} working</text>')
    ph = tl.show(0, 0.8, dur=0.1)
    out.append(f'<text class="{ph}" x="24" y="{top+62}" font-size="15" fill="{c["faint"]}">what\'s on your mind?</text>')
    for (t, t2, send, text) in msgs:
        w = int(len(text) * 9.1) + 10
        r = tl.reveal(t, t2, w); vis = tl.show(t - 0.05, send, dur=0.05); cid = f"c{tl.n}"
        out.append(f'<clipPath id="{cid}"><rect class="{r}" x="24" y="{top+40}" height="30" width="0"/></clipPath>'
                   f'<g class="{vis}"><text x="24" y="{top+62}" font-size="15" fill="{c["text"]}" clip-path="url(#{cid})">{E(text)}</text></g>')
    css_gust = "@keyframes g{0%{opacity:.3}50%{opacity:1}100%{opacity:.3}}.g{animation:g 1.2s infinite}"
    return svg(W, H, "".join(out), "".join(tl.css) + css_gust + css_scroll,
               "a bise session: five ideas in a row to main, five agents start, they sync, you change your mind, one card asks you, everything ships.")

# ---------- demo ----------
# the same story as the landing's flow demo (site/index.html, run()): five ideas in a row,
# the team syncs, you change your mind, one card, everything ships.
def demo_b(c):
    W, T = 960, 36
    tl = TL(T); out = []; feed = []; marks = []  # marks: (time, baseline) of each feed row, for the scroll
    fx, lh = 40, 29
    acc = lambda t: f'<tspan fill="{c["acc"]}">{t}</tspan>'
    y = [76]
    def line(t, html, gap=0, color="text", size=14):
        y[0] += gap
        k = tl.show(t); marks.append((t, y[0])); feed.append(f'<text class="{k}" x="{fx}" y="{y[0]}" font-size="{size}" fill="{c[color]}" xml:space="preserve">{html}</text>'); y[0] += lh; return k
    def you(t, s, gap=14):
        y[0] += gap
        k = tl.show(t); r = tl.show(t + 0.55, dur=0.1); marks.append((t, y[0]))
        feed.append(f'<g class="{k}"><rect x="{fx-16}" y="{y[0]-15}" width="3" height="19" fill="{c["acc"]}"/>'
                   f'<text x="{fx}" y="{y[0]}" font-size="14" font-weight="700" fill="{c["text"]}" xml:space="preserve">{E(s)} <tspan font-weight="400" fill="{c["faint"]}">✓</tspan></text></g>'
                   f'<text class="{r}" x="{fx + int((len(s) + sum(ord(ch) > 0xFFFF for ch in s) + 1) * 8.45)}" y="{y[0]}" font-size="14" fill="{c["acc"]}">✓✓</text>')
        y[0] += lh
    main = lambda t, s: line(t, f'{acc(":*")} {E(s)}')
    msgs = []  # (typing start, typing end, send, text): drawn in the composer once its y is known
    def say(t, t2, send, text, gap=14):
        msgs.append((t, t2, send, text)); you(send, text, gap)
    tool = lambda t, g, d: line(t, f'<tspan fill="{c["faint"]}">{g}</tspan> <tspan fill="{c["dim"]}">{E(d)}</tspan> {acc("✓")}', color="dim", size=13)
    def chip(t, a_, b_, text):
        y[0] += 6; k = tl.show(t); marks.append((t, y[0] + 22))
        w = int((len(a_) + len(b_) + 6) * 7.3) + 16
        feed.append(f'<g class="{k}"><rect x="{fx-4}" y="{y[0]-14}" width="{w}" height="20" rx="4" fill="{c["chip"]}"/>'
                    f'<text x="{fx+4}" y="{y[0]}" font-size="12" fill="{c["dim"]}" xml:space="preserve">✉ <tspan font-weight="700" fill="{c["text"]}">{a_}</tspan> <tspan fill="{c["faint"]}">→</tspan> {b_}</text>'
                    f'<text x="{fx+12}" y="{y[0]+22}" font-size="13" fill="{c["dim"]}">{E(text)}</text></g>')
        y[0] += 50
    say(0.8, 2.6, 2.8, "signup is slow on mobile. can you look?", gap=0)
    main(3.4, "on it. perf is profiling it on a mid-range phone.")
    tool(4.2, "ƒ", "perf · reading the Sentry trace")
    say(4.6, 5.8, 6.0, "oh and dark mode. people keep asking")
    main(6.5, "dark-mode started. settings page first, then the rest.")
    say(6.9, 7.9, 8.1, "and the 404 page is so sad. make it less sad")
    main(8.6, "cheering it up. sad-404 started.")
    chip(9.4, "dark-mode", "sad-404", "the 404 uses the old gray. i'm switching it, don't.")
    say(10.0, 10.9, 11.1, "wait also the csv export crashes on emoji 😭")
    main(11.6, "emoji-csv is on it. it's always unicode.")
    tool(12.4, "$", "emoji-csv · running the export test")
    line(13.4, "⌥1 · now talking to perf", gap=10, color="dim", size=13)
    say(13.9, 14.8, 15.0, "why 0.9 s and not 0.5?", gap=4)
    line(15.8, f'<tspan fill="{c["dim"]}">@ perf:</tspan> the fonts block the first paint. preloading them.')
    line(16.8, "⌥0 · back to main", gap=6, color="dim", size=13)
    main(17.4, "dark-mode asked which gray. i said the one in tokens.css.")
    # the card: it asks, you press 2, it folds to one answered line
    y[0] += 18; cy0 = y[0]
    k = tl.show(19.6, 22.0, dur=0.2); a2 = tl.show(22.0, dur=0.2)
    marks.append((19.6, cy0 + 44))
    feed.append(f'<g class="{k}"><rect x="{fx-16}" y="{cy0-16}" width="600" height="68" rx="4" fill="{c["raised"]}"/><rect x="{fx-16}" y="{cy0-16}" width="3" height="68" fill="{c["acc"]}"/>'
               f'<text x="{fx}" y="{cy0}" font-size="14" font-weight="700" fill="{c["acc"]}">? perf needs you</text>'
               f'<text x="{fx}" y="{cy0+21}" font-size="14" fill="{c["text"]}">the hero image is 4.2 MB. compress it, or lazy-load it?</text>'
               f'<text x="{fx}" y="{cy0+42}" font-size="13" fill="{c["dim"]}" xml:space="preserve">1 · compress   2 · both   alt+r answer with text</text></g>'
               f'<text class="{a2}" x="{fx}" y="{cy0}" font-size="14" fill="{c["dim"]}">{acc("✓")} perf · you said both</text>')
    y[0] = cy0 + lh
    done = [(23.4, "emoji-csv done", "🦄 exports fine now. the file was read as latin-1."),
            (24.6, "perf done", "signup: 4.1 s → 0.9 s on a mid-range phone."),
            (25.8, "sad-404 and dark-mode done", "the dog has a hat. settings is dark."),
            (27.0, "all four done", "four things, zero tabs.")]
    for i, (t, a, b) in enumerate(done):
        line(t, f'{acc("✓")} {a} <tspan fill="{c["dim"]}">· {E(b)}</tspan>', gap=14 if i == 0 else 0)
    say(28.0, 28.8, 29.0, "you're the best")
    main(29.6, ":*")
    # the frame, sized to the story
    H = 600; top = H - 150; view = top - 14  # the feed shows rows 49..view, then scrolls
    css_scroll, cur, frames = [], 0, ["0%{transform:translateY(0)}"]
    pc = lambda x: f"{x / T * 100:.2f}%"
    for t, yb in sorted(marks):
        need = max(0, yb + 8 - view)
        if need > cur:
            frames.append(f"{pc(t)}{{transform:translateY(-{cur}px);animation-timing-function:ease-out}}{pc(t + 0.45)}{{transform:translateY(-{need}px)}}")
            cur = need
    frames.append(f"{pc(T - 0.2)}{{transform:translateY(-{cur}px)}}100%{{transform:translateY(0)}}")
    css_scroll = f"@keyframes scr{{{''.join(frames)}}}.scr{{animation:scr {T}s linear infinite}}"
    out.append(f'<clipPath id="feedclip"><rect x="0" y="49" width="679" height="{top - 49}"/></clipPath>'
               f'<g clip-path="url(#feedclip)"><g class="scr">{"".join(feed)}</g></g>')
    out.insert(0, f'<rect width="{W}" height="{H}" rx="14" fill="{c["bg"]}" stroke="{c["line"]}"/>'
               f'<text x="24" y="32" font-size="14" fill="{c["text"]}" font-weight="700">bise {acc(":*")}</text>'
               f'<text x="{W-24}" y="32" text-anchor="end" font-size="13" fill="{c["dim"]}">~/acme</text>'
               f'<line x1="0" y1="48" x2="{W}" y2="48" stroke="{c["line"]}"/>'
               f'<line x1="680" y1="48" x2="680" y2="{top}" stroke="{c["line"]}"/>'
               f'<text x="700" y="76" font-size="12" fill="{c["dim"]}">agents · ⌥ + number</text>'
               f'<text x="700" y="102" font-size="14" fill="{c["dim"]}">0 {acc(":*")} <tspan fill="{c["text"]}">main</tspan></text>')
    # the panel: each agent's ∿ turns into a ✓ when it ships
    for n, (name, t0, t1) in enumerate([("perf", 3.5, 24.6), ("dark-mode", 6.6, 25.8), ("sad-404", 8.7, 25.8),
                                         ("emoji-csv", 11.7, 23.4)], 1):
        yy = 102 + n * 24; k = tl.show(t0)
        out.append(f'<text class="{k}" x="700" y="{yy}" font-size="14" fill="{c["dim"]}" xml:space="preserve">{n} <tspan class="g" fill="{c["acc"]}">∿</tspan> <tspan fill="{c["text"]}">{name}</tspan></text>')
        d = tl.show(t1)
        out.append(f'<rect class="{d}" x="709" y="{yy-15}" width="17" height="20" fill="{c["bg"]}"/><text class="{d}" x="713" y="{yy}" font-size="14" fill="{c["acc"]}">✓</text>')
    # the composer
    out.append(f'<rect x="1" y="{top}" width="{W-2}" height="{H-top-1}" fill="{c["raised"]}"/>'
               f'<path d="M1 {top} H{W-1}" stroke="{c["faint"]}"/>'
               f'<text class="{tl.show(0, 13.4, dur=0.1)}" x="24" y="{top+26}" font-size="13" fill="{c["dim"]}">you → <tspan fill="{c["acc"]}" font-weight="700">main</tspan></text>'
               f'<text class="{tl.show(13.4, 16.8, dur=0.1)}" x="24" y="{top+26}" font-size="13" fill="{c["dim"]}">you → <tspan fill="{c["acc"]}" font-weight="700">perf</tspan></text>'
               f'<text class="{tl.show(16.8, dur=0.1)}" x="24" y="{top+26}" font-size="13" fill="{c["dim"]}">you → <tspan fill="{c["acc"]}" font-weight="700">main</tspan></text>'
               f'<text x="24" y="{top+102}" font-size="12" fill="{c["faint"]}">⏎ send · @ agent · ⌥0-9 switch · / commands · ? help</text>')
    for n, (t0, t1) in [(1, (3.5, 6.6)), (2, (6.6, 8.7)), (3, (8.7, 11.7)), (4, (11.7, 23.4)),
                        (3, (23.4, 24.6)), (2, (24.6, 25.8))]:
        k = tl.show(t0, t1, dur=0.15)
        out.append(f'<text class="{k}" x="{W-24}" y="{top+26}" text-anchor="end" font-size="13" fill="{c["dim"]}"><tspan class="g" fill="{c["acc"]}">∿</tspan> {n} working</text>')
    ph = tl.show(0, 0.8, dur=0.1)
    out.append(f'<text class="{ph}" x="24" y="{top+62}" font-size="15" fill="{c["faint"]}">what\'s on your mind?</text>')
    for (t, t2, send, text) in msgs:
        w = int(len(text) * 9.1) + 10
        r = tl.reveal(t, t2, w); vis = tl.show(t - 0.05, send, dur=0.05); cid = f"c{tl.n}"
        out.append(f'<clipPath id="{cid}"><rect class="{r}" x="24" y="{top+40}" height="30" width="0"/></clipPath>'
                   f'<g class="{vis}"><text x="24" y="{top+62}" font-size="15" fill="{c["text"]}" clip-path="url(#{cid})">{E(text)}</text></g>')
    css_gust = "@keyframes g{0%{opacity:.3}50%{opacity:1}100%{opacity:.3}}.g{animation:g 1.2s infinite}"
    return svg(W, H, "".join(out), "".join(tl.css) + css_gust + css_scroll,
               "a bise session: four ideas in a row, agents call tools and message each other, you ask one agent directly, one card asks you, everything ships.")

# ---------- team ----------
def team(c):
    W, H, T = 960, 300, 10
    tl = TL(T); o = [f'<rect width="{W}" height="{H}" rx="14" fill="{c["bg"]}" stroke="{c["line"]}"/>']
    o.append(f'<text x="60" y="56" font-size="16" font-weight="700" fill="{c["text"]}">you</text>'
             f'<text x="140" y="56" font-size="14" fill="{c["dim"]}">"dark mode please. also the csv export crashes on emoji 😭"</text>'
             f'<path d="M72 68 V104" stroke="{c["faint"]}"/>'
             f'<text x="60" y="126" font-size="16" font-weight="700" fill="{c["text"]}"><tspan fill="{c["acc"]}">:*</tspan> main</text>'
             f'<text x="190" y="126" font-size="14" fill="{c["dim"]}">always listening. knows everything going on in the repo.</text>'
             f'<path d="M72 138 V262 M72 166 H96 M72 196 H96 M72 226 H96 M72 256 H96" stroke="{c["faint"]}" fill="none"/>')
    jobs = [("theme", "the dark colors, in one place"), ("toggle", "the switch in settings, remembered per user"),
            ("docs", "new screenshots, a changelog line"), ("emoji-csv", "the export fix. unrelated, so it runs too.")]
    o.append('<circle r="4" fill="%s"><animateMotion dur="%ss" repeatCount="indefinite" keyPoints="0;1;1" keyTimes="0;.12;1" path="M72 70 V110"/></circle>' % (c["acc"], T))
    for i, (n, d) in enumerate(jobs):
        yy = 170 + i * 30; k = tl.show(1.4 + i * 0.5)
        o.append(f'<text class="{k}" x="106" y="{yy}" font-size="14" fill="{c["text"]}" xml:space="preserve"><tspan class="g" fill="{c["acc"]}">∿</tspan> {n}</text>'
                 f'<text class="{k}" x="230" y="{yy}" font-size="14" fill="{c["dim"]}">{E(d)}</text>')
        dn = tl.show(5.0 + i * 0.9)
        o.append(f'<rect class="{dn}" x="104" y="{yy-15}" width="12" height="19" fill="{c["bg"]}"/><text class="{dn}" x="106" y="{yy}" font-size="14" fill="{c["acc"]}">✓</text>')
    k = tl.show(3.4, 5.6)
    o.append(f'<g class="{k}"><rect x="600" y="182" width="290" height="44" rx="4" fill="{c["chip"]}"/>'
             f'<text x="612" y="200" font-size="12" fill="{c["dim"]}">✉ <tspan font-weight="700" fill="{c["text"]}">theme</tspan> <tspan fill="{c["faint"]}">→</tspan> toggle</text>'
             f'<text x="624" y="218" font-size="12" fill="{c["dim"]}">the color tokens are in theme.ts</text></g>')
    css = "@keyframes g{0%{opacity:.3}50%{opacity:1}100%{opacity:.3}}.g{animation:g 1.2s infinite}"
    return svg(W, H, "".join(o), "".join(tl.css) + css, "you talk to main; main splits one idea into four jobs; the agents tell each other what they touch.")


# ---------- small feature demos (like the landing's minis) ----------
CW = 8.45  # width of one 14px mono cell

class Mini:
    def __init__(s, c, T, H=250, right=""):
        s.c, s.T, s.H, s.W = c, T, H, 680
        s.tl = TL(T); s.o = [f'<rect width="{s.W}" height="{H}" rx="12" fill="{c["bg"]}" stroke="{c["line"]}"/>']
        s.fx, s.y = 28, 44
        if right: s.o.append(f'<text x="{s.W-24}" y="30" text-anchor="end" font-size="12" fill="{c["dim"]}">{right}</text>')
    def acc(s, t): return f'<tspan fill="{s.c["acc"]}">{t}</tspan>'
    def dim(s, t): return f'<tspan fill="{s.c["dim"]}">{t}</tspan>'
    def row(s, t, html, color="text", size=14, gap=0, out=None, x=None, y=None):
        if y is None: s.y += gap; yy = s.y; s.y += 27
        else: yy = y
        k = s.tl.show(t, out)
        s.o.append(f'<text class="{k}" x="{x or s.fx}" y="{yy}" font-size="{size}" fill="{s.c[color]}" xml:space="preserve">{html}</text>')
        return yy
    def you(s, t, text, gap=8, read=0.6, to=""):
        s.y += gap; yy = s.y; s.y += 27
        k = s.tl.show(t); r = s.tl.show(t + read, dur=0.1)
        s.o.append(f'<g class="{k}"><rect x="{s.fx-14}" y="{yy-15}" width="3" height="19" fill="{s.c["acc"]}"/>'
                   f'<text x="{s.fx}" y="{yy}" font-size="14" font-weight="700" fill="{s.c["text"]}" xml:space="preserve">{E(text)} <tspan font-weight="400" fill="{s.c["faint"]}">✓</tspan></text></g>'
                   f'<text class="{r}" x="{s.fx + int((len(text) + 1) * CW)}" y="{yy}" font-size="14" fill="{s.c["acc"]}">✓✓</text>')
    def chip(s, t, a, b, text, gap=6):
        s.y += gap; yy = s.y; k = s.tl.show(t)
        w = int((len(a) + len(b) + 6) * 7.3) + 16
        s.o.append(f'<g class="{s.tl.show(t)}"><rect x="{s.fx-4}" y="{yy-14}" width="{w}" height="20" rx="4" fill="{s.c["chip"]}"/>'
                   f'<text x="{s.fx+4}" y="{yy}" font-size="12" fill="{s.c["dim"]}" xml:space="preserve">✉ <tspan font-weight="700" fill="{s.c["text"]}">{a}</tspan> <tspan fill="{s.c["faint"]}">→</tspan> {b}</text>'
                   f'<text x="{s.fx+12}" y="{yy+22}" font-size="13" fill="{s.c["dim"]}">{E(text)}</text></g>')
        s.y += 50
    def swap(s, t0, t1, x, y, a_html, b_html, size=14, a_color="text"):
        """a_html until t1, then b_html (e.g. ∿ → ✓)."""
        k1 = s.tl.show(t0, t1, dur=0.15); k2 = s.tl.show(t1, dur=0.15)
        s.o.append(f'<text class="{k1}" x="{x}" y="{y}" font-size="{size}" fill="{s.c[a_color]}" xml:space="preserve">{a_html}</text>'
                   f'<text class="{k2}" x="{x}" y="{y}" font-size="{size}" fill="{s.c[a_color]}" xml:space="preserve">{b_html}</text>')
    def composer(s, spans, typing=()):
        """spans: [(t0, t1, who_html)] for the 'you → x' label; typing: [(t, t2, send, text)]"""
        c, W, H = s.c, s.W, s.H; top = H - 62
        s.o.append(f'<rect x="1" y="{top}" width="{W-2}" height="{H-top-1}" rx="0" fill="{c["raised"]}"/><path d="M1 {top} H{W-1}" stroke="{c["faint"]}"/>'
                   f'<rect x="1" y="{H-12}" width="{W-2}" height="11" rx="11" fill="{c["raised"]}"/>')
        for (t0, t1, who) in spans:
            k = s.tl.show(t0, t1, dur=0.12)
            s.o.append(f'<text class="{k}" x="24" y="{top+22}" font-size="12" fill="{c["dim"]}">you → {who}</text>')
        ends = [0] + [send for (_, _, send, _) in typing]
        starts = [t for (t, _, _, _) in typing] + [s.T]
        for a, b in zip(ends, starts):
            if b - a > 0.3:
                k = s.tl.show(a + 0.1, b, dur=0.1)
                s.o.append(f'<text class="{k}" x="24" y="{top+46}" font-size="14" fill="{c["faint"]}">what\'s on your mind?</text>')
        for (t, t2, send, text) in typing:
            w = int(len(text) * CW) + 10
            r = s.tl.reveal(t, t2, w); vis = s.tl.show(t - 0.05, send, dur=0.05); cid = f"c{s.tl.n}"
            s.o.append(f'<clipPath id="{cid}"><rect class="{r}" x="24" y="{top+28}" height="26" width="0"/></clipPath>'
                       f'<g class="{vis}"><text x="24" y="{top+46}" font-size="14" fill="{c["text"]}" clip-path="url(#{cid})">{E(text)}</text></g>')
    def svg(s, label):
        css = "@keyframes g{0%{opacity:.3}50%{opacity:1}100%{opacity:.3}}.g{animation:g 1.2s infinite}"
        return svg(s.W, s.H, "".join(s.o), "".join(s.tl.css) + css, label)

def f_talk(c):
    m = Mini(c, 11, 250, "∿ = working")
    m.you(0.9, "signup is slow on mobile. can you look?", gap=0)
    m.row(1.6, f'{m.acc(":*")} perf started.')
    m.you(3.4, "oh and dark mode. people keep asking")
    m.row(4.0, f'{m.acc(":*")} dark-mode started too.')
    m.row(4.3, f'{m.dim("you never wait for a turn. the composer is never locked.")}', size=13, gap=4)
    m.composer([(0, None, f'<tspan fill="{c["acc"]}" font-weight="700">main</tspan>')],
               [(0.2, 0.8, 0.9, "signup is slow on mobile. can you look?"), (2.2, 3.2, 3.4, "oh and dark mode. people keep asking")])
    for t0, t1, n in [(1.6, 4.0, 1), (4.0, 10.2, 2)]:
        k = m.tl.show(t0, t1, dur=0.12)
        m.o.append(f'<text class="{k}" x="{m.W-24}" y="{m.H-40}" text-anchor="end" font-size="12" fill="{c["dim"]}"><tspan class="g" fill="{c["acc"]}">∿</tspan> {n} working</text>')
    return m.svg("you send a second idea while the first agent works: the composer is never locked.")

def f_sync(c):
    m = Mini(c, 10, 230, "between agents · folded by default")
    m.chip(1.0, "dark-mode", "theme", "which file has the colors?")
    m.chip(2.6, "theme", "dark-mode", "tokens.css. i'm editing it, don't touch it.", gap=0)
    m.row(4.4, f'{m.acc(":*")} dark-mode and theme agreed on who edits tokens.css.', gap=4)
    m.row(5.0, f'{m.dim("you didn’t have to do anything.")}', size=13)
    return m.svg("two agents message each other about a shared file, then main sums it up in one line.")

def f_tools(c):
    m = Mini(c, 10, 230, "$ bash · ƒ TypeScript")
    rows = [(0.8, "ƒ", "reading the Sentry issue", 1.8), (1.9, "ƒ", "checking the Linear ticket", 2.9),
            (3.0, "$", "running the failing test", 4.4), (4.5, "ƒ", "opening a PR on GitHub", 6.2)]
    for (t0, g, d, t1) in rows:
        yy = m.y; m.y += 27
        m.row(t0, f'{m.dim(g)} {E(d)}', y=yy)
        m.swap(t0, t1, m.fx + int((len(d) + 3) * CW), yy, f'<tspan class="g" fill="{c["acc"]}">∿</tspan>', f'<tspan fill="{c["acc"]}">✓</tspan>')
    m.row(6.6, f'{m.dim("every MCP server, always on. the agent calls them from code, so its context stays small.")}', size=13, gap=6)
    return m.svg("an agent calls Sentry, Linear and GitHub tools from code, one row each.")

def f_card(c):
    m = Mini(c, 10, 230, "ctrl+g opens the inbox")
    m.row(0.6, f'{m.acc(":*")} perf found why signup is slow.')
    yy = m.y + 6; k = m.tl.show(1.4, 5.2, dur=0.2); a = m.tl.show(5.2, dur=0.2)
    m.o.append(f'<g class="{k}"><rect x="{m.fx-14}" y="{yy-17}" width="620" height="72" rx="4" fill="{c["raised"]}"/><rect x="{m.fx-14}" y="{yy-17}" width="3" height="72" fill="{c["acc"]}"/>'
               f'<text x="{m.fx}" y="{yy}" font-size="14" font-weight="700" fill="{c["acc"]}">? perf needs you</text>'
               f'<text x="{m.fx}" y="{yy+22}" font-size="14" fill="{c["text"]}">the hero image is 4.2 MB. compress it, or lazy-load it?</text>'
               f'<text x="{m.fx}" y="{yy+44}" font-size="13" fill="{c["dim"]}" xml:space="preserve">1 compress   2 lazy-load   3 both</text></g>'
               f'<text class="{a}" x="{m.fx}" y="{yy}" font-size="14" fill="{c["dim"]}">{m.acc("✓")} perf · you said both</text>')
    kp = m.tl.show(4.6, 5.2, dur=0.1)
    m.o.append(f'<rect class="{kp}" x="{m.fx + int(28*7.6)}" y="{yy+30}" width="{int(7*7.6)+12}" height="20" rx="3" fill="{c["chip"]}"/>')
    m.y = yy + 34
    m.row(6.0, f'{m.acc("✓")} perf done {m.dim("· signup 4.1 s → 0.9 s")}', gap=4)
    return m.svg("a card asks one question; you answer with one key; it folds into one line.")

def f_direct(c):
    m = Mini(c, 12, 250, "⌥ + number, or @name")
    m.row(0.5, f'{m.acc("✓")} perf done {m.dim("· signup 4.1 s → 0.9 s")}', gap=0)
    m.row(2.2, f'{m.dim("⌥1 · now talking to perf")}', size=13, gap=4)
    m.you(3.9, "why 0.9 and not 0.5?")
    m.row(4.7, f'{m.dim("@ perf:")} the fonts block the first paint. want me to preload them?')
    m.you(6.9, "yes. then back to main")
    m.row(7.6, f'{m.dim("@ perf:")} on it.')
    main = f'<tspan fill="{c["acc"]}" font-weight="700">main</tspan>'; perf = f'<tspan fill="{c["acc"]}" font-weight="700">perf</tspan>'
    m.composer([(0, 2.2, main), (2.2, 8.4, perf), (8.4, None, main)],
               [(3.0, 3.7, 3.9, "why 0.9 and not 0.5?"), (5.8, 6.7, 6.9, "yes. then back to main")])
    return m.svg("you switch to one agent with alt+1, ask why, push it, and go back to main.")

def f_steer(c):
    m = Mini(c, 10, 230, "✓ sent · ✓✓ read")
    yy = m.y; m.y += 27
    m.swap(0.4, 3.6, m.fx, yy, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> sad-404 <tspan fill="{c["dim"]}">· swapping the sad dog for a sun…</tspan>',
           f'<tspan class="g" fill="{c["acc"]}">∿</tspan> sad-404 <tspan fill="{c["dim"]}">· putting a hat on the dog…</tspan>')
    m.you(1.4, "@sad-404 keep the dog. just give it a hat", read=1.2)
    m.row(3.3, f'{m.dim("@ sad-404:")} got it. the dog keeps its job. hat on.', gap=4)
    m.row(4.2, f'{m.dim("the correction lands in the running agent. nothing restarts.")}', size=13, gap=6)
    return m.svg("you correct a running agent; the double check shows it read the message; it changes course.")

def f_resume(c):
    m = Mini(c, 10, 230, "nothing gets dropped")
    yy = m.y; m.y += 27
    m.swap(0.4, 1.8, m.fx, yy, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> dark-mode <tspan fill="{c["dim"]}">· 6 of 11 pages dark</tspan>',
           f'<tspan fill="{c["acc"]}">✗</tspan> dark-mode <tspan fill="{c["dim"]}">stopped: the provider answered 503.</tspan>')
    m.row(3.0, f'{m.acc(":*")} dark-mode stopped half-way. i started it again, from page 7.', gap=4)
    m.row(4.2, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> dark-mode {m.dim("· 7 of 11 pages…")}')
    m.row(5.2, f'{m.dim("quit bise, update it, come back tomorrow: the agents pick up where they were.")}', size=13, gap=6)
    return m.svg("an agent stops on a provider error; main starts it again from where it stopped.")

def f_worktree(c):
    m = Mini(c, 10, 230, "ψ = its own worktree")
    m.row(0.5, f'{m.acc(":*")} migrate-db needs its own copy of the repo. the others keep the shared folder.')
    yy = m.y; m.y += 27
    m.swap(1.6, 5.0, m.fx, yy, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> migrate-db <tspan fill="{c["dim"]}">ψ db-v2 · running the migration on a copy</tspan>',
           f'<tspan fill="{c["acc"]}">✓</tspan> migrate-db <tspan fill="{c["dim"]}">merged into main. worktree cleaned up.</tspan>')
    m.row(1.9, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> dark-mode {m.dim("· shared folder")}')
    m.row(2.2, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> emoji-csv {m.dim("· shared folder")}')
    m.row(5.6, f'{m.dim("worktrees only when they help. no hundred branches to merge.")}', size=13, gap=6)
    return m.svg("one agent gets its own worktree for a risky job; the others share the folder; the worktree is cleaned up after.")

FEATS = [("talk", f_talk), ("sync", f_sync), ("tools", f_tools), ("card", f_card), ("direct", f_direct),
         ("steer", f_steer), ("resume", f_resume), ("worktree", f_worktree)]

for mode, c in PAL.items():
    for name, f in [("hero", hero), ("demo", demo), ("demo-b", demo_b), ("team", team)]:
        open(os.path.join(HERE, f"{name}-{mode}.svg"), "w").write(f(c))
    os.makedirs(os.path.join(HERE, "feat"), exist_ok=True)
    for name, f in FEATS:
        open(os.path.join(HERE, "feat", f"{name}-{mode}.svg"), "w").write(f(c))
print("ok")
