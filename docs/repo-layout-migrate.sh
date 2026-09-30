#!/usr/bin/env bash
# layout-2/layout-3: flatten projects/switchboard/ into docs/ site/ tests/ packaging/.
# Run at the root of a clean checkout of main; it makes the moves and the
# path edits, and commits nothing. Mechanical on purpose: at landing, run it
# again on the main of that day (no rebase of a 380-reference diff).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
[ -d projects/switchboard ] || { echo "no projects/switchboard: already moved?"; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "tracked changes in the tree: commit or stash first"; exit 1; }
P=projects/switchboard
for d in docs site tests packaging; do
  [ ! -e "$d" ] || { echo "$d/ exists at the root (an untracked leftover?): git mv would nest into it; remove it first"; exit 1; }
done

# 1. the moves (git keeps the history of each file: git log --follow)
git mv "$P/docs/brand/site" site
git mv "$P/docs" docs
git mv "$P/spec" docs/spec
git mv "$P/IMPLEMENTATION.md" docs/IMPLEMENTATION.md
git mv "$P/README.md" docs/README.md
git mv "$P/tests" tests
git mv "$P/packaging" packaging
# what is left is untracked (__pycache__, site/.vercel of a checkout): say it
find projects -mindepth 1 -type d -empty -delete 2>/dev/null || true
left="$(find projects -mindepth 1 -maxdepth 4 2>/dev/null | head -5 || true)"
[ -z "$left" ] || { echo "untracked leftovers under projects/ (move .vercel/ by hand, drop the rest):"; echo "$left"; }
rmdir projects/switchboard projects 2>/dev/null || true

# 2. the paths in every tracked text file, except the recorded fixtures (a
# session log says what an agent typed that day), the history doc of the last
# reorg and the at-files test (its tree is made up, "sb/" matches in it)
files() {
  git ls-files -z | xargs -0 grep -IlZ -e 'projects/switchboard' -e 'brand/site' -e 'spec/session-format' -- 2>/dev/null \
    | tr '\0' '\n' | grep -v -e '^tests/fixtures/' -e '^docs/root-layout-plan.md$' -e '^rust/tui/src/files.rs$' || true
}
files | while IFS= read -r f; do
  perl -pi -e '
    s#projects/switchboard/docs/brand/site#site#g;
    s#projects/switchboard/spec/#docs/spec/#g;
    s#projects/switchboard/IMPLEMENTATION\.md#docs/IMPLEMENTATION.md#g;
    s#projects/switchboard/README\.md#docs/README.md#g;
    s#projects/switchboard/##g;
    s#\(projects/switchboard\)#(docs/)#g;
    s#\(projects/switchboard, #(docs/, #g;
    s#(?<![\w/.])docs/brand/site/#site/#g;
    s#(?<![\w/.])docs/brand/site\b#site#g;
    s#(?<![\w/.-])spec/session-format\.ts#docs/spec/session-format.ts#g;
  ' "$f"
done
perl -pi -e 's#^//! Design: projects/switchboard/docs/#//! Design: docs/#' rust/tui/src/files.rs
# serve.py's docstring names the old folder without the projects/ prefix
perl -pi -e 's#^"""Serve brand/site/ #"""Serve site/ #' site/content/serve.py

# 3. the scripts that find the root by depth: tests/ is one level deep now
for f in tests/*.py; do perl -pi -e 's#os\.path\.join\((HERE|os\.path\.dirname\(__file__\)), "\.\.", "\.\.", "\.\."\)#os.path.join($1, "..")#g' "$f"; done
perl -pi -e 's#/\.\./\.\./\.\.#/..#g' tests/gate.sh tests/run_all.sh
# the paths written as os.path.join parts
perl -pi -e 's#"projects", "switchboard", "spec", #"docs", "spec", #' tests/session_ev.py
perl -pi -e 's#"\.\.", "\.\.", "\.\.", "\.\.", "\.\."\)#"..", "..", "..")#; s#"projects", "switchboard", "tests"#"tests"#' docs/brand/qa/capture.py

# 4. relative links whose two ends no longer sit at the same depth
#    docs/brand/* -> site/ is ../../site now; docs/README.md and
#    docs/IMPLEMENTATION.md moved into docs/ next to what they link
perl -pi -e 's#"(site/book/[a-z]+\.html)#"../../$1#g; s#>(site/book/)#>../../$1#g; s#at (site/book/)#at ../../$1#g; s#\(brand/site/ is#(the top-level site/ is#g' docs/brand/tui-*.html
perl -pi -e 's#"\.\./site/index\.html#"../../../site/index.html#g; s#>\.\./site/#>../../../site/#g; s#at \.\./site/#at ../../../site/#g; s#\(brand/site/ is#(the top-level site/ is#g' docs/brand/landing/index.html
perl -pi -e 's#\]\((site/)#](../../$1#g' docs/brand/bise-book.md
perl -pi -e 's#\[`\.\./IMPLEMENTATION\.md`\]\(\.\./IMPLEMENTATION\.md\)#[`IMPLEMENTATION.md`](IMPLEMENTATION.md)#g' docs/rfc-0001-switchboard.md
perl -pi -e 's#\]\(docs/#](#g; s#\]\((packaging|tests)/#](../$1/#g' docs/README.md docs/IMPLEMENTATION.md
# 5. the root README's "what's in here": the projects/switchboard/ row (now
#    "[``]()" after step 2) becomes one row per new folder, in the form the
#    README uses that day: a table ("| [`x/`](x/) | ... |") or a list
#    ("- [`x/`](x/) · ...")
perl -0pi -e '
  my @rows = (["docs/", "design docs, RFCs, the implementation notes"],
              ["site/", "[bise.dev](https://bise.dev), a static site (the installer too)"],
              ["tests/", "the gate (`gate.sh`), e2e and TUI tests"],
              ["packaging/", "build, install and release scripts"]);
  s{^\| \[``\]\(\) \|[^\n]*\n}{join "", map { "| [`$_->[0]`]($_->[0]) | $_->[1] |\n" } @rows}me
  or s{^- \[``\]\(\)[^\n]*\n}{join "", map { "- [`$_->[0]`]($_->[0]) · $_->[1]\n" } @rows}me;
  s{(\[`docs/brand/`\]\(docs/brand/\)[^\n]*?)the brand book, the site, these images}{$1the brand book, the issue list, these images};
' README.md
grep -q '(site/)' README.md || { echo "README: the what's-in-here row was not rewritten: edit it by hand"; exit 1; }
# the at-files test's made-up tree stays; nothing else names projects/
echo "moved. left to read by hand:"; git grep -n -e 'projects/switchboard' -e '"projects", "switchboard"' -- ':!tests/fixtures' ':!docs/root-layout-plan.md' ':!docs/repo-layout-*' || true
