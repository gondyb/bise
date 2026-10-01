// Spike driver: a throwaway Chrome profile, the spike extension loaded
// Run: node docs/research/computer-use-spike/run.mjs (macOS, Chrome installed).
// It opens a separate Chrome window on a temp profile for ~15 s, then quits it.
// Results go to $TMPDIR/spike-result.json (see result.json here for one run).
// through CDP (Extensions.loadUnpacked, pipe only), then the extension
// opens a background tab group and acts in it with chrome.debugger.
// Measures: active tab unchanged, frontmost macOS app unchanged.
import { spawn, execSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";

const here = path.dirname(new URL(import.meta.url).pathname);
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = mkdtempSync(path.join(process.env.TMPDIR, "chrome-profile-"));
const front = () => execSync("lsappinfo info -only pid `lsappinfo front`; lsappinfo info -only name `lsappinfo front`").toString().trim().replace(/\n/g, " ");

const page = (title) =>
  "data:text/html," + encodeURIComponent(`<title>${title}</title><h1>${title}</h1>
  <button onclick="window.clicks=(window.clicks||0)+1">Buy</button> <input placeholder="note">
  <script>window.clicks=0</script>`);

const log = [];
const note = (k, v) => { log.push([k, v]); console.error(k, JSON.stringify(v)); };

// Native host registered inside the throwaway profile only.
execSync(`mkdir -p "${profile}/NativeMessagingHosts"`);
writeFileSync(path.join(here, "host.sh"), `#!/bin/bash\nexec "${process.execPath}" "${path.join(here, "host.mjs")}"\n`, { mode: 0o755 });
note("front_before_launch", front());
const chrome = spawn(CHROME, [
  `--user-data-dir=${profile}`,
  "--remote-debugging-pipe",
  "--enable-unsafe-extension-debugging",
  "--no-first-run", "--no-default-browser-check", "--disable-sync",
  "--window-size=900,600",
  page("user page"),
], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });

// CDP over the pipe: fd 3 = to Chrome, fd 4 = from Chrome, \0-separated JSON.
const toChrome = chrome.stdio[3], fromChrome = chrome.stdio[4];
let id = 0, buf = "";
const pending = new Map();
fromChrome.on("data", (d) => {
  buf += d.toString();
  let i;
  while ((i = buf.indexOf("\0")) >= 0) {
    const msg = JSON.parse(buf.slice(0, i)); buf = buf.slice(i + 1);
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
  }
});
const send = (method, params = {}, sessionId) => new Promise((res, rej) => {
  const m = { id: ++id, method, params }; if (sessionId) m.sessionId = sessionId;
  pending.set(m.id, (r) => (r.error ? rej(new Error(method + ": " + JSON.stringify(r.error))) : res(r.result)));
  toChrome.write(JSON.stringify(m) + "\0");
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

try {
  await sleep(1500);
  const { id: extId } = await send("Extensions.loadUnpacked", { path: path.join(here, "ext") });
  note("extension", extId);
  // An unpacked extension's id comes from its path: allow it once known
  // (the product pins it with a "key" in the manifest).
  writeFileSync(`${profile}/NativeMessagingHosts/dev.bise.spike.json`, JSON.stringify({
    name: "dev.bise.spike", description: "bise spike host", path: path.join(here, "host.sh"),
    type: "stdio", allowed_origins: [`chrome-extension://${extId}/`],
  }));
  await sleep(800);
  const { targetInfos } = await send("Target.getTargets");
  const sw = targetInfos.find((t) => t.type === "service_worker" && t.url.includes(extId));
  const { sessionId } = await send("Target.attachToTarget", { targetId: sw.targetId, flatten: true });
  const ev = async (expr) => {
    const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true }, sessionId);
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 400));
    return r.result.value;
  };

  // The "user" now works in another app: put Terminal-ish focus back.
  note("front_after_launch", front());
  note("before", await ev("spike.snapshot()"));

  const a = await ev(`spike.openTab("computer-use", ${JSON.stringify(page("agent page"))})`);
  const b = await ev(`spike.openTab("computer-use", ${JSON.stringify(page("agent page 2"))})`);
  const c = await ev(`spike.openTab("pr-designer", ${JSON.stringify(page("other agent"))})`);
  note("tabs", { a, b, c });
  note("groups", { a: await ev(`spike.groupInfo(${a.groupId})`), c: await ev(`spike.groupInfo(${c.groupId})`) });
  note("after_open", await ev("spike.snapshot()"));
  note("front_after_open", front());

  const act = await ev(`spike.act(${a.tabId})`);
  note("act", act);
  note("after_act", await ev("spike.snapshot()"));
  note("front_after_act", front());
  const shot = await ev("globalThis.lastShot || ''");
  if (shot) writeFileSync(path.join(process.env.TMPDIR, "spike-shot.png"), Buffer.from(shot, "base64"));

  for (const [tab, mode] of [[c.tabId, "focus"], [b.tabId, "plain"], [b.tabId, "plain"]]) {
    note("click_" + mode, await ev(`spike.clickTimings(${tab}, "${mode}")`));
  }
  note("front_after_clicks", front());
  try { note("native", await ev("spike.native()")); } catch (e) { note("native_error", String(e)); }

  // Stop: detach from the extension side (the product's "stop"), then check the event.
  await ev(`spike.detach(${a.tabId})`);
  await sleep(200);
  note("events", await ev("spike.events"));
} catch (e) {
  note("error", String(e));
} finally {
  writeFileSync(path.join(process.env.TMPDIR, "spike-result.json"), JSON.stringify(log, null, 1));
  chrome.kill("SIGTERM");
  await sleep(1000);
  rmSync(profile, { recursive: true, force: true });
}
