import Base
import ../core/types.bend as T
import ../runtime/main.bend as Rt
import ../runtime/bash.bend as Sh
import ../runtime/tools.bend as X

def show_res.ok(+tag: String, +ok: Bool, +out: String) -> IO(Unit):
  IO.print(String.append(tag, String.append(" ok=", String.append(Bool.show(ok), String.append(" out=[", String.append(out, "]"))))))

def show_res.body(+tag: String, +r: X.ExecRes) -> IO(Unit):
  match r:
    case X.Ex{+ok, +out}:
      show_res.ok(tag, ok, out)

def show_res(tag: String, +r: X.ExecRes) -> IO(Unit):
  show_res.body(tag, r)

def main() -> IO(Unit):
  do IO<Unit>:
    r : X.ExecRes <- Sh.bash_exec("echo sync-ok")
    u1 : Unit <- show_res("SYNC", r)
    r2 : X.ExecRes <- Sh.bash_exec("read x; echo got:$x")
    u2 : Unit <- show_res("HANDOFF", r2)
    IO.pure(Unit, Unit{})
