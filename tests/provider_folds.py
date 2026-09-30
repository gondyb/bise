"""Reference folds, one per wire family (BISE-153): a provider's reply
(SSE bytes or whole JSON) -> the turn it says:
  {"text", "reasoning", "calls": [{"id", "name", "args"}], "error", "usage"}.
Written from the APIs' docs, independent of fake_provider.py's renderers:
provider_families.py checks the fake and the recorded fixtures with them,
live_providers.py --record checks what it records. They also say what a
Bend fold (core/*-stream.bend) must get out of a fixture."""
import json


def sse_events(raw):
    """[(event name or None, data string)] - the SSE spec: events end at a
    blank line (LF or CRLF), `data:` lines join with LF, `:` = comment"""
    text = raw.decode() if isinstance(raw, bytes) else raw
    out = []
    for block in text.replace("\r\n", "\n").replace("\r", "\n").split("\n\n"):
        name, data = None, []
        for line in block.split("\n"):
            if not line or line.startswith(":"):
                continue
            field, _, value = line.partition(":")
            value = value[1:] if value.startswith(" ") else value
            if field == "event":
                name = value
            elif field == "data":
                data.append(value)
        if data:
            out.append((name, "\n".join(data)))
    return out


def turn():
    return {"text": "", "reasoning": "", "calls": [], "error": None, "usage": None}


def args_of(s):
    try:
        return json.loads(s) if s else {}
    except ValueError:
        return {"<bad json>": s}


def err_text(e):
    if isinstance(e, dict):
        return e.get("message") or e.get("status") or json.dumps(e)
    return str(e)


# --- Anthropic Messages (docs.anthropic.com/en/docs/build-with-claude/streaming)

def anthropic_whole(d, t):
    if d.get("type") == "error":
        t["error"] = err_text(d.get("error"))
        return t
    for b in d.get("content", []):
        if b["type"] == "text":
            t["text"] += b["text"]
        elif b["type"] == "thinking":
            t["reasoning"] += b.get("thinking", "")
        elif b["type"] == "tool_use":
            t["calls"].append({"id": b["id"], "name": b["name"], "args": b.get("input", {})})
    t["usage"] = d.get("usage")
    return t


def anthropic_sse(raw, t):
    blocks, usage = {}, {}
    for name, data in sse_events(raw):
        d = json.loads(data)
        k = d.get("type", name)
        if k == "message_start":
            usage = dict(d["message"].get("usage") or {})
        elif k == "content_block_start":
            blocks[d["index"]] = dict(d["content_block"], _json="")
        elif k == "content_block_delta":
            b, x = blocks[d["index"]], d["delta"]
            if x["type"] == "text_delta":
                b["text"] = b.get("text", "") + x["text"]
            elif x["type"] == "thinking_delta":
                b["thinking"] = b.get("thinking", "") + x["thinking"]
            elif x["type"] == "signature_delta":
                b["signature"] = x["signature"]
            elif x["type"] == "input_json_delta":
                b["_json"] += x["partial_json"]
        elif k == "message_delta":
            usage.update(d.get("usage") or {})
        elif k == "error":
            t["error"] = err_text(d.get("error"))
    for i in sorted(blocks):
        b = blocks[i]
        if b["type"] == "tool_use":
            b["input"] = args_of(b["_json"]) if b["_json"] else b.get("input", {})
    anthropic_whole({"content": [blocks[i] for i in sorted(blocks)], "usage": usage or None}, t)
    return t


# --- OpenAI Chat Completions (platform.openai.com/docs/api-reference/chat-streaming;
# reasoning_content: api-docs.deepseek.com/guides/reasoning_model; `reasoning`: OpenRouter, Groq)

def openai_chat_whole(d, t):
    if "error" in d or "choices" not in d:
        # Mistral's 401 is {"detail": "..."} (FastAPI), not OpenAI's {"error": {...}}
        t["error"] = err_text(d.get("error") or d.get("detail") or d.get("message") or d)
        return t
    m = d["choices"][0]["message"]
    t["text"] = m.get("content") or ""
    t["reasoning"] = m.get("reasoning_content") or m.get("reasoning") or ""
    for c in m.get("tool_calls") or []:
        t["calls"].append({"id": c["id"], "name": c["function"]["name"],
                           "args": args_of(c["function"]["arguments"])})
    t["usage"] = d.get("usage")
    return t


