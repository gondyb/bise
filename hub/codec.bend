# hub/codec.bend — the journal events and messages as JSON, in the exact
# serde format of rust/switchboard/src/model.rs (the Rust mirror and the
# journal file read them back).
import Base
import ../vendor/json.bend as J
import ./model.bend as M

def jmaybe_nat(m: Maybe<&2, Nat>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+n}:
      J.JNum{n}

def jmaybe_str(m: Maybe<&2, String>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+s}:
      J.JStr{s}

def life_str(l: M.Life) -> String:
  match l:
    case M.LActive{}:
      "active"
    case M.LStopped{}:
      "stopped"
    case M.LArchived{}:
      "archived"
    case M.LFailed{}:
      "failed"

def decl_str(d: M.Decl) -> String:
  match d:
    case M.DWorking{}:
      "working"
    case M.DDone{}:
      "done"
    case M.DBlocked{}:
      "blocked"

def jmaybe_decl(m: Maybe<&2, M.Decl>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+d}:
      J.JStr{decl_str(d)}

def msg_json(+m: M.Msg) -> J.Json:
  J.obj([J.kv("id", J.JNum{M.Msg.id(m)}), J.kv("thread", J.JNum{M.Msg.thread(m)}),
    J.kv("from", J.JStr{M.Msg.from(m)}), J.kv("to", J.JStr{M.Msg.to(m)}),
    J.kv("reply_to", jmaybe_nat(M.Msg.reply_to(m))),
    J.kv("expect_reply", J.JBool{M.Msg.expect(m)}), J.kv("auto", J.JBool{M.Msg.auto(m)}),
    J.kv("text", J.JStr{M.Msg.text(m)}), J.kv("created_ms", J.JNum{M.Msg.created(m)}),
    J.kv("plain", J.JBool{M.Msg.plain(m)}), J.kv("queued", J.JBool{M.Msg.queued(m)})])

def card_json(+c: M.Card) -> J.Json:
  J.obj([J.kv("id", J.JNum{M.Card.id(c)}), J.kv("kind", J.JStr{M.Card.kind(c)}),
    J.kv("agent", J.JStr{M.Card.agent(c)}), J.kv("text", J.JStr{M.Card.text(c)}),
    J.kv("for_msg", jmaybe_nat(M.Card.for_msg(c))), J.kv("created_ms", J.JNum{M.Card.created(c)})])

def mst_kvs(s: M.MSt) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match s:
    case M.Queued{+r}:
      [J.kv("state", J.JStr{"queued"}), J.kv("reason", J.JStr{r})]
    case M.Delivered{}:
      [J.kv("state", J.JStr{"delivered"})]
    case M.Rejected{+e}:
      [J.kv("state", J.JStr{"rejected"}), J.kv("error", J.JStr{e})]
    case M.Cancelled{}:
      [J.kv("state", J.JStr{"cancelled"})]

def ty(+t: String) -> Sigma<&2, &2, String, _ => J.Json>:
  J.kv("type", J.JStr{t})

def ev_json(e: M.Ev) -> J.Json:
  match e:
    case M.ETaskCreated{+name, +parent, +brief, +ws, +at}:
      J.obj([ty("task_created"), J.kv("name", J.JStr{name}), J.kv("parent", J.JStr{parent}),
        J.kv("brief", brief), J.kv("ws", ws), J.kv("at_ms", J.JNum{at})])
    case M.ELifecycle{+name, +life, +reason}:
      J.obj([ty("lifecycle"), J.kv("name", J.JStr{name}), J.kv("lifecycle", J.JStr{life_str(life)}),
        J.kv("reason", jmaybe_str(reason))])
    case M.EWorkspace{+name, +ws}:
      J.obj([ty("workspace_changed"), J.kv("name", J.JStr{name}), J.kv("ws", ws)])
    case M.ESnapshot{+name, +snap}:
      J.obj([ty("snapshot"), J.kv("name", J.JStr{name}), J.kv("snapshot_ref", jmaybe_str(snap))])
    case M.EDeclared{+name, +status, +note}:
      J.obj([ty("declared"), J.kv("name", J.JStr{name}), J.kv("status", jmaybe_decl(status)),
        J.kv("note", J.JStr{note})])
    case M.EReported{+name, +report}:
      J.obj([ty("reported"), J.kv("name", J.JStr{name}), J.kv("report", report)])
    case M.EFileTouched{+name, +path}:
      J.obj([ty("file_touched"), J.kv("name", J.JStr{name}), J.kv("path", J.JStr{path})])
    case M.ERenamed{+name, +nn}:
      J.obj([ty("renamed"), J.kv("name", J.JStr{name}), J.kv("new_name", J.JStr{nn})])
    case M.EMsgSent{+m}:
      J.obj([ty("message_sent"), J.kv("msg", msg_json(m))])
    case M.EMsgState{+id, +s}:
      J.obj(ty("message_state") <> J.kv("id", J.JNum{id}) <> mst_kvs(s))
    case M.ESettled{+id}:
      J.obj([ty("message_settled"), J.kv("id", J.JNum{id})])
    case M.ECardOpened{+c}:
      J.obj([ty("card_opened"), J.kv("card", card_json(c))])
    case M.ECardClosed{+id, +res}:
      J.obj([ty("card_closed"), J.kv("id", J.JNum{id}), J.kv("resolution", J.JStr{res})])
    case M.EMainNote{+text, +at}:
      J.obj([ty("main_note"), J.kv("text", J.JStr{text}), J.kv("at_ms", J.JNum{at})])
    case M.ENotesFlushed{}:
      J.obj([ty("main_notes_flushed")])

