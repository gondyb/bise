# runtime/skills.bend — the Agent Skills feature (Vibe CLI design).
#
# Progressive disclosure, exactly as vibe_sdk/harness/core
# features/skills: the system prompt carries ONLY name + description +
# path per skill (an <available-skills> section), and the `skill` tool
# loads the full SKILL.md instructions on demand. Callers must use an
# exact name from the section; a miss returns the error contract with
# the available list.
#
# The catalog is a local index file written at live startup by scanning
# the standard skill folders (~/.agents/skills, ~/.vibe/skills, and the
# repo's .agents/skills) for SKILL.md files with YAML frontmatter
# (name:, description:) — the Agent Skills format. Scripted runs never
# scan and never read the index: the scenarios stay hermetic.
#
# index line: "name\tdescription\tpath" (tab-separated; the scan strips
# tabs and quotes from the fields, so the split is unambiguous)
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ./persist.bend as P
import ./mcp.bend as Mc
import ../vendor/json.bend as J
import ./proc.bend as Proc
import ./skills-pure.bend as Skp
import ./plugins.bend as Pl

# ---- the tool contract ----

def SKILL_USAGE() -> String:
  "skill: call exactly with {\"name\":\"<exact name>\"}; no other argument keys are accepted"

# ---- the index file (mirror of the MCP index pattern) ----

def index_path() -> IO(String):
  P.env_path("BEND_SKILLS_INDEX", "/.bend-harness/skills-index.txt")

# ---- the startup scan (live only) ----
# One POSIX shell script writes two indexes: every SKILL.md under a
# skill folder with a name: and a single-line description: in its
# frontmatter.
#   $1 the shared index: the user folders (~/.agents/skills,
#      ~/.vibe/skills); the TUI `$` popup reads it
#   $2 this session's index: the workspace folder
#      ($BEND_WORKDIR/.agents/skills - the hub runs a task's REPL in the
#      app root, so $PWD would miss the task's workspace - else
#      $PWD/.agents/skills), then the plugin skills ($3, written by the
#      plugins bridge, runtime/plugins.bend)
# Each index is written to a temp file then renamed: every REPL start
# rescans, and a skill call or the TUI reading the shared index mid-scan
# must see the old index or the new one, never a truncated file.
# skill_ok keeps absolute paths only, so a relative one would drop
# every local skill. Tabs and quotes are stripped from the fields so the
# tab-separated line stays unambiguous; folded multi-line YAML
# descriptions are out of scope (the Agent Skills format is one line).

