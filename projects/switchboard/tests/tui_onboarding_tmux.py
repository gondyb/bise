"""The first launch (BISE-60, book §15, mockup tui-onboarding.html) in a
real terminal (tmux), on a throwaway hub with the fake provider and an
empty state root (XDG_STATE_HOME) and HOME in a temp dir:

- a first launch plays the five steps, enter by enter; the screens are
  saved in $SB_ONBOARDING_SHOTS (default: the temp dir) for the mockup
  comparison;
- the second launch goes straight to the normal UI;
- esc on a fresh state root skips it and marks it seen.

python3 -u projects/switchboard/tests/tui_onboarding_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, wait_screen  # noqa: E402
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
        for s in ["hi, i'm bise :*", "your ideas. my hands. lots of them.", "● ○ ○ ○ ○ ○"]:
            assert s in sc, sc
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
