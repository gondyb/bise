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
import ./persist.bend as P
import ./mcp.bend as Mc
import 0x16458a2db4f36577294543ec2a6d420c/json.bend as Jv
import 0x9bfd9d57916f3439316c2775fd1f10b4/main.bend as Snap

# ---- the newline char (the mcp.bend nl() pattern) ----

def nl.get(m: Maybe<&2, Char>) -> Char:
  match m:
    case Some{+c}:
      c
    case None{}:
      Char.from_u32(10)

def nl() -> Char:
  nl.get(String.get("\n", 0n))

def tab() -> Char:
  nl.get(String.get("\t", 0n))

# ---- the index line format, parsed ----

type Skill is Data:
  Skill{name: String, desc: String, path: String}

def line_fields.name(fields: List<&2, String>) -> String:
  match fields:
    case Nil{}:
      ""
    case Con{+n, _}:
      n

def line_fields.desc(fields: List<&2, String>) -> String:
  match fields:
    case Con{_, Con{+d, _}}:
      d
    case _:
      ""

# the path is everything after the second tab, tabs rejoined (a path
# can never contain one, but the rejoin keeps the parse total)
def line_fields.tail_join(rest: List<&2, String>, +s: String) -> String:
  match rest:
    case Nil{}:
      s
    case Con{+x, +r2}:
      String.append(s, String.append("\t", line_fields.tail_join(r2, x)))

def line_fields.tail(rest: List<&2, String>) -> String:
  match rest:
    case Nil{}:
      ""
    case Con{+s, +r2}:
      line_fields.tail_join(r2, s)

def line_fields.path(fields: List<&2, String>) -> String:
  match fields:
    case Con{_, Con{_, +rest}}:
      line_fields.tail(rest)
    case _:
      ""

def skill_of_line.of(+fields: List<&2, String>) -> Skill:
  Skill{line_fields.name(fields), line_fields.desc(fields),
    line_fields.path(fields)}

def skill_of_line(line: String) -> Skill:
  skill_of_line.of(String.split(line, tab()))


def skill_name(+sk: Skill) -> String:
  match sk:
    case Skill{+name, _, _}:
      name

# a well-formed line: non-empty name and description, absolute path
def skill_ok(+sk: Skill) -> Bool:
  match sk:
    case Skill{+name, +desc, +path}:
      Bool.and(Bool.not(String.is_empty(name)),
        Bool.and(Bool.not(String.is_empty(desc)),
          String.starts_with(path, "/")))

def skill_ok_line(line: String) -> Bool:
  skill_ok(skill_of_line(line))

# ---- find a skill by exact name (first hit wins) ----

def find_skill.hit(+sk: Skill, +name: String) -> Bool:
  Bool.and(skill_ok(sk), String.eq(skill_name(sk), name))

def find_skill.put(hit: Bool, +sk: Skill,
    +rest_res: Maybe<&2, Skill>) -> Maybe<&2, Skill>:
  match hit:
    case True{}:
      Some{sk}
    case False{}:
      rest_res

def find_skill.go(lines: List<&2, String>, +name: String)
    -> Maybe<&2, Skill>:
  match lines:
    case Nil{}:
      None{}
    case Con{+l, rest}:
      find_skill.put(find_skill.hit(skill_of_line(l), name), skill_of_line(l),
        find_skill.go(rest, name))

def find_skill(+content: String, +name: String) -> Maybe<&2, Skill>:
  find_skill.go(String.split(content, nl()), name)

# ---- the prompt sections (pure; the laws pin the exact contracts) ----

# XML escaping per vibe_sdk prompt.rs: &, <, >, ", '
def esc_apos(is_a: Bool, +c: U32) -> String:
  match is_a:
    case True{}:
      "&apos;"
    case False{}:
      SCon{Chr{c}, SNil{}}

def esc_quot(is_q: Bool, +c: U32) -> String:
  match is_q:
    case True{}:
      "&quot;"
    case False{}:
      esc_apos(U32.is_eq(c, 39), c)

