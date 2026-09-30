# History rewrite before going public

Plan, proof and runbook for rewriting the git history of gvergnaud/bise so
that no commit holds private material. Follows [public-audit.md](public-audit.md)
(BISE-258), whose purge recipe it extends. Nothing here was run on GitHub or
on the shared checkout: the proof ran on `git clone --mirror` copies in /tmp.

This document describes what goes; it never quotes it. The exact strings,
paths and scripts are in the gitignored kit `private/history-rewrite/`
(README.md, paths-drop.txt, replace.txt, replace-message.txt, mailmap.txt,
patterns.txt, rewrite.sh, verify.sh, prove.sh).

## 1. What the rewrite removes

Scan: gitleaks on the mirror, then `rg` over `git log -p --all` (48 MB:
contents, commit messages, author and committer lines, tag messages) for
colleague names and emails, Slack links and ids, work emails, the people of
the outreach list, employer and policy talk, internal hosts and URLs, every
host and every email address in the history, and the list of every path
ever committed.

Dropped from every commit (paths-drop.txt, 11 entries):

| what | why |
|---|---|
| `projects/switchboard/docs/launch/` (7 files) | launch drafts, DMs to named people, employer and manager notes. Already moved to `private/launch/` by c743197 |
| `projects/switchboard/docs/brand/site/content/` and its old path `site/content/` (25 files) | designer's local-only drafts (never deployed, in .vercelignore): the launch plan and the hand-off prompt (employer policy, colleagues, people to reach), landing drafts naming the employer, review pages. Designer's call: drop, not redact |
| `projects/switchboard/docs/pitch.md`, `pitch-fr.md` and their old paths `docs/pitch*.md` | internal go-to-market: a wedge and a plan inside the employer. Designer's call: drop |
| `projects/switchboard/docs/public-audit.md` | it names what it found (colleagues, emails, Slack ids) to explain it |
| `repl-live`, `repl-scripted`, `sb-core` at the root | old committed Bend builds (78 blobs, 190 MB raw). Not private: size only |

Replaced in every blob (replace.txt) and every message (replace-message.txt):

| what | where | becomes |
|---|---|---|
| the user quote naming two colleagues (BISE-255 spec) | bise-issues.md | the first testers |
| the sentence naming the employer in the "hi, i'm Gabriel" bio | old README.md and site/index.html | removed (the bio keeps "i build coding agents for a living.") |
| the private npm registry host of the employer | old `repl-tui/package-lock.json` (154 URLs) | `https://registry.npmjs.org/` |
| the private tailnet host of the Anthropic proxy | LAWS.bend, provider-pure.bend, provider.bend, main.bend, models.toml, catalog tests, providers.md, packaging.md (also at HEAD) | `foundry-proxy.example.com` |
| the employer's name next to the endpoint security agent | packaging.md | "a managed Mac" |
| an unreleased product name of the employer | bise-book.md, bise-issues.md | "a chat app" |

Swapped blob: the real session fixture
`tests/fixtures/session-real/orphan-call-4346/events.jsonl` (blob 1c24125,
2.2 MB, commits 91a98f0 and 76ebc2c) becomes BISE-258's redacted version
(59debf6) wherever it appears.

Author identity (mailmap.txt): the one commit whose email is the laptop's
tailnet name (5fd475a) gets the usual `gvergnaud <gabriel.vergnaud@gmail.com>`.

Commit messages: the 16 `launch:` commits and the other commits that only
touched dropped paths become empty and filter-repo drops them (96 commits:
launch, launch plan, pitch, brand/content drafts, binary rebuilds). That
removes every message that named a DM recipient, the employer policy or a
manager. The replacements above also run on the messages that stay.

Kept on purpose:

- Mistral as a model provider (catalog, onboarding, FAQ, API hosts,
  fixtures recorded against the public API): public and fine (designer).
- The fake keys of the redaction tests (gitleaks flags 3: redaction.rs,
  14-redaction/input.jsonl, secrets.json).
