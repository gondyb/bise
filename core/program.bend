# core/program.bend — D1: programmatic tool calling, in Bend.
#
# The orchestration language is a JS-lite the model already knows, with
# TypeScript type annotations accepted and ignored (a type-blind parser:
# no tsgo, no external stripper):
#
#   const me = github_app.get_me({});
#   const issues = github_app.search_issues({"q": "user:gvergnaud"});
#   if (me.ok && issues.ok) {
#     return me.login + " has issues";
#   }
#   return "failed";
#
# Statements: const / if-else / return. Expressions: tool calls by name
# (args are the RAW text between the parentheses — JSON for remote
# tools, plain text for string-arg tools), string literals, variable
# references with .field paths over JSON results, and '+'
# concatenation. Conditions: truthiness, !, ==, !=, &&, || (short-
# circuiting, correct precedence).
#
# The interpreter is a pure, fuel-bounded state machine: a program is a
# value, tool results are consumed in execution order, execution pauses
# at the first missing result (the durable effect boundary), and replay
# re-runs the program with the recorded results — it cannot diverge.
# Machine architecture follows the hub JSON parser: one self-recursive
# step def per machine, pending decisions packaged as state
# constructors (Bend matches only parameters and fields, never computed
# values).
import Base
import ./types.bend as T
import ../vendor/json.bend as J

# the interpreter's verdict
type PR is Data:
  PDone{out: String}
  # the program stopped at a missing tool result: emit this call
  PWait{name: String, args: String}
  # the program cannot proceed further (unbound variable, spent budget)
  PFail{why: String}

# ---- character classes ----

def is_ws(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 32),
    Bool.or(U32.is_eq(c, 10),
      Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 9))))

def idc(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 95),
    Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)),
      Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)),
        Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)))))

def chr_str(c: U32) -> String:
  SCon{Chr{c}, SNil{}}

# ---- tokens ----

type Tok is Data:
  TId{s: String}
  TStr{s: String}
  TCall{name: String, args: String}
  TSemi{}
  TEq2{}
  TNe2{}
  TAnd2{}
  TOr2{}
  TBang{}
  TPlus{}
  TAsgn{}
  TColon{}
  TDot{}
  TLB{}
  TRB{}
  TLP{}
  TRP{}
  TComma{}
  TLt{}
  TGt{}
  TBad{}

type CC is Data:
  KWs{}
  KCom{}
  KId{}
  KQuote{}
  KPunct{}
  KBad{}

def after_line(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, +t}:
      match c:
        case 10:
          t
        case _:
          after_line(t)

def is_punct(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 59),
    Bool.or(U32.is_eq(c, 61),
      Bool.or(U32.is_eq(c, 33),
        Bool.or(U32.is_eq(c, 38),
          Bool.or(U32.is_eq(c, 124),
            Bool.or(U32.is_eq(c, 43),
              Bool.or(U32.is_eq(c, 58),
                Bool.or(U32.is_eq(c, 46),
                  Bool.or(U32.is_eq(c, 123),
                    Bool.or(U32.is_eq(c, 125),
                      Bool.or(U32.is_eq(c, 40),
                        Bool.or(U32.is_eq(c, 41),
                          Bool.or(U32.is_eq(c, 44),
                            Bool.or(U32.is_eq(c, 60),
                              U32.is_eq(c, 62)))))))))))))))


def classify.str(p: Bool, +c: U32) -> CC:
  match p:
    case True{}:
      KPunct{}
    case False{}:
      KBad{}

def classify.rest(idch: Bool, +c: U32) -> CC:
  match idch:
    case True{}:
      KId{}
    case False{}:
      classify.str(is_punct(c), c)

def classify(+c: U32) -> CC:
  match c:
    case 32:
      KWs{}
    case 10:
      KWs{}
    case 13:
      KWs{}
    case 9:
      KWs{}
    case 47:
      KCom{}
    case 34:
      KQuote{}
    case 39:
      KQuote{}
    case 96:
      KQuote{}
    case _:
      classify.rest(idc(c), c)

type ChainRes is Data:
  CROk{chain: String, rest: String}

# a dotted identifier chain: name(.name)* — tool names and ref paths.
# One fuel-bounded state machine (the pattern of the hub JSON parser):
# pending decisions ride the state constructor.
type ChainSt is Data:
  CSeg{}
  CStep{idch: Bool, isdot: Bool, c: U32}
  CDot{}
  CDotStep{idch: Bool, c: U32}
  CDotQ{}

def read_chain(fuel: Nat, st: ChainSt, s: String, +acc: String)
    -> ChainRes:
  match fuel:
    case 0n:
      CROk{acc, s}
    case 1n+k:
      match st:
        case CSeg{}:
          match s:
            case SNil{}:
              CROk{acc, SNil{}}
            case SCon{Chr{+c}, +t}:
              read_chain(k, CStep{idc(c), U32.is_eq(c, 46), c}, t, acc)
        case CStep{idch, isdot, +c}:
          match idch:
            case True{}:
              read_chain(k, CSeg{}, s, String.append(acc, chr_str(c)))
            case False{}:
              match isdot:
                case True{}:
                  read_chain(k, CDot{}, s, acc)
                case False{}:
                  match c:
                    case 63:
                      read_chain(k, CDotQ{}, s, acc)
                    case _:
                      CROk{acc, SCon{Chr{c}, s}}
        case CDot{}:
          match s:
            case SNil{}:
              CROk{acc, SNil{}}
            case SCon{Chr{+c}, +t}:
              read_chain(k, CDotStep{idc(c), c}, t, acc)
        case CDotStep{idch, +c}:
          match idch:
            case True{}:
              read_chain(k, CSeg{}, s,
                String.append(acc, String.append(".", chr_str(c))))
            case False{}:
              match c:
                case 63:
                  read_chain(k, CDotQ{}, s, acc)
                case _:
                  CROk{acc, SCon{Chr{c}, s}}
        case CDotQ{}:
          match s:
            case SNil{}:
              CROk{acc, SNil{}}
            case SCon{Chr{+c2}, +t2}:
              match c2:
                case 46:
                  read_chain(k, CDot{}, t2, acc)
                case _:
                  CROk{acc, SCon{Chr{63}, s}}

