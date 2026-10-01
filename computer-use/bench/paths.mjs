#!/usr/bin/env node
// The paths the bench never takes (docs/computer-use-ship.md §5), on the
// real stack (stack-lib.mjs: headless throwaway Chrome + the extension +
// this tree's host and broker), the test playing an agent over C3:
//
//   1. the debugging bar's Cancel: a stop by the user (C6 cancel_bar), the
//      running call fails `stopped` at once, no reattach until resume
//      (the TUI sends it at the user's next message)
//   2. pages the extension can't drive: chrome://, the Web Store reached
//      by a link, another extension's page, the PDF viewer
//   3. the user closes the agent's tab group by hand
//   4. the extension's service worker stopped (MV3): the next call waits
//      for it to wake (its 30 s alarm) and works on the same tab
//   5. two browsers (two Chrome profiles) at once, then one quits mid-call:
//      the call fails no_browser at once, the other browser goes on, a
//      relaunched browser works again
//
//   node --max-old-space-size=1024 computer-use/bench/paths.mjs --bise <path> [only...]
//
// ~1.5 min (4 waits for the worker's alarm). Exit 1 when a check fails.
// The Web Store check needs the network (skipped without it).
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { startStack, sleep } from "./stack-lib.mjs";

const args = process.argv.slice(2);
const bi = args.indexOf("--bise");
const bise = path.resolve(bi >= 0 ? args[bi + 1] : "bise");
const only = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--bise");
const want = (name) => !only.length || only.includes(name);

const results = [];
const check = (name, ok, detail = "") => {
  results.push({ name, ok: !!ok });
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail === "" ? "" : "  " + (typeof detail === "string" ? detail : JSON.stringify(detail)).slice(0, 400)}`);
};

// A minimal one-page PDF (Chrome's viewer shows it; its text is out of the AX tree's reach).
const PDF = ["%PDF-1.4", "1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj", "2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj",
  "3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 200]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj",
  "4 0 obj<</Length 44>>stream", "BT /F1 24 Tf 20 100 Td (Hello PDF) Tj ET", "endstream endobj",
  "5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj", "trailer<</Root 1 0 R>>", "%%EOF"].join(String.fromCharCode(10));
const PAGE = `<!doctype html><title>Paths</title><h1>Paths</h1><button onclick="this.textContent='pressed'">Press</button>
<a href="https://chromewebstore.google.com/category/extensions">store</a> <a href="chrome://settings">settings</a>`;
const server = createServer((req, res) => {
  if (req.url.startsWith("/doc.pdf")) { res.writeHead(200, { "content-type": "application/pdf" }); res.end(PDF); return; }
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end(PAGE);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/`;

const s = await startStack({ bise });
const tabId = (target) => Number(String(target).slice(4));
const attachedTo = async (ext, id) => (await ext.ev("chrome.debugger.getTargets()")).some((t) => t.tabId === id && t.attached);
const waitFor = async (f, ms = 10_000) => { const end = Date.now() + ms; while (Date.now() < end) { if (await f()) return true; await sleep(100); } return false; };

