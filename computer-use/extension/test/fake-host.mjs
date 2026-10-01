#!/usr/bin/env node
// The fake native host Chrome spawns in the tests: native messaging frames
// (4-byte LE length + JSON) on stdio ⇄ JSON lines on a TCP socket to the
// test, which plays bise's broker (contract C4).
import net from "node:net";

const sock = net.connect(Number(process.argv[2]), "127.0.0.1");
let inBuf = Buffer.alloc(0);
process.stdin.on("data", (d) => {
  inBuf = Buffer.concat([inBuf, d]);
  while (inBuf.length >= 4) {
    const n = inBuf.readUInt32LE(0);
    if (inBuf.length < 4 + n) break;
    sock.write(inBuf.subarray(4, 4 + n).toString() + "\n");
    inBuf = inBuf.subarray(4 + n);
  }
});
let lineBuf = "";
sock.on("data", (d) => {
  lineBuf += d.toString();
  let i;
  while ((i = lineBuf.indexOf("\n")) >= 0) {
    const out = Buffer.from(lineBuf.slice(0, i));
    lineBuf = lineBuf.slice(i + 1);
    const len = Buffer.alloc(4);
    len.writeUInt32LE(out.length);
    process.stdout.write(Buffer.concat([len, out]));
  }
});
const bye = () => process.exit(0);
process.stdin.on("end", bye);
sock.on("close", bye);
sock.on("error", bye);
