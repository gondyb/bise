#!/usr/bin/env python3
"""bise.dev/book/screens: whole screens of today's design, drawn with the design system's mock language.
python3 docs/brand/design-system/screens.py  -> writes site/book/screens.html, prints any line whose width is off.
the parts (panel rows, divider, inbox box...) follow content.py; the PR screens follow docs/pr-design.md §4.1."""
import html, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../../.."))
OUT = os.path.join(ROOT, "site/book/screens.html")
sys.path.insert(0, HERE)
import make  # noqa: E402  (term lines, inline markdown, the width check)

# ------------------------------------------------------------------ the frame
def frame(feed, pan, divl, divr, comp, head="{d:~/acme · # 1 in the inbox}", split=True, W=100, S=66):
    """a whole bise screen. feed rows start at column 1; panel rows at S+1 (absolute pads inside are fine).
    a feed row starting with % is an open inbox item: raised one step inside its box."""
    L = []
    if S:
        L.append("{r:╭─} {b:bise} {a::*} " + (f"{{|{S}─}}{{r:┬}}{{>─}} " if split else "{>─} ") + head + " {r:─╮}")
    else:
        L.append("{r:╭─} {b:bise} {a::*} {>─} " + head + " {r:─╮}")
    n = max(len(feed), len(pan) if S else 0)
    for i in range(n):
        f = feed[i] if i < len(feed) else ""
        mark = f.startswith("%")
        if mark: f = f[1:]
        if S:
            p = pan[i] if i < len(pan) else ""
            s = "{r:│}" + f + f"{{|{S}}}{{r:│}}" + p + f"{{|{W - 1}}}{{r:│}}"
        else:
            s = "{r:│}" + f + f"{{|{W - 1}}}{{r:│}}"
        L.append(("%" if mark else "") + s)
    tail = (" " + divr + " {r:─┤}") if divr else "{r:─┤}"
    L.append("{r:├─} " + divl + (f" {{|{S}─}}{{r:┴}}{{>─}}" if S else " {>─}") + tail)
    for c in comp:
        L.append("^{r:│}" + c + f"{{|{W - 1}}}{{r:│}}")
    L.append("{r:╰}{>─}{r:╯}")
    return L

def ibox(title, right, rows, E=65):
    """the inbox box in the feed: box from column 2 to E-1. rows are inner text (from column 4),
    or a row starting with % = an open item's row (raised, with the heavy bar)."""
    out = [f" {{r:╭─}} {{f:{title}}} {{>{E}─}} {{f:{right}}} {{r:─╮}}"]
    for r in rows:
        if r.startswith("%"):
            out.append("% {r:│} {a:┃}" + r[1:] + f"{{|{E - 1}}}{{r:│}}")
        else:
            out.append(" {r:│} " + r + f"{{|{E - 1}}}{{r:│}}")
    out.append(f" {{r:╰}}{{>{E}─}}{{r:╯}}")
    return out

you = lambda t: "  {a:│} " + t
me = lambda t: "  {ab::*} " + t
cont = lambda t: "     " + t
row = lambda n, g, name, t="", pct="", lead="  ", mark="": f"{lead}{{f:{n}}} {g} {name}" + (f"{{|88}}{{d:{t}}}" if t else "") + (f"{{|93}}{{d:{pct}}}" if pct else "") + (f"{{|97}}{mark}" if mark else "")
held = lambda n, g, name, word, lead="  ", acc=False, mark="": f"{lead}{{f:{n}}} {g} {name}{{|88}}" + (f"{{a:{word}}}" if acc else f"{{d:{word:>8}}}") + (f"{{|97}}{mark}" if mark else "")
under = lambda t: "      " + t  # held: the line under a solo-worktree row, at the name's column
top = lambda branch, pr="": "{r:╭─} {d:ψ " + branch + "} {>99─}" + (f" {pr} {{r:─}}" if pr else "{r:──}")
lid = lambda t: "{r:│}  " + t

