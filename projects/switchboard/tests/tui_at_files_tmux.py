"""The `@` popup in a real terminal (tmux), against the fake provider:
agents first, then the files and folders of the workspace (.gitignore
respected); `@` opens it at the start or inline; picking a file puts its
relative path in the composer, picking an agent `@name`.

python3 -u projects/switchboard/tests/tui_at_files_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen, wait_gone  # noqa: E402
import tui_tmux  # noqa: E402
from tui_composer_tmux import composer, wait_composer  # noqa: E402
import tui_composer_tmux  # noqa: E402

S = "sbat%d" % os.getpid()
tui_tmux.S = S
tui_composer_tmux.S = S


def popup_rows():
    """The popup rows above the composer (between the split borders)."""
    return [r for r in screen().splitlines() if ("▪" in r or "▸" in r or " @" in r) and "┃" in r]


def main():
    E = e2e.Env()
    for d in ["src/sb", "target/debug", "docs"]:
        os.makedirs(os.path.join(E.ws, d), exist_ok=True)
    for f, body in [(".gitignore", "target/\n"), ("src/app.rs", ""), ("src/sb/mention.rs", ""),
                    ("docs/at-notes.md", ""), ("target/debug/appcache.rs", "")]:
        with open(os.path.join(E.ws, f), "w") as fh:
            fh.write(body)
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42)
        wait_screen("Switchboard")
        wait_screen(" idle")
        # a task, so an agent is listed
        typed('crée [[bash: sb spawn notes --objective "écris {{bash: echo hi}}"]]')
        keys("Enter")
        wait_screen("new task @notes")
        wait_screen("◀ notes m_", 60)
        # `@` alone at the start: the agent, then the root of the workspace
        typed("@")
        sc = wait_screen("▸ docs/")
        assert "@notes" in sc and "▪ README" in sc, sc
        rows = popup_rows()
        assert "@notes" in rows[0], rows
        # inline: agents and files matching "not", the agent first
        keys("BSpace")
        typed("read @not")
        sc = wait_screen("docs/at-notes.md")
        rows = popup_rows()
        assert "@notes" in rows[0] and "docs/at-notes.md" in rows[1], rows
        # file name before path, the ignored target/ never listed
        for _ in range(3):
            keys("BSpace")
        typed("app")
        sc = wait_screen("src/app.rs")
        assert "appcache" not in sc, sc
        keys("Tab")
        wait_composer("read src/app.rs")
        wait_gone("▪ src/app.rs")
        # a folder part narrows; Enter picks too
        typed("and @sb/me")
        wait_screen("src/sb/mention.rs")
        keys("Enter")
        wait_composer("read src/app.rs and src/sb/mention.rs")
        # mid-word @ (an email) opens nothing
        typed("to a@b")
        time.sleep(0.5)
        assert "▪" not in screen(), screen()
        keys("C-u")
        # picking an agent at the start keeps the routing form
        typed("@no")
        wait_screen("@notes")
        keys("Tab")
        wait_composer("@notes")
        print(screen())
        ok = True
        print("PASS tui at-files")
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
