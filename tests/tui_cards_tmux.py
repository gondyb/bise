"""The inbox (cards v2) in a real terminal (tmux) against the fake
provider: three items in the strip above the divider, typing to main
goes on; ctrl+g selects the inbox (a letter goes back to the composer
and lands in it; only the row selected shows its keys), ↓ loops over
the rows (never leaves; esc does),
↓ ↑ ⏎ opens a row with no option highlighted, ↓ ⏎
picks one, ← → switch items, text + ⏎ answers another, ctrl+x closes
the last and the thread's draft comes back.

python3 -u tests/tui_cards_tmux.py
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
        # the strip: 3 rows, the label row, no keys on the rows (BISE-253)
        t.wait("inbox · 3 waiting for you")
        t.wait("ctrl+g select")
        sc = t.wait("? main · first: pick one")
        assert "1 alpha" not in sc and "×" not in sc, sc
        assert "? main · second: say something" in sc, sc
        # typing still goes to main: the digit is text in the thread
        t.typed("hello main 1")
        t.wait("hello main 1")
        t.wait_re(in_view("main"))
        t.wait("ctrl+g inbox")
        # ctrl+g selects the inbox: the ▸ on the first row, its keys on
        # it alone, the key bar
        t.keys("C-g")
        sc = t.wait("↑↓ choose   1-2 answer   ⏎ open   esc back")
        assert "▸ ? main · first: pick one" in sc, sc
        assert "1 alpha  2 beta   ⏎ open  ×" in sc, sc
        # a letter goes back to the composer and lands in it
        t.typed("!")
        t.wait("hello main 1!")
        t.wait_gone("esc back")
        # ↓ ×4 loops over the 3 rows (the last goes to the first) and
        # never leaves; ↑ on the first: the last; esc leaves
        t.keys("C-g")
        t.wait("esc back")
        for want in ("second", "third", "first", "second"):
            t.keys("Down")
            t.wait("▸ ? main · " + want)
        t.keys("Up")
        t.wait("▸ ? main · first")
        t.keys("Up")
        sc = t.wait("▸ ? main · third")
        assert "esc back to your message" in sc, sc  # third has no options
        t.keys("Escape")
        t.wait_gone("esc back")
        # ctrl+g ↓ ↑ ⏎: the first row opens, nothing highlighted
        t.keys("C-g")
        t.wait("esc back")
        t.keys("Down")
        t.wait("▸ ? main · second")
        t.keys("Up")
        t.wait("▸ ? main · first")
        t.keys("Enter")
        sc = t.wait("you → ? main · your answer")
        assert "hello main 1" not in sc, sc
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
        sc = t.wait("hello main 1!")
        t.wait_re(in_view("main"))
        t.wait_gone("ctrl+g select", 20)
        sc = t.wait("you said alpha")
        assert "you said draft for two" in sc, sc
        time.sleep(0.3)
        print("PASS tui cards")


if __name__ == "__main__":
    run(main)
