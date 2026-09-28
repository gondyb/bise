# vendor/json.bend - hub package 0x1f4d6c03caf955232d0b0dc6e6f36cf4
# (pure RFC 8259 JSON; no LICENSE file, so MIT-0 under the hub terms),
# vendored so the build never depends on a patched ~/.bend/lib.
# One change from the published file, in check_safe_mul10_step: bend
# 2.0.31 removed Nat.read.fit / Nat.read.max from Base, so the bound is
# written out - acc <= (2^48 - 1 - d) / 10, with 2^48 - 1 built as
# 16777215 * 16777217 (Nat literals stop at 2^32 - 1).
# json.bend: A production-grade, highly reusable JSON library for Bend 2.
# Compliant with RFC 8259 (JSON specification) and RFC 6901 (JSON Pointer).
#
# Supports:
#   - Full JSON AST: Null, Bool, Number (Pos Nat, Neg Nat), String, Array, Object
#   - Full RFC 8259 character escaping and unescaping
#   - Safe type guards and typed value unwrapping
#   - Comprehensive Object operations: get, has, set, remove, obj_len, keys, values, entries, merge, sort_keys
#   - Functional Array combinators: arr_len, arr_get, arr_push, arr_set, arr_pop, arr_remove, arr_insert, arr_map, arr_filter, arr_fold
#   - Ergonomic typed field extractors with fallback defaults: get_str, get_num, get_bool, etc.
#   - RFC 6901 JSON Pointer queries and deep path updates: pointer, pointer_set
#   - High-performance compact serialization: stringify
#   - Configurable pretty printing with arbitrary indentation: pretty, pretty_indent
#   - Streaming NDJSON (Newline Delimited JSON): parse_ndjson, stringify_ndjson
#   - Fast JSON validation: validate
#   - Linear recursive-descent parser: parse

import Base

# ==============================================================================
# 1. AST Data Types
# ==============================================================================

