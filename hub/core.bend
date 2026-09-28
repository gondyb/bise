# hub/core.bend — the hub's decisions (RFC 0001, RFC 0003): a pure step
# from (state, input) to (state, effects). Ported from
# rust/switchboard/src/core.rs; the Rust daemon runs the effects.
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./cx.bend as X

type Snd is Data:
  Snd{from: String, to: String, text: String, expect: Bool,
    reply_to: Maybe<&2, Nat>, plain: Bool, auto: Bool, queued: Bool,
    via: Maybe<&2, String>}

type SendR is Data:
  SOk{m: M.Msg}
  SErr{e: String}

type TaskR is Data:
  TOk{name: String}
  TErr{e: String}
  THalt{}

type DropR is Data:
  DDone{}
  DAsk{q: String}
  DErr{e: String}
  DHalt{}

type NewTask is Data:
  NewTask{parent: String, base: String, brief: J.Json, brief_text: String,
    objective: String, worktree: Bool, with_changes: Bool}

type Hub is Data:
  Hub{d: M.St, r: X.R}

type In is Data:
  IReq{}
  ITick{}
  IIdle{}
  ITurnStarted{}
  ISteerRx{}
  ISteered{}
  IAssistant{}
  ITouch{}
  IReady{}
  IExited{}
  IBoot{}
  ISay{}
  IRoute{}
  INew{}
  IDrop{}
  IConfirmDrop{}
  IRestore{}
  IIsolate{}
  IRename{}
  IAnswer{}
  ICancel{}
  IInterrupt{}
  IPassthrough{}
  IFocus{}
  IClose{}
  IForceRun{}
  INone{}

type CardK is Data:
  KQuestion{}
  KDrop{}
  KPlain{}
  KOther{}

type Cmd is Data:
  CSend{}
  CWait{}
  CAsk{}
  CStatus{}
  CReport{}
  CSpawn{}
  CInterrupt{}
  CStop{}
  CDrop{}
  CCard{}

# the hub after an input: the new state and its effects, or, when a git
# answer is missing, the hub unchanged and the query (the daemon replays
# the input with the answer)
type Out is Data:
  Out{hub: Hub, fx: List<&2, X.Fx>, need: Maybe<&2, J.Json>}

# a sequence of inputs: the hub at the end and every journal event, in
# order (the model of sb-core's loop, for the law run_replays)
type RunR is Data:
  RunR{hub: Hub, journal: List<&2, M.Ev>}

def WAIT_CAP() -> Nat:
  25n

def MAX_CRASHES() -> Nat:
  5n

def ok_body(+kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> J.Json:
  J.obj(M.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs, J.kv("ok", J.JBool{True{}})))

def err_body(+e: String) -> J.Json:
  J.obj([J.kv("ok", J.JBool{False{}}), J.kv("error", J.JStr{e})])

def ns(+n: Nat) -> String:
  Nat.show(n)

def mid(+n: Nat) -> String:
  String.append("m_", Nat.show(n))

def cat(+xs: List<&2, String>) -> String:
  String.concat(xs)

def run_str(r: M.Run) -> String:
  match r:
    case M.RDown{}:
      "down"
    case M.RStarting{}:
      "starting"
    case M.RIdle{}:
      "idle"
    case M.RBusy{}:
      "busy"

def is_busy(r: M.Run) -> Bool:
  match r:
    case M.RBusy{}:
      True{}
    case _:
      False{}

def is_idle(r: M.Run) -> Bool:
  match r:
    case M.RIdle{}:
      True{}
    case _:
      False{}

def is_down(r: M.Run) -> Bool:
  match r:
    case M.RDown{}:
      True{}
    case _:
      False{}

def is_active(l: M.Life) -> Bool:
  match l:
    case M.LActive{}:
      True{}
    case _:
      False{}

# ---- the runtime lists ----

def rt_find(xs: List<&2, X.Rt>, +n: String) -> X.Rt:
  match xs:
    case Nil{}:
      X.Rt{n, M.RDown{}, False{}, None{}}
    case Con{+h, +t}:
      Bool.pick(X.Rt, String.eq(X.Rt.name(h), n), h, rt_find(t, n))

def rt_put(xs: List<&2, X.Rt>, +x: X.Rt) -> List<&2, X.Rt>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      Bool.pick(List<&2, X.Rt>, String.eq(X.Rt.name(h), X.Rt.name(x)), x <> t, h <> rt_put(t, x))

def rt_rename(xs: List<&2, X.Rt>, +old: String, +new: String) -> List<&2, X.Rt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Rt, String.eq(X.Rt.name(h), old), X.Rt.with_name(h, new), h) <> rt_rename(t, old, new)

def any_waiter(xs: List<&2, X.Waiter>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(X.Waiter.agent(h), n) || any_waiter(t, n)

def waiters_of(xs: List<&2, X.Waiter>, +n: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_of(t, n)
      Bool.pick(List<&2, X.Waiter>, String.eq(X.Waiter.agent(h), n), h <> rest, rest)

def waiters_not(xs: List<&2, X.Waiter>, +n: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_not(t, n)
      Bool.pick(List<&2, X.Waiter>, String.eq(X.Waiter.agent(h), n), rest, h <> rest)

def waiters_late(xs: List<&2, X.Waiter>, +now: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_late(t, now)
      Bool.pick(List<&2, X.Waiter>, Nat.is_le(X.Waiter.deadline(h), now), h <> rest, rest)

def waiters_keep(xs: List<&2, X.Waiter>, +now: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_keep(t, now)
      Bool.pick(List<&2, X.Waiter>, Nat.is_le(X.Waiter.deadline(h), now), rest, h <> rest)

def waiters_rename(xs: List<&2, X.Waiter>, +old: String, +new: String) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Waiter, String.eq(X.Waiter.agent(h), old), X.Waiter.with_agent(h, new), h) <> waiters_rename(t, old, new)

# the first waiter a new message releases (RFC 0003 §6.2): the agent's
# wait for this reply, or any wait when the message asks a question
def waiter_hit(+w: X.Waiter, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Bool:
  String.eq(X.Waiter.agent(w), to) && (M.maybe_nat_is(reply_to, X.Waiter.msg(w)) || asks)

def waiter_pick(xs: List<&2, X.Waiter>, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Maybe<&2, X.Waiter>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, X.Waiter>, waiter_hit(h, to, reply_to, asks), Some{h}, waiter_pick(t, to, reply_to, asks))

def waiter_drop(xs: List<&2, X.Waiter>, +tok: Nat) -> List<&2, X.Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, X.Waiter>, Nat.is_eq(X.Waiter.token(h), tok), t, h <> waiter_drop(t, tok))

def steer_get(xs: List<&2, X.Steer>, +n: String) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Nat>, String.eq(X.Steer.agent(h), n), X.Steer.ids(h), steer_get(t, n))

def steer_del(xs: List<&2, X.Steer>, +n: String) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = steer_del(t, n)
      Bool.pick(List<&2, X.Steer>, String.eq(X.Steer.agent(h), n), rest, h <> rest)

def steer_add(+xs: List<&2, X.Steer>, +n: String, +ids: List<&2, Nat>) -> List<&2, X.Steer>:
  M.snoc(X.Steer, steer_del(xs, n), X.Steer{n, List.append(&2, Nat, steer_get(xs, n), ids)})

def steer_rename(xs: List<&2, X.Steer>, +old: String, +new: String) -> List<&2, X.Steer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(X.Steer, String.eq(X.Steer.agent(h), old), X.Steer{new, X.Steer.ids(h)}, h) <> steer_rename(t, old, new)

def cnt_get(xs: List<&2, X.Cnt>, +n: String) -> X.Cnt:
  match xs:
    case Nil{}:
      X.Cnt{n, 0n, 0n}
    case Con{+h, +t}:
      Bool.pick(X.Cnt, String.eq(X.Cnt.agent(h), n), h, cnt_get(t, n))

def cnt_del(xs: List<&2, X.Cnt>, +n: String) -> List<&2, X.Cnt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = cnt_del(t, n)
      Bool.pick(List<&2, X.Cnt>, String.eq(X.Cnt.agent(h), n), rest, h <> rest)

def kv_get(xs: List<&2, X.Kv>, +n: String) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      Bool.pick(String, String.eq(X.Kv.key(h), n), X.Kv.val(h), kv_get(t, n))

def kv_del(xs: List<&2, X.Kv>, +n: String) -> List<&2, X.Kv>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = kv_del(t, n)
      Bool.pick(List<&2, X.Kv>, String.eq(X.Kv.key(h), n), rest, h <> rest)

def kn_get(xs: List<&2, X.Kn>, +n: String) -> Nat:
  match xs:
    case Nil{}:
      0n
    case Con{+h, +t}:
      Bool.pick(Nat, String.eq(X.Kn.key(h), n), X.Kn.n(h), kn_get(t, n))

def kn_del(xs: List<&2, X.Kn>, +n: String) -> List<&2, X.Kn>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = kn_del(t, n)
      Bool.pick(List<&2, X.Kn>, String.eq(X.Kn.key(h), n), rest, h <> rest)

def ids_of(xs: List<&2, M.Msg>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      M.Msg.id(h) <> ids_of(t)

# ---- context shortcuts ----

def st(-o: M.St, +cx: X.Cx<o>) -> M.St:
  X.d(o, cx)

def rr(-o: M.St, +cx: X.Cx<o>) -> X.R:
  X.Cx.r(o, cx)

def set_r(-o: M.St, +cx: X.Cx<o>, +r: X.R) -> X.Cx<o>:
  X.with_r(o, cx, r)

def out(-o: M.St, +cx: X.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  X.out(o, cx, J.obj(kvs))

def line(-o: M.St, +cx: X.Cx<o>, +agent: String, +kind: String, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"line"}), J.kv("agent", J.JStr{agent}), J.kv("kind", J.JStr{kind}), J.kv("text", J.JStr{text})])

def reply(-o: M.St, +cx: X.Cx<o>, +token: Nat, +body: J.Json) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"reply"}), J.kv("token", J.JNum{token}), J.kv("body", body)])

def reply_err(-o: M.St, +cx: X.Cx<o>, +token: Nat, +e: String) -> X.Cx<o>:
  reply(o, cx, token, err_body(e))

def notice(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"notice"}), J.kv("text", J.JStr{text})])

def rt(-o: M.St, +cx: X.Cx<o>, +n: String) -> X.Rt:
  rt_find(X.R.rts(rr(o, cx)), n)

def jturn(m: Maybe<&2, Nat>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+t}:
      J.JNum{t}

