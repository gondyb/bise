# core/api.bend — the provider families: every per-family fact, pure
# Bend, in one place.
#
# runtime/remote.bend prints the harness wire request (MODEL/TOOL/MSG/
# CALL lines, pinned by LAWS.bend); core/wire.bend parses it; the
# family of the model maps it to its JSON body and maps the provider
# response back to the OK/CALL/END reply the Core parses:
#   core/wire.bend       the wire, shared by every family
#   core/oai-chat.bend   OpenAI Chat Completions (and compatibles)
#   core/oai-stream.bend OpenAI Chat, streamed
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
#   MFacts                    what the catalog says of the model
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
import ./oai-stream.bend as Os
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

# the anthropic-beta flags: the model's (catalog `betas`, BISE-146: the
# direct API takes no 1M-context flag), else the foundry proxy's list
def anth_betas.pick(empty: Bool, +betas: String) -> String:
  match empty:
    case True{}:
      "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"
    case False{}:
      betas

def anth_betas(+betas: String) -> String:
  anth_betas.pick(String.is_empty(betas), betas)

def headers(st: Style, +betas: String, +key: String) -> List<&2, Hdr>:
  match st:
    case OpenAI{}:
      headers.auth(String.is_empty(key),
        Hdr{"Authorization", String.append("Bearer ", key)},
        [Hdr{"Content-Type", "application/json"}])
    case Anthropic{}:
      headers.auth(String.is_empty(key), Hdr{"x-api-key", key},
        [Hdr{"anthropic-version", "2023-06-01"},
          Hdr{"anthropic-beta", anth_betas(betas)},
          Hdr{"Content-Type", "application/json"}])

# ---- streaming: every family streams (Server-Sent Events) ----

def streams(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      True{}
    case Anthropic{}:
      True{}

def wire_body(st: Style, +body: String) -> String:
  match st:
    case OpenAI{}:
      Os.stream_body(body)
    case Anthropic{}:
      As.stream_body(body)

# ---- what the catalog says of the model ----
# runtime/provider-pure.bend fills it from the models file (a key of
# models."p/id", else of providers.p): reasoning = `reasoning = true`;
# output = `max_output` (0n: not set); max_key = the name of the output
# cap in an OpenAI Chat body ("max_completion_tokens" for OpenAI itself,
# whose reasoning models refuse max_tokens; "max_tokens" elsewhere);
# thinking / betas: the raw keys (Anthropic, BISE-146; "" = not set).

type MFacts is Data:
  MFacts{reasoning: Bool, output: Nat, max_key: String, thinking: String,
    betas: String}

def facts_betas(f: MFacts) -> String:
  match f:
    case MFacts{_, _, _, _, +betas}:
      betas

# no catalog entry: today's body (reasoning_effort on, no output cap)
def facts0() -> MFacts:
  MFacts{True{}, 0n, "max_tokens", "", ""}

# ---- the body and the reply ----

def api_body_for(st: Style, +model: String, +facts: MFacts, +req: W.WReq)
    -> String:
  match st:
    case OpenAI{}:
      match facts:
        case MFacts{+reasoning, +output, +max_key, _, _}:
          Oai.api_body_with(model, Oai.Opts{reasoning, output, max_key}, req)
    case Anthropic{}:
      match facts:
        case MFacts{+reasoning, +output, _, +thinking, _}:
          Anth.api_body_anth_for(model, output, thinking, reasoning, req)

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
      Os.reply_any(text)
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

# the "usage" object a streamed body spreads over its events
def stream_usage(st: Style, +text: String) -> J.Json:
  match st:
    case OpenAI{}:
      Os.usage(text)
    case Anthropic{}:
      As.usage(text)
