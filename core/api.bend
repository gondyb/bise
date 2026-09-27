# core/api.bend — the provider API adapter, pure Bend.
#
# Converts the harness wire request (MODEL/TOOL/MSG/CALL lines, built by
# runtime/remote.bend and pinned by LAWS.bend) into an OpenAI-compatible
# chat-completions JSON body, and a provider response back into the
# OK/CALL/END reply the Core parses (runtime/remote.bend parse_reply).
# This is the port of what the Rust bridge used to do: the mapping now
# lives in pure Bend and is pinned by LAWS.bend.
#
# Tool results pair with assistant calls FIFO on the ids ("call_<n>"),
# exactly as the bridge did; GLM content arrives as an array of blocks
# whose text fields are joined.
import Base
import ./compaction.bend as C
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J

# ---- single-line flattening (every reply line is one line) ----

def newline.get(m: Maybe<&2, Char>) -> Char:
  match m:
    case Some{c}:
      c
    case None{}:
      Char.from_u32(32)

def newline() -> Char:
  newline.get(String.get("\n", 0n))

def flatten(+s: String) -> String:
  String.join(String.split(s, newline()), " ")

# ---- newline escaping: the wire is single-line, markdown is multi-line ----
# Real newlines in a provider text travel as the two chars backslash-n;
# the TUI unescapes for rendering, and parse_wire unescapes message
# contents so the API JSON carries real newlines again. One
# self-recursive def per direction (state in Data, fuel first).

type ENl is Data:
  EGo{s: String}
  EStep{is_nl: Bool, c: U32, t: String}

def escape_nl.go(fuel: Nat, st: ENl, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case EGo{SNil{}}:
          String.reverse(acc)
        case EGo{SCon{Chr{+c}, t}}:
          escape_nl.go(k, EStep{U32.is_eq(c, 10), c, t}, acc)
        case EStep{True{}, _, t}:
          escape_nl.go(k, EGo{t}, SCon{Chr{110}, SCon{Chr{92}, acc}})
        case EStep{False{}, +c, t}:
          escape_nl.go(k, EGo{t}, SCon{Chr{c}, acc})

def str_len(s: String) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{_, t}:
      Nat.add(str_len(t), 1n)

def escape_nl(+s: String) -> String:
  escape_nl.go(Nat.add(Nat.mul(str_len(s), 2n), 16n), EGo{s}, SNil{})

type UNl is Data:
  UGo{s: String}
  UStep{is_bs: Bool, c: U32, t: String}
  UNl{t: String}
  UPair{is_n: Bool, c2: U32, t2: String}

def unescape_nl.go(fuel: Nat, st: UNl, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case UGo{SNil{}}:
          String.reverse(acc)
        case UGo{SCon{Chr{+c}, t}}:
          unescape_nl.go(k, UStep{U32.is_eq(c, 92), c, t}, acc)
        case UStep{True{}, _, t}:
          unescape_nl.go(k, UNl{t}, acc)
        case UStep{False{}, +c, t}:
          unescape_nl.go(k, UGo{t}, SCon{Chr{c}, acc})
        case UNl{SCon{Chr{+c2}, t2}}:
          unescape_nl.go(k, UPair{U32.is_eq(c2, 110), c2, t2}, acc)
        case UNl{SNil{}}:
          unescape_nl.go(k, UGo{SNil{}}, SCon{Chr{92}, acc})
        case UPair{True{}, _, t2}:
          unescape_nl.go(k, UGo{t2}, SCon{Chr{10}, acc})
        case UPair{False{}, +c2, t2}:
          unescape_nl.go(k, UGo{t2}, SCon{Chr{c2}, SCon{Chr{92}, acc}})

def unescape_nl(+s: String) -> String:
  unescape_nl.go(Nat.add(Nat.mul(str_len(s), 2n), 16n), UGo{s}, SNil{})

# ---- wire arg encoding: reversible, unlike escape_nl ----
# escape_nl is for markdown text; a literal backslash-n in that text is
# indistinguishable from an encoded newline after a round-trip. Tool
# args are commands and JSON text full of backslashes, so the arg
# encoding must decode to exactly what was encoded: LF becomes
# backslash capital N, CR backslash capital R, backslash doubled.
# A literal sentinel sequence encodes with a doubled backslash and
# decodes back. The capital letters keep legacy wire content (which
# never carries them) intact through the decode.

type WEnc is Data:
  WEGo{s: String}
  WEPickLf{is_lf: Bool, c: U32, t: String}
  WEPickBs{is_bs: Bool, c: U32, t: String}
  WEPickCr{is_cr: Bool, c: U32, t: String}
  WEBS{t: String}
  WELF{t: String}
  WECR{t: String}
  WEOther{c: U32, t: String}

def wire_encode.go(fuel: Nat, st: WEnc, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case WEGo{SNil{}}:
          String.reverse(acc)
        case WEGo{SCon{Chr{+c}, t}}:
          wire_encode.go(k, WEPickLf{U32.is_eq(c, 10), c, t}, acc)
        case WEPickLf{True{}, _, t}:
          wire_encode.go(k, WELF{t}, acc)
        case WEPickLf{False{}, +c, t}:
          wire_encode.go(k, WEPickBs{U32.is_eq(c, 92), c, t}, acc)
        case WEPickBs{True{}, _, t}:
          wire_encode.go(k, WEBS{t}, acc)
        case WEPickBs{False{}, +c, t}:
          wire_encode.go(k, WEPickCr{U32.is_eq(c, 13), c, t}, acc)
        case WEPickCr{True{}, _, t}:
          wire_encode.go(k, WECR{t}, acc)
        case WEPickCr{False{}, +c, t}:
          wire_encode.go(k, WEOther{c, t}, acc)
        case WEBS{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{92}, SCon{Chr{92}, acc}})
        case WELF{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{78}, SCon{Chr{92}, acc}})
        case WECR{t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{82}, SCon{Chr{92}, acc}})
        case WEOther{+c, t}:
          wire_encode.go(k, WEGo{t}, SCon{Chr{c}, acc})

