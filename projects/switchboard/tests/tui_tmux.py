"""Drive the switchboard TUI in a real terminal (tmux) against the fake
provider, and check the screen: panel, checkout, Esc, preview, cards.

python3 -u projects/switchboard/tests/tui_tmux.py
"""
import contextlib
import itertools
import os
import re
import subprocess
import sys
import time
import traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402

def tmux(*a):
    return subprocess.run(["tmux", *a], capture_output=True, text=True).stdout


def wait_until(fn, timeout, what, poll=0.2):
    """Call `fn` until it returns a truthy value, and return it; after
    `timeout` s: AssertionError(what()). The one poll loop of the tmux
    tests (the screen, the fake provider's log, a file). `timeout` is for
    an idle machine: a loaded one gets it times load_factor() (read at
    each poll), so a test that passes returns as soon as it would, and
    only a broken one waits longer before failing."""
    t0 = time.time()
    while True:
        got = fn()
        if got:
            return got
        if time.time() - t0 >= timeout * e2e.load_factor():
            raise AssertionError(what())
        time.sleep(poll)


class Tui:
    """One switchboard TUI in its own tmux session, on the throwaway hub
    `E` (e2e.Env). Made by `tui_session`, which also tears it down."""

    def __init__(self, E, name):
        self.E = E
        self.name = name

    def screen(self, colors=False):
        """The pane's text (with colors: its SGR escapes too)."""
        return tmux("capture-pane", "-p", *(["-e"] if colors else []), "-t", self.name)

    def keys(self, *k):
        tmux("send-keys", "-t", self.name, *k)

    def typed(self, text):
        tmux("send-keys", "-t", self.name, "-l", text)

    def wait_any(self, conds, timeout=40, poll=0.2, colors=False):
        """Poll the screen (with its SGR escapes if `colors`) until one of
        `conds` holds; return (its index, the screen). A cond is a
        substring, a compiled regex (searched multiline) or a function of
        the screen. The one poll loop of the tmux tests: a flake fix here
        fixes them all."""
        def holds(c, sc):
            if isinstance(c, str):
                return c in sc
            if isinstance(c, re.Pattern):
                return c.search(sc) is not None
            return bool(c(sc))
        last = [""]

        def match():
            last[0] = sc = self.screen(colors)
            for i, c in enumerate(conds):
                if holds(c, sc):
                    return i, sc
            return None

        def missing():
            print(last[0])
            if not last[0].strip():
                # an empty screen: the session is gone (tmux said why)
                r = subprocess.run(["tmux", "capture-pane", "-p", "-t", self.name],
                                   capture_output=True, text=True)
                print("[empty screen: tmux %r; the TUI's exit is on its screen while it lives]"
                      % r.stderr.strip())
            return "not on screen: %s" % " | ".join(
                "/%s/" % c.pattern if isinstance(c, re.Pattern) else
                repr(c) if isinstance(c, str) else getattr(c, "__doc__", None) or c.__name__
                for c in conds)
        return wait_until(match, timeout, missing, poll)

    def wait(self, needle, timeout=40):
        """Wait for the text `needle` on the screen; return the screen."""
        return self.wait_any([needle], timeout)[1]

    def wait_re(self, pattern, timeout=40):
        """Wait for the regex `pattern` on the screen (multiline)."""
        return self.wait_any([re.compile(pattern, re.M)], timeout)[1]

    def wait_gone(self, needle, timeout=10):
        def gone(sc):
            return needle not in sc
        gone.__doc__ = "gone: %r" % needle
        return self.wait_any([gone], timeout, poll=0.1)[1]

    def start(self, cols, rows, extra_env=""):
        """(Re)open the TUI: a new tmux session of the same name."""
        tmux("kill-session", "-t", self.name)
        start_tui(self.E, cols, rows, extra_env, self.name)

    def close(self, ok):
        """Kill the session, stop the hub the TUI started, clean up (the
        throwaway dirs are kept, SB_KEEP, when the test failed)."""
        tmux("kill-session", "-t", self.name)
        sock = os.path.join(self.E.state, "hub.sock")
        try:
            e2e.Client(sock).send({"op": "stop_hub"})
            t0 = time.time()
            while os.path.exists(sock) and time.time() - t0 < 1:
                time.sleep(0.05)
        except Exception:
            pass
        if not ok:
            os.environ["SB_KEEP"] = "1"
        self.E.close()


_sessions = itertools.count()


@contextlib.contextmanager
def tui_session(cols, rows, env="", E=None):
    """`with tui_session(150, 42) as t:` a TUI on a new throwaway hub (or
    on `E`, which it then owns), torn down at the end, pass or fail."""
    E = E or e2e.Env()
    t = Tui(E, "sbtui%d_%d" % (os.getpid(), next(_sessions)))
    ok = False
    try:
        t.start(cols, rows, env)
        yield t
        ok = True
    finally:
        t.close(ok)


