#!/usr/bin/env node
// Measure Chrome's intensive throttling on agent tabs hidden for a while
// (design §3): three background tabs poll every second for MINUTES (default
// 6.5), each in one mode:
//   plain      never attached until the action at the end
//   attached   chrome.debugger attached (with focus emulation) the whole time
//   lifecycle  attached + Page.setWebLifecycleState {state: "active"}
// Then: the poll rate over the last minute, and one click whose result
// needs a 100 ms timer (does the action work, how long).
// Run: node --max-old-space-size=1024 computer-use/extension/test/throttle.mjs [minutes]
// Result: $TMPDIR/cu-throttle.json.
import { writeFileSync } from "node:fs";
import path from "node:path";
import { servePages, fakeBroker, launch, loadExtension, sleep } from "./harness.mjs";

const minutes = Number(process.argv[2] || 6.5);
const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: pages.base + "next.html?user" });
const out = { minutes, modes: {} };
try {
  await sleep(1500);
  const ext = await loadExtension(b);
  await broker.until(() => broker.hellos.length > 0);
  const modes = ["plain", "attached", "lifecycle"];
  const target = {};
  for (const m of modes) {
    const r = await broker.request("throttle-" + m, "open", { url: pages.base + "poll.html?" + m });
    target[m] = r.result.target;
  }
  // attached: a snapshot attaches (and sets focus emulation); lifecycle: plus the lifecycle call.
  await broker.request("throttle-attached", "snapshot", { target: target.attached });
  await broker.request("throttle-lifecycle", "snapshot", { target: target.lifecycle });
  const lid = Number(target.lifecycle.slice(4));
  out.lifecycleCall = await ext.ev(`chrome.debugger.sendCommand({tabId: ${lid}}, "Page.setWebLifecycleState", {state: "active"}).then(() => "ok", e => String(e))`);
  const t0 = Date.now();
  console.log(`waiting ${minutes} min…`);
  await sleep(minutes * 60_000);
  for (const m of modes) {
    const id = Number(target[m].slice(4));
    const tab = await ext.ev(`chrome.tabs.get(${id}).then(t => ({ status: t.status, discarded: t.discarded, frozen: t.frozen }))`);
    // A frozen tab runs no script: read it once the action woke it up.
    const read = () => ext.ev(`chrome.scripting.executeScript({target: {tabId: ${id}}, world: "MAIN", func: () => ({ ticks: window.ticks.slice(), vis: document.visibilityState })}).then(r => r[0].result, e => null)`);
    const t1 = Date.now();
    let info = await read();
    const readBeforeAct = !!info;
    const r = await broker.request("throttle-" + m, "act", { target: target[m], action: "click", locator: { role: "button", name: "Start job" }, timeout_ms: 10_000 });
    const res = await broker.request("throttle-" + m, "act", { target: target[m], action: "wait", text: "done", timeout_ms: 70_000 });
    if (!info) info = (await read()) || { ticks: [], vis: "?" };
    const last = info.ticks.filter((t) => t > t1 - 60_000 && t <= t1);
    const gaps = last.slice(1).map((t, i) => t - last[i]);
    out.modes[m] = {
      tab,
      readBeforeAct,
      visibilityState: info.vis,
      ticksLastMinute: last.length,
      maxGapMs: gaps.length ? Math.max(...gaps) : null,
      totalTicks: info.ticks.length,
      expectedTicks: Math.floor((Date.now() - t0) / 1000),
      clickOk: r.ok,
      clickMs: r.ms,
      clickChangedShowsDone: r.ok && r.result.changed.includes("done"),
      timerResultMs: r.ms + res.ms,
      error: r.error || res.error,
    };
    console.log(m, JSON.stringify(out.modes[m]));
  }
} catch (e) {
  out.error = String(e.stack || e);
  console.log(out.error);
} finally {
  writeFileSync(path.join(process.env.TMPDIR || "/tmp", "cu-throttle.json"), JSON.stringify(out, null, 1));
  await b.quit();
  pages.close();
  broker.close();
}
