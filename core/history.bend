# core/history.bend — history queries used by compaction (ADR 0011).
# - the most recent non-injected user message is protected from removal
# - dropping an assistant tool call drops its result messages with it
# - a lone oversized text message is middle-truncated to fit
import Base
import ./types.bend as T
import ./estimate.bend as E

# ---- predicates ----

def is_user(m: T.Msg) -> Bool:
  match m:
    case T.Msg{role, _, _, injected}:
      match role:
        case T.RUser{}:
          Bool.not(injected)
        case _:
          False{}

def is_tool_res(m: T.Msg) -> Bool:
  match m:
    case T.Msg{role, _, _, _}:
      match role:
        case T.RTool{}:
          True{}
        case _:
          False{}

def is_asst_with_calls(m: T.Msg) -> Bool:
  match m:
    case T.Msg{role, _, calls, _}:
      match role:
        case T.RAsst{}:
          match calls:
            case Nil{}:
              False{}
            case Con{_, _}:
              True{}
        case _:
          False{}

# Bool to Nat, for saturating budget arithmetic
def b2n(b: Bool) -> Nat:
  match b:
    case True{}:
      1n
    case False{}:
      0n

# ---- queries ----

def has_user(ms: List<&2, T.Msg>) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case Con{m, rest}:
      Bool.or(is_user(m), has_user(rest))

# the most recent non-injected user message, if any
def latest_user(ms: List<&2, T.Msg>) -> Maybe<&2, T.Msg>:
  List.find(~T.Msg, ~is_user, List.reverse(&2, T.Msg, ms))

def skip_tools.put(is_tool: Bool, +m: T.Msg, skipped: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match is_tool:
    case True{}:
      skipped
    case False{}:
      Con{m, skipped}

# drop the leading run of tool-result messages
def skip_tools(ms: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, rest}:
      skip_tools.put(is_tool_res(m), m, skip_tools(rest))

# drop the head message, pairing away its tool results when it has calls
def drop_head_tail(paired: Bool, +m: T.Msg, rest: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match paired:
    case True{}:
      skip_tools(rest)
    case False{}:
      rest

def drop_one.pick(is_u: Bool, has_later: Bool, paired: Bool,
    +m: T.Msg, rest: List<&2, T.Msg>) -> Maybe<&2, List<&2, T.Msg>>:
  match is_u:
    case True{}:
      match has_later:
        case True{}:
          # a user message, but a later one exists: it is not protected
          Some{rest}
        case False{}:
          # m IS the protected latest user message; drop from the rest,
          # which holds no non-injected user messages
          match rest:
            case Nil{}:
              None{}
            case Con{+r, rrest}:
              Some{Con{m, drop_head_tail(is_asst_with_calls(r), r, rrest)}}
    case False{}:
      Some{drop_head_tail(paired, m, rest)}

# drop_one: remove the oldest item that is not the protected latest
# non-injected user message; an assistant tool call takes its results with it.
def drop_one(ms: List<&2, T.Msg>) -> Maybe<&2, List<&2, T.Msg>>:
  match ms:
    case Nil{}:
      None{}
    case Con{+m, +rest}:
      drop_one.pick(is_user(m), has_user(rest), is_asst_with_calls(m), m, rest)

# ---- middle truncation of a lone oversized text message ----
# keeps a head, a "[...]" marker, and a tail, within `budget` tokens
def truncate.pick(fits: Bool, +s: String, budget: Nat) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      +room = Nat.sub(Nat.mul(budget, 4n), 5n)
      front = Nat.div(Nat.mul(room, 3n), 4n)
      back = Nat.div(room, 4n)
      String.append(String.append(String.take(s, front), "[...]"),
        String.drop(s, Nat.sub(String.length(s), back)))

def truncate(+s: String, +budget: Nat) -> String:
  truncate.pick(Nat.is_le(String.length(s), Nat.mul(budget, 4n)), s, budget)

# ---- preserved recent user messages (ADR 0011 selection) ----
# walk from the latest, keep non-injected user messages while they fit the
# budget; the most recent one is kept even over budget (the final fit
# truncates it); stop at the first older one that does not fit.

def pick_recent.put(include: Bool, +m: T.Msg, acc: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match include:
    case True{}:
      Con{m, acc}
    case False{}:
      acc

def pick_recent(rev: List<&2, T.Msg>, +budget: Nat, +first: Bool, +stop: Bool,
    acc: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match rev:
    case Nil{}:
      acc
    case +m <> rest:
      match stop:
        case True{}:
          acc
        case False{}:
          +t = E.msg_tokens(m)
          +fits = Nat.is_le(t, budget)
          +include = Bool.and(is_user(m), Bool.or(first, fits))
          pick_recent(rest,
            Nat.sub(budget, Nat.mul(t, b2n(include))),
            Bool.and(first, Bool.not(is_user(m))),
            Bool.or(stop, Bool.and(is_user(m),
              Bool.and(Bool.not(fits), Bool.not(first)))),
            pick_recent.put(include, m, acc))

def preserve_recent(budget: Nat, ms: List<&2, T.Msg>) -> List<&2, T.Msg>:
  List.reverse(&2, T.Msg,
    pick_recent(List.reverse(&2, T.Msg, ms), budget, True{}, False{}, Nil{}))

# ---- the commit convention (pinned by LAWS.bend) ----
# canonical history is OLDEST-FIRST: a commit lands at the tail, never
# at the head (a live run once caught commits prepending, which the
# scripted scenarios masked).

def commit(h: List<&2, T.Msg>, +m: T.Msg) -> List<&2, T.Msg>:
  List.append(&2, T.Msg, h, Con{m, Nil{}})

def last.put(r: Maybe<&2, T.Msg>, +m: T.Msg) -> Maybe<&2, T.Msg>:
  match r:
    case Some{+x}:
      Some{x}
    case None{}:
      Some{m}

def last(ms: List<&2, T.Msg>) -> Maybe<&2, T.Msg>:
  match ms:
    case Nil{}:
      None{}
    case Con{+m, rest}:
      last.put(last(rest), m)

def all_user.put(is_u: Bool, +m: T.Msg, rest: Bool) -> Bool:
  match is_u:
    case True{}:
      rest
    case False{}:
      False{}

def all_user(ms: List<&2, T.Msg>) -> Bool:
  match ms:
    case Nil{}:
      True{}
    case Con{+m, rest}:
      all_user.put(is_user(m), m, all_user(rest))
