# runtime/oneshot-pure.bend — the one-shot call's pure part (BISE-126).
#
# A one-shot call (repl-live with BISE_ONESHOT set, runtime/oneshot.bend)
# is a short label, not a conversation: its body goes out "light". The
# family bodies (core/api.bend) ask for adaptive thinking / effort high /
# reasoning_effort high on every call; a small model may refuse them
# (Haiku 4.5: "adaptive thinking is not supported on this model") and a
# label needs none. light() drops those keys from the top-level object;
# anything else (a body that is not an object) goes out unchanged.
import Base
import ../vendor/json.bend as J
import ../core/wire.bend as W

def heavy_key(+k: String) -> Bool:
  Bool.or(String.eq(k, "thinking"),
    Bool.or(String.eq(k, "output_config"), String.eq(k, "reasoning_effort")))

def light.put(drop: Bool, kv: Sigma<&2, &2, String, _ => J.Json>,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match drop:
    case True{}:
      rest
    case False{}:
      Con{kv, rest}

def light.kv(kv: Sigma<&2, &2, String, _ => J.Json>,
    +rest: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match kv:
    case Tuple{+k, v}:
      light.put(heavy_key(k), J.kv(k, v), rest)

def light.kvs(kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match kvs:
    case Nil{}:
      Nil{}
    case Con{kv, rest}:
      light.kv(kv, light.kvs(rest))

def light.json(j: J.Json, +body: String) -> String:
  match j:
    case J.JObj{+kvs}:
      J.stringify(J.JObj{light.kvs(kvs)})
    case _:
      body

def light(+body: String) -> String:
  light.json(W.parse_json(body), body)

# BISE_ONESHOT set (non-empty): the light body; else the body as is
def light_if(oneshot: Bool, +body: String) -> String:
  match oneshot:
    case True{}:
      light(body)
    case False{}:
      body
