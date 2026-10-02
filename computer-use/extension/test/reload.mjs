#!/usr/bin/env node
// A loaded-unpacked extension reloads itself when bise syncs a new build
// into its folder (sw.js checkBuild; rust browsers::sync_extension).
// Headless, throwaway profile: node computer-use/extension/test/reload.mjs
import { cpSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { extDir, fakeBroker, launch, loadExtension, sleep } from "./harness.mjs";

const checks = [];
const check = (name, ok, info) => { checks.push(!!ok); console.log(`${ok ? "ok  " : "FAIL"} ${name}${info !== undefined ? "  " + JSON.stringify(info) : ""}`); };

const dir = mkdtempSync(path.join(process.env.TMPDIR || "/tmp", "cu-ext-"));
cpSync(extDir, dir, { recursive: true, filter: (s) => !s.includes(`${path.sep}test`) });
const stamp = (b) => {
  writeFileSync(path.join(dir, "build.js"), `export const BUILD = "${b}";\n`);
  writeFileSync(path.join(dir, "build.json"), JSON.stringify({ build: b }) + "\n");
};
stamp("a");

const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: "about:blank" });
try {
  const ext = await loadExtension(b, dir);
  // the worker's module may not have run yet right after the load
  for (let i = 0; i < 50 && (await ext.ev("typeof globalThis.bise")) !== "object"; i++) await sleep(100);
  check("loaded build a", (await ext.ev("bise.build")) === "a");
  await ext.ev("bise.checkBuild()");
  await sleep(300);
  check("same build on disk: no reload", (await ext.ev("bise.build")) === "a");
  stamp("b");
  await ext.ev("bise.checkBuild()").catch(() => {}); // the worker goes away mid-call
  // A headless Chrome with an extension loaded through CDP doesn't start
  // the reloaded worker again by itself (a real Chrome does, on the
  // reload's onInstalled; setup-check's 'an update is ready' row covers a
  // browser that doesn't): check the reload happened, the old worker is gone.
  let gone = false;
  for (let i = 0; i < 25 && !gone; i++) {
    await sleep(200);
    const { targetInfos } = await b.send("Target.getTargets");
    gone = !targetInfos.some((t) => t.type === "service_worker" && t.url.includes(ext.id));
  }
  check("a new build on disk: the extension reloads itself", gone);
} finally {
  await b.quit?.();
  broker.close?.();
  rmSync(dir, { recursive: true, force: true });
}


const failed = checks.filter((c) => !c).length;
console.log(failed ? `${failed} of ${checks.length} checks failed` : `${checks.length}/${checks.length} checks passed`);
process.exit(failed ? 1 : 0);
