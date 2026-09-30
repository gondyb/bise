"""The demo's guided tips (tour.rs), in a real terminal (tmux) on a
throwaway hub with the fake provider: main spawns three agents whose
objectives start with `bise demo` (as the bise-demo skill does), and
each tip comes at its moment and goes on its action: the team started
(⌥1 looks inside pm), inside pm (esc back to main), a card (ctrl+g, 1),
a word for dev-api (⌥3, type, ⏎; it stays while the marks move, goes
when you leave), pm dropped (ctrl+s finds it), and ctrl+s ends the tour.
An agent with another objective gets no tip.

python3 -u projects/switchboard/tests/tui_demo_tips_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, panel_row, in_view  # noqa: E402

COLS, ROWS = 150, 42


def spawn(name):
    return '[[bash: sb spawn %s --objective "bise demo (role-play, not real work): you are %s"]]' % (name, name)


def settled(t):
    """Every agent idle and main too: a typed [[bash: …]] then runs in a
    turn of its own (sent into a running turn, the fake only acks it)."""
    t.wait_re(r"4 \S+ dev-api \S*\s+idle", 60)
    for n, name in ((2, "pm"), (3, "designer")):
        t.wait_re(r"%d \S+ %s \S*\s+idle" % (n, name), 60)
    t.wait_re(r"┴ idle · ", 60)


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait(" idle")
        # an ordinary agent: no tip
        t.typed('[[bash: sb spawn other --objective "fix the login"]]')
        t.keys("Enter")
        t.wait_re(panel_row(1, "other"))
        assert "your team just started" not in t.screen(), t.screen()
        t.typed(" ".join(spawn(n) for n in ("pm", "designer", "dev-api")))
        t.keys("Enter")
        t.wait_re(panel_row(4, "dev-api"), 30)
        # 1. the team started: at pm's row (⌥2: `other` has 1)
        sc = t.wait("your team just started.")
        print("---- start ----\n" + sc)
        t.wait("looks inside pm. →")
        assert "⌥2" in sc, sc
        # 2. inside pm
        t.keys("M-2")
        t.wait_re(in_view("pm"))
        sc = t.wait("this is pm's own thread, live.")
        print("---- inside ----\n" + sc)
        t.wait_gone("your team just started.")
        t.keys("Escape")
        t.wait_re(in_view("main"))
        t.wait_gone("this is pm's own thread")
        # 3. a card: ctrl+g, 1
        settled(t)
        t.typed("[[bash: sb card \"$(printf 'designer asks: where does the button go?\\n1. next to the filters\\n2. in the menu')\"]]")
        t.keys("Enter")
        sc = t.wait("needs you. ctrl+g opens", 30)
        print("---- card ----\n" + sc)
        # one item: ctrl+g opens it, a digit picks
        t.keys("C-g")
        t.wait("1-2 pick")
        t.typed("1")
        t.wait_gone("needs you. ctrl+g opens", 20)
        # 4. a word for dev-api
        sc = t.wait("dev-api waits for a word from", 20)
        print("---- steer ----\n" + sc)
        t.keys("M-4")
        t.wait_re(in_view("dev-api"))
        t.wait("dev-api waits for a word from")
        t.typed("ship it friday")
        t.keys("Enter")
        t.wait("ship it friday ✓✓", 30)
        assert "dev-api waits for a word from" in t.screen(), "the tip stays while the marks move"
        t.keys("Escape")
        t.wait_re(in_view("main"))
        t.wait_gone("dev-api waits for a word from")
        # 5. main drops pm (as the skill does): the end
        settled(t)
        t.typed("[[bash: sb drop pm]]")
        t.keys("Enter")
        sc = t.wait("that's it. pm is archived:", 30)
        print("---- end ----\n" + sc)
        t.wait("finds it. ⌥3 ⌥4 open the")
        # ctrl+s: the palette, the tour is over
        t.keys("C-s")
        t.wait_gone("that's it. pm is archived:")
        t.keys("Escape")
        t.wait_gone("that's it. pm is archived:")
        print("PASS tui demo tips")


if __name__ == "__main__":
    run(main)
