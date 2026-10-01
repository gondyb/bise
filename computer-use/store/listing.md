# Chrome Web Store listing: bise computer use

Texts to paste in the developer console, field by field. The zip comes
from `computer-use/store/build.sh`. Edge Add-ons takes the same zip and
texts.

## Package

- Item name (from manifest.json): **bise computer use**
- Summary (manifest `description`, max 132 chars): Lets your bise agents use the web in their own tab groups, in the background.
- Version: from manifest.json (0.1.0 for the first upload).

## Store listing tab

**Description** (plain text, max 16,000 chars):

```
bise computer use lets the AI agents of bise (https://bise.dev), the terminal app that runs one agent per task in parallel, use websites in your own browser while you keep working.

What it does
- Each agent works in its own tab group, named after it ("bise · api-v2"). Its tabs open in the background and never take your active tab.
- Agents read pages as text, click, type, fill forms and take screenshots of their own tabs, with your logins, like you would.
- A small "↖ api-v2" marker shows which tab an agent is using. Touch one of its tabs and the agent pauses; stop it from bise at any time.

What it never does
- It never types passwords, 2FA codes or card numbers: an agent asks you to sign in yourself.
- It never touches your own tabs, browser settings, extension pages or the Web Store.
- It sends nothing to bise's servers. Pages are read on your Mac and go only to the AI model you chose in bise, for the agent that asked.

Requirements
- bise on macOS, with computer use turned on (run /computer-use in bise). The extension does nothing without it.

Open source: https://github.com/gvergnaud/bise
```

- Category: **Productivity** (Workflow & Planning if offered)
- Language: English
- Icon 128x128: `computer-use/extension/icons/128.png` (designer's, also in docs/brand/computer-use/icons)
- Screenshots, 1280x800, upload in this order (designer's, docs/brand/computer-use/store/ on main, source store.html):
  1. `1-own-tabs.png`
  2. `2-one-line-per-action.png`
  3. `3-setup.png` (the setup screen's final words, "Chrome is ready. ask any agent to use it.": re-render if the screen changes)
- Small promo tile 440x280: `docs/brand/computer-use/store/tile-440x280.png`
- Official URL: https://bise.dev (verify the domain in Search Console if the console asks)
- Homepage URL: https://bise.dev
- Support URL: https://github.com/gvergnaud/bise/issues

## Privacy tab

**Single purpose**:

```
Let the user's bise agents (AI agents running in the bise app on the same Mac) open, read and act on web pages in their own background tab groups, so the user can delegate web tasks without giving up his browser.
```

**Permission justifications**:

| Permission | Justification to paste |
|---|---|
| `debugger` | Agents act on pages with the Chrome DevTools Protocol (input events, accessibility tree, screenshots) in their own background tabs only. This is what lets them click and type without bringing the tab to the front or moving the user's mouse. Chrome shows its "started debugging this browser" bar while it is attached; Cancel stops every agent. |
| `tabs` | Open, list, navigate and close the agents' own tabs, and read their URL and title to report where an agent is. The user's own tabs are never read or changed. |
| `tabGroups` | Each agent's tabs live in a tab group named after the agent ("bise · api-v2"), so the user sees at a glance which tabs are an agent's. |
| `nativeMessaging` | The extension talks only to the bise app installed on the same Mac (native host dev.bise.computer_use), which relays the agents' requests. Nothing goes to a server. |
| `scripting` | Injects a small overlay into an agent's own tabs: the "↖ agent" marker, and detecting when the user touches that tab (the agent then pauses). |
| Host permission `<all_urls>` | Agents work on whatever site their task needs (the user's dashboards, shops, docs). The extension acts only in tabs that an agent opened itself; some pages are always refused (browser settings, extension stores, password managers). |
| Remote code | **No**: all code ships in the package. |

**Data usage** (check exactly these):

- Personally identifiable information: **no** collection by the developer.
- Website content: **yes** (text, images and screenshots of the pages an agent works on, in its own tabs). Handled on the user's device and sent only to the AI model provider the user configured in bise, for that agent's task.
- Authentication information, financial info, health, location, personal communications, web history, user activity: **no** (the extension refuses password fields; it doesn't read history; it doesn't record the user's activity).

Certify all three:
- I do not sell or transfer user data to third parties, outside of the approved use cases (the user's own AI provider, at his request, is the use case).
- I do not use or transfer user data for purposes that are unrelated to my item's single purpose.
- I do not use or transfer user data to determine creditworthiness or for lending purposes.

Privacy policy URL: **https://bise.dev/privacy** (draft: `computer-use/store/privacy.md`, the designer publishes it).

## Distribution tab

- Visibility: **Unlisted** (first release; Public later).
- Regions: all.
- Pricing: free.

## After approval

- Send the item ID (32 letters) to the computer-use task: the native host's
  `allowed_origins` gets `chrome-extension://<id>/` next to the dev ID, and
  `/computer-use` links to the Store page instead of "Load unpacked".
- Edge Add-ons (partner.microsoft.com/dashboard/microsoftedge): same zip,
  same texts, free account; its ID goes into `allowed_origins` too.

## Review notes (the "notes for the reviewer" field, if asked)

```
The extension does nothing on its own: it needs the bise app (open source, https://github.com/gvergnaud/bise) on macOS with computer use turned on. To try it: install bise (curl -fsSL https://bise.dev/install.sh | sh), run bise, type /computer-use and follow the steps, then ask the agent "open news.ycombinator.com in a tab with computer use and tell me the top story". The debugger permission is attached only to tabs the agent opened, in its own tab group.
```
