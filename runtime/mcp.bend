# runtime/mcp.bend — the Mistral MCP connectors, fully in Bend.
#
# At live startup the REPL fetches the connector bootstrap
# (api.mistral.ai/v1/connectors/bootstrap) and writes a local tool index:
# one line per remote tool: "<cid> <cname> <tool> : #<desc>". The tools
# never enter the model context (they are not in the API tools array);
# the agent reaches them through the ordinary mechanisms:
#   - search_tool_functions: one discovery protocol over the static
#     catalog PLUS this index (merged_search below)
#   - direct calls by name: any "<connector>.<tool>" the model calls —
#     top-level or from a run_program program — resolves against the
#     index here (mcp_call below)
# A call speaks MCP Streamable HTTP against
# /v1/connectors-gateway/<connector_id>/mcp: initialize,
# notifications/initialized, tools/call — three requests on ONE pooled
# connection, because the gateway state is per-connection.
#
# Defs are ordered bottom-up: a def only calls defs defined ABOVE it.
# Recursive loops pass their recursive result to a pure "put" helper
# (Bend is lazy, so the unchosen branch is never forced).
import Base
import ../core/types.bend as T
import ../core/discovery.bend as D
import ../core/text.bend as Tx
import ./persist.bend as P
import ./tools.bend as X
import ./tools-pure.bend as Xp
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http
# request bodies are built with J; every PARSE goes through Jv, which keeps
# numbers as text: Bend's Nat is 48-bit, and the connector bootstrap
# carries 2^53-1 JSON-schema maximums that would fail to parse otherwise
import ../vendor/json.bend as J
import 0x16458a2db4f36577294543ec2a6d420c/json.bend as Jv
import ./mcp-pure.bend as Mcp

# what an MCP tool call answers (converted to ExecRes by the runtime)
type MR is Data:
  MR{ok: Bool, out: String}

type McpTool is Data:
  MT{cid: String, tool: String}

# ---- tolerant JSON accessors (numbers-as-text JSON) ----

def jdone(m: Maybe<&2, Jv.Val>) -> Jv.Val:
  match m:
    case Some{+w}:
      w
    case None{}:
      Jv.Null{}

def jstr.w(w: Jv.Val, +dflt: String) -> String:
  match w:
    case Jv.Str{+s}:
      s
    case _:
      dflt

def jstr.m(m: Maybe<&2, Jv.Val>, +dflt: String) -> String:
  match m:
    case Some{+w}:
      jstr.w(w, dflt)
    case None{}:
      dflt

def jstr(v: Jv.Val, +k: String, +dflt: String) -> String:
  jstr.m(Jv.get(v, k), dflt)

def jbool.w(w: Jv.Val, +dflt: Bool) -> Bool:
  match w:
    case Jv.Flag{on}:
      on
    case _:
      dflt

def jbool.m(m: Maybe<&2, Jv.Val>, +dflt: Bool) -> Bool:
  match m:
    case Some{+w}:
      jbool.w(w, dflt)
    case None{}:
      dflt

def jbool(v: Jv.Val, +k: String, +dflt: Bool) -> Bool:
  jbool.m(Jv.get(v, k), dflt)

def jarr.w(w: Jv.Val) -> List<&2, Jv.Val>:
  match w:
    case Jv.Arr{+xs}:
      xs
    case _:
      Nil{}

def jarr.m(m: Maybe<&2, Jv.Val>) -> List<&2, Jv.Val>:
  match m:
    case Some{+w}:
      jarr.w(w)
    case None{}:
      Nil{}

def jarr(v: Jv.Val, +k: String) -> List<&2, Jv.Val>:
  jarr.m(Jv.get(v, k))

def index_path.pick(empty: Bool, +home: String, +ir: String) -> String:
  Bool.pick(String, empty,
    String.append(home, "/.bend-harness/mcp-index.txt"), ir)

def index_path() -> IO(String):
  do IO<String>:
    ir : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MCP_INDEX")
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    +ix : String <- IO.pure(String, P.env_or(ir, ""))
    +hx : String <- IO.pure(String, P.env_or(hr, ""))
    IO.pure(String, index_path.pick(String.is_empty(ix), hx, ix))

