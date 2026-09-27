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

def find.put(hit: Bool, here: Maybe<&2, Nat>, rest: Maybe<&2, Nat>)
    -> Maybe<&2, Nat>:
  match hit:
    case True{}:
      here
    case False{}:
      rest

# the index of the first occurrence of `pat` in `s`, counting from `i`
def find(s: String, +pat: String, +i: Nat) -> Maybe<&2, Nat>:
  match s:
    case SNil{}:
      None{}
    case SCon{_, +t}:
      find.put(String.starts_with(s, pat), Some{i},
        find(t, pat, Nat.add(i, 1n)))
