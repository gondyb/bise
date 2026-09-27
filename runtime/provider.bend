# runtime/provider.bend — the live provider call.
#
# The provider table (model -> API style, URL, key, headers), the HTTPS
# call through the hub HTTP client, the retry policy for transient
# failures, and the reply rendered as the OK/CALL/END lines
# runtime/remote.bend parses.
import Base
import ../core/api.bend as Api
import ./persist.bend as P
import ./remote.bend as Rem
import ../core/config.bend as Cf
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http
import 0xbf477e663cf4acb1369a68e0f0fa713b/encoding/encoding.bend as Enc
import ./settings.bend as St
# ---- the live provider call, fully in Bend (hub HTTP client) ----
# The wire request (MODEL/TOOL/MSG/CALL lines) becomes the
# OpenAI-compatible JSON in core/api.bend, is POSTed to api.mistral.ai
# by the hub HTTP client, and the reply maps back through core/api.bend
# into the OK/CALL/END string remote.bend parses. No bridge process.

def api_err(e: Http.Err) -> String:
  match e:
    case Http.ErrUrl{}:
      "bad url"
    case Http.ErrDns{}:
      "dns"
    case Http.ErrConnect{+c, +w}:
      String.append("connect ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrTls{+c, +w}:
      String.append("tls ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrRead{+c, +w}:
      String.append("read ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrWrite{+c, +w}:
      String.append("write ",
        String.append(U32.show(c), String.append(" ", w)))
    case Http.ErrTimeout{}:
      "timeout"
    case Http.ErrRedirect{}:
      "too many redirects"
    case Http.ErrBad{}:
      "bad response"

# BEND_WIRE_DUMP=<path>: write the last provider request body (debug)
def dump_wire.write(+path: String, +body: String) -> IO(Unit):
  do IO<Unit>:
    u : Unit <- P.write_file(path, body)
    IO.pure(Unit, Unit{})

def dump_wire.skip(empty: Bool, +path: String, +body: String) -> IO(Unit):
  match empty:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      dump_wire.write(path, body)

def dump_wire(+path: String, +body: String) -> IO(Unit):
  dump_wire.skip(String.is_empty(path), path, body)

def model_call.err(e: Http.Err) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    IO.pure(Rem.RemoteRes,
      Rem.RRErr{String.append("provider unreachable: ", api_err(e))})

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

def model_key_env.pick(is_anth: Bool) -> String:
  match is_anth:
    case True{}:
      "ANTHROPIC_FOUNDRY_API_KEY"
    case False{}:
      "MISTRAL_API_KEY"

def model_key_env(st: Api.Style) -> String:
  model_key_env.pick(Api.is_anthropic(st))

def oai_headers(+key: String) -> Map<&2, List<&2, String>>:
  Http.set(Http.set(Http.empty(), "Authorization",
      String.append("Bearer ", key)),
    "Content-Type", "application/json")

def anth_headers(+key: String) -> Map<&2, List<&2, String>>:
  Http.set(Http.set(Http.set(Http.set(Http.empty(),
      "x-api-key", key),
    "anthropic-version", "2023-06-01"),
  "anthropic-beta",
  "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"),
  "Content-Type", "application/json")

def model_headers.pick(is_anth: Bool, +key: String) -> Map<&2, List<&2, String>>:
  match is_anth:
    case True{}:
      anth_headers(key)
    case False{}:
      oai_headers(key)

def model_headers(st: Api.Style, +key: String) -> Map<&2, List<&2, String>>:
  model_headers.pick(Api.is_anthropic(st), key)

def model_call.reply(st: Api.Style, +res: Http.Res) -> String:
  match res:
    case Http.Res{+status, _, _}:
      Api.reply_of_for(st, status, Http.text(res))

# BEND_WIRE_DUMP also captures the raw provider response (suffix .reply):
# the tool-call arguments exactly as the provider serialized them,
# before any harness parsing - the ground truth for wire-encoding bugs.
def model_call.dump_res(+path: String, +body: String) -> IO(Unit):
  do IO<Unit>:
    u : Unit <- P.write_file(String.append(path, ".reply"), body)
    IO.pure(Unit, Unit{})

def model_call.res.dumper(m: Result<&1, &1, U32 & String, String>, +res: Http.Res)
    -> IO(Unit):
  match m:
    case Fail{_}:
      IO.pure(Unit, Unit{})
    case Done{+path}:
      model_call.dump_res(path, Http.text(res))

def model_call.res(+res: Http.Res, st: Api.Style) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    dw2 : Unit <- P.write_file("/tmp/ap-reply-dump.txt", Http.text(res))
    dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
    d : Unit <- model_call.res.dumper(dr, res)
    reply : String <- IO.pure(String, model_call.reply(st, res))
    IO.pure(Rem.RemoteRes, Rem.parse_reply(reply))

# transient failures get another chance: connection loss, DNS, timeouts
# and server-side statuses (5xx, 429) retry with a pause; everything else
# (4xx auth/validation) fails the candidate immediately. The outcome is
# classified PURELY into an Attempt; the retry goes through a
# continuation closure that self-references the loop (the exec_done
# idiom — mutual recursion between defs is not expressible).
type Attempt is Data:
  AOk{res: Http.Res}
  AFail{e: Http.Err}
  ARetry{after_ms: U32, why: String}

def retryable_err(e: Http.Err) -> Bool:
  match e:
    case Http.ErrConnect{_, _}:
      True{}
    case Http.ErrDns{}:
      True{}
    case Http.ErrTimeout{}:
      True{}
    case _:
      False{}

def retryable_status(+s: U32) -> Bool:
  Bool.or(U32.is_le(500, s), U32.is_eq(s, 429))

def attempt_res.status_of(+res: Http.Res) -> U32:
  match res:
    case Http.Res{+status, _, _}:
      status

def attempt_ok(retryable: Bool, +res: Http.Res) -> Attempt:
  match retryable:
    case True{}:
      ARetry{1500, String.append("provider ",
        String.append(U32.show(attempt_res.status_of(res)), " (transient)"))}
    case False{}:
      AOk{res}

def attempt_res(+res: Http.Res) -> Attempt:
  attempt_ok(retryable_status(attempt_res.status_of(res)), res)

def attempt_fail(retryable: Bool, +e: Http.Err) -> Attempt:
  match retryable:
    case True{}:
      ARetry{1500, api_err(e)}
    case False{}:
      AFail{e}

def attempt_fail2(+e: Http.Err) -> Attempt:
  attempt_fail(retryable_err(e), e)

def model_call.attempt_result(r: Result<&2, &2, Http.Err, Http.Res>)
    -> Attempt:
  match r:
    case Done{+res}:
      attempt_res(res)
    case Fail{+e}:
      attempt_fail2(e)

def interrupt_path_of(+port: String) -> String:
  String.append("/tmp/bend-interrupt-", String.append(port, ".txt"))

def interrupt_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, interrupt_path_of(p))

# BR-005: a Ctrl+C landing during a failed attempt must not wait for
# the remaining retries: the flag is read between attempts, and when
# set the call bails out - the completion_fail path then fails the
# turn cleanly instead of another 2-minute round trip.
def model_call.step.retry.interrupted(set: Bool, +ipath: String,
    +k: Nat, +after_ms: U32,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match set:
    case True{}:
      do IO<Rem.RemoteRes>:
        # consume the flag so the boundary drains stay clean
        u : Unit <- P.write_file(ipath, "")
        IO.pure(Rem.RemoteRes, Rem.RRErr{"interrupted by the user"})
    case False{}:
      do IO<Rem.RemoteRes>:
        IO.sleep(after_ms)
        next(k)

def model_call.step.retry.flag(m: Maybe<&2, String>, +ipath: String,
    +k: Nat, +after_ms: U32,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match m:
    case None{}:
      do IO<Rem.RemoteRes>:
        IO.sleep(after_ms)
        next(k)
    case Some{+c}:
      model_call.step.retry.interrupted(String.is_empty(c), ipath, k,
        after_ms, next)

def model_call.step.retry(can: Bool, +k: Nat, +after_ms: U32, +why: String,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match can:
    case True{}:
      do IO<Rem.RemoteRes>:
        +ipath : String <- interrupt_path()
        m : Maybe<&2, String> <- P.read_file(ipath)
        model_call.step.retry.flag(m, ipath, k, after_ms, next)
    case False{}:
      IO.pure(Rem.RemoteRes, Rem.RRErr{why})

def model_call.step(a: Attempt, +k: Nat, st: Api.Style,
    next: Nat -> IO(Rem.RemoteRes)) -> IO(Rem.RemoteRes):
  match a:
    case AOk{+res}:
      model_call.res(res, st)
    case AFail{+e}:
      model_call.err(e)
    case ARetry{+after_ms, +why}:
      model_call.step.retry(Nat.is_lt(0n, k), k, after_ms, why, next)

@unsafe
def model_call.try(fuel: Nat, +request: String, +key: String,
    +st: Api.Style, +model: String) -> IO(Rem.RemoteRes):
  match fuel:
    case 0n:
      IO.pure(Rem.RemoteRes,
        Rem.RRErr{"provider unreachable: retries exhausted"})
    case 1n+k:
      do IO<Rem.RemoteRes>:
        +body : String <- IO.pure(String,
          Enc.utf8.encode(Api.api_body_for(st, model,
            Api.parse_wire(request))))
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        dump_dbg : Unit <- dump_wire(P.env_or(dr, ""), body)
        hdrs : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
          model_headers(st, key))
        r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch.with("POST",
          model_url(st), hdrs, body, 120000)
        +a : Attempt <- IO.pure(Attempt, model_call.attempt_result(r))
        model_call.step(a, k, st,
          k2 => model_call.try(k2, request, key, st, model))

# three attempts total: the first try plus two retries
def model_call.go(+request: String, +key: String, +st: Api.Style,
    +model: String) -> IO(Rem.RemoteRes):
  model_call.try(3n, request, key, st, model)

def model_call.key(empty: Bool, +request: String, +key: String,
    +st: Api.Style, +model: String) -> IO(Rem.RemoteRes):
  match empty:
    case True{}:
      IO.pure(Rem.RemoteRes,
        Rem.RRErr{String.append(model_key_env(st), " is not set")})
    case False{}:
      model_call.go(request, key, st, model)

# THE model resolution - the one function every surface uses: the
# provider call below AND the harness-info line the clients (TUI,
# bend_client) display. BEND_MODEL > config.toml model > default;
# aliases resolve through the provider table.
def resolved_model() -> IO(String):
  do IO<String>:
    mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
    +cfgt : String <- St.read_cfg()
    IO.pure(String,
      resolve_model(Cf.env_cfg_or(P.env_or(mr, ""),
        Cf.cfg_get(cfgt, "model"), "claude-opus-5-5")))

@unsafe
def model_call(+request: String) -> IO(Rem.RemoteRes):
  do IO<Rem.RemoteRes>:
    # the provider table: the resolved model carries the style, the
    # api_base and the key env
    +model : String <- resolved_model()
    +st : Api.Style <- IO.pure(Api.Style, model_style(model))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env(model_key_env(st))
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    model_call.key(String.is_empty(key), request, key, st, model)
