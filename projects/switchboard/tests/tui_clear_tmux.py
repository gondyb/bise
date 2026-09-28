"""/clear and Ctrl+L in a real terminal (tmux) on a throwaway hub with the
fake provider: the feed in focus is emptied (like a terminal clear), and
scrolling up pages the cleared lines back from the hub, in their order;
the lines that come after the clear show below the notice.

python3 -u projects/switchboard/tests/tui_clear_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone  # noqa: E402
import tui_tmux  # noqa: E402


def scroll_up_until(needle, tries=30):
    for _ in range(tries):
        if needle in screen():
            return screen()
        keys("PageUp")
        time.sleep(0.3)
    print(screen())
    raise AssertionError("not back after scrolling up: %r" % needle)


def main():
    S = "sbclear%d" % os.getpid()
    tui_tmux.S = S
    E = e2e.Env()
    ok = False
    try:
        tui_tmux.start_tui(E, 120, 30)
        wait_screen("Switchboard")
        wait_screen(" idle")
        typed("first-marker")
        keys("Enter")
        wait_screen("ack: first-marker")
        typed("second-marker")
        keys("Enter")
        wait_screen("ack: second-marker")
        # /clear: the feed is empty but for the notice
        typed("/clear")
        keys("Enter")
        sc = wait_screen("display cleared")
        assert "first-marker" not in sc and "second-marker" not in sc, sc
        print("---- after /clear ----\n" + sc)
        # a line after the clear shows below the notice
        typed("third-marker")
        keys("Enter")
        sc = wait_screen("ack: third-marker")
        assert "first-marker" not in sc, sc
        assert sc.index("display cleared") < sc.index("third-marker"), sc
        # scrolling up brings the cleared lines back, in order
        sc = scroll_up_until("ack: first-marker")
        keys("End")
        time.sleep(0.5)
        sc = screen()
        print("---- scrolled back, then End ----\n" + sc)
        for a, b in [("first-marker", "second-marker"), ("ack: second-marker", "display cleared"),
                     ("display cleared", "third-marker")]:
            assert a in sc and b in sc and sc.index(a) < sc.index(b), (a, b, sc)
        # Ctrl+L: the same clear, without the notice
        keys("C-l")
        wait_gone("third-marker")
        sc = scroll_up_until("ack: third-marker")
        print("---- Ctrl+L, then scrolled back ----\n" + sc)
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
    if ok:
        print("OK: /clear and Ctrl+L empty the feed; scrolling up pages the cleared lines back in order")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
