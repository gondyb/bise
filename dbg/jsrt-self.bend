# the live V8 path: a program that files self.reload() but returns a
# CONSTANT — the accumulator must still carry the ack in the result
import Base
import ../core/api.bend as Api
import ../runtime/main.bend as Rt
import ../runtime/selftools.bend as Sf
import ../runtime/tools.bend as X

def res_out(+r: X.ExecRes) -> String:
  match r:
    case X.Ex{+ok, +out}:
      String.append(Bool.show(ok), String.append("|", out))

def pair_res(pr: Rt.Out & X.ExecRes) -> X.ExecRes:
  match pr:
    case (_, r):
      r

def main() -> IO(Unit):
  do IO<Unit>:
    pr : Rt.Out & X.ExecRes <- Rt.exec_program(1000n,
      (Rt.OStd{}, (Sf.def_none(), Rt.EpRun{})),
      Api.call_code("const r = self.reload(); return 'done';"),
      Nil{}, True{})
    +r : X.ExecRes <- IO.pure(X.ExecRes, pair_res(pr))
    u : Unit <- IO.print(String.append("V8-RESULT=[", String.append(res_out(r), "]\n")))
    IO.pure(Unit, Unit{})
