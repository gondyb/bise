# JSON values, parsed and encoded as RFC 8259.
import Base

# JSON (RFC 8259) over text; decode bytes with Utf8 first. Nesting has no
# depth limit other than input size: the parser keeps its own stack, since a
# Bend def cannot call itself through another def.
#   import ./json/json.bend as Json

# Num keeps the number's exact text, so no precision is lost.
type Val is Data:
  Null{}
  Flag{on: Bool}
  Num{s: String}
  Str{s: String}
  Arr{xs: List<&2, Val>}
  Obj{m: Map<&2, Val>}

type Next is Data:
  Next{v: Val, rest: String}

# An open container: array items so far (reversed), or object fields and the
# key whose value comes next.
type Open is Data:
  OArr{xs: List<&2, Val>}
  OObj{m: Map<&2, Val>, key: String}

type St is Data:
  SValue{s: String, stack: List<&2, Open>}
  SKey{s: String, stack: List<&2, Open>}
  SColon{key: String, s: String, stack: List<&2, Open>}
  SStr{s: String, acc: String, key: Bool, stack: List<&2, Open>}
  SAfter{v: Val, s: String, stack: List<&2, Open>}
  SDone{v: Val, s: String}
  SFail{}

# Whitespace
# ----------

def is_ws(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 32), Bool.or(U32.is_eq(c, 9), Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13))))

# Bool.pick runs both arms, so loops peek: the next char's class is a Bool
# param and only the matching arm recurses.
def ws.go(t: String, +c: U32, ws: Bool) -> String:
  match t:
    case SNil{}:
      match ws:
        case True{}:
          SNil{}
        case False{}:
          SCon{Chr{c}, SNil{}}
    case SCon{Chr{+d}, u}:
      match ws:
        case True{}:
          ws.go(u, d, is_ws(d))
        case False{}:
          SCon{Chr{c}, SCon{Chr{d}, u}}

def skip_ws(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      ws.go(t, c, is_ws(c))

# Numbers (§6)
# ------------
# States: 0 start, 1 after "-", 2 after a leading "0", 3 int digits, 4 after
# ".", 5 fraction digits, 6 after "e", 7 after the exponent sign, 8 exponent
# digits. 99 means the char does not continue the number.

def digit(+c: U32) -> Bool:
  Bool.and(U32.is_le(48, c), U32.is_le(c, 57))

def num.trans(+st: U32, +c: U32) -> U32:
  +d = digit(c)
  +z = U32.is_eq(c, 48)
  +dot = U32.is_eq(c, 46)
  +e = Bool.or(U32.is_eq(c, 101), U32.is_eq(c, 69))
  +sign = Bool.or(U32.is_eq(c, 43), U32.is_eq(c, 45))
  Bool.pick(U32, U32.is_eq(st, 0), Bool.pick(U32, U32.is_eq(c, 45), 1, Bool.pick(U32, z, 2, Bool.pick(U32, d, 3, 99))),
  Bool.pick(U32, U32.is_eq(st, 1), Bool.pick(U32, z, 2, Bool.pick(U32, d, 3, 99)),
  Bool.pick(U32, Bool.or(U32.is_eq(st, 2), U32.is_eq(st, 3)), Bool.pick(U32, Bool.and(d, U32.is_eq(st, 3)), 3, Bool.pick(U32, dot, 4, Bool.pick(U32, e, 6, 99))),
  Bool.pick(U32, Bool.or(U32.is_eq(st, 4), U32.is_eq(st, 5)), Bool.pick(U32, d, 5, Bool.pick(U32, Bool.and(e, U32.is_eq(st, 5)), 6, 99)),
  Bool.pick(U32, U32.is_eq(st, 6), Bool.pick(U32, sign, 7, Bool.pick(U32, d, 8, 99)),
  Bool.pick(U32, Bool.or(U32.is_eq(st, 7), U32.is_eq(st, 8)), Bool.pick(U32, d, 8, 99), 99))))))

def num.accepts(+st: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(st, 2), U32.is_eq(st, 3)), Bool.or(U32.is_eq(st, 5), U32.is_eq(st, 8)))

