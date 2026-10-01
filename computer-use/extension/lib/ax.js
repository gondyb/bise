// The page as text (contract C2) and the locators of C1, from the nodes of
// CDP Accessibility.getFullAXTree. Pure: no chrome.* here, so node tests
// (test/unit.test.mjs) run it on recorded trees.

// Chrome's internal roles → the ARIA names C2 uses on web and apps.
const ROLE_MAP = {
  StaticText: "text",
  image: "img",
  RootWebArea: "document",
  WebArea: "document",
  LabelText: "label",
  Canvas: "canvas",
  Iframe: "iframe",
  IframePresentational: "iframe",
  MenuListOption: "option",
  MenuListPopup: "listbox",
  DescriptionListTerm: "term",
  DescriptionListDetail: "definition",
  ListMarker: "listmarker",
  ScrollArea: "scrollarea",
};

// Nodes that never show: their children take their place.
const FLATTEN = new Set(["generic", "none", "presentation", "ignored", "GenericContainer", "Section", "MenuListPopup", "Ruby", "LayoutTable", "LayoutTableRow", "LayoutTableCell"]);
// Nodes dropped with their children.
const DROP = new Set(["InlineTextBox", "LineBreak", "ListMarker"]);

// Form controls: matched by `label`, never by `text`.
export const FORM_ROLES = new Set(["textbox", "searchbox", "combobox", "checkbox", "radio", "slider", "spinbutton", "switch", "listbox"]);
// Roles an action can type into.
export const TEXT_ROLES = new Set(["textbox", "searchbox", "combobox", "spinbutton"]);

export function ariaRole(raw) {
  if (!raw) return "generic";
  if (ROLE_MAP[raw]) return ROLE_MAP[raw];
  return raw.charAt(0).toLowerCase() + raw.slice(1);
}

const oneLine = (s) => String(s ?? "").replace(/\s+/g, " ").trim();
const cut = (s, n) => (s.length > n ? s.slice(0, n - 1) + "…" : s);
const quote = (s) => '"' + s.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';

function prop(node, name) {
  const p = (node.properties || []).find((x) => x.name === name);
  return p ? p.value?.value : undefined;
}

/**
 * Walk the AX nodes into entries, one per shown line.
 * @param nodes CDP AXNode[]
 * @param opts { refFor(backendId) → number, passwords: Set<backendId> }
 * @returns entries [{ depth, role, name, value, ref, backendId, disabled,
 *   focused, checked, expanded, parent (index or -1), line }]
 */
export function walk(nodes, opts = {}) {
  const byId = new Map(nodes.map((n) => [n.nodeId, n]));
  const root = nodes.find((n) => !n.parentId) || nodes[0];
  const refFor = opts.refFor || (() => 0);
  const passwords = opts.passwords || new Set();
  const entries = [];
  if (!root) return entries;
  const visit = (node, depth, parent, parentName) => {
    if (!node) return;
    const raw = node.role?.value;
    if (DROP.has(raw)) return;
    const kids = (node.childIds || []).map((id) => byId.get(id));
    const isRoot = node === root;
    const name = oneLine(node.name?.value);
    const role = ariaRole(raw);
    const flatten = isRoot || node.ignored || FLATTEN.has(raw) || FLATTEN.has(role) || (role === "paragraph" && !name && kids.length === 1);
    if (flatten) {
      for (const k of kids) visit(k, depth, parent, parentName);
      return;
    }
    // A text that repeats its parent's name (the label of a button) adds nothing.
    if (role === "text" && (!name || name === parentName)) return;
    const backendId = node.backendDOMNodeId;
    let value = node.value?.value;
    value = value === undefined || value === null ? "" : oneLine(value);
    if (passwords.has(backendId)) value = "•••";
    const e = {
      depth,
      role,
      name,
      value: value && value !== name ? value : "",
      ref: backendId ? refFor(backendId) : 0,
      backendId,
      disabled: prop(node, "disabled") === true,
      focused: prop(node, "focused") === true,
      checked: prop(node, "checked") === "true" || prop(node, "checked") === true || prop(node, "checked") === "mixed",
      expanded: prop(node, "expanded") === true,
      editable: !!prop(node, "editable"),
      parent,
    };
    if (passwords.has(backendId)) e.password = true;
    e.line = line(e);
    const index = entries.length;
    entries.push(e);
    for (const k of kids) visit(k, depth + 1, index, name);
  };
  visit(root, 0, -1, "");
  return entries;
}

/** One C2 line, without its indentation. */
export function line(e) {
  let s = `- ${e.role}`;
  if (e.name) s += " " + quote(cut(e.name, 100));
  if (e.ref) s += ` [e${e.ref}]`;
  if (e.value) s += ` value=${quote(cut(e.value, 80))}`;
  if (e.disabled) s += " (disabled)";
  if (e.focused) s += " (focused)";
  if (e.checked) s += " (checked)";
  if (e.expanded) s += " (expanded)";
  return s;
}

/**
 * The C2 text: `# <title> · <host>` then the lines, cut at maxNodes.
 * @returns { text, refs, truncated, lines (the shown lines, indented) }
 */
