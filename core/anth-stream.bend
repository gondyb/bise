# core/anth-stream.bend — the Anthropic Messages API, streamed.
#
# A non-streaming reply sends nothing until the whole generation is
# done (a 32k-token opus-5.5 reply: first byte after 255 s), so a read
# timeout cannot tell a long reply from a dead connection. Streamed, the
# reply is a Server-Sent Events body: the first event lands in about a
# second and events keep coming (deltas, pings; the largest gap measured
# over a 100 s reply was 5 s).
#
# This module is the pure half: the request body with "stream": true,
# and the fold that rebuilds the final message from the events - the
# exact JSON a non-streaming reply carries - so Api.reply_anth maps it
# to the same OK/CALL/END lines, byte for byte.
#
# The events (https://docs.anthropic.com/en/api/messages-streaming):
#   message_start, ping, message_stop            nothing to keep
#   content_block_start {content_block}          a new block
#   content_block_delta {delta}                  text_delta, thinking_delta,
#                                                signature_delta,
#                                                input_json_delta
#   content_block_stop                           nothing to keep
#   message_delta {delta: {stop_reason, ...}}    how the message ended
#   error {error}                                the reply is that error
# Blocks stream one at a time (start, deltas, stop), so a delta always
# belongs to the block started last.
import Base
import ./api.bend as Api
import ./text.bend as Tx
import ../vendor/json.bend as J

# the request body with "stream": true (the body is always a non-empty
# JSON object: its fields follow the new one)
def stream_body(+body: String) -> String:
  String.append("{\"stream\":true,", String.drop(body, 1n))

# ---- a block being rebuilt: its deltas, newest first ----

type Blk is Data:
  BThink{parts: List<&2, String>, sig: List<&2, String>}
  BText{parts: List<&2, String>}
  BTool{id: String, name: String, parts: List<&2, String>}
  BRaw{j: J.Json}

# the fold state: the blocks (newest first), the message_delta's delta
# (stop_reason, stop_details), and the error event if one came
type Acc is Data:
  Acc{blocks: List<&2, Blk>, fin: J.Json, err: Maybe<&2, J.Json>}

def acc0() -> Acc:
  Acc{Nil{}, J.JNull{}, None{}}

def str_of(+j: J.Json, +key: String) -> String:
  J.get_str_or(j, key, "")

# ---- content_block_start: the block, as it opens ----

def blk.tool(is_tool: Bool, +cb: J.Json) -> Blk:
  match is_tool:
    case True{}:
      BTool{str_of(cb, "id"), str_of(cb, "name"), Nil{}}
    case False{}:
      # redacted_thinking and anything newer arrive whole
      BRaw{cb}

def blk.text(is_text: Bool, +cb: J.Json) -> Blk:
  match is_text:
    case True{}:
      BText{Con{str_of(cb, "text"), Nil{}}}
    case False{}:
      blk.tool(String.eq(str_of(cb, "type"), "tool_use"), cb)

def blk.of(is_think: Bool, +cb: J.Json) -> Blk:
  match is_think:
    case True{}:
      BThink{Con{str_of(cb, "thinking"), Nil{}},
        Con{str_of(cb, "signature"), Nil{}}}
    case False{}:
      blk.text(String.eq(str_of(cb, "type"), "text"), cb)

def block_of(+cb: J.Json) -> Blk:
  blk.of(String.eq(str_of(cb, "type"), "thinking"), cb)

# ---- content_block_delta: one piece for the open block ----

type DKind is Data:
  DThink{}
  DSig{}
  DText{}
  DJson{}
  DNone{}

def dkind.json(hit: Bool) -> DKind:
  match hit:
    case True{}:
      DJson{}
    case False{}:
      DNone{}

def dkind.text(hit: Bool, +t: String) -> DKind:
  match hit:
    case True{}:
      DText{}
    case False{}:
      dkind.json(String.eq(t, "input_json_delta"))

def dkind.sig(hit: Bool, +t: String) -> DKind:
  match hit:
    case True{}:
      DSig{}
    case False{}:
      dkind.text(String.eq(t, "text_delta"), t)

def dkind.think(hit: Bool, +t: String) -> DKind:
  match hit:
    case True{}:
      DThink{}
    case False{}:
      dkind.sig(String.eq(t, "signature_delta"), t)

def dkind_of(+t: String) -> DKind:
  dkind.think(String.eq(t, "thinking_delta"), t)

# the piece a delta carries
def piece_of(k: DKind, +d: J.Json) -> String:
  match k:
    case DThink{}:
      str_of(d, "thinking")
    case DSig{}:
      str_of(d, "signature")
    case DText{}:
      str_of(d, "text")
    case DJson{}:
      str_of(d, "partial_json")
    case DNone{}:
      ""

