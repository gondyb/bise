# runtime/mcp-pure.bend - the pure half of runtime/mcp.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx

# ---- the published alias: a valid identifier (vibe_sdk requires it) ----
# vibe's _normalize_name: anything outside [a-zA-Z0-9_-] flattens to
# "_", the edges strip, the result caps at 256 (an empty name becomes
# "unnamed"). tools.<alias>.<function> in the sandbox needs a clean
# identifier; the connector id (used for the gateway URL) stays raw.

def norm_chr.ok(ok: Bool, +c: U32) -> String:
  match ok:
    case True{}:
      SCon{Chr{c}, SNil{}}
    case False{}:
      "_"

def norm_chr.digit(+c: U32) -> Bool:
  Bool.and(U32.is_le(48, c), U32.is_le(c, 57))

def norm_chr.alpha(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_le(65, c), U32.is_le(c, 90)),
    Bool.and(U32.is_le(97, c), U32.is_le(c, 122)))

def norm_chr.sym(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 45))

def norm_chr.ok2(d: Bool, a: Bool, s: Bool) -> Bool:
  Bool.or(d, Bool.or(a, s))

def norm_chr(+c: U32) -> String:
  norm_chr.ok(norm_chr.ok2(norm_chr.digit(c), norm_chr.alpha(c),
    norm_chr.sym(c)), c)

