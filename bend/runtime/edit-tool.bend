# runtime/edit-tool.bend - Vibe's `edit` and `write_file` tools, the IO
# half: core/edit.bend holds every check, string and transformation
# (law-pinned); this module reads the file, asks the disk what a path
# is, and writes. The edit's write is atomic (a temp file next to the
# target, its mode copied, then renamed over it, like Vibe's
# atomic_replace); write_file creates the parent folders and never
# overwrites (the shell's noclobber, Vibe's open mode "x").
import Base
import ../core/edit.bend as Ed
import ./tools-pure.bend as Xp
import ./persist.bend as P

# ---- the shell helpers: (exit code, output) ----

def sh.res(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> U32 & String:
  match r:
    case Done{(code, (out, err))}:
      (code, String.append(out, err))
    case Fail{(_, why)}:
      (127, why)

def sh(+script: String, +path: String, +input: String) -> IO(U32 & String):
  do IO<U32 & String>:
    r : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run(
      "/bin/sh", Con{"-c", Con{script, Con{path, Nil{}}}}, input, 1048576, 60000)
    IO.pure(U32 & String, sh.res(r))

# what a path is on the disk: "dir", "file" or "none"
def KIND_SCRIPT() -> String:
  "if [ -d \"$0\" ]; then echo dir; elif [ -e \"$0\" ]; then echo file; else echo none; fi"

def kind.of(r: U32 & String) -> String:
  match r:
    case (_, +out):
      String.trim(out)

def kind(+path: String) -> IO(String):
  do IO<String>:
    r : U32 & String <- sh(KIND_SCRIPT(), path, "")
    IO.pure(String, kind.of(r))

# the atomic replace: the text on stdin, a temp file next to the target
# with the target's mode, then a rename; a failure removes the temp file
def ATOMIC_SCRIPT() -> String:
  "t=\"$(dirname -- \"$0\")/.$(basename -- \"$0\").tmp.$$\"; cat > \"$t\" || { rm -f \"$t\"; exit 1; }; m=$(stat -f %Lp \"$0\" 2>/dev/null || stat -c %a \"$0\" 2>/dev/null) && chmod \"$m\" \"$t\"; mv -f \"$t\" \"$0\" || { rm -f \"$t\"; exit 1; }"

# write_file: the parents, then an exclusive create
def CREATE_SCRIPT() -> String:
  "mkdir -p -- \"$(dirname -- \"$0\")\" && { set -C; cat > \"$0\"; }"

def fail(+msg: String) -> Xp.ExecRes:
  Xp.Ex{False{}, msg}

# ---- bytes, for the "not valid text" error ----

def bytes.got(res: Result<&1, &1, U32 & String, List<&2, U32>>) -> List<&2, U32>:
  match res:
    case Done{+bs}:
      bs
    case Fail{_}:
      Nil{}

def bytes.read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  match r:
    case (f, res):
      do IO<List<&2, U32>>:
        u : Unit <- File.close(f)
        IO.pure(List<&2, U32>, bytes.got(res))

def bytes.sized(fs: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  match fs:
    case (f, Done{n}):
      do IO<List<&2, U32>>:
        r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, n)
        bytes.read(r)
    case (f, Fail{_}):
      do IO<List<&2, U32>>:
        u : Unit <- File.close(f)
        IO.pure(List<&2, U32>, Nil{})

def bytes.opened(o: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match o:
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})
    case Done{f}:
      do IO<List<&2, U32>>:
        fs : File & Result<&1, &1, U32 & String, U32> <- File.size(f)
        bytes.sized(fs)