def chain_read(+t: String, +acc: String) -> ChainRes:
  read_chain(Nat.add(Nat.mul(String.length(t), 4n), 16n), CSeg{}, t, acc)

# string literal parse result
type SRes is Data:
  SOk{val: String, rest: String}
  SErr{}

# string literals ('...', "...", `...`) with escapes (one machine, the
# terminator is whatever quote opened the literal)
type StrSt is Data:
  SC{qq: U32}
  SCStep{qq: U32, close: Bool, esc: Bool, c: U32}
  SEscP{qq: U32}
  SEscD{qq: U32, d: U32}

def read_str(fuel: Nat, st: StrSt, s: String, +acc: String) -> SRes:
  match fuel:
    case 0n:
      SErr{}
    case 1n+k:
      match st:
        case SC{+qq}:
          match s:
            case SNil{}:
              SErr{}
            case SCon{Chr{+c}, +t}:
              read_str(k, SCStep{qq, U32.is_eq(c, qq), U32.is_eq(c, 92), c},
                t, acc)
        case SCStep{qq, close, esc, +c}:
          match close:
            case True{}:
              SOk{acc, s}
            case False{}:
              match esc:
                case True{}:
                  read_str(k, SEscP{qq}, s, acc)
                case False{}:
                  read_str(k, SC{qq}, s, String.append(acc, chr_str(c)))
        case SEscP{+qq}:
          match s:
            case SNil{}:
              SErr{}
            case SCon{Chr{+d}, +t}:
              read_str(k, SEscD{qq, d}, t, acc)
        case SEscD{qq, +d}:
          match d:
            case 110:
              read_str(k, SC{qq}, s, String.append(acc, "\n"))
            case 116:
              read_str(k, SC{qq}, s, String.append(acc, "\t"))
            case 34:
              read_str(k, SC{qq}, s, String.append(acc, "\""))
            case 39:
              read_str(k, SC{qq}, s, String.append(acc, "'"))
            case 92:
              read_str(k, SC{qq}, s, String.append(acc, "\\"))
            case _:
              read_str(k, SC{qq}, s, String.append(acc, chr_str(d)))

def read_string(qq: U32, +s: String) -> SRes:
  read_str(Nat.add(Nat.mul(String.length(s), 4n), 16n), SC{qq}, s, "")

# is this chain a keyword (never a tool call)?
def is_kw(+s: String) -> Bool:
  Bool.or(String.eq(s, "if"),
    Bool.or(String.eq(s, "else"),
      Bool.or(String.eq(s, "const"),
        Bool.or(String.eq(s, "return"),
          Bool.or(String.eq(s, "as"),
            String.eq(s, "await"))))))

# after a chain: skip whitespace, is the next char '('?
type LpRes is Data:
  LpYes{rest: String}
  LpNo{rest: String}

type LpSt is Data:
  LS{}
  LStep{ws: Bool, c: U32, t: String, orig: String}

def lpeek(fuel: Nat, st: LpSt, s: String) -> LpRes:
  match fuel:
    case 0n:
      LpNo{s}
    case 1n+k:
      match st:
        case LS{}:
          match s:
            case SNil{}:
              LpNo{SNil{}}
            case SCon{Chr{+c}, +t}:
              lpeek(k, LStep{is_ws(c), c, t, s}, s)
        case LStep{ws, +c, +t, +orig}:
          match ws:
            case True{}:
              lpeek(k, LS{}, t)
            case False{}:
              match c:
                case 40:
                  LpYes{t}
                case _:
                  LpNo{orig}

def lp(+rest: String) -> LpRes:
  lpeek(Nat.add(Nat.mul(String.length(rest), 4n), 16n), LS{}, rest)

# punctuation (with two-char operator lookahead)
type PRes is Data:
  PODone{tok: Tok, rest: String}
  PBad{}

def pun.eq.trip(eq2: Bool, +t: String) -> PRes:
  match eq2:
    case True{}:
      PODone{TEq2{}, String.drop(t, 2n)}
    case False{}:
      PODone{TEq2{}, String.drop(t, 1n)}

def pun.eq.dbl(eq: Bool, +t: String) -> PRes:
  match eq:
    case True{}:
      pun.eq.trip(String.starts_with(t, "=="), t)
    case False{}:
      PODone{TAsgn{}, t}

def pun.eq.d(eq: Bool, +t: String) -> PRes:
  pun.eq.dbl(eq, t)

def pun.eq(+t: String) -> PRes:
  pun.eq.d(String.starts_with(t, "="), t)

def pun.bang.trip(ne2: Bool, +t: String) -> PRes:
  match ne2:
    case True{}:
      PODone{TNe2{}, String.drop(t, 2n)}
    case False{}:
      PODone{TNe2{}, String.drop(t, 1n)}

def pun.bang.dbl(ne: Bool, +t: String) -> PRes:
  match ne:
    case True{}:
      pun.bang.trip(String.starts_with(t, "=="), t)
    case False{}:
      PODone{TBang{}, t}

def pun.bang.d(ne: Bool, +t: String) -> PRes:
  pun.bang.dbl(ne, t)

