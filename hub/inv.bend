# hub/inv.bend — the invariants the laws state about the hub's state
# (LAWS.bend). Definitions only: the proofs are in PROOF.bend.
import Base
import ./model.bend as M
import ./cx.bend as X

# the message ids, in the order of the state
def ids(xs: List<&2, M.MRec>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      M.Msg.id(M.MRec.msg(r)) <> ids(t)

# no id appears twice
def Distinct(ns: List<&2, Nat>) -> Data:
  match ns:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{M.nat_in(t, x) == False{} : Bool}, Distinct(t)>

# every id is below `n`
def Below(ns: List<&2, Nat>, +n: Nat) -> Data:
  match ns:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{Nat.is_lt(x, n) == True{} : Bool}, Below(t, n)>

# law msgs_unique: each message is ONE record (its text and its single
# delivery state, RFC 0003 §6.1): no two records share an id, and every
# id is below the next free one (so a new message can never take one)
def Unique(+s: M.St) -> Data:
  X.Both<Distinct(ids(M.St.msgs(s))), Below(ids(M.St.msgs(s)), M.St.next_msg(s))>

# `a` is the start of `b`
def IsPrefix(a: List<&2, Nat>, b: List<&2, Nat>) -> Data:
  match a b:
    case Nil{} _:
      Unit
    case Con{+x, +t} Nil{}:
      Empty
    case Con{+x, +t} Con{+y, +u}:
      X.Both<{x == y : Nat}, IsPrefix(t, u)>

# law msgs_kept: the messages of a state, in order, start the messages
# of any later state: a message once sent is never removed
def Kept(+s1: M.St, +s2: M.St) -> Data:
  IsPrefix(ids(M.St.msgs(s1)), ids(M.St.msgs(s2)))