def num.end(+st: U32, acc: String, rest: String) -> Maybe<&2, Next>:
  Bool.pick(Maybe<&2, Next>, num.accepts(st), Some{Next{Num{String.reverse(acc)}, rest}}, None{})

def num.go(t: String, +c: U32, +st: U32, acc: String, stop: Bool) -> Maybe<&2, Next>:
  match t:
    case SNil{}:
      match stop:
        case True{}:
          num.end(st, acc, SCon{Chr{c}, SNil{}})
        case False{}:
          num.end(num.trans(st, c), SCon{Chr{c}, acc}, SNil{})
    case SCon{Chr{+d}, u}:
      match stop:
        case True{}:
          num.end(st, acc, SCon{Chr{c}, SCon{Chr{d}, u}})
        case False{}:
          +n = num.trans(st, c)
          num.go(u, d, n, SCon{Chr{c}, acc}, U32.is_eq(num.trans(n, d), 99))

def num(s: String) -> Maybe<&2, Next>:
  match s:
    case SNil{}:
      None{}
    case SCon{Chr{+c}, t}:
      num.go(t, c, 0, SNil{}, U32.is_eq(num.trans(0, c), 99))

# Literals
# --------

def lit.if(+s: String, v: Val, n: Nat, ok: Bool) -> Maybe<&2, Next>:
  match ok:
    case False{}:
      None{}
    case True{}:
      Some{Next{v, String.drop(s, n)}}

def lit(+s: String, word: String, v: Val, +n: Nat) -> Maybe<&2, Next>:
  lit.if(s, v, n, String.starts_with(s, word))

# Strings (§7)
# ------------

type Esc is Data:
  Esc{cp: U32, rest: String}

def hexd(+c: U32) -> U32:
  Bool.pick(U32, digit(c), (c - 48 : U32), Bool.pick(U32, Bool.and(U32.is_le(97, c), U32.is_le(c, 102)), (c - 87 : U32), Bool.pick(U32, Bool.and(U32.is_le(65, c), U32.is_le(c, 70)), (c - 55 : U32), 99)))

def hex4.go(s: String, +acc: U32, n: Nat) -> Maybe<&2, Esc>:
  match s:
    case SNil{}:
      match n:
        case 0n:
          Some{Esc{acc, SNil{}}}
        case 1n+k:
          None{}
    case SCon{Chr{+c}, t}:
      match n:
        case 0n:
          Some{Esc{acc, SCon{Chr{c}, t}}}
        case 1n+k:
          +h = hexd(c)
          Bool.pick(Maybe<&2, Esc>, U32.is_eq(h, 99), None{}, hex4.go(t, (acc * 16 + h : U32), k))

def hex4(s: String) -> Maybe<&2, Esc>:
  hex4.go(s, 0, 4n)

def is_hi(+v: U32) -> Bool:
  Bool.and(U32.is_le(55296, v), U32.is_le(v, 56319))

def is_lo(+v: U32) -> Bool:
  Bool.and(U32.is_le(56320, v), U32.is_le(v, 57343))

def pair.lo(+hi: U32, +rest: String, lo: Maybe<&2, Esc>) -> Esc:
  match lo:
    case None{}:
      Esc{65533, rest}
    case Some{Esc{+v, r}}:
      Bool.pick(Esc, is_lo(v), Esc{(65536 + (hi - 55296 : U32) * 1024 + (v - 56320 : U32) : U32), r}, Esc{65533, rest})

# §8.2: a high surrogate pairs with a following \uDC00-\uDFFF; a lone one is U+FFFD.
def pair(+hi: U32, +rest: String) -> Esc:
  Bool.pick(Esc, String.starts_with(rest, "\\u"), pair.lo(hi, rest, hex4(String.drop(rest, 2n))), Esc{65533, rest})

