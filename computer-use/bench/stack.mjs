#!/usr/bin/env node
// The whole stack, for real, without the user's browser (wave 2):
// a throwaway headless Chrome with the bise extension, whose native host
// is this tree's `bise computer-use chrome-host` (so the real broker),
// all under a temp BISE_HOME. Then it runs a command against that stack
// (the bench, `bise computer-use status`, ...) and tears everything down.
//
//   node computer-use/bench/stack.mjs --bise <path> [--browser chrome|edge|brave] -- <cmd...>
//
// The command gets BISE_HOME (the stack's) in its env, and STACK_BISE.
// Example:
//   node computer-use/bench/stack.mjs --bise rust/target/debug/bise -- \
//     node computer-use/bench/bench.mjs --local --bise rust/target/debug/bise
import { spawnSync } from "node:child_process";
import path from "node:path";
import { startStack } from "./stack-lib.mjs";

const args = process.argv.slice(2);
const sep = args.indexOf("--");
const opts = sep < 0 ? args : args.slice(0, sep);
const cmd = sep < 0 ? [] : args.slice(sep + 1);
const opt = (k, d) => { const i = opts.indexOf(k); return i >= 0 ? opts[i + 1] : d; };
const bise = path.resolve(opt("--bise", "bise"));
const browser = opt("--browser", "chrome");

let code = 1;
let stack = null;
try {
  stack = await startStack({ bise, browser });
  const st = stack.status();
  console.error("stack:", JSON.stringify({ browsers: st.browsers, apps: st.apps }));
  if (cmd.length) {
    const r = spawnSync(cmd[0], cmd.slice(1), { stdio: "inherit", env: stack.env });
    code = r.status ?? 1;
  } else {
    code = 0;
  }
} catch (e) {
  console.error("stack:", e.message || e);
} finally {
  if (stack) await stack.stop();
}
process.exit(code);
