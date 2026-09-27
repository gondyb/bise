#!/usr/bin/env python3
"""Parity test: the user's path (./run.sh -> TUI in a pty) and the
programmatic path (bend_client -> ./run.sh --headless) must run the
SAME thing. For one config, every surface must agree:

  U: the TUI status-bar label, the TUI /status line, the model in the
     real HTTP body the user's session sent (BEND_WIRE_DUMP)
  C: the model bend_client read from READY, the model in the real
     HTTP body the client's session sent
  both: the process tree (the same run.sh-built binaries)

Usage: python3 -u test-parity.py [repo] [config-path-or-empty] [expected-model]
  python3 -u test-parity.py                                   # the real config
  python3 -u test-parity.py "" /tmp/glm.toml zai-glm-5-3      # a non-default model

Each run makes two real model calls. Test sessions go to
/tmp/parity-sessions (never the user's --continue).
"""
import fcntl, json, os, pty, re, select, struct, subprocess, sys, termios, time

REPO = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else os.path.dirname(os.path.abspath(__file__))
CFG = sys.argv[2] if len(sys.argv) > 2 else ""
EXPECT = sys.argv[3] if len(sys.argv) > 3 else "claude-opus-5-5"
REPO = os.path.realpath(REPO)  # ps shows real paths (/private/tmp)
sys.path.insert(0, REPO)
from bend_client import BendSession  # noqa: E402

def base_env(tag):
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    # sessions are state, not logic: keep the test out of the user's --continue
    env["BEND_SESSIONS_DIR"] = "/tmp/parity-sessions"
    env["BEND_WIRE_DUMP"] = "/tmp/parity-wire-%s.json" % tag
    if CFG:
        env["BEND_CONFIG"] = CFG
    else:
        env.pop("BEND_CONFIG", None)
    env.pop("BEND_MODEL", None)
    return env

def strip(b):
    t = b.decode("utf-8", "replace")
    t = re.sub(r"\x1b\[[0-9;?<>]*[a-zA-Z~]", "", t)
    return re.sub(r"\x1b\][^\x07]*\x07", "", t)

def wire_model(tag):
    with open("/tmp/parity-wire-%s.json" % tag) as f:
        return json.load(f).get("model")

def tree(pid):
    """The exact process tree of one session: the bend-harness parent
    (pid) and its REPL child - by the parent->child relation, not by
    name (other harness sessions may run on the machine)."""
    def exe(p):
        out = subprocess.run(["ps", "-o", "command=", "-p", str(p)],
                             capture_output=True, text=True).stdout.strip()
        first = out.split()[0] if out else ""
        return os.path.realpath(os.path.join(REPO, first)) if first.startswith("./") else first
    kids = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout.split()
    return [exe(pid)] + sorted(exe(k) for k in kids)

for tag in ("U", "C"):
    try:
        os.remove("/tmp/parity-wire-%s.json" % tag)
    except FileNotFoundError:
        pass

# ---- U: the user's path, literally ./run.sh in a terminal ----
m, s = pty.openpty()
fcntl.ioctl(s, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))
p = subprocess.Popen([os.path.join(REPO, "run.sh")], stdin=s, stdout=s, stderr=s,
                     cwd=REPO, env=base_env("U"), close_fds=True)
os.close(s)
buf = b""
def pump(sec, until=None):
    global buf
    end = time.time() + sec
    while time.time() < end:
        r, _, _ = select.select([m], [], [], 0.2)
        if r:
            try:
                buf += os.read(m, 65536)
            except OSError:
                return
        if until and until in strip(buf):
            return
pump(60, until="bend-harness ·")          # the TUI is up
procs_u = tree(p.pid)
os.write(m, b"What is 1234 + 4321? Reply with the number only.")
pump(0.5)
os.write(m, b"\r")
pump(240, until="5555")                  # the reply (not in the prompt)
pump(3)
os.write(m, b"/status")
pump(0.5)
os.write(m, b"\r")
pump(5, until="seuil de compaction")
screen_u = strip(buf)
os.write(m, b"\x03")                      # idle Ctrl+C quits
deadline = time.time() + 10               # the parent's reload grace is 3s
while p.poll() is None and time.time() < deadline:
    pump(0.5)
if p.poll() is None:
    p.kill()

label_u = re.search(r"bend-harness · (\S+)", screen_u)
status_u = re.search(r"modèle (\S+) · 127\.0\.0\.1:\d+ · seuil de compaction (\d+)", screen_u)

# ---- C: the programmatic path, bend_client ----
old_env = dict(os.environ)
env_c = base_env("C")          # computed BEFORE the swap
os.environ.clear()
os.environ.update(env_c)
c = BendSession.fresh()
procs_c = tree(c.proc.pid)
c.say("What is 2000 + 3333? Reply with the number only.", timeout=240)
reply_c = c.last_assistant()
c.close()
os.environ.clear()
os.environ.update(old_env)

# ---- the verdict ----
facts = {
    "U status-bar label": label_u.group(1) if label_u else None,
    "U /status model": status_u.group(1) if status_u else None,
    "U wire model (HTTP body)": wire_model("U"),
    "C READY model": c.model,
    "C wire model (HTTP body)": wire_model("C"),
}
print("config:", CFG or "(the user's real ~/.bend-harness/config.toml)")
for k, v in facts.items():
    print("  %-28s %s" % (k, v))
print("  %-28s %s" % ("U /status threshold", status_u.group(2) if status_u else None))
print("  %-28s %s" % ("C READY threshold", c.threshold))
print("  %-28s %s" % ("U replied 5555", "5555" in screen_u))
print("  %-28s %s" % ("C replied 5333", "5333" in (reply_c or "")))
print("  U processes:", procs_u)
print("  C processes:", procs_c)
ok = (all(v == EXPECT for v in facts.values())
      and status_u and status_u.group(2) == c.threshold
      and "5555" in screen_u and "5333" in (reply_c or "")
      and procs_u and procs_u == procs_c)
time.sleep(1)
left = subprocess.run(["pgrep", "-f", os.path.join(REPO, "repl-live")],
                      capture_output=True, text=True).stdout.split()
print("  %-28s %s" % ("REPL processes left", left))
for pid in left:
    print("    ", subprocess.run(["ps", "-o", "pid=,ppid=,lstart=,command=", "-p", pid],
                                capture_output=True, text=True).stdout.strip())
ok = ok and not left
print("PARITY", "OK" if ok else "FAILED", "(expected model %s)" % EXPECT)
