"""End-to-end tests of Switchboard: the real hub (bend-harness sbd), real
Bend REPLs (repl-live), the real `sb` CLI through the agents' bash tool,
real git worktrees, a scripted provider (fake_provider.py).

Run from anywhere:  python3 -u projects/switchboard/tests/e2e.py [name...]
"""
import json
import os
import queue
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
EXE = os.path.join(ROOT, "rust", "target", "debug", "bend-harness")


class Env:
    def __init__(self):
        self.tmp = tempfile.mkdtemp(prefix="sb-e2e-")
        self.ws = os.path.join(self.tmp, "ws")
        self.state = os.path.join(self.tmp, "st")
        os.makedirs(self.ws)
        sh(self.ws, "git init -q && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo base > README && git add README && git commit -qm init")
        self.fake_log = os.path.join(self.tmp, "fake.log")
        self.fake = subprocess.Popen(
            [sys.executable, "-u", os.path.join(HERE, "fake_provider.py")],
            stdout=subprocess.PIPE, text=True, env={**os.environ, "FAKE_LOG": self.fake_log})
        port = self.fake.stdout.readline().split()[1]
        self.env = {
            **os.environ,
            "SB_STATE_DIR": self.state,
            "BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % port,
            "BEND_MODEL": "mistral-small-latest",
            "MISTRAL_API_KEY": "fake-key",
            "BEND_MCP_INDEX": os.path.join(self.tmp, "mcp-index.txt"),
            "BEND_SKILLS_INDEX": os.path.join(self.tmp, "skills-index.txt"),
            "BEND_BG_ROOT": os.path.join(self.tmp, "bg"),
            "BEND_BG_AFTER": "30",
        }
        self.hub = None

    def start_hub(self):
        err = open(os.path.join(self.tmp, "hub.stderr"), "a")
        self.hub = subprocess.Popen([EXE, "sbd", "--workspace", self.ws], cwd=ROOT, env=self.env,
                                    stdin=subprocess.DEVNULL, stdout=err, stderr=err)
        sock = os.path.join(self.state, "hub.sock")
        t0 = time.time()
        while not os.path.exists(sock):
            if time.time() - t0 > 20:
                raise RuntimeError("hub did not start")
            time.sleep(0.05)
        return Client(sock)

    def stop_hub(self):
        c = Client(os.path.join(self.state, "hub.sock"))
        c.send({"op": "stop_hub"})
        self.hub.wait(timeout=20)
        self.hub = None

    def close(self):
        if self.hub:
            try:
                self.stop_hub()
            except Exception:
                self.hub.kill()
        self.fake.kill()
        if os.environ.get("SB_KEEP") != "1":
            shutil.rmtree(self.tmp, ignore_errors=True)
        else:
            print("kept", self.tmp)

    def fake_requests(self):
        try:
            return [json.loads(l) for l in open(self.fake_log)]
        except FileNotFoundError:
            return []


def sh(cwd, script):
    subprocess.run(["/bin/sh", "-c", script], cwd=cwd, check=True)


