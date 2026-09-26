# the live V8 path: a program that files self.reload() but returns a
# CONSTANT — the accumulator must still carry the ack in the result
import Base
import ../core/api.bend as Api
import ../runtime/main.bend as Rt

def res_out(+r: Rt.ExecRes) -> String:
  match r:
    case Rt.Ex{+ok, +out}:
      String.append(Bool.show(ok), String.append("|", out))

def pair_res(pr: Rt.Out & Rt.ExecRes) -> Rt.ExecRes:
  match pr:
    case (_, r):
      r

def main() -> IO(Unit):
  do IO<Unit>:
    pr : Rt.Out & Rt.ExecRes <- Rt.exec_program(1000n,
      (Rt.OStd{}, (Rt.def_none(), Rt.EpRun{})),
      Api.call_code("const r = self.reload(); return 'done';"),
      Nil{}, True{})
    +r : Rt.ExecRes <- IO.pure(Rt.ExecRes, pair_res(pr))
    u : Unit <- IO.print(String.append("V8-RESULT=[", String.append(res_out(r), "]\n")))
    IO.pure(Unit, Unit{})
