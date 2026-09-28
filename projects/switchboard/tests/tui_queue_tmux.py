"""Queued messages (BISE-89) on a throwaway hub (tmux, fake provider):
during a slow turn, tab keeps the composer text in the TUI (nothing goes
to the hub), the queue shows above the composer with its hint and the
panel row counts it; ↑ in an empty composer pops the newest back to
edit, tab queues it again; when the turn ends the queue goes out in
order, one message per turn.

python3 -u projects/switchboard/tests/tui_queue_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, keys, typed, wait_screen, wait_re, wait_gone, in_view  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbqueue%d" % os.getpid()
tui_tmux.S = S
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


def composer_row():
    """The composer's text (book §13, the composer block): the last run
    of rows with the bar in column 1, their text from column 3."""
    rows = tui_tmux.screen().splitlines()
    bar = [i for i, r in enumerate(rows) if r.lstrip().startswith("│")]
    if not bar:
        return ""
    first = bar[-1]
    while first > 0 and rows[first - 1].lstrip().startswith("│"):
        first -= 1
    return " ".join(r[r.index("│") + 1:].strip() for r in rows[first:bar[-1] + 1] if r[r.index("│") + 1:].strip())


def main():
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42)
        wait_screen("bise :*")
        wait_re(in_view("main"))
        # a slow turn: main runs `sleep 8`
        typed("[[bash: sleep 8]]")
        keys("Enter")
        wait_screen("tab queue · ⏎ steer", 20)
        # tab queues: nothing goes to the hub
        typed("queued-one")
        keys("Tab")
        wait_screen(" › queued-one", 10)
        typed("queued-two")
        keys("Tab")
        wait_screen(" › queued-two", 10)
        sc = wait_screen("· 2 queued", 5)
        assert HINT in sc, sc
        assert sc.index(" › queued-one") < sc.index(" › queued-two"), sc
        assert not any("queued-" in m for m in sent(E)), sent(E)
        # ↑ in an empty composer: the newest back to edit; tab queues it again
        keys("Up")
        wait_screen("· 1 queued", 5)
        assert "queued-two" in composer_row(), tui_tmux.screen()
        typed(" edited")
        keys("Tab")
        sc = wait_screen(" › queued-two edited", 5)
        wait_screen("· 2 queued", 5)
        assert "queued" not in composer_row(), tui_tmux.screen()
        assert not any("queued-" in m for m in sent(E)), sent(E)
        # the turn ends: the queue goes out in order, one per turn
        t0 = time.time()
        while time.time() - t0 < 60:
            got = [m for m in sent(E) if "queued-" in m]
            if len(got) >= 2:
                break
            time.sleep(0.3)
        got = [m for m in sent(E) if "queued-" in m]
        assert got[:1] == ["queued-one"], got
        assert any(m == "queued-two edited" for m in got), got
        wait_screen("ack: queued-two edited", 30)
        wait_gone(HINT, 10)
        sc = tui_tmux.screen()
        print(sc)
        assert "· 2 queued" not in sc and "· 1 queued" not in sc, sc
        ok = True
        print("PASS tui queue")
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
