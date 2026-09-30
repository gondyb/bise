# Public audit: secrets and private data (BISE-258)

Audit of gvergnaud/bise before it goes public: the tree at 7ff772f and the
whole history (908 commits, every branch and tag of the local repo and of
origin: `main`, `switchboard`, `single-entry`, `composer-selection`,
`code-quality-audit`, tags `v2026.9.29` to `v2026.9.30-4`).

Tools: gitleaks 8 (`gitleaks git` on a mirror, `gitleaks dir` on the tree),
then `rg` over `git log -p --all` (44 MB) for key shapes (sk-, sk-ant-,
xox*-, ghp_, github_pat_, AIza, AKIA, JWT, PEM), Slack links, emails,
colleague names, internal hosts and personal paths, plus the list of every
path ever committed and every blob over 500 KB.

## Result

**No real secret.** Nothing to rotate. Every key-shaped string in the history
is a fake written for the redaction tests (below, "noise").

**One file of private text:** a real session log copied as a test fixture.
It is redacted at HEAD; the original stays in 2 commits of the history until
the purge below runs.

## Findings

### Secrets (rotate + purge)

None.

### Private text (purge)

1. `projects/switchboard/tests/fixtures/session-real/orphan-call-4346/events.jsonl`
   (before the layout move: `tests/fixtures/session-real/orphan-call-4346/events.jsonl`),
   blob `1c2412559d4a`, 2.2 MB, added in 91a98f0 (BISE-242), moved in 76ebc2c.
   main's real events.jsonl of 2026-09-29: 272 user messages, 456 assistant
   messages, 318 tool results. It holds Slack DM links
   (`slack.com/archives/D09G1LWRSSH/…`, `D0937RRSQ00`, `D09394RMKB2`) and
   Slack user ids, colleague names and work emails (Benjamin Waterlot, Pini,
   `benjamin.waterlot@`, `pierre.mallet.groleau@ext.`, `…wietchner@mistral.ai`),
   `gabriel.vergnaud@gmail.com`, the full system prompt with the private
   skill list (le-chat-grafana: le-chat-server, Grafana Loki/Tempo/Mimir),
   and whole tool outputs.
   - HEAD fix (BISE-258): every free-text field is replaced by `[redacted]`
     (text, args, thinking signatures, descriptions, error messages, paths);
     the structure the test needs stays (seq, types, turns, call ids, the
     `tool X ok:`/`failed:` prefix, the `<agent_message …>` envelope, and the
     args of `call_4346`: `sb inspect worktrees-home --last 3 | tail -c 3000`).
     2.2 MB → 454 KB. `rust/session/tests/pairing.rs`: 7/7 pass on it.
   - History: the purge swaps the original blob for the redacted one in both
     commits (below).

2. `projects/switchboard/docs/brand/bise-issues.md`, BISE-255 spec line: a
   user quote with two colleagues' first names ("benjamin et pini").
   From af8890f in every later version. Low.
   - HEAD fix: "les premiers testeurs". History: `--replace-text` below.

3. Commit 5fd475a's author email
   `gabrielvergnaud@macbook-pro-gabrielvergnaud.cheetah-koi.ts.net`: a machine
   name on a Tailscale tailnet. Low. History: `--mailmap` below maps it to
   `gvergnaud <gabriel.vergnaud@gmail.com>` (the email of the other 911
   commits).

### Not committed, but one `git add` away

- `projects/switchboard/docs/brand/site/content/prompt-main.html`,
  `prompt-task.html` (untracked): rendered system prompts with the same
  private skill list (le-chat-grafana, Vibe Work) and paths. Now ignored
  (`content/.gitignore`: `prompt-*.html`).
- `projects/switchboard/docs/brand/site/.env.local` (untracked, ignored by
  `site/.gitignore`): a Vercel OIDC token (JWT). Never in the history. Keep it
  local.
- `logs/` (ignored): the harness's session logs. Never in the history.

### Noise (no action)

- Fake keys for the redaction tests: `AKIAABCDEFGHIJKLMNOP`,
  `ghp_0123…`, `sk-proj-ABCD…`, `sk-ant-api03-abcdef…` (`rust/session/src/redact.rs`,
  `rust/session/tests/redaction.rs`), `hunter2-the-mistral-key-…` and
  `sk-ant-api03-Zx8QeP2…` (`tests/fixtures/session/14-redaction/`),
  `sk-test-SECRET-0123456789`, `Bearer mistral-test-key` (mcp_bootstrap.py).
  gitleaks flags 3 of them; they are test data.