def wire_encode(+s: String) -> String:
  wire_encode.go(Nat.add(Nat.mul(str_len(s), 8n), 64n), WEGo{s}, SNil{})

type WDec is Data:
  WDGo{s: String}
  WDPick{is_bs: Bool, c: U32, t: String}
  WDBS{s: String}
  WDBSPair{is_bs2: Bool, c2: U32, t2: String}
  WDBSN{is_n: Bool, c2: U32, t2: String}
  WDBSR{is_r: Bool, c2: U32, t2: String}
  WDEmit{c: U32, t: String}
  WDLeg{c2: U32, t2: String}

def wire_decode.go(fuel: Nat, st: WDec, acc: String) -> String:
  match fuel:
    case 0n:
      String.reverse(acc)
    case 1n+k:
      match st:
        case WDGo{SNil{}}:
          String.reverse(acc)
        case WDGo{SCon{Chr{+c}, t}}:
          wire_decode.go(k, WDPick{U32.is_eq(c, 92), c, t}, acc)
        case WDPick{True{}, _, t}:
          wire_decode.go(k, WDBS{t}, acc)
        case WDPick{False{}, +c, t}:
          wire_decode.go(k, WDEmit{c, t}, acc)
        case WDBS{SNil{}}:
          String.reverse(SCon{Chr{92}, acc})
        case WDBS{SCon{Chr{+c2}, t2}}:
          wire_decode.go(k, WDBSPair{U32.is_eq(c2, 92), c2, t2}, acc)
        case WDBSPair{True{}, _, t2}:
          wire_decode.go(k, WDEmit{92, t2}, acc)
        case WDBSPair{False{}, +c2, t2}:
          wire_decode.go(k, WDBSN{U32.is_eq(c2, 78), c2, t2}, acc)
        case WDBSN{True{}, _, t2}:
          wire_decode.go(k, WDEmit{10, t2}, acc)
        case WDBSN{False{}, +c2, t2}:
          wire_decode.go(k, WDBSR{U32.is_eq(c2, 82), c2, t2}, acc)
        case WDBSR{True{}, _, t2}:
          wire_decode.go(k, WDEmit{13, t2}, acc)
        case WDBSR{False{}, +c2, t2}:
          wire_decode.go(k, WDLeg{c2, t2}, acc)
        case WDLeg{+c2, t2}:
          wire_decode.go(k, WDGo{t2}, SCon{Chr{c2}, SCon{Chr{92}, acc}})
        case WDEmit{+c, t}:
          wire_decode.go(k, WDGo{t}, SCon{Chr{c}, acc})

def wire_decode(+s: String) -> String:
  wire_decode.go(Nat.add(Nat.mul(str_len(s), 8n), 64n), WDGo{s}, SNil{})

# ---- the parsed wire request ----

type WCall is Data:
  WCall{id: String, name: String, args: String}

type WMsg is Data:
  WMsg{role: String, content: String, calls: List<&2, WCall>}

