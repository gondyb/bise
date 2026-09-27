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

# Additional patches for bend 2.0.32

bend 2.0.32 changed `TCP.listen(port)` and `UDP.bind(port)` to
`TCP.listen(host, port)` / `UDP.bind(host, port)`: `host` is an IPv4
literal ("127.0.0.1" = this machine only, "0.0.0.0" = every interface).
The 2.0.31 patches above still apply; on top of them:

## 0xbf477e663cf4acb1369a68e0f0fa713b (http/dns)

- dns/dns.bend (resolve.q): `UDP.bind(0)` -> `UDP.bind("0.0.0.0", 0)`.
  An ephemeral client socket that must reach an external DNS server.
- http.bend (serve.with): `TCP.listen(port)` ->
  `TCP.listen("127.0.0.1", port)`, matching the URL it prints
  (`http://127.0.0.1:<port>`). The harness does not use this server.

The harness's own listeners (runtime/repl-live.bend, runtime/repl.bend)
bind "127.0.0.1": every client connects there, and a REPL that runs bash
must not be reachable from the network.

Also new in 2.0.32: a def that relies on `@unsafe` or foreign code now
makes `bend PROOF.bend` print SOME PROOFS FAIL (exit 1) and list every
such def, where 2.0.31 printed "All terms check, but N defs rely on
unsafe or foreign code". The laws concerned are the same 19 as before;
no law is false. `bend <file> -o <out>` still builds (exit 0).
