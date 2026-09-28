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
import ../vendor/json.bend as J

def usage_line.fields(+inp: Nat, +out: Nat, +cr: Nat, +cw: Nat,
    +model: String) -> String:
  String.concat(["  obs: usage: model=", model, " in=", Nat.show(inp),
    " out=", Nat.show(out), " cache_read=", Nat.show(cr),
    " cache_write=", Nat.show(cw)])

# Anthropic Messages: input_tokens counts only the uncached part
def usage_line.anth(+u: J.Json, +model: String) -> String:
  +cr = J.get_num_or(u, "cache_read_input_tokens", 0n)
  +cw = J.get_num_or(u, "cache_creation_input_tokens", 0n)
  usage_line.fields(
    Nat.add(J.get_num_or(u, "input_tokens", 0n), Nat.add(cr, cw)),
    J.get_num_or(u, "output_tokens", 0n), cr, cw, model)

# OpenAI-compatible: prompt_tokens already counts the cached part
def usage_line.oai(+u: J.Json, +model: String) -> String:
  usage_line.fields(J.get_num_or(u, "prompt_tokens", 0n),
    J.get_num_or(u, "completion_tokens", 0n),
    J.get_num_or(Api.field_or(u, "prompt_tokens_details", J.JNull{}),
      "cached_tokens", 0n),
    0n, model)

def usage_line.style(st: Api.Style, +u: J.Json, +model: String) -> String:
  match st:
    case Api.OpenAI{}:
      usage_line.oai(u, model)
    case Api.Anthropic{}:
      usage_line.anth(u, model)

def usage_line.usage(u: J.Json, st: Api.Style, +model: String) -> String:
  match u:
    case J.JObj{+kvs}:
      usage_line.style(st, J.JObj{kvs}, model)
    case _:
      ""

def usage_line.ok(ok: Bool, st: Api.Style, +model: String, +body: String)
    -> String:
  match ok:
    case True{}:
      usage_line.usage(
        Api.field_or(Api.parse_json(body), "usage", J.JNull{}), st, model)
    case False{}:
      ""

def usage_line(st: Api.Style, +model: String, +status: U32, +body: String)
    -> String:
  usage_line.ok(U32.is_eq(status, 200), st, model, body)
