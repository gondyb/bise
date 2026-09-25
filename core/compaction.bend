# core/compaction.bend — ADR 0011: core-owned context budgeting and compaction.
#
# - the pending projection is fitted to the resolved token threshold
# - the canonical history is never touched here; only the projection and
#   the accepted replacement are built
# - summary validation: <summary>..</summary> block, fallback trimmed text,
#   one format retry, then invalid_compaction_summary
import Base
import ./types.bend as T
import ./estimate.bend as E
import ./history.bend as H

# ---- fit result ----
type FitR is Data:
  FOk{ms: List<&2, T.Msg>}
  FBig{}

# middle-truncate the lone text-only latest user message to fit
def fit.lone.trunc(ok_budget: Bool, +m: T.Msg, +budget: Nat) -> FitR:
  match ok_budget:
    case True{}:
      match m:
        case T.Msg{role, +text, _, injected}:
          FOk{Con{T.Msg{role, H.truncate(text, budget), Nil{}, injected},
            Nil{}}}
    case False{}:
      FBig{}

def fit.lone.ok(fits: Bool, +m: T.Msg, +budget: Nat) -> FitR:
  match fits:
    case True{}:
      FOk{Con{m, Nil{}}}
    case False{}:
      fit.lone.trunc(Nat.is_le(3n, budget), m, budget)

def fit.lone(ms: List<&2, T.Msg>, +budget: Nat) -> FitR:
  match ms:
    case Nil{}:
      FBig{}
    case Con{+m, rest}:
      match rest:
        case Con{_, _}:
          FBig{}
        case Nil{}:
          fit.lone.ok(Nat.is_le(E.msg_tokens(m), budget), m, budget)

def fit.zero(fits: Bool, +ms: List<&2, T.Msg>, +budget: Nat) -> FitR:
  match fits:
    case True{}:
      FOk{ms}
    case False{}:
      fit.lone(ms, budget)

# fit `ms` into `budget` tokens: drop the oldest unprotected item (with
# tool-call pairing) until it fits; a lone latest user message is
# middle-truncated; a budget too small for the marker is too large.
# `k` is a drop fuel (the list length) that drives the termination check;
# `fits` and `d` are the precomputed check and drop for the current round.
def fit(+k: Nat, +ms: List<&2, T.Msg>, fits: Bool,
    d: Maybe<&2, List<&2, T.Msg>>, +budget: Nat) -> FitR:
  match k:
    case 0n:
      fit.zero(fits, ms, budget)
    case 1n+j:
      match fits:
        case True{}:
          FOk{ms}
        case False{}:
          match d:
            case None{}:
              fit.lone(ms, budget)
            case Some{+rest}:
              fit(j, rest, Nat.is_le(E.msgs_tokens(rest), budget),
                H.drop_one(rest), budget)

def fit_list(+budget: Nat, +ms: List<&2, T.Msg>) -> FitR:
  fit(List.length(&2, T.Msg, ms), ms,
    Nat.is_le(E.msgs_tokens(ms), budget), H.drop_one(ms), budget)

# ---- substring search (first occurrence) ----

def find.put(hit: Bool, here: Maybe<&2, Nat>, rest: Maybe<&2, Nat>)
    -> Maybe<&2, Nat>:
  match hit:
    case True{}:
      here
    case False{}:
      rest

def find(s: String, +pat: String, +i: Nat) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      None{}
    case SCon{_, +t}:
      find.put(String.starts_with(s, pat), Some{i},
        find(t, pat, Nat.add(i, 1n)))

# ---- summary validation ----
# a complete <summary>..</summary> block with non-empty trimmed content,
# else (no delimiters) the trimmed text as a compatibility fallback,
# else malformed (None)

def parse.non_empty(is_e: Bool, +m: String) -> Maybe<&2, String>:
  match is_e:
    case True{}:
      None{}
    case False{}:
      Some{m}

# permissive: an unclosed <summary> takes the rest of the text as the
# summary (a truncated close tag is not a reason to fail compaction)
def parse.close(c: Maybe<&2, Nat>, +rest: String) -> Maybe<&2, String>:
  match c:
    case None{}:
      +trimmed = String.trim(rest)
      parse.non_empty(String.is_empty(trimmed), trimmed)
    case Some{q}:
      +mid = String.trim(String.take(rest, q))
      parse.non_empty(String.is_empty(mid), mid)

