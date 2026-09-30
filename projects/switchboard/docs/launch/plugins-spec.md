# launch plugins · one per provider · spec for main

goal: the `launch` agent can watch the launch live (ranks, replies, numbers) from inside bise. **read-only everywhere. no tool that posts, replies, votes or sends.** Kit gets a draft tool only.

where: user root `~/.agents/plugins/<name>/` (personal, keys stay out of the repo). bise loads stdio MCP servers only (docs/plugins.md), plugin edits apply on `/reload`.
secrets: from the environment (the server gets the REPL env) or `${PLUGIN_DATA}/credentials.json`, never in the plugin folder.
stack: one small Node stdio server per plugin (@modelcontextprotocol/sdk), no build step if possible. each tool returns compact JSON (no raw HTML, cut long texts).
test: each server answers `tools/list`, and one real call per tool works (`bise plugins` shows no diagnostic).

priority = what matters on launch day, no key first.

## 1. `hn` · Hacker News · no key

APIs: Firebase `https://hacker-news.firebaseio.com/v0/`, Algolia `https://hn.algolia.com/api/v1/`.
- `find_story(query)`: Algolia search, stories only, last 48 h → id, title, points, comments, age.
- `story_status(id)`: points, comment count, age, **rank** on /newest, /show and the front page (scan `topstories`, `showstories`, `newstories` ids, give the position or null).
- `comments(id, since?)`: the comment tree flattened: id, author, time, parent, text (plain), depth. `since` = only newer ones.

## 2. `bise-stats` · GitHub numbers · no new key (uses `gh auth token` or GH_TOKEN)

- `repo_stats()`: stars, forks, watchers, open issues, open PRs for gvergnaud/bise.
- `release_downloads()`: per release and per asset `download_count`. the installer downloads a release asset, so this is our install count (to verify with main: which asset = one install).
- `new_issues(since)` and `stargazers(since)`: with timestamps (the star-history API header).

## 3. `bluesky` · no key for reading

API: public AppView `https://public.api.bsky.app/xrpc/`.
- `search_posts(q, since?)`: `app.bsky.feed.searchPosts` (bise.dev, "bise", the handle).
- `thread(uri)`: `app.bsky.feed.getPostThread` → replies flattened, like/repost/quote counts.
(to verify: searchPosts may need auth on the public AppView. if so: an app password in credentials.json, still read-only.)

## 4. `reddit` · no key first

API: public `.json` endpoints (`https://www.reddit.com/r/<sub>/comments/<id>.json`), with a real User-Agent.
- `post(url_or_id)`: score, upvote ratio, comment count, removed?
- `comments(url_or_id, since?)`: flattened like `hn`.
- `search(q, subs?)`: mentions of bise.dev.
(to verify: reddit may block unauthenticated calls. fallback: a free "script" OAuth app, read-only.)

## 5. `x` · needs Gabriel: X developer account + a bearer token + a few $ of credits

X API v2 is pay-per-use since feb 2026 (≈ $0.005 per post read, no free tier; source: postproxy.dev, to verify on developer.x.com). launch-day reading = a few hundred posts = a few dollars.
- `mentions(since)`: recent search `bise.dev OR "@<handle>" OR to:<handle>`.
- `thread_replies(tweet_id)`: `conversation_id:<id>`.
- `post_metrics(tweet_id)`: likes, reposts, quotes, replies, impressions (public_metrics).
- `quotes(tweet_id)`.
no write endpoints, even if the token allows it.

## 6. `kit` · needs Gabriel: Kit API key (v4)

API: `https://api.kit.com/v4/`.
- `list_broadcasts()`, `broadcast_stats(id)`: recipients, open rate, click rate, unsubscribes.
- `create_draft(subject, preview, html)`: a broadcast **with no send_at**, so it stays a draft Gabriel sends himself. no send, no schedule tool.
- `subscriber_count()`.
(to verify: whether Kit ships its own MCP server; if it's HTTP-only, bise can't load it today, so ours stays.)

## skipped

- **LinkedIn:** the API for personal posts and their comments needs an approved partner app. not doable today.
- **Vercel analytics** (hits on bise.dev/install): Vercel's MCP is remote HTTP, bise loads stdio only. `release_downloads` covers installs. (to verify: a REST endpoint for Web Analytics.)
