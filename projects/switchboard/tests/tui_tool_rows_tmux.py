"""BISE-223: in main, a bash call is one row with the model's description
(`$ je dis bonjour … ✓ 0.1s`), a failed one adds its error line; ctrl+o
opens every call into its box (the description is its title), ctrl+o
again brings the rows back. Through the real binaries (tmux, fake
provider: `[[bash: CMD @@ DESC]]`).

python3 -u projects/switchboard/tests/tui_tool_rows_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run  # noqa: E402


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("[[bash: echo bonjour @@ je dis bonjour]] "
                "[[bash: echo 'Error: pas de chance' >&2; exit 3 @@ je rate exprès]]")
        t.keys("Enter")
        sc = t.wait_re(r"\$ je rate exprès +✗ exit 3", 60)
        t.wait("done: ", 30)
        sc = t.screen()
        print("---- rows ----\n%s" % sc)
        assert "$ je dis bonjour" in sc, sc
        assert "Error: pas de chance" in sc, sc
        # the code stays behind the row
        assert "│ echo bonjour" not in sc and "╭─ $" not in sc, sc
        # ctrl+o: every box, the description as title
        t.keys("C-o")
        sc = t.wait("╭─ $ je dis bonjour ✓")
        t.wait("╭─ $ je rate exprès ✗ exit 3")
        assert "│ echo bonjour" in t.screen(), t.screen()
        print("---- ctrl+o ----\n%s" % t.screen())
        # ctrl+o again: the rows
        t.keys("C-o")
        t.wait_gone("╭─ $ je dis bonjour")
        t.wait_re(r"\$ je dis bonjour +✓")
        print("PASS tui tool rows")


if __name__ == "__main__":
    run(main)
