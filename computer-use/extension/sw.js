// bise computer use: the service worker.
//
// It talks to bise over native messaging (contract C4 of
// docs/computer-use-briefs.md): bise asks for open / tabs / snapshot /
// screenshot / act on behalf of an agent; each agent works in its own tab
// group "bise · <agent>", in background tabs, through chrome.debugger (CDP).
// The user stays in control: the debugging bar's Cancel or closing the group
// stops the agent; touching one of its tabs pauses it.

import { walk, render, pick, diff, describeLocator, TEXT_ROLES } from "./lib/ax.js";
import { parseKeys, chordEvents, keyLabel } from "./lib/keys.js";
import { summary, failure, withPlace, hostOf, refusal, label } from "./lib/text.js";
import { jpegSize } from "./lib/jpeg.js";

const HOST = "dev.bise.computer_use";
const MAX_TABS = 5;
const GROUP_PREFIX = "bise · ";
const CODES = new Set(["not_set_up", "no_browser", "no_helper", "no_permission", "not_found", "ambiguous", "stale_ref", "stopped", "paused", "refused", "timeout", "needs_front", "bad_args"]);
const ACTIONS = new Set(["click", "fill", "type", "press", "select", "check", "hover", "scroll", "goto", "close", "wait", "read"]);

class CuError extends Error {
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    Object.assign(this, extra);
  }
}
const fail = (code, message, extra) => { throw new CuError(code, message, extra); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- state

/** agent name → { name, groupId, stopped, closing, closingGroup } */
const agents = new Map();
/** tab id → { agent, attached, paused, userTouched, refs: backendId→n, byRef: n→backendId, nextRef, navs, acting, actingUntil, queue, overlayNavs } */
const tabs = new Map();
/** What happened, for the tests and for debugging (last 200). */
const log = [];
const note = (x) => {
  log.push({ ...x, at: Date.now() });
  if (log.length > 200) log.shift();
};

function agentOf(name) {
  let a = agents.get(name);
  if (!a) agents.set(name, (a = { name, groupId: null, stopped: false, closing: false, closingGroup: null }));
  return a;
}

function register(tabId, agent) {
  const t = { agent, attached: false, paused: false, userTouched: false, refs: new Map(), byRef: new Map(), nextRef: 1, navs: 0, acting: false, actingUntil: 0, queue: Promise.resolve(), overlayNavs: -1 };
  tabs.set(tabId, t);
  return t;
}

const tabsOf = (agent) => [...tabs.entries()].filter(([, t]) => t.agent === agent).map(([id]) => id);

// After a service worker restart, the groups say who owns what.
async function recover() {
  try {
    for (const g of await chrome.tabGroups.query({})) {
      if (!g.title?.startsWith(GROUP_PREFIX)) continue;
      const a = agentOf(g.title.slice(GROUP_PREFIX.length));
      a.groupId = g.id;
      for (const tab of await chrome.tabs.query({ groupId: g.id })) if (!tabs.has(tab.id)) register(tab.id, a.name);
    }
  } catch {
    // no window yet
  }
}
const ready = recover();

// ---------------------------------------------------------------- native port

let port = null;
let retry = 500;
let timer = null;

function connect() {
  timer = null;
  let p;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch {
    return later();
  }
  port = p;
  p.onMessage.addListener((m) => {
    retry = 500;
    onHost(m);
  });
  p.onDisconnect.addListener(() => {
    void chrome.runtime.lastError;
    if (port === p) port = null;
    later();
  });
  hello(p);
}

function later() {
  if (timer) return;
  timer = setTimeout(connect, retry);
  retry = Math.min(retry * 2, 30_000);
}

function post(msg) {
  try {
    port?.postMessage(msg);
  } catch {
    // the port died; connect() comes back
  }
}

async function hello(p) {
  let list = navigator.userAgentData?.brands || [];
  try {
    const hi = await navigator.userAgentData.getHighEntropyValues(["fullVersionList"]);
    if (hi.fullVersionList?.length) list = hi.fullVersionList;
  } catch {
    // keep the short versions
  }
  const brand = (s) => list.find((b) => b.brand === s);
  let browser = "chrome";
  let b = brand("Google Chrome");
  if (brand("Microsoft Edge")) [browser, b] = ["edge", brand("Microsoft Edge")];
  else if (brand("Opera")) [browser, b] = ["opera", brand("Opera")];
  else if (brand("Brave") || navigator.brave) [browser, b] = ["brave", brand("Brave")];
  const version = (b || brand("Chromium"))?.version || "";
  try {
    p.postMessage({ hello: { browser, version, extension_version: chrome.runtime.getManifest().version } });
  } catch {
    // disconnected meanwhile
  }
}

async function onHost(m) {
  await ready;
  if (m.stop) return stopAgent(m.stop, null);
  if (m.resume) return resumeAgent(m.resume);
  if (m.release) return releaseAgent(m.release);
  if (m.drop) return dropAgent(m.drop);
  if (m.id === undefined || !m.op) return;
  try {
    const result = await run(m.agent, m.op, m.args || {});
    post({ id: m.id, ok: true, result });
  } catch (e) {
    post({ id: m.id, ok: false, error: toError(e) });
  }
}

function toError(e) {
  if (e && CODES.has(e.code)) {
    const out = { code: e.code, message: e.message };
    if (e.candidates?.length) out.candidates = e.candidates;
    if (e.summary) out.summary = e.summary;
    return out;
  }
  const msg = String(e?.message || e);
  if (/No node|Could not find node|does not belong to the document/i.test(msg)) return { code: "stale_ref", message: "that element is gone from the page; take a new snapshot" };
  if (/No tab with id|tab was closed|Detached while handling|target closed/i.test(msg)) return { code: "not_found", message: "the tab is gone; computer.tabs() lists yours" };
  return { code: "refused", message: `chrome refused: ${msg}` };
}

// ---------------------------------------------------------------- ops

async function run(agent, op, args) {
  if (!agent || typeof agent !== "string") fail("bad_args", "no agent name in the request");
  if (op === "tabs") return listTabs(agent);
  if (agents.get(agent)?.stopped) fail("stopped", "the user stopped you in the browser; ask before you start again", { summary: "you stopped it" });
  switch (op) {
    case "open": return open(agent, args);
    case "snapshot": return snapshotOp(agent, args);
    case "screenshot": return screenshotOp(agent, args);
    case "act": return act(agent, args);
    default: fail("bad_args", `unknown op "${op}"`);
  }
}

async function listTabs(agent) {
  const out = [];
  for (const id of tabsOf(agent)) {
    try {
      const tab = await chrome.tabs.get(id);
      out.push({ target: `tab:${id}`, url: tab.url, title: tab.title, user_touched: tabs.get(id).userTouched });
    } catch {
      tabs.delete(id);
    }
  }
  return out;
}

async function windowFor(a) {
  if (a.groupId !== null) {
    try {
      return (await chrome.tabGroups.get(a.groupId)).windowId;
    } catch {
      a.groupId = null;
    }
  }
  try {
    return (await chrome.windows.getLastFocused({ windowTypes: ["normal"] })).id;
  } catch {
    return null;
  }
}

async function groupTab(a, tab) {
  if (a.groupId !== null) {
    try {
      await chrome.tabs.group({ tabIds: [tab.id], groupId: a.groupId });
      return;
    } catch {
      a.groupId = null;
    }
  }
  a.groupId = await chrome.tabs.group({ tabIds: [tab.id], createProperties: { windowId: tab.windowId } });
  await chrome.tabGroups.update(a.groupId, { title: GROUP_PREFIX + a.name, color: "pink", collapsed: false });
}

function normalUrl(raw) {
  const url = String(raw ?? "").trim();
  if (!url) fail("bad_args", "a url is needed");
  return /^[a-z][a-z0-9+.-]*:/i.test(url) ? url : "https://" + url;
}

async function open(agent, args) {
  const url = normalUrl(args.url);
  const why = refusal(url);
  if (why) fail("refused", `${why}; ask the user to do this part`, { summary: failure("open", hostOf(url), "refused") });
  const a = agentOf(agent);
  if (tabsOf(agent).length >= MAX_TABS) fail("refused", "you already have 5 tabs open; close one (act close) first");
  const windowId = await windowFor(a);
  let tab;
  if (windowId === null) {
    const w = await chrome.windows.create({ url, focused: false });
    tab = w.tabs[0];
  } else {
    let index;
    if (a.groupId !== null) {
      const mine = await chrome.tabs.query({ groupId: a.groupId });
      if (mine.length) index = Math.max(...mine.map((t) => t.index)) + 1;
    }
    tab = await chrome.tabs.create({ url, active: false, windowId, ...(index !== undefined ? { index } : {}) });
  }
  register(tab.id, agent);
  await groupTab(a, tab);
  await waitLoaded(tab.id, Math.max(args.timeout_ms ?? 0, 15_000));
  const now = await chrome.tabs.get(tab.id);
  ensureOverlay(tab.id);
  return { target: `tab:${tab.id}`, url: now.url, title: now.title, summary: summary("open", null, {}, hostOf(now.url)) };
}

async function waitLoaded(tabId, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const tab = await chrome.tabs.get(tabId);
    if (tab.status === "complete") return true;
    await sleep(100);
  }
  return false;
}

