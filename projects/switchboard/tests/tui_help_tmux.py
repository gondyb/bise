"""/help and /shortcuts in a real terminal (tmux) on a throwaway hub with
the fake provider, at 80 and 200 columns: the overlay opens, shows its
sections and key chips, filters as you type, Tab switches the page, Esc
clears the filter then closes; /keys is an alias. Prints the captures.

TMPDIR=/tmp/hf-run python3 -u projects/switchboard/tests/tui_help_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone  # noqa: E402
import tui_tmux  # noqa: E402


def run(cols, rows):
    S = "sbhelp%d_%d" % (os.getpid(), cols)
    tui_tmux.S = S
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, cols, rows)
        wait_screen("bise :*")
        wait_screen(" idle")
        # /help: the commands and the essential keys
        typed("/help")
        keys("Enter")
        sc = wait_screen("essential keys · every key")
        assert "commands" in sc and "/shortcuts" in sc, sc
        print("---- /help at %d columns ----\n%s" % (cols, sc))
        # Tab: every key, sections
        keys("Tab")
        sc = wait_screen("agents (empty composer)")
        assert "talk to agents" in sc, sc
        print("---- Tab -> /shortcuts at %d columns ----\n%s" % (cols, sc))
        # type to filter
        typed("subword")
        sc = wait_screen("filter: subword")
        assert "ctrl+option+←" in sc and "agents (empty" not in sc, sc
        print("---- filter 'subword' at %d columns ----\n%s" % (cols, sc))
        # Esc clears the filter, Esc again closes
        keys("Escape")
        wait_gone("filter: subword")
        keys("Escape")
        wait_gone("talk to agents")
        # /keys alias, then PgDn reaches the end (Ghostty tips)
        typed("/keys")
        keys("Enter")
        wait_screen("talk to agents")
        keys("End")
        sc = wait_screen("keyprobe")
        print("---- /keys, End at %d columns ----\n%s" % (cols, sc))
        keys("Escape")
        wait_gone("keyprobe")
        # the composer works again
        typed("still typing")
        wait_screen("still typing")
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
    return ok


def main():
    ok = run(80, 30) and run(200, 50)
    if ok:
        print("OK: /help and /shortcuts overlay at 80 and 200 columns (open, Tab, filter, Esc, /keys, End)")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
