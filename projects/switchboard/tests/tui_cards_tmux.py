"""Cards v2 in a real terminal (tmux) against the fake provider: three
cards in the strip above the divider, typing to main goes on, ctrl+g
opens the card view, a digit answers one, text + ⏎ another, ctrl+x
closes the last; esc keeps the card's draft, and the thread's draft
comes back.

python3 -u projects/switchboard/tests/tui_cards_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, in_view  # noqa: E402

COLS, ROWS = 150, 42


def card(text):
    return "[[bash: sb card \"$(printf '%s')\"]]" % text


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed(" ".join([
            card("first: pick one\\n1. alpha\\n2. beta"),
            card("second: say something"),
            card("third: never mind"),
        ]))
        t.keys("Enter")
        # the strip: 3 rows, the label row, the options on the first
        sc = t.wait("3 cards")
        t.wait("ctrl+g open")
        sc = t.wait("1 alpha  2 beta")
        assert "? main · first: pick one" in sc, sc
        assert "? main · second: say something" in sc, sc
        # typing still goes to main: the digit is text in the thread
        t.typed("hello main 1")
        t.wait("hello main 1")
        t.wait_re(in_view("main"))
        # ctrl+g: the card view on the top card, its own empty composer
        t.keys("C-g")
        sc = t.wait("main's card · your answer")
        assert "hello main 1" not in sc, sc
        t.wait("1-2 pick")
        # a digit on the empty composer answers
        t.typed("2")
        sc = t.wait("? main needs you")
        t.wait_gone("first: pick one", 20)
        # the second card: type, esc keeps its draft, the thread's back
        t.typed("draft for two")
        t.wait("draft for two")
        t.keys("Escape")
        t.wait("hello main 1")
        t.wait_gone("draft for two")
        t.wait("2 cards")
        # ctrl+g: the top card again, its draft kept; ⏎ answers with it
        t.keys("C-g")
        t.wait("draft for two")
        t.keys("Enter")
        t.wait("third: never mind")
        t.wait_gone("draft for two")
        # ctrl+x: close without answering; none left: back to the thread
        t.keys("C-x")
        sc = t.wait("hello main 1")
        t.wait_re(in_view("main"))
        t.wait_gone("ctrl+g open", 20)
        sc = t.wait("you said beta")
        assert "you said draft for two" in sc, sc
        time.sleep(0.3)
        print("PASS tui cards")


if __name__ == "__main__":
    run(main)