def norm_go(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      String.append(norm_chr(c), norm_go(t))

def trim_go.next(head_edge: Bool, tail_edge: Bool, +s: String) -> String:
  match head_edge:
    case True{}:
      String.drop(s, 1n)
    case False{}:
      match tail_edge:
        case True{}:
          String.take(s, Nat.sub(String.length(s), 1n))
        case False{}:
          s

# fuel-bound edge trim: one character per iteration, so 2*len fuel is
# always enough (+s: the edges and the recursion each read the string)
def trim_go(fuel: Nat, +s: String) -> String:
  match fuel:
    case 0n:
      s
    case 1n+k:
      trim_go(k, trim_go.next(
        Bool.or(String.starts_with(s, "_"), String.starts_with(s, "-")),
        Bool.or(String.ends_with(s, "_"), String.ends_with(s, "-")), s))

def norm_trim(+s: String) -> String:
  trim_go(Nat.add(String.length(s), String.length(s)), s)

def norm_pick(empty: Bool, +s: String) -> String:
  match empty:
    case True{}:
      "unnamed"
    case False{}:
      s

def norm_take.fits(fits: Bool, +s: String) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      String.take(s, 256n)

def normalize_name(+s: String) -> String:
  norm_pick(String.is_empty(s),
    norm_take.fits(Nat.is_le(String.length(s), 256n),
      norm_trim(norm_go(s))))

# alias dedupe (vibe): colliding normalized aliases get _2, _3, ...
# (the codebase patterns: leaf helpers first, the recursive def calls
# itself by its own name, the recursive result rides as an argument)

def seen_pick(hit: Bool, rest_res: Bool) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      rest_res

def seen_has(seen: List<&2, String>, +alias: String) -> Bool:
  match seen:
    case Nil{}:
      False{}
    case Con{+h, rest}:
      seen_pick(String.eq(h, alias), seen_has(rest, alias))

def suffix.base(+base: String, +n: Nat) -> String:
  String.append(base, String.append("_", Nat.show(n)))

# fuel = len(seen) + 2 tries: one suffix per seen alias, plus slack;
# each try checks base_n against the seen list
def suffix_go(fuel: Nat, +base: String, +n: Nat,
    +seen: List<&2, String>, taken: Bool) -> String:
  match fuel:
    case 0n:
      suffix.base(base, n)
    case 1n+k:
      match taken:
        case True{}:
          suffix_go(k, base, Nat.add(n, 1n), seen,
            seen_has(seen, suffix.base(base, Nat.add(n, 1n))))
        case False{}:
          suffix.base(base, n)

def dedupe.pick(taken: Bool, +alias: String, +seen: List<&2, String>)
    -> String:
  match taken:
    case True{}:
      # the loop starts by trying <alias>_2 (not the alias itself)
      suffix_go(Nat.add(List.length(&2, String, seen), 2n), alias, 2n, seen,
        seen_has(seen, suffix.base(alias, 2n)))
    case False{}:
      alias

# +seen: both the collision check and the suffix search read the list
def dedupe(+alias: String, +seen: List<&2, String>) -> String:
  dedupe.pick(seen_has(seen, alias), alias, seen)



# the bootstrap endpoint, exactly as the vibe CLI calls it: the
# web_search builtin rides along as an MCP-callable connector, and
# auth-actionable (not yet connected) connectors are included (they
# stay out of the index: only ready connectors index their tools)
def bootstrap_url() -> String:
  "https://api.mistral.ai/v1/connectors/bootstrap?supports_mcp=true&include_auth_actionable_connectors=true&builtin_connectors=web_search"

# ---- the index line format, parsed ----
# line: "<cid> <cname> <tool> : #<desc>"

def line_parts.n2(sp: Maybe<&2, Nat>, +cid: String, +cname: String,
    +rest: String) -> String & String & String & String:
  match sp:
    case Some{+q}:
      (cid, cname, String.take(rest, q),
        String.drop(rest, Nat.add(q, 3n)))
    case None{}:
      (cid, cname, "", "")

def line_parts.n(+cid: String, +cname: String, +rest: String)
    -> String & String & String & String:
  line_parts.n2(Tx.find(rest, " : ", 0n), cid, cname, rest)

def line_parts.c2(sp: Maybe<&2, Nat>, +cid: String, +rest: String)
    -> String & String & String & String:
  match sp:
    case Some{+q}:
      line_parts.n(cid, String.take(rest, q),
        String.drop(rest, Nat.add(q, 1n)))
    case None{}:
      (cid, "", "", "")

def line_parts.c(+cid: String, +rest: String)
    -> String & String & String & String:
  line_parts.c2(Tx.find(rest, " ", 0n), cid, rest)

def line_parts.d(sp: Maybe<&2, Nat>, +line: String)
    -> String & String & String & String:
  match sp:
    case Some{+p}:
      line_parts.c(String.take(line, p), String.drop(line, Nat.add(p, 1n)))
    case None{}:
      ("", "", "", "")

def line_parts(+line: String) -> String & String & String & String:
  line_parts.d(Tx.find(line, " ", 0n), line)

# the tool name of an index line ("" when the line is malformed)
def line_tool.of(sp: String & String & String & String) -> String:
  (a0, a1, tool, a3) = sp
  tool

def line_tool(+line: String) -> String:
  line_tool.of(line_parts(line))

# ---- the index file becomes tools ----

def pub_name(+cname: String, +tool: String) -> String:
  String.append(cname, String.append(".", tool))

def index_tool.of(cid: String, cname: String, tool: String, +desc: String)
    -> T.ToolDef:
  T.ToolDef{pub_name(cname, tool), desc}

# skip a malformed line; cons its ToolDef onto the recursive result
# (the discovery.bend pattern: the unchosen branch is never forced)
def index_put.of(sp: String & String & String & String,
    +rest_res: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  (cid, cname, tool, desc) = sp
  Con{index_tool.of(cid, cname, tool, desc), rest_res}

def index_put(skip: Bool, +line: String,
    +rest_res: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match skip:
    case True{}:
      rest_res
    case False{}:
      index_put.of(line_parts(line), rest_res)

def index_go(lines: List<&2, String>) -> List<&2, T.ToolDef>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+line, rest}:
      index_put(String.is_empty(line_tool(line)), line, index_go(rest))

def index_of(+content: String) -> List<&2, T.ToolDef>:
  index_go(String.split(content, Tx.newline()))

# ---- the unified search: static catalog + MCP index ----
# search_tool_functions routes here; reading the index is an effect, so
# the search runs on the EExec path (bash and MCP calls live there too)

# the live surface searches the LIVE catalog (the parity tools, the
# skill loader, and the self.* harness tools) plus the MCP index; the
# scripted surface stays on catalog_short so the scenarios never read
# files (a search that could not see `skill` made the model guess wrong
# names from inside the sandbox)
def merged_tools.live(cat: List<&2, T.ToolDef>, m: Maybe<&2, String>)
    -> List<&2, T.ToolDef>:
  match m:
    case None{}:
      cat
    case Some{+content}:
      List.append(&2, T.ToolDef, cat, index_of(content))
