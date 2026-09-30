#!/usr/bin/env python3
"""versions.sh prune (BISE-133) in the bise layout, in a temp HOME.

bug-restart: the first `sb restart` to 51c081a built the version, then
prune exited 1 (pipefail: marks' loop ended on a missing
$STATE/*/hub.root, always in the bise layout), so `versions.sh build`
exited 1 after "built" and the hub did not switch; the second restart
found the version built and switched without a build.

Checks: prune exits 0 in the bise layout; it keeps the 3 newest, the one
a hub marks and the one a process runs from (with >64 KB of ps output
after its line: `printf | grep -q` got SIGPIPE there and removed it);
it removes the others.

python3 -u tests/versions_prune.py
"""
import os, subprocess, sys, tempfile, time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SCRIPT = os.environ.get("VERSIONS_SH", os.path.join(ROOT, "scripts", "versions.sh"))
PRIVATE = ("BISE_HOME", "SB_VERSIONS_DIR", "SB_BUILD_DIR", "SB_KEEP_VERSIONS", "XDG_STATE_HOME",
           "BISE_EXPORTS_FOR", "BEND_SESSION_FILE", "BEND_CONTEXT_FILE", "BEND_WIRE_LOG",
           "BEND_REPL_PORT", "BEND_CONFIG", "BEND_SESSIONS_DIR", "BEND_IMAGE_DIR", "BEND_MCP_INDEX",
           "BEND_SKILLS_INDEX", "BEND_PLUGINS_STATE", "BEND_PLUGINS_DATA", "BEND_RUN_DIR")


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        sys.exit("FAIL " + what)


def main():
    tmp = tempfile.mkdtemp(prefix="sbvp-", dir="/tmp")
    home = os.path.join(tmp, "home")
    bise = os.path.join(home, ".bise")
    versions = os.path.join(bise, "dev", "versions")
    os.makedirs(os.path.join(bise, "hubs", "h1"))
    os.makedirs(os.path.join(bise, "dev", "build"))
    with open(os.path.join(bise, "migrated.json"), "w") as f:
        f.write("{}\n")
    # v0 oldest ... v6 newest
    for i in range(7):
        d = os.path.join(versions, "v%d" % i)
        os.makedirs(d)
        with open(os.path.join(d, "VERSION"), "w") as f:
            f.write("id=v%d\nbuilt=2026-09-29T10:0%d:00Z\n" % (i, i))
    with open(os.path.join(bise, "hubs", "h1", "versions.json"), "w") as f:
        f.write('{"current":"%s/v1","good":"%s/v1"}' % (versions, versions))
    env = {k: v for k, v in os.environ.items() if k not in PRIVATE}
    env["HOME"] = home
    # v2 in use: a process with its path on the command line, then 200 KB
    # of command lines after it in ps's output
    procs = [subprocess.Popen(["/bin/sh", "-c", "sleep 60; :", os.path.join(versions, "v2", "bise")],
                              start_new_session=True)]
    for _ in range(2):
        procs.append(subprocess.Popen(["/bin/sh", "-c", "sleep 60; :", "x" * 100_000],
                                      start_new_session=True))
    t0 = time.time()
    try:
        r = subprocess.run([SCRIPT, "prune"], env=env, capture_output=True, text=True, timeout=60)
    finally:
        for p in procs:  # the shell and its sleep
            os.killpg(p.pid, 9)
            p.wait()
    print(r.stderr.strip())
    print("prune: %.1f s" % (time.time() - t0))
    check(r.returncode == 0, "prune exits 0 in the bise layout (got %d)" % r.returncode)
    left = sorted(os.listdir(versions))
    check(left == ["v1", "v2", "v4", "v5", "v6"],
          "kept the 3 newest, the marked v1, the in-use v2 (left: %s)" % left)
    subprocess.run(["rm", "-rf", tmp])
    print("versions_prune: ok")


if __name__ == "__main__":
    main()
