import Base
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap

def main() -> IO(Unit):
  do IO<Unit>:
    o1 : String <- Snap.run(Con{"/bin/sh", Con{"-c",
      Con{"echo OUT; echo ERR >&2; exit 42", Nil{}}}})
    u : Unit <- IO.print(String.append("[out1=", String.append(o1, "]")))
    o2 : String <- Snap.run(Con{"/bin/sh", Con{"-c",
      Con{"echo OUT2; exit 0", Nil{}}}})
    u2 : Unit <- IO.print(String.append("[out2=", String.append(o2, "]")))
    IO.pure(Unit, Unit{})
