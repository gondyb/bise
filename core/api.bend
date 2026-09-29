# core/api.bend — the provider families: every per-family fact, pure
# Bend, in one place.
#
# runtime/remote.bend prints the harness wire request (MODEL/TOOL/MSG/
# CALL lines, pinned by LAWS.bend); core/wire.bend parses it; the
# family of the model maps it to its JSON body and maps the provider
# response back to the OK/CALL/END reply the Core parses:
#   core/wire.bend       the wire, shared by every family
#   core/oai-chat.bend   OpenAI Chat Completions (and compatibles)
#   core/anthropic.bend  Anthropic Messages (+ core/anth-stream.bend)
#
# The family is one of the two registries of the provider arc (the
# other one, the providers, is runtime/provider-pure.bend: a provider
# names its family by `api = "<name>"`). Everything the runtime needs to
# know about a family is answered here, so runtime/provider.bend and
# runtime/usage-pure.bend never match on it:
#   style_of_api / api_name   the `api` name <-> the family
#   endpoint                  the path appended to a provider's base_url
#   headers                   the HTTP headers (the key, when there is one)
#   streams / wire_body       whether the call streams (SSE), its body
#   api_body_for              the request body
#   reply_of_for              a whole reply -> OK/CALL/END
#   stream_reply              a streamed reply -> OK/CALL/END
#   usage_of / stream_usage   the token usage of a reply
# A new family (BISE-147 Responses, 148 Gemini) is one more constructor,
# one more case in each function below, and its wire module.

import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./anthropic.bend as Anth
import ./anth-stream.bend as As
import ../vendor/json.bend as J

type Style is Data:
  OpenAI{}
  Anthropic{}

# ---- the family's name (`api = ...` in a provider table) ----

def style_of_api.chat(is_chat: Bool, is_anth: Bool) -> Maybe<&2, Style>:
  match is_chat:
    case True{}:
      Some{OpenAI{}}
    case False{}:
      match is_anth:
        case True{}:
          Some{Anthropic{}}
        case False{}:
          None{}

# None: a family bise does not speak (yet): openai-responses, gemini,
# bedrock-converse, a typo
def style_of_api(+api: String) -> Maybe<&2, Style>:
  style_of_api.chat(String.eq(api, "openai-chat"), String.eq(api, "anthropic"))

def api_name(st: Style) -> String:
  match st:
    case OpenAI{}:
      "openai-chat"
    case Anthropic{}:
      "anthropic"

def is_anthropic(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      False{}
    case Anthropic{}:
      True{}

# ---- the endpoint: base_url + this path ----

def endpoint(st: Style) -> String:
  match st:
    case OpenAI{}:
      "/chat/completions"
    case Anthropic{}:
      "/messages"

# ---- the headers, in the order they are set ----
# No key (a provider with key_env = "": a local server) sends no auth
# header.

type Hdr is Data:
  Hdr{name: String, val: String}

def headers.auth(no_key: Bool, +h: Hdr, +rest: List<&2, Hdr>)
    -> List<&2, Hdr>:
  match no_key:
    case True{}:
      rest
    case False{}:
      Con{h, rest}

def headers(st: Style, +key: String) -> List<&2, Hdr>:
  match st:
    case OpenAI{}:
      headers.auth(String.is_empty(key),
        Hdr{"Authorization", String.append("Bearer ", key)},
        [Hdr{"Content-Type", "application/json"}])
    case Anthropic{}:
      headers.auth(String.is_empty(key), Hdr{"x-api-key", key},
        [Hdr{"anthropic-version", "2023-06-01"},
          Hdr{"anthropic-beta",
            "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"},
          Hdr{"Content-Type", "application/json"}])

# ---- streaming: the Messages API streams, the others answer whole ----

def streams(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      False{}
    case Anthropic{}:
      True{}

def wire_body(st: Style, +body: String) -> String:
  match st:
    case OpenAI{}:
      body
    case Anthropic{}:
      As.stream_body(body)

# ---- the body and the reply ----

def api_body_for(st: Style, +model: String, +req: W.WReq) -> String:
  match st:
    case OpenAI{}:
      Oai.api_body(model, req)
    case Anthropic{}:
      Anth.api_body_anth(model, req)

def reply_of_for(st: Style, +status: U32, +body: String) -> String:
  match st:
    case OpenAI{}:
      Oai.reply_of(status, body)
    case Anthropic{}:
      Anth.reply_anth_of(status, body)

# a 200 of a streaming family (its events, or a whole JSON error body)
def stream_reply(st: Style, +text: String) -> String:
  match st:
    case OpenAI{}:
      Oai.reply_of(200, text)
    case Anthropic{}:
      As.reply_any(text)

# ---- the token usage ----
# in: every input token of the call (cached ones included); out: the
# tokens the model wrote; cr/cw: the cache reads and writes.

type Usage is Data:
  Usage{inp: Nat, out: Nat, cr: Nat, cw: Nat}

# Anthropic Messages: input_tokens counts only the uncached part;
# OpenAI-compatible: prompt_tokens already counts the cached part
def usage_of(st: Style, +u: J.Json) -> Usage:
  match st:
    case OpenAI{}:
      Usage{J.get_num_or(u, "prompt_tokens", 0n),
        J.get_num_or(u, "completion_tokens", 0n),
        J.get_num_or(W.field_or(u, "prompt_tokens_details", J.JNull{}),
          "cached_tokens", 0n),
        0n}
    case Anthropic{}:
      +cr = J.get_num_or(u, "cache_read_input_tokens", 0n)
      +cw = J.get_num_or(u, "cache_creation_input_tokens", 0n)
      Usage{Nat.add(J.get_num_or(u, "input_tokens", 0n), Nat.add(cr, cw)),
        J.get_num_or(u, "output_tokens", 0n), cr, cw}

# the "usage" object a streamed body spreads over its events (OpenAI
# Chat does not stream yet: BISE-144 gives it its own fold; until then
# it reads as today)
def stream_usage(st: Style, +text: String) -> J.Json:
  match st:
    case OpenAI{}:
      As.usage(text)
    case Anthropic{}:
      As.usage(text)
