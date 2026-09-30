"""BISE-265: ctrl+s (cmd+k where the terminal passes it) finds an agent
by name. Two agents, one dropped: ctrl+s opens the palette in the
composer's place (`you → find an agent`, the draft waits), a partial
name filters, ⏎ opens that agent's view (its own draft; main's waits);
esc closes and the draft is back; the archived one is found by name under `earlier` and opens its
read-only history; `/switch <name>` opens it on a query. Through the
real binaries (tmux, fake provider).

python3 -u projects/switchboard/tests/tui_palette_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, panel_row, in_view  # noqa: E402


def spawn(t, n, name, objective):
    t.typed('[[bash: sb spawn %s --objective "%s"]]' % (name, objective))
    t.keys("Enter")
    t.wait_re(panel_row(n, name))
    t.wait_re(r"%s +(→ \S+|m_\d)" % name, 60)


def main():
    with tui_session(130, 40) as t:
        t.wait("bise :*")
        t.wait(" idle")
        spawn(t, 1, "dark-mode", "dark colors first")
        spawn(t, 2, "old-beta", "the csv export")
        t.typed("/drop old-beta")
        t.keys("Enter")
        if t.wait_any(["answer y", "@old-beta archived"], 20)[0] == 0:
            t.typed("y")
            t.keys("Enter")
        t.wait("▸ 1 archived")
        # open, a partial name, ⏎: that agent's view
        t.typed("my draft")
        t.keys("C-s")
        t.wait("you → find an agent")
        t.wait("type part of a name")
        sc = t.wait("↑↓ choose")
        assert "old-beta" not in sc.split("find an agent")[1], "no archived on an empty query:\n" + sc
        t.typed("dar")
        sc = t.wait("1 agent")
        print("---- dar ----\n" + sc)
        t.keys("Enter")
        t.wait_re(in_view("dark-mode"), 20)
        # each view keeps its own draft: main's waits there
        t.wait("talk to dark-mode directly")
        # esc closes: nothing moves, the draft is back
        t.typed("second draft")
        t.keys("C-s")
        t.wait("you → find an agent")
        t.typed("zz")
        t.wait('no agent called “zz”')
        t.keys("Escape")
        t.wait_gone("find an agent")
        t.wait_re(in_view("dark-mode"))
        t.wait("second draft")
        # the archived one, by name: under `earlier`, its read-only history
        t.keys("C-s")
        t.typed("beta")
        sc = t.wait("earlier · read-only")
        print("---- beta ----\n" + sc)
        t.keys("Enter")
        t.wait("old-beta is archived: read-only")
        t.wait("the csv export")
        # /switch on a query: back to main
        t.keys("C-u")
        t.typed("/switch mai")
        t.keys("Enter")
        t.wait("you → find an agent")
        t.keys("Enter")
        t.wait_re(in_view("main"), 20)
        t.wait("my draft")
        print("PASS tui palette")


if __name__ == "__main__":
    run(main)
