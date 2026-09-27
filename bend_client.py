#!/usr/bin/env python3
"""bend_client — drive a live bend-harness session programmatically.

The same thing the TUI does, but a library: launch the Bend REPL
(repl-live) on a private port, speak the line protocol, read the obs
stream until the turn ends ("--- idle").

  from bend_client import BendSession
  s = BendSession.fresh(bg_after=3)      # or BendSession.resume(path)
  lines = s.say("hello")                 # blocks until the turn ends
  print(s.last_assistant())
  s.close()

CLI:
  python3 bend_client.py --message "hello"
  python3 bend_client.py --continue --message "go on"
  python3 bend_client.py --message "..." --bg-after 3

The wire is single-line (newlines travel escaped as literal backslash-n
in assistant text); last_assistant() unescapes them for reading.
"""

import argparse
import glob
import os
import socket
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.abspath(__file__))
REPL = os.path.join(REPO, "repl-live")
IDLE = "--- idle"
DEFAULT_TIMEOUT = 900.0


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


class BendSession:
    """One live harness process + one TCP client connection."""

    def __init__(self, port, session_file, log_path, proc=None, sock=None):
        self.port = port
        self.session_file = session_file
        self.log_path = log_path
        self.proc = proc
        self.sock = sock
        self.buf = b""
        self.lines = []

    # ---- constructors ----

    @classmethod
    def _start(cls, port, session_file, bg_after=None, continue_=False):
        log_path = "/tmp/bend-client-%d.log" % port
        env = dict(os.environ)
        env["BEND_REPL_PORT"] = str(port)
        env["BEND_SESSION_FILE"] = session_file
        if bg_after is not None:
            env["BEND_BG_AFTER"] = str(bg_after)
        if continue_:
            env["BEND_CONTINUE"] = "1"
        log = open(log_path, "w")
        proc = subprocess.Popen(
            [REPL], cwd=REPO, env=env, stdout=log, stderr=subprocess.STDOUT
        )
        # wait for the banner (a TCP probe would steal the greet)
        deadline = time.time() + 30
        while time.time() < deadline:
            try:
                with open(log_path) as f:
                    if "REPL on" in f.read():
                        break
            except OSError:
                pass
            if proc.poll() is not None:
                raise RuntimeError("repl died at startup (see %s)" % log_path)
            time.sleep(0.05)
        else:
            proc.kill()
            raise RuntimeError("repl did not start (see %s)" % log_path)
        sock = socket.create_connection(("127.0.0.1", port), timeout=10)
        sock.settimeout(1.0)
        sess = cls(port, session_file, log_path, proc, sock)
        sess._drain(2.0)  # the --continue greeting, if any
        return sess

    @classmethod
    def fresh(cls, bg_after=None):
        port = free_port()
        session_file = "/tmp/bend-sessions/session-%d.txt" % port
        os.makedirs(os.path.dirname(session_file), exist_ok=True)
        if os.path.exists(session_file):
            os.remove(session_file)
        return cls._start(port, session_file, bg_after=bg_after)

    @classmethod
    def resume(cls, session_file, bg_after=None):
        port = free_port()
        return cls._start(port, session_file, bg_after=bg_after, continue_=True)

    # ---- the wire ----

    def _drain(self, seconds):
        """Read whatever arrives within `seconds` (no idle wait)."""
        end = time.time() + seconds
        while time.time() < end:
            try:
                chunk = self.sock.recv(65536)
                if not chunk:
                    raise RuntimeError("repl closed the connection")
                self.buf += chunk
            except socket.timeout:
                continue
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                text = line.decode("utf-8", "replace").strip()
                if text:
                    self.lines.append(text)
        return self.lines

    def _recv_until_idle(self, timeout):
        end = time.time() + timeout
        while time.time() < end:
            try:
                chunk = self.sock.recv(65536)
                if not chunk:
                    raise RuntimeError("repl closed the connection")
                self.buf += chunk
            except socket.timeout:
                continue
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                text = line.decode("utf-8", "replace").strip()
                if text:
                    self.lines.append(text)
                    if text == IDLE:
                        return self.lines
        raise RuntimeError("turn did not end within %.0fs" % timeout)

    def send(self, line, timeout=DEFAULT_TIMEOUT):
        """Send one protocol line; return the obs lines of the turn."""
        self.lines = []
        self.sock.sendall((line + "\n").encode())
        return self._recv_until_idle(timeout)

    def say(self, text, timeout=DEFAULT_TIMEOUT, verbose=False):
        # the socket line is single-line: real newlines escape (the
        # REPL's say path unescapes them into the message text)
        lines = self.send(text.replace("\n", "\\n"), timeout)
        if verbose:
            for line in lines:
                print(line)
        return lines

    def steer(self, text, timeout=DEFAULT_TIMEOUT):
        return self.send("steer " + text, timeout)

    def steer_midturn(self, text):
        """Steer the RUNNING turn through the file side-channel.

        The harness reads the socket only between turns; the runtime
        drains /tmp/bend-steer-<port>.txt at every model/tool safe
        boundary and commits the text into the running turn (ADR 0005).
        Use this while a turn is in flight (say() in another thread).
        """
        path = "/tmp/bend-steer-%d.txt" % self.port
        with open(path, "a") as f:
            f.write(text + "\n")

    def notify(self, text, timeout=DEFAULT_TIMEOUT):
        return self.send("notify " + text, timeout)

    def compact(self, timeout=DEFAULT_TIMEOUT):
        return self.send("compact", timeout)

    # ---- reading the conversation ----

    @staticmethod
    def unescape(text):
        return text.replace("\\n", "\n")

    def assistant_texts(self, lines=None):
        lines = self.lines if lines is None else lines
        prefix = "obs: assistant: "
        out = []
        for line in lines:
            if line.startswith(prefix):
                out.append(self.unescape(line[len(prefix):]))
        return out

    def last_assistant(self, lines=None):
        texts = self.assistant_texts(lines)
        return texts[-1] if texts else ""

    def close(self):
        """Detach: the process dies with us; the session checkpoint stays."""
        try:
            self.sock.sendall(b"quit\n")
        except OSError:
            pass
        try:
            self.sock.close()
        except OSError:
            pass
        if self.proc:
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def main():
    ap = argparse.ArgumentParser(description="drive a bend-harness session")
    ap.add_argument("--message", required=True)
    ap.add_argument("--continue", dest="continue_", action="store_true")
    ap.add_argument("--bg-after", type=int, default=None)
    ap.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    args = ap.parse_args()

    if args.continue_:
        files = sorted(glob.glob("/tmp/bend-sessions/session-*.txt"),
                       key=os.path.getmtime)
        if not files:
            print("no session to continue", file=sys.stderr)
            return 1
        sess = BendSession.resume(files[-1], bg_after=args.bg_after)
        print("# resumed %s" % files[-1])
    else:
        sess = BendSession.fresh(bg_after=args.bg_after)

    try:
        sess.say(args.message, timeout=args.timeout, verbose=True)
        print("\n---- idle. session: %s" % sess.session_file)
    finally:
        sess.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
