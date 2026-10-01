# Computer use: design

Status: design + spike, not built. Task `computer-use`, 2026-10. Spike
code and one run's output: [research/computer-use-spike/](research/computer-use-spike/).
Designer's screens: signed off (m_3551, m_3554, §8); the build is
signed off from tmux captures.

## 1. Recommendation (short)

1. **Built into bise, not a plugin to install.** The user types
   `/computer-use` once; a checklist gets the permissions, the Chrome
   extension and a live test done. Agents get a `computer` object in code
   mode with a small Playwright-like API (`tab.getByRole(...).click()`).
2. **Web = the user's own Chrome, through a bise extension + a native
   host.** Each agent works in its own pink tab group `bise · <agent>`,
   in inactive tabs, through `chrome.debugger` (CDP). No window raise, no
   cursor move, no focus change. The spike proves it (§3).
3. **Desktop apps = a small signed helper app, `bise Computer Use.app`**
   (Swift, menu-bar-less), that holds the Accessibility and Screen
   Recording grants and acts through AX actions and per-process events.
   The web part needs **no** macOS permission, so it ships first.
4. **Every action goes through the approvals gate** like any tool call;
   in `auto`, buy / send / delete / sign-in and the first use of a site
   or app ask with a card; in `yolo` nothing asks (the user's rule).
5. **Cost:** web MVP with setup and cards ~7 agent-days, Chromium family
   (Edge, Brave, Arc, Vivaldi) included; desktop apps +5, built in
   parallel; Chrome Web Store review is calendar time on top (days).
   Firefox and Safari are later phases (§4.1b). Briefs:
   [computer-use-briefs.md](computer-use-briefs.md).

The user's answers (2026-10-01, through main, m_3558) are in §10.

## 2. What exists elsewhere (read on this machine)

| | How it sees | How it acts | Browser | Background |
|---|---|---|---|---|
| Vibe `open-computer-use` plugin (`~/.vibe/plugins/open-computer-use`, npm `open-computer-use` 0.3.5, MIT) | `get_app_state`: screenshot + AX tree with indices | `click` by index or x,y (`click_method`: accessibility, app_post, sky_click, global), `type_text`, `press_key`, `set_value`, `scroll`, `drag` | Chrome as a desktop app, through AX | yes for AX actions; its own signed `Open Computer Use.app` (`LSUIElement`, team J9P29FA5BX) holds the TCC grants |
| Codex / ChatGPT desktop (docs `learn.chatgpt.com/docs/computer-use`, `/browser`, `/chrome-extension`; `~/lab/codex/codex-rs/config/src/{computer_use,browser_use}.rs`) | screenshots + AX; CDP in the browser | AX on macOS, foreground on Windows | 3 paths: a built-in browser (own profile), the user's Chrome through an extension + native messaging, a cloud browser | macOS: "a scoped task in the background while you keep working" |
| Claude in Chrome (public) | DOM/AX via extension | CDP via `chrome.debugger` | user's Chrome, its own tab group | yes |

What we copy from Codex/ChatGPT:

- **App approval** per app (bundle id) and **site approval** per host:
  "allow once / always for this site / no"; an always list the user can
  edit (Codex: `[computer_use.macos] bundle_ids`, `[browser_use] origins`
  with `access`, `downloads`, `uploads`, `full_cdp_access`).
- **Sensitive actions ask** even on an allowed site: submit, purchase,
  permission changes, deletion.
- **Never automate** the terminal, the agent app itself, admin
  authentication and macOS security/privacy prompts.
- Extension permissions they ask for: debugger, all sites, tab groups,
  native messaging, downloads, history (we skip history and bookmarks).
- Browser history access is per request, never "always".

What we do differently: one tab group **per agent** (they have one
agent); a Playwright-like SDK in code mode instead of 9 flat tools; no
separate install.

## 3. Spike (done): background tab group + CDP action

Setup: Chrome 154 (the user's installed binary) on a **throwaway
profile** (`--user-data-dir` in `$TMPDIR`, deleted after), the spike
extension loaded with CDP `Extensions.loadUnpacked` (pipe only:
`--remote-debugging-pipe --enable-unsafe-extension-debugging`; branded
Chrome ignores `--load-extension` since 137). The user's own Chrome was
not touched. `node docs/research/computer-use-spike/run.mjs` reruns it.

| Check | Result |
|---|---|
| tab groups per agent | 3 inactive tabs → 2 groups, `bise · computer-use` (2 tabs) and `bise · pr-designer` (1 tab), colour and title set |
| user's tab | stays active, window focus unchanged, through every step |
| frontmost macOS app | Chrome is not raised by any tab or CDP action (measured with `lsappinfo front`: while the user moved to Ghostty, 4 rounds of clicks did not bring Chrome back). Only the **launch** of the throwaway Chrome took focus, which the product never does |
| AX tree of the hidden tab | `Accessibility.getFullAXTree`: 12 nodes, button and textbox found by role + name |
| click | AX node → `DOM.getBoxModel` → `Input.dispatchMouseEvent`: lands (`clicks: 1`) |
| type | `DOM.focus` + `Input.insertText`: "hello from bise" in the field |
| screenshot of the hidden tab | `Page.captureScreenshot`: 18 KB PNG in 32-420 ms, correct pixels |
| `document.visibilityState` | `hidden` during all of it |
| native messaging | extension → native host → reply in ~400 ms (cold spawn), host manifest inside the throwaway profile |

**The one trap found:** in a hidden tab, the first
`Input.dispatchMouseEvent` (`mouseMoved`) blocks **5.0 s** (it waits for
a frame that never comes), every time. `Emulation.setFocusEmulationEnabled
{enabled: true}` after attaching removes it: **13-24 ms**. Keys and
`mousePressed/Released` were fast in both cases. So the extension sets
focus emulation on every attach.

Not checked by the spike:

- Chrome's yellow "bise started debugging this browser [Cancel]" bar: not
  seen (no screen capture). Chrome shows it while the extension is
  attached; `chrome.debugger.onDetach` with reason `canceled_by_user`
  fires on Cancel. A detach by the extension itself fires nothing (seen).
- Long runs: Chrome's intensive throttling of hidden tabs (timers once a
  minute after 5 min hidden) may slow pages that poll. To measure in
  phase 1 (candidates: `Page.setWebLifecycleState`, keeping the group in
  a second, unfocused window).
- Real sites (login walls, iframes, shadow DOM, canvas apps).
- The desktop (AX) half: not in the spike's scope.

## 4. Options compared

### 4.1 Web

| Option | User's logins | Background, own tab group | Install | Verdict |
|---|---|---|---|---|
| A. Chrome as a desktop app through AX (open-computer-use) | yes | no tab groups; acts on the visible tab; Chrome's AX tree must be switched on for the whole browser (slow) | none extra | fallback only |
| B. CDP port on the user's Chrome (`--remote-debugging-port`, Playwright `connectOverCDP`) | yes | yes | user restarts Chrome with a flag; **Chrome 136+ refuses it on the default profile** | dead |
| C. A separate Chrome profile driven by Playwright/CDP | no (sign in again) | yes, plus flags against throttling | none | later: the "localhost browser" for testing the user's own web app |
| **D. bise extension + native host in the user's Chrome** | yes | **yes (spike)** | one "Add to Chrome" | **pick** |

### 4.1b Other browsers

Desktop shares (StatCounter, desktop, worldwide, mid-2026, as quoted by
testmuai.com and demandsage.com; not checked against the CSV): Chrome
~66-71 %, **Edge ~11-13 %**, Safari ~6-7 %, Firefox ~6 %, Opera ~2 %,
Brave ~1.7 %. On macOS alone Safari's share is much higher (I have no
source for that number). All platforms (Aug 2026): Chrome 69.4 %,
Safari 15.8 %, Edge 5.4 %, Firefox 3.0 %.

