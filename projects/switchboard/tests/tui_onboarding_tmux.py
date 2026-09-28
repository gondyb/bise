"""The first launch (BISE-60, book §15, mockup tui-onboarding.html) in a
real terminal (tmux), on a throwaway hub with the fake provider and an
empty state root (XDG_STATE_HOME) and HOME in a temp dir:

- a first launch plays the five steps, enter by enter; the screens are
  saved in $SB_ONBOARDING_SHOTS (default: the temp dir) for the mockup
  comparison;
- the second launch goes straight to the normal UI;
- esc on a fresh state root skips it and marks it seen;
- the one-time hints (BISE-61) of the first run: the first agent, the
  first card, the first message between agents; each one goes away when
  used or after the next message, and is marked in hints.json.

python3 -u projects/switchboard/tests/tui_onboarding_tmux.py
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone  # noqa: E402
import tui_tmux  # noqa: E402

NORMAL = "⏎ send · @ agent · / commands"


def launch(E, state_root, home):
    tmux("kill-session", "-t", tui_tmux.S)
    tui_tmux.start_tui(E, 120, 34, "XDG_STATE_HOME=%s HOME=%s ANTHROPIC_FOUNDRY_API_KEY=" % (state_root, home))


def main():
    tui_tmux.S = "sbonb%d" % os.getpid()
    E = e2e.Env()
    E.env.pop("SB_ONBOARDING", None)
    shots = os.environ.get("SB_ONBOARDING_SHOTS") or os.path.join(E.tmp, "shots")
    os.makedirs(shots, exist_ok=True)
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    root = os.path.join(E.tmp, "state-root")
    flag = os.path.join(root, "switchboard", "onboarded")
    ok = False

    def shot(name, sc):
        with open(os.path.join(shots, name + ".txt"), "w") as f:
            f.write(sc)
        print("---- %s ----\n%s" % (name, sc))

    try:
        launch(E, root, home)
        # 1 welcome: typed, then the :* pop
        sc = wait_screen("press enter ↵", 30)
        time.sleep(0.5)
        sc = screen()
        for s in ["hi, i'm bise :*", "bise /beez/ · french: a kiss on the cheek. also a north wind.",
                  "ideas in. little kisses out. also pull requests.", "● ○ ○ ○ ○ ○"]:
            assert s in sc, sc
        rows = sc.splitlines()
        hi = next(i for i, r in enumerate(rows) if "hi, i'm bise :*" in r)
        assert "bise /beez/" in rows[hi + 1], sc      # the gloss, right under the name
        shot("1-welcome", sc)
        # 2 theme: two previews, ←→ switches live
        keys("Enter")
        sc = wait_screen("←→ switch · enter keep")
        for s in ["so i picked dark.", "you can change it any time with /theme.", "fix the flaky login test",
                  "on it: auth-fix takes it.", "auth-fix is done.", "○ ● ○ ○ ○ ○"]:
            assert s in sc, sc
        shot("2-theme", sc)
        keys("Right")
        time.sleep(0.3)
        keys("Left")
        # 3 model: the fake env has MISTRAL_API_KEY and a mistral model
        keys("Enter")
        sc = wait_screen("which model should do the work?")
        for s in ["i found a key in your environment.", "1 · use MISTRAL_API_KEY found",
                  "mistral, already set up. nothing to paste.", "2 · paste another key",
                  "3 · sign in with the browser", "↑↓ choose · enter ok"]:
            assert s in sc, sc
        shot("3-model", sc)
        # 4 folder and the honest line
        keys("Enter")
        sc = wait_screen("enter ok · o another folder")
        for s in ["i'll work in ", "git repo ✓", "one honest thing: agents run commands here without asking you."]:
            assert s in sc, sc
        shot("4-folder", sc)
        keys("o")
        wait_screen("another folder? start me there")
        # 5 the three lines, one by one
        keys("Enter")
        sc = wait_screen("enter, and say what's on your mind.")
        for s in ["how it works, in three lines:", "you talk to me. i start agents for the work, in the background.",
                  "they show up on the right.", "when someone needs you, you get a card. the rest can wait."]:
            assert s in sc, sc
        shot("5-how-it-works", sc)
        # 6 the normal UI, and the flag
        keys("Enter")
        sc = wait_screen(NORMAL)
        shot("6-first-run", sc)
        assert os.path.exists(flag), flag
        # BISE-61: the one-time hints of the first run, one at a time
        hints = os.path.join(root, "switchboard", "hints.json")
        typed('[[bash: sb spawn t1 --objective "{{bash: sb report blocked pick-one}}"]]')
        keys("Enter")
        sc = wait_screen("new: your agents.", 60)
        shot("7-hint-first-agent", sc)
        keys("M-1")                       # used: it goes away
        wait_gone("new: your agents.")
        keys("Escape")
        sc = wait_screen("a card: someone needs you.", 60)
        shot("8-hint-first-card", sc)
        typed("ok")                       # the next user message: it goes away
        keys("Enter")
        wait_gone("a card: someone needs you.")
        typed('[[bash: sb spawn t2 --objective "{{bash: sleep 60}}"]] '
              '[[bash: sb spawn t3 --objective "{{bash: sb ask t2 v1-or-v2 --timeout 60}}"]]')
        keys("Enter")
        sc = wait_screen("agents talk to each other.", 60)
        shot("9-hint-first-level3", sc)
        with open(hints) as f:
            seen = json.load(f)
        assert seen == {"first_agent": True, "first_card": True, "first_level3": True}, seen
        # the second launch: no onboarding
        launch(E, root, home)
        sc = wait_screen(NORMAL)
        time.sleep(0.5)
        sc = screen()
        assert "press enter ↵" not in sc and "hi, i'm" not in sc, sc
        # esc skips on a fresh root, and marks it seen
        root2 = os.path.join(E.tmp, "state-root-2")
        launch(E, root2, home)
        wait_screen("hi, i'm")
        keys("Escape")
        wait_screen(NORMAL)
        assert os.path.exists(os.path.join(root2, "switchboard", "onboarded"))
        ok = True
        print("PASS tui onboarding")
    finally:
        tmux("kill-session", "-t", tui_tmux.S)
        try:
            c = e2e.Client(os.path.join(E.state, "hub.sock"))
            c.send({"op": "stop_hub"})
            time.sleep(1)
        except Exception:
            pass
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
