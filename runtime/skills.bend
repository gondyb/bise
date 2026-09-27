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
import 0x1f4d6c03caf955232d0b0dc6e6f36cf4/json.bend as J
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap
import ./skills-pure.bend as Skp

# ---- the tool contract ----

def SKILL_USAGE() -> String:
  "skill: call exactly with {\"name\":\"<exact name>\"}; no other argument keys are accepted"

# ---- the index file (mirror of the MCP index pattern) ----

def index_path.pick2(empty: Bool, +fallback: String, +path: String) -> String:
  match empty:
    case True{}:
      fallback
    case False{}:
      path

def index_path.default(+home: String) -> String:
  String.append(home, "/.bend-harness/skills-index.txt")

def index_path.pick(empty: Bool, +home: String, +env: String) -> String:
  index_path.pick2(empty, index_path.default(home), env)

def index_path() -> IO(String):
  do IO<String>:
    ir : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_SKILLS_INDEX")
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    +ix : String <- IO.pure(String, P.env_or(ir, ""))
    +hx : String <- IO.pure(String, P.env_or(hr, ""))
    IO.pure(String, index_path.pick(String.is_empty(ix), hx, ix))

# ---- the startup scan (live only) ----
# One POSIX shell script writes the index: every SKILL.md under the
# standard skill folders with a name: and a single-line description: in
# its frontmatter. The repo folder is scanned as $PWD/.agents/skills:
# skill_ok keeps absolute paths only, so a relative one would drop
# every local skill. Tabs and quotes are stripped from the fields so the
# tab-separated line stays unambiguous; folded multi-line YAML
# descriptions are out of scope (the Agent Skills format is one line).

def scan_script(+idx: String) -> String:
  String.append(": > \"",
    String.append(idx,
      String.append("\"\nfor d in \"$HOME/.agents/skills\" \"$HOME/.vibe/skills\" \"$PWD/.agents/skills\"; do\n  [ -d \"$d\" ] || continue\n  for f in \"$d\"/*/SKILL.md; do\n    [ -f \"$f\" ] || continue\n    n=$(sed -n 's/^name:[[:space:]]*//p' \"$f\" | head -n 1)\n    s=$(sed -n 's/^description:[[:space:]]*//p' \"$f\" | head -n 1)\n    n=$(printf '%s' \"$n\" | tr -d '\\t\\r\"')\n    s=$(printf '%s' \"$s\" | tr -d '\\t\\r\"')\n    [ -n \"$n\" ] && [ -n \"$s\" ] && printf '%s\\t%s\\t%s\\n' \"$n\" \"$s\" \"$f\" >> \"",
        String.append(idx, "\"\n  done\ndone"))))

def scan_path(+port: String) -> String:
  String.append("/tmp/bend-skills-scan-", String.append(port, ".sh"))

def skills_scan() -> IO(Unit):
  do IO<Unit>:
    +idx : String <- index_path()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    +path : String <- IO.pure(String, scan_path(port))
    u : Unit <- P.write_file(path, scan_script(idx))
    out : String <- Snap.run(Con{"/bin/sh", Con{path, Nil{}}})
    IO.pure(Unit, Unit{})

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

def exec_skill.live.found(fs: Maybe<&2, Skp.Skill>) -> IO(Mc.MR):
  match fs:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{}, "skills index unreadable: no skills are available"})
    case Some{+sk}:
      read_skill(sk)

def exec_skill.live.go(m: Maybe<&2, String>, +name: String) -> IO(Mc.MR):
  match m:
    case None{}:
      IO.pure(Mc.MR,
        Mc.MR{False{}, "skills index unreadable: no skills are available"})
    case Some{+content}:
      exec_skill.live.found(Skp.find_skill(content, name))

def exec_skill.live.pick(n: Maybe<&2, String>) -> IO(Mc.MR):
  match n:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{}, SKILL_USAGE()})
    case Some{+name}:
      do IO<Mc.MR>:
        idx : String <- index_path()
        m : Maybe<&2, String> <- P.read_file(idx)
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

def prompt_content.read(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{+c}:
      c

def prompt_content() -> IO(String):
  do IO<String>:
    idx : String <- index_path()
    m : Maybe<&2, String> <- P.read_file(idx)
    IO.pure(String, prompt_content.read(m))