type Json is Data:
  JNull{}
  JBool{val: Bool}
  JNum{val: Nat}
  JNeg{val: Nat}
  JStr{val: String}
  JArr{vals: List<&2, Json>}
  JObj{kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  JRaw{raw: String}

# Parser stack frames
type Frame is Data:
  FArr{items: List<&2, Json>}
  FObjVal{key: String, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}

# String parser types
type StrCharKind is Data:
  SCQuote{}
  SCSlash{}
  SCControl{}
  SCOther{}

type StrRes is Data:
  SErr{msg: String}
  SOk{val: String, rest: String}

type Hex4Res is Data:
  H4Ok{val: U32, rest: String}
  H4Err{msg: String}

type EscRes is Data:
  EscChar{c: Char, rest: String}
  EscErr{msg: String}

type EscCharKind is Data:
  ECQuote{}
  ECSlash{}
  ECNewline{}
  ECCarriage{}
  ECTab{}
  ECBackspace{}
  ECFormFeed{}
  ECControl{code: U32}
  ECOther{code: U32}

type StrState is Data:
  StrNormal{}
  StrEscaping{}
  StrEscCheck{res: EscRes}
  StrChar{kind: StrCharKind, c: U32}

type PtrUnescState is Data:
  PUNormal{}
  PUTilde{}
  PUChar{is_tilde: Bool, c: U32}
  PUTildeChar{is_one: Bool, is_zero: Bool, c: U32}

type PtrSplitState is Data:
  PSNormal{}
  PSChar{is_slash: Bool, c: U32}

type NatState is Data:
  NParseDigits{}
  NCheckDigit{is_d: Bool, c: U32}
  NCheckSafe{is_safe: Bool, d: Nat}

type LineSplitState is Data:
  LSNormal{}
  LSCheckNl{is_nl: Bool, c: U32}
  LSEmpty{is_empty: Bool, line: String}

type WsState is Data:
  WsStart{}
  WsChar{is_w: Bool, c: U32}

# Number parser result
type NumRes is Data:
  NInt{is_n: Bool, val: Nat, rest: String}
  NRaw{raw: String, rest: String}
  NErr{msg: String}

type NumCharKind is Data:
  NCZero{}
  NCNonZeroDigit{d: Nat}
  NCMinus{}
  NCPlus{}
  NCDot{}
  NCExp{}
  NCOther{c: U32}

type NumState is Data:
  NStart{}
  NStartChar{kind: NumCharKind, c: U32}
  NAfterMinus{}
  NAfterMinusChar{kind: NumCharKind, c: U32}
  NAfterZero{is_n: Bool}
  NAfterZeroChar{is_n: Bool, kind: NumCharKind, c: U32}
  NIntDigits{is_n: Bool, acc: Nat, is_over: Bool}
  NIntDigitsChar{is_n: Bool, acc: Nat, is_over: Bool, kind: NumCharKind, c: U32}
  NFracStart{}
  NFracStartChar{kind: NumCharKind, c: U32}
  NFracDigits{}
  NFracDigitsChar{kind: NumCharKind, c: U32}
  NExpStart{}
  NExpStartChar{kind: NumCharKind, c: U32}
  NExpSign{}
  NExpSignChar{kind: NumCharKind, c: U32}
  NExpDigits{}
  NExpDigitsChar{kind: NumCharKind, c: U32}

# General character classification
type CharKind is Data:
  KindLBracket{}
  KindRBracket{}
  KindLBrace{}
  KindRBrace{}
  KindQuote{}
  KindComma{}
  KindColon{}
  KindNull{}
  KindTrue{}
  KindFalse{}
  KindMinus{}
  KindDigit{}
  KindOther{}

# Parser state machine actions
type StepAct is Data:
  ActValStart{}
  ActValChar{kind: CharKind, c: U32}
  ActArrOpen{}
  ActArrOpenChar{kind: CharKind, c: U32}
  ActArrNext{v: Json, items: List<&2, Json>}
  ActArrNextChar{kind: CharKind, v: Json, items: List<&2, Json>}
  ActObjOpen{}
  ActObjOpenChar{kind: CharKind, c: U32}
  ActKeyStart{kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActKeyStartChar{kind: CharKind, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActKeyParsed{res: StrRes, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActColon{key: String, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActColonChar{kind: CharKind, key: String, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActObjNext{key: String, v: Json, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActObjNextChar{kind: CharKind, key: String, v: Json, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  ActHaveVal{v: Json}
  ActCheckEof{v: Json}
  ActStrParsed{res: StrRes}
  ActNumParsed{res: NumRes}
  ActLitCheck{expected: String, val: Json, ok: Bool, rest: String}

# Serializer stack task
type PrintTask is Data:
  PVal{j: Json}
  PArr{items: List<&2, Json>}
  PObj{kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}
  PLit{s: String}

# Pretty printer stack task
type PrettyTask is Data:
  PrVal{j: Json, indent: String}
  PrArr{items: List<&2, Json>, indent: String}
  PrObj{kvs: List<&2, Sigma<&2, &2, String, _ => Json>>, indent: String}
  PrLit{s: String}

# Size calculation stack task
type SizeTask is Data:
  SzVal{j: Json}
  SzArr{vals: List<&2, Json>}
  SzObj{kvs: List<&2, Sigma<&2, &2, String, _ => Json>>}


# ==============================================================================
# 2. Constructors and Builders
# ==============================================================================

def null() -> Json:
  JNull{}

def bool(b: Bool) -> Json:
  JBool{b}

def true_val() -> Json:
  JBool{True{}}

def false_val() -> Json:
  JBool{False{}}

def num(n: Nat) -> Json:
  JNum{n}

def neg(n: Nat) -> Json:
  JNeg{n}

def str(s: String) -> Json:
  JStr{s}

def arr(vals: List<&2, Json>) -> Json:
  JArr{vals}

def obj(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> Json:
  JObj{kvs}

def kv(key: String, val: Json) -> Sigma<&2, &2, String, _ => Json>:
  Tuple{key, val}

def raw(s: String) -> Json:
  JRaw{s}

# ==============================================================================
# 3. Type Checking and Guards
# ==============================================================================

def is_null(j: Json) -> Bool:
  match j:
    case JNull{}:
      True{}
    case _:
      False{}

def is_bool(j: Json) -> Bool:
  match j:
    case JBool{_}:
      True{}
    case _:
      False{}

def is_num(j: Json) -> Bool:
  match j:
    case JNum{_}:
      True{}
    case _:
      False{}

def is_neg(j: Json) -> Bool:
  match j:
    case JNeg{_}:
      True{}
    case _:
      False{}

def is_str(j: Json) -> Bool:
  match j:
    case JStr{_}:
      True{}
    case _:
      False{}

def is_arr(j: Json) -> Bool:
  match j:
    case JArr{_}:
      True{}
    case _:
      False{}

def is_obj(j: Json) -> Bool:
  match j:
    case JObj{_}:
      True{}
    case _:
      False{}

def is_raw(j: Json) -> Bool:
  match j:
    case JRaw{_}:
      True{}
    case _:
      False{}

# ==============================================================================
# 4. Value Extraction
# ==============================================================================

def as_bool(j: Json) -> Maybe<&2, Bool>:
  match j:
    case JBool{b}:
      Some{b}
    case _:
      None{}

def as_num(j: Json) -> Maybe<&2, Nat>:
  match j:
    case JNum{n}:
      Some{n}
    case _:
      None{}

def as_neg(j: Json) -> Maybe<&2, Nat>:
  match j:
    case JNeg{n}:
      Some{n}
    case _:
      None{}

def as_str(j: Json) -> Maybe<&2, String>:
  match j:
    case JStr{s}:
      Some{s}
    case _:
      None{}

def as_arr(j: Json) -> Maybe<&2, List<&2, Json>>:
  match j:
    case JArr{vals}:
      Some{vals}
    case _:
      None{}

def as_obj(j: Json) -> Maybe<&2, List<&2, Sigma<&2, &2, String, _ => Json>>>:
  match j:
    case JObj{kvs}:
      Some{kvs}
    case _:
      None{}

def as_raw(j: Json) -> Maybe<&2, String>:
  match j:
    case JRaw{s}:
      Some{s}
    case _:
      None{}

# ==============================================================================
# 5. Object Operations & Dictionary Utilities
# ==============================================================================

def get_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>, +key: String) -> Maybe<&2, Json>:
  match kvs:
    case Nil{}:
      None{}
    case Tuple{k, +v} <> t:
      Bool.pick(Maybe<&2, Json>, String.eq(k, key),
        Some{v},
        get_kv(t, key))

def get(j: Json, key: String) -> Maybe<&2, Json>:
  match j:
    case JObj{kvs}:
      get_kv(kvs, key)
    case _:
      None{}

def has.is_some(m: Maybe<&2, Json>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def has(j: Json, key: String) -> Bool:
  has.is_some(get(j, key))

def set_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>, +key: String, +val: Json) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match kvs:
    case Nil{}:
      [Tuple{key, val}]
    case Tuple{+k, +v} <> +t:
      Bool.pick(List<&2, Sigma<&2, &2, String, _ => Json>>, String.eq(k, key),
        Tuple{key, val} <> t,
        Tuple{k, v} <> set_kv(t, key, val))

def set(j: Json, key: String, val: Json) -> Json:
  match j:
    case JObj{kvs}:
      JObj{set_kv(kvs, key, val)}
    case _:
      JObj{[Tuple{key, val}]}

def remove_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>, +key: String) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match kvs:
    case Nil{}:
      Nil{}
    case Tuple{+k, +v} <> +t:
      Bool.pick(List<&2, Sigma<&2, &2, String, _ => Json>>, String.eq(k, key),
        t,
        Tuple{k, v} <> remove_kv(t, key))

def remove(j: Json, key: String) -> Json:
  match j:
    case JObj{kvs}:
      JObj{remove_kv(kvs, key)}
    case _:
      j

def obj_len_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> Nat:
  match kvs:
    case Nil{}:
      0n
    case _ <> t:
      1n+obj_len_kv(t)

def obj_len(j: Json) -> Nat:
  match j:
    case JObj{kvs}:
      obj_len_kv(kvs)
    case _:
      0n

def keys_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, String>:
  match kvs:
    case Nil{}:
      Nil{}
    case Tuple{k, _} <> t:
      k <> keys_kv(t)

def keys(j: Json) -> List<&2, String>:
  match j:
    case JObj{kvs}:
      keys_kv(kvs)
    case _:
      Nil{}

def values_kv(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, Json>:
  match kvs:
    case Nil{}:
      Nil{}
    case Tuple{_, v} <> t:
      v <> values_kv(t)

def values(j: Json) -> List<&2, Json>:
  match j:
    case JObj{kvs}:
      values_kv(kvs)
    case _:
      Nil{}

def entries(j: Json) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match j:
    case JObj{kvs}:
      kvs
    case _:
      Nil{}

def merge_kvs(overrides: List<&2, Sigma<&2, &2, String, _ => Json>>, base: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match overrides:
    case Nil{}:
      base
    case Tuple{k, v} <> t:
      merge_kvs(t, set_kv(base, k, v))

def merge(j1: Json, j2: Json) -> Json:
  match j1:
    case JObj{kvs1}:
      match j2:
        case JObj{kvs2}:
          JObj{merge_kvs(kvs2, kvs1)}
        case _:
          j2
    case _:
      j2

def insert_kv_go(+ek: String, +ev: Json, +kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match kvs:
    case Nil{}:
      [Tuple{ek, ev}]
    case Tuple{+k, +v} <> +t:
      Bool.pick(List<&2, Sigma<&2, &2, String, _ => Json>>, String.is_le(ek, k),
        Tuple{ek, ev} <> Tuple{k, v} <> t,
        Tuple{k, v} <> insert_kv_go(ek, ev, t))

def insert_kv(elem: Sigma<&2, &2, String, _ => Json>, kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  Tuple{ek, ev} = elem
  insert_kv_go(ek, ev, kvs)

def sort_kvs(kvs: List<&2, Sigma<&2, &2, String, _ => Json>>) -> List<&2, Sigma<&2, &2, String, _ => Json>>:
  match kvs:
    case Nil{}:
      Nil{}
    case h <> t:
      insert_kv(h, sort_kvs(t))

def sort_keys(j: Json) -> Json:
  match j:
    case JObj{kvs}:
      JObj{sort_kvs(kvs)}
    case _:
      j

def to_map(j: Json) -> Maybe<&2, Map<&2, Json>>:
  match j:
    case JObj{kvs}:
      Some{Map.from_list(&2, Json, kvs)}
    case _:
      None{}

def from_map(m: Map<&2, Json>) -> Json:
  JObj{Map.to_list(&2, Json, m)}

# ==============================================================================
# 6. Array Operations & Functional Combinators
# ==============================================================================

def arr_len_list(vals: List<&2, Json>) -> Nat:
  match vals:
    case Nil{}:
      0n
    case _ <> t:
      1n+arr_len_list(t)

def arr_len(j: Json) -> Nat:
  match j:
    case JArr{vals}:
      arr_len_list(vals)
    case _:
      0n

def arr_get_list(vals: List<&2, Json>, idx: Nat) -> Maybe<&2, Json>:
  match vals:
    case Nil{}:
      None{}
    case h <> t:
      match idx:
        case 0n:
          Some{h}
        case 1n+p:
          arr_get_list(t, p)

def arr_get(j: Json, idx: Nat) -> Maybe<&2, Json>:
  match j:
    case JArr{vals}:
      arr_get_list(vals, idx)
    case _:
      None{}

def arr_push(j: Json, elem: Json) -> Json:
  match j:
    case JArr{vals}:
      JArr{List.append(&2, Json, vals, [elem])}
    case _:
      JArr{[elem]}

def list_set_idx(xs: List<&2, Json>, idx: Nat, +val: Json) -> List<&2, Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> +t:
      match idx:
        case 0n:
          val <> t
        case 1n+p:
          h <> list_set_idx(t, p, val)

def arr_set(j: Json, idx: Nat, val: Json) -> Json:
  match j:
    case JArr{vals}:
      JArr{list_set_idx(vals, idx, val)}
    case _:
      j

def list_pop_step(h: Json, res: Pair(Maybe<&2, Json>, List<&2, Json>)) -> Pair(Maybe<&2, Json>, List<&2, Json>):
  Tuple{m, rem} = res
  Tuple{m, h <> rem}

def list_pop_last(xs: List<&2, Json>) -> Pair(Maybe<&2, Json>, List<&2, Json>):
  match xs:
    case Nil{}:
      Tuple{None{}, Nil{}}
    case h <> t:
      match t:
        case Nil{}:
          Tuple{Some{h}, Nil{}}
        case th <> tt:
          list_pop_step(h, list_pop_last(th <> tt))

def arr_pop_step(res: Pair(Maybe<&2, Json>, List<&2, Json>)) -> Pair(Maybe<&2, Json>, Json):
  Tuple{m, rem} = res
  Tuple{m, JArr{rem}}

def arr_pop(j: Json) -> Pair(Maybe<&2, Json>, Json):
  match j:
    case JArr{vals}:
      arr_pop_step(list_pop_last(vals))
    case _:
      Tuple{None{}, j}

def list_remove(xs: List<&2, Json>, idx: Nat) -> List<&2, Json>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      match idx:
        case 0n:
          t
        case 1n+p:
          h <> list_remove(t, p)

def arr_remove(j: Json, idx: Nat) -> Json:
  match j:
    case JArr{vals}:
      JArr{list_remove(vals, idx)}
    case _:
      j

def list_insert(xs: List<&2, Json>, idx: Nat, +val: Json) -> List<&2, Json>:
  match xs:
    case Nil{}:
      [val]
    case h <> t:
      match idx:
        case 0n:
          val <> h <> t
        case 1n+p:
          h <> list_insert(t, p, val)

def arr_insert(j: Json, idx: Nat, val: Json) -> Json:
  match j:
    case JArr{vals}:
      JArr{list_insert(vals, idx, val)}
    case _:
      JArr{[val]}

def list_map(~f: Json -> Json, xs: List<&2, Json>) -> List<&2, Json>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      f(h) <> list_map(~f, t)

def arr_map(~f: Json -> Json, j: Json) -> Json:
  match j:
    case JArr{vals}:
      JArr{list_map(~f, vals)}
    case _:
      j

def list_filter(~f: Json -> Bool, +xs: List<&2, Json>) -> List<&2, Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> +t:
      Bool.pick(List<&2, Json>, f(h),
        h <> list_filter(~f, t),
        list_filter(~f, t))

def arr_filter(~f: Json -> Bool, j: Json) -> Json:
  match j:
    case JArr{vals}:
      JArr{list_filter(~f, vals)}
    case _:
      j

def list_fold(~f: Json -> Json -> Json, xs: List<&2, Json>, acc: Json) -> Json:
  match xs:
    case Nil{}:
      acc
    case h <> t:
      list_fold(~f, t, f(acc, h))

def arr_fold(~f: Json -> Json -> Json, acc: Json, j: Json) -> Json:
  match j:
    case JArr{vals}:
      list_fold(~f, vals, acc)
    case _:
      acc

# ==============================================================================
# 7. Ergonomic Typed Field Extractors
# ==============================================================================

def get_str_step(m: Maybe<&2, Json>) -> Maybe<&2, String>:
  match m:
    case Some{v}:
      as_str(v)
    case None{}:
      None{}

def get_str(j: Json, key: String) -> Maybe<&2, String>:
  get_str_step(get(j, key))

def get_num_step(m: Maybe<&2, Json>) -> Maybe<&2, Nat>:
  match m:
    case Some{v}:
      as_num(v)
    case None{}:
      None{}

def get_num(j: Json, key: String) -> Maybe<&2, Nat>:
  get_num_step(get(j, key))

def get_neg_step(m: Maybe<&2, Json>) -> Maybe<&2, Nat>:
  match m:
    case Some{v}:
      as_neg(v)
    case None{}:
      None{}

def get_neg(j: Json, key: String) -> Maybe<&2, Nat>:
  get_neg_step(get(j, key))

def get_bool_step(m: Maybe<&2, Json>) -> Maybe<&2, Bool>:
  match m:
    case Some{v}:
      as_bool(v)
    case None{}:
      None{}

def get_bool(j: Json, key: String) -> Maybe<&2, Bool>:
  get_bool_step(get(j, key))

def get_arr_step(m: Maybe<&2, Json>) -> Maybe<&2, List<&2, Json>>:
  match m:
    case Some{v}:
      as_arr(v)
    case None{}:
      None{}

def get_arr(j: Json, key: String) -> Maybe<&2, List<&2, Json>>:
  get_arr_step(get(j, key))

def get_obj_step(m: Maybe<&2, Json>) -> Maybe<&2, List<&2, Sigma<&2, &2, String, _ => Json>>>:
  match m:
    case Some{v}:
      as_obj(v)
    case None{}:
      None{}

def get_obj(j: Json, key: String) -> Maybe<&2, List<&2, Sigma<&2, &2, String, _ => Json>>>:
  get_obj_step(get(j, key))

def get_str_or_step(m: Maybe<&2, String>, dflt: String) -> String:
  match m:
    case Some{s}:
      s
    case None{}:
      dflt

def get_str_or(j: Json, key: String, dflt: String) -> String:
  get_str_or_step(get_str(j, key), dflt)

def get_num_or_step(m: Maybe<&2, Nat>, dflt: Nat) -> Nat:
  match m:
    case Some{n}:
      n
    case None{}:
      dflt

def get_num_or(j: Json, key: String, dflt: Nat) -> Nat:
  get_num_or_step(get_num(j, key), dflt)

def get_bool_or_step(m: Maybe<&2, Bool>, dflt: Bool) -> Bool:
  match m:
    case Some{b}:
      b
    case None{}:
      dflt

def get_bool_or(j: Json, key: String, dflt: Bool) -> Bool:
  get_bool_or_step(get_bool(j, key), dflt)

# ==============================================================================
# 8. RFC 6901 JSON Pointer Traversal & Deep Mutation
# ==============================================================================

def str_len_loop(s: String, acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{_, t}:
      str_len_loop(t, 1n+acc)

def str_len(s: String) -> Nat:
  str_len_loop(s, 0n)

def str_from_rev_loop(cs: List<&2, Char>, cur: String) -> String:
  match cs:
    case Nil{}:
      cur
    case c <> t:
      str_from_rev_loop(t, SCon{c, cur})

def unescape_ptr_loop(fuel: Nat, st: PtrUnescState, s: String, acc: List<&2, Char>) -> String:
  match fuel:
    case 0n:
      str_from_rev_loop(acc, "")
    case 1n++p:
      match st:
        case PUNormal{}:
          match s:
            case SNil{}:
              str_from_rev_loop(acc, "")
            case SCon{Chr{+c}, +t}:
              unescape_ptr_loop(p, PUChar{U32.is_eq(c, 126), c}, t, acc)
        case PUChar{is_tilde, c}:
          match is_tilde:
            case True{}:
              unescape_ptr_loop(p, PUTilde{}, s, acc)
            case False{}:
              unescape_ptr_loop(p, PUNormal{}, s, Chr{c} <> acc)
        case PUTilde{}:
          match s:
            case SNil{}:
              str_from_rev_loop(Chr{126} <> acc, "")
            case SCon{Chr{+c}, +t}:
              unescape_ptr_loop(p, PUTildeChar{U32.is_eq(c, 49), U32.is_eq(c, 48), c}, t, acc)
        case PUTildeChar{is_one, is_zero, c}:
          match is_one:
            case True{}:
              unescape_ptr_loop(p, PUNormal{}, s, Chr{47} <> acc)
            case False{}:
              match is_zero:
                case True{}:
                  unescape_ptr_loop(p, PUNormal{}, s, Chr{126} <> acc)
                case False{}:
                  unescape_ptr_loop(p, PUNormal{}, s, Chr{c} <> Chr{126} <> acc)

def unescape_ptr_token(+s: String) -> String:
  unescape_ptr_loop(Nat.add(Nat.mul(str_len(s), 2n), 10n), PUNormal{}, s, Nil{})

def split_ptr_loop(fuel: Nat, st: PtrSplitState, s: String, +cur: String) -> List<&2, String>:
  match fuel:
    case 0n:
      [unescape_ptr_token(String.reverse(cur))]
    case 1n++p:
      match st:
        case PSNormal{}:
          match s:
            case SNil{}:
              [unescape_ptr_token(String.reverse(cur))]
            case SCon{Chr{+c}, +t}:
              split_ptr_loop(p, PSChar{U32.is_eq(c, 47), c}, t, cur)
        case PSChar{is_slash, c}:
          match is_slash:
            case True{}:
              unescape_ptr_token(String.reverse(cur)) <> split_ptr_loop(p, PSNormal{}, s, "")
            case False{}:
              split_ptr_loop(p, PSNormal{}, s, SCon{Chr{c}, cur})

def split_ptr_tokens(+s: String) -> List<&2, String>:
  split_ptr_loop(Nat.add(Nat.mul(str_len(s), 2n), 10n), PSNormal{}, s, "")

def parse_ptr_tokens_slash(is_slash: Bool, +t: String) -> Maybe<&2, List<&2, String>>:
  match is_slash:
    case True{}:
      Some{split_ptr_tokens(t)}
    case False{}:
      None{}

def parse_ptr_tokens(+path: String) -> Maybe<&2, List<&2, String>>:
  match path:
    case SNil{}:
      Some{Nil{}}
    case SCon{Chr{+c}, +t}:
      parse_ptr_tokens_slash(U32.is_eq(c, 47), t)

def check_safe_mul10_step(is_small: Bool, acc: Nat, d: Nat) -> Bool:
  match is_small:
    case True{}:
      True{}
    case False{}:
      Nat.is_le(acc, Nat.div(Nat.sub(Nat.mul(16777215n, 16777217n), d), 10n))

def check_safe_mul10(+acc: Nat, d: Nat) -> Bool:
  check_safe_mul10_step(Nat.is_le(acc, 10000n), acc, d)

def parse_nat_loop(fuel: Nat, st: NatState, s: String, +acc: Nat) -> Maybe<&2, Nat>:
  match fuel:
    case 0n:
      Some{acc}
    case 1n++p:
      match st:
        case NParseDigits{}:
          match s:
            case SNil{}:
              Some{acc}
            case SCon{Chr{+c}, +t}:
              parse_nat_loop(p, NCheckDigit{Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), c}, t, acc)
        case NCheckDigit{is_d, c}:
          match is_d:
            case True{}:
              +d = U32.to_nat(U32.sub(c, 48))
              parse_nat_loop(p, NCheckSafe{check_safe_mul10(acc, d), d}, s, acc)
            case False{}:
              None{}
        case NCheckSafe{is_safe, d}:
          match is_safe:
            case True{}:
              parse_nat_loop(p, NParseDigits{}, s, Nat.add(Nat.mul(acc, 10n), d))
            case False{}:
              None{}

def parse_nat_lead(is_zero: Bool, is_digit: Bool, +c: U32, +t: String) -> Maybe<&2, Nat>:
  match is_zero:
    case True{}:
      match t:
        case SNil{}:
          Some{0n}
        case _:
          None{}
    case False{}:
      match is_digit:
        case True{}:
          +fuel = {Nat.add(Nat.mul(str_len(t), 2n), 10n) : Nat}
          parse_nat_loop(fuel, NParseDigits{}, t, U32.to_nat(U32.sub(c, 48)))
        case False{}:
          None{}

def parse_nat(+s: String) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      None{}
    case SCon{Chr{+c}, +t}:
      is_zero = U32.is_eq(c, 48)
      is_digit = Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57))
      parse_nat_lead(is_zero, is_digit, c, t)

def pointer_resolve_arr(j: Json, m: Maybe<&2, Nat>) -> Maybe<&2, Json>:
  match m:
    case Some{idx}:
      arr_get(j, idx)
    case None{}:
      None{}

def pointer_resolve_token(j: Json, token: String) -> Maybe<&2, Json>:
  match j:
    case JObj{_}:
      get(j, token)
    case JArr{_}:
      pointer_resolve_arr(j, parse_nat(token))
    case _:
      None{}

def pointer_traverse_loop(+tokens: List<&2, String>, cur: Maybe<&2, Json>) -> Maybe<&2, Json>:
  match tokens:
    case Nil{}:
      cur
    case token <> rest:
      match cur:
        case None{}:
          None{}
        case Some{j}:
          pointer_traverse_loop(rest, pointer_resolve_token(j, token))

def pointer_start(m: Maybe<&2, List<&2, String>>, j: Json) -> Maybe<&2, Json>:
  match m:
    case Some{tokens}:
      pointer_traverse_loop(tokens, Some{j})
    case None{}:
      None{}

def pointer(j: Json, path: String) -> Maybe<&2, Json>:
  pointer_start(parse_ptr_tokens(path), j)

def pointer_set_arr_leaf(m: Maybe<&2, Nat>, j: Json, val: Json) -> Json:
  match m:
    case Some{idx}:
      arr_set(j, idx, val)
    case None{}:
      j

def child_or_obj(m: Maybe<&2, Json>) -> Json:
  match m:
    case Some{c}:
      c
    case None{}:
      JObj{Nil{}}

def child_or_null(m: Maybe<&2, Json>) -> Json:
  match m:
    case Some{c}:
      c
    case None{}:
      JNull{}

def arr_child(j: Json, m: Maybe<&2, Nat>) -> Json:
  match m:
    case Some{idx}:
      child_or_null(arr_get(j, idx))
    case None{}:
      JNull{}

def pointer_set_arr_res(m: Maybe<&2, Nat>, j: Json, new_child: Json) -> Json:
  match m:
    case Some{idx}:
      arr_set(j, idx, new_child)
    case None{}:
      j

def pointer_set_traverse(+tokens: List<&2, String>, +j: Json, +val: Json) -> Json:
  match tokens:
    case Nil{}:
      val
    case +token <> rest:
      match rest:
        case Nil{}:
          match j:
            case JObj{_}:
              set(j, token, val)
            case JArr{_}:
              pointer_set_arr_leaf(parse_nat(token), j, val)
            case _:
              j
        case _ <> _:
          match j:
            case JObj{_}:
              +child = child_or_obj(get(j, token))
              set(j, token, pointer_set_traverse(rest, child, val))
            case JArr{_}:
              +idx_m = parse_nat(token)
              +child = arr_child(j, idx_m)
              pointer_set_arr_res(idx_m, j, pointer_set_traverse(rest, child, val))
            case _:
              j

def pointer_set_step(m: Maybe<&2, List<&2, String>>, j: Json, val: Json) -> Json:
  match m:
    case Some{tokens}:
      pointer_set_traverse(tokens, j, val)
    case None{}:
      j

def pointer_set(j: Json, path: String, val: Json) -> Json:
  pointer_set_step(parse_ptr_tokens(path), j, val)

# ==============================================================================
# 9. String Escaping & Serialization (stringify)
# ==============================================================================

def hex_to_char(+d: U32) -> Char:
  Bool.pick(Char, U32.is_lt(d, 10),
    {Chr{U32.add(d, 48)} : Char},
    {Chr{U32.add(U32.sub(d, 10), 97)} : Char})

def escape_char(+c: Char) -> String:
  match c:
    case Chr{+code}:
      Bool.pick(String, U32.is_eq(code, 34), "\\\"",
        Bool.pick(String, U32.is_eq(code, 92), "\\\\",
          Bool.pick(String, U32.is_eq(code, 8), "\\b",
            Bool.pick(String, U32.is_eq(code, 12), "\\f",
              Bool.pick(String, U32.is_eq(code, 10), "\\n",
                Bool.pick(String, U32.is_eq(code, 13), "\\r",
                  Bool.pick(String, U32.is_eq(code, 9), "\\t",
                    Bool.pick(String, U32.is_lt(code, 32),
                      String.from_list([{Chr{92}:Char}, {Chr{117}:Char}, {Chr{48}:Char}, {Chr{48}:Char}, hex_to_char(U32.div(code, 16)), hex_to_char(U32.mod(code, 16))]),
                      Char.show(Chr{code})))))))))

def classify_esc_char(+c: U32) -> EscCharKind:
  Bool.pick(EscCharKind, U32.is_eq(c, 34), ECQuote{},
    Bool.pick(EscCharKind, U32.is_eq(c, 92), ECSlash{},
      Bool.pick(EscCharKind, U32.is_eq(c, 10), ECNewline{},
        Bool.pick(EscCharKind, U32.is_eq(c, 13), ECCarriage{},
          Bool.pick(EscCharKind, U32.is_eq(c, 9), ECTab{},
            Bool.pick(EscCharKind, U32.is_eq(c, 8), ECBackspace{},
              Bool.pick(EscCharKind, U32.is_eq(c, 12), ECFormFeed{},
                Bool.pick(EscCharKind, U32.is_lt(c, 32), ECControl{c},
                  ECOther{c}))))))))

def escape_step(kind: EscCharKind, acc: List<&2, Char>) -> List<&2, Char>:
  match kind:
    case ECQuote{}:
      {Chr{34}:Char} <> {Chr{92}:Char} <> acc
    case ECSlash{}:
      {Chr{92}:Char} <> {Chr{92}:Char} <> acc
    case ECNewline{}:
      {Chr{110}:Char} <> {Chr{92}:Char} <> acc
    case ECCarriage{}:
      {Chr{114}:Char} <> {Chr{92}:Char} <> acc
    case ECTab{}:
      {Chr{116}:Char} <> {Chr{92}:Char} <> acc
    case ECBackspace{}:
      {Chr{98}:Char} <> {Chr{92}:Char} <> acc
    case ECFormFeed{}:
      {Chr{102}:Char} <> {Chr{92}:Char} <> acc
    case ECControl{+code}:
      hex_to_char(U32.mod(code, 16)) <> hex_to_char(U32.div(code, 16)) <> {Chr{48}:Char} <> {Chr{48}:Char} <> {Chr{117}:Char} <> {Chr{92}:Char} <> acc
    case ECOther{code}:
      Chr{code} <> acc

def escape_str_loop(fuel: Nat, s: String, acc: List<&2, Char>) -> String:
  match fuel:
    case 0n:
      str_from_rev_loop(acc, "")
    case 1n++p:
      match s:
        case SNil{}:
          str_from_rev_loop(acc, "")
        case SCon{Chr{+code}, t}:
          escape_str_loop(p, t, escape_step(classify_esc_char(code), acc))

def escape_string(+s: String) -> String:
  +len = str_len(s)
  +fuel = Nat.add(len, 10n)
  escape_str_loop(fuel, s, Nil{})

def quote_string(+s: String) -> String:
  +len = str_len(s)
  +fuel = Nat.add(len, 10n)
  "\"" ++ escape_str_loop(fuel, s, Nil{}) ++ "\""

def json_size_loop(fuel: Nat, +tasks: List<&2, SizeTask>, +acc: Nat) -> Nat:
  match fuel:
    case 0n:
      acc
    case 1n++p:
      match tasks:
        case Nil{}:
          acc
        case h <> t:
          match h:
            case SzVal{j}:
              match j:
                case JNull{}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
                case JBool{_}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
                case JNum{_}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
                case JNeg{_}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
                case JStr{_}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
                case JArr{vals}:
                  json_size_loop(p, SzArr{vals} <> t, Nat.add(acc, 1n))
                case JObj{kvs}:
                  json_size_loop(p, SzObj{kvs} <> t, Nat.add(acc, 1n))
                case JRaw{_}:
                  json_size_loop(p, t, Nat.add(acc, 1n))
            case SzArr{items}:
              match items:
                case Nil{}:
                  json_size_loop(p, t, acc)
                case vh <> vt:
                  json_size_loop(p, SzVal{vh} <> SzArr{vt} <> t, acc)
            case SzObj{kvs}:
              match kvs:
                case Nil{}:
                  json_size_loop(p, t, acc)
                case Tuple{_, v} <> kt:
                  json_size_loop(p, SzVal{v} <> SzObj{kt} <> t, acc)

def json_size(j: Json) -> Nat:
  json_size_loop(4294967295n, [SzVal{j}], 0n)

def join_chunks_loop(fuel: Nat, xs: List<&2, String>, cur: String) -> String:
  match fuel:
    case 0n:
      cur
    case 1n++p:
      match xs:
        case Nil{}:
          cur
        case h <> t:
          join_chunks_loop(p, t, h ++ cur)

def stringify_loop(fuel: Nat, +tasks: List<&2, PrintTask>, +acc: List<&2, String>) -> Result<&2, &2, String, String>:
  match fuel:
    case 0n:
      match tasks:
        case Nil{}:
          Done{join_chunks_loop(4294967295n, acc, "")}
        case _:
          Fail{"fuel exhausted in stringify"}
    case 1n++p:
      match tasks:
        case Nil{}:
          Done{join_chunks_loop(4294967295n, acc, "")}
        case h <> t:
          match h:
            case PLit{s}:
              stringify_loop(p, t, s <> acc)
            case PVal{j}:
              match j:
                case JNull{}:
                  stringify_loop(p, t, "null" <> acc)
                case JBool{b}:
                  match b:
                    case True{}:
                      stringify_loop(p, t, "true" <> acc)
                    case False{}:
                      stringify_loop(p, t, "false" <> acc)
                case JNum{n}:
                  stringify_loop(p, t, Nat.show(n) <> acc)
                case JNeg{n}:
                  stringify_loop(p, t, Nat.show(n) <> "-" <> acc)
                case JStr{s}:
                  stringify_loop(p, t, quote_string(s) <> acc)
                case JArr{vals}:
                  match vals:
                    case Nil{}:
                      stringify_loop(p, t, "[]" <> acc)
                    case vh <> vt:
                      stringify_loop(p, PVal{vh} <> PArr{vt} <> PLit{"]"} <> t, "[" <> acc)
                case JObj{kvs}:
                  match kvs:
                    case Nil{}:
                      stringify_loop(p, t, "{}" <> acc)
                    case Tuple{k, v} <> kt:
                      stringify_loop(p, PVal{v} <> PObj{kt} <> PLit{"}"} <> t, ":" <> quote_string(k) <> "{" <> acc)
                case JRaw{s}:
                  stringify_loop(p, t, s <> acc)
            case PArr{items}:
              match items:
                case Nil{}:
                  stringify_loop(p, t, acc)
                case vh <> vt:
                  stringify_loop(p, PVal{vh} <> PArr{vt} <> t, "," <> acc)
            case PObj{kvs}:
              match kvs:
                case Nil{}:
                  stringify_loop(p, t, acc)
                case Tuple{k, v} <> kt:
                  stringify_loop(p, PVal{v} <> PObj{kt} <> t, ":" <> quote_string(k) <> "," <> acc)

def stringify_checked(+j: Json) -> Result<&2, &2, String, String>:
  +size = json_size(j)
  +fuel = Nat.add(Nat.mul(size, 4n), 16n)
  stringify_loop(fuel, [PVal{j}], Nil{})

def stringify_unwrap(r: Result<&2, &2, String, String>) -> String:
  match r:
    case Done{s}:
      s
    case Fail{_}:
      ""

def stringify(j: Json) -> String:
  stringify_unwrap(stringify_checked(j))

# ==============================================================================
# 10. Pretty Printing with Configurable Indentation
# ==============================================================================

def make_spaces(n: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n+p:
      " " ++ make_spaces(p)

def make_custom_indent(count: Nat, +step_str: String) -> String:
  match count:
    case 0n:
      ""
    case 1n+cp:
      step_str ++ make_custom_indent(cp, step_str)

def pretty_indent_loop(fuel: Nat, +tasks: List<&2, PrettyTask>, +step_str: String, +acc: List<&2, String>) -> String:
  match fuel:
    case 0n:
      join_chunks_loop(4294967295n, acc, "")
    case 1n++p:
      match tasks:
        case Nil{}:
          join_chunks_loop(4294967295n, acc, "")
        case h <> t:
          match h:
            case PrLit{s}:
              pretty_indent_loop(p, t, step_str, s <> acc)
            case PrVal{j, +ind}:
              match j:
                case JNull{}:
                  pretty_indent_loop(p, t, step_str, "null" <> acc)
                case JBool{b}:
                  match b:
                    case True{}:
                      pretty_indent_loop(p, t, step_str, "true" <> acc)
                    case False{}:
                      pretty_indent_loop(p, t, step_str, "false" <> acc)
                case JNum{n}:
                  pretty_indent_loop(p, t, step_str, Nat.show(n) <> acc)
                case JNeg{n}:
                  pretty_indent_loop(p, t, step_str, Nat.show(n) <> "-" <> acc)
                case JStr{s}:
                  pretty_indent_loop(p, t, step_str, quote_string(s) <> acc)
                case JArr{vals}:
                  match vals:
                    case Nil{}:
                      pretty_indent_loop(p, t, step_str, "[]" <> acc)
                    case vh <> vt:
                      +next_ind = {ind ++ step_str : String}
                      pretty_indent_loop(p, PrVal{vh, next_ind} <> PrArr{vt, next_ind} <> PrLit{"\n" ++ ind ++ "]"} <> t, step_str, ("[\n" ++ next_ind) <> acc)
                case JObj{kvs}:
                  match kvs:
                    case Nil{}:
                      pretty_indent_loop(p, t, step_str, "{}" <> acc)
                    case Tuple{k, v} <> kt:
                      +next_ind = {ind ++ step_str : String}
                      pretty_indent_loop(p, PrVal{v, next_ind} <> PrObj{kt, next_ind} <> PrLit{"\n" ++ ind ++ "}"} <> t, step_str, (": ") <> quote_string(k) <> ("{\n" ++ next_ind) <> acc)
                case JRaw{s}:
                  pretty_indent_loop(p, t, step_str, s <> acc)
            case PrArr{items, +ind}:
              match items:
                case Nil{}:
                  pretty_indent_loop(p, t, step_str, acc)
                case vh <> vt:
                  pretty_indent_loop(p, PrVal{vh, ind} <> PrArr{vt, ind} <> t, step_str, (",\n" ++ ind) <> acc)
            case PrObj{kvs, +ind}:
              match kvs:
                case Nil{}:
                  pretty_indent_loop(p, t, step_str, acc)
                case Tuple{k, v} <> kt:
                  pretty_indent_loop(p, PrVal{v, ind} <> PrObj{kt, ind} <> t, step_str, (": ") <> quote_string(k) <> (",\n" ++ ind) <> acc)

def pretty_indent(+j: Json, step: Nat) -> String:
  +size = json_size(j)
  +fuel = Nat.add(Nat.mul(size, 4n), 16n)
  pretty_indent_loop(fuel, [PrVal{j, ""}], make_spaces(step), Nil{})

def pretty(j: Json) -> String:
  pretty_indent(j, 2n)

# ==============================================================================
# 11. Parser Implementation (parse)
# ==============================================================================

def classify_str_char(+c: U32) -> StrCharKind:
  Bool.pick(StrCharKind, U32.is_eq(c, 34), SCQuote{},
    Bool.pick(StrCharKind, U32.is_eq(c, 92), SCSlash{},
      Bool.pick(StrCharKind, U32.is_lt(c, 32), SCControl{},
        SCOther{})))

def classify_char(+c: U32) -> CharKind:
  Bool.pick(CharKind, U32.is_eq(c, 91), KindLBracket{},
    Bool.pick(CharKind, U32.is_eq(c, 93), KindRBracket{},
      Bool.pick(CharKind, U32.is_eq(c, 123), KindLBrace{},
        Bool.pick(CharKind, U32.is_eq(c, 125), KindRBrace{},
          Bool.pick(CharKind, U32.is_eq(c, 34), KindQuote{},
            Bool.pick(CharKind, U32.is_eq(c, 44), KindComma{},
              Bool.pick(CharKind, U32.is_eq(c, 58), KindColon{},
                Bool.pick(CharKind, U32.is_eq(c, 110), KindNull{},
                  Bool.pick(CharKind, U32.is_eq(c, 116), KindTrue{},
                    Bool.pick(CharKind, U32.is_eq(c, 102), KindFalse{},
                      Bool.pick(CharKind, U32.is_eq(c, 45), KindMinus{},
                        Bool.pick(CharKind, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), KindDigit{},
                          KindOther{}))))))))))))

def hex_digit_val(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), U32.sub(c, 48),
    Bool.pick(U32, Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102)), U32.add(U32.sub(c, 97), 10),
      Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 70)), U32.add(U32.sub(c, 65), 10),
        255)))

def make_hex4(+d1: U32, +d2: U32, +d3: U32, +d4: U32, rest: String) -> Hex4Res:
  valid = Bool.and(U32.is_lt(d1, 16), Bool.and(U32.is_lt(d2, 16), Bool.and(U32.is_lt(d3, 16), U32.is_lt(d4, 16))))
  Bool.pick(Hex4Res, valid,
    H4Ok{U32.add(U32.mul(d1, 4096), U32.add(U32.mul(d2, 256), U32.add(U32.mul(d3, 16), d4))), rest},
    H4Err{"invalid hex in unicode escape"})

def parse_hex4(s: String) -> Hex4Res:
  match s:
    case SNil{}:
      H4Err{"incomplete unicode escape"}
    case SCon{Chr{+c1}, +t1}:
      match t1:
        case SNil{}:
          H4Err{"incomplete unicode escape"}
        case SCon{Chr{+c2}, +t2}:
          match t2:
            case SNil{}:
              H4Err{"incomplete unicode escape"}
            case SCon{Chr{+c3}, +t3}:
              match t3:
                case SNil{}:
                  H4Err{"incomplete unicode escape"}
                case SCon{Chr{+c4}, +rest}:
                  make_hex4(hex_digit_val(c1), hex_digit_val(c2), hex_digit_val(c3), hex_digit_val(c4), rest)

def finish_low_hex4(is_valid_lo: Bool, +hi_val: U32, +lo_val: U32, rest: String) -> EscRes:
  match is_valid_lo:
    case True{}:
      scalar = U32.add(65536, U32.add(U32.mul(U32.sub(hi_val, 55296), 1024), U32.sub(lo_val, 56320)))
      EscChar{{Chr{scalar} : Char}, rest}
    case False{}:
      EscErr{"lone surrogate"}

def handle_low_hex4(+hi_val: U32, h4: Hex4Res) -> EscRes:
  match h4:
    case H4Err{_}:
      EscErr{"lone surrogate"}
    case H4Ok{+lo_val, rest}:
      is_valid_lo = Bool.and(U32.is_ge(lo_val, 56320), U32.is_le(lo_val, 57343))
      finish_low_hex4(is_valid_lo, hi_val, lo_val, rest)

def check_slash_u(is_slash_u: Bool, +hi_val: U32, t2: String) -> EscRes:
  match is_slash_u:
    case True{}:
      handle_low_hex4(hi_val, parse_hex4(t2))
    case False{}:
      EscErr{"lone surrogate"}

def parse_low_surrogate(+hi_val: U32, rest: String) -> EscRes:
  match rest:
    case SNil{}:
      EscErr{"lone surrogate"}
    case SCon{Chr{+c1}, +t1}:
      match t1:
        case SNil{}:
          EscErr{"lone surrogate"}
        case SCon{Chr{+c2}, +t2}:
          is_slash_u = Bool.and(U32.is_eq(c1, 92), U32.is_eq(c2, 117))
          check_slash_u(is_slash_u, hi_val, t2)

def check_surrogate_hi(is_hi: Bool, +val: U32, rest: String) -> EscRes:
  match is_hi:
    case True{}:
      parse_low_surrogate(val, rest)
    case False{}:
      EscChar{{Chr{val} : Char}, rest}

def check_surrogate_kind(is_hi: Bool, is_lo: Bool, +val: U32, rest: String) -> EscRes:
  match is_lo:
    case True{}:
      EscErr{"lone surrogate"}
    case False{}:
      check_surrogate_hi(is_hi, val, rest)

def check_surrogate(+val: U32, rest: String) -> EscRes:
  is_hi = Bool.and(U32.is_ge(val, 55296), U32.is_le(val, 56319))
  is_lo = Bool.and(U32.is_ge(val, 56320), U32.is_le(val, 57343))
  check_surrogate_kind(is_hi, is_lo, val, rest)

def handle_unicode_hex4(h4: Hex4Res) -> EscRes:
  match h4:
    case H4Err{msg}:
      EscErr{msg}
    case H4Ok{val, rest}:
      check_surrogate(val, rest)

def parse_escape_step(is_u: Bool, +c: U32, +rest: String) -> EscRes:
  match is_u:
    case True{}:
      handle_unicode_hex4(parse_hex4(rest))
    case False{}:
      Bool.pick(EscRes, U32.is_eq(c, 34), EscChar{Chr{34}, rest},
        Bool.pick(EscRes, U32.is_eq(c, 92), EscChar{Chr{92}, rest},
          Bool.pick(EscRes, U32.is_eq(c, 47), EscChar{Chr{47}, rest},
            Bool.pick(EscRes, U32.is_eq(c, 98), EscChar{Chr{8}, rest},
              Bool.pick(EscRes, U32.is_eq(c, 102), EscChar{Chr{12}, rest},
                Bool.pick(EscRes, U32.is_eq(c, 110), EscChar{Chr{10}, rest},
                  Bool.pick(EscRes, U32.is_eq(c, 114), EscChar{Chr{13}, rest},
                    Bool.pick(EscRes, U32.is_eq(c, 116), EscChar{Chr{9}, rest},
                      EscErr{"invalid escape sequence"}))))))))

def parse_escape_seq(+c: U32, rest: String) -> EscRes:
  parse_escape_step(U32.is_eq(c, 117), c, rest)

def parse_str.loop(fuel: Nat, st: StrState, s: String, acc: List<&2, Char>) -> StrRes:
  match fuel:
    case 0n:
      SErr{"fuel exhausted in string parse"}
    case 1n++p:
      match st:
        case StrNormal{}:
          match s:
            case SNil{}:
              SErr{"unterminated string literal"}
            case SCon{Chr{+c}, +t}:
              parse_str.loop(p, StrChar{classify_str_char(c), c}, t, acc)

        case StrChar{kind, c}:
          match kind:
            case SCQuote{}:
              SOk{str_from_rev_loop(acc, ""), s}
            case SCSlash{}:
              parse_str.loop(p, StrEscaping{}, s, acc)
            case SCControl{}:
              SErr{"unescaped control character in string"}
            case SCOther{}:
              parse_str.loop(p, StrNormal{}, s, Chr{c} <> acc)

        case StrEscaping{}:
          match s:
            case SNil{}:
              SErr{"unterminated string escape"}
            case SCon{Chr{+c}, +t}:
              parse_str.loop(p, StrEscCheck{parse_escape_seq(c, t)}, "", acc)

        case StrEscCheck{res}:
          match res:
            case EscChar{c_val, rest}:
              parse_str.loop(p, StrNormal{}, rest, c_val <> acc)
            case EscErr{msg}:
              SErr{msg}

def is_json_ws(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 32), Bool.or(U32.is_eq(c, 9), Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13))))

