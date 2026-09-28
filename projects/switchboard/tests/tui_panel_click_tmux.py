"""A click on an agent in the right panel focuses it, like Alt+N, in a
real terminal (tmux) against the fake provider: the SGR mouse reports
of a left press on each task row, then on main.

python3 -u projects/switchboard/tests/tui_panel_click_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_re, panel_row, in_view  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbclick%d" % os.getpid()
tui_tmux.S = S
COLS, ROWS = 150, 42


def click_on(label):
    """A left press + release (SGR 1006) on the panel row showing
    `label` (the panel is the right quarter of the screen)."""
    panel_x = COLS - max(28, min(40, COLS // 4))
    for y, row in enumerate(screen().splitlines()):
        x = row.find(label, panel_x)
        if x >= 0:
            # SGR coordinates are 1-based
            seq = "\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1)
            tmux("send-keys", "-t", S, "-l", seq)
            return
    print(screen())
    raise AssertionError("not in the panel: %r" % label)


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, COLS, ROWS)
        wait_screen("bise :*")
        wait_screen(" idle")
        typed('[[bash: sb spawn t1 --objective "first"]] [[bash: sb spawn t2 --objective "second"]]')
        keys("Enter")
        wait_re(panel_row(1, "t1"))
        wait_re(panel_row(2, "t2"))
        click_on(" t1")
        wait_re(in_view("t1"))
        click_on(" t2")
        wait_re(in_view("t2"))
        # the selected agent shows its objective under its row: that
        # row belongs to the agent too
        keys("C-k")
        keys("C-k")
        wait_screen("first")
        click_on("first")
        wait_re(in_view("t1"))
        click_on(" main")
        wait_re(in_view("main"))
        # the feed still takes clicks: the composer keeps its text
        typed("still here")
        wait_screen("still here")
        time.sleep(0.3)
        ok = True
        print("PASS tui panel click")
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
