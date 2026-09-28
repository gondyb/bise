# BR-008 — New files "vanish" from the workspace: apply_patch wrote them in the version dir

- Severity: high (work seems lost; agents suspect each other of `git clean` / `git stash`)
- Component: runtime/patch-tool.bend (apply_patch), with rust/switchboard/src/daemon.rs (REPL cwd)

## What the user saw

New untracked files seemed to vanish from the shared tree:

- `rust/tui/src/term.rs` (task term-panel), twice, 15:50–15:52;
- `rust/tui/src/help.rs` (task help-format), ~15:58.

The pattern in the transcripts: `apply_patch` answers `Done! - added
rust/tui/src/term.rs`, then a bash command in the workspace gets `No such
file or directory` (cargo: `file not found for module term`). A second
`Add File` on the same path then FAILS with `already exists`, while bash
still cannot see the file. The two tools looked at two different
directories.

## Root cause

Nothing deleted the files. They were never written in the workspace.

1. The hub starts each REPL with its working directory set to the app root,
   which is now a version dir: `~/.local/state/switchboard/versions/<sha>/`
   (daemon.rs: `cmd.current_dir(&self.opts.app_root)`). The runtime needs
   it (tool descriptions, prompts, `rust/jsrt`). The agent's own directory
   comes in `BEND_WORKDIR`.
2. The bash tool cds into `BEND_WORKDIR` before each command
   (`with_workdir`, bash-pure.bend). `run_typescript`'s `bash` too.
3. apply_patch did not: it passed the patch path unchanged to
   `read_file`/`write_file` and to `mkdir -p` / `rm -f`. A relative path
   resolved against the version dir.

Evidence: both "lost" files were found in the version dir of the REPL that
wrote them (term-panel's `repl.json`: `versions/a071daf/repl-live`):

    versions/a071daf/rust/tui/src/term.rs   15:51 (Add #60 at 15:50:22, Update #84 at 15:51:08)
    versions/a071daf/rust/tui/src/help.rs   15:58

`lsof -d cwd` on the running REPLs shows `versions/<sha>` as their cwd.
term-panel's third try used an absolute path and the file stayed.

A relative `Update File` on an existing workspace file failed with `does
not exist`. So agents often switched to absolute paths, which hid the bug
until two new files were added with relative paths.

Ruled out: git stash/clean/checkout (stash list empty, reflog shows only
commits and one mixed reset), the bash temp-script cleanup, EDR
quarantine, the version switch.

## Fix

`exec_patch.live` reads `BEND_WORKDIR` and resolves each relative path
(Add, Update, Delete) against it before validating or writing
(`resolve_path`, patch-tool.bend). An absolute path, or no `BEND_WORKDIR`
(a REPL outside Switchboard), stays unchanged. The summary line then shows
the absolute path, so the model sees where the file went.

Test (throwaway dirs, cwd `/tmp/van-app`, `BEND_WORKDIR=/tmp/van-ws`):
relative Add `sub/new.txt`, relative Update of it, absolute Add, relative
Delete. Result: only `/tmp/van-ws/sub/new.txt` exists (content `world`),
`/tmp/van-app` stays empty. `bend PROOF.bend`: ALL PROOFS CHECK.

Other tools: bash and `run_typescript`'s bash already use `BEND_WORKDIR`.
The other file reads/writes in the runtime use paths built by the runtime
(temp files, skills, MCP index), not paths from the model.

## Old REPLs

A running REPL keeps its binary until the next version switch. It then
moves to the new binary at its next idle, with the same session. Until
then, use ABSOLUTE paths in apply_patch.

## Recovery

Files written in version dirs (everything besides the build outputs): only
the two above. Both are older than the committed versions (term.rs was
re-created from them and committed in 94d2e54; help.rs was committed in
e0b75c4). The lines found only in the version-dir copies are code that
was removed later on purpose (for example `term::HELP`, which help.rs
replaced). Nothing to recover.
