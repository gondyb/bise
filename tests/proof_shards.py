#!/usr/bin/env python3
"""Split PROOF.bend into N shard files that check in parallel (gate.sh quick).

    proof_shards.py <repo root> <N> <out dir>

Each shard = PROOF.bend's header (imports made absolute, to a copy of the
bend dir in <out dir>/src: bend refuses an import whose real path has a
dot, like ~/.bise/worktrees/..., so <out dir> must have none) + every helper def
(the non-`Laws.` defs and the `Laws.` defs other proofs use) + every N-th
`Laws.` proof. A shard's check then reports the proofs it lacks as TODOs:
it passes when bend says `Error: <k> TODOs found.` with k exactly the
number written to P<i>.expect (the laws in the other shards); a wrong
proof prints its Location instead, a new unproven law makes k larger.
The whole PROOF.bend still runs unsplit in run_all.sh (gate.sh full).
"""
import os
import re
import shutil
import sys

root, n, out = os.path.abspath(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
src = os.path.join(os.path.realpath(out), "src")
shutil.rmtree(src, ignore_errors=True)
shutil.copytree(root, src, ignore=shutil.ignore_patterns("*.out", "*.c"))
head, chunks, cur = [], [], None
for line in open(os.path.join(root, "PROOF.bend")).read().split("\n"):
    m = re.match(r"def ([^(:\s]+)", line)
    if m:
        cur = (m.group(1), [line])
        chunks.append(cur)
    elif cur is None:
        head.append(re.sub(r"^import \./", "import " + src + "/", line))
    else:
        cur[1].append(line)
used = {u for _, ls in chunks for u in re.findall(r"Laws\.[A-Za-z0-9_.]+", "\n".join(ls[1:]))}
helpers = [c for c in chunks if not c[0].startswith("Laws.") or c[0] in used]
laws = [c for c in chunks if not (not c[0].startswith("Laws.") or c[0] in used)]
for i in range(n):
    part = laws[i::n]
    with open(os.path.join(out, f"P{i}.bend"), "w") as f:
        f.write("\n".join(head) + "\n" + "\n".join("\n".join(ls) for _, ls in helpers + part) + "\n")
    with open(os.path.join(out, f"P{i}.expect"), "w") as f:
        f.write(str(len(laws) - len(part)))
