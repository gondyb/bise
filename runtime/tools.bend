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
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J
import ./tools-pure.bend as Xtp

def search_desc.read(m: Maybe<&2, String>) -> String:
  Xtp.desc_or(m, "Discover callable tool functions for run_typescript.")

def search_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-search.txt")
    IO.pure(String, search_desc.read(m))

def rt_desc.read(m: Maybe<&2, String>) -> String:
  Xtp.desc_or(m, "Executes TypeScript code in a sandbox environment.")

def rt_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-run-typescript.txt")
    IO.pure(String, rt_desc.read(m))

def bash_desc.read(m: Maybe<&2, String>) -> String:
  Xtp.desc_or(m, "Run a real shell command and return its output. Args: the command.")

def bash_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-bash.txt")
    IO.pure(String, bash_desc.read(m))

def skill_desc.read(m: Maybe<&2, String>) -> String:
  Xtp.desc_or(m, "Load a specialized skill that provides domain-specific instructions and workflows.")

def skill_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-skill.txt")
    IO.pure(String, skill_desc.read(m))

def patch_desc() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file("tool-desc-apply-patch.txt")
    IO.pure(String, Xtp.patch_desc.read(m))

def catalog() -> IO(List<&2, T.ToolDef>):
  do IO<List<&2, T.ToolDef>>:
    +sd : String <- search_desc()
    +rd : String <- rt_desc()
    +bd : String <- bash_desc()
    +kd : String <- skill_desc()
    +pd : String <- patch_desc()
    IO.pure(List<&2, T.ToolDef>, Xtp.catalog_live(sd, rd, bd, kd, pd))

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
