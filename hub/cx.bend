# hub/cx.bend — the step context of the hub: the durable state, the
# runtime bookkeeping and the effects of one input. `Core<o>` carries a
# proof that its state is the replay of its journal over `o`, the state
# before the input: every change of the durable state goes through
# `emit`, which extends the proof (law replay_reproduces).
import Base

import ../vendor/json.bend as J

import ./model.bend as M

type Fx is Data:
  FJ{ev: M.Ev}
  FO{o: J.Json}

type Rt is Data:
  Rt{name: String, run: M.Run, waiting: Bool, turn: Maybe<&2, Nat>}

type Waiter is Data:
  Waiter{token: Nat, agent: String, msg: Nat, deadline: Nat}

type Steer is Data:
  Steer{agent: String, ids: List<&2, Nat>}

type Cnt is Data:
  Cnt{agent: String, recv: Nat, inj: Nat}

type Kv is Data:
  Kv{key: String, val: String}

type Kn is Data:
  Kn{key: String, n: Nat}

# the runtime part of the hub (never journaled) and the facts of the input
type R is Data:
  R{rts: List<&2, Rt>, waiters: List<&2, Waiter>, steered: List<&2, Steer>,
    counts: List<&2, Cnt>, last: List<&2, Kv>, crashes: List<&2, Kn>,
    workspace: String, dirty: Bool, now: Nat, git: Bool,
    ans: List<&2, J.Json>, need: Maybe<&2, J.Json>}

type Core<-o: M.St> is Data:
  Core{d: M.St, fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}}

type Cx<-o: M.St> is Data:
  Cx{c: Core<o>, r: R}

# ---- accessors (generated) ----
def Rt.name(x: Rt) -> String:
  match x:
    case Rt{+name, _, _, _}:
      name

def Rt.with_name(x: Rt, +v: String) -> Rt:
  match x:
    case Rt{_, +run, +waiting, +turn}:
      Rt{v, run, waiting, turn}

def Rt.run(x: Rt) -> M.Run:
  match x:
    case Rt{_, +run, _, _}:
      run

def Rt.with_run(x: Rt, +v: M.Run) -> Rt:
  match x:
    case Rt{+name, _, +waiting, +turn}:
      Rt{name, v, waiting, turn}

def Rt.waiting(x: Rt) -> Bool:
  match x:
    case Rt{_, _, +waiting, _}:
      waiting

def Rt.with_waiting(x: Rt, +v: Bool) -> Rt:
  match x:
    case Rt{+name, +run, _, +turn}:
      Rt{name, run, v, turn}

def Rt.turn(x: Rt) -> Maybe<&2, Nat>:
  match x:
    case Rt{_, _, _, +turn}:
      turn

def Rt.with_turn(x: Rt, +v: Maybe<&2, Nat>) -> Rt:
  match x:
    case Rt{+name, +run, +waiting, _}:
      Rt{name, run, waiting, v}

def Waiter.token(x: Waiter) -> Nat:
  match x:
    case Waiter{+token, _, _, _}:
      token

def Waiter.with_token(x: Waiter, +v: Nat) -> Waiter:
  match x:
    case Waiter{_, +agent, +msg, +deadline}:
      Waiter{v, agent, msg, deadline}

def Waiter.agent(x: Waiter) -> String:
  match x:
    case Waiter{_, +agent, _, _}:
      agent

def Waiter.with_agent(x: Waiter, +v: String) -> Waiter:
  match x:
    case Waiter{+token, _, +msg, +deadline}:
      Waiter{token, v, msg, deadline}

def Waiter.msg(x: Waiter) -> Nat:
  match x:
    case Waiter{_, _, +msg, _}:
      msg

def Waiter.with_msg(x: Waiter, +v: Nat) -> Waiter:
  match x:
    case Waiter{+token, +agent, _, +deadline}:
      Waiter{token, agent, v, deadline}

def Waiter.deadline(x: Waiter) -> Nat:
  match x:
    case Waiter{_, _, _, +deadline}:
      deadline

def Waiter.with_deadline(x: Waiter, +v: Nat) -> Waiter:
  match x:
    case Waiter{+token, +agent, +msg, _}:
      Waiter{token, agent, msg, v}

def Steer.agent(x: Steer) -> String:
  match x:
    case Steer{+agent, _}:
      agent

