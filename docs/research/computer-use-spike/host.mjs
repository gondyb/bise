#!/usr/bin/env node
// Minimal native host: 4-byte LE length + JSON, echo with the host pid.
let buf = Buffer.alloc(0);
process.stdin.on("data", (d) => {
  buf = Buffer.concat([buf, d]);
  while (buf.length >= 4) {
    const n = buf.readUInt32LE(0);
    if (buf.length < 4 + n) break;
    const msg = JSON.parse(buf.subarray(4, 4 + n).toString());
    buf = buf.subarray(4 + n);
    const out = Buffer.from(JSON.stringify({ echo: msg, hostPid: process.pid }));
    const len = Buffer.alloc(4); len.writeUInt32LE(out.length);
    process.stdout.write(Buffer.concat([len, out]));
  }
});
