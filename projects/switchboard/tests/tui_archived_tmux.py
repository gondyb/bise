"""Archived tasks in the panel, in a real terminal (tmux) on a throwaway
hub with the fake provider: two tasks are dropped; the panel shows one
dim, folded `▸ 2 archived` row; a click expands it (newest first); a
click on an archived task opens its history read-only (typed text is
not sent, the hint says /restore); /restore brings it back live.

python3 -u projects/switchboard/tests/tui_archived_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone, wait_re, panel_row, in_view  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbarch%d" % os.getpid()
tui_tmux.S = S
COLS, ROWS = 150, 42
PANEL_X = COLS - max(28, min(40, COLS // 4))


def click_on(label):
    """A left press + release (SGR 1006) on the panel row showing `label`."""
    for y, row in enumerate(screen().splitlines()):
        x = row.find(label, PANEL_X)
        if x >= 0:
            seq = "\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1)
            tmux("send-keys", "-t", S, "-l", seq)
            return
    print(screen())
    raise AssertionError("not in the panel: %r" % label)


def panel():
    return "\n".join(r[PANEL_X:] for r in screen().splitlines())


def drop(name):
    typed("/drop %s" % name)
    keys("Enter")
    t0 = time.time()
    while time.time() - t0 < 20:
        sc = screen()
        if "answer y" in sc:
            typed("y")
            keys("Enter")
            break
        if "@%s archived" % name in sc:
            break
        time.sleep(0.2)
    wait_screen("@%s archived" % name)


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, COLS, ROWS)
        wait_screen("bise :*")
        wait_screen(" idle")
        typed('[[bash: sb spawn t1 --objective "first-objective"]] [[bash: sb spawn t2 --objective "second-objective"]]')
        keys("Enter")
        wait_re(panel_row(1, "t1"))
        wait_re(panel_row(2, "t2"))
        wait_re(r"t1 +m_\d", 60)
        wait_re(r"t2 +m_\d", 60)
        drop("t1")
        time.sleep(1.1)
        drop("t2")
        wait_screen("▸ 2 archived")
        p = panel()
        assert " t1" not in p, p
        print("---- folded ----\n" + p)
        click_on("▸ 2 archived")
        wait_screen("▾ 2 archived")
        p = panel()
        print("---- expanded ----\n" + p)
        assert p.index("– t2") < p.index("– t1"), "newest first:\n" + p
        click_on("– t1")
        sc = wait_screen("t1 is archived: read-only")
        assert "read-only history" in sc, sc
        # its history is in the feed
        wait_screen("first-objective")
        typed("hello-archived")
        keys("Enter")
        sc = wait_screen("@t1 is archived: its history is read-only")
        print("---- archived feed ----\n" + sc)
        # the hub never got the line
        time.sleep(0.5)
        assert "you: hello-archived" not in screen()
        # folding keeps the task in focus listed
        click_on("▾ 2 archived")
        wait_screen("▸ 2 archived")
        assert "– t1" in panel() and "– t2" not in panel(), panel()
        # /restore (an existing user command) brings it back
        typed("/restore t1")
        keys("Enter")
        wait_re(in_view("t1"), 30)
        wait_screen("▸ 1 archived")
        ok = True
        print("PASS tui archived")
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