def Steer.ids(x: Steer) -> List<&2, Nat>:
  match x:
    case Steer{_, +ids}:
      ids

def Cnt.agent(x: Cnt) -> String:
  match x:
    case Cnt{+agent, _, _}:
      agent

def Cnt.recv(x: Cnt) -> Nat:
  match x:
    case Cnt{_, +recv, _}:
      recv

def Cnt.inj(x: Cnt) -> Nat:
  match x:
    case Cnt{_, _, +inj}:
      inj

def Kv.key(x: Kv) -> String:
  match x:
    case Kv{+key, _}:
      key

def Kv.val(x: Kv) -> String:
  match x:
    case Kv{_, +val}:
      val

def Kn.key(x: Kn) -> String:
  match x:
    case Kn{+key, _}:
      key

def Kn.n(x: Kn) -> Nat:
  match x:
    case Kn{_, +n}:
      n

def R.rts(x: R) -> List<&2, Rt>:
  match x:
    case R{+rts, _, _, _, _, _, _, _, _, _, _, _}:
      rts

def R.with_rts(x: R, +v: List<&2, Rt>) -> R:
  match x:
    case R{_, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{v, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.waiters(x: R) -> List<&2, Waiter>:
  match x:
    case R{_, +waiters, _, _, _, _, _, _, _, _, _, _}:
      waiters

def R.with_waiters(x: R, +v: List<&2, Waiter>) -> R:
  match x:
    case R{+rts, _, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, v, steered, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.steered(x: R) -> List<&2, Steer>:
  match x:
    case R{_, _, +steered, _, _, _, _, _, _, _, _, _}:
      steered

def R.with_steered(x: R, +v: List<&2, Steer>) -> R:
  match x:
    case R{+rts, +waiters, _, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, v, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.counts(x: R) -> List<&2, Cnt>:
  match x:
    case R{_, _, _, +counts, _, _, _, _, _, _, _, _}:
      counts

def R.with_counts(x: R, +v: List<&2, Cnt>) -> R:
  match x:
    case R{+rts, +waiters, +steered, _, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, v, last, crashes, workspace, dirty, now, git, ans, need}

def R.last(x: R) -> List<&2, Kv>:
  match x:
    case R{_, _, _, _, +last, _, _, _, _, _, _, _}:
      last

def R.with_last(x: R, +v: List<&2, Kv>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, _, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, v, crashes, workspace, dirty, now, git, ans, need}

def R.crashes(x: R) -> List<&2, Kn>:
  match x:
    case R{_, _, _, _, _, +crashes, _, _, _, _, _, _}:
      crashes

def R.with_crashes(x: R, +v: List<&2, Kn>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, _, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, v, workspace, dirty, now, git, ans, need}

def R.workspace(x: R) -> String:
  match x:
    case R{_, _, _, _, _, _, +workspace, _, _, _, _, _}:
      workspace

def R.with_workspace(x: R, +v: String) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, _, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, v, dirty, now, git, ans, need}

def R.dirty(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, +dirty, _, _, _, _}:
      dirty

def R.with_dirty(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, _, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, v, now, git, ans, need}

def R.now(x: R) -> Nat:
  match x:
    case R{_, _, _, _, _, _, _, _, +now, _, _, _}:
      now

def R.with_now(x: R, +v: Nat) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, _, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, v, git, ans, need}

def R.git(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, _, _, +git, _, _}:
      git

def R.with_git(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, _, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, v, ans, need}

def R.ans(x: R) -> List<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, +ans, _}:
      ans

def R.with_ans(x: R, +v: List<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, _, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, v, need}

def R.need(x: R) -> Maybe<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, +need}:
      need

def R.with_need(x: R, +v: Maybe<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, _}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, v}

# ---- end accessors ----

# the journal events among the effects, in order
def journal(fx: List<&2, Fx>) -> List<&2, M.Ev>:
  match fx:
    case Nil{}:
      Nil{}
    case Con{f, rest}:
      match f:
        case FJ{+e}:
          e <> journal(rest)
        case FO{_}:
          journal(rest)



# ---- the replay lemmas ----

def apply_all_snoc(+evs: List<&2, M.Ev>, -s: M.St, -e: M.Ev)
    -> {M.apply(M.apply_all(evs, s), e) == M.apply_all(M.snoc(M.Ev, evs, e), s) : M.St}:
  match evs:
    case Nil{}:
      {==}
    case Con{h, rest}:
      apply_all_snoc(rest, M.apply(s, h), e)

