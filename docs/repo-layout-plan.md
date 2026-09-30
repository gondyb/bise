# Repo layout plan 2: no more projects/switchboard/

Status: APPROVED by the user. Proven twice: `layout-2` (branch `sb/layout-2`,
on b7c87c2, never merged) and `layout-3` (the script re-run on a57f628,
the main of 2026-10). Landed as one commit made by the script on the main
of the landing day (never a tree built before). The model is the last
reorg ([root-layout-plan.md](root-layout-plan.md), step 5 "Later" named this
one).

Why: `projects/` holds one folder, and bise is the whole repo now. Its four
parts go to the root, one folder per kind of thing.

## The root after

```
.agents/ .github/ .gitignore
LICENSE NOTICE README.md THIRD_PARTY_NOTICES run.sh
bend/        the Bend sources (unchanged)
rust/        the app (unchanged)
prompts/     the prompts (unchanged)
scripts/     bins.sh versions.sh sb-dev.sh ... (unchanged)
docs/        design docs, RFCs, research/, spec/session-format.ts,
             IMPLEMENTATION.md, README.md (the docs index),
             brand/ (bise-book.md, bise-issues.md, qa/, readme/ images, mockups)
site/        bise.dev: the static site, vercel.json, install.sh
tests/       gate.sh run_all.sh e2e.py fake_provider.py tui_*_tmux.py fixtures/
packaging/   build-dist.sh install.sh publish-release.sh test-install.sh ...
```

## What moves where

| before | after | why |
|---|---|---|
| projects/switchboard/docs/ | docs/ | the design docs |
| projects/switchboard/docs/brand/ | docs/brand/ | the book and its issue list stay together (bise-issues.md links bise-book.md; the book links ../pitch.md, ../images.md) |
| projects/switchboard/docs/brand/site/ | site/ | it is a product (bise.dev), deployed by itself: not a doc |
| projects/switchboard/spec/session-format.ts | docs/spec/session-format.ts | one file, a spec: a doc |
| projects/switchboard/IMPLEMENTATION.md | docs/IMPLEMENTATION.md | a doc |
| projects/switchboard/README.md | docs/README.md | the index of the docs |
| projects/switchboard/tests/ | tests/ | |
| projects/switchboard/packaging/ | packaging/ | name kept: ~40 docs and the issue list already say `packaging/...` |

File names do not change: `rg bise-issues.md`, `loop-speed.md` etc. still
find them. The rule for any old path: drop `projects/switchboard/`; except
`.../docs/brand/site` -> `site`, `.../spec/` -> `docs/spec/`,
`.../IMPLEMENTATION.md` and `.../README.md` -> `docs/...`.

## The edits (all made by [repo-layout-migrate.sh](repo-layout-migrate.sh))

Mechanical on purpose: the diff touches ~500 files (renames) and 381 path
mentions, and bise-issues.md changes every hour. At landing, the script runs
on the main of that day instead of a rebase.

1. `git mv` of the eight entries above; `projects/` is gone.
2. Every path in tracked text files (perl, see the script): the Rust
   fixture paths of rust/session/tests (`../../tests/fixtures/...`),
   release.rs (`packaging/publish-release.sh`), release.yml (3 lines),
   publish-release.sh (`site/install.sh`), test-release-gh.sh (its fake
   repo), third-party-notices.py and THIRD_PARTY_NOTICES (same text, the
   `--check` stays green), run.sh, the root README (images, table), the
   doc comments of rust/ and bend/, the docs and the issue list.
   Not rewritten: tests/fixtures/ (recorded session logs: what an agent
   typed that day), docs/root-layout-plan.md (history), and the made-up
   tree of the at-files test in rust/tui/src/files.rs (its "sb/me" query
   needs `projects/switchboard/` in it).