def esc.u(e: Maybe<&2, Esc>) -> Maybe<&2, Esc>:
  match e:
    case None{}:
      None{}
    case Some{Esc{+v, +r}}:
      Some{Bool.pick(Esc, is_hi(v), pair(v, r), Bool.pick(Esc, is_lo(v), Esc{65533, r}, Esc{v, r}))}

def esc.simple(+e: U32) -> U32:
  Bool.pick(U32, U32.is_eq(e, 34), 34, Bool.pick(U32, U32.is_eq(e, 92), 92, Bool.pick(U32, U32.is_eq(e, 47), 47,
  Bool.pick(U32, U32.is_eq(e, 98), 8, Bool.pick(U32, U32.is_eq(e, 102), 12, Bool.pick(U32, U32.is_eq(e, 110), 10,
  Bool.pick(U32, U32.is_eq(e, 114), 13, Bool.pick(U32, U32.is_eq(e, 116), 9, 99))))))))

# One escape after the backslash.
def esc.one(t: String) -> Maybe<&2, Esc>:
  match t:
    case SNil{}:
      None{}
    case SCon{Chr{+e}, +u}:
      +v = esc.simple(e)
      Bool.pick(Maybe<&2, Esc>, U32.is_eq(e, 117), esc.u(hex4(u)), Bool.pick(Maybe<&2, Esc>, U32.is_eq(v, 99), None{}, Some{Esc{v, u}}))

def str.esc(e: Maybe<&2, Esc>, acc: String, key: Bool, stack: List<&2, Open>) -> St:
  match e:
    case None{}:
      SFail{}
    case Some{Esc{cp, r}}:
      SStr{r, SCon{Chr{cp}, acc}, key, stack}

def str.close(+acc: String, t: String, key: Bool, stack: List<&2, Open>) -> St:
  match key:
    case True{}:
      SColon{String.reverse(acc), t, stack}
    case False{}:
      SAfter{Str{String.reverse(acc)}, t, stack}

def str.c(quote: Bool, ctl: Bool, bs: Bool, c: U32, t: String, acc: String, key: Bool, stack: List<&2, Open>) -> St:
  match quote:
    case True{}:
      str.close(acc, t, key, stack)
    case False{}:
      match ctl:
        case True{}:
          SFail{}
        case False{}:
          match bs:
            case True{}:
              str.esc(esc.one(t), acc, key, stack)
            case False{}:
              SStr{t, SCon{Chr{c}, acc}, key, stack}

# §7: unescaped chars below U+0020 are not allowed in a string.
def str.step(s: String, acc: String, key: Bool, stack: List<&2, Open>) -> St:
  match s:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, t}:
      str.c(U32.is_eq(c, 34), U32.is_lt(c, 32), U32.is_eq(c, 92), c, t, acc, key, stack)

# Machine
# -------

def got(m: Maybe<&2, Next>, stack: List<&2, Open>) -> St:
  match m:
    case None{}:
      SFail{}
    case Some{Next{v, r}}:
      SAfter{v, r, stack}

def open.close(r: String, close: Val, stack: List<&2, Open>, empty: Bool, open: Open) -> St:
  match empty:
    case True{}:
      SAfter{close, String.drop(r, 1n), stack}
    case False{}:
      match open:
        case OArr{xs}:
          SValue{r, Con{OArr{xs}, stack}}
        case OObj{m, k}:
          SKey{r, Con{OObj{m, k}, stack}}

