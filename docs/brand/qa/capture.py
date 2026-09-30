"""BISE-82 visual QA: reproduce the screens of tui-screens.html in the real
TUI (tmux, a throwaway hub, the scripted fake provider) and capture them
with their colors (tmux capture-pane -e) as .ansi + .html.

python3 -u docs/brand/qa/capture.py dark|light|ascii [onboarding]

Output: docs/brand/qa/shots/<pass>/<nn>-<screen>.{ansi,html}
The scripted provider cannot play model behavior (main's words, a
correction, sending work back): those screens are marked n/a in
visual-qa.md.
"""
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tests"))
sys.path.insert(0, HERE)
import e2e  # noqa: E402
import ansi2html  # noqa: E402

PASS = sys.argv[1] if len(sys.argv) > 1 else "dark"
ONBOARD = len(sys.argv) > 2 and sys.argv[2] == "onboarding"
MODE = "light" if PASS == "light" else "dark"
OUT = os.path.join(HERE, "shots", PASS)
S = "bise-qa-%d" % os.getpid()
LOG = []


def tmux(*a):
    return subprocess.run(["tmux", *a], capture_output=True, text=True).stdout


def screen():
    return tmux("capture-pane", "-p", "-t", S)


def keys(*k):
    tmux("send-keys", "-t", S, *k)


def typed(text):
    tmux("send-keys", "-t", S, "-l", text)


def say(text):
    typed(text)
    time.sleep(0.2)
    keys("Enter")


def wait(needle, timeout=40):
    """Wait for `needle` on screen; never fails (the capture shows)."""
    t0 = time.time()
    while time.time() - t0 < timeout:
        if needle in screen():
            return True
        time.sleep(0.25)
    LOG.append("timeout waiting for %r" % needle)
    print("  (timeout: %r)" % needle)
    return False


N = [0]


def shot(name, note=""):
    time.sleep(0.6)
    N[0] += 1
    base = os.path.join(OUT, "%02d-%s" % (N[0], name))
    raw = tmux("capture-pane", "-e", "-p", "-t", S)
    open(base + ".ansi", "w").write(raw)
    open(base + ".html", "w").write(ansi2html.page(raw, MODE, "%s (%s)" % (name, PASS)))
    LOG.append("%02d %s %s" % (N[0], name, note))
    print("shot", N[0], name)


def start(E, cols, rows, extra=""):
    # never the real home: the TUI saves its theme, the onboarding flag
    # and hints (prefs), crash reports in bise's home; every capture gets
    # its own temp HOME and bise home (BISE_HOME)
    home = os.path.join(E.tmp, "home")
    state_root = os.path.join(E.tmp, "state-root")
    os.makedirs(home, exist_ok=True)
    os.makedirs(state_root, exist_ok=True)
    env = "HOME=%s BISE_HOME=%s BISE_THEME=%s COLORTERM=truecolor%s %s" % (
        home, state_root, MODE, " BISE_ASCII=1" if PASS == "ascii" else "", extra)
    envs = " ".join("%s=%s" % (k, subprocess.list2cmdline([v])) for k, v in E.env.items()
                    if k.startswith(("SB_", "BEND_", "MISTRAL_")))
    cmd = "cd %s && env %s %s %s switchboard --workspace %s; sleep 600" % (ROOT, env, envs, e2e.EXE, E.ws)
    tmux("new-session", "-d", "-s", S, "-x", str(cols), "-y", str(rows), cmd)


def new_env():
    E = e2e.Env()
    E.env["SB_CORE_BIN"] = os.path.join(ROOT, "sb-core")
    return E


def stop(E):
    """The TUI started the hub itself: stop it (it stops its REPLs), then
    the tmux session, then the fake provider and the temp dirs."""
    try:
        e2e.Client(os.path.join(E.state, "hub.sock")).send({"op": "stop_hub"})
        time.sleep(2)
    except Exception as e:  # noqa: BLE001
        print("  (stop_hub: %s)" % e)
    tmux("kill-session", "-t", S)
    E.close()


def onboarding():
    E = new_env()
    E.env.pop("SB_ONBOARDING", None)
    E.env["SB_ONBOARDING"] = "on"
    try:
        start(E, 112, 36)
        time.sleep(4)
        shot("onboarding-1-welcome")
        time.sleep(4)
        shot("onboarding-2-typed")
        for i in range(3, 9):
            keys("Enter")
            time.sleep(2.5)
            shot("onboarding-%d" % i)
    finally:
        stop(E)


