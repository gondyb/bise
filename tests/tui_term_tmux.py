"""The embedded terminal (Ctrl+`) in a real terminal (tmux), on a
throwaway hub with the fake provider: the toggle shows a shell at the
bottom, `echo` runs, colors render, Ctrl+C reaches the shell (not the
app); hidden, the keys go back to the composer; shown again, the shell
kept its state; /quit kills the shell (no orphan).

tmux sends Ctrl+` as NUL (Ctrl+Space); the kitty form (CSI 96;5u, what
Ghostty sends under DISAMBIGUATE) is written to the pane directly.

python3 -u tests/tui_term_tmux.py
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until  # noqa: E402

TITLE = "terminal · ctrl+` hide"


def alive(pid):
    return subprocess.run(["kill", "-0", str(pid)], capture_output=True).returncode == 0


def main():
    with tui_session(150, 42, "SHELL=/bin/bash") as t:
        t.wait("bise :*")
        t.wait(" idle")
        # show (legacy NUL), run a command
        t.keys("C-Space")
        t.wait(TITLE)
        t.wait("keys go to the shell")          # the hint row
        t.typed("export PS1='$ '; X=kept; echo hi-$((6*7)) $PWD")
        t.keys("Enter")
        sc = t.wait("hi-42 ")
        assert os.path.realpath(t.E.ws) in sc or t.E.ws in sc, sc   # cwd = the workspace
        # colors reach the screen
        t.typed("printf '\\033[31mRED\\033[0m\\n'")
        t.keys("Enter")
        # "RED" is on screen as soon as the command is typed: wait for the
        # red output itself (under load the shell runs it later)
        red = r"\x1b\[[0-9;]*(31|38;5;1)[0-9;]*mRED"
        t.wait_any([re.compile(red)], 20, poll=0.1, colors=True)
        # Ctrl+C goes to the shell: the sleep dies, the app stays
        nap = "100.%d" % os.getpid()     # a sleep of our own, to see it run
        t.typed("sleep " + nap)
        t.keys("Enter")
        wait_until(lambda: subprocess.run(["pgrep", "-f", "^sleep %s$" % nap],
                                          capture_output=True).returncode == 0,
                   20, lambda: "the shell never ran sleep %s" % nap, poll=0.1)
        t.keys("C-c")
        t.typed("echo after-int")
        t.keys("Enter")
        t.wait("after-int")
        t.typed("echo pid=$$")
        t.keys("Enter")
        # the typed line "echo pid=$$" shows before its output: wait for digits
        t.wait("pid=")
        sc = t.wait_any([re.compile(r"pid=(\d+)")], 5, poll=0.1)[1]
        pid = int(re.search(r"pid=(\d+)", sc).group(1))
        assert alive(pid)
        # hide with the kitty encoding: the composer gets the keys back
        t.typed("\x1b[96;5u")
        t.wait_gone(TITLE)
        t.typed("to the composer")
        t.wait("to the composer")
        t.keys("C-u")
        t.wait_gone("to the composer")
        assert alive(pid), "hidden, the shell must keep running"
        # show again: same shell, same state
        t.keys("C-Space")
        t.wait(TITLE)
        t.wait("after-int")                     # the screen survived
        t.typed("echo state=$X")
        t.keys("Enter")
        t.wait("state=kept")
        # hide, quit the app: the shell is killed
        t.keys("C-Space")
        t.wait_gone(TITLE)
        t.typed("/quit")
        t.keys("Enter")
        wait_until(lambda: not alive(pid), 10, lambda: "orphan shell %d after /quit" % pid, poll=0.1)
        print("OK: terminal panel (toggle NUL + kitty, echo, cwd, colors, Ctrl+C to the shell, hidden keys to the composer, state kept, shell killed on exit)")


if __name__ == "__main__":
    run(main)
