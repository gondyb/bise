# runtime/proc.bend — run a program, the way the runtime always has:
# the answer is the exit status on its own first line, then everything
# the program printed (stdout and stderr interleaved, in the order they
# were written). Built on Base's Process.run (bend 2.0.32), which
# replaced the hub's snaprun package and its C effects.
#
# Process.run keeps stdout and stderr apart; the program runs under
# `/bin/sh -c 'exec 2>&1; exec "$0" "$@"'` so stderr joins stdout in
# the one stream, argv passed through untouched (no shell parsing of
# the arguments). stdin is empty, as before.
import Base

# generous bounds: the bash tool hands long commands to the background
# itself, and the tools cap what they show the model; these only stop a
# runaway child from holding the REPL forever
def max_output() -> U32:
  268435456

def timeout_ms() -> U32:
  86400000

def answer.streams(+code: U32, oe: String & String) -> String:
  match oe:
    case (out, err):
      String.append(U32.show(code), String.append("\n", String.append(out, err)))

def answer.done(res: U32 & (String & String)) -> String:
  match res:
    case (code, oe):
      answer.streams(code, oe)

def answer.fail(e: U32 & String) -> String:
  match e:
    case (_, why):
      # never started, timed out, or printed past max_output
      String.append("127\nbend-harness: ", why)

def answer(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> String:
  match r:
    case Done{res}:
      answer.done(res)
    case Fail{e}:
      answer.fail(e)

# Process.run takes an affine list
def args1(xs: List<&2, String>) -> List<&1, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, rest}:
      Con{h, args1(rest)}

# argv[0] is the program (looked up in PATH), the rest its arguments
def run(+argv: List<&2, String>) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run(
      "/bin/sh", Con{"-c", Con{"exec 2>&1; exec \"$0\" \"$@\"", args1(argv)}}, "",
      max_output(), timeout_ms())
    IO.pure(String, answer(r))
