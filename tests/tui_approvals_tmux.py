"""Approvals (docs/approvals-design.md §8-§10) in a real terminal (tmux)
against the fake provider, the checker off, a temp HOME and BISE_HOME:
the key bar's `⇧⇥ yolo`, shift+tab and its 3-second flash, the mode kept
in config.toml, `/approvals`, a force push to main in auto: the tool row
`? waiting for you`, the card in the inbox (main's view and a task's),
its look (no "always" for a hard rule), a no with a note and its fold;
a checker-off card for a network call (it asks even with the sandbox)
with "always allow … here", allowed with `1`.

SB_DUMP=<dir>: every screen checked is written there (designer's review).

python3 -u tests/tui_approvals_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

COLS, ROWS = 140, 40


def open_card(t, who):
    """ctrl+g: the inbox selected on the card (⏎ opens it), or its view"""
    t.keys("C-g")
    i, _ = t.wait_any(["▸ ? %s wants to run" % who, "type why not, ⏎ says no"])
    if i == 0:
        t.keys("Enter")


def main():
    E = e2e.Env()
    home, bise = os.path.join(E.tmp, "home"), os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('[roles]\nclassify = "off"\n')
    E.env.update(HOME=home, BISE_HOME=bise)
    dump = os.environ.get("SB_DUMP")
    n = [0]

    def shot(t, name, sc):
        """designer's review: the screen as the user sees it"""
        if dump:
            os.makedirs(dump, exist_ok=True)
            n[0] += 1
            with open(os.path.join(dump, "%02d-%s.txt" % (n[0], name)), "w") as f:
                f.write(sc)
            with open(os.path.join(dump, "%02d-%s.ansi" % (n[0], name)), "w") as f:
                f.write(t.screen(colors=True))

    env = "HOME=%s BISE_HOME=%s" % (home, bise)
    with tui_session(COLS, ROWS, env=env, E=E) as t:
        t.wait("bise :*")
        sc = t.wait("⇧⇥ yolo")
        last = sc.rstrip("\n").split("\n")
        bar = [l for l in last if "⇧⇥ yolo" in l][0]
        assert bar.rstrip().rstrip("│").rstrip().endswith("⇧⇥ yolo"), bar
        shot(t, "yolo-key-bar", sc)
        # shift+tab: the flash, then the tag says auto; config.toml keeps it
        t.keys("BTab")
        sc = t.wait("auto · edits run, commands ask you")
        shot(t, "switch-flash", sc)
        t.wait("⇧⇥ auto", timeout=10)
        cfg = open(os.path.join(bise, "config.toml")).read()
        assert 'approvals = "auto"' in cfg, cfg
        # /approvals: the mode, the checker, the rules
        t.typed("/approvals")
        t.keys("Enter")
        sc = t.wait("checker: off · every command asks you. /models changes it.")
        assert "approvals: auto · edits run, commands ask you" in sc, sc
        shot(t, "slash-approvals", sc)
        # a force push to main: the row waits for you, a card with no always
        t.typed("[[bash: git push origin main --force]]")
        t.keys("Enter")
        sc = t.wait("waiting for you")
        t.wait("main wants to run")
        sc = t.screen()
        shot(t, "tool-row-and-strip", sc)
        open_card(t, "main")
        sc = t.wait("type why not, ⏎ says no")
        assert "always allow" not in sc, sc
        assert "1 allow" in sc and "3 no" in sc and "2 " not in sc.split("1 allow")[1][:40], sc
        shot(t, "card-hard-rule", sc)
        t.typed("use a branch")
        t.keys("Enter")
        sc = t.wait('you said no to main: git push origin main --force · "use a branch"')
        assert "you said deny" not in sc, sc
        shot(t, "fold-no", sc)
        t.wait(" idle")
        # a task's call: the card shows in main's view and in the task's
        # a network call: it asks even when the sandbox contains the rest
        t.typed("/new t1: {{bash: curl -s -m 1 http://127.0.0.1:9/}}")
        t.keys("Enter")
        sc = t.wait("t1 wants to run", timeout=60)
        shot(t, "card-in-main-view", sc)
        t.keys("M-1")
        sc = t.wait("waiting for you")
        assert "t1 wants to run" in sc, sc
        shot(t, "card-in-task-view", sc)
        open_card(t, "t1")
        sc = t.wait("2 always allow ")
        t.wait("type why not, ⏎ says no")
        shot(t, "card-checker-off", sc)
        t.keys("1")
        sc = t.wait("you allowed t1: curl -s -m 1 http://127.0.0.1:9/")
        shot(t, "fold-allowed", sc)


if __name__ == "__main__":
    run(main)
