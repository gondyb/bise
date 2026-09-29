#!/usr/bin/env python3
"""run_typescript in a scripted session runs on V8 (bend-jsrt), like a
live one (BISE-118: one engine; the Core's JS-lite interpreter is gone).

Drives `bend-harness --headless --scripted` over its wire socket: each `prog: <code>` line
makes the scripted model call run_typescript with that code. The session
checkpoint then holds each program's one tool result, what the model
reads back: a value, a typed program that calls a tool (the call goes
out through the runtime and its result comes back into the re-run), a
failed inner call, a throw.
"""
import glob, os, socket, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
EXE = os.path.join(os.path.abspath(os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "rust", "target")),
                   "debug", "bend-harness")

def jsrt_env():
    """The harness finds this tree's rust/jsrt build; a fresh worktree has
    none: take the main tree's (the engine rarely changes)."""
    if os.path.exists(os.path.join(ROOT, "rust/jsrt/target/debug/bend-jsrt")):
        return {}
    wl = subprocess.run(["git", "worktree", "list", "--porcelain"], cwd=ROOT,
                        capture_output=True, text=True).stdout
    main = wl.splitlines()[0].split(" ", 1)[1] if wl else ROOT
    for rel in ("rust/jsrt/target/debug/bend-jsrt", "rust/jsrt/target/release/bend-jsrt", "bend-jsrt"):
        p = os.path.join(main, rel)
        if os.path.exists(p):
            return {"BEND_JSRT_BIN": p}
    sys.exit("FAIL no bend-jsrt: build it with ./run.sh (or cd rust/jsrt && cargo build)")

SESSION_VARS = ("BEND_SESSION_FILE", "BEND_CONTEXT_FILE", "BEND_WIRE_LOG", "BEND_REPL_PORT",
                "BEND_DEBUG_DIR", "SB_SOCKET", "SB_AGENT", "SB_TASK", "SB_CORE_BIN")

PROGRAMS = [
    ("return 6 * 7", "tool run_typescript ok: 42"),
    ('const r: string = await search_tool_functions({query: "bash"}); return typeof r',
     "tool run_typescript ok: string"),
    ("return await no_such_tool({})",
     "tool run_typescript failed: program call failed: unknown tool: no_such_tool"),
    ('throw new Error("boom")', "tool run_typescript failed: boom"),
]

def run_session(env, codes):
    """`bend-harness --headless --scripted`: READY, then one `prog:` turn
    per program over the wire socket (each ends at `--- idle`)."""
    proc = subprocess.Popen([EXE, "--headless", "--scripted"], cwd=ROOT, env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True)
    try:
        ready = proc.stdout.readline()
        if not ready.startswith("READY "):
            sys.exit("FAIL no READY: %r\n%s" % (ready, proc.stderr.read()[-2000:] if proc.poll() is not None else ""))
        port = int(dict(kv.split("=", 1) for kv in ready.split()[1:] if "=" in kv)["port"])
        sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        f = sock.makefile("rb")
        for code in codes:
            sock.sendall(("prog: %s\n" % code).encode())
            while True:
                line = f.readline()
                if not line:
                    sys.exit("FAIL the REPL closed the connection")
                if line.strip() == b"--- idle":
                    break
        sock.close()
    finally:
        proc.stdin.close()  # the hang-up the harness watches
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()

def main():
    home = tempfile.mkdtemp(prefix="sb-scripted-ts-")
    env = dict(os.environ, HOME=home, BISE_HOME=os.path.join(home, "bise"),
               BEND_SESSIONS_DIR=os.path.join(home, "sessions"), **jsrt_env())
    # run from an agent's shell, the env names that agent's live session
    # (its context, wire log, steer/interrupt files, hub): never touch it
    for k in SESSION_VARS:
        env.pop(k, None)
    run_session(env, [code for code, _ in PROGRAMS])
    sessions = glob.glob(os.path.join(home, "sessions", "*.txt"))
    if len(sessions) != 1:
        sys.exit("FAIL %d session files" % len(sessions))
    results = [l.split(" : ", 1)[1] for l in open(sessions[0]).read().splitlines()
               if l.startswith("MSG False tool : ")]
    bad = 0
    for (code, want), got in zip(PROGRAMS, results + [""] * len(PROGRAMS)):
        ok = got.startswith(want)
        bad += not ok
        print("%s %s -> %s" % ("ok  " if ok else "FAIL", code, got[:120]))
    if bad or len(results) != len(PROGRAMS):
        sys.exit("FAIL %d of %d programs (%d results)" % (bad, len(PROGRAMS), len(results)))
    print("scripted run_typescript: %d programs on V8 (bend-jsrt)" % len(PROGRAMS))

if __name__ == "__main__":
    main()
