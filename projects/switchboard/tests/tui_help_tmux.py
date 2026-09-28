"""/help and /shortcuts in a real terminal (tmux) on a throwaway hub with
the fake provider, at 80 and 200 columns: the overlay opens, shows its
sections and key chips, filters as you type, Tab switches the page, Esc
clears the filter then closes; /keys is an alias. Prints the captures.

TMPDIR=/tmp/hf-run python3 -u projects/switchboard/tests/tui_help_tmux.py
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen  # noqa: E402
import tui_tmux  # noqa: E402


def wait_gone(needle, timeout=10):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if needle not in screen():
            return
        time.sleep(0.1)
    print(screen())
    raise AssertionError("still on screen: %r" % needle)


def run(cols, rows):
    S = "sbhelp%d_%d" % (os.getpid(), cols)
    tui_tmux.S = S
    E = e2e.Env()
    ok = False
    try:
        envs = " ".join("%s=%s" % (k, subprocess.list2cmdline([v])) for k, v in E.env.items()
                        if k.startswith(("SB_", "BEND_", "MISTRAL_")))
        cmd = "cd %s && env %s %s switchboard --workspace %s; sleep 30" % (
            e2e.ROOT, envs, e2e.EXE, E.ws)
        tmux("new-session", "-d", "-s", S, "-x", str(cols), "-y", str(rows), cmd)
        wait_screen("Switchboard")
        wait_screen(" idle")
        # /help: the commands and the essential keys
        typed("/help")
        keys("Enter")
        sc = wait_screen("Essential keys")
        assert "Commands" in sc and "/shortcuts" in sc, sc
        print("---- /help at %d columns ----\n%s" % (cols, sc))
        # Tab: every key, sections
        keys("Tab")
        sc = wait_screen("Tasks (empty composer)")
        assert "Talk to agents" in sc, sc
        print("---- Tab -> /shortcuts at %d columns ----\n%s" % (cols, sc))
        # type to filter
        typed("subword")
        sc = wait_screen("filter: subword")
        assert "Ctrl+Option+←" in sc and "Tasks (empty" not in sc, sc
        print("---- filter 'subword' at %d columns ----\n%s" % (cols, sc))
        # Esc clears the filter, Esc again closes
        keys("Escape")
        wait_gone("filter: subword")
        keys("Escape")
        wait_gone("Talk to agents")
        # /keys alias, then PgDn reaches the end (Ghostty tips)
        typed("/keys")
        keys("Enter")
        wait_screen("Talk to agents")
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