type WTool is Data:
  WTool{name: String, desc: String}

# cur is the message whose CALL sub-lines are still arriving
type WReq is Data:
  WReq{tools: List<&2, WTool>, msgs: List<&2, WMsg>, cur: Maybe<&2, WMsg>}

# ---- wire request parsing (the mirror of remote.bend's serialization) ----

def parse_tool.cut(sp: Maybe<&2, Nat>, +rest: String) -> WTool:
  match sp:
    case Some{+p}:
      WTool{String.take(rest, p), String.drop(rest, Nat.add(p, 3n))}
    case None{}:
      WTool{rest, ""}

def parse_tool(+rest: String) -> WTool:
  parse_tool.cut(C.find(rest, " : ", 0n), rest)

def parse_call.name(sp: Maybe<&2, Nat>, +id: String, +tail: String) -> WCall:
  match sp:
    case Some{+q}:
      WCall{id, String.take(tail, q), wire_decode(String.drop(tail, Nat.add(q, 3n)))}
    case None{}:
      WCall{id, tail, ""}

def parse_call.id(sp: Maybe<&2, Nat>, +line: String) -> WCall:
  match sp:
    case Some{+p}:
      parse_call.name(C.find(String.drop(line, Nat.add(p, 1n)), " : ", 0n),
        String.take(line, p), String.drop(line, Nat.add(p, 1n)))
    case None{}:
      WCall{"0", line, ""}

def parse_call.body(+line: String) -> WCall:
  parse_call.id(C.find(line, " ", 0n), line)

def parse_call(+line: String) -> WCall:
  parse_call.body(String.drop(line, 7n))

def add_call(cur: Maybe<&2, WMsg>, +line: String) -> Maybe<&2, WMsg>:
  match cur:
    case Some{+m}:
      match m:
        case WMsg{+role, +content, +calls}:
          Some{WMsg{role, content, Con{parse_call(line), calls}}}
    case None{}:
      cur

def push_msg(cur: Maybe<&2, WMsg>, +msgs: List<&2, WMsg>) -> List<&2, WMsg>:
  match cur:
    case Some{+m}:
      Con{m, msgs}
    case None{}:
      msgs

def start_msg.cut(sp: Maybe<&2, Nat>, req: WReq, cur: Maybe<&2, WMsg>,
    +rest: String) -> WReq:
  match sp:
    case Some{+p}:
      match req:
        case WReq{+tools, +msgs, _}:
          WReq{tools, push_msg(cur, msgs),
            Some{WMsg{String.take(rest, p),
              unescape_nl(String.drop(rest, Nat.add(p, 3n))),
              Nil{}}}}
    case None{}:
      req

def start_msg(req: WReq, cur: Maybe<&2, WMsg>, +rest: String) -> WReq:
  start_msg.cut(C.find(rest, " : ", 0n), req, cur, rest)

def cur_of(req: WReq) -> Maybe<&2, WMsg>:
  match req:
    case WReq{_, _, +cur}:
      cur

def add_tool(req: WReq, +line: String) -> WReq:
  match req:
    case WReq{+tools, +msgs, +cur}:
      WReq{Con{parse_tool(String.drop(line, 5n)), tools}, msgs, cur}

def step_tool(is_tool: Bool, +req: WReq, +line: String) -> WReq:
  match is_tool:
    case True{}:
      add_tool(req, line)
    case False{}:
      req
def step_msg(is_msg: Bool, +req: WReq, +line: String) -> WReq:
  match is_msg:
    case True{}:
      start_msg(req, cur_of(req), String.drop(line, 4n))
    case False{}:
      step_tool(String.starts_with(line, "TOOL "), req, line)
def step_call(is_call: Bool, +req: WReq, +line: String) -> WReq:
  match is_call:
    case True{}:
      match req:
        case WReq{+tools, +msgs, +cur}:
          WReq{tools, msgs, add_call(cur, line)}
    case False{}:
      step_msg(String.starts_with(line, "MSG "), req, line)
def step(+req: WReq, +line: String) -> WReq:
  step_call(String.starts_with(line, "  CALL "), req, line)


def parse_finish(req: WReq) -> WReq:
  match req:
    case WReq{+tools, +msgs, +cur}:
      WReq{tools, List.reverse(&2, WMsg, push_msg(cur, msgs)), None{}}

def parse_go(lines: List<&2, String>, +req: WReq) -> WReq:
  match lines:
    case Nil{}:
      parse_finish(req)
    case Con{+line, rest}:
      parse_go(rest, step(req, line))

