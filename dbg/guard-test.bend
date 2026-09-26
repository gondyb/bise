import Base
import ../runtime/main.bend as Rt

def show(+r: Rt.ExecRes) -> IO(Unit):
  match r:
    case Rt.Ex{+ok, +out}:
      IO.print(String.append("ok=", String.append(Bool.show(ok), String.append(" out=[", String.append(out, "]")))))

def main() -> IO(Unit):
  do IO<Unit>:
    # pre-create the bg dir for port 99, then REMOVE it mid-flight
    IO.print("A: command with the dir present")
    r : Rt.ExecRes <- Rt.bash_exec("echo present")
    u1 : Unit <- show(r)
    IO.print("B: dir removed between commands (guard must recreate)")
    u2 : Unit <- IO.pure(Unit, Unit{})
    r2 : Rt.ExecRes <- Rt.bash_exec("echo after-rm")
    u3 : Unit <- show(r2)
    IO.pure(Unit, Unit{})
