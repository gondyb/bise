"""The embedded terminal (Ctrl+`) in a real terminal (tmux), on a
throwaway hub with the fake provider: the toggle shows a shell at the
bottom, `echo` runs, colors render, Ctrl+C reaches the shell (not the
app); hidden, the keys go back to the composer; shown again, the shell
kept its state; /quit kills the shell (no orphan).

tmux sends Ctrl+` as NUL (Ctrl+Space); the kitty form (CSI 96;5u, what
Ghostty sends under DISAMBIGUATE) is written to the pane directly.

python3 -u projects/switchboard/tests/tui_term_tmux.py
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbterm%d" % os.getpid()
tui_tmux.S = S
TITLE = "terminal · ctrl+` hide"


def alive(pid):
    return subprocess.run(["kill", "-0", str(pid)], capture_output=True).returncode == 0


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42, "SHELL=/bin/bash")
        wait_screen("bise :*")
        wait_screen(" idle")
        # show (legacy NUL), run a command
        keys("C-Space")
        wait_screen(TITLE)
        wait_screen("keys go to the shell")          # the hint row
        typed("export PS1='$ '; X=kept; echo hi-$((6*7)) $PWD")
        keys("Enter")
        sc = wait_screen("hi-42 ")
        assert os.path.realpath(E.ws) in sc or E.ws in sc, sc   # cwd = the workspace
        # colors reach the screen
        typed("printf '\\033[31mRED\\033[0m\\n'")
        keys("Enter")
        # "RED" is on screen as soon as the command is typed: wait for the
        # red output itself (under load the shell runs it later)
        red = r"\x1b\[[0-9;]*(31|38;5;1)[0-9;]*mRED"
        t0 = time.time()
        colored = ""
        while time.time() - t0 < 20:
            colored = tmux("capture-pane", "-p", "-e", "-t", S)
            if re.search(red, colored):
                break
            time.sleep(0.1)
        assert re.search(red, colored), repr(colored[-2000:])
        # Ctrl+C goes to the shell: the sleep dies, the app stays
        typed("sleep 100")
        keys("Enter")
        time.sleep(0.5)
        keys("C-c")
        typed("echo after-int")
        keys("Enter")
        wait_screen("after-int")
        typed("echo pid=$$")
        keys("Enter")
        # the typed line "echo pid=$$" shows before its output: wait for digits
        wait_screen("pid=")
        m = None
        for _ in range(50):
            m = re.search(r"pid=(\d+)", screen())
            if m:
                break
            time.sleep(0.1)
        assert m, screen()
        pid = int(m.group(1))
        assert alive(pid)
        # hide with the kitty encoding: the composer gets the keys back
        typed("\x1b[96;5u")
        wait_gone(TITLE)
        typed("to the composer")
        wait_screen("to the composer")
        keys("C-u")
        wait_gone("to the composer")
        assert alive(pid), "hidden, the shell must keep running"
        # show again: same shell, same state
        keys("C-Space")
        wait_screen(TITLE)
        wait_screen("after-int")                     # the screen survived
        typed("echo state=$X")
        keys("Enter")
        wait_screen("state=kept")
        # hide, quit the app: the shell is killed
        keys("C-Space")
        wait_gone(TITLE)
        typed("/quit")
        keys("Enter")
        t0 = time.time()
        while alive(pid) and time.time() - t0 < 10:
            time.sleep(0.1)
        assert not alive(pid), "orphan shell %d after /quit" % pid
        print("OK: terminal panel (toggle NUL + kitty, echo, cwd, colors, Ctrl+C to the shell, hidden keys to the composer, state kept, shell killed on exit)")
        ok = True
    finally:
        tmux("kill-session", "-t", S)
        try:
            c = e2e.Client(os.path.join(E.state, "hub.sock"))
            c.send({"op": "stop_hub"})
            time.sleep(1)
        except Exception:
            pass
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