def value.step(+s: String, +c: U32, +t: String, +stack: List<&2, Open>) -> St:
  +r = skip_ws(t)
  Bool.pick(St, U32.is_eq(c, 91), open.close(r, Arr{Nil{}}, stack, String.starts_with(r, "]"), OArr{Nil{}}),
  Bool.pick(St, U32.is_eq(c, 123), open.close(r, Obj{Map.new(&2, Val)}, stack, String.starts_with(r, "}"), OObj{Map.new(&2, Val), ""}),
  Bool.pick(St, U32.is_eq(c, 34), SStr{t, SNil{}, False{}, stack},
  Bool.pick(St, U32.is_eq(c, 116), got(lit(s, "true", Flag{True{}}, 4n), stack),
  Bool.pick(St, U32.is_eq(c, 102), got(lit(s, "false", Flag{False{}}, 5n), stack),
  Bool.pick(St, U32.is_eq(c, 110), got(lit(s, "null", Null{}, 4n), stack),
  got(num(s), stack)))))))

def value(s: String, stack: List<&2, Open>) -> St:
  match s:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, +t}:
      value.step(SCon{Chr{c}, t}, c, t, stack)

def key(s: String, stack: List<&2, Open>) -> St:
  match s:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, t}:
      Bool.pick(St, U32.is_eq(c, 34), SStr{t, SNil{}, True{}, stack}, SFail{})

def colon.top(k: String, r: String, stack: List<&2, Open>) -> St:
  match stack:
    case Nil{}:
      SFail{}
    case Con{OArr{xs}, up}:
      SFail{}
    case Con{OObj{m, old}, up}:
      SValue{r, Con{OObj{m, k}, up}}

def colon(k: String, s: String, stack: List<&2, Open>) -> St:
  match s:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, t}:
      Bool.pick(St, U32.is_eq(c, 58), colon.top(k, t, stack), SFail{})

# A finished value joins its container; then "," or the closing bracket.
def after.arr.c(comma: Bool, close: Bool, t: String, xs: List<&2, Val>, up: List<&2, Open>) -> St:
  match comma:
    case True{}:
      SValue{t, Con{OArr{xs}, up}}
    case False{}:
      match close:
        case True{}:
          SAfter{Arr{List.reverse(&2, Val, xs)}, t, up}
        case False{}:
          SFail{}

def after.arr(xs: List<&2, Val>, r: String, up: List<&2, Open>) -> St:
  match r:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, t}:
      after.arr.c(U32.is_eq(c, 44), U32.is_eq(c, 93), t, xs, up)

def after.obj.c(comma: Bool, close: Bool, t: String, m: Map<&2, Val>, up: List<&2, Open>) -> St:
  match comma:
    case True{}:
      SKey{t, Con{OObj{m, ""}, up}}
    case False{}:
      match close:
        case True{}:
          SAfter{Obj{m}, t, up}
        case False{}:
          SFail{}

def after.obj(m: Map<&2, Val>, r: String, up: List<&2, Open>) -> St:
  match r:
    case SNil{}:
      SFail{}
    case SCon{Chr{+c}, t}:
      after.obj.c(U32.is_eq(c, 44), U32.is_eq(c, 125), t, m, up)

def after(v: Val, s: String, stack: List<&2, Open>) -> St:
  match stack:
    case Nil{}:
      SDone{v, s}
    case Con{OArr{xs}, up}:
      after.arr(Con{v, xs}, skip_ws(s), up)
    case Con{OObj{m, k}, up}:
      after.obj(Map.set(&2, Val, m, k, v), skip_ws(s), up)

def step(st: St) -> St:
  match st:
    case SValue{s, stack}:
      value(skip_ws(s), stack)
    case SKey{s, stack}:
      key(skip_ws(s), stack)
    case SColon{k, s, stack}:
      colon(k, skip_ws(s), stack)
    case SStr{s, acc, k, stack}:
      str.step(s, acc, k, stack)
    case SAfter{v, s, stack}:
      after(v, s, stack)
    case SDone{v, s}:
      SDone{v, s}
    case SFail{}:
      SFail{}

def done(v: Val, rest: String) -> Maybe<&2, Val>:
  match rest:
    case SNil{}:
      Some{v}
    case SCon{h, t}:
      None{}

