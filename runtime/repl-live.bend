# runtime/repl-live.bend — the live REPL entry (real provider, real tools).
#
#   bend runtime/repl-live.bend -o repl-live && ./repl-live
#   nc 127.0.0.1 7702
#
# The provider call and the bash tool run INSIDE this process: the hub
# HTTP client does the TLS call to api.mistral.ai (core/api.bend maps
# the wire request to the API JSON and the reply back), and Base's
# Process.run (runtime/proc.bend) runs /bin/sh. The line server itself is
# runtime/repl-core.bend; this file builds the live session config.
import Base
import ../core/types.bend as T
import ./persist.bend as P
import ./mcp.bend as Mcp
import ./skills.bend as Sk
import ./plugins.bend as Pl
import ./skills-pure.bend as Skp
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ./main.bend as R
import ../runtime/bash.bend as Sh
import ../runtime/provider.bend as Pv
import ../runtime/provider-pure.bend as Pvp
import ../runtime/settings.bend as St
import ./repl-core.bend as Rc
import ./debug.bend as Dbg
import ./repl-core-pure.bend as Rcp
import ./oneshot.bend as Os

# the compaction threshold for real conversations, in approximate
# tokens, with the ADR 0011 selection budget (20000) for preserved user
# messages. The same precedence as everything else: BEND_THRESHOLD env
# > config.toml threshold > 80 % of the model's context window (from
# bise's catalog, BISE-150: runtime/provider-pure.bend `threshold`)
def thr_of_cfg(+envv: String, +cfgt: String, +window: U32) -> Nat:
  U32.to_nat(Pvp.threshold(envv, cfgt, window))

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), loaded from prompt-*.txt
def prompt_read(+path: String) -> IO(String):
  P.read_or(path, "")

def prompt_join(+tu: String, +ct: String) -> String:
  String.append(
    "You are a precise assistant running on the Bend Unified Harness. Discover additional tools with `search_tool_functions`, and call them using `run_typescript`.",
    String.append("\n\n", String.append(tu, String.append("\n\n", ct))))

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), then the skills sections
# (rules before the <available-skills> catalog, the vibe_sdk order);
# both only when the skills index is non-empty
def live_prompt.go(+base: String, +content: String) -> String:
  Skp.prompt_with(base, content)

# BEND_EXTRA_PROMPT: a file whose content ends the system prompt (a
# switchboard agent's role); unset or missing: nothing is added
def extra_prompt.read(empty: Bool, +path: String) -> IO(String):
  match empty:
    case True{}:
      IO.pure(String, "")
    case False{}:
      prompt_read(path)

def extra_prompt() -> IO(String):
  do IO<String>:
    er : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_EXTRA_PROMPT")
    +path : String <- IO.pure(String, P.env_or(er, ""))
    extra_prompt.read(String.is_empty(path), path)

# BEND_TOOLS_NOTE: whether rg and git are on this machine (BISE-166,
# written by bend-harness / the hub), after the current-time section;
# unset or blank: nothing is added
def live_prompt() -> IO(String):
  do IO<String>:
    +tu : String <- prompt_read("prompt-tool-use.txt")
    +ct : String <- prompt_read("prompt-current-time.txt")
    tn : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_TOOLS_NOTE")
    +note : String <- IO.pure(String, P.env_or(tn, ""))
    +content : String <- Sk.prompt_content()
    +extra : String <- extra_prompt()
    IO.pure(String, Rcp.prompt_extra(live_prompt.go(prompt_join(tu, Rcp.prompt_extra(ct, note)), content), extra))

def live_cfg(+threshold: Nat) -> IO(T.Cfg):
  do IO<T.Cfg>:
    +p : String <- live_prompt()
    +cat : List<&2, T.ToolDef> <- X.catalog()
    IO.pure(T.Cfg, T.Cfg{threshold, 20000n, 3n, p, cat})

def live_main() -> IO(Unit):
  do IO<Unit>:
    # the config template lands on disk on first run, before anything
    # reads it
    dbg : Unit <- Dbg.start()
    ut : Unit <- St.write_cfg_template()
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_THRESHOLD")
    +cfgt : String <- St.read_cfg()
    +window : U32 <- Pv.resolved_window()
    +threshold : Nat <- IO.pure(Nat, thr_of_cfg(P.env_or(env, ""), cfgt, window))
    +p : U32 <- Rc.repl_port(7702)
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", p))
    # the single source of truth for every client: printed BEFORE the
    # banner the parent waits for
    +model : String <- Pv.resolved_model()
    +info : String <- R.harness_info(model, Nat.show(threshold))
    ui : Unit <- IO.print(info)
    IO.print(String.append("bend-harness LIVE REPL on 127.0.0.1:",
      String.append(U32.show(p),
        String.append(" (real model, FS tools, bash) — compaction threshold: ",
          String.append(Nat.show(threshold), " tokens (80% of the model's context; BEND_THRESHOLD or config.toml threshold to change)")))))
    # the plugins bridge, then the skills scan (it reads the plugin
    # skills), then the prompt (it lists the skills)
    u5 : Unit <- Pl.start()
    u4 : Unit <- Sk.skills_scan()
    +cfg : T.Cfg <- live_cfg(threshold)
    +ld : P.Load <- P.load(T.new_session(cfg))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    u : Unit <- Mcp.bootstrap(key)
    u3 : Unit <- Sh.bash_cleanup(U32.show(p))
    f : Rc.Flow <- Rc.serve(True{}, l, Rc.ld_greet(ld), Rc.ld_sess(ld))
    # Stop (a /reload) lands here: main returns, the process exits 0,
    # and the parent respawns the latest code on the same session.
    # The marker line lets the parent tell a reload from a crash.
    u2 : Unit <- Rc.flow_note(f)
    IO.pure(Unit, Unit{})

# BISE_ONESHOT=<request file> (BISE-126): one provider call, its answer on
# stdout, exit (runtime/oneshot.bend); else the live REPL
@unsafe
def main.pick(empty: Bool, +path: String) -> IO(Unit):
  match empty:
    case True{}:
      live_main()
    case False{}:
      Os.run(path)

@unsafe
def main() -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ONESHOT")
    +path : String <- IO.pure(String, P.env_or(r, ""))
    main.pick(String.is_empty(path), path)
