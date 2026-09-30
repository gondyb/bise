# Public audit: secrets and private data (BISE-258)

Audit of gvergnaud/bise before it went public: the tree and the whole
history (every branch and tag, local and on GitHub).

Tools: gitleaks 8 (`gitleaks git` on a mirror, `gitleaks dir` on the tree),
then `rg` over `git log -p --all` for key shapes (sk-, sk-ant-, xox*-, ghp_,
github_pat_, AIza, AKIA, JWT, PEM), chat links, emails, people's names,
internal hosts and personal paths, plus the list of every path ever
committed and every blob over 500 KB.

## Result

**No real secret.** Nothing to rotate. Every key-shaped string is a fake
written for the redaction tests (`rust/session/src/redact.rs`,
`rust/session/tests/redaction.rs`, `tests/fixtures/session/14-redaction/`,
mcp_bootstrap.py); gitleaks flags 3 of them.

**Private text, fixed at HEAD:**

- a real session log copied as a test fixture
  (`tests/fixtures/session-real/orphan-call-4346/events.jsonl`): every
  free-text field is `[redacted]`, the structure `rust/session/tests/pairing.rs`
  needs stays (2.2 MB → 454 KB);
- people's names in a quoted user request (bise-issues.md): generic;
- a private proxy URL in the built-in provider table: bise ships no URL for
  the `foundry` provider, its users set `base_url` under
  `[providers.foundry]` in `~/.bise/config.toml`;
- employer mentions in the docs: reworded;
- launch drafts, pitches and landing-page review rounds: moved out of the
  repo (`/private/`, gitignored; `.gitignore` lists their old paths).

**The history is not rewritten** (the user's call, 2026-09-30: nothing in it
is confidential). Older commits still hold the original versions.

## Kept on purpose

- Public hosts only in the tree: `api.mistral.ai`, `console.mistral.ai`,
  `{resource}.openai.azure.com`, `llm.corp.example`.
- The project's lineage (the harness and SDK it started from) is named.
- `/Users/gabrielvergnaud/...` paths in docs and code; third-party copyright
  emails in THIRD_PARTY_NOTICES; `site/gabriel.jpg`, the landing photo.
- Provider fixtures recorded with `live_providers.py --record`: no key, no
  host, no conversation.

## Never committed

- `.env*`, `auth.json`, keys, stray `events.jsonl` (`.gitignore`); the
  rendered system prompts `prompt-*.html` (`site/content/.gitignore`); the
  site's Vercel token (`site/.env.local`); `logs/`.

## Re-run the check on HEAD

The patterns are in the gitignored `private/history-rewrite/` (they name what
they look for). On HEAD's tree only:

```sh
git grep -I -P -i -l -f private/history-rewrite/patterns.txt HEAD   # expect nothing
```
