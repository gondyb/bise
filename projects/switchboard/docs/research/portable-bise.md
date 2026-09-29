# Research — arc 1: a portable `bise`

Status: research only, no product code changed. Read at HEAD `db19ec1`
(BISE-114 landed: the Bend binaries left git; BISE-113 "remove the solo
TUI" and BISE-118 "one run_typescript engine" were still in flight).
Builds on `docs/packaging.md` (the 2026-09 prototype); this note does not
repeat it, it says what changed, what is still missing, and the plan.

The goal, in the user's words: `bise` is one command you can start in any
folder, for any project, on every supported macOS (not Windows yet). The
CLIs the agents need and the session files are portable. The state of
bise lives in `~/.bise/`, like `~/.codex/` and `~/.claude/`.

Issue ids: this plan uses **BISE-120..BISE-133**. research-providers uses
BISE-140..153 and agrees on the same `~/.bise/` layout (§3.2).

---

## 1. What exists today

### 1.1 How bise starts

- **`./run.sh` is the only real entry point, and it is a dev script.** It
  `cd`s into the repo (`run.sh:34`), puts `~/.cargo/bin` and `~/.bend/bin`
  on the PATH, runs `cargo build` every time (`run.sh:47-51`), builds
  `bend-jsrt` when missing or stale (`run.sh:56-63`), then `./bins.sh`
  puts `repl-live`, `repl-scripted`, `sb-core` in place from a build
  cache or compiles them with `bend` (`run.sh:68`). It needs the repo,
  cargo and bend. The user's folder survives only as `SB_LAUNCH_DIR`
  (`run.sh:33`).
- **The binary finds its files through an "app root".** `app_root()`
  looks for `repl-live` in the **cwd first**, then next to the exe, then 3
  parents up (`rust/harness/src/main.rs:191-203`). The single-session path
  `chdir`s to the exe dir unless the cwd holds the REPL
  (`main.rs:418-424`). So running an installed `bise` inside the dev repo
  picks the repo's REPL (packaging.md C1, still open).
- **Runtime files read by relative path** from the app root (the REPL's
  cwd is the app root, `rust/switchboard/src/daemon.rs:556`):
  `tool-desc-*.txt` (`runtime/tools.bend:15-32`), `prompt-*.txt`
  (`runtime/repl-live.bend:71-72`). Fine for a bundle: they ship in the
  version dir.
- **The V8 engine** is found by the harness (`export_jsrt_bin`,
  `main.rs:219-233`: `<root>/bend-jsrt`, else the dev debug/release
  build) and handed over as `BEND_JSRT_BIN`; `runtime/main.bend:208`
  falls back to the dev path `rust/jsrt/target/debug/bend-jsrt`.
  packaging.md C6 is done by BISE-114.
- **`sb-core`**: the hub uses `<root>/sb-core` (`main.rs:268-270`), else
  `CARGO_MANIFEST_DIR/../../sb-core` — a build-machine path baked into
  the binary (`rust/switchboard/src/core.rs:382-385`, C4, still open).
- **`/reload`** recompiles the REPL with `bend` found on PATH or in
  `~/.bend/bin` (`main.rs:813-840`); without the sources it keeps the
  binary. Harmless when installed.
- **Dev instance**: `sb-dev.sh` builds into `$SB_DEV_ROOT`
  (`/tmp/sb-dev`, `sb-dev.sh:25`) with its own hub.
- **Packaging prototype** (`projects/switchboard/packaging/`):
  `build-dist.sh` (tarball from a commit, via `versions.sh`), `install.sh`
  (prefix `~/.local/share/bend-harness`, launcher in `~/.local/bin`,
  `install.sh:28-29`), `test-install.sh` (clean fake HOME). It passed
  23/23 on arm64 (packaging.md §5). `release.sh` is a third build path
  and still strips (C12).

### 1.2 How the agents get `sb` and the other CLIs

- The hub writes a shim `<state>/bin/sb` = `exec '<abs exe>' sb "$@"`
  (`daemon.rs:936-943`, rewritten at every hub start, `daemon.rs:956`).
- Each agent REPL gets `PATH=<state>/bin:<the hub's PATH>`
  (`daemon.rs:580-586`). The hub's PATH is the PATH of the TUI that first
  started it (`client.rs:14-29`, detached, lives on). **Nothing adds the
  usual tool dirs**: a hub started from a thin environment (an IDE task, a
  launchd job, `env -i`) gives agents no `/opt/homebrew/bin`.
- External programs the product itself runs:

| Program | Where | On a fresh Mac | When missing |
|---|---|---|---|
| `/bin/sh`, `kill`, `ps`, `sleep`, `cat`, `tail`, `find`, `mkfifo`, `sed`, `nohup` | runtime tools (`runtime/bash-pure.bend`, `proc.bend`, `skills.bend:55-69`, `plugins.bend:63`), `daemon/repl.rs:25-38`, `switch.rs:105-142` | in the base system | — |
| `git` | worktrees (`worktree.rs:100`), `/version` (`daemon/versions.rs:45,117,190`) | **`/usr/bin/git` is a stub**: it opens the "install Command Line Tools" dialog and fails | `-w` tasks: "not a git repository" (`hub/core.bend:1001`) or **"git introuvable"** (French, `worktree.rs:106`); `/version` lists nothing |
| `rsync` | backup before a switch (`switch.rs:297-305`) | in the base system (macOS ships openrsync since 15) | backup silently skipped |
| `pbcopy` | clipboard, then OSC 52 fallback (`tui/src/clipboard.rs:52-68`) | base system | falls back |
| `date` | feed times (`tui/src/feed.rs:1071`, `sb/feed.rs:186`) | base system | — |
| `bend` | `/reload` recompile only | absent | keeps the binary (fine) |
| `cargo`, `versions.sh`, the repo | `/version <commit>`, `/restart latest` (`daemon/versions.rs:184-230`) | absent | "versions.sh not found" |

- What the **agents** are told to use: `rg` ("Prefer `rg` over `grep`",
  `tool-desc-bash.txt:1`) and `sb`. `rg` is **not** on a fresh Mac: the
  model gets "command not found" and retries with grep (a wasted turn,
  every session). `python3` is also a Command Line Tools stub (dialog);
  the product does not need it, agents often try it. `tmux` and
  `python3` are used by tests only (`bend_client.py`, `tests/`).

### 1.3 Where state lives today (5 roots, 3 naming schemes)

| What | Path today | Code |
|---|---|---|
| keys | `~/.bend-harness/.env`, then `~/.vibe/.env` | `main.rs:166-168`, `tui/src/voice.rs:340`, `tui/src/onboarding.rs:152-162` |
| config | `~/.bend-harness/config.toml` (`BEND_CONFIG`), `/tmp/bend-harness-config.toml` without HOME | `runtime/settings.bend:19-21`, `onboarding.rs:184-185` |
| single sessions | `~/.bend-harness/sessions/` (`BEND_SESSIONS_DIR`), legacy `session-repl-*.txt` | `main.rs:463-470` |
| TUI prefs | `~/.bend-harness/tui.json` (voice, theme) | `voice.rs:280`, `theme_detect.rs:89` |
| plugins | `~/.bend-harness/plugins.json`, `plugin-data/`, `run/<port>/` | `plugins/src/state.rs:15`, `resolve.rs:147`, `runtime/plugins.bend:23`, `tui/src/plugins.rs:14` |
| images, crashes, indexes | `~/.bend-harness/{images,crashes,mcp-index.txt,skills-index.txt}` | `images/src/lib.rs:209`, `tui/src/crash.rs:88`, `main.rs:276,532`, `tui/src/skills.rs:58` |
| hubs | `~/.local/state/switchboard/<name>-<hash>/` (`SB_STATE_DIR`, `XDG_STATE_HOME`): `hub.sock`, `journal.jsonl`, `hub.log`, `agents/<n>/session.txt`, `worktrees/`, `bin/sb` | `switchboard/src/paths.rs:47-89`, `daemon.rs:508` |
| onboarding, hints, tip | `~/.local/state/switchboard/{onboarded,hints.json,tip}` | `onboarding.rs:54-61`, `hints.rs:81`, `keybar.rs:197` |
| dev versions + build cache | `~/.local/state/switchboard/{versions,build}` (1.8 GB + 2.6 GB here) | `versions.sh:28-31`, `daemon/versions.rs:259-280`, `build-dist.sh:32-33` |
| side channels | `/tmp/bend-{prog,res,steer,interrupt,sh,skills-scan,plugins-start}-<port>.*` (world-readable dir) | `runtime/main.bend:225-227`, `main-pure.bend:658`, `provider-pure.bend:65`, `bash-pure.bend:245`, `skills.bend:72`, `plugins.bend:68` |
| switch backups | `/tmp/sb-backup-<ws>-<ms>` | `switch.rs:297` |
| per-project config | `<workspace>/.switchboard/config.toml` | `paths.rs:87` |

"Home" is resolved in **~15 places**, each with its own fallback (`/tmp`,
cwd, `""`). There is no single "where is bise's home" function, in Rust or
in Bend.

### 1.4 macOS facts measured on this Mac (macOS 26.6, arm64)

| Binary | Links | `minos` (oldest macOS it runs on) | Signature |
|---|---|---|---|
| `bend-harness` (release, 8 MB) | libSystem, libiconv, CoreFoundation, CoreAudio, AudioUnit | 11.0 | ad-hoc |
| `bend-jsrt` (release, 65 MB) | libSystem, libc++, libiconv | 11.0 | ad-hoc |
| `repl-live` (2.8 MB), `sb-core` (2.1 MB) | libSystem only | **26.0** | ad-hoc |

**The Bend binaries only run on macOS 26 (Tahoe)** today: `bend -o` calls
the C compiler with the host SDK's default target. Checked:
`MACOSX_DEPLOYMENT_TARGET=12.0 bend hub/main.bend -o x` gives `minos
12.0`. So it is a one-line fix in the build scripts, but today a
tarball built on this Mac fails on macOS 13-15 ("built for macOS 26.0
which is newer than running OS"). Only system dylibs are linked: nothing
to bundle. Everything is arm64-only; no x86_64 build was ever made.

### 1.5 How Codex CLI and Claude Code do it (checked 2026-10)

| | Claude Code | Codex CLI |
|---|---|---|
| Install | `curl -fsSL https://claude.ai/install.sh \| bash`; Homebrew cask (no auto-update); npm (legacy) | `npm i -g @openai/codex`, `brew install --cask codex`, release tarballs |
| Code | `~/.local/share/claude/versions/<v>`, `~/.local/bin/claude` = symlink | npm/brew prefix |
| State | `~/.claude/` (settings, `projects/<path>/<session>.jsonl`, plugins) + `~/.claude.json`; `CLAUDE_CONFIG_DIR` | `~/.codex/` (config.toml, auth.json, sessions/, log/, history.jsonl, skills); `CODEX_HOME` |
| Temp | `/tmp/claude-<uid>/…` (`CLAUDE_CODE_TMPDIR`) | — |
| Update | background auto-update, `claude update`, `claude doctor` | the package manager |
| macOS | signed "Anthropic PBC", notarized | signed per-arch binaries |

Both: one dot-folder for state, one env var to move it, code kept apart
from state. Claude Code's layout is the one to copy (it is also what our
prototype already does).

---

## 2. The gaps

1. **No `bise` command.** The product name is `bend-harness`; the only
   entry is `./run.sh` in the repo. The installer is a prototype nobody
   runs.
2. **App root can be hijacked by the cwd** (C1) and `sb-core` has a
   build-machine fallback path (C4).
3. **State is spread over 5 roots with 3 names**; ~15 home lookups; no
   `BISE_HOME`; no migration. `/tmp` holds per-port files other users can
   read (C8).
4. **Old versions and the migration fight.** `/version back` or a
   rollback starts an *older* binary (`switch.rs:180-189` passes only
   `--workspace`). That binary computes the old state path: after a move
   to `~/.bise/`, it would start an empty hub. Old versions do honour
   `SB_STATE_DIR`, `BEND_CONFIG`, `BEND_SESSIONS_DIR`, `BEND_IMAGE_DIR`,
   `BEND_MCP_INDEX`, `BEND_SKILLS_INDEX` — so the fix is to always export
   them (§3.3).
5. **Worktrees hold absolute paths** (`.git` file in each worktree +
   `<repo>/.git/worktrees/<n>/gitdir`): moving a hub's state dir breaks
   them unless `git worktree repair` runs. A running hub's socket and its
   agents' `SB_SOCKET` also point at the old path.
6. **Minimum macOS = 26 in practice** (Bend binaries), no x86_64, no
   Developer ID signature, no notarization. An EDR already deleted
   unsigned stripped binaries once (packaging.md §6).
7. **Agents' tools are luck.** `rg` is advertised but not shipped; `git`
   may be the CLT stub that pops a dialog; the agents' PATH is whatever
   the first TUI had; errors are partly French (C11).
8. **`/version` and `/restart latest` need the repo** (`versions.sh`,
   `git`, cargo, bend). An installed bise has none: it needs releases
   and a self-update instead (C9).
9. **Three build paths** (`run.sh`/`bins.sh`, `versions.sh`,
   `release.sh` + `build-dist.sh`), and `release.sh` strips (C12).

---

## 3. Target design (simple)

### 3.1 Two folders, one command

```
~/.local/bin/bise -> ~/.local/share/bise/current/bise   (the command)
~/.local/share/bise/                    CODE (replaceable, like Claude Code)
  versions/<version>/   bise, repl-live, repl-scripted, sb-core, bend-jsrt,
                        rg, tool-desc-*.txt, prompt-*.txt, VERSION
  current -> versions/<version>
~/.bise/                                STATE (yours; BISE_HOME moves it)
  config.toml           model, providers (research-providers' format)
  auth.json             keys, 0600 (research-providers; .env files still read)
  prefs.json            voice, theme, hints, tip, onboarded (was 4 files)
  sessions/             single-agent sessions (if any remain after BISE-113)
  hubs/<name>-<hash>/   one per workspace: hub.sock, hub.pid, hub.log,
                        journal.jsonl, agents/, worktrees/, bin/sb, backups/
  plugins.json, plugin-data/, images/, crashes/
  cache/                models.json (providers), skills/mcp indexes
  run/<pid-or-port>/    side channels (0700; was /tmp/bend-*)
  dev/                  versions/ + build/ of versions.sh (dev only)
  migrated.json         what was imported from the old places, when
```

- **One function knows the home**: `bise_home()` = `$BISE_HOME`, else
  `~/.bise`. Rust: a tiny crate `rust/home` used by every crate. Bend:
  never computes it; the harness exports the exact paths (`BISE_HOME`,
  `SB_STATE_DIR`, `BEND_CONFIG`, `BEND_SESSIONS_DIR`, `BEND_IMAGE_DIR`,
  `BEND_MCP_INDEX`, `BEND_SKILLS_INDEX`, `BEND_RUN_DIR`) before it starts
  anything. The runtime already reads most of them.
- **No XDG split** on macOS (Codex and Claude Code do the same).
  `XDG_STATE_HOME` stops being read; `SB_STATE_DIR` stays as the
  per-hub override (tests, sb-dev).
- **Unix socket path limit** (104 bytes on macOS):
  `~/.bise/hubs/<32 chars>-<8 hex>/hub.sock` is ~90 bytes with a 20-char
  user name. `bise doctor` warns when a `BISE_HOME` makes it too long.
- **Workspace = the folder you launch in** (as today, `sb_workspace`,
  `main.rs:237-251`). Not a git repo: bise works, only `-w` worktrees
  are refused with a clear message.

### 3.2 The app root

The binary finds its own files, never the cwd's: `BISE_APP_ROOT`, else
the exe's dir when it holds `VERSION`, else (dev only) today's lookup.
`sb-core`, `bend-jsrt`, `rg`, the text files: all next to the exe. The
`CARGO_MANIFEST_DIR` fallback goes away in release builds.

### 3.3 Migration (once, and old state stays readable)

At start, if `~/.bise/migrated.json` does not exist:

1. **User files are copied**, never moved: `~/.bend-harness/*` →
   `~/.bise/…`, the 4 prefs files → `prefs.json`. Old dirs stay, so an
   old version (dev `/version`, a rollback) still works.
2. **Hubs**: for each `~/.local/state/switchboard/<id>/`:
   - hub **not running** (no live socket): `mv` to `~/.bise/hubs/<id>`
     (same disk: instant), then `git worktree repair` for its worktrees,
     and leave a `MOVED` note with the new path in the old dir;
   - hub **running**: leave it; `Paths` uses the old dir for that
     workspace until the hub stops (the next start moves it). The TUI
     says "this hub still runs from the old place; it moves at its next
     restart". `move-live.sh` already does this dance for the dev hub.
3. `versions/` stays where it is (running hubs and `switch.json` point
   at its absolute paths); `build/` moves to `~/.bise/dev/build` (a cache).
4. `migrated.json` records what moved. Delete the old dirs: a later,
   manual `bise doctor --clean-old` (one release later).

Why this is safe with old versions: bise **exports `SB_STATE_DIR` and the
`BEND_*` paths** before starting the hub, the switcher or a re-exec'd TUI.
Every past version honours them (`paths.rs:51`, `settings.bend`,
`main.rs:463`), so a rollback finds the moved hub.

### 3.4 The agents' tools

- The hub builds the agents' PATH on purpose:
  `<hub>/bin` (sb) : `<app root>/tools` (bundled `rg`) : the user's PATH :
  `/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`
  (deduplicated). The bundled `rg` comes right after `sb`, so every agent
  gets a known `rg` and the prompt's "prefer rg" is always true. The
  standard dirs at the end make a hub started from a thin environment
  still find Homebrew tools.
- **`git`**: required for worktrees and dev versions, not for bise
  itself. Check it as `git --version` exit status (not `command -v`: the
  CLT stub exists and pops a dialog). Onboarding and `bise doctor` say
  "run `xcode-select --install`". Messages in English.
- `python3`: not needed, not shipped; `bise doctor` just reports it.
- `rsync` backup → a Rust copy into `hubs/<id>/backups/` (keep 3).

### 3.5 Install, update, versions

- `curl -fsSL <url>/install.sh | sh` (the prototype, renamed): detects
  arm64 vs x86_64 **and Rosetta** (`sysctl -n sysctl.proc_translated`: an
  x86 shell on an M-series Mac gets the arm64 build), checks sha256,
  unpacks into `versions/<v>`, flips `current`, links `~/.local/bin/bise`,
  adds one PATH line. Then a Homebrew cask (same tarballs).
- **Two tarballs, not a universal binary**: V8 is 65 MB per arch; a
  universal build doubles every download for nothing.
- **Minimum macOS 13** (Ventura; see decision 2): `MACOSX_DEPLOYMENT_TARGET`
  for cargo and bend in every build script, and a gate check on `minos`.
- **Signed and notarized**: Developer ID, hardened runtime, timestamp;
  entitlements `com.apple.security.cs.allow-jit` for `bend-jsrt` (V8),
  `com.apple.security.device.audio-input` for `bise` (voice). Submit a zip
  with `notarytool`; bare Mach-O cannot be stapled, Gatekeeper checks
  online (a `.pkg` later can be stapled). Never strip after signing.
- **`bise update`** + a daily background check (never blocks the start;
  `BISE_NO_UPDATE=1`): downloads the next version, verifies sha256 and
  `codesign --verify`, flips `current`. Brew installs: tell the user to
  `brew upgrade` instead.
- **Running hubs are never switched silently**: the hub tells main and
  the user "bise 0.4 is ready"; `/restart latest` switches with the
  existing probation and rollback. In an installed build `/version` lists
  installed + released versions (from `latest.json`); the git/`versions.sh`
  path stays for dev builds (`VERSION` has `repo=`).
- **`bise doctor`**: version, app root, `BISE_HOME`, socket path length,
  signature, `git` real or stub, `rg`, PATH line, running hubs and where
  their state is (old or new place).

---

## 4. Plan

Hours are focused work for one agent, tests included. "Parallel" = can
start now in its own worktree without touching files another task owns.

| # | Issue | Size | Owns | When |
|---|---|---|---|---|
| 1 | **BISE-120** `bise_home()`: new crate `rust/home` (home, hub dir, prefs, run dir, the list of exported env paths); every Rust lookup of §1.3 goes through it; `XDG_STATE_HOME` dropped; tests with a fake HOME | 5 h | `rust/home/` (new), `rust/Cargo.toml`, `switchboard/src/paths.rs`, `daemon/versions.rs:259-280`, `tui/src/{onboarding,hints,keybar,voice,theme_detect,crash,plugins,skills}.rs`, `plugins/src/{state,resolve}.rs`, `images/src/lib.rs`, `harness/src/main.rs` (env loading, sessions, exports) | must wait for BISE-113 (debt-solo edits `main.rs` and the tui) |
| 2 | **BISE-121** Migration once (§3.3): copy user files, move idle hubs + `git worktree repair`, keep running hubs in place, `migrated.json`, export `SB_STATE_DIR`/`BEND_*` so old versions follow; tests: old layout in a fake HOME, running-hub case, rollback to a pre-120 binary finds the hub | 6 h | `rust/home/src/migrate.rs`, `switchboard/src/paths.rs` (old-dir rule), `switch.rs` (env for the switcher) | must wait for BISE-120 |
| 3 | **BISE-122** Side channels out of `/tmp`: `BEND_RUN_DIR` (0700, `~/.bise/run/<port>`), switch backups in `hubs/<id>/backups` with a Rust copy (no rsync) | 3 h | `runtime/{main,main-pure,provider-pure,bash-pure,skills,plugins}.bend`, `tui/src/commands.rs` (steer/interrupt paths), `switchboard/src/switch.rs:297-305` | must wait for BISE-118 (debt-ts is in `runtime/main.bend`) and BISE-120 |
| 4 | **BISE-123** App root never from the cwd (C1), no `CARGO_MANIFEST_DIR` fallback in release (C4), `BISE_APP_ROOT` | 2 h | `harness/src/main.rs:191-233`, `switchboard/src/core.rs:382-386` | must wait for BISE-113 |
| 5 | **BISE-124** Minimum macOS: `MACOSX_DEPLOYMENT_TARGET` (one value in one file) for cargo and bend in every build path; `release.sh` deleted in favour of `build-dist.sh` (C12); gate check "no binary has `minos` above the target" | 2 h | `bins.sh`, `versions.sh`, `packaging/build-dist.sh`, `release.sh`, `gate.sh` (one check), `rust/.cargo/config.toml` | must wait for debt-bins (owns `bins.sh`, `versions.sh`, `gate.sh` now) |
| 6 | **BISE-125** Rename the command to `bise`: cargo bin name, `bise --version` from `VERSION` (C5), usage text; `bend-harness` kept as a symlink one release; scripts' `CMD=` | 3 h | `rust/harness/Cargo.toml`, `main.rs` (arg parsing), `run.sh`, `packaging/*`, `versions.sh` (file name in the version dir), `switch.rs:173-176` (exe name) | must wait for BISE-123 and BISE-124 |
| 7 | **BISE-126** Agents' tools: build the agents' PATH (§3.4), bundle `rg` (pinned release, per arch) in `<app root>/tools`, `git` checked with `git --version`, English errors (C11: `worktree.rs:106`, others) | 3 h | `switchboard/src/daemon.rs:580-586`, `worktree.rs`, `packaging/build-dist.sh` (fetch rg), `versions.sh` (copy rg) | parallel now for `daemon.rs`/`worktree.rs`; the packaging half waits for BISE-124 |
| 8 | **BISE-127** `bise doctor` (§3.5) | 3 h | `rust/harness/src/doctor.rs` (new), one dispatch line in `main.rs` | must wait for BISE-120 |
| 9 | **BISE-128** CI: GitHub Actions, darwin-arm64 (macos-15) + darwin-x86_64 (macos-15-intel), pinned bend, caches, `build-dist.sh`, `test-install.sh` on both, draft release + `latest.json` | 6 h | `.github/workflows/release.yml` (new), `packaging/test-install.sh` | parallel now (new files); green only after BISE-124 |
| 10 | **BISE-129** Signing + notarization in `build-dist.sh`: entitlements files, `codesign --options runtime --timestamp`, `notarytool submit --wait`, `spctl --assess` check; secrets in CI | 4 h | `packaging/build-dist.sh`, `packaging/entitlements/*.plist` (new), the workflow's sign step | must wait for decision 1 (the certificate) and BISE-128 |
| 11 | **BISE-130** Installer v1: `bise` names, prefix `~/.local/share/bise`, Rosetta check, download from the release URL, the launcher shrinks to "exec current" (logic moves into `bise`) | 3 h | `packaging/install.sh`, `packaging/test-install.sh` | must wait for BISE-125 |
| 12 | **BISE-131** `bise update` + daily background check; brew detection | 4 h | `rust/harness/src/update.rs` (new), `main.rs` dispatch | must wait for BISE-128 (a `latest.json` to read) and BISE-125 |
| 13 | **BISE-132** Installed-mode `/version` and `/restart latest`: versions from installed + `latest.json`; the hub announces a ready update; dev mode unchanged when `VERSION` has `repo=` (C9) | 5 h | `switchboard/src/daemon/versions.rs`, `switch.rs` | must wait for BISE-131 |
| 14 | **BISE-133** Portability test on clean macOS (CI VMs, both arches, macOS 13 if a runner exists, else the oldest available): install, launch in a non-git folder, a git repo, a path with spaces and accents, a deep path; PATH without Homebrew; an agent runs `sb list` and `rg`; `bise doctor` green; uninstall | 4 h | `packaging/test-install.sh`, `projects/switchboard/tests/portable_*.sh` (new) | must wait for BISE-126, BISE-130 |
| — | Homebrew cask (tap repo) | 2 h | the tap repo | after BISE-129 |

Order in short: **now** BISE-126 (hub half) and BISE-128 in parallel;
**after debt-solo/debt-bins** BISE-120, 123, 124; then 121, 122, 125,
127; then 129, 130, 131; then 132, 133, the cask. Total ≈ 55 h.

Coordination: research-providers' BISE-14x (config.toml, auth.json,
cache/models.json) use `bise_home()` from BISE-120; whichever lands first,
the other adapts one call.

---

## 5. Decisions for the user

1. **Who signs and where it is published.** Apple Developer ID
   (Mistral's or yours) and the release host (public GitHub Releases, a
   private bucket). Blocks BISE-129 and any install outside this Mac; the
   EDR makes unsigned builds a non-starter.
2. **Oldest macOS.** Recommendation: **13 Ventura** (Apple still patches
   14+; 13 costs nothing but a test runner). Options: 14 (fewer to test),
   12 (older Intel Macs).
3. **Ship `rg` with bise?** Recommendation: **yes** (≈5 MB, MIT; the
   prompt already assumes it). `git` stays a requirement we check, not a
   thing we ship.
4. **Auto-update**: recommendation **on by default**, daily check,
   running hubs never switched without `/restart latest`. Or: only
   `bise update` by hand.
5. **Code folder**: `~/.local/share/bise` (code) + `~/.bise` (state), as
   Claude Code, recommended; or everything in `~/.bise/` (one folder,
   but deleting it also deletes the program).

Already decided by the brief, not asked again: state in `~/.bise/`,
macOS only, old state imported once and still readable.
