# runtime/persist.bend — the session checkpoint on disk (--continue).
#
# The REPL always writes the session to BEND_SESSION_FILE after every
# connection (turn boundaries: turn, pending, queued and notifications
# are all empty there), and restores it on startup when BEND_CONTINUE
# is set — the client is greeted with how many messages came back.
import Base
import ../core/types.bend as T
import ../core/checkpoint.bend as K

# what loading gives back: the session to serve, and the greeting to
# send on the first connection (None = fresh session)
type Load is Data:
  LD{sess: T.Session, greet: Maybe<&2, Nat>}

def env_or(r: Result<&1, &1, U32 & String, String>, +dflt: String) -> String:
  match r:
    case Done{+s}:
      s
    case Fail{_}:
      dflt

def history_of(+sess: T.Session) -> List<&2, T.Msg>:
  match sess:
    case T.Sess{_, +history, _, _, _, _, _, _}:
      history

# ---- write (best effort: a failed save never kills the session) ----

def write.done(fw: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  match fw:
    case (f2, Fail{_}):
      do IO<Unit>:
        File.close(f2)
        IO.pure(Unit, Unit{})
    case (f2, Done{u}):
      do IO<Unit>:
        File.close(f2)
        IO.pure(Unit, Unit{})

def write.opened(f: Result<&1, &1, U32 & String, File>, +content: String)
    -> IO(Unit):
  match f:
    case Fail{_}:
      IO.pure(Unit, Unit{})
    case Done{file}:
      do IO<Unit>:
        fw : File & Result<&1, &1, U32 & String, Unit> <- File.write(file,
          content)
        write.done(fw)

def write_file(+path: String, +content: String) -> IO(Unit):
  do IO<Unit>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    write.opened(f, content)

# BISE-198: the session file is written to a temp file next to it, then
# renamed over it, so a kill during a save leaves the old file or the new
# one, never a half file; umask 077 makes both 0600 (a checkpoint is a
# whole conversation). One /bin/sh runs it all: Base has no rename or
# chmod. The text goes through stdin; a failure (disk full) removes the
# temp file and keeps the old checkpoint.
def ATOMIC_SCRIPT() -> String:
  "umask 077; t=\"$0.tmp.$$\"; cat > \"$t\" && mv -f \"$t\" \"$0\" || { rm -f \"$t\"; exit 1; }"

def write_atomic(+path: String, +content: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run(
      "/bin/sh", Con{"-c", Con{ATOMIC_SCRIPT(), Con{path, Nil{}}}}, content,
      65536, 600000)
    IO.pure(Unit, Unit{})

def save.when(skip: Bool, +path: String, +sess: T.Session) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      write_atomic(path, K.to_text(K.export(sess)))

def save(+sess: T.Session) -> IO(Unit):
  do IO<Unit>:
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_SESSION_FILE")
    +path : String <- IO.pure(String, env_or(fr, ""))
    save.when(String.is_empty(path), path, sess)

# ---- read (None when the file is missing or unreadable) ----

def read.cont(empty: Bool, f2: File, +chunk: String, acc: String,
    next: File -> String -> IO(String)) -> IO(String):
  match empty:
    case True{}:
      do IO<String>:
        File.close(f2)
        IO.pure(String, acc)
    case False{}:
      next(f2)(String.append(acc, chunk))

def read.more(fr: File & Result<&1, &1, U32 & String, String>, acc: String,
    next: File -> String -> IO(String)) -> IO(String):
  match fr:
    case (f2, Fail{_}):
      do IO<String>:
        File.close(f2)
        IO.pure(String, acc)
    case (f2, Done{+chunk}):
      read.cont(String.is_empty(chunk), f2, chunk, acc, next)

@unsafe
def read_all(f: File, acc: String) -> IO(String):
  do IO<String>:
    fr : File & Result<&1, &1, U32 & String, String> <- File.read(f, 65536)
    read.more(fr, acc, f2 => a2 => read_all(f2, a2))

def read.opened(f: Result<&1, &1, U32 & String, File>) -> IO(Maybe<&2, String>):
  match f:
    case Fail{_}:
      IO.pure(Maybe<&2, String>, None{})
    case Done{file}:
      do IO<Maybe<&2, String>>:
        content : String <- read_all(file, "")
        IO.pure(Maybe<&2, String>, Some{content})

def read_file(+path: String) -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "r")
    read.opened(f)

# the path in the env var `name`, else $HOME followed by `rel` (the var
# unset or empty)
def env_path(+name: String, +rel: String) -> IO(String):
  do IO<String>:
    ir : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    +ix : String <- IO.pure(String, env_or(ir, ""))
    +hx : String <- IO.pure(String, env_or(hr, ""))
    IO.pure(String,
      Bool.pick(String, String.is_empty(ix), String.append(hx, rel), ix))

# a file's text, or dflt when it cannot be read
def read_or(+path: String, +dflt: String) -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- read_file(path)
    IO.pure(String, Maybe.default(&2, String, m, dflt))

# ---- load: fresh, or the checkpoint when BEND_CONTINUE is set ----

def load.greeted.put(empty: Bool, +sess: T.Session, +n: Nat) -> Load:
  match empty:
    case True{}:
      LD{sess, None{}}
    case False{}:
      LD{sess, Some{n}}

def load.greeted(+sess: T.Session) -> IO(Load):
  do IO<Load>:
    +n : Nat <- IO.pure(Nat, List.length(&2, T.Msg, history_of(sess)))
    IO.pure(Load, load.greeted.put(Nat.is_eq(n, 0n), sess, n))

# a restored session keeps what it saved, but the compaction threshold
# of this start (BISE-135: /model may have changed the model, so its
# window, since the save: the fresh config's threshold follows it)
def cfg_threshold(+c: T.Cfg) -> Nat:
  match c:
    case T.Cfg{+threshold, _, _, _, _}:
      threshold

def with_threshold.cfg(+c: T.Cfg, +n: Nat) -> T.Cfg:
  match c:
    case T.Cfg{_, +sel, +nulls, +sys, +tools}:
      T.Cfg{n, sel, nulls, sys, tools}

def with_threshold(+s: T.Session, +n: Nat) -> T.Session:
  match s:
    case T.Sess{+cfg, +h, +i, +a, +t, +p, +q, +no}:
      T.Sess{with_threshold.cfg(cfg, n), h, i, a, t, p, q, no}

def fresh_threshold(+fresh: T.Session) -> Nat:
  match fresh:
    case T.Sess{+cfg, _, _, _, _, _, _, _}:
      cfg_threshold(cfg)

def load.chk(c: Maybe<&2, K.Checkpoint>, +fresh: T.Session) -> IO(Load):
  match c:
    case None{}:
      IO.pure(Load, LD{fresh, None{}})
    case Some{+chk}:
      load.greeted(with_threshold(K.restore(chk), fresh_threshold(fresh)))

def load.content(m: Maybe<&2, String>, +fresh: T.Session) -> IO(Load):
  match m:
    case None{}:
      IO.pure(Load, LD{fresh, None{}})
    case Some{+content}:
      load.chk(K.from_text(content), fresh)

def load.path(unset: Bool, +path: String, +fresh: T.Session) -> IO(Load):
  match unset:
    case True{}:
      IO.pure(Load, LD{fresh, None{}})
    case False{}:
      do IO<Load>:
        m : Maybe<&2, String> <- read_file(path)
        load.content(m, fresh)

def load.want(want: Bool, +path: String, +fresh: T.Session) -> IO(Load):
  match want:
    case False{}:
      IO.pure(Load, LD{fresh, None{}})
    case True{}:
      load.path(String.is_empty(path), path, fresh)

def load(+fresh: T.Session) -> IO(Load):
  do IO<Load>:
    cr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_CONTINUE")
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_SESSION_FILE")
    +path : String <- IO.pure(String, env_or(fr, ""))
    load.want(String.eq(env_or(cr, ""), "1"), path, fresh)