def skip_ws_loop(fuel: Nat, st: WsState, s: String) -> String:
  match fuel:
    case 0n:
      s
    case 1n++p:
      match st:
        case WsStart{}:
          match s:
            case SNil{}:
              ""
            case SCon{Chr{+c}, +t}:
              skip_ws_loop(p, WsChar{is_json_ws(c), c}, t)
        case WsChar{is_w, c}:
          match is_w:
            case True{}:
              skip_ws_loop(p, WsStart{}, s)
            case False{}:
              SCon{Chr{c}, s}

def skip_ws(s: String) -> String:
  skip_ws_loop(4294967295n, WsStart{}, s)

def classify_num_char(+c: U32) -> NumCharKind:
  Bool.pick(NumCharKind, U32.is_eq(c, 48), NCZero{},
    Bool.pick(NumCharKind, Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57)), NCNonZeroDigit{U32.to_nat(U32.sub(c, 48))},
      Bool.pick(NumCharKind, U32.is_eq(c, 45), NCMinus{},
        Bool.pick(NumCharKind, U32.is_eq(c, 43), NCPlus{},
          Bool.pick(NumCharKind, U32.is_eq(c, 46), NCDot{},
            Bool.pick(NumCharKind, Bool.or(U32.is_eq(c, 101), U32.is_eq(c, 69)), NCExp{},
              NCOther{c}))))))