/** The agent's own tab, alive: { tabId, t }. */
async function owned(agent, target) {
  const m = /^tab:(\d+)$/.exec(String(target ?? ""));
  if (!m) fail("bad_args", `target must be "tab:<id>" here, got ${JSON.stringify(target ?? null)}`);
  const tabId = Number(m[1]);
  const t = tabs.get(tabId);
  if (!t || t.agent !== agent) fail("not_found", `${target} is not one of your tabs; computer.tabs() lists them`);
  try {
    await chrome.tabs.get(tabId);
  } catch {
    tabs.delete(tabId);
    fail("not_found", `${target} is closed; computer.tabs() lists your tabs`);
  }
  return { tabId, t };
}

/** One action at a time per tab. */
function serial(t, fn) {
  const run = t.queue.then(fn, fn);
  t.queue = run.catch(() => {});
  return run;
}

// ---------------------------------------------------------------- CDP

const cdp = (tabId, method, params = {}) => chrome.debugger.sendCommand({ tabId }, method, params);

async function attach(tabId, t) {
  if (t.attached) return;
  const tab = await chrome.tabs.get(tabId);
  const why = refusal(tab.url || tab.pendingUrl || "");
  if (why) fail("refused", `${why}; ask the user to do this part`);
  try {
    await chrome.debugger.attach({ tabId }, "1.3");
  } catch (e) {
    if (!/already attached/i.test(e.message)) fail("refused", `chrome won't let bise drive this tab: ${e.message}`);
  }
  t.attached = true;
  // Without it the first mouse event in a hidden tab waits 5 s for a frame (spike, design §3).
  await cdp(tabId, "Emulation.setFocusEmulationEnabled", { enabled: true });
  await cdp(tabId, "Page.enable");
  await cdp(tabId, "DOM.enable");
  await cdp(tabId, "Accessibility.enable");
}

