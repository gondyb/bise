# Hub package patches for bend 2.0.31

The pinned hub packages (content-hash imports) were written for the
2.0.29 C runtime API. bend 2.0.31 changed that API, so the cached
copies under ~/.bend/lib were patched in place. These patches live
OUTSIDE the repo; re-fetching the packages reverts them.

## 0xbf477e663cf4acb1369a68e0f0fa713b (http/wire/bytes/url/dns/zlib)

- dns/dns.bend: numeric def-name segments renamed (resolve.3 ->
  resolve.dns3, resolve.1b/2/2b -> resolve.dns_*) - the 2.0.31 parser
  rejects numeric segments in dotted def names.
- effs/wire.c: Loc -> u64 (the Loc typedef is gone), Nat -> u64,
  Corpus -> u64* (blk API renamed), and every raw effect CID renamed
  to the namespaced form the 2.0.31 codegen emits
  (CID_RECV -> CID_0XBF..._WIRE_RECV, ...). The file already guards
  every effect block with #ifdef, so unused effects (whose CID defines
  are not emitted) stay skipped.

## 0x1f4d6c03caf955232d0b0dc6e6f36cf4 (json)

- json.bend: Nat.read.fit / Nat.read.max (removed base extensions)
  replaced by Nat.is_le(acc, Nat.div(Nat.sub(2^48-1, d), 10n)), the
  max built by factorization (16777215n * 16777217n) because 2.0.31
  caps Nat literals at 2^32-1.

## 0x9bfd9d57916f3439316c2775fd1f10b4 (snaprun)

- exec.c/start.c/par.c: CID_SNAPRUN_* renamed to the namespaced form
  (CID_0X9BFD..._MAIN_SNAPRUN_*) and the io_eff constructors wrapped
  in #ifdef guards (same pattern as wire.c) for unused effects.