G = "{g1}"
DIV = "{d:you →} {a:main} {f:·} {d:opus 5.5} {f:·} {d:high} {f:·} {d:yolo}"
DIV_W = DIV + " {f:·} {g5}"
DIV_HELD = DIV_W + " {d:working · 1m}"
CTX, CTX_HELD = "{d:58k · 22%}", "{d:58k / 262k tokens · 22%}"
KEYS = "      {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}"
KEYS_IN = KEYS + "   {t:ctrl+1} {d:inbox}"
KEYS_HELD = "      {t:ctrl+c} {d:interrupt}   {t:ctrl+f} {d:find}   {t:ctrl+1} {d:open an inbox item}   {t:ctrl+s} {d:find agent}"
KEYS_ITEM = "      {t:1-3} {d:answer}   {t:←→} {d:choose}   {t:↑↓} {d:other items}   {t:esc} {d:back to your message}"
comp = lambda text, keys=KEYS_IN: ["  {a:│}", "  {a:│}   " + text + "{a:▌}", "  {a:│}", keys]
COMP_EMPTY = lambda keys=KEYS: ["  {f:│}", "  {f:│}   {d:what's on your mind?}", "  {f:│}", keys]
COMP_ANSWER = ["  {a:│}", "  {a:│}   {a:▌}", "  {a:│}", KEYS_ITEM]
DRAFT = "and give the sad dog a hat"

# ------------------------------------------------------------------ the cast
# 0 main · 1 perf ✓ · 2 dark-mode + 4 i18n share sb/dark-mode · 3 sad-404 asks · 5 release
PAN = ["", "  {d:agents}",
       row(0, G, "main {a::*} {d:@ 2}", " 1m", "22%"),
       row(1, "{a:✓}", "perf"),
       row(3, "{a:?}", "sad-404", pct="18%"),
       row(5, "{d:○}", "release", pct=" 8%"),
       "",
       top("sb/dark-mode", "{d:↑}"),
       row(2, G, "dark-mode", " 3m", "12%", lead="{r:│} "),
       row(4, G, "i18n {a:•}", "42s", "31%", lead="{r:╰} "),
       "",
       "  {d:inbox}",
       "  {f:1} {a:?} sad-404  {d:the dog: a h…}"]
PAN_HELD = ["", "  {d:agents}",
            held(0, G, "main {a::*} {d:@ 2}", "working"),
            held(1, "{a:✓}", "perf", "done"),
            held(3, "{a:?}", "sad-404", "asks you", acc=True),
            held(5, "{d:○}", "release", "idle"),
            "",
            top("sb/dark-mode", "{d:↑ #412}"),
            lid("{d:changes asked · checks pass}"),
            held(2, G, "dark-mode", "working", lead="{r:│} "),
            held(4, G, "i18n {a:•}", "working", lead="{r:╰} "),
            "",
            "  {d:inbox}",
            "  {a:1} {a:?} sad-404  {d:the dog: a h…}"]

FEED = ["",
        you("signup is slow on mobile. and the 404 is sad {a:✓✓}"),
        "",
        me("on it: perf, dark-mode and sad-404 started."),
        "  {f:▸ 9 messages between 4 agents}",
        "",
        "  {ch: ✉ dark-mode → main }",
        "    {d:which gray for the borders?}",
        me("i answered dark-mode: the gray in tokens.css."),
        "",
        "  {d:$} {d:runs the signup benchmark}{>63} {d:✓ 4.2s}",
        "  {a:✓} perf is done {f:·} signup 4.1 s → 0.9 s",
        ""]
INBOX1 = ibox("inbox · 1 waiting for you", "ctrl+1 open", ["{f:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>62} {f:4m}"])
INBOX1_HELD = ibox("inbox · 1 waiting for you", "ctrl+1 open", ["{a:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>62} {f:4m}"])
HEAD_HELD = "{d:~/acme ·} {g3} {d:3 working · ✓ 1 done · ↑ 1 PR · # 1 in the inbox}"

S = []  # (id, title, text, [(tab, lines, W)], caption)
def screen(sid, title, text, tabs, cap=""):
    S.append((sid, title, text, tabs, cap))

# ------------------------------------------------------------------ 1. the main screen
screen("main", "the main screen",
"one frame, one thread, one panel. you talk to main; the agents work on the right; what needs you waits in its box above the divider. hold ctrl and every word comes back in its place.",
[("rest", frame(FEED + INBOX1, PAN, DIV_W, CTX, comp(DRAFT)), 100),
 ("ctrl held", frame([l.replace("{f:▸ 9 messages between 4 agents}", "{f:▸} {a:ctrl+o} {f:expand}") for l in FEED] + INBOX1_HELD,
                     PAN_HELD, DIV_HELD, CTX_HELD, comp(DRAFT, KEYS_HELD), head=HEAD_HELD, split=False), 100)],
"100 columns. the panel: your agents as rows; two agents that share a worktree get a small box (its branch and PR in the border); then the inbox.")

