#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["uharfbuzz>=0.40"]
# ///
"""Builds the README's animated SVGs (dark + light), in the paper art direction. Run: uv run make.py
The SVGs are shown on GitHub as <img>: no JS, no web fonts. CSS keyframes only.
paper: each image is a sheet of paper (light paper, or dark paper for prefers-color-scheme: dark).
the screens sit on it as figures (a lighter card, a pencil frame, a flat offset shadow) and keep the
terminal's own look inside: monospace, the TUI colors. the display words (bise :*, the tagline, the
captions) are Newsreader and Caveat outlined to paths by ink.py, so they render without web fonts."""
from html import escape as E
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ink
# the screens' own colors, on each paper (site/content/landing-paper.html, the 'figure' screens)
PAL = {
  "dark":  dict(mode="dark", bg="#1f1c19", text="#ece6da", dim="#c4bcaf", faint="#6f685f", wind="#38332e", acc="#f4a6b0", raised="#2a2622", foot="#24201d", chip="#2b2723", line="#2e2a26"),
  "light": dict(mode="light", bg="#faf7f0", text="#1d1a17", dim="#5a5349", faint="#a89d8a", wind="#cfc5b4", acc="#c8264a", raised="#efe8db", foot="#f3eee4", chip="#e3dac8", line="#e6dece"),
}
MONO = "ui-monospace, SFMono-Regular, 'JetBrains Mono', Menlo, Consolas, 'Liberation Mono', monospace"
BASE_CSS = f"text{{font-family:{MONO};}} @media (prefers-reduced-motion: reduce){{*{{animation:none!important}}}}"

def svg(w, h, body, css, label, c=None, pad=18, caption=None, defs=""):
    """c given: the body is a screen of w x h, laid on a sheet of paper as a figure (with an optional caption)."""
    if c is not None:
        m = c["mode"]; t = ink.PAPER[m]
        cap_h = 40 if caption else 0
        W, H = w + 2 * pad + 3, h + 2 * pad + 3 + cap_h
        cap = ""
        if caption:
            d, _ = ink.text_path(caption, "serif-italic", 17, x=W / 2, y=pad + h + 3 + 30, anchor="middle", wght=400, opsz=16)
            cap = f'<path d="{d}" fill="{t["kick"]}"/>'
        body = (f'<defs>{ink.paper_defs(m)}<clipPath id="card"><rect width="{w}" height="{h}" rx="10"/></clipPath>{defs}</defs>'
                f'<rect width="{W}" height="{H}" rx="8" fill="{t["paper"]}"/><rect width="{W}" height="{H}" rx="8" fill="url(#paper)"/>'
                + ink.figure(m, w, h, f'<g clip-path="url(#card)">{body}</g>', x=pad, y=pad) + cap)
        w, h = W, H
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-label="{E(label)}">'
            f'<style>{BASE_CSS}{css}</style>{body}</svg>\n')