async function detach(tabId, t) {
  if (!t.attached) return;
  t.attached = false;
  try {
    await chrome.debugger.detach({ tabId });
  } catch {
    // already gone
  }
}

chrome.debugger.onEvent.addListener((src, method, params) => {
  const t = tabs.get(src.tabId);
  if (!t) return;
  if (method === "Page.frameNavigated" && !params.frame.parentId) {
    // A new document: the old refs are stale (their numbers are never reused).
    t.navs++;
    t.refs.clear();
    t.byRef.clear();
  }
});

chrome.debugger.onDetach.addListener((src, reason) => {
  const t = tabs.get(src.tabId);
  if (!t) return;
  t.attached = false;
  if (reason === "canceled_by_user") stopAgent(t.agent, "cancel_bar");
});

async function evaluate(tabId, expression) {
  const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
  return r.result?.value;
}

async function callOn(tabId, backendNodeId, fn, args = []) {
  const { object } = await cdp(tabId, "DOM.resolveNode", { backendNodeId, objectGroup: "bise" });
  try {
    const r = await cdp(tabId, "Runtime.callFunctionOn", { objectId: object.objectId, functionDeclaration: fn.toString(), arguments: args, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result?.value;
  } finally {
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise" }).catch(() => {});
  }
}

async function passwordIds(tabId) {
  const ids = new Set();
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression: "Array.from(document.querySelectorAll('input[type=password]'))", objectGroup: "bise-pw" });
    if (!r.result?.objectId) return ids;
    const { result } = await cdp(tabId, "Runtime.getProperties", { objectId: r.result.objectId, ownProperties: true });
    for (const p of result) {
      if (p.value?.subtype !== "node") continue;
      const { node } = await cdp(tabId, "DOM.describeNode", { objectId: p.value.objectId });
      ids.add(node.backendNodeId);
    }
  } catch {
    // no document yet
  } finally {
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise-pw" }).catch(() => {});
  }
  return ids;
}

function refFor(t, backendId) {
  let n = t.refs.get(backendId);
  if (!n) {
    n = t.nextRef++;
    t.refs.set(backendId, n);
    t.byRef.set(n, backendId);
  }
  return n;
}

/** The page now: entries (all nodes), the C2 text cut at maxNodes, lines (all). */
async function takeSnapshot(tabId, t, maxNodes = Infinity) {
  const navs = t.navs;
  const { nodes } = await cdp(tabId, "Accessibility.getFullAXTree");
  // Masking needs the DOM's input types: ask only when a field holds a value.
  const valued = nodes.some((n) => !n.ignored && n.role?.value === "textbox" && n.value?.value);
  const passwords = valued ? await passwordIds(tabId) : new Set();
  const entries = walk(nodes, { refFor: (b) => refFor(t, b), passwords });
  const tab = await chrome.tabs.get(tabId);
  const host = hostOf(tab.url);
  const shown = render(entries, { title: tab.title, host, maxNodes });
  const all = maxNodes === Infinity ? shown : render(entries, { title: tab.title, host, maxNodes: Infinity });
  return { entries, text: shown.text, refs: shown.refs, truncated: shown.truncated, lines: all.lines, head: all.text.split("\n")[0], url: tab.url, title: tab.title, host, navs };
}

async function snapshotOp(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  return serial(t, async () => {
    await attach(tabId, t);
    const max = Number.isFinite(args.max_nodes) && args.max_nodes > 0 ? Math.floor(args.max_nodes) : 400;
    const s = await takeSnapshot(tabId, t, max);
    return { target: args.target, url: s.url, title: s.title, text: s.text, refs: s.refs, truncated: s.truncated };
  });
}

// ---------------------------------------------------------------- finding an element

const quadArea = (q) => Math.abs((q[2] - q[0]) * (q[5] - q[1]) - (q[4] - q[0]) * (q[3] - q[1]));