def pun.bang(+t: String) -> PRes:
  pun.bang.d(String.starts_with(t, "="), t)

def pun.amp.d(amp: Bool, +t: String) -> PRes:
  match amp:
    case True{}:
      PODone{TAnd2{}, String.drop(t, 1n)}
    case False{}:
      PBad{}

def pun.amp(+t: String) -> PRes:
  pun.amp.d(String.starts_with(t, "&"), t)

def pun.pipe.d(pipe: Bool, +t: String) -> PRes:
  match pipe:
    case True{}:
      PODone{TOr2{}, String.drop(t, 1n)}
    case False{}:
      PBad{}

def pun.pipe(+t: String) -> PRes:
  pun.pipe.d(String.starts_with(t, "|"), t)

def pun.c(c: U32, +t: String) -> PRes:
  match c:
    case 59:
      PODone{TSemi{}, t}
    case 61:
      pun.eq(t)
    case 33:
      pun.bang(t)
    case 38:
      pun.amp(t)
    case 124:
      pun.pipe(t)
    case 43:
      PODone{TPlus{}, t}
    case 58:
      PODone{TColon{}, t}
    case 46:
      PODone{TDot{}, t}
    case 123:
      PODone{TLB{}, t}
    case 125:
      PODone{TRB{}, t}
    case 40:
      PODone{TLP{}, t}
    case 41:
      PODone{TRP{}, t}
    case 44:
      PODone{TComma{}, t}
    case 60:
      PODone{TLt{}, t}
    case 62:
      PODone{TGt{}, t}
    case _:
      PBad{}

# raw JSON-arg capture classes (chars between the call parentheses)
type JC is Data:
  JEnd{}
  JEsc{}
  JQuote{}
  JOpen{}
  JClose{}
  JChar{}

def jclass.end(z: Bool) -> JC:
  match z:
    case True{}:
      JEnd{}
    case False{}:
      JChar{}

def jclass.out(depth: Nat, c: U32) -> JC:
  match c:
    case 34:
      JQuote{}
    case 123:
      JOpen{}
    case 125:
      JClose{}
    case 41:
      jclass.end(Nat.is_eq(depth, 0n))
    case _:
      JChar{}

def jclass(depth: Nat, in_str: Bool, c: U32) -> JC:
  match in_str:
    case True{}:
      match c:
        case 92:
          JEsc{}
        case 34:
          JQuote{}
        case _:
          JChar{}
    case False{}:
      jclass.out(depth, c)

# the tokenizer machine: one state per pending decision
type TkAct is Data:
  TkStart{}
  TkStartC{cc: CC, c: U32, t: String}
  TkChain{res: ChainRes}
  TkChainDec{kw: Bool, lp: LpRes, chain: String}
  TkStr{res: SRes}
  TkP{res: PRes}
  TkJson{depth: Nat, in_str: Bool, name: String, acc: String}
  TkJsonC{jc: JC, c: U32, t: String, depth: Nat, in_str: Bool,
    name: String, acc: String}

def tk(fuel: Nat, act: TkAct, s: String, out: List<&2, Tok>)
    -> List<&2, Tok>:
  match fuel:
    case 0n:
      List.reverse(&2, Tok, out)
    case 1n+k:
      match act:
        case TkStart{}:
          match s:
            case SNil{}:
              List.reverse(&2, Tok, out)
            case SCon{Chr{+c}, +t}:
              tk(k, TkStartC{classify(c), c, t}, t, out)
        case TkStartC{cc, +c, +t}:
          match cc:
            case KWs{}:
              tk(k, TkStart{}, t, out)
            case KCom{}:
              tk(k, TkStart{}, after_line(t), out)
            case KId{}:
              tk(k, TkChain{chain_read(t, chr_str(c))}, t, out)
            case KQuote{}:
              tk(k, TkStr{read_string(c, t)}, t, out)
            case KPunct{}:
              tk(k, TkP{pun.c(c, t)}, t, out)
            case KBad{}:
              tk(k, TkStart{}, t, Con{TBad{}, out})
        case TkChain{res}:
          match res:
            case CROk{+chain, +rest}:
              tk(k, TkChainDec{is_kw(chain), lp(rest), chain}, rest, out)
        case TkChainDec{kw, lp, +chain}:
          match kw:
            case True{}:
              tk(k, TkStart{}, s, Con{TId{chain}, out})
            case False{}:
              match lp:
                case LpYes{+rest}:
                  tk(k, TkJson{0n, False{}, chain, ""}, rest, out)
                case LpNo{+rest}:
                  tk(k, TkStart{}, rest, Con{TId{chain}, out})
        case TkStr{res}:
          match res:
            case SOk{+val, +rest}:
              tk(k, TkStart{}, rest, Con{TStr{val}, out})
            case SErr{}:
              List.reverse(&2, Tok, Con{TBad{}, out})
        case TkP{res}:
          match res:
            case PODone{+tok, +rest}:
              tk(k, TkStart{}, rest, Con{tok, out})
            case PBad{}:
              List.reverse(&2, Tok, Con{TBad{}, out})
        case TkJson{+depth, +in_str, +name, +acc}:
          match s:
            case SNil{}:
              List.reverse(&2, Tok, Con{TBad{}, out})
            case SCon{Chr{+c}, +t}:
              tk(k, TkJsonC{jclass(depth, in_str, c), c, t, depth, in_str,
                name, acc}, t, out)
        case TkJsonC{jc, +c, +t, depth, in_str, +name, +acc}:
          match jc:
            case JEnd{}:
              tk(k, TkStart{}, t, Con{TCall{name, acc}, out})
            case JEsc{}:
              tk(k, TkJson{depth, True{}, name,
                String.append(acc, String.append("\\", chr_str(c)))}, t, out)
            case JQuote{}:
              tk(k, TkJson{depth, Bool.not(in_str), name,
                String.append(acc, "\"")}, t, out)
            case JOpen{}:
              tk(k, TkJson{Nat.add(depth, 1n), in_str, name,
                String.append(acc, "{")}, t, out)
            case JClose{}:
              tk(k, TkJson{Nat.sub(depth, 1n), in_str, name,
                String.append(acc, "}")}, t, out)
            case JChar{}:
              tk(k, TkJson{depth, in_str, name,
                String.append(acc, chr_str(c))}, t, out)

