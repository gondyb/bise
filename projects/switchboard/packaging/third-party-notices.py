#!/usr/bin/env python3
"""third-party-notices.py - write THIRD_PARTY_NOTICES at the repo root.

    projects/switchboard/packaging/third-party-notices.py [--check]

The inventory of what the bise bundle ships or the repo vendors:
  - the Rust crates linked into `bise` (rust/, package bend-harness) and
    into `bend-jsrt` (rust/jsrt), from `cargo tree` (normal dependencies,
    no build or proc-macro crates, macOS and Linux targets) and the
    license files of each crate in ~/.cargo/registry (cargo-about style:
    for "A OR B" the first license of PREFER is used);
  - the parts that cargo does not see (V8 and the C/C++ libraries built
    into it, the Bend runtime and packages, gemoji, fonts): MANUAL below,
    their license texts in packaging/notices/.

It prints a summary (crates per license) and every FLAG: a license that
is not permissive (MPL: allowed unmodified, LGPL/GPL/AGPL: a decision)
or no license at all. --check: exit 1 when THIRD_PARTY_NOTICES is not
what this script writes (after a Cargo.lock change: run it again).
Offline: needs the crates in ~/.cargo/registry (any cargo build did it).
"""

import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = subprocess.check_output(["git", "-C", HERE, "rev-parse", "--show-toplevel"], text=True).strip()
NOTICES = os.path.join(HERE, "notices")
OUT = os.path.join(REPO, "THIRD_PARTY_NOTICES")
TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]

# the licenses that need nothing but their text and notice kept
PERMISSIVE = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Zlib", "Unlicense", "BSL-1.0", "Unicode-3.0", "Unicode-DFS-2016", "CDLA-Permissive-2.0",
    "CC0-1.0", "0BSD",
}
# file-level copyleft: fine unmodified, the source stays available (crates.io)
WEAK_COPYLEFT = {"MPL-2.0", "MPL-2.0+"}
# for "A OR B": the one we take
PREFER = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Zlib", "BSL-1.0", "Unlicense",
          "Apache-2.0 WITH LLVM-exception", "CC0-1.0", "MIT-0", "0BSD"]