- Provider fixtures recorded with `live_providers.py --record` (85a9231):
  `anthropic/foundry-tool-call.sse` (a message id, `echo` tool call),
  `openai-chat/mistral-tool-call.sse` (`echo live-ok-79782`),
  `mistral-bad-key.401.json` (`{"detail"}`). No key, no host, no
  conversation.
- The e2e and `session/01…15` fixtures: written by hand or by `gen.py`.
- Public hosts only: `api.mistral.ai` (chat, connectors bootstrap,
  connectors-gateway, audio), `console.mistral.ai`, `{resource}.openai.azure.com`,
  `llm.corp.example`. No internal Mistral host.
- `/Users/gabrielvergnaud/...` paths in docs and code (fine, per the brief).
- Emails in THIRD_PARTY_NOTICES / icu.LICENSE: third-party copyright lines.
- `site/gabriel.jpg`: the landing photo, intended.
- Large binaries: `repl-live`, `repl-scripted`, `sb-core` (Bend native
  builds) were committed until BISE-114 (last 2026-09-27): 78 blobs, 190 MB
  raw. Not private; they make every clone heavier. Optional in the purge.

## History purge (not run on the real repo)

It rewrites every commit from 91a98f0's era on (other SHAs change too with
the mailmap and the binaries), so it needs a force push, and every clone
(the user + 3 collaborators) must re-clone. Tags move to the new commits.

Proven on a mirror clone of the local repo (908 commits, 20 refs):
6 s; 902 commits after (6 commits only touched the binaries); after
`git gc --aggressive` the pack goes from 34.0 MB to 10.2 MB. Checked on the
result: no Waterlot, Slack link, "benjamin et pini" or ts.net in
`git log -p --all`; no binary left; the only fixture blob is the redacted one
(59debf6, the same bytes as HEAD's), so HEAD's commit of BISE-258 becomes empty
for that file.

```sh
# 0. everyone pushes what they have, then stops pushing until step 4
cd /tmp && rm -rf bise-purge && mkdir bise-purge && cd bise-purge
git clone --mirror https://github.com/gvergnaud/bise.git bise.git
# the redacted fixture, from main after BISE-258
git -C bise.git show main:projects/switchboard/tests/fixtures/session-real/orphan-call-4346/events.jsonl > redacted-events.jsonl
printf '%s\n' 'benjamin et pini soon==>les premiers testeurs soon' > replace.txt
printf '%s\n' 'gvergnaud <gabriel.vergnaud@gmail.com> Gabriel Vergnaud <gabrielvergnaud@macbook-pro-gabrielvergnaud.cheetah-koi.ts.net>' > mailmap.txt
cd bise.git
# 1. rewrite (brew install git-filter-repo)
git filter-repo --force \
  --blob-callback 'if blob.original_id == b"1c2412559d4a506ef8c5c2416215fb3fdef2ae8e": blob.data = open("../redacted-events.jsonl","rb").read()' \
  --replace-text ../replace.txt --mailmap ../mailmap.txt \
  --path repl-live --path repl-scripted --path sb-core --invert-paths
# (drop the last line to keep the old binaries in the history)
# 2. check
git log -p --all | rg -i -c 'waterlot|slack\.com/archives|benjamin et pini|cheetah-koi'   # expect nothing
git rev-list --objects --all | rg -c ' (repl-live|repl-scripted|sb-core)$'               # expect nothing
# 3. push (filter-repo removed the remote on purpose)
git remote add origin https://github.com/gvergnaud/bise.git
git push --force --mirror origin
# 4. everyone: re-clone (a pull would merge the old history back in)
```

Before the switch to public:

- GitHub keeps the old commits reachable by SHA (and in any fork or open PR
  ref `refs/pull/*`) until its own GC. While the repo is private this is
  harmless; the safest way to publish is to push the rewritten mirror to a
  fresh repo (delete and recreate `gvergnaud/bise`, or create it under a new
  name and rename), so no old SHA exists on GitHub at all. Otherwise ask
  GitHub Support to purge the cached views of 91a98f0 and 76ebc2c.
- Local refs not on GitHub (`refs/stash`, `refs/switchboard/trash/*`,
  `sb/layout-2`) are not pushed by the steps above; they still hold the old
  fixture in the user's clone, which is fine as long as they are not pushed.
- Old worktrees and branches of the agents are based on old SHAs: finish or
  drop them before the purge, and start new ones from the new main.
