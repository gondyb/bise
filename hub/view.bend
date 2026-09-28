# hub/view.bend — the state the Rust views read (board, snapshot,
# contexts, prompts), as JSON: sb-core is the only source of truth, the
# daemon stores this as is.
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./codec.bend as C

import ./cx.bend as X

import ./core.bend as H

def jdecl(d: Maybe<&2, M.Decl>, +note: String) -> J.Json:
  match d:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.obj([J.kv("status", J.JStr{C.decl_str(x)}), J.kv("note", J.JStr{note})])

def agent_json(+a: M.Agent, +x: X.Rt) -> J.Json:
  J.obj([J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("dir", J.JStr{M.Agent.dir(a)}),
    J.kv("is_main", J.JBool{M.Agent.is_main(a)}), J.kv("parent", C.jmaybe_str(M.Agent.parent(a))),
    J.kv("brief", M.Agent.brief(a)), J.kv("created_ms", J.JNum{M.Agent.created(a)}),
    J.kv("ws", M.Agent.ws(a)), J.kv("lifecycle", J.JStr{C.life_str(M.Agent.life(a))}),
    J.kv("failure", C.jmaybe_str(M.Agent.failure(a))),
    J.kv("declared", jdecl(M.Agent.declared(a), M.Agent.note(a))),
    J.kv("last_report", M.Agent.report(a)),
    J.kv("aliases", J.JArr{H.jstrs(M.Agent.aliases(a))}), J.kv("files", J.JArr{H.jstrs(M.Agent.files(a))}),
    J.kv("snapshot_ref", C.jmaybe_str(M.Agent.snap(a))),
    J.kv("run", J.JStr{H.run_str(X.Rt.run(x))}), J.kv("waiting", J.JBool{X.Rt.waiting(x)}),
    J.kv("turn_ms", H.jturn(X.Rt.turn(x)))])

def agents_json(xs: List<&2, M.Agent>, +rts: List<&2, X.Rt>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      agent_json(a, H.rt_find(rts, M.Agent.name(a))) <> agents_json(t, rts)

def rec_json(+r: M.MRec) -> J.Json:
  J.obj([J.kv("msg", C.msg_json(M.MRec.msg(r))), J.kv("state", J.obj(C.mst_kvs(M.MRec.st(r)))),
    J.kv("settled", J.JBool{M.MRec.settled(r)})])

# a match, not Bool.pick: pick would encode every message, kept or not
def recs_json.keep(keep: Bool, +r: M.MRec, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      rec_json(r) <> rest
    case False{}:
      rest

def recs_json(xs: List<&2, M.MRec>, +all: Bool, +ids: List<&2, Nat>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      recs_json.keep(all || M.nat_in(ids, M.Msg.id(M.MRec.msg(r))), r, recs_json(t, all, ids))

def cards_json(xs: List<&2, M.Card>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      C.card_json(c) <> cards_json(t)

def opt_list(m: Maybe<&2, Nat>) -> List<&2, Nat>:
  match m:
    case None{}:
      Nil{}
    case Some{+r}:
      [r]

# the message ids an event touches (a reply settles its question)
def ev_ids(e: M.Ev) -> List<&2, Nat>:
  match e:
    case M.EMsgSent{+m}:
      M.Msg.id(m) <> opt_list(M.Msg.reply_to(m))
    case M.EMsgState{+id, _}:
      [id]
    case M.ESettled{+id}:
      [id]
    case _:
      Nil{}

def touched(evs: List<&2, M.Ev>) -> List<&2, Nat>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, Nat, ev_ids(e), touched(t))

# the view: every agent, card and note; the messages in `ids` (or all)
def view(+s: M.St, +rts: List<&2, X.Rt>, +all: Bool, +ids: List<&2, Nat>) -> J.Json:
  J.obj([J.kv("agents", J.JArr{agents_json(M.St.agents(s), rts)}),
    J.kv("order", J.JArr{H.jstrs(M.St.order(s))}),
    J.kv("msgs", J.JArr{recs_json(M.St.msgs(s), all, ids)}),
    J.kv("all_msgs", J.JBool{all}),
    J.kv("cards", J.JArr{cards_json(M.St.cards(s))}),
    J.kv("notes", J.JArr{H.jstrs(M.St.notes(s))}),
    J.kv("next_msg", J.JNum{M.St.next_msg(s)}), J.kv("next_card", J.JNum{M.St.next_card(s)})])
