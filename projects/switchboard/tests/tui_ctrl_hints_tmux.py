"""The ctrl hints (ctrlhint.rs) through the real binary's input parser: the
kitty keyboard protocol's bytes are written to the pane as a terminal with
flags 1+2+8+16 sends them (tmux itself does not speak the protocol, so the
TUI keeps flag 1 alone, but it reads what arrives). Ctrl held alone shows
the ctrl keys in the key bar after ~150 ms, its release takes them away at
once, a ctrl+o combo never shows them, and the associated text types é
(dead key), å (option) and A (caps lock).

TMPDIR=/tmp/ch-run python3 -u projects/switchboard/tests/tui_ctrl_hints_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, tmux  # noqa: E402

CTRL_DOWN = "\x1b[57442;5u"
CTRL_UP = "\x1b[57442;1:3u"
HINT = "ctrl+c quit"


def raw(t, s):
    tmux("send-keys", "-t", t.name, "-H", *("%02x" % b for b in s.encode()))


def check(cols, rows):
    with tui_session(cols, rows) as t:
        t.wait("bise :*")
        t.wait(" idle")
        before = t.screen()
        assert HINT not in before, before
        # held alone: the hints come, the frame's rows stay where they are
        raw(t, CTRL_DOWN)
        sc = t.wait(HINT, timeout=5)
        assert "ctrl+v paste image" in sc, sc
        assert len(sc.splitlines()) == len(before.splitlines()), sc
        print("---- ctrl held at %d columns ----\n%s" % (cols, sc))
        raw(t, CTRL_UP)
        t.wait_gone(HINT, timeout=2)
        # ctrl+o: a combo, no hints even held
        raw(t, CTRL_DOWN + "\x1b[111;5u" + "\x1b[111;5:3u")
        time.sleep(0.8)
        assert HINT not in t.screen(), t.screen()
        raw(t, CTRL_UP)
        # the typed text: a dead key's é, option's å, caps lock's A, a plain t
        raw(t, "\x1b[101;;233u" + "\x1b[101;1:3u" + "\x1b[97;3;229u" + "\x1b[97;65;65u" + "t")
        sc = t.wait("\u00e9\u00e5At", timeout=5)
        print("---- typed with the flags' bytes ----\n%s" % sc)


def main():
    check(100, 30)
    print("OK: ctrl hints held/released/combo and the associated text (é å A)")


if __name__ == "__main__":
    run(main)
