"""BISE-237: ctrl+f finds in the history. Two messages carry `needle`,
a long one between them (opened with ctrl+o) pushes the first off the screen. ctrl+f, type
`needle`: the newest match is current (`4 of 4`); ↑ goes up match by
match and the view scrolls to the old one; ↓ comes back; esc closes and
the draft is back. NO_COLOR: the current match is reversed, the others
underlined. Through the real binaries (tmux, fake provider: `ack: ...`).

python3 -u projects/switchboard/tests/tui_find_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run  # noqa: E402

FILLER = " ".join(["filler"] * 350)


def send(t, text, ack):
    t.typed(text)
    t.keys("Enter")
    t.wait(ack, 60)
    t.wait(" idle", 60)


def main():
    with tui_session(120, 40, "NO_COLOR=1") as t:
        t.wait("bise :*")
        t.wait(" idle")
        send(t, "the needle one", "ack: the needle one")
        send(t, FILLER, "ack: filler filler")
        send(t, "the needle two", "ack: the needle two")
        # a long message of yours folds to 20 rows (BISE-239, BISE-262): opened
        # (ctrl+o), the filler pushes the first needle off the screen
        t.keys("C-o")
        t.wait("filler ▾")
        assert "needle one" not in t.screen(), t.screen()
        t.typed("my draft")
        t.keys("C-f")
        t.wait("find in main")
        t.wait("find in the history")
        t.wait("⏎ older")
        t.typed("needle")
        sc = t.wait("4 of 4")
        # the current match reversed, the others underlined (NO_COLOR)
        col = t.screen(colors=True)
        assert "\x1b[7mneedle" in col, col[-3000:]
        assert "\x1b[4mneedle" in col, col[-3000:]
        t.keys("Up")
        t.wait("3 of 4")
        # up again: the ack of the first one, far above; the view goes there
        t.keys("Up")
        sc = t.wait("2 of 4")
        sc = t.wait("ack: the needle one")
        assert "the needle two" not in sc, sc
        t.keys("Up")
        t.wait("1 of 4")
        # past the oldest: back to the newest
        t.keys("Up")
        t.wait("back to the newest")
        sc = t.wait("ack: the needle two")
        t.keys("Down")
        t.wait("back to the oldest")
        t.wait("the needle one")
        t.keys("Down")
        t.wait("2 of 4")
        # esc: the field closes, the draft is back, the view stays
        t.keys("Escape")
        sc = t.wait("my draft")
        assert "find in main" not in sc and "ack: the needle one" in sc, sc
        print("PASS tui find")


if __name__ == "__main__":
    run(main)