/** Where to click the element: { ok, x, y, box } or { ok: false, reason }. */
async function pointOf(tabId, backendNodeId) {
  try {
    await cdp(tabId, "DOM.scrollIntoViewIfNeeded", { backendNodeId });
  } catch {
    // text nodes and some svg: getContentQuads still works
  }
  let quads;
  try {
    ({ quads } = await cdp(tabId, "DOM.getContentQuads", { backendNodeId }));
  } catch {
    return { ok: false, reason: "it isn't visible" };
  }
  const q = quads.find((x) => quadArea(x) > 1);
  if (!q) return { ok: false, reason: "it isn't visible" };
  const xs = [q[0], q[2], q[4], q[6]], ys = [q[1], q[3], q[5], q[7]];
  const box = { x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
  const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
  const left = Math.max(box.x, 0), top = Math.max(box.y, 0);
  const right = Math.min(box.x + box.width, vv.clientWidth), bottom = Math.min(box.y + box.height, vv.clientHeight);
  if (right - left < 1 || bottom - top < 1) return { ok: false, reason: "it is off screen" };
  const x = Math.round((left + right) / 2), y = Math.round((top + bottom) / 2);
  // Does a click there land on it? (a cookie banner, a modal…)
  try {
    await cdp(tabId, "DOM.getDocument", { depth: 0 });
    const hit = await cdp(tabId, "DOM.getNodeForLocation", { x, y, includeUserAgentShadowDOM: true, ignorePointerEventsNone: false });
    if (hit.backendNodeId !== backendNodeId) {
      const { object } = await cdp(tabId, "DOM.resolveNode", { backendNodeId: hit.backendNodeId, objectGroup: "bise" });
      const inside = await callOn(tabId, backendNodeId, function (o) {
        const t = this.nodeType === 1 ? this : this.parentElement;
        const up = (a, b) => { for (let n = a; n; n = n.parentNode || n.host) if (n === b) return true; return false; };
        return !!t && (up(o, t) || up(t, o));
      }, [{ objectId: object.objectId }]);
      if (inside === false) return { ok: false, reason: "something covers it", covered: true };
    }
  } catch (e) {
    // hit test unavailable: click anyway
    note({ hitTest: String(e.message || e) });
  }
  return { ok: true, x, y, box };
}

function targetLabel(args, el) {
  if (el) return el.name ? `"${label(el.name)}"` : "";
  if (args.locator) {
    const l = args.locator;
    const w = l.name ?? l.text ?? l.label ?? l.name_re ?? l.text_re;
    return w !== undefined ? `"${label(w)}"` : l.role || "";
  }
  return args.ref ? `${args.ref}` : "";
}

/**
 * Wait (≤ deadline) until the ref or locator is one element, visible
 * and enabled when asked. Returns { entry, snap, point? }.
 */
async function resolve(tabId, t, args, deadline, { visible = true, enabled = false } = {}) {
  const loc = args.locator;
  if (args.ref && loc) fail("bad_args", "give a ref or a locator, not both");
  if (loc && typeof loc !== "object") fail("bad_args", "locator must be an object like {role, name}");
  const desc = args.ref ? args.ref : describeLocator(loc);
  for (;;) {
    const snap = await takeSnapshot(tabId, t);
    let entry = null, problem;
    if (args.ref) {
      const n = Number(/^e?(\d+)$/.exec(String(args.ref))?.[1]);
      if (!n) fail("bad_args", `bad ref ${JSON.stringify(args.ref)}; refs look like "e12"`);
      const backendId = t.byRef.get(n);
      if (!backendId) {
        if (n < t.nextRef) fail("stale_ref", `${args.ref} is from before the page changed; take a new snapshot`);
        fail("bad_args", `there is no ${args.ref} on this page; take a snapshot first`);
      }
      entry = snap.entries.find((e) => e.backendId === backendId) || null;
      if (!entry) {
        try {
          await cdp(tabId, "DOM.resolveNode", { backendNodeId: backendId });
        } catch {
          fail("stale_ref", `${args.ref} is gone from the page; take a new snapshot`);
        }
        problem = { code: "timeout", reason: "it isn't visible" };
      }
    } else {
      const p = pick(snap.entries, loc || {});
      if (p.entry) entry = p.entry;
      else problem = p;
    }
    if (entry) {
      if (enabled && entry.disabled) problem = { code: "timeout", reason: "it's disabled" };
      else if (!visible) return { entry, snap };
      else {
        const pt = await pointOf(tabId, entry.backendId);
        if (pt.ok) return { entry, snap, point: pt };
        problem = { code: "timeout", reason: pt.reason };
      }
    }
    if (Date.now() >= deadline) {
      const ms = args.timeout_ms ?? 5000;
      if (problem.code === "not_found") fail("not_found", `nothing matches ${desc} (waited ${ms} ms); the closest elements are in candidates`, { candidates: problem.candidates });
      if (problem.code === "ambiguous") fail("ambiguous", `${problem.count} elements match ${desc}; add nth or a more exact name`, { candidates: problem.candidates, reason: "several match" });
      const extra = problem.reason === "something covers it" ? " (a popup or banner?); close it first" : "";
      fail("timeout", `${desc}: ${problem.reason} after ${ms} ms${extra}`, { reason: problem.reason, candidates: entry ? [entry.line] : undefined });
    }
    await sleep(120);
  }
}

// ---------------------------------------------------------------- the overlay (cursor, takeover)

async function ensureOverlay(tabId) {
  const t = tabs.get(tabId);
  if (!t || t.overlayNavs === t.navs) return;
  try {
    await chrome.scripting.executeScript({ target: { tabId }, files: ["overlay.js"] });
    t.overlayNavs = t.navs;
  } catch {
    // a page scripts can't run on
  }
}

async function cursor(tabId, agent, point, ring) {
  try {
    await ensureOverlay(tabId);
    const tab = await chrome.tabs.get(tabId);
    const shown = chrome.tabs.sendMessage(tabId, { bise: "cursor", x: point.x, y: point.y, agent, ring });
    // Let the user see the glide before the click, only when the tab is in view.
    if (tab.active) await Promise.race([shown, sleep(400)]);
    else shown.catch(() => {});
  } catch {
    // purely visual
  }
}

chrome.runtime.onMessage.addListener((msg, sender) => {
  if (msg?.bise !== "input" || !sender.tab) return;
  const t = tabs.get(sender.tab.id);
  // CDP input is trusted too: ignore what arrives while (or right after) we act.
  // Only a tab in view can get the user's input (a hidden agent tab can't).
  if (!t || !sender.tab.active || t.acting || Date.now() < t.actingUntil) return;
  pause(sender.tab.id, t, msg.kind);
});

function pause(tabId, t, why) {
  note({ pause: tabId, why });
  t.userTouched = true;
  if (t.paused) return;
  t.paused = true;
  post({ event: "paused", agent: t.agent, target: `tab:${tabId}` });
}

// ---------------------------------------------------------------- act

async function mouse(tabId, x, y, kind) {
  const t0 = Date.now();
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  if (kind === "click") {
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
  }
  // The hidden-tab trap (design §3) shows here as ~5000 ms.
  note({ input: kind, ms: Date.now() - t0 });
}

async function keys(tabId, chords) {
  for (const c of chords) for (const ev of chordEvents(c)) await cdp(tabId, "Input.dispatchKeyEvent", ev);
}

function focusFn(selectAll) {
  return `function () {
    const el = this.nodeType === 1 ? this : this.parentElement;
    el.focus();
    if (${selectAll}) {
      if (typeof el.select === "function") { try { el.select(); return; } catch (e) {} }
      if (el.isContentEditable) { const r = document.createRange(); r.selectNodeContents(el); const s = getSelection(); s.removeAllRanges(); s.addRange(r); }
    } else if (typeof el.setSelectionRange === "function") {
      try { const n = el.value.length; el.setSelectionRange(n, n); } catch (e) {}
    }
  }`;
}

function isPasswordFn() {
  return this.tagName === "INPUT" && this.type === "password";
}

/** Passwords are the user's (ship plan §6): never typed by an agent, on any site. */
function refusePassword() {
  fail("refused", "the user types passwords himself; ask him to sign in, then go on", { reason: "he types passwords himself" });
}

function readFn() {
  if (this.nodeType === 3) return this.textContent;
  const el = this;
  if (el.tagName === "INPUT" && el.type === "password") return "•••";
  if (el.tagName === "SELECT") return el.selectedOptions[0]?.label ?? "";
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return el.value;
  return el.innerText ?? el.textContent ?? "";
}

function selectFn(v) {
  if (this.tagName !== "SELECT") return { error: "not_select" };
  const opts = [...this.options];
  const want = String(v);
  const o = opts.find((x) => x.value === want) || opts.find((x) => x.label.trim() === want) || opts.find((x) => x.label.trim().toLowerCase().includes(want.toLowerCase()));
  if (!o) return { error: "no_option", options: opts.slice(0, 10).map((x) => x.label.trim()) };
  this.value = o.value;
  this.dispatchEvent(new Event("input", { bubbles: true }));
  this.dispatchEvent(new Event("change", { bubbles: true }));
  return { label: o.label.trim() };
}

function scrollFn(dx, dy) {
  let n = this && this.nodeType ? (this.nodeType === 1 ? this : this.parentElement) : null;
  const scrollable = (e) => {
    const s = getComputedStyle(e);
    return (e.scrollHeight > e.clientHeight + 1 && /(auto|scroll|overlay)/.test(s.overflowY)) || (e.scrollWidth > e.clientWidth + 1 && /(auto|scroll|overlay)/.test(s.overflowX));
  };
  while (n && n !== document.body && n !== document.documentElement && !scrollable(n)) n = n.parentElement;
  const box = n && n !== document.body && n !== document.documentElement ? n : document.scrollingElement;
  const before = [box.scrollLeft, box.scrollTop].join();
  box.scrollBy(dx, dy);
  return before !== [box.scrollLeft, box.scrollTop].join();
}

function viewSize() {
  let n = this && this.nodeType ? (this.nodeType === 1 ? this : this.parentElement) : null;
  while (n && n !== document.body && n !== document.documentElement && !(n.scrollHeight > n.clientHeight + 1 && /(auto|scroll|overlay)/.test(getComputedStyle(n).overflowY))) n = n.parentElement;
  const box = n && n !== document.body && n !== document.documentElement ? n : null;
  const sig = () => [scrollX, scrollY, box ? box.scrollLeft : 0, box ? box.scrollTop : 0].join();
  return { w: box ? box.clientWidth : innerWidth, h: box ? box.clientHeight : innerHeight, sig: sig() };
}

/** Wait for the page to settle after an action, then snapshot it. */
async function settle(tabId, t, deadline) {
  await sleep(60);
  while (Date.now() < deadline + 10_000) {
    const tab = await chrome.tabs.get(tabId);
    if (tab.status !== "loading") break;
    await sleep(100);
  }
  let prev = await takeSnapshot(tabId, t);
  for (let i = 0; i < 8; i++) {
    await sleep(100);
    const next = await takeSnapshot(tabId, t);
    if (next.navs === prev.navs && next.lines.join("\n") === prev.lines.join("\n")) return next;
    prev = next;
  }
  return prev;
}

function changedText(before, after) {
  if (after.navs !== before.navs || after.url !== before.url && hostOf(after.url) !== hostOf(before.url)) {
    const lines = [after.head, ...after.lines.slice(0, 19).map((l) => l)];
    if (after.lines.length > 19) lines[19] = `… ${after.lines.length - 18} more lines (snapshot for all)`;
    return lines.slice(0, 20).join("\n");
  }
  return diff(before.lines, after.lines);
}

async function act(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  const action = args.action;
  if (!ACTIONS.has(action)) fail("bad_args", `unknown action ${JSON.stringify(action)}; one of ${[...ACTIONS].join(", ")}`);
  const place = async () => hostOf((await chrome.tabs.get(tabId).catch(() => ({}))).url || "");
  if (t.paused) fail("paused", "the user is using this tab; wait until they give it back, or ask them", { summary: withPlace(failure(action, targetLabel(args), "paused"), await place()) });
  return serial(t, async () => {
    let el = null;
    try {
      t.acting = true;
      await attach(tabId, t);
      const out = await doAct(tabId, t, agent, action, args, (e) => (el = e));
      return out;
    } catch (e) {
      if (e instanceof CuError && !e.summary) e.summary = failure(action, targetLabel(args, el), e.code, e.reason);
      if (!(e instanceof CuError)) {
        const err = toError(e);
        throw new CuError(err.code, err.message, { summary: withPlace(failure(action, targetLabel(args, el), err.code), await place()) });
      }
      if (e.summary) e.summary = withPlace(e.summary, await place());
      throw e;
    } finally {
      t.acting = false;
      t.actingUntil = Date.now() + 600;
    }
  });
}

async function doAct(tabId, t, agent, action, args, seen) {
  const started = Date.now();
  const timeout = Math.min(Math.max(Number.isFinite(args.timeout_ms) ? args.timeout_ms : 5000, 0), 120_000);
  const deadline = Date.now() + timeout;
  const hasTarget = !!(args.ref || args.locator);
  const need = (opts) => resolve(tabId, t, args, deadline, opts).then((r) => (seen(r.entry), r));
  let el = null, before = null, extra = "";

  switch (action) {
    case "click": {
      if (!hasTarget) fail("bad_args", "click needs a ref or a locator");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      await mouse(tabId, r.point.x, r.point.y, "click");
      break;
    }
    case "hover": {
      if (!hasTarget) fail("bad_args", "hover needs a ref or a locator");
      const r = await need({ visible: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, false);
      await mouse(tabId, r.point.x, r.point.y, "move");
      break;
    }
    case "fill":
    case "type": {
      const text = args.text ?? args.value;
      if (typeof text !== "string") fail("bad_args", `${action} needs text`);
      if (hasTarget) {
        const r = await need({ visible: true, enabled: true });
        ({ entry: el, snap: before } = r);
        if (!TEXT_ROLES.has(el.role) && !el.editable) fail("bad_args", `${el.line} is not a text field`, { reason: "it isn't a text field" });
        if (await callOn(tabId, el.backendId, isPasswordFn)) refusePassword();
        await cursor(tabId, agent, r.point, true);
        await callOn(tabId, el.backendId, focusFn(action === "fill"));
      } else if (action === "fill") {
        fail("bad_args", "fill needs a ref or a locator");
      } else {
        before = await takeSnapshot(tabId, t);
        const focused = await cdp(tabId, "Runtime.evaluate", { expression: "document.activeElement?.tagName === 'INPUT' && document.activeElement.type === 'password'", returnByValue: true });
        if (focused.result?.value === true) refusePassword();
      }
      if (text === "" && action === "fill") await keys(tabId, parseKeys("Delete"));
      else if (text !== "") await cdp(tabId, "Input.insertText", { text });
      break;
    }
    case "press": {
      const chords = parseKeys(args.keys ?? args.text);
      if (hasTarget) {
        const r = await need({ visible: true });
        ({ entry: el, snap: before } = r);
        await callOn(tabId, el.backendId, focusFn(false));
      } else {
        before = await takeSnapshot(tabId, t);
      }
      await keys(tabId, chords);
      extra = keyLabel(args.keys ?? args.text);
      break;
    }
    case "select": {
      if (!hasTarget) fail("bad_args", "select needs a ref or a locator");
      const value = args.value ?? args.text;
      if (value === undefined) fail("bad_args", "select needs a value (the option's value or label)");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      const res = await callOn(tabId, el.backendId, selectFn, [{ value }]);
      if (res?.error === "not_select") fail("bad_args", `${el.line} is not a <select>; click it, then click the option`, { reason: "it isn't a list" });
      if (res?.error === "no_option") fail("not_found", `no option "${value}" in ${el.line}`, { candidates: res.options });
      break;
    }
    case "check": {
      if (!hasTarget) fail("bad_args", "check needs a ref or a locator");
      const want = args.value !== false;
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      if (el.checked !== want) {
        await cursor(tabId, agent, r.point, true);
        await mouse(tabId, r.point.x, r.point.y, "click");
        await sleep(50);
        const now = (await takeSnapshot(tabId, t)).entries.find((e) => e.backendId === el.backendId);
        if (now && now.checked !== want) fail("timeout", `clicking ${el.line} didn't ${want ? "check" : "uncheck"} it`, { reason: "it didn't change" });
      }
      break;
    }
    case "scroll": {
      let node = null, point = null;
      if (hasTarget) {
        const r = await need({ visible: !args.direction ? false : true });
        ({ entry: el, snap: before } = r);
        node = el.backendId;
        point = r.point;
      } else {
        before = await takeSnapshot(tabId, t);
      }
      if (node && !args.direction && args.amount === undefined) {
        await callOn(tabId, node, function () { (this.nodeType === 1 ? this : this.parentElement).scrollIntoView({ block: "center", inline: "nearest" }); });
        break;
      }
      const dir = args.direction ?? "down";
      if (!["up", "down", "left", "right"].includes(dir)) fail("bad_args", `direction is up, down, left or right, not ${JSON.stringify(dir)}`);
      const size = node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`);
      const amount = Number.isFinite(args.amount) ? args.amount : Math.round(0.8 * (dir === "left" || dir === "right" ? size.w : size.h));
      const [dx, dy] = { up: [0, -amount], down: [0, amount], left: [-amount, 0], right: [amount, 0] }[dir];
      const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
      const x = point ? point.x : Math.round(vv.clientWidth / 2), y = point ? point.y : Math.round(vv.clientHeight / 2);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX: dx, deltaY: dy });
      await sleep(80);
      const after = node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`);
      if (after.sig === size.sig) {
        // The wheel didn't move it (some pages, hidden tabs): scroll from script.
        if (node) await callOn(tabId, node, scrollFn, [{ value: dx }, { value: dy }]);
        else await evaluate(tabId, `(${scrollFn}).call(null, ${dx}, ${dy})`);
      }
      break;
    }
    case "goto": {
      const url = normalUrl(args.url);
      const why = refusal(url);
      if (why) fail("refused", `${why}; ask the user to do this part`, { summary: failure("goto", hostOf(url), "refused") });
      before = await takeSnapshot(tabId, t);
      const r = await cdp(tabId, "Page.navigate", { url });
      if (r.errorText) fail("bad_args", `couldn't load ${url}: ${r.errorText}`, { summary: failure("goto", hostOf(url), "bad_args", r.errorText) });
      await sleep(50);
      await waitLoaded(tabId, Math.max(timeout, 15_000));
      break;
    }
    case "close": {
      const tab = await chrome.tabs.get(tabId);
      const a = agentOf(agent);
      // Its last tab: Chrome removes the group next. Remember which group
      // (not a timed flag: a close right after another agent-closed group
      // raced the timer and read as the user closing the group).
      if (tabsOf(agent).length === 1) a.closingGroup = a.groupId;
      await detach(tabId, t);
      tabs.delete(tabId);
      await chrome.tabs.remove(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("close", null, args, hostOf(tab.url)) };
    }
    case "wait": {
      if (hasTarget) {
        const r = await need({ visible: true });
        el = r.entry;
      } else if (args.text !== undefined) {
        const want = String(args.text).toLowerCase();
        for (;;) {
          const body = String((await evaluate(tabId, "document.body ? document.body.innerText : ''")) || "").toLowerCase();
          if (body.includes(want)) break;
          if (Date.now() >= deadline) fail("timeout", `the page doesn't show "${args.text}" after ${timeout} ms`, { reason: "it never showed up" });
          await sleep(150);
        }
      } else {
        await sleep(Math.min(Math.max(args.amount ?? 0, 0), timeout));
      }
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("wait", el, args, hostOf(tab.url)) };
    }
    case "read": {
      let text;
      if (hasTarget) {
        const r = await need({ visible: false });
        el = r.entry;
        text = await callOn(tabId, el.backendId, readFn);
      } else {
        text = await evaluate(tabId, "document.body ? document.body.innerText : ''");
      }
      text = String(text ?? "");
      if (text.length > 4000) text = text.slice(0, 4000) + "…";
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: text, summary: summary("read", el, args, hostOf(tab.url)) };
    }
  }

  const t1 = Date.now();
  const after = await settle(tabId, t, deadline);
  note({ act: action, actMs: t1 - started, settleMs: Date.now() - t1 });
  return {
    ok: true,
    url: after.url,
    title: after.title,
    changed: changedText(before, after),
    summary: summary(action, el, args, after.host, extra),
  };
}

