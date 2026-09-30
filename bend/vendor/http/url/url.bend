# origin-form paths and query strings, percent-encoded.
import Base

# origin-form /path[?k=v] with RFC 3986 percent-encoding.

type Url is Data:
  Url{path: String, query: Map<&2, String>}

def unreserved(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_le(65, c), U32.is_le(c, 90)), Bool.or(Bool.and(U32.is_le(97, c), U32.is_le(c, 122)), Bool.or(Bool.and(U32.is_le(48, c), U32.is_le(c, 57)), Bool.or(U32.is_eq(c, 45), Bool.or(U32.is_eq(c, 46), Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 126)))))))

def path_ok(+c: U32) -> Bool:
  Bool.or(unreserved(c), U32.is_eq(c, 47))

def hexn(+n: U32) -> U32:
  Bool.pick(U32, U32.is_le(n, 9), (48 + n : U32), (55 + n : U32))

def hexhi(+n: U32) -> U32:
  U32.div(n, 16)

def hexlo(+n: U32) -> U32:
  U32.mod(n, 16)

def pct.enc.push(+c: U32, acc: String, keep: Bool) -> String:
  match keep:
    case True{}:
      SCon{Chr{c}, acc}
    case False{}:
      SCon{Chr{hexn(hexlo(c))}, SCon{Chr{hexn(hexhi(c))}, SCon{Chr{37}, acc}}}

