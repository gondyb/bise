import Base
import ./core/types.bend as T
import ./core/program.bend as P

def show(p: P.PR) -> String:
  match p:
    case P.PDone{+out}:
      String.append("DONE: ", String.take(out, 120n))
    case P.PWait{+name, +args}:
      String.append("WAIT: ", name)
    case P.PFail{+why}:
      String.append("FAIL: ", why)

def showp(p: Maybe<&2, List<&2, T.Stmt>>) -> String:
  match p:
    case None{}:
      "parse None"
    case Some{+ss}:
      String.append("parse ok len=", Nat.show(List.length(&2, T.Stmt, ss)))

def from_some(m: Maybe<&2, List<&2, T.Stmt>>) -> List<&2, T.Stmt>:
  match m:
    case Some{+ss}:
      ss
    case None{}:
      Nil{}

def main() -> IO(Unit):
  do IO<Unit>:
    +p1 : String <- IO.pure(String, "if (me && me.login) { return me.login; }")
    u : Unit <- IO.print(showp(P.parse(p1)))
    +p2 : String <- IO.pure(String, "if (me.login) { return me.login; }")
    u1 : Unit <- IO.print(showp(P.parse(p2)))
    +p3 : String <- IO.pure(String, "if (me && me.ok) { return me.login; } return 'failed';")
    u2 : Unit <- IO.print(showp(P.parse(p3)))
    +p4 : String <- IO.pure(String, "if (me.login) { return me.login; } return 'failed';")
    +json : String <- IO.pure(String, "{\"login\":\"gvergnaud\",\"id\":9265418}")
    u3 : Unit <- IO.print(show(P.run(from_some(P.parse(p4)), Con{json, Nil{}})))
    u4 : Unit <- IO.print(show(P.run(from_some(P.parse(p4)), Nil{})))
    IO.pure(Unit, Unit{})
