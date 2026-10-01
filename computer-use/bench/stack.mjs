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
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { launch, loadExtension, sleep, EXT_ID } from "../extension/test/harness.mjs";

const args = process.argv.slice(2);
const sep = args.indexOf("--");
const opts = sep < 0 ? args : args.slice(0, sep);
const cmd = sep < 0 ? [] : args.slice(sep + 1);
const opt = (k, d) => { const i = opts.indexOf(k); return i >= 0 ? opts[i + 1] : d; };
const bise = path.resolve(opt("--bise", "bise"));
const browser = opt("--browser", "chrome");

// A short BISE_HOME: the broker's unix socket path must stay under 104 bytes.
const home = mkdtempSync(path.join(process.env.HOME, ".bise", "gate", "cu-stack-"));
mkdirSync(path.join(home, "run"), { recursive: true, mode: 0o700 });
process.env.BISE_HOME = home;
process.env.STACK_BISE = bise;

const status = () => {
  const r = spawnSync(bise, ["computer-use", "status"], { encoding: "utf8", env: process.env });
  try { return JSON.parse(r.stdout); } catch { return { raw: r.stdout + r.stderr }; }
};

let b;
let broker;
const cleanup = async () => {
  if (b) await b.quit();
  spawnSync(bise, ["computer-use", "stop", "--all"], { env: process.env });
  if (broker) try { broker.kill("SIGTERM"); } catch { /* gone */ }
  // the broker started by the host: it exits when idle; make sure
  spawnSync("pkill", ["-f", `BISE_HOME=${home}`]);
  rmSync(home, { recursive: true, force: true });
};

let code = 1;
try {
  // the broker first, so the host and the MCP servers find it
  broker = spawn(bise, ["computer-use", "broker", "--no-helper-launch"], { env: process.env, stdio: "ignore" });
  await sleep(500);
  // harness.launch writes a fake host; overwrite it with the real one
  b = await launch({ browser, brokerPort: 0, startUrl: "about:blank" });
  const hostSh = path.join(b.profile, "host.sh");
  writeFileSync(hostSh, `#!/bin/bash\nexport BISE_HOME="${home}"\nexec "${bise}" computer-use chrome-host "$@"\n`, { mode: 0o755 });
  writeFileSync(path.join(b.profile, "NativeMessagingHosts", "dev.bise.computer_use.json"), JSON.stringify({
    name: "dev.bise.computer_use", description: "bise computer use (stack)", path: hostSh, type: "stdio",
    allowed_origins: [`chrome-extension://${EXT_ID}/`],
  }));
  const ext = await loadExtension(b);
  if (ext.id !== EXT_ID) throw new Error(`extension id ${ext.id}, expected ${EXT_ID}`);
  let st;
  for (let i = 0; i < 50; i++) {
    st = status();
    if ((st.browsers || []).some((x) => x.connected)) break;
    await sleep(200);
  }
  console.error("stack:", JSON.stringify({ browsers: st.browsers, apps: st.apps }));
  if (!(st.browsers || []).some((x) => x.connected)) throw new Error("the extension never reached the broker");
  if (cmd.length) {
    const r = spawnSync(cmd[0], cmd.slice(1), { stdio: "inherit", env: process.env });
    code = r.status ?? 1;
  } else {
    code = 0;
  }
} catch (e) {
  console.error("stack:", e.message || e);
} finally {
  await cleanup();
}
process.exit(code);
