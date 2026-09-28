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
import ../core/anth-stream.bend as As
import ./debug.bend as Dbg
import ./usage-pure.bend as Us
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

# one attempt as the loop and the debug log see it: the policy's
# verdict, the HTTP status (0: no response) and the raw body received
type Got is Data:
  Got{a: Pvp.Attempt, status: U32, raw: String}

# a transport failure names the host the call went to (Pvp.net_cause)
def got_err(+host: String, +e: Http.Err) -> Got:
  Got{Pvp.attempt_err(host, net_of(e)), 0, ""}

def got_res(+res: Http.Res, st: Api.Style) -> Got:
  match res:
    case Http.Res{+status, _, _}:
      Got{verdict_res(res, st), status, Http.text(res)}

def model_call.verdict(r: Result<&2, &2, Http.Err, Http.Res>, st: Api.Style,
    +host: String) -> IO(Got):
  match r:
    case Done{+res}:
      do IO<Got>:
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        d : Unit <- model_call.res.dumper(dr, res)
        IO.pure(Got, got_res(res, st))
    case Fail{+e}:
      IO.pure(Got, got_err(host, e))

# ---- the loop's output: stdout (scripted) or the client socket ----
# Defined here, below main, so the provider call carries it through (its
# retry notes reach the client while the call waits). Sockets are affine
# handles, so Out is a Type: it is threaded, never copied.

type Out is Type:
  OStd{}
  ONet{sock: Socket}

def out.pair(sm: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket:
  match sm:
    case (s2, _):
      s2

# the wire log (BEND_WIRE_LOG, set by the Switchboard hub): every batch
# sent to the client is appended there FIRST. The hub reads the REPL's
# output from this file, not from the socket, so a hub that restarts
# (a version switch, a crash) resumes at its saved offset and loses no
# line, even one emitted while no hub was connected. BR-007: the notes
# of a call in flight (provider_retry) go there too, or the hub never
# sees them.
def wire_log.when(skip: Bool, +path: String, +text: String) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        f : Result<&1, &1, U32 & String, File> <- File.open(path, "a")
        P.write.opened(f, text)

def wire_log(+text: String) -> IO(Unit):
  do IO<Unit>:
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_LOG")
    +path : String <- IO.pure(String, P.env_or(fr, ""))
    wire_log.when(String.is_empty(path), path, text)

def note(n: Out, +line: String) -> IO(Out):
  match n:
    case OStd{}:
      do IO<Out>:
        IO.print(line)
        IO.pure(Out, OStd{})
    case ONet{sock}:
      do IO<Out>:
        wire_log(String.append(line, "\n"))
        sm : Socket & Result<&1, &1, U32 & String, Unit> <- TCP.send(sock,
          String.append(line, "\n"))
        s2 : Socket <- IO.pure(Socket, out.pair(sm))
        IO.pure(Out, ONet{s2})

# the token usage of a reply, as a feed line (runtime/usage-pure.bend);
# a reply without usage, or a failed call, prints nothing
def usage_note.pick(empty: Bool, n: Out, +line: String) -> IO(Out):
  match empty:
    case True{}:
      IO.pure(Out, n)
    case False{}:
      note(n, line)

# the attempt's body: a whole JSON reply, or the Anthropic stream's
# events (Us.usage_line reads both); no response (status 0) prints nothing
def usage_note(g: Got, n: Out, st: Api.Style, +model: String) -> IO(Out):
  match g:
    case Got{_, +status, +raw}:
      +line = Us.usage_line(st, model, status, raw)
      usage_note.pick(String.is_empty(line), n, line)

def interrupt_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, Pvp.interrupt_path_of(p))

