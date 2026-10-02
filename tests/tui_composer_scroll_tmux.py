"""A composer text taller than the composer (12 text rows at 40 rows):
the view follows the cursor (at the end, then at the very top with
cmd+↑), says what is scrolled out (`↑ 8 lines above` / `↓ 8 lines
below`), the wheel over it scrolls its text, and shift+↑ / shift+↓
grow and shrink one selection from where it started, like a web text
field: typing replaces it. Through the real binaries (tmux, fake
provider: `ack: ...`).

python3 -u tests/tui_composer_scroll_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

CMD_UP = "\x1b[1;9A"
CMD_DOWN = "\x1b[1;9B"
SHIFT_UP = "\x1b[1;2A"
SHIFT_DOWN = "\x1b[1;2B"


def wheel(down, col, row):
    """An SGR mouse wheel event at the 1-based (col, row)."""
    return "\x1b[<%d;%d;%dM" % (65 if down else 64, col, row)


def text_row(sc, needle):
    """The 1-based screen row holding `needle` (the first), or None."""
    for i, line in enumerate(sc.split("\n")):
        if needle in line:
            return i + 1
    return None


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        for i in range(1, 21):
            t.typed("line %d" % i)
            if i < 20:
                t.keys("C-j")
        # the cursor at the end: the last 12 lines, the rest said above
        sc = t.wait("line 20")
        assert "↑ 8 lines above" in sc and "line 9" in sc, sc
        assert "line 8\n" not in sc and "↓" not in sc.split("↑ 8 lines above")[1].split("line 20")[0], sc
        # cmd+↑: the very top, its cursor row on screen
        t.typed(CMD_UP)
        sc = t.wait("↓ 8 lines below")
        assert "line 1" in sc and "line 12" in sc and "line 13" not in sc, sc
        assert "lines above" not in sc, sc
        # the wheel over the composer scrolls its text (not the history),
        # the cursor stays on line 1
        row = text_row(sc, "line 5")
        for _ in range(3):
            t.typed(wheel(True, 10, row))
        sc = t.wait("↑ 3 lines above")
        assert "↓ 5 lines below" in sc and "line 4\n" not in sc and "line 15" in sc, sc
        # typing brings the view back to the cursor: "Xline 1"
        t.typed("X")
        sc = t.wait("Xline 1")
        assert "↓ 8 lines below" in sc and "lines above" not in sc, sc
        # cmd+↓, then shift+↑ three times and shift+↓ once: one selection
        # from the end of line 20 back to the end of line 18 ("\nline
        # 19\nline 20"); typing replaces it
        t.typed(CMD_DOWN)
        t.wait("↑ 8 lines above")
        for _ in range(3):
            t.typed(SHIFT_UP)
        t.typed(SHIFT_DOWN)
        t.typed("Z")
        sc = t.wait("line 18Z")
        assert "line 19" not in sc and "line 20" not in sc, sc
        assert "line 17" in sc, sc
        # the whole text goes as typed
        t.keys("Enter")
        sc = t.wait("ack: Xline 1", 60)
        print("PASS tui composer scroll")


if __name__ == "__main__":
    run(main)
