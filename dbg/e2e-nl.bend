import Base
import ../core/types.bend as T 
import ../core/session.bend as S
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../runtime/remote.bend as Rem

def cfg() -> T.Cfg:
  T.Cfg{600n, 40n, 3n, "sys", Nil{}}

def dump_res(tag: String, +r: T.Res) -> IO(T.Session):
  match r:
    case T.Rejected{+why}:
      do IO<T.Session>:
        IO.print(String.append(tag, String.append(" REJECTED: ", why)))
        IO.pure(T.Session, T.Sess{cfg(), Nil{}, 0n, 0, None{}, None{}, Nil{}, Nil{}})
    case T.Accepted{obs, acts, next}:
      IO.pure(T.Session, next)

def pend_count(w: List<&2, T.Call>) -> U32:
  match w:
    case Nil{}: 0
    case Con{c, t}: U32.add(pend_count(t), 1)

def pends(or: T.Session) -> String:
  match or:
    case T.Sess{_, _, _, _, _, Some{T.PTools{+w}}, _, _}:
      String.append("PTools n=", U32.show(pend_count(w)))
    case _:
      "other"

def hist(or: T.Session) -> List<&2, T.Msg>:
  match or:
    case T.Sess{cdg, history, caps, acts, turn, pending, queued, notifs}:
      history

def main() -> IO(Unit):
  do IO<Unit>:
    s0 : T.Session <- IO.pure(T.Session, T.new_session(cfg()))
    s1 : T.Session <- dump_res("user", S.apply(s0, T.UserMsg{T.Queue{}, "run a cmd"}))
    s2 : T.Session <- dump_res("call", S.apply(s1,
      T.CompletionOk{0, "", Con{T.Call{1, "bash", "printf a"}, Nil{}}}))
    +s3 : T.Session <- dump_res("tool", S.apply(s2, T.ToolOk{2, "simple single line"}))
    IO.print(String.append("pending: ", pends(s3)))
    IO.print("=== history texts ===")
    +h : List<&2, T.Msg> <- IO.pure(List<&2, T.Msg>, hist(s3))
    IO.print(Rem.ser_msgs(h))
    IO.print("=== wire ===")
    +wire : String <- IO.pure(String, Rem.build_request(cfg(), h))
    IO.print(wire)
    IO.print("=== api body ===")
    +body : String <- IO.pure(String,
      Oai.api_body("m", W.parse_wire(wire)))
    IO.print(body)
    IO.pure(Unit, Unit{})
