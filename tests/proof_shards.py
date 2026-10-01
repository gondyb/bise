#!/usr/bin/env python3
"""Split PROOF.bend into N shard files that check in parallel (gate.sh quick).

    proof_shards.py <repo root> <N> <out dir>

The bend dir is copied into <out dir> and each shard P<i>.bend is written
next to that copy's PROOF.bend, so the header's `import ./...` lines stay
as they are: bend refuses an absolute import path with a dot in it (only
plain names), and <out dir> lives under ~/.bise/gate, a relative one
resolves anywhere. Each shard = PROOF.bend's header + every helper def
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
shutil.copytree(root, out, ignore=shutil.ignore_patterns("*.out", "*.c"), dirs_exist_ok=True)
head, chunks, cur = [], [], None
for line in open(os.path.join(root, "PROOF.bend")).read().split("\n"):
    m = re.match(r"def ([^(:\s]+)", line)
    if m:
        cur = (m.group(1), [line])
        chunks.append(cur)
    elif cur is None:
        head.append(line)
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