- `/Users/gabrielvergnaud/...` paths, the machine name in 12 QA screenshot
  files, the name of the endpoint security product (now without the
  employer's name next to it), third-party names and emails in the license notices.

Needs Gabriel (not redacted, decide after the talk with the employer): the
project's lineage. The name of the employer's internal harness (about 50
lines: PLAN.md, the RFCs, crate descriptions, old prompts, 2 messages), its
SDK's name (about 120 lines, mostly code comments like "same contract as
X", 11 messages), checkouts of the employer's internal repos in 5 files
(PLAN.md, images.md, plugins.md, at-mentions.md, a bench comment in
rust/tui/src/files.rs), and 2 internal doc titles in PLAN.md. Rewriting
them is one more line per term in replace.txt; say which.

## 2. Proof

`private/history-rewrite/prove.sh <src> <dir>`: mirror, rewrite, verify,
sizes, tag map, HEAD diff. Run on 2026-09-30 on both sources:

| source | commits | refs | pack before | pack after |
|---|---|---|---|---|
| shared checkout (main c2ae122, with sb/*, stash, trash refs) | 973 → 877 | 21 → 15 | 43.7 MiB | 10.1 MiB |
| github.com/gvergnaud/bise (main c2ae122) | 967 → 871 | 11 → 11 | 53.2 MiB | 10.1 MiB |

The rewrite takes about 10 s, plus about 10 s for `gc --aggressive`.

`verify.sh` on each rewritten mirror: **CLEAN, 0 hits** in every ref for
the 44 patterns of patterns.txt (contents, messages, identities, tag
messages), none of the 11 dropped paths in any tree, the original fixture
blob unreachable; gitleaks: the 3 known test fakes only. The same script on
the original mirror finds all of them (control).

HEAD after the rewrite: 8 files change by 12 lines (the host and wording
replacements above) and the 25 dropped files are gone from the tip. Gate on
exactly that tree (the rewritten main's tree id 4fc80e6, rebuilt in a gate
worktree): `gate.sh quick` GREEN in 14 s (bise-catalog + bend-harness
tests, clippy, PROOF in 4 shards). The full gate was not run.

Tags (all move; annotated tags stay annotated):

| tag | old | new |
|---|---|---|
| v2026.9.29 | 005c14b | 64eaa3c |
| v2026.9.29-2 | 16c038c | 10773d4 |
| v2026.9.30 | 294f278 | 675b990 |
| v2026.9.30-2 | 0899e3a | 587f8a1 |
| v2026.9.30-3 | 2211f78 | 0956a99 |
| v2026.9.30-4 | af8890f | 798fdf6 |

Every commit gets a new SHA (the old → new map: `filter-repo/commit-map` in
the rewritten mirror; keep it in the kit after the real run).

## 3. Consequences

**Releases.** `bise update` reads `latest.json` and updates when the
release `id` differs from the installed `VERSION` id and its `built` date is
newer (rust/home/src/release.rs `is_update`). The id is the short commit of
the build (af8890f today), but only as an opaque string: nothing resolves it
in git. So a fresh release cut from the new main (new id, newer build date)
updates every installed bise, old ids included. The old releases cannot be
kept as they are: their binaries embed the private proxy host (checked in
the v2026.9.30-4 darwin-arm64 tarball), and their `latest.json` carries the
full old commit SHA, which opens the old history on GitHub after a
force-push. **Decision: drop the 3 old releases (v2026.9.29, v2026.9.30-3,
v2026.9.30-4) and cut one fresh release after the rewrite.** The old tags
go with them (not pushed to the new repo; the map above stays in this doc).

**The private proxy.** After the rewrite the built-in `foundry` provider
points at a placeholder. Gabriel's config uses `model = "opus-5.5"` (an
alias of it), so before the first build of the new main, add to
`~/.bise/config.toml`:

```toml
[providers.foundry]
base_url = "<the real proxy URL>/anthropic/v1"
```

config.toml merges into the catalog key by key and the models file carries
it to the Bend side (not run live). Removing the built-in private provider
from the product is a separate task.

**The live hub.** Its binaries keep running (built versions under
`versions/<old id>`). `versions.json` and the journal name old ids: the
versions stay on disk and `/restart current` works; `/restart <old sha>` no
longer resolves once the old objects are gone; commits mentioned in old
threads (`sb show`) become dead links (look them up in the commit map).
`/restart` (latest) builds the new HEAD under a new id.

**Worktrees and branches.** Every task worktree is on an old SHA. They must
be finished and removed (`gate.sh done <name>`) before the switch. Local
refs not on GitHub (`sb/layout-2`, `sb/ports`, `refs/stash`,
`refs/switchboard/trash/*`) are rewritten by the kit too; keep only the
ones still wanted.

**Collaborators** (1 member, 2 pending invitations; handles in the kit
README): delete their clone and re-clone. A pull would merge the old history back, and a
push of an old branch would publish it again. Installed bise: `bise update`
is enough.

**Shared checkout.** It stays in place (the hub is tied to its path, and
`private/`, `logs/`, `.env.local`, `.vercel/` and the root binaries are
ignored files a fresh clone would not have).

### Force-push vs a fresh repo

| | force-push to gvergnaud/bise | fresh gvergnaud/bise |
|---|---|---|
| old commits on GitHub | still fetchable by SHA until GitHub's GC; cached views; needs a Support ticket to purge | none |
| old releases | kept, must be deleted by hand (private host in the binaries, old SHAs in latest.json) | gone with the old repo |
| to redo | nothing | description, homepage, topics, issues on, wiki off; re-invite the 3 collaborators |
| what is lost | nothing | nothing: 0 stars, 0 forks, 0 issues, 0 PRs, 0 secrets, 0 variables, 0 webhooks, 0 deploy keys, no branch protection (not available on this plan) |
| Vercel | deploys bise.dev with the CLI (`site/.vercel/project.json`; no check runs or statuses on main): nothing to redo | same |
| update channel | same URL | same URL once the new repo has a release (`bise update` fails politely in between) |

**Recommendation: a fresh repo.** Rename the current one to a private
archive (it keeps the old history and old releases as a backup, for Gabriel
only), create a new gvergnaud/bise, push the rewritten history. Nothing is
lost, and no old SHA, including the full SHA printed in the old
latest.json, can reach the public repo. The force-push saves 5 minutes of
settings but leaves a purge to GitHub Support.

## 4. Runbook

Main runs it on Gabriel's go. Stop at the first failure.

1. **Quiet.** The running tasks have landed; every agent is idle; no task
   worktree holds unpushed work; the collaborators stop pushing.
2. **Forward commit on main** (like c743197): move
   `docs/brand/site/content/`, `docs/pitch.md`, `docs/pitch-fr.md` and
   `docs/public-audit.md` to `private/`, and fix the links to them
   (`projects/switchboard/README.md`, bise-book.md, bise-issues.md,
   `.gitignore`). Without it the in-place update (step 8) deletes these
   files from the working tree. Designer has uncommitted edits in
   `site/content/`: move those too.
3. **Proxy.** Add the `[providers.foundry] base_url` lines to
   `~/.bise/config.toml` (section 3).
4. **Backup.** `git clone --mirror <shared checkout> ~/bise-pre-rewrite.git`
   (keeps every local ref). Keep it until the repo has been public for a
   while.
5. **Rewrite.**
   `private/history-rewrite/prove.sh <shared checkout> /tmp/bise-rewrite`.
   It must print `CLEAN`. If new private strings turned up since this
   proof, add them to replace.txt and patterns.txt and run again. Copy
   `/tmp/bise-rewrite/bise.git/filter-repo/commit-map` into the kit. In the
   mirror, delete the refs not to publish (old tags, `sb/*`, stash, trash;
   keep `main` and the branches still wanted).
6. **Gate.** `gate.sh new rewrite-check`, then in it:
   `git fetch /tmp/bise-rewrite/bise.git main && git checkout --detach FETCH_HEAD`,
   `gate.sh quick` (and `full` if time allows), then `gate.sh done rewrite-check`.
7. **GitHub.** Rename gvergnaud/bise to a private archive name; remove its
   collaborators and cancel its invitations. Create gvergnaud/bise
   (private), same description, homepage and topics, issues on, wiki off.
   From the mirror: `git remote add origin https://github.com/gvergnaud/bise.git`
   then `git push origin main <other branches>` (no tags). Clone the new
   repo fresh and run `verify.sh` on it: `CLEAN`.
8. **Shared checkout, in place.** With a clean tree:
   `git fetch /tmp/bise-rewrite/bise.git main:refs/rewrite/main`,
   `git reset --keep refs/rewrite/main` (only the 8 files of section 2
   change), remove the old worktrees (`git worktree prune`), delete the old
   local refs (other branches, `refs/stash`, `refs/switchboard/trash/*`,
   `refs/remotes/origin/*`, old tags, `refs/rewrite/main`),
   `git reflog expire --expire=now --all && git gc --prune=now`,
   `git fetch origin && git branch -u origin/main`. Check:
   `private/history-rewrite/verify.sh .` prints `CLEAN`.
9. **Hub.** `/restart` builds the new HEAD. Check a turn on the proxy model.
10. **Release.** Tag the new main (next version, e.g. `v2026.10.1`), push
    the tag: CI makes the draft; `publish-release.sh <tag> --publish`. On an
    installed bise: `bise update --check` names the new id.
11. **People.** Re-invite the 3 collaborators (kit README); tell them to
    delete their clone and re-clone.
12. **Before the switch to public:** run `verify.sh` on a fresh clone once
    more, and settle the lineage question of section 1.
