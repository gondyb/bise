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

# the recipient of a message (null: unknown message)
def waits_to(m: Maybe<&2, M.Msg>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.JStr{M.Msg.to(x)}

# who `n` waits on: the recipient of the message its first `sb wait` (or
# `sb ask`) waits for the reply to; null when it waits on no one
def waits_on(+ws: List<&2, X.Waiter>, +s: M.St, +n: String) -> J.Json:
  match ws:
    case Nil{}:
      J.JNull{}
    case Con{+w, +t}:
      Bool.pick(J.Json, String.eq(X.Waiter.agent(w), n),
        waits_to(M.msg_of(s, X.Waiter.msg(w))), waits_on(t, s, n))

def agent_json(+a: M.Agent, +x: X.Rt, +wo: J.Json) -> J.Json:
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
    J.kv("waiting_on", wo),
    J.kv("turn_ms", H.jturn(X.Rt.turn(x)))])

def is_archived(l: M.Life) -> Bool:
  match l:
    case M.LArchived{}:
      True{}
    case _:
      False{}

# a match, not Bool.pick: pick would encode every agent, kept or not
def agents_json.keep(keep: Bool, +a: M.Agent, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: M.St, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      agent_json(a, H.rt_find(rts, M.Agent.name(a)), waits_on(ws, s, M.Agent.name(a))) <> rest
    case False{}:
      rest

# every agent (`all`), else the ones not archived and the archived ones
# in `names` (the only ones a step can change, see `changed`)
def agents_json(xs: List<&2, M.Agent>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: M.St, +all: Bool, +names: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      agents_json.keep(all || Bool.not(is_archived(M.Agent.life(a))) || M.str_in(names, M.Agent.name(a)),
        a, rts, ws, s, agents_json(t, rts, ws, s, all, names))

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

# the agents an event changes (a rename: both names)
def ev_names(e: M.Ev) -> List<&2, String>:
  match e:
    case M.ETaskCreated{+name, _, _, _, _}:
      [name]
    case M.ELifecycle{+name, _, _}:
      [name]
    case M.EWorkspace{+name, _}:
      [name]
    case M.ESnapshot{+name, _}:
      [name]
    case M.EDeclared{+name, _, _}:
      [name]
    case M.EReported{+name, _}:
      [name]
    case M.EFileTouched{+name, _}:
      [name]
    case M.ERenamed{+name, +nn}:
      [name, nn]
    case _:
      Nil{}

def names_of(evs: List<&2, M.Ev>) -> List<&2, String>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, String, ev_names(e), names_of(t))

# a runtime at rest: no REPL, no wait, no turn (what rt_find answers
# for an agent it does not know)
def rt_rest(turn: Maybe<&2, Nat>, down: Bool, waiting: Bool) -> Bool:
  match turn:
    case None{}:
      down && Bool.not(waiting)
    case Some{_}:
      False{}

def hot.keep(rest: Bool, +n: String, +acc: List<&2, String>) -> List<&2, String>:
  match rest:
    case True{}:
      acc
    case False{}:
      n <> acc

def hot(rts: List<&2, X.Rt>) -> List<&2, String>:
  match rts:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      hot.keep(rt_rest(X.Rt.turn(x), H.is_down(X.Rt.run(x)), X.Rt.waiting(x)), X.Rt.name(x), hot(t))

def waiter_names(ws: List<&2, X.Waiter>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      X.Waiter.agent(w) <> waiter_names(t)

# the agents whose runtime or wait may change: not at rest, or waiting
def live(+rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>) -> List<&2, String>:
  List.append(&2, String, hot(rts), waiter_names(ws))

# the archived agents a step may change (hub-lag): the ones its events
# name (a durable change is an event, model.bend `apply`) and the ones
# live before or after it (a runtime at rest is one value: a change goes
# through a state that is not). Every other archived agent is as the
# last view left it: the daemon keeps it.
def changed(+evs: List<&2, M.Ev>, +before: List<&2, String>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>) -> List<&2, String>:
  List.append(&2, String, names_of(evs), List.append(&2, String, before, live(rts, ws)))

# the view: every agent not archived, the archived ones in `names` (or
# all), every card and note; the messages in `ids` (or all)
def view(+s: M.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +all: Bool, +ids: List<&2, Nat>, +names: List<&2, String>) -> J.Json:
  J.obj([J.kv("agents", J.JArr{agents_json(M.St.agents(s), rts, ws, s, all, names)}),
    J.kv("all_agents", J.JBool{all}),
    J.kv("order", J.JArr{H.jstrs(M.St.order(s))}),
    J.kv("msgs", J.JArr{recs_json(M.St.msgs(s), all, ids)}),
    J.kv("all_msgs", J.JBool{all}),
    J.kv("cards", J.JArr{cards_json(M.St.cards(s))}),
    J.kv("notes", J.JArr{H.jstrs(M.St.notes(s))}),
    J.kv("next_msg", J.JNum{M.St.next_msg(s)}), J.kv("next_card", J.JNum{M.St.next_card(s)})])
