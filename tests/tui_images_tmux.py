"""Images in the composer (docs/images.md), in a real terminal (tmux),
against the fake provider: an image picked in the `@` popup, a file
path pasted like a Finder drag-and-drop, Ctrl+V and an empty paste
(the clipboard image) each become `[Image #N]`, drawn as the chip `▣ N` (BISE-70) with
the strip above the composer; on send the request to the provider
carries each one as an image_url part with the PNG data, and the feed
shows the image by name, not the marker.

python3 -u tests/tui_images_tmux.py
"""
import base64
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, tmux, wait_until, MAIN_IDLE  # noqa: E402
from tui_composer_tmux import composer  # noqa: E402

# 1x1 PNG
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")


def wait_composer(t, text, timeout=5):
    """tui_composer_tmux.wait_composer, the hint shown while images are
    attached (`ctrl+v paste image · @ file`) left out too."""
    def shown(sc):
        return re.sub(r"\s{2,}ctrl\+v paste image.*$", "", composer(t, sc), flags=re.M) == text
    shown.__doc__ = "composer %r" % text
    t.wait_any([shown], timeout, poll=0.1)


def paste(t, text):
    """A bracketed paste, as a terminal does for a dropped file."""
    tmux("set-buffer", "-b", "shot", text)
    tmux("paste-buffer", "-p", "-d", "-b", "shot", "-t", t.name)


def main():
    E = e2e.Env()
    os.makedirs(os.path.join(E.ws, "shots"), exist_ok=True)
    with open(os.path.join(E.ws, "shots", "red-blue.png"), "wb") as f:
        f.write(PNG)
    drop = os.path.join(E.tmp, "drop dir", "Screen Shot 1.png")
    os.makedirs(os.path.dirname(drop))
    with open(drop, "wb") as f:
        f.write(PNG)
    clip = os.path.join(E.tmp, "clip.png")
    with open(clip, "wb") as f:
        f.write(PNG)
    store = os.path.join(E.tmp, "images")
    E.env["BEND_IMAGE_DIR"] = store
    with tui_session(150, 42, "BEND_CLIPBOARD_IMAGE_FILE=%s" % clip, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # the @ popup: an image is attached, not inserted as a path
        t.typed("look @red-bl")
        t.wait("shots/red-blue.png")
        t.keys("Tab")
        # the chip is a pill ` ▣ 1 ` (BISE-205): its padding cells are blanks
        wait_composer(t, "look  ▣ 1")
        # the attachments box above your message says what the chip is:
        # `attached` in its top border, the tip in its bottom border
        t.wait("╭─ attached ─")
        t.wait("backspace on a chip removes it ─╯")
        # the file name only, never its path (book §13), in the strip and
        # in the divider's flash '✓ attached ▣ 1 red-blue.png'
        sc = t.wait("▣ 1 red-blue.png")
        assert "shots/red-blue" not in sc, sc
        t.wait("1×1 · 70 B")
        # a dropped file: the terminal pastes its shell-escaped path
        paste(t, drop.replace(" ", "\\ ") + " ")
        wait_composer(t, "look  ▣ 1   ▣ 2")
        # a paste that is not an image path stays text
        paste(t, "and")
        wait_composer(t, "look  ▣ 1   ▣ 2  and")
        # Ctrl+V: the clipboard image
        t.keys("C-v")
        wait_composer(t, "look  ▣ 1   ▣ 2  and  ▣ 3")
        t.wait("│   ▣ 3  clipboard")  # the box row: its edge, the pill, a blank, the name
        # backspace on a chip removes it whole (and its box row)
        t.keys("BSpace")
        t.keys("BSpace")
        wait_composer(t, "look  ▣ 1   ▣ 2  and")
        # Cmd+V on an image in a terminal that sends an empty bracketed
        # paste: the clipboard image too
        tmux("send-keys", "-t", t.name, "-l", "\x1b[200~\x1b[201~")
        wait_composer(t, "look  ▣ 1   ▣ 2  and  ▣ 3")
        t.typed("colors?")
        t.keys("Enter")
        # the feed names the image: `[Image #1 shots/red-blue.png]`, or
        # the chip `▣ red-blue.png` once render.rs draws the chips
        sc = t.wait_any(["[Image #1 shots/red-blue.png]", "▣ red-blue.png"], 40)[1]
        assert "red-blue.png" in sc and "╭─ attached" not in sc, sc
        # the reply names the images as the model saw them; no marker
        # (neither the user's nor the request's framing) on screen
        sc = t.wait("ack: look [Image #1] [Image #2] and [Image #3] colors?", 40)
        assert "<image name=" not in sc, sc
        reqs = wait_until(lambda: [r for r in t.E.fake_requests() if r.get("images")], 30,
                          lambda: "no request with images: %r" % t.E.fake_requests())
        imgs = reqs[0]["images"]
        assert len(imgs) == 3, imgs
        data = "data:image/png;base64," + base64.b64encode(PNG).decode()
        assert all(u == data for u in imgs), imgs
        # the store holds the copies
        assert any(n.endswith(".b64") for n in os.listdir(store)), os.listdir(store)
        print("PASS tui images")


if __name__ == "__main__":
    run(main)
