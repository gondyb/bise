# runtime/bash.bend — the bash tool.
#
# A command runs inside a generated /bin/sh wrapper: a sync window, then
# a background handoff with a file contract (slot dir, out, rc, stdin
# fifo). The contract text is pure and pinned by LAWS.bend; bash_exec
# is the one effect.
import Base
import ../core/text.bend as Tx
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ./persist.bend as P
import ./mcp.bend as Mcp
import ../core/config.bend as Cf
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
import ./settings.bend as St
import ./bash-pure.bend as Shp
def bg_after.env(+envv: String, +dflt: U32) -> U32:
  Shp.bg_after.read(U32.read(envv), dflt)

def bg_after.pick2(empty: Bool, +envv: String, +cfgt: String,
    +dflt: U32) -> U32:
  match empty:
    case True{}:
      Cf.cfg_u32(cfgt, "bg_after", dflt)
    case False{}:
      bg_after.env(envv, dflt)

def bg_after.or_cfg(+envv: String, +cfgt: String, +dflt: U32) -> U32:
  bg_after.pick2(String.is_empty(envv), envv, cfgt, dflt)

# env > config > default; the config file only feeds LIVE sessions
# (BEND_REPL_PORT is the live marker - the scripted suites stay
# hermetic against the user's config)
def bg_after.pick(port_empty: Bool, +envv: String) -> IO(U32):
  match port_empty:
    case True{}:
      IO.pure(U32, Shp.bg_after.clamp(bg_after.env(envv, 30)))
    case False{}:
      do IO<U32>:
        +cfgt : String <- St.read_cfg()
        IO.pure(U32, Shp.bg_after.clamp(bg_after.or_cfg(envv, cfgt, 30)))

def bg_after() -> IO(U32):
  do IO<U32>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_AFTER")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    bg_after.pick(String.is_empty(P.env_or(pr, "")), P.env_or(r, ""))

def bg_dir() -> IO(String):
  do IO<String>:
    root : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_ROOT")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +r : String <- IO.pure(String, P.env_or(root, "/tmp"))
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    IO.pure(String, Shp.bg_dir_of(r, p))

@unsafe
def bash_exec(command: String) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    +secs : U32 <- bg_after()
    +bg : String <- bg_dir()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    # NO flatten: real newlines stay — the heredoc and the runner
    # subshell both accept multi-line commands (flatten silently
    # restructured them)
    +script : String <- IO.pure(String,
      Shp.bg_wrap(String.trim(command), bg, secs))
    +path : String <- IO.pure(String, Shp.bash_script_path(port, script))
    w : Unit <- P.write_file(path, script)
    out : String <- Snap.run(Con{"/bin/sh", Con{path, Nil{}}})
    IO.pure(Xp.ExecRes, Shp.exec_of(out))

# boot cleanup: the wrappers of a previous process on this port cannot be
# running (the parent waits for the old REPL to die before respawning), so
# stale /tmp/bend-sh-<port>-*.sh files are safe to remove - the wrapper
# leak (one file per bash call) would otherwise grow without bound.
def bash_cleanup(+port: String) -> IO(Unit):
  do IO<Unit>:
    out : String <- Snap.run(Con{"/bin/sh", Con{"-c",
      Con{String.append("rm -f /tmp/bend-sh-",
        String.append(port, "-*.sh")), Nil{}}}})
    IO.pure(Unit, Unit{})

def exec_mr(m: Mcp.MR) -> Xp.ExecRes:
  match m:
    case Mcp.MR{+ok, +out}:
      Xp.Ex{ok, out}

def exec_search(+args: String, +live: Bool) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : Mcp.MR <- Mcp.merged_search(args, live)
    IO.pure(Xp.ExecRes, exec_mr(r))

def exec_mcp.key(+name: String, +args: String, +key: String)
    -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : Mcp.MR <- Mcp.mcp_call(name, args, key)
    IO.pure(Xp.ExecRes, exec_mr(r))

def exec_mcp.scripted(+name: String) -> Xp.ExecRes:
  Xp.Ex{False{},
    String.append("unknown tool: ", String.append(name,
      " — remote MCP tools need a live session"))}

def exec_mcp.live(+name: String, +args: String) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    exec_mcp.key(name, args, key)

def exec_mcp(+name: String, +args: String, live: Bool) -> IO(Xp.ExecRes):
  match live:
    case True{}:
      exec_mcp.live(name, args)
    case False{}:
      IO.pure(Xp.ExecRes, exec_mcp.scripted(name))
