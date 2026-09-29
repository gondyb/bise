# runtime/usage-pure.bend - the token usage of one provider reply.
#
# After each model call the REPL prints one feed line the clients read
# (the TUI shows the context fill of the agent from it):
#   "  obs: usage: model=M in=I out=O cache_read=R cache_write=W"
# in: every input token of the call (cached ones included) - the size
# of the context the model saw; out: the tokens it wrote. No usage in
# the reply (or an error status): no line (""). Pure: no IO.
import Base
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../vendor/json.bend as J

def usage_line.fields(+inp: Nat, +out: Nat, +cr: Nat, +cw: Nat,
    +model: String) -> String:
  String.concat(["  obs: usage: model=", model, " in=", Nat.show(inp),
    " out=", Nat.show(out), " cache_read=", Nat.show(cr),
    " cache_write=", Nat.show(cw)])

# the family reads its own usage object (core/api.bend, Api.usage_of)
def usage_line.of(u: Api.Usage, +model: String) -> String:
  match u:
    case Api.Usage{+inp, +out, +cr, +cw}:
      usage_line.fields(inp, out, cr, cw, model)

def usage_line.usage(u: J.Json, st: Api.Style, +model: String) -> String:
  match u:
    case J.JObj{+kvs}:
      usage_line.of(Api.usage_of(st, J.JObj{kvs}), model)
    case _:
      ""

# the body: a whole JSON reply carries "usage"; a stream (Server-Sent
# Events) spreads it over its events (the family's Api.stream_usage)
def usage_line.body(is_json: Bool, st: Api.Style, +body: String) -> J.Json:
  match is_json:
    case True{}:
      W.field_or(W.parse_json(body), "usage", J.JNull{})
    case False{}:
      Api.stream_usage(st, body)

def usage_line.ok(ok: Bool, +st: Api.Style, +model: String, +body: String)
    -> String:
  match ok:
    case True{}:
      usage_line.usage(
        usage_line.body(String.starts_with(String.trim(body), "{"), st, body),
        st, model)
    case False{}:
      ""

def usage_line(st: Api.Style, +model: String, +status: U32, +body: String)
    -> String:
  usage_line.ok(U32.is_eq(status, 200), st, model, body)