# set the runtime state of an agent (the Rust mirror follows)
def set_rt(-o: M.St, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  +cx2 = set_r(o, cx, X.R.with_rts(rr(o, cx), rt_put(X.R.rts(rr(o, cx)), x)))
  out(o, cx2, [J.kv("fx", J.JStr{"rt"}), J.kv("agent", J.JStr{X.Rt.name(x)}),
    J.kv("run", J.JStr{run_str(X.Rt.run(x))}), J.kv("waiting", J.JBool{X.Rt.waiting(x)}),
    J.kv("turn_ms", jturn(X.Rt.turn(x)))])

def set_run(-o: M.St, +cx: X.Cx<o>, +n: String, +run: M.Run) -> X.Cx<o>:
  set_rt(o, cx, X.Rt.with_run(rt(o, cx, n), run))

def set_waiting(-o: M.St, +cx: X.Cx<o>, +n: String, +w: Bool) -> X.Cx<o>:
  set_rt(o, cx, X.Rt.with_waiting(rt(o, cx, n), w))

def set_turn(-o: M.St, +cx: X.Cx<o>, +n: String, +t: Maybe<&2, Nat>) -> X.Cx<o>:
  set_rt(o, cx, X.Rt.with_turn(rt(o, cx, n), t))

def has_agent(-o: M.St, +cx: X.Cx<o>, +n: String) -> Bool:
  Maybe.is_some(&2, M.Agent, M.get_agent(st(o, cx), n))

# the runtime change applies only to a known agent (like the Rust
# `if let Some(a) = self.st.agents.get_mut(..)`)
def if_agent(-o: M.St, known: Bool, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  match known:
    case True{}:
      set_rt(o, cx, x)
    case False{}:
      cx

def upd_rt(-o: M.St, +cx: X.Cx<o>, +x: X.Rt) -> X.Cx<o>:
  if_agent(o, has_agent(o, cx, X.Rt.name(x)), cx, x)

def dirty(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  X.dirty(o, cx)

def now(-o: M.St, +cx: X.Cx<o>) -> Nat:
  X.now(o, cx)

# every event keeps the journal's invariants: a caller that is not sending
# a message proves them with `Unit{}, Unit{}`
def emit(-o: M.St, +cx: X.Cx<o>, +e: M.Ev, k: X.FreshEv(X.d(o, cx), e), kq: X.QuietEv(e)) -> X.Cx<o>:
  X.emit(o, cx, e, k, kq)

# ---- env queries (replayed answers, see hub/main.bend) ----

def ask.go(-o: M.St, ans: List<&2, J.Json>, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  match ans:
    case Con{+a, +rest}:
      (set_r(o, cx, X.R.with_ans(rr(o, cx), rest)), Some{a})
    case Nil{}:
      (set_r(o, cx, X.R.with_need(rr(o, cx), Maybe.or(&2, J.Json, X.R.need(rr(o, cx)), Some{q}))), None{})

# the answer to an env query, or None: the step halts and the daemon
# replays the input with the answer
def ask(-o: M.St, +cx: X.Cx<o>, +q: J.Json) -> X.Cx<o> & Maybe<&2, J.Json>:
  ask.go(o, X.R.ans(rr(o, cx)), cx, q)

# ---- status and relation ----

def status_of_decl(d: Maybe<&2, M.Decl>) -> String:
  match d:
    case Some{M.DDone{}}:
      "done"
    case Some{M.DBlocked{}}:
      "blocked"
    case _:
      "idle"

def status_of_run(+a: M.Agent, x: X.Rt) -> String:
  match x:
    case X.Rt{_, run, +w, _}:
      match run:
        case M.RDown{}:
          "starting"
        case M.RStarting{}:
          "starting"
        case M.RBusy{}:
          Bool.pick(String, w, "waiting", "working")
        case M.RIdle{}:
          status_of_decl(M.Agent.declared(a))

# the status every view shows (model.rs Agent::status)
def status_str(a: M.Agent, +x: X.Rt) -> String:
  match a:
    case M.Agent{+n, +im, +pa, life, +de, +al, +fi, +ws, +di, +br, +cr, +fa, +no, +rp, +sn}:
      match life:
        case M.LArchived{}:
          "archived"
        case M.LStopped{}:
          "stopped"
        case M.LFailed{}:
          "failed"
        case M.LActive{}:
          status_of_run(M.Agent{n, im, pa, M.LActive{}, de, al, fi, ws, di, br, cr, fa, no, rp, sn}, x)

# the relation of the sender to the recipient, as the recipient reads it
# (prompts.rs relation)
def relation(+from: String, +to: String, +fp: Maybe<&2, String>, +tp: Maybe<&2, String>) -> String:
  Bool.pick(String, String.eq(from, M.USER()), "user",
    Bool.pick(String, String.eq(from, M.HUB()), "hub",
      Bool.pick(String, M.maybe_str_is(tp, from) || (String.eq(from, M.MAIN()) && Bool.not(String.eq(to, M.MAIN()))), "parent",
        Bool.pick(String, M.maybe_str_is(fp, to) || String.eq(to, M.MAIN()), "child", "peer"))))

def parent_of(m: Maybe<&2, M.Agent>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      M.Agent.parent(a)

# ---- notes and cards ----

def note_main(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  emit(o, cx, M.EMainNote{text, now(o, cx)}, Unit{}, Unit{})

# opens card number next_card (the caller reads it first when it needs it)
def open_card(-o: M.St, +cx: X.Cx<o>, +kind: String, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>) -> X.Cx<o>:
  +id = M.St.next_card(st(o, cx))
  +cx2 = emit(o, cx, M.ECardOpened{M.Card{id, kind, agent, text, for_msg, now(o, cx)}}, Unit{}, Unit{})
  line(o, cx2, M.MAIN(), "card", cat(["#", ns(id), " ", kind, " @", agent, " : ", text]))

def close_ids(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>, +res: String) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      +cx2 = emit(o, cx, M.ECardClosed{id, res}, Unit{}, Unit{})
      close_ids(o, rest, line(o, cx2, M.MAIN(), "card-closed", cat(["#", ns(id), " ", res])), res)

# the cards of `agent` whose kind is in `kinds` (Nil: every kind)
def card_ids(cs: List<&2, M.Card>, +agent: String, +kinds: List<&2, String>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      +rest = card_ids(t, agent, kinds)
      +hit = String.eq(M.Card.agent(c), agent) && (List.is_empty(&2, String, kinds) || M.str_in(kinds, M.Card.kind(c)))
      Bool.pick(List<&2, Nat>, hit, M.Card.id(c) <> rest, rest)

def close_cards(-o: M.St, +cx: X.Cx<o>, +agent: String, +kinds: List<&2, String>, +res: String) -> X.Cx<o>:
  close_ids(o, card_ids(M.St.cards(st(o, cx)), agent, kinds), cx, res)

# ---- REPL lifecycle ----

def spawn.if(-o: M.St, down: Bool, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  match down:
    case True{}:
      +cx2 = set_run(o, cx, n, M.RStarting{})
      dirty(o, out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{n}),
        J.kv("resume", J.JBool{resume}), J.kv("crash_note", J.JNull{})]))
    case False{}:
      cx

def spawn(-o: M.St, +cx: X.Cx<o>, +n: String, +resume: Bool) -> X.Cx<o>:
  spawn.if(o, has_agent(o, cx, n) && is_down(X.Rt.run(rt(o, cx, n))), cx, n, resume)

# ---- waiters ----

def deliver_to_waiter(-o: M.St, +cx: X.Cx<o>, +w: X.Waiter, +m: M.Msg, +kind: String) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.EMsgState{M.Msg.id(m), M.Delivered{}}, Unit{}, Unit{})
  +cx3 = reply(o, cx2, X.Waiter.token(w), ok_body([
    J.kv("type", J.JStr{kind}), J.kv("from", J.JStr{M.Msg.from(m)}),
    J.kv("message_id", J.JStr{mid(M.Msg.id(m))}),
    J.kv("thread", J.JStr{String.append("t_", ns(M.Msg.thread(m)))}),
    J.kv("expects_reply", J.JBool{M.Msg.expect(m)}), J.kv("auto", J.JBool{M.Msg.auto(m)}),
    J.kv("message", J.JStr{M.Msg.text(m)})]))
  +cx4 = line(o, cx3, X.Waiter.agent(w), "msg-in", cat([M.Msg.from(m), " ", mid(M.Msg.id(m)), " : ", M.Msg.text(m)]))
  +still = any_waiter(X.R.waiters(rr(o, cx4)), X.Waiter.agent(w))
  +x = X.Rt.with_waiting(rt(o, cx4, X.Waiter.agent(w)), still)
  dirty(o, upd_rt(o, cx4, x))

def reply_all(-o: M.St, ws: List<&2, X.Waiter>, +cx: X.Cx<o>, +why: String) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      reply_all(o, rest, reply_err(o, cx, X.Waiter.token(w), why), why)

def fail_waiters(-o: M.St, +cx: X.Cx<o>, +agent: String, +why: String) -> X.Cx<o>:
  +ws = X.R.waiters(rr(o, cx))
  +cx2 = set_r(o, cx, X.R.with_waiters(rr(o, cx), waiters_not(ws, agent)))
  +cx3 = reply_all(o, waiters_of(ws, agent), cx2, why)
  upd_rt(o, cx3, X.Rt.with_waiting(rt(o, cx3, agent), False{}))

# ---- delivery (RFC 0003 §6.1, RFC 0001 §10.1) ----

def not_queued_mode(xs: List<&2, M.Msg>) -> List<&2, M.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = not_queued_mode(t)
      Bool.pick(List<&2, M.Msg>, M.Msg.queued(h), rest, h <> rest)

def any_from_user(xs: List<&2, M.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(M.Msg.from(h), M.USER()) || any_from_user(t)

def deliver_each(-o: M.St, batch: List<&2, M.Msg>, +cx: X.Cx<o>, +name: String, +acc: List<&2, J.Json>) -> X.Cx<o> & List<&2, J.Json>:
  match batch:
    case Nil{}:
      (cx, acc)
    case Con{+m, +rest}:
      +s = st(o, cx)
      +rel = relation(M.Msg.from(m), M.Msg.to(m), parent_of(M.get_agent(s, M.Msg.from(m))), parent_of(M.get_agent(s, M.Msg.to(m))))
      +cx2 = emit(o, cx, M.EMsgState{M.Msg.id(m), M.Delivered{}}, Unit{}, Unit{})
      +cx3 = Bool.pick(X.Cx<o>, M.Msg.plain(m) || Maybe.is_some(&2, String, M.Msg.via(m)),
        line(o, cx2, name, "you", M.Msg.text(m)),
        line(o, cx2, name, "msg-in", cat([M.Msg.from(m), " ", mid(M.Msg.id(m)), " : ", M.Msg.text(m)])))
      deliver_each(o, rest, cx3, name, M.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{M.Msg.id(m)}), J.kv("rel", J.JStr{rel})])))

def pump.flush(-o: M.St, flush: Bool, +cx: X.Cx<o>) -> X.Cx<o>:
  match flush:
    case True{}:
      emit(o, cx, M.ENotesFlushed{}, Unit{}, Unit{})
    case False{}:
      cx

def pump.send.go(-o: M.St, busy: Bool, +cx: X.Cx<o>, +name: String, +ids: List<&2, Nat>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  match busy:
    case True{}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_steered(r, steer_add(X.R.steered(r), name, ids)))
      dirty(o, out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"steer"}) <> kvs))
    case False{}:
      # the turn starts now: nothing else may `say` meanwhile
      +x = rt(o, cx, name)
      +cx2 = upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(x, M.RBusy{}), Some{now(o, cx)}))
      dirty(o, out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs))

def pump.send(-o: M.St, busy: Bool, p: X.Cx<o> & List<&2, J.Json>, +name: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> X.Cx<o>:
  match p:
    case (+cx, +msgs):
      pump.send.go(o, busy, cx, name, ids, M.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs0, J.kv("msgs", J.JArr{msgs})))

def jstrs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JStr{h} <> jstrs(t)

def pump.batch(-o: M.St, batch: List<&2, M.Msg>, +cx: X.Cx<o>, +name: String, +busy: Bool, +is_main: Bool) -> X.Cx<o>:
  match batch:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +b = {h <> t : List<&2, M.Msg>}
      +notes = Bool.pick(List<&2, String>, is_main, M.St.notes(st(o, cx)), Nil{})
      +cx2 = pump.flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)
      +status = is_main && any_from_user(b)
      pump.send(o, busy, deliver_each(o, b, cx2, name, Nil{}), name, ids_of(b), [
        J.kv("agent", J.JStr{name}),
        J.kv("notes", J.JArr{jstrs(notes)}),
        J.kv("status", J.JBool{status})])

def pump.run(-o: M.St, run: M.Run, +cx: X.Cx<o>, +name: String, +queued: List<&2, M.Msg>, +is_main: Bool) -> X.Cx<o>:
  match run:
    case M.RDown{}:
      cx
    case M.RStarting{}:
      cx
    case M.RBusy{}:
      # a queued-mode message waits for the end of the turn
      pump.batch(o, not_queued_mode(queued), cx, name, True{}, is_main)
    case M.RIdle{}:
      pump.batch(o, queued, cx, name, False{}, is_main)

def pump.active(-o: M.St, active: Bool, +cx: X.Cx<o>, +name: String, +is_main: Bool) -> X.Cx<o>:
  match active:
    case True{}:
      pump.run(o, X.Rt.run(rt(o, cx, name)), cx, name, M.queued_for(st(o, cx), name), is_main)
    case False{}:
      cx

def pump.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      pump.active(o, is_active(M.Agent.life(a)), cx, name, M.Agent.is_main(a))

# deliver what is queued for `name`, when its state allows it
def pump(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o>:
  pump.agent(o, M.get_agent(st(o, cx), name), cx, name)

# ---- messages (RFC 0003) ----

def serr(-o: M.St, +cx: X.Cx<o>, +e: String) -> X.Cx<o> & SendR:
  (cx, SErr{e})

def send.waiter(-o: M.St, mw: Maybe<&2, X.Waiter>, +cx: X.Cx<o>, +m: M.Msg) -> X.Cx<o> & SendR:
  match mw:
    case Some{+w}:
      # a waiting recipient gets it as the result of its `sb wait`
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_waiters(r, waiter_drop(X.R.waiters(r), X.Waiter.token(w))))
      +kind = Bool.pick(String, M.maybe_nat_is(M.Msg.reply_to(m), X.Waiter.msg(w)), "reply", "incoming_request")
      (deliver_to_waiter(o, cx2, w, m, kind), SOk{m})
    case None{}:
      (pump(o, cx, M.Msg.to(m)), SOk{m})

def question_for(cs: List<&2, M.Card>, +r: Nat) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      +rest = question_for(t, r)
      Bool.pick(List<&2, Nat>, String.eq(M.Card.kind(c), "question") && M.maybe_nat_is(M.Card.for_msg(c), r), M.Card.id(c) <> rest, rest)

def send.cards(-o: M.St, r: Maybe<&2, Nat>, +cx: X.Cx<o>, +from: String) -> X.Cx<o>:
  match r:
    case None{}:
      cx
    case Some{+id}:
      close_ids(o, question_for(M.St.cards(st(o, cx)), id), cx, String.append("répondue via @", from))

def quiet_ok(a: Bool, e: Bool) -> {Bool.and(a, Bool.and(e, Bool.not(a))) == False{} : Bool}:
  match a e:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} _:
      {==}

