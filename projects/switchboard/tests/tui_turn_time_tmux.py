"""When a turn ended, on hover (BISE-271), in a real terminal (tmux)
against the fake provider: a mouse move over the reply (SGR 1003 any
motion) shows `HH:MM · now` right-aligned on its row; a move to your
message, then off the history, hides it; the reply's text never moves.

python3 -u projects/switchboard/tests/tui_turn_time_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run  # noqa: E402

COLS, ROWS = 150, 42
LABEL = re.compile(r"\d\d:\d\d · now")


def move(t, x, y):
    """A mouse move with no button (SGR 1006, any-motion: 35), 0-based."""
    t.typed("\x1b[<35;%d;%dM" % (x + 1, y + 1))


def row_with(t, text):
    rows = t.screen().splitlines()
    ys = [i for i, r in enumerate(rows) if text in r]
    assert ys, "%r not on screen:\n%s" % (text, t.screen())
    return ys[-1], rows[ys[-1]]


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("what time is it")
        t.keys("Enter")
        t.wait("ack: what time is it")
        t.wait(" idle")
        assert not LABEL.search(t.screen()), t.screen()
        y, row = row_with(t, "ack: what time is it")
        x = row.find("ack:")
        # over the reply: its turn's end, on that row, right of the text
        move(t, x + 2, y)
        t.wait_re(r"ack: what time is it.*\d\d:\d\d · now")
        y2, row2 = row_with(t, "ack: what time is it")
        assert y2 == y and row2.find("ack:") == x, (row, row2)
        m = LABEL.search(row2)
        assert m and m.start() > row2.find("what time is it") + len("what time is it"), row2
        # over your message: nothing
        rows = t.screen().splitlines()
        ys = [i for i, r in enumerate(rows) if "what time is it" in r and "ack:" not in r]
        assert ys, t.screen()
        move(t, rows[ys[-1]].find("what"), ys[-1])
        t.wait_gone("· now")
        # back on the reply, then away to the composer: gone again
        move(t, x + 2, y)
        t.wait("· now")
        move(t, 5, ROWS - 3)
        t.wait_gone("· now")
        print("PASS tui turn time")


if __name__ == "__main__":
    run(main)