def read_bytes(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    o : Result<&1, &1, U32 & String, File> <- File.open(path, "r")
    bytes.opened(o)

# ---- edit ----

def edit.wrote(r: U32 & String, +path: String, +msg: String, +old: String,
    +new: String) -> Xp.ExecRes:
  match r:
    case (code, +out):
      Bool.pick(Xp.ExecRes, U32.is_eq(code, 0),
        Xp.Ex{True{}, Ed.edit_result(path, msg, old, new)},
        fail(Ed.err_os(path, String.trim(out))))

def edit.apply(e: Ed.Edited, +path: String, +nl: String, +old: String,
    +new: String) -> IO(Xp.ExecRes):
  match e:
    case Ed.EdFail{+msg}:
      IO.pure(Xp.ExecRes, fail(msg))
    case Ed.EdOk{+text, +msg}:
      do IO<Xp.ExecRes>:
        r : U32 & String <- sh(ATOMIC_SCRIPT(), path, Ed.restore(text, nl))
        IO.pure(Xp.ExecRes, edit.wrote(r, path, msg, old, new))

def edit.text(+text: String, +path: String, +old: String, +new: String,
    +all: Bool) -> IO(Xp.ExecRes):
  edit.apply(Ed.apply(Ed.normalize(text), old, new, all), path,
    Ed.nl_style(text), old, new)

# a U+FFFD in the text: a real one, or a byte that is not UTF-8
def edit.bad(m: Maybe<&2, Nat>, +text: String, +path: String, +old: String,
    +new: String, +all: Bool) -> IO(Xp.ExecRes):
  match m:
    case Some{+n}:
      IO.pure(Xp.ExecRes, fail(Ed.err_not_text(path, n)))
    case None{}:
      edit.text(text, path, old, new, all)

def edit.check_text(odd: Bool, +text: String, +path: String, +old: String,
    +new: String, +all: Bool) -> IO(Xp.ExecRes):
  match odd:
    case False{}:
      edit.text(text, path, old, new, all)
    case True{}:
      do IO<Xp.ExecRes>:
        bs : List<&2, U32> <- read_bytes(path)
        edit.bad(Ed.utf8_bad(bs), text, path, old, new, all)

def edit.read(m: Maybe<&2, String>, +path: String, +old: String, +new: String,
    +all: Bool) -> IO(Xp.ExecRes):
  match m:
    case None{}:
      IO.pure(Xp.ExecRes, fail(Ed.err_permission(path)))
    case Some{+text}:
      edit.check_text(Ed.has_replacement(text), text, path, old, new, all)

def edit.go(+path: String, +old: String, +new: String, +all: Bool) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    m : Maybe<&2, String> <- P.read_file(path)
    edit.read(m, path, old, new, all)

def edit.kind(+k: String, +path: String, +old: String, +new: String,
    +all: Bool) -> IO(Xp.ExecRes):
  Bool.pick(IO(Xp.ExecRes), String.eq(k, "none"),
    IO.pure(Xp.ExecRes, fail(Ed.err_missing(path))),
    Bool.pick(IO(Xp.ExecRes), String.eq(k, "dir"),
      IO.pure(Xp.ExecRes, fail(Ed.err_not_file(path))),
      edit.go(path, old, new, all)))

# ---- write_file ----

def write.done(r: U32 & String, +path: String, +content: String) -> Xp.ExecRes:
  match r:
    case (code, +out):
      Bool.pick(Xp.ExecRes, U32.is_eq(code, 0),
        Xp.Ex{True{}, Ed.write_result(path, Ed.utf8_len(content), content)},
        fail(Ed.werr_write(path, String.trim(out))))

def write.go(+path: String, +content: String) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : U32 & String <- sh(CREATE_SCRIPT(), path, content)
    IO.pure(Xp.ExecRes, write.done(r, path, content))

def write.kind(+k: String, +path: String, +content: String) -> IO(Xp.ExecRes):
  Bool.pick(IO(Xp.ExecRes), String.eq(k, "none"),
    write.go(path, content),
    IO.pure(Xp.ExecRes, fail(Ed.werr_exists(path))))

# ---- the working folder a relative path lives in ----
# BEND_WORKDIR (where the agent's bash cds, like apply_patch's), else
# the process's PWD
def workdir() -> IO(String):
  do IO<String>:
    wr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WORKDIR")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("PWD")
    +w : String <- IO.pure(String, P.env_or(wr, ""))
    IO.pure(String, Bool.pick(String, String.is_empty(w), P.env_or(pr, "/"), w))

def home() -> IO(String):
  do IO<String>:
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    IO.pure(String, P.env_or(hr, ""))

def resolved(+path: String) -> IO(String):
  do IO<String>:
    +d : String <- workdir()
    +h : String <- home()
    IO.pure(String, Ed.resolve(d, h, path))

# ---- the tools ----

def exec_edit.args(bad: Maybe<&2, String>, a: Ed.EditArgs) -> IO(Xp.ExecRes):
  match bad:
    case Some{+msg}:
      IO.pure(Xp.ExecRes, fail(msg))
    case None{}:
      match a:
        case Ed.EditArgs{+p, +old, +new, +all}:
          do IO<Xp.ExecRes>:
            +path : String <- resolved(p)
            +k : String <- kind(path)
            edit.kind(k, path, old, new, all)

def exec_edit.of(a: Ed.EditArgs) -> IO(Xp.ExecRes):
  match a:
    case Ed.EditArgs{+p, +old, +new, +all}:
      exec_edit.args(Ed.check_args(p, old, new), Ed.EditArgs{p, old, new, all})

def exec_write.args(bad: Maybe<&2, String>, a: Ed.WriteArgs) -> IO(Xp.ExecRes):
  match bad:
    case Some{+msg}:
      IO.pure(Xp.ExecRes, fail(msg))
    case None{}:
      match a:
        case Ed.WriteArgs{+p, +content}:
          do IO<Xp.ExecRes>:
            +path : String <- resolved(p)
            +k : String <- kind(path)
            write.kind(k, path, content)

def exec_write.of(a: Ed.WriteArgs) -> IO(Xp.ExecRes):
  match a:
    case Ed.WriteArgs{+p, +content}:
      exec_write.args(Ed.check_write(p, content), Ed.WriteArgs{p, content})

def scripted(+name: String) -> Xp.ExecRes:
  fail(String.append(name, " needs a live session"))

def exec_edit(+args: String, live: Bool) -> IO(Xp.ExecRes):
  match live:
    case True{}:
      exec_edit.of(Ed.edit_args(args))
    case False{}:
      IO.pure(Xp.ExecRes, scripted("edit"))

def exec_write(+args: String, live: Bool) -> IO(Xp.ExecRes):
  match live:
    case True{}:
      exec_write.of(Ed.write_args(args))
    case False{}:
      IO.pure(Xp.ExecRes, scripted("write_file"))
