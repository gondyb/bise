import Base
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J

def show(r: Result<&2, &2, String, J.Json>) -> String:
  match r:
    case Fail{+e}:
      String.append("FAIL: ", e)
    case Done{+j}:
      "OK"

def main() -> IO(Unit):
  do IO<Unit>:
    u : Unit <- IO.print(String.append("Nat.read.max()=",
      Nat.show(Nat.read.max())))
    u2 : Unit <- IO.print(String.append("parse 9007199254740991: ",
      show(J.parse("{\"m\":9007199254740991}"))))
    u3 : Unit <- IO.print(String.append("parse 1727122965001469: ",
      show(J.parse("{\"m\":1727122965001469}"))))
    u4 : Unit <- IO.print(String.append("parse -122.4194: ",
      show(J.parse("{\"m\":-122.4194}"))))
    IO.pure(Unit, Unit{})
