"""A long paste becomes a chip (BISE-240, book §13), in a real terminal
(tmux), against the fake provider: a bracketed paste of 300 lines puts
the chip `▤ 1` in the composer and a row in the attachments box, not 300
lines; a short paste stays inline; on send the model gets the whole text
in a `<pasted>` tag where the chip was, and the history shows the chip
and one dim row, never the 300 lines.

python3 -u tests/tui_paste_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, tmux, wait_until  # noqa: E402
from tui_composer_tmux import composer, wait_composer  # noqa: E402

N = 300
TEXT = "".join("pasted line %03d of the log\n" % i for i in range(1, N + 1))


def paste(t, text):
    """A bracketed paste, as a terminal does for Cmd+V."""
    tmux("set-buffer", "-b", "long", text)
    tmux("paste-buffer", "-p", "-d", "-b", "long", "-t", t.name)


def main():
    with tui_session(120, 40) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("sum up ")
        paste(t, TEXT)
        # one chip, not 300 lines; the flash names it
        wait_composer(t, "sum up  ▤ 1")
        sc = t.wait("╭─ attached ─")
        assert "pasted line 002" not in composer(t, sc), sc
        t.wait("300 lines · 8 kB")
        t.wait("▤ 1  “pasted line 001 of the log pasted line 00")
        # a short paste stays text
        paste(t, "please")
        wait_composer(t, "sum up  ▤ 1  please")
        t.keys("Enter")
        # the model gets the whole text in its tag, where the chip was
        def got():
            return [r for r in t.E.fake_requests() if r.get("user", "").startswith("sum up")]
        reqs = wait_until(got, 40, lambda: "no request: %r" % t.E.fake_requests())
        r = reqs[0]
        head = r["user"]
        assert head.startswith('sum up <pasted n="1" lines="300">\npasted line 001 of the log\n'), head[:200]
        # one message, not cut where a recv ended (repl-core.bend)
        assert r["user_len"] >= len(TEXT), r["user_len"]
        assert r["user_tail"].endswith("pasted line 300 of the log\n</pasted> please"), r["user_tail"]
        assert len(reqs) == 1 and not any("pasted line 2" in u[:20] for u in r["users"]), r["users"]
        # the history: the chip in your line, one dim row under it
        sc = t.wait("sum up ▤ 1 please")
        t.wait("▤ 1 “pasted line 001 of the log")
        t.wait("· 300 lines")
        sc = t.screen()
        # (the fake's `ack:` echoes the tag: that is the model's text)
        assert "pasted line 150" not in sc, sc
        assert not any("<pasted" in l for l in sc.splitlines() if "sum up" in l and "ack:" not in l), sc
        assert "╭─ attached" not in sc, sc
        # ctrl+o opens your message: the full paste under it (300 rows:
        # its head `▤ 1 · 300 lines` scrolls off; the model's `ack:` only
        # echoes the first lines)
        t.keys("C-o")
        t.wait("│  pasted line 300 of the log")
        print("PASS tui paste")


if __name__ == "__main__":
    run(main)
