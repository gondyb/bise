# test-details.bend — checks the details paths of search_tool_functions.
#   bend test-details.bend -o /tmp/test-details && /tmp/test-details
import Base
import ./core/types.bend as T
import ./core/discovery.bend as D
import ./runtime/tools.bend as X

def show(label: String, +r: String) -> IO(Unit):
  do IO<Unit>:
    IO.print(String.append("\n[", String.append(label, String.append("]\n  ",
      String.append(String.join(String.split(r, '\n'), " | "), "\n----")))))

def main() -> IO(Unit):
  do IO<Unit>:
    # a merged-like catalog: the short catalog + a fake index entry
    +cat : List<&2, T.ToolDef> <- IO.pure(List<&2, T.ToolDef>,
      List.append(&2, T.ToolDef, X.catalog_short(),
        Con{T.ToolDef{"github_app.list_issues",
          "#List issues. | input: {\"owner\": {}, \"repo\": {}}"}, Nil{}}))
    u1 : Unit <- show("details found", X.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"github_app.list_issues\"]}", cat))
    u2 : Unit <- show("details two found", X.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"github_app.list_issues\", \"search_tool_functions\"]}", cat))
    u3 : Unit <- show("details missing only", X.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"no.such_tool\"]}", cat))
    u4 : Unit <- show("details found+missing", X.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"github_app.list_issues\", \"no.such_tool\"]}", cat))
    u5 : Unit <- show("details functionNames alias", X.search_reply(
      "{\"mode\": \"details\", \"functionNames\": [\"search_tool_functions\"]}", cat))
    u6 : Unit <- show("details empty functions", X.search_reply(
      "{\"mode\": \"details\"}", cat))
    u7 : Unit <- show("details with query only", X.search_reply(
      "{\"mode\": \"details\", \"query\": \"github_app.list_issues\"}", cat))
    u8 : Unit <- show("details non-array functions", X.search_reply(
      "{\"mode\": \"details\", \"functions\": \"github_app.list_issues\"}", cat))
    u9 : Unit <- show("details mixed array", X.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"github_app.list_issues\", 42]}", cat))
    u10 : Unit <- show("flat details", X.search_reply(
      "details: search_tool_functions", cat))
    u11 : Unit <- show("flat details missing", X.search_reply(
      "details: no_such_tool", cat))
    u12 : Unit <- show("flat unknown syntax", X.search_reply(
      "hello", cat))
    u13 : Unit <- show("empty object args", X.search_reply("{}", cat))
    IO.pure(Unit, Unit{})
