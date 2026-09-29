# the live V8 path: a program that files self.reload() but returns a
# CONSTANT — the accumulator must still carry the ack in the result
import Base
import ../core/wire.bend as W
import ../runtime/main.bend as Rt
import ../runtime/selftools.bend as Sf
import ../runtime/selftools-pure.bend as Sfp
import ../runtime/tools.bend as X
import ../runtime/tools-pure.bend as Xp

def res_out(+r: Xp.ExecRes) -> String:
  match r:
    case Xp.Ex{+ok, +out}:
      String.append(Bool.show(ok), String.append("|", out))

def pair_res(pr: Rt.Out & Xp.ExecRes) -> Xp.ExecRes:
  match pr:
    case (_, r):
      r

def main() -> IO(Unit):
  do IO<Unit>:
    pr : Rt.Out & Xp.ExecRes <- Rt.exec_program(1000n,
      (Rt.OStd{}, (Sfp.def_none(), Rt.EpRun{})),
      W.call_code("const r = self.reload(); return 'done';"),
      Nil{}, True{})
    +r : Xp.ExecRes <- IO.pure(Xp.ExecRes, pair_res(pr))
    u : Unit <- IO.print(String.append("V8-RESULT=[", String.append(res_out(r), "]\n")))
    IO.pure(Unit, Unit{})
