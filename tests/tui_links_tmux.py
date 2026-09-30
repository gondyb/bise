"""Links in the feed, in a real terminal (tmux) against the fake
provider: a markdown link and a bare url in your message and in the
reply are drawn as OSC 8 hyperlinks (tmux keeps them: capture-pane -e),
and a plain click on one opens it (BISE_OPEN: a script that logs the
url, in place of `open`).

python3 -u tests/tui_links_tmux.py
"""
import os
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until  # noqa: E402

COLS, ROWS = 150, 42


def main():
    tmp = tempfile.mkdtemp(prefix="sblinks")
    log = os.path.join(tmp, "opened")
    opener = os.path.join(tmp, "open.sh")
    with open(opener, "w") as f:
        f.write('#!/bin/sh\necho "$1" >> %s\n' % log)
    os.chmod(opener, 0o755)
    with tui_session(COLS, ROWS, "BISE_OPEN=" + opener) as t:
        t.wait("bise :*")
        t.wait(" idle")
        t.typed("read [the guide](https://guide.example/start) then https://bare.example/x.")
        t.keys("Enter")
        t.wait("ack: read the guide then https://bare.example/x.")
        # tmux keeps the hyperlinks of the cells: the label and the bare
        # url, each inside its own OSC 8
        sc = t.screen(colors=True)
        for url, text in (("https://guide.example/start", "the guide"), ("https://bare.example/x", "https://bare.example/x")):
            opened = sc.find(";" + url + "\x1b\\")
            assert opened >= 0, "no OSC 8 for %s in %r" % (url, sc[-3000:])
            assert text in sc[opened:opened + 400].replace("\x1b[4m", ""), sc[opened:opened + 400]
        # a plain click (press + release, SGR 1006) on the label of the reply
        rows = t.screen().splitlines()
        y = max(i for i, r in enumerate(rows) if "ack: read the guide" in r)
        x = rows[y].find("the guide") + 3
        t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))
        wait_until(lambda: os.path.exists(log) and "https://guide.example/start" in open(log).read(), 10, lambda: "the click opens the guide")
        t.wait("opening https://guide.example/start")
        # a click on the plain text next to it opens nothing
        x = rows[y].find("ack:")
        t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))
        time.sleep(0.5)
        assert open(log).read().split() == ["https://guide.example/start"], open(log).read()
        print("PASS tui links")


if __name__ == "__main__":
    run(main)
