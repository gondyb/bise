"""The inbox (cards v2) in a real terminal (tmux) against the fake
provider: three items in the strip above the divider, numbered, typing
to main goes on; ctrl+2 (the kitty form, BISE-302) opens row 2 with no
option highlighted, ctrl+1 row 1, ↓ ⏎ picks an option, ← → switch
items, text + ⏎ answers another, ctrl+x closes the last and the
thread's draft comes back. ctrl+g does nothing any more. Then a
terminal without ctrl+1-9 (BISE_CTRL_DIGITS=0): the strip says `click to
open`, the key bar `/inbox`, and a click on a row opens it.

python3 -u tests/tui_cards_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, in_view, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42


def card(text):
    return "[[bash: sb card \"$(printf '%s')\"]]" % text


CTRL = "\x1b[%d;5u"   # ctrl+digit, the kitty keyboard protocol's form


def click(t, needle):
    """A left click on the first screen row holding `needle`."""
    rows = t.screen().splitlines()
    y = next(i for i, r in enumerate(rows) if needle in r)
    x = rows[y].index(needle)
    t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))


def main():
    # the terminal sends ctrl+1-9 (tmux answers for the user's own
    # server: the override makes it sure)
    with tui_session(COLS, ROWS, env="BISE_CTRL_DIGITS=1 BISE_CLICKS=1") as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed(" ".join([
            card("first: pick one\\n1. alpha\\n2. beta"),
            card("second: say something"),
            card("third: never mind"),
        ]))
        t.keys("Enter")
        # the strip: 3 numbered rows, the label row, no keys on the rows
        t.wait("inbox · 3 waiting for you")
        t.wait("ctrl+1-3 open")
        sc = t.wait(" 1 ? main · first: pick one")
        assert "1 alpha" not in sc and "×" not in sc, sc
        assert " 2 ? main · second: say something" in sc, sc
        assert " 3 ? main · third: never mind" in sc, sc
        # typing still goes to main: the digit is text in the thread
        t.typed("hello main 1")
        t.wait("hello main 1")
        t.wait_re(in_view("main"))
        t.wait("/ commands   ctrl+1 inbox")
        # ctrl+g is gone: nothing happens
        t.keys("C-g")
        time.sleep(0.3)
        assert "you → ? main" not in t.screen(), t.screen()
        # ctrl+2: the second row, straight in the view
        t.typed(CTRL % ord("2"))
        sc = t.wait("you → ? main · your answer")
        assert "second: say something" in sc and "hello main 1" not in sc, sc
        # ctrl+1 in the view: the first item, nothing highlighted
        t.typed(CTRL % ord("1"))
        sc = t.wait("1-2 pick")
        assert "▸" not in sc, sc
        # a reflex ⏎ does nothing; ↓ ⏎ picks option 1
        t.keys("Enter")
        t.keys("Down")
        t.wait("pick “alpha”")
        t.wait("▸ 1 alpha")
        t.keys("Enter")
        t.wait_gone("first: pick one", 20)
        sc = t.wait("second: say something")
        t.wait("1 of 2")
        # → the next item, ← back
        t.keys("Right")
        t.wait("2 of 2")
        t.wait("third: never mind")
        t.keys("Left")
        t.wait("1 of 2")
        # typing answers: ⏎ sends the text
        t.typed("draft for two")
        t.wait("⏎ send as your answer")
        t.keys("Enter")
        t.wait_gone("draft for two")
        t.wait("third: never mind")
        # ctrl+x: close without answering; none left: back to the thread
        t.keys("C-x")
        sc = t.wait("hello main 1")
        t.wait_re(in_view("main"))
        t.wait_gone("waiting for you", 20)
        sc = t.wait("you said alpha")
        assert "you said draft for two" in sc, sc
    # a terminal without ctrl+1-9: never a ctrl key on screen; a click
    # on a row opens it
    with tui_session(COLS, ROWS, env="BISE_CTRL_DIGITS=0 BISE_CLICKS=1") as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed(card("only: yes or no?\\n1. yes\\n2. no"))
        t.keys("Enter")
        sc = t.wait("click to open")
        assert "ctrl+1" not in sc, sc
        t.wait("/ commands   /inbox")
        click(t, " 1 ? main · only")
        t.wait("you → ? main · your answer")
        t.keys("Escape")
        t.wait_re(in_view("main"))
    time.sleep(0.3)
    print("PASS tui cards")


if __name__ == "__main__":
    run(main)
