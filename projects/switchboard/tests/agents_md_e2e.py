"""BISE-232: the AGENTS.md files reach every agent's system prompt.

A git repo with AGENTS.md at its root and in sub/, a global AGENTS.md
in a throwaway BISE_HOME, a hub started in sub/: main and a task that
shares the workspace read global + root + sub (root first); a task in
a worktree reads its worktree's files (the committed root file, not the
workspace's uncommitted edit; sub/ is not on its root-to-cwd chain).

Run from anywhere:  python3 -u projects/switchboard/tests/agents_md_e2e.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from e2e import EXE, Env, check, sh  # noqa: E402


def block(E, agent):
    reqs = [r for r in E.fake_requests() if r["agent"] == agent and r.get("agents_md")]
    return reqs[-1]["agents_md"] if reqs else ""


def for_dir(text, d):
    """the block is for folder d (as given, or resolved: /var is /private/var)"""
    return any(text.startswith("# AGENTS.md instructions for %s\n" % p) for p in (d, os.path.realpath(d)))


def in_order(text, *needles):
    at = [text.find(n) for n in needles]
    return all(i >= 0 for i in at) and at == sorted(at)


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = Env()
    ok = False
    try:
        home = os.path.join(E.tmp, "bise-home")
        os.makedirs(home)
        open(os.path.join(home, "AGENTS.md"), "w").write("GLOBAL-RULE-g1\n")
        E.env["BISE_HOME"] = home
        sh(E.ws, "mkdir sub && echo ROOT-RULE-r1 > AGENTS.md && echo SUB-RULE-s1 > sub/AGENTS.md"
                 " && git add -A && git commit -qm docs && echo ROOT-EDIT-r2 >> AGENTS.md")
        root = E.ws
        E.ws = os.path.join(root, "sub")
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("salut")
        c.wait(lambda: block(E, "main"), 90, "a model call of main")
        m = block(E, "main")
        check(for_dir(m, E.ws), "main's block: " + m[:300])
        check(in_order(m, "GLOBAL-RULE-g1", "--- project-doc ---", "ROOT-RULE-r1", "ROOT-EDIT-r2", "SUB-RULE-s1"),
              "main reads global, root (as it is), then sub: " + m)
        c.say("/new t1: note ceci")
        c.wait(lambda: block(E, "t1"), 90, "a model call of t1")
        t1 = block(E, "t1")
        check(in_order(t1, "GLOBAL-RULE-g1", "ROOT-RULE-r1", "SUB-RULE-s1"), "t1 (shared) reads the same chain: " + t1)
        c.say("/new -w t2: note cela")
        c.wait(lambda: block(E, "t2"), 90, "a model call of t2")
        wt = c.agent("t2")["path"]
        t2 = block(E, "t2")
        check(for_dir(t2, wt), "t2 works in its worktree: " + t2[:300])
        own = next((f for f in ("Contents of %s/AGENTS.md:" % p for p in (wt, os.path.realpath(wt))) if f in t2), "?")
        check(in_order(t2, "GLOBAL-RULE-g1", own, "ROOT-RULE-r1"),
              "t2 reads its worktree's root file: " + t2)
        check("ROOT-EDIT-r2" not in t2 and "SUB-RULE-s1" not in t2,
              "t2 reads the committed file, and sub/ is not on its chain: " + t2)
        ok = True
        print("PASS agents_md_e2e", flush=True)
    except Exception as e:
        print("FAIL agents_md_e2e: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.ws = root if "root" in dir() else E.ws
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
