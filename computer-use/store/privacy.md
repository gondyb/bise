# Privacy (draft for bise.dev/privacy)

Draft for the designer to publish at https://bise.dev/privacy. The Chrome
Web Store and Edge Add-ons need this URL. It covers bise and its
computer use extension. Plain language; facts only. Check with the
computer-use task before changing a fact.

---

## Privacy

*Last updated: October 2026*

bise is an app that runs on your Mac. It has no account, no server of
its own and no analytics. This page says what bise and its browser
extension, **bise computer use**, do with your data.

### What stays on your Mac

- Your conversations with the agents, their work, their files and
  settings live in `~/.bise` on your Mac.
- Your provider keys stay on your Mac (in your config or your
  keychain). bise never sends them anywhere but to that provider.

### What goes to your AI provider

bise sends the agents' conversations to the AI model provider **you**
chose (for example Anthropic, OpenAI or Mistral), with your key, to get
the agents' answers. That provider's own privacy policy applies to what
it receives.

### Computer use (off unless you turn it on)

Computer use is off by default. When you turn it on with `/computer-use`:

- **What the agents see.** An agent reads the pages it opens in its own
  tabs (as text, and screenshots when it needs to see the page), and the
  windows of the Mac apps it drives. That content goes only to the AI
  provider that agent uses, for that agent's task. It is not sent to
  bise or to anyone else.
- **Your tabs stay yours.** Agents work in their own tab groups. They
  don't read or change your other tabs, your history or your bookmarks.
- **No passwords.** Agents never type into password fields. When a site
  needs you to sign in, the agent asks you to do it.
- **Some places are always off limits:** browser settings, extension
  stores, password managers, Keychain, Terminal and macOS privacy
  settings.
- **Screenshots** are saved in the agent's temporary folder on your Mac
  and deleted with the agent.
- **The extension** talks only to the bise app on the same Mac (Chrome's
  native messaging). It has no server and no remote code.
- **Turning it off:** `/computer-use off` stops it; `/computer-use
  uninstall` also removes what it installed, and tells you how to remove
  the extension and the macOS permissions.

### What bise collects

Nothing. bise has no telemetry. When you update it, the download comes
from GitHub Releases, and GitHub counts downloads.

### Contact

Questions or concerns: open an issue at
https://github.com/gvergnaud/bise/issues, or write to
gabriel.vergnaud@gmail.com.
