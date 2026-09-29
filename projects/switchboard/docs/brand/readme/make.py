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
            f'<text x="480" y="242" text-anchor="middle" font-size="14" fill="{c["dim"]}">ramble. interrupt. change your mind. i run the agents. you stay in flow.</text>')
    return svg(W, H, body, "".join(css), "bise :* multi-agent coding, made human.")

# ---------- demo ----------
# the same story as the landing's flow demo (site/index.html, run()): five ideas in a row,
# the team syncs, you change your mind, one card, everything ships.
def demo(c):
    W, T = 960, 31
    tl = TL(T); out = []
    fx, lh = 40, 21
    acc = lambda t: f'<tspan fill="{c["acc"]}">{t}</tspan>'
    y = [76]
    def line(t, html, gap=0, color="text", size=14):
        y[0] += gap
        k = tl.show(t); out.append(f'<text class="{k}" x="{fx}" y="{y[0]}" font-size="{size}" fill="{c[color]}" xml:space="preserve">{html}</text>'); y[0] += lh; return k
    def you(t, s, gap=6):
        y[0] += gap
        k = tl.show(t); r = tl.show(t + 0.55, dur=0.1)
        out.append(f'<g class="{k}"><rect x="{fx-16}" y="{y[0]-15}" width="3" height="19" fill="{c["acc"]}"/>'
                   f'<text x="{fx}" y="{y[0]}" font-size="14" font-weight="700" fill="{c["text"]}" xml:space="preserve">{E(s)} <tspan font-weight="400" fill="{c["faint"]}">✓</tspan></text></g>'
                   f'<text class="{r}" x="{fx + int((len(s) + 1) * 8.45)}" y="{y[0]}" font-size="14" fill="{c["acc"]}">✓✓</text>')
        y[0] += lh
    main = lambda t, s: line(t, f'{acc(":*")} {E(s)}')
    msgs = []  # (typing start, typing end, send, text): drawn in the composer once its y is known
    def say(t, t2, send, text, gap=6):
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
    line(12.8, "▸ 9 messages between 5 agents", gap=6, color="dim", size=13)
    main(13.6, "dark-mode asked which gray. i said the one in tokens.css.")
    say(14.4, 15.8, 16.0, "actually keep the sad dog on the 404. just give it a hat")
    main(16.6, "told sad-404. the dog keeps its job. now with a hat.")
    # the card: it asks, you press 2, it folds to one answered line
    y[0] += 10; cy0 = y[0]
    k = tl.show(17.8, 19.9, dur=0.2); a2 = tl.show(19.9, dur=0.2)
    out.append(f'<g class="{k}"><rect x="{fx-16}" y="{cy0-16}" width="600" height="68" rx="4" fill="{c["raised"]}"/><rect x="{fx-16}" y="{cy0-16}" width="3" height="68" fill="{c["acc"]}"/>'
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
        line(t, f'{acc("✓")} {a} <tspan fill="{c["dim"]}">· {E(b)}</tspan>', gap=6 if i == 0 else 0)
    say(25.4, 26.2, 26.4, "you're the best")
    main(27.0, ":*")
    # the frame, sized to the story
    top = y[0] + 14; H = top + 150
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
        out.append(f'<rect class="{d}" x="712" y="{yy-14}" width="12" height="18" fill="{c["bg"]}"/><text class="{d}" x="713" y="{yy}" font-size="14" fill="{c["acc"]}">✓</text>')
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
    return svg(W, H, "".join(out), "".join(tl.css) + css_gust,
               "a bise session: five ideas in a row to main, five agents start, they sync, you change your mind, one card asks you, everything ships.")

# ---------- team ----------
def team(c):
    W, H, T = 960, 300, 10
    tl = TL(T); o = [f'<rect width="{W}" height="{H}" rx="14" fill="{c["bg"]}" stroke="{c["line"]}"/>']
    o.append(f'<text x="60" y="56" font-size="16" font-weight="700" fill="{c["text"]}">you</text>'
             f'<text x="140" y="56" font-size="14" fill="{c["dim"]}">"add dark mode. and the api client moves to v3."</text>'
             f'<path d="M72 68 V104" stroke="{c["faint"]}"/>'
             f'<text x="60" y="126" font-size="16" font-weight="700" fill="{c["text"]}"><tspan fill="{c["acc"]}">:*</tspan> main</text>'
             f'<text x="190" y="126" font-size="14" fill="{c["dim"]}">always listening. knows everything going on in the repo.</text>'
             f'<path d="M72 138 V262 M72 166 H96 M72 196 H96 M72 226 H96 M72 256 H96" stroke="{c["faint"]}" fill="none"/>')
    jobs = [("theme", "the dark colors, in one place"), ("toggle", "the switch in settings, remembered per user"),
            ("docs", "new screenshots, a changelog line"), ("api-v3", "the client upgrade. unrelated, so it runs too.")]
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

for mode, c in PAL.items():
    for name, f in [("hero", hero), ("demo", demo), ("team", team)]:
        open(os.path.join(HERE, f"{name}-{mode}.svg"), "w").write(f(c))
print("ok")