def tokenize(+src: String) -> List<&2, Tok>:
  tk(Nat.add(Nat.mul(String.length(src), 10n), 100n), TkStart{}, src, Nil{})

# ---- the parser: one state machine over the token list ----

# "a.b.c" -> name "a", path ["b", "c"]
type RefRes is Data:
  ROk{name: String, path: List<&2, String>}
  RErr{}

def find_dot(+s: String, +i: Nat) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      None{}
    case SCon{Chr{+c}, +t}:
      match c:
        case 46:
          Some{i}
        case _:
          find_dot(t, Nat.add(i, 1n))

def C_find(+chain: String) -> Maybe<&2, Nat>:
  find_dot(chain, 0n)

def split_ref.cut(sp: Maybe<&2, Nat>, +chain: String) -> RefRes:
  match sp:
    case Some{+p}:
      ROk{String.take(chain, p),
        String.split(String.drop(chain, Nat.add(p, 1n)), '.')}
    case None{}:
      ROk{chain, Nil{}}

def split_ref(+chain: String) -> RefRes:
  split_ref.cut(C_find(chain), chain)

# const, let, and var all declare a binding
def is_decl(+w: String) -> Bool:
  Bool.or(String.eq(w, "const"),
    Bool.or(String.eq(w, "let"), String.eq(w, "var")))

# a chain of digits is a number literal (values stay strings; "1.5" too)
def digits(+s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, +t}:
      match c:
        case 48:
          digits(t)
        case 49:
          digits(t)
        case 50:
          digits(t)
        case 51:
          digits(t)
        case 52:
          digits(t)
        case 53:
          digits(t)
        case 54:
          digits(t)
        case 55:
          digits(t)
        case 56:
          digits(t)
        case 57:
          digits(t)
        case 46:
          digits(t)
        case _:
          False{}

def numlit.first(ch: Char) -> Bool:
  match ch:
    case Chr{+c}:
      match c:
        case 48:
          True{}
        case 49:
          True{}
        case 50:
          True{}
        case 51:
          True{}
        case 52:
          True{}
        case 53:
          True{}
        case 54:
          True{}
        case 55:
          True{}
        case 56:
          True{}
        case 57:
          True{}
        case _:
          False{}

def numlit.head(m: Maybe<&2, Char>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+ch}:
      numlit.first(ch)

def is_numlit(+ch: String) -> Bool:
  Bool.and(digits(ch), numlit.head(String.get(ch, 0n)))

# word literals render as their JSON text
def is_wordlit(+w: String) -> Bool:
  Bool.or(String.eq(w, "true"),
    Bool.or(String.eq(w, "false"),
      Bool.or(String.eq(w, "null"),
        Bool.or(String.eq(w, "undefined"), String.eq(w, "NaN")))))

def lit_of.is_false(f: Bool, +w: String) -> String:
  match f:
    case True{}:
      "false"
    case False{}:
      "null"

def lit_of.is_true(t: Bool, +w: String) -> String:
  match t:
    case True{}:
      "true"
    case False{}:
      lit_of.is_false(String.eq(w, "false"), w)

def lit_of(+w: String) -> String:
  lit_of.is_true(String.eq(w, "true"), w)

type PxAct is Data:
  PxList{acc: List<&2, T.Stmt>, close: Bool}
  PxEndTok{close: Bool, acc: List<&2, T.Stmt>}
  PxRBTok{close: Bool, acc: List<&2, T.Stmt>}
  PxListW{is_const: Bool, is_ret: Bool, is_if: Bool,
    acc: List<&2, T.Stmt>, close: Bool, rest: List<&2, Tok>}
  PxBlockDone{ss: List<&2, T.Stmt>}
  PxStmtDone{s: T.Stmt}
  PxConstName{}
  PxConstAnno{name: String}
  PxConstType{depth: Nat, name: String}
  PxConstEq{eq0: Bool, name: String}
  PxAsW{is_as: Bool, name: String, e: T.Expr, rest: List<&2, Tok>}
  PxReturn{}
  PxIfLP{}
  PxIfCond{}
  PxIfThenLB{cond: T.Cond}
  PxIfElse{cond: T.Cond, thn: List<&2, T.Stmt>}
  PxElseW2{is_else: Bool, cond: T.Cond, thn: List<&2, T.Stmt>,
    r2: List<&2, Tok>}
  PxElseW{is_else: Bool, cond: T.Cond, thn: List<&2, T.Stmt>,
    toks: List<&2, Tok>}
  PxExpr{}
  PxIdW{is_await: Bool, is_num: Bool, is_lit: Bool, ch: String,
    rest: List<&2, Tok>}
  PxRef{res: RefRes}
  PxHave{e: T.Expr}
  PxHaveRoute{e: T.Expr}
  PxAsSkip{name: String, e: T.Expr}
  PxCondRoute{c: T.Cond}
  PxUnit{}
  PxUnitDone{c: T.Cond}
  PxCond{}
  PxCondDone{c: T.Cond}