# fuel: every step but the last eats at least one char.
def run(fuel: Nat, st: St) -> Maybe<&2, Val>:
  match fuel:
    case 0n:
      None{}
    case 1n+f:
      match st:
        case SDone{v, s}:
          done(v, skip_ws(s))
        case SFail{}:
          None{}
        case SValue{s, stack}:
          run(f, step(SValue{s, stack}))
        case SKey{s, stack}:
          run(f, step(SKey{s, stack}))
        case SColon{k, s, stack}:
          run(f, step(SColon{k, s, stack}))
        case SStr{s, acc, k, stack}:
          run(f, step(SStr{s, acc, k, stack}))
        case SAfter{v, s, stack}:
          run(f, step(SAfter{v, s, stack}))

# One JSON text: a value with optional whitespace around it.
def parse(+s: String) -> Maybe<&2, Val>:
  run(Nat.add(String.length(s), 2n), SValue{s, Nil{}})

# Access
# ------

def get.hit(r: Map<&2, Val> & Bool) -> Bool:
  (m, b) = r
  b

def get.some(r: Map<&2, Val> & Val) -> Maybe<&2, Val>:
  (m, x) = r
  Some{x}

def get.has(+m: Map<&2, Val>, +k: String, hit: Bool) -> Maybe<&2, Val>:
  match hit:
    case False{}:
      None{}
    case True{}:
      get.some(Map.get(Val, Null{}, m, k))

def get.of(+m: Map<&2, Val>, +k: String) -> Maybe<&2, Val>:
  get.has(m, k, get.hit(Map.has(&2, Val, m, k)))

# The value under key k of an object.
def get(v: Val, k: String) -> Maybe<&2, Val>:
  match v:
    case Obj{m}:
      get.of(m, k)
    case Null{}:
      None{}
    case Flag{on}:
      None{}
    case Num{n}:
      None{}
    case Str{s}:
      None{}
    case Arr{xs}:
      None{}

# Encode
# ------

def hexc(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), (48 + n : U32), (87 + n : U32))}

# The escaped form of one char, reversed (it is pushed onto a reversed acc).
def esc.put(+c: U32, +acc: String) -> String:
  Bool.pick(String, U32.is_eq(c, 34), SCon{Chr{34}, SCon{Chr{92}, acc}},
  Bool.pick(String, U32.is_eq(c, 92), SCon{Chr{92}, SCon{Chr{92}, acc}},
  Bool.pick(String, U32.is_eq(c, 10), SCon{Chr{110}, SCon{Chr{92}, acc}},
  Bool.pick(String, U32.is_eq(c, 13), SCon{Chr{114}, SCon{Chr{92}, acc}},
  Bool.pick(String, U32.is_eq(c, 9), SCon{Chr{116}, SCon{Chr{92}, acc}},
  Bool.pick(String, U32.is_lt(c, 32), SCon{hexc(U32.and(c, 15)), SCon{hexc(U32.shrn(c, 4n)), SCon{Chr{48}, SCon{Chr{48}, SCon{Chr{117}, SCon{Chr{92}, acc}}}}}},
  SCon{Chr{c}, acc}))))))

