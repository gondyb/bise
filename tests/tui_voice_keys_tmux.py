"""The voice shortcuts in a real terminal (tmux), on a throwaway hub with
the fake provider: /voice ends with its keys (ctrl+r dictation, ctrl+r
twice voice mode, the voice mode keys) at 150 and 80 columns; ctrl held
alone (the kitty protocol's bytes, as in tui_ctrl_hints_tmux.py) shows
`ctrl+r dictate` with dictation on and `ctrl+r twice voice mode` always;
/help has ctrl+r twice. The agent in view working: `tab queue   ⏎ steer` only
with text in the composer, in main's view and an agent's.

python3 -u tests/tui_voice_keys_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, tmux, MAIN_IDLE  # noqa: E402

CTRL_DOWN = "\x1b[57442;5u"
CTRL_UP = "\x1b[57442;1:3u"
HINT = "ctrl+c quit"


def raw(t, s):
    tmux("send-keys", "-t", t.name, "-H", *("%02x" % b for b in s.encode()))


def voice_screen(t, cols):
    t.typed("/voice")
    t.keys("Enter")
    sc = t.wait("in voice mode", timeout=10)
    print("---- /voice at %d columns ----\n%s" % (cols, sc))
    for want in ["ctrl+r", "dictation: you talk, it types. any key stops.", "ctrl+r twice", "voice mode, with the agent in view",
                 "space sends", "hold space keeps the floor", "m mutes", "esc leaves"]:
        assert want in sc, (want, sc)
    assert "ctrl+r twice starts" not in sc, sc
    t.keys("Escape")
    t.wait_gone("in voice mode", timeout=5)


def ctrl_held(t, cols, dictation):
    raw(t, CTRL_DOWN)
    sc = t.wait(HINT, timeout=5)
    bar = next(line for line in reversed(sc.splitlines()) if HINT in line)
    print("---- ctrl held at %d columns, dictation %s ----\n%s" % (cols, "on" if dictation else "off", sc))
    raw(t, CTRL_UP)
    t.wait_gone(HINT, timeout=2)
    return bar


def check(cols, rows, dictation):
    with tui_session(cols, rows, "SB_VOICE=%d" % dictation) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        bar = ctrl_held(t, cols, dictation)
        if cols >= 150:
            assert "ctrl+r twice voice mode" in bar, bar
            assert ("ctrl+r dictate" in bar) == bool(dictation), bar
        voice_screen(t, cols)
        if cols >= 150 and dictation:
            t.typed("/help")
            t.keys("Enter")
            t.wait("type to filter", timeout=10)
            # the voice section is below the fold: the filter finds it
            t.typed("voice")
            sc = t.wait("ctrl+r twice", timeout=10)
            assert "voice mode: talk with the agent in view" in sc, sc
            print("---- /help ----\n%s" % sc)
            t.keys("Escape")


def bar_of(sc):
    """The key bar: the screen's last non-empty row."""
    return next(line for line in reversed(sc.splitlines()) if line.strip())


def steer_bar():
    """The agent in view works (designer): `tab queue   ⏎ steer` only with
    text in the composer, in main's view and in an agent's."""
    with tui_session(150, 42) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed('[[bash: sb spawn t1 --objective "wait {{bash: sleep 40}}"]] [[bash: sleep 25]]')
        t.keys("Enter")
        sc = t.wait("ctrl+c interrupt", 20)
        bar = bar_of(sc)
        assert "tab queue" not in bar and "steer" not in bar, bar
        print("---- main works, empty composer ----\n%s" % sc)
        t.typed("later")
        sc = t.wait("tab queue   ⏎ steer   ctrl+c interrupt", 5)
        print("---- main works, text in the composer ----\n%s" % sc)
        t.keys("Tab")
        t.wait(" › later", 5)
        # t1's view, t1 working
        t.wait_re(r"\b1 \S+ t1\b", 20)
        t.keys("M-1")
        sc = t.wait("esc back to main   ctrl+c interrupt", 20)
        print("---- t1 works, empty composer ----\n%s" % sc)
        t.typed("later")
        sc = t.wait("esc back to main   tab queue   ⏎ steer   ctrl+c interrupt", 5)
        print("---- t1 works, text in the composer ----\n%s" % sc)


def main():
    for cols, rows in [(150, 42), (80, 30)]:
        for dictation in (1, 0):
            check(cols, rows, dictation)
    steer_bar()
    print("OK: /voice shows its keys, ctrl held shows ctrl+r dictate / ctrl+r twice voice mode, /help has ctrl+r twice, "
          "tab queue / ⏎ steer only with text")


if __name__ == "__main__":
    run(main)
