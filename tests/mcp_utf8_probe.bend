# tests/mcp_utf8.py's probe: POSTs runtime/mcp.bend's call_body with a
# non-ASCII argument to $PROBE_URL and prints the HTTP status (or "fail").
import Base
import ../bend/runtime/mcp.bend as Mcp
import ../bend/vendor/http/http.bend as Http

def show(r: Http.Pool & Result<&2, &2, Http.Err, Http.Res>) -> String:
  (p, res) = r
  match res:
    case Fail{_}:
      "fail"
    case Done{+x}:
      match x:
        case Http.Res{+st, _, _}:
          U32.show(st)

def url_of(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{+u}:
      u
    case Fail{_}:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    ur : Result<&1, &1, U32 & String, String> <- IO.get_env("PROBE_URL")
    +body : String <- IO.pure(String, Mcp.call_body("act", "{\"locator\":{\"text_re\":\"25\\\\.0 s · paused\"},\"note\":\"élan ↖ 🙂\"}"))
    r : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(Http.pool.new(), "POST", url_of(ur), Http.empty(), body, 5000, Http.ModeFollow{})
    IO.print(show(r))