def parse_wire(+request: String) -> WReq:
  parse_go(String.split(request, newline()), WReq{Nil{}, Nil{}, None{}})

# ---- wire request -> API JSON body ----

def call_id_of(+id: String) -> String:
  String.append("call_", id)

def arg_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties",
      J.obj(Con{J.kv("arg", J.obj(Con{J.kv("type", J.str("string")), Nil{}})),
        Nil{}})),
      Con{J.kv("required", J.arr(Con{J.str("arg"), Nil{}})), Nil{}}}})

# ---- per-tool parameter schemas (exact vibe_sdk schemas) ----
# run_typescript: {"code": string} (deny_unknown_fields)
# search_tool_functions: the SearchRequest schema (mode/query/
# functions/functionNames/connectors)

def code_schema() -> J.Json:
  J.obj(
    Con{J.kv("type",
        J.str("object")),
      Con{J.kv("additionalProperties",
          J.JBool{False{}}),
        Con{J.kv("properties",
            J.obj(
              Con{J.kv("code",
                  J.obj(
                    Con{J.kv("type",
                        J.str("string")),
                      Nil{}})),
                Nil{}})),
          Con{J.kv("required",
              J.arr(
                Con{
                  J.str("code"),
                  Nil{}})),
            Nil{}}}}})

def search_schema() -> J.Json:
  J.obj(
    Con{J.kv("type",
        J.str("object")),
      Con{J.kv("additionalProperties",
          J.JBool{False{}}),
        Con{J.kv("properties",
            J.obj(
              Con{J.kv("mode",
                  J.obj(
                    Con{J.kv("type",
                        J.str("string")),
                      Con{J.kv("enum",
                          J.arr(
                            Con{
                              J.str("best_match"),
                              Con{
                                J.str("details"),
                                Con{
                                  J.str("all_connector_capabilities"),
                                  Nil{}}}})),
                        Con{J.kv("description",
                            J.str("best_match returns ranked names; details returns declarations for exact functions; all_connector_capabilities lists exact connector inventories.")),
                          Con{J.kv("default",
                              J.str("best_match")),
                            Nil{}}}}})),
                Con{J.kv("query",
                    J.obj(
                      Con{J.kv("type",
                          J.str("string")),
                        Con{J.kv("description",
                            J.str("Rewrite the needed operation into short tool keywords, e.g. \"list issues\"; do not pass the full user request.")),
                          Nil{}}})),
                  Con{J.kv("functions",
                      J.obj(
                        Con{J.kv("type",
                            J.str("array")),
                          Con{J.kv("items",
                              J.obj(
                                Con{J.kv("type",
                                    J.str("string")),
                                  Con{J.kv("minLength",
                                      J.JNum{1n}),
                                    Nil{}}})),
                            Con{J.kv("description",
                                J.str("Exact {connector}.{function} names for details; required in details mode.")),
                              Con{J.kv("maxItems",
                                  J.JNum{10n}),
                                Con{J.kv("default",
                                    J.arr(
                                      Nil{})),
                                  Nil{}}}}}})),
                    Con{J.kv("functionNames",
                        J.obj(
                          Con{J.kv("type",
                              J.str("array")),
                            Con{J.kv("items",
                                J.obj(
                                  Con{J.kv("type",
                                      J.str("string")),
                                    Con{J.kv("minLength",
                                        J.JNum{1n}),
                                      Nil{}}})),
                              Con{J.kv("description",
                                  J.str("Deprecated alias for functions.")),
                                Con{J.kv("maxItems",
                                    J.JNum{10n}),
                                  Nil{}}}}})),
                      Con{J.kv("connectors",
                          J.obj(
                            Con{J.kv("type",
                                J.str("array")),
                              Con{J.kv("items",
                                  J.obj(
                                    Con{J.kv("type",
                                        J.str("string")),
                                      Con{J.kv("minLength",
                                          J.JNum{1n}),
                                        Nil{}}})),
                                Con{J.kv("description",
                                    J.str("Exact connector names for best_match/all_connector_capabilities; unused in details.")),
                                  Con{J.kv("maxItems",
                                      J.JNum{10n}),
                                    Con{J.kv("default",
                                        J.arr(
                                          Nil{})),
                                      Nil{}}}}}})),
                        Nil{}}}}}})),
          Nil{}}}})

def tool_schema.rt(is_rt: Bool, +name: String) -> J.Json:
  match is_rt:
    case True{}:
      code_schema()
    case False{}:
      arg_schema()