type PxF is Data:
  FxList{acc: List<&2, T.Stmt>, close: Bool}
  FxConst{name: String}
  FxRet{}
  FxIfCond{}
  FxIfThen{cond: T.Cond}
  FxIfElse{cond: T.Cond, thn: List<&2, T.Stmt>}
  FxCat{left: T.Expr}
  FxCmpL{}
  FxCmpEq{left: T.Expr, ne: Bool}
  FxNot{}
  FxAnd{left: T.Cond}
  FxOr{left: T.Cond}

def px(fuel: Nat, act: PxAct, stack: List<&2, PxF>, toks: List<&2, Tok>)
    -> Maybe<&2, List<&2, T.Stmt>>:
  match fuel:
    case 0n:
      None{}
    case 1n+k:
      match act:
        case PxList{+acc, close}:
          match toks:
            case Nil{}:
              px(k, PxEndTok{close, acc}, stack, toks)
            case Con{TRB{}, +rest}:
              px(k, PxRBTok{close, acc}, stack, rest)
            case Con{TSemi{}, +rest}:
              px(k, PxList{acc, close}, stack, rest)
            case Con{TId{+w}, +rest}:
              px(k, PxListW{is_decl(w), String.eq(w, "return"),
                String.eq(w, "if"), acc, close, rest}, stack, rest)
            case _:
              None{}
        case PxEndTok{close, +acc}:
          match close:
            case True{}:
              None{}
            case False{}:
              Some{List.reverse(&2, T.Stmt, acc)}
        case PxRBTok{close, +acc}:
          match close:
            case True{}:
              px(k, PxBlockDone{List.reverse(&2, T.Stmt, acc)}, stack, toks)
            case False{}:
              None{}
        case PxListW{+is_const, +is_ret, +is_if, +acc, +close, +rest}:
          match is_const:
            case True{}:
              px(k, PxConstName{}, FxList{acc, close} <> stack, rest)
            case False{}:
              match is_ret:
                case True{}:
                  px(k, PxReturn{}, FxList{acc, close} <> stack, rest)
                case False{}:
                  match is_if:
                    case True{}:
                      px(k, PxIfLP{}, FxList{acc, close} <> stack, rest)
                    case False{}:
                      None{}
        case PxBlockDone{+ss}:
          match stack:
            case FxIfThen{+cond} <> r:
              px(k, PxIfElse{cond, ss}, r, toks)
            case FxIfElse{+cond, +thn} <> r:
              px(k, PxStmtDone{T.SIf{cond, thn, ss}}, r, toks)
            case _:
              None{}
        case PxStmtDone{+s}:
          match stack:
            case FxList{acc, close} <> r:
              px(k, PxList{Con{s, acc}, close}, r, toks)
            case _:
              None{}
        case PxConstName{}:
          match toks:
            case Con{TId{+nm}, +rest}:
              px(k, PxConstAnno{nm}, stack, rest)
            case _:
              None{}
        case PxConstAnno{+name}:
          match toks:
            case Con{TColon{}, +rest}:
              px(k, PxConstType{0n, name}, stack, rest)
            case Con{TAsgn{}, +rest}:
              px(k, PxExpr{}, FxConst{name} <> stack, rest)
            case _:
              None{}
        case PxConstType{+depth, +name}:
          match toks:
            case Con{TAsgn{}, +rest}:
              px(k, PxConstEq{Nat.is_eq(depth, 0n), name}, stack, rest)
            case Con{TLt{}, +rest}:
              px(k, PxConstType{Nat.add(depth, 1n), name}, stack, rest)
            case Con{TLP{}, +rest}:
              px(k, PxConstType{Nat.add(depth, 1n), name}, stack, rest)
            case Con{TLB{}, +rest}:
              px(k, PxConstType{Nat.add(depth, 1n), name}, stack, rest)
            case Con{TGt{}, +rest}:
              px(k, PxConstType{Nat.sub(depth, 1n), name}, stack, rest)
            case Con{TRP{}, +rest}:
              px(k, PxConstType{Nat.sub(depth, 1n), name}, stack, rest)
            case Con{TRB{}, +rest}:
              px(k, PxConstType{Nat.sub(depth, 1n), name}, stack, rest)
            case Con{TBad{}, +rest}:
              None{}
            case Nil{}:
              None{}
            case Con{_, +rest}:
              px(k, PxConstType{depth, name}, stack, rest)
        case PxConstEq{+eq0, +name}:
          match eq0:
            case True{}:
              px(k, PxExpr{}, FxConst{name} <> stack, toks)
            case False{}:
              None{}
        case PxAsW{+is_as, +name, +e, +rest}:
          match is_as:
            case True{}:
              px(k, PxAsSkip{name, e}, stack, rest)
            case False{}:
              None{}
        case PxReturn{}:
          match toks:
            case Con{TSemi{}, +rest}:
              px(k, PxStmtDone{T.SRet{T.EStr{""}}}, stack, rest)
            case _:
              px(k, PxExpr{}, FxRet{} <> stack, toks)
        case PxIfLP{}:
          match toks:
            case Con{TLP{}, +rest}:
              px(k, PxIfCond{}, stack, rest)
            case _:
              None{}
        case PxIfCond{}:
          px(k, PxUnit{}, FxIfCond{} <> stack, toks)
        case PxIfThenLB{+cond}:
          match toks:
            case Con{TLB{}, +rest}:
              px(k, PxList{Nil{}, True{}}, FxIfThen{cond} <> stack, rest)
            case _:
              None{}
        case PxIfElse{+cond, +thn}:
          match toks:
            case Con{TId{+w}, +rest}:
              px(k, PxElseW{String.eq(w, "else"), cond, thn, toks}, stack,
                rest)
            case _:
              px(k, PxStmtDone{T.SIf{cond, thn, Nil{}}}, stack, toks)
        case PxElseW{+is_else, +cond, +thn, +full}:
          match is_else:
            case True{}:
              match full:
                case Con{TId{+w}, +r2}:
                  px(k, PxElseW2{String.eq(w, "else"), cond, thn, r2},
                    stack, r2)
                case _:
                  px(k, PxStmtDone{T.SIf{cond, thn, Nil{}}}, stack, full)
            case False{}:
              px(k, PxStmtDone{T.SIf{cond, thn, Nil{}}}, stack, full)
        case PxElseW2{is_else, +cond, +thn, +r2}:
          match is_else:
            case True{}:
              match r2:
                case Con{TLB{}, +r3}:
                  px(k, PxList{Nil{}, True{}},
                    FxIfElse{cond, thn} <> stack, r3)
                case _:
                  None{}
            case False{}:
              px(k, PxStmtDone{T.SIf{cond, thn, Nil{}}}, stack, toks)
        case PxExpr{}:
          match toks:
            case Con{TCall{+n, +a}, +rest}:
              px(k, PxHave{T.ECall{n, a}}, stack, rest)
            case Con{TStr{+s}, +rest}:
              px(k, PxHave{T.EStr{s}}, stack, rest)
            case Con{TId{+ch}, +rest}:
              px(k, PxIdW{String.eq(ch, "await"), is_numlit(ch),
                is_wordlit(ch), ch, rest}, stack, rest)
            case _:
              None{}
        case PxIdW{is_await, is_num, is_lit, +ch, +rest}:
          match is_await:
            case True{}:
              px(k, PxExpr{}, stack, rest)
            case False{}:
              match is_num:
                case True{}:
                  px(k, PxHave{T.EStr{ch}}, stack, rest)
                case False{}:
                  match is_lit:
                    case True{}:
                      px(k, PxHave{T.EStr{lit_of(ch)}}, stack, rest)
                    case False{}:
                      px(k, PxRef{split_ref(ch)}, stack, rest)
        case PxRef{res}:
          match res:
            case ROk{+name, +path}:
              px(k, PxHave{T.ERef{name, path}}, stack, toks)
            case RErr{}:
              None{}
        case PxHave{+e}:
          match toks:
            case Con{TPlus{}, +rest}:
              px(k, PxExpr{}, FxCat{e} <> stack, rest)
            case _:
              px(k, PxHaveRoute{e}, stack, toks)
        case PxUnit{}:
          match toks:
            case Con{TBang{}, +rest}:
              px(k, PxUnit{}, FxNot{} <> stack, rest)
            case _:
              px(k, PxExpr{}, FxCmpL{} <> stack, toks)
        case PxUnitDone{+c}:
          match stack:
            case FxNot{} <> r:
              px(k, PxUnitDone{T.CNot{c}}, r, toks)
            case FxAnd{+left} <> r:
              px(k, PxUnitDone{T.CAnd{left, c}}, r, toks)
            case _:
              match toks:
                case Con{TAnd2{}, +rest}:
                  px(k, PxUnit{}, FxAnd{c} <> stack, rest)
                case Con{TOr2{}, +rest}:
                  px(k, PxCond{}, FxOr{c} <> stack, rest)
                case _:
                  px(k, PxCondDone{c}, stack, toks)
        case PxCond{}:
          px(k, PxUnit{}, stack, toks)
        case PxCondDone{+c}:
          match stack:
            case FxOr{+left} <> r:
              px(k, PxCondDone{T.COr{left, c}}, r, toks)
            case _:
              match toks:
                case Con{TOr2{}, +rest}:
                  px(k, PxCond{}, FxOr{c} <> stack, rest)
                case _:
                  px(k, PxCondRoute{c}, stack, toks)
        case PxHaveRoute{+e}:
          match stack:
            case FxCat{+left} <> r:
              px(k, PxHave{T.ECat{left, e}}, r, toks)
            case FxCmpL{} <> r:
              match toks:
                case Con{TEq2{}, +rest}:
                  px(k, PxExpr{}, FxCmpEq{e, False{}} <> r, rest)
                case Con{TNe2{}, +rest}:
                  px(k, PxExpr{}, FxCmpEq{e, True{}} <> r, rest)
                case _:
                  px(k, PxUnitDone{T.CTruth{e}}, r, toks)
            case FxCmpEq{+left, +ne} <> r:
              match ne:
                case True{}:
                  px(k, PxUnitDone{T.CNe{left, e}}, r, toks)
                case False{}:
                  px(k, PxUnitDone{T.CEq{left, e}}, r, toks)
            case FxConst{+name} <> r:
              match toks:
                case Con{TSemi{}, +rest}:
                  px(k, PxStmtDone{T.SConst{name, e}}, r, rest)
                case Con{TId{+w}, +rest}:
                  px(k, PxAsW{String.eq(w, "as"), name, e, rest}, r, rest)
                case Nil{}:
                  px(k, PxStmtDone{T.SConst{name, e}}, r, toks)
                case _:
                  None{}
            case FxRet{} <> r:
              match toks:
                case Con{TSemi{}, +rest}:
                  px(k, PxStmtDone{T.SRet{e}}, r, rest)
                case Nil{}:
                  px(k, PxStmtDone{T.SRet{e}}, r, toks)
                case _:
                  None{}
            case _:
              None{}
        case PxAsSkip{+name, +e}:
          match toks:
            case Con{TSemi{}, +rest}:
              px(k, PxStmtDone{T.SConst{name, e}}, stack, rest)
            case Con{_, +rest}:
              px(k, PxAsSkip{name, e}, stack, rest)
            case Nil{}:
              None{}
        case PxCondRoute{+c}:
          match stack:
            case FxIfCond{} <> r:
              match toks:
                case Con{TRP{}, +rest}:
                  px(k, PxIfThenLB{c}, r, rest)
                case _:
                  None{}
            case _:
              None{}

