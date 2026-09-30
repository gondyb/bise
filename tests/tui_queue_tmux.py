"""Queued messages (BISE-89) on a throwaway hub (tmux, fake provider):
during a slow turn, tab keeps the composer text in the TUI (nothing goes
to the hub), the queue shows above the composer with its hint and the
panel row counts it; ↑ in an empty composer pops the newest back to
edit, tab queues it again; when the turn ends the queue goes out in
order, one message per turn.

python3 -u tests/tui_queue_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, in_view, pane_rows, wait_until  # noqa: E402

HINT = "queued · sent when this turn ends · ↑ edit"


def sent(E):
    """The messages main's model answered, in order (the fake provider
    answers `ack: <the message>`)."""
    out = []
    for r in E.fake_requests():
        reply = r.get("reply")
        text = reply.get("content", "") if isinstance(reply, dict) else str(reply or "")
        if r.get("agent") == "main" and text.startswith("ack: "):
            out.append(text[len("ack: "):])
    return out


def composer_row(t):
    """The composer's text (book §8 "The frame"): the rows with the bar
    under the divider, their text after the bar."""
    return " ".join(x for x in pane_rows(t.screen().splitlines()) if x)


def main():
    with tui_session(150, 42) as t:
        t.wait("bise :*")
        t.wait_re(in_view("main"))
        # a slow turn: main runs `sleep 8`
        t.typed("[[bash: sleep 8]]")
        t.keys("Enter")
        t.wait("tab queue   ⏎ steer", 20)
        # tab queues: nothing goes to the hub
        t.typed("queued-one")
        t.keys("Tab")
        t.wait(" › queued-one", 10)
        t.typed("queued-two")
        t.keys("Tab")
        t.wait(" › queued-two", 10)
        sc = t.wait("· 2 queued", 5)
        assert HINT in sc, sc
        assert sc.index(" › queued-one") < sc.index(" › queued-two"), sc
        assert not any("queued-" in m for m in sent(t.E)), sent(t.E)
        # ↑ in an empty composer: the newest back to edit; tab queues it again
        t.keys("Up")
        t.wait("· 1 queued", 5)
        assert "queued-two" in composer_row(t), t.screen()
        t.typed(" edited")
        t.keys("Tab")
        sc = t.wait(" › queued-two edited", 5)
        t.wait("· 2 queued", 5)
        assert "queued" not in composer_row(t), t.screen()
        assert not any("queued-" in m for m in sent(t.E)), sent(t.E)
        # the turn ends: the queue goes out in order, one per turn
        def queued():
            return [m for m in sent(t.E) if "queued-" in m]
        got = wait_until(lambda: len(queued()) >= 2 and queued(), 60,
                         lambda: "the queue did not go out: %r" % queued(), poll=0.3)
        assert got[:1] == ["queued-one"], got
        assert any(m == "queued-two edited" for m in got), got
        t.wait("ack: queued-two edited", 30)
        t.wait_gone(HINT, 10)
        sc = t.screen()
        print(sc)
        assert "· 2 queued" not in sc and "· 1 queued" not in sc, sc
        print("PASS tui queue")


if __name__ == "__main__":
    run(main)
