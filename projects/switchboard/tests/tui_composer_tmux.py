"""The composer in a real terminal (tmux, legacy key encodings), against
the fake provider: Up recalls the history and Down past the newest entry
brings the draft back; Option+←/→ (ESC b / ESC f) jump words, Cmd+←/→
(Ctrl+A / Ctrl+E in Ghostty) jump to the line ends, Option+Backspace and
Cmd+Backspace (Ctrl+U) delete a word / to the line start, Ctrl+/ (0x1F)
undoes, Option+` then e (ESC ` e) types è. The mouse (SGR reports written to the pane): a drag in the feed
selects and copies on release ("copied N chars"), a drag in the composer
too; the copies go to BEND_CLIPBOARD_FILE, never the real clipboard.

python3 -u projects/switchboard/tests/tui_composer_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen  # noqa: E402
import tui_tmux  # noqa: E402

S = "sbcomp%d" % os.getpid()
tui_tmux.S = S


def composer():
    """The composer's text row(s) (book §8 "The frame"): the rows with the
    bar `│` under the divider, the blank bar rows left out, each row's
    text after the bar."""
    rows = screen().rstrip("\n").splitlines()
    return "\n".join(x for x in tui_tmux.pane_rows(rows) if x)


def wait_composer(text, timeout=5):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if composer() == text:
            return
        time.sleep(0.1)
    print(screen())
    raise AssertionError("composer %r, expected %r" % (composer(), text))


def mouse(kind, x, y):
    """One SGR mouse report at the 0-based cell (x, y): press, drag, release."""
    code, end = {"press": (0, "M"), "drag": (32, "M"), "release": (0, "m")}[kind]
    typed("\x1b[<%d;%d;%d%s" % (code, x + 1, y + 1, end))


def find(text):
    """The 0-based (column, row) of `text` on the screen."""
    for y, r in enumerate(screen().splitlines()):
        if text in r:
            return r.index(text), y
    raise AssertionError("not on screen: %r" % text)


def wait_clip(path, text, timeout=5):
    t0 = time.time()
    got = None
    while time.time() - t0 < timeout:
        if os.path.exists(path):
            got = open(path).read()
            if got == text:
                return
        time.sleep(0.1)
    raise AssertionError("clipboard %r, expected %r" % (got, text))


def main():
    E = e2e.Env()
    clip = os.path.join(E.tmp, "clipboard.txt")
    E.env["BEND_CLIPBOARD_FILE"] = clip
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42)
        wait_screen("bise :*")
        wait_screen(" idle")
        # one entry in the history
        typed("first message")
        keys("Enter")
        wait_screen("first message")
        time.sleep(0.5)
        # a draft; Up shows the history, Down brings the draft back
        typed("my draft words")
        wait_composer("my draft words")
        keys("Up")
        wait_composer("first message")
        keys("Down")
        wait_composer("my draft words")
        # word left (ESC b), then type: inserted before "words"
        keys("M-b")
        typed("X")
        wait_composer("my draft Xwords")
        # line start (Ctrl+A = Cmd+←) and line end (Ctrl+E = Cmd+→)
        keys("C-a")
        typed("Y")
        keys("C-e")
        typed("Z")
        wait_composer("Ymy draft XwordsZ")
        # word right from the start (ESC f): after "Ymy"
        keys("C-a")
        keys("M-f")
        typed("!")
        wait_composer("Ymy! draft XwordsZ")
        # Option+Backspace deletes the word before the cursor
        keys("C-e")
        keys("M-BSpace")
        wait_composer("Ymy! draft")
        # Cmd+Backspace (Ctrl+U) deletes to the line start; Ctrl+/ undoes
        keys("C-u")
        wait_composer("")   # the empty composer: the prompt and the cursor only
        keys("C-_")
        wait_composer("Ymy! draft")
        # the feed: drag over "first message" in the fake reply, release copies
        x, y = find("ack: first message")
        mouse("press", x + 5, y)
        mouse("drag", x + 10, y)
        mouse("drag", x + 17, y)
        mouse("release", x + 17, y)
        wait_screen("copied 13 chars")
        wait_clip(clip, "first message")
        # a plain click in the feed selects nothing (no copy)
        os.remove(clip)
        mouse("press", x + 2, y)
        mouse("release", x + 2, y)
        time.sleep(0.5)
        assert not os.path.exists(clip)
        # the composer: a double click selects the word, the release copies
        x, y = find("Ymy! draft")
        mouse("press", x + 6, y)
        mouse("release", x + 6, y)
        mouse("press", x + 6, y)
        mouse("release", x + 6, y)
        wait_clip(clip, "draft")
        # typing replaces the selection
        typed("text")
        wait_composer("Ymy! text")
        # macOS accents with Option as Alt (Ghostty on U.S. layouts):
        # Option+` e = ESC ` e -> è; Option+e e -> é; Option+c -> ç
        keys("M-`")
        typed("e")
        keys("M-e")
        typed("e")
        keys("M-c")
        wait_composer("Ymy! textèéç")
        print("OK: composer (Up/Down keep the draft, word and line jumps, deletes, undo, Option accents, mouse selection + copy in the feed and the composer)")
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
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