def journal_snoc_ev(+fx: List<&2, Fx>, -e: M.Ev)
    -> {M.snoc(M.Ev, journal(fx), e) == journal(M.snoc(Fx, fx, FJ{e})) : List<&2, M.Ev>}:
  match fx:
    case Nil{}:
      {==}
    case Con{f, rest}:
      match f:
        case FJ{h}:
          %journal_snoc_ev(rest, e) : {h <> M.snoc(M.Ev, journal(rest), e) == h <> _ : List<&2, M.Ev>}
          {==}
        case FO{_}:
          journal_snoc_ev(rest, e)

def journal_snoc_out(+fx: List<&2, Fx>, -x: J.Json)
    -> {journal(fx) == journal(M.snoc(Fx, fx, FO{x})) : List<&2, M.Ev>}:
  match fx:
    case Nil{}:
      {==}
    case Con{f, rest}:
      match f:
        case FJ{h}:
          %journal_snoc_out(rest, x) : {h <> journal(rest) == h <> _ : List<&2, M.Ev>}
          {==}
        case FO{_}:
          journal_snoc_out(rest, x)

def emit_ok(-o: M.St, +d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -e: M.Ev)
    -> {M.apply(d, e) == M.apply_all(journal(M.snoc(Fx, fx, FJ{e})), o) : M.St}:
  %journal_snoc_ev(fx, e) : {M.apply(d, e) == M.apply_all(_, o) : M.St}
  %apply_all_snoc(journal(fx), o, e) : {M.apply(d, e) == _ : M.St}
  %p : {M.apply(d, e) == M.apply(_, e) : M.St}
  {==}

def out_ok(-o: M.St, +d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -x: J.Json)
    -> {d == M.apply_all(journal(M.snoc(Fx, fx, FO{x})), o) : M.St}:
  %journal_snoc_out(fx, x) : {d == M.apply_all(_, o) : M.St}
  p

# ---- the core primitives ----

def core_emit(-o: M.St, c: Core<o>, +e: M.Ev) -> Core<o>:
  match c:
    case Core{+d, +fx, p}:
      Core{M.apply(d, e), M.snoc(Fx, fx, FJ{e}), emit_ok(o, d, fx, p, e)}

def core_out(-o: M.St, c: Core<o>, +x: J.Json) -> Core<o>:
  match c:
    case Core{+d, +fx, p}:
      Core{d, M.snoc(Fx, fx, FO{x}), out_ok(o, d, fx, p, x)}

def core_start(+d: M.St) -> Core<d>:
  Core{d, Nil{}, {==}}

def Core.d(-o: M.St, c: Core<o>) -> M.St:
  match c:
    case Core{+d, _, _}:
      d

def Core.fx(-o: M.St, c: Core<o>) -> List<&2, Fx>:
  match c:
    case Core{_, +fx, _}:
      fx

# ---- the context ----

def Cx.c(-o: M.St, cx: Cx<o>) -> Core<o>:
  match cx:
    case Cx{+c, _}:
      c

def Cx.r(-o: M.St, cx: Cx<o>) -> R:
  match cx:
    case Cx{_, +r}:
      r

def d(-o: M.St, +cx: Cx<o>) -> M.St:
  Core.d(o, Cx.c(o, cx))

def mark(+r: R) -> R:
  match r:
    case R{+a, +b, +c, +e, +f, +g, +w, _, +n, +gi, +an, +ne}:
      R{a, b, c, e, f, g, w, True{}, n, gi, an, ne}

def emit(-o: M.St, cx: Cx<o>, +e: M.Ev) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_emit(o, c, e), mark(r)}

def out(-o: M.St, cx: Cx<o>, +x: J.Json) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_out(o, c, x), r}

def with_r(-o: M.St, cx: Cx<o>, +r: R) -> Cx<o>:
  match cx:
    case Cx{+c, _}:
      Cx{c, r}

def dirty(-o: M.St, +cx: Cx<o>) -> Cx<o>:
  with_r(o, cx, mark(Cx.r(o, cx)))

def now(-o: M.St, +cx: Cx<o>) -> Nat:
  R.now(Cx.r(o, cx))
