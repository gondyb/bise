"""BISE-293: the first message answers on a provider as strict as the
real ones about tool names (tmux, fake provider, throwaway hub).

The user's first message on OpenAI ("show me what you can do") failed:
OpenAI refuses a tool name off ^[a-zA-Z0-9_-]+$, and bise sent
self.compact / self.reload. The fake provider now refuses such a name
with the API's own 400 (tools and history calls), so:

- openai-chat (OpenAI's pattern) and anthropic (Anthropic's): the demo
  prompt is answered, no request was refused for a name, and no tool
  name bise sent has a dot (self.* are run_typescript bindings only);
- a refused request reads once, in two lines: `✗ the turn stopped:
  Fake refused the request (400).` then the provider's own words, never
  the raw JSON, never twice.

python3 -u tests/tui_tool_names_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, in_view  # noqa: E402

DEMO = "show me what you can do"
NAME = re.compile(r"^[a-zA-Z0-9_-]{1,64}$")


def first_turn(E, family, env=""):
    with tui_session(150, 42, env=env, E=E) as t:
        t.wait("bise :*")
        t.wait_re(in_view("main"))
        t.typed(DEMO)
        t.keys("Enter")
        sc = t.wait("ack: " + DEMO, 60)
        recs = E.fake_requests()
        refused = [r for r in recs if r.get("name_error")]
        assert not refused, "%s: a tool name refused: %r" % (family, refused)
        mine = [r for r in recs if r.get("family") == family and r.get("status") == 200]
        assert mine, "%s: no answered request: %r" % (family, [(r.get("family"), r.get("status")) for r in recs])
        assert "refused" not in sc and "{" not in sc.split("ack: " + DEMO)[0][-400:], sc
        print(sc)
        print("ok  %s: the demo prompt answered, every tool name valid" % family)
        # a refused request: one failure, bise's line then the provider's words
        t.typed("[[error: badname]] one more")
        t.keys("Enter")
        sc = t.wait("the turn stopped:", 60)
        t.wait("said: \"", 10)
        sc = t.screen()
        print(sc)
        assert re.search(r"✗ the turn stopped: (the provider|\S+) refused the request \(400\)\.", sc), sc
        assert "candidate discarded" not in sc, sc
        assert '"error"' not in sc and "invalid_request_error" not in sc, sc
        print("ok  %s: a refused request reads once, in bise's words and the provider's" % family)


def main():
    E = e2e.Env()
    first_turn(E, "openai-chat")
    E = e2e.Env()
    url = E.env["BEND_PROVIDER_URL"].replace("/chat/completions", "/messages")
    E.env.update(BEND_PROVIDER_URL=url, BEND_MODEL="anthropic/claude-opus-5-5")
    # the TUI passes on SB_/BEND_/MISTRAL_ variables only
    first_turn(E, "anthropic", "ANTHROPIC_API_KEY=fake-key")
    print("PASS tui tool names")


if __name__ == "__main__":
    run(main)
