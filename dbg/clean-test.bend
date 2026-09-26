import Base
import ../runtime/main.bend as Rt

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print("cleanup test")
    u : Unit <- Rt.bash_cleanup("9914")
    IO.print("done")
    IO.pure(Unit, Unit{})
