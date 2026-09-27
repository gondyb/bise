# runtime/settings.bend — the config file (~/.bend-harness/config.toml).
#
# Where the file lives, reading it, and writing the commented template
# on first run. The parsing itself is pure: core/config.bend.
import Base
import ../core/text.bend as Tx
import ./persist.bend as P
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
# ---- the config file (~/.bend-harness/config.toml) ----
# The vibe CLI ergonomics: a commented template is written on first
# run; every key answers the same precedence - environment variable >
# config file > built-in default. BEND_CONFIG points at another path.

def cfg_path_pick.home(empty: Bool, +home: String) -> String:
  match empty:
    case True{}:
      "/tmp/bend-harness-config.toml"
    case False{}:
      String.append(home, "/.bend-harness/config.toml")

def cfg_path_pick.pick(empty: Bool, +custom: String, +home: String)
    -> String:
  match empty:
    case True{}:
      cfg_path_pick.home(String.is_empty(home), home)
    case False{}:
      custom

def cfg_path_pick(+custom: String, +home: String) -> String:
  cfg_path_pick.pick(String.is_empty(custom), custom, home)

def cfg_path() -> IO(String):
  do IO<String>:
    cr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_CONFIG")
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    IO.pure(String,
      cfg_path_pick(P.env_or(cr, ""), P.env_or(hr, "")))

def cfg_text_of(m: Maybe<&2, String>) -> String:
  match m:
    case Some{+t}:
      t
    case None{}:
      ""

def read_cfg_m() -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    +path : String <- cfg_path()
    m : Maybe<&2, String> <- P.read_file(path)
    IO.pure(Maybe<&2, String>, m)

def read_cfg() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- read_cfg_m()
    IO.pure(String, cfg_text_of(m))

def CONFIG_TEMPLATE() -> String:
  "# bend-harness configuration.\n# Precedence on every key: environment variable > this file > built-in default.\n\n# The model to use: a name or an alias from the provider table\n# (opus-5.5 / claude-opus-5-5 -> foundry-anthropic; zai-glm-5-3 -> mistral)\nmodel = \"opus-5.5\"\n\n# The compaction threshold, in estimated tokens (default 800000)\nthreshold = 800000\n\n# A bash command still running after this many seconds moves to the\n# background and the agent keeps going (default 30)\nbg_after = 30\n"

# the commented template lands on disk when the file does not exist
def write_cfg_template.m(m: Maybe<&2, String>, +path: String)
    -> IO(Unit):
  match m:
    case Some{_}:
      IO.pure(Unit, Unit{})
    case None{}:
      do IO<Unit>:
        o : String <- Snap.run(Con{"/bin/sh", Con{"-c",
          Con{String.append("mkdir -p -- \"$(dirname -- ",
            String.append(Tx.sh_quote(path), ")\"")), Nil{}}}})
        w : Unit <- P.write_file(path, CONFIG_TEMPLATE())
        IO.pure(Unit, Unit{})

def write_cfg_template() -> IO(Unit):
  do IO<Unit>:
    +path : String <- cfg_path()
    m : Maybe<&2, String> <- P.read_file(path)
    u : Unit <- write_cfg_template.m(m, path)
    IO.pure(Unit, Unit{})
