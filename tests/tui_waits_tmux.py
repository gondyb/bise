"""The agents panel on a throwaway hub (tmux, fake provider): an agent
inside `sb ask` shows the waiting glyph `…` (the hub's status), and the
panel numbers stay while agents live: dropping agent 1 keeps agent 2's
number, and Alt+2 still goes to it.

python3 -u tests/tui_waits_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, panel_row, in_view, MAIN_IDLE  # noqa: E402


def main():
    with tui_session(150, 42) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # t1 does nothing; t2 sleeps (busy); t3 asks t2 and waits
        t.typed('[[bash: sb spawn t1 --objective "idle"]] '
              '[[bash: sb spawn t2 --objective "{{bash: sleep 90}}"]] '
              '[[bash: sb spawn t3 --objective "{{bash: sb ask t2 v1-or-v2 --timeout 120}}"]]')
        t.keys("Enter")
        t.wait_re(panel_row(3, "t3"), 60)
        # BISE-303: the glyph says it waits (`…`), no word at rest
        sc = t.wait_re(r"\b3 … t3\b", 60)
        print(sc)
        # drop t1: t2 and t3 keep their numbers
        t.typed("/drop t1")
        t.keys("Enter")
        if t.wait_any(["answer y", "@t1 archived"], 20)[0] == 0:
            t.typed("y")
            t.keys("Enter")
        t.wait("@t1 archived", 20)
        t.wait_re(panel_row(2, "t2"))
        t.wait_re(panel_row(3, "t3"))
        t.keys("M-2")
        t.wait_re(in_view("t2"))
        print("PASS tui waits")


if __name__ == "__main__":
    run(main)
