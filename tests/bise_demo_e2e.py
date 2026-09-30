"""The built-in bise-demo skill reaches main, and only main.

A hub on the fake provider in a throwaway state: main's `skill` call
with bise-demo loads the app root's prompts/skills/bise-demo/SKILL.md
(the fake answers "done: <the tool result>"), a task's same call is an
unknown skill (the built-in skills are main's), and the workspace has
no change (the skill is read, never copied there).

Run from anywhere:  python3 -u tests/bise_demo_e2e.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from e2e import EXE, Env, check, out  # noqa: E402


def replies(E, agent):
    return [str(r.get("reply", {}).get("content") or "") for r in E.fake_requests() if r["agent"] == agent]


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = Env()
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("show me what you can do [[skill: bise-demo]]")
        c.wait(lambda: any(r.startswith("done: ") for r in replies(E, "main")), 90, "main's skill call")
        m = next(r for r in replies(E, "main") if r.startswith("done: "))
        check("# bise demo: a small team" in m and "name: bise-demo" not in m, "main loads the built-in skill: " + m[:300])
        c.say("/new t1: {{skill: bise-demo}} [[skill: bise-demo]]")
        c.wait(lambda: any(r.startswith("done: ") for r in replies(E, "t1")), 90, "t1's skill call")
        t = next(r for r in replies(E, "t1") if r.startswith("done: "))
        check("unknown skill: bise-demo" in t, "a task has no built-in skill: " + t[:300])
        check(out(E.ws, "git status --porcelain") == "", "the workspace is unchanged")
        ok = True
        print("PASS bise_demo_e2e", flush=True)
    except Exception as e:
        print("FAIL bise_demo_e2e: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