**Chromium family in v1** (one extension, +0.5 day): Edge, Brave,
Vivaldi, Opera and Arc run MV3 Chrome extensions with `tabGroups`,
`debugger` and `nativeMessaging`. What differs per browser is where the
native host manifest goes, and the store:

| Browser | Native host manifest dir (`~/Library/Application Support/…`) | Store |
|---|---|---|
| Chrome | `Google/Chrome/NativeMessagingHosts/` | Chrome Web Store |
| Edge | `Microsoft Edge/NativeMessagingHosts/` | Edge Add-ons (own listing, free), or the Chrome Web Store once the user allows "other stores" |
| Brave | `BraveSoftware/Brave-Browser/NativeMessagingHosts/` | Chrome Web Store |
| Vivaldi | `Vivaldi/NativeMessagingHosts/` | Chrome Web Store |
| Arc | to check (it reads Chrome's dir on some versions); its tab model has no real tab groups: best effort, the tabs open without a group | Chrome Web Store |

The broker writes the manifest in each installed browser's dir (setup
and "repair" do it) and names the browser in the setup row and the `↖`
line ("↖ driving Edge · github.com"). The extension stays the same; one
`browser` field in its hello message says which browser it runs in.

**Firefox (later)**: no `chrome.debugger`, so no CDP input in the
user's own Firefox.
- WebDriver BiDi or Marionette give real input but need Firefox started
  with `--remote-debugging-port` / `--marionette`: the user restarts his
  Firefox, and it shows the striped "remote control" address bar on every
  window. Not "in the background, never disturbs him".
