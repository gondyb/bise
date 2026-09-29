# notices/

License texts that cargo cannot find, read by `../third-party-notices.py`
to write THIRD_PARTY_NOTICES (repo root). Fetched once (2026-10) from the
upstream repositories, at the version shipped where there is one:

| file | from |
|---|---|
| v8.LICENSE, v8.LICENSE.fdlibm, v8-glibc.LICENSE, v8-utf8-decoder.LICENSE, v8-siphash.LICENSE | github.com/v8/v8 @ 15.0.245.2 (the V8 of the v8 crate 150.4.0) |
| rusty_v8.LICENSE | github.com/denoland/rusty_v8 @ v150.4.0 (the crate excludes LICENSE*) |
| abseil-cpp.LICENSE | github.com/abseil/abseil-cpp |
| icu.LICENSE | github.com/unicode-org/icu |
| llvm.LICENSE | github.com/llvm/llvm-project libcxx/LICENSE.TXT (libc++, libc++abi, llvm-libc) |
| simdutf.LICENSE-MIT, fp16.LICENSE, fast_float.LICENSE-MIT, dragonbox.LICENSE-Boost | their GitHub repos |
| gemoji.LICENSE | github.com/github/gemoji (rust/tui/src/emoji.tsv) |
| deno.LICENSE.md, nugine-simd.LICENSE, dasp.LICENSE-MIT, dprint-swc-ext.LICENSE | the repos of crates packaged without a license file |
| MIT.spdx.txt | spdx.org/licenses/MIT (sys_traits declares MIT and has no text anywhere) |

After a V8 (v8 crate) upgrade: fetch the V8 files again at the new V8
version (v8/include/v8-version.h in the crate) and update MANUAL in the
script.
