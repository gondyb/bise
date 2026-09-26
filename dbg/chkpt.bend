import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/checkpoint.bend as K
import ../runtime/remote.bend as Rem

def cfg() -> T.Cfg:
  T.Cfg{600n, 40n, 3n, "sys", Nil{}}

def dump_res(+r: T.Res) -> IO(T.Session):
  match r:
    case T.Rejected{+why}:
      IO.pure(T.Session, T.Sess{cfg(), Nil{}, 0n, 0, None{}, None{}, Nil{}, Nil{}})
    case T.Accepted{obs, acts, next}:
      IO.pure(T.Session, next)

def hist(or: T.Session) -> List<&2, T.Msg>:
  match or:
    case T.Sess{cdg, history, in, ac, tu, pe, qu, no}:
      history

def ck_of(m: Maybe<&2, K.Checkpoint>) -> K.Checkpoint:
  match m:
    case Some{c}: c
    case None{}: K.Chk{1, T.Sess{cfg(), Nil{}, 0n, 0, None{}, None{}, Nil{}, Nil{}}}

def main() -> IO(Unit):
  do IO<Unit>:
    s0 : T.Session <- IO.pure(T.Session, T.new_session(cfg()))
    s1 : T.Session <- dump_res(S.apply(s0, T.UserMsg{T.Queue{}, "run"}))
    s2 : T.Session <- dump_res(S.apply(s1,
      T.CompletionOk{0, "", Con{T.Call{1, "bash", "x"}, Nil{}}}))
    s3 : T.Session <- dump_res(S.apply(s2, T.ToolOk{2, "a\nb\nc"}))
    +txt : String <- IO.pure(String, K.to_text(K.export(s3)))
    IO.print(txt)
    +ck : Maybe<&2, K.Checkpoint> <- IO.pure(Maybe<&2, K.Checkpoint>, K.from_text(txt))
    +back : T.Session <- IO.pure(T.Session, K.restore(ck_of(ck)))

    +h2 : List<&2, T.Msg> <- IO.pure(List<&2, T.Msg>, hist(back))
    IO.print(Rem.ser_msgs(h2))
    IO.pure(Unit, Unit{})