def parse.toks(+toks: List<&2, Tok>) -> Maybe<&2, List<&2, T.Stmt>>:
  px(Nat.add(Nat.mul(List.length(&2, Tok, toks), 30n), 1000n), PxList{Nil{}, False{}},
    Nil{}, toks)

def parse(+src: String) -> Maybe<&2, List<&2, T.Stmt>>:
  parse.toks(tokenize(src))

# ---- the interpreter: a pure fuel-bounded state machine ----

type Bind is Data:
  B{name: String, json: String}

type RRes is Data:
  RFound{v: String}
  RMiss{name: String}

def on_str(on: Bool) -> String:
  match on:
    case True{}:
      "true"
    case False{}:
      "false"

# render a JSON value as text: a string is its content, every other
# value its JSON text (numbers keep their written form)
def render(v: J.Json) -> String:
  match v:
    case J.JStr{+s}:
      s
    case J.JBool{+on}:
      on_str(on)
    case J.JNull{}:
      "null"
    case _:
      J.stringify(v)

# the parse result as a Maybe (a malformed value is no value)
def parse_m(r: Result<&2, &2, String, J.Json>) -> Maybe<&2, J.Json>:
  match r:
    case Done{+j}:
      Some{j}
    case Fail{_}:
      None{}

# walk a .field path over a parsed JSON result
type WalkSt is Data:
  WVal{v: J.Json, path: List<&2, String>}
  WGot{m: Maybe<&2, J.Json>, path: List<&2, String>}

