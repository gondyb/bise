# runtime/provider-pure.bend - the pure half of runtime/provider.bend:
# the provider table and the retry policy the laws pin. No IO, no
# @unsafe and no foreign code in its closure (the PROOF gate refuses any
# in the import closure since Bend 2.0.32) - so no Http types either:
# runtime/provider.bend maps Http.Err onto NetErr and a response onto
# (status, Retry-After, mapped reply) before asking for a verdict.
import Base
import ../core/api.bend as Api
import ../core/text.bend as Tx

# ---- the provider table (the vibe config.toml mirror) ----
# A provider carries an api_base, a key env and an api_style; a model
# points at one. The active model today is opus-5.5: alias
# "opus-5.5" -> name "claude-opus-5-5" on foundry-anthropic (the
# Messages API through the foundry proxy); the Mistral provider keeps
# the OpenAI-compatible mapping the laws pinned with GLM.

def resolve_model.pick(is_alias: Bool, +name: String) -> String:
  match is_alias:
    case True{}:
      "claude-opus-5-5"
    case False{}:
      name

def resolve_model(+name: String) -> String:
  resolve_model.pick(String.eq(name, "opus-5.5"), name)

def model_style.pick(is_claude: Bool) -> Api.Style:
  match is_claude:
    case True{}:
      Api.Anthropic{}
    case False{}:
      Api.OpenAI{}

def model_style(+model: String) -> Api.Style:
  model_style.pick(String.starts_with(model, "claude"))

def model_url.pick(is_anth: Bool) -> String:
  match is_anth:
    case True{}:
      "https://foundry-proxy.cheetah-koi.ts.net/anthropic/v1/messages"
    case False{}:
      "https://api.mistral.ai/v1/chat/completions"

def model_url(st: Api.Style) -> String:
  model_url.pick(Api.is_anthropic(st))

# BEND_PROVIDER_URL=<url> points the call elsewhere (a local fault
# injector in the tests); unset, the provider table decides
def provider_url.pick(empty: Bool, +env: String, st: Api.Style) -> String:
  match empty:
    case True{}:
      model_url(st)
    case False{}:
      env

def provider_url(+env: String, st: Api.Style) -> String:
  provider_url.pick(String.is_empty(env), env, st)

def model_key_env.pick(is_anth: Bool) -> String:
  match is_anth:
    case True{}:
      "ANTHROPIC_FOUNDRY_API_KEY"
    case False{}:
      "MISTRAL_API_KEY"

def model_key_env(st: Api.Style) -> String:
  model_key_env.pick(Api.is_anthropic(st))

def interrupt_path_of(+port: String) -> String:
  String.append("/tmp/bend-interrupt-", String.append(port, ".txt"))

# ---- a transport failure, without the Http types ----

type NetErr is Data:
  NUrl{}
  NDns{}
  NConnect{code: U32, what: String}
  NTls{code: U32, what: String}
  NRead{code: U32, what: String}
  NWrite{code: U32, what: String}
  NTimeout{}
  NRedirect{}
  NBad{}

def coded(+label: String, +c: U32, +w: String) -> String:
  String.append(label, String.append(U32.show(c), String.append(" ", w)))

def api_err(e: NetErr) -> String:
  match e:
    case NUrl{}:
      "bad url"
    case NDns{}:
      "dns"
    case NConnect{+c, +w}:
      coded("connect ", c, w)
    case NTls{+c, +w}:
      coded("tls ", c, w)
    case NRead{+c, +w}:
      coded("read ", c, w)
    case NWrite{+c, +w}:
      coded("write ", c, w)
    case NTimeout{}:
      "timeout"
    case NRedirect{}:
      "too many redirects"
    case NBad{}:
      "bad response"

# ---- the retry policy ----
# Nothing a provider answers may kill the turn at the first blow:
# connection loss (connect, DNS, timeout, a read/write cut mid-response,
# a garbled response), the transient statuses (408, 409, 425, 429 and
# every 5xx - 529 is Anthropic's "overloaded") and a 200 that carries an
# error body (the proxy answers overloads that way) all retry, up to
# max_attempts in total, with an exponential backoff (1s, 2s, 4s, 8s,
# 16s, then 30s) - or the server's Retry-After when it sends one
# (capped at 60s). Everything else (auth, validation, a bad URL, TLS)
# fails the turn immediately: retrying it only wastes the user's time.

