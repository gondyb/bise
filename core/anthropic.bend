# core/anthropic.bend — the Anthropic Messages family, pure Bend.
#
# The Messages API shape the foundry-anthropic proxy serves (probed
# live: non-streaming works, thinking blocks are optional, tool_use
# blocks carry their own ids): the parsed wire request (core/wire.bend
# WReq) becomes a Messages JSON body, a Messages response becomes the
# OK/CALL/END reply. The streamed reply folds into the same response
# first (core/anth-stream.bend).

import Base
import ./image.bend as Im
import ./wire.bend as W
import ../vendor/json.bend as J

def input_json.parsed(j: J.Json) -> J.Json:
  match j:
    case J.JNull{}:
      J.obj(Nil{})
    case _:
      j

def input_json.pick(structured: Bool, +args: String) -> J.Json:
  match structured:
    case True{}:
      input_json.parsed(W.parsed_or(J.parse(args)))
    case False{}:
      J.obj(Con{J.kv("arg", J.str(args)), Nil{}})

# the tool_use "input" is a JSON object: structured tools pass their
# JSON through, the flat tools keep the {"arg": ...} wrapper
def input_json(+name: String, +args: String) -> J.Json:
  input_json.pick(W.is_structured(name), args)

def tool_use_json(c: W.WCall) -> J.Json:
  match c:
    case W.WCall{+id, +name, +args}:
      J.obj(Con{J.kv("type", J.str("tool_use")),
        Con{J.kv("id", J.str(W.call_id_of(id))),
        Con{J.kv("name", J.str(name)),
        Con{J.kv("input", input_json(name, args)), Nil{}}}}})

def tool_result_json(+id: String, +content: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("tool_result")),
    Con{J.kv("tool_use_id", J.str(id)),
    Con{J.kv("content", W.img_content(Im.IAnth{}, content)), Nil{}}}})

# a user message: one text block, or the parts when it carries images
def user_blocks.pick(has: Bool, +content: String) -> List<&2, J.Json>:
  match has:
    case True{}:
      Im.content_blocks(Im.IAnth{}, content)
    case False{}:
      Con{W.text_block_json(content), Nil{}}

def user_blocks(+content: String) -> List<&2, J.Json>:
  user_blocks.pick(Im.has_image(content), content)

# ---- Anthropic: the message walk ----
# The wire roles do not map 1:1 onto Messages messages: the system
# message becomes the top-level system field, and a tool result rides
# a USER message (consecutive results merge into the same one, the
# vibe mapper's shape). The walk carries its routes in constructors
# and matches them on the next hop - no computed scrutinee, and the
# ids FIFO pairs each result with its call exactly as the OpenAI
# adapter does.

type AMsg is Data:
  AU{blocks: List<&2, J.Json>}
  AA{blocks: List<&2, J.Json>}

def user_head_of(acc: List<&2, AMsg>) -> Bool:
  match acc:
    case Nil{}:
      False{}
    case Con{+a, _}:
      match a:
        case AU{_}:
          True{}
        case AA{_}:
          False{}