# ---- bootstrap: the connector catalog becomes index lines ----

def is_ready(c: Jv.Val) -> Bool:
  jbool(jdone(Jv.get(c, "status")), "is_ready", False{})

# descriptions may contain raw newlines; the index is line-oriented, so
# newline and carriage-return characters flatten to spaces
def flat.chr(is_nl: Bool, +c: U32) -> String:
  match is_nl:
    case True{}:
      " "
    case False{}:
      SCon{Chr{c}, SNil{}}

def flat(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      String.append(
        flat.chr(Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13)), c), flat(t))

# the input schema rides in the index line so details mode returns real
# declarations (vibe_sdk shows input_schema in details, not just the text)
def tool_line.schema(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case None{}:
      ""
    case Some{+v}:
      String.append(" | input: ", flat(Jv.encode(v)))

def tool_line(+cid: String, +cname: String, +t: Jv.Val) -> String:
  String.append(cid, String.append(" ", String.append(cname,
    String.append(" ", String.append(flat(jstr(t, "name", "?")),
      String.append(" : ", String.append("#",
        String.append(flat(jstr(t, "description", "")),
          tool_line.schema(Jv.get(t, "inputSchema"))))))))))

def tool_lines(+cid: String, +cname: String, ts: List<&2, Jv.Val>) -> String:
  match ts:
    case Nil{}:
      ""
    case Con{+t, rest}:
      String.append(tool_line(cid, cname, t),
        String.append("\n", tool_lines(cid, cname, rest)))

# the cname of an index line is the normalized, deduplicated alias; the
# connector id (for the gateway URL) stays raw
def conn_lines.emit(+c: Jv.Val, alias: String, +rest_res: String) -> String:
  String.append(tool_lines(jstr(c, "id", ""), alias, jarr(c, "tools")),
    String.append("\n", rest_res))

# seen = the aliases BEFORE the current connector (the dedupe must not
# see the connector it is naming); seen2 adds the current one for the
# rest of the list
def conn_lines.ready(skip: Bool, +c: Jv.Val, +rest: List<&2, Jv.Val>,
    +seen: List<&2, String>, +rest_res: String,
    +seen2: List<&2, String>) -> String:
  match skip:
    case True{}:
      conn_lines.emit(c, "", rest_res)
    case False{}:
      conn_lines.emit(c,
        Mcp.dedupe(Mcp.normalize_name(jstr(c, "name", "")), seen), rest_res)

def conn_lines.go.step(skip: Bool, +c: Jv.Val, +rest: List<&2, Jv.Val>,
    +seen: List<&2, String>, +seen2: List<&2, String>,
    rest_res: String) -> String:
  conn_lines.ready(skip, c, rest, seen, rest_res, seen2)

def conn_lines.go(cs: List<&2, Jv.Val>, +seen: List<&2, String>) -> String:
  match cs:
    case Nil{}:
      ""
    case Con{+c, +rest}:
      conn_lines.go.step(Bool.not(is_ready(c)), c, rest, seen,
        Con{Mcp.normalize_name(jstr(c, "name", "")), seen},
        conn_lines.go(rest, Con{Mcp.normalize_name(jstr(c, "name", "")), seen}))

def conn_lines(cs: List<&2, Jv.Val>) -> String:
  conn_lines.go(cs, Nil{})

def bootstrap_root(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case None{}:
      ""
    case Some{+v}:
      conn_lines(jarr(v, "connectors"))

def bootstrap_body(+body: String) -> String:
  bootstrap_root(Jv.parse(body))

def bootstrap.go(+path: String, r: Result<&2, &2, Http.Err, Http.Res>)
    -> IO(Unit):
  match r:
    case Fail{+e}:
      do IO<Unit>:
        IO.print("[mcp] bootstrap failed; MCP tools unavailable")
        IO.pure(Unit, Unit{})
    case Done{+res}:
      do IO<Unit>:
        u : Unit <- P.write_file(path, bootstrap_body(Http.text(res)))
        IO.print("[mcp] connector index written")
        IO.pure(Unit, Unit{})

def bootstrap(+key: String) -> IO(Unit):
  do IO<Unit>:
    +path : String <- index_path()
    +h : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
      Http.set(Http.empty(), "Authorization", String.append("Bearer ", key)))
    r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch("GET",
      Mcp.bootstrap_url(), h, "")
    bootstrap.go(path, r)

# ---- resolve "<cname>.<tool>" against the index lines ----

def find_line.hit.of(sp: String & String & String & String, +v: String)
    -> Bool:
  (a0, cname, tool, a3) = sp
  String.eq(Mcp.pub_name(cname, tool), v)

def find_line.hit(+line: String, +v: String) -> Bool:
  find_line.hit.of(Mcp.line_parts(line), v)

def find_put.of(sp: String & String & String & String) -> McpTool:
  (cid, a1, tool, a3) = sp
  MT{cid, tool}

def find_put(hit: Bool, +line: String, +rest_res: Maybe<&2, McpTool>)
    -> Maybe<&2, McpTool>:
  match hit:
    case True{}:
      Some{find_put.of(Mcp.line_parts(line))}
    case False{}:
      rest_res

def find_line.go(lines: List<&2, String>, +v: String)
    -> Maybe<&2, McpTool>:
  match lines:
    case Nil{}:
      None{}
    case Con{+line, rest}:
      find_put(find_line.hit(line, v), line, find_line.go(rest, v))

def find_line(+content: String, +v: String) -> Maybe<&2, McpTool>:
  find_line.go(String.split(content, Tx.newline()), v)

def merged_search.pick(cat: List<&2, T.ToolDef>, +args: String,
    m: Maybe<&2, String>) -> MR:
  MR{True{}, Xp.search_reply(args, Mcp.merged_tools.live(cat, m))}

def merged_search.live(+args: String) -> IO(MR):
  do IO<MR>:
    +cat : List<&2, T.ToolDef> <- X.catalog()
    +path : String <- index_path()
    m : Maybe<&2, String> <- P.read_file(path)
    IO.pure(MR, merged_search.pick(cat, args, m))

def merged_search(+args: String, +live: Bool) -> IO(MR):
  match live:
    case True{}:
      merged_search.live(args)
    case False{}:
      IO.pure(MR, MR{True{}, Xp.search_reply(args, X.catalog_short())})

# ---- a call by name: three requests on one pooled connection ----

def init_body() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"bend-harness\",\"version\":\"1.0\"}}}"

def notif_body() -> String:
  "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}"

def call_body(+tool: String, +args: String) -> String:
  J.stringify(J.obj(Con{J.kv("jsonrpc", J.str("2.0")), Con{J.kv("id", J.num(2n)), Con{J.kv("method", J.str("tools/call")), Con{J.kv("params", J.obj(Con{J.kv("name", J.str(tool)), Con{J.kv("arguments", J.raw(args)), Nil{}}})), Nil{}}}}}))

def gateway_url(+cid: String) -> String:
  String.append("https://api.mistral.ai/v1/connectors-gateway/",
    String.append(cid, "/mcp"))

def mcp_hdrs(+key: String) -> Map<&2, List<&2, String>>:
  Http.set(Http.set(Http.set(Http.empty(), "Authorization",
      String.append("Bearer ", key)),
    "Content-Type", "application/json"), "Accept",
    "application/json, text/event-stream")

def content_texts(blocks: List<&2, Jv.Val>) -> String:
  match blocks:
    case Nil{}:
      ""
    case Con{+b, rest}:
      String.append(jstr(b, "text", ""),
        String.append("\n", content_texts(rest)))

def call_res(+j: Jv.Val) -> String & Bool:
  (content_texts(jarr(jdone(Jv.get(j, "result")), "content")),
    jbool(jdone(Jv.get(j, "result")), "isError", False{}))

def call_done.res(sp: String & Bool) -> MR:
  (out, is_err) = sp
  MR{Bool.not(is_err), out}

def call_done(p3: Http.Pool, +body: String) -> IO(MR):
  do IO<MR>:
    u : Unit <- Http.pool.close(p3)
    IO.pure(MR, call_done.res(call_res(jdone(Jv.parse(body)))))

def call_fail(p3: Http.Pool, +why: String) -> IO(MR):
  do IO<MR>:
    u : Unit <- Http.pool.close(p3)
    IO.pure(MR, MR{False{}, String.append("mcp call failed: ", why)})

def step3.ok(p3: Http.Pool, is_ok: Bool, +res: Http.Res) -> IO(MR):
  match is_ok:
    case True{}:
      call_done(p3, Http.text(res))
    case False{}:
      call_fail(p3, "non-200 http status")

def step3.status(p3: Http.Pool, status: U32, +res: Http.Res) -> IO(MR):
  step3.ok(p3, U32.is_eq(status, 200), res)

def step3.res(p3: Http.Pool, +res: Http.Res) -> IO(MR):
  match res:
    case Http.Res{+status, _, _}:
      step3.status(p3, status, res)

def step3.run2(pr3: Http.Pool & Result<&2, &2, Http.Err, Http.Res>,
    +url: String, +h: Map<&2, List<&2, String>>, +tool: String,
    +args: String) -> IO(MR):
  (p3, r3) = pr3
  match r3:
    case Fail{+e}:
      call_fail(p3, "transport")
    case Done{+res}:
      step3.res(p3, res)

def step3(pr2: Http.Pool & Result<&2, &2, Http.Err, Http.Res>, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String) -> IO(MR):
  (p2, _) = pr2
  do IO<MR>:
    pr3 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(p2,
      "POST", url, h, call_body(tool, args), 60000, Http.ModeFollow{})
    step3.run2(pr3, url, h, tool, args)

def step2(pr1: Http.Pool & Result<&2, &2, Http.Err, Http.Res>, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String) -> IO(MR):
  (p1, _) = pr1
  do IO<MR>:
    pr2 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(p1,
      "POST", url, h, notif_body(), 20000, Http.ModeFollow{})
    step3(pr2, url, h, tool, args)

def step1(r1: Result<&2, &2, Http.Err, Http.Res>, p1: Http.Pool, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String) -> IO(MR):
  match r1:
    case Fail{+e}:
      call_fail(p1, "initialize failed")
    case Done{+res}:
      step2((p1, r1), url, h, tool, args)

def mcp_call.found2(cid: String, +tool: String, +args: String, +key: String,
    pr1: Http.Pool & Result<&2, &2, Http.Err, Http.Res>, +url: String,
    +h: Map<&2, List<&2, String>>) -> IO(MR):
  (p1, r1) = pr1
  step1(r1, p1, url, h, tool, args)

def mcp_call.found(+cid: String, +tool: String, +args: String, +key: String)
    -> IO(MR):
  do IO<MR>:
    +url : String <- IO.pure(String, gateway_url(cid))
    +h : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
      mcp_hdrs(key))
    pr1 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(
      Http.pool.new(), "POST", url, h, init_body(), 20000,
      Http.ModeFollow{})
    mcp_call.found2(cid, tool, args, key, pr1, url, h)

