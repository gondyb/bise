# AGENTS.md in bise (BISE-232)

What bise reads, compared with Codex and Vibe. Sources read on 2026-09-29:
Codex `openai/codex` a44afa5 (`codex-rs/core/src/agents_md.rs`,
`codex-home/src/instructions/mod.rs`, `core/src/context/user_instructions.rs`,
`config/src/config_toml.rs`); Vibe `mistral-vibe` 2.25.8 (latest on uv:
`vibe/core/config/harness_files/_harness_manager.py`,
`vibe/core/system_prompt.py`, `vibe/core/prompts/agents_doc.md`,
`vibe/app_server/_agents_md_hooks.py`).

## Codex vs Vibe

| | Codex | Vibe |
|---|---|---|
| global file | `$CODEX_HOME/AGENTS.override.md`, else `AGENTS.md` (first with text, trimmed) | `$VIBE_HOME/AGENTS.md` (`~/.vibe`), no override |
| project root | nearest ancestor with a `project_root_markers` entry (default `.git`); none: the cwd alone; `[]`: no walk | the trust root (the trusted folder, may be above cwd), or `--add-dir` roots |
| files read | each folder from root down to cwd: the first of `AGENTS.override.md`, `AGENTS.md`, `project_doc_fallback_filenames` | each folder from trust root down to cwd: `AGENTS.md` only |
| merge | concatenated, root first; global first, then `--- project-doc ---` | root first, each under `Contents of <dir>/AGENTS.md:`; user section, then "Project instructions" |
| size limit | `project_doc_max_bytes` (32 KiB) over all project files: the crossing file is cut, the rest dropped; global not counted; 0 disables | none |
| where in the prompt | a user-role message `# AGENTS.md instructions for <cwd>` + `<INSTRUCTIONS>…</INSTRUCTIONS>`; the scope and precedence rules are in the base prompt ("AGENTS.md spec") | a section of the system prompt (`agents_doc.md`: "project over user, closer to cwd wins") |
| deeper files | not loaded; the base prompt tells the model to look for them | loaded lazily: reading a file below cwd appends the AGENTS.md files between it and cwd to the tool result, once per folder |
| trust | untrusted project: no project file | project files only in a trusted folder |
| refresh | per turn (the model is told when they change) | at session start |
| subagents | inherit the snapshot | same Core rules (the hook runs in subagents too) |

## What bise does (rust/switchboard/src/agents_md.rs)

Codex's discovery, overrides, limits and config keys, as is:

- global: `$BISE_HOME/AGENTS.override.md`, else `$BISE_HOME/AGENTS.md` (`~/.bise`);
- project: root = nearest folder with a `.git` (a folder or a worktree's `.git` file); no root: the working folder alone; each folder from the root down to the working folder: the first of `AGENTS.override.md`, `AGENTS.md`, then `project_doc_fallback_filenames`;
- budget: 32 KiB of project files (`project_doc_max_bytes`), cut on a character boundary;
- config.toml top-level keys, Codex's names: `project_doc_max_bytes`, `project_doc_fallback_filenames` (e.g. `["CLAUDE.md"]`: bise reads no CLAUDE.md unless asked), `project_root_markers`.

Rendering: Codex's block (`# AGENTS.md instructions for <cwd>`, `<INSTRUCTIONS>`,
global first, `--- project-doc ---`), with Vibe's `Contents of <path>:` line
over each file and, first, Codex's scope and precedence rules in a few lines
(bise's base prompt has no "AGENTS.md spec"): a file applies to its tree,
deeper wins, project over user, direct instructions over AGENTS.md, look for
AGENTS.md files when working in another folder.

Where: in the **system prompt**, after the skills and before the agent's role
(Vibe's place; Codex uses a user message). The system prompt survives
compaction; a user message would need re-injection.

Who: every agent the hub starts, main and each task, from its own working
folder (`BEND_WORKDIR`): the workspace, or the task's worktree (its committed
files, not the workspace's). The hub writes the block to
`<agent state>/agents-md.md` at each REPL start and `/reload`, and the REPL
reads it through `BEND_AGENTS_MD` (bend/runtime/repl-live.bend). So an edit
reaches an agent at its next reload, not mid-session.

Not taken (yet): Vibe's lazy injection of deeper AGENTS.md files on a file
read (the prompt tells the model to look instead, as Codex does); a trust
gate (bise has no trusted-folders list); per-turn refresh; the headless
`bise --headless` session (it runs in the app root, not a project).

Note: a task worktree starts at the repo root, even when the hub runs in a
subfolder, so a subfolder's AGENTS.md reaches main (and shared tasks) there,
not a worktree task.

Tests: `agents_md::tests` (chain, worktree `.git` file, no marker, override
and fallbacks, budget, global, rendering) and
`projects/switchboard/tests/agents_md_e2e.py` (a hub in `repo/sub`: main
and a shared task get global + root + sub; a worktree task gets its
worktree's committed root file).