- A WebExtension (tabs, `tabGroups` in recent Firefox (to check),
  content scripts, native messaging): it can read the DOM and click with
  DOM events, but those events are untrusted (`isTrusted: false`) and many
  sites ignore them; no screenshot of a hidden tab (`captureTab` exists
  but renders the visible tab area only, to check).
- Third path: the apps helper drives Firefox through macOS AX (Firefox
  exposes the web AX tree), the extension only manages the tabs.
- Cost: a 1-day spike (which of the three holds on 5 real sites), then
  ~3 days. Value: ~6 % of desktops.

**Safari (later)**: a Safari Web Extension must ship inside a signed
macOS app, built with Xcode. `bise Computer Use.app` can be that app
(the same bundle id family, the same signing), so phase 5 already pays
for most of it.
- No `debugger`, no CDP. The extension handles tabs (no tab group API in
  Safari's WebExtensions, to check: else one Safari window per agent) and
  reads the page; actions go through the helper's AX (Safari exposes a
  rich web AX tree, the VoiceOver one): `AXPress` on links and buttons,
  `AXValue` on fields, no window raise.
- `safaridriver` (WebDriver) only drives separate automation windows
  with their own session: not the user's logins.
- The user enables the extension once in Safari → Settings → Extensions
  (and "allow on every website"): one more setup row.
- Cost: after phase 5, a 1-day spike (AX actions on a background Safari
  window), then ~3-4 days. Value: ~6-7 % of desktops worldwide, higher
  on Macs, our only platform.

### 4.2 Desktop apps

| Option | Verdict |
|---|---|
| Wrap `open-computer-use` as is | fast to try, but a third-party binary holds the user's TCC grants under someone else's team id, and its tool shape is not ours. OK for a private experiment, not to ship |
| A Rust subcommand of `bise` | TCC gives the grant to the **terminal** (Ghostty, iTerm2), i.e. to every program run in it. No |
| **Own helper `bise Computer Use.app`** (Swift, signed, `LSUIElement`), started with LaunchServices (`open -g`) so it is its own responsible process | **pick**. Its name is what System Settings lists ("turn on bise Computer Use"). Swift because AX, ScreenCaptureKit and `CGEvent.postToPid` are native there; ~1.5k lines. open-computer-use (MIT) is a reference |

