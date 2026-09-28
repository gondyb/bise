# hub/inv.bend — the invariants the laws state about the hub's state
# (LAWS.bend). Definitions only: the proofs are in PROOF.bend.
import Base
import ./model.bend as M
import ./cx.bend as X
import ./core.bend as H

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

# the card ids, in the order of the state
def card_ids(cs: List<&2, M.Card>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      M.Card.id(c) <> card_ids(t)

# law cards_unique: no two open cards share an id, every id is below the
# next free one
def UniqueCards(+s: M.St) -> Data:
  X.Both<Distinct(card_ids(M.St.cards(s))), Below(card_ids(M.St.cards(s)), M.St.next_card(s))>

def stuck.agent(m: Maybe<&2, M.Agent>, +s: M.St, +n: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.and(H.is_active(M.Agent.life(a)), Bool.not(List.is_empty(&2, M.Msg, M.queued_for(s, n))))

# law L4: agent `n` is idle, active, and has messages queued (a message
# stuck: nothing would ever deliver it)
def stuck(+s: M.St, +rts: List<&2, X.Rt>, +n: String) -> Bool:
  Bool.and(H.is_idle(X.Rt.run(H.rt_find(rts, n))), stuck.agent(M.get_agent(s, n), s, n))