# ------------------------------------------------------------------ 2. you ask, main starts agents
P_START = ["", "  {d:agents}", row(0, G, "main {a::*}", " 2s", " 3%")]
P_ON = ["", "  {d:agents}", row(0, G, "main {a::*}", " 9s", " 6%"), row(2, G, "sad-404", " 4s", " 2%"),
        row(1, G, "perf", " 4s", " 2%", mark="{d:ψ}")]
F_ASK = ["", you("signup is slow on mobile, and the 404 is sad {a:✓✓}"), ""]
screen("start", "you ask, main starts agents",
"you say it the way you'd say it to a person. main splits the work, starts the agents, picks who needs a worktree, and tells you in one line.",
[("read", frame(F_ASK, P_START, DIV_W, "{d:4k · 2%}", COMP_EMPTY(), head="{d:~/acme}"), 100),
 ("on it", frame(F_ASK + [me("on it: perf and sad-404 started. perf gets its own"), cont("worktree: it touches the build."),
                          "  {f:▸ 4 messages between 3 agents}"], P_ON, DIV_W, "{d:9k · 3%}", COMP_EMPTY(), head="{d:~/acme}"), 100),
 ("you change your mind", frame(F_ASK + [me("on it: perf and sad-404 started. perf gets its own"), cont("worktree: it touches the build."),
                          "  {f:▸ 4 messages between 3 agents}", "", you("oh and the dog wears a hat {a:✓✓}"), "",
                          me("told sad-404: a hat.")], P_ON, DIV_W, "{d:11k · 4%}", COMP_EMPTY(), head="{d:~/acme}"), 100)],
"the ✓✓ in accent: main has read it. you never wait for a turn to end to say more.")

# ------------------------------------------------------------------ 3. main answers for you
F_RT = ["", me("on it: perf, dark-mode and sad-404 started."), "  {f:▸ 9 messages between 4 agents}", "",
        "  {ch: ✉ dark-mode → main }", "    {d:which gray for the borders?}",
        me("i answered dark-mode: the gray in tokens.css."), cont("{d:like everywhere else.}"), ""]
P_RT = [l for l in PAN if "inbox" not in l and "the dog" not in l]
screen("router", "you're not the router",
"agents ask main first. main answers the obvious ones the way you would, and says why in one line. only a real decision reaches you, in the inbox.",
[("main answers", frame(F_RT, P_RT[:-1], DIV_W, CTX, COMP_EMPTY(), head="{d:~/acme}"), 100),
 ("only you can say", frame(F_RT + ["  {ch: ✉ sad-404 → main }", "    {d:the dog: a hat, or a scarf?}",
                                    me("that one's yours. it's in your inbox."), ""] + INBOX1, PAN, DIV_W, CTX, COMP_EMPTY(KEYS_IN)), 100),
 ("you answered", frame(F_RT + ["  {ch: ✉ sad-404 → main }", "    {d:the dog: a hat, or a scarf?}",
                                me("that one's yours. it's in your inbox."), "", "  {a:✓} {d:you answered sad-404: a hat}"],
                        P_RT[:-1], "{a:✓} inbox clear", "", COMP_EMPTY(), head="{d:~/acme}"), 100)],
"what main told an agent is one line in its thread; the messages themselves are folded, never gone.")

# ------------------------------------------------------------------ 4. the inbox: an approval
P_AP = ["", "  {d:agents}", row(0, G, "main {a::*}", " 1m", "22%"), row(3, "{a:?}", "sad-404", pct="18%"),
        row(5, "{a:?}", "release", pct=" 8%"), row(2, G, "dark-mode", " 3m", "12%", mark="{d:↑}"),
        "", "  {d:inbox}", "  {f:1} {a:?} release  {d:$ npm publish…}", "  {f:2} {a:?} sad-404  {d:the dog: a h…}"]
F_AP = ["", me("perf's fix is in. release is shipping 2.5.0."), "  {f:▸ 6 messages between 3 agents}", ""]
IT1 = ["%", "% {ab:?} {b:release wants to run}{>62} {d:1 of 2 · 2m}", "%   $ npm publish --access public", "%",
       "%   {d:it publishes the package: everyone can install it.}", "%   {d:release ships 2.5.0 · last step: ✓ npm run build}", "%",
       "%   {a:1} {d:allow}   {a:2} {d:always allow npm publish * here}   {a:3} {d:no}", "%   {f:or type why not, ⏎ says no}", "%"]