### 4.3 How agents call it

| Option | Verdict |
|---|---|
| 9 flat MCP tools (Vibe/Codex style) | each step a round trip of the model; pixel coordinates; the model re-reads big trees |
| **Playwright-like SDK in code mode**, over a few raw tools | **pick**: one program does open → find → fill → click → check; locators by role and name work on the web (CDP AX tree) **and** on apps (AXUIElement) with one grammar |

## 5. Architecture

```text
agent's program (bend-jsrt, code mode)
  computer.browser.open(...) / tab.getByRole(...).click()      ← SDK in the prelude
        │ each action = one tool call  tools.computer.<raw>     ← gated (§7), replayed by position
        ▼
repl-live ── plugin bridge (built-in plugin "computer") ── `bise computer-use mcp` (stdio, per session)
                                                               │ unix socket ~/.bise/run/computer-use.sock
                                                               ▼
                                     broker: `bise computer-use broker` (one per machine)
                                        ├─ web:  native messaging port ⇄ bise Chrome extension (one per Chrome profile)
                                        │          tab groups, chrome.debugger (CDP), overlay cursor, takeover detection
                                        └─ apps: XPC/socket ⇄ bise Computer Use.app (AX, ScreenCaptureKit, overlay window)
hub ── "stop <agent>" / "stop all" ──▶ broker
```

- **Built-in plugin.** The `computer` namespace is a plugin that lives in
  the app root (`<app root>/plugins/computer/`), always discovered, shown
  in `/plugins` as `built-in`, disable-able like the others. It reuses
  the whole plugin path that exists (`docs/plugins.md`: bridge, index,
  `search_tool_functions`, the gate). No new transport.
- **Broker.** Chrome spawns the native host when the extension calls
  `connectNative`; that host process **is** the broker (Rust, a `bise`
  subcommand, no TCC needed). It listens on the socket; each agent's MCP
  server connects with its `SB_AGENT` name. An open native port also
  keeps the MV3 service worker alive. When Chrome is closed and an agent
  needs only apps, the MCP server starts the broker itself.
- **Native host manifest** at
  `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/dev.bise.computer_use.json`,
  pointing at a stable shim `~/.bise/bin/bise-chrome-host` (the bise
  version switch moves the install dir; the shim follows `current`). The
  extension id is pinned with a `key` in its manifest, so
  `allowed_origins` is known before install.
- **Agent identity.** Tab groups, cards, stop and the sidebar mark all key
  on `SB_AGENT`. A single-agent `bise --headless` session uses its
  session name.

### 5.1 The extension (MV3)

- Permissions: `tabs`, `tabGroups`, `debugger`, `nativeMessaging`,
  `scripting`, `downloads`; host `<all_urls>`. No history, no bookmarks.
- One group per agent: `chrome.tabs.create({active: false})` in the last
  focused normal window, `chrome.tabs.group`, `tabGroups.update({title:
  "bise · <agent>", color: "pink", collapsed: false})` (designer: not
  collapsed, so you see it work). At most 5 tabs per agent (raise later).
- Attach on first action, `Emulation.setFocusEmulationEnabled` (§3),
  detach when the agent's turn ends (the yellow bar goes away when no
  agent drives).
- **Takeover:** `tabs.onActivated` on an agent tab, or a user
  click/keypress seen by the content script in it → that tab is paused;
  the next action returns `paused: the user is using this tab` and the
  agent row shows "? you took the wheel · ⏎ give it back".
- **Stop:** `debugger.onDetach(canceled_by_user)` (the yellow bar's
  Cancel) or the group closed by the user → the broker marks the agent
  stopped; its running action fails with "stopped by the user"; later
  actions fail until the user gives it back (`/computer-use` or the row).
