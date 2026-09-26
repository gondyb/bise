# core/commands.bend — the user-facing command language, server-side.
#
# Any client (nc, the TUI, a future one) sends raw user lines; the REPL
# interprets them here, in pure Bend:
#   - plain text becomes an implicit "say <text>"
#   - /commands map onto the harness protocol
#   - raw protocol words (say, steer, notify, compact, interrupt, quit,
#     reload, help) pass through untouched
# LAWS.bend pins this contract law by law.
import Base
import ./types.bend as T
import ./compaction.bend as C

# what one user line becomes: a protocol line to run, or a local
# warning that changes no session state
type Interpretation is Data:
  Send{line: String}
  Note{warning: String}

type CmdLine is Data:
  CL{op: String, rest: String}

def op_of.found(sp: Maybe<&2, Nat>, +line: String) -> CmdLine:
  match sp:
    case None{}:
      CL{line, ""}
    case Some{+p}:
      CL{String.take(line, p), String.drop(line, Nat.add(p, 1n))}

def op_of(+line: String) -> CmdLine:
  op_of.found(C.find(line, " ", 0n), line)

# the harness protocol words: sent verbatim, never re-interpreted
def is_raw_word(+w: String) -> Bool:
  Bool.or(String.eq(w, "say"),
   Bool.or(String.eq(w, "steer"),
   Bool.or(String.eq(w, "notify"),
   Bool.or(String.eq(w, "compact"),
   Bool.or(String.eq(w, "interrupt"),
   Bool.or(String.eq(w, "quit"),
   Bool.or(String.eq(w, "reload"),
    String.eq(w, "help"))))))))

# ---- /command dispatch (the codebase's Bool-chain idiom; defs are
# ordered so every call target is already defined) ----

# /reload restarts the harness process on the current session; the
# SERVER gates it between turns (see core/session.bend reload_allowed)
def interpret.reload(eq: Bool, +name: String) -> Interpretation:
  match eq:
    case True{}:
      Send{"reload"}
    case False{}:
      Note{String.append("unknown command: /",
        String.append(name, " — try /help"))}

def interpret.quit(eq: Bool, eq_exit: Bool, +name: String) -> Interpretation:
  match eq:
    case True{}:
      Send{"quit"}
    case False{}:
      match eq_exit:
        case True{}:
          Send{"quit"}
        case False{}:
          interpret.reload(String.eq(name, "reload"), name)

def interpret.help(eq: Bool, +name: String, +rest: String) -> Interpretation:
  match eq:
    case True{}:
      Send{"help"}
    case False{}:
      interpret.quit(String.eq(name, "quit"), String.eq(name, "exit"), name)

def interpret.interrupt(eq: Bool, +name: String, +rest: String) -> Interpretation:
  match eq:
    case True{}:
      Send{"interrupt"}
    case False{}:
      interpret.help(String.eq(name, "help"), name, rest)

def interpret.compact(eq: Bool, +name: String, +rest: String) -> Interpretation:
  match eq:
    case True{}:
      Send{"compact"}
    case False{}:
      interpret.interrupt(String.eq(name, "interrupt"), name, rest)

def interpret.notify.arg(empty: Bool, +rest: String) -> Interpretation:
  match empty:
    case True{}:
      Note{"/notify needs a message"}
    case False{}:
      Send{String.append("notify ", String.trim(rest))}

def interpret.notify(eq: Bool, +name: String, +rest: String) -> Interpretation:
  match eq:
    case True{}:
      interpret.notify.arg(String.is_empty(String.trim(rest)), rest)
    case False{}:
      interpret.compact(String.eq(name, "compact"), name, rest)

def interpret.steer.arg(empty: Bool, +rest: String) -> Interpretation:
  match empty:
    case True{}:
      Note{"/steer needs a message"}
    case False{}:
      Send{String.append("steer ", String.trim(rest))}

def interpret.steer(eq: Bool, +name: String, +rest: String) -> Interpretation:
  match eq:
    case True{}:
      interpret.steer.arg(String.is_empty(String.trim(rest)), rest)
    case False{}:
      interpret.notify(String.eq(name, "notify"), name, rest)

def interpret.slash(+name: String, +rest: String) -> Interpretation:
  interpret.steer(String.eq(name, "steer"), name, rest)

def interpret.line(empty: Bool, raw: Bool, is_slash: Bool, +op: String,
    +rest: String, +line: String) -> Interpretation:
  match empty:
    case True{}:
      # empty line: the raw dispatcher's business (it shows help)
      Send{line}
    case False{}:
      match raw:
        case True{}:
          Send{line}
        case False{}:
          match is_slash:
            case True{}:
              interpret.slash(String.drop(op, 1n), rest)
            case False{}:
              Send{String.append("say ", line)}

def interpret.cl2(cl: CmdLine, +line: String) -> Interpretation:
  match cl:
    case CL{+op, +rest}:
      interpret.line(String.is_empty(line), is_raw_word(op),
        String.starts_with(op, "/"), op, rest, line)

def interpret(+line: String) -> Interpretation:
  interpret.cl2(op_of(line), line)
