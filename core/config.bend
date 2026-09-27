# core/config.bend — the config.toml reader, pure Bend.
#
# A pragmatic TOML subset, enough for the harness configuration: key
# = value lines, [section] headers, # comments and blank lines.
# Values are quoted strings, bare words or numbers. The parser stores
# (path, value) pairs - a key inside a section carries the section
# prefix ("harness.bg_after") - and the lookups match the exact path.
# Precedence on every key, everywhere in the runtime: environment
# variable > config file > built-in default.
import Base

type KV is Data:
  KV{key: String, val: String}

# ---- one value: trimmed, quotes stripped when quoted ----

def cfg_val.quoted(both: Bool, +t: String) -> String:
  match both:
    case True{}:
      String.take(String.drop(t, 1n), Nat.sub(String.length(t), 2n))
    case False{}:
      t

def cfg_val.quot(ends_q: Bool, +t: String) -> String:
  match ends_q:
    case True{}:
      cfg_val.quoted(Bool.and(String.starts_with(t, "\""),
        Nat.is_le(2n, String.length(t))), t)
    case False{}:
      t

def cfg_val(+raw: String) -> String:
  cfg_val.quot(String.ends_with(String.trim(raw), "\""), String.trim(raw))

# ---- the key path: a section prefix when inside a section ----

def cfg_path.pick(empty: Bool, +sect: String, +key: String) -> String:
  match empty:
    case True{}:
      key
    case False{}:
      String.append(sect, String.append(".", key))

def cfg_path(+sect: String, +key: String) -> String:
  cfg_path.pick(String.is_empty(sect), sect, key)

# ---- one pair line: key = value (a value may carry '=' itself) ----

def cfg_pair.parts(parts: List<&2, String>, +sect: String) -> KV:
  match parts:
    case Nil{}:
      KV{"", ""}
    case Con{+k, +rest}:
      KV{cfg_path(sect, String.trim(k)),
        cfg_val(String.join(rest, String.from_list(Con{Char.from_u32(61), Nil{}})))}

def cfg_pair(+l: String, +sect: String) -> KV:
  cfg_pair.parts(String.split(l, Char.from_u32(61)), sect)

# ---- one section line: the name inside the brackets ----

def cfg_sect.cut(close: Bool, +rest: String) -> String:
  match close:
    case True{}:
      String.take(String.trim(rest),
        Nat.sub(String.length(String.trim(rest)), 1n))
    case False{}:
      String.trim(rest)

def cfg_sect(+l: String) -> String:
  cfg_sect.cut(String.ends_with(String.trim(l), "]"),
    String.drop(String.trim(l), 1n))

# ---- the line walk ----
# The per-line routes ride in constructors and are matched on the
# next hop - no computed scrutinee; every recursive call shrinks the
# line list.

type CWalk is Data:
  CN{todo: List<&2, String>, sect: String, acc: List<&2, KV>}
  CR{is_blank: Bool, is_comment: Bool, is_sect: Bool, is_pair: Bool,
    line: String, todo: List<&2, String>, sect: String, acc: List<&2, KV>}
  CS{sect2: String, todo: List<&2, String>, acc: List<&2, KV>}
  CP{kv: KV, todo: List<&2, String>, sect: String, acc: List<&2, KV>}

def cfg_len(lines: List<&2, String>) -> Nat:
  match lines:
    case Nil{}:
      0n
    case Con{_, +rest}:
      Nat.add(cfg_len(rest), 1n)

