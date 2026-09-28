"""A scripted OpenAI-style chat-completions provider for the switchboard
end-to-end tests (BEND_PROVIDER_URL points here).

The script lives in the conversation itself:
- a user message holding `[[bash: CMD]]` markers makes the agent call its
  bash tool with each CMD, in order, one call per model request;
- when it holds no `[[...]]` marker, `{{bash: CMD}}` markers are used
  instead (so main's message can carry the script of a task's brief);
- once every marker ran (or there is none), the agent answers
  "done: <last tool result>" or "ack: <the message>".

Each request is logged to $FAKE_LOG (one JSON line: agent, last user
message, reply) for the assertions.
"""
import http.server
import json
import os
import re
import sys

LOG = os.environ.get("FAKE_LOG", "/tmp/sb-fake.log")
MARK = re.compile(r"\[\[bash: (.*?)\]\]", re.S)
INNER = re.compile(r"\{\{bash: (.*?)\}\}", re.S)


def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(b.get("text", "") for b in content if isinstance(b, dict))
    return ""


def agent_of(messages):
    for m in messages:
        if m.get("role") == "system":
            t = text_of(m.get("content"))
            g = re.search(r"# Your role: task `([^`]+)`", t)
            if g:
                return g.group(1)
            if "# Your role: `main`" in t:
                return "main"
    return "?"


def reply_for(messages):
    # the last real user message: the injected state block is not one
    idx = None
    for i, m in enumerate(messages):
        if m.get("role") == "user" and not text_of(m.get("content")).lstrip().startswith("<switchboard_state>"):
            idx = i
    if idx is None:
        return {"content": "ack: (nothing)"}
    user = text_of(messages[idx]["content"])
    # the hub's notes about the past are not a script to run
    user = re.sub(r"<switchboard_notes>.*?</switchboard_notes>", "", user, flags=re.S)
    user = re.sub(r"<task_status>.*?</task_status>", "", user, flags=re.S).strip()
    after = messages[idx + 1:]
    calls_done = sum(len(m.get("tool_calls") or []) for m in after if m.get("role") == "assistant")
    marks = MARK.findall(user) or INNER.findall(user)
    if calls_done < len(marks):
        cmd = marks[calls_done].strip()
        return {
            "content": "",
            "tool_calls": [{
                "id": "call_%d_%d" % (idx, calls_done),
                "type": "function",
                "function": {"name": "bash", "arguments": json.dumps({"arg": cmd})},
            }],
        }
    results = [text_of(m.get("content")) for m in after if m.get("role") == "tool"]
    if results:
        return {"content": "done: " + results[-1].strip()[:400]}
    one = " ".join(user.split())
    return {"content": "ack: " + one[:300]}


class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        n = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(n) or b"{}")
        msgs = body.get("messages", [])
        msg = reply_for(msgs)
        agent = agent_of(msgs)
        last_user = ""
        user = ""
        for m in msgs:
            if m.get("role") == "user":
                last_user = text_of(m.get("content"))
                if not last_user.lstrip().startswith("<switchboard_state>"):
                    user = last_user
        with open(LOG, "a") as f:
            f.write(json.dumps({"agent": agent, "last_user": last_user[:3000], "user": user[:3000],
                                "reply": msg}) + "\n")
        out = {
            "id": "fake",
            "object": "chat.completion",
            "model": body.get("model", "fake"),
            "choices": [{"index": 0, "message": {"role": "assistant", **msg},
                         "finish_reason": "tool_calls" if msg.get("tool_calls") else "stop"}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
        }
        data = json.dumps(out).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
    print("PORT", srv.server_address[1], flush=True)
    srv.serve_forever()
