"""Images in the composer (docs/images.md), in a real terminal (tmux),
against the fake provider: an image picked in the `@` popup, a file
path pasted like a Finder drag-and-drop, and Ctrl+V (the clipboard
image) each become `[Image #N]`, drawn as the chip `▣ N` (BISE-70) with
the strip above the composer; on send the request to the provider
carries each one as an image_url part with the PNG data, and the feed
shows the image by name, not the marker.

python3 -u projects/switchboard/tests/tui_images_tmux.py
"""
import base64
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tmux, screen, keys, typed, wait_screen  # noqa: E402
import tui_tmux  # noqa: E402
import tui_composer_tmux  # noqa: E402

S = "sbimg%d" % os.getpid()
tui_tmux.S = S
tui_composer_tmux.S = S

# 1x1 PNG
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")


def wait_composer(text, timeout=5):
    """tui_composer_tmux.wait_composer, the hint shown while images are
    attached (`ctrl+v paste image · @ file`) left out too."""
    import re
    t0 = time.time()
    got = ""
    while time.time() - t0 < timeout:
        got = re.sub(r"\s{2,}ctrl\+v paste image.*$", "", tui_composer_tmux.composer(), flags=re.M)
        if got == text:
            return
        time.sleep(0.1)
    print(screen())
    raise AssertionError("composer %r, expected %r" % (got, text))


def paste(text):
    """A bracketed paste, as a terminal does for a dropped file."""
    tmux("set-buffer", "-b", "shot", text)
    tmux("paste-buffer", "-p", "-d", "-b", "shot", "-t", S)


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
    ok = False
    try:
        tui_tmux.start_tui(E, 150, 42, "BEND_CLIPBOARD_IMAGE_FILE=%s" % clip)
        wait_screen("bise :*")
        wait_screen(" idle")
        # the @ popup: an image is attached, not inserted as a path
        typed("look @red-bl")
        wait_screen("shots/red-blue.png")
        keys("Tab")
        wait_composer("look ▣ 1")
        # the strip above the composer says what the chip is
        wait_screen("attached · backspace on a chip removes it")
        # the file name only, never its path (book §13), in the strip and
        # in the divider's flash '✓ attached ▣ 1 red-blue.png'
        sc = wait_screen("▣ 1 red-blue.png")
        assert "shots/red-blue" not in sc, sc
        wait_screen("1×1 · 70 B")
        # a dropped file: the terminal pastes its shell-escaped path
        paste(drop.replace(" ", "\\ ") + " ")
        wait_composer("look ▣ 1 ▣ 2")
        # a paste that is not an image path stays text
        paste("and")
        wait_composer("look ▣ 1 ▣ 2 and")
        # Ctrl+V: the clipboard image
        keys("C-v")
        wait_composer("look ▣ 1 ▣ 2 and ▣ 3")
        wait_screen("▣ 3 clipboard")
        # backspace on a chip removes it whole (and its strip row)
        keys("BSpace")
        keys("BSpace")
        wait_composer("look ▣ 1 ▣ 2 and")
        keys("C-v")
        wait_composer("look ▣ 1 ▣ 2 and ▣ 3")
        typed("colors?")
        keys("Enter")
        # the feed names the image: `[Image #1 shots/red-blue.png]`, or
        # the chip `▣ red-blue.png` once render.rs draws the chips
        t0 = time.time()
        while time.time() - t0 < 40:
            sc = screen()
            if "[Image #1 shots/red-blue.png]" in sc or "▣ red-blue.png" in sc:
                break
            time.sleep(0.2)
        assert "red-blue.png" in sc and "attached ·" not in sc, sc
        # the reply names the images as the model saw them; no marker
        # (neither the user's nor the request's framing) on screen
        sc = wait_screen("ack: look [Image #1] [Image #2] and [Image #3] colors?", 40)
        assert "<image name=" not in sc, sc
        t0 = time.time()
        reqs = []
        while time.time() - t0 < 30:
            reqs = [r for r in E.fake_requests() if r.get("images")]
            if reqs:
                break
            time.sleep(0.2)
        assert reqs, E.fake_requests()
        imgs = reqs[0]["images"]
        assert len(imgs) == 3, imgs
        data = "data:image/png;base64," + base64.b64encode(PNG).decode()
        assert all(u == data for u in imgs), imgs
        # the store holds the copies
        assert any(n.endswith(".b64") for n in os.listdir(store)), os.listdir(store)
        ok = True
        print("PASS tui images")
    finally:
        tmux("kill-session", "-t", S)
        try:
            c = e2e.Client(os.path.join(E.state, "hub.sock"))
            c.send({"op": "stop_hub"})
        except Exception:
            pass
        E.close()
    # outside the finally: a sys.exit there would swallow the traceback
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