class TL:
    """Timeline: each element fades in at t and stays until the loop fades out."""
    def __init__(s, T): s.T, s.css, s.n, s.starts = T, [], 0, []
    def show(s, t, out=None, dur=0.35):
        s.n += 1; k = f"k{s.n}"; T = s.T; out = out if out is not None else T - 0.8
        s.starts.append(t + dur)
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
    """a sheet of paper: the name in Newsreader, the kiss in Caveat (its * sits level with the colon:
    a mouth), the north wind blowing next to it, a highlighter and a pen underline that draw themselves,
    seeds drifting by."""
    m = c["mode"]; t = ink.PAPER[m]
    W, H = 960, 340
    tp = ink.text_path
    css = []
    # the name: 'bise ' in Newsreader 600, tight; ':' and '*' in Caveat 700, the * lowered
    NS, KS = 104, 96
    d_bise, w_bise = tp("bise", "serif", NS, track=-.045, wght=600, opsz=72)
    w_sp = ink.measure(" ", "serif", NS, wght=600, opsz=72) * .45
    _, w_col = tp(":", "hand", KS, wght=700); _, w_star = tp("*", "hand", KS, wght=700)
    w_name = w_bise + w_sp + w_col + w_star - KS * .02
    x0, yb = (W - w_name) / 2 - 40, 150
    d_bise, _ = tp("bise", "serif", NS, x=x0, y=yb, track=-.045, wght=600, opsz=72)
    xk = x0 + w_bise + w_sp
    d_col, _ = tp(":", "hand", KS, x=xk, y=yb, wght=700)
    d_star, _ = tp("*", "hand", KS, x=xk + w_col - KS * .02, y=yb + KS * .3, wght=700)
    # the tagline, with a highlighter on 'made for humans'
    TS, ty = 34, 220
    a, b, e = "a multi-agent harness, ", "made for humans", "."
    wa, wb, we = (ink.measure(s, "serif", TS, track=-.015, wght=400, opsz=36) for s in (a, b, e))
    tx = (W - wa - wb - we) / 2
    d_t = "".join(tp(s, "serif", TS, x=x, y=ty, track=-.015, wght=400, opsz=36)[0] for s, x in ((a, tx), (b, tx + wa), (e, tx + wa + wb)))
    hx, hw = tx + wa - 4, wb + 8
    # the line under it, in italic, the pen underlines 'you stay in flow'
    SS, sy = 21, 268
    p1, p2, p3 = "meet your team lead. ", "you stay in flow", ", i run the agents."
    w1, w2, w3 = (ink.measure(s, "serif-italic", SS, wght=400, opsz=20) for s in (p1, p2, p3))
    sx = (W - w1 - w2 - w3) / 2
    d_s = "".join(tp(s, "serif-italic", SS, x=x, y=sy, wght=400, opsz=20)[0] for s, x in ((p1, sx), (p2, sx + w1), (p3, sx + w1 + w2)))
    ux0, ux1, uy = sx + w1 - 3, sx + w1 + w2 + 3, sy + 7
    ul = f"M{ux0:.1f} {uy+1:.1f} C{ux0+w2*.2:.1f} {uy-3:.1f},{ux0+w2*.4:.1f} {uy+3:.1f},{ux0+w2*.6:.1f} {uy:.1f} S{ux1-8:.1f} {uy-2:.1f},{ux1:.1f} {uy:.1f}"
    css += ["@keyframes hl{from{transform:scaleX(0)}to{transform:scaleX(1)}}.hl{transform-box:fill-box;transform-origin:left;transform:scaleX(0);animation:hl 1.1s .6s ease forwards}",
            "@keyframes pen{to{stroke-dashoffset:0}}.draw{stroke-dasharray:1;stroke-dashoffset:1;animation:pen 1.2s 1.5s cubic-bezier(.6,.1,.3,1) forwards}",
            "@keyframes kiss{0%,86%,100%{opacity:1}90%{opacity:.25}94%{opacity:1}}.kiss{animation:kiss 5s infinite}",
            "@keyframes bob{50%{transform:translateY(-7px) rotate(-2deg)}}.bob{animation:bob 5s ease-in-out infinite;transform-box:fill-box;transform-origin:center}",
            "@keyframes gust{to{stroke-dashoffset:-46}}.gust path{stroke-dasharray:14 9;animation:gust 1.6s linear infinite}"]
    # seeds: a few * drift across the page, the wind carries them
    seeds = []
    for i, (x, y, dl, dur) in enumerate([(70, 70, 0, 18), (150, 290, -6, 22), (300, 40, -11, 20), (620, 300, -3, 24), (790, 60, -14, 19), (880, 250, -8, 21), (460, 315, -17, 23)]):
        css.append(f"@keyframes s{i}{{0%{{opacity:0;transform:translate(0,0) rotate(0)}}12%{{opacity:.5}}80%{{opacity:.4}}100%{{opacity:0;transform:translate({160 + i * 23 % 120}px,-{40 + i * 17 % 60}px) rotate(200deg)}}}}"
                   f".s{i}{{opacity:0;transform-box:fill-box;transform-origin:center;animation:s{i} {dur}s linear {dl}s infinite}}")
        seeds.append(f'<text class="s{i}" x="{x}" y="{y}" font-size="{13 + i % 3 * 2}" fill="{t["kick"]}">{"·" if i % 4 == 3 else "*"}</text>')
    cx, cy, cs = xk + w_col + w_star + 4, 34, .86  # the cloud, right of the kiss, a bit above
    body = (f'<defs>{ink.paper_defs(m)}{ink.cloud_defs(m)}</defs>'
            f'<rect width="{W}" height="{H}" rx="8" fill="{t["paper"]}"/><rect width="{W}" height="{H}" rx="8" fill="url(#paper)"/>'
            f'<g>{"".join(seeds)}</g>'
            f'<path d="{d_bise}" fill="{t["ink"]}"/><g class="kiss" fill="{t["pen"]}"><path d="{d_col}"/><path d="{d_star}"/></g>'
            f'<g transform="translate({cx:.1f} {cy}) scale({cs})"><g class="bob">{ink.cloud(m)}</g></g>'
            f'<rect class="hl" x="{hx:.1f}" y="{ty - TS * .62:.1f}" width="{hw:.1f}" height="{TS * .72:.1f}" rx="3" fill="{t["hl"]}"/>'
            f'<path d="{d_t}" fill="{t["ink"]}"/><path d="{d_s}" fill="{t["dim"]}"/>'
            f'<path class="draw" pathLength="1" d="{ul}" fill="none" stroke="{t["pen"]}" stroke-width="2.4" stroke-linecap="round"/>')
    return svg(W, H, body, "".join(css), "bise :* a multi-agent harness, made for humans.")

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
    out.insert(0, f'<rect width="{W}" height="{H}" fill="{c["bg"]}"/>'
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
    out.append(f'<rect x="0" y="{top}" width="{W}" height="{H-top}" fill="{c["foot"]}"/>'
               f'<path d="M0 {top} H{W}" stroke="{c["line"]}"/>'
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
               "a bise session: five ideas in a row to main, five agents start, they sync, you change your mind, one card asks you, everything ships.", c, caption="fig. 1 · a bise session, playing live")

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
    out.insert(0, f'<rect width="{W}" height="{H}" fill="{c["bg"]}"/>'
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
    out.append(f'<rect x="0" y="{top}" width="{W}" height="{H-top}" fill="{c["foot"]}"/>'
               f'<path d="M0 {top} H{W}" stroke="{c["line"]}"/>'
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
               "a bise session: four ideas in a row, agents call tools and message each other, you ask one agent directly, one card asks you, everything ships.", c, caption="fig. 1 · a bise session, playing live")

# ---------- team ----------
def team(c):
    W, H, T = 960, 300, 10
    tl = TL(T); o = [f'<rect width="{W}" height="{H}" fill="{c["bg"]}"/>']
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
    return svg(W, H, "".join(o), "".join(tl.css) + css, "you talk to main; main splits one idea into four jobs; the agents tell each other what they touch.", c)


# ---------- small feature demos (like the landing's minis) ----------
CW = 8.45  # width of one 14px mono cell

class Mini:
    def __init__(s, c, T, H=250, right=""):
        s.c, s.T, s.H, s.W = c, T, H, 680
        s.tl = TL(T); s.o = [f'<rect width="{s.W}" height="{H}" fill="{c["bg"]}"/>']
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
        s.o.append(f'<rect x="0" y="{top}" width="{W}" height="{H-top}" fill="{c["foot"]}"/><path d="M0 {top} H{W}" stroke="{c["line"]}"/>')
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
        return svg(s.W, s.H, "".join(s.o), "".join(s.tl.css) + css, label, s.c, pad=16)

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
    m.row(6.6, f'{m.dim("every MCP server, always on. called from code: the context stays small.")}', size=13, gap=6)
    return m.svg("an agent calls Sentry, Linear and GitHub tools from code, one row each.")

def f_card(c):
    m = Mini(c, 10, 230, "ctrl+1 opens it, or a click")
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
    m.row(0.5, f'{m.acc(":*")} migrate-db gets its own worktree. the others share the folder.')
    yy = m.y; m.y += 27
    m.swap(1.6, 5.0, m.fx, yy, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> migrate-db <tspan fill="{c["dim"]}">ψ db-v2 · running the migration on a copy</tspan>',
           f'<tspan fill="{c["acc"]}">✓</tspan> migrate-db <tspan fill="{c["dim"]}">merged into main. worktree cleaned up.</tspan>')
    m.row(1.9, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> dark-mode {m.dim("· shared folder")}')
    m.row(2.2, f'<tspan class="g" fill="{c["acc"]}">∿</tspan> emoji-csv {m.dim("· shared folder")}')
    m.row(5.6, f'{m.dim("no branch to name, no folder to delete. nothing to think about.")}', size=13, gap=6)
    return m.svg("one agent gets its own worktree for a risky job; the others share the folder; the worktree is cleaned up after.")

def f_quiet(c):
    m = Mini(c, 12, 250, "ctrl+o opens it all · ctrl+o folds it back")
    m.row(0.3, f'{m.acc(":*")} perf and theme are on it.', gap=0)
    y0 = m.y
    g = f'<tspan fill="{c["faint"]}">▸</tspan>'
    for (t0, t1, n) in [(1.0, 2.0, 3), (2.0, 3.0, 7), (3.0, 4.2, 12)]:
        m.row(t0, f'{g} {m.dim(f"{n} folded · tool calls and messages between agents")}', out=t1, y=y0)
    # ctrl+o pressed
    kp = m.tl.show(3.7, 4.2, dur=0.1)
    m.o.append(f'<g class="{kp}"><rect x="{m.W-92}" y="{y0-15}" width="64" height="20" rx="3" fill="{c["chip"]}"/>'
               f'<text x="{m.W-60}" y="{y0}" text-anchor="middle" font-size="12" fill="{c["text"]}">ctrl+o</text></g>')
    rows = [(4.3, f'{m.acc("✉")} perf {m.dim("→")} theme {m.dim("· which file has the colors?")}', None),
            (4.5, f'{m.dim("ƒ")} reading the Lighthouse report', "✓"),
            (4.7, f'{m.acc("✉")} theme {m.dim("→")} perf {m.dim("· tokens.css. i’m on it, don’t touch it")}', None),
            (4.9, f'{m.dim("$")} npm run build', "✓"),
            (5.1, f'{m.dim("$")} running the signup test', "✓")]
    for i, (t0, html, mark) in enumerate(rows):
        yy = y0 + i * 27
        if mark: html += f' <tspan fill="{c["acc"]}">{mark}</tspan>'
        m.row(t0, html, size=13, out=8.2, y=yy)
    kp2 = m.tl.show(7.9, 8.3, dur=0.1)
    m.o.append(f'<g class="{kp2}"><rect x="{m.W-92}" y="{y0-15}" width="64" height="20" rx="3" fill="{c["chip"]}"/>'
               f'<text x="{m.W-60}" y="{y0}" text-anchor="middle" font-size="12" fill="{c["text"]}">ctrl+o</text></g>')
    m.row(8.5, f'{g} {m.dim("12 folded · tool calls and messages between agents")}', y=y0)
    m.row(9.0, f'{m.acc("✓")} perf done {m.dim("· signup 4.1 s → 0.9 s")}', y=y0 + 27)
    m.row(0.3, f'{m.dim("what matters to you shows. the rest is one key away, in full.")}', size=13, y=m.H - 22)
    return m.svg("the work between agents stays folded into one line; ctrl+o opens every tool call and message, ctrl+o folds them back.")

# ---------- the README's feature scenes: a tiny bise, drawn like the TUI ----------
# one scene per feature of bise.dev (site/index.html: the cards, then "and so much more"), adapted
# from the site's mini demos (the `plays` scripts). the screen: the header (bise :* and the counts),
# the feed, the composer under its divider (you → main), the key bar. only the lines the idea needs.
def cells(s):
    """the terminal columns of s: an emoji takes two."""
    return sum(2 if ord(ch) > 0xFFFF else 1 for ch in s)

class Tui(Mini):
    """a Mini with the TUI's chrome. the chrome (header, panel, key bar, rules) sits in groups that zen can fade."""
    W = 680
    def __init__(s, c, T, H=270, panel=False, keys="⏎ send   @ agent   ⌥0-9 switch   ctrl+o open   ? help"):
        s.c, s.T, s.H, s.W = c, T, H, 680
        s.tl = TL(T); s.fx, s.y = 40, 72
        s.chrome, s.feed, s.over = [], [], []  # the chrome (fades in zen), the feed and the composer (never fade)
        s.o = s.feed  # Mini's row/you/swap/chip draw into the feed
        s.top = H - 84  # the divider's row
        s.px = s.W - 168 if panel else None  # the agents panel's left edge
        s.keys = keys
        s.fade = []  # (t0, t1): zen spans
        s.last = []  # drawn over everything, the chrome too (what covers the key bar)
        s.marks = []  # (t, baseline) of each feed row: the feed scrolls to keep the newest one in view
        s.comp = None
    def row(s, t, html, color="text", size=14, gap=0, out=None, x=None, y=None):
        yy = Mini.row(s, t, html, color, size, gap, out, x, y)
        s.marks.append((t, yy, y is not None, len(s.feed) - 1)); return yy  # a row put at a given y (a fold) may scroll the feed back
    def acc_b(s, t): return f'<tspan fill="{s.c["acc"]}" font-weight="700">{t}</tspan>'
    def gust(s): return f'<tspan class="g" fill="{s.c["acc"]}">∿</tspan>'
    def header(s, spans):
        """spans: [(t0, t1, html)] for the counts on the right of the header."""
        c = s.c
        s.chrome.append(f'<text x="24" y="28" font-size="13" font-weight="700" fill="{c["text"]}">bise {s.acc(":*")}</text>'
                        f'<line x1="0" y1="42" x2="{s.W}" y2="42" stroke="{c["line"]}"/>')
        for (t0, t1, html) in spans:
            k = s.tl.show(t0, t1, dur=0.15)
            s.chrome.append(f'<text class="{k}" x="{s.W-24}" y="28" text-anchor="end" font-size="12" fill="{c["dim"]}" xml:space="preserve">{html}</text>')
    def agents(s, rows):
        """rows: [(t0, name, [(t, mark_html)])]: one row per agent in the panel; its mark changes at each t."""
        c, x = s.c, s.px
        s.chrome.append(f'<line x1="{x-16}" y1="42" x2="{x-16}" y2="{s.top}" stroke="{c["line"]}"/>'
                        f'<text x="{x}" y="66" font-size="12" fill="{c["faint"]}">agents</text>')
        for i, (t0, name, marks) in enumerate(rows):
            yy = 90 + i * 22
            k = s.tl.show(t0)
            s.chrome.append(f'<text class="{k}" x="{x+20}" y="{yy}" font-size="14" fill="{c["text"]}">{E(name)}</text>')
            for j, (t, mk) in enumerate(marks):
                t1 = marks[j + 1][0] if j + 1 < len(marks) else None
                kk = s.tl.show(t, t1, dur=0.15)
                s.chrome.append(f'<text class="{kk}" x="{x}" y="{yy}" font-size="14" fill="{c["acc"]}">{mk}</text>')
    def you(s, t, text, gap=8, read=0.5, html=None, w=None):
        """your message in the feed: a pen bar, bold, ✓ then ✓✓ once read."""
        s.y += gap; yy = s.y; s.y += 27
        k = s.tl.show(t); r = s.tl.show(t + read, dur=0.1); s.marks.append((t, yy, False, len(s.feed)))
        w = w if w is not None else cells(text)
        s.feed.append(f'<g class="{k}"><rect x="{s.fx-14}" y="{yy-15}" width="3" height="19" fill="{s.c["acc"]}"/>'
                      f'<text x="{s.fx}" y="{yy}" font-size="14" font-weight="700" fill="{s.c["text"]}" textLength="{(w + 2) * CW:.1f}" lengthAdjust="spacing" xml:space="preserve">{html or E(text)} <tspan font-weight="400" fill="{s.c["faint"]}">✓</tspan></text></g>'
                      f'<rect class="{r}" x="{s.fx + (w + 1) * CW - 1:.1f}" y="{yy-15}" width="{CW+2:.1f}" height="19" fill="{s.c["bg"]}"/>'
                      f'<text class="{r}" x="{s.fx + (w + 1) * CW:.1f}" y="{yy}" font-size="14" fill="{s.c["acc"]}">✓✓</text>')
    def main(s, t, text, gap=0, out=None): return s.row(t, f'{s.acc(":*")} {text}', gap=gap, out=out)
    def msg(s, t, a, b, text, gap=0, out=None, y=None):
        """a message between agents, on one row: the chip '✉ a → b', then what it says (dim)."""
        if y is None: s.y += gap; yy = s.y; s.y += 27
        else: yy = y
        k = s.tl.show(t, out); s.marks.append((t, yy, y is not None, len(s.feed)))
        n = cells(a) + cells(b) + 6; w = n * 7.3
        s.feed.append(f'<g class="{k}"><rect x="{s.fx-4}" y="{yy-14}" width="{w:.0f}" height="20" rx="4" fill="{s.c["chip"]}"/>'
                      f'<text x="{s.fx+4}" y="{yy}" font-size="12" fill="{s.c["dim"]}" textLength="{(n - 1) * 7.3 - 8:.1f}" lengthAdjust="spacing" xml:space="preserve">✉ <tspan font-weight="700" fill="{s.c["text"]}">{E(a)}</tspan> <tspan fill="{s.c["faint"]}">→</tspan> {E(b)}</text>'
                      f'<text x="{s.fx + w + 6:.0f}" y="{yy}" font-size="13" fill="{s.c["dim"]}" xml:space="preserve">{E(text)}</text></g>')
        return yy
    def type(s, t, t2, text, x=None, row=0):
        """text typed in the composer between t and t2: whole characters, one at a time (one tspan each, no
        sliding mask). textLength pins the line to the cell grid, so what is drawn after it lines up in any
        mono font. it stays until the send (the composer's group hides it)."""
        x = x if x is not None else 64
        n = cells(text); w = n * CW + 2
        T = s.T; p = lambda v: f"{v / T * 100:.2f}%"
        chars = list(text); spans = []
        for i, ch in enumerate(chars):
            if ch == " ": spans.append(" "); continue  # a space shows nothing: no keyframes
            s.tl.n += 1; k = f"q{s.tl.n}"; at = t + (t2 - t) * (i + 1) / len(chars)
            s.tl.css.append(f"@keyframes {k}{{0%,{p(at)}{{opacity:0}}{p(at + .01)},100%{{opacity:1}}}}.{k}{{animation:{k} {T}s linear infinite}}")
            spans.append(f'<tspan class="{k}">{E(ch)}</tspan>')
        yy = s.top + 34 + row * 22
        s.typed.append((t, f'<text x="{x}" y="{yy}" font-size="14" fill="{s.c["text"]}" textLength="{n * CW:.1f}" lengthAdjust="spacing" xml:space="preserve">{"".join(spans)}</text>'))
        return x + w
    typed = None
    def composer(s, sends, who=None):
        """sends: [(t_start_typing, t_send)]: the typed text shows between them; the placeholder fills the gaps.
        drawn at the end (svg), under whatever the scene puts in s.over (chips, the key it presses)."""
        s.comp = (sends, who)
    def _composer(s):
        sends, who = s.comp; o = []
        c, W = s.c, s.W; top = s.top
        who = who or [(0, None, "main")]  # [(t0, t1, name)]: who you talk to, over time
        o.append(f'<rect x="0" y="{top}" width="{W}" height="{s.H-top}" fill="{c["foot"]}"/>')
        s.chrome.append(f'<path d="M0 {top} H16 M{24 + 18 * 7.3 + 8:.0f} {top} H{W}" stroke="{c["line"]}"/>')
        for (t0, t1, name) in who:
            k = s.tl.show(t0, t1, dur=0.12) if t0 or t1 else None
            o.append(f'<text{f" class={chr(34)}{k}{chr(34)}" if k else ""} x="24" y="{top+4}" font-size="12" fill="{c["dim"]}">you → {s.acc_b(name)}</text>')
        o.append(f'<rect x="48" y="{top+19}" width="2" height="21" fill="{c["acc"]}"/>')
        ends = [0] + [b for (_, b) in sends]; starts = [a for (a, _) in sends] + [s.T]
        for a, b in zip(ends, starts):
            if b - a > 0.3:
                k = s.tl.show(a + 0.1, b, dur=0.1)
                o.append(f'<text class="{k}" x="64" y="{top+34}" font-size="14" fill="{c["faint"]}">what\'s on your mind?</text>')
        for (a, b) in sends:
            k = s.tl.show(a - 0.05, b, dur=0.05)
            o.append(f'<g class="{k}">{"".join(h for (t, h) in (s.typed or []) if a - 0.06 <= t < b)}</g>')
        s.chrome.append(f'<text x="64" y="{s.H-16}" font-size="12" fill="{c["faint"]}" xml:space="preserve">{s.keys}</text>')
        return "".join(o)
    def _scroll(s):
        """the feed sticks to the bottom: when a new row would pass the divider, it scrolls up (ease-out)."""
        T = s.T; p = lambda v: f"{v / T * 100:.2f}%"; view = s.top - 14 - 27  # one blank row above the divider
        fr, cur, steps = ["0%{transform:translateY(0)}"], 0, []
        for t, yb, back, _ in sorted(s.marks):
            need = max(0, yb + 6 - view)
            if need > cur or (back and need != cur):
                fr.append(f"{p(t)}{{transform:translateY(-{cur}px);animation-timing-function:ease-out}}{p(t + 0.35)}{{transform:translateY(-{need}px)}}")
                cur = need; steps.append((t, need))
        if len(fr) == 1: return "", ""
        # a row the scroll pushes above the first row's place fades out as the scroll starts: it never reaches the header
        for t, yb, back, i in s.marks:
            off = next((ts for ts, nd in steps if ts >= t and yb - nd < 72 - 4), None)
            if off is None or i >= len(s.feed): continue
            s.tl.n += 1; k = f"h{s.tl.n}"
            s.tl.css.append(f"@keyframes {k}{{0%,{p(off)}{{opacity:1}}{p(off + .2)},100%{{opacity:0}}}}.{k}{{animation:{k} {T}s linear infinite}}")
            s.feed[i] = f'<g class="{k}">{s.feed[i]}</g>'
        fr.append(f"{p(T - 0.3)}{{transform:translateY(-{cur}px)}}100%{{transform:translateY(0)}}")
        return f"@keyframes scr{{{''.join(fr)}}}.scr{{animation:scr {T}s linear infinite}}", ' class="scr"'
    def zen(s, t0, t1, depth=.42):
        """the chrome fades between t0 and t1 (zen.rs: 250 ms, mixed toward its background)."""
        s.fade.append((t0, t1, depth))
    def svg(s, label):
        T = s.T; p = lambda v: f"{v / T * 100:.2f}%"
        css = "@keyframes g{0%{opacity:.3}50%{opacity:1}100%{opacity:.3}}.g{animation:g 1.2s infinite}"
        cls = ""
        if s.fade:
            fr = ["0%{opacity:1}"]
            for (a, b, d) in s.fade: fr.append(f"{p(a)}{{opacity:1}}{p(a+.25)},{p(b)}{{opacity:{d}}}{p(b+.25)}{{opacity:1}}")
            css += f"@keyframes zen{{{''.join(fr)}100%{{opacity:1}}}}.zen{{animation:zen {T}s linear infinite}}"
            cls = ' class="zen"'
        comp = s._composer() if s.comp else ""
        scss, scls = s._scroll(); css += scss
        fw = (s.px - 17) if s.px else s.W
        feedclip = f'<clipPath id="feed"><rect x="0" y="43" width="{fw}" height="{s.top - 43}"/></clipPath>'
        # the still: the last thing that happens holds at least 2 s before the loop fades out (designer)
        last = max(t for t in s.tl.starts if t < T - 0.8)
        assert last + 2 <= T - 0.8, f"{label[:40]}: the last event ends at {last:.2f} s, the loop at {T} s: no 2 s still"
        body = (f'<rect width="{s.W}" height="{s.H}" fill="{s.c["bg"]}"/>' + feedclip +
                f'<g clip-path="url(#feed)"><g{scls}>{"".join(s.feed)}</g></g>' + comp + "".join(s.over) + f'<g{cls}>{"".join(s.chrome)}</g>' + "".join(s.last))
        return svg(s.W, s.H, body, "".join(s.tl.css) + css, label, s.c, pad=16)

def t_talk(c):
    """talk whenever. you never wait: you type the next idea while main answers the last one."""
    m = Tui(c, 10); m.typed = []
    one, two, three = "signup is slow on mobile", "and the csv export crashes on emoji 😭", "oh, the cookie banner hides the buy button"
    m.header([(2.1, 4.0, f'{m.gust()} 1 working'), (4.0, 6.3, f'{m.gust()} 2 working'), (6.3, None, f'{m.gust()} 3 working')])
    m.type(0.3, 1.3, one); m.you(1.4, one, gap=0)
    m.type(1.7, 3.2, two); m.main(2.1, "on it. perf started.")
    m.you(3.4, two, gap=8)
    m.type(3.7, 5.5, three); m.main(4.0, "emoji-csv started. it's always unicode.")
    m.you(5.7, three, gap=8)
    m.main(6.3, "cookies started. 3 agents working, you're free.")
    m.composer([(0.3, 1.4), (1.7, 3.4), (3.7, 5.7)])
    return m.svg("you send three ideas in a row. main starts an agent for each while you type the next one: you never wait.")

def t_zen(c):
    """zen mode while you type: the chrome steps back, the feed keeps coming, the send brings it all back."""
    m = Tui(c, 11, panel=True)
    m.typed = []
    msg = "release notes for all of this, when they're done"
    m.header([(0, 3.0, f'{m.gust()} 3 working'), (3.0, 4.9, f'{m.gust()} 2 working · {m.acc("✓")} 1 done'),
              (4.9, 6.9, f'{m.gust()} 1 working · {m.acc("✓")} 2 done'), (6.9, None, f'{m.gust()} 2 working · {m.acc("✓")} 2 done')])
    m.agents([(0, "main", [(0, ":*")]), (0, "perf", [(0, m.gust()), (3.0, "✓")]), (0, "dark-mode", [(0, m.gust()), (4.9, "✓")]),
              (0, "cookies", [(0, m.gust())]), (6.9, "release", [(6.9, m.gust())])])
    m.main(0.1, "on it: perf, dark-mode and cookies started.")
    m.row(0.1, f'{m.dim("▸ 9 messages between 3 agents")}', size=13)
    m.type(1.6, 5.4, msg)
    m.row(3.0, f'{m.acc("✓")} perf done {m.dim("· signup 4.1 s → 0.9 s")}', gap=8)
    m.row(4.2, f'{m.dim("▸ 4 more messages")}', size=13)
    m.you(6.2, msg, gap=8)
    m.main(6.9, "on it: release started.")
    m.zen(1.6, 6.2)
    k = m.tl.show(1.6, 6.2, dur=0.25)
    m.over.append(f'<text class="{k}" x="{m.W-24}" y="{m.top+34}" text-anchor="end" font-size="12" fill="{c["acc"]}">zen</text>')
    m.composer([(1.6, 6.2)])
    return m.svg("you start typing and everything else fades: the agents, the counts. perf finishes meanwhile. you send, and it all comes back.")

def t_screenshot(c):
    """show it a screenshot: ctrl+v pastes an image; it is one chip in your text."""
    m = Tui(c, 10, keys="ctrl+v paste image   @ file   ⏎ send   ? help")
    m.typed = []
    a, b = "the cookie banner hides the buy button ", "on mobile"
    m.header([(0, None, f'{m.gust()} 2 working')])
    m.main(0.1, "perf and dark-mode are on it.")
    x1 = m.type(0.4, 2.0, a)
    chip_w = 5 * CW  # ` ▣ 1 `: 5 cells
    m.type(3.4, 4.1, b, x=x1 + chip_w + CW)
    # ctrl+v: the key lights up, the box 'attached' opens above the text, the chip lands in the text
    kp = m.tl.show(2.5, 3.0, dur=0.1)
    m.over.append(f'<g class="{kp}"><rect x="58" y="{m.H-31}" width="{6*7.3+12:.0f}" height="20" rx="3" fill="{c["chip"]}"/></g>')
    send = 4.7
    top = m.top
    bx, by, bw = 64, top - 58, 300
    kb = m.tl.show(2.8, send, dur=0.15)
    m.over.append(f'<g class="{kb}"><rect x="{bx-6}" y="{by-8}" width="{bw+12}" height="56" fill="{c["bg"]}"/>'
                  f'<rect x="{bx}" y="{by}" width="{bw}" height="44" rx="6" fill="{c["foot"]}" stroke="{c["faint"]}"/>'
                  f'<rect x="{bx+10}" y="{by-8}" width="{8*7.3+10:.0f}" height="14" fill="{c["foot"]}"/>'
                  f'<text x="{bx+14}" y="{by+4}" font-size="12" fill="{c["dim"]}">attached</text>'
                  f'<text x="{bx+16}" y="{by+29}" font-size="14" fill="{c["text"]}" xml:space="preserve"><tspan fill="{c["acc"]}">▣ 1</tspan>  Screenshot 1.png  <tspan fill="{c["faint"]}">1440×900</tspan></text></g>')
    kc = m.tl.show(2.8, send, dur=0.1)
    chip = lambda x, y: (f'<rect x="{x:.1f}" y="{y-15}" width="{chip_w:.1f}" height="20" rx="3" fill="{c["chip"]}"/>'
                         f'<text x="{x + CW:.1f}" y="{y}" font-size="14" fill="{c["acc"]}">▣ 1</text>')
    m.over.append(f'<g class="{kc}">{chip(x1, top + 34)}</g>')
    full = cells(a) + 5 + 1 + cells(b)
    m.you(send, a + b, gap=8, w=full, html=E(a) + " " * 6 + E(b))  # the chip is drawn over the 5 blank cells
    ky = m.y - 27
    k = m.tl.show(send)
    m.feed.append(f'<g class="{k}">{chip(m.fx + cells(a) * CW, ky)}</g>')
    m.main(5.4, "cookies started. it has the screenshot.", gap=0)
    m.composer([(0.4, send)])
    return m.svg("you type a message, paste a screenshot with ctrl+v: it lands as one chip in your text, and the agent gets the image.")

def t_resume(c):
    """hand it something huge: one big goal, main runs agents in waves, restarts the one that stops, keeps going."""
    m = Tui(c, 11.5); m.typed = []
    goal = "dark mode on every page, not just settings"
    m.header([(2.3, 4.8, f'{m.gust()} 3 working'), (4.8, 6.0, f'{m.gust()} 2 working'), (6.0, 8.2, f'{m.gust()} 3 working'),
              (8.2, None, f'{m.acc("✓")} 9 done')])
    m.type(0.2, 1.5, goal); m.you(1.7, goal, gap=0)
    m.main(2.3, "12 pages. i'll run 3 agents at a time and keep going till it's done.")
    g = m.gust()
    m.row(3.4, f'{g} dark-1 {m.dim("·")} {g} dark-2 {m.dim("·")} {g} dark-3', color="dim", gap=8)
    m.row(4.8, f'{m.acc("✗")} dark-2 {m.dim("stopped: the provider answered 503.")}')
    m.main(6.0, "started dark-2 again, from page 7.")
    m.row(7.2, f'{m.dim("▸ 9 agents over 40 minutes")}', size=13, gap=8)
    m.row(8.2, f'{m.acc("✓")} 12 of 12 pages dark, tests green.')
    m.composer([(0.2, 1.7)])
    return m.svg("you give main one big goal. it runs three agents at a time, starts again the one that stops on an error, and keeps going until all 12 pages are done.")

def t_worktree(c):
    """worktrees? don't think about it: one agent gets its own copy, works there, it is cleaned up after."""
    m = Tui(c, 10, panel=True); m.typed = []
    psi = f' <tspan fill="{c["acc"]}">ψ</tspan>'
    m.header([(0, 1.9, f'{m.gust()} 2 working'), (1.9, 5.0, f'{m.gust()} 3 working'), (5.0, None, f'{m.gust()} 2 working · {m.acc("✓")} 1 done')])
    m.agents([(0, "main", [(0, ":*")]), (0, "dark-mode", [(0, m.gust())]), (0, "cookies", [(0, m.gust())]),
              (1.9, "perf", [(1.9, m.gust()), (5.0, "✓")])])
    k = m.tl.show(1.9, 5.0, dur=0.15)  # the ψ next to perf while its worktree lives
    m.chrome.append(f'<text class="{k}" x="{m.px + 20 + 5 * 8.45:.0f}" y="{90 + 3 * 22}" font-size="14" fill="{c["acc"]}">ψ</text>')
    m.main(0.2, "dark-mode and cookies share your folder.")
    m.main(1.3, "perf needs a clean build to time signup.", gap=8)
    m.row(1.9, f'   it gets its own worktree:{psi} perf.', color="text")
    m.row(5.0, f'{m.acc("✓")} perf done {m.dim("· signup 4.1 s → 0.9 s")}', gap=8)
    m.row(5.6, f'{m.dim("  worktree merged and cleaned up.")}', size=13)
    m.composer([])
    return m.svg("dark-mode and cookies share your folder; perf gets its own worktree for a clean build, finishes, and the worktree is cleaned up.")

def t_card(c):
    """you're not the router: main answers the obvious questions for you; only the real decision reaches you."""
    m = Tui(c, 12); m.typed = []
    m.header([(0, None, f'{m.gust()} 3 working')])
    m.msg(0.4, "dark-mode", "main", "which gray for the borders?")
    m.main(1.4, "i told dark-mode: the gray in tokens.css, like everywhere else.")
    m.msg(2.7, "emoji-csv", "main", "add a BOM so excel opens it?", gap=8)
    m.main(3.7, "i told emoji-csv: yes, excel needs it. the old exports had one.")
    m.msg(5.0, "cookies", "main", "legal wants the banner. drop it anyway?", gap=8)
    m.main(6.0, f'{m.acc("that one's yours:")} cookies asks if the banner can go.')
    ans = "it stays. half the size"
    m.type(6.6, 7.6, ans); m.you(7.8, ans, gap=8)
    m.main(8.4, "told cookies. half the size, the buy button shows.")
    m.composer([(6.6, 7.8)])
    return m.svg("three agents ask main a question. main answers two of them itself, the way you would, and passes you the one decision that is yours.")

def t_sync(c):
    """agents sync on their own: they ask and hand off; it folds into one line; nothing for you."""
    m = Tui(c, 9); m.typed = []
    m.header([(0, None, f'{m.gust()} 3 working')])
    y0 = m.y
    m.msg(0.5, "release", "emoji-csv", "did the export format change?", out=4.4)
    m.msg(1.4, "emoji-csv", "release", "no. same columns, now in utf-8.", out=4.4)
    m.msg(2.3, "release", "dark-mode", "a screenshot for the notes?", out=4.4)
    m.msg(3.2, "dark-mode", "release", "done, docs/dark.png", out=4.4)
    m.row(4.6, f'{m.dim("▸ 4 messages between 3 agents")}', size=13, y=y0)
    m.row(5.3, f'{m.acc(":*")} release has what it needs. nothing for you.', y=y0 + 35)
    m.composer([])
    return m.svg("release asks emoji-csv and dark-mode what it needs; they answer; the four messages fold into one line, and main says there is nothing for you.")

def t_tools(c):
    """all your MCPs, always on: bise calls them from code, so a hundred servers don't fill its context."""
    m = Tui(c, 10); m.typed = []
    q = "signup is slow on mobile. since when?"
    m.header([(0, None, "42 MCP servers")])
    m.main(0.2, "42 MCP servers on. all of them, all the time.")
    m.row(0.2, f'{m.dim("  github · linear · sentry · slack · notion · postgres · +36")}', size=13)
    m.type(0.7, 2.0, q); m.you(2.2, q, gap=8)
    m.row(2.9, f'<tspan fill="{c["faint"]}">╭─</tspan> {m.dim("ƒ typescript")}', size=13, gap=8)
    bar = f'<tspan fill="{c["faint"]}">│</tspan>'
    aw = f'<tspan fill="{c["acc"]}">await</tspan>'
    for i, line in enumerate([f'slow = {aw} tools.sentry.slowest("signup", "mobile")',
                              f'pr   = {aw} tools.github.mergedBefore(slow.since)',
                              f'said = {aw} tools.slack.search("signup slow")']):
        m.row(3.3 + i * 0.45, f'{bar} {line}', size=13, color="dim")
    m.main(5.0, "since tuesday: #412 added a 4.2 MB hero image.", gap=8)
    m.row(5.4, "  #support saw it the same day. want perf on it?")
    m.composer([(0.7, 2.2)])
    return m.svg("42 MCP servers are on. you ask why signup is slow; main writes a few lines of code that call Sentry, GitHub and Slack, and answers in two lines.")

def t_plugins(c):
    """bring your Agent Plugins: skills, MCP servers, hooks load as they are."""
    m = Tui(c, 9); m.typed = []
    ask = "draft the release notes, the usual way"
    m.header([(5.2, None, f'{m.gust()} 1 working')])
    m.main(0.4, "hi. i found your setup:")
    for i, (a, b) in enumerate([("14 skills", "~/.agents/skills"), ("3 plugins", "~/.agents/plugins, .vibe"), ("6 MCP servers", ".mcp.json")]):
        # the paths: one column, at a fixed x (an absolute tspan x, so it does not hang on the font's widths)
        m.row(1.0 + i * 0.5, f'   {m.acc("✓")} {a}<tspan x="{m.fx + 22 * CW:.0f}" fill="{c["dim"]}">{E(b)}</tspan>')
    m.main(2.8, "everything you had works here. nothing to port.", gap=8)
    m.type(3.4, 4.6, ask); m.you(4.8, ask, gap=8)
    m.main(5.4, "release started, with your release-notes skill.")
    m.composer([(3.4, 4.8)])
    return m.svg("on its first run, bise finds your skills, plugins and MCP servers; you ask for release notes and the agent uses your own skill.")

def press(m, t, label, x=None, y=None, dur=0.5):
    """a key pressed: its name on a key cap, for half a second (right of the composer row by default)."""
    c = m.c; w = cells(label) * 7.3 + 14
    x = x if x is not None else m.W - 24 - w; y = y if y is not None else m.top + 34
    k = m.tl.show(t, t + dur, dur=0.08)
    m.over.append(f'<g class="{k}"><rect x="{x:.0f}" y="{y-15}" width="{w:.0f}" height="21" rx="4" fill="{c["chip"]}" stroke="{c["faint"]}"/>'
                  f'<text x="{x + w / 2:.0f}" y="{y}" text-anchor="middle" font-size="12" font-weight="700" fill="{c["text"]}">{E(label)}</text></g>')

def t_direct(c):
    """talk to any agent, anytime: ⌥1, ask perf why, push it, ⌥0 back to main."""
    m = Tui(c, 11); m.typed = []
    q1, q2 = "so why is signup slow?", "ouch. add a check so it can't come back"
    m.header([(0, None, f'{m.gust()} 3 working')])
    m.main(0.2, "perf is on it. signup takes 4.1 s on a phone.")
    press(m, 1.0, "⌥1"); m.row(1.4, f'{m.dim("⌥1 · now talking to perf")}', size=13, gap=8)
    m.type(1.9, 2.9, q1); m.you(3.1, q1, gap=4)
    m.row(3.8, f'{m.dim("@ perf to you:")} the hero image is 4.2 MB. on a phone, 3 s.')
    m.type(4.4, 5.8, q2); m.you(6.0, q2)
    m.row(6.6, f'{m.dim("@ perf to you:")} on it.')
    press(m, 7.2, "⌥0"); m.row(7.6, f'{m.dim("⌥0 · back to main")}', size=13, gap=8)
    m.composer([(1.9, 3.1), (4.4, 6.0)], who=[(0, 1.4, "main"), (1.4, 7.6, "perf"), (7.6, None, "main")])
    return m.svg("you press alt+1 and talk to perf directly: why is signup slow, then add a check. alt+0 takes you back to main. nobody stopped.")

def t_prs(c):
    """fits your PR flow: its own branch and PR, CI, a review bot, you, merged; the panel shows where it stands."""
    m = Tui(c, 11, panel=True); m.typed = []
    ans = "good bot. do it"
    m.header([(0, 7.8, f'{m.gust()} 1 working'), (7.8, None, f'{m.acc("✓")} 1 done')])
    m.agents([(0, "main", [(0, ":*")]), (0, "cookies", [(0, m.gust()), (7.8, "✓")])])
    # under cookies, its PR's state (the panel shows where each one stands)
    for (t0, t1, html) in [(0.3, 1.6, f'#409 {m.dim("· checks")} {m.gust()}'), (1.6, 2.9, f'#409 {m.dim("·")} {m.acc("✗ checks")}'),
                           (2.9, 4.2, f'#409 {m.dim("· green")}'), (4.2, 7.8, f'#409 {m.dim("· review")}'), (7.8, None, f'#409 {m.dim("· merged")}')]:
        k = m.tl.show(t0, t1, dur=0.15)
        m.chrome.append(f'<text class="{k}" x="{m.px + 20}" y="{90 + 2 * 22}" font-size="12" fill="{c["text"]}" xml:space="preserve">{html}</text>')
    m.main(0.3, "cookies opened #409: the banner, half the size.")
    m.row(1.6, f'{m.acc("↑")} {m.dim("#409 · checks fail: e2e/checkout.spec.ts")}', size=13)
    m.main(2.9, "cookies fixed the test. #409 is green.")
    m.row(4.2, f'{m.acc("↑")} {m.dim("#409 · review-bot: make the close button 44 px")}', size=13)
    m.type(4.8, 5.6, ans); m.you(5.8, ans)
    m.main(6.4, "told cookies. 44 px, pushed to #409.")
    m.row(7.8, f'{m.acc("✓")} #409 merged {m.dim("· cookies archived, worktree removed")}', gap=8)
    m.composer([(4.8, 5.8)])
    return m.svg("cookies opens pull request #409. CI fails, it fixes the test; a review bot asks for a bigger button, you say do it; #409 merges. the panel shows where it stands.")

def t_tokens(c):
    """your token bill can relax: a job too small for an agent, main does it itself."""
    m = Tui(c, 8); m.typed = []
    ask = "fix the typo in the footer"
    m.header([(0, None, f'{m.gust()} 2 working')])
    m.main(0.2, "perf and dark-mode are on it.")
    m.type(0.6, 1.6, ask); m.you(1.8, ask, gap=8)
    m.main(2.5, "done, i did it myself. too small for an agent.")
    m.row(3.1, f'{m.dim("  footer.html · recieve → receive")}', size=13)
    m.row(3.7, f'<tspan fill="{c["faint"]}">  0 agents started</tspan>', size=13)
    m.composer([(0.6, 1.8)])
    return m.svg("you ask for a one-word typo fix. main does it itself: too small for an agent. 0 agents started, the count stays at 2.")

def t_voice(c):
    """or just talk: /voice on, ctrl+r, say it; it lands as text in the composer."""
    m = Tui(c, 9.5, keys="ctrl+r record   ⏎ send   @ agent   ? help"); m.typed = []
    said = "and keep the buy button visible on small phones"
    m.header([(0, None, f'{m.gust()} 1 working')])
    m.main(0.2, "cookies started. the banner is getting smaller.")
    press(m, 0.9, "ctrl+r")
    # recording: the dot, a level meter (the newest bar on the right), the time; the key bar says how to stop
    rec0, rec1 = 1.1, 3.6
    k = m.tl.show(rec0, rec1, dur=0.1)
    bars = []
    for i in range(8):
        bars.append(f'<rect class="mb{i % 3}" x="{88 + i * 9}" y="{m.top + 22}" width="6" height="14" rx="1" fill="{c["acc"]}"/>')
    m.over.append(f'<g class="{k}"><rect x="52" y="{m.top + 14}" width="{m.W - 80}" height="30" fill="{c["foot"]}"/>'
                  f'<text x="64" y="{m.top + 34}" font-size="14" fill="{c["acc"]}">●</text>{"".join(bars)}</g>')
    for i, (a, b) in enumerate([(rec0, 2.1), (2.1, 3.1), (3.1, rec1)]):
        kt = m.tl.show(a, b, dur=0.05)
        m.over.append(f'<text class="{kt}" x="170" y="{m.top + 34}" font-size="13" fill="{c["dim"]}">0:0{i}</text>')
    kk = m.tl.show(rec0, rec1, dur=0.1)
    m.last.append(f'<g class="{kk}"><rect x="60" y="{m.H - 30}" width="{m.W - 80}" height="20" fill="{c["foot"]}"/>'
                  f'<text x="64" y="{m.H - 16}" font-size="12" fill="{c["faint"]}" xml:space="preserve">any key stop, keep the text   esc drop it</text></g>')
    # three meter shapes, so the bars never move together (no animation-delay: each its own keyframes)
    for j, ks in enumerate(["0%,100%{transform:scaleY(.15)}30%{transform:scaleY(1)}60%{transform:scaleY(.45)}",
                            "0%,100%{transform:scaleY(.7)}25%{transform:scaleY(.2)}70%{transform:scaleY(.95)}",
                            "0%,100%{transform:scaleY(.4)}45%{transform:scaleY(.8)}80%{transform:scaleY(.1)}"]):
        m.tl.css.append(f"@keyframes mb{j}{{{ks}}}.mb{j}{{transform-box:fill-box;transform-origin:bottom;animation:mb{j} {1.0 + j * .17:.2f}s ease-in-out infinite}}")
    m.type(3.7, 4.3, said)  # the words land at once, as the transcript does
    m.you(5.2, said, gap=8)
    m.main(5.8, "told cookies: the buy button stays visible.")
    m.composer([(3.7, 5.2)])
    return m.svg("you press ctrl+r and say it; a level meter moves while you talk; your words land in the composer as text, and you send them.")

def t_quote(c):
    """ask about anything on screen: select lines, start typing, they come along as a quote."""
    m = Tui(c, 9); m.typed = []
    q = "what made the difference?"
    hl = ink.PAPER[c["mode"]]["hl"]
    m.header([(0, None, f'{m.acc("✓")} 1 done')])
    yy = m.main(0.2, "perf is done. signup went from 4.1 s to 0.9 s on a phone.")
    # the selection: the words light up as you drag over them
    x0 = m.fx + cells(":* perf is done. signup went from ") * CW
    k = m.tl.show(1.0, 4.3, dur=0.5)
    m.feed.insert(len(m.feed) - 1, f'<rect class="{k}" x="{x0 - 2:.0f}" y="{yy - 15}" width="{cells("4.1 s to 0.9 s") * CW + 4:.0f}" height="20" rx="2" fill="{hl}"/>')
    chip_w = 5 * CW
    x1 = 64 + chip_w + CW
    m.type(2.2, 3.4, q, x=x1)
    kc = m.tl.show(2.0, 4.3, dur=0.1)
    chip = lambda x, y: (f'<rect x="{x:.1f}" y="{y-15}" width="{chip_w:.1f}" height="20" rx="3" fill="{c["chip"]}"/>'
                         f'<text x="{x + CW:.1f}" y="{y}" font-size="14" fill="{c["acc"]}">❝ 1</text>')
    m.over.append(f'<g class="{kc}">{chip(64, m.top + 34)}</g>')
    m.you(4.3, q, gap=8, w=6 + cells(q), html=" " * 6 + E(q))
    kf = m.tl.show(4.3)
    m.feed.append(f'<g class="{kf}">{chip(m.fx, m.y - 27)}</g>')
    m.marks.append((4.3, m.y - 27, False, len(m.feed) - 1))
    m.row(5.0, f'{m.dim("@ perf to you:")} a 180 KB hero image, and the fonts load first.')
    m.composer([(2.0, 4.3)])
    return m.svg("you select '4.1 s to 0.9 s' in perf's answer and start typing: the lines come along as a quote chip, and perf answers about them.")

def t_model(c):
    """a model per agent: the big one for the hard job, a fast one for the chores. /model and /reasoning."""
    m = Tui(c, 10, panel=True); m.typed = []
    m.px = m.W - 200
    cmd1, cmd2 = "/model opus", "/reasoning hi"
    m.header([(0, None, f'{m.gust()} 3 working')])
    m.agents([(0, "main", [(0, ":*")]), (0, "perf", [(0, m.gust())]), (0, "cookies", [(0, m.gust())]), (0, "release", [(0, m.gust())])])
    for i, spans in enumerate([[(0, None, "opus·hi")], [(0, None, "opus·hi")], [(0, None, "haiku·lo")],
                               [(0, 3.4, "haiku·lo"), (3.4, 5.6, "opus·lo"), (5.6, None, "opus·hi")]]):
        for (t0, t1, s_) in spans:
            k = m.tl.show(t0, t1, dur=0.15)
            m.chrome.append(f'<text class="{k}" x="{m.W - 24}" y="{90 + i * 22}" text-anchor="end" font-size="12" fill="{c["acc"] if t0 else c["faint"]}">{s_}</text>')
    m.main(0.2, "release is on haiku: release notes are a chore.")
    press(m, 0.9, "⌥3"); m.row(1.2, f'{m.dim("⌥3 · now talking to release")}', size=13, gap=8)
    m.type(1.7, 2.6, cmd1); m.you(2.8, cmd1, gap=4)
    m.row(3.4, f'{m.dim("release · model opus")}', size=13)
    m.type(3.9, 4.9, cmd2); m.you(5.0, cmd2)
    m.row(5.6, f'{m.dim("release · reasoning hi")}', size=13)
    m.composer([(1.7, 2.8), (3.9, 5.0)], who=[(0, 1.2, "main"), (1.2, None, "release")])
    return m.svg("each agent shows its model in the panel. you switch to release and type /model opus, then /reasoning hi: its line goes from haiku·lo to opus·hi.")

SCENES = [("talk", t_talk), ("zen", t_zen), ("screenshot", t_screenshot),
          ("resume", t_resume), ("worktree", t_worktree), ("card", t_card), ("sync", t_sync), ("tools", t_tools), ("plugins", t_plugins),
          ("direct", t_direct), ("prs", t_prs), ("tokens", t_tokens), ("voice", t_voice), ("quote", t_quote), ("model", t_model)]

FEATS = [("quiet", f_quiet), ("steer", f_steer)] + SCENES

for mode, c in PAL.items():
    for name, f in [("hero", hero), ("demo", demo), ("demo-b", demo_b), ("team", team)]:
        open(os.path.join(HERE, f"{name}-{mode}.svg"), "w").write(f(c))
    os.makedirs(os.path.join(HERE, "feat"), exist_ok=True)
    for name, f in FEATS:
        open(os.path.join(HERE, "feat", f"{name}-{mode}.svg"), "w").write(f(c))
print("ok")
