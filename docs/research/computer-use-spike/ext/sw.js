// Spike service worker: driven from the test harness through CDP
// (Runtime.evaluate on this worker). In the product, the same calls come
// from the bise native host over native messaging.

const groups = new Map(); // agent name -> groupId
const events = [];
chrome.debugger.onDetach.addListener((src, reason) => events.push({ ev: "detach", tabId: src.tabId, reason }));
chrome.tabGroups.onRemoved.addListener((g) => events.push({ ev: "group_removed", id: g.id }));

async function cdp(tabId, method, params = {}) {
  return chrome.debugger.sendCommand({ tabId }, method, params);
}

// Open a tab for an agent in its own collapsed group, without activating it.
async function openTab(agent, url, windowId) {
  const tab = await chrome.tabs.create({ url, active: false, windowId });
  let groupId = groups.get(agent);
  if (groupId === undefined) {
    groupId = await chrome.tabs.group({ tabIds: [tab.id], createProperties: { windowId: tab.windowId } });
    await chrome.tabGroups.update(groupId, { title: `bise · ${agent}`, color: "purple", collapsed: true });
    groups.set(agent, groupId);
  } else {
    await chrome.tabs.group({ tabIds: [tab.id], groupId });
  }
  // wait for load
  for (let i = 0; i < 50; i++) {
    const t = await chrome.tabs.get(tab.id);
    if (t.status === "complete") break;
    await new Promise((r) => setTimeout(r, 100));
  }
  return { tabId: tab.id, groupId, windowId: tab.windowId };
}

async function snapshot() {
  const wins = await chrome.windows.getAll({ populate: true });
  return wins.map((w) => ({
    id: w.id, focused: w.focused, state: w.state,
    tabs: w.tabs.map((t) => ({ id: t.id, active: t.active, groupId: t.groupId, url: t.url.slice(0, 40) })),
  }));
}

async function groupInfo(groupId) {
  const g = await chrome.tabGroups.get(groupId);
  return { title: g.title, color: g.color, collapsed: g.collapsed };
}

// The CDP part: attach, read the AX tree, click a button by its AX node,
// type into a field, screenshot, all in the inactive tab.
async function act(tabId) {
  const out = {};
  const t0 = Date.now();
  await chrome.debugger.attach({ tabId }, "1.3");
  out.attachMs = Date.now() - t0;
  await cdp(tabId, "DOM.enable");
  await cdp(tabId, "Accessibility.enable");
  const ax = await cdp(tabId, "Accessibility.getFullAXTree");
  out.axNodes = ax.nodes.length;
  const button = ax.nodes.find((n) => n.role?.value === "button" && n.name?.value === "Buy");
  const field = ax.nodes.find((n) => n.role?.value === "textbox");
  out.found = { button: !!button, field: !!field };

  // click by AX node -> backend DOM node -> box model -> CDP mouse events
  const box = await cdp(tabId, "DOM.getBoxModel", { backendNodeId: button.backendDOMNodeId });
  const q = box.model.content;
  const x = (q[0] + q[4]) / 2, y = (q[1] + q[5]) / 2;
  const t1 = Date.now();
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    await cdp(tabId, "Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
  }
  out.clickMs = Date.now() - t1;

  // type: focus the field through DOM, then insertText
  await cdp(tabId, "DOM.focus", { backendNodeId: field.backendDOMNodeId });
  await cdp(tabId, "Input.insertText", { text: "hello from bise" });

  const r = await cdp(tabId, "Runtime.evaluate", {
    expression: "JSON.stringify({clicks: window.clicks, typed: document.querySelector('input').value, hidden: document.visibilityState, focus: document.hasFocus()})",
    returnByValue: true,
  });
  out.page = JSON.parse(r.result.value);

  const t2 = Date.now();
  try {
    const shot = await Promise.race([
      cdp(tabId, "Page.captureScreenshot", { format: "png" }),
      new Promise((_, rej) => setTimeout(() => rej(new Error("timeout 5s")), 5000)),
    ]);
    out.screenshot = { bytes: Math.round(shot.data.length * 0.75), ms: Date.now() - t2 };
    globalThis.lastShot = shot.data;
  } catch (e) {
    out.screenshot = { error: String(e), ms: Date.now() - t2 };
  }
  return out;
}

// Per-event timings of a CDP click in a hidden tab, with an optional
// workaround applied first.
async function clickTimings(tabId, mode) {
  const out = { mode };
  try { await chrome.debugger.attach({ tabId }, "1.3"); } catch {}
  if (mode === "lifecycle") await cdp(tabId, "Page.setWebLifecycleState", { state: "active" });
  if (mode === "focus") await cdp(tabId, "Emulation.setFocusEmulationEnabled", { enabled: true });
  if (mode === "screencast") {
    await cdp(tabId, "Page.enable");
    await cdp(tabId, "Page.startScreencast", { format: "jpeg", quality: 10, everyNthFrame: 1 });
  }
  await cdp(tabId, "Runtime.evaluate", { expression: "window.clicks=0" });
  const ax = await cdp(tabId, "Accessibility.getFullAXTree");
  const button = ax.nodes.find((n) => n.role?.value === "button" && n.name?.value === "Buy");
  const box = await cdp(tabId, "DOM.getBoxModel", { backendNodeId: button.backendDOMNodeId });
  const q = box.model.content;
  const x = (q[0] + q[4]) / 2, y = (q[1] + q[5]) / 2;
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    const t = Date.now();
    await cdp(tabId, "Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
    out[type] = Date.now() - t;
  }
  const t = Date.now();
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyDown", key: "a", text: "a" });
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyUp", key: "a" });
  out.key = Date.now() - t;
  const r = await cdp(tabId, "Runtime.evaluate", { expression: "window.clicks", returnByValue: true });
  out.clicks = r.result.value;
  if (mode === "screencast") await cdp(tabId, "Page.stopScreencast");
  return out;
}

async function detach(tabId) {
  await chrome.debugger.detach({ tabId });
}

globalThis.spike = { openTab, snapshot, groupInfo, act, clickTimings, detach, events };

async function native() {
  const t = Date.now();
  const r = await chrome.runtime.sendNativeMessage("dev.bise.spike", { hello: "bise" });
  return { ...r, ms: Date.now() - t };
}
globalThis.spike.native = native;
