# runtime/provider.bend — the live provider call.
#
# The HTTPS call through the hub HTTP client, the retry loop, and the
# reply rendered as the OK/CALL/END lines runtime/remote.bend parses.
# The provider table and the retry policy are pure and live in
# runtime/provider-pure.bend (the laws pin them there); this module
# only maps the Http values onto that policy and runs the effects.
import Base
import ../core/api.bend as Api
import ./persist.bend as P
import ./remote.bend as Rem
import ../core/config.bend as Cf
import 0xbf477e663cf4acb1369a68e0f0fa713b/http.bend as Http
import 0xbf477e663cf4acb1369a68e0f0fa713b/encoding/encoding.bend as Enc
import ./settings.bend as St
import ./provider-pure.bend as Pvp
# ---- the live provider call, fully in Bend (hub HTTP client) ----
# The wire request (MODEL/TOOL/MSG/CALL lines) becomes the provider
# JSON in core/api.bend, is POSTed by the hub HTTP client, and the reply
# maps back through core/api.bend into the OK/CALL/END string remote.bend
# parses. No bridge process.

# the transport failure, as the pure policy sees it
def net_of(e: Http.Err) -> Pvp.NetErr:
  match e:
    case Http.ErrUrl{}:
      Pvp.NUrl{}
    case Http.ErrDns{}:
      Pvp.NDns{}
    case Http.ErrConnect{+c, +w}:
      Pvp.NConnect{c, w}
    case Http.ErrTls{+c, +w}:
      Pvp.NTls{c, w}
    case Http.ErrRead{+c, +w}:
      Pvp.NRead{c, w}
    case Http.ErrWrite{+c, +w}:
      Pvp.NWrite{c, w}
    case Http.ErrTimeout{}:
      Pvp.NTimeout{}
    case Http.ErrRedirect{}:
      Pvp.NRedirect{}
    case Http.ErrBad{}:
      Pvp.NBad{}

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

# a response, judged by the pure policy: its status, its Retry-After,
# and the reply the style mapper made of its body
def verdict_res(+res: Http.Res, st: Api.Style) -> Pvp.Attempt:
  match res:
    case Http.Res{+status, +h, _}:
      Pvp.attempt_status(status, Http.header(h, "retry-after"),
        Api.reply_of_for(st, status, Http.text(res)))

def model_call.verdict(r: Result<&2, &2, Http.Err, Http.Res>, st: Api.Style)
    -> IO(Pvp.Attempt):
  match r:
    case Done{+res}:
      do IO<Pvp.Attempt>:
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        d : Unit <- model_call.res.dumper(dr, res)
        IO.pure(Pvp.Attempt, verdict_res(res, st))
    case Fail{+e}:
      IO.pure(Pvp.Attempt, Pvp.attempt_err(net_of(e)))

# ---- where the retry notes go: the loop's output, carried through ----
# (runtime/main.bend's Out, mirrored: this module sits below main)

type Note is Type:
  NStd{}
  NNet{sock: Socket}

def note.pair(sm: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket:
  match sm:
    case (s2, _):
      s2

def note(n: Note, +line: String) -> IO(Note):
  match n:
    case NStd{}:
      do IO<Note>:
        IO.print(line)
        IO.pure(Note, NStd{})
    case NNet{sock}:
      do IO<Note>:
        sm : Socket & Result<&1, &1, U32 & String, Unit> <- TCP.send(sock,
          String.append(line, "\n"))
        s2 : Socket <- IO.pure(Socket, note.pair(sm))
        IO.pure(Note, NNet{s2})

def interrupt_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, Pvp.interrupt_path_of(p))

# BR-005: a Ctrl+C landing during a failed attempt must not wait for
# the remaining retries: the flag is read between attempts, and when
# set the call bails out - the completion_fail path then fails the
# turn cleanly instead of another 2-minute round trip.
def model_call.step.retry.interrupted(set: Bool, n: Note, +ipath: String,
    +k: Nat, +after_ms: U32,
    next: Nat -> Note -> IO(Note & Rem.RemoteRes)) -> IO(Note & Rem.RemoteRes):
  match set:
    case True{}:
      do IO<Note & Rem.RemoteRes>:
        # consume the flag so the boundary drains stay clean
        u : Unit <- P.write_file(ipath, "")
        IO.pure(Note & Rem.RemoteRes, (n, Rem.RRErr{"interrupted by the user"}))
    case False{}:
      do IO<Note & Rem.RemoteRes>:
        IO.sleep(after_ms)
        next(k)(n)