def safe_mul_add(is_safe: Bool, acc: Nat, d: Nat) -> Nat:
  match is_safe:
    case True{}:
      Nat.add(Nat.mul(acc, 10n), d)
    case False{}:
      acc

def parse_num_loop(fuel: Nat, st: NumState, s: String, acc: List<&2, Char>) -> NumRes:
  match fuel:
    case 0n:
      NErr{"fuel exhausted in number parser"}
    case 1n++p:
      match st:
        case NStart{}:
          match s:
            case SNil{}:
              NErr{"unexpected EOF in number"}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NStartChar{classify_num_char(c), c}, t, acc)

        case NStartChar{kind, c}:
          match kind:
            case NCMinus{}:
              parse_num_loop(p, NAfterMinus{}, s, Chr{c} <> acc)
            case NCZero{}:
              parse_num_loop(p, NAfterZero{False{}}, s, Chr{c} <> acc)
            case NCNonZeroDigit{d}:
              parse_num_loop(p, NIntDigits{False{}, d, False{}}, s, Chr{c} <> acc)
            case _:
              NErr{"expected digit or minus"}

        case NAfterMinus{}:
          match s:
            case SNil{}:
              NErr{"expected digit after minus"}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NAfterMinusChar{classify_num_char(c), c}, t, acc)

        case NAfterMinusChar{kind, c}:
          match kind:
            case NCZero{}:
              parse_num_loop(p, NAfterZero{True{}}, s, Chr{c} <> acc)
            case NCNonZeroDigit{d}:
              parse_num_loop(p, NIntDigits{True{}, d, False{}}, s, Chr{c} <> acc)
            case _:
              NErr{"expected digit after minus"}

        case NAfterZero{is_n}:
          match s:
            case SNil{}:
              NInt{is_n, 0n, ""}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NAfterZeroChar{is_n, classify_num_char(c), c}, t, acc)

        case NAfterZeroChar{is_n, kind, c}:
          match kind:
            case NCZero{}:
              NErr{"leading zero in number"}
            case NCNonZeroDigit{_}:
              NErr{"leading zero in number"}
            case NCDot{}:
              parse_num_loop(p, NFracStart{}, s, Chr{c} <> acc)
            case NCExp{}:
              parse_num_loop(p, NExpStart{}, s, Chr{c} <> acc)
            case _:
              NInt{is_n, 0n, SCon{Chr{c}, s}}

        case NIntDigits{is_n, +acc_val, is_over}:
          match s:
            case SNil{}:
              Bool.pick(NumRes, is_over, NErr{"number too large"}, NInt{is_n, acc_val, ""})
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NIntDigitsChar{is_n, acc_val, is_over, classify_num_char(c), c}, t, acc)

        case NIntDigitsChar{is_n, +acc_val, is_over, kind, c}:
          match kind:
            case NCZero{}:
              +is_safe = check_safe_mul10(acc_val, 0n)
              new_over = Bool.or(is_over, Bool.not(is_safe))
              new_val = safe_mul_add(is_safe, acc_val, 0n)
              parse_num_loop(p, NIntDigits{is_n, new_val, new_over}, s, Chr{c} <> acc)
            case NCNonZeroDigit{+d}:
              +is_safe = check_safe_mul10(acc_val, d)
              new_over = Bool.or(is_over, Bool.not(is_safe))
              new_val = safe_mul_add(is_safe, acc_val, d)
              parse_num_loop(p, NIntDigits{is_n, new_val, new_over}, s, Chr{c} <> acc)
            case NCDot{}:
              parse_num_loop(p, NFracStart{}, s, Chr{c} <> acc)
            case NCExp{}:
              parse_num_loop(p, NExpStart{}, s, Chr{c} <> acc)
            case _:
              Bool.pick(NumRes, is_over, NErr{"number too large"}, NInt{is_n, acc_val, SCon{Chr{c}, s}})

        case NFracStart{}:
          match s:
            case SNil{}:
              NErr{"expected digit after decimal point"}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NFracStartChar{classify_num_char(c), c}, t, acc)

        case NFracStartChar{kind, c}:
          match kind:
            case NCZero{}:
              parse_num_loop(p, NFracDigits{}, s, Chr{c} <> acc)
            case NCNonZeroDigit{_}:
              parse_num_loop(p, NFracDigits{}, s, Chr{c} <> acc)
            case _:
              NErr{"expected digit after decimal point"}

        case NFracDigits{}:
          match s:
            case SNil{}:
              NRaw{str_from_rev_loop(acc, ""), ""}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NFracDigitsChar{classify_num_char(c), c}, t, acc)

        case NFracDigitsChar{kind, c}:
          match kind:
            case NCZero{}:
              parse_num_loop(p, NFracDigits{}, s, Chr{c} <> acc)
            case NCNonZeroDigit{_}:
              parse_num_loop(p, NFracDigits{}, s, Chr{c} <> acc)
            case NCExp{}:
              parse_num_loop(p, NExpStart{}, s, Chr{c} <> acc)
            case _:
              NRaw{str_from_rev_loop(acc, ""), SCon{Chr{c}, s}}

        case NExpStart{}:
          match s:
            case SNil{}:
              NErr{"expected digit or sign in exponent"}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NExpStartChar{classify_num_char(c), c}, t, acc)

        case NExpStartChar{kind, c}:
          match kind:
            case NCPlus{}:
              parse_num_loop(p, NExpSign{}, s, Chr{c} <> acc)
            case NCMinus{}:
              parse_num_loop(p, NExpSign{}, s, Chr{c} <> acc)
            case NCZero{}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case NCNonZeroDigit{_}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case _:
              NErr{"expected digit or sign in exponent"}

        case NExpSign{}:
          match s:
            case SNil{}:
              NErr{"expected digit after exponent sign"}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NExpSignChar{classify_num_char(c), c}, t, acc)

        case NExpSignChar{kind, c}:
          match kind:
            case NCZero{}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case NCNonZeroDigit{_}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case _:
              NErr{"expected digit after exponent sign"}

        case NExpDigits{}:
          match s:
            case SNil{}:
              NRaw{str_from_rev_loop(acc, ""), ""}
            case SCon{Chr{+c}, +t}:
              parse_num_loop(p, NExpDigitsChar{classify_num_char(c), c}, t, acc)

        case NExpDigitsChar{kind, c}:
          match kind:
            case NCZero{}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case NCNonZeroDigit{_}:
              parse_num_loop(p, NExpDigits{}, s, Chr{c} <> acc)
            case _:
              NRaw{str_from_rev_loop(acc, ""), SCon{Chr{c}, s}}