def tool_schema.search(is_search: Bool, +name: String) -> J.Json:
  match is_search:
    case True{}:
      search_schema()
    case False{}:
      tool_schema.rt(String.eq(name, "run_typescript"), name)

def tool_schema(+name: String) -> J.Json:
  tool_schema.search(String.eq(name, "search_tool_functions"), name)

def tool_fn_json(+name: String, +desc: String) -> J.Json:
  J.obj(Con{J.kv("name", J.str(name)),
    Con{J.kv("description", J.str(desc)),
      Con{J.kv("parameters", tool_schema(name)), Nil{}}}})

def tool_json(t: WTool) -> J.Json:
  match t:
    case WTool{+name, +desc}:
      J.obj(Con{J.kv("type", J.str("function")),
        Con{J.kv("function", tool_fn_json(name, desc)), Nil{}}})

def tools_json.link(rest_j: J.Json, +t: WTool) -> J.Json:
  match rest_j:
    case J.JArr{+vals}:
      J.JArr{Con{tool_json(t), vals}}
    case _:
      rest_j

def tools_json(ts: List<&2, WTool>) -> J.Json:
  match ts:
    case Nil{}:
      J.JArr{Nil{}}
    case Con{+t, rest}:
      tools_json.link(tools_json(rest), t)

def args_json.structured(is_structured: Bool, +args: String) -> String:
  match is_structured:
    case True{}:
      args
    case False{}:
      J.stringify(J.obj(Con{J.kv("arg", J.str(args)), Nil{}}))

def args_json(+name: String, +args: String) -> String:
  args_json.structured(
    Bool.or(String.eq(name, "run_typescript"),
      String.eq(name, "search_tool_functions")),
    args)

def call_json(c: WCall) -> J.Json:
  match c:
    case WCall{+id, +name, +args}:
      J.obj(Con{J.kv("id", J.str(call_id_of(id))),
        Con{J.kv("type", J.str("function")),
          Con{J.kv("function",
            J.obj(Con{J.kv("name", J.str(name)),
              Con{J.kv("arguments", J.str(args_json(name, args))), Nil{}}})),
            Nil{}}}})

def calls_json.link(rest_j: J.Json, +c: WCall) -> J.Json:
  match rest_j:
    case J.JArr{+vals}:
      J.JArr{Con{call_json(c), vals}}
    case _:
      rest_j

def calls_json(calls: List<&2, WCall>) -> J.Json:
  match calls:
    case Nil{}:
      J.JArr{Nil{}}
    case Con{+c, rest}:
      calls_json.link(calls_json(rest), c)

def ids_of(calls: List<&2, WCall>) -> List<&2, String>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match c:
        case WCall{+id, _, _}:
          Con{call_id_of(id), ids_of(rest)}

# tool results pair with assistant calls FIFO: each tool message takes
# the oldest unmatched id
def msg_json.tool(ids: List<&2, String>, +content: String)
    -> J.Json & List<&2, String>:
  match ids:
    case Con{+id, rest}:
      (J.obj(Con{J.kv("role", J.str("tool")),
        Con{J.kv("content", J.str(content)),
          Con{J.kv("tool_call_id", J.str(id)), Nil{}}}}), rest)
    case Nil{}:
      (J.obj(Con{J.kv("role", J.str("tool")),
        Con{J.kv("content", J.str(content)),
          Con{J.kv("tool_call_id", J.str("call_0")), Nil{}}}}), Nil{})

def content_json.pick(empty: Bool, +text: String) -> J.Json:
  match empty:
    case True{}:
      J.JNull{}
    case False{}:
      J.str(text)

def content_json(+text: String) -> J.Json:
  content_json.pick(String.is_empty(text), text)

# ---- the way back: marked history reasoning becomes a thinking block ----
# The vibe CLI sends the assistant's own reasoning back as a structured
# thinking block ahead of the text block (mistral.py prepare_message ->
# ThinkChunk). The think markers are the transport INSIDE the harness
# wire/history; they are NEVER sent literally to the API. An assistant
# message without markers is sent as a plain string, exactly as before.

type ThinkSpan is Data:
  TSpan{n0: Nat, n1: Nat}

# the search ran on the tail past the start marker (C.find takes a
# position accumulator, not an offset), so the hit is RELATIVE: rebase
# it to the absolute end-marker position
def asst_think.end(m: Maybe<&2, Nat>, +content: String, +n0: Nat)
    -> Maybe<&2, ThinkSpan>:
  match m:
    case Some{+r}:
      Some{TSpan{n0, Nat.add(Nat.add(n0, 7n), r)}}
    case None{}:
      None{}

