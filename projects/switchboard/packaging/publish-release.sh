#!/usr/bin/env bash
# publish-release.sh — build bise and publish it as a GitHub Release of
# gvergnaud/bise (BISE-217): the one command of a release. The user runs
# it (an agent's shell cannot write the ~100 MB bundle), from the repo.
#
#   projects/switchboard/packaging/publish-release.sh [<tag>] [--rev <rev>]
#       [--add <tarball>]... [--from <tarball>]... [--repo <owner/repo>]
#       [--notes <text>] [--draft] [--dry-run]
#
#   <tag>      vX.Y.Z (default v<YYYY.M.D>, then -2, -3... when taken)
#   --rev      the commit to build (default HEAD); it must be on GitHub
#              already (git push first): the tag is made there
#   --add      another archive of the same commit to publish too (the
#              x86_64 one of CI: gh run download <run> -R gvergnaud/bise
#              -n dist-darwin-x86_64 -D /tmp/x86)
#   --from     publish these archives, build nothing (they name the commit)
#   --draft    a draft (not the latest release: installs don't see it)
#   --dry-run  build and lay out the release, print the gh command, stop
#
# Steps: build-dist.sh <rev> (this Mac's arch) -> make-release.sh --url
# <channel> (install.sh stamped with the channel, latest.json, the
# tarballs + .sha256) -> gh release create <tag> --target <commit>. The
# channel is the LATEST release's download URL, so a new release is what
# every install and `bise update` reads next, with no other change:
#   https://github.com/<repo>/releases/latest/download
# Private repo: installs and updates read it through the reader's `gh`
# login (or GH_TOKEN); public: plain curl. install.sh is the same file
# in both cases, and the one bise.dev/install serves (site/install.sh):
# this script says when that copy is stale.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$HERE" && git rev-parse --show-toplevel)"
SITE_INSTALL="$REPO_DIR/projects/switchboard/docs/brand/site/install.sh"
repo=gvergnaud/bise tag="" rev=HEAD notes="" draft=0 dry=0
adds=() froms=()
while [ $# -gt 0 ]; do
  case "$1" in
    --rev) rev="$2"; shift ;;
    --add) adds+=("$2"); shift ;;
    --from) froms+=("$2"); shift ;;
    --repo) repo="$2"; shift ;;
    --notes) notes="$2"; shift ;;
    --draft) draft=1 ;;
    --dry-run) dry=1 ;;
    -h|--help) sed -n '2,29p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "publish-release: unknown argument $1" >&2; exit 2 ;;
    *) tag="$1" ;;
  esac
  shift
done
say() { echo "publish-release: $*" >&2; }
die() { say "error: $*"; exit 1; }
channel="https://github.com/$repo/releases/latest/download"

command -v gh >/dev/null 2>&1 || die "the GitHub CLI is needed: brew install gh, then gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not logged in: run gh auth login"
gh repo view "$repo" --json name >/dev/null 2>&1 || die "gh cannot read $repo (another account? gh auth status)"

# the tag: given, else today's (the first one free)
if [ -z "$tag" ]; then
  base="v$(date -u +%Y.%-m.%-d)" tag="$base" n=1
  while gh release view "$tag" -R "$repo" >/dev/null 2>&1; do n=$((n + 1)); tag="$base-$n"; done
elif gh release view "$tag" -R "$repo" >/dev/null 2>&1; then
  die "$repo has a release $tag already (gh release delete $tag -R $repo --cleanup-tag, or another tag)"
fi
version="${tag#v}"

work="${TMPDIR:-/tmp}/bise-publish-$tag"
rm -rf "$work"; mkdir -p "$work/build" "$work/release"
tarballs=()
if [ ${#froms[@]} -gt 0 ]; then
  tarballs=("${froms[@]}")
else
  commit="$(cd "$REPO_DIR" && git rev-parse --verify "$rev^{commit}")" || die "no commit $rev"
  # the tag is made on GitHub at this commit: it must be there
  gh api "repos/$repo/commits/$commit" --jq .sha >/dev/null 2>&1 \
    || die "$commit is not on GitHub: git push origin main (or pass --rev a pushed commit)"
  say "building $commit (this Mac: $(uname -m); a few minutes the first time)"
  tarballs+=("$(BISE_CHANNEL=stable "$HERE/build-dist.sh" "$commit" --out "$work/build")")
fi
[ ${#adds[@]} -eq 0 ] || tarballs+=("${adds[@]}")
for t in "${tarballs[@]}"; do [ -f "$t" ] || die "no archive $t"; done

"$HERE/make-release.sh" --out "$work/release" --url "$channel" --version "$version" "${tarballs[@]}" >/dev/null
commit="$(sed -n 's/^  "commit": "\(.*\)",$/\1/p' "$work/release/latest.json")"
[ -n "$commit" ] || die "latest.json names no commit"
gh api "repos/$repo/commits/$commit" --jq .sha >/dev/null 2>&1 \
  || die "$commit (the archives' commit) is not on GitHub: push it first"
targets="$(sed -n 's/^    "\(darwin-[a-z0-9_]*\)": {.*/\1/p' "$work/release/latest.json" | tr '\n' ' ')"
say "release $tag: $targets(commit ${commit:0:12}) in $work/release"
ls -la "$work/release" >&2

notes="${notes:-bise $version (commit ${commit:0:12}), macOS 14+: ${targets% }.
Install: curl -fsSL https://bise.dev/install | sh   (private repo: gh auth login first).
Update: bise update, or /restart latest in Switchboard.}"
args=(release create "$tag" "$work/release"/* -R "$repo" --target "$commit" --title "bise $version" --notes "$notes")
[ "$draft" = 0 ] || args+=(--draft)
if [ "$dry" = 1 ]; then
  say "dry run, nothing published; the command:"
  printf ' %q' gh "${args[@]}" >&2; echo >&2
  exit 0
fi
gh "${args[@]}"
say "published: https://github.com/$repo/releases/tag/$tag"
[ "$draft" = 1 ] || say "the latest release now: every install reads $channel/latest.json"

# bise.dev/install serves a copy of the stamped installer
if ! cmp -s "$work/release/install.sh" "$SITE_INSTALL"; then
  say "bise.dev/install is stale: cp '$work/release/install.sh' '$SITE_INSTALL', commit, and ask designer to deploy the site"
fi
