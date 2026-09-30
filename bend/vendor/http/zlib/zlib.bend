# DEFLATE, gzip, and zlib decoding (RFC 1951, 1952, 1950) over byte strings.
import Base

# Bytes are one Char per octet (0..255), as everywhere in bend-net.

def byte(h: Char) -> U32:
  Chr{c} = h
  c

def bits(+n: U32) -> Nat:
  U32.to_nat(n)

def mask(+n: U32) -> U32:
  (U32.shln(1, bits(n)) - 1 : U32)

# Bit reader. over counts bytes read past the end; any over means the input was cut short.
type Br is Data:
  Br{s: String, buf: U32, cnt: U32, over: U32}

def br.new(s: String) -> Br:
  Br{s, 0, 0, 0}

def br.pull(b: Br) -> Br:
  Br{s, +buf, +cnt, +over} = b
  match s:
    case SNil{}:
      Br{SNil{}, buf, (cnt + 8 : U32), (over + 1 : U32)}
    case SCon{h, t}:
      Br{t, U32.or(buf, U32.shln(byte(h), bits(cnt))), (cnt + 8 : U32), over}

def br.pull.if(b: Br, short: Bool) -> Br:
  match short:
    case True{}:
      br.pull(b)
    case False{}:
      b

def br.short(+b: Br, +n: U32) -> Bool:
  Br{s, buf, +cnt, over} = b
  U32.is_lt(cnt, n)

# n is at most 16, so three bytes always fill the buffer.
def br.fill(fuel: Nat, +b: Br, +n: U32) -> Br:
  match fuel:
    case 0n:
      b
    case 1n+f:
      br.fill(f, br.pull.if(b, br.short(b, n)), n)

def br.cut(b: Br, +n: U32) -> Br & U32:
  Br{s, +buf, cnt, over} = b
  (Br{s, U32.shrn(buf, bits(n)), (cnt - n : U32), over}, U32.and(buf, mask(n)))

def br.take(b: Br, +n: U32) -> Br & U32:
  br.cut(br.fill(3n, b, n), n)

def br.drop.of(r: Br & U32) -> Br:
  (b, v) = r
  b

def br.drop(b: Br, +n: U32) -> Br:
  br.drop.of(br.cut(b, n))

# RFC 1951 §3.2.4: a stored block starts on a byte boundary.
def br.align(+b: Br) -> Br:
  Br{s, buf, +cnt, over} = b
  br.drop(b, U32.mod(cnt, 8))

def br.ok(+b: Br) -> Bool:
  Br{s, buf, cnt, +over} = b
  U32.is_eq(over, 0)

def br.bytes(k: Nat, +buf: U32) -> String:
  match k:
    case 0n:
      SNil{}
    case 1n+p:
      SCon{Chr{U32.and(buf, 255)}, br.bytes(p, U32.shrn(buf, 8n))}

def br.rest.of(b: Br) -> String:
  Br{s, +buf, +cnt, over} = b
  br.bytes(bits(U32.div(cnt, 8)), buf) ++ s

# Whole bytes still in the buffer go back in front of the unread input.
def br.rest(+b: Br) -> String:
  br.rest.of(br.align(b))

# Huffman codes as a trie: one step per bit. A read bit picks o (1) or z (0).
# Bad: two codes collided, so the code set was over-subscribed.
type Ht is Data:
  HtNone{}
  HtBad{}
  HtLeaf{sym: U32}
  HtNode{z: Ht, o: Ht}

def ht.left(t: Ht) -> Ht:
  match t:
    case HtNode{z, o}:
      z
    case HtNone{}:
      HtNone{}
    case HtBad{}:
      HtBad{}
    case HtLeaf{s}:
      HtBad{}

def ht.right(t: Ht) -> Ht:
  match t:
    case HtNode{z, o}:
      o
    case HtNone{}:
      HtNone{}
    case HtBad{}:
      HtBad{}
    case HtLeaf{s}:
      HtBad{}

def ht.leaf(t: Ht, +sym: U32) -> Ht:
  match t:
    case HtNone{}:
      HtLeaf{sym}
    case HtBad{}:
      HtBad{}
    case HtLeaf{s}:
      HtBad{}
    case HtNode{z, o}:
      HtBad{}

def ht.ins(path: List<&2, Bool>, +t: Ht, +sym: U32) -> Ht:
  match path:
    case Nil{}:
      ht.leaf(t, sym)
    case Con{b, rest}:
      match b:
        case True{}:
          HtNode{ht.left(t), ht.ins(rest, ht.right(t), sym)}
        case False{}:
          HtNode{ht.ins(rest, ht.left(t), sym), ht.right(t)}