def model_call.step.retry.flag(m: Maybe<&2, String>, n: Note, +ipath: String,
    +k: Nat, +after_ms: U32,
    next: Nat -> Note -> IO(Note & Rem.RemoteRes)) -> IO(Note & Rem.RemoteRes):
  match m:
    case None{}:
      do IO<Note & Rem.RemoteRes>:
        IO.sleep(after_ms)
        next(k)(n)
    case Some{+c}:
      model_call.step.retry.interrupted(String.is_empty(c), n, ipath, k,
        after_ms, next)

def model_call.step.retry(can: Bool, n: Note, +k: Nat, +tries: Nat,
    +delay_ms: U32, +why: String,
    next: Nat -> Note -> IO(Note & Rem.RemoteRes)) -> IO(Note & Rem.RemoteRes):
  match can:
    case True{}:
      do IO<Note & Rem.RemoteRes>:
        n2 : Note <- note(n, Pvp.retry_line(tries, why, delay_ms))
        +ipath : String <- interrupt_path()
        m : Maybe<&2, String> <- P.read_file(ipath)
        model_call.step.retry.flag(m, n2, ipath, k, delay_ms, next)
    case False{}:
      IO.pure(Note & Rem.RemoteRes,
        (n, Rem.RRErr{Pvp.exhausted_text(Nat.add(tries, 1n), why)}))

def model_call.step(a: Pvp.Attempt, n: Note, +k: Nat, +tries: Nat,
    next: Nat -> Note -> IO(Note & Rem.RemoteRes)) -> IO(Note & Rem.RemoteRes):
  match a:
    case Pvp.AOk{+reply}:
      IO.pure(Note & Rem.RemoteRes, (n, Rem.parse_reply(reply)))
    case Pvp.AFail{+why}:
      IO.pure(Note & Rem.RemoteRes, (n, Rem.RRErr{why}))
    case Pvp.ARetry{+after_ms, +why}:
      model_call.step.retry(Nat.is_lt(0n, k), n, k, tries,
        Pvp.retry_delay_ms(after_ms, tries), why, next)

@unsafe
def model_call.try(fuel: Nat, +tries: Nat, n: Note, +request: String,
    +key: String, +st: Api.Style, +model: String)
    -> IO(Note & Rem.RemoteRes):
  match fuel:
    case 0n:
      IO.pure(Note & Rem.RemoteRes,
        (n, Rem.RRErr{"provider unreachable: retries exhausted"}))
    case 1n+k:
      do IO<Note & Rem.RemoteRes>:
        +body : String <- IO.pure(String,
          Enc.utf8.encode(Api.api_body_for(st, model,
            Api.parse_wire(request))))
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        dump_dbg : Unit <- dump_wire(P.env_or(dr, ""), body)
        hdrs : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
          model_headers(st, key))
        ur : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_PROVIDER_URL")
        r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch.with("POST",
          Pvp.provider_url(P.env_or(ur, ""), st), hdrs, body, Pvp.step_timeout_ms())
        a : Pvp.Attempt <- model_call.verdict(r, st)
        model_call.step(a, n, k, tries,
          k2 => n2 => model_call.try(k2, Nat.add(tries, 1n), n2, request, key,
            st, model))

def model_call.go(n: Note, +request: String, +key: String, +st: Api.Style,
    +model: String) -> IO(Note & Rem.RemoteRes):
  model_call.try(Pvp.max_attempts(), 0n, n, request, key, st, model)

def model_call.key(empty: Bool, n: Note, +request: String, +key: String,
    +st: Api.Style, +model: String) -> IO(Note & Rem.RemoteRes):
  match empty:
    case True{}:
      IO.pure(Note & Rem.RemoteRes,
        (n, Rem.RRErr{String.append(Pvp.model_key_env(st), " is not set")}))
    case False{}:
      model_call.go(n, request, key, st, model)

# THE model resolution - the one function every surface uses: the
# provider call below AND the harness-info line the clients (TUI,
# bend_client) display. BEND_MODEL > config.toml model > default;
# aliases resolve through the provider table.
def resolved_model() -> IO(String):
  do IO<String>:
    mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
    +cfgt : String <- St.read_cfg()
    IO.pure(String,
      Pvp.resolve_model(Cf.env_cfg_or(P.env_or(mr, ""),
        Cf.cfg_get(cfgt, "model"), "claude-opus-5-5")))

@unsafe
def model_call(n: Note, +request: String) -> IO(Note & Rem.RemoteRes):
  do IO<Note & Rem.RemoteRes>:
    # the provider table: the resolved model carries the style, the
    # api_base and the key env
    +model : String <- resolved_model()
    +st : Api.Style <- IO.pure(Api.Style, Pvp.model_style(model))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env(Pvp.model_key_env(st))
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    model_call.key(String.is_empty(key), n, request, key, st, model)
