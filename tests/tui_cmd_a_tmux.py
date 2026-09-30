"""BISE-267: cmd+a (SUPER+a under the kitty keyboard protocol, what
Ghostty sends with `keybind = super+a=unbind`) selects the whole
composer text; typing replaces it and ⏎ sends only the new text. Through
the real binaries (tmux, fake provider: `ack: ...`).

python3 -u tests/tui_cmd_a_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run  # noqa: E402

CMD_A = "\x1b[97;9u"


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("old draft one")
        t.keys("C-j")
        t.typed("old draft two")
        t.keys("Left", "Left")
        t.wait("old draft two")
        t.typed(CMD_A)
        t.typed("fresh words")
        sc = t.wait("fresh words")
        assert "old draft" not in sc, sc
        t.keys("Enter")
        sc = t.wait("ack: fresh words", 60)
        assert "old draft" not in sc, sc
        print("PASS tui cmd+a")


if __name__ == "__main__":
    run(main)
