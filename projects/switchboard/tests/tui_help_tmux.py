"""/help and /shortcuts in a real terminal (tmux) on a throwaway hub with
the fake provider, at 80 and 200 columns: the overlay opens, shows its
sections and key chips, filters as you type, Tab switches the page, Esc
clears the filter then closes; /keys is an alias. Prints the captures.

TMPDIR=/tmp/hf-run python3 -u projects/switchboard/tests/tui_help_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run  # noqa: E402


def check(cols, rows):
    with tui_session(cols, rows) as t:
        t.wait("bise :*")
        t.wait(" idle")
        # /help: the commands and the essential keys
        t.typed("/help")
        t.keys("Enter")
        sc = t.wait("essential keys · every key")
        assert "commands" in sc and "/shortcuts" in sc, sc
        print("---- /help at %d columns ----\n%s" % (cols, sc))
        # Tab: every key, sections
        t.keys("Tab")
        sc = t.wait("agents (empty composer)")
        assert "talk to agents" in sc, sc
        print("---- Tab -> /shortcuts at %d columns ----\n%s" % (cols, sc))
        # type to filter
        t.typed("subword")
        sc = t.wait("filter: subword")
        assert "ctrl+option+←" in sc and "agents (empty" not in sc, sc
        print("---- filter 'subword' at %d columns ----\n%s" % (cols, sc))
        # Esc clears the filter, Esc again closes
        t.keys("Escape")
        t.wait_gone("filter: subword")
        t.keys("Escape")
        t.wait_gone("talk to agents")
        # /keys alias, then PgDn reaches the end (Ghostty tips)
        t.typed("/keys")
        t.keys("Enter")
        t.wait("talk to agents")
        t.keys("End")
        sc = t.wait("keyprobe")
        print("---- /keys, End at %d columns ----\n%s" % (cols, sc))
        t.keys("Escape")
        t.wait_gone("keyprobe")
        # the composer works again
        t.typed("still typing")
        t.wait("still typing")


def main():
    check(80, 30)
    check(200, 50)
    print("OK: /help and /shortcuts overlay at 80 and 200 columns (open, Tab, filter, Esc, /keys, End)")


if __name__ == "__main__":
    run(main)