def parse_number(fuel: Nat, s: String) -> NumRes:
  parse_num_loop(fuel, NStart{}, s, Nil{})

def parse_step(fuel: Nat, act: StepAct, +stack: List<&2, Frame>, s: String) -> Result<&2, &2, String, Json>:
  match fuel:
    case 0n:
      Fail{"fuel exhausted in parser"}
    case 1n++p:
      match act:
        case ActCheckEof{+v}:
          match s:
            case SNil{}:
              Done{v}
            case SCon{c, t}:
              Fail{"unexpected trailing characters after JSON root"}

        case ActHaveVal{+v}:
          match stack:
            case Nil{}:
              parse_step(p, ActCheckEof{v}, Nil{}, skip_ws(s))
            case FArr{items} <> rest_stack:
              parse_step(p, ActArrNext{v, items}, rest_stack, skip_ws(s))
            case FObjVal{key, kvs} <> rest_stack:
              parse_step(p, ActObjNext{key, v, kvs}, rest_stack, skip_ws(s))

        case ActArrOpen{}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF in array: missing ']'"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActArrOpenChar{classify_char(c), c}, stack, t)

        case ActArrOpenChar{kind, c}:
          match kind:
            case KindRBracket{}:
              parse_step(p, ActHaveVal{JArr{Nil{}}}, stack, s)
            case _:
              parse_step(p, ActValStart{}, FArr{Nil{}} <> stack, SCon{Chr{c}, s})

        case ActArrNext{+v, +items}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF in array: missing ']'"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActArrNextChar{classify_char(c), v, items}, stack, t)

        case ActArrNextChar{kind, +v, +items}:
          match kind:
            case KindRBracket{}:
              parse_step(p, ActHaveVal{JArr{List.reverse(&2, Json, v <> items)}}, stack, s)
            case KindComma{}:
              parse_step(p, ActValStart{}, FArr{v <> items} <> stack, skip_ws(s))
            case _:
              Fail{"expected ',' or ']' in array"}

        case ActObjOpen{}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF in object: missing '}'"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActObjOpenChar{classify_char(c), c}, stack, t)

        case ActObjOpenChar{kind, c}:
          match kind:
            case KindRBrace{}:
              parse_step(p, ActHaveVal{JObj{Nil{}}}, stack, s)
            case _:
              parse_step(p, ActKeyStart{Nil{}}, stack, SCon{Chr{c}, s})

        case ActKeyStart{+kvs}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF in object: missing key string"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActKeyStartChar{classify_char(c), kvs}, stack, t)

        case ActKeyStartChar{kind, +kvs}:
          match kind:
            case KindQuote{}:
              parse_step(p, ActKeyParsed{parse_str.loop(p, StrNormal{}, s, Nil{}), kvs}, stack, "")
            case _:
              Fail{"expected string key in object"}

        case ActKeyParsed{res, +kvs}:
          match res:
            case SErr{msg}:
              Fail{msg}
            case SOk{key, rest}:
              parse_step(p, ActColon{key, kvs}, stack, skip_ws(rest))

        case ActColon{key, +kvs}:
          match s:
            case SNil{}:
              Fail{"expected ':' after object key"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActColonChar{classify_char(c), key, kvs}, stack, t)

        case ActColonChar{kind, key, +kvs}:
          match kind:
            case KindColon{}:
              parse_step(p, ActValStart{}, FObjVal{key, kvs} <> stack, skip_ws(s))
            case _:
              Fail{"expected ':' after object key"}

        case ActObjNext{key, +v, +kvs}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF in object: missing '}'"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActObjNextChar{classify_char(c), key, v, kvs}, stack, t)

        case ActObjNextChar{kind, +key, +v, +kvs}:
          match kind:
            case KindRBrace{}:
              parse_step(p, ActHaveVal{JObj{List.reverse(&2, Sigma<&2, &2, String, _ => Json>, Tuple{key, v} <> kvs)}}, stack, s)
            case KindComma{}:
              parse_step(p, ActKeyStart{Tuple{key, v} <> kvs}, stack, skip_ws(s))
            case _:
              Fail{"expected ',' or '}' in object"}

        case ActStrParsed{res}:
          match res:
            case SErr{msg}:
              Fail{msg}
            case SOk{val, rest}:
              parse_step(p, ActHaveVal{JStr{val}}, stack, rest)

        case ActNumParsed{res}:
          match res:
            case NErr{msg}:
              Fail{msg}
            case NRaw{raw_str, rest}:
              parse_step(p, ActHaveVal{JRaw{raw_str}}, stack, rest)
            case NInt{is_n, val, rest}:
              match is_n:
                case True{}:
                  parse_step(p, ActHaveVal{JNeg{val}}, stack, rest)
                case False{}:
                  parse_step(p, ActHaveVal{JNum{val}}, stack, rest)

        case ActLitCheck{expected, +val, ok, rest}:
          match ok:
            case True{}:
              parse_step(p, ActHaveVal{val}, stack, rest)
            case False{}:
              Fail{"invalid literal: expected " ++ expected}

        case ActValStart{}:
          match s:
            case SNil{}:
              Fail{"unexpected EOF: expected a JSON value"}
            case SCon{Chr{+c}, +t}:
              parse_step(p, ActValChar{classify_char(c), c}, stack, t)

        case ActValChar{kind, +c}:
          match kind:
            case KindLBracket{}:
              parse_step(p, ActArrOpen{}, stack, skip_ws(s))
            case KindLBrace{}:
              parse_step(p, ActObjOpen{}, stack, skip_ws(s))
            case KindQuote{}:
              parse_step(p, ActStrParsed{parse_str.loop(p, StrNormal{}, s, Nil{})}, stack, "")
            case KindNull{}:
              +full_str = {SCon{Chr{c}, s} : String}
              parse_step(p, ActLitCheck{"null", JNull{}, String.starts_with(full_str, "null"), String.drop(full_str, 4n)}, stack, "")
            case KindTrue{}:
              +full_str = {SCon{Chr{c}, s} : String}
              parse_step(p, ActLitCheck{"true", JBool{True{}}, String.starts_with(full_str, "true"), String.drop(full_str, 4n)}, stack, "")
            case KindFalse{}:
              +full_str = {SCon{Chr{c}, s} : String}
              parse_step(p, ActLitCheck{"false", JBool{False{}}, String.starts_with(full_str, "false"), String.drop(full_str, 5n)}, stack, "")
            case KindMinus{}:
              parse_step(p, ActNumParsed{parse_number(p, SCon{Chr{c}, s})}, stack, "")
            case KindDigit{}:
              parse_step(p, ActNumParsed{parse_number(p, SCon{Chr{c}, s})}, stack, "")
            case _:
              Fail{"unexpected character when reading JSON value"}