# The code's bits, most significant first: the order the stream sends them.
def ht.path(n: Nat, +code: U32, acc: List<&2, Bool>) -> List<&2, Bool>:
  match n:
    case 0n:
      acc
    case 1n+p:
      ht.path(p, U32.shr(code), Con{U32.is_eq(U32.and(code, 1), 1), acc})

# Canonical codes (RFC 1951 §3.2.2): count lengths, find each length's first code, then assign in symbol order.
def cnt.add(ar: Array<U32> & U32, +len: U32) -> Array<U32>:
  (a, v) = ar
  Array.set(U32, a, len, (v + 1 : U32))

def cnt.go(xs: List<&2, U32>, a: Array<U32>) -> Array<U32>:
  match xs:
    case Nil{}:
      a
    case Con{+l, t}:
      cnt.go(t, cnt.add(Array.get(U32, a, l), l))

def nx.set(+i: U32, cv: Array<U32> & U32, n: Array<U32>, +code: U32) -> Array<U32> & Array<U32> & U32:
  (c, v) = cv
  +code2 = U32.shl((code + Bool.pick(U32, U32.is_eq(i, 1), 0, v) : U32))
  (c, Array.set(U32, n, i, code2), code2)

def nx.step(+i: U32, st: Array<U32> & Array<U32> & U32) -> Array<U32> & Array<U32> & U32:
  (c, n, code) = st
  nx.set(i, Array.get(U32, c, (i - 1 : U32)), n, code)

def nx.go(k: Nat, +i: U32, st: Array<U32> & Array<U32> & U32) -> Array<U32> & Array<U32> & U32:
  match k:
    case 0n:
      st
    case 1n+f:
      nx.go(f, (i + 1 : U32), nx.step(i, st))

def as.put(+l: U32, +sym: U32, nv: Array<U32> & U32, t: Ht) -> Array<U32> & Ht:
  (n, +code) = nv
  (Array.set(U32, n, l, (code + 1 : U32)), ht.ins(ht.path(bits(l), code, Nil{}), t, sym))

def as.one.nz(+l: U32, +sym: U32, n: Array<U32>, t: Ht, zero: Bool) -> Array<U32> & Ht:
  match zero:
    case True{}:
      (n, t)
    case False{}:
      as.put(l, sym, Array.get(U32, n, l), t)

def as.one(+l: U32, +sym: U32, st: Array<U32> & Ht) -> Array<U32> & Ht:
  (n, t) = st
  as.one.nz(l, sym, n, t, U32.is_zero(l))

def as.go(xs: List<&2, U32>, +sym: U32, st: Array<U32> & Ht) -> Array<U32> & Ht:
  match xs:
    case Nil{}:
      st
    case Con{+l, t}:
      as.go(t, (sym + 1 : U32), as.one(l, sym, st))

def ht.of(st: Array<U32> & Ht) -> Ht:
  (n, t) = st
  t

def ht.assign(xs: List<&2, U32>, st: Array<U32> & Array<U32> & U32) -> Ht:
  (c, n, code) = st
  ht.of(as.go(xs, 0, (n, HtNone{})))

# lens[i] is the code length of symbol i; 0 means the symbol is unused.
def ht.build(+lens: List<&2, U32>) -> Ht:
  ht.assign(lens, nx.go(15n, 1, (cnt.go(lens, Array.new(U32, 4n, 0)), Array.new(U32, 4n, 0), 0)))

# No code: 9999, never a symbol.
def ht.step(z: Ht, o: Ht, bv: Br & U32) -> Ht & Br:
  (b, +v) = bv
  (Bool.pick(Ht, U32.is_eq(v, 1), o, z), b)

# A code is at most 15 bits (§3.2.7), so 16 steps always reach a leaf or a miss.
def ht.dec(fuel: Nat, st: Ht & Br) -> Br & U32:
  match fuel:
    case 0n:
      (t, b) = st
      (b, 9999)
    case 1n+f:
      (t, b) = st
      match t:
        case HtLeaf{s}:
          (b, s)
        case HtNone{}:
          (b, 9999)
        case HtBad{}:
          (b, 9999)
        case HtNode{z, o}:
          ht.dec(f, ht.step(z, o, br.take(b, 1)))