// ---------------------------------------------------------------- screenshot

async function screenshotOp(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  return serial(t, async () => {
    await attach(tabId, t);
    const maxW = Math.min(Math.max(Number.isFinite(args.max_width) ? args.max_width : 1280, 64), 4096);
    const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
    const dpr = (await evaluate(tabId, "devicePixelRatio")) || 1;
    let clip = { x: vv.pageX, y: vv.pageY, width: vv.clientWidth, height: vv.clientHeight };
    if (args.ref || args.locator) {
      const r = await resolve(tabId, t, args, Date.now() + (args.timeout_ms ?? 5000), { visible: true });
      const b = r.point.box;
      const after = (await cdp(tabId, "Page.getLayoutMetrics")).cssVisualViewport;
      const x0 = Math.max(b.x, 0), y0 = Math.max(b.y, 0);
      const x1 = Math.min(b.x + b.width, after.clientWidth), y1 = Math.min(b.y + b.height, after.clientHeight);
      clip = { x: x0 + after.pageX, y: y0 + after.pageY, width: x1 - x0, height: y1 - y0 };
    }
    const scale = Math.min(1, maxW / (clip.width * dpr)) * 1;
    const shot = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: 80, clip: { ...clip, scale }, captureBeyondViewport: false });
    const { width, height } = jpegSize(shot.data);
    return { data: shot.data, mime: "image/jpeg", width, height };
  });
}

