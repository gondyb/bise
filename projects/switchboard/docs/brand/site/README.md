# bise website (static)

This folder is the whole site. No build step: serve it as is (GitHub Pages,
Netlify, Cloudflare Pages, `python3 -m http.server`).

| path | page |
|---|---|
| `index.html` | the landing page |
| `book/index.html` | the brand book (UI, UX, tone, philosophy, values) |
| `book/screens.html` | every screen of the product |
| `book/live.html` | the live simulation (`?t=45&paused` to jump) |
| `book/onboarding.html` | the first launch |
| `install.sh` | `bise.dev/install`: the installer, a copy of the one the latest release carries (`packaging/publish-release.sh` says when it is stale) |

Old paths (`brand/tui-screens.html`, `brand/tui-live.html`,
`brand/tui-onboarding.html`, `brand/landing/index.html`) are small redirects
to these pages. Owner: the `marketing` task.
