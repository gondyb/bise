"""Never lose what the user types (BISE-120a), in a real terminal (tmux)
against the fake provider: a draft is on disk once it stops moving, it
comes back after the TUI died (its tmux session killed) and restarted;
a sent prompt leaves the draft and comes back with Up after a restart.

python3 -u projects/switchboard/tests/tui_drafts_tmux.py
"""
import glob
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, pane_rows  # noqa: E402


def composer(sc):
    """The composer's text (its rows, inside the frame's bar)."""
    return " ".join(r.strip() for r in pane_rows(sc.splitlines()))


def main():
    E = e2e.Env()
    # a bise home of its own: the drafts go there, not in the real one
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(bise)
    env = "BISE_HOME=%s" % bise

    def files():
        return glob.glob(os.path.join(bise, "drafts", "*.json"))

    def on_disk():
        f = files()
        return json.load(open(f[0])) if f else {}

    with tui_session(150, 40, env, E=E) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("the first prompt")
        t.keys("Enter")
        t.wait("the first prompt")
        t.wait(" idle")
        t.typed("half a thought")
        t.wait("half a thought")
        # written once it stops moving (the debounce), before any quit
        d = wait_until(lambda: on_disk().get("drafts", {}).get("main"), 5,
                       lambda: "no draft on disk: %r" % on_disk())
        assert d["text"] == "half a thought", d
        assert on_disk()["history"] == ["the first prompt"], on_disk()
        mode = os.stat(files()[0]).st_mode & 0o777
        assert mode == 0o600, oct(mode)
        # the TUI dies (its terminal is gone), a new one starts: the draft is back
        t.start(150, 40, env)
        t.wait(" idle")
        sc = t.wait("half a thought")
        assert "half a thought" in composer(sc), sc
        # sent: gone from the file at once, and from the composer after a restart
        t.keys("Enter")
        wait_until(lambda: "main" not in on_disk().get("drafts", {"main": 1}), 5,
                   lambda: "the sent draft is still on disk: %r" % on_disk())
        assert on_disk()["history"] == ["half a thought", "the first prompt"], on_disk()
        t.wait(" idle")
        t.start(150, 40, env)
        t.wait(" idle")
        t.wait("what's on your mind?")
        # the sent prompts come back with Up, newest first
        t.keys("Up")
        t.wait_any([lambda s: "half a thought" in composer(s)])
        t.keys("Up")
        t.wait_any([lambda s: "the first prompt" in composer(s)])
        print("OK: drafts on disk (debounced, 0600), back after a restart; sent prompts in the history")


if __name__ == "__main__":
    run(main)
