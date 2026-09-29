"""The first launch (BISE-60, book §15, mockup tui-onboarding.html) in a
real terminal (tmux), on a throwaway hub with the fake provider and an
empty bise home (BISE_HOME) and HOME in a temp dir:

- a first launch plays the five steps, enter by enter; the screens are
  saved in $SB_ONBOARDING_SHOTS (default: the temp dir) for the mockup
  comparison;
- the second launch goes straight to the normal UI;
- esc on a fresh state root skips it and marks it seen;
- the one-time hints (BISE-61) of the first run: the first agent, the
  first card, the first message between agents; each one goes away when
  used or after the next message, and is marked in prefs.json.

python3 -u projects/switchboard/tests/tui_onboarding_tmux.py
"""
import json
import re
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "⏎ send   @ agent"  # the key bar (BISE-98/99)


def flat(sc):
    """The screen's rows trimmed and joined: a phrase reads across a wrap
    (the onboarding's content column is 64 wide, book §15 'Layout')."""
    return " ".join(r.strip() for r in sc.splitlines() if r.strip())


def key_envs():
    """Every key variable of bise's catalog (+ the GOOGLE_API_KEY alias):
    the model step lists each provider whose key it finds."""
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY"})


def env(state_root, home):
    # the fake env's MISTRAL_API_KEY stays: the one key the step finds
    blank = " ".join("%s=" % k for k in key_envs() if k != "MISTRAL_API_KEY")
    return "BISE_HOME=%s HOME=%s %s" % (state_root, home, blank)


def prefs(root):
    """The prefs.json of a bise home (BISE-160), {} before any."""
    try:
        with open(os.path.join(root, "prefs.json")) as f:
            return json.load(f)
    except FileNotFoundError:
        return {}


def main():
    E = e2e.Env()
    E.env.pop("SB_ONBOARDING", None)
    shots = os.environ.get("SB_ONBOARDING_SHOTS") or os.path.join(E.tmp, "shots")
    os.makedirs(shots, exist_ok=True)
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    root = os.path.join(E.tmp, "state-root")

    def shot(name, sc):
        with open(os.path.join(shots, name + ".txt"), "w") as f:
            f.write(sc)
        print("---- %s ----\n%s" % (name, sc))

    with tui_session(120, 34, env(root, home), E=E) as t:
        # 1 welcome: typed, then the :* pop
        sc = t.wait("press enter ↵", 30)
        time.sleep(0.5)
        sc = t.screen()
        for s in ["hi, i'm bise :*", "bise /beez/ · french, n.", "1. a quick kiss on the cheek :*",
                  "2. a brisk north wind", "3. a terminal where your agents ship while you think",
                  "ideas in. little kisses out. also pull requests.", "● ○ ○ ○ ○ ○"]:
            assert s in flat(sc), sc
        rows = sc.splitlines()
        hi = next(i for i, r in enumerate(rows) if "hi, i'm bise :*" in r)
        assert "bise /beez/" in rows[hi + 2], sc      # the definition, a blank row under the name
        shot("1-welcome", sc)
        # 2 theme: two previews, ←→ switches live
        t.keys("Enter")
        sc = t.wait("←→ switch · enter keep")
        for s in ["so i picked dark.", "you can change it any time with /theme.", "fix the flaky login test",
                  "on it: auth-fix takes it.", "auth-fix is done.", "○ ● ○ ○ ○ ○"]:
            assert s in flat(sc), sc
        shot("2-theme", sc)
        t.keys("Right")
        time.sleep(0.3)
        t.keys("Left")
        # 3 model: the fake env has MISTRAL_API_KEY and a mistral model
        t.keys("Enter")
        sc = t.wait("which model should do the work?")
        for s in ["i found a key in your environment.", "1 · use MISTRAL_API_KEY found",
                  "mistral, already set up. nothing to paste.", "2 · paste another key",
                  "3 · sign in with the browser", "↑↓ choose · enter ok"]:
            assert s in flat(sc), sc
        shot("3-model", sc)
        # 4 folder and the honest line
        t.keys("Enter")
        sc = t.wait("enter ok · o another folder")
        for s in ["i'll work in ", "git repo ✓", "one honest thing: agents run commands here without asking you."]:
            assert s in flat(sc), sc
        shot("4-folder", sc)
        t.keys("o")
        t.wait("another folder? start me there")
        # 5 the three lines, one by one
        t.keys("Enter")
        sc = t.wait("enter, and say what's on your mind.")
        for s in ["how it works, in three lines:", "you talk to me. i start agents for the work, in the background.",
                  "they show up on the right.", "when someone needs you, you get a card. the rest can wait."]:
            assert s in flat(sc), sc
        shot("5-how-it-works", sc)
        # 6 the normal UI, and the flag
        t.keys("Enter")
        sc = t.wait(NORMAL)
        shot("6-first-run", sc)
        # BISE-92: bise paints its ground on every cell (dark here: tmux gives
        # no OSC 11 answer): the capture with colors holds the ground
        colors = t.screen(colors=True)
        assert "48;2;20;18;17" in colors, colors[:2000]
        assert prefs(root).get("onboarded"), prefs(root)
        # BISE-61: the one-time hints of the first run, one at a time
        t.typed('[[bash: sb spawn t1 --objective "{{bash: sb report blocked pick-one}}"]]')
        t.keys("Enter")
        sc = t.wait("new: your agents.", 60)
        shot("7-hint-first-agent", sc)
        t.keys("M-1")                       # used: it goes away
        t.wait_gone("new: your agents.")
        t.keys("Escape")
        sc = t.wait("a card: someone needs you.", 60)
        shot("8-hint-first-card", sc)
        t.typed("ok")                       # the next user message: it goes away
        t.keys("Enter")
        t.wait_gone("a card: someone needs you.")
        t.typed('[[bash: sb spawn t2 --objective "{{bash: sleep 60}}"]] '
              '[[bash: sb spawn t3 --objective "{{bash: sb ask t2 v1-or-v2 --timeout 60}}"]]')
        t.keys("Enter")
        sc = t.wait("agents talk to each other.", 60)
        shot("9-hint-first-level3", sc)
        seen = prefs(root).get("hints")
        assert seen == {"first_agent": True, "first_card": True, "first_level3": True}, seen
        # the second launch: no onboarding
        t.start(120, 34, env(root, home))
        sc = t.wait(NORMAL)
        time.sleep(0.5)
        sc = t.screen()
        assert "press enter ↵" not in sc and "hi, i'm" not in sc, sc
        # esc skips on a fresh root, and marks it seen
        root2 = os.path.join(E.tmp, "state-root-2")
        t.start(120, 34, env(root2, home))
        t.wait("hi, i'm")
        t.keys("Escape")
        t.wait(NORMAL)
        assert prefs(root2).get("onboarded"), prefs(root2)
        print("PASS tui onboarding")


if __name__ == "__main__":
    run(main)