ROW2 = "{f:2} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>62} {f:4m}"
DIV_ANS = lambda who: "{d:you →} {a:? " + who + "} {f:·} {d:your answer}"
screen("inbox", "the inbox: one key answers",
"an item opens where its row was. one digit answers it, the next one opens by itself, and your draft waits until the inbox is clear.",
[("waiting", frame(F_AP + ibox("inbox · 2 waiting for you", "ctrl+1-2 open",
                    ["{f:1} {a:?} release {f:·} $ npm publish --access public{>62} {f:2m}", ROW2]),
                   P_AP, DIV_W, CTX, comp(DRAFT, KEYS_IN)), 100),
 ("ctrl+1 opens it", frame(F_AP + ibox("inbox · 2 waiting for you", "ctrl+1-2 open", IT1 + [ROW2]),
                           P_AP, DIV_ANS("release"), "", COMP_ANSWER), 100),
 ("1: the next one opens", frame(F_AP + ibox("inbox · 1 waiting for you", "ctrl+1 open",
                    ["{a:✓} {d:you allowed release: npm publish --access public}",
                     "% {ab:?} {b:sad-404 asks}{>62} {d:1 of 1 · 4m}", "%   the dog: a hat, or a scarf?", "%",
                     "%   {a:1} {d:a hat}   {a:2} {d:a scarf}", "%   {f:or type your answer, ⏎ sends it}"]),
                    P_AP, DIV_ANS("sad-404"), "", COMP_ANSWER), 100),
 ("1: inbox clear", frame(F_AP + ["  {a:✓} {d:you allowed release: npm publish --access public}", "  {a:✓} {d:you answered sad-404: a hat}"],
                          [l for l in P_AP if "inbox" not in l and "…" not in l], "{a:✓} inbox clear", "", comp(DRAFT, KEYS)), 100)],
"1, then 1: two answers, two keys. after the last one the box goes, your draft comes back, and the divider says ✓ inbox clear for 2 s.")

# ------------------------------------------------------------------ 5. approvals: yolo or auto
P_AU = [l for l in PAN if "inbox" not in l and "the dog" not in l and "sad-404" not in l][:-1]
TIP = ["{|28}{r:╭─}{>64─}{r:─╮}", "{|28}{r:│} {d:you're in yolo: everything runs.}{|63}{r:│}",
       "{|28}{r:│} {t:⇧⇥} {d:changes it.}{|63}{r:│}", "{|28}{r:╰}{>64─}{r:╯}"]
F_AU = ["", me("on it: perf and dark-mode started."), "  {f:▸ 6 messages between 3 agents}", ""]
screen("approvals", "approvals: yolo or auto",
"two modes for every agent, on the divider. yolo runs everything (the default). auto runs what's safe at once, has a small model check the rest, and asks you only when it's risky.",
[("yolo, the first time", frame(F_AU + [""] * 7 + TIP, P_AU, DIV.replace("{d:yolo}", "{a:yolo}") + " {f:·} {g5}", CTX,
                                 comp(DRAFT, KEYS), head="{d:~/acme}"), 100),
 ("⇧⇥: auto", frame(F_AU, P_AU, DIV.replace("{d:yolo}", "{a:auto}") + " {f:·} {g5}", CTX,
                     comp(DRAFT, "      {a:auto} {f:·} {d:safe calls run, risky ones ask you}"), head="{d:~/acme}"), 100),
 ("checking…", frame(F_AU + ["  {d:$} {d:installs the deps}{>63} {d:✓ 3.1s}", "  {d:$} {t:pushes sb/perf to origin}{>63} {d:checking…}"],
                     P_AU, DIV.replace("{d:yolo}", "{d:auto}") + " {f:·} {g5}", CTX, comp(DRAFT, KEYS), head="{d:~/acme}"), 100),
 ("a hard rule", frame(F_AU + ibox("inbox · 1 waiting for you", "ctrl+1 open",
                        ["%", "% {ab:?} {b:perf wants to run}{>62} {d:1 of 1 · 1m}", "%   $ git push origin main", "%",
                         "%   {d:it pushes straight to main. this one always asks.}", "%",
                         "%   {a:1} {d:allow}   {a:3} {d:no}", "%   {f:or type why not, ⏎ says no}", "%"]),
                       P_AU, DIV_ANS("perf"), "", COMP_ANSWER), 100)],
"reads, edits in the repo, messages between agents and your saved rules never ask. a hard rule (main, secrets, a wipe) has no \"always\".")