def cwalk_go(fuel: Nat, w: CWalk) -> List<&2, KV>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match w:
        case CN{Nil{}, _, +acc}:
          List.reverse(&2, KV, acc)
        case CN{Con{+l, +rest}, +sect, +acc}:
          cwalk_go(k, CR{String.is_empty(String.trim(l)),
            String.starts_with(String.trim(l), "#"),
            String.starts_with(String.trim(l), "["),
            String.contains(String.trim(l), "="),
            l, rest, sect, acc})
        case CR{+is_blank, +is_comment, +is_sect, +is_pair, +line,
            +todo, +sect, +acc}:
          match is_blank:
            case True{}:
              cwalk_go(k, CN{todo, sect, acc})
            case False{}:
              match is_comment:
                case True{}:
                  cwalk_go(k, CN{todo, sect, acc})
                case False{}:
                  match is_sect:
                    case True{}:
                      cwalk_go(k, CS{cfg_sect(line), todo, acc})
                    case False{}:
                      match is_pair:
                        case True{}:
                          cwalk_go(k, CP{cfg_pair(line, sect), todo,
                            sect, acc})
                        case False{}:
                          cwalk_go(k, CN{todo, sect, acc})
        case CS{+sect2, +todo, +acc}:
          cwalk_go(k, CN{todo, sect2, acc})
        case CP{+kv, +todo, +sect, +acc}:
          cwalk_go(k, CN{todo, sect, Con{kv, acc}})

def parse_toml(+text: String) -> List<&2, KV>:
  +lines = String.split(text, Char.from_u32(10))
  cwalk_go(Nat.add(Nat.mul(cfg_len(lines), 2n), 16n),
    CN{lines, "", Nil{}})

# ---- the lookups ----

def cfg_head_match(kv: KV, +key: String) -> Bool:
  match kv:
    case KV{+k, _}:
      String.eq(k, key)

def cfg_val_of(kv: KV) -> String:
  match kv:
    case KV{_, +v}:
      v

def cfg_find.last(found: Bool, +v: String) -> Maybe<&2, String>:
  match found:
    case True{}:
      Some{v}
    case False{}:
      None{}

# the loop: found/v describe the head the PREVIOUS hop examined (the
# read-ahead idiom - the match Bool is always an argument)
def cfg_find.go(kvs: List<&2, KV>, +key: String, found: Bool,
    +v: String) -> Maybe<&2, String>:
  match kvs:
    case Nil{}:
      cfg_find.last(found, v)
    case Con{+kv, +rest}:
      match found:
        case True{}:
          Some{v}
        case False{}:
          cfg_find.go(rest, key, cfg_head_match(kv, key),
            cfg_val_of(kv))

def cfg_find(kvs: List<&2, KV>, +key: String) -> Maybe<&2, String>:
  match kvs:
    case Nil{}:
      None{}
    case Con{+kv, +rest}:
      cfg_find.go(rest, key, cfg_head_match(kv, key), cfg_val_of(kv))

def cfg_get(+text: String, +key: String) -> Maybe<&2, String>:
  cfg_find(parse_toml(text), key)

def cfg_or(m: Maybe<&2, String>, +dflt: String) -> String:
  match m:
    case Some{+s}:
      s
    case None{}:
      dflt

# the precedence everywhere: environment variable > config > default
def env_cfg_or.pick(empty: Bool, +envv: String,
    +cfgm: Maybe<&2, String>, +dflt: String) -> String:
  match empty:
    case True{}:
      cfg_or(cfgm, dflt)
    case False{}:
      envv

def env_cfg_or(+envv: String, +cfgm: Maybe<&2, String>,
    +dflt: String) -> String:
  env_cfg_or.pick(String.is_empty(envv), envv, cfgm, dflt)

# numbers ride as U32 (threshold, background seconds)
def cfg_u32.read(m: Maybe<&2, U32>, +dflt: U32) -> U32:
  match m:
    case Some{+u}:
      u
    case None{}:
      dflt

def cfg_u32.get(m: Maybe<&2, String>, +dflt: U32) -> U32:
  match m:
    case Some{+s}:
      cfg_u32.read(U32.read(s), dflt)
    case None{}:
      dflt

def cfg_u32(+text: String, +key: String, +dflt: U32) -> U32:
  cfg_u32.get(cfg_get(text, key), dflt)