def send.emit(-o: M.St, +cx: X.Cx<o>, +s: Snd, +thread: Nat) -> X.Cx<o> & SendR:
  match s:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via}:
      # an automatic reply never asks (law auto_never_asks); the id is the
      # next free one (law msg_ids_fresh)
      +m = {M.Msg{M.St.next_msg(st(o, cx)), thread, from, to, reply_to, Bool.and(expect, Bool.not(auto)), auto, text, now(o, cx), plain, queued, via} : M.Msg}
      +cx1 = emit(o, cx, M.EMsgSent{m}, {==}, quiet_ok(auto, expect))
      # a reply to the message a question card stands for answers the
      # card; the user's own answers close their card in answer_card
      +cx2 = send.cards(o, Bool.pick(Maybe<&2, Nat>, String.eq(from, M.USER()), None{}, reply_to), cx1, from)
      +w = Bool.pick(Maybe<&2, X.Waiter>, queued, None{},
        waiter_pick(X.R.waiters(rr(o, cx2)), to, reply_to, expect && Bool.not(plain) && Bool.not(String.eq(from, M.USER()))))
      send.waiter(o, w, cx2, m)

def send.orig(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +s: Snd, +r: Nat) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("reply_to inconnu : ", mid(r)))
    case Some{+orig}:
      send.emit(o, cx, s, M.Msg.thread(orig))

def send.thread(-o: M.St, reply_to: Maybe<&2, Nat>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match reply_to:
    case None{}:
      send.emit(o, cx, s, M.St.next_msg(st(o, cx)))
    case Some{+r}:
      send.orig(o, M.msg_of(st(o, cx), r), cx, s, r)

def Snd.to(x: Snd) -> String:
  match x:
    case Snd{_, +to, _, _, _, _, _, _, _}:
      to

def Snd.reply_to(x: Snd) -> Maybe<&2, Nat>:
  match x:
    case Snd{_, _, _, _, +reply_to, _, _, _, _}:
      reply_to

def send.revive(-o: M.St, ok: Bool, +cx: X.Cx<o>, +s: Snd, +why: String) -> X.Cx<o> & SendR:
  match ok:
    case False{}:
      serr(o, cx, why)
    case True{}:
      +to = Snd.to(s)
      +cx2 = emit(o, cx, M.ELifecycle{to, M.LActive{}, None{}}, Unit{}, Unit{})
      +r = rr(o, cx2)
      +cx3 = set_r(o, cx2, X.R.with_crashes(r, kn_del(X.R.crashes(r), to)))
      send.thread(o, Snd.reply_to(s), spawn(o, cx3, to, True{}), s)

def dropped_wt(+a: M.Agent) -> Bool:
  M.ws_worktree(M.Agent.ws(a)) && M.ws_dropped(M.Agent.ws(a))

def unavailable(-o: M.St, +cx: X.Cx<o>, +s: Snd, +a: M.Agent) -> String:
  cat(["recipient_unavailable: @", Snd.to(s), " est ", status_str(a, rt(o, cx, Snd.to(s))),
    Bool.pick(String, dropped_wt(a), " (son worktree a été supprimé : /restore)", "")])

def create.sent(-o: M.St, p: X.Cx<o> & SendR, +name: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SOk{_}):
      (cx, TOk{name})
    case (+cx, SErr{+e}):
      (cx, TErr{e})

def NewTask.parent(x: NewTask) -> String:
  match x:
    case NewTask{+parent, _, _, _, _, _, _}:
      parent

def NewTask.objective(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, _, +objective, _, _}:
      objective

def NewTask.brief(x: NewTask) -> J.Json:
  match x:
    case NewTask{_, _, +brief, _, _, _, _}:
      brief

def NewTask.brief_text(x: NewTask) -> String:
  match x:
    case NewTask{_, _, _, +brief_text, _, _, _}:
      brief_text

def branch_label(b: J.Json) -> String:
  match b:
    case J.JStr{+s}:
      cat([" (worktree ", s, ")"])
    case _:
      ""

def Snd.from(x: Snd) -> String:
  match x:
    case Snd{+from, _, _, _, _, _, _, _, _}:
      from

def send.life(-o: M.St, life: M.Life, +cx: X.Cx<o>, +s: Snd, +a: M.Agent) -> X.Cx<o> & SendR:
  match life:
    case M.LActive{}:
      send.thread(o, Snd.reply_to(s), cx, s)
    case M.LFailed{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) || M.maybe_str_is(M.Agent.parent(a), Snd.from(s)), cx, s, unavailable(o, cx, s, a))
    case M.LStopped{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))
    case M.LArchived{}:
      send.revive(o, String.eq(Snd.from(s), M.USER()) && Bool.not(dropped_wt(a)), cx, s, unavailable(o, cx, s, a))

def send.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: aucun agent nommé ", Snd.to(s)))
    case Some{+a}:
      send.life(o, M.Agent.life(a), cx, s, a)

def send.checked(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match pre:
    case Some{+e}:
      serr(o, cx, e)
    case None{}:
      send.agent(o, M.get_agent(st(o, cx), Snd.to(s)), cx, s)

def Snd.text(x: Snd) -> String:
  match x:
    case Snd{_, _, +text, _, _, _, _, _, _}:
      text

def Snd.with_to(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, _, +text, +expect, +reply_to, +plain, +auto, +queued, +via}:
      Snd{from, v, text, expect, reply_to, plain, auto, queued, via}

def send.known(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match m:
    case None{}:
      serr(o, cx, String.append("recipient_unknown: aucun agent nommé ", Snd.to(s)))
    case Some{+to}:
      +pre = Bool.pick(Maybe<&2, String>, String.eq(to, Snd.from(s)), Some{"invalid_recipient: un agent ne s'écrit pas à lui-même"},
        Bool.pick(Maybe<&2, String>, M.trim_is_empty(Snd.text(s)), Some{"message vide"}, None{}))
      send.checked(o, pre, cx, Snd.with_to(s, to))

def send.user(-o: M.St, user: Bool, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  match user:
    case True{}:
      serr(o, cx, "invalid_recipient: pour parler à l'utilisateur, passe par main (sb card)")
    case False{}:
      send.known(o, M.resolve(st(o, cx), Snd.to(s)), cx, s)

# send a message; answers it, or why it was refused
def send(-o: M.St, +cx: X.Cx<o>, +s: Snd) -> X.Cx<o> & SendR:
  send.user(o, String.eq(Snd.to(s), M.USER()), cx, s)

def msg(+from: String, +to: String, +text: String, +expect: Bool, +reply_to: Maybe<&2, Nat>, +plain: Bool, +auto: Bool) -> Snd:
  Snd{from, to, text, expect, reply_to, plain, auto, False{}, None{}}

def create.go(-o: M.St, +cx: X.Cx<o>, +t: NewTask, +name: String, +ws: J.Json) -> X.Cx<o> & TaskR:
  +parent = NewTask.parent(t)
  +label = branch_label(M.ws_branch(ws))
  +obj = M.one_line(NewTask.objective(t))
  +cx2 = emit(o, cx, M.ETaskCreated{name, parent, NewTask.brief(t), ws, now(o, cx)}, Unit{}, Unit{})
  +who = Bool.pick(String, String.eq(parent, M.USER()), "toi", parent)
  +cx3 = line(o, cx2, M.MAIN(), "spawn", cat([who, " → nouvelle tâche @", name, label, " : ", obj]))
  +cx4 = Bool.pick(X.Cx<o>, String.eq(parent, M.USER()), note_main(o, cx3, cat(["the user created the task @", name, label, ": ", obj])), cx3)
  +cx5 = spawn(o, cx4, name, False{})
  create.sent(o, send(o, cx5, msg(parent, name, cat(["# Task `", name, "`\n\n", NewTask.brief_text(t)]), String.eq(parent, M.MAIN()), None{}, String.eq(parent, M.USER()), False{})), name)

def ans_ok.go(m: Maybe<&2, J.Json>, +j: J.Json) -> Result<&2, &2, String, J.Json>:
  match m:
    case Some{+v}:
      Done{v}
    case None{}:
      Fail{J.get_str_or(j, "err", "erreur git")}

def ans_ok(+j: J.Json) -> Result<&2, &2, String, J.Json>:
  ans_ok.go(J.get(j, "ok"), j)

def create.ws.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      create.go(o, cx, t, name, ws)

def create.ws(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+a}):
      create.ws.res(o, ans_ok(a), cx, t, name)

def NewTask.with_changes(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, _, +with_changes}:
      with_changes

def create.git(-o: M.St, git: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match git:
    case False{}:
      (cx, TErr{"le workspace n'est pas un dépôt git : pas de worktree possible"})
    case True{}:
      create.ws(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_create"}), J.kv("name", J.JStr{name}),
        J.kv("with_changes", J.JBool{NewTask.with_changes(t)})])), t, name)

def shared_ws(+path: String) -> J.Json:
  J.obj([J.kv("mode", J.JStr{"shared"}), J.kv("path", J.JStr{path}), J.kv("branch", J.JNull{}),
    J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}})])

def create.wt(-o: M.St, wt: Bool, +cx: X.Cx<o>, +t: NewTask, +name: String) -> X.Cx<o> & TaskR:
  match wt:
    case False{}:
      create.go(o, cx, t, name, shared_ws(X.R.workspace(rr(o, cx))))
    case True{}:
      create.git(o, X.R.git(rr(o, cx)), cx, t, name)

def NewTask.worktree(x: NewTask) -> Bool:
  match x:
    case NewTask{_, _, _, _, _, +worktree, _}:
      worktree

def NewTask.base(x: NewTask) -> String:
  match x:
    case NewTask{_, +base, _, _, _, _, _}:
      base

def strip_dash.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+c, +t}:
      Bool.pick(String, Char.is_eq(c, '-'), strip_dash.go(t), SCon{c, t})

def strip_dash(+s: String) -> String:
  String.reverse(strip_dash.go(String.reverse(s)))

def candidate(+base: String, +i: Nat) -> String:
  +suffix = String.append("-", ns(i))
  String.append(strip_dash(String.take(base, Nat.sub(24n, String.length(suffix)))), suffix)

def unique_name.go(fuel: Nat, +s: M.St, +base: String, +i: Nat) -> String:
  match fuel:
    case 0n:
      candidate(base, i)
    case 1n+f:
      Bool.pick(String, M.name_taken(s, candidate(base, i)), unique_name.go(f, s, base, 1n+i), candidate(base, i))

def unique_name(+s: M.St, +base: String) -> String:
  Bool.pick(String, M.name_taken(s, base), unique_name.go(1000n, s, base, 2n), base)

# RFC 0001 §7.1 and RFC 0002 §3-4 (the name was validated by the daemon)
def create_task(-o: M.St, +cx: X.Cx<o>, +t: NewTask) -> X.Cx<o> & TaskR:
  create.wt(o, NewTask.worktree(t), cx, t, unique_name(st(o, cx), NewTask.base(t)))

def reject_ids(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      reject_ids(o, rest, emit(o, cx, M.EMsgState{id, M.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}))

def stop_task(-o: M.St, +cx: X.Cx<o>, +name: String, +life: M.Life, +reason: String) -> X.Cx<o>:
  +cx1 = out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{name})])
  +cx2 = fail_waiters(o, cx1, name, "stopped")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, steer_del(X.R.steered(r), name)))
  +cx4 = upd_rt(o, cx3, X.Rt.with_turn(X.Rt.with_run(rt(o, cx3, name), M.RDown{}), None{}))
  +cx5 = emit(o, cx4, M.ELifecycle{name, life, Bool.pick(Maybe<&2, String>, String.is_empty(reason), None{}, Some{reason})}, Unit{}, Unit{})
  +cx6 = reject_ids(o, ids_of(M.queued_for(st(o, cx5), name)), cx5)
  close_cards(o, cx6, name, Nil{}, "tâche arrêtée")

def plural(+n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(n, 1n), "s", "")

def loss_text(+dirty: Nat, +unpushed: Nat) -> String:
  cat([ns(dirty), " fichier", plural(dirty), " modifié", plural(dirty), " et ", ns(unpushed),
    " commit", plural(unpushed), " non poussé", plural(unpushed), " seront sauvegardés (/restore)"])

def drop.label(r: Result<&2, &2, String, J.Json>) -> String:
  match r:
    case Fail{+e}:
      String.append(" — worktree NON supprimé : ", e)
    case Done{J.JStr{_}}:
      " — travail sauvegardé (/restore)"
    case Done{_}:
      " — worktree supprimé"

def drop.snap(j: J.Json) -> Maybe<&2, String>:
  match j:
    case J.JStr{+s}:
      Some{s}
    case _:
      None{}

