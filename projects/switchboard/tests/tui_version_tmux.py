"""The /version picker in a real terminal (tmux), against the fake
provider: '/version ' opens the list of versions in the composer popup
(tree, the recent commits of this repo, with their marks), the typed text
filters it, the arrows move, Enter asks the hub to build then switch.
An unknown commit gets a clear answer.

python3 -u projects/switchboard/tests/tui_version_tmux.py
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbver%d" % os.getpid()
tui_tmux.S = S


def git(*a):
    return subprocess.run(["git", *a], cwd=e2e.ROOT, capture_output=True, text=True).stdout.strip()


def main():
    E = e2e.Env()
    ok = False
    head = git("log", "-1", "--format=%h")
    second = git("log", "-2", "--format=%h").splitlines()[-1]
    second_subject = git("log", "-1", "--format=%s", second)
    # a state root of its own: the marks do not depend on the versions
    # built in ~/.local/state, and its build dir is a file, so the build
    # Enter starts fails at once (versions.sh: mkdir) and builds nothing
    xdg = os.path.join(E.tmp, "xdg")
    os.makedirs(os.path.join(xdg, "switchboard"))
    open(os.path.join(xdg, "switchboard", "build"), "w").close()
    try:
        tui_tmux.start_tui(E, 160, 42, "XDG_STATE_HOME=%s" % xdg)
        wait_screen("bise :*")
        wait_screen(" idle")
        # the popup: tree and the commits, with the current one marked
        typed("/version ")
        sc = wait_screen("the working tree")
        assert head in sc, sc
        assert "◉" in sc and "[current]" in sc, sc   # the dev tree runs: tree is current
        # the text filters (on the revision or the subject)
        typed(second)
        wait_screen("/version %s" % second)     # the keys are in (the subject is on screen before the filter)
        tui_tmux.wait_gone("the working tree")
        sc = wait_screen(second_subject[:40])
        assert "the working tree" not in sc, sc
        assert head not in sc.split("/version")[-1] or head == second, sc
        # clear the filter, move down twice (tree -> head -> second)
        for _ in range(len(second)):
            keys("BSpace")
        wait_screen("the working tree")
        keys("Down")
        keys("Down")
        time.sleep(0.3)
        # Tab fills the composer with the selected entry
        keys("Tab")
        sc = wait_screen("/version %s" % second)
        # an unknown commit: a clear answer, nothing is built
        for _ in range(len("/version %s" % second)):
            keys("BSpace")
        typed("/version zzzz999")   # matches no entry: the popup is closed
        time.sleep(0.3)
        keys("Enter")
        wait_screen("unknown commit zzzz999")
        # Enter on an entry builds, then switches: the build is announced
        # (the working tree: a commit would first check out a worktree)
        typed("/version ")
        wait_screen("the working tree")
        time.sleep(0.3)
        keys("Enter")
        wait_screen("version tree: building", 20)
        wait_screen("build of tree failed", 20)
        print("OK: /version picker (list, filter, arrows, Tab, unknown commit, Enter builds)")
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