def esc.go(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      esc.go(t, esc.put(c, acc))

def esc(s: String) -> String:
  esc.go(s, SNil{})

type Item is Data:
  ILit{s: String}
  IVal{v: Val}

def items.sep(first: Bool, tail: List<&2, Item>) -> List<&2, Item>:
  match first:
    case True{}:
      tail
    case False{}:
      Con{ILit{","}, tail}

def items.arr(xs: List<&2, Val>, first: Bool, rest: List<&2, Item>) -> List<&2, Item>:
  match xs:
    case Nil{}:
      Con{ILit{"]"}, rest}
    case Con{x, t}:
      items.sep(first, Con{IVal{x}, items.arr(t, False{}, rest)})

def items.obj(kvs: List<&2, Sigma<&2, &2, String, _ => Val>>, first: Bool, rest: List<&2, Item>) -> List<&2, Item>:
  match kvs:
    case Nil{}:
      Con{ILit{"}"}, rest}
    case (k, v) <> t:
      items.sep(first, Con{ILit{"\"" ++ esc(k) ++ "\":"}, Con{IVal{v}, items.obj(t, False{}, rest)}})

def put(s: String, racc: String) -> String:
  String.reverse(s) ++ racc

# ponytail: @unsafe because it walks a work list, not the Val; each step
# emits text or swaps a container for its strictly smaller parts.
@unsafe
def enc.go(items: List<&2, Item>, racc: String) -> String:
  match items:
    case Nil{}:
      String.reverse(racc)
    case Con{ILit{s}, t}:
      enc.go(t, put(s, racc))
    case Con{IVal{Null{}}, t}:
      enc.go(t, put("null", racc))
    case Con{IVal{Flag{True{}}}, t}:
      enc.go(t, put("true", racc))
    case Con{IVal{Flag{False{}}}, t}:
      enc.go(t, put("false", racc))
    case Con{IVal{Num{n}}, t}:
      enc.go(t, put(n, racc))
    case Con{IVal{Str{s}}, t}:
      enc.go(t, put("\"" ++ esc(s) ++ "\"", racc))
    case Con{IVal{Arr{xs}}, t}:
      enc.go(Con{ILit{"["}, items.arr(xs, True{}, t)}, racc)
    case Con{IVal{Obj{m}}, t}:
      enc.go(Con{ILit{"{"}, items.obj(Map.to_list(&2, Val, m), True{}, t)}, racc)

# Compact JSON; object keys come out sorted.
def encode(v: Val) -> String:
  enc.go(Con{IVal{v}, Nil{}}, "")

def at.go(xs: List<&2, Val>, n: Nat) -> Maybe<&2, Val>:
  match xs:
    case Nil{}:
      None{}
    case Con{v, t}:
      match n:
        case 0n:
          Some{v}
        case 1n+p:
          at.go(t, p)


def at(v: Val, n: Nat) -> Maybe<&2, Val>:
  match v:
    case Arr{xs}:
      at.go(xs, n)
    case Null{}:
      None{}
    case Flag{on}:
      None{}
    case Num{s}:
      None{}
    case Str{s}:
      None{}
    case Obj{m}:
      None{}

def u32.over(+acc: U32, +c: U32) -> Bool:
  Bool.or(U32.is_lt(429496729, acc), Bool.and(U32.is_eq(acc, 429496729), U32.is_lt(53, c)))

def u32.dig(s: String, +acc: U32, bad: Bool) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      match bad:
        case True{}:
          None{}
        case False{}:
          Some{acc}
    case SCon{Chr{+c}, t}:
      u32.dig(t, (acc * 10 + (c - 48 : U32) : U32), Bool.or(bad, Bool.or(Bool.not(digit(c)), u32.over(acc, c))))

def u32.zero(t: String) -> Maybe<&2, U32>:
  match t:
    case SNil{}:
      Some{0}
    case SCon{d, u}:
      None{}

def u32.lead(+c: U32, t: String, zero: Bool) -> Maybe<&2, U32>:
  match zero:
    case True{}:
      u32.zero(t)
    case False{}:
      u32.dig(t, (c - 48 : U32), Bool.or(Bool.not(digit(c)), u32.over(0, c)))

def u32.num(s: String) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      None{}
    case SCon{Chr{+c}, t}:
      u32.lead(c, t, U32.is_eq(c, 48))

def u32(v: Val) -> Maybe<&2, U32>:
  match v:
    case Num{s}:
      u32.num(s)
    case Null{}:
      None{}
    case Flag{on}:
      None{}
    case Str{s}:
      None{}
    case Arr{xs}:
      None{}
    case Obj{m}:
      None{}