def main():
    os.makedirs(OUT, exist_ok=True)
    if ONBOARD:
        onboarding()
        open(os.path.join(OUT, "log-onboarding.txt"), "w").write("\n".join(LOG) + "\n")
        return
    E = new_env()
    open(os.path.join(E.ws, "shot.png"), "wb").write(bytes.fromhex(
        "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
        "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082"))
    try:
        start(E, 150, 42)
        wait("bise :*", 30)
        time.sleep(3)
        shot("first-run", "empty main feed")
        # --- you ask, main starts agents
        say('[[bash: sb spawn docs --objective "document the api {{bash: sb report blocked \'v1 or v2 for the api docs? the brief says keep old clients, but v2 removes /users.\'}}"]]'
            ' [[bash: sb spawn bench --objective "benchmark the api {{bash: sb report done \'p95 at 180 ms, nothing to fix.\'}}"]]'
            ' [[bash: sb spawn deploy --objective "deploy staging {{bash: sb report failed \'the staging token expired.\'}}"]]')
        wait("new agent @deploy", 60)
        time.sleep(8)
        shot("you-ask-main-starts-agents")
        say('[[bash: sb spawn auth-fix --objective "fix the cookie {{bash: sleep 900}}"]]'
            ' [[bash: sb spawn talk-a --objective "{{bash: sb send talk-b \'can you check logout?\' && sb send main \'logout is fine on my side\' && sb send talk-b \'and the webkit test?\' && sb send main \'webkit is green\' && sb send talk-b \'thanks\'}}"]]'
            ' [[bash: sb spawn talk-b --objective "{{bash: sleep 3 && sb send talk-a \'checked, logout works\' && sb send talk-a \'webkit is green too\'}}"]]')
        wait("new agent @talk-b", 60)
        time.sleep(15)
        shot("levels-and-reports", "what's for you, what isn't + reports in main + busy hour (fold)")
        # --- a question card (main escalates a question of an agent)
        say('[[bash: sb card "the brief says keep old clients working, but v2 removes /users. do we document v1 or v2?\n1. v1\n2. v2"]]')
        time.sleep(6)
        keys("C-g")
        time.sleep(1)
        shot("cards-a-question", "ctrl+g")
        for i in range(1, 6):
            keys("C-n")
            shot("cards-every-kind-%d" % i, "ctrl+n")
        keys("C-f")
        shot("card-full-screen", "ctrl+f")
        keys("C-f")
        keys("C-g")
        # --- the panel, preview, inside an agent
        keys("C-k")
        keys("C-k")
        shot("agents-panel-selection", "ctrl+k twice")
        keys("Space")
        shot("preview-an-agent", "space")
        keys("Escape")
        keys("C-k")
        keys("C-k")
        keys("Enter")
        time.sleep(2)
        shot("inside-an-agent", "enter on the selection")
        keys("C-o")
        shot("everything-disclosed", "ctrl+o")
        keys("C-o")
        keys("Escape")
        time.sleep(1)
        # --- markdown tables (main's reply is the tool output)
        say("[[bash: printf '\\n| agent | status | p95 |\\n|:---|:---:|---:|\\n| bench | done | 180 ms |\\n| auth-fix | working | — |\\n| docs | needs you | — |\\n']]")
        wait("needs you |", 5)
        time.sleep(4)
        shot("markdown-tables")
        # --- a failing tool
        say("[[bash: ls /nonexistent-dir-for-qa]]")
        time.sleep(5)
        shot("a-failing-tool")
        # --- a turn in progress, steer, marks
        say("[[bash: sleep 10 && echo slept]]")
        time.sleep(2)
        say("also check the logs")
        time.sleep(1)
        shot("turn-in-progress-steer", "sending / received marks")
        time.sleep(12)
        shot("message-marks-read", "after the turn")
        # --- talk to an agent from main
        say("@bench can you rerun the p95?")
        time.sleep(6)
        shot("talk-to-an-agent-from-main")
        # --- images: a dropped path becomes a chip
        typed("look at this ")
        tmux("set-buffer", "-b", "qa", os.path.join(E.ws, "shot.png"))
        tmux("paste-buffer", "-p", "-b", "qa", "-t", S)  # a dropped file: a bracketed paste
        typed(" ")
        time.sleep(1.5)
        shot("images-attaching")
        keys("Enter")
        time.sleep(5)
        shot("images-in-the-history")
        # --- slash commands, help, versions
        typed("/")
        time.sleep(1)
        shot("slash-commands")
        keys("Escape")
        keys("C-u")
        say("/help")
        time.sleep(1.5)
        shot("help")
        keys("Escape")
        say("/version")
        time.sleep(2)
        shot("versions")
        keys("Escape")
        keys("C-u")
        # --- the terminal panel
        keys("C-Space")
        time.sleep(2)
        typed("echo hello from the panel")
        keys("Enter")
        time.sleep(1)
        shot("terminal-panel", "ctrl+` (NUL)")
        keys("C-Space")
        time.sleep(1)
        # --- drop an agent
        keys("C-k")
        keys("C-k")
        keys("D")
        shot("drop-an-agent", "D on a selection")
        keys("n")
        keys("Escape")
        # --- compaction
        say("/compact")
        time.sleep(1.5)
        shot("compaction-running")
        time.sleep(8)
        shot("compaction-done")
        # --- a narrow terminal (and tables in it)
        tmux("resize-window", "-t", S, "-x", "64", "-y", "40")
        time.sleep(1.5)
        shot("narrow-terminal", "64 columns")
        tmux("resize-window", "-t", S, "-x", "150", "-y", "42")
        time.sleep(1)
        # --- provider errors: the fake provider goes away
        E.fake.kill()
        say("are you there?")
        time.sleep(12)
        shot("provider-error", "fake provider killed")
    finally:
        open(os.path.join(OUT, "log.txt"), "w").write("\n".join(LOG) + "\n")
        stop(E)


if __name__ == "__main__":
    main()