# BR-005: a Ctrl+C landing during a failed attempt must not wait for
# the remaining retries: the pause before a retry is slept in 1 s
# slices and the flag is read between them. BR-007: only a flag WITH
# content is an interrupt (Pvp.interrupt_flag) - the cleared flag
# is an empty file - and the stop keeps the error that made the call
# retry.
def model_call.wait(fuel: Nat, +ipath: String, +left: U32) -> IO(Bool):
  match fuel:
    case 0n:
      IO.pure(Bool, False{})
    case 1n+k:
      do IO<Bool>:
        m : Maybe<&2, String> <- P.read_file(ipath)
        +stop : Bool <- IO.pure(Bool, Pvp.interrupt_flag(m))
        IO.sleep(Pvp.slice_ms(stop, left))
        more : Bool <- model_call.wait(k, ipath, Pvp.left_after(stop, left))
        IO.pure(Bool, Bool.or(stop, more))

def model_call.step.retry.go(stop: Bool, n: Out, +ipath: String, +k: Nat,
    +tries: Nat, +why: String,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match stop:
    case True{}:
      do IO<Out & Rem.RemoteRes>:
        # consume the flag so the boundary drains stay clean
        u : Unit <- P.write_file(ipath, "")
        IO.pure(Out & Rem.RemoteRes,
          (n, Rem.RRErr{Pvp.interrupted_text(Nat.add(tries, 1n), why)}))
    case False{}:
      next(k)(n)

def model_call.step.retry(can: Bool, n: Out, +k: Nat, +tries: Nat,
    +delay_ms: U32, +why: String,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match can:
    case True{}:
      do IO<Out & Rem.RemoteRes>:
        n2 : Out <- note(n, Pvp.retry_line(tries, why, delay_ms))
        +ipath : String <- interrupt_path()
        stop : Bool <- model_call.wait(Pvp.wait_slices(delay_ms), ipath,
          delay_ms)
        model_call.step.retry.go(stop, n2, ipath, k, tries, why, next)
    case False{}:
      IO.pure(Out & Rem.RemoteRes,
        (n, Rem.RRErr{Pvp.exhausted_text(Nat.add(tries, 1n), why)}))

def model_call.step(a: Pvp.Attempt, n: Out, +k: Nat, +tries: Nat,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match a:
    case Pvp.AOk{+reply}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.parse_reply(reply)))
    case Pvp.AFail{+why}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.RRErr{why}))
    case Pvp.ARetry{+after_ms, +why}:
      model_call.step.retry(Nat.is_lt(0n, k), n, k, tries,
        Pvp.retry_delay_ms(after_ms, tries), why, next)

# ---- the streamed call (Anthropic): Server-Sent Events ----
# The body arrives in pieces (raw bytes; decoded once, whole). Between
# two pieces the interrupt flag is read, so a Ctrl+C lands during a
# long generation instead of after it; a read that waits longer than
# stream_step_ms is a dead connection (a timeout the policy retries).

type Rd is Type:
  RdEnd{st: Http.Stream, parts: List<&2, String>}
  RdErr{st: Http.Stream, parts: List<&2, String>, e: Http.Err}
  RdStop{st: Http.Stream, parts: List<&2, String>}

@unsafe
def sse.flag.set(on: Bool, st: Http.Stream, +parts: List<&2, String>,
    +ipath: String) -> IO(Rd):
  match on:
    case True{}:
      IO.pure(Rd, RdStop{st, parts})
    case False{}:
      sse.read(st, parts, ipath)

@unsafe
def sse.flag(f: Maybe<&2, String>, st: Http.Stream, +parts: List<&2, String>,
    +ipath: String) -> IO(Rd):
  match f:
    case None{}:
      sse.read(st, parts, ipath)
    case Some{+c}:
      sse.flag.set(Pvp.interrupt_requested(c), st, parts, ipath)

@unsafe
def sse.piece(m: Maybe<&2, String>, st: Http.Stream, +parts: List<&2, String>,
    +ipath: String) -> IO(Rd):
  match m:
    case None{}:
      IO.pure(Rd, RdEnd{st, parts})
    case Some{+p}:
      do IO<Rd>:
        f : Maybe<&2, String> <- P.read_file(ipath)
        sse.flag(f, st, Con{p, parts}, ipath)