def pct.enc.path(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      pct.enc.path(t, pct.enc.push(c, acc, path_ok(c)))

def pct.enc.query(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      pct.enc.query(t, pct.enc.push(c, acc, unreserved(c)))

def pct.encode_path(s: String) -> String:
  pct.enc.path(s, SNil{})

def pct.encode_q(s: String) -> String:
  pct.enc.query(s, SNil{})

def unhex.H(c: U32, hex: Bool) -> Maybe<&2, U32>:
  match hex:
    case True{}:
      Some{(c - 55 : U32)}
    case False{}:
      None{}

def unhex.h(+c: U32, lo: Bool) -> Maybe<&2, U32>:
  match lo:
    case True{}:
      Some{(c - 87 : U32)}
    case False{}:
      unhex.H(c, Bool.and(U32.is_le(65, c), U32.is_le(c, 70)))

def unhex.d(+c: U32, dec: Bool, lo: Bool) -> Maybe<&2, U32>:
  match dec:
    case True{}:
      Some{(c - 48 : U32)}
    case False{}:
      unhex.h(c, lo)

def unhex(+c: U32) -> Maybe<&2, U32>:
  unhex.d(c, Bool.and(U32.is_le(48, c), U32.is_le(c, 57)), Bool.and(U32.is_le(97, c), U32.is_le(c, 102)))

# Lit: plain bytes. Hi: after "%". Lo: after "%" and one hex digit. Bad: never decodes.
type Pd is Data:
  PdLit{}
  PdHi{}
  PdLo{hi: U32}
  PdBad{}

def pct.lit(+c: U32, racc: String, pct: Bool) -> Pd & String:
  match pct:
    case True{}:
      (PdHi{}, racc)
    case False{}:
      (PdLit{}, SCon{Chr{c}, racc})

def pct.hi(m: Maybe<&2, U32>, racc: String) -> Pd & String:
  match m:
    case None{}:
      (PdBad{}, racc)
    case Some{x}:
      (PdLo{x}, racc)

def pct.lo(+x: U32, m: Maybe<&2, U32>, racc: String) -> Pd & String:
  match m:
    case None{}:
      (PdBad{}, racc)
    case Some{y}:
      (PdLit{}, SCon{Chr{(x * 16 + y : U32)}, racc})

def pct.byte(h: Char) -> U32:
  Chr{c} = h
  c

def pct.end(st: Pd, racc: String) -> Maybe<&2, String>:
  match st:
    case PdLit{}:
      Some{String.reverse(racc)}
    case PdHi{}:
      None{}
    case PdLo{x}:
      None{}
    case PdBad{}:
      None{}

def pct.step(st: Pd, +c: U32, racc: String) -> Pd & String:
  match st:
    case PdLit{}:
      pct.lit(c, racc, U32.is_eq(c, 37))
    case PdHi{}:
      pct.hi(unhex(c), racc)
    case PdLo{+x}:
      pct.lo(x, unhex(c), racc)
    case PdBad{}:
      (PdBad{}, racc)

def pct.end.of(sr: Pd & String) -> Maybe<&2, String>:
  (st, racc) = sr
  pct.end(st, racc)

def pct.step.of(sr: Pd & String, +c: U32) -> Pd & String:
  (st, racc) = sr
  pct.step(st, c, racc)

def pct.decode.go(s: String, sr: Pd & String) -> Maybe<&2, String>:
  match s:
    case SNil{}:
      pct.end.of(sr)
    case SCon{h, t}:
      pct.decode.go(t, pct.step.of(sr, pct.byte(h)))

def pct.decode(s: String) -> Maybe<&2, String>:
  pct.decode.go(s, (PdLit{}, ""))

# Bool.pick evaluates both arms; a split that recursed inside it would reverse acc at every byte.
def take.cut(acc: String, rest: String) -> String & String:
  (String.reverse(String.drop(acc, 1n)), rest)

# hit: the last char pushed onto acc was the stop char.
def take_to.go(s: String, +stop: U32, acc: String, hit: Bool) -> String & String:
  match s:
    case SNil{}:
      match hit:
        case True{}:
          take.cut(acc, SNil{})
        case False{}:
          (String.reverse(acc), SNil{})
    case SCon{Chr{+c}, t}:
      match hit:
        case True{}:
          take.cut(acc, SCon{Chr{c}, t})
        case False{}:
          take_to.go(t, stop, SCon{Chr{c}, acc}, U32.is_eq(c, stop))

# (text before the first stop char, text after it)
def take_to(s: String, stop: U32) -> String & String:
  take_to.go(s, stop, SNil{}, False{})

def pair(kv: String) -> String & String:
  take_to(kv, 61)

def qput.decv(m: Map<&2, String>, key: String, v: Maybe<&2, String>) -> Map<&2, String>:
  match v:
    case None{}:
      m
    case Some{val}:
      Map.set(&2, String, m, key, val)

def qput.dec(m: Map<&2, String>, k: Maybe<&2, String>, v: Maybe<&2, String>) -> Map<&2, String>:
  match k:
    case None{}:
      m
    case Some{key}:
      qput.decv(m, key, v)

def qput.kv(m: Map<&2, String>, kv: String & String) -> Map<&2, String>:
  (k, v) = kv
  qput.dec(m, pct.decode(k), pct.decode(v))

def qput(m: Map<&2, String>, kv: String) -> Map<&2, String>:
  qput.kv(m, pair(kv))

def qfold(xs: List<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}:
      m
    case Con{h, t}:
      qfold(t, qput(m, h))

def parse.q(path: String, rest: String, hasq: Bool) -> Maybe<&2, Url>:
  match hasq:
    case False{}:
      Some{Url{path, Map.new(&2, String)}}
    case True{}:
      Some{Url{path, qfold(String.split(rest, '&'), Map.new(&2, String))}}

def parse.path(path: Maybe<&2, String>, +rest: String) -> Maybe<&2, Url>:
  match path:
    case None{}:
      None{}
    case Some{p}:
      parse.q(p, rest, Bool.not(String.is_empty(rest)))

def parse.split(pr: String & String) -> Maybe<&2, Url>:
  (path, +rest) = pr
  parse.path(pct.decode(path), rest)

def parse.ok(+s: String, ok: Bool) -> Maybe<&2, Url>:
  match ok:
    case False{}:
      None{}
    case True{}:
      parse.split(take_to(s, 63))

def parse(+s: String) -> Maybe<&2, Url>:
  parse.ok(s, String.starts_with(s, "/"))

def enc.pairs.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      enc.pairs.go(t, acc ++ ("&" ++ pct.encode_q(k) ++ "=" ++ pct.encode_q(v)))

def enc.pairs(m: Map<&2, String>) -> String:
  enc.pairs.go(Map.to_list(&2, String, m), "")

def encode.q(path: String, q: String, empty: Bool) -> String:
  match empty:
    case True{}:
      path
    case False{}:
      path ++ "?" ++ String.drop(q, 1n)

def encode.go(path: String, +q: String) -> String:
  encode.q(path, q, String.is_empty(q))

def encode(u: Url) -> String:
  Url{path, query} = u
  encode.go(pct.encode_path(path), enc.pairs(query))

def form.drop(q: String, empty: Bool) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.drop(q, 1n)

def form.go(+q: String) -> String:
  form.drop(q, String.is_empty(q))

# application/x-www-form-urlencoded. Space is %20.
def form(m: Map<&2, String>) -> String:
  form.go(enc.pairs(m))

# Absolute http(s) URLs (RFC 3986 §3) for clients.
# ---------------------------------------------

# target is the RFC 9112 §3.2.1 origin-form: path (never empty) and query, no fragment.
type Abs is Data:
  Abs{scheme: String, host: String, port: U32, target: String}

def has(s: String, +c: U32) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{Chr{d}, t}:
      Bool.or(U32.is_eq(d, c), has(t, c))

def auth_end(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 47), Bool.or(U32.is_eq(c, 63), U32.is_eq(c, 35)))

def take.back(acc: String, rest: String) -> String & String:
  match acc:
    case SNil{}:
      (SNil{}, rest)
    case SCon{x, a2}:
      (String.reverse(a2), SCon{x, rest})

# §3.2: the authority runs to the first "/", "?" or "#".
def take_auth.go(s: String, acc: String, hit: Bool) -> String & String:
  match s:
    case SNil{}:
      match hit:
        case True{}:
          take.back(acc, SNil{})
        case False{}:
          (String.reverse(acc), SNil{})
    case SCon{Chr{+c}, t}:
      match hit:
        case True{}:
          take.back(acc, SCon{Chr{c}, t})
        case False{}:
          take_auth.go(t, SCon{Chr{c}, acc}, auth_end(c))

def default_port(+scheme: String) -> Maybe<&2, U32>:
  Bool.pick(Maybe<&2, U32>, String.eq(scheme, "http"), Some{80}, Bool.pick(Maybe<&2, U32>, String.eq(scheme, "https"), Some{443}, None{}))

def abs.target(+pq: String) -> String:
  Bool.pick(String, String.starts_with(pq, "/"), pq, "/" ++ pq)

def abs.fst(ab: String & String) -> String:
  (a, b) = ab
  a

# §3.5: the fragment is for the client; it never goes on the wire.
def abs.build(scheme: String, host: String, port: Maybe<&2, U32>, rest: String) -> Maybe<&2, Abs>:
  match port:
    case None{}:
      None{}
    case Some{p}:
      Some{Abs{scheme, host, p, abs.target(abs.fst(take_to(rest, 35)))}}

def abs.port.num(n: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match n:
    case None{}:
      None{}
    case Some{+p}:
      Bool.pick(Maybe<&2, U32>, U32.is_le(p, 65535), Some{p}, None{})

# §3.2.3: an empty port means the scheme's default.
def abs.port(+ps: String, dflt: Maybe<&2, U32>) -> Maybe<&2, U32>:
  Bool.pick(Maybe<&2, U32>, String.is_empty(ps), dflt, abs.port.num(U32.read(ps)))

def abs.hostok(scheme: String, host: String, port: Maybe<&2, U32>, rest: String, ok: Bool) -> Maybe<&2, Abs>:
  match ok:
    case False{}:
      None{}
    case True{}:
      abs.build(scheme, host, port, rest)

# §3.2.2: hosts are case-insensitive. IPv6 literals are refused (IPv4-only runtime).
def abs.hostport(scheme: String, dflt: Maybe<&2, U32>, hp: String & String, rest: String) -> Maybe<&2, Abs>:
  (+host, ps) = hp
  abs.hostok(scheme, String.to_lower(host), abs.port(ps, dflt), rest, Bool.not(Bool.or(String.is_empty(host), String.starts_with(host, "["))))

# WHATWG fetch refuses URLs that carry credentials (user:pass@).
def abs.auth.if(+scheme: String, auth: String, rest: String, creds: Bool) -> Maybe<&2, Abs>:
  match creds:
    case True{}:
      None{}
    case False{}:
      abs.hostport(scheme, default_port(scheme), take_to(auth, 58), rest)

def abs.auth(scheme: String, ar: String & String) -> Maybe<&2, Abs>:
  (+auth, rest) = ar
  abs.auth.if(scheme, auth, rest, has(auth, 64))

def abs.slashes(scheme: String, +rest: String, ok: Bool) -> Maybe<&2, Abs>:
  match ok:
    case False{}:
      None{}
    case True{}:
      abs.auth(String.to_lower(scheme), take_auth.go(String.drop(rest, 2n), SNil{}, False{}))

def abs.scheme(sr: String & String) -> Maybe<&2, Abs>:
  (scheme, +rest) = sr
  abs.slashes(scheme, rest, String.starts_with(rest, "//"))

# scheme://host[:port][/path][?query][#fragment]
def absolute(s: String) -> Maybe<&2, Abs>:
  abs.scheme(take_to(s, 58))

def abs.port.dflt(m: Maybe<&2, U32>) -> U32:
  match m:
    case None{}:
      0
    case Some{p}:
      p

# RFC 9110 §7.2: the Host field; the default port may be left out.
def host_field(a: Abs) -> String:
  Abs{+scheme, +host, +port, target} = a
  Bool.pick(String, U32.is_eq(port, abs.port.dflt(default_port(scheme))), host, host ++ ":" ++ U32.show(port))

# Reference resolution (RFC 3986 §5.2) for Location headers.
# ----------------------------------------------------------

def dots.pop(stack: List<&2, String>) -> List<&2, String>:
  match stack:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      t

def dots.push(seg: String, stack: List<&2, String>, dot: Bool, up: Bool) -> List<&2, String>:
  match dot:
    case True{}:
      stack
    case False{}:
      match up:
        case True{}:
          dots.pop(stack)
        case False{}:
          Con{seg, stack}

def dots.join(xs: List<&2, String>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      dots.join(t, "/" ++ h ++ acc)

# stack is reversed; trail: the last segment was "." or "..", so a "/" ends the path.
def dots.go(segs: List<&2, String>, stack: List<&2, String>, trail: Bool) -> String:
  match segs:
    case Nil{}:
      +out = dots.join(stack, "")
      Bool.pick(String, Bool.and(trail, Bool.not(String.is_empty(out))), out ++ "/", Bool.pick(String, String.is_empty(out), "/", out))
    case Con{+seg, t}:
      +dot = String.eq(seg, ".")
      +up = String.eq(seg, "..")
      dots.go(t, dots.push(seg, stack, dot, up), Bool.or(dot, up))

def dots.abs(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      "/"
    case Con{lead, t}:
      dots.go(t, Nil{}, False{})

# §5.2.4 for an absolute path: "." and ".." segments go away.
def remove_dots(path: String) -> String:
  dots.abs(String.split(path, '/'))

def dir.go(s: String, +acc: String, last: String) -> String:
  match s:
    case SNil{}:
      String.reverse(last)
    case SCon{Chr{+c}, t}:
      +acc2 = {SCon{Chr{c}, acc} : String}
      dir.go(t, acc2, Bool.pick(String, U32.is_eq(c, 47), acc2, last))

# §5.2.3: the base path up to and including its last "/".
def dir(path: String) -> String:
  dir.go(path, SNil{}, SNil{})

def with_q(path: String, q: String, has_q: Bool) -> String:
  match has_q:
    case True{}:
      path ++ "?" ++ q
    case False{}:
      path

def res.build(scheme: String, host: String, port: U32, path: String, q: String, has_q: Bool) -> Maybe<&2, Abs>:
  Some{Abs{scheme, host, port, with_q(path, q, has_q)}}

# A path, its query, and whether a "?" was present at all.
type PQ is Data:
  PQ{path: String, q: String, has_q: Bool}

def res.pq(pq: String & String, has_q: Bool) -> PQ:
  (p, q) = pq
  PQ{p, q, has_q}

def res.ref(+r: String) -> PQ:
  res.pq(take_to(r, 63), has(r, 63))

# §5.2.2: an empty path keeps the base path (and its query unless one is given);
# "/..." replaces the path; anything else merges with the base directory.
def res.rel.go(+scheme: String, +host: String, +port: U32, +bpath: String, +bq: String, +bhas: Bool, +rp: String, +q: String, +rhas: Bool) -> Maybe<&2, Abs>:
  Bool.pick(Maybe<&2, Abs>, String.is_empty(rp),
    Bool.pick(Maybe<&2, Abs>, rhas, res.build(scheme, host, port, bpath, q, True{}), res.build(scheme, host, port, bpath, bq, bhas)),
    res.build(scheme, host, port, remove_dots(Bool.pick(String, String.starts_with(rp, "/"), rp, dir(bpath) ++ rp)), q, rhas))

def res.rel(scheme: String, host: String, port: U32, bpath: String, bq: String, bhas: Bool, r: PQ) -> Maybe<&2, Abs>:
  PQ{rp, q, rhas} = r
  res.rel.go(scheme, host, port, bpath, bq, bhas, rp, q, rhas)

def res.base(+r: String, b: PQ, a: Abs) -> Maybe<&2, Abs>:
  PQ{bpath, bq, bhas} = b
  Abs{scheme, host, port, target} = a
  res.rel(scheme, host, port, bpath, bq, bhas, res.ref(abs.fst(take_to(r, 35))))

def res.local(+r: String, a: Abs) -> Maybe<&2, Abs>:
  Abs{+scheme, +host, +port, +target} = a
  res.base(r, res.ref(target), Abs{scheme, host, port, target})

def abs.scheme_of(a: Abs) -> String:
  Abs{scheme, host, port, target} = a
  scheme

def res.net(+r: String, a: Abs, net: Bool) -> Maybe<&2, Abs>:
  match net:
    case True{}:
      absolute(abs.scheme_of(a) ++ ":" ++ r)
    case False{}:
      res.local(r, a)

def scheme.ok(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.or(Bool.or(Bool.and(U32.is_le(97, c), U32.is_le(c, 122)), Bool.and(U32.is_le(65, c), U32.is_le(c, 90))), Bool.or(Bool.and(U32.is_le(48, c), U32.is_le(c, 57)), Bool.or(U32.is_eq(c, 43), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 46))))), scheme.ok(t))

# §3.1: a ":" before any "/", "?" or "#" with a valid scheme name before it.
def abs.has_scheme(+pre: String, colon: Bool) -> Bool:
  Bool.and(colon, Bool.and(Bool.not(String.is_empty(pre)), scheme.ok(pre)))
# ref against base: absolute http(s) URLs as is, "//host...", "/path", "?q",
# or a relative path. Other schemes are None (fetch cannot follow them).
def resolve(a: Abs, +ref: String) -> Maybe<&2, Abs>:
  +low = String.to_lower(ref)
  Bool.pick(Maybe<&2, Abs>, Bool.or(String.starts_with(low, "http://"), String.starts_with(low, "https://")), absolute(ref),
    Bool.pick(Maybe<&2, Abs>, abs.has_scheme(abs.fst(take_to(ref, 58)), has(ref, 58)), None{}, res.net(ref, a, String.starts_with(ref, "//"))))

