#!/usr/bin/env python3
"""The skills index at the live REPL's start, and the skill tool's errors.

A real repl-live on the scripted fake provider (fake_provider.py), with a
temp HOME holding one skill:
1. the startup scan writes the shared index ($BEND_SKILLS_INDEX) and the
   session's one by a rename: no temp file is left, and a reader never
   sees a truncated index (every REPL start rescans it);
2. the skill tool loads a skill of the index;
3. a name the index lacks says "unknown skill", not "skills index
   unreadable" (an empty index said that in main's session, and the
   search went to the index's path instead of its content);
4. the session's index goes to $BEND_RUN_DIR/<port> (qa-explore J: it
   went to ~/.bend-harness/run/<port> whatever BEND_RUN_DIR said).
"""
import os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, HERE)
from repl_bash_env import clean_env, free_port, tool_results  # noqa: E402

SKILL = """---
name: alpha
description: The alpha test skill.
---
Alpha body: say ALPHA-OK.
"""


def main():
    tmp = tempfile.mkdtemp(prefix="sb-skills-scan-")
    os.makedirs(os.path.join(tmp, ".agents", "skills", "alpha"))
    open(os.path.join(tmp, ".agents", "skills", "alpha", "SKILL.md"), "w").write(SKILL)
    cache = os.path.join(tmp, "cache")
    index = os.path.join(cache, "skills-index.txt")
    env = clean_env(tmp)
    fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")],
                            stdout=subprocess.PIPE, text=True,
                            env={**env, "FAKE_LOG": os.path.join(tmp, "fake.log")})
    port = free_port()
    session = os.path.join(tmp, "session.txt")
    ws = os.path.join(tmp, "ws")
    os.makedirs(ws)
    env.update({
        "BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % fake.stdout.readline().split()[1],
        "BEND_MODEL": "mistral-small-latest", "MISTRAL_API_KEY": "fake-key",
        "BEND_MCP_INDEX": os.path.join(tmp, "mcp.txt"), "BEND_SKILLS_INDEX": index,
        "BEND_PLUGINS_STATE": os.path.join(tmp, "plugins.json"),
        "BEND_PLUGINS_DATA": os.path.join(tmp, "plugin-data"),
        "BEND_CONFIG": os.path.join(tmp, "config.toml"),
        "BEND_MCP_BOOTSTRAP_URL": "http://127.0.0.1:9/none",
        "BEND_REPL_PORT": str(port), "BEND_WORKDIR": ws, "BEND_RUN_DIR": os.path.join(tmp, "run"),
        "BEND_SESSION_FILE": session, "BEND_WIRE_LOG": os.path.join(tmp, "wire.log"),
    })
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                            stdout=open(log, "w"), stderr=open(err, "w"))
    fails = []

    def check(name, ok, detail=""):
        print("%s %s" % ("ok  " if ok else "FAIL", name))
        if not ok:
            fails.append("%s %s" % (name, detail))

    try:
        t0 = time.time()
        while "REPL on" not in open(log).read():
            if repl.poll() is not None or time.time() - t0 > 60:
                sys.exit("FAIL no REPL banner: %s" % open(err).read()[-500:])
            time.sleep(0.1)
        while not os.path.exists(index) and time.time() - t0 < 30:
            time.sleep(0.1)  # the scan may end after the banner
        idx = open(index).read() if os.path.exists(index) else ""
        check("the scan writes the shared index (its folder created)",
              idx.startswith("alpha\tThe alpha test skill.\t/"), repr(idx))
        check("no temp file is left next to it", os.path.isdir(cache) and os.listdir(cache) == ["skills-index.txt"],
              repr(os.listdir(cache)))
        sidx = os.path.join(tmp, "run", str(port), "skills-index.txt")
        while not os.path.exists(sidx) and time.time() - t0 < 30:
            time.sleep(0.1)  # the scan writes it after the shared index
        check("the session's index is under $BEND_RUN_DIR/<port>",
              os.path.exists(sidx) and not os.path.exists(os.path.join(tmp, ".bend-harness")),
              repr(os.listdir(tmp)))
        sock = socket.create_connection(("127.0.0.1", port), timeout=120)
        sock.sendall(b"run [[skill: alpha]] [[skill: nope]]\n")
        f = sock.makefile("rb")
        while True:
            line = f.readline()
            if not line:
                sys.exit("FAIL the REPL closed the connection: %s" % open(err).read()[-300:])
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        res = tool_results(session)
        check("a skill of the index loads", len(res) == 2 and "Alpha body: say ALPHA-OK." in res[0]
              and "name: alpha" not in res[0], repr(res))
        check("a name the index lacks is an unknown skill, not an unreadable index",
              len(res) == 2 and "unknown skill: nope" in res[1] and "unreadable" not in res[1], repr(res))
    finally:
        repl.kill()
        fake.kill()
    if fails:
        sys.exit("FAIL %s" % fails)
    print("PASS skills_scan")


if __name__ == "__main__":
    main()
