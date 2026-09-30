"""Model roles and the voice setup (BISE-298) in a real terminal (tmux),
in a clean bise home and HOME, main on Anthropic, no voice key, the fake
provider behind each provider's base_url (config.toml), the tests' tone
microphone (SB_VOICE_FAKE_MIC):

- ctrl+r with voice off and no voice key opens the voice screen (`which
  model should listen to you?`, Mistral preselected); Mistral → its voice
  models → its key step: a wrong key (the provider's words), a key with
  no credit (saved, then credit added: enter tries again), then it works:
  `✓ voice is on: mistral/voxtral-mini-latest.` and config.toml's
  `[roles] voice`;
- /models shows the voice row; /model's last row opens every role;
  /provider tags Mistral `voice`;
- the lines while talking: a wrong key, no credit, the provider down
  (each keeps the clip: ctrl+r sends it again), the key gone, then the
  kept clip transcribed into the composer.

python3 -u tests/tui_roles_tmux.py
"""
import json
import os
import re
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "   @ file   "


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        return sorted(set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M)))


def main():
    tmp = tempfile.mkdtemp(prefix="sb-roles-")
    credit = os.path.join(tmp, "credit")
    E = e2e.Env(fake_env={"FAKE_CREDIT": credit, "FAKE_STT_TEXT": "hello from the voice"})
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    base = "http://127.0.0.1:%s/v1" % port
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('[roles]\nmain = "anthropic/claude-opus-5-5"\n')
        for p in ("anthropic", "mistral", "openai", "elevenlabs"):
            f.write('\n[providers.%s]\nbase_url = "%s"\n' % (p, base))
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    auth = os.path.join(root, "auth.json")
    blank = " ".join("%s=" % k for k in key_envs() if k != "ANTHROPIC_API_KEY")
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off SB_ONBOARDING=off SB_VOICE_FAKE_MIC=1 ANTHROPIC_API_KEY=good-anthropic %s" % (root, home, blank)

    def set_key(k):
        a = json.load(open(auth)) if os.path.exists(auth) else {}
        if k is None:
            a.pop("mistral", None)
        else:
            a["mistral"] = {"type": "api", "key": k}
        with open(auth, "w") as f:
            json.dump(a, f)

    with tui_session(120, 40, env, E=E) as t:
        t.wait(NORMAL, 60)
        time.sleep(1)
        # voice off, no voice key: ctrl+r opens the voice screen
        t.keys("C-r")
        sc = t.wait("which model should listen to you?")
        assert "you talk, it types in the composer." in sc, sc
        assert "ready" not in sc and "another provider" in sc, sc
        assert re.search(r"› Mistral +voxtral-mini-latest · the key also works for chat", sc), sc
        assert re.search(r"ElevenLabs +scribe_v2 · voice only", sc), sc
        assert "Groq" not in sc and "Deepgram" not in sc, sc
        # esc: voice stays off, said once
        t.keys("Escape")
        t.wait("voice is off. /voice when you want it.")
        # /voice: the same screen (no setup that works)
        # /voice ⏎ fills the line; its first row is the toggle
        t.typed("/voice")
        t.keys("Enter")
        t.wait("turn voice on (ctrl+r: you talk, it types)")
        t.keys("Enter")
        t.wait("which model should listen to you?")
        t.keys("Enter")
        sc = t.wait("which model?")
        assert "voxtral-transcribe-3" in sc and "mistral-medium-latest" not in sc, sc
        t.keys("Enter")
        t.wait("paste your Mistral key")
        # a wrong key: the provider's words, nothing saved
        t.typed("bad-key-1")
        t.keys("Enter")
        sc = t.wait("Mistral says this key is wrong.", 30)
        assert "Invalid API Key" in sc, sc
        assert not os.path.exists(auth), "nothing saved"
        t.keys("Enter")
        t.wait("paste your Mistral key")
        # no credit: saved all the same; credit added, enter tries again
        t.typed("broke-key-1")
        t.keys("Enter")
        t.wait("has no credit yet", 30)
        assert "broke-key-1" in open(auth).read(), "a normal provider key"
        open(credit, "w").close()
        t.keys("Enter")
        sc = t.wait("voice is on: mistral/voxtral-mini-latest.", 30)
        assert "press ctrl+r and talk, any key stops. /voice turns it off." in sc, sc
        cfg = open(os.path.join(root, "config.toml")).read()
        assert 'voice = "mistral/voxtral-mini-latest"' in cfg, cfg
        stt = [r for r in E.fake_requests() if r.get("family") == "stt"]
        assert stt and all(r["model"] == "voxtral-mini-latest" for r in stt), stt
        # /models: the voice row; /model: every role; /provider: the tag
        t.typed("/models")
        t.keys("Enter")
        sc = t.wait("which model does what?")
        assert re.search(r"voice +mistral/voxtral-mini-latest", sc), sc
        assert re.search(r"main +anthropic/claude-opus-5-5", sc), sc
        assert re.search(r"agents +same as main", sc), sc
        t.keys("Escape")
        t.wait(NORMAL)
        time.sleep(0.5)
        t.typed("/model ro")
        t.wait("every role…")
        t.keys("C-u")
        t.typed("/provider")
        t.keys("Enter")
        sc = t.wait("the keys i can use.")
        assert re.search(r"Mistral +✓ saved in bise +voice", sc), sc
        # too wide for the row: the tags go under it
        assert re.search(r"Anthropic +✓ from ANTHROPIC_API_KEY *\n +main · agents · small jobs \(titles, summaries\)", sc), sc
        t.keys("Escape")
        t.wait(NORMAL)
        time.sleep(0.5)

        def talk():
            t.keys("C-r")
            time.sleep(0.8)
            t.keys("Space")

        # while talking: a wrong key
        set_key("bad-key-2")
        talk()
        sc = t.wait("Mistral says the voice key is wrong. /provider fixes it.", 30)
        assert "Invalid API Key" in sc and "your recording is kept: ctrl+r retry" in sc, sc
        # no credit (the kept clip goes again: no recording)
        os.remove(credit)
        set_key("broke-key-2")
        t.keys("C-r")
        sc = t.wait("your Mistral account has no credit yet.", 30)
        # the provider down
        set_key("down-key-2")
        t.keys("C-r")
        # the line wraps in the feed: its head, then the rest
        sc = t.wait("i couldn't reach Mistral to transcribe. try again, or /voice setup for another", 30)
        assert "the service is overloaded" in sc, sc
        # the key gone
        set_key(None)
        t.keys("C-r")
        t.wait("voice needs a key. /voice setup picks one.", 30)
        # a good key: the kept clip lands in the composer
        set_key("good-key-3")
        t.keys("C-r")
        t.wait("hello from the voice", 30)
    print("ok")


if __name__ == "__main__":
    run(main)
