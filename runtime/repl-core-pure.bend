# runtime/repl-core-pure.bend - the pure half of runtime/repl-core.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/wire.bend as W
import ./model.bend as M
import ../core/types.bend as T
import ../core/text.bend as Tx

# ---- what one input line asks for ----

type Plan is Data:
  PRun{cmds: List<&2, T.Cmd>, script: List<&2, M.Reply>}
  PHelp{}
  PQuit{}
  PReload{}

# the scripted model's plan for a turn
def script_for.prog(is_prog: Bool, +text: String) -> List<&2, M.Reply>:
  match is_prog:
    case True{}:
      Con{M.ROk{"", Con{T.Call{1, "run_typescript",
          String.trim(String.drop(text, 5n))}, Nil{}}},
        Con{M.ROk{"Program complete.", Nil{}}, Nil{}}}
    case False{}:
      Con{M.ROk{String.append("You said: ", text), Nil{}}, Nil{}}

def script_for(is_tools: Bool, is_prog: Bool, +text: String)
    -> List<&2, M.Reply>:
  match is_tools:
    case True{}:
      Con{M.ROk{"", Con{T.Call{1, "search_tool_functions", "best_match: catalog"},
            Nil{}}},
        Con{M.ROk{"I searched the catalog.", Nil{}}, Nil{}}}
    case False{}:
      script_for.prog(is_prog, text)

def script_for_entry(+text: String) -> List<&2, M.Reply>:
  script_for(String.starts_with(text, "tools"),
    String.starts_with(text, "prog:"), text)

def plan_reload(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PReload{}
    case False{}:
      PHelp{}

def plan_quit(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PQuit{}
    case False{}:
      plan_reload(String.eq(op, "reload"), op)

def plan_interrupt(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Interrupt{}, Nil{}}, Nil{}}
    case False{}:
      plan_quit(String.eq(op, "quit"), op)

def plan_compact(eq: Bool, +op: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Compact{""}, Nil{}},
        Con{M.ROk{"<summary>The REPL conversation so far, compacted on request.</summary>", Nil{}},
          Nil{}}}
    case False{}:
      plan_interrupt(String.eq(op, "interrupt"), op)

def plan_notify(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.Notify{rest}, Nil{}}, Nil{}}
    case False{}:
      plan_compact(String.eq(op, "compact"), op)

def plan_steer(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      PRun{Con{T.UserMsg{T.Steer{}, W.unescape_nl(rest)}, Nil{}},
        script_for_entry(W.unescape_nl(rest))}
    case False{}:
      plan_notify(String.eq(op, "notify"), op, rest)

def plan_say(eq: Bool, +op: String, +rest: String) -> Plan:
  match eq:
    case True{}:
      # the client escapes real newlines to keep the socket line
      # single-line; the message text carries the real newlines
      PRun{Con{T.UserMsg{T.Queue{}, W.unescape_nl(rest)}, Nil{}},
        script_for_entry(W.unescape_nl(rest))}
    case False{}:
      plan_steer(String.eq(op, "steer"), op, rest)

# a chunk may carry SEVERAL lines (kernel coalescing while a turn or a
# compaction ran); each line becomes its own command, run in order
def cmd_lines.keep(empty: Bool, +l: String, +rest: List<&2, String>)
    -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{l, rest}

def cmd_lines.go(lines: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      cmd_lines.keep(String.is_empty(String.trim(l)), String.trim(l),
        cmd_lines.go(rest))

def cmd_lines(+data: String) -> List<&2, String>:
  cmd_lines.go(String.split(data, Tx.newline()))

# ---- BEND_EXTRA_PROMPT: a role appended to the system prompt ----
# A switchboard hub gives each agent its role (main, or a task with its
# brief rules) as a file; the live prompt ends with it. Empty: the
# prompt is unchanged.
def prompt_extra.pick(empty: Bool, +base: String, +extra: String)
    -> String:
  match empty:
    case True{}:
      base
    case False{}:
      String.append(base, String.append("\n\n", extra))

def prompt_extra(+base: String, +extra: String) -> String:
  prompt_extra.pick(String.is_empty(String.trim(extra)), base, extra)
