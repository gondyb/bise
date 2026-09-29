# core/obs.bend — observation rendering, pure.
#
# Every obs renders as ONE line with the "  obs: " prefix: the wire
# contract every client (TUI, nc) parses. LAWS.bend pins the format.
import Base
import ./types.bend as T
import ./wire.bend as W

# a preview must stay on ONE wire line: newlines become spaces (a raw
# newline in an obs splits it, and the client reads the tail as noise).
# In-session texts carry newlines escaped (backslash-n, the wire
# convention): the preview unescapes them first, then flattens.
def one_line.code(nl: Bool, c: U32) -> U32:
  match nl:
    case True{}:
      32
    case False{}:
      c

def one_line(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, +t}:
      SCon{Chr{one_line.code(U32.is_eq(c, 10), c)}, one_line(t)}

def show_obs(o: T.Obs) -> String:
  match o:
    case T.TurnStarted{}:
      "  obs: turn_started"
    case T.SteerReceived{text}:
      String.append("  obs: steering_received: ", text)
    case T.Steered{text}:
      String.append("  obs: steered: ", text)
    case T.NotifReceived{text}:
      String.append("  obs: notification_received: ", text)
    case T.NotifDelivered{text}:
      String.append("  obs: notification_delivered: ", text)
    case T.CandidateDiscarded{cause}:
      String.append("  obs: candidate_discarded: ", cause)
    case T.Assistant{text}:
      String.append("  obs: assistant: ", text)
    case T.ToolStarted{call}:
      String.append("  obs: tool_started #", U32.show(call))
    case T.ToolFinished{call, ok}:
      match ok:
        case True{}:
          String.append("  obs: tool_finished #", String.append(U32.show(call), " ok"))
        case False{}:
          String.append("  obs: tool_finished #", String.append(U32.show(call), " failed"))
    case T.ToolCommitted{call}:
      String.append("  obs: tool_result_committed #", U32.show(call))
    case T.CompactionStarted{compaction, auto}:
      match auto:
        case True{}:
          String.append("  obs: compaction_started #", String.append(U32.show(compaction), " (auto)"))
        case False{}:
          String.append("  obs: compaction_started #", String.append(U32.show(compaction), " (manual)"))
    case T.CompactionFailed{code}:
      String.append("  obs: context_compaction_failed: ", code)
    case T.CompactionDone{summary}:
      String.append("  obs: compaction_done: ", String.append(String.take(one_line(String.trim(W.unescape_nl(summary))), 40n), "..."))
    case T.NullIteration{}:
      "  obs: null_iteration"
    case T.TurnDone{outcome}:
      String.append("  obs: turn_done: ", outcome)