def openai_chat_sse(raw, t):
    calls = {}
    for _, data in sse_events(raw):
        if data.strip() == "[DONE]":
            break
        d = json.loads(data)
        if d.get("error"):
            t["error"] = err_text(d["error"])
        if d.get("usage"):
            t["usage"] = d["usage"]
        for ch in d.get("choices") or []:
            x = ch.get("delta") or {}
            t["text"] += x.get("content") or ""
            t["reasoning"] += x.get("reasoning_content") or x.get("reasoning") or ""
            for i, c in enumerate(x.get("tool_calls") or []):
                cur = calls.setdefault(c.get("index", i), {"id": "", "name": "", "args": ""})
                cur["id"] = c.get("id") or cur["id"]
                f = c.get("function") or {}
                cur["name"] = f.get("name") or cur["name"]
                cur["args"] += f.get("arguments") or ""
    t["calls"] = [dict(calls[i], args=args_of(calls[i]["args"])) for i in sorted(calls)]
    return t


# --- OpenAI Responses (platform.openai.com/docs/api-reference/responses-streaming)

def responses_whole(d, t):
    if d.get("error"):
        t["error"] = err_text(d["error"])
        return t
    for it in d.get("output", []):
        if it["type"] == "message":
            t["text"] += "".join(c.get("text", "") for c in it["content"] if c["type"] == "output_text")
        elif it["type"] == "reasoning":
            t["reasoning"] += "".join(s.get("text", "") for s in it.get("summary", []))
        elif it["type"] == "function_call":
            t["calls"].append({"id": it["call_id"], "name": it["name"], "args": args_of(it["arguments"])})
    t["usage"] = d.get("usage")
    return t


def responses_sse(raw, t):
    items, final = {}, None
    for name, data in sse_events(raw):
        d = json.loads(data)
        k = d.get("type", name)
        if k == "response.output_item.added":
            items[d["output_index"]] = dict(d["item"], _args="", _text="", _sum="")
        elif k == "response.output_text.delta":
            items[d["output_index"]]["_text"] += d["delta"]
        elif k == "response.reasoning_summary_text.delta":
            items[d["output_index"]]["_sum"] += d["delta"]
        elif k == "response.function_call_arguments.delta":
            items[d["output_index"]]["_args"] += d["delta"]
        elif k in ("response.completed", "response.incomplete"):
            final = d["response"]
        elif k == "response.failed":
            t["error"] = err_text(d["response"].get("error"))
        elif k == "error":
            t["error"] = err_text(d)
    for i in sorted(items):
        it = items[i]
        if it["type"] == "message":
            t["text"] += it["_text"]
        elif it["type"] == "reasoning":
            t["reasoning"] += it["_sum"]
        elif it["type"] == "function_call":
            t["calls"].append({"id": it["call_id"], "name": it["name"], "args": args_of(it["_args"])})
    if final:
        # the deltas and the final response say the same thing
        whole = responses_whole(final, turn())
        assert (whole["text"], whole["calls"]) == (t["text"], t["calls"]), "response.completed differs from its deltas"
        t["usage"] = final.get("usage")
    return t


# --- Gemini (ai.google.dev/api/generate-content#method:-models.streamgeneratecontent;
# thought signatures: ai.google.dev/gemini-api/docs/thought-signatures)

def gemini_chunk(d, t):
    if "error" in d:
        t["error"] = err_text(d["error"])
        return t
    for c in d.get("candidates", [])[:1]:
        for p in (c.get("content") or {}).get("parts", []):
            if "functionCall" in p:
                f = p["functionCall"]
                t["calls"].append({"id": f.get("id", ""), "name": f["name"], "args": f.get("args", {}),
                                   "signature": p.get("thoughtSignature")})
            elif p.get("thought"):
                t["reasoning"] += p.get("text", "")
            else:
                t["text"] += p.get("text", "")
    if d.get("usageMetadata"):
        t["usage"] = d["usageMetadata"]
    return t


def gemini_sse(raw, t):
    for _, data in sse_events(raw):
        gemini_chunk(json.loads(data), t)
    return t


def fold(family, raw, stream=True):
    """the turn a reply says; `stream`: raw is SSE, else one JSON body
    (Gemini: an object or the JSON array of streamGenerateContent)"""
    t = turn()
    if stream:
        return {"anthropic": anthropic_sse, "openai-chat": openai_chat_sse,
                "openai-responses": responses_sse, "gemini": gemini_sse}[family](raw, t)
    d = json.loads(raw)
    if family == "gemini":
        for x in d if isinstance(d, list) else [d]:
            gemini_chunk(x, t)
        return t
    return {"anthropic": anthropic_whole, "openai-chat": openai_chat_whole,
            "openai-responses": responses_whole}[family](d, t)
