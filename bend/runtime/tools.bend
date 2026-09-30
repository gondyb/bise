# runtime/tools.bend — the Runtime's tool catalog.
#
# Tool descriptions for run_typescript and search_tool_functions are
# exact copies from the Vibe SDK (vibe_sdk/harness/core), loaded from
# tool-desc-*.txt so the literal braces in the docs don't fight the
# Bend string parser. Schemas live in core/api.bend.
import Base
import ../core/types.bend as T
import ../core/discovery.bend as D
import ./persist.bend as P
import ../vendor/json.bend as J
import ./tools-pure.bend as Xtp

def search_desc() -> IO(String):
  P.read_or("prompts/tool-desc-search.txt",
    "Discover callable tool functions for run_typescript.")

def rt_desc() -> IO(String):
  P.read_or("prompts/tool-desc-run-typescript.txt",
    "Executes TypeScript code in a sandbox environment.")

def bash_desc() -> IO(String):
  P.read_or("prompts/tool-desc-bash.txt",
    "Run a real shell command and return its output. Args: the command.")

def skill_desc() -> IO(String):
  P.read_or("prompts/tool-desc-skill.txt",
    "Load a specialized skill that provides domain-specific instructions and workflows.")

def patch_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("prompts/tool-desc-apply-patch.txt")
    IO.pure(String, Xtp.patch_desc.read(m))

# the catalog of one provider's requests: `patch` = the OpenAI provider
# (apply_patch), else Vibe's edit and write_file (tools-pure.bend)
def catalog_for(+patch: Bool) -> IO(List<&2, T.ToolDef>):
  do IO<List<&2, T.ToolDef>>:
    +sd : String <- search_desc()
    +rd : String <- rt_desc()
    +bd : String <- bash_desc()
    +kd : String <- skill_desc()
    +pd : String <- patch_desc()
    IO.pure(List<&2, T.ToolDef>, Xtp.catalog_live(sd, rd, bd, kd, pd, patch))

# the tool names only matter here (the MCP groups section)
def catalog() -> IO(List<&2, T.ToolDef>):
  catalog_for(False{})

# short fallback for pure contexts (laws, scenarios that don't need
# the full vibe_sdk description text)
def catalog_short() -> List<&2, T.ToolDef>:
  Xtp.catalog_pure(
    "Discover callable tool functions for run_typescript. Use mode=best_match first with a capability query; it returns ranked exact function names. If a returned name fits, call mode=details with that exact name before using it.",
    "Executes TypeScript code in a sandbox environment. The sandbox expects an async function main() containing all your code. Tool functions are available under tools.<group>.<function>; no imports needed. The result of main() is the tool output.",
    "Run a real shell command and return its output. Args: the command.")

def exec_cmd.pick(ok: Bool, +out: String, +id: U32) -> T.Cmd:
  match ok:
    case True{}:
      T.ToolOk{id, Xtp.cap_out(out)}
    case False{}:
      T.ToolFail{id, Xtp.cap_out(out)}

def exec_cmd(r: Xtp.ExecRes, +id: U32) -> T.Cmd:
  match r:
    case Xtp.Ex{+ok, +out}:
      exec_cmd.pick(ok, out, id)