# a piece joins the open block when the kinds agree; anything else
# leaves the block as it was
def apply(k: DKind, +p: String, +b: Blk) -> Blk:
  match k:
    case DThink{}:
      match b:
        case BThink{+parts, +sig}:
          BThink{Con{p, parts}, sig}
        case _:
          b
    case DSig{}:
      match b:
        case BThink{+parts, +sig}:
          BThink{parts, Con{p, sig}}
        case _:
          b
    case DText{}:
      match b:
        case BText{+parts}:
          BText{Con{p, parts}}
        case _:
          b
    case DJson{}:
      match b:
        case BTool{+id, +name, +parts}:
          BTool{id, name, Con{p, parts}}
        case _:
          b
    case DNone{}:
      b

def apply_head(k: DKind, +p: String, +blocks: List<&2, Blk>) -> List<&2, Blk> :
  match blocks:
    case Nil{}:
      Nil{}
    case Con{+b, +rest}:
      Con{apply(k, p, b), rest}

# ---- one event ----

type EKind is Data:
  EStart{}
  EDelta{}
  EFin{}
  EErr{}
  ESkip{}

def ekind.err(hit: Bool) -> EKind:
  match hit:
    case True{}:
      EErr{}
    case False{}:
      ESkip{}

def ekind.fin(hit: Bool, +t: String) -> EKind:
  match hit:
    case True{}:
      EFin{}
    case False{}:
      ekind.err(String.eq(t, "error"))

def ekind.delta(hit: Bool, +t: String) -> EKind:
  match hit:
    case True{}:
      EDelta{}
    case False{}:
      ekind.fin(String.eq(t, "message_delta"), t)

def ekind.start(hit: Bool, +t: String) -> EKind:
  match hit:
    case True{}:
      EStart{}
    case False{}:
      ekind.delta(String.eq(t, "content_block_delta"), t)

def ekind_of(+t: String) -> EKind:
  ekind.start(String.eq(t, "content_block_start"), t)

def step.delta(+d: J.Json, +blocks: List<&2, Blk>) -> List<&2, Blk>:
  apply_head(dkind_of(str_of(d, "type")),
    piece_of(dkind_of(str_of(d, "type")), d), blocks)

def step(k: EKind, +e: J.Json, +a: Acc) -> Acc:
  match k:
    case EStart{}:
      match a:
        case Acc{+blocks, +fin, +err}:
          Acc{Con{block_of(Api.field_or(e, "content_block", J.JNull{})), blocks},
            fin, err}
    case EDelta{}:
      match a:
        case Acc{+blocks, +fin, +err}:
          Acc{step.delta(Api.field_or(e, "delta", J.JNull{}), blocks), fin, err}
    case EFin{}:
      match a:
        case Acc{+blocks, _, +err}:
          Acc{blocks, Api.field_or(e, "delta", J.JNull{}), err}
    case EErr{}:
      match a:
        case Acc{+blocks, +fin, _}:
          Acc{blocks, fin, Some{e}}
    case ESkip{}:
      a

# ---- the SSE body: every "data:" line is one event ----

def event.parsed(r: Result<&2, &2, String, J.Json>, +a: Acc) -> Acc:
  match r:
    case Done{+e}:
      step(ekind_of(str_of(e, "type")), e, a)
    case Fail{_}:
      a

def event.line(is_data: Bool, +l: String, +a: Acc) -> Acc:
  match is_data:
    case True{}:
      event.parsed(J.parse(String.trim(String.drop(l, 5n))), a)
    case False{}:
      a

def fold(lines: List<&2, String>, +a: Acc) -> Acc:
  match lines:
    case Nil{}:
      a
    case Con{+l, rest}:
      fold(rest, event.line(String.starts_with(l, "data:"), l, a))

# ---- the final message ----

def joined(+parts: List<&2, String>) -> String:
  String.join(List.reverse(&2, String, parts), "")

# a tool input streams as JSON text; none at all is the empty object
def tool_input.parsed(r: Result<&2, &2, String, J.Json>) -> J.Json:
  match r:
    case Done{+j}:
      j
    case Fail{_}:
      J.obj(Nil{})

def tool_input.of(empty: Bool, +s: String) -> J.Json:
  match empty:
    case True{}:
      J.obj(Nil{})
    case False{}:
      tool_input.parsed(J.parse(s))

def tool_input(+s: String) -> J.Json:
  tool_input.of(String.is_empty(String.trim(s)), s)

