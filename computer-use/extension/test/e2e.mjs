#!/usr/bin/env node
// End to end: a throwaway Chrome (or Edge: --browser edge) with the
// extension, the test plays bise's broker over the fake native host (C4)
// and drives every op and action on the local pages of test/pages/.
// Run: node --max-old-space-size=1024 computer-use/extension/test/e2e.mjs [--browser chrome|edge]
// Headless, on a temp profile (~1 min): no window shows; it quits the browser at the end.
// Result: $TMPDIR/cu-e2e-<browser>.json; exit code 1 when a check fails.
import { writeFileSync } from "node:fs";
import path from "node:path";
import { servePages, fakeBroker, launch, loadExtension, pageSession, sleep, EXT_ID, installed } from "./harness.mjs";

const browser = process.argv.includes("--browser") ? process.argv[process.argv.indexOf("--browser") + 1] : "chrome";
if (!installed(browser)) {
  console.log(`${browser} is not installed here: skipped`);
  process.exit(0);
}

const results = [];
const check = (name, ok, detail = "") => {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? "  " + (typeof detail === "string" ? detail : JSON.stringify(detail)) : ""}`);
};

const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ browser, brokerPort: broker.port, startUrl: pages.base + "next.html?user" });
const A = "api-v2", B = "pr-designer";

try {
  await sleep(1500);
  const ext = await loadExtension(b);
  check("extension id is the pinned one", ext.id === EXT_ID, ext.id);
  check("hello on the native port", await broker.until(() => broker.hellos.length > 0, 10_000), broker.hellos[0]);
  const hello = broker.hellos[0] || {};
  check("hello names the browser and versions", hello.browser === (browser === "edge" ? "edge" : "chrome") && /^\d+\./.test(hello.version) && hello.extension_version === "0.1.0", hello);

  const req = async (agent, op, args) => {
    const r = await broker.request(agent, op, args);
    return r;
  };
  const act = (agent, target, action, rest = {}) => req(agent, "act", { target, action, ...rest });
  const win = () => ext.ev("chrome.windows.getAll({populate: true}).then(ws => ws.map(w => ({id: w.id, focused: w.focused, tabs: w.tabs.map(t => ({id: t.id, active: t.active, groupId: t.groupId, url: t.url}))})))");
  const userTab = async () => (await win()).flatMap((w) => w.tabs).find((t) => t.url.includes("?user"));
  const before = await userTab();
  check("user's tab active at start", before?.active, before);

  // ---- open: groups per agent, background tabs
  const o1 = await req(A, "open", { url: pages.base + "form.html" });
  const o2 = await req(A, "open", { url: pages.base + "slow.html" });
  const o3 = await req(B, "open", { url: pages.base + "form.html" });
  check("open returns target, url, title", o1.ok && /^tab:\d+$/.test(o1.result.target) && o1.result.title === "Test shop", o1.result || o1.error);
  check("open summary", o1.result?.summary === "opened a tab · 127.0.0.1:" + new URL(pages.base).port, o1.result?.summary);
  const T1 = o1.result.target, T2 = o2.result.target, T3 = o3.result.target;
  const groups = await ext.ev("chrome.tabGroups.query({}).then(gs => gs.map(g => ({title: g.title, color: g.color, collapsed: g.collapsed})))");
  check("one pink, open group per agent", groups.length === 2 && groups.every((g) => g.color === "pink" && !g.collapsed) && groups.some((g) => g.title === "bise · api-v2") && groups.some((g) => g.title === "bise · pr-designer"), groups);
  const ws = await win();
  const tabsA = ws.flatMap((w) => w.tabs).filter((t) => [T1, T2].includes("tab:" + t.id));
  check("agent tabs share one group", tabsA.length === 2 && tabsA[0].groupId === tabsA[1].groupId && tabsA.every((t) => !t.active), tabsA);
  check("user's tab still active after open", (await userTab())?.active);

  const lt = await req(A, "tabs", {});
  check("tabs lists this agent's tabs only", lt.ok && lt.result.length === 2 && lt.result.every((t) => [T1, T2].includes(t.target) && t.user_touched === false), lt.result);
  const foreign = await req(B, "snapshot", { target: T1 });
  check("another agent's tab is not_found", !foreign.ok && foreign.error.code === "not_found", foreign.error);

  // ---- snapshot (C2)
  const s1 = await req(A, "snapshot", { target: T1 });
  const text = s1.result?.text || "";
  check("snapshot first line is # title · host", text.split("\n")[0] === `# Test shop · 127.0.0.1:${new URL(pages.base).port}`, text.split("\n")[0]);
  check("snapshot lines carry roles, names, refs", /- button "Add to cart" \[e\d+\]/.test(text) && /- heading "Test shop" \[e\d+\]/.test(text) && /- searchbox "Search" \[e\d+\] value="old text"/.test(text), text);
  check("snapshot marks disabled", /- button "Pay now" \[e\d+\] \(disabled\)/.test(text));
  check("password value is masked", /- textbox "Password" \[e\d+\] value="•••"/.test(text) && !text.includes("hunter2"));
  check("snapshot refs count", s1.result.refs === (text.match(/\[e\d+\]/g) || []).length && s1.result.truncated === false, s1.result.refs);
  const s1small = await req(A, "snapshot", { target: T1, max_nodes: 3 });
  check("max_nodes cuts and says so", s1small.result.truncated === true && s1small.result.refs === 3, s1small.result.text);
  console.log(text);

  // ---- click (the 5 s trap is gone: focus emulation)
  const c1 = await act(A, T1, "click", { locator: { role: "button", name: "Add to cart" } });
  check("click lands", c1.ok && c1.result.changed.includes('+ text "bought 1"') || c1.result?.changed.includes("bought 1"), c1.result || c1.error);
  check("click answers in < 5 s (settle included)", c1.ms < 5000, `${c1.ms} ms`);
  check("click summary", c1.result?.summary === `clicked "Add to cart" · 127.0.0.1:${new URL(pages.base).port}`, c1.result?.summary);
  const ref = /- button "Add to cart" \[(e\d+)\]/.exec(text)[1];
  const c2 = await act(A, T1, "click", { ref });
  check("click by ref", c2.ok && c2.result.changed.includes("bought 2"), c2.result?.changed || c2.error);
  const timing = [];
  for (let i = 0; i < 3; i++) { const r = await act(A, T1, "click", { ref }); timing.push(r.ms); }
  check("three more clicks answer in < 5 s", timing.every((ms) => ms < 5000), timing);
  // The 5 s trap is in the mouse events themselves: measured inside the worker.
  const inputs = (await ext.ev("bise.log")).filter((x) => x.input === "click").map((x) => x.ms);
  check("no 5 s trap: every click's mouse events take < 1 s", inputs.length >= 5 && inputs.every((ms) => ms < 1000), inputs);

  // ---- locator errors
  const amb = await act(A, T1, "click", { locator: { role: "button", name: "More" }, timeout_ms: 300 });
  check("ambiguous lists candidates", !amb.ok && amb.error.code === "ambiguous" && amb.error.candidates.length === 2, amb.error);
  const nth = await act(A, T1, "click", { locator: { role: "button", name: "More", nth: 1 } });
  check("nth picks one", nth.ok, nth.error);
  const nf = await act(A, T1, "click", { locator: { role: "button", name: "Checkout" }, timeout_ms: 300 });
  check("not_found lists candidates", !nf.ok && nf.error.code === "not_found" && nf.error.candidates.length > 0 && nf.error.summary === 'couldn\'t find "Checkout"', nf.error);
  const dis = await act(A, T1, "click", { locator: { role: "button", name: "Pay now" }, timeout_ms: 400 });
  check("disabled → timeout", !dis.ok && dis.error.code === "timeout" && /disabled/.test(dis.error.message) && dis.error.summary === "couldn't click \"Pay now\": it's disabled", dis.error);
  const re = await act(A, T1, "read", { locator: { text_re: "price: \\d+/i" } });
  check("text_re with flags + read", re.ok && re.result.changed === "Price: 12,50 €", re.result || re.error);

  // ---- fill, type, press, select, check, hover
  const f1 = await act(A, T1, "fill", { locator: { role: "searchbox" }, text: "usb-c cable 2m" });
  check("fill replaces the value", f1.ok && f1.result.changed.includes('value="usb-c cable 2m"') && f1.result.summary.startsWith('typed in "Search"'), f1.result || f1.error);
  const p1 = await act(A, T1, "press", { keys: "Enter" });
  check("press Enter submits", p1.ok && p1.result.changed.includes("searched usb-c cable 2m") && p1.result.summary.startsWith("pressed enter"), p1.result || p1.error);
  const t1 = await act(A, T1, "type", { locator: { label: "Email" }, text: "ana@" });
  const t2 = await act(A, T1, "type", { locator: { label: "Email" }, text: "example.com" });
  check("type appends", t1.ok && t2.ok && t2.result.changed.includes('value="ana@example.com"'), t2.result || t2.error);
  const p2 = await act(A, T1, "press", { locator: { label: "Email" }, keys: "Meta+A Backspace" });
  const emailNow = await act(A, T1, "read", { locator: { label: "Email" } });
  check("press chords: Meta+A then Backspace clears", p2.ok && emailNow.result?.changed === "", { p2: p2.error, read: emailNow.result?.changed });
  const sel = await act(A, T1, "select", { locator: { label: "Size" }, value: "Large" });
  check("select by label", sel.ok && /value="Large"/.test(sel.result.changed) && sel.result.summary.startsWith('picked "Large" in "Size"'), sel.result || sel.error);
  const ch = await act(A, T1, "check", { locator: { role: "checkbox", name: "Remember me" } });
  check("check", ch.ok && ch.result.changed.includes("(checked)") && ch.result.summary.startsWith('checked "Remember me"'), ch.result || ch.error);
  const un = await act(A, T1, "check", { locator: { role: "checkbox", name: "Remember me" }, value: false });
  check("uncheck (value false)", un.ok && un.result.summary.startsWith('unchecked "Remember me"'), un.result || un.error);
  const hv = await act(A, T1, "hover", { locator: { role: "button", name: "Menu" } });
  check("hover fires mouseenter", hv.ok && hv.result.changed.includes('"hovered"'), hv.result || hv.error);
  const pwr = await act(A, T1, "read", { locator: { label: "Password" } });
  check("read never returns a password", pwr.ok && pwr.result.changed === "•••", pwr.result);
  const pg = await act(A, T1, "read", {});
  check("read without a target: the page text", pg.ok && pg.result.changed.includes("Test shop") && pg.result.changed.length <= 4001, pg.result?.changed?.slice(0, 80));

  // ---- auto-wait, covered element, wait
  const cov = await act(A, T2, "click", { locator: { role: "button", name: "Under the popup" }, timeout_ms: 500 });
  check("covered element → timeout naming it", !cov.ok && cov.error.code === "timeout" && /covers/.test(cov.error.message) && cov.error.summary === "couldn't click \"Under the popup\": something covers it", cov.error);
  const acc = await act(A, T2, "click", { locator: { name: "Accept cookies" } });
  const under = await act(A, T2, "click", { locator: { name: "Under the popup" } });
  check("after closing the popup the click lands", acc.ok && under.ok && under.result.changed.includes("clicked under"), under.result || under.error);
  const lm = await act(A, T2, "click", { timeout_ms: 3000, locator: { role: "button", name: "Load more" } });
  check("auto-wait for an element that shows up later", lm.ok, lm.error);
  const wt = await act(A, T2, "wait", { text: "Results are ready" });
  check("wait for text", wt.ok, wt.error);
  const ws2 = await act(A, T2, "wait", { amount: 200 });
  check("wait amount", ws2.ok && ws2.ms >= 200, ws2.ms);

  // ---- scroll
  const sc = await req(A, "open", { url: pages.base + "scroll.html" });
  const T4 = sc.result.target;
  const yOf = async (tabId) => ext.ev(`chrome.scripting.executeScript({target:{tabId:${tabId}}, func: () => [scrollY, document.getElementById('box').scrollTop]}).then(r => r[0].result)`);
  const id4 = Number(T4.slice(4));
  const sd = await act(A, T4, "scroll", {});
  const y1 = await yOf(id4);
  check("scroll down by default (80% of the view)", sd.ok && y1[0] > 200 && sd.result.summary.startsWith("scrolled down"), { y1, err: sd.error });
  const su = await act(A, T4, "scroll", { direction: "up", amount: 100 });
  const y2 = await yOf(id4);
  check("scroll up by amount", su.ok && Math.abs(y1[0] - y2[0] - 100) <= 1, { y1, y2 });
  const sr = await act(A, T4, "scroll", { locator: { role: "heading", name: "Reviews" } });
  check("scroll to an element", sr.ok && sr.result.summary.startsWith('scrolled to "Reviews"'), sr.result || sr.error);
  const sb = await act(A, T4, "scroll", { locator: { role: "region", name: "Inner list" }, direction: "down", amount: 50 });
  const y3 = await yOf(id4);
  check("scroll inside an element", sb.ok && y3[1] === 50, { y3, err: sb.error });

  // ---- screenshot
  const shot = await req(A, "screenshot", { target: T1, max_width: 640 });
  check("screenshot: jpeg ≤ max_width", shot.ok && shot.result.mime === "image/jpeg" && shot.result.width > 0 && shot.result.width <= 640 && shot.result.data.startsWith("/9j/"), { w: shot.result?.width, h: shot.result?.height, ms: shot.ms, err: shot.error });
  const eshot = await req(A, "screenshot", { target: T1, locator: { role: "button", name: "Add to cart" } });
  check("screenshot of one element", eshot.ok && eshot.result.width > 0 && eshot.result.width < 300 && eshot.result.height < 100, { w: eshot.result?.width, h: eshot.result?.height, err: eshot.error });
  if (shot.ok) writeFileSync(path.join(process.env.TMPDIR || "/tmp", "cu-e2e-shot.jpg"), Buffer.from(shot.result.data, "base64"));

  // ---- goto, stale refs, refusals, limits
  const g = await act(A, T1, "goto", { url: pages.base + "next.html" });
  check("goto", g.ok && g.result.title === "Next page" && g.result.summary === "went to 127.0.0.1:" + new URL(pages.base).port && g.result.changed.startsWith("# Next page"), g.result || g.error);
  const stale = await act(A, T1, "click", { ref });
  check("old ref after navigation → stale_ref", !stale.ok && stale.error.code === "stale_ref", stale.error);
  const lk = await act(A, T1, "goto", { url: pages.base + "form.html" });
  const link = await act(A, T1, "click", { locator: { role: "link", name: "Next page" } });
  check("click a link navigates, changed shows the new page", lk.ok && link.ok && link.result.title === "Next page" && link.result.changed.startsWith("# Next page"), link.result || link.error);
  const chromeUrl = await act(A, T1, "goto", { url: "chrome://settings" });
  check("chrome:// refused", !chromeUrl.ok && chromeUrl.error.code === "refused", chromeUrl.error);
  const store = await req(A, "open", { url: "https://chromewebstore.google.com/" });
  check("the Web Store refused", !store.ok && store.error.code === "refused", store.error);
  const have = (await req(A, "tabs", {})).result.length;
  let fifth;
  for (let i = have; i < 5; i++) fifth = await req(A, "open", { url: pages.base + "next.html" });
  check("up to 5 tabs per agent", fifth.ok && (await req(A, "tabs", {})).result.length === 5, fifth.error);
  const sixth = await req(A, "open", { url: pages.base + "next.html" });
  check("a 6th tab is refused", !sixth.ok && sixth.error.code === "refused" && /5 tabs/.test(sixth.error.message), sixth.error);
  const bad = await act(A, T1, "fly", {});
  check("unknown action → bad_args", !bad.ok && bad.error.code === "bad_args", bad.error);
  const badKey = await act(A, T1, "press", { keys: "Hyper+Q" });
  check("unknown key → bad_args", !badKey.ok && badKey.error.code === "bad_args", badKey.error);

  // ---- two agents at once
  const [x, y] = await Promise.all([
    act(A, T2, "read", { locator: { role: "heading" } }),
    act(B, T3, "click", { locator: { role: "button", name: "Add to cart" } }),
  ]);
  check("two agents act at the same time", x.ok && y.ok && y.result.changed.includes("bought 1"), { x: x.error, y: y.error });
  check("user's tab still active after all actions", (await userTab())?.active);
  const t2tab = await ext.ev(`chrome.tabs.get(${Number(T2.slice(4))}).then(t => t.active)`);
  const vis = await ext.ev(`chrome.scripting.executeScript({target:{tabId:${Number(T2.slice(4))}}, func: () => document.visibilityState}).then(r => r[0].result)`);
  // Focus emulation makes an attached background tab report "visible" (measured): the tab stays inactive.
  check("agent tab stayed in the background", t2tab === false, { active: t2tab, visibilityState: vis });

  // ---- takeover: the user activates an agent tab
  const id3 = Number(T3.slice(4));
  await ext.ev(`chrome.tabs.update(${id3}, {active: true}).then(() => 1)`);
  check("activating an agent tab → paused event", await broker.until(() => broker.events.some((e) => e.event === "paused" && e.agent === B && e.target === T3)), broker.events);
  const pz = await act(B, T3, "click", { locator: { role: "button", name: "Add to cart" } });
  check("act on a paused tab → paused", !pz.ok && pz.error.code === "paused" && pz.error.summary === 'couldn\'t click "Add to cart": you\'re using this tab', pz.error);
  const tb = await req(B, "tabs", {});
  check("tabs shows user_touched", tb.result.find((t) => t.target === T3)?.user_touched === true, tb.result);
  broker.send({ resume: B });
  await sleep(200);
  const rs = await act(B, T3, "click", { locator: { role: "button", name: "Add to cart" } });
  check("resume → acts again (in the visible tab, cursor drawn)", rs.ok, rs.error);
  const cur = await req(B, "screenshot", { target: T3 });
  if (cur.ok) writeFileSync(path.join(process.env.TMPDIR || "/tmp", "cu-cursor.jpg"), Buffer.from(cur.result.data, "base64"));
  // The user clicks in the (visible) tab: the content script sees it.
  await sleep(700);
  const n0 = broker.events.length;
  const user = await pageSession(b, (t) => t.targetId && t.url.includes("form.html") && true);
  const visibleTarget = (await b.send("Target.getTargets")).targetInfos.find((t) => t.type === "page" && t.url.includes("form.html"));
  void visibleTarget;
  await user.call("Input.dispatchMouseEvent", { type: "mousePressed", x: 5, y: 5, button: "left", clickCount: 1 });
  await user.call("Input.dispatchMouseEvent", { type: "mouseReleased", x: 5, y: 5, button: "left", clickCount: 1 });
  await user.detach();
  check("a user click in the tab → paused", await broker.until(() => broker.events.slice(n0).some((e) => e.event === "paused" && e.agent === B), 3000), broker.events.slice(n0));
  const cursorDrawn = await ext.ev(`chrome.scripting.executeScript({target:{tabId:${id3}}, func: () => !!document.querySelector('bise-cursor')}).then(r => r[0].result)`);
  check("cursor overlay is in the page", cursorDrawn === true);
  await ext.ev(`chrome.tabs.update(${before.id}, {active: true}).then(() => 1)`);

  // ---- release, stop by closing the group, drop
  broker.send({ release: A });
  await sleep(300);
  const attached = await ext.ev(`[...bise.tabs.values()].filter(t => t.agent === "${A}" && t.attached).length`);
  const kept = (await req(A, "tabs", {})).result.length;
  check("release detaches (tabs stay)", attached === 0 && kept === 5, { attached, kept });
  const groupA = await ext.ev(`chrome.tabGroups.query({title: "bise · ${A}"}).then(g => g[0].id)`);
  await ext.ev(`chrome.tabs.query({groupId: ${groupA}}).then(ts => chrome.tabs.remove(ts.map(t => t.id))).then(() => 1)`);
  check("closing the group → stopped event", await broker.until(() => broker.events.some((e) => e.event === "stopped" && e.agent === A && e.reason === "group_closed")), broker.events.filter((e) => e.event === "stopped"));
  const st = await req(A, "open", { url: pages.base + "next.html" });
  check("a stopped agent gets stopped", !st.ok && st.error.code === "stopped", st.error);
  broker.send({ resume: A });
  await sleep(100);
  const again = await req(A, "open", { url: pages.base + "next.html" });
  check("resume → open works again", again.ok, again.error);
  broker.send({ stop: A });
  await sleep(100);
  const st2 = await act(A, again.result.target, "read", {});
  check("stop from bise → stopped, no event", !st2.ok && st2.error.code === "stopped" && broker.events.filter((e) => e.event === "stopped" && e.agent === A).length === 1, st2.error);
  const userTouchedTabs = (await req(B, "tabs", {})).result.length;
  broker.send({ drop: B });
  broker.send({ drop: A });
  await sleep(500);
  const left = (await win()).flatMap((w) => w.tabs);
  check("drop closes untouched tabs, ungroups touched ones", left.some((t) => t.id === id3 && t.groupId === -1) && !left.some((t) => t.id === Number(again.result.target.slice(4))) && userTouchedTabs === 1, left);
  check("no stopped event for bise's own drop", !broker.events.some((e) => e.event === "stopped" && e.agent === B));

  // ---- the port comes back after the host dies (a broker restart)
  const hellosBefore = broker.hellos.length;
  broker.drop();
  check("the extension reconnects and says hello again", await broker.until(() => broker.hellos.length > hellosBefore && broker.connected(), 10_000), broker.hellos.length);
  const after = await req("late", "tabs", {});
  check("requests work after the reconnect", after.ok && Array.isArray(after.result), after.error);
  console.log("sw log:", JSON.stringify(await ext.ev("bise.log")));
} catch (e) {
  check("no crash", false, String(e.stack || e));
} finally {
  writeFileSync(path.join(process.env.TMPDIR || "/tmp", `cu-e2e-${browser}.json`), JSON.stringify(results, null, 1));
  await b.quit();
  pages.close();
  broker.close();
  const failed = results.filter((r) => !r.ok);
  console.log(`\n${results.length - failed.length}/${results.length} checks passed on ${browser}`);
  process.exit(failed.length ? 1 : 0);
}
