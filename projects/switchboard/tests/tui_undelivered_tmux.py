"""A message to a dropped agent on a throwaway hub (tmux, fake provider):
the hub says `undelivered` (C2 amendment, BISE-86); your line ends with
`✗` and `✗ not delivered: t1 stopped. ⏎ send again · esc drop` follows;
⏎ sends it again (a second `✗`), esc drops the question.

python3 -u projects/switchboard/tests/tui_undelivered_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, keys, typed, wait_screen, wait_re, wait_gone, in_view  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbundeliv%d" % os.getpid()
tui_tmux.S = S
ASK = "✗ not delivered: t1 stopped. ⏎ send again · esc drop"


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42)
        wait_screen("bise :*")
        wait_re(in_view("main"))
        # an agent in its own worktree: dropped, nothing revives it
        typed("/new -w t1: idle")
        keys("Enter")
        wait_screen("@t1", 30)
        time.sleep(2)
        typed("/drop t1")
        keys("Enter")
        t0 = time.time()
        while time.time() - t0 < 20:
            sc = tui_tmux.screen()
            if "answer y" in sc:
                typed("y")
                keys("Enter")
                break
            if "@t1 archived" in sc:
                break
            time.sleep(0.2)
        wait_screen("@t1 archived", 20)
        typed("@t1 hello-undelivered")
        keys("Enter")
        wait_re(r"hello-undelivered ✗", 20)
        sc = wait_screen(ASK, 20)
        print(sc)
        # ⏎ sends it again: a second ✗, the first question is answered
        keys("Enter")
        t0 = time.time()
        while tui_tmux.screen().count("hello-undelivered ✗") < 2:
            if time.time() - t0 > 20:
                print(tui_tmux.screen())
                raise AssertionError("not sent again")
            time.sleep(0.2)
        assert tui_tmux.screen().count(ASK) == 1, tui_tmux.screen()
        # esc drops the question
        keys("Escape")
        wait_gone(ASK)
        assert "✗ not delivered: t1 stopped." in tui_tmux.screen()
        ok = True
        print("PASS tui undelivered")
    finally:
        tmux("kill-session", "-t", S)
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
