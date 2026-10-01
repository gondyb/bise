#!/usr/bin/env python3
"""The foundry proxy's base URL reaches a running hub (Ben's report,
2026-10-01: ANTHROPIC_FOUNDRY_BASE_URL was ignored, and the hub's
cache/models.toml kept base_url = "" until `bise switchboard --stop`).

A real hub, the scripted provider (e2e.Env), a temp HOME and BISE_HOME.
  1. no URL anywhere: the hub's models file has no `base_url = ""`; its
     foundry table names the variable (base_url_env);
  2. ANTHROPIC_FOUNDRY_BASE_URL in ~/.vibe/.env (Vibe's file, Claude
     Code's form without /v1), written while the hub runs: the next
     message rewrites the file with the URL (+ /v1);
  3. config.toml's [providers.foundry] base_url, written while the hub
     runs: it wins at the next message;
  4. a task spawned now gets the same file (BISE_MODELS_FILE);
the same hub process all along: no --stop.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402


def foundry_table(text):
    """the [providers.foundry] table of a models file, its lines"""
    part = text.split("[providers.foundry]\n", 1)[1]
    return part.split("\n[", 1)[0]


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    E.env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"))
    for k in ("ANTHROPIC_FOUNDRY_BASE_URL", "ANTHROPIC_FOUNDRY_API_KEY", "BISE_MODELS_FILE"):
        E.env.pop(k, None)
    models = os.path.join(bise, "cache", "models.toml")
    read = lambda: open(models).read()  # noqa: E731
    ok = False
    try:
        c = E.start_hub()
        pid = E.hub.pid
        c.wait_status("main", "idle", 60)
        c.wait(lambda: os.path.exists(models), 20, "the hub's models file")
        t = read()
        check('base_url = ""' not in t, "no empty base_url in the models file")
        f = foundry_table(t)
        check("base_url =" not in f and 'base_url_env = "ANTHROPIC_FOUNDRY_BASE_URL"' in f,
              "no URL: foundry names its variable: %r" % f)

        # 2. the URL in ~/.vibe/.env, the hub running: the next message
        os.makedirs(os.path.join(home, ".vibe"))
        with open(os.path.join(home, ".vibe", ".env"), "w") as fh:
            fh.write("ANTHROPIC_FOUNDRY_BASE_URL=https://vibe.foundry.test/anthropic\n")
        c.say("hello")
        c.wait(lambda: 'base_url = "https://vibe.foundry.test/anthropic/v1"' in foundry_table(read()), 30,
               "the .env file's URL (+ /v1) in the models file")
        c.wait_idle("main")

        # 3. config.toml, the hub running: it wins at the next message
        with open(os.path.join(bise, "config.toml"), "w") as fh:
            fh.write('[providers.foundry]\nbase_url = "https://cfg.foundry.test/anthropic/v1"\n')
        c.say("again")
        c.wait(lambda: 'base_url = "https://cfg.foundry.test/anthropic/v1"' in foundry_table(read()), 30,
               "config.toml's URL in the models file")
        c.wait_idle("main")

        # 4. a task spawned now: the same file, its REPL's BISE_MODELS_FILE
        probe = os.path.join(E.ws, "probe.txt")
        c.say("[[bash: sb spawn tt --objective '{{bash: echo \"$BISE_MODELS_FILE\" > probe.txt}}']]")
        c.wait(lambda: os.path.exists(probe) and open(probe).read().strip(), 90, "the task's BISE_MODELS_FILE")
        got = open(probe).read().strip()
        check(os.path.realpath(got) == os.path.realpath(models), "the task reads the hub's file: %r" % got)
        check('base_url = "https://cfg.foundry.test/anthropic/v1"' in foundry_table(open(got).read()),
              "the task's file has the URL")
        check(E.hub.pid == pid and E.hub.poll() is None, "the same hub all along")
        ok = True
    finally:
        E.close()
    print("PASS" if ok else "FAIL")


if __name__ == "__main__":
    main()