@unsafe
def sse.got(r: Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, String>>,
    +parts: List<&2, String>, +ipath: String) -> IO(Rd):
  match r:
    case (st, res):
      match res:
        case Fail{+e}:
          IO.pure(Rd, RdErr{st, parts, e})
        case Done{+m}:
          sse.piece(m, st, parts, ipath)

@unsafe
def sse.read(st: Http.Stream, +parts: List<&2, String>, +ipath: String)
    -> IO(Rd):
  do IO<Rd>:
    r : Http.Stream & Result<&2, &2, Http.Err, Maybe<&2, String>> <- Http.stream.read(st)
    sse.got(r, parts, ipath)

# the body text, decoded once from the raw pieces (newest first)
def sse.text(+parts: List<&2, String>) -> String:
  Enc.utf8.decode(String.join(List.reverse(&2, String, parts), ""))

def sse.dump(m: Result<&1, &1, U32 & String, String>, +text: String)
    -> IO(Unit):
  match m:
    case Fail{_}:
      IO.pure(Unit, Unit{})
    case Done{+path}:
      model_call.dump_res(path, text)

# a 200 streams events; any other status carries a plain error body
def sse.verdict.pick(ok: Bool, +status: U32, +retry_after: String,
    +text: String) -> Pvp.Attempt:
  match ok:
    case True{}:
      Pvp.attempt_status(200, retry_after, As.reply_any(text))
    case False{}:
      Pvp.attempt_status(status, retry_after,
        Api.reply_of_for(Api.Anthropic{}, status, text))

def sse.body(+status: U32, +retry_after: String, +text: String)
    -> IO(Got):
  do IO<Got>:
    dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
    d : Unit <- sse.dump(dr, text)
    IO.pure(Got,
      Got{sse.verdict.pick(U32.is_eq(status, 200), status, retry_after, text),
        status, text})

# a stream cut short keeps what it had received (the debug log shows
# how far the reply got)
def sse.done(rd: Rd, +status: U32, +retry_after: String, +ipath: String,
    +host: String) -> IO(Got):
  match rd:
    case RdEnd{st, +parts}:
      do IO<Got>:
        c : Unit <- Http.stream.close(st)
        sse.body(status, retry_after, sse.text(parts))
    case RdErr{st, +parts, +e}:
      do IO<Got>:
        c : Unit <- Http.stream.close(st)
        IO.pure(Got, Got{Pvp.attempt_err(host, net_of(e)), status,
          sse.text(parts)})
    case RdStop{st, +parts}:
      do IO<Got>:
        c : Unit <- Http.stream.close(st)
        # consume the flag: the boundary drains stay clean
        u : Unit <- P.write_file(ipath, "")
        IO.pure(Got, Got{Pvp.AFail{"interrupted by the user"}, status,
          sse.text(parts)})

@unsafe
def sse.head(sr: Http.Stream & Http.Res, +ipath: String, +host: String)
    -> IO(Got):
  match sr:
    case (st, res):
      match res:
        case Http.Res{+status, +h, _}:
          do IO<Got>:
            rd : Rd <- sse.read(st, Nil{}, ipath)
            sse.done(rd, status, Http.header(h, "retry-after"), ipath, host)

@unsafe
def sse.opened(r: Result<&1, &1, Http.Err, Http.Stream>, +ipath: String,
    +host: String) -> IO(Got):
  match r:
    case Fail{+e}:
      IO.pure(Got, got_err(host, e))
    case Done{st}:
      sse.head(Http.stream.res(st), ipath, host)

@unsafe
def sse.call(+url: String, hdrs: Map<&2, List<&2, String>>, +body: String)
    -> IO(Got):
  do IO<Got>:
    +ipath : String <- interrupt_path()
    r : Result<&1, &1, Http.Err, Http.Stream> <- Http.open.with("POST", url,
      hdrs, body, Pvp.stream_step_ms())
    sse.opened(r, ipath, Pvp.url_host(url))