def ht.decode(t: Ht, b: Br) -> Br & U32:
  ht.dec(16n, (t, b))

def nth(xs: List<&2, U32>, +i: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{h, t}:
      Bool.pick(U32, U32.is_zero(i), h, nth(t, (i - 1 : U32)))

def rep(k: Nat, +v: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match k:
    case 0n:
      acc
    case 1n+p:
      rep(p, v, Con{v, acc})

def take.n(k: Nat, xs: List<&2, U32>) -> List<&2, U32>:
  match k:
    case 0n:
      Nil{}
    case 1n+p:
      match xs:
        case Nil{}:
          Nil{}
        case Con{h, t}:
          Con{h, take.n(p, t)}

def drop.n(k: Nat, xs: List<&2, U32>) -> List<&2, U32>:
  match k:
    case 0n:
      xs
    case 1n+p:
      match xs:
        case Nil{}:
          Nil{}
        case Con{h, t}:
          drop.n(p, t)

# RFC 1951 §3.2.5
def len.base() -> List<&2, U32>:
  [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258]

def len.extra() -> List<&2, U32>:
  [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0]

def dist.base() -> List<&2, U32>:
  [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577]

def dist.extra() -> List<&2, U32>:
  [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13]

# RFC 1951 §3.2.6
def fixed.lit() -> Ht:
  ht.build(rep(144n, 8, rep(112n, 9, rep(24n, 7, rep(8n, 8, Nil{})))))

def fixed.dist() -> Ht:
  ht.build(rep(30n, 5, Nil{}))

# Output: the 32 KiB window (indexes wrap), the write position, the byte count, and the bytes, reversed.
type Ow is Data:
  Ow{w: U32, n: U32, racc: String}

def out.put(win: Array<U32>, +c: U32, o: Ow) -> Array<U32> & Ow:
  Ow{+w, +n, racc} = o
  (Array.set(U32, win, w, c), Ow{(w + 1 : U32), (n + 1 : U32), SCon{Chr{c}, racc}})

type Mo is Data:
  MoHead{}
  MoCodes{lit: Ht, dist: Ht}
  MoDone{}
  MoBad{}

type Is is Data:
  Is{br: Br, o: Ow, mode: Mo, last: Bool}

def inf.next(last: Bool) -> Mo:
  match last:
    case True{}:
      MoDone{}
    case False{}:
      MoHead{}

def inf.bad(win: Array<U32>, b: Br, o: Ow) -> Array<U32> & Is:
  (win, Is{b, o, MoBad{}, True{}})

# Stored block (§3.2.4): LEN, then NLEN (its complement), then LEN raw bytes.
def sb.put2(p: Array<U32> & Ow, b: Br) -> Array<U32> & Br & Ow:
  (a, o) = p
  (a, b, o)

def sb.put(win: Array<U32>, bv: Br & U32, o: Ow) -> Array<U32> & Br & Ow:
  (b, +c) = bv
  sb.put2(out.put(win, c, o), b)

def sb.byte(st: Array<U32> & Br & Ow) -> Array<U32> & Br & Ow:
  (win, b, o) = st
  sb.put(win, br.take(b, 8), o)

def sb.go(k: Nat, st: Array<U32> & Br & Ow) -> Array<U32> & Br & Ow:
  match k:
    case 0n:
      st
    case 1n+f:
      sb.go(f, sb.byte(st))

def sb.done(t: Array<U32> & Br & Ow, +last: Bool) -> Array<U32> & Is:
  (win, b, o) = t
  (win, Is{b, o, inf.next(last), last})

def sb.ok(win: Array<U32>, b: Br, +len: U32, o: Ow, +last: Bool, ok: Bool) -> Array<U32> & Is:
  match ok:
    case False{}:
      inf.bad(win, b, o)
    case True{}:
      sb.done(sb.go(bits(len), (win, b, o)), last)

def sb.nlen(win: Array<U32>, bv: Br & U32, +len: U32, o: Ow, +last: Bool) -> Array<U32> & Is:
  (b, +nlen) = bv
  sb.ok(win, b, len, o, last, U32.is_eq(U32.xor(len, 65535), nlen))

def sb.len(win: Array<U32>, bv: Br & U32, o: Ow, +last: Bool) -> Array<U32> & Is:
  (b, +len) = bv
  sb.nlen(win, br.take(b, 16), len, o, last)

def inf.stored(win: Array<U32>, b: Br, o: Ow, +last: Bool) -> Array<U32> & Is:
  sb.len(win, br.take(br.align(b), 16), o, last)

# Dynamic block (§3.2.7): code lengths for the code-length code, then the lengths themselves.
# cl.slot[i]: where symbol i's length sits in the order the stream sends them.
def cl.slot() -> List<&2, U32>:
  [3, 17, 15, 13, 11, 9, 7, 5, 4, 6, 8, 10, 12, 14, 16, 18, 0, 1, 2]

def cl.place(xs: List<&2, U32>, +vals: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+i, t}:
      Con{nth(vals, i), cl.place(t, vals)}

def cl.push(bv: Br & U32, acc: List<&2, U32>) -> Br & List<&2, U32>:
  (b, v) = bv
  (b, Con{v, acc})

def cl.one(st: Br & List<&2, U32>) -> Br & List<&2, U32>:
  (b, acc) = st
  cl.push(br.take(b, 3), acc)

def cl.read(k: Nat, st: Br & List<&2, U32>) -> Br & List<&2, U32>:
  match k:
    case 0n:
      st
    case 1n+f:
      cl.read(f, cl.one(st))

# Lengths so far (reversed), how many, the last one (for code 16), and whether it went wrong.
type Cd is Data:
  Cd{br: Br, acc: List<&2, U32>, n: U32, prev: U32, bad: Bool}

def cd.fill(+b: Br, +acc: List<&2, U32>, +n: U32, +v: U32, +cnt: U32, +total: U32) -> Cd:
  Bool.pick(Cd, U32.is_lt(total, (n + cnt : U32)), Cd{b, acc, n, v, True{}}, Cd{b, rep(bits(cnt), v, acc), (n + cnt : U32), v, False{}})

def cd.rep(bv: Br & U32, +v: U32, +base: U32, acc: List<&2, U32>, +n: U32, +total: U32) -> Cd:
  (b, +e) = bv
  cd.fill(b, acc, n, v, (base + e : U32), total)

def cd.prev(b: Br, acc: List<&2, U32>, +n: U32, +prev: U32, +total: U32, ok: Bool) -> Cd:
  match ok:
    case False{}:
      Cd{b, acc, n, prev, True{}}
    case True{}:
      cd.rep(br.take(b, 2), prev, 3, acc, n, total)

def cd.z18(b: Br, acc: List<&2, U32>, +n: U32, +total: U32, ok: Bool) -> Cd:
  match ok:
    case True{}:
      cd.rep(br.take(b, 7), 0, 11, acc, n, total)
    case False{}:
      Cd{b, acc, n, 0, True{}}

def cd.z17(b: Br, acc: List<&2, U32>, +n: U32, +s: U32, +total: U32, ok: Bool) -> Cd:
  match ok:
    case True{}:
      cd.rep(br.take(b, 3), 0, 3, acc, n, total)
    case False{}:
      cd.z18(b, acc, n, total, U32.is_eq(s, 18))

def cd.c16(b: Br, acc: List<&2, U32>, +n: U32, +prev: U32, +s: U32, +total: U32, ok: Bool) -> Cd:
  match ok:
    case True{}:
      cd.prev(b, acc, n, prev, total, U32.is_lt(0, n))
    case False{}:
      cd.z17(b, acc, n, s, total, U32.is_eq(s, 17))

def cd.lit(b: Br, acc: List<&2, U32>, +n: U32, +prev: U32, +s: U32, +total: U32, ok: Bool) -> Cd:
  match ok:
    case True{}:
      Cd{b, Con{s, acc}, (n + 1 : U32), s, False{}}
    case False{}:
      cd.c16(b, acc, n, prev, s, total, U32.is_eq(s, 16))

def cd.sym(bv: Br & U32, acc: List<&2, U32>, +n: U32, +prev: U32, +total: U32) -> Cd:
  (b, +s) = bv
  cd.lit(b, acc, n, prev, s, total, U32.is_lt(s, 16))

def cd.step.go(+cl: Ht, +total: U32, stop: Bool, cs: Cd) -> Cd:
  match stop:
    case False{}:
      Cd{b, acc, +n, +prev, bad} = cs
      cd.sym(ht.decode(cl, b), acc, n, prev, total)
    case True{}:
      Cd{b, acc, n, prev, bad} = cs
      Cd{b, acc, n, prev, bad}

def cd.stop(+st: Cd, +total: U32) -> Bool:
  Cd{b, acc, +n, prev, +bad} = st
  Bool.or(bad, U32.is_le(total, n))

# Each step adds at least one length, so total steps suffice.
def cd.go(k: Nat, +cl: Ht, +total: U32, +st: Cd) -> Cd:
  match k:
    case 0n:
      st
    case 1n+f:
      cd.go(f, cl, total, cd.step.go(cl, total, cd.stop(st, total), st))

def dyn.tables(b: Br, +lens: List<&2, U32>, +hlit: U32, bad: Bool) -> Br & Mo:
  match bad:
    case True{}:
      (b, MoBad{})
    case False{}:
      (b, MoCodes{ht.build(take.n(bits(hlit), lens)), ht.build(drop.n(bits(hlit), lens))})

def dyn.done(st: Cd, +hlit: U32, +total: U32) -> Br & Mo:
  Cd{b, acc, +n, prev, +bad} = st
  dyn.tables(b, List.reverse(&2, U32, acc), hlit, Bool.or(bad, Bool.not(U32.is_eq(n, total))))

def dyn.lens(st: Br & List<&2, U32>, +hlit: U32, +hdist: U32) -> Br & Mo:
  (b, acc) = st
  +total = (hlit + hdist : U32)
  +clens = cl.place(cl.slot(), List.reverse(&2, U32, acc))
  dyn.done(cd.go(bits(total), ht.build(clens), total, Cd{b, Nil{}, 0, 0, False{}}), hlit, total)

def dyn.clen(bv: Br & U32, +hlit: U32, +hdist: U32) -> Br & Mo:
  (b, +hclen) = bv
  dyn.lens(cl.read(bits((hclen + 4 : U32)), (b, Nil{})), hlit, hdist)

def dyn.hdist(bv: Br & U32, +hlit: U32) -> Br & Mo:
  (b, +hd) = bv
  dyn.clen(br.take(b, 4), hlit, (hd + 1 : U32))

def dyn.hlit(bv: Br & U32) -> Br & Mo:
  (b, +hl) = bv
  dyn.hdist(br.take(b, 5), (hl + 257 : U32))

def dyn.read(b: Br) -> Br & Mo:
  dyn.hlit(br.take(b, 5))

# A match copies len bytes from d back; the window's indexes wrap, so w - d needs no mask.
def cp.put(av: Array<U32> & U32, o: Ow) -> Array<U32> & Ow:
  (a, +c) = av
  out.put(a, c, o)

def cp.one(+d: U32, st: Array<U32> & Ow) -> Array<U32> & Ow:
  (win, o) = st
  Ow{+w, n, racc} = o
  cp.put(Array.get(U32, win, (w - d : U32)), Ow{w, n, racc})

def cp.go(k: Nat, +d: U32, st: Array<U32> & Ow) -> Array<U32> & Ow:
  match k:
    case 0n:
      st
    case 1n+f:
      cp.go(f, d, cp.one(d, st))

def inf.codes(p: Array<U32> & Ow, b: Br, +lit: Ht, +dist: Ht, +last: Bool) -> Array<U32> & Is:
  (win, o) = p
  (win, Is{b, o, MoCodes{lit, dist}, last})

def ow.n(+o: Ow) -> U32:
  Ow{w, +n, racc} = o
  n

# RFC 1951 §3.2.5: a distance may not reach before the first byte out.
def inf.far(win: Array<U32>, b: Br, +o: Ow, +len: U32, +d: U32, +lit: Ht, +dist: Ht, +last: Bool, ok: Bool) -> Array<U32> & Is:
  match ok:
    case False{}:
      inf.bad(win, b, o)
    case True{}:
      inf.codes(cp.go(bits(len), d, (win, o)), b, lit, dist, last)

def inf.dx(win: Array<U32>, bv: Br & U32, +db: U32, +len: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool) -> Array<U32> & Is:
  (b, +e) = bv
  +d = (db + e : U32)
  inf.far(win, b, o, len, d, lit, dist, last, U32.is_le(d, ow.n(o)))

def inf.ds(win: Array<U32>, b: Br, +ds: U32, +len: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool, ok: Bool) -> Array<U32> & Is:
  match ok:
    case False{}:
      inf.bad(win, b, o)
    case True{}:
      inf.dx(win, br.take(b, nth(dist.extra(), ds)), nth(dist.base(), ds), len, o, lit, dist, last)

def inf.d(win: Array<U32>, bv: Br & U32, +len: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool) -> Array<U32> & Is:
  (b, +ds) = bv
  inf.ds(win, b, ds, len, o, lit, dist, last, U32.is_lt(ds, 30))

def inf.lx(win: Array<U32>, bv: Br & U32, +lb: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool) -> Array<U32> & Is:
  (b, +e) = bv
  inf.d(win, ht.decode(dist, b), (lb + e : U32), o, lit, dist, last)

def inf.ls(win: Array<U32>, b: Br, +s: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool, ok: Bool) -> Array<U32> & Is:
  match ok:
    case False{}:
      inf.bad(win, b, o)
    case True{}:
      +i = (s - 257 : U32)
      inf.lx(win, br.take(b, nth(len.extra(), i)), nth(len.base(), i), o, lit, dist, last)

def inf.end(win: Array<U32>, b: Br, +s: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool, eob: Bool) -> Array<U32> & Is:
  match eob:
    case True{}:
      (win, Is{b, o, inf.next(last), last})
    case False{}:
      inf.ls(win, b, s, o, lit, dist, last, U32.is_le(s, 285))

def inf.lit(win: Array<U32>, b: Br, +s: U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool, ok: Bool) -> Array<U32> & Is:
  match ok:
    case True{}:
      inf.codes(out.put(win, s, o), b, lit, dist, last)
    case False{}:
      inf.end(win, b, s, o, lit, dist, last, U32.is_eq(s, 256))

def inf.sym(win: Array<U32>, bv: Br & U32, +o: Ow, +lit: Ht, +dist: Ht, +last: Bool) -> Array<U32> & Is:
  (b, +s) = bv
  inf.lit(win, b, s, o, lit, dist, last, U32.is_lt(s, 256))

def inf.dyn(win: Array<U32>, bm: Br & Mo, o: Ow, +last: Bool) -> Array<U32> & Is:
  (b, m) = bm
  (win, Is{b, o, m, last})

def inf.t2(win: Array<U32>, b: Br, o: Ow, +last: Bool, is2: Bool) -> Array<U32> & Is:
  match is2:
    case True{}:
      inf.dyn(win, dyn.read(b), o, last)
    case False{}:
      inf.bad(win, b, o)

def inf.t1(win: Array<U32>, b: Br, o: Ow, +last: Bool, +t: U32, is1: Bool) -> Array<U32> & Is:
  match is1:
    case True{}:
      (win, Is{b, o, MoCodes{fixed.lit(), fixed.dist()}, last})
    case False{}:
      inf.t2(win, b, o, last, U32.is_eq(t, 2))

def inf.t0(win: Array<U32>, b: Br, o: Ow, +last: Bool, +t: U32, is0: Bool) -> Array<U32> & Is:
  match is0:
    case True{}:
      inf.stored(win, b, o, last)
    case False{}:
      inf.t1(win, b, o, last, t, U32.is_eq(t, 1))

# §3.2.3: BFINAL, then BTYPE.
def inf.head(win: Array<U32>, bv: Br & U32, o: Ow) -> Array<U32> & Is:
  (b, +v) = bv
  +t = U32.shr(v)
  inf.t0(win, b, o, U32.is_eq(U32.and(v, 1), 1), t, U32.is_zero(t))

# Every step reads at least one bit, so 8 steps per input byte always finish a valid stream.
def inf.go(k: Nat, st: Array<U32> & Is) -> Array<U32> & Is:
  match k:
    case 0n:
      st
    case 1n+f:
      (win, ist) = st
      Is{b, o, mode, +last} = ist
      match mode:
        case MoHead{}:
          inf.go(f, inf.head(win, br.take(b, 3), o))
        case MoCodes{+lit, +dist}:
          inf.go(f, inf.sym(win, ht.decode(lit, b), o, lit, dist, last))
        case MoDone{}:
          (win, Is{b, o, MoDone{}, last})
        case MoBad{}:
          (win, Is{b, o, MoBad{}, last})

# out: the bytes; rest: the input after the stream; n: len(out) mod 2^32.
type Inflated is Data:
  Inflated{out: String, rest: String, n: U32}

def inf.fin(ok: Bool, b: Br, o: Ow) -> Maybe<&2, Inflated>:
  match ok:
    case False{}:
      None{}
    case True{}:
      Ow{w, n, racc} = o
      Some{Inflated{String.reverse(racc), br.rest(b), n}}

def inf.result(st: Array<U32> & Is) -> Maybe<&2, Inflated>:
  (win, ist) = st
  Is{+b, o, mode, last} = ist
  match mode:
    case MoDone{}:
      inf.fin(br.ok(b), b, o)
    case MoHead{}:
      None{}
    case MoCodes{l, d}:
      None{}
    case MoBad{}:
      None{}

def inflate.rest(+s: String) -> Maybe<&2, Inflated>:
  inf.result(inf.go(Nat.add(Nat.mul(String.length(s), 8n), 8n), (Array.new(U32, 15n, 0), Is{br.new(s), Ow{0, 0, ""}, MoHead{}, False{}})))

def inflate.out(m: Maybe<&2, Inflated>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{Inflated{out, rest, n}}:
      Some{out}

# Raw DEFLATE. None: malformed or cut short.
def inflate(+s: String) -> Maybe<&2, String>:
  inflate.out(inflate.rest(s))

# CRC-32 (RFC 1952 §8), reflected, polynomial 0xEDB88320.
def crc.bit(+c: U32) -> U32:
  Bool.pick(U32, U32.is_eq(U32.and(c, 1), 1), U32.xor(U32.shr(c), 3988292384), U32.shr(c))

def crc.bits(k: Nat, +c: U32) -> U32:
  match k:
    case 0n:
      c
    case 1n+p:
      crc.bits(p, crc.bit(c))

def crc.fill(k: Nat, +i: U32, a: Array<U32>) -> Array<U32>:
  match k:
    case 0n:
      a
    case 1n+p:
      crc.fill(p, (i + 1 : U32), Array.set(U32, a, i, crc.bits(8n, i)))

def crc.table() -> Array<U32>:
  crc.fill(256n, 0, Array.new(U32, 8n, 0))

def crc.mix(av: Array<U32> & U32, +c: U32) -> Array<U32> & U32:
  (a, +t) = av
  (a, U32.xor(t, U32.shrn(c, 8n)))

def crc.step(st: Array<U32> & U32, +b: U32) -> Array<U32> & U32:
  (a, +c) = st
  crc.mix(Array.get(U32, a, U32.and(U32.xor(c, b), 255)), c)

def crc.go(s: String, st: Array<U32> & U32) -> Array<U32> & U32:
  match s:
    case SNil{}:
      st
    case SCon{h, t}:
      crc.go(t, crc.step(st, byte(h)))

def crc.of(st: Array<U32> & U32) -> U32:
  (a, +c) = st
  U32.xor(c, 4294967295)

def crc32(s: String) -> U32:
  crc.of(crc.go(s, (crc.table(), 4294967295)))

# Adler-32 (RFC 1950 §9).
def adler.go(s: String, +a: U32, +b: U32) -> U32:
  match s:
    case SNil{}:
      U32.or(U32.shln(b, 16n), a)
    case SCon{h, t}:
      +a2 = U32.mod((a + byte(h) : U32), 65521)
      adler.go(t, a2, U32.mod((b + a2 : U32), 65521))

def adler32(s: String) -> U32:
  adler.go(s, 1, 0)

# Little- and big-endian 32-bit words at the front of s.
def le32(s: String) -> Maybe<&2, U32>:
  match s:
    case SCon{Chr{+a}, SCon{Chr{+b}, SCon{Chr{+c}, SCon{Chr{+d}, t}}}}:
      Some{U32.or(U32.or(a, U32.shln(b, 8n)), U32.or(U32.shln(c, 16n), U32.shln(d, 24n)))}
    case SNil{}:
      None{}
    case SCon{x, y}:
      None{}

def be32(s: String) -> Maybe<&2, U32>:
  match s:
    case SCon{Chr{+a}, SCon{Chr{+b}, SCon{Chr{+c}, SCon{Chr{+d}, t}}}}:
      Some{U32.or(U32.or(U32.shln(a, 24n), U32.shln(b, 16n)), U32.or(U32.shln(c, 8n), d))}
    case SNil{}:
      None{}
    case SCon{x, y}:
      None{}

def flag(+f: U32, +bit: U32) -> Bool:
  Bool.not(U32.is_zero(U32.and(f, bit)))

def check(ok: Bool, +out: String) -> Maybe<&2, String>:
  match ok:
    case True{}:
      Some{out}
    case False{}:
      None{}

# RFC 1952 §2.3: after the fixed 10 bytes, optional FEXTRA, FNAME, FCOMMENT, FHCRC.
# hit: the byte just read was the terminating zero.
def gz.zstr(s: String, hit: Bool) -> Maybe<&2, String>:
  match s:
    case SNil{}:
      match hit:
        case True{}:
          Some{SNil{}}
        case False{}:
          None{}
    case SCon{Chr{+c}, t}:
      match hit:
        case True{}:
          Some{SCon{Chr{c}, t}}
        case False{}:
          gz.zstr(t, U32.is_zero(c))

def gz.skip(k: Nat, s: String) -> Maybe<&2, String>:
  match k:
    case 0n:
      Some{s}
    case 1n+p:
      match s:
        case SNil{}:
          None{}
        case SCon{h, t}:
          gz.skip(p, t)

def gz.extra.len(s: String) -> Maybe<&2, String>:
  match s:
    case SCon{Chr{+a}, SCon{Chr{+b}, t}}:
      gz.skip(bits(U32.or(a, U32.shln(b, 8n))), t)
    case SNil{}:
      None{}
    case SCon{x, y}:
      None{}

def gz.fextra(on: Bool, +s: String) -> Maybe<&2, String>:
  match on:
    case True{}:
      gz.extra.len(s)
    case False{}:
      Some{s}

def gz.fstr(on: Bool, +s: String) -> Maybe<&2, String>:
  match on:
    case True{}:
      gz.zstr(s, False{})
    case False{}:
      Some{s}

def gz.fhcrc(on: Bool, +s: String) -> Maybe<&2, String>:
  match on:
    case True{}:
      gz.skip(2n, s)
    case False{}:
      Some{s}

def gz.flags(+f: U32, s: String) -> Maybe<&2, String>:
  do Maybe<&2, String>:
    s1 : String <- gz.fextra(flag(f, 4), s)
    s2 : String <- gz.fstr(flag(f, 8), s1)
    s3 : String <- gz.fstr(flag(f, 16), s2)
    gz.fhcrc(flag(f, 2), s3)

def gz.head(s: String) -> Maybe<&2, String>:
  match s:
    case SCon{Chr{+id1}, SCon{Chr{+id2}, SCon{Chr{+cm}, SCon{Chr{+f}, t}}}}:
      Bool.pick(Maybe<&2, String>, Bool.and(Bool.and(U32.is_eq(id1, 31), U32.is_eq(id2, 139)), U32.is_eq(cm, 8)), gz.flags(f, String.drop(t, 6n)), None{})
    case SNil{}:
      None{}
    case SCon{x, y}:
      None{}

def gz.trail2(+out: String, +n: U32, crc: Maybe<&2, U32>, size: Maybe<&2, U32>) -> Maybe<&2, String>:
  do Maybe<&2, String>:
    c : U32 <- crc
    z : U32 <- size
    check(Bool.and(U32.is_eq(c, crc32(out)), U32.is_eq(z, n)), out)

def gz.trail(m: Maybe<&2, Inflated>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{Inflated{+out, +rest, +n}}:
      gz.trail2(out, n, le32(rest), le32(String.drop(rest, 4n)))

def gz.body(m: Maybe<&2, String>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+s}:
      gz.trail(inflate.rest(s))

# One gzip member; the CRC-32 and ISIZE must match. None: malformed, cut short, or corrupt.
# ponytail: bytes after the first member are ignored; loop over members if a server concatenates them.
def gunzip(s: String) -> Maybe<&2, String>:
  gz.body(gz.head(s))

# RFC 1950: CMF and FLG, then DEFLATE, then Adler-32, big-endian.
def zl.sum(+out: String, a: Maybe<&2, U32>) -> Maybe<&2, String>:
  match a:
    case None{}:
      None{}
    case Some{+v}:
      check(U32.is_eq(v, adler32(out)), out)

def zl.trail(m: Maybe<&2, Inflated>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{Inflated{+out, +rest, n}}:
      zl.sum(out, be32(rest))

def zl.ok(+cmf: U32, +flg: U32) -> Bool:
  Bool.and(Bool.and(U32.is_eq(U32.and(cmf, 15), 8), U32.is_le(U32.shrn(cmf, 4n), 7)), Bool.and(U32.is_zero(U32.mod((cmf * 256 + flg : U32), 31)), Bool.not(flag(flg, 32))))

def unzlib(s: String) -> Maybe<&2, String>:
  match s:
    case SCon{Chr{+cmf}, SCon{Chr{+flg}, +t}}:
      Bool.pick(Maybe<&2, String>, zl.ok(cmf, flg), zl.trail(inflate.rest(t)), None{})
    case SNil{}:
      None{}
    case SCon{x, y}:
      None{}
