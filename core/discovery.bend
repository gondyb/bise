# core/discovery.bend — D2: dynamic tool discovery.
# The model searches a tool catalog instead of every schema being injected:
# best_match ranks name-prefix hits above substring hits; details loads
# exact names. Results are rendered as text for the model.
import Base
import ./types.bend as T

# does one tool match the query anywhere?
def def_matches(+q: String, td: T.ToolDef) -> Bool:
  match td:
    case T.ToolDef{name, desc}:
      Bool.or(String.contains(name, q), String.contains(desc, q))

def def_prefix(+q: String, td: T.ToolDef) -> Bool:
  match td:
    case T.ToolDef{name, _}:
      String.starts_with(name, q)

def def_infix(+q: String, +td: T.ToolDef) -> Bool:
  Bool.and(def_matches(q, td), Bool.not(def_prefix(q, td)))

def search.put(keep: Bool, +t: T.ToolDef,
    rest: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match keep:
    case True{}:
      Con{t, rest}
    case False{}:
      rest

def search_all(+q: String, tools: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      Nil{}
    case +t <> rest:
      search.put(def_matches(q, t), t, search_all(q, rest))

def search_prefix(+q: String, tools: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      Nil{}
    case +t <> rest:
      search.put(def_prefix(q, t), t, search_prefix(q, rest))

def search_infix(+q: String, tools: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      Nil{}
    case +t <> rest:
      search.put(def_infix(q, t), t, search_infix(q, rest))

# ranked best-match results, capped at `limit`
def best_match(+q: String, +tools: List<&2, T.ToolDef>, limit: Nat)
    -> List<&2, T.ToolDef>:
  List.take(&2, T.ToolDef,
    List.append(&2, T.ToolDef, search_prefix(q, tools),
      search_infix(q, tools)), limit)

# exact-name lookup over a list of requested names
def in_names(+name: String, names: List<&2, String>) -> Bool:
  match names:
    case Nil{}:
      False{}
    case Con{n, rest}:
      Bool.or(String.eq(name, n), in_names(name, rest))

def search_details(+names: List<&2, String>, tools: List<&2, T.ToolDef>)
    -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      Nil{}
    case +t <> rest:
      match t:
        case T.ToolDef{name, _}:
          search.put(in_names(name, names), t, search_details(names, rest))

# ---- model-facing rendering ----

def show_tool(td: T.ToolDef) -> String:
  match td:
    case T.ToolDef{name, desc}:
      String.append(String.append(name, ": "), desc)

def show_tools(ts: List<&2, T.ToolDef>) -> String:
  match ts:
    case Nil{}:
      ""
    case Con{t, rest}:
      String.append(String.append(show_tool(t), "\n"), show_tools(rest))