# ------------------------------------------------------------------ 6. inside an agent
F_AG = ["", "  {ch: ✉ main → dark-mode }", "    {d:dark mode for the settings page. tokens.css has the grays.}", "",
        "  {d:$ ▸ 4 commands · reads the theme files}{>63} {d:✓ 1.2s}", "",
        "  {ch: ✉ dark-mode → main }", "    {d:which gray for the borders?}",
        "  {ch: ✉ main → dark-mode }", "    {d:the one in tokens.css, like everywhere else.}", "",
        "  {d:± edit web/src/theme.ts ✓ +18 −2 ▸}", "  {d:$} {t:runs the visual tests}{>63} {g1} {d:12s}", ""]
P_AG = [l.replace("dark-mode{|88}", "{b:dark-mode}{|88}") for l in PAN]
screen("agent", "inside an agent",
"⌥2 or a click on its row opens an agent. you see everything it did, and you can talk to it directly. esc brings you back to main.",
[("dark-mode's view", ["{r:╭─} {b:bise} {a::*} {f:·} {d:dims the borders to tokens.css} {|66─}{r:┬}{>─} {d:~/acme} {r:─╮}"] +
                      frame(F_AG, P_AG, "{d:you →} {a:dark-mode} {f:·} {d:sonnet 5.5} {f:·} {d:low} {f:·} {d:yolo} {f:·} {d:ψ} {f:·} {g5}",
                            "{d:31k · 12%}", COMP_EMPTY("      {t:esc} {d:back to main}   {t:@} {d:file}   {t:$} {d:skills}   {t:/} {d:commands}"))[1:], 100)],
"the header carries the agent's role line: what it's doing now, rewritten after each of its turns.")

# ------------------------------------------------------------------ 7. tool calls
F_TC = ["", you("why is the hero so slow? {a:✓✓}"), "",
        "  {d:$} {d:installs the deps}{>63} {d:✓ 3.1s}",
        "  {d:ƒ} {t:reads the open issues on GitHub}{>63} {g1} {d:12s}",
        "  {e:$} {d:runs the tests}{>63} {e:✗ exit 1 · 0.8s}",
        "     {d:FAIL signup.spec.ts › keeps the email after a re…}",
        "  {d:$ ▸ 6 commands · weighs the hero image}{>63} {d:✓ 3.2s}", ""]
BOX = ["  {r:╭─} {d:$ weighs the hero image ✓ 0.1s} {>64─}{r:─╮}", "  {r:│}  ls -la public/hero.png{|63}{r:│}",
       "  {r:├}{>64─}{r:┤}", "  {r:│}  {d:-rw-r--r--  1 gab  staff  4.2M  hero.png}{|63}{r:│}", "  {r:╰}{>64─}{r:╯}", ""]
P_TC = ["", "  {d:agents}", row(0, G, "main {a::*}", "18s", " 9%")]
screen("tools", "tool calls: one row each",
"each command is one dim row: what the model says it's doing, in your language, and its state. a failure shows its first error line. a click opens one, ctrl+o opens them all.",
[("rows", frame(F_TC, P_TC, DIV_W, "{d:23k · 9%}", COMP_EMPTY(), head="{d:~/acme}"), 100),
 ("one opened", frame(F_TC + BOX, P_TC, DIV_W, "{d:23k · 9%}", COMP_EMPTY(), head="{d:~/acme}"), 100)],
"$ bash, ƒ TypeScript. done is dim, never green. 4 done calls in a run fold into ▸ n commands.")

