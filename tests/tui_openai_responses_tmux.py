"""BISE-147: OpenAI on the Responses API (tmux, fake provider, throwaway
hub). The user's model, openai/gpt-6.1-sol, takes tools with reasoning
on /v1/responses only; the fake provider answers there as strictly as
OpenAI (store false: a replayed reasoning item needs its
encrypted_content and no id; each function_call_output needs its
function_call; tool names on the pattern), and refuses tools with
reasoning_effort on Chat Completions for a gpt-6 model.

- the demo prompt is answered, streamed from /v1/responses;
- a turn with reasoning and two bash calls: each request after the first
  replays the reasoning item (its encrypted_content, no id) and the
  calls with their outputs; the answer shows; no request is refused.

python3 -u tests/tui_openai_responses_tmux.py
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, in_view  # noqa: E402

DEMO = "show me what you can do"


def main():
    E = e2e.Env()
    url = E.env["BEND_PROVIDER_URL"].replace("/chat/completions", "/responses")
    E.env.update(BEND_PROVIDER_URL=url, BEND_MODEL="openai/gpt-6.1-sol")
    # the TUI passes on SB_/BEND_/MISTRAL_ variables only
    with tui_session(150, 42, env="OPENAI_API_KEY=fake-key", E=E) as t:
        t.wait("bise :*")
        t.wait_re(in_view("main"))
        t.typed(DEMO)
        t.keys("Enter")
        t.wait("ack: " + DEMO, 60)
        print("ok  the demo prompt answered on openai-responses")
        t.typed("[[think: two steps]] [[bash: echo resp-one]] [[bash: echo resp-two]]")
        t.keys("Enter")
        sc = t.wait("done: ", 90)
        print(sc)
        recs = E.fake_requests()
        bad = [r for r in recs if r.get("status") != 200]
        assert not bad, "a request refused: %r" % bad
        mine = [r for r in recs if r.get("family") == "openai-responses"]
        assert mine and all(r["stream"] and r["path"] == "/v1/responses" for r in mine), \
            [(r.get("family"), r.get("path"), r.get("stream")) for r in recs]
        assert len(mine) >= 4, "the demo turn, then the reasoning turn's 3 requests: %d" % len(mine)
        print("ok  a reasoning turn with two bash calls, every request accepted (%d)" % len(mine))
        print("PASS tui openai responses")


if __name__ == "__main__":
    run(main)
