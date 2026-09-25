# debug-http.bend — check the bootstrap fetch: status + body length + prefix
import Base
import runtime/persist.bend as P
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http

def env_or(r: Result<&1, &1, U32 & String, String>, +dflt: String) -> String:
  match r:
    case Done{+s}:
      s
    case Fail{_}:
      dflt

def show_res.done(+res: Http.Res) -> IO(Unit):
  match res:
    case Http.Res{+status, _, _}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("[dbg] status=", U32.show(status)))
        u2 : Unit <- IO.print(String.append("[dbg] body len=",
          Nat.show(String.length(Http.text(res)))))
        u3 : Unit <- IO.print(String.append("[dbg] body prefix=<",
          String.append(String.take(Http.text(res), 120n), ">")))
        IO.pure(Unit, Unit{})

def show_res(r: Result<&2, &2, Http.Err, Http.Res>) -> IO(Unit):
  match r:
    case Fail{+e}:
      IO.print("[dbg] fetch FAILED")
    case Done{+res}:
      show_res.done(res)

def main() -> IO(Unit):
  do IO<Unit>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +h : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
      Http.set(Http.empty(), "Authorization",
        String.append("Bearer ", env_or(kr, ""))))
    r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch("GET",
      "https://api.mistral.ai/v1/connectors/bootstrap?supports_mcp=true",
      h, "")
    u : Unit <- show_res(r)
    IO.pure(Unit, Unit{})