# what cargo does not see. files: in packaging/notices/ (fetched once from
# the upstream repos at the version shipped; see notices/README.md)
MANUAL = [
    ("bend-jsrt: V8 and the libraries built into it", [
        dict(name="V8 15.0.245.2 (the JavaScript engine, in the rusty_v8 static library)",
             license="BSD-3-Clause", url="https://chromium.googlesource.com/v8/v8",
             files=["v8.LICENSE", "v8.LICENSE.fdlibm"]),
        dict(name="rusty_v8 150.4.0 (crate v8, the Rust binding)", license="MIT",
             url="https://github.com/denoland/rusty_v8", files=["rusty_v8.LICENSE"]),
        dict(name="glibc IBM Accurate Mathematical Library (sin/cos: V8's third_party/glibc, v8_use_libm_trig_functions)",
             license="LGPL-2.1-or-later", url="https://sourceware.org/git/?p=glibc.git",
             files=["v8-glibc.LICENSE"],
             notice="Used unmodified; its source: https://chromium.googlesource.com/v8/v8/+/15.0.245.2/third_party/glibc"),
        dict(name="ICU (V8's Intl; data built in)", license="Unicode-3.0",
             url="https://github.com/unicode-org/icu", files=["icu.LICENSE"]),
        dict(name="Abseil (abseil-cpp)", license="Apache-2.0",
             url="https://github.com/abseil/abseil-cpp", files=["abseil-cpp.LICENSE"]),
        dict(name="LLVM libc++, libc++abi, llvm-libc (V8's C++ runtime, use_custom_libcxx)",
             license="Apache-2.0 WITH LLVM-exception", url="https://github.com/llvm/llvm-project",
             files=["llvm.LICENSE"]),
        dict(name="simdutf", license="Apache-2.0 OR MIT", url="https://github.com/simdutf/simdutf",
             files=["simdutf.LICENSE-MIT"]),
        dict(name="FP16", license="MIT", url="https://github.com/Maratyszcza/FP16", files=["fp16.LICENSE"]),
        dict(name="fast_float", license="Apache-2.0 OR MIT OR BSL-1.0",
             url="https://github.com/fastfloat/fast_float", files=["fast_float.LICENSE-MIT"]),
        dict(name="Dragonbox", license="Apache-2.0 WITH LLVM-exception OR BSL-1.0",
             url="https://github.com/jk-jeon/dragonbox", files=["dragonbox.LICENSE-Boost"]),
        dict(name="UTF-8 decoder (Bjoern Hoehrmann, V8's third_party/utf8-decoder)", license="MIT",
             url="https://bjoern.hoehrmann.de/utf-8/decoder/dfa/", files=["v8-utf8-decoder.LICENSE"]),
        dict(name="SipHash (V8's third_party/siphash)", license="CC0-1.0",
             url="https://github.com/veorq/SipHash", files=["v8-siphash.LICENSE"]),
    ]),
    ("repl-live, repl-scripted, sb-core: the Bend runtime and packages", [
        dict(name="Bend (the compiler's C runtime, compiled into the three binaries)",
             license="Apache-2.0", url="https://github.com/bendlang/bend",
             notice="Copyright 2026 HigherOrderCO", files=[]),
        dict(name="BendHub package 0xbf477e663cf4acb1369a68e0f0fa713b (http, dns, url, encoding, zlib, json, wire)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
        dict(name="BendHub package 0x16458a2db4f36577294543ec2a6d420c (json)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
        dict(name="BendHub package 0x1f4d6c03caf955232d0b0dc6e6f36cf4 (json, vendored and patched as vendor/json.bend)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
    ]),
    ("bise: data built in", [
        dict(name="gemoji (the emoji shortcode table, rust/tui/src/emoji.tsv)", license="MIT",
             url="https://github.com/github/gemoji", files=["gemoji.LICENSE"]),
    ]),
]

# crates whose package has no license file: the upstream repo's, fetched
# once into notices/ (sys_traits has none upstream either: the SPDX MIT
# text, and a FLAG)
CRATE_FILES = {
    "v8": ["rusty_v8.LICENSE"],
    "deno_core": ["deno.LICENSE.md"],
    "serde_v8": ["deno.LICENSE.md"],
    "base64-simd": ["nugine-simd.LICENSE"],
    "vsimd": ["nugine-simd.LICENSE"],
    "dasp_sample": ["dasp.LICENSE-MIT"],
    "dprint-swc-ext": ["dprint-swc-ext.LICENSE"],
    "sys_traits": ["MIT.spdx.txt"],
}
NO_UPSTREAM_TEXT = {"sys_traits"}

NOT_SHIPPED = """\
Not shipped, listed for completeness:
  - OpenSSL 3: the Bend http package loads the system's libssl/libcrypto at
    run time (Apache-2.0); bise does not ship it.
  - JetBrains Mono (SIL Open Font License 1.1): the site
    (projects/switchboard/docs/brand/site) loads it from Google Fonts; no
    font file is in the repo or the bundle.
  - The Rust standard library (MIT OR Apache-2.0) is linked into every Rust
    binary; its license is the Apache-2.0 text below.
"""


def sh(args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True)


def crates(ws, pkg):
    """{(name, version)} linked into the binary of the workspace at ws"""
    args = ["cargo", "tree", "--offline", "-e", "normal,no-proc-macro", "--prefix", "none", "-f", "{p}"]
    for t in TARGETS:
        args += ["--target", t]
    if pkg:
        args += ["-p", pkg]
    out = set()
    for line in sh(args, ws).splitlines():
        m = re.match(r"(\S+) v(\S+)( \((.*)\))?", line)
        if not m:
            continue
        path = m.group(4) or ""
        # our own crates (a path in the repo, not rust/vendor): not third party
        if path.startswith("/") and "/rust/vendor/" not in path:
            continue
        out.add((m.group(1), m.group(2)))
    return out


def metadata(ws):
    md = json.loads(sh(["cargo", "metadata", "--offline", "--format-version", "1"], ws))
    return {(p["name"], p["version"]): p for p in md["packages"]}


def norm_spdx(expr):
    expr = (expr or "").strip()
    # old style "MIT/Apache-2.0"
    if "/" in expr and " OR " not in expr and "(" not in expr:
        expr = " OR ".join(x.strip() for x in expr.split("/"))
    return expr


def choose(expr):
    """the license we use for this crate, or None (all of expr applies)"""
    if not expr or "(" in expr or " AND " in expr:
        return None
    opts = [o.strip() for o in expr.split(" OR ")]
    if len(opts) == 1:
        return opts[0]
    for p in PREFER:
        if p in opts:
            return p
    return None


def ids(expr):
    return {t for t in re.split(r"\s+(?:AND|OR)\s+|[()]", expr.replace(" WITH ", "_WITH_")) if t.strip()}


def classify(expr):
    """ok | weak | FLAG"""
    if not expr:
        return "FLAG"
    c = choose(expr)
    need = {c} if c else {i.replace("_WITH_", " WITH ") for i in ids(expr)}
    if need <= PERMISSIVE:
        return "ok"
    if need <= PERMISSIVE | WEAK_COPYLEFT:
        return "weak"
    return "FLAG"


LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)", re.I)


def license_files(root):
    out = []
    for f in sorted(os.listdir(root)):
        p = os.path.join(root, f)
        if LICENSE_FILE.match(f) and os.path.isfile(p):
            out.append(p)
        elif f.lower() in ("license", "licenses") and os.path.isdir(p):
            out += [os.path.join(p, g) for g in sorted(os.listdir(p)) if os.path.isfile(os.path.join(p, g))]
    return out


def kind(text):
    t = text[:3000]
    if "Apache License" in t and "Version 2.0" in t:
        return "Apache-2.0"
    if "Permission is hereby granted, free of charge" in t:
        return "MIT"
    if "This is free and unencumbered software" in t:
        return "Unlicense"
    return None


def pick_files(files, chosen):
    """the files of the chosen license (all of them when unsure)"""
    if not chosen or len(files) <= 1:
        return files
    keep = []
    for f in files:
        name = os.path.basename(f).upper()
        k = kind(read(f))
        if chosen == "MIT" and ("MIT" in name or k == "MIT"):
            keep.append(f)
        elif chosen == "Apache-2.0" and ("APACHE" in name or k == "Apache-2.0"):
            keep.append(f)
        elif name.startswith("NOTICE") or name.startswith("COPYRIGHT"):
            keep.append(f)
    return keep if [f for f in keep if not os.path.basename(f).upper().startswith(("NOTICE", "COPYRIGHT"))] else files


def read(p):
    with open(p, encoding="utf-8", errors="replace") as fh:
        return fh.read()


def clean(text):
    lines = [ln.rstrip() for ln in text.replace("\r\n", "\n").replace("\t", "    ").split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


def main():
    check = "--check" in sys.argv[1:]
    groups = [("bise", os.path.join(REPO, "rust"), "bend-harness"),
              ("bend-jsrt", os.path.join(REPO, "rust", "jsrt"), None)]
    entries = {}  # (name, version) -> dict
    for binary, ws, pkg in groups:
        md = metadata(ws)
        for key in crates(ws, pkg):
            p = md[key]
            e = entries.setdefault(key, dict(name=key[0], version=key[1], license=norm_spdx(p.get("license")),
                                              url=p.get("repository") or p.get("homepage") or "https://crates.io/crates/" + key[0],
                                              root=os.path.dirname(p["manifest_path"]), bins=set()))
            e["bins"].add(binary)

    texts = {}  # sha -> [text, [users]]
    order = []

    def add_text(text, user):
        text = clean(text)
        h = hashlib.sha256(re.sub(r"\s+", " ", text).encode()).hexdigest()
        if h not in texts:
            texts[h] = [text, []]
            order.append(h)
        if user not in texts[h][1]:
            texts[h][1].append(user)

    apache = clean(read(os.path.join(REPO, "LICENSE")))
    flags, counts, missing = [], {}, []
    for key in sorted(entries, key=lambda k: (k[0].lower(), k[1])):
        e = entries[key]
        user = f"{e['name']} {e['version']}"
        chosen = choose(e["license"])
        cls = classify(e["license"])
        counts[e["license"] or "(none)"] = counts.get(e["license"] or "(none)", 0) + 1
        if cls != "ok":
            flags.append((cls, user, e["license"] or "(no license field)", "/".join(sorted(e["bins"]))))
        files = pick_files(license_files(e["root"]), chosen)
        if not files and e["name"] in CRATE_FILES:
            files = [os.path.join(NOTICES, f) for f in CRATE_FILES[e["name"]]]
        if e["name"] in NO_UPSTREAM_TEXT:
            flags.append(("no-text", user, e["license"] + ": no license file in the crate or its repo; the SPDX text is used",
                          "/".join(sorted(e["bins"]))))
        e["chosen"] = chosen
        if not files:
            if chosen == "Apache-2.0":
                add_text(apache, user)  # the standard text, as in LICENSE
            else:
                missing.append(user)
        for f in files:
            add_text(read(f), user)

    manual_counts = {}
    for _, items in MANUAL:
        for it in items:
            manual_counts[it["license"]] = manual_counts.get(it["license"], 0) + 1
            cls = classify(it["license"])
            if cls != "ok":
                flags.append((cls, it["name"], it["license"], "manual"))

    w = []
    w.append("bise - third-party notices")
    w.append("==========================")
    w.append("")
    w.append("bise is licensed under the Apache License 2.0 (see LICENSE). The bise")
    w.append("distribution contains, or is built from, the third-party software listed")
    w.append("here, each under its own license. The license texts follow the lists.")
    w.append("")
    w.append("Generated by projects/switchboard/packaging/third-party-notices.py from")
    w.append("rust/Cargo.lock and rust/jsrt/Cargo.lock: do not edit by hand.")
    w.append("")
    w.append("1. Summary")
    w.append("")
    w.append(f"Rust crates ({len(entries)}), by license expression:")
    for lic, n in sorted(counts.items(), key=lambda x: (-x[1], x[0])):
        w.append(f"  {n:4d}  {lic}")
    w.append("")
    w.append("Other components (section 3), by license:")
    for lic, n in sorted(manual_counts.items(), key=lambda x: (-x[1], x[0])):
        w.append(f"  {n:4d}  {lic}")
    w.append("")
    w.append("2. Rust crates")
    w.append("")
    w.append("   Binary: bise = the `bise` command (rust/), jsrt = `bend-jsrt` (rust/jsrt).")
    w.append("   For a choice of licenses (\"A OR B\"), bise uses the one after \"->\".")
    w.append("")
    for key in sorted(entries, key=lambda k: (k[0].lower(), k[1])):
        e = entries[key]
        lic = e["license"] or "(no license field)"
        if e["chosen"] and e["chosen"] != e["license"]:
            lic += f" -> {e['chosen']}"
        bins = ",".join("jsrt" if b == "bend-jsrt" else b for b in sorted(e["bins"]))
        w.append(f"  {e['name']} {e['version']}  [{bins}]  {lic}")
        w.append(f"      {e['url']}")
    w.append("")
    mpl = [entries[k] for k in sorted(entries) if classify(entries[k]["license"]) == "weak"]
    if mpl:
        w.append("   The MPL-2.0 crates are used unmodified; their source code:")
        for e in mpl:
            w.append(f"      https://crates.io/crates/{e['name']}/{e['version']}")
        w.append("")
    w.append("   rust/vendor/crossterm is crossterm 0.28.1 with a patch (see its")
    w.append("   README-bise.md), under its MIT license (rust/vendor/crossterm/LICENSE).")
    w.append("")
    w.append("3. Other components")
    for title, items in MANUAL:
        w.append("")
        w.append(f"   {title}")
        w.append("")
        for it in items:
            w.append(f"  {it['name']}  {it['license']}")
            w.append(f"      {it['url']}")
            if it.get("notice"):
                w.append(f"      {it['notice']}")
            for f in it["files"]:
                add_text(read(os.path.join(NOTICES, f)), it["name"].split(" (")[0])
            if it["license"].startswith("Apache-2.0") and not it["files"]:
                add_text(apache, it["name"].split(" (")[0])
    w.append("")
    w.append(NOT_SHIPPED.rstrip())
    w.append("")
    w.append("4. License texts")
    w.append("")
    w.append("   Each text once, with the components it applies to.")
    for h in order:
        text, users = texts[h]
        w.append("")
        w.append("-" * 78)
        w.append("Used by: " + ", ".join(users))
        w.append("-" * 78)
        w.append("")
        w.append(text)
    w.append("")
    out = "\n".join(w)

    print(f"{len(entries)} crates, {len(texts)} distinct license texts", file=sys.stderr)
    for lic, n in sorted(counts.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {n:4d}  {lic}", file=sys.stderr)
    for cls, who, lic, where in flags:
        print(f"FLAG {cls}: {who}: {lic} ({where})", file=sys.stderr)
    for m in missing:
        print(f"MISSING license file: {m}", file=sys.stderr)
    if check:
        cur = read(OUT) if os.path.exists(OUT) else ""
        if cur != out:
            print("THIRD_PARTY_NOTICES is out of date: run " + os.path.relpath(__file__, REPO), file=sys.stderr)
            return 1
        return 0
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