# the verdict on one attempt: the mapped reply, a final failure, or a
# retry after a pause (0 = no server hint, the backoff decides)
type Attempt is Data:
  AOk{reply: String}
  AFail{why: String}
  ARetry{after_ms: U32, why: String}

def max_attempts() -> Nat:
  10n

def retryable_err(e: NetErr) -> Bool:
  match e:
    case NConnect{_, _}:
      True{}
    case NDns{}:
      True{}
    case NTimeout{}:
      True{}
    case NRead{_, _}:
      True{}
    case NWrite{_, _}:
      True{}
    case NBad{}:
      True{}
    case _:
      False{}

def retryable_status(+s: U32) -> Bool:
  Bool.or(U32.is_le(500, s),
    Bool.or(U32.is_eq(s, 429),
      Bool.or(U32.is_eq(s, 408),
        Bool.or(U32.is_eq(s, 409), U32.is_eq(s, 425)))))

# the Retry-After header in seconds -> ms, capped at 60s; absent or
# not a number -> 0 (the backoff decides)
def retry_after.of(m: Maybe<&2, U32>) -> U32:
  match m:
    case None{}:
      0
    case Some{+secs}:
      U32.min(U32.mul(secs, 1000), 60000)

def retry_after_ms(+h: String) -> U32:
  retry_after.of(U32.read(String.trim(h)))

# the pause before retry number n (0-based): 1s doubling up to 30s
def backoff_ms(n: Nat) -> U32:
  match n:
    case 0n:
      1000
    case 1n:
      2000
    case 2n:
      4000
    case 3n:
      8000
    case 4n:
      16000
    case _:
      30000

def retry_delay_ms.pick(no_hint: Bool, +hint: U32, n: Nat) -> U32:
  match no_hint:
    case True{}:
      backoff_ms(n)
    case False{}:
      hint

# the server's hint when it gave one, else the backoff
def retry_delay_ms(+hint: U32, n: Nat) -> U32:
  retry_delay_ms.pick(U32.is_eq(hint, 0), hint, n)

def first_line(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+l, _}:
      l

# a 200 whose body is an error ("ERROR provider 200: Overloaded") is a
# transient failure too
def attempt_body(is_err_body: Bool, +hint: U32, +reply: String) -> Attempt:
  match is_err_body:
    case True{}:
      ARetry{hint,
        String.drop(first_line(String.split(String.trim(reply), Tx.newline())),
          6n)}
    case False{}:
      AOk{reply}

def attempt_status.pick(retryable: Bool, +status: U32, +hint: U32,
    +reply: String) -> Attempt:
  match retryable:
    case True{}:
      ARetry{hint, String.append("provider ",
        String.append(U32.show(status), " (transient)"))}
    case False{}:
      attempt_body(String.starts_with(reply, "ERROR provider 200:"), hint,
        reply)

# a response: its status, its Retry-After header, and the reply the
# style mapper made of its body
def attempt_status(+status: U32, +retry_after: String, +reply: String)
    -> Attempt:
  attempt_status.pick(retryable_status(status), status,
    retry_after_ms(retry_after), reply)

def attempt_err.pick(retryable: Bool, +e: NetErr) -> Attempt:
  match retryable:
    case True{}:
      ARetry{0, api_err(e)}
    case False{}:
      AFail{String.append("provider unreachable: ", api_err(e))}

# a transport failure: no response at all
def attempt_err(+e: NetErr) -> Attempt:
  attempt_err.pick(retryable_err(e), e)

# the line the client shows while the call waits (the TUI renders it
# as a warning): "provider_retry: 2/10 · <why> · retry in 4s"
def retry_line(+n: Nat, +why: String, +delay_ms: U32) -> String:
  String.append("  obs: provider_retry: ",
    String.append(Nat.show(Nat.add(n, 1n)),
      String.append("/",
        String.append(Nat.show(max_attempts()),
          String.append(" · ",
            String.append(String.take(why, 200n),
              String.append(" · retry in ",
                String.append(U32.show(U32.div(delay_ms, 1000)), "s"))))))))

# the final error once every attempt failed: the count and the last cause
def exhausted_text(+tries: Nat, +why: String) -> String:
  String.append("provider failed after ",
    String.append(Nat.show(tries),
      String.append(" attempts: ", why)))
