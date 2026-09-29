# runtime/debug.bend — the session debug log, IO side.
#
# Writes into BEND_DEBUG_DIR (the parent sets it per session; unset in
# the scripted suites, where every call here is a no-op): events.jsonl,
# and the request / raw reply of every failed provider attempt. Best
# effort: a failed write never touches the session. The lines and the
# policy are pure, in runtime/debug-pure.bend.
import Base
import ./persist.bend as P
import ./debug-pure.bend as Dp
import ./provider-pure.bend as Pvp
import ./proc.bend as Proc

def dir() -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_DEBUG_DIR")
    IO.pure(String, P.env_or(r, ""))

def append_file(+path: String, +content: String) -> IO(Unit):
  do IO<Unit>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "a")
    P.write.opened(f, content)

def append.go(skip: Bool, +d: String, +text: String) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      append_file(d ++ "/events.jsonl", text)

# append text to events.jsonl (nothing without a debug dir or a text)
def append(+d: String, +text: String) -> IO(Unit):
  append.go(String.is_empty(d) || String.is_empty(text), d, text)

# the event time: wall-clock ms (see Dp.stamp)
def now(+d: String) -> IO(Nat):
  do IO<Nat>:
    +mono : Nat <- IO.now()
    c : Maybe<&2, String> <- P.read_file(d ++ "/clock")
    IO.pure(Nat, Dp.stamp(c, mono))

def start.go(off: Bool, +d: String) -> IO(Unit):
  match off:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        +mono : Nat <- IO.now()
        +probe : String <- Proc.run(Con{"sh", Con{"-c", Con{
          "perl -MTime::HiRes=time -e 'printf(\"%d\", time()*1000)' 2>/dev/null || echo $(( $(date +%s) * 1000 ))",
          Nil{}}}})
        +offset : Nat <- IO.pure(Nat, Dp.anchor(String.lines(probe), mono))
        w : Unit <- P.write_file(d ++ "/clock", Nat.show(offset))
        append(d, Dp.ev_line(Nat.add(mono, offset), "repl_start",
          Con{Dp.FNat{"mono_ms", mono}, Con{Dp.FNat{"clock_offset_ms", offset},
            Nil{}}}) ++ "\n")

# at REPL startup, before any event: the clock anchor, and the start
def start() -> IO(Unit):
  do IO<Unit>:
    +d : String <- dir()
    start.go(String.is_empty(d), d)

def sent.go(off: Bool, +d: String, +lines: List<&2, String>) -> IO(Unit):
  match off:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        +ts : Nat <- now(d)
        append(d, Dp.crumbs(lines, ts))

# the breadcrumbs of one batch of lines sent to the client
def sent(+lines: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    +d : String <- dir()
    sent.go(String.is_empty(d), d, lines)

def write_named.pick(skip: Bool, +d: String, +name: String, +content: String)
    -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      P.write_file(d ++ "/" ++ name, content)

def attempt.plan(p: Dp.Plan, +d: String, +call: Nat, +tries: Nat,
    +model: String, +url: String, +status: U32, +json: String,
    +raw: String) -> IO(Unit):
  match p:
    case Dp.PSkip{}:
      IO.pure(Unit, Unit{})
    case Dp.PRecovered{}:
      do IO<Unit>:
        +ts : Nat <- now(d)
        append(d, Dp.recovered_line(ts, call, tries, model) ++ "\n")
    case Dp.PFailed{+verdict, +why, +after_ms}:
      do IO<Unit>:
        +ts : Nat <- now(d)
        # the request once per call (its first failure), the raw reply
        # of every failed attempt
        r1 : Unit <- write_named.pick(Nat.is_lt(0n, tries), d,
          Dp.req_name(call), json)
        r2 : Unit <- write_named.pick(String.is_empty(raw), d,
          Dp.reply_name(call, tries), Dp.clip(raw, Dp.reply_max()))
        append(d, Dp.failed_line(ts, call, tries, verdict, why, after_ms,
          status, model, url, String.length(json), String.length(raw))
          ++ "\n")

def attempt.on(off: Bool, +d: String, +call: Nat, +tries: Nat,
    +model: String, +url: String, +status: U32, +json: String,
    +raw: String, a: Pvp.Attempt) -> IO(Unit):
  match off:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      attempt.plan(Dp.plan(a, tries), d, call, tries, model, url, status,
        json, raw)

# one provider attempt, recorded when it is an incident (a failure, or
# the success that ended a run of failures)
def attempt(+call: Nat, +tries: Nat, +model: String, +url: String,
    +status: U32, +json: String, +raw: String, a: Pvp.Attempt) -> IO(Unit):
  do IO<Unit>:
    +d : String <- dir()
    attempt.on(String.is_empty(d), d, call, tries, model, url, status, json,
      raw, a)