// ---------------------------------------------------------------- stop, resume, release, drop

async function stopAgent(name, reason) {
  const a = agentOf(name);
  const was = a.stopped;
  a.stopped = true;
  for (const id of tabsOf(name)) await detach(id, tabs.get(id));
  if (reason && !was) post({ event: "stopped", agent: name, reason });
}

function resumeAgent(name) {
  const a = agentOf(name);
  a.stopped = false;
  for (const id of tabsOf(name)) tabs.get(id).paused = false;
}

async function releaseAgent(name) {
  for (const id of tabsOf(name)) {
    const t = tabs.get(id);
    await detach(id, t);
    chrome.tabs.sendMessage(id, { bise: "cursor", hide: true }).catch(() => {});
  }
}

async function dropAgent(name) {
  await releaseAgent(name);
  const a = agentOf(name);
  const ids = tabsOf(name);
  const touched = ids.some((id) => tabs.get(id).userTouched);
  for (const id of ids) tabs.delete(id);
  a.closing = true;
  if (touched) {
    // The user used them: they become ordinary tabs.
    try {
      await chrome.tabs.ungroup(ids);
    } catch {
      // closed meanwhile
    }
  } else {
    try {
      await chrome.tabs.remove(ids);
    } catch {
      // closed meanwhile
    }
  }
  agents.delete(name);
}

// ---------------------------------------------------------------- the user's moves

chrome.tabs.onActivated.addListener(({ tabId }) => {
  const t = tabs.get(tabId);
  if (t) pause(tabId, t, "activated");
});

chrome.tabs.onRemoved.addListener((tabId) => {
  tabs.delete(tabId);
});

chrome.tabs.onUpdated.addListener((tabId, info) => {
  const t = tabs.get(tabId);
  if (!t) return;
  if (info.status === "complete") ensureOverlay(tabId);
  const a = agents.get(t.agent);
  if (info.groupId !== undefined && a && a.groupId !== null && info.groupId !== a.groupId) t.userTouched = true;
});

chrome.tabGroups.onRemoved.addListener((group) => {
  for (const a of agents.values()) {
    if (a.groupId !== group.id) continue;
    a.groupId = null;
    if (a.closingGroup === group.id) a.closingGroup = null;
    else if (!a.closing) stopAgent(a.name, "group_closed");
  }
});

chrome.runtime.onStartup.addListener(() => {});
chrome.runtime.onInstalled.addListener(() => {});

// For the tests (test/e2e.mjs reads it through CDP on this worker).
globalThis.bise = { agents, tabs, log, connected: () => !!port };

connect();
