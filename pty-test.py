#!/usr/bin/env python3
"""Drive the TUI under a pty through the REAL entry point (the
bend-harness parent, scripted mode), capture output, check submit."""
import os, pty, select, subprocess, sys, time

KEYS = sys.argv[1] if len(sys.argv) > 1 else "court test"
PORT = sys.argv[2] if len(sys.argv) > 2 else "7700"
# the parent spawns repl-scripted itself: the same path ./run.sh takes

master, slave = pty.openpty()
# set a terminal size on the slave
import fcntl, termios, struct
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 90, 0, 0))

p = subprocess.Popen(
    ["./rust/target/debug/bend-harness", "--scripted", "--port", PORT],
    stdin=slave, stdout=slave, stderr=slave,
    cwd="/Users/gabrielvergnaud/lab/bend-lab/harness",
    close_fds=True,
)
os.close(slave)

out = b""
def drain(t):
    global out
    end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([master], [], [], 0.1)
        if r:
            try:
                out += os.read(master, 65536)
            except OSError:
                break

drain(3.0)                       # the parent starts the REPL first
os.write(master, KEYS.encode())
drain(0.3)
os.write(master, b"\r")          # Enter
drain(3.0)                       # wait for the reply
os.write(master, b"\x03")        # ctrl+c (empty input -> quit)
drain(1.5)

if p.poll() is None:
    print("STILL RUNNING, killing")
    p.kill()
else:
    print("exited rc=%s" % p.returncode)

text = out.decode("utf-8", "replace")
# strip ANSI for grepping
import re
plain = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", text)
plain = re.sub(r"\x1b\][^\x07]*\x07", "", plain)
print("has 'You said':", "You said" in plain)
print("has submitted echo '> ':", KEYS[:6] in plain)
sys.stdout.write(plain[-400:])
