"""Selecting text in the embedded terminal (Ctrl+`), in a real terminal
(tmux) against the fake provider: a drag over a line of the shell's
output highlights it and its release copies it (like the history); a
double click copies the word; cmd+c (kitty CSI 99;9u) copies the
selection and never types `c` in the shell; a shell program that asks
for the mouse (DECSET 1000 + 1006) gets the press, shift+drag still
selects. The copies go to BEND_CLIPBOARD_FILE, never the real clipboard.

python3 -u projects/switchboard/tests/tui_term_select_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until  # noqa: E402

TITLE = "terminal · ctrl+` hide"


def read(path):
    return open(path).read() if os.path.exists(path) else None


def wait_clip(path, text, timeout=5):
    wait_until(lambda: read(path) == text, timeout,
               lambda: "clipboard %r, expected %r" % (read(path), text), poll=0.1)


def sgr(b, x, y, up=False):
    """A mouse report (SGR 1006) at the 0-based cell (x, y)."""
    return "\x1b[<%d;%d;%d%s" % (b, x + 1, y + 1, "m" if up else "M")


def main():
    E = e2e.Env()
    clip = os.path.join(E.tmp, "clipboard.txt")
    E.env["BEND_CLIPBOARD_FILE"] = clip
    with tui_session(150, 42, "SHELL=/bin/sh", E=E) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.keys("C-Space")
        t.wait(TITLE)
        t.typed("export PS1='$ '; echo sel-$((6*7))-alpha beta-gamma")
        t.keys("Enter")
        sc = t.wait_re(r"^\S*\s*sel-42-alpha beta-gamma")
        rows = sc.splitlines()
        y = next(i for i, r in enumerate(rows) if re.search(r"^\S*\s*sel-42-alpha beta-gamma", r))
        x = rows[y].find("sel-42")
        # drag from the first char to the last one of the output line, release
        end = x + len("sel-42-alpha beta-gamma") - 1
        t.typed(sgr(0, x, y) + sgr(32, x + 5, y) + sgr(32, end, y))
        # held: highlighted (the selection's background on those cells)
        t.wait_any([re.compile(r"\x1b\[[0-9;]*48;[0-9;]*m[^\x1b]*sel-42")], 10, poll=0.1, colors=True)
        t.typed(sgr(0, end, y, up=True))
        wait_clip(clip, "sel-42-alpha beta-gamma")
        t.wait("copied 23 chars")
        # a double click on a word copies the word
        os.remove(clip)
        bx = rows[y].find("beta")
        t.typed(sgr(0, bx + 1, y) + sgr(0, bx + 1, y, up=True) + sgr(0, bx + 1, y) + sgr(0, bx + 1, y, up=True))
        wait_clip(clip, "beta-gamma")
        # cmd+c copies the selection, the shell gets no `c`
        os.remove(clip)
        t.typed(sgr(0, x, y) + sgr(32, x + 5, y) + sgr(0, x + 5, y, up=True))
        wait_clip(clip, "sel-42")
        os.remove(clip)
        t.typed("\x1b[99;9u")
        wait_clip(clip, "sel-42")
        t.typed("echo after-copy")
        t.keys("Enter")
        sc = t.wait_re(r"^\S?after-copy\s*\S?$")
        assert "cecho" not in sc, sc
        # a program that asks for the mouse gets it; shift+drag selects
        t.typed("printf '\\033[?1000h\\033[?1006h'; cat -v")
        t.keys("Enter")
        sc = t.wait_re(r"cat -v\s*\S?$")
        rows = sc.splitlines()
        cy = max(i for i, r in enumerate(rows) if "cat -v" in r)
        t.typed(sgr(0, 10, cy + 1) + sgr(0, 10, cy + 1, up=True))
        t.wait_re(r"\^\[\[<0;\d+;\d+M")        # cat -v shows the forwarded press
        os.remove(clip)
        # shift (+4) + drag over "after-copy"
        ay = max(i for i, r in enumerate(rows) if re.search(r"^\S?after-copy\s*\S?$", r))
        ax = rows[ay].find("after-copy")
        t.typed(sgr(4, ax, ay) + sgr(36, ax + 9, ay) + sgr(4, ax + 9, ay, up=True))
        wait_clip(clip, "after-copy")
        t.keys("C-c")
        print("OK: terminal selection (drag highlights and copies, double click word, cmd+c copies, mouse to the program, shift+drag selects)")


if __name__ == "__main__":
    run(main)