export function render(entries, { title = "", host = "", maxNodes = 400 } = {}) {
  const shown = entries.slice(0, Math.max(0, maxNodes));
  const lines = shown.map((e) => "  ".repeat(e.depth) + e.line);
  const head = `# ${oneLine(title) || "untitled"} · ${host || "page"}`;
  const truncated = entries.length > shown.length;
  const tail = truncated ? [`# … ${entries.length - shown.length} more nodes (raise max_nodes, or scroll and snapshot again)`] : [];
  return {
    text: [head, ...lines, ...tail].join("\n"),
    refs: shown.filter((e) => e.ref).length,
    truncated,
    lines,
  };
}

/** `"Anker.*2 m/i"` → RegExp (flags after the last `/`). Bad source → bad_args. */
export function parseRe(src) {
  const m = /^(.*)\/([dgimsuy]*)$/s.exec(src);
  const [body, flags] = m && m[1] !== "" ? [m[1], m[2]] : [src, ""];
  try {
    return new RegExp(body, flags.replace("g", ""));
  } catch (e) {
    throw Object.assign(new Error(`bad regex ${JSON.stringify(src)}: ${e.message}`), { code: "bad_args" });
  }
}

function textTest(plain, re, exact) {
  if (re !== undefined) {
    const r = parseRe(re);
    return (s) => r.test(s);
  }
  if (plain === undefined) return null;
  const want = oneLine(plain);
  if (exact) return (s) => s === want;
  const low = want.toLowerCase();
  return (s) => s.toLowerCase().includes(low);
}

/** A short human label of a locator, for errors and summaries. */
export function describeLocator(loc = {}) {
  const what = loc.name ?? loc.text ?? loc.label ?? (loc.name_re ? `/${loc.name_re}/` : loc.text_re ? `/${loc.text_re}/` : "");
  if (what && loc.role) return `${loc.role} "${what}"`;
  if (what) return `"${what}"`;
  return loc.role || "element";
}

/**
 * Entries that match a C1 locator, in document order (before nth).
 * Text matches keep the innermost element only, like Playwright's getByText.
 */
export function match(entries, loc = {}) {
  const role = loc.role ? String(loc.role).toLowerCase() : null;
  const name = textTest(loc.name, loc.name_re, loc.exact);
  const text = textTest(loc.text, loc.text_re, loc.exact);
  const label = textTest(loc.label, undefined, loc.exact);
  let out = [];
  entries.forEach((e, i) => {
    if (role && e.role !== role) return;
    if (name && !name(e.name)) return;
    if (label && !(FORM_ROLES.has(e.role) && label(e.name))) return;
    if (text && (FORM_ROLES.has(e.role) || !e.name || !text(e.name))) return;
    out.push(i);
  });
  if (text) {
    const set = new Set(out);
    const hasMatchedChild = new Set();
    for (const i of out) {
      for (let p = entries[i].parent; p >= 0; p = entries[p].parent) if (set.has(p)) hasMatchedChild.add(p);
    }
    out = out.filter((i) => !hasMatchedChild.has(i));
  }
  return out.map((i) => entries[i]);
}

/** Pick one entry: { entry } | { code: not_found|ambiguous, candidates }. */
export function pick(entries, loc = {}) {
  const found = match(entries, loc);
  const nth = loc.nth;
  if (found.length === 0) {
    const near = loc.role ? entries.filter((e) => e.role === String(loc.role).toLowerCase()) : entries.filter((e) => e.ref && e.role !== "text");
    return { code: "not_found", candidates: near.slice(0, 10).map((e) => e.line) };
  }
  if (nth !== undefined && nth !== null) {
    const i = nth < 0 ? found.length + nth : nth;
    if (i < 0 || i >= found.length) return { code: "not_found", candidates: found.slice(0, 10).map((e) => e.line) };
    return { entry: found[i], count: found.length };
  }
  if (found.length > 1) return { code: "ambiguous", candidates: found.slice(0, 10).map((e) => e.line), count: found.length };
  return { entry: found[0], count: 1 };
}

/**
 * The `changed` of an act: a line diff of two snapshots (lines without
 * indentation), `+ ` added, `- ` removed, at most `max` lines.
 */
export function diff(before, after, max = 20) {
  const a = before.map((l) => l.trim().replace(/^- /, ""));
  const b = after.map((l) => l.trim().replace(/^- /, ""));
  let s = 0;
  while (s < a.length && s < b.length && a[s] === b[s]) s++;
  let ea = a.length, eb = b.length;
  while (ea > s && eb > s && a[ea - 1] === b[eb - 1]) { ea--; eb--; }
  const x = a.slice(s, ea), y = b.slice(s, eb);
  const out = [];
  if (x.length * y.length > 4_000_000) {
    for (const l of x) out.push("- " + l);
    for (const l of y) out.push("+ " + l);
  } else {
    // LCS table, then walk it in order.
    const n = x.length, m = y.length;
    const t = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) t[i][j] = x[i] === y[j] ? t[i + 1][j + 1] + 1 : Math.max(t[i + 1][j], t[i][j + 1]);
    let i = 0, j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && x[i] === y[j]) { i++; j++; }
      else if (i < n && (j === m || t[i + 1][j] >= t[i][j + 1])) out.push("- " + x[i++]);
      else out.push("+ " + y[j++]);
    }
  }
  if (out.length <= max) return out.join("\n");
  return [...out.slice(0, max - 1), `… ${out.length - max + 1} more changes`].join("\n");
}