def run(test):
    """A test file's main: `test()` passes or its traceback prints, exit 1."""
    try:
        test()
    except BaseException:
        traceback.print_exc()
        sys.exit(1)
    sys.exit(0)


def panel_row(n, name):
    """The regex of agent `name`'s panel row: its number, a status glyph."""
    return r"\b%d \S+ %s\b" % (n, name)


def in_view(name):
    """The regex of the divider naming the agent in view (book §8 "The
    frame": `├─ you → main ─…─ idle ─┤`, BISE-98)."""
    return r"you → %s " % re.escape(name)


PLACEHOLDER = re.compile(r"^(what's on your mind\?|talk to \S+ directly)$")


def pane_rows(rows):
    """The composer's rows (BISE-98): from the bottom, the first run of
    rows whose text, inside the frame's edges, starts with the bar `│`
    (the key bar and the frame's bottom edge are skipped; a popup may
    hide the divider); each row's text after the bar."""
    out = []
    for r in reversed(rows):
        r = r.rstrip()
        if r.startswith("│"):
            r = r[1:].rstrip()
            if r.endswith("│"):
                r = r[:-1]
        if r.lstrip().startswith("│"):
            text = r.lstrip()[1:].strip()
            # the empty composer's dim placeholder is not text
            out.append("" if PLACEHOLDER.match(text) else text)
        elif out:
            break
    return out[::-1]


def start_tui(E, cols, rows, extra_env, session):
    """Open the switchboard TUI of the throwaway hub E in the tmux session
    `session`, with E's SB_/BEND_/MISTRAL_ env (+ extra_env, "K=V ...")."""
    envs = " ".join("%s=%s" % (k, subprocess.list2cmdline([v])) for k, v in E.env.items()
                    if k.startswith(("SB_", "BEND_", "MISTRAL_")))
    unset = " ".join("-u " + k for k in e2e.AGENT_VARS)   # tmux's server env may carry them
    # a TUI that exits early leaves its last screen and its exit code
    # until close() kills the session (the timeout print shows them)
    cmd = "cd %s && env %s %s%s %s switchboard --workspace %s; echo \"[switchboard exited: $?]\"; sleep 600" % (
        e2e.ROOT, unset, extra_env + " " if extra_env else "", envs, e2e.EXE, E.ws)
    tmux("new-session", "-d", "-s", session, "-x", str(cols), "-y", str(rows), cmd)


def main():
    with tui_session(150, 42) as t:
        sc = t.wait("bise :*")
        t.wait_re(in_view("main"))
        assert "⏎ send   @ file" in sc, sc
        t.wait(" idle")
        t.typed('crée [[bash: sb spawn t1 --objective "écris {{bash: echo hi-t1}}"]]')
        t.keys("Enter")
        sc = t.wait_re(panel_row(1, "t1"))
        t.wait("new agent @t1")
        t.wait_re(r"t1 +(→ \S+|m_\d)", 60)           # the automatic reply in main's feed (level 3: names in columns)
        # select the task with Ctrl+K (next: main, then t1), enter it
        t.keys("C-k")
        t.keys("C-k")
        sc = t.wait("⏎ enter   space preview")
        t.keys("Enter")
        sc = t.wait_re(in_view("t1"))
        assert "you're talking to t1 directly. main isn't in the loop. esc back to main." in sc, sc
        # its brief, folded (BISE-12)
        assert "brief" in sc, sc
        t.wait("done: tool bash ok: hi-t1")
        # talk to it directly
        t.typed("salut t1")
        t.keys("Enter")
        t.wait("ack: salut t1")
        # Esc goes back to main, which learns about it
        t.keys("Escape")
        sc = t.wait_re(in_view("main"))
        t.wait("You talked to @t1 (1 message)")
        # Alt+1 checks out task 1 again; Esc back
        t.keys("M-1")
        t.wait_re(in_view("t1"))
        t.keys("Escape")
        t.wait_re(in_view("main"))
        # preview: select, Space; the status says it; Esc closes
        t.keys("C-k")
        t.keys("C-k")
        t.keys("Space")
        t.wait("preview of t1")
        t.keys("Escape")
        # a slash command and its notice
        t.typed("/agents")
        t.keys("Enter")
        t.wait("écris {{bash: echo hi-t1}}")
        # drop from the panel with D: it asks first (BISE-43), y drops
        t.keys("C-k")
        t.keys("C-k")
        t.typed("D")
        t.wait("drop t1? its history stays in archived. y / n")
        t.typed("y")
        t.wait("@t1 archived", 20)
        t.wait("1 archived")
        print(t.screen())
    print("PASS tui")


if __name__ == "__main__":
    run(main)