def block_json(+b: Blk) -> J.Json:
  match b:
    case BThink{+parts, +sig}:
      J.obj(Con{J.kv("type", J.str("thinking")),
        Con{J.kv("thinking", J.str(joined(parts))),
        Con{J.kv("signature", J.str(joined(sig))), Nil{}}}})
    case BText{+parts}:
      J.obj(Con{J.kv("type", J.str("text")),
        Con{J.kv("text", J.str(joined(parts))), Nil{}}})
    case BTool{+id, +name, +parts}:
      J.obj(Con{J.kv("type", J.str("tool_use")),
        Con{J.kv("id", J.str(id)),
        Con{J.kv("name", J.str(name)),
        Con{J.kv("input", tool_input(joined(parts))), Nil{}}}}})
    case BRaw{+j}:
      j

def blocks_json(bs: List<&2, Blk>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{+b, +rest}:
      Con{block_json(b), blocks_json(rest)}

def message.of(+blocks: List<&2, Blk>, +fin: J.Json) -> J.Json:
  J.obj(Con{J.kv("type", J.str("message")),
    Con{J.kv("content", J.arr(blocks_json(List.reverse(&2, Blk, blocks)))),
    Con{J.kv("stop_reason", J.str(str_of(fin, "stop_reason"))),
    Con{J.kv("stop_details", Api.field_or(fin, "stop_details", J.JNull{})),
      Nil{}}}}})

# the message a non-streaming reply would have carried; an error event
# IS the reply ({"type":"error","error":{...}}, what a 200 error body is)
def message(+a: Acc) -> J.Json:
  match a:
    case Acc{+blocks, +fin, +err}:
      match err:
        case Some{+e}:
          e
        case None{}:
          message.of(blocks, fin)

# the whole SSE body -> the OK/CALL/END reply (Api.reply_anth, as for a
# non-streaming body)
def reply(+sse: String) -> String:
  Api.reply_anth(message(fold(String.split(sse, Tx.newline()), acc0())))

# a 200 is not always a stream: the proxy answers an overload with a
# whole JSON error body. JSON in, the whole-reply mapping; events in,
# the fold.
def reply_any.pick(is_json: Bool, +body: String) -> String:
  match is_json:
    case True{}:
      Api.reply_of_for(Api.Anthropic{}, 200, body)
    case False{}:
      reply(body)

def reply_any(+body: String) -> String:
  reply_any.pick(String.starts_with(String.trim(body), "{"), body)

# ---- the token usage (runtime/usage-pure.bend reads it) ----
# A whole reply carries "usage"; a streamed one splits it: message_start
# has the input side (message.usage, output_tokens still ~1), every
# message_delta the running usage (output_tokens final in the last
# one). The usage a whole reply would carry: the last delta's fields in
# front of the start's (J.get finds the first key). No message_start:
# no usage (null).

type Use is Data:
  Use{start: J.Json, delta: J.Json}

def use.delta(hit: Bool, +e: J.Json, +u: Use) -> Use:
  match hit:
    case True{}:
      match u:
        case Use{+s, _}:
          Use{s, Api.field_or(e, "usage", J.JNull{})}
    case False{}:
      u

def use.start(hit: Bool, +t: String, +e: J.Json, +u: Use) -> Use:
  match hit:
    case True{}:
      match u:
        case Use{_, +d}:
          Use{Api.field_or(Api.field_or(e, "message", J.JNull{}), "usage",
            J.JNull{}), d}
    case False{}:
      use.delta(String.eq(t, "message_delta"), e, u)

def use.event(r: Result<&2, &2, String, J.Json>, +u: Use) -> Use:
  match r:
    case Done{+e}:
      use.start(String.eq(str_of(e, "type"), "message_start"),
        str_of(e, "type"), e, u)
    case Fail{_}:
      u

def use.line(is_data: Bool, +l: String, +u: Use) -> Use:
  match is_data:
    case True{}:
      use.event(J.parse(String.trim(String.drop(l, 5n))), u)
    case False{}:
      u

def use.fold(lines: List<&2, String>, +u: Use) -> Use:
  match lines:
    case Nil{}:
      u
    case Con{+l, rest}:
      use.fold(rest, use.line(String.starts_with(l, "data:"), l, u))

def use.kvs(+j: J.Json) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match j:
    case J.JObj{+kvs}:
      kvs
    case _:
      Nil{}

def use.merged(+s: J.Json, +d: J.Json) -> J.Json:
  match s:
    case J.JObj{+kvs}:
      J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>, use.kvs(d),
        kvs))
    case _:
      J.JNull{}

def use.json(+u: Use) -> J.Json:
  match u:
    case Use{+s, +d}:
      use.merged(s, d)

def usage(+sse: String) -> J.Json:
  use.json(use.fold(String.split(sse, Tx.newline()),
    Use{J.JNull{}, J.JNull{}}))