# ------------------------------------------------------------------ 8. /models
PICK = [
 ("/models", ["{b:which model does what?}{>}", "{d:each role picks a provider, then a model. one provider can serve several.}{>}", "{>}",
              "{a:›} main         Mistral  {d:mistral-large-latest · high}{>}", "  agents       {d:same as main · Mistral · mistral-large-latest}{>}",
              "  small jobs   {d:auto · Mistral · ministral-8b-latest}{>}", "  checker      {d:auto · Mistral · mistral-small-latest}{>}",
              "  voice        Mistral  {d:voxtral-mini-latest}{>}", "{>}",
              "{t:↑↓} {d:choose} {f:·} {t:enter} {d:change} {f:·} {t:esc} {d:back}{>}"], 78),
 ("1 · which provider?", ["{b:agents: which provider?}{>}", "{d:now: same as main · Mistral · mistral-large-latest}{>}", "{>}",
              "{a:›} same as main   {d:Mistral · mistral-large-latest}{>}", "  Mistral        {a:✓} ready {f:· main, small jobs use it}{>}",
              "  Anthropic      {a:✓} ready{>}", "  OpenAI         {d:not set up}{>}", "  {d:more providers…}{>}"], 78),
 ("2 · which model?", ["{b:agents · Anthropic: which model?}{>}", "{d:type to filter, or a model id that isn't listed.}{>}",
              "{a:›} {d:son}{a:▌}{>}", "{>}", "{a:›} claude-sonnet-5-5   {a:recommended}{>}", "  claude-sonnet-4-6{>}"], 78),
 ("back, ✓", ["{b:which model does what?}{>}", "{d:each role picks a provider, then a model. one provider can serve several.}{>}", "{>}",
              "  main         Mistral  {d:mistral-large-latest · high}{>}", "{a:›} agents       Anthropic  {d:claude-sonnet-5-5 · medium}  {a:✓}{>}",
              "  small jobs   {d:auto · Mistral · ministral-8b-latest}{>}", "  checker      {d:auto · Mistral · mistral-small-latest}{>}",
              "  voice        Mistral  {d:voxtral-mini-latest}{>}"], 78)]
screen("models", "/models: which model does what",
"every role picks a provider, then a model, then how hard it thinks. what works comes first; a step with one choice is skipped; esc goes back one step.",
PICK, "full screen, in place of the thread. /provider holds the keys; /model is the quick list for the agent in view.")

# ------------------------------------------------------------------ 9. worktrees and PRs
P_PR = ["", "  {d:agents}", row(0, G, "main {a::*} {d:@ 2}", " 1m", "22%"), row(5, "{d:○}", "release", pct=" 8%"),
        row(2, G, "login-fix", " 5m", " 9%", mark="{e:↑}"), row(3, "{a:?}", "sad-404", pct="18%", mark="{a:↑}"),
        row(6, G, "emoji-csv", " 2m", "14%", mark="{d:ψ}"), "",
        top("sb/dark-mode", "{d:↑}"), row(1, G, "dark-mode", " 3m", "12%", lead="{r:│} "), row(4, G, "i18n {a:•}", "42s", "31%", lead="{r:╰} "), "",
        "  {d:inbox}", "  {f:1} {a:?} sad-404  {d:#409 is approv…}"]
P_PR_HELD = ["", "  {d:agents}", held(0, G, "main {a::*} {d:@ 2}", "working"), held(5, "{d:○}", "release", "idle"),
             held(2, G, "login-fix", "working", mark="{e:↑}"), under("{d:#415 ·} {e:checks fail}{d:: e2e…}"),
             held(3, "{a:?}", "sad-404", "asks you", acc=True, mark="{a:↑}"), under("{d:#409 · approved · checks…}"),
             held(6, G, "emoji-csv", "working", mark="{d:ψ}"), under("{d:no PR yet · 2 commits}"), "",
             top("sb/dark-mode", "{d:↑ #412}"), lid("{d:changes asked · checks pass}"),
             held(1, G, "dark-mode", "working", lead="{r:│} "), held(4, G, "i18n {a:•}", "working", lead="{r:╰} "), "",
             "  {d:inbox}", "  {a:1} {a:?} sad-404  {d:#409 is approv…}"]
F_PR = ["", me("dark-mode opened #412: dark mode with tokens.css."), "  {f:▸ 2 messages from GitHub}", "",
        me("alice asked for changes on #412: the toggle's"), cont("contrast, a test, a name. dark-mode is on them."), "",
        me("#415's checks fail on the login test"), cont("(e2e/login.spec.ts:42). login-fix is on it."), "",
        "  {a:✓} {d:perf's #401 merged · perf archived, its worktree removed}", ""]
ROW_M = "{f:1} {a:?} sad-404 {f:·} #409 is approved, checks pass. merge it?{>62} {f:1m}"
IB_PR = ibox("inbox · 1 waiting for you", "ctrl+1 open", [ROW_M])
IT_M = ibox("inbox · 1 waiting for you", "ctrl+1 open",
            ["%", "% {ab:?} {b:sad-404's PR is ready}{>62} {d:1 of 1 · 1m}", "%   #409 · the sad 404 gets a dog in a hat", "%",
             "%   {d:alice approved it, the checks pass.}", "%   {d:it merges with your gh login, squash.}", "%",
             "%   {a:1} {d:merge it}   {a:2} {d:not yet}", "%"])