# ---- decoding (the journal replayed at boot) ----

def get_nat(+j: J.Json, +k: String) -> Nat:
  J.get_num_or(j, k, 0n)

def get_s(+j: J.Json, +k: String) -> String:
  J.get_str_or(j, k, "")

def get_b(+j: J.Json, +k: String) -> Bool:
  J.get_bool_or(j, k, False{})

def get_j(+j: J.Json, +k: String) -> J.Json:
  Maybe.default(&2, J.Json, J.get(j, k), J.JNull{})

def opt_nat(m: Maybe<&2, J.Json>) -> Maybe<&2, Nat>:
  match m:
    case Some{J.JNum{+n}}:
      Some{n}
    case _:
      None{}

def opt_str(m: Maybe<&2, J.Json>) -> Maybe<&2, String>:
  match m:
    case Some{J.JStr{+s}}:
      Some{s}
    case _:
      None{}

def life_of(+s: String) -> M.Life:
  Bool.pick(M.Life, String.eq(s, "stopped"), M.LStopped{},
    Bool.pick(M.Life, String.eq(s, "archived"), M.LArchived{},
      Bool.pick(M.Life, String.eq(s, "failed"), M.LFailed{}, M.LActive{})))

def decl_of(m: Maybe<&2, String>) -> Maybe<&2, M.Decl>:
  match m:
    case None{}:
      None{}
    case Some{+s}:
      Some{Bool.pick(M.Decl, String.eq(s, "done"), M.DDone{},
        Bool.pick(M.Decl, String.eq(s, "blocked"), M.DBlocked{}, M.DWorking{}))}

def msg_of(+j: J.Json) -> M.Msg:
  M.Msg{get_nat(j, "id"), get_nat(j, "thread"), get_s(j, "from"), get_s(j, "to"),
    opt_nat(J.get(j, "reply_to")), get_b(j, "expect_reply"), get_b(j, "auto"),
    get_s(j, "text"), get_nat(j, "created_ms"), get_b(j, "plain"), get_b(j, "queued")}

def card_of(+j: J.Json) -> M.Card:
  M.Card{get_nat(j, "id"), get_s(j, "kind"), get_s(j, "agent"), get_s(j, "text"),
    opt_nat(J.get(j, "for_msg")), get_nat(j, "created_ms")}

def mst_of(+j: J.Json) -> M.MSt:
  +s = get_s(j, "state")
  Bool.pick(M.MSt, String.eq(s, "delivered"), M.Delivered{},
    Bool.pick(M.MSt, String.eq(s, "rejected"), M.Rejected{get_s(j, "error")},
      Bool.pick(M.MSt, String.eq(s, "cancelled"), M.Cancelled{}, M.Queued{get_s(j, "reason")})))

def ev_of(+j: J.Json) -> Maybe<&2, M.Ev>:
  +t = get_s(j, "type")
  +n = get_s(j, "name")
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "task_created"), Some{M.ETaskCreated{n, get_s(j, "parent"), get_j(j, "brief"), get_j(j, "ws"), get_nat(j, "at_ms")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "lifecycle"), Some{M.ELifecycle{n, life_of(get_s(j, "lifecycle")), opt_str(J.get(j, "reason"))}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "workspace_changed"), Some{M.EWorkspace{n, get_j(j, "ws")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "snapshot"), Some{M.ESnapshot{n, opt_str(J.get(j, "snapshot_ref"))}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "declared"), Some{M.EDeclared{n, decl_of(opt_str(J.get(j, "status"))), get_s(j, "note")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "reported"), Some{M.EReported{n, get_j(j, "report")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "file_touched"), Some{M.EFileTouched{n, get_s(j, "path")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "renamed"), Some{M.ERenamed{n, get_s(j, "new_name")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "message_sent"), Some{M.EMsgSent{msg_of(get_j(j, "msg"))}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "message_state"), Some{M.EMsgState{get_nat(j, "id"), mst_of(j)}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "message_settled"), Some{M.ESettled{get_nat(j, "id")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "card_opened"), Some{M.ECardOpened{card_of(get_j(j, "card"))}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "card_closed"), Some{M.ECardClosed{get_nat(j, "id"), get_s(j, "resolution")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "main_note"), Some{M.EMainNote{get_s(j, "text"), get_nat(j, "at_ms")}},
  Bool.pick(Maybe<&2, M.Ev>, String.eq(t, "main_notes_flushed"), Some{M.ENotesFlushed{}},
  None{})))))))))))))))