def drop.ws(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o>:
  match r:
    case Fail{_}:
      cx
    case Done{+snap}:
      +cx2 = emit(o, cx, M.EWorkspace{M.Agent.name(a), J.set(M.Agent.ws(a), "dropped", J.JBool{True{}})}, Unit{}, Unit{})
      emit(o, cx2, M.ESnapshot{M.Agent.name(a), drop.snap(snap)}, Unit{}, Unit{})

def drop.finish(-o: M.St, +cx: X.Cx<o>, +a: M.Agent, +label: String) -> X.Cx<o> & DropR:
  +cx2 = line(o, cx, M.MAIN(), "info", cat(["@", M.Agent.name(a), " archivée", label]))
  # a client looking at it goes back to main
  (out(o, cx2, [J.kv("fx", J.JStr{"focus_main"}), J.kv("name", J.JStr{M.Agent.name(a)})]), DDone{})

def drop.git(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent) -> X.Cx<o> & DropR:
  match p:
    case (+cx, None{}):
      (cx, DHalt{})
    case (+cx, Some{+ans}):
      drop.finish(o, drop.ws(o, ans_ok(ans), cx, a), a, drop.label(ans_ok(ans)))

def drop.now(-o: M.St, has_wt: Bool, +cx: X.Cx<o>, +a: M.Agent, +loss: J.Json) -> X.Cx<o> & DropR:
  match has_wt:
    case False{}:
      drop.finish(o, stop_task(o, cx, M.Agent.name(a), M.LArchived{}, "drop"), a, "")
    case True{}:
      +cx2 = stop_task(o, cx, M.Agent.name(a), M.LArchived{}, "drop")
      drop.git(o, ask(o, cx2, J.obj([J.kv("q", J.JStr{"worktree_drop"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a)),
        J.kv("dirty", J.JNum{J.get_num_or(loss, "dirty", 0n)}), J.kv("unpushed", J.JNum{J.get_num_or(loss, "unpushed", 0n)})])), a)

def drop.decide(-o: M.St, confirm: Bool, +cx: X.Cx<o>, +a: M.Agent, +has_wt: Bool, +loss: J.Json, +busy: Bool) -> X.Cx<o> & DropR:
  match confirm:
    case True{}:
      +dirty = J.get_num_or(loss, "dirty", 0n)
      +unpushed = J.get_num_or(loss, "unpushed", 0n)
      +parts = List.append(&2, String, Bool.pick(List<&2, String>, busy, ["la tâche est en cours"], Nil{}),
        Bool.pick(List<&2, String>, Nat.is_gt(Nat.add(dirty, unpushed), 0n), [loss_text(dirty, unpushed)], Nil{}))
      (cx, DAsk{cat(["Drop @", M.Agent.name(a), " ? ", String.join(parts, " ; "), ". [y/N]"])})
    case False{}:
      drop.now(o, has_wt, cx, a, loss)

def drop.loss(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent, +force: Bool, +has_wt: Bool) -> X.Cx<o> & DropR:
  match p:
    case (+cx, None{}):
      (cx, DHalt{})
    case (+cx, Some{+loss}):
      +busy = is_busy(X.Rt.run(rt(o, cx, M.Agent.name(a))))
      +any = Nat.is_gt(Nat.add(J.get_num_or(loss, "dirty", 0n), J.get_num_or(loss, "unpushed", 0n)), 0n)
      drop.decide(o, Bool.not(force) && (busy || any), cx, a, has_wt, loss, busy)

def drop.wt(-o: M.St, has_wt: Bool, +cx: X.Cx<o>, +a: M.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match has_wt:
    case True{}:
      drop.loss(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_loss"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a))])), a, force, True{})
    case False{}:
      drop.loss(o, (cx, Some{J.obj([J.kv("dirty", J.JNum{0n}), J.kv("unpushed", J.JNum{0n})])}), a, force, False{})

def drop.checks(-o: M.St, is_main: Bool, life: M.Life, +cx: X.Cx<o>, +a: M.Agent, +force: Bool) -> X.Cx<o> & DropR:
  match is_main:
    case True{}:
      (cx, DErr{"main ne se droppe pas"})
    case False{}:
      match life:
        case M.LArchived{}:
          (cx, DErr{cat(["@", M.Agent.name(a), " est déjà archivée"])})
        case _:
          drop.wt(o, M.ws_worktree(M.Agent.ws(a)) && Bool.not(M.ws_dropped(M.Agent.ws(a))), cx, a, force)

def drop.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  match m:
    case None{}:
      (cx, DErr{String.append("aucune tâche nommée ", name)})
    case Some{+a}:
      drop.checks(o, M.Agent.is_main(a), M.Agent.life(a), cx, a, force)

def agent_of(+s: M.St, m: Maybe<&2, String>) -> Maybe<&2, M.Agent>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      M.get_agent(s, n)

# RFC 0001 §9.4 + RFC 0002 §5
def drop_task(-o: M.St, +cx: X.Cx<o>, +name: String, +force: Bool) -> X.Cx<o> & DropR:
  drop.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name, force)

def restore.go(-o: M.St, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  +n = M.Agent.name(a)
  +cx2 = emit(o, cx, M.ELifecycle{n, M.LActive{}, None{}}, Unit{}, Unit{})
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_crashes(r, kn_del(X.R.crashes(r), n)))
  +cx4 = spawn(o, cx3, n, True{})
  (line(o, cx4, M.MAIN(), "info", cat(["@", n, " restaurée"])), TOk{n})

def restore.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      +cx2 = emit(o, cx, M.EWorkspace{M.Agent.name(a), ws}, Unit{}, Unit{})
      restore.go(o, emit(o, cx2, M.ESnapshot{M.Agent.name(a), None{}}, Unit{}, Unit{}), a)

def restore.ans(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+ans}):
      restore.res(o, ans_ok(ans), cx, a)

def jopt(m: Maybe<&2, String>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.JStr{x}

def restore.wt(-o: M.St, dropped: Bool, +cx: X.Cx<o>, +a: M.Agent) -> X.Cx<o> & TaskR:
  match dropped:
    case True{}:
      restore.ans(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_restore"}), J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("ws", M.Agent.ws(a)),
        J.kv("snapshot_ref", jopt(M.Agent.snap(a)))])), a)
    case False{}:
      restore.go(o, cx, a)

def restore.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("aucune tâche nommée ", name)})
    case Some{+a}:
      Bool.pick(X.Cx<o> & TaskR, is_active(M.Agent.life(a)), (cx, TErr{cat(["@", M.Agent.name(a), " est active"])}),
        restore.wt(o, M.ws_worktree(M.Agent.ws(a)) && M.ws_dropped(M.Agent.ws(a)), cx, a))

