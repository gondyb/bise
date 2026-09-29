# core/api.bend — the provider API adapter, pure Bend: the family
# dispatch.
#
# runtime/remote.bend prints the harness wire request (MODEL/TOOL/MSG/
# CALL lines, pinned by LAWS.bend); core/wire.bend parses it; the
# family of the model maps it to its JSON body and maps the provider
# response back to the OK/CALL/END reply the Core parses:
#   core/wire.bend       the wire, shared by every family
#   core/oai-chat.bend   OpenAI Chat Completions (and compatibles)
#   core/anthropic.bend  Anthropic Messages (+ core/anth-stream.bend)
# A new family is one more module and one more case below.

import Base
import ./wire.bend as W
import ./oai-chat.bend as Oai
import ./anthropic.bend as Anth

# ---- the provider adapters (the vibe CLI structure) ----
# One mapping per api_style, selected by the provider table in
# runtime/provider.bend (the mirror of the vibe config: a provider carries
# an api_base, a key env and an api_style; a model points at one).
# The OpenAI-style mapping (core/oai-chat.bend) stays exactly as the
# laws pinned it; the Anthropic-style mapping is core/anthropic.bend.

type Style is Data:
  OpenAI{}
  Anthropic{}

# ---- the style dispatch ----

def is_anthropic(st: Style) -> Bool:
  match st:
    case OpenAI{}:
      False{}
    case Anthropic{}:
      True{}

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
