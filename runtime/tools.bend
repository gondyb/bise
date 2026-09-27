# runtime/tools.bend — the Runtime's tool catalog.
#
# Tool descriptions for run_typescript and search_tool_functions are
# exact copies from the Vibe SDK (vibe_sdk/harness/core), loaded from
# tool-desc-*.txt so the literal braces in the docs don't fight the
# Bend string parser. Schemas live in core/api.bend.
import Base
import ../core/types.bend as T
import ../core/discovery.bend as D
import ./persist.bend as P
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J

def desc_or(m: Maybe<&2, String>, +dflt: String) -> String:
  match m:
    case Some{+s}:
      s
    case None{}:
      dflt

def search_desc.read(m: Maybe<&2, String>) -> String:
  desc_or(m, "Discover callable tool functions for run_typescript.")

def search_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-search.txt")
    IO.pure(String, search_desc.read(m))

def rt_desc.read(m: Maybe<&2, String>) -> String:
  desc_or(m, "Executes TypeScript code in a sandbox environment.")

def rt_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-run-typescript.txt")
    IO.pure(String, rt_desc.read(m))

def bash_desc.read(m: Maybe<&2, String>) -> String:
  desc_or(m, "Run a real shell command and return its output. Args: the command.")

def bash_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-bash.txt")
    IO.pure(String, bash_desc.read(m))

def skill_desc.read(m: Maybe<&2, String>) -> String:
  desc_or(m, "Load a specialized skill that provides domain-specific instructions and workflows.")

def skill_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-skill.txt")
    IO.pure(String, skill_desc.read(m))

# pure: build the catalog from given descriptions
def catalog_pure(+sd: String, +rd: String, +bd: String) -> List<&2, T.ToolDef>:
  Con{T.ToolDef{"search_tool_functions", sd},
   Con{T.ToolDef{"self.sleep", "Sleep for a bounded number of seconds (0 < s < 20), then report."},
    Con{T.ToolDef{"bash", bd},
     Con{T.ToolDef{"run_typescript", rd},
      Nil{}}}}}

# ---- self.* harness tools (live catalog only) ----
# The agent drives its own harness from inside a turn: request a
# compaction of the conversation, or a restart of the process on the
# latest code. Both take no arguments; both only take effect at the
# END of the current turn (see deferred_of in runtime/main.bend), so
# the model can keep working and read the ack as the tool result.
def self_tool_defs() -> List<&2, T.ToolDef>:
  Con{T.ToolDef{"self.reload",
    "Restart the harness process with the latest code. Use it after editing the harness source: the session, the model context and background commands are preserved; the restart happens at the end of the current turn. Takes no arguments."},
   Con{T.ToolDef{"self.compact",
    "Compact the conversation history into a summary. Use it when the context grows large: protected recent user messages are preserved, the compaction runs at the end of the current turn. Takes no arguments."},
    Nil{}}}

# pure helpers the laws pin: catalog size and positional extraction
def catalog_len(tools: List<&2, T.ToolDef>) -> Nat:
  match tools:
    case Nil{}:
      0n
    case Con{_, +rest}:
      Nat.add(catalog_len(rest), 1n)

def tool_name_at(n: Nat, tools: List<&2, T.ToolDef>) -> String:
  match n:
    case 0n:
      match tools:
        case Nil{}:
          ""
        case Con{T.ToolDef{+name, _}, _}:
          name
    case 1n+k:
      match tools:
        case Nil{}:
          ""
        case Con{_, +rest}:
          tool_name_at(k, rest)

def tool_desc_at(n: Nat, tools: List<&2, T.ToolDef>) -> String:
  match n:
    case 0n:
      match tools:
        case Nil{}:
          ""
        case Con{T.ToolDef{_, +desc}, _}:
          desc
    case 1n+k:
      match tools:
        case Nil{}:
          ""
        case Con{_, +rest}:
          tool_desc_at(k, rest)

# the live catalog: the four vibe-parity tools, the skill loader, and
# the self.* harness tools (catalog_short keeps the scripted scenarios
# byte-identical)
def catalog_live(+sd: String, +rd: String, +bd: String, +kd: String)
    -> List<&2, T.ToolDef>:
  List.append(&2, T.ToolDef,
    List.append(&2, T.ToolDef, catalog_pure(sd, rd, bd),
      Con{T.ToolDef{"skill", kd}, Nil{}}),
    self_tool_defs())

