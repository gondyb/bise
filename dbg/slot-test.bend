import Base
import ../runtime/main.bend as Rt

def show(+r: Rt.ExecRes) -> IO(Unit):
  match r:
    case Rt.Ex{+ok, +out}:
      IO.print(String.append("ok=", String.append(Bool.show(ok), String.append(" out=[", String.append(out, "]")))))

def main() -> IO(Unit):
  do IO<Unit>:
    # handoff, then the process dies on its own; the slot must free itself
    r : Rt.ExecRes <- Rt.bash_exec("sleep 3 && echo done1")
    u1 : Unit <- show(r)
    IO.sleep(2000)
    r2 : Rt.ExecRes <- Rt.bash_exec("echo slot-reuse")
    u2 : Unit <- show(r2)
    IO.pure(Unit, Unit{})
