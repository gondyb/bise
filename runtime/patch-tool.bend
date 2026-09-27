# runtime/patch-tool.bend — the apply_patch tool executor.
#
# core/patch.bend parses and applies the V4A patch in memory; this
# module reads the files, validates the whole patch, then writes.
import Base
import ../core/text.bend as Tx
import ./tools.bend as X
import ./persist.bend as P
import ../core/patch.bend as Pa
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
# ---- apply_patch: the Codex V4A file editor ----
# core/patch.bend owns the parser and the strict sequential matcher
# (both law-pinned); this layer only moves bytes. VALIDATE first,
# apply second: every action is checked against the real files BEFORE
# anything is written, so a patch is all-or-nothing - a half-applied
# patch is the worst possible ergonomics for the model. Every failure
# is loud and names the file (and the chunk, for a stale context).

# a validated action, ready to execute blind
type Prep is Data:
  PrepAdd{path: String, content: String}
  PrepUpdate{path: String, content: String}
  PrepDelete{path: String}
  PrepBad{reason: String}

type PV is Data:
  PVOk{preps: List<&2, Prep>}
  PVFail{reason: String}

# the loop's end: the first PrepBad fails the whole patch
def exec_patch.finish(acc: List<&2, Prep>, +good: List<&2, Prep>) -> PV:
  match acc:
    case Nil{}:
      PVOk{List.reverse(&2, Prep, good)}
    case Con{+p, +rest}:
      match p:
        case PrepBad{+reason}:
          PVFail{reason}
        case _:
          exec_patch.finish(rest, Con{p, good})

# the next iteration's read: the head action's file is opened inside
# the PREVIOUS do-block (m always arrives as an argument, never as a
# computed scrutinee)
def exec_patch.read_next(rest: List<&2, Pa.PAction>) -> IO(Maybe<&2, String>):
  match rest:
    case Nil{}:
      IO.pure(Maybe<&2, String>, None{})
    case Con{+act, _}:
      P.read_file(Pa.p_path_of(act))

def prep_add(bad: Bool, +path: String, +content: String) -> Prep:
  match bad:
    case True{}:
      PrepBad{String.append("*** Add File: ", String.append(path,
        ": the path is empty"))}
    case False{}:
      PrepAdd{path, content}

def prep_of_po(po: Pa.POut, +path: String,
    +chunks: List<&2, Pa.PChunk>) -> Prep:
  match po:
    case Pa.POut{True{}, +content, _}:
      PrepUpdate{path, content}
    case Pa.POut{False{}, _, +failed}:
      PrepBad{String.append("*** Update File: ", String.append(path,
        String.append(": chunk ", String.append(Nat.show(Nat.add(failed, 1n)),
          String.append(" (near '",
            String.append(Pa.p_chunk_anchor(chunks, failed),
              "') not found - the file changed since it was read: re-read it and adjust the patch"))))))}

# validate every action against the real files; `m` is the read
# result for the head action
def exec_patch.val(actions: List<&2, Pa.PAction>, +acc: List<&2, Prep>,
    m: Maybe<&2, String>) -> IO(PV):
  match actions:
    case Nil{}:
      do IO<PV>:
        IO.pure(PV, exec_patch.finish(acc, Nil{}))
    case Con{+act, +rest}:
      match act:
        case Pa.PAdd{+path, +content}:
          match m:
            case Some{_}:
              do IO<PV>:
                IO.pure(PV, PVFail{String.append("*** Add File: ",
                  String.append(path, " already exists"))})
            case None{}:
              do IO<PV>:
                p2 : Prep <- IO.pure(Prep,
                  prep_add(String.is_empty(String.trim(path)), path, content))
                m2 : Maybe<&2, String> <- exec_patch.read_next(rest)
                exec_patch.val(rest, Con{p2, acc}, m2)
        case Pa.PDelete{+path}:
          match m:
            case None{}:
              do IO<PV>:
                IO.pure(PV, PVFail{String.append("*** Delete File: ",
                  String.append(path, " does not exist"))})
            case Some{_}:
              do IO<PV>:
                m2 : Maybe<&2, String> <- exec_patch.read_next(rest)
                exec_patch.val(rest, Con{PrepDelete{path}, acc}, m2)
        case Pa.PUpdate{+path, +chunks}:
          match m:
            case None{}:
              do IO<PV>:
                IO.pure(PV, PVFail{String.append("*** Update File: ",
                  String.append(path, " does not exist"))})
            case Some{+content}:
              do IO<PV>:
                po : Pa.POut <- IO.pure(Pa.POut,
                  Pa.apply_update(content, chunks))
                p2 : Prep <- IO.pure(Prep, prep_of_po(po, path, chunks))
                m2 : Maybe<&2, String> <- exec_patch.read_next(rest)
                exec_patch.val(rest, Con{p2, acc}, m2)

