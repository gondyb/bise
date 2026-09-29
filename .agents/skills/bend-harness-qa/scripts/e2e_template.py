#!/usr/bin/env python3
"""bend-harness e2e template — copy, adapt the TURN and the ASSERTS.

Run:  python3 -u e2e_template.py
(the -u matters: unbuffered stdout, or a long turn looks dead)

Pattern: fresh live session -> one precise turn -> assert on the obs
lines AND the final answer -> print PASS/FAIL. Adapt the `TASK` string
and the `verdict()` function; keep everything else.
"""
import sys
import threading
import time

import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "scripts"))
from bend_client import BendSession

# one precise instruction: exact tool args, one expected behavior
TASK = (
    "Call the skill tool with name \"plain-language\" "
    "(args exactly {\"name\":\"plain-language\"}), then tell me the "
    "first heading of the loaded instructions."
)
TIMEOUT = 420


def verdict(lines, s):
    """Return (ok: bool, details: str). Assert on evidence, not vibes."""
    final = s.last_assistant(lines)
    tool_called = any(l.startswith("tool #") and " skill " in l for l in lines)
    ok = tool_called and "Plain Language" in final
    return ok, "tool_called=%s final=%r" % (tool_called, final[:120])


def main():
    s = BendSession.fresh()  # bg_after=2 shrinks the bash handoff window
    try:
        lines = s.say(TASK, timeout=TIMEOUT)
        ok, details = verdict(lines, s)
        print("--- transcript:")
        for l in lines:
            print("   ", l)
        print("--- verdict:", details)
        print("E2E:", "PASS" if ok else "FAIL")
        return 0 if ok else 1
    finally:
        s.close()


if __name__ == "__main__":
    sys.exit(main())

# --- optional: mid-turn steering variant -------------------------------
# import threading, time
# result = {}
# def turn():
#     result["lines"] = s.say("run: sleep 7 && echo done, then report", timeout=TIMEOUT)
# t = threading.Thread(target=turn); t.start()
# time.sleep(8)                       # inside the bash window
# s.steer_midturn("STEERING: ...")    # side-channel, read at the next boundary
# t.join()
# ok = ("steering_received" in " ".join(result["lines"]))
