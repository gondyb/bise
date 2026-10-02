"""cmd+↑ / cmd+↓ (SUPER+Up/Down under the kitty keyboard protocol, what
Ghostty sends with `keybind = super+arrow_up=unbind` and the other arrow
lines /setup offers) go to the very start / end of a multi-line composer
text; with shift they select from the cursor to there, and typing
replaces the selection. Through the real binaries (tmux, fake provider:
`ack: ...`).

python3 -u tests/tui_cmd_arrows_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

CMD_UP = "\x1b[1;9A"
CMD_DOWN = "\x1b[1;9B"
CMD_SHIFT_UP = "\x1b[1;10A"
CMD_SHIFT_DOWN = "\x1b[1;10B"


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("alpha one")
        t.keys("C-j")
        t.typed("beta two")
        t.keys("C-j")
        t.typed("gamma three")
        t.wait("gamma three")
        # cmd+↑ from the last row: the text's very start
        t.typed(CMD_UP)
        t.typed("X")
        t.wait("Xalpha one")
        # cmd+↓ from the first row: the very end
        t.typed(CMD_DOWN)
        t.typed("Y")
        t.wait("gamma threeY")
        # cmd+↑ then cmd+shift+↓: everything selected, typing replaces it
        t.typed(CMD_UP)
        t.typed(CMD_SHIFT_DOWN)
        t.typed("fresh words")
        sc = t.wait("fresh words")
        assert "alpha one" not in sc and "gamma three" not in sc, sc
        # cmd+shift+↑ from the end: back to one word, then cmd+↑ types first
        t.typed(CMD_SHIFT_UP)
        t.typed("new")
        t.typed(CMD_UP)
        t.typed("a ")
        t.wait("a new")
        t.keys("Enter")
        sc = t.wait("ack: a new", 60)
        assert "fresh words" not in sc and "alpha one" not in sc, sc
        print("PASS tui cmd+arrows")


if __name__ == "__main__":
    run(main)