def out(cwd, script):
    return subprocess.run(["/bin/sh", "-c", script], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


class Client:
    def __init__(self, sock_path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(sock_path)
        self.q = queue.Queue()
        self.events = []
        self.state = None
        self.lock = threading.Lock()
        self.s.sendall(b'{"op":"hello"}\n')
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        f = self.s.makefile("r")
        for line in f:
            try:
                v = json.loads(line)
            except ValueError:
                continue
            with self.lock:
                self.events.append(v)
                if v.get("ev") == "state":
                    self.state = v

    def send(self, v):
        self.s.sendall((json.dumps(v) + "\n").encode())

    def say(self, text, focus="main"):
        self.send({"op": "input", "focus": focus, "text": text})

    def lines(self, agent=None):
        with self.lock:
            return [e["line"] for e in self.events if e.get("ev") == "line" and (agent is None or e["agent"] == agent)]

    def notices(self):
        with self.lock:
            return [e for e in self.events if e.get("ev") in ("notice", "confirm")]

    def agent(self, name):
        with self.lock:
            st = self.state or {}
        for a in st.get("agents", []):
            if a["name"] == name:
                return a
        return None

    def cards(self):
        with self.lock:
            return list((self.state or {}).get("cards", []))

    def wait(self, pred, timeout=90, what="condition"):
        t0 = time.time()
        while time.time() - t0 < timeout:
            try:
                if pred():
                    return
            except Exception:
                pass
            time.sleep(0.1)
        raise AssertionError("timeout waiting for %s" % what)

    def wait_line(self, agent, needle, timeout=90):
        self.wait(lambda: any(needle in l for l in self.lines(agent)), timeout, "%r in %s" % (needle, agent))

    def wait_status(self, name, statuses, timeout=90):
        if isinstance(statuses, str):
            statuses = [statuses]
        self.wait(lambda: self.agent(name) and self.agent(name)["status"] in statuses, timeout,
                  "%s in %s (now %s)" % (name, statuses, (self.agent(name) or {}).get("status")))

    def wait_idle(self, *names, timeout=120):
        for n in names:
            self.wait_status(n, ["idle", "done", "blocked"], timeout)


def check(cond, msg):
    if not cond:
        raise AssertionError(msg)


# ---- scenarios ----

def t_spawn_and_auto_reply(E, c):
    c.wait_status("main", "idle", 60)
    c.say('crée une tâche [[bash: sb spawn t1 --objective "écris le fichier {{bash: echo hello-from-t1 > t1.txt && echo wrote}}"]]')
    c.wait(lambda: c.agent("t1") is not None, 60, "t1 exists")
    c.wait_line("main", "nouvelle tâche @t1")
    c.wait(lambda: os.path.exists(os.path.join(E.ws, "t1.txt")), 90, "t1.txt written in the workspace")
    check(open(os.path.join(E.ws, "t1.txt")).read().strip() == "hello-from-t1", "t1.txt content")
    # t1's turn ends: its reply comes back to main automatically
    c.wait_line("main", "sb msg-in : t1 m_", 90)
    c.wait_idle("main", "t1")
    reqs = [r for r in E.fake_requests() if r["agent"] == "main"]
    check(any('auto="true"' in r["user"] and "from=\"t1\"" in r["user"] for r in reqs),
          "main saw t1's automatic reply")
    # the board reaches main's model calls
    board = open(os.path.join(E.state, "agents", "main", "context.txt")).read()
    check("t1" in board and "<task_board>" in board, "main's context has the board: " + board)
    check(any("<switchboard_state>" in r["last_user"] for r in reqs), "the board is injected as the last message")


def t_direct_message_and_note(E, c):
    c.wait_idle("main", "t1")
    c.send({"op": "focus", "focus": "t1"})
    c.say("parle-moi directement", focus="t1")
    c.wait_line("t1", "sb you : parle-moi directement")
    c.wait_line("t1", "ack: parle-moi directement")
    c.wait_idle("t1")
    c.send({"op": "focus", "focus": "main"})
    c.wait_line("main", "sb direct : Tu as parlé à @t1 (1 message)")
    c.say("et alors ?")
    c.wait(lambda: any(r["agent"] == "main" and r["user"].endswith("et alors ?") for r in E.fake_requests()), 60,
           "main's request with the new message")
    last = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("et alors ?")][0]["user"]
    check(last.startswith("<switchboard_notes>") and "parle-moi directement" in last and "ack: parle-moi" in last,
          "main got the direct-exchange note with the next message: " + last)
    # explicit route, no main turn
    c.wait_idle("main")
    before = len([r for r in E.fake_requests() if r["agent"] == "main"])
    c.say("@t1 route explicite")
    c.wait_line("main", "sb route : toi → @t1 : route explicite")
    c.wait_line("t1", "ack: route explicite")
    c.wait_idle("t1")
    check(len([r for r in E.fake_requests() if r["agent"] == "main"]) == before, "an explicit route costs no main turn")


def t_ask_and_wait(E, c):
    c.wait_idle("main")
    c.say('/new t2: {{bash: sb ask main "quelle version ?"}}')
    c.wait(lambda: c.agent("t2") is not None, 60, "t2 exists")
    # main answers in its turn; the end of its turn is the automatic reply
    c.wait_line("t2", "reply from main", 120)
    c.wait_idle("t2", "main")
    tr = open(os.path.join(E.state, "agents", "t2", "transcript.log")).read()
    check("reply from main" in tr and "quelle version" in tr, "t2's wait returned main's answer")


def t_escalation_card(E, c):
    c.wait_idle("main")
    # t3 asks main; main escalates to the user with a card; the user answers
    c.say('/new t3: {{bash: sb send main --expect-reply "v1 ou v2 ?"}}')
    c.wait(lambda: c.agent("t3") is not None, 60, "t3")
    c.wait_line("main", "sb msg-in : t3", 90)
    c.wait_idle("main", "t3")
    msg_id = None
    for l in c.lines("main"):
        if l.startswith("sb msg-in : t3 m_") and "v1 ou v2" in l:
            msg_id = l.split()[4]
    check(msg_id, "the question id")
    c.say("[[bash: sb card --for %s \"v1 ou v2 ?\"]]" % msg_id)
    c.wait(lambda: any(cd["kind"] == "question" for cd in c.cards()), 60, "a question card")
    card = [cd for cd in c.cards() if cd["kind"] == "question"][0]
    c.wait_idle("main")
    c.say("/answer %d v2" % card["id"])
    c.wait_line("t3", 'from="user"', 60)
    c.wait(lambda: not c.cards(), 30, "card closed")
    c.wait_idle("t3")


def t_worktree_drop_restore(E, c):
    c.wait_idle("main")
    c.say('/new -w t4: {{bash: echo wt > wt.txt && git add wt.txt && git commit -qm wt && echo committed}}')
    c.wait(lambda: c.agent("t4") is not None, 60, "t4")
    a = c.agent("t4")
    check(a["mode"] == "worktree" and a["branch"] == "sb/t4", "worktree mode: %r" % a)
    wt = a["path"]
    check(wt.startswith(E.state), "the worktree lives in the state dir: " + wt)
    c.wait_line("t4", "done: tool bash ok: committed", 90)
    c.wait_idle("t4")
    check(out(wt, "git log -1 --format=%s") == "wt", "the commit is on the task's branch")
    check(not os.path.exists(os.path.join(E.ws, "wt.txt")), "the workspace is untouched")
    c.say("/drop t4")
    c.wait(lambda: any(n.get("ev") == "confirm" for n in c.notices()), 30, "a confirmation")
    conf = [n for n in c.notices() if n.get("ev") == "confirm"][-1]
    check("1 commit non poussé" in conf["text"], conf["text"])
    c.send({"op": "confirm", "id": conf["id"], "yes": True})
    c.wait_status("t4", "archived", 30)
    check(not os.path.exists(wt), "worktree removed")
    refs = out(E.ws, "git for-each-ref --format='%(refname)' refs/switchboard")
    check("refs/switchboard/trash/t4/" in refs, "snapshot ref: " + refs)
    c.say("@t4 encore ?")
    c.wait(lambda: any("/restore" in n.get("text", "") for n in c.notices()), 30, "restore hint")
    c.say("/restore t4")
    c.wait_status("t4", ["idle", "starting"], 60)
    a = c.agent("t4")
    check(os.path.exists(os.path.join(a["path"], "wt.txt")), "restored worktree has the work")
    check(out(a["path"], "git log -1 --format=%s") == "wt", "restored branch has the commit")


def t_restart_keeps_everything(E, c):
    c.wait_idle("main")
    n_main = len(c.lines("main"))
    before = [a["name"] for a in c.state["agents"]]
    E.stop_hub()
    c2 = E.start_hub()
    c2.wait(lambda: c2.state is not None, 30, "state")
    names = [a["name"] for a in c2.state["agents"]]
    check(names == before, "every agent survives: %r vs %r" % (names, before))
    c2.wait(lambda: len(c2.lines("main")) >= n_main, 30, "main's feed replayed")
    c2.wait_status("main", "idle", 60)
    c2.say("après redémarrage")
    c2.wait_line("main", "ack: après redémarrage", 60)
    return c2


def t_cli_errors(E, c):
    # the CLI refuses tasks' main-only commands, through the real shim
    c.wait_idle("main")
    c.say('@t1 [[bash: sb spawn nope --objective x; echo rc=$?]]')
    c.wait_line("t1", "réservé à main", 90)
    c.wait_idle("t1")


def t_crash_status_and_tasks(E, c):
    c.wait_idle("main", "t1")
    # a task crashes: main hears it from the hub
    pid = open(os.path.join(E.state, "agents", "t1", "repl.pid")).read().strip()
    os.kill(int(pid), 9)
    c.wait_line("main", "sb msg-in : switchboard", 60)
    c.wait(lambda: any(r["agent"] == "main" and "Task @t1 crashed" in r["user"] for r in E.fake_requests()), 60,
           "main's model got the crash notification")
    c.wait_status("t1", ["idle", "done", "blocked"], 60)
    c.wait_idle("main")
    # every user message to main starts with the task status
    c.say("[[bash: sb tasks]]")
    c.wait(lambda: any(r["agent"] == "main" and r["user"].endswith("[[bash: sb tasks]]") for r in E.fake_requests()), 60,
           "main's request")
    u = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("[[bash: sb tasks]]")][0]["user"]
    check(u.startswith("<task_status>") and "\nt1 " in u, "status block: " + u)
    # sb tasks gives main the detail
    c.wait_line("main", "## t1 —", 60)
    c.wait_idle("main")


SCENARIOS = [
    t_spawn_and_auto_reply,
    t_direct_message_and_note,
    t_ask_and_wait,
    t_escalation_card,
    t_worktree_drop_restore,
    t_cli_errors,
    t_crash_status_and_tasks,
    t_restart_keeps_everything,
]


def main():
    wanted = sys.argv[1:]
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = Env()
    ok = True
    try:
        c = E.start_hub()
        for f in SCENARIOS:
            if wanted and f.__name__ not in wanted:
                continue
            t0 = time.time()
            try:
                r = f(E, c)
                if isinstance(r, Client):
                    c = r
                print("PASS %s (%.1fs)" % (f.__name__, time.time() - t0), flush=True)
            except Exception as e:
                ok = False
                print("FAIL %s: %s" % (f.__name__, e), flush=True)
                os.environ["SB_KEEP"] = "1"
                break
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