- **Cursor overlay:** a content script draws, in a closed shadow root
  with `pointer-events: none`, the designer's pointer (dark #141211 arrow,
  1.5px #ece6da edge, soft pink #f4a6b0 glow, the agent's name in a pill
  with a 2px pink left edge); before each click it glides there (~150
  ms), then a pink ring grows and fades in 300 ms. Purely visual: the
  click itself is CDP input. Not drawn on pages where scripts can't run
  (chrome://, the Web Store): there the action is refused anyway.
- Refused origins: `chrome://*`, `chrome-extension://*`, the Web Store,
  `accounts.google.com/signin` password steps (the user does them).

### 5.2 The helper app (desktop)

- Started with `open -g -a "bise Computer Use"` (no activation). Its own
  TCC entries. Talks to the broker over a unix socket it is told at start.
- See: the target app's AX tree (window → elements; role, title/label,
  value, enabled, focused, frame), cut to a budget; screenshots of **one
  window** with ScreenCaptureKit (works when the window is covered; never
  the full screen).
- Act, in order of preference, all without raising the app or moving the
  cursor: `AXPress` / `AXConfirm` / `AXShowMenu`; `AXValue` set; keys and
  text with `CGEvent.postToPid` to that pid; scroll with `AXScrollBy*`
  or pid-posted wheel events. Some apps ignore pid-posted input; then the
  action fails with "Figma needs to be in front for this" and the agent
  can ask (a card "bring Figma to the front?"), never a global CGEvent
  silently.
- Overlay cursor: a click-through, non-activating `NSPanel` over the
  target window, drawn only while that window is visible.
- Refused apps: terminals (Terminal, Ghostty, iTerm2, WezTerm, kitty,
  Alacritty, Warp), bise itself, System Settings' Privacy & Security,
  Keychain Access, password managers, `loginwindow`.
- User input in the driven app (an `NSEvent` global monitor, mouse/keys
  while that app is frontmost) pauses the agent, like the web takeover.

## 6. The agent API: a small Playwright-like SDK

Exposed in code mode as a global `computer`, written in TypeScript in the
jsrt prelude (or a file the prelude includes), compiled into calls to the
raw `tools.computer.*` functions. It respects the replay model of
`run_typescript` (rust/jsrt/src/main.rs): every action is one tool call,
handles are plain ids (`tab 1742`, `app com.figma.Desktop`), locators are
lazy descriptors resolved on the broker side, nothing random.

```ts
async function main() {
  const tab = await computer.browser.open("https://www.amazon.fr");     // bg tab, group "bise · <agent>"
  await tab.getByRole("searchbox").fill("usb-c cable 2m");
  await tab.press("Enter");
  await tab.getByRole("link", { name: /Anker.*2 m/ }).first().click();
  const price = await tab.getByText(/€/).first().textContent();
  return [{ type: "text", text: price }, await tab.screenshot()];      // image block for the model
}
```

```ts
const figma = await computer.app("Figma");                              // by name or bundle id
const snap = await figma.snapshot();        // compact AX text with refs: - button "Share" [e14]
await figma.ref("e14").click();             // or figma.getByRole("button", { name: "Share" })
await figma.getByRole("textbox", { name: "Email" }).fill("ana@example.com");
```

Surface (first version, small on purpose):