def esc_gt(is_gt: Bool, +c: U32) -> String:
  match is_gt:
    case True{}:
      "&gt;"
    case False{}:
      esc_quot(U32.is_eq(c, 34), c)

def esc_lt(is_lt: Bool, +c: U32) -> String:
  match is_lt:
    case True{}:
      "&lt;"
    case False{}:
      esc_gt(U32.is_eq(c, 62), c)

def esc_code(is_amp: Bool, +c: U32) -> String:
  match is_amp:
    case True{}:
      "&amp;"
    case False{}:
      esc_lt(U32.is_eq(c, 60), c)

def escape_xml(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      String.append(esc_code(U32.is_eq(c, 38), c), escape_xml(t))

# one <skill> entry, the exact vibe_sdk format
def skill_entry(+sk: Skill) -> String:
  match sk:
    case Skill{+name, +desc, +path}:
      String.append("  <skill>\n    <name>",
        String.append(escape_xml(name),
          String.append("</name>\n    <description>",
            String.append(escape_xml(desc),
              String.append("</description>\n    <path>",
                String.append(escape_xml(path),
                  "</path>\n  </skill>"))))))

# dedupe by name, first occurrence wins (the conn_names.go pattern)
def names_has(names: List<&2, String>, +v: String) -> Bool:
  match names:
    case Nil{}:
      False{}
    case Con{+n, rest}:
      Bool.or(String.eq(n, v), names_has(rest, v))

def catalog_put(ok: Bool, +line: String, +rest_out: String) -> String:
  match ok:
    case True{}:
      String.append(skill_entry(skill_of_line(line)),
        String.append("\n", rest_out))
    case False{}:
      rest_out

def catalog_go(lines: List<&2, String>, +seen: List<&2, String>,
    +rest_out: String) -> String:
  match lines:
    case Nil{}:
      rest_out
    case Con{+l, rest}:
      catalog_put(
        Bool.and(skill_ok_line(l),
          Bool.not(names_has(seen, skill_name(skill_of_line(l))))),
        l, catalog_go(rest, Con{skill_name(skill_of_line(l)), seen}, rest_out))

def skills_catalog.pick(empty: Bool, +content: String) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append("<available-skills>\n",
        String.append(catalog_go(String.split(content, nl()), Nil{}, ""),
          "</available-skills>"))

def skills_catalog(+content: String) -> String:
  skills_catalog.pick(
    String.is_empty(catalog_go(String.split(content, nl()), Nil{}, "")),
    content)

# the rules section, the exact vibe_sdk text (read_file -> bash: the
# Bend harness reaches files through the shell)
def skills_rules() -> String:
  "## Available Skills\n\nSkills contain task-specific instructions. When a request matches a skill description, proactively call the `skill` tool with its exact name before starting the task, then follow the loaded instructions. If that skill's full instructions are already present in the current context, follow them without calling `skill` again. Load every skill that materially applies.\n\nCall `skill` only with a name listed in `<available-skills>`. Connector names and unlisted names in skill instructions are not skills.\n\nDo not read a root `SKILL.md` path with `bash` after loading a skill. Use `bash` only for support or reference files mentioned by the loaded skill."

# ---- the tool contract ----

# the exact vibe_sdk tool description (tools.rs DESCRIPTION)
def SKILL_DESC() -> String:
  "Load a specialized skill that provides domain-specific instructions and workflows. When a task matches one of the available skills listed in your system prompt, call this tool with the exact skill name to load the full skill instructions. Only listed names are valid. Call exactly with {\"name\":\"<exact name>\"}; no other argument keys are accepted."

def SKILL_USAGE() -> String:
  "skill: call exactly with {\"name\":\"<exact name>\"}; no other argument keys are accepted"

# args are the exact vibe_sdk schema: {"name": "..."}
def args_name.pick(empty: Bool, +name: String) -> Maybe<&2, String>:
  match empty:
    case True{}:
      None{}
    case False{}:
      Some{name}

def args_name.w(+w: Jv.Val) -> Maybe<&2, String>:
  args_name.pick(String.is_empty(Mc.jstr(w, "name", "")),
    Mc.jstr(w, "name", ""))

def args_name.parse(m: Maybe<&2, Jv.Val>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+w}:
      args_name.w(w)

def args_name(+args: String) -> Maybe<&2, String>:
  args_name.parse(Jv.parse(args))

# ---- the miss error (the vibe_sdk contract, suggestions omitted:
# the available list is the operative part) ----

def avail_step(new: Bool, +name: String, +acc: List<&2, String>)
    -> List<&2, String>:
  match new:
    case True{}:
      Con{name, acc}
    case False{}:
      acc

def avail_names.go(lines: List<&2, String>, +seen: List<&2, String>,
    +acc: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+l, rest}:
      avail_names.go(rest, Con{skill_name(skill_of_line(l)), seen},
        avail_step(Bool.and(skill_ok_line(l),
            Bool.not(names_has(seen, skill_name(skill_of_line(l))))),
          skill_name(skill_of_line(l)), acc))

def quote_join(names: List<&2, String>) -> String:
  match names:
    case Nil{}:
      ""
    case Con{+n, rest}:
      String.append(String.append("\"", String.append(n, "\"")),
        quote_join(rest))

def avail_text(names: List<&2, String>) -> String:
  match names:
    case Nil{}:
      " No skills are available."
    case Con{_, _}:
      String.append(" Available skills: ",
        String.append(quote_join(names), "."))

def not_available(+name: String, +content: String) -> String:
  String.append("Skill \"",
    String.append(name,
      String.append("\" is not available. Use an exact name from the available skills below. Connector names and unlisted names in skill instructions are not skills.",
        avail_text(avail_names.go(String.split(content, nl()), Nil{},
          Nil{})))))

# ---- the index file (mirror of the MCP index pattern) ----

def env_or(r: Result<&1, &1, U32 & String, String>, +dflt: String) -> String:
  match r:
    case Done{+v}:
      v
    case Fail{_}:
      dflt

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
    +ix : String <- IO.pure(String, env_or(ir, ""))
    +hx : String <- IO.pure(String, env_or(hr, ""))
    IO.pure(String, index_path.pick(String.is_empty(ix), hx, ix))

# ---- the startup scan (live only) ----
# One POSIX shell script writes the index: every SKILL.md under the
# standard skill folders with a name: and a single-line description: in
# its frontmatter. Tabs and quotes are stripped from the fields so the
# tab-separated line stays unambiguous; folded multi-line YAML
# descriptions are out of scope (the Agent Skills format is one line).

def scan_script(+idx: String) -> String:
  String.append(": > \"",
    String.append(idx,
      String.append("\"\nfor d in \"$HOME/.agents/skills\" \"$HOME/.vibe/skills\" \".agents/skills\"; do\n  [ -d \"$d\" ] || continue\n  for f in \"$d\"/*/SKILL.md; do\n    [ -f \"$f\" ] || continue\n    n=$(sed -n 's/^name:[[:space:]]*//p' \"$f\" | head -n 1)\n    s=$(sed -n 's/^description:[[:space:]]*//p' \"$f\" | head -n 1)\n    n=$(printf '%s' \"$n\" | tr -d '\\t\\r\"')\n    s=$(printf '%s' \"$s\" | tr -d '\\t\\r\"')\n    [ -n \"$n\" ] && [ -n \"$s\" ] && printf '%s\\t%s\\t%s\\n' \"$n\" \"$s\" \"$f\" >> \"",
        String.append(idx, "\"\n  done\ndone"))))

def scan_path(+port: String) -> String:
  String.append("/tmp/bend-skills-scan-", String.append(port, ".sh"))

def skills_scan() -> IO(Unit):
  do IO<Unit>:
    +idx : String <- index_path()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, env_or(pr, "0"))
    +path : String <- IO.pure(String, scan_path(port))
    u : Unit <- P.write_file(path, scan_script(idx))
    out : String <- Snap.run(Con{"/bin/sh", Con{path, Nil{}}})
    IO.pure(Unit, Unit{})