def parse.open(o: Maybe<&2, Nat>, +text: String) -> Maybe<&2, String>:
  match o:
    case None{}:
      +trimmed = String.trim(text)
      parse.non_empty(String.is_empty(trimmed), trimmed)
    case Some{p}:
      +rest = String.drop(text, Nat.add(p, 9n))
      parse.close(find(rest, "</summary>", 0n), rest)

def parse_summary(+text: String) -> Maybe<&2, String>:
  parse.open(find(text, "<summary>", 0n), text)

# a compaction reply is valid only without tool calls
def valid_summary(text: String, calls: List<&2, T.Call>) -> Maybe<&2, String>:
  match calls:
    case Nil{}:
      parse_summary(text)
    case Con{_, _}:
      None{}

# ---- the pending projection ----
type PRes is Data:
  POk{input: List<&2, T.Msg>}
  PBig{}

# fixed implementation-owned compaction prompt; manual compaction appends
# the caller's instructions under <extra_compaction_instructions>
def prompt.with_extra(has_extra: Bool, +base: String, +extra: String) -> String:
  match has_extra:
    case True{}:
      String.append(base, String.append(
        "\n<extra_compaction_instructions>", String.append(extra,
          "</extra_compaction_instructions>")))
    case False{}:
      base

def compaction_prompt(+extra: String) -> String:
  prompt.with_extra(Bool.not(String.is_empty(extra)),
    "Summarize the conversation so far for the agent that continues it. Reply with a single <summary>...</summary> block containing a non-empty summary. Do not call tools.", extra)

def project.build(f: FitR, +head: List<&2, T.Msg>) -> PRes:
  match f:
    case FOk{ms}:
      POk{List.append(&2, T.Msg, head, ms)}
    case FBig{}:
      PBig{}

# [system, compaction prompt] ++ fitted history, within the threshold
def project(cfg: T.Cfg, history: List<&2, T.Msg>, +extra: String) -> PRes:
  match cfg:
    case T.Cfg{+threshold, _, _, +system, +tools}:
      +prompt = T.injected_user(compaction_prompt(extra))
      +sysmsg = T.sys(system)
      overhead = Nat.add(E.msg_tokens(sysmsg),
        Nat.add(E.msg_tokens(prompt), E.tool_tokens(tools)))
      budget = Nat.sub(threshold, overhead)
      project.build(fit_list(budget, history),
        Con{sysmsg, Con{prompt, Nil{}}})

# ---- the replacement context ----
type RRes is Data:
  ROk{history: List<&2, T.Msg>}
  RBig{}

# 1. the current system message (projected, not stored in canonical history)
# 2. an injected user-role preamble
# 3. recent non-injected user messages, preserved within the selection
#    budget, always including the most recent one
# 4. an injected final user message containing the accepted summary
def replace.build(f: FitR, +preamble: T.Msg, +final: T.Msg) -> RRes:
  match f:
    case FOk{sel}:
      ROk{List.append(&2, T.Msg, Con{preamble, sel}, Con{final, Nil{}})}
    case FBig{}:
      RBig{}
def replace(cfg: T.Cfg, history: List<&2, T.Msg>, summary: String) -> RRes:
  match cfg:
    case T.Cfg{+threshold, +select_budget, _, +system, +tools}:
      +sysmsg = T.sys(system)
      +preamble = T.injected_user("The earlier conversation was compacted. A summary replaces it; continue from the preserved user messages.")
      +final = T.injected_user(String.append(
        "Summary of the earlier conversation:\n<summary>", String.append(
          summary, "</summary>")))
      overhead = Nat.add(E.msg_tokens(sysmsg),
        Nat.add(E.msg_tokens(preamble),
          Nat.add(E.msg_tokens(final), E.tool_tokens(tools))))
      budget = Nat.sub(threshold, overhead)
      replace.build(fit_list(budget, H.preserve_recent(select_budget, history)),
        preamble, final)

# ---- session-creation validation ----
# a threshold that cannot fit the system message, tool catalog, and
# compaction prompt is rejected
def base_overhead(cfg: T.Cfg) -> Nat:
  match cfg:
    case T.Cfg{_, _, _, +system, +tools}:
      Nat.add(E.text_tokens(system),
        Nat.add(E.tool_tokens(tools), E.text_tokens(compaction_prompt(""))))

def valid_threshold(cfg: T.Cfg) -> Bool:
  match cfg:
    case T.Cfg{+threshold, _, _, _, _}:
      Nat.is_lt(base_overhead(cfg), threshold)
