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

def model_url(st: Api.Style) -> String:
  match st:
    case Api.Anthropic{}:
      "https://foundry-proxy.cheetah-koi.ts.net/anthropic/v1/messages"
    case Api.OpenAI{}:
      "https://api.mistral.ai/v1/chat/completions"

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

def model_key_env(st: Api.Style) -> String:
  match st:
    case Api.Anthropic{}:
      "ANTHROPIC_FOUNDRY_API_KEY"
    case Api.OpenAI{}:
      "MISTRAL_API_KEY"

def interrupt_path_of(+port: String) -> String:
  String.append("/tmp/bend-interrupt-", String.append(port, ".txt"))

# the interrupt flag file: the TUI writes "1" on Ctrl+C, the runtime
# clears it to "" at every turn end. Only a non-empty flag is a request
# (an empty one read as a Ctrl+C turned every retry after the first
# turn into "interrupted by the user").
def interrupt_requested(+c: String) -> Bool:
  Bool.not(String.is_empty(String.trim(c)))

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

# ---- what a failure means for the user ----
# The short code (api_err) is kept inside the text: it is what a bug
# report needs. The text around it names the host the call went to and
# the one thing the user can check. English, one line.

# "https://host:port/path" -> "host:port"; anything else is itself
def url_host.head(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, _}:
      h

def url_host.rest(+rest: String) -> String:
  url_host.head(String.split(rest, '/'))

def url_host.http(is_http: Bool, +url: String) -> String:
  match is_http:
    case True{}:
      url_host.rest(String.drop(url, 7n))
    case False{}:
      url_host.rest(url)

def url_host.pick(is_https: Bool, +url: String) -> String:
  match is_https:
    case True{}:
      url_host.rest(String.drop(url, 8n))
    case False{}:
      url_host.http(String.starts_with(url, "http://"), url)

def url_host(+url: String) -> String:
  url_host.pick(String.starts_with(url, "https://"), url)

def check_network() -> String:
  " — check your network or VPN"

def net_cause(+host: String, e: NetErr) -> String:
  match e:
    case NUrl{}:
      String.concat(["bad provider URL (", host,
        ") — check BEND_PROVIDER_URL or the provider table"])
    case NDns{}:
      String.concat(["cannot resolve ", host, " (dns)", check_network()])
    case NConnect{+c, +w}:
      String.concat(["cannot reach ", host, " (", coded("connect ", c, w),
        ")", check_network()])
    case NTls{+c, +w}:
      String.concat(["TLS handshake with ", host, " failed (",
        coded("tls ", c, w), ")", check_network()])
    case NRead{+c, +w}:
      String.concat(["connection to ", host, " lost while reading (",
        coded("read ", c, w), ")", check_network()])
    case NWrite{+c, +w}:
      String.concat(["connection to ", host, " lost while sending (",
        coded("write ", c, w), ")", check_network()])
    case NTimeout{}:
      # the limit depends on the call: stream_step_ms between two
      # events of a stream, step_timeout_ms for a whole reply
      String.concat(["no answer from ", host, " (timeout)"])
    case NRedirect{}:
      String.concat(["too many redirects from ", host])
    case NBad{}:
      String.concat(["garbled response from ", host])

# a final HTTP error: the status and the body the style mapper kept,
# plus what to check when the status says it
def status_hint(+s: U32) -> String:
  Bool.pick(String, Bool.or(U32.is_eq(s, 401), U32.is_eq(s, 403)),
    " — check the API key",
    Bool.pick(String, U32.is_eq(s, 404),
      " — check the model name and the provider URL",
      Bool.pick(String, U32.is_eq(s, 413), " — the request is too large",
        "")))

# ---- the interrupt flag, as the retry loop reads it ----
# The flag file is CLEARED by writing "" (every turn end does it), so an
# existing empty file is the normal state: only content means "stop"
# (interrupt_requested, the one predicate). No file: no request.
def interrupt_flag(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+c}:
      interrupt_requested(c)

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

# how long one read may wait for data. A non-streaming reply sends
# NOTHING until the whole generation is done: measured on opus-5.5, a
# 32k-token reply arrives after 255 s (first byte = last byte). At
# 120 s every long reply timed out, and the retries replayed the same
# doomed request. 600 s is Anthropic's own non-streaming ceiling.
def step_timeout_ms() -> U32:
  600000

# a streamed reply keeps data coming (the first event in ~1 s, the
# largest gap measured over a 100 s opus-5.5 reply: 5 s), so a read
# that waits 90 s is a dead connection, not a long answer
def stream_step_ms() -> U32:
  90000

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

# a final HTTP error (not 200, not transient): the turn fails on its
# first line, with the hint for the status
def attempt_final(is_err: Bool, +status: U32, +hint: U32, +reply: String)
    -> Attempt:
  match is_err:
    case True{}:
      AFail{String.append(
        String.drop(first_line(String.split(String.trim(reply), Tx.newline())),
          6n),
        status_hint(status))}
    case False{}:
      attempt_body(String.starts_with(reply, "ERROR provider 200:"), hint,
        reply)

def attempt_status.pick(retryable: Bool, +status: U32, +hint: U32,
    +reply: String) -> Attempt:
  match retryable:
    case True{}:
      ARetry{hint, String.append("provider ",
        String.append(U32.show(status), " (transient)"))}
    case False{}:
      attempt_final(Bool.and(Bool.not(U32.is_eq(status, 200)),
          String.starts_with(reply, "ERROR provider ")),
        status, hint, reply)

# a response: its status, its Retry-After header, and the reply the
# style mapper made of its body
def attempt_status(+status: U32, +retry_after: String, +reply: String)
    -> Attempt:
  attempt_status.pick(retryable_status(status), status,
    retry_after_ms(retry_after), reply)

def attempt_err.pick(retryable: Bool, +host: String, +e: NetErr) -> Attempt:
  match retryable:
    case True{}:
      ARetry{0, net_cause(host, e)}
    case False{}:
      AFail{net_cause(host, e)}

# a transport failure: no response at all. The cause names the host and
# says what the user can do about it (the text every retry line, the
# final error and an interrupted retry loop carry)
def attempt_err(+host: String, +e: NetErr) -> Attempt:
  attempt_err.pick(retryable_err(e), host, e)

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
  String.append(String.concat(["provider failed after ", Nat.show(tries),
    " attempts: "]), why)

# an interrupt during the retries stops them, and keeps the error that
# made them necessary: the user sees both
def interrupted_text(+tries: Nat, +why: String) -> String:
  String.append(String.concat(["stopped retrying (interrupted by the user) after ",
    Nat.show(tries), " failed attempts; last error: "]), why)

# the pause before a retry is slept in 1 s slices, the flag read between
# them: how many slices a pause takes
def wait_slices(+ms: U32) -> Nat:
  U32.to_nat(U32.add(U32.div(ms, 1000), 1))

# one slice of the pause: nothing left or an interrupt -> 0
def slice_ms(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.min(left, 1000))

def left_after(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.sub(left, U32.min(left, 1000)))