3. The scripts that find the root by depth: tests/*.py
   (`os.path.join(HERE, "..", "..", "..")` -> `".."`), gate.sh and
   run_all.sh (`$(dirname $0)/../../..` -> `/..`).
4. Relative links whose ends moved apart: docs/brand/tui-*.html and
   landing/index.html redirect to `../../site/...`; bise-book.md links
   `../../site/...`; docs/README.md and IMPLEMENTATION.md link `rfc-...`,
   `../packaging/`, `../tests/`; rfc-0001 links `IMPLEMENTATION.md`.
5. The root README table: one row per new folder.

## What does not need a change

- `sb restart` and `/version`: scripts/versions.sh, bins.sh and the hub's
  `dev_workspace` name nothing under projects/. One landing is enough, no
  compat step. A version dir is self-contained: rolling back to an old
  commit still builds (bins.sh reads bend/, prompts/, not tests/).
- install.sh (both copies): no projects/ path in it, so bise.dev/install
  needs no redeploy for this change.
- The gate seed (~/.bise/cache/gate-seed/<key>): keyed on Cargo.lock,
  the Cargo.tomls, config.toml and rustc, not on paths. The session tests
  recompile (their fixture path changed), nothing else.
- CI: release.yml's cache keys hash bend/ files only.
- The skills in .agents/: no projects/ path.

## What needs a change outside git

1. **The live hub, `/release-bise`**: the running binary looks for
   `projects/switchboard/packaging/publish-release.sh`. After the landing,
   `sb restart` (the new binary looks in `packaging/`). Until then
   /release-bise says the script is missing; nothing else in the hub reads
   these folders.
2. **Untracked files in the shared checkout**: git moves tracked files and
   leaves the others in `projects/`. Known ones: the Vercel link
   `projects/switchboard/docs/brand/site/.vercel/` -> `site/.vercel/`, the
   site's annotation notes `.../site/content/notes/` -> `site/content/notes/`,
   `projects/switchboard/tests/__pycache__` (drop). Then `rm -rf projects`
   once `find projects -type f` is empty. Uncommitted edits there (today:
   `site/book/onboarding.html`, `brand/readme/preview.html`) must be
   committed or moved by their owner first.
3. **designer**: the dev server runs from the old folder; restart it from
   the new one: `cd site && python3 content/serve.py 4747` (serve.py finds
   its root from its own path). The deploy archives `site` instead of
   `projects/switchboard/docs/brand/site` (`git archive HEAD site | tar -x
   -C <tmp>`, copy `site/.vercel` in, `vercel deploy --prod`).
4. **Running agents**: an agent in a worktree made before the landing has
   the old layout in its tree: its `gate.sh` works there. From the shared
   checkout, `projects/switchboard/tests/gate.sh` is gone: use
   `tests/gate.sh` (`tests/gate.sh done <name>` removes an old task folder
   the same way). A rebase of an old branch follows the renames; a
   conflict is possible only where both sides edited the same lines.
   Land when few tasks are open, and tell the open ones (one message).
5. **main's briefs and notes**: `tests/gate.sh` (new/quick/full/wait/done),
   `docs/loop-speed.md`, `docs/brand/bise-book.md`,
   `docs/brand/bise-issues.md`, `site/` (designer's site), `packaging/`
   (build-dist.sh, test-install.sh, publish-release.sh),
   `docs/IMPLEMENTATION.md`. The rule above covers the rest.

## Landing steps (one landing)

1. No task running (main waits); note the untracked files under projects/
   (`git status --short --ignored projects`).
2. On a fresh worktree of the main of that moment (`gate.sh new`):
   `bash /tmp/m.sh` (this script, as committed), read what it prints, add
   this plan and the script to docs/, then commit through a private
   `GIT_INDEX_FILE` read from the CURRENT HEAD (`git diff --stat HEAD`:
   the migration only) and move main to it.
3. `tests/gate.sh quick` on it (the full gate ran on the proof tree).
4. main pushes; in the shared checkout moves the untracked files (above),
   then `rm -rf projects`.
5. `sb restart`; designer restarts the dev server; main updates its briefs
   and tells the open tasks.

## Changes to the script since layout-2 (layout-3)

- `find projects -mindepth 1 -type d -empty -delete` before the leftover
  check, guarded (`|| true`): git mv leaves empty folders, and the old
  `find` failed under `set -e` once projects/ was gone.
- Bare `spec/session-format.ts` (no projects/ prefix: bend/core/ev.bend,
  rust/session/src/types.rs, tests/session_ev.py, session-format.md,
  bise-issues.md) -> `docs/spec/session-format.ts`.
- site/content/serve.py's docstring: `brand/site/` -> `site/`.
- Everything else new since b7c87c2 is covered by the same rules and
  needed no new line: the .gitignore entries (BISE-258 secrets, the
  BISE-275 private drafts, designer's hero-*/type-*/preview.html/video
  rules: all become `/site/...` and `/docs/...`), the new tests
  (tui_cmd_a, tui_keys, tui_file_links, tui_demo_tips, core_restart,
  bise_demo_e2e...), packaging/test-update-flow.sh and setup.md,
  release.rs (/release-bise), the readme images (docs/brand/readme/feat/).

## Proof in the worktree (layout-2, on b7c87c2)

Branch `sb/layout-2`, one commit on b7c87c2, made by the script above
(`projects/` gone, 518 files: 412 pure renames, the rest renames with path
edits or edits).
- `tests/gate.sh full`: GREEN (158 s): cargo build + tests + clippy,
  run_all.sh (e2e, PROOF, session_ev against docs/spec/session-format.ts,
  every tmux test), minos. A first run failed on two paths written as
  `os.path.join` parts (session_ev.py, capture.py): the script fixes them
  now. worktree_home timed out once under load and passed alone and in the
  second full run.
- `packaging/build-dist.sh HEAD` + `packaging/test-install.sh`: GREEN,
  31/31 checks. Caveat: bend-jsrt came from the build cache of an earlier
  commit (bend-jsrt-17206663e02f, cloned into the temp BISE_HOME under this
  commit's key): the agent's bash tool caps a written file at 50 MB and
  the rusty_v8 static lib is bigger, so no fresh V8 build there. The jsrt
  sources are not touched by this change.
- `python3 packaging/third-party-notices.py --check`: in sync.
- `site/`: `python3 content/serve.py 4799` from site/ serves /, /book/,
  /book/screens.html, /install.sh, /og.png (200).
- The script refuses a dirty tree and a root that already has docs/ site/
  tests/ packaging/ (an ignored `tests/__pycache__` would make `git mv`
  nest the folder: seen once).
- Not tested: a live `sb restart` after the landing, CI (release.yml),
  a Vercel deploy from site/.