# the whole reply at once (the OpenAI-compatible providers)
def whole.call(+url: String, hdrs: Map<&2, List<&2, String>>, +body: String,
    st: Api.Style) -> IO(Got):
  do IO<Got>:
    r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch.with("POST", url,
      hdrs, body, Pvp.step_timeout_ms())
    model_call.verdict(r, st, Pvp.url_host(url))

@unsafe
def call.pick(is_anth: Bool, +url: String, hdrs: Map<&2, List<&2, String>>,
    +body: String, st: Api.Style) -> IO(Got):
  match is_anth:
    case True{}:
      sse.call(url, hdrs, body)
    case False{}:
      whole.call(url, hdrs, body, st)

# the wire body: the Messages API streams, the others answer whole
def wire_body.pick(is_anth: Bool, +body: String) -> String:
  match is_anth:
    case True{}:
      As.stream_body(body)
    case False{}:
      body

# the attempt, into the session debug log when it is an incident
def model_call.log(+call: Nat, +tries: Nat, +model: String, +url: String,
    +json: String, g: Got) -> IO(Unit):
  match g:
    case Got{+a, +status, +raw}:
      Dbg.attempt(call, tries, model, url, status, json, raw, a)

def got_attempt(g: Got) -> Pvp.Attempt:
  match g:
    case Got{+a, _, _}:
      a

# one call = up to max_attempts attempts; `call` (its start time, ms)
# names the call in the debug log
@unsafe
def model_call.try(fuel: Nat, +tries: Nat, n: Out, +request: String,
    +key: String, +st: Api.Style, +model: String, +call: Nat)
    -> IO(Out & Rem.RemoteRes):
  match fuel:
    case 0n:
      IO.pure(Out & Rem.RemoteRes,
        (n, Rem.RRErr{"provider unreachable: retries exhausted"}))
    case 1n+k:
      do IO<Out & Rem.RemoteRes>:
        +json : String <- IO.pure(String,
          wire_body.pick(Api.is_anthropic(st),
            Api.api_body_for(st, model, Api.parse_wire(request))))
        +body : String <- IO.pure(String, Enc.utf8.encode(json))
        dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_DUMP")
        dump_dbg : Unit <- dump_wire(P.env_or(dr, ""), body)
        hdrs : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
          model_headers(st, key))
        ur : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_PROVIDER_URL")
        +url : String <- IO.pure(String, Pvp.provider_url(P.env_or(ur, ""), st))
        +g : Got <- call.pick(Api.is_anthropic(st), url, hdrs, body, st)
        lg : Unit <- model_call.log(call, tries, model, url, json, g)
        nu : Out <- usage_note(g, n, st, model)
        model_call.step(got_attempt(g), nu, k, tries,
          k2 => n2 => model_call.try(k2, Nat.add(tries, 1n), n2, request, key,
            st, model, call))

def model_call.go(n: Out, +request: String, +key: String, +st: Api.Style,
    +model: String) -> IO(Out & Rem.RemoteRes):
  do IO<Out & Rem.RemoteRes>:
    +call : Nat <- IO.now()
    model_call.try(Pvp.max_attempts(), 0n, n, request, key, st, model, call)

def model_call.key(empty: Bool, n: Out, +request: String, +key: String,
    +st: Api.Style, +model: String) -> IO(Out & Rem.RemoteRes):
  match empty:
    case True{}:
      IO.pure(Out & Rem.RemoteRes,
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
def model_call(n: Out, +request: String) -> IO(Out & Rem.RemoteRes):
  do IO<Out & Rem.RemoteRes>:
    # the provider table: the resolved model carries the style, the
    # api_base and the key env
    +model : String <- resolved_model()
    +st : Api.Style <- IO.pure(Api.Style, Pvp.model_style(model))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env(Pvp.model_key_env(st))
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    model_call.key(String.is_empty(key), n, request, key, st, model)
