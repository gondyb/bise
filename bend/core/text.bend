# core/text.bend — string helpers shared by every module.
#
# One home for the small text primitives the parsers and the wire code
# all need. Everything here is pure and total.
import Base

# the line-feed character
def newline() -> Char:
  Char.from_u32(10)

# every line break becomes one space: the text fits one wire line
def flatten(+s: String) -> String:
  String.join(String.split(s, newline()), " ")

# ---- substring search (first occurrence) ----

# `here`: does `pat` start at the head of `s`. The scan stops at the
# first hit (a strict call would evaluate the rest of the string too:
# every search used to walk its whole input, even when the hit came
# first - megabytes per model call on the request and the body)
def find.go(s: String, +pat: String, +i: Nat, here: Bool) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      None{}
    case SCon{_, +t}:
      match here:
        case True{}:
          Some{i}
        case False{}:
          find.go(t, pat, Nat.add(i, 1n), String.starts_with(t, pat))

# the index of the first occurrence of `pat` in `s`, counting from `i`
def find(+s: String, +pat: String, +i: Nat) -> Maybe<&2, Nat>:
  find.go(s, pat, i, String.starts_with(s, pat))

# `s` with every occurrence of `pat` taken out (the laws' view of a
# request body without its moving cache marks, BISE-268); fuel = the
# length, as each hop consumes at least one char
def drop_all.go(fuel: Nat, here: Bool, +s: String, +pat: String) -> String:
  match fuel:
    case 0n:
      s
    case 1n+k:
      match here:
        case True{}:
          drop_all.go(k,
            String.starts_with(String.drop(s, String.length(pat)), pat),
            String.drop(s, String.length(pat)), pat)
        case False{}:
          match s:
            case SNil{}:
              SNil{}
            case SCon{h, +t}:
              SCon{h, drop_all.go(k, String.starts_with(t, pat), t, pat)}

def drop_all(+s: String, +pat: String) -> String:
  drop_all.go(String.length(s),
    Bool.and(Bool.not(String.is_empty(pat)), String.starts_with(s, pat)),
    s, pat)

# one shell word: single-quoted, each inner quote closed, escaped and
# reopened ('\'') so no path can end the word early
def sh_quote(+path: String) -> String:
  String.append("'", String.append(
    String.join(String.split(path, Char.from_u32(39)), "'\\''"), "'"))

