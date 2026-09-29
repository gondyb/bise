# bend/tests/canon-probe.bend — what the Core keeps of a BEND-SESSION file:
# CANON_IN=<file> prints K.to_text(K.from_text(<file>)) (read like the
# REPL reads it, runtime/persist.bend). The BISE-197 migration proof diffs
# it against bise-session's legacy::to_text(parse()).

import Base
import ../core/checkpoint.bend as K
import ../runtime/persist.bend as P

def out(c: Maybe<&2, K.Checkpoint>) -> String:
  match c:
    case None{}:
      "NONE"
    case Some{+chk}:
      K.to_text(chk)

def go(m: Maybe<&2, String>) -> IO(Unit):
  match m:
    case None{}:
      IO.print("UNREADABLE")
    case Some{+s}:
      IO.print(out(K.from_text(s)))

def main() -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("CANON_IN")
    +path : String <- IO.pure(String, P.env_or(r, ""))
    m : Maybe<&2, String> <- P.read_file(path)
    go(m)
