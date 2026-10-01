# Computer use: the checks only a person can run

The §5 paths of `docs/computer-use-ship.md` run automatically on the real
stack (headless throwaway Chrome, the extension, this tree's host and
broker), never on your browser:

    node --max-old-space-size=1024 computer-use/bench/paths.mjs --bise <bise>

(cancel, refused pages, group closed, service worker stopped, two
browsers then one quits, the helper quits and reopens; ~1.5 min). What
headless Chrome can't show is below: run each once on your Mac before
announcing computer use. Each says what to do and what you should see.
"An agent" means any agent you ask, in plain words, to use the web or an
app (the plugin on: `/computer-use`).

## 1. The debugging bar's Cancel

Headless Chrome has no bar; the test fires the extension's handler the way
Chrome does.

1. Ask an agent: "open https://example.com in Chrome, then wait for 2
   minutes for the text 'never'" (any page it keeps driving works).
2. When Chrome shows the bar "bise computer use started debugging this
   browser", click **Cancel** in it.

You should see, within a second:

- in main's feed: `↖ <agent> stopped driving Chrome · you pressed Cancel in Chrome`;
- the agent's call fails `stopped`, and it says it stopped and asks you
  before going on (it doesn't try again by itself);
- the bar does not come back while the agent waits; its tabs stay open in
  its pink group.

3. Answer the agent ("go on"). It drives again (your message is the go
   ahead: the TUI runs `bise computer-use resume <agent>`), and the bar
   comes back.

## 2. The Mac sleeps mid-task

1. Ask an agent for a slow web task (e.g. "open 3 pages of
   https://news.ycombinator.com one by one and read their titles").
2. While it works: Apple menu → Sleep. Wait 1 minute, wake the Mac.

You should see: the call that was running when the Mac slept finishes or
fails with `timeout` (no hang, no crash), and the agent's next call works
without `/computer-use`. `bise computer-use status` shows Chrome
`connected: true`.

## 3. Screen Recording: grant, Quit & Reopen

The automated run checks the reopen with the real helper code (dev host);
the grant itself is macOS's.

1. `/computer-use`, select the **screen recording** row, ⏎.
2. macOS shows its prompt, then System Settings → Privacy & Security →
   Screen Recording. Turn on **bise Computer Use**. macOS asks to **Quit &
   Reopen**: accept.

You should see: the row turns `✓` by itself within about a second, without
leaving `/computer-use`. Then ask an agent for a screenshot of an open app
window (e.g. TextEdit): it gets the image.

## 4. A work profile (managed browser)

Needs a Mac or a Chrome profile your organisation manages (`chrome://policy`
lists policies). bise reads the policies macOS gives the browser
(`/Library/Managed Preferences`, MDM): the extension blocked
(`ExtensionInstallBlocklist`, `ExtensionSettings`), its link to bise blocked
(`NativeMessagingBlocklist`, `NativeMessagingUserLevelHosts` false), or the
debugger blocked (`DeveloperToolsAvailability` 2).

1. `/computer-use` in that browser.

You should see: the **chrome extension** row fails with "your organisation
blocks … in Chrome (<policy>); use a browser or profile it doesn't
manage", instead of waiting for the extension.

2. If your profile gets its policies from the cloud (a signed-in work
   account, "Your browser is managed by …" with no MDM): bise can't read
   those. Note what happens (the row waits, or the extension connects and
   an agent's first action fails) and the exact error text of the agent's
   call. With `DeveloperToolsAvailability` 2 the extension says "your
   organisation's browser policy blocks extensions from driving tabs"
   when Chrome's error names a policy; send us Chrome's exact words if it
   says "the browser doesn't let extensions drive this page (…)" instead.