| Object | Methods |
|---|---|
| `computer` | `browser.open(url)`, `browser.tabs()` (this agent's tabs only), `browser.tab(id)`, `apps()`, `app(nameOrBundleId)`, `status()` |
| tab / app | `snapshot({ maxNodes })`, `screenshot({ element? })`, `getByRole(role, {name, exact})`, `getByText(t)`, `getByLabel(t)`, `ref(id)`, `press(keys)`, `scroll(dir, amount)`, `goto(url)` (tab), `close()` (tab), `waitFor(text \| locator, {timeout})` |
| locator | `click()`, `fill(text)`, `type(text)`, `check()`, `select(value)`, `hover()`, `textContent()`, `count()`, `first()`, `nth(i)` |

Rules that make it efficient for the model:

- **Snapshot first, screenshot when needed.** `snapshot()` is the
  Playwright-MCP style aria text with `[eN]` refs, cut to a budget (~3k
  tokens), cheap and exact; `screenshot()` returns an image block (≤
  1280 px wide, JPEG), only when looks matter.
- **Every action returns what changed**: `{ ok, url, title, changed:
  "<short aria diff>" }`, so the program often needs no new snapshot.
- **Auto-wait** like Playwright: an action waits (≤ 5 s) until the
  locator resolves to one visible, enabled element; 0 or several matches
  → a clear error listing the candidates with refs.
- **Same locator grammar** for web (CDP AX tree) and apps (AXUIElement:
  `AXButton` → `button`, `AXTextField` → `textbox`; the full map is in computer-use-briefs.md C2).
- Raw tools stay callable (`tools.computer.act({...})`) for odd cases;
  coordinates exist (`tab.clickAt(x, y)`) but are the last resort.
- A skill `computer-use` (built-in) teaches the 10 lines above plus "use a
  connector or CLI when one exists; computer use for the rest".

Raw tools (what the gate sees, one line each, readable on a card):
`computer.open`, `computer.snapshot`, `computer.screenshot`,
`computer.act` (`{target, action: click|fill|type|press|select|scroll|
goto|close, locator|ref, text}`), `computer.tabs`, `computer.apps`,
`computer.status`.

## 7. Approvals, privacy, stop

### 7.1 Approvals (on top of approvals-design.md)

Every `computer.*` call goes through the gate like a connector call.

- `yolo`: nothing asks (the user's firm rule). The refused targets (§5.1,
  §5.2) are not approvals, they are refusals: they hold in every mode.
- `auto`:
  - runs at once: `status`, `tabs`, `apps`, `snapshot`, `screenshot` of
    an already allowed site/app; actions on `localhost`/`127.0.0.1`;
    actions on allowed sites/apps that the rules below don't catch;
  - **first use of a site (host) or app (bundle id)**: card "? api-v2
    wants to use amazon.fr in Chrome · first time" → 1 allow once, 2
    always on amazon.fr, 3 no. "always" goes to `~/.bise/approvals.toml`
    under `[computer_use] sites = [...] apps = [...]` (global, not per
    repo: a site is not a repo thing);
  - **always asks, never "always"** (hard rules, judged on the target
    element's role + name + the page's host + nearby text, by the broker,
    no model): buy/pay/order (`place order`, `buy now`, `pay`, `commander`,
    `payer`, `checkout`...), send/post/publish (on mail, chat and social
    hosts, or a `send` button anywhere), delete/remove account or data,
    permission and security settings, typing into a password field
    (sign-in card: 1 i'll do it, 2 let it, 3 no);
  - the rest of the actions on allowed sites go to the `checker` with the
    action line, the host, the element and the task: same tiers and cache
    as bash commands.
- The card (designer, §8): the approvals card, plus a `▣` chip that opens
  a crop of the target element in Preview (no picture inside the TUI).

### 7.2 Privacy: what goes where

- Snapshots and screenshots go **to the model of the agent that asked**,
  as tool results, like any tool output; nowhere else. `/computer-use`
  says it in one line, with the provider's name.
- A tab screenshot is that tab only (CDP), an app screenshot that window
  only (ScreenCaptureKit). Never the desktop, never other tabs.
- Password fields: values never read (AX secure text has none; the
  extension masks `input[type=password]` in snapshots).
- Stored: only in the agent's session (feed, journal), deleted with it;
  the `▣` crops in the agent's `TMPDIR`.
- Page text is untrusted input: the snapshot is wrapped as "content from
  <host>" and the skill says so (prompt injection is the main risk; the
  hard rules in §7.1 are the backstop, they do not ask the model).

### 7.3 Stop

| From | Effect |
|---|---|
| ctrl+c in the agent's view, `sb interrupt` | the turn stops (exists today); the broker detaches its tabs/app |
| click on the `↖` mark, `/stop <agent>` | the same, from anywhere |
| Chrome's yellow bar Cancel | `onDetach(canceled_by_user)` → the agent is stopped |
| closing the agent's tab group | the same |
| touching an agent tab or the driven app | pause, not stop: "? you took the wheel · ⏎ give it back" |
| `/computer-use` → "stop all" | every agent lets go |

Each stop writes one line in main's feed: "↖ api-v2 stopped driving
Chrome · you stopped it". `/drop` of an agent closes its group unless the
user touched one of its tabs.

## 8. bise UI (designer's calls, m_3551)

`/computer-use`: one screen like `/models`, one row per step, the
selected row shows its fix line and its ⏎ action. Glyphs: `✓` done
(accent), `?` waits for you, `∿` checking, `✗` failed, `·` not yet.

```
 computer use · agents can drive your apps and Chrome

 ✓ chrome              Chrome 154
 ? chrome extension    add bise to Chrome
 · live test           last

 ⏎ open its Web Store page   ↑↓ step   esc later
```

- Order (settled with designer, m_3554): **chrome → chrome extension →
  live test**. The web needs no macOS permission. While apps are not
  shipped, the accessibility and screen recording rows **are not shown at
  all** (a row for a feature that doesn't exist yet is noise). When apps
  ship (phase 5), they come back after the live test under a faint
  "for apps" label, and stay optional: Chrome works without them. Poll
  every second; a fix flips the row to `✓` and the cursor moves on.
- Chrome failures and fixes (designer's lines): Chrome isn't open (⏎ open
  Chrome); add bise to Chrome (⏎ open its Web Store page; before the
  store: ⏎ shows the 3 "load unpacked" steps); it's in the "Work"
  profile, add it to the one you browse with; Chrome too old (⏎
  chrome://settings/help); the extension can't reach bise (⏎ repair:
  rewrites the native host manifest and shim).
- Phase 5 rows: "turn on bise Computer Use in the list"; denied: "macOS
  says no. Privacy & Security → Accessibility, turn on bise Computer
  Use" (the helper holds the grant, never the terminal). Screen Recording: the
  "reopen" line only if macOS needs it for the helper (to check: the
  helper can relaunch itself, the user's terminal is not involved).
- Live test: opens `bise · setup` group in the background with a local
  test page served by the broker, clicks, types, screenshots, closes it;
  "✓ ready. ask any agent to use Chrome or an app. /computer-use checks
  it again any time".
- While an agent drives: the `↖` mark in the row's last column (accent;
  ASCII `C`; font check with fontTools in SF Mono, Menlo, JetBrains
  Mono), ctrl held shows "↖ driving Chrome · amazon.fr"; the divider
  "you → api-v2 · opus 5.5 · yolo · ↖ Chrome"; tool rows "↖ clicked "Add
  to cart" · amazon.fr".
- Cards: §7.1. The inbox counts them like the other cards.
- First run without setup: an agent that calls `computer.*` gets "computer
  use isn't set up: ask the user to run /computer-use" and main's board
  says it once.

## 9. Plan

Cost in agent-days (one agent, gate green, designer review included).
Nothing lands on main before the user tests a local build (like
approvals: one branch, `computer-use`).

| Phase | What | Cost |
|---|---|---|
| 0 | this doc + the spike | done |
| 1 | **Web core**: extension (groups, attach + focus emulation, AX snapshot with refs, actions, screenshot, takeover, stop, cursor overlay), broker + native host + shim (Rust, `bise computer-use broker|mcp`), built-in plugin `computer`, raw tools; an e2e test on a throwaway profile (the spike harness grows into it) | 3 |
| 2 | **SDK** in the jsrt prelude (locators, auto-wait, diffs), the built-in skill, `search_tool_functions` text; a 10-task bench on real sites (agent steps, tokens, wall time) | 1.5 |
| 3 | **`/computer-use`** screen, checks, repair, live test; `↖` mark, divider, tool rows, stop paths; tmux captures for designer | 1.5 |
| 4 | **Approvals**: site/app allow lists in approvals.toml, hard rules (buy/send/delete/sign-in), checker prompt for actions, the `▣` crop chip | 1 |
| 5 | **Desktop apps**: `bise Computer Use.app` (Swift: AX tree, actions, pid events, ScreenCaptureKit, overlay panel, takeover), signing + notarization in packaging, TCC flow in `/computer-use` | 5 |
| 6 | **Store**: Chrome Web Store listing (developer account, privacy policy, review: days of calendar time), Edge Add-ons listing, installer step | 0.5 + review |
| 1+ | **Chromium family** in phase 1: native host manifests for Edge, Brave, Vivaldi, Opera, Arc; browser named in rows and lines | 0.5 |
| 7 | **Safari** (after 5): spike, then the Safari Web Extension inside `bise Computer Use.app`, actions through AX (§4.1b) | 1 + 3-4 |
| 8 | **Firefox**: spike (BiDi vs extension vs AX), then the pick (§4.1b) | 1 + 3 |

Web-only MVP (phases 1-4, Chromium family included): **~7.5
agent-days**, usable with "load unpacked" before the store review ends.
With apps: **~12.5**. The user wants the apps part started now, in
parallel with the web MVP: both build on the contracts (the broker's
socket protocol, the raw tools, the locator grammar), written first
(half a day). The parallel split is in
[computer-use-briefs.md](computer-use-briefs.md).

### 9.1 What the user does himself (once)

- **Chrome Web Store**: register as a developer with his Google account
  (a one-time US$5 fee, 2-step verification on, a verified contact
  email, publisher name "Gabriel Vergnaud"); then, with the package the
  `cu-store` agent prepares: upload it (the first upload gives the
  extension's final id, which the native host manifest needs: do it
  early, as an unlisted draft), fill the listing (texts, icon, 1280x800
  screenshots, the single-purpose line, why it needs `debugger`,
  `<all_urls>`, `nativeMessaging`, `tabGroups`, the data-use form, a
  privacy policy URL), submit, answer the review if it asks.
- **Edge Add-ons** (Partner Center, free): the same package and texts.
- **Apple**: the Developer ID Application certificate and the notary
  credentials packaging.md already plans for; the helper app is signed
  and notarized with them (bundle id `dev.bise.computer-use`).

Risks: hidden-tab throttling on long tasks (§3); sites that detect CDP
(some banks, anti-bot) → the agent says so and stops; the debugger
yellow bar on every Chrome window while an agent drives (Chrome rule, no
way around it in the user's Chrome); Web Store review of a `debugger` +
`<all_urls>` extension may ask questions; pid-posted keys ignored by some
apps.

## 10. The user's decisions (2026-10-01, m_3558)

1. **Web first, and the apps part starts now too**, in parallel (two
   tracks on the same contracts).
2. **`yolo` is `yolo`**: nothing asks, not even "Place order". The
   `/computer-use` screen says it in one line: "in yolo, agents act
   without asking, purchases included. ⇧⇥ for auto."
3. **"Always allow a site" is global**, all repos
   (`[computer_use] sites` in `~/.bise/approvals.toml`).
4. **Chromium family in v1** (Edge, Brave, Vivaldi, Opera, Arc best
   effort: +0.5 day). Firefox and Safari: later phases with costs and
   shares (§4.1b, §9 phases 7-8).
5. **His accounts**: the Chrome Web Store under Gabriel's developer
   account (what he does himself: §9.1); bundle id
   `dev.bise.computer-use`; signed with his Developer ID (packaging.md).
6. **Swift** for the helper app (main explained what it is; he did not
   object).