def asst_think.start(m: Maybe<&2, Nat>, +content: String)
    -> Maybe<&2, ThinkSpan>:
  match m:
    case Some{+n0}:
      asst_think.end(C.find(String.drop(content, Nat.add(n0, 7n)), "</think>", 0n), content, n0)
    case None{}:
      None{}

# the marker span: (start, end) of "<think>R</think>" in the text
def asst_think.find(+content: String) -> Maybe<&2, ThinkSpan>:
  asst_think.start(C.find(content, "<think>", 0n), content)

def asst_think.reasoning(+content: String, +n0: Nat, +n1: Nat) -> String:
  String.trim(String.take(String.drop(content, Nat.add(n0, 7n)),
    Nat.sub(n1, Nat.add(n0, 7n))))

def asst_think.visible.strip(is_nl: Bool, +rest: String) -> String:
  match is_nl:
    case True{}:
      String.drop(rest, 1n)
    case False{}:
      rest

def asst_think.visible(+rest: String) -> String:
  asst_think.visible.strip(String.starts_with(rest, "\n"), rest)

# the exact vibe ThinkChunk shape: the thinking block first, the text
# block second (a thinking-only assistant message carries no text block)
def think_block_json(+reasoning: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("thinking")),
    Con{J.kv("thinking",
      J.arr(Con{J.obj(Con{J.kv("type", J.str("text")),
        Con{J.kv("text", J.str(reasoning)), Nil{}}}), Nil{}})),
    Nil{}}})

def text_block_json(+text: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("text")),
    Con{J.kv("text", J.str(text)), Nil{}}})

def asst_content.marked.pick(empty_vis: Bool, +reasoning: String,
    +visible: String) -> J.Json:
  match empty_vis:
    case True{}:
      J.arr(Con{think_block_json(reasoning), Nil{}})
    case False{}:
      J.arr(Con{think_block_json(reasoning),
        Con{text_block_json(visible), Nil{}}})

def asst_think.raw_visible(+content: String, +n1: Nat) -> String:
  String.drop(content, Nat.add(n1, 8n))

def asst_content.found(pr: ThinkSpan, +content: String) -> J.Json:
  match pr:
    case TSpan{+n0, +n1}:
      asst_content.marked.pick(String.is_empty(asst_think.visible(
          asst_think.raw_visible(content, n1))),
        asst_think.reasoning(content, n0, n1),
        asst_think.visible(asst_think.raw_visible(content, n1)))

def asst_content.pick(parts: Maybe<&2, ThinkSpan>, +content: String) -> J.Json:
  match parts:
    case None{}:
      content_json(content)
    case Some{+pr}:
      asst_content.found(pr, content)

def asst_content_json(+content: String) -> J.Json:
  asst_content.pick(asst_think.find(content), content)

def msg_json.asst(+content: String, +calls: List<&2, WCall>,
    ids: List<&2, String>) -> J.Json & List<&2, String>:
  match calls:
    case Nil{}:
      (J.obj(Con{J.kv("role", J.str("assistant")),
        Con{J.kv("content", asst_content_json(content)), Nil{}}}), ids)
    case Con{_, _}:
      (J.obj(Con{J.kv("role", J.str("assistant")),
        Con{J.kv("content", asst_content_json(content)),
          Con{J.kv("tool_calls", calls_json(calls)), Nil{}}}}),
        List.append(&2, String, ids, ids_of(calls)))

def msg_json.pick(is_tool: Bool, is_asst: Bool, +role: String,
    +content: String, +calls: List<&2, WCall>, ids: List<&2, String>)
    -> J.Json & List<&2, String>:
  match is_tool:
    case True{}:
      msg_json.tool(ids, content)
    case False{}:
      match is_asst:
        case True{}:
          msg_json.asst(content, calls, ids)
        case False{}:
          (J.obj(Con{J.kv("role", J.str(role)),
            Con{J.kv("content", J.str(content)), Nil{}}}), ids)

def msg_json(m: WMsg, ids: List<&2, String>) -> J.Json & List<&2, String>:
  match m:
    case WMsg{+role, +content, +calls}:
      msg_json.pick(String.eq(role, "tool"), String.eq(role, "assistant"),
        role, content, calls, ids)

# the message walk is one self-recursive def over a Data state (the
# json package's parser pattern): computed values travel inside
# constructors and are matched on the next hop, as parameters
type MsgPair is Data:
  MP{j: J.Json, ids2: List<&2, String>}