P_MERGED = [l for l in P_PR if "sad-404" not in l and "inbox" not in l][:-1]
screen("prs", "worktrees and pull requests",
"agents share your folder; one that needs isolation gets a worktree. alone in it, the agent is a row with one mark in the last column: ψ no PR yet, ↑ its PR. when several agents share a worktree, they get a small box: git lives in its border (ψ, the branch, the PR's ↑), agents live in rows. main says what GitHub said, once, in words.",
[("rest", frame(F_PR + IB_PR, P_PR, DIV_W, CTX, comp("ship the sad dog once it's merged")), 100),
 ("ctrl held", frame(F_PR + IB_PR, P_PR_HELD, DIV_HELD, CTX_HELD, comp("ship the sad dog once it's merged", KEYS_HELD),
                     head="{d:~/acme · lands via PRs ·} {g3} {d:5 working · ↑ 3 PRs · # 1 in the inbox}", split=False), 100),
 ("ready to merge", frame(F_PR + IT_M, P_PR, DIV_ANS("sad-404"), "", COMP_ANSWER), 100),
 ("1: merged", frame(F_PR[:-1] + ["  {a:✓} {d:sad-404's #409 merged · sad-404 archived, worktree removed}"], P_MERGED,
                     "{a:✓} inbox clear", "", comp("ship the sad dog once it's merged", KEYS), head="{d:~/acme}"), 100)],
"↑ is dim while nothing is yours to do, red when checks fail, pink only when an inbox item asks you (ready to merge). no agent ever merges. merging from the inbox comes next; today you merge on GitHub.")

# ------------------------------------------------------------------ 10. narrow
screen("narrow", "a narrow terminal",
"under 90 columns the panel goes. the header keeps the short counts, ⌥ + a number still opens an agent, and the inbox keeps its box.",
[("80 columns", frame(FEED + ibox("inbox · 1 waiting for you", "ctrl+1 open",
                       ["{f:1} {a:?} sad-404 {f:·} the dog: a hat, or a scarf?{>75} {f:4m}"], E=78),
                      [], "{d:you →} {a:main} {f:·} {d:opus·hi} {f:·} {d:yolo} {f:·} {g3}", "{d:58k · 22%}", comp(DRAFT),
                      head="{g1} {d:3 · ? 1 · ✓ 1 · # 1}", W=80, S=None), 80)],
"short on room, things go in a written order: the long context, the context, the branch, then opus 5.5 · high becomes opus·hi. the mode goes last.")

# ------------------------------------------------------------------ render
def render_tabs(tabs, where):
    tb = '<div class="tabs">' + "".join(f'<button class="{"on" if i == 0 else ""}" data-i="{i}">{html.escape(t[0])}</button>' for i, t in enumerate(tabs)) + "</div>" if len(tabs) > 1 else ""
    scr = ""
    for i, (label, lines, W) in enumerate(tabs):
        out = []
        for l in lines:
            r2 = l.startswith("%")
            h = make.line(l[1:] if r2 else l, W, f"{where}/{label}")
            if r2:
                E = 64 if W == 100 else 77
                h = h.replace('<div class="ln">', f'<div class="ln r2" style="--a:3ch;--b:{E}ch">', 1)
            out.append(h)
        scr += f'<div class="screen{" on" if i == 0 else ""}" style="--w:{W}">' + "".join(out) + "</div>"
    return tb, scr