def catalog() -> IO(List<&2, T.ToolDef>):
  do IO<List<&2, T.ToolDef>>:
    +sd : String <- search_desc()
    +rd : String <- rt_desc()
    +bd : String <- bash_desc()
    +kd : String <- skill_desc()
    IO.pure(List<&2, T.ToolDef>, catalog_live(sd, rd, bd, kd))

# short fallback for pure contexts (laws, scenarios that don't need
# the full vibe_sdk description text)
def catalog_short() -> List<&2, T.ToolDef>:
  catalog_pure(
    "Discover callable tool functions for run_typescript. Use mode=best_match first with a capability query; it returns ranked exact function names. If a returned name fits, call mode=details with that exact name before using it.",
    "Executes TypeScript code in a sandbox environment. The sandbox expects an async function main() containing all your code. Tool functions are available under tools.<group>.<function>; no imports needed. The result of main() is the tool output.",
    "Run a real shell command and return its output. Args: the command.")

# ---- the unified search: the vibe_sdk SearchRequest schema ----
# The live model sends {"mode": ..., "query": ...} (the exact vibe_sdk
# schema pinned by search_schema in core/api.bend): best_match returns up
# to 20 ranked exact function names, details returns declarations for
# exact names, all_connector_capabilities lists connector inventories.
# The scripted suite keeps the flat 'best_match: query' syntax below.

def SR_USAGE() -> String:
  "Usage: mode=best_match with query; mode=details with functions=[names]; mode=all_connector_capabilities with connectors=[names]"

# strings of a JSON array field
def jstrs_of(vals: List<&2, J.Json>, +acc: List<&2, String>) -> List<&2, String>:
  match vals:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+v, rest}:
      match v:
        case J.JStr{+s}:
          jstrs_of(rest, Con{s, acc})
        case _:
          jstrs_of(rest, acc)

def jstrs.arr(v: J.Json) -> List<&2, String>:
  match v:
    case J.JArr{+vals}:
      jstrs_of(vals, Nil{})
    case _:
      Nil{}

def jstrs.found(m: Maybe<&2, J.Json>) -> List<&2, String>:
  match m:
    case Some{+v}:
      jstrs.arr(v)
    case None{}:
      Nil{}

def jstrs(j: J.Json, +key: String) -> List<&2, String>:
  jstrs.found(J.get(j, key))

# ---- tool field accessors ----

def tool_name(td: T.ToolDef) -> String:
  match td:
    case T.ToolDef{+name, _}:
      name

def tool_desc(td: T.ToolDef) -> String:
  match td:
    case T.ToolDef{_, +desc}:
      desc

def word_hit(+w: String, +td: T.ToolDef) -> Bool:
  Bool.or(String.contains(tool_name(td), w), String.contains(tool_desc(td), w))

def bool_nat(b: Bool) -> Nat:
  match b:
    case True{}:
      1n
    case False{}:
      0n

# a tool's score = the number of query words it matches anywhere
def word_score(words: List<&2, String>, +td: T.ToolDef) -> Nat:
  match words:
    case Nil{}:
      0n
    case Con{+w, rest}:
      Nat.add(bool_nat(word_hit(w, td)), word_score(rest, td))

def len_defs(ts: List<&2, T.ToolDef>) -> Nat:
  match ts:
    case Nil{}:
      0n
    case Con{_, rest}:
      Nat.add(1n, len_defs(rest))