type MWalk is Data:
  MNext{todo: List<&2, WMsg>, ids: List<&2, String>, acc: List<&2, J.Json>}
  MPair{pair: MsgPair, todo: List<&2, WMsg>, acc: List<&2, J.Json>}

def is_empty_tools(ts: List<&2, WTool>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

def msgs_len(ms: List<&2, WMsg>) -> Nat:
  match ms:
    case Nil{}:
      0n
    case Con{_, rest}:
      Nat.add(msgs_len(rest), 1n)

def pair_of.wrap(t: J.Json & List<&2, String>) -> MsgPair:
  match t:
    case (j, ids2):
      MP{j, ids2}

def pair_of(m: WMsg, ids: List<&2, String>) -> MsgPair:
  pair_of.wrap(msg_json(m, ids))

def msgs_go(fuel: Nat, w: MWalk) -> List<&2, J.Json>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case MNext{Nil{}, _, +acc}:
          List.reverse(&2, J.Json, acc)
        case MNext{Con{+m, +rest}, +ids, +acc}:
          msgs_go(k, MPair{pair_of(m, ids), rest, acc})
        case MPair{+pair, +todo, +acc}:
          match pair:
            case MP{+j, +ids2}:
              msgs_go(k, MNext{todo, ids2, Con{j, acc}})

def msgs_json(+msgs: List<&2, WMsg>) -> List<&2, J.Json>:
  msgs_go(Nat.add(Nat.mul(msgs_len(msgs), 2n), 16n),
    MNext{msgs, Nil{}, Nil{}})

# reasoning_effort rides EVERY call, the vibe CLI parity: the active
# model zai-glm-5-3 runs with thinking "high", and the vibe CLI maps
# thinking medium/high/max to reasoning_effort "high" on the Mistral
# API (core/llm/backend/mistral.py _THINKING_TO_REASONING_EFFORT)
def api_body.pick(empty_tools: Bool, +model: String, +tools: List<&2, WTool>,
    +msgs: List<&2, WMsg>) -> String:
  match empty_tools:
    case True{}:
      # a compaction call offers no tools at all
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        Con{J.kv("reasoning_effort", J.str("high")),
        Con{J.kv("messages", J.arr(msgs_json(msgs))), Nil{}}}}))
    case False{}:
      J.stringify(J.obj(Con{J.kv("model", J.str(model)),
        Con{J.kv("reasoning_effort", J.str("high")),
        Con{J.kv("messages", J.arr(msgs_json(msgs))),
          Con{J.kv("tools", tools_json(tools)),
            Con{J.kv("tool_choice", J.str("auto")), Nil{}}}}}}))

def api_body(+model: String, +req: WReq) -> String:
  match req:
    case WReq{+tools, +msgs, _}:
      api_body.pick(is_empty_tools(tools), model, tools, msgs)


# ---- provider response -> the OK/CALL/END reply (remote.bend parses it) ----

def parsed_or(r: Result<&2, &2, String, J.Json>) -> J.Json:
  match r:
    case Done{+j}:
      j
    case Fail{_}:
      J.JNull{}

def parse_json(+body: String) -> J.Json:
  parsed_or(J.parse(body))

def field_or.found(m: Maybe<&2, J.Json>, dflt: J.Json) -> J.Json:
  match m:
    case Some{+v}:
      v
    case None{}:
      dflt

def field_or(j: J.Json, +key: String, dflt: J.Json) -> J.Json:
  field_or.found(J.get(j, key), dflt)

def arr_field_or.arr(v: J.Json) -> List<&2, J.Json>:
  match v:
    case J.JArr{+vals}:
      vals
    case _:
      Nil{}

def arr_field_or.found(m: Maybe<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+v}:
      arr_field_or.arr(v)
    case None{}:
      Nil{}

def arr_field_or(j: J.Json, +key: String) -> List<&2, J.Json>:
  arr_field_or.found(J.get(j, key))

def fn_of(tc: J.Json) -> J.Json:
  field_or(tc, "function", J.JNull{})

def arg_of.parsed(r: Result<&2, &2, String, J.Json>, +raw: String) -> String:
  match r:
    case Done{+j}:
      J.get_str_or(j, "arg", raw)
    case Fail{_}:
      raw

def arg_of(+raw: String) -> String:
  arg_of.parsed(J.parse(raw), raw)

