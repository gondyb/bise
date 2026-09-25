# runtime/tools.bend — the Runtime's tool executors.
#
# Tools are pure functions where possible; self.sleep is a Runtime timer
# (D9 bis: bounded 0 < seconds < 60) and never a completion tool.
import Base
import ../core/types.bend as T
import ../core/discovery.bend as D

def catalog() -> List<&2, T.ToolDef>:
  Con{T.ToolDef{"search_tool_functions", "Search every available tool: the local catalog AND the remote MCP connector tools (Gmail, GitHub, Slack, Granola...). Args: 'best_match: query' or 'details: name name'. Remote tools are named '<connector>.<tool>' and can be called directly."},
   Con{T.ToolDef{"self.sleep", "Sleep for a bounded number of seconds (0 < s < 60), then report."},
    Con{T.ToolDef{"bash", "Run a real shell command and return its output. Args: the command."},
     Con{T.ToolDef{"run_program", "Run a real TypeScript/JavaScript program of tool calls in one turn (executed by a V8 engine with a 10s budget). Args: the source. Call tools by name as functions (any discoverable tool, including remote MCP); results arrive as values; console.log for debug. Example: const me = github_app.get_me({}); if (me.login) { return me.login; } return fallback;"},
      Nil{}}}}}

def search_reply.mode(details: Bool, +args: String,
    +tools: List<&2, T.ToolDef>) -> String:
  match details:
    case True{}:
      D.show_tools(D.search_details(
        String.split(String.trim(String.drop(args, 8n)), ' '), tools))
    case False{}:
      D.show_tools(D.best_match(
        String.trim(String.drop(args, 11n)), tools, 5n))

def search_reply.pick(known: Bool, +args: String, +tools: List<&2, T.ToolDef>)
    -> String:
  match known:
    case True{}:
      search_reply.mode(String.starts_with(args, "details:"), args, tools)
    case False{}:
      # bare query: best match
      D.show_tools(D.best_match(args, tools, 5n))

def search_reply(+args: String, tools: List<&2, T.ToolDef>) -> String:
  search_reply.pick(
    Bool.or(String.starts_with(args, "details:"),
      String.starts_with(args, "best_match:")),
    args, tools)