def patch_summary.go(lines: List<&2, String>) -> String:
  match lines:
    case Nil{}:
      ""
    case Con{+l, +rest}:
      match rest:
        case Nil{}:
          String.append("- ", l)
        case Con{_, _}:
          String.append(String.append("- ", l),
            String.append("\n", patch_summary.go(rest)))

def patch_summary(lines: List<&2, String>) -> String:
  match lines:
    case Nil{}:
      "Done!"
    case Con{_, _}:
      String.append("Done!\n", patch_summary.go(lines))

# apply the validated actions blind
def exec_patch.run(preps: List<&2, Prep>, +acc: List<&2, String>)
    -> IO(List<&2, String>):
  match preps:
    case Nil{}:
      do IO<List<&2, String>>:
        IO.pure(List<&2, String>, List.reverse(&2, String, acc))
    case Con{+p, +rest}:
      match p:
        case PrepBad{+reason}:
          # unreachable: validate rejected the patch before any write
          do IO<List<&2, String>>:
            IO.pure(List<&2, String>, acc)
        case PrepAdd{+path, +content}:
          do IO<List<&2, String>>:
            o : String <- Snap.run(Con{"/bin/sh", Con{"-c",
              Con{String.append("mkdir -p -- \"$(dirname -- ",
                String.append(Tx.sh_quote(path), ")\"")), Nil{}}}})
            w : Unit <- P.write_file(path, content)
            exec_patch.run(rest, Con{String.append("added ", path), acc})
        case PrepUpdate{+path, +content}:
          do IO<List<&2, String>>:
            w : Unit <- P.write_file(path, content)
            exec_patch.run(rest, Con{String.append("updated ", path), acc})
        case PrepDelete{+path}:
          do IO<List<&2, String>>:
            o : String <- Snap.run(Con{"/bin/sh", Con{"-c",
              Con{String.append("rm -f -- ", Tx.sh_quote(path)), Nil{}}}})
            exec_patch.run(rest, Con{String.append("deleted ", path), acc})

def exec_patch.done(pv: PV) -> IO(X.ExecRes):
  match pv:
    case PVFail{+reason}:
      IO.pure(X.ExecRes, X.Ex{False{}, reason})
    case PVOk{+preps}:
      do IO<X.ExecRes>:
        lines : List<&2, String> <- exec_patch.run(preps, Nil{})
        IO.pure(X.ExecRes, X.Ex{True{}, patch_summary(lines)})

def exec_patch.parse_none() -> X.ExecRes:
  X.Ex{False{}, "invalid patch: the first line must be '*** Begin Patch' and the last '*** End Patch'; inside an Update File section every line must start with ' ' (context), '+' (added) or '-' (removed); an Update File section needs at least one chunk"}

def exec_patch.parse(acts: Maybe<&2, List<&2, Pa.PAction>>) -> IO(X.ExecRes):
  match acts:
    case None{}:
      IO.pure(X.ExecRes, exec_patch.parse_none())
    case Some{+actions}:
      do IO<X.ExecRes>:
        m : Maybe<&2, String> <- exec_patch.read_next(actions)
        pv : PV <- exec_patch.val(actions, Nil{}, m)
        exec_patch.done(pv)

def exec_patch.live(+args: String) -> IO(X.ExecRes):
  do IO<X.ExecRes>:
    dw : Unit <- P.write_file("/tmp/ap-args-dump.txt", args)
    acts : Maybe<&2, List<&2, Pa.PAction>> <- IO.pure(
      Maybe<&2, List<&2, Pa.PAction>>, Pa.parse_patch(args))
    exec_patch.parse(acts)

def exec_patch.scripted() -> X.ExecRes:
  X.Ex{False{}, "apply_patch needs a live session"}

def exec_patch(+args: String, live: Bool) -> IO(X.ExecRes):
  match live:
    case True{}:
      exec_patch.live(args)
    case False{}:
      IO.pure(X.ExecRes, exec_patch.scripted())