def parse(+s: String) -> Result<&2, &2, String, Json>:
  +fuel = {Nat.add(Nat.mul(str_len(s), 8n), 100n) : Nat}
  parse_step(fuel, ActValStart{}, Nil{}, skip_ws(s))

# ==============================================================================
# 12. Validation & Streaming NDJSON Support
# ==============================================================================

def validate_step(r: Result<&2, &2, String, Json>) -> Bool:
  match r:
    case Done{_}:
      True{}
    case Fail{_}:
      False{}

def validate(s: String) -> Bool:
  validate_step(parse(s))

def stringify_ndjson(xs: List<&2, Json>) -> String:
  match xs:
    case Nil{}:
      ""
    case h <> t:
      stringify(h) ++ "\n" ++ stringify_ndjson(t)

def split_lines_done(is_empty: Bool, +line: String) -> List<&2, String>:
  match is_empty:
    case True{}:
      Nil{}
    case False{}:
      [line]

def split_lines.loop(fuel: Nat, st: LineSplitState, s: String, +cur: String) -> List<&2, String>:
  match fuel:
    case 0n:
      +line = String.trim(String.reverse(cur))
      split_lines_done(String.is_empty(line), line)
    case 1n++p:
      match st:
        case LSNormal{}:
          match s:
            case SNil{}:
              +line = String.trim(String.reverse(cur))
              split_lines_done(String.is_empty(line), line)
            case SCon{Chr{+c}, +t}:
              split_lines.loop(p, LSCheckNl{U32.is_eq(c, 10), c}, t, cur)
        case LSCheckNl{is_nl, c}:
          match is_nl:
            case True{}:
              +line = String.trim(String.reverse(cur))
              split_lines.loop(p, LSEmpty{String.is_empty(line), line}, s, "")
            case False{}:
              split_lines.loop(p, LSNormal{}, s, SCon{Chr{c}, cur})
        case LSEmpty{is_empty, line}:
          match is_empty:
            case True{}:
              split_lines.loop(p, LSNormal{}, s, "")
            case False{}:
              line <> split_lines.loop(p, LSNormal{}, s, "")

def split_lines_loop(+s: String, cur: String) -> List<&2, String>:
  fuel = Nat.add(Nat.mul(str_len(s), 3n), 20n)
  split_lines.loop(fuel, LSNormal{}, s, cur)

def list_parse_lines(lines: List<&2, String>) -> List<&2, Result<&2, &2, String, Json>>:
  match lines:
    case Nil{}:
      Nil{}
    case h <> t:
      parse(h) <> list_parse_lines(t)

def parse_ndjson(s: String) -> List<&2, Result<&2, &2, String, Json>>:
  list_parse_lines(split_lines_loop(s, ""))