# extract "code" from structured JSON tool arguments (run_typescript);
# pass through if the args are not JSON or have no code field
def call_code.res(r: Result<&2, &2, String, J.Json>, +raw: String) -> String:
  match r:
    case Done{+j}:
      J.get_str_or(j, "code", raw)
    case Fail{_}:
      raw

def call_code(+args: String) -> String:
  call_code.res(J.parse(args), args)

def tc_line(idx: Nat, tc: J.Json) -> String:
  +fn = fn_of(tc)
  String.append("CALL ",
    String.append(Nat.show(Nat.add(idx, 1n)),
      String.append(" ",
        String.append(J.get_str_or(fn, "name", ""),
          String.append(" : ",
            String.trim(wire_encode(arg_of(J.get_str_or(fn, "arguments", "")))))))))

def tc_lines.go(tcs: List<&2, J.Json>, +i: Nat) -> List<&2, String>:
  match tcs:
    case Nil{}:
      Nil{}
    case Con{+tc, rest}:
      Con{tc_line(i, tc), tc_lines.go(rest, Nat.add(i, 1n))}

def tc_lines(tcs: List<&2, J.Json>) -> List<&2, String>:
  tc_lines.go(tcs, 0n)

# the texts of one thinking block parts, newline-joined
def think_texts(parts: List<&2, J.Json>) -> String:
  match parts:
    case Nil{}:
      ""
    case Con{+p, rest}:
      String.append(J.get_str_or(p, "text", ""),
        String.append("\n", think_texts(rest)))

def block_think.has(has: Bool, +b: J.Json) -> String:
  match has:
    case True{}:
      String.append("<think>",
        String.append(think_texts(arr_field_or(b, "thinking")),
          "</think>"))
    case False{}:
      ""

def block_think(has: Bool, +b: J.Json) -> String:
  block_think.has(has, b)

def block_text.pick(is_text: Bool, has_think: Bool, +b: J.Json) -> String:
  match is_text:
    case True{}:
      J.get_str_or(b, "text", "")
    case False{}:
      block_think(has_think, b)

# one GLM content block: a text block surfaces its text; a thinking
# block carries its parts in a nested thinking array -- they surface
# wrapped in think markers, so the model keeps its own reasoning in the
# history it sees back (GLM native reasoning convention; dropping it
# here is the bug that made every multi-tool turn reason blind)
def block_text(+b: J.Json) -> String:
  block_text.pick(Bool.not(String.is_empty(J.get_str_or(b, "text", ""))),
    Bool.not(List.is_empty(&2, J.Json, arr_field_or(b, "thinking"))), b)

def blocks_text(blocks: List<&2, J.Json>) -> String:
  match blocks:
    case Nil{}:
      ""
    case Con{+b, rest}:
      String.append(block_text(b),
        String.append("\n", blocks_text(rest)))

# GLM returns content as an array of blocks; the blocks are joined
def content_of(j: J.Json) -> String:
  match j:
    case J.JStr{+s}:
      s
    case J.JArr{+blocks}:
      String.trim(blocks_text(blocks))
    case _:
      ""

def choice_msg.found(m: Maybe<&2, J.Json>) -> J.Json:
  match m:
    case Some{+c0}:
      field_or(c0, "message", J.JNull{})
    case None{}:
      J.JNull{}

def choice_msg(j: J.Json) -> J.Json:
  choice_msg.found(J.arr_get(field_or(j, "choices", J.JNull{}), 0n))

def join_lines(lines: List<&2, String>) -> String:
  match lines:
    case Nil{}:
      ""
    case Con{+l, rest}:
      String.append(l, String.append("\n", join_lines(rest)))

def reply_ok(+msg: J.Json) -> String:
  +content = content_of(field_or(msg, "content", J.JNull{}))
  +calls = tc_lines(arr_field_or(msg, "tool_calls"))
  join_lines(Con{String.append("OK ", escape_nl(content)),
    List.append(&2, String, calls, Con{"END", Nil{}})})

def reply_err(+status: U32, +body: String) -> String:
  String.append("ERROR provider ",
    String.append(U32.show(status),
      String.append(": ",
        String.append(String.take(String.trim(flatten(body)), 300n), "\nEND\n"))))

def reply_of.check(ok: Bool, +status: U32, +body: String, +j: J.Json) -> String:
  match ok:
    case True{}:
      reply_ok(choice_msg(j))
    case False{}:
      reply_err(status, body)

def reply_of(+status: U32, +body: String) -> String:
  reply_of.check(U32.is_eq(status, 200), status, body, parse_json(body))