def keep_word(keep: Bool, +w: String, +acc: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      Con{w, acc}
    case False{}:
      acc

def words_of.clean(ws: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+w, rest}:
      words_of.clean(rest, keep_word(Bool.not(String.is_empty(w)), w, acc))

def words_of(+q: String) -> List<&2, String>:
  words_of.clean(String.split(String.trim(q), ' '), Nil{})

def is_nil_strs(ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

# does any connector match the tool name?
def conn_hit(conns: List<&2, String>, +td: T.ToolDef) -> Bool:
  match conns:
    case Nil{}:
      False{}
    case Con{+c, rest}:
      Bool.or(String.starts_with(tool_name(td), String.append(c, ".")),
        conn_hit(rest, td))

# tools of the given connectors: an EMPTY connector list keeps
# everything (no filter); a non-empty list keeps only matches
def conn_keep(conns: List<&2, String>, +td: T.ToolDef) -> Bool:
  match conns:
    case Nil{}:
      True{}
    case Con{_, _}:
      conn_hit(conns, td)

def tools_of(tools: List<&2, T.ToolDef>, +conns: List<&2, String>,
    +acc: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      List.reverse(&2, T.ToolDef, acc)
    case Con{+t, rest}:
      tools_of(rest, conns, D.search.put(conn_keep(conns, t), t, acc))

# tools scoring exactly s, in catalog order (the put/reverse idiom)
def rank_at(tools: List<&2, T.ToolDef>, +words: List<&2, String>, +s: Nat,
    +level: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      List.reverse(&2, T.ToolDef, level)
    case Con{+t, rest}:
      rank_at(rest, words, s,
        D.search.put(Nat.is_eq(word_score(words, t), s), t, level))

# descending scores [n .. 1] driven by the words list (one per word)
def scores_go(words: List<&2, String>, +cur: Nat,
    +acc: List<&2, Nat>) -> List<&2, Nat>:
  match words:
    case Nil{}:
      acc
    case Con{_, rest}:
      scores_go(rest, Nat.add(cur, 1n), Con{cur, acc})

def scores_of(words: List<&2, String>) -> List<&2, Nat>:
  scores_go(words, 1n, Nil{})

# ranked by descending score (equal scores keep catalog order)
def rank_desc(scores: List<&2, Nat>, +words: List<&2, String>,
    +tools: List<&2, T.ToolDef>, +acc: List<&2, T.ToolDef>)
    -> List<&2, T.ToolDef>:
  match scores:
    case Nil{}:
      acc
    case Con{+s, rest}:
      rank_desc(rest, words, tools,
        List.append(&2, T.ToolDef, acc, rank_at(tools, words, s, Nil{})))

def ranked(+words: List<&2, String>, +tools: List<&2, T.ToolDef>)
    -> List<&2, T.ToolDef>:
  rank_desc(scores_of(words), words, tools, Nil{})

# ---- renderers ----

def names_of(ts: List<&2, T.ToolDef>, +acc: List<&2, String>) -> List<&2, String>:
  match ts:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+t, rest}:
      names_of(rest, Con{tool_name(t), acc})

def show_names(ts: List<&2, T.ToolDef>) -> String:
  String.join(names_of(ts, Nil{}), "\n")

def more_flag.pick(too_many: Bool) -> String:
  match too_many:
    case True{}:
      "moreCandidatesAvailable: true\n"
    case False{}:
      ""

def more_flag(+count: Nat) -> String:
  more_flag.pick(Nat.is_le(21n, count))

# ---- the structured modes ----

def is_nil_defs(ts: List<&2, T.ToolDef>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

def sr_best.none(none: Bool, +words: List<&2, String>,
    +tools: List<&2, T.ToolDef>) -> String:
  match none:
    case True{}:
      "No matching tool candidates for the query."
    case False{}:
      String.append(more_flag(len_defs(ranked(words, tools))),
        show_names(List.take(&2, T.ToolDef, ranked(words, tools), 20n)))

def sr_best.has_any(+words: List<&2, String>, +tools: List<&2, T.ToolDef>) -> String:
  sr_best.none(is_nil_defs(ranked(words, tools)), words, tools)

def sr_best.pick(empty: Bool, +query: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      SR_USAGE()
    case False{}:
      sr_best.has_any(words_of(query),
        tools_of(tools, jstrs(j, "connectors"), Nil{}))

def sr_best.go(+query: String, +j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_best.pick(String.is_empty(query), query, j, tools)

def sr_best(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_best.go(J.get_str_or(j, "query", ""), j, tools)

# ---- details-mode missing-name reporting (the vibe_sdk contract:
# a reply never comes back silently empty; unfound names are listed) ----

def has_name(+name: String, tools: List<&2, T.ToolDef>) -> Bool:
  match tools:
    case Nil{}:
      False{}
    case Con{+t, rest}:
      Bool.or(String.eq(tool_name(t), name), has_name(name, rest))

def keep_str(keep: Bool, +s: String, +rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      Con{s, rest}
    case False{}:
      rest

# requested names absent from the catalog, in request order
def missing_names(names: List<&2, String>, +tools: List<&2, T.ToolDef>)
    -> List<&2, String>:
  match names:
    case Nil{}:
      Nil{}
    case Con{+n, rest}:
      keep_str(Bool.not(has_name(n, tools)), n, missing_names(rest, tools))

def missing_lines(names: List<&2, String>, +acc: List<&2, String>)
    -> List<&2, String>:
  match names:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+n, rest}:
      missing_lines(rest,
        Con{String.append("- No exact tool function found for: ", n), acc})

def show_missing.some(empty: Bool, +names: List<&2, String>) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append("\n# Missing Exact Names\n",
        String.join(missing_lines(names, Nil{}), "\n"))

def show_missing(+names: List<&2, String>) -> String:
  show_missing.some(is_nil_strs(names), names)

# the details reply: declarations for the names that resolve, the
# vibe_sdk empty message when none do, and the missing list either way
def det_reply.none(none: Bool, +names: List<&2, String>,
    +found: List<&2, T.ToolDef>, +tools: List<&2, T.ToolDef>) -> String:
  match none:
    case True{}:
      String.append(
        "No exact tool function details loaded. Call best_match/all_connector_capabilities first, then request exact returned function names.",
        show_missing(missing_names(names, tools)))
    case False{}:
      String.append(D.show_tools(found),
        show_missing(missing_names(names, tools)))

def det_reply(+names: List<&2, String>, +tools: List<&2, T.ToolDef>) -> String:
  det_reply.none(is_nil_defs(D.search_details(names, tools)), names,
    D.search_details(names, tools), tools)

def sr_det(names: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  det_reply(names, tools)

def sr_det.pick(empty: Bool, names: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      "Details mode requires at least one exact function name returned by best_match or all_connector_capabilities mode."
    case False{}:
      sr_det(names, tools)

def sr_det.entry(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_det.pick(is_nil_strs(List.append(&2, String, jstrs(j, "functions"),
    jstrs(j, "functionNames"))),
    List.append(&2, String, jstrs(j, "functions"), jstrs(j, "functionNames")),
    tools)

def sr_caps(conns: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  D.show_tools(tools_of(tools, conns, Nil{}))

def sr_caps.pick(empty: Bool, conns: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      "all_connector_capabilities requires at least one connector name."
    case False{}:
      sr_caps(conns, tools)

def sr_caps.entry(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_caps.pick(is_nil_strs(jstrs(j, "connectors")), jstrs(j, "connectors"), tools)

def sr_mode.caps(is_caps: Bool, +j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  match is_caps:
    case True{}:
      sr_caps.entry(j, tools)
    case False{}:
      SR_USAGE()

def sr_mode.det(is_det: Bool, +mode: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match is_det:
    case True{}:
      sr_det.entry(j, tools)
    case False{}:
      sr_mode.caps(String.eq(mode, "all_connector_capabilities"), j, tools)

def sr_mode.best(is_best: Bool, +mode: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match is_best:
    case True{}:
      sr_best(j, tools)
    case False{}:
      sr_mode.det(String.eq(mode, "details"), mode, j, tools)

def sr_dispatch(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_mode.best(String.eq(J.get_str_or(j, "mode", "best_match"), "best_match"),
    J.get_str_or(j, "mode", "best_match"), j, tools)

# ---- the flat syntax of the scripted suite ----

def flat_mode(details: Bool, +args: String, tools: List<&2, T.ToolDef>) -> String:
  match details:
    case True{}:
      det_reply(String.split(String.trim(String.drop(args, 8n)), ' '), tools)
    case False{}:
      D.show_tools(D.best_match(
        String.trim(String.drop(args, 11n)), tools, 5n))

def flat_pick(known: Bool, +args: String, tools: List<&2, T.ToolDef>) -> String:
  match known:
    case False{}:
      SR_USAGE()
    case True{}:
      flat_mode(String.starts_with(args, "details:"), args, tools)

def flat_reply(+args: String, tools: List<&2, T.ToolDef>) -> String:
  flat_pick(Bool.or(String.starts_with(args, "details:"),
    String.starts_with(args, "best_match:")), args, tools)

def sr_parse(r: Result<&2, &2, String, J.Json>, +args: String,
    tools: List<&2, T.ToolDef>) -> String:
  match r:
    case Done{+j}:
      sr_dispatch(j, tools)
    case Fail{_}:
      flat_reply(args, tools)

def search_reply(+args: String, tools: List<&2, T.ToolDef>) -> String:
  sr_parse(J.parse(String.trim(args)), args, tools)
