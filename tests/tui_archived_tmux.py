"""Archived tasks in the panel, in a real terminal (tmux) on a throwaway
hub with the fake provider: two tasks are dropped; the panel shows one
dim, folded `▸ 2 archived` row; a click expands it (newest first); a
click on an archived task opens its history read-only (typed text is
not sent, the hint says /restore); /restore brings it back live.

python3 -u tests/tui_archived_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, panel_row, in_view  # noqa: E402

COLS, ROWS = 150, 42
PANEL_X = COLS - max(28, min(40, COLS // 4))


def click_on(t, label):
    """A left press + release (SGR 1006) on the panel row showing `label`."""
    for y, row in enumerate(t.screen().splitlines()):
        x = row.find(label, PANEL_X)
        if x >= 0:
            seq = "\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1)
            t.typed(seq)
            return
    print(t.screen())
    raise AssertionError("not in the panel: %r" % label)


def panel(t):
    return "\n".join(r[PANEL_X:] for r in t.screen().splitlines())


def drop(t, name):
    t.typed("/drop %s" % name)
    t.keys("Enter")
    if t.wait_any(["answer y", "@%s archived" % name], 20)[0] == 0:
        t.typed("y")
        t.keys("Enter")
    t.wait("@%s archived" % name)


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait(" idle")
        # one after the other: the archived rows are sorted by the last
        # report (newest first), so t2 must report after t1 (spawned in
        # one message, their report order was up to the load)
        t.typed('[[bash: sb spawn t1 --objective "first-objective"]]')
        t.keys("Enter")
        t.wait_re(panel_row(1, "t1"))
        t.wait_re(r"t1 +(→ \S+|m_\d)", 60)
        t.typed('[[bash: sb spawn t2 --objective "second-objective"]]')
        t.keys("Enter")
        t.wait_re(panel_row(2, "t2"))
        t.wait_re(r"t2 +(→ \S+|m_\d)", 60)
        drop(t, "t1")
        drop(t, "t2")
        t.wait("▸ 2 archived")
        p = panel(t)
        assert " t1" not in p, p
        print("---- folded ----\n" + p)
        click_on(t, "▸ 2 archived")
        t.wait("▾ 2 archived")
        p = panel(t)
        print("---- expanded ----\n" + p)
        assert p.index("– t2") < p.index("– t1"), "newest first:\n" + p
        click_on(t, "– t1")
        sc = t.wait("t1 is archived: read-only")
        assert "read-only history" in sc, sc
        # its history is in the feed
        t.wait("first-objective")
        t.typed("hello-archived")
        t.keys("Enter")
        sc = t.wait("@t1 is archived: its history is read-only")
        print("---- archived feed ----\n" + sc)
        # the hub never got the line
        time.sleep(0.5)
        assert "you: hello-archived" not in t.screen()
        # folding keeps the task in focus listed
        click_on(t, "▾ 2 archived")
        t.wait("▸ 2 archived")
        assert "– t1" in panel(t) and "– t2" not in panel(t), panel(t)
        # /restore (an existing user command) brings it back
        t.typed("/restore t1")
        t.keys("Enter")
        t.wait_re(in_view("t1"), 30)
        t.wait("▸ 1 archived")
        print("PASS tui archived")


if __name__ == "__main__":
    run(main)