# ---- frontmatter stripping (vibe_sdk ProjectSkill: the tool returns
# the CONTENT, never the YAML header) ----
# The fold pattern of checkpoint.bend: a pure per-line step over a
# state record, one self-recursive driver.

type FmMode is Data:
  FmBefore{}
  FmInside{}
  FmContent{}

type Fm is Data:
  Fm{mode: FmMode, acc: List<&2, String>}

def fm_open(+line: String) -> Bool:
  String.eq(String.trim(line), "---")

def fm_acc(+st: Fm) -> List<&2, String>:
  match st:
    case Fm{_, +acc}:
      acc

def fm_step.before(open: Bool, +st: Fm, +line: String) -> Fm:
  match open:
    case True{}:
      Fm{FmInside{}, fm_acc(st)}
    case False{}:
      # no frontmatter at all: everything is content
      Fm{FmContent{}, Con{line, fm_acc(st)}}

def fm_step.inside(open: Bool, +st: Fm) -> Fm:
  match open:
    case True{}:
      Fm{FmContent{}, fm_acc(st)}
    case False{}:
      st

def fm_step.content(+st: Fm, +line: String) -> Fm:
  Fm{FmContent{}, Con{line, fm_acc(st)}}

def fm_step.dispatch(mode: FmMode, +st: Fm, +line: String) -> Fm:
  match mode:
    case FmBefore{}:
      fm_step.before(fm_open(line), st, line)
    case FmInside{}:
      fm_step.inside(fm_open(line), st)
    case FmContent{}:
      fm_step.content(st, line)