def scan_script() -> String:
  "scan() {
  [ -d \"$1\" ] || return 0
  for f in \"$1\"/*/SKILL.md; do
    [ -f \"$f\" ] || continue
    n=$(sed -n 's/^name:[[:space:]]*//p' \"$f\" | head -n 1)
    s=$(sed -n 's/^description:[[:space:]]*//p' \"$f\" | head -n 1)
    n=$(printf '%s' \"$n\" | tr -d '\\t\\r\"')
    s=$(printf '%s' \"$s\" | tr -d '\\t\\r\"')
    [ -n \"$n\" ] && [ -n \"$s\" ] && printf '%s\\t%s\\t%s\\n' \"$n\" \"$s\" \"$f\"
  done
}
mkdir -p \"$(dirname \"$1\")\" \"$(dirname \"$2\")\"
{ scan \"$HOME/.agents/skills\"; scan \"$HOME/.vibe/skills\"; } > \"$1.$$\" && mv -f \"$1.$$\" \"$1\"
w=\"${BEND_WORKDIR:-$PWD}\"
{ scan \"$w/.agents/skills\"; cat \"$3\" 2>/dev/null; } > \"$2.$$\" && mv -f \"$2.$$\" \"$2\""

def scan_path(+port: String) -> String:
  String.append("/tmp/bend-skills-scan-", String.append(port, ".sh"))

def session_index_path() -> IO(String):
  do IO<String>:
    +r : String <- Pl.run_dir()
    IO.pure(String, String.append(r, "/skills-index.txt"))

def skills_scan() -> IO(Unit):
  do IO<Unit>:
    +idx : String <- index_path()
    +sidx : String <- session_index_path()
    +psk : String <- Pl.skills_index_path()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    +path : String <- IO.pure(String, scan_path(port))
    u : Unit <- P.write_file(path, scan_script())
    out : String <- Proc.run(Con{"/bin/sh", Con{path, Con{idx, Con{sidx, Con{psk, Nil{}}}}}})
    IO.pure(Unit, Unit{})

# the index the catalog and the skill tool read: this session's index
# first (workspace and plugin skills win a name clash: the first
# occurrence of a name is kept), then the shared one
def index_content.join(ms: Maybe<&2, String>, mu: Maybe<&2, String>)
    -> Maybe<&2, String>:
  match ms:
    case None{}:
      mu
    case Some{+s}:
      Some{String.append(s, String.append("
", Maybe.default(&2, String, mu, "")))}

def index_content() -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    +sidx : String <- session_index_path()
    +idx : String <- index_path()
    ms : Maybe<&2, String> <- P.read_file(sidx)
    mu : Maybe<&2, String> <- P.read_file(idx)
    IO.pure(Maybe<&2, String>, index_content.join(ms, mu))

# ---- execution ----

def skill_path(+sk: Skp.Skill) -> String:
  match sk:
    case Skp.Skill{_, _, +path}:
      path

def read_skill.go(m: Maybe<&2, String>, +sk: Skp.Skill) -> Mc.MR:
  match m:
    case None{}:
      Mc.MR{False{},
        String.append("skill file unreadable: ", skill_path(sk))}
    case Some{+content}:
      Mc.MR{True{}, Skp.strip_frontmatter(content)}

def read_skill(sk: Skp.Skill) -> IO(Mc.MR):
  match sk:
    case Skp.Skill{_, _, +path}:
      do IO<Mc.MR>:
        m : Maybe<&2, String> <- P.read_file(path)
        IO.pure(Mc.MR, read_skill.go(m, sk))

# a name the index lacks is not an unreadable index (an empty index said
# "unreadable" and sent the search to the wrong place)
def exec_skill.live.found(fs: Maybe<&2, Skp.Skill>, +name: String) -> IO(Mc.MR):
  match fs:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{},
        String.append("unknown skill: ", String.append(name,
          ": call it with an exact name from <available-skills>"))})
    case Some{+sk}:
      read_skill(sk)

def exec_skill.live.go(m: Maybe<&2, String>, +name: String) -> IO(Mc.MR):
  match m:
    case None{}:
      IO.pure(Mc.MR,
        Mc.MR{False{}, "skills index unreadable: no skills are available"})
    case Some{+content}:
      exec_skill.live.found(Skp.find_skill(content, name), name)

def exec_skill.live.pick(n: Maybe<&2, String>) -> IO(Mc.MR):
  match n:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{}, SKILL_USAGE()})
    case Some{+name}:
      do IO<Mc.MR>:
        m : Maybe<&2, String> <- index_content()
        exec_skill.live.go(m, name)

def exec_skill.live(+args: String) -> IO(Mc.MR):
  do IO<Mc.MR>:
    n : Maybe<&2, String> <- IO.pure(Maybe<&2, String>, Skp.args_name(args))
    exec_skill.live.pick(n)

def exec_skill.scripted() -> Mc.MR:
  Mc.MR{False{}, "skills need a live session"}

def exec_skill(+args: String, live: Bool) -> IO(Mc.MR):
  match live:
    case True{}:
      exec_skill.live(args)
    case False{}:
      IO.pure(Mc.MR, exec_skill.scripted())

def prompt_content() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- index_content()
    IO.pure(String, Maybe.default(&2, String, m, ""))
