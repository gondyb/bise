// The real stack as a library (stack.mjs is its command line, paths.mjs
// its tests): a throwaway headless Chrome with the bise extension, whose
// native host is a given `bise computer-use chrome-host` (so the real
// broker), all under a temp BISE_HOME. Never the user's browser.
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, readFileSync, existsSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { launch, loadExtension, sleep, EXT_ID } from "../extension/test/harness.mjs";

export { sleep };

const NL = String.fromCharCode(10);

/**
 * Start the stack: the broker, then a browser whose extension reached it.
 * Returns { home, env, status, connected, agent, events, ctl, b, ext,
 * killBrowser, startBrowser, stop }.
 */
export async function startStack({ bise, browser = "chrome", brokerArgs = ["--no-helper-launch"] }) {
  // A short BISE_HOME: the broker's unix socket path must stay under 104 bytes.
  const home = mkdtempSync(path.join(process.env.HOME, ".bise", "gate", "cu-stack-"));
  mkdirSync(path.join(home, "run"), { recursive: true, mode: 0o700 });
  const env = { ...process.env, BISE_HOME: home, STACK_BISE: bise };
  const ctl = (args) => spawnSync(bise, ["computer-use", ...args], { encoding: "utf8", env });
  const status = () => {
    const r = ctl(["status"]);
    try { return JSON.parse(r.stdout); } catch { return { raw: r.stdout + r.stderr }; }
  };
  const connected = () => (status().browsers || []).filter((x) => x.connected).length;

  // the broker first, so the host and the agents find it
  const broker = spawn(bise, ["computer-use", "broker", ...brokerArgs], { env, stdio: "ignore" });
  await sleep(500);

  let b = null, ext = null;
  const all = [];
  /** One more browser (its own throwaway profile, like a second Chrome profile); it becomes `b`. */
  const startBrowser = async () => {
    const before = connected();
    // harness.launch writes a fake host; overwrite it with the real one
    b = await launch({ browser, brokerPort: 0, startUrl: "about:blank" });
    const hostSh = path.join(b.profile, "host.sh");
    writeFileSync(hostSh, ["#!/bin/bash", `export BISE_HOME="${home}"`, `exec "${bise}" computer-use chrome-host "$@"`, ""].join(NL), { mode: 0o755 });
    writeFileSync(path.join(b.profile, "NativeMessagingHosts", "dev.bise.computer_use.json"), JSON.stringify({
      name: "dev.bise.computer_use", description: "bise computer use (stack)", path: hostSh, type: "stdio",
      allowed_origins: [`chrome-extension://${EXT_ID}/`],
    }));
    ext = await loadExtension(b);
    if (ext.id !== EXT_ID) throw new Error(`extension id ${ext.id}, expected ${EXT_ID}`);
    all.push(b);
    for (let i = 0; i < 50 && connected() <= before; i++) await sleep(200);
    if (connected() <= before) throw new Error("the extension never reached the broker");
    return { b, ext };
  };

  /** C3 as one agent (or as `bise computer-use`: name null, the ctl role): call(op, args) resolves to {ok, result|error, ms}. */
  const agent = async (name) => {
    const sock = net.connect(path.join(home, "run", "computer-use.sock"));
    await new Promise((res, rej) => { sock.once("connect", res); sock.once("error", rej); });
    let buf = "", id = 0;
    const pending = new Map();
    sock.on("data", (d) => {
      buf += d.toString();
      let i;
      while ((i = buf.indexOf(NL)) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        let m;
        try { m = JSON.parse(line); } catch { continue; }
        if (pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
      }
    });
    sock.on("close", () => {
      for (const f of pending.values()) f({ ok: false, error: { code: "closed", message: "the broker closed the connection" } });
      pending.clear();
    });
    const hello = name === null ? { op: "hello", role: "ctl" } : { op: "hello", agent: name, session: "s", tmpdir: process.env.TMPDIR || "/tmp" };
    sock.write(JSON.stringify(hello) + NL);
    return {
      call(op, args = {}) {
        const n = ++id, t0 = Date.now();
        return new Promise((res) => {
          pending.set(n, (m) => res({ ok: m.ok === true, result: m.result, error: m.error, ms: Date.now() - t0 }));
          sock.write(JSON.stringify({ id: n, op, args }) + NL);
        });
      },
      close: () => sock.destroy(),
    };
  };

  /** C6 events.jsonl, parsed. */
  const events = () => {
    const f = path.join(home, "run", "computer-use", "events.jsonl");
    if (!existsSync(f)) return [];
    return readFileSync(f, "utf8").split(NL).filter(Boolean).map((l) => JSON.parse(l));
  };

  const stop = async () => {
    for (const x of all) await x.quit();
    ctl(["stop", "--all"]);
    try { broker.kill("SIGTERM"); } catch { /* gone */ }
    // the brokers and hosts this stack started
    spawnSync("pkill", ["-f", `BISE_HOME=${home}`]);
    rmSync(home, { recursive: true, force: true });
  };

  const stack = {
    home, env, status, connected, agent, events, ctl, stop, startBrowser,
    get b() { return b; },
    get ext() { return ext; },
    /** The browser dies at once (quit, crash): SIGKILL, no goodbye to the host. */
    async killBrowser(which = b) {
      try { which.proc.kill("SIGKILL"); } catch { /* gone */ }
      await sleep(300);
    },
  };
  try {
    await startBrowser();
  } catch (e) {
    await stop();
    throw e;
  }
  return stack;
}
