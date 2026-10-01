"""BISE-223: in main, a bash call is one row with the model's description
(`$ je dis bonjour … ✓ 0.1s`), a failed one adds its error line; ctrl+o
opens every call into its box (the description is its title), ctrl+o
again brings the rows back. BISE-283: a skill call is a sentence,
`read skill bise-demo` (no JSON, no `▸ output`), a failed one
`read skill bise-dmeo … ✗ unknown skill`; ctrl+o opens its SKILL.md in a
box. Through the real binaries (tmux, fake provider: `[[bash: CMD @@
DESC]]`, `[[skill: NAME]]`, `[[write_file: JSON]]`).

python3 -u tests/tui_tool_rows_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
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
        # BISE-283: skill calls read as sentences
        t.typed("[[skill: bise-demo]] [[skill: bise-dmeo]]")
        t.keys("Enter")
        t.wait_re(r"read skill bise-dmeo +✗ unknown skill", 60)
        t.wait("unknown skill: bise-dmeo", 30)   # the fake's `done: <result>`
        sc = t.screen()
        print("---- skill rows ----\n%s" % sc)
        assert "read skill bise-demo" in sc, sc
        assert '{"name"' not in sc and "skill ✓" not in sc and "▸ output" not in sc, sc
        # ctrl+o: the SKILL.md in a box titled with the sentence
        t.keys("C-o")
        t.wait("╭─ read skill bise-demo ✓")
        t.wait("# bise demo")
        print("---- skill ctrl+o ----\n%s" % t.screen())
        t.keys("C-o")
        t.wait_gone("╭─ read skill bise-demo")
        # BISE-304: 3 done edits fold into one row; ctrl+o brings the rows
        # (and their diffs) back
        t.typed(" ".join('[[write_file: {"file_path": "%s", "content": "a\\nb\\n"}]]' % p
                         for p in ("src/a.ts", "src/b.ts", "README.md")))
        t.keys("Enter")
        sc = t.wait_re(r"± ▸ 3 files · a\.ts, b\.ts, README\.md +✓ \+9", 60)
        print("---- edits fold ----\n%s" % sc)
        assert "± write src/a.ts" not in sc, sc
        t.keys("C-o")
        sc = t.wait("± write src/a.ts ✓ +3 ▾")
        assert "± ▾ 3 files" in sc, sc
        t.keys("C-o")
        t.wait_gone("± write src/a.ts")
        print("PASS tui tool rows")


if __name__ == "__main__":
    run(main)