try {
  // ---- 1. the debugging bar's Cancel
  if (want("cancel")) {
    const a = await s.agent("cancel-me");
    const o = await a.call("open", { url: base });
    const T = o.result?.target;
    const snap = await a.call("snapshot", { target: T });
    check("cancel: the agent drives its tab (attached)", snap.ok && (await attachedTo(s.ext, tabId(T))), snap.error);
    // a call in flight when the user presses Cancel
    const slow = a.call("act", { target: T, action: "wait", text: "never shows", timeout_ms: 20_000 });
    await sleep(500);
    // What Chrome does on Cancel: it detaches every extension debugger and
    // fires onDetach(canceled_by_user). Headless Chrome has no bar to click,
    // so the test detaches and fires the extension's own handler.
    await s.ext.ev(`chrome.debugger.detach({tabId: ${tabId(T)}}).catch(() => {}).then(() => bise.detached({tabId: ${tabId(T)}}, "canceled_by_user"))`);
    const t0 = Date.now();
    const r = await slow;
    check("cancel: the running call fails `stopped` at once", !r.ok && r.error?.code === "stopped" && Date.now() - t0 < 3000, { ms: Date.now() - t0, error: r.error });
    const ev = await waitFor(() => s.events().some((e) => e.agent === "cancel-me" && e.event === "stopped" && e.by === "cancel_bar"));
    check("cancel: events.jsonl says stopped by cancel_bar (main's feed: \"you pressed Cancel in Chrome\")", ev, s.events());
    check("cancel: state.json marks the agent stopped", (await a.call("status")).result?.me?.stopped === true);
    for (const [op, a2] of [["snapshot", { target: T }], ["act", { target: T, action: "click", locator: { role: "button", name: "Press" } }], ["open", { url: base }]]) {
      const x = await a.call(op, a2);
      check(`cancel: ${op} after it fails \`stopped\``, !x.ok && x.error?.code === "stopped", x.error);
    }
    check("cancel: the tab was not attached again", !(await attachedTo(s.ext, tabId(T))));
    // the user's next message to the agent: the TUI runs `resume`
    s.ctl(["resume", "cancel-me"]);
    const again = await a.call("act", { target: T, action: "click", locator: { role: "button", name: "Press" } });
    check("cancel: after resume (the user's next message) it drives again", again.ok && /pressed/.test(again.result?.changed || ""), again.error);
    await a.call("act", { target: T, action: "close" });
    a.close();
  }

  // ---- 2. pages the extension can't drive
  if (want("refused")) {
    const a = await s.agent("pages");
    const set = await a.call("open", { url: "chrome://settings" });
    check("refused: open chrome://settings says why", !set.ok && set.error?.code === "refused" && /chrome:\/\/ pages/.test(set.error.message) && /ask the user/.test(set.error.message), set.error);
    const ext = await a.call("open", { url: "chrome-extension://mhjfbmdgcfjbbpaeojofohoefgiehjai/index.html" });
    check("refused: another extension's page", !ext.ok && ext.error?.code === "refused", ext.error);
    const store = await a.call("open", { url: "https://chromewebstore.google.com/" });
    check("refused: open the Web Store", !store.ok && store.error?.code === "refused" && /extension store/.test(store.error.message), store.error);

    const T = (await a.call("open", { url: base })).result.target;
    await a.call("snapshot", { target: T });
    const toSettings = await a.call("act", { target: T, action: "click", locator: { role: "link", name: "settings" } });
    check("refused: a link to chrome://settings does nothing (Chrome blocks it), the tab stays", toSettings.ok && toSettings.result.url === base, toSettings.error || toSettings.result.url);
    const toStore = await a.call("act", { target: T, action: "click", locator: { role: "link", name: "store" } });
    const online = toStore.ok || !/ERR_|chrome-error/.test(JSON.stringify(toStore));
    if (online && !(toStore.ok && toStore.result.url === base)) {
      check("refused: a link to the Web Store: refused, says where the tab went (not \"the tab is gone\")",
        !toStore.ok && toStore.error?.code === "refused" && /chromewebstore\.google\.com/.test(toStore.error.message) && /chromewebstore/.test(toStore.error.summary || ""), toStore.error);
      const after = await a.call("snapshot", { target: T });
      check("refused: then a snapshot of that tab is refused too", !after.ok && after.error?.code === "refused", after.error);
    } else {
      console.log("skip refused: the Web Store link (no network)");
    }

    const P = (await a.call("open", { url: base + "doc.pdf" })).result?.target;
    const pdf = await a.call("snapshot", { target: P });
    check("refused: a PDF tab's snapshot says the viewer can't be read, and what to do", pdf.ok && /shows a PDF/.test(pdf.result.text) && /from its URL/.test(pdf.result.text), pdf.result?.text || pdf.error);
    const read = await a.call("act", { target: P, action: "read" });
    check("refused: read on a PDF tab says so (not an empty page)", read.ok && /shows a PDF/.test(read.result.changed), read.result?.changed ?? read.error);
    a.close();
  }

  // ---- 3. the user closes the agent's tab group
  if (want("group")) {
    const a = await s.agent("grouped");
    const T = (await a.call("open", { url: base })).result.target;
    await a.call("snapshot", { target: T });
    // the user closes the group: its tabs go, Chrome removes the group
    const { targetInfos } = await s.b.send("Target.getTargets");
    const page = targetInfos.find((t) => t.type === "page" && t.url === base);
    await s.b.send("Target.closeTarget", { targetId: page.targetId });
    const ev = await waitFor(() => s.events().some((e) => e.agent === "grouped" && e.event === "stopped" && e.by === "group_closed"));
    check("group: closing it by hand stops the agent (group_closed)", ev, s.events().filter((e) => e.agent === "grouped"));
    const x = await a.call("open", { url: base });
    check("group: the next call fails `stopped`", !x.ok && x.error?.code === "stopped", x.error);
    s.ctl(["resume", "grouped"]);
    a.close();
  }

  // ---- 4. the service worker stopped
  if (want("worker")) {
    const a = await s.agent("sleepy");
    const T = (await a.call("open", { url: base })).result.target;
    await a.call("snapshot", { target: T });
    // what Chrome does to an MV3 worker it suspends: stop it
    const { targetInfos } = await s.b.send("Target.getTargets");
    const page = targetInfos.find((t) => t.type === "page");
    const { sessionId } = await s.b.send("Target.attachToTarget", { targetId: page.targetId, flatten: true });
    await s.b.send("ServiceWorker.enable", {}, sessionId);
    await s.b.send("ServiceWorker.stopAllWorkers", {}, sessionId);
    const gone = await waitFor(() => s.connected() === 0, 5000);
    check("worker: stopping it closes the port (the broker sees no browser)", gone);
    const t0 = Date.now();
    const snap = await a.call("snapshot", { target: T });
    check("worker: the next call waits for it to wake (alarm) and works on the same tab", snap.ok && /# Paths/.test(snap.result.text), { ms: Date.now() - t0, error: snap.error });
    check("worker: it woke within the 30 s alarm (+ slack)", Date.now() - t0 < 40_000, Date.now() - t0);
    const act = await a.call("act", { target: T, action: "click", locator: { role: "button", name: "Press" } });
    check("worker: acting works after the wake", act.ok, act.error);
    await a.call("act", { target: T, action: "close" });
    a.close();
  }

  // ---- 5. two browsers, then one quits mid-call
  if (want("quit")) {
    const first = { b: s.b, ext: s.ext };
    const a = await s.agent("on-first");
    const T1 = (await a.call("open", { url: base })).result?.target;
    await s.startBrowser(); // a second Chrome profile: the most recent one gets the next open
    check("quit: two browsers connected at once", s.connected() === 2, s.connected());
    const c = await s.agent("on-second");
    const T2 = (await c.call("open", { url: base })).result?.target;
    const onSecond = (await s.ext.ev("chrome.tabs.query({})")).some((t) => t.id === tabId(T2));
    check("quit: a new agent's open goes to the most recent browser", onSecond, T2);
    check("quit: the first agent keeps its tab in the first browser", (await a.call("snapshot", { target: T1 })).ok);
    // a call in flight in the first browser when it quits (crash, cmd+q, the Mac shutting down)
    const slow = a.call("act", { target: T1, action: "wait", text: "never shows", timeout_ms: 30_000 });
    await sleep(500);
    await s.killBrowser(first.b);
    const t0 = Date.now();
    const r = await slow;
    check("quit: the running call fails no_browser, fast, naming Chrome", !r.ok && r.error?.code === "no_browser" && /Chrome closed/.test(r.error.message) && Date.now() - t0 < 5000, { ms: Date.now() - t0, error: r.error });
    const t1 = Date.now();
    const r2 = await a.call("snapshot", { target: T1 });
    check("quit: the agent's old tab is not_found (it is gone with its browser), no wait", !r2.ok && ["not_found", "no_browser"].includes(r2.error?.code) && Date.now() - t1 < 5000, { ms: Date.now() - t1, error: r2.error });
    const other = await c.call("act", { target: T2, action: "click", locator: { role: "button", name: "Press" } });
    check("quit: the other browser's agent goes on", other.ok, other.error);
    const re = await a.call("open", { url: base });
    check("quit: the first agent opens again in the browser left", re.ok, re.error);
    // the last browser quits too: no_browser at once, never not_set_up
    await s.killBrowser(s.b);
    await waitFor(() => s.connected() === 0, 5000);
    const t2 = Date.now();
    const none = await a.call("open", { url: base });
    check("quit: no browser left: no_browser at once, says Chrome is closed", !none.ok && none.error?.code === "no_browser" && /Chrome is closed/.test(none.error.message) && Date.now() - t2 < 5000, { ms: Date.now() - t2, error: none.error });
    // the user opens Chrome again
    await s.startBrowser();
    const back = await a.call("open", { url: base });
    check("quit: after the browser is back, open works", back.ok, back.error);
    a.close();
    c.close();
  }
  // ---- 6. the helper app quits and reopens (what a Screen Recording grant does)
  if (want("helper")) {
    // The real helper code in its dev host (scripts/bundle.sh --devhost):
    // accessory app, no window, never in front. Granting Screen Recording
    // makes macOS quit it and open it again (Quit & Reopen); the broker
    // must pick the new one up on the same socket, with no restart.
    const devhost = path.join(process.env.CU_DEV_DIR || path.join(process.env.HOME, ".bise/cache/cu-apps-dev"), "bise Computer Use dev.app/Contents/MacOS/cu-devhost");
    if (!existsSync(devhost)) {
      console.log(`skip helper: no dev host at ${devhost} (computer-use/macos-app/scripts/bundle.sh --debug --devhost)`);
    } else {
      const sock = path.join(s.home, "run", "computer-use-app.sock");
      const startHelper = () => spawn(devhost, ["--socket", sock, "--no-takeover"], { stdio: "ignore" });
      let h = startHelper();
      await waitFor(() => existsSync(sock), 5000);
      // `permissions` is what /computer-use polls every second (C6); `apps`
      // is an agent's call. The helper answers both, granted or not
      // (without Accessibility, apps is no_permission: still its answer).
      const ctl = await s.agent(null);
      const a = await s.agent("app-bot");
      const answered = (r) => r.ok || r.error?.code === "no_permission";
      const known = (r) => r.ok && typeof r.result?.screen_recording === "boolean";
      const p0 = await ctl.call("permissions");
      check("helper: the real helper answers the setup poll", known(p0), p0.result || p0.error);
      const a0 = await a.call("apps");
      check("helper: and an agent's apps", answered(a0), a0.error);
      // macOS quits it (SIGTERM, like Quit & Reopen)
      h.kill("SIGTERM");
      await new Promise((r) => h.once("exit", r));
      check("helper: on quit it removes its socket", await waitFor(() => !existsSync(sock), 3000));
      const p1 = await ctl.call("permissions");
      check("helper: while it is away the poll says unknown (the row shows checking), at once", p1.ok && p1.result.screen_recording === null && p1.ms < 3000, { ms: p1.ms, r: p1.result });
      const gone = await a.call("apps");
      check("helper: and apps fails no_helper, not a hang", !gone.ok && gone.error?.code === "no_helper" && gone.ms < 3000, { ms: gone.ms, error: gone.error });
      // macOS opens it again
      h = startHelper();
      await waitFor(() => existsSync(sock), 5000);
      const t0 = Date.now();
      const p2 = await ctl.call("permissions");
      check("helper: the reopened helper answers the next poll (same broker, < 1 s)", known(p2) && Date.now() - t0 < 1000, { ms: Date.now() - t0, r: p2.result || p2.error });
      const a2 = await a.call("apps");
      check("helper: and the agent's next apps", answered(a2), a2.error);
      h.kill("SIGTERM");
      a.close();
      ctl.close();
    }
  }
} catch (e) {
  check("no crash", false, String(e.stack || e));
} finally {
  await s.stop();
  server.close();
}
const failed = results.filter((r) => !r.ok);
console.log(`${results.length - failed.length}/${results.length} ok`);
process.exit(failed.length ? 1 : 0);
