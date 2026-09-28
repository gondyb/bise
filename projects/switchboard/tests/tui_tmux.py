"""Drive the switchboard TUI in a real terminal (tmux) against the fake
provider, and check the screen: panel, checkout, Esc, preview, cards.

python3 -u projects/switchboard/tests/tui_tmux.py
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402

S = "sbtui%d" % os.getpid()


def tmux(*a):
    return subprocess.run(["tmux", *a], capture_output=True, text=True).stdout


def screen():
    return tmux("capture-pane", "-p", "-t", S)


def keys(*k):
    tmux("send-keys", "-t", S, *k)


def typed(text):
    tmux("send-keys", "-t", S, "-l", text)


def wait_screen(needle, timeout=40):
    t0 = time.time()
    while time.time() - t0 < timeout:
        sc = screen()
        if needle in sc:
            return sc
        time.sleep(0.2)
    print(screen())
    raise AssertionError("not on screen: %r" % needle)


def main():
    E = e2e.Env()
    ok = False
    try:
        envs = " ".join("%s=%s" % (k, subprocess.list2cmdline([v])) for k, v in E.env.items()
                        if k.startswith(("SB_", "BEND_", "MISTRAL_")))
        cmd = "cd %s && env %s %s switchboard --workspace %s; sleep 30" % (e2e.ROOT, envs, e2e.EXE, E.ws)
        tmux("new-session", "-d", "-s", S, "-x", "150", "-y", "42", cmd)
        sc = wait_screen("Switchboard")
        assert "Message to main…" in sc, sc
        wait_screen(" idle")
        typed('crée [[bash: sb spawn t1 --objective "écris {{bash: echo hi-t1}}"]]')
        keys("Enter")
        sc = wait_screen("1 t1")
        wait_screen("new task @t1")
        wait_screen("◀ t1 m_", 60)            # the automatic reply in main's feed
        # select the task with Ctrl+K (next: main, then t1), enter it
        keys("C-k")
        keys("C-k")
        sc = wait_screen("⏎ enter · Space preview")
        keys("Enter")
        sc = wait_screen("@t1 ·")
        assert "you talk to the task directly" in sc, sc
        assert "Direct message to @t1…" in sc, sc
        assert "# Task `t1`" in sc or "Task" in sc, sc
        wait_screen("done: tool bash ok: hi-t1")
        # talk to it directly
        typed("salut t1")
        keys("Enter")
        wait_screen("ack: salut t1")
        # Esc goes back to main, which learns about it
        keys("Escape")
        sc = wait_screen("Message to main…")
        wait_screen("You talked to @t1 (1 message)")
        # Alt+1 checks out task 1 again; Esc back
        keys("M-1")
        wait_screen("Direct message to @t1…")
        keys("Escape")
        wait_screen("Message to main…")
        # preview: select, Space; the status says it; Esc closes
        keys("C-k")
        keys("C-k")
        keys("Space")
        wait_screen("preview of @t1")
        keys("Escape")
        # a slash command and its notice
        typed("/tasks")
        keys("Enter")
        wait_screen("écris {{bash: echo hi-t1}}")
        # drop from the panel with D
        keys("C-k")
        keys("C-k")
        typed("D")
        wait_screen("@t1 archived", 20)
        wait_screen("1 archived")
        print(screen())
        ok = True
        print("PASS tui")
    finally:
        tmux("kill-session", "-t", S)
        E.start_hub  # noqa: B018 (the hub was started by the TUI)
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
