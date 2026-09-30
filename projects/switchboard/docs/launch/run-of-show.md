# run of show · wed sep 30 (paris time)

latest release: v2026.9.30-4. repo gvergnaud/bise: private until Gabriel's click.

## decisions (before 13:00)

| # | decision | status |
|---|---|---|
| 1 | launch without approvals, said plainly | **decided: yes** (site FAQ already says it) |
| 2 | macOS only, Linux later | **decided** |
| 3 | LGPL glibc bit in the prebuilt V8 | open · main's pick: accept for launch, rebuild later |
| 4 | Mistral: no employer name in any post, site or README (**decided**). heads-up to the manager before 16:00: Gabriel's call (launch pick: yes) | open |
| 5 | X handle (site links x.com/GabrielVergnaud) | open |
| 6 | who you actually know in outreach.md | open |
| 7 | you on camera in the video? French posts beyond LinkedIn thu? | open |
| 8 | demo repo for the video: a toy repo (like 'acme' from the landing demos) or a real one? none exists yet | open |

## timeline

| time | what | who | done |
|---|---|---|---|
| 11:00-12:00 | decisions 3-7 | Gabriel | [ ] |
| 11:00-13:00 | record the 60 s video ([video.md](video.md)) | Gabriel | [ ] |
| 12:00-13:00 | heads-up DMs ([outreach.md](outreach.md)) | Gabriel | [ ] |
| 13:00-14:00 | read every post once, fix the voice, fill the (to verify) | Gabriel + launch | [ ] |
| 14:00 | prepare the Kit email (schedule 16:15), X thread in drafts | Gabriel | [ ] |
| 14:30 | repo public (settings → danger zone → public) | Gabriel | [ ] |
| 14:30-15:30 | clean install test (checklist below) | main | [ ] |
| 15:30 | **go / no-go** | Gabriel | [ ] |
| 15:30 | noindex removed from vercel.json, deploy | designer | [ ] |
| 16:00 | X thread + video, then Bluesky, LinkedIn EN | Gabriel | [ ] |
| 16:05 | Show HN + first comment right after | Gabriel | [ ] |
| 16:15 | newsletter (Kit) | Gabriel | [ ] |
| 16:30 | r/ClaudeAI, r/commandline | Gabriel | [ ] |
| 16:00-22:00 | reply to everything within the hour. bugs → main | Gabriel, main | [ ] |
| 22:00 | thank-you post with real numbers | Gabriel | [ ] |
| thu | LinkedIn FR, first feature post (the inbox) | Gabriel | [ ] |

## go / no-go (15:30)

every line must be yes. one no = we slip the posts, we don't post around it.

- [x] (designer, 7b7bef3) no "Mistral's agent tools" line left on the site, README or posts (Mistral as a model provider is fine)
- [ ] https://github.com/gvergnaud/bise loads logged out (no 404)
- [ ] `curl -fsSL bise.dev/install | sh` works on a clean mac, no gh token, installs v2026.9.30-4 (or later)
- [ ] onboarding with a real key → first message → main answers → one agent starts and finishes
- [ ] `bise doctor` on that machine: ✓ on macOS, bise, signature, home, git, config, keys, model, disk. no `!` line except known dev-only ones (on Gabriel's dev mac today: PATH and hubs warnings, both dev-tree only)
- [ ] README renders on GitHub (animated SVGs load), install line is the same everywhere
- [ ] README, site FAQ, HN comment all say: macOS only, no approvals yet, bring your own key, pre-release
- [ ] bise.dev: no `X-Robots-Tag: noindex` (`curl -sI https://bise.dev | grep -i robots` prints nothing)
- [ ] OG image shows on a link preview (paste bise.dev in an X draft)
- [ ] the desktop CTA link points to the right X handle
- [ ] the video is exported, captions burned in, < 2:20 and < 512 MB for X
- [ ] the "built with bise" number, if used, comes from a real count (to verify: 919 commits on this repo's HEAD today; the public repo may differ)

## if something breaks after 16:00

- install broken: pin a reply under the X thread and the HN comment with the workaround, main fixes, new release. don't delete posts.
- a bug in a reply: thank them, open an issue with their words, link it back when fixed.
- a harsh HN comment: answer once, with facts, no defense. "fair. here's what's true today: …"
