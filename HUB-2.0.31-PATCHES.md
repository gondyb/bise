# Hub package patches for bend 2.0.31

The pinned hub packages (content-hash imports) were written for the
2.0.29 C runtime API. bend 2.0.31 changed that API, so the cached
copies under ~/.bend/lib were patched in place. These patches live
OUTSIDE the repo; re-fetching the packages reverts them. Only the http
package still needs one (see "Patches removed" below).

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

## Patches removed (fixed in the repo instead)

- 0x1f4d6c03caf955232d0b0dc6e6f36cf4 (json): vendored as
  vendor/json.bend, with its one-line fix (Nat.read.fit / Nat.read.max,
  gone from Base, written out as Nat.is_le(acc, Nat.div(Nat.sub(2^48-1,
  d), 10n))). No module imports the hub hash anymore.
- 0x9bfd9d57916f3439316c2775fd1f10b4 (snaprun): dropped. Base 2.0.32
  ships Process.run; runtime/proc.bend wraps it with the same answer
  (exit status line, then stdout and stderr interleaved).

Only the http package below is still patched in ~/.bend/lib: no
2.0.32-compatible version exists on the hub yet (bend-kit-http
0.21.1.0 still calls UDP.bind(port) in its dns dependency). A build
against a cache holding pristine copies of every other package passes
(BEND_LIB=<dir> points bend at another cache).

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

Also new in 2.0.32: ANY def in the checked import closure that is
`@unsafe` or foreign - even one no law reaches - makes `bend PROOF.bend`
print SOME PROOFS FAIL (exit 1), where 2.0.31 printed "All terms check,
but N defs rely on unsafe or foreign code". `bend <file> -o <out>` still
builds (exit 0).

The gate is green again (ALL PROOFS CHECK) because LAWS.bend imports
only pure modules:
- core/program.bend parses JSON with the pure RFC 8259 library
  (vendor/json.bend) instead of 0x16458a2d (whose encoder is `@unsafe`);
  runtime/main.bend and runtime/skills.bend switched with it.
- every runtime module the laws pin is split in two: `<name>-pure.bend`
  holds the defs the laws reach (no IO, no `@unsafe`, no foreign code
  in their closure) and `<name>.bend` keeps the effects and imports it.
  The LAWS aliases (Rt, Sh, Xt, Pv, Sf, St, Mc, Sk, Rp) point at the
  pure halves, so the law texts did not change.
- the provider retry policy judges plain values (status, Retry-After,
  mapped reply; NetErr for transport failures) instead of the Http
  types, whose library is `@unsafe`.