# a call with no arguments still needs a JSON object body
def call_args.of(is_empty: Bool, +args: String) -> String:
  match is_empty:
    case True{}:
      "{}"
    case False{}:
      args

def call_args(+args: String) -> String:
  call_args.of(String.is_empty(args), args)

def mcp_call.mt(mt: McpTool, +args: String, +key: String) -> IO(MR):
  match mt:
    case MT{+cid, +tool}:
      mcp_call.found(cid, tool, call_args(args), key)

def index_missing() -> MR:
  MR{False{}, "tool index not found — remote MCP tools are unavailable in this session"}

def mcp_call.resolved(m: Maybe<&2, McpTool>, +name: String, +args: String,
    +key: String) -> IO(MR):
  match m:
    case None{}:
      IO.pure(MR, MR{False{},
        String.append("unknown tool: ", String.append(name,
          " — discover tools with search_tool_functions"))})
    case Some{+mt}:
      mcp_call.mt(mt, args, key)

def mcp_call.read(m: Maybe<&2, String>, +name: String, +args: String,
    +key: String) -> IO(MR):
  match m:
    case None{}:
      IO.pure(MR, index_missing())
    case Some{+content}:
      mcp_call.resolved(find_line(content, name), name, args, key)

def mcp_call(+name: String, +args: String, +key: String) -> IO(MR):
  do IO<MR>:
    +path : String <- index_path()
    m : Maybe<&2, String> <- P.read_file(path)
    mcp_call.read(m, name, args, key)