def walk.step(fuel: Nat, st: WalkSt) -> Maybe<&2, J.Json>:
  match fuel:
    case 0n:
      None{}
    case 1n+k:
      match st:
        case WVal{+v, +path}:
          match path:
            case Nil{}:
              Some{v}
            case Con{+p, +rest}:
              walk.step(k, WGot{J.get(v, p), rest})
        case WGot{+m, +path}:
          match m:
            case None{}:
              None{}
            case Some{+w}:
              walk.step(k, WVal{w, path})

def walk(v: J.Json, +path: List<&2, String>) -> Maybe<&2, J.Json>:
  walk.step(Nat.add(Nat.mul(List.length(&2, String, path), 2n), 8n),
    WVal{v, path})

def render_m(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      ""
    case Some{+v}:
      render(v)

# a missing field renders as "" (truthy: false)
def refval.deep.res(r: Maybe<&2, J.Json>, +path: List<&2, String>) -> String:
  match r:
    case Some{+v}:
      render_m(walk(v, path))
    case None{}:
      ""

def refval.deep(+j: String, +path: List<&2, String>) -> String:
  refval.deep.res(parse_m(J.parse(j)), path)

def refval.found(j: String, +path: List<&2, String>) -> RRes:
  match path:
    case Nil{}:
      RFound{j}
    case Con{_, _}:
      RFound{refval.deep(j, path)}

type LookupSt is Data:
  LEnv{env: List<&2, Bind>, name: String}
  LHit{hit: Bool, j: String, rest: List<&2, Bind>, name: String}

def lookup.go(fuel: Nat, st: LookupSt) -> Maybe<&2, String>:
  match fuel:
    case 0n:
      None{}
    case 1n+k:
      match st:
        case LEnv{+env, +name}:
          match env:
            case Nil{}:
              None{}
            case Con{B{+n, +j}, +rest}:
              lookup.go(k, LHit{String.eq(n, name), j, rest, name})
        case LHit{hit, +j, +rest, +name}:
          match hit:
            case True{}:
              Some{j}
            case False{}:
              lookup.go(k, LEnv{rest, name})

def lookup(+env: List<&2, Bind>, +name: String) -> Maybe<&2, String>:
  lookup.go(Nat.add(Nat.mul(List.length(&2, Bind, env), 2n), 8n),
    LEnv{env, name})

# JSON truthiness for conditions
def truthy.v(v: J.Json) -> Bool:
  match v:
    case J.JStr{+s}:
      Bool.not(String.is_empty(s))
    case J.JNum{+n}:
      Bool.not(Nat.is_eq(n, 0n))
    case J.JRaw{+r}:
      Bool.not(String.eq(r, "0"))
    case J.JBool{+on}:
      on
    case J.JNull{}:
      False{}
    case _:
      True{}

def truthy.res(r: Maybe<&2, J.Json>) -> Bool:
  match r:
    case None{}:
      False{}
    case Some{+v}:
      truthy.v(v)

def truthy(+j: String) -> Bool:
  truthy.res(parse_m(J.parse(j)))

def refval.look(m: Maybe<&2, String>, +name: String,
    +path: List<&2, String>) -> RRes:
  match m:
    case None{}:
      RMiss{name}
    case Some{+j}:
      refval.found(j, path)

def refval(env: List<&2, Bind>, +name: String, +path: List<&2, String>)
    -> RRes:
  refval.look(lookup(env, name), name, path)

type EvAct is Data:
  EvStmts{stmts: List<&2, T.Stmt>}
  EvExpr{e: T.Expr}
  EvRef{res: RRes}
  EvVal{v: String}
  EvCond{c: T.Cond}
  EvBool{b: Bool}
  EvNot{b: Bool}
  EvAndV{b: Bool, right: T.Cond}
  EvOrV{b: Bool, right: T.Cond}
  EvIfV{b: Bool, thn: List<&2, T.Stmt>, els: List<&2, T.Stmt>,
    stmts: List<&2, T.Stmt>}

type EvF is Data:
  IConst{name: String, stmts: List<&2, T.Stmt>}
  IRet{}
  IIf{thn: List<&2, T.Stmt>, els: List<&2, T.Stmt>,
    stmts: List<&2, T.Stmt>}
  ICatL{right: T.Expr}
  ICat{left: String}
  ICmpL2{right: T.Expr, ne: Bool}
  ICmp{left: String, ne: Bool}
  ITruth{}
  INot{}
  IAnd{right: T.Cond}
  IAndT{}
  IOr{right: T.Cond}
  IOrT{}

def ev(fuel: Nat, act: EvAct, stack: List<&2, EvF>, +env: List<&2, Bind>,
    results: List<&2, String>) -> PR:
  match fuel:
    case 0n:
      PFail{"program budget exhausted"}
    case 1n+k:
      match act:
        case EvStmts{+stmts}:
          match stmts:
            case Nil{}:
              PDone{""}
            case Con{+s, +rest}:
              match s:
                case T.SConst{+name, +e}:
                  ev(k, EvExpr{e}, IConst{name, rest} <> stack, env,
                    results)
                case T.SIf{+c, +thn, +els}:
                  ev(k, EvCond{c}, IIf{thn, els, rest} <> stack, env,
                    results)
                case T.SRet{+e}:
                  ev(k, EvExpr{e}, IRet{} <> stack, env, results)
        case EvExpr{+e}:
          match e:
            case T.EStr{+t}:
              ev(k, EvVal{t}, stack, env, results)
            case T.ECall{+n, +a}:
              match results:
                case Con{+r, +more}:
                  ev(k, EvVal{r}, stack, env, more)
                case Nil{}:
                  PWait{n, a}
            case T.ERef{+n, +p}:
              ev(k, EvRef{refval(env, n, p)}, stack, env, results)
            case T.ECat{+l, +r}:
              ev(k, EvExpr{l}, ICatL{r} <> stack, env, results)
        case EvRef{res}:
          match res:
            case RFound{+v}:
              ev(k, EvVal{v}, stack, env, results)
            case RMiss{+n}:
              PFail{String.append("unknown variable: ", n)}
        case EvVal{+v}:
          match stack:
            case ICatL{+right} <> r:
              ev(k, EvExpr{right}, ICat{v} <> r, env, results)
            case ICat{+left} <> r:
              ev(k, EvVal{String.append(left, v)}, r, env, results)
            case ICmpL2{+right, ne} <> r:
              ev(k, EvExpr{right}, ICmp{v, ne} <> r, env, results)
            case ICmp{+left, +ne} <> r:
              match ne:
                case True{}:
                  ev(k, EvBool{Bool.not(String.eq(left, v))}, r, env,
                    results)
                case False{}:
                  ev(k, EvBool{String.eq(left, v)}, r, env, results)
            case ITruth{} <> r:
              ev(k, EvBool{truthy(v)}, r, env, results)
            case IConst{+name, +stmts} <> r:
              ev(k, EvStmts{stmts}, r, Con{B{name, v}, env}, results)
            case Con{IRet{}, _}:
              PDone{v}
            case _:
              PFail{"bad program state"}
        case EvCond{+c}:
          match c:
            case T.CTruth{+e}:
              ev(k, EvExpr{e}, ITruth{} <> stack, env, results)
            case T.CNot{+cc}:
              ev(k, EvCond{cc}, INot{} <> stack, env, results)
            case T.CEq{+l, +r2}:
              ev(k, EvExpr{l}, ICmpL2{r2, False{}} <> stack, env, results)
            case T.CNe{+l, +r2}:
              ev(k, EvExpr{l}, ICmpL2{r2, True{}} <> stack, env, results)
            case T.CAnd{+l, +r2}:
              ev(k, EvCond{l}, IAnd{r2} <> stack, env, results)
            case T.COr{+l, +r2}:
              ev(k, EvCond{l}, IOr{r2} <> stack, env, results)
        case EvBool{b}:
          match stack:
            case INot{} <> r:
              ev(k, EvNot{b}, r, env, results)
            case IAnd{+right} <> r:
              ev(k, EvAndV{b, right}, r, env, results)
            case IAndT{} <> r:
              ev(k, EvBool{b}, r, env, results)
            case IOr{+right} <> r:
              ev(k, EvOrV{b, right}, r, env, results)
            case IOrT{} <> r:
              ev(k, EvBool{b}, r, env, results)
            case IIf{+thn, +els, +stmts} <> r:
              ev(k, EvIfV{b, thn, els, stmts}, r, env, results)
            case _:
              PFail{"bad program state"}
        case EvNot{b}:
          match b:
            case True{}:
              ev(k, EvBool{False{}}, stack, env, results)
            case False{}:
              ev(k, EvBool{True{}}, stack, env, results)
        case EvAndV{b, +right}:
          match b:
            case True{}:
              ev(k, EvCond{right}, IAndT{} <> stack, env, results)
            case False{}:
              ev(k, EvBool{False{}}, stack, env, results)
        case EvOrV{b, +right}:
          match b:
            case True{}:
              ev(k, EvBool{True{}}, stack, env, results)
            case False{}:
              ev(k, EvCond{right}, IOrT{} <> stack, env, results)
        case EvIfV{b, +thn, +els, +stmts}:
          match b:
            case True{}:
              ev(k, EvStmts{List.append(&2, T.Stmt, thn, stmts)}, stack,
                env, results)
            case False{}:
              ev(k, EvStmts{List.append(&2, T.Stmt, els, stmts)}, stack,
                env, results)

def run(stmts: List<&2, T.Stmt>, results: List<&2, String>) -> PR:
  ev(100000n, EvStmts{stmts}, Nil{}, Nil{}, results)