def build():
    shell = open(os.path.join(HERE, "shell.html"), encoding="utf-8").read()
    css_vars = shell[shell.index(":root,[data-theme"):shell.index("*{box-sizing")]
    css_term = shell[shell.index("/* terminal mocks */"):shell.index("/* swatches */")]
    gust = shell[shell.index("  // the gust"):shell.index("})();\n</script>")]
    annotate = shell[shell.index("<script>/* notes"):shell.index("</body>")]
    nav = "".join(f'<a href="#{s[0]}"><span>{n + 1:02d}</span> {html.escape(s[1])}</a>' for n, s in enumerate(S))
    figs = []
    for n, (sid, title, text, tabs, cap) in enumerate(S):
        tb, scr = render_tabs(tabs, sid)
        figs.append(f'<section id="{sid}"><h2><span class="n">{n + 1:02d}</span>{html.escape(title)}</h2><p>{make.inline(text)}</p>'
                    f'<figure class="ex">{tb}<div class="scr">{scr}</div>' + (f"<figcaption>{make.inline(cap)}</figcaption>" if cap else "") + "</figure></section>")
    doc = f"""<!doctype html>
<html lang="en" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>bise :* — every screen</title>
<meta name="description" content="every screen of bise, drawn as it is today: the thread, the agents panel, the inbox, approvals, models, worktrees and pull requests.">
<link rel="canonical" href="https://bise.dev/book/screens">
<meta name="theme-color" content="#141211">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<meta property="og:image" content="https://bise.dev/og.png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&display=swap" rel="stylesheet">
<style>
{css_vars}*{{box-sizing:border-box;margin:0;padding:0}}
html{{-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale;scroll-padding-top:24px}}
body{{background:var(--bg);color:var(--text);font-family:"JetBrains Mono",ui-monospace,Menlo,monospace;font-size:14.5px;line-height:1.7}}
a{{color:var(--acc);text-decoration:none}}a:hover{{text-decoration:underline}}
main{{max-width:1000px;margin:0 auto;padding:56px 28px 120px}}
.top{{display:flex;gap:18px;align-items:baseline;color:var(--dim);font-size:13px;margin-bottom:40px}}
.top .sp{{flex:1}} .top b{{color:var(--text);font-size:16px}} .top b span{{color:var(--acc)}}
.toggle{{display:flex;gap:4px}}
.toggle button{{font:inherit;font-size:12px;background:transparent;color:var(--dim);border:1px solid var(--line);border-radius:4px;padding:2px 9px;cursor:pointer}}
.toggle button.on{{color:var(--acc);border-color:var(--acc)}}
h1{{font-size:30px;line-height:1.25;margin-bottom:14px}}
.lead{{color:var(--dim);max-width:72ch;margin-bottom:28px}}
nav.idx{{columns:2;max-width:72ch;margin-bottom:56px;font-size:13.5px}}
nav.idx a{{display:block;color:var(--dim)}} nav.idx a:hover{{color:var(--text);text-decoration:none}} nav.idx span{{color:var(--faint);margin-right:1ch}}
section{{margin-bottom:64px}}
h2{{font-size:19px;margin-bottom:8px}} h2 .n{{color:var(--acc);margin-right:1.2ch;font-weight:400}}
section>p{{color:var(--dim);max-width:76ch;margin-bottom:6px}}
code{{font:inherit;color:var(--text);background:var(--bg2);border-radius:3px;padding:0 4px}}
{css_term}.screen .ln.r2{{background:linear-gradient(90deg,transparent var(--a),var(--t-raised2) var(--a),var(--t-raised2) var(--b),transparent var(--b))}}
@media (max-width:700px){{main{{padding:32px 16px 80px}} nav.idx{{columns:1}}}}
</style>
</head>
<body>
<main>
<div class="top"><b>bise <span>:*</span></b><a href="/book/">brand book</a><a href="/design/">design system</a><span class="sp"></span>
<div class="toggle"><button data-t="dark" class="on">dark</button><button data-t="light">light</button></div></div>
<h1>every screen</h1>
<p class="lead">bise as it looks today, screen by screen, in the terminal's real colors and spacing. the tabs show the same screen a moment later, or with ctrl held. why each part looks the way it does: the <a href="/design/">design system</a>.</p>
<nav class="idx">{nav}</nav>
{"".join(figs)}
</main>
<script>
(() => {{
  document.querySelectorAll("figure.ex .tabs").forEach(tb => tb.addEventListener("click", e => {{
    const b = e.target.closest("button"); if (!b) return;
    tb.querySelectorAll("button").forEach(x => x.classList.toggle("on", x === b));
    tb.parentElement.querySelectorAll(".screen").forEach((s, i) => s.classList.toggle("on", i == b.dataset.i));
  }}));
  document.querySelectorAll(".toggle button").forEach(b => b.onclick = () => {{
    document.documentElement.dataset.theme = b.dataset.t;
    document.querySelectorAll(".toggle button").forEach(x => x.classList.toggle("on", x === b)); }});
{gust}}})();
</script>
{annotate}</body>
</html>
"""
    open(OUT, "w", encoding="utf-8").write(doc)
    print(len(S), "screens", len(doc), "bytes")
    for w, s, why in make.BAD: print("BAD", w, why, "|", s)

if __name__ == "__main__":
    build()