def restore_task(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  restore.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

def isolate.sent(-o: M.St, p: X.Cx<o> & SendR, +n: String, +branch: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, SErr{+e}):
      (cx, TErr{e})
    case (+cx, SOk{_}):
      (line(o, cx, M.MAIN(), "info", cat(["@", n, " travaille maintenant dans le worktree ", branch])), TOk{n})

def isolate.res(-o: M.St, r: Result<&2, &2, String, J.Json>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match r:
    case Fail{+e}:
      (cx, TErr{e})
    case Done{+ws}:
      +path = M.ws_path(ws)
      +branch = J.get_str_or(ws, "branch", "")
      +cx2 = emit(o, cx, M.EWorkspace{n, ws}, Unit{}, Unit{})
      # the REPL takes its new BEND_WORKDIR at the next spawn
      +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
      +cx4 = spawn(o, upd_rt(o, cx3, X.Rt.with_run(rt(o, cx3, n), M.RDown{})), n, True{})
      +text = cat(["[switchboard] Your working directory is now the isolated git worktree `", path, "` (branch `", branch, "`). Work only there from now on."])
      isolate.sent(o, send(o, cx4, msg(M.USER(), n, text, False{}, None{}, True{}, False{})), n, branch)

def isolate.ans(-o: M.St, p: X.Cx<o> & Maybe<&2, J.Json>, +n: String) -> X.Cx<o> & TaskR:
  match p:
    case (+cx, None{}):
      (cx, THalt{})
    case (+cx, Some{+ans}):
      isolate.res(o, ans_ok(ans), cx, n)

def isolate.pre(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +n: String) -> X.Cx<o> & TaskR:
  match pre:
    case Some{+e}:
      (cx, TErr{e})
    case None{}:
      isolate.ans(o, ask(o, cx, J.obj([J.kv("q", J.JStr{"worktree_create"}), J.kv("name", J.JStr{n}), J.kv("with_changes", J.JBool{False{}})])), n)

def isolate.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  match m:
    case None{}:
      (cx, TErr{String.append("aucune tâche nommée ", name)})
    case Some{+a}:
      +n = M.Agent.name(a)
      +pre = Bool.pick(Maybe<&2, String>, M.Agent.is_main(a) || M.ws_worktree(M.Agent.ws(a)), Some{cat(["@", n, " a déjà son propre dossier"])},
        Bool.pick(Maybe<&2, String>, Bool.not(List.is_empty(&2, String, M.Agent.files(a))),
          Some{cat(["@", n, " a déjà modifié des fichiers du workspace (", String.join(List.take(&2, String, M.Agent.files(a), 3n), ", "), ") : impossible de séparer son travail du tien"])},
          Bool.pick(Maybe<&2, String>, is_busy(X.Rt.run(rt(o, cx, n))), Some{cat(["@", n, " est en plein tour : attends la fin"])},
            Bool.pick(Maybe<&2, String>, X.R.git(rr(o, cx)), None{}, Some{"le workspace n'est pas un dépôt git"}))))
      isolate.pre(o, pre, cx, n)

def isolate_task(-o: M.St, +cx: X.Cx<o>, +name: String) -> X.Cx<o> & TaskR:
  isolate.agent(o, agent_of(st(o, cx), M.resolve(st(o, cx), name)), cx, name)

# ---- the user ----

def user_sent(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  out(o, cx, [J.kv("fx", J.JStr{"user_sent"}), J.kv("text", J.JStr{text})])

def close_card(-o: M.St, +cx: X.Cx<o>, +id: Nat, +res: String) -> X.Cx<o>:
  +cx2 = emit(o, cx, M.ECardClosed{id, res}, Unit{}, Unit{})
  line(o, cx2, M.MAIN(), "card-closed", cat(["#", ns(id), " ", res]))

def answer.q.sent(-o: M.St, p: X.Cx<o> & SendR, +c: M.Card, +asker: String, +text: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      +id = M.Card.id(c)
      +cx2 = note_main(o, cx, cat(["the user answered card #", ns(id), " (@", asker, " asked: \"", M.one_line(M.Card.text(c)), "\"): \"", M.one_line(text), "\""]))
      +cx3 = line(o, cx2, M.MAIN(), "route", cat(["toi → @", asker, " (réponse à la carte #", ns(id), ") : ", text]))
      close_card(o, cx3, id, "répondue")

def answer.q(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match m:
    case Some{+q}:
      answer.q.sent(o, send(o, cx, msg(M.USER(), M.Msg.from(q), text, False{}, Some{M.Msg.id(q)}, False{}, False{})), c, M.Msg.from(q), text)
    case None{}:
      +id = M.Card.id(c)
      +cx2 = note_main(o, cx, cat(["the user answered card #", ns(id), " (\"", M.one_line(M.Card.text(c)), "\"): \"", M.one_line(text), "\""]))
      close_card(o, cx2, id, "répondue")

def for_msg_of(+s: M.St, m: Maybe<&2, Nat>) -> Maybe<&2, M.Msg>:
  match m:
    case None{}:
      None{}
    case Some{+id}:
      M.msg_of(s, id)

def answer.dropped(-o: M.St, p: X.Cx<o> & DropR, +c: M.Card) -> X.Cx<o>:
  match p:
    case (+cx, DErr{+e}):
      close_card(o, notice(o, cx, e), M.Card.id(c), "acceptée")
    case (+cx, _):
      close_card(o, note_main(o, cx, String.append("the user accepted to drop @", M.Card.agent(c))), M.Card.id(c), "acceptée")

def answer.drop(-o: M.St, yes: Bool, +cx: X.Cx<o>, +c: M.Card) -> X.Cx<o>:
  match yes:
    case True{}:
      answer.dropped(o, drop_task(o, cx, M.Card.agent(c), True{}), c)
    case False{}:
      close_card(o, note_main(o, cx, String.append("the user refused to drop @", M.Card.agent(c))), M.Card.id(c), "refusée")

def answer.plain.sent(-o: M.St, p: X.Cx<o> & SendR, +id: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      close_card(o, cx, id, "répondue")

def is_yes(+t: String) -> Bool:
  +s = String.to_lower(String.trim(t))
  String.eq(s, "y") || String.eq(s, "yes") || String.eq(s, "o") || String.eq(s, "oui") || String.eq(s, "ok")

def card_kind(+k: String) -> CardK:
  Bool.pick(CardK, String.eq(k, "question"), KQuestion{},
    Bool.pick(CardK, String.eq(k, "drop"), KDrop{},
      Bool.pick(CardK, String.eq(k, "blocked") || String.eq(k, "failed") || String.eq(k, "restart"), KPlain{}, KOther{})))

def answer.kind(-o: M.St, k: CardK, +cx: X.Cx<o>, +c: M.Card, +text: String) -> X.Cx<o>:
  match k:
    case KQuestion{}:
      answer.q(o, for_msg_of(st(o, cx), M.Card.for_msg(c)), cx, c, text)
    case KDrop{}:
      answer.drop(o, is_yes(text), cx, c)
    case KPlain{}:
      answer.plain.sent(o, send(o, cx, msg(M.USER(), M.Card.agent(c), text, False{}, None{}, True{}, False{})), M.Card.id(c))
    case KOther{}:
      close_card(o, cx, M.Card.id(c), "vue")

def answer.card(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +id: Nat, +text: String) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("aucune carte ouverte #", ns(id)))
    case Some{+c}:
      answer.kind(o, card_kind(M.Card.kind(c)), cx, c, text)

def answer_card(-o: M.St, +cx: X.Cx<o>, +id: Nat, +text: String) -> X.Cx<o>:
  answer.card(o, M.card_of(M.St.cards(st(o, cx)), id), cx, id, text)

def asked_by(m: Maybe<&2, M.Msg>, +to: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+q}:
      String.eq(M.Msg.from(q), to)

# RFC 0001 §7.3: answering a task that waits on the user, from its own
# view, answers its question
def question_card(cs: List<&2, M.Card>, +s: M.St, +to: String) -> Maybe<&2, Nat>:
  match cs:
    case Nil{}:
      None{}
    case Con{+c, +t}:
      Bool.pick(Maybe<&2, Nat>, String.eq(M.Card.kind(c), "question") && asked_by(for_msg_of(s, M.Card.for_msg(c)), to),
        Some{M.Card.id(c)}, question_card(t, s, to))

def says.sent(-o: M.St, p: X.Cx<o> & SendR, +to: String, +text: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{_}):
      Bool.pick(X.Cx<o>, String.eq(to, M.MAIN()), cx, user_sent(o, cx, text))

def says.card(-o: M.St, card: Maybe<&2, Nat>, +cx: X.Cx<o>, +to: String, +text: String) -> X.Cx<o>:
  match card:
    case Some{+id}:
      answer_card(o, user_sent(o, cx, text), id, text)
    case None{}:
      says.sent(o, send(o, cx, msg(M.USER(), to, text, False{}, None{}, True{}, False{})), to, text)

# plain text from the user to the agent in focus
def user_says(-o: M.St, +cx: X.Cx<o>, +to: String, +text: String) -> X.Cx<o>:
  says.card(o, Bool.pick(Maybe<&2, Nat>, String.eq(to, M.MAIN()), None{}, question_card(M.St.cards(st(o, cx)), st(o, cx), to)), cx, to, text)

def close_names(xs: List<&2, String>, +target: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = close_names(t, target)
      +first = String.take(target, 1n)
      Bool.pick(List<&2, String>, Bool.not(String.eq(n, M.MAIN())) && (String.starts_with(n, Bool.pick(String, String.is_empty(first), "?", first)) || String.contains(n, target)),
        String.append("@", n) <> rest, rest)

def route.sent(-o: M.St, p: X.Cx<o> & SendR, +name: String, +text: String, +focus: String) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      notice(o, cx, e)
    case (+cx, SOk{+m}):
      +cx2 = line(o, cx, M.MAIN(), "route", cat(["toi → @", name, " : ", text]))
      +cx3 = Bool.pick(X.Cx<o>, Bool.not(String.eq(focus, M.MAIN())) || Bool.not(String.eq(name, focus)),
        note_main(o, cx2, cat(["the user wrote directly to @", name, ": \"", M.one_line(text), "\""])), cx2)
      out(o, cx3, [J.kv("fx", J.JStr{"routed"}), J.kv("id", J.JNum{M.Msg.id(m)})])

def route.target(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +target: String, +text: String, +focus: String) -> X.Cx<o>:
  match m:
    case None{}:
      +close = close_names(M.St.order(st(o, cx)), target)
      +hint = Bool.pick(String, List.is_empty(&2, String, close), "", cat([" — tu voulais dire ", String.join(close, ", "), " ?"]))
      notice(o, cx, cat(["aucune tâche nommée @", target, hint]))
    case Some{+name}:
      # RFC 0003 §5.1: from another view, the task reads it tagged and
      # its answer comes back to that view
      +elsewhere = Bool.not(String.eq(name, focus))
      route.sent(o, send(o, cx, Snd{M.USER(), name, text, elsewhere, None{}, Bool.not(elsewhere), False{}, False{},
        Bool.pick(Maybe<&2, String>, elsewhere, Some{focus}, None{})}), name, text, focus)

# `@task text`: a direct message, without main
def route(-o: M.St, +cx: X.Cx<o>, +target: String, +text: String, +focus: String) -> X.Cx<o>:
  Bool.pick(X.Cx<o>, String.eq(target, M.MAIN()), user_says(o, cx, M.MAIN(), text),
    route.target(o, M.resolve(st(o, cx), target), cx, target, text, focus))

def user_new.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, _):
      cx

def user_drop.res(-o: M.St, p: X.Cx<o> & DropR, +name: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      note_main(o, cx, String.append("the user dropped the task @", name))
    case (+cx, DAsk{+q}):
      +canon = Maybe.default(&2, String, M.resolve(st(o, cx), name), name)
      out(o, cx, [J.kv("fx", J.JStr{"confirm"}), J.kv("name", J.JStr{canon}), J.kv("text", J.JStr{q})])
    case (+cx, DErr{+e}):
      notice(o, cx, e)
    case (+cx, DHalt{}):
      cx

def user_confirm.res(-o: M.St, p: X.Cx<o> & DropR, +name: String) -> X.Cx<o>:
  match p:
    case (+cx, DErr{+e}):
      notice(o, cx, e)
    case (+cx, _):
      note_main(o, cx, String.append("the user dropped the task @", name))

def user_restore.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, String.append("the user restored the task @", n))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

def user_isolate.res(-o: M.St, p: X.Cx<o> & TaskR) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      note_main(o, cx, cat(["the user moved @", n, " into its own git worktree"]))
    case (+cx, TErr{+e}):
      notice(o, cx, e)
    case (+cx, THalt{}):
      cx

def rename.go(-o: M.St, ok: Bool, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  match ok:
    case False{}:
      notice(o, cx, String.append("nom invalide ou déjà pris : ", new))
    case True{}:
      +cx2 = emit(o, cx, M.ERenamed{old, new}, Unit{}, Unit{})
      +r = rr(o, cx2)
      +cx3 = set_r(o, cx2, X.R.with_rts(X.R.with_waiters(X.R.with_steered(r, steer_rename(X.R.steered(r), old, new)),
        waiters_rename(X.R.waiters(r), old, new)), rt_rename(X.R.rts(r), old, new)))
      +cx4 = out(o, cx3, [J.kv("fx", J.JStr{"renamed"}), J.kv("old", J.JStr{old}), J.kv("new", J.JStr{new})])
      +cx5 = line(o, cx4, M.MAIN(), "info", cat(["@", old, " s'appelle maintenant @", new]))
      note_main(o, cx5, cat(["the user renamed @", old, " to @", new, " (the old name still works)"]))

def rename.old(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +name: String, +new: String, +valid: Bool) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("aucune tâche nommée @", name))
    case Some{+old}:
      Bool.pick(X.Cx<o>, String.eq(old, M.MAIN()), notice(o, cx, String.append("aucune tâche nommée @", name)),
        rename.go(o, valid && Bool.not(M.name_taken(st(o, cx), new)), cx, old, new))

def cancel.to(m: Maybe<&2, M.Msg>) -> String:
  match m:
    case None{}:
      ""
    case Some{+x}:
      M.Msg.to(x)

def cancel.state(-o: M.St, s: Maybe<&2, M.MSt>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match s:
    case Some{M.Queued{_}}:
      +cx2 = emit(o, cx, M.EMsgState{id, M.Cancelled{}}, Unit{}, Unit{})
      +to = cancel.to(M.msg_of(st(o, cx2), id))
      +cx3 = line(o, cx2, M.MAIN(), "info", cat(["routage vers @", to, " annulé"]))
      note_main(o, cx3, cat(["the user cancelled the message ", mid(id), " to @", to, " before delivery"]))
    case _:
      notice(o, cx, "trop tard : le message est déjà livré, envoie une correction")

def cancel(-o: M.St, last: Maybe<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match last:
    case None{}:
      notice(o, cx, "aucun routage à annuler")
    case Some{+id}:
      cancel.state(o, M.state_of(st(o, cx), id), cx, id)

def interrupt.go(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case Some{+n}:
      Bool.pick(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        out(o, cx, [J.kv("fx", J.JStr{"interrupt"}), J.kv("agent", J.JStr{n})]),
        notice(o, cx, "aucun tour en cours à interrompre"))
    case None{}:
      notice(o, cx, "aucun tour en cours à interrompre")

def passthrough(-o: M.St, +cx: X.Cx<o>, +focus: String, +l: String) -> X.Cx<o>:
  Bool.pick(X.Cx<o>, has_agent(o, cx, focus) && is_idle(X.Rt.run(rt(o, cx, focus))),
    out(o, dirty(o, set_run(o, cx, focus, M.RBusy{})), [J.kv("fx", J.JStr{"passthrough"}), J.kv("agent", J.JStr{focus}), J.kv("line", J.JStr{l})]),
    notice(o, cx, "l'agent n'est pas inactif : réessaie à la fin du tour"))

def focus.cards(-o: M.St, +cx: X.Cx<o>, +f: String) -> X.Cx<o>:
  Bool.pick(X.Cx<o>, String.eq(f, M.MAIN()), cx, close_cards(o, cx, f, ["done"], "vue"))

# the daemon computed what main learns about a direct exchange (RFC
# 0001 §7.4); a task's view closes its done cards
def focus(-o: M.St, +cx: X.Cx<o>, +f: String, note: Maybe<&2, String>, +direct: String) -> X.Cx<o>:
  match note:
    case Some{+n}:
      focus.cards(o, line(o, note_main(o, cx, n), M.MAIN(), "direct", direct), f)
    case None{}:
      focus.cards(o, cx, f)

# ---- the agents' `sb` requests (RFC 0003 §4, RFC 0001 §7.2, §7.5) ----

def jnat(+j: J.Json, +k: String) -> Nat:
  J.get_num_or(j, k, 0n)

def jstr(+j: J.Json, +k: String) -> String:
  J.get_str_or(j, k, "")

def jbool(+j: J.Json, +k: String) -> Bool:
  J.get_bool_or(j, k, False{})

def jopt_nat(m: Maybe<&2, J.Json>) -> Maybe<&2, Nat>:
  match m:
    case Some{J.JNum{+n}}:
      Some{n}
    case _:
      None{}

def jopt_str(m: Maybe<&2, J.Json>) -> Maybe<&2, String>:
  match m:
    case Some{J.JStr{+s}}:
      Some{s}
    case _:
      None{}

def jstr_list.one(h: J.Json, +rest: List<&2, String>) -> List<&2, String>:
  match h:
    case J.JStr{+s}:
      s <> rest
    case _:
      rest

def jstr_list(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = jstr_list(t)
      jstr_list.one(h, rest)

def main_only(-o: M.St, +cx: X.Cx<o>, +token: Nat) -> X.Cx<o>:
  reply_err(o, cx, token, "réservé à main (le parent des tâches)")

def req_send.delivered(m: Maybe<&2, M.MSt>) -> Bool:
  match m:
    case Some{M.Delivered{}}:
      True{}
    case _:
      False{}

def req_send.res(-o: M.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      +delivered = req_send.delivered(M.state_of(st(o, cx), M.Msg.id(m)))
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(M.Msg.id(m))}),
        J.kv("thread", J.JStr{String.append("t_", ns(M.Msg.thread(m)))}), J.kv("to", J.JStr{M.Msg.to(m)}),
        J.kv("delivery", J.JStr{Bool.pick(String, delivered, "delivered", "queued")})]))

# the oldest delivered message from `asker` to `me` that still expects
# `me`'s reply
def first_from(xs: List<&2, M.Msg>, +asker: String) -> Maybe<&2, Nat>:
  match xs:
    case Nil{}:
      None{}
    case Con{+m, +t}:
      Bool.pick(Maybe<&2, Nat>, String.eq(M.Msg.from(m), asker), Some{M.Msg.id(m)}, first_from(t, asker))

def open_question.go(m: Maybe<&2, String>, +s: M.St, +me: String) -> Maybe<&2, Nat>:
  match m:
    case None{}:
      None{}
    case Some{+asker}:
      first_from(M.unanswered_for(s, me), asker)

def open_question(+s: M.St, +asker: String, +me: String) -> Maybe<&2, Nat>:
  open_question.go(M.resolve(s, asker), s, me)

def req_send(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  +to = jstr(q, "to")
  +expect = jbool(q, "expect_reply")
  # a plain message to an agent that asked us something answers it
  +reply_to = Maybe.or(&2, Nat, jopt_nat(J.get(q, "reply_to")),
    Bool.pick(Maybe<&2, Nat>, expect, None{}, open_question(st(o, cx), to, from)))
  req_send.res(o, send(o, cx, Snd{from, to, jstr(q, "text"), expect, reply_to, False{}, False{}, jbool(q, "queued"), None{}}), token)

def wait.existing(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +w: X.Waiter) -> X.Cx<o>:
  match m:
    case Some{+r}:
      # the reply may already be there
      deliver_to_waiter(o, cx, w, r, "reply")
    case None{}:
      +r = rr(o, cx)
      +cx2 = set_r(o, cx, X.R.with_waiters(r, M.snoc(X.Waiter, X.R.waiters(r), w)))
      dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), True{})))

def reply_of(xs: List<&2, M.MRec>, +id: Nat, +replier: String) -> Maybe<&2, M.Msg>:
  match xs:
    case Nil{}:
      None{}
    case Con{+r, +t}:
      +m = M.MRec.msg(r)
      Bool.pick(Maybe<&2, M.Msg>,
        M.maybe_nat_is(M.Msg.reply_to(m), id) && (String.eq(M.Msg.from(m), replier) || String.eq(M.Msg.from(m), M.USER())),
        Some{m}, reply_of(t, id, replier))

def wait.mine(-o: M.St, mine: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +orig: M.Msg, +timeout: Nat) -> X.Cx<o>:
  match mine:
    case False{}:
      reply_err(o, cx, token, "on n'attend que la réponse à ses propres messages")
    case True{}:
      +w = {X.Waiter{token, from, M.Msg.id(orig), Nat.add(now(o, cx), Nat.mul(1000n, Nat.min(timeout, WAIT_CAP())))} : X.Waiter}
      wait.existing(o, reply_of(M.St.msgs(st(o, cx)), M.Msg.id(orig), M.Msg.to(orig)), cx, w)

def wait.orig(-o: M.St, m: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+orig}:
      wait.mine(o, String.eq(M.Msg.from(orig), from), cx, token, from, orig, timeout)

def wait(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +id: Nat, +timeout: Nat) -> X.Cx<o>:
  wait.orig(o, M.msg_of(st(o, cx), id), cx, token, from, id, timeout)

def req_ask.res(-o: M.St, p: X.Cx<o> & SendR, +token: Nat, +from: String, +timeout: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      wait(o, cx, token, from, M.Msg.id(m), timeout)

def decl_of(+s: String) -> M.Decl:
  Bool.pick(M.Decl, String.eq(s, "done"), M.DDone{}, Bool.pick(M.Decl, String.eq(s, "blocked"), M.DBlocked{}, M.DWorking{}))

def req_status(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +is_main: Bool, +q: J.Json) -> X.Cx<o>:
  +status = jstr(q, "status")
  +note = jstr(q, "note")
  +cx2 = emit(o, cx, M.EDeclared{from, Some{decl_of(status)}, note}, Unit{}, Unit{})
  +cx3 = Bool.pick(X.Cx<o>, String.eq(status, "blocked") && Bool.not(is_main),
    open_card(o, cx2, "blocked", from, Bool.pick(String, String.is_empty(note), "bloquée", note), None{}), cx2)
  reply(o, cx3, token, ok_body(Nil{}))

def report_lines(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      String.append("\n- ", h) <> report_lines(t)

def report_text(+kind: String, +summary: String, +decisions: List<&2, String>) -> String:
  +head = cat(["[report: ", kind, "] ", String.trim(summary)])
  Bool.pick(String, List.is_empty(&2, String, decisions), head,
    cat([head, "\nDecisions:", String.concat(report_lines(decisions))]))

def req_report.sent(-o: M.St, p: X.Cx<o> & SendR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, SErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, SOk{+m}):
      reply(o, cx, token, ok_body([J.kv("message_id", J.JStr{mid(M.Msg.id(m))})]))

def req_report.kind(-o: M.St, +cx: X.Cx<o>, +from: String, +kind: String, +summary: String) -> X.Cx<o>:
  Bool.pick(X.Cx<o>, String.eq(kind, "done"),
    open_card(o, emit(o, cx, M.EDeclared{from, Some{M.DDone{}}, ""}, Unit{}, Unit{}), "done", from, M.clip(M.one_line(summary), 200n), None{}),
    Bool.pick(X.Cx<o>, String.eq(kind, "blocked"),
      open_card(o, emit(o, cx, M.EDeclared{from, Some{M.DBlocked{}}, summary}, Unit{}, Unit{}), "blocked", from, summary, None{}),
      Bool.pick(X.Cx<o>, String.eq(kind, "failed"),
        open_card(o, emit(o, cx, M.ELifecycle{from, M.LFailed{}, Some{summary}}, Unit{}, Unit{}), "failed", from, summary, None{}),
        cx)))

def req_report.go(-o: M.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case True{}:
      reply_err(o, cx, token, "main ne se rapporte à personne")
    case False{}:
      +kind = jstr(q, "kind")
      +summary = jstr(q, "summary")
      +decisions = jstr_list(Maybe.default(&2, List<&2, J.Json>, J.get_arr(q, "decisions"), Nil{}))
      +cx2 = emit(o, cx, M.EReported{from, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{kind}),
        J.kv("summary", J.JStr{summary}), J.kv("decisions", J.JArr{jstrs(decisions)}), J.kv("auto", J.JBool{False{}})])}, Unit{}, Unit{})
      +cx3 = req_report.kind(o, cx2, from, kind, summary)
      +parent = Maybe.default(&2, String, parent_of(M.get_agent(st(o, cx3), from)), M.MAIN())
      +to = Bool.pick(String, String.eq(parent, M.USER()), M.MAIN(), parent)
      # the report answers the parent's open request, if any: no
      # automatic reply repeats it at the end of the turn
      +reply_to = open_question(st(o, cx3), to, from)
      req_report.sent(o, send(o, cx3, msg(from, to, report_text(kind, summary, decisions), False{}, reply_to, False{}, False{})), token)

def req_spawn.ws(m: Maybe<&2, M.Agent>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+a}:
      M.Agent.ws(a)

def req_spawn.res(-o: M.St, p: X.Cx<o> & TaskR, +token: Nat) -> X.Cx<o>:
  match p:
    case (+cx, TOk{+n}):
      +ws = req_spawn.ws(M.get_agent(st(o, cx), n))
      reply(o, cx, token, ok_body([J.kv("name", J.JStr{n}), J.kv("path", J.JStr{M.ws_path(ws)}), J.kv("branch", M.ws_branch(ws))]))
    case (+cx, TErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, THalt{}):
      cx

def new_task(+parent: String, +q: J.Json) -> NewTask:
  NewTask{parent, jstr(q, "base"), Maybe.default(&2, J.Json, J.get(q, "brief"), J.JNull{}), jstr(q, "brief_text"),
    jstr(q, "objective"), jbool(q, "worktree"), jbool(q, "with_changes")}

def req_spawn.pre(-o: M.St, pre: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match pre:
    case Some{+e}:
      reply_err(o, cx, token, e)
    case None{}:
      req_spawn.res(o, create_task(o, cx, new_task(M.MAIN(), q)), token)

def req_interrupt(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("aucune tâche nommée ", agent))
    case Some{+n}:
      Bool.pick(X.Cx<o>, is_busy(X.Rt.run(rt(o, cx, n))),
        reply(o, out(o, cx, [J.kv("fx", J.JStr{"interrupt"}), J.kv("agent", J.JStr{n})]), token, ok_body(Nil{})),
        reply_err(o, cx, token, "pas de tour en cours"))

def req_stop(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +agent: String, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("aucune tâche nommée ", agent))
    case Some{+n}:
      Bool.pick(X.Cx<o>, String.eq(n, M.MAIN()), reply_err(o, cx, token, String.append("aucune tâche nommée ", agent)),
        reply(o, line(o, stop_task(o, cx, n, M.LStopped{}, reason), M.MAIN(), "info", cat(["main → @", n, " arrêtée : ", reason])), token, ok_body(Nil{})))

def req_drop.res(-o: M.St, p: X.Cx<o> & DropR, +token: Nat, +agent: String) -> X.Cx<o>:
  match p:
    case (+cx, DDone{}):
      reply(o, cx, token, ok_body([J.kv("dropped", J.JBool{True{}})]))
    case (+cx, DAsk{+q}):
      +n = Maybe.default(&2, String, M.resolve(st(o, cx), agent), agent)
      +id = M.St.next_card(st(o, cx))
      +text = Bool.pick(String, String.ends_with(q, " [y/N]"), String.take(q, Nat.sub(String.length(q), 6n)), q)
      +cx2 = open_card(o, cx, "drop", n, cat(["main propose : ", text, " (réponds oui ou non)"]), None{})
      reply(o, cx2, token, ok_body([J.kv("dropped", J.JBool{False{}}), J.kv("card", J.JNum{id}),
        J.kv("reason", J.JStr{"the user must confirm: an attention card is open"})]))
    case (+cx, DErr{+e}):
      reply_err(o, cx, token, e)
    case (+cx, DHalt{}):
      cx

def req_card.settle(-o: M.St, m: Maybe<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+id}:
      emit(o, cx, M.ESettled{id}, Unit{}, Unit{})

def req_card.go(-o: M.St, +cx: X.Cx<o>, +token: Nat, +agent: String, +text: String, +for_msg: Maybe<&2, Nat>) -> X.Cx<o>:
  +id = M.St.next_card(st(o, cx))
  +cx2 = open_card(o, cx, "question", agent, text, for_msg)
  # main hands the question over: no automatic reply
  +cx3 = req_card.settle(o, for_msg, cx2)
  reply(o, cx3, token, ok_body([J.kv("card", J.JNum{id})]))

def req_card.msg(-o: M.St, q: Maybe<&2, M.Msg>, +cx: X.Cx<o>, +token: Nat, +text: String, +id: Nat) -> X.Cx<o>:
  match q:
    case None{}:
      reply_err(o, cx, token, String.append("message inconnu : ", mid(id)))
    case Some{+m}:
      req_card.go(o, cx, token, M.Msg.from(m), text, Some{id})

def req_card(-o: M.St, for_msg: Maybe<&2, Nat>, +cx: X.Cx<o>, +token: Nat, +text: String) -> X.Cx<o>:
  match for_msg:
    case None{}:
      req_card.go(o, cx, token, M.MAIN(), text, None{})
    case Some{+id}:
      req_card.msg(o, M.msg_of(st(o, cx), id), cx, token, text, id)

def cmd_tag(+c: String) -> Cmd:
  Bool.pick(Cmd, String.eq(c, "send"), CSend{},
  Bool.pick(Cmd, String.eq(c, "wait"), CWait{},
  Bool.pick(Cmd, String.eq(c, "ask"), CAsk{},
  Bool.pick(Cmd, String.eq(c, "status"), CStatus{},
  Bool.pick(Cmd, String.eq(c, "report"), CReport{},
  Bool.pick(Cmd, String.eq(c, "spawn"), CSpawn{},
  Bool.pick(Cmd, String.eq(c, "interrupt"), CInterrupt{},
  Bool.pick(Cmd, String.eq(c, "stop"), CStop{},
  Bool.pick(Cmd, String.eq(c, "drop"), CDrop{}, CCard{})))))))))

def req.main.go(-o: M.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSpawn{}:
      req_spawn.pre(o, jopt_str(J.get(q, "pre_err")), cx, token, q)
    case CInterrupt{}:
      req_interrupt(o, M.resolve(st(o, cx), jstr(q, "agent")), cx, token, jstr(q, "agent"))
    case CStop{}:
      req_stop(o, M.resolve(st(o, cx), jstr(q, "agent")), cx, token, jstr(q, "agent"), jstr(q, "reason"))
    case CDrop{}:
      req_drop.res(o, drop_task(o, cx, jstr(q, "agent"), False{}), token, jstr(q, "agent"))
    case _:
      req_card(o, jopt_nat(J.get(q, "for")), cx, token, jstr(q, "text"))

def req.main(-o: M.St, is_main: Bool, +cx: X.Cx<o>, +token: Nat, +c: Cmd, +q: J.Json) -> X.Cx<o>:
  match is_main:
    case False{}:
      main_only(o, cx, token)
    case True{}:
      req.main.go(o, c, cx, token, q)

def req.go(-o: M.St, c: Cmd, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  match c:
    case CSend{}:
      req_send(o, cx, token, from, q)
    case CWait{}:
      wait(o, cx, token, from, jnat(q, "msg"), jnat(q, "timeout_s"))
    case CAsk{}:
      req_ask.res(o, send(o, cx, msg(from, jstr(q, "to"), jstr(q, "text"), True{}, None{}, False{}, False{})), token, from, jnat(q, "timeout_s"))
    case CStatus{}:
      req_status(o, cx, token, from, String.eq(from, M.MAIN()), q)
    case CReport{}:
      req_report.go(o, String.eq(from, M.MAIN()), cx, token, from, q)
    case other:
      req.main(o, String.eq(from, M.MAIN()), cx, token, other, q)

def req.from(-o: M.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +token: Nat, +raw: String, +q: J.Json) -> X.Cx<o>:
  match m:
    case None{}:
      reply_err(o, cx, token, String.append("agent inconnu : ", raw))
    case Some{+from}:
      req.go(o, cmd_tag(jstr(q, "cmd")), cx, token, from, q)

def agent_req(-o: M.St, +cx: X.Cx<o>, +token: Nat, +from: String, +q: J.Json) -> X.Cx<o>:
  req.from(o, M.resolve(st(o, cx), from), cx, token, from, q)

# ---- the step ----

def Hub.d(x: Hub) -> M.St:
  match x:
    case Hub{+d, _}:
      d

def Hub.r(x: Hub) -> X.R:
  match x:
    case Hub{_, +r}:
      r

def hub_new(+workspace: String) -> Hub:
  Hub{M.init(workspace), X.R{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, Nil{}, workspace, False{}, 0n, False{}, Nil{}, None{}}}

# the context of one input over the hub
def start(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  +r = Hub.r(h)
  +r2 = X.R.with_need(X.R.with_ans(X.R.with_git(X.R.with_now(X.R.with_dirty(r, False{}), jnat(i, "now")), jbool(i, "git")),
    Maybe.default(&2, List<&2, J.Json>, J.get_arr(i, "ans"), Nil{})), None{})
  X.Cx{X.core_start(Hub.d(h)), r2}

def agent_decl(m: Maybe<&2, M.Agent>) -> Maybe<&2, M.Decl>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      M.Agent.declared(a)

def turn.resume(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      +cx2 = emit(o, cx, M.EDeclared{agent, None{}, ""}, Unit{}, Unit{})
      close_cards(o, cx2, agent, ["blocked", "done"], "reprise")
    case False{}:
      cx

def decl_resumes(d: Maybe<&2, M.Decl>) -> Bool:
  match d:
    case Some{M.DDone{}}:
      True{}
    case Some{M.DBlocked{}}:
      True{}
    case _:
      False{}

def turn_started(-o: M.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +decl = agent_decl(M.get_agent(st(o, cx), agent))
  +cx2 = dirty(o, upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RBusy{}), Some{now(o, cx)})))
  turn.resume(o, decl_resumes(decl), cx2, agent)

def steer_count(-o: M.St, +cx: X.Cx<o>, +agent: String, +injected: Bool) -> X.Cx<o>:
  +r = rr(o, cx)
  +c = cnt_get(X.R.counts(r), agent)
  +c2 = Bool.pick(X.Cnt, injected, X.Cnt{agent, X.Cnt.recv(c), 1n+X.Cnt.inj(c)}, X.Cnt{agent, 1n+X.Cnt.recv(c), X.Cnt.inj(c)})
  set_r(o, cx, X.R.with_counts(r, M.snoc(X.Cnt, cnt_del(X.R.counts(r), agent), c2)))

def assistant(-o: M.St, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  +r = rr(o, cx)
  set_r(o, cx, X.R.with_last(r, M.snoc(X.Kv, kv_del(X.R.last(r), agent), X.Kv{agent, text})))

# ---- file overlap (RFC 0001 §10.3) ----

def touch.others(-o: M.St, others: List<&2, String>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match others:
    case Nil{}:
      cx
    case Con{+h, +t}:
      +text = cat([path, " est modifié par @", agent, " et par @", String.join({h <> t : List<&2, String>}, ", @")])
      note_main(o, open_card(o, cx, "overlap", agent, text, None{}), String.append("file overlap: ", text))

def overlap_hit(m: Maybe<&2, M.Agent>, +agent: String, +path: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.not(M.Agent.is_main(a)) && Bool.not(String.eq(M.Agent.name(a), agent))
        && Bool.not(M.ws_worktree(M.Agent.ws(a))) && is_active(M.Agent.life(a))
        && M.str_in(M.Agent.files(a), path)

def overlap_others(xs: List<&2, String>, +s: M.St, +agent: String, +path: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = overlap_others(t, s, agent, path)
      Bool.pick(List<&2, String>, overlap_hit(M.get_agent(s, n), agent, path), n <> rest, rest)

def touch.new(-o: M.St, known: Bool, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match known:
    case True{}:
      cx
    case False{}:
      +cx2 = emit(o, cx, M.EFileTouched{agent, path}, Unit{}, Unit{})
      touch.others(o, overlap_others(M.St.order(st(o, cx2)), st(o, cx2), agent, path), cx2, agent, path)

def touch.shared(-o: M.St, shared: Bool, +cx: X.Cx<o>, +agent: String, +path: String, +known: Bool) -> X.Cx<o>:
  match shared:
    case True{}:
      touch.new(o, known, cx, agent, path)
    case False{}:
      cx

def touch.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      # only a shared task: a worktree has its own files
      touch.shared(o, Bool.not(M.Agent.is_main(a)) && Bool.not(M.ws_worktree(M.Agent.ws(a))), cx, agent, path, M.str_in(M.Agent.files(a), path))

def touch(-o: M.St, +cx: X.Cx<o>, +agent: String, +path: String) -> X.Cx<o>:
  touch.agent(o, M.get_agent(st(o, cx), agent), cx, agent, path)

def repl_ready(-o: M.St, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  +cx2 = upd_rt(o, cx, X.Rt.with_waiting(X.Rt.with_run(rt(o, cx, agent), M.RIdle{}), False{}))
  pump(o, dirty(o, cx2), agent)

def fst(-o: M.St, p: X.Cx<o> & SendR) -> X.Cx<o>:
  match p:
    case (+cx, _):
      cx

# a notification of the hub to main: it wakes main like any message
def notify_main(-o: M.St, +cx: X.Cx<o>, +text: String) -> X.Cx<o>:
  fst(o, send(o, cx, msg(M.HUB(), M.MAIN(), text, False{}, None{}, False{}, False{})))

# ---- the REPLs ----

def crash.restart(-o: M.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +cx2 = set_run(o, cx, agent, M.RStarting{})
  +cx3 = out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{agent}),
    J.kv("resume", J.JBool{True{}}), J.kv("crash_note", J.JStr{reason})])
  +cx4 = line(o, cx3, M.MAIN(), "warn", cat(["la session de @", agent, " a planté (", reason, ") — redémarrage ", ns(n), "/", ns(MAX_CRASHES())]))
  Bool.pick(X.Cx<o>, String.eq(agent, M.MAIN()), cx4,
    notify_main(o, cx4, cat(["Task @", agent, " crashed (", reason, "). Its session restarts on its saved checkpoint (attempt ", ns(n), "/", ns(MAX_CRASHES()), "); the turn it was running is interrupted and it may need a new message to continue."])))

def crash.fail(-o: M.St, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  +why = cat(["la REPL a planté ", ns(n), " fois : ", reason])
  +cx2 = emit(o, cx, M.ELifecycle{agent, M.LFailed{}, Some{why}}, Unit{}, Unit{})
  +cx3 = open_card(o, cx2, "failed", agent, why, None{})
  Bool.pick(X.Cx<o>, String.eq(agent, M.MAIN()), cx3,
    notify_main(o, cx3, cat(["Task @", agent, " failed: ", why, ". It is not restarted anymore. A new message from you or the user restarts it; otherwise tell the user."])))

def crash.pick(-o: M.St, again: Bool, +cx: X.Cx<o>, +agent: String, +n: Nat, +reason: String) -> X.Cx<o>:
  match again:
    case True{}:
      crash.restart(o, cx, agent, n, reason)
    case False{}:
      crash.fail(o, cx, agent, n, reason)

def crash.count(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String, +reason: String) -> X.Cx<o>:
  match go:
    case False{}:
      cx
    case True{}:
      +r = rr(o, cx)
      +n = {1n+kn_get(X.R.crashes(r), agent) : Nat}
      +cx2 = set_r(o, cx, X.R.with_crashes(r, M.snoc(X.Kn, kn_del(X.R.crashes(r), agent), X.Kn{agent, n})))
      crash.pick(o, Nat.is_le(n, MAX_CRASHES()), cx2, agent, n, reason)

def exited.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +crashed: Bool, +reason: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +cx2 = set_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RDown{}), None{}))
      crash.count(o, crashed && is_active(M.Agent.life(a)), dirty(o, cx2), agent, reason)

def repl_exited(-o: M.St, +cx: X.Cx<o>, +agent: String, +crashed: Bool, +reason: String) -> X.Cx<o>:
  +cx2 = fail_waiters(o, cx, agent, "interrupted_by_restart")
  +r = rr(o, cx2)
  +cx3 = set_r(o, cx2, X.R.with_steered(r, steer_del(X.R.steered(r), agent)))
  exited.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, crashed, reason)

def boot.live(-o: M.St, live: Bool, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match live:
    case True{}:
      +cx2 = set_run(o, cx, n, M.RStarting{})
      out(o, cx2, [J.kv("fx", J.JStr{"spawn"}), J.kv("agent", J.JStr{n}),
        J.kv("resume", J.JBool{True{}}), J.kv("crash_note", J.JNull{})])
    case False{}:
      cx

def boot.one(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      boot.live(o, is_active(M.Agent.life(a)), cx, n)

def boot.each(-o: M.St, names: List<&2, String>, +cx: X.Cx<o>) -> X.Cx<o>:
  match names:
    case Nil{}:
      dirty(o, cx)
    case Con{+n, +rest}:
      boot.each(o, rest, boot.one(o, M.get_agent(st(o, cx), n), cx, n))

# the daemon started: spawn every live agent
def boot(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  boot.each(o, M.St.order(st(o, cx)), cx)

def tick.each(-o: M.St, late: List<&2, X.Waiter>, +cx: X.Cx<o>) -> X.Cx<o>:
  match late:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = reply(o, cx, X.Waiter.token(w), J.obj([J.kv("ok", J.JBool{False{}}), J.kv("error", J.JStr{"timeout"}),
        J.kv("hint", J.JStr{cat(["no reply to ", mid(X.Waiter.msg(w)), " yet. It stays valid: end your turn (the reply will wake you up) or continue with an explicit assumption."])})]))
      +still = any_waiter(X.R.waiters(rr(o, cx2)), X.Waiter.agent(w))
      +cx3 = dirty(o, upd_rt(o, cx2, X.Rt.with_waiting(rt(o, cx2, X.Waiter.agent(w)), still)))
      tick.each(o, rest, pump(o, cx3, X.Waiter.agent(w)))

def tick(-o: M.St, +cx: X.Cx<o>) -> X.Cx<o>:
  +r = rr(o, cx)
  +late = waiters_late(X.R.waiters(r), X.R.now(r))
  tick.each(o, late, set_r(o, cx, X.R.with_waiters(r, waiters_keep(X.R.waiters(r), X.R.now(r)))))

def requeue(-o: M.St, ids: List<&2, Nat>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ids:
    case Nil{}:
      cx
    case Con{+id, +rest}:
      requeue(o, rest, emit(o, cx, M.EMsgState{id, M.Queued{"steer_leftover"}}, Unit{}, Unit{}))

def skip_steered(xs: List<&2, M.Msg>, +skip: Bool, +ids: List<&2, Nat>) -> List<&2, M.Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = skip_steered(t, skip, ids)
      Bool.pick(List<&2, M.Msg>, skip && M.nat_in(ids, M.Msg.id(h)), rest, h <> rest)

# RFC 0003 §5.1: the end-of-turn answer to `@task message` typed in
# another view goes back to that view, and main gets it as a note
def auto_reply(-o: M.St, via: Maybe<&2, String>, +cx: X.Cx<o>, +agent: String, +m: M.Msg, +body: String) -> X.Cx<o>:
  match via:
    case None{}:
      fst(o, send(o, cx, msg(agent, M.Msg.from(m), body, False{}, Some{M.Msg.id(m)}, False{}, True{})))
    case Some{+view}:
      +cx2 = emit(o, cx, M.ESettled{M.Msg.id(m)}, Unit{}, Unit{})
      +cx3 = line(o, cx2, view, "msg-in", cat(["@", agent, " : ", body]))
      note_main(o, cx3, cat(["@", agent, " answered the user (asked from @", view, "'s view: \"", M.clip(M.one_line(M.Msg.text(m)), 500n),
        "\"): \"", M.clip_tail(M.one_line(body), 2000n), "\""]))

# RFC 0003 §7: a question gets an answer, the end of the turn
def auto_replies(-o: M.St, qs: List<&2, M.Msg>, +cx: X.Cx<o>, +agent: String, +body: String) -> X.Cx<o>:
  match qs:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      auto_replies(o, rest, auto_reply(o, Bool.pick(Maybe<&2, String>, String.eq(M.Msg.from(m), M.USER()), M.Msg.via(m), None{}), cx, agent, m, body), agent, body)

def idle.report(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String, +text: String) -> X.Cx<o>:
  match go:
    case True{}:
      emit(o, cx, M.EReported{agent, J.obj([J.kv("at_ms", J.JNum{now(o, cx)}), J.kv("kind", J.JStr{"turn"}),
        J.kv("summary", J.JStr{M.clip(text, 1000n)}), J.kv("decisions", J.JArr{Nil{}}), J.kv("auto", J.JBool{True{}})])}, Unit{}, Unit{})
    case False{}:
      cx

def idle.nudge(-o: M.St, go: Bool, +cx: X.Cx<o>, +agent: String) -> X.Cx<o>:
  match go:
    case True{}:
      +cx2 = upd_rt(o, cx, X.Rt.with_turn(X.Rt.with_run(rt(o, cx, agent), M.RBusy{}), Some{now(o, cx)}))
      out(o, cx2, [J.kv("fx", J.JStr{"say"}), J.kv("agent", J.JStr{agent}),
        J.kv("text", J.JStr{"[switchboard] The message(s) just above arrived while you were writing your last answer: you have not answered them yet. Answer them now."})])
    case False{}:
      cx

def idle.agent(-o: M.St, m: Maybe<&2, M.Agent>, +cx: X.Cx<o>, +agent: String, +leftover: Bool, +unread: Bool, +steered: List<&2, Nat>) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+a}:
      +cx2 = dirty(o, set_rt(o, cx, X.Rt{agent, M.RIdle{}, False{}, None{}}))
      +r = rr(o, cx2)
      +text = kv_get(X.R.last(r), agent)
      +cx3 = set_r(o, cx2, X.R.with_last(r, kv_del(X.R.last(r), agent)))
      +qs = skip_steered(M.unanswered_for(st(o, cx3), agent), leftover || unread, steered)
      +body = Bool.pick(String, M.trim_is_empty(text), "(pas de réponse écrite à la fin du tour)", text)
      +cx4 = auto_replies(o, qs, cx3, agent, body)
      # RFC 0001 §7.2: the automatic report (board only)
      +cx5 = idle.report(o, Bool.not(M.Agent.is_main(a)) && Bool.not(M.trim_is_empty(text)), cx4, agent, text)
      +cx6 = idle.nudge(o, unread && List.is_empty(&2, M.Msg, M.queued_for(st(o, cx5), agent)), cx5, agent)
      pump(o, cx6, agent)

def repl_idle(-o: M.St, +cx: X.Cx<o>, +agent: String, +leftover: Bool) -> X.Cx<o>:
  +cx1 = fail_waiters(o, cx, agent, "turn_ended")
  +r = rr(o, cx1)
  +steered = steer_get(X.R.steered(r), agent)
  +c = cnt_get(X.R.counts(r), agent)
  +cx2 = set_r(o, cx1, X.R.with_counts(X.R.with_steered(r, steer_del(X.R.steered(r), agent)), cnt_del(X.R.counts(r), agent)))
  # steering committed after the last model call: the model never read
  # it; a new turn makes it answer
  +unread = Bool.not(leftover) && Nat.is_gt(X.Cnt.recv(c), X.Cnt.inj(c)) && Bool.not(List.is_empty(&2, Nat, steered))
  # leftover: the runtime never read them: they go again, as a new turn
  +cx3 = Bool.pick(X.Cx<o>, leftover, requeue(o, steered, cx2), cx2)
  idle.agent(o, M.get_agent(st(o, cx3), agent), cx3, agent, leftover, unread, steered)

# ---- waits (RFC 0003 §6.2) ----

def in_tag(+t: String) -> In:
  Bool.pick(In, String.eq(t, "req"), IReq{},
  Bool.pick(In, String.eq(t, "tick"), ITick{},
  Bool.pick(In, String.eq(t, "idle"), IIdle{},
  Bool.pick(In, String.eq(t, "turn_started"), ITurnStarted{},
  Bool.pick(In, String.eq(t, "steer_rx"), ISteerRx{},
  Bool.pick(In, String.eq(t, "steered"), ISteered{},
  Bool.pick(In, String.eq(t, "assistant"), IAssistant{},
  Bool.pick(In, String.eq(t, "touch"), ITouch{},
  Bool.pick(In, String.eq(t, "ready"), IReady{},
  Bool.pick(In, String.eq(t, "exited"), IExited{},
  Bool.pick(In, String.eq(t, "boot"), IBoot{},
  Bool.pick(In, String.eq(t, "say"), ISay{},
  Bool.pick(In, String.eq(t, "route"), IRoute{},
  Bool.pick(In, String.eq(t, "new"), INew{},
  Bool.pick(In, String.eq(t, "drop"), IDrop{},
  Bool.pick(In, String.eq(t, "confirm_drop"), IConfirmDrop{},
  Bool.pick(In, String.eq(t, "restore"), IRestore{},
  Bool.pick(In, String.eq(t, "isolate"), IIsolate{},
  Bool.pick(In, String.eq(t, "rename"), IRename{},
  Bool.pick(In, String.eq(t, "answer"), IAnswer{},
  Bool.pick(In, String.eq(t, "cancel"), ICancel{},
  Bool.pick(In, String.eq(t, "interrupt"), IInterrupt{},
  Bool.pick(In, String.eq(t, "passthrough"), IPassthrough{},
  Bool.pick(In, String.eq(t, "focus"), IFocus{},
  Bool.pick(In, String.eq(t, "close"), IClose{},
  Bool.pick(In, String.eq(t, "force_run"), IForceRun{},
  INone{}))))))))))))))))))))))))))

# `/close N`: the user files a card without answering it
def close_by_user(-o: M.St, m: Maybe<&2, M.Card>, +cx: X.Cx<o>, +id: Nat) -> X.Cx<o>:
  match m:
    case None{}:
      notice(o, cx, String.append("aucune carte ouverte #", ns(id)))
    case Some{+c}:
      note_main(o, close_card(o, cx, id, "classée"), cat(["the user closed card #", ns(id), " (", M.Card.kind(c), " @", M.Card.agent(c), ": \"", M.one_line(M.Card.text(c)), "\") without answering"]))

def force_run_of(+r: String) -> M.Run:
  Bool.pick(M.Run, String.eq(r, "starting"), M.RStarting{},
    Bool.pick(M.Run, String.eq(r, "idle"), M.RIdle{},
      Bool.pick(M.Run, String.eq(r, "busy"), M.RBusy{}, M.RDown{})))

def dispatch.go(-o: M.St, tag: In, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  match tag:
    case IReq{}:
      agent_req(o, cx, jnat(i, "token"), jstr(i, "from"), Maybe.default(&2, J.Json, J.get(i, "req"), J.JNull{}))
    case ITick{}:
      tick(o, cx)
    case IIdle{}:
      repl_idle(o, cx, jstr(i, "agent"), jbool(i, "leftover"))
    case ITurnStarted{}:
      turn_started(o, cx, jstr(i, "agent"))
    case ISteerRx{}:
      steer_count(o, cx, jstr(i, "agent"), False{})
    case ISteered{}:
      steer_count(o, cx, jstr(i, "agent"), True{})
    case IAssistant{}:
      assistant(o, cx, jstr(i, "agent"), jstr(i, "text"))
    case ITouch{}:
      touch(o, cx, jstr(i, "agent"), jstr(i, "path"))
    case IReady{}:
      repl_ready(o, cx, jstr(i, "agent"))
    case IExited{}:
      repl_exited(o, cx, jstr(i, "agent"), jbool(i, "crashed"), jstr(i, "reason"))
    case IBoot{}:
      boot(o, cx)
    case ISay{}:
      user_says(o, cx, jstr(i, "focus"), jstr(i, "text"))
    case IRoute{}:
      route(o, cx, jstr(i, "target"), jstr(i, "text"), jstr(i, "focus"))
    case INew{}:
      user_new.res(o, create_task(o, cx, new_task(M.USER(), i)))
    case IDrop{}:
      user_drop.res(o, drop_task(o, cx, jstr(i, "name"), jbool(i, "force")), jstr(i, "name"))
    case IConfirmDrop{}:
      user_confirm.res(o, drop_task(o, cx, jstr(i, "name"), True{}), jstr(i, "name"))
    case IRestore{}:
      user_restore.res(o, restore_task(o, cx, jstr(i, "name")))
    case IIsolate{}:
      user_isolate.res(o, isolate_task(o, cx, jstr(i, "name")))
    case IRename{}:
      rename.old(o, M.resolve(st(o, cx), jstr(i, "name")), cx, jstr(i, "name"), jstr(i, "new_name"), jbool(i, "valid"))
    case IAnswer{}:
      answer_card(o, cx, jnat(i, "card"), jstr(i, "text"))
    case ICancel{}:
      cancel(o, jopt_nat(J.get(i, "last")), cx)
    case IInterrupt{}:
      interrupt.go(o, M.resolve(st(o, cx), jstr(i, "agent")), cx)
    case IPassthrough{}:
      passthrough(o, cx, jstr(i, "focus"), jstr(i, "line"))
    case IFocus{}:
      focus(o, cx, jstr(i, "focus"), jopt_str(J.get(i, "note")), jstr(i, "direct"))
    case IClose{}:
      close_by_user(o, M.card_of(M.St.cards(st(o, cx)), jnat(i, "card")), cx, jnat(i, "card"))
    case IForceRun{}:
      # tests only: put an agent's REPL in a given state
      set_run(o, cx, jstr(i, "agent"), force_run_of(jstr(i, "run")))
    case INone{}:
      cx

def dispatch(-o: M.St, +cx: X.Cx<o>, +i: J.Json) -> X.Cx<o>:
  dispatch.go(o, in_tag(jstr(i, "t")), cx, i)

# one input: the context after it (its state is the replay of its
# journal over the hub's state: law replay_reproduces)
def step(+h: Hub, +i: J.Json) -> X.Cx<Hub.d(h)>:
  dispatch(Hub.d(h), start(h, i), i)

# ---- the end of a turn ----

def Snd.with_from(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{_, +to, +text, +expect, +reply_to, +plain, +auto, +queued, +via}:
      Snd{v, to, text, expect, reply_to, plain, auto, queued, via}

def Snd.with_text(x: Snd, +v: String) -> Snd:
  match x:
    case Snd{+from, +to, _, +expect, +reply_to, +plain, +auto, +queued, +via}:
      Snd{from, to, v, expect, reply_to, plain, auto, queued, via}

def Snd.expect(x: Snd) -> Bool:
  match x:
    case Snd{_, _, _, +expect, _, _, _, _, _}:
      expect

def Snd.with_expect(x: Snd, +v: Bool) -> Snd:
  match x:
    case Snd{+from, +to, +text, _, +reply_to, +plain, +auto, +queued, +via}:
      Snd{from, to, text, v, reply_to, plain, auto, queued, via}

def Snd.with_reply_to(x: Snd, +v: Maybe<&2, Nat>) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, _, +plain, +auto, +queued, +via}:
      Snd{from, to, text, expect, v, plain, auto, queued, via}

def Snd.plain(x: Snd) -> Bool:
  match x:
    case Snd{_, _, _, _, _, +plain, _, _, _}:
      plain

def Snd.with_plain(x: Snd, +v: Bool) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, _, +auto, +queued, +via}:
      Snd{from, to, text, expect, reply_to, v, auto, queued, via}

def Snd.auto(x: Snd) -> Bool:
  match x:
    case Snd{_, _, _, _, _, _, +auto, _, _}:
      auto

def Snd.with_auto(x: Snd, +v: Bool) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, _, +queued, +via}:
      Snd{from, to, text, expect, reply_to, plain, v, queued, via}

def Snd.queued(x: Snd) -> Bool:
  match x:
    case Snd{_, _, _, _, _, _, _, +queued, _}:
      queued

def Snd.with_queued(x: Snd, +v: Bool) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, _, +via}:
      Snd{from, to, text, expect, reply_to, plain, auto, v, via}

def Snd.via(x: Snd) -> Maybe<&2, String>:
  match x:
    case Snd{_, _, _, _, _, _, _, _, +via}:
      via

def Snd.with_via(x: Snd, +v: Maybe<&2, String>) -> Snd:
  match x:
    case Snd{+from, +to, +text, +expect, +reply_to, +plain, +auto, +queued, _}:
      Snd{from, to, text, expect, reply_to, plain, auto, queued, v}

# ---- committing a step (what sb-core keeps) ----

def commit.go(-o: M.St, need: Maybe<&2, J.Json>, +c: X.Core<o>, +r: X.R, +h: Hub) -> Out:
  match need:
    case Some{+q}:
      Out{h, Nil{}, Some{q}}
    case None{}:
      Out{Hub{X.Core.d(o, c), X.R.with_ans(r, Nil{})}, X.Core.fx(o, c), None{}}

def commit.cx(-o: M.St, cx: X.Cx<o>, +h: Hub) -> Out:
  match cx:
    case X.Cx{+c, +r}:
      commit.go(o, X.R.need(r), c, r, h)

def commit(+h: Hub, +i: J.Json) -> Out:
  commit.cx(Hub.d(h), step(h, i), h)

def RunR.hub(x: RunR) -> Hub:
  match x:
    case RunR{+hub, _}:
      hub

def Out.fx(x: Out) -> List<&2, X.Fx>:
  match x:
    case Out{_, +fx, _}:
      fx

def RunR.journal(x: RunR) -> List<&2, M.Ev>:
  match x:
    case RunR{_, +journal}:
      journal

def run.cat(+o1: Out, +rr: RunR) -> RunR:
  RunR{RunR.hub(rr), List.append(&2, M.Ev, X.journal(Out.fx(o1)), RunR.journal(rr))}

def Out.hub(x: Out) -> Hub:
  match x:
    case Out{+hub, _, _}:
      hub

def run(ins: List<&2, J.Json>, +h: Hub) -> RunR:
  match ins:
    case Nil{}:
      RunR{h, Nil{}}
    case Con{+i, +rest}:
      run.cat(commit(h, i), run(rest, Out.hub(commit(h, i))))

def Out.need(x: Out) -> Maybe<&2, J.Json>:
  match x:
    case Out{_, _, +need}:
      need