def call_blocks(calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      Con{tool_use_json(c), call_blocks(rest)}

def asst_blocks.text(empty: Bool, +t: String,
    +calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  match empty:
    case True{}:
      call_blocks(calls)
    case False{}:
      Con{W.text_block_json(t), call_blocks(calls)}

def asst_blocks(+t: String, +calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  asst_blocks.text(String.is_empty(t), t, calls)

def anth_thinking_block(+reasoning: String, +sig: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("thinking")),
    Con{J.kv("thinking", J.str(reasoning)),
    Con{J.kv("signature", J.str(sig)), Nil{}}}})

# the thinking block of one span: a signed span replays verbatim, an
# unsigned one is dropped (the probed fallback, the model re-thinks)
def asst_seg_think.pick(is_replay: Bool, +reason: String, +sig: String)
    -> List<&2, J.Json>:
  match is_replay:
    case True{}:
      Con{anth_thinking_block(reason, sig), Nil{}}
    case False{}:
      Nil{}

def asst_seg_think(tm: W.ThinkMark) -> List<&2, J.Json>:
  match tm:
    case W.TM{+reason, +sig}:
      asst_seg_think.pick(Bool.and(Bool.not(String.is_empty(sig)),
          Bool.not(String.is_empty(reason))), reason, sig)

# the plain text between two spans: a whitespace-only gap (the LF the
# reply joins its blocks with) is no block at all - the Messages API
# rejects an empty text block
def asst_seg_text.pick(empty: Bool, +t: String) -> List<&2, J.Json>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Con{W.text_block_json(t), Nil{}}

def asst_seg_text(+t: String) -> List<&2, J.Json>:
  asst_seg_text.pick(String.is_empty(String.trim(t)), String.trim(t))

# EVERY span of the message, in order: one Anthropic reply can carry
# several thinking blocks (each with its own signature), and a span
# left in the text would ride as plain text - the provider refuses
# that as reasoning extraction (probed: stop_reason "refusal").
# The fuel bounds the walk; the span found ahead rides the next hop.
def asst_blocks_signed.go(fuel: Nat, pr: Maybe<&2, W.ThinkSpan>,
    +content: String, +calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      asst_blocks(W.asst_think.visible(content), calls)
    case 1n+k:
      match pr:
        case None{}:
          asst_blocks(String.trim(W.asst_think.visible(content)), calls)
        case Some{W.TSpan{+n0, +n1}}:
          +rest = W.asst_think.raw_visible(content, n1)
          List.append(&2, J.Json,
            asst_seg_text(String.take(content, n0)),
            List.append(&2, J.Json,
              asst_seg_think(W.think_split(W.asst_think.raw_span(content, n0, n1))),
              asst_blocks_signed.go(k, W.asst_think.find(rest), rest, calls)))

def asst_blocks_signed(pr: Maybe<&2, W.ThinkSpan>, +content: String,
    +calls: List<&2, W.WCall>) -> List<&2, J.Json>:
  match pr:
    case None{}:
      asst_blocks(content, calls)
    case Some{+sp}:
      asst_blocks_signed.go(64n, Some{sp}, content, calls)


def amsg_json(m: AMsg) -> J.Json:
  match m:
    case AU{+blocks}:
      J.obj(Con{J.kv("role", J.str("user")),
        Con{J.kv("content", J.arr(blocks)), Nil{}}})
    case AA{+blocks}:
      match blocks:
        case Nil{}:
          J.obj(Con{J.kv("role", J.str("assistant")),
            Con{J.kv("content", J.str("")), Nil{}}})
        case Con{_, _}:
          J.obj(Con{J.kv("role", J.str("assistant")),
            Con{J.kv("content", J.arr(blocks)), Nil{}}})

def amsgs_json.go(acc: List<&2, AMsg>) -> List<&2, J.Json>:
  match acc:
    case Nil{}:
      Nil{}
    case Con{+a, +rest}:
      Con{amsg_json(a), amsgs_json.go(rest)}

# the walk accumulates reversed; the final list is chronological
def amsgs_json(acc: List<&2, AMsg>) -> List<&2, J.Json>:
  amsgs_json.go(List.reverse(&2, AMsg, acc))

type AWalk is Data:
  AN{todo: List<&2, W.WMsg>, ids: List<&2, String>, acc: List<&2, AMsg>}
  AR{is_sys: Bool, is_user: Bool, is_asst: Bool, m: W.WMsg,
    todo: List<&2, W.WMsg>, ids: List<&2, String>, acc: List<&2, AMsg>}
  AT{user_head: Bool, acc: List<&2, AMsg>, ids: List<&2, String>,
    m: W.WMsg, todo: List<&2, W.WMsg>}

def awalk_go(fuel: Nat, w: AWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case AN{Nil{}, _, +acc}:
          amsgs_json(acc)
        case AN{Con{+m, +rest}, +ids, +acc}:
          match m:
            case W.WMsg{+role, +content, +calls}:
              awalk_go(k, AR{String.eq(role, "system"),
                String.eq(role, "user"), String.eq(role, "assistant"),
                m, rest, ids, acc})
        case AR{+is_sys, +is_user, +is_asst, +m, +todo, +ids, +acc}:
          match is_sys:
            case True{}:
              awalk_go(k, AN{todo, ids, acc})
            case False{}:
              match is_user:
                case True{}:
                  match m:
                    case W.WMsg{_, +content, _}:
                      awalk_go(k, AN{todo, ids,
                        Con{AU{user_blocks(content)}, acc}})
                case False{}:
                  match is_asst:
                    case True{}:
                      match m:
                        case W.WMsg{_, +content, +calls}:
                          awalk_go(k, AN{todo,
                            List.append(&2, String, ids, W.ids_of(calls)),
                            Con{AA{asst_blocks_signed(
                                W.asst_think.find(content),
                                  content, calls)}, acc}})
                    case False{}:
                      awalk_go(k, AT{user_head_of(acc), acc, ids, m, todo})
        case AT{+user_head, +acc, +ids, +m, +todo}:
          match user_head:
            case True{}:
              match acc:
                case Con{+last, +arest}:
                  match last:
                    case AU{+blocks}:
                      match ids:
                        case Con{+id, +irest}:
                          match m:
                            case W.WMsg{_, +content, _}:
                              awalk_go(k, AN{todo, irest,
                                Con{AU{List.append(&2, J.Json, blocks,
                                  Con{tool_result_json(id, content),
                                    Nil{}})}, arest}})
                        case Nil{}:
                          match m:
                            case W.WMsg{_, +content2, _}:
                              awalk_go(k, AN{todo, Nil{},
                                Con{AU{List.append(&2, J.Json, blocks,
                                  Con{tool_result_json("call_0", content2),
                                    Nil{}})}, arest}})
                    # user_head_of guarantees AU; AA here is unreachable
                    case AA{_}:
                      awalk_go(k, AN{todo, ids, acc})
                case Nil{}:
                  awalk_go(k, AN{todo, ids, acc})
            case False{}:
              match ids:
                case Con{+id2, +irest2}:
                  match m:
                    case W.WMsg{_, +content3, _}:
                      awalk_go(k, AN{todo, irest2,
                        Con{AU{Con{tool_result_json(id2, content3),
                          Nil{}}}, acc}})
                case Nil{}:
                  match m:
                    case W.WMsg{_, +content4, _}:
                      awalk_go(k, AN{todo, Nil{},
                        Con{AU{Con{tool_result_json("call_0", content4),
                          Nil{}}}, acc}})


# a tool message takes THREE hops (AN, AR, AT): the fuel covers that
# with room - an exhausted walk would return NO messages at all
# (probed: "messages: at least one message is required" past ~15 results)
def amsgs(+msgs: List<&2, W.WMsg>) -> List<&2, J.Json>:
  awalk_go(Nat.add(Nat.mul(List.length(&2, W.WMsg, msgs), 4n), 16n),
    AN{msgs, Nil{}, Nil{}})

# ---- Anthropic: the body ----

def wire_sys.is_sys(is_sys: Bool, +content: String) -> Maybe<&2, String>:
  match is_sys:
    case True{}:
      Some{content}
    case False{}:
      None{}

# the wire leads with the system prompt; it becomes the system field
def wire_sys(msgs: List<&2, W.WMsg>) -> Maybe<&2, String>:
  match msgs:
    case Nil{}:
      None{}
    case Con{+m, _}:
      match m:
        case W.WMsg{+role, +content, _}:
          wire_sys.is_sys(String.eq(role, "system"), content)

def api_body_anth.with_sys(sys: Maybe<&2, String>,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match sys:
    case Some{+s}:
      Con{J.kv("system", J.arr(Con{J.obj(Con{J.kv("type", J.str("text")),
        Con{J.kv("text", J.str(s)), Nil{}}}), Nil{}})), rest}
    case None{}:
      rest

# the thinking config the vibe CLI sends for opus-5.5 (thinking "high"):
# adaptive thinking, summarized display, the effort in output_config,
# and the adaptive max_tokens budget
def anth_thinking_json() -> J.Json:
  J.obj(Con{J.kv("type", J.str("adaptive")),
    Con{J.kv("display", J.str("summarized")), Nil{}}})

def anth_effort_json() -> J.Json:
  J.obj(Con{J.kv("effort", J.str("high")), Nil{}})

# the Messages API tool-name pattern is ^[a-zA-Z0-9_-]{1,128}$ - the
# self.* harness tools carry a dot and ride as self_* on the wire;
# the reply side restores the harness names (probed: the proxy
# rejects "self.reload" with invalid_request_error)
def anth_tool_name_out.pick2(is_compact: Bool, +name: String) -> String:
  match is_compact:
    case True{}:
      "self_compact"
    case False{}:
      name

def anth_tool_name_out.pick(is_reload: Bool, is_compact: Bool,
    +name: String) -> String:
  match is_reload:
    case True{}:
      "self_reload"
    case False{}:
      anth_tool_name_out.pick2(is_compact, name)

def anth_tool_name_out(+name: String) -> String:
  anth_tool_name_out.pick(String.eq(name, "self.reload"),
    String.eq(name, "self.compact"), name)

def anth_tool_name_in.pick2(is_compact: Bool, +name: String) -> String:
  match is_compact:
    case True{}:
      "self.compact"
    case False{}:
      name

def anth_tool_name_in.pick(is_reload: Bool, is_compact: Bool,
    +name: String) -> String:
  match is_reload:
    case True{}:
      "self.reload"
    case False{}:
      anth_tool_name_in.pick2(is_compact, name)

def anth_tool_name_in(+name: String) -> String:
  anth_tool_name_in.pick(String.eq(name, "self_reload"),
    String.eq(name, "self_compact"), name)

# the Anthropic tool shape: {name, description, input_schema} - no
# function wrapper, the schema rides as input_schema
def tool_json_anth(t: W.WTool) -> J.Json:
  match t:
    case W.WTool{+name, +desc}:
      J.obj(Con{J.kv("name", J.str(anth_tool_name_out(name))),
        Con{J.kv("description", J.str(desc)),
        Con{J.kv("input_schema", W.tool_schema(name)), Nil{}}}})

def tools_json_anth.link(rest_j: J.Json, +t: W.WTool) -> J.Json:
  match rest_j:
    case J.JArr{+vals}:
      J.JArr{Con{tool_json_anth(t), vals}}
    case _:
      rest_j

def tools_json_anth(ts: List<&2, W.WTool>) -> J.Json:
  match ts:
    case Nil{}:
      J.JArr{Nil{}}
    case Con{+t, rest}:
      tools_json_anth.link(tools_json_anth(rest), t)

def api_body_anth.pick(empty_tools: Bool, +model: String,
    +tools: List<&2, W.WTool>, +msgs: List<&2, W.WMsg>) -> String:
  match empty_tools:
    case True{}:
      J.stringify(J.obj(api_body_anth.with_sys(wire_sys(msgs),
        Con{J.kv("model", J.str(model)),
        Con{J.kv("max_tokens", J.num(32768n)),
        Con{J.kv("thinking", anth_thinking_json()),
        Con{J.kv("output_config", anth_effort_json()),
        Con{J.kv("messages", J.arr(amsgs(msgs))), Nil{}}}}}})))
    case False{}:
      J.stringify(J.obj(api_body_anth.with_sys(wire_sys(msgs),
        Con{J.kv("model", J.str(model)),
        Con{J.kv("max_tokens", J.num(32768n)),
        Con{J.kv("thinking", anth_thinking_json()),
        Con{J.kv("output_config", anth_effort_json()),
        Con{J.kv("messages", J.arr(amsgs(msgs))),
        Con{J.kv("tools", tools_json_anth(tools)),
        Con{J.kv("tool_choice",
          J.obj(Con{J.kv("type", J.str("auto")), Nil{}})),
        Nil{}}}}}}}})))

def api_body_anth(+model: String, +req: W.WReq) -> String:
  match req:
    case W.WReq{+tools, +msgs, _}:
      api_body_anth.pick(W.is_empty_tools(tools), model, tools, msgs)

# ---- Anthropic: the reply ----
# A Messages response carries content blocks: text blocks surface
# their text, thinking blocks surface their text wrapped in the think
# markers (the same transport the GLM adapter uses, so the TUI and
# the history need no change), and tool_use blocks become the CALL
# lines the Core parses - 1-based ids, the flat "arg" rehydrated, the
# structured inputs riding their JSON.

def anth_block.think(is_think: Bool, +b: J.Json) -> String:
  match is_think:
    case True{}:
      W.think_marked(J.get_str_or(b, "thinking", ""),
        J.get_str_or(b, "signature", ""))
    case False{}:
      ""

def anth_block.text(is_text: Bool, +b: J.Json) -> String:
  match is_text:
    case True{}:
      J.get_str_or(b, "text", "")
    case False{}:
      anth_block.think(String.eq(J.get_str_or(b, "type", ""), "thinking"),
        b)

def anth_block_text(+b: J.Json) -> String:
  anth_block.text(String.eq(J.get_str_or(b, "type", ""), "text"), b)

def anth_blocks_text(blocks: List<&2, J.Json>) -> String:
  match blocks:
    case Nil{}:
      ""
    case Con{+b, +rest}:
      String.append(anth_block_text(b),
        String.append("\n", anth_blocks_text(rest)))

# the flat tools send {"arg": ...}; a structured input rides as its
# JSON - what the wire args were before the adapter wrapped them
def anth_arg.arg(m: Maybe<&2, String>, +j: J.Json) -> String:
  match m:
    case Some{+s}:
      s
    case None{}:
      J.stringify(j)

def anth_arg(+j: J.Json) -> String:
  anth_arg.arg(J.get_str(j, "arg"), j)

def anth_tc_line(idx: Nat, +b: J.Json) -> String:
  String.append("CALL ",
    String.append(Nat.show(Nat.add(idx, 1n)),
      String.append(" ",
        String.append(anth_tool_name_in(J.get_str_or(b, "name", "")),
          String.append(" : ",
            String.trim(W.wire_encode(anth_arg(
              W.field_or(b, "input", J.JNull{})))))))))

# the kind of the NEXT head (a leaf helper: the loop cannot match a
# stale bind after its Bool dispatch - the p_replace.go idiom)
def anth_tc_next(rest: List<&2, J.Json>) -> Bool:
  match rest:
    case Nil{}:
      False{}
    case Con{+b2, _}:
      String.eq(J.get_str_or(b2, "type", ""), "tool_use")

def anth_tc_lines.go(blocks: List<&2, J.Json>, +i: Nat, is_tu: Bool)
    -> List<&2, String>:
  match blocks:
    case Nil{}:
      Nil{}
    case Con{+b, +rest}:
      match is_tu:
        case True{}:
          # the head is a tool_use: emit its line, the counter moves
          Con{anth_tc_line(i, b),
            anth_tc_lines.go(rest, Nat.add(i, 1n), anth_tc_next(rest))}
        case False{}:
          # not a tool_use: skip it, the id counter stands
          anth_tc_lines.go(rest, i, anth_tc_next(rest))

def anth_tc_lines(blocks: List<&2, J.Json>) -> List<&2, String>:
  match blocks:
    case Nil{}:
      Nil{}
    case Con{+b, _}:
      anth_tc_lines.go(blocks, 0n,
        String.eq(J.get_str_or(b, "type", ""), "tool_use"))

# a 200 that carries an error body surfaces as an error (the proxy
# answers overloads that way)
# a refusal carries no content: an empty reply would be retried as a
# null iteration until the limit; it surfaces as the provider's reason
def reply_anth.refusal(is_refusal: Bool, +j: J.Json) -> String:
  match is_refusal:
    case True{}:
      String.append("ERROR provider refusal: ",
        String.append(String.take(String.trim(
          J.get_str_or(W.field_or(j, "stop_details", J.JNull{}), "explanation",
            "the model refused")), 300n), "\nEND\n"))
    case False{}:
      +blocks = W.arr_field_or(j, "content")
      W.join_lines(Con{String.append("OK ",
          W.escape_nl(String.trim(anth_blocks_text(blocks)))),
        List.append(&2, String, anth_tc_lines(blocks),
          Con{"END", Nil{}})})

def reply_anth.err(is_err: Bool, +j: J.Json) -> String:
  match is_err:
    case True{}:
      String.append("ERROR provider 200: ",
        String.append(String.take(String.trim(
          J.get_str_or(W.field_or(j, "error", J.JNull{}), "message",
            "unknown error")), 300n), "\nEND\n"))
    case False{}:
      reply_anth.refusal(
        String.eq(J.get_str_or(j, "stop_reason", ""), "refusal"), j)

def reply_anth(+j: J.Json) -> String:
  reply_anth.err(String.eq(J.get_str_or(j, "type", ""), "error"), j)

def reply_anth_of.check(ok: Bool, +status: U32, +body: String,
    +j: J.Json) -> String:
  match ok:
    case True{}:
      reply_anth(j)
    case False{}:
      W.reply_err(status, body)

def reply_anth_of(+status: U32, +body: String) -> String:
  reply_anth_of.check(U32.is_eq(status, 200), status, body, W.parse_json(body))