def fm_step(+st: Fm, +line: String) -> Fm:
  match st:
    case Fm{+mode, _}:
      fm_step.dispatch(mode, st, line)

def fm_go(lines: List<&2, String>, st: Fm) -> Fm:
  match lines:
    case Nil{}:
      st
    case Con{+l, rest}:
      fm_go(rest, fm_step(st, l))

def strip_frontmatter.acc(st: Fm) -> String:
  match st:
    case Fm{_, +acc}:
      String.trim(String.join(List.reverse(&2, String, acc), "\n"))

def strip_frontmatter(+content: String) -> String:
  strip_frontmatter.acc(fm_go(String.split(content, nl()),
    Fm{FmBefore{}, Nil{}}))

# ---- execution ----

def skill_path(+sk: Skill) -> String:
  match sk:
    case Skill{_, _, +path}:
      path

def read_skill.go(m: Maybe<&2, String>, +sk: Skill) -> Mc.MR:
  match m:
    case None{}:
      Mc.MR{False{},
        String.append("skill file unreadable: ", skill_path(sk))}
    case Some{+content}:
      Mc.MR{True{}, strip_frontmatter(content)}

def read_skill(sk: Skill) -> IO(Mc.MR):
  match sk:
    case Skill{_, _, +path}:
      do IO<Mc.MR>:
        m : Maybe<&2, String> <- P.read_file(path)
        IO.pure(Mc.MR, read_skill.go(m, sk))

def exec_skill.live.found(fs: Maybe<&2, Skill>) -> IO(Mc.MR):
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
      exec_skill.live.found(find_skill(content, name))

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
    n : Maybe<&2, String> <- IO.pure(Maybe<&2, String>, args_name(args))
    exec_skill.live.pick(n)

def exec_skill.scripted() -> Mc.MR:
  Mc.MR{False{}, "skills need a live session"}

def exec_skill(+args: String, live: Bool) -> IO(Mc.MR):
  match live:
    case True{}:
      exec_skill.live(args)
    case False{}:
      IO.pure(Mc.MR, exec_skill.scripted())

# ---- the live prompt section assembly (repl-live uses these) ----

def prompt_with.pick(empty: Bool, +base: String, +content: String) -> String:
  # rules then catalog; both only when skills exist
  match empty:
    case True{}:
      base
    case False{}:
      String.append(base,
        String.append("\n\n",
          String.append(skills_rules(),
            String.append("\n\n", skills_catalog(content)))))

def prompt_with(+base: String, +content: String) -> String:
  prompt_with.pick(String.is_empty(skills_catalog(content)), base, content)

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
