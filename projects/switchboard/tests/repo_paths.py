#!/usr/bin/env python3
"""Every repo path a dev script names exists (root cleanup, 005c14b).

The bug: after bins.sh/versions.sh moved to scripts/, versions.sh still
ran "$REPO/versions.sh" prune after each build: "No such file", and
"prune failed (every version kept)". Nothing failed, so nothing saw it.

Checks the scripts whose $REPO is this repo (scripts/*.sh,
packaging/build-dist.sh, test-release.sh, test-release-gh.sh) and run.sh:
each "$REPO/<path>" and "$(dirname "$0")/<path>" names a file or folder of
the tree. Build outputs (rust/target/...) are skipped; a path with a glob
or a variable is checked up to it.
"""
import glob
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
FILES = sorted(glob.glob(os.path.join(ROOT, "scripts", "*.sh"))) + [
    os.path.join(ROOT, "projects/switchboard/packaging", f)
    for f in ("build-dist.sh", "test-release.sh", "test-release-gh.sh")
] + [os.path.join(ROOT, "run.sh")]
# the fallbacks for a tree from before the cleanup (an old commit, another
# checkout), on purpose
OLD_LAYOUT_OK = {"versions.sh"}
REF = re.compile(r'(\$REPO|\$\{REPO\}|\$\(dirname "\$0"\))/([A-Za-z0-9_.][A-Za-z0-9_./-]*)')

bad = []
for f in FILES:
    if not os.path.exists(f):
        continue
    here = os.path.dirname(f)
    for n, line in enumerate(open(f), 1):
        if line.lstrip().startswith("#"):
            continue
        for base, path in REF.findall(line):
            path = path.rstrip("./")
            if path.startswith("rust/target") or not path:
                continue
            if "|| vs=" in line or "[ -x \"$REPO/versions.sh\" ]" in line:
                continue
            d = ROOT if "REPO" in base else here
            if not os.path.exists(os.path.join(d, path)):
                rel = os.path.relpath(f, ROOT)
                bad.append(f"{rel}:{n}: {base}/{path} does not exist")

for b in bad:
    print("FAIL", b)
if bad:
    sys.exit(1)
print(f"repo_paths: ok ({len(FILES)} scripts)")
