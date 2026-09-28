"""The agents panel on a throwaway hub (tmux, fake provider): an agent
inside `sb ask` shows `waits {name}` (the hub's `waiting_on`), and the
panel numbers stay while agents live: dropping agent 1 keeps agent 2's
number, and Alt+2 still goes to it.

python3 -u projects/switchboard/tests/tui_waits_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, keys, typed, wait_screen, wait_re, panel_row, in_view  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbwaits%d" % os.getpid()
tui_tmux.S = S


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42)
        wait_screen("bise :*")
        wait_screen(" idle")
        # t1 does nothing; t2 sleeps (busy); t3 asks t2 and waits
        typed('[[bash: sb spawn t1 --objective "idle"]] '
              '[[bash: sb spawn t2 --objective "{{bash: sleep 90}}"]] '
              '[[bash: sb spawn t3 --objective "{{bash: sb ask t2 v1-or-v2 --timeout 120}}"]]')
        keys("Enter")
        wait_re(panel_row(3, "t3"), 60)
        sc = wait_re(panel_row(3, "t3") + r".*waits t2", 60)
        print(sc)
        # drop t1: t2 and t3 keep their numbers
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
        wait_re(panel_row(2, "t2"))
        wait_re(panel_row(3, "t3"))
        keys("M-2")
        wait_re(in_view("t2"))
        ok = True
        print("PASS tui waits")
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
