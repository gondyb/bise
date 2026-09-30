# X plan · launch thread + 5 capsules + daily posts

rules for every post: pain first, then the promise, then the proof. lowercase. upload the videos to X directly (a Loom link gets shown to fewer people). links go in a reply or the last post, never in a hook post.

---

## 1. the launch thread (wed 16:00)

### the hook (post 1). pick one

| # | hook |
|---|---|
| A | i had five agent tabs open and couldn't tell you what any of them was doing. so i built a terminal where i only talk to one :* |
| B | agents made me faster and my days emptier. so i built bise :* |
| C | you didn't become an engineer to babysit robots. |
| D | "wait, which tab was that agent in?" never again. |
| E | i stopped babysitting my coding agents. one of them does it now. |

**my pick: A.** it's a real scene, everyone with Claude Code has lived it, and it makes people want to see the rest. then under the video: "it's called bise. one thread per repo. you talk, it runs the agents. you stay in flow."

posts 2-7: as in [posts.md](posts.md). post 2 changes if you pick A (the five tabs are already in the hook): start with "i was the router, the memory and the merge tool…".

### the video (40-60 s, scripted, landing style)

- 0-12 s · **the pain:** the landing's "before" (five tabs, "should i continue?", two agents in the same file). caption: `five agents. five tabs. one very tired you.`
- 12-50 s · **the calm:** a real screen recording ([video.md](video.md)): one thread, three ideas typed, three agents start, a card answered with one key, ✓ lines.
- 50-60 s · `one thread per repo. you stay in flow.` + bise.dev

(to verify with designer: can the landing's before/after and flow demos be exported as video for today?)

---

## 2. the 5 how-it-works capsules

format: 2-3 min, screen + your face in a circle, bottom left. talk as you would to a friend, no script to read, just the beats below. one real demo per capsule. cut to < 2:20 for X (above that, only premium accounts can post it).

### capsule 1 · "the thread that runs itself"

hook: **my disk was full of copies of my repo. one worktree per agent, and i had to clean them all up myself.**

beats:
1. the pain: one session per agent, one worktree per session, you manage all of it.
2. in bise there are no sessions. one thread per repo, it never ends.
3. talk anytime: type while main works. ✓ got it, ✓✓ read it. no "wait for the turn to end".
4. agents talk to each other: show a ✉ between two agents ("are you touching tokens.css?").
5. worktrees: ψ shows up when an agent needs its own copy, and it's gone once the work is merged. you never think about it.
6. nothing dropped: an agent stops half-way, main picks the work back up.

to verify before recording: when exactly is a worktree removed (when the task ends, or after main merges it)? ask main.

### capsule 2 · "memory without a memory system"

hook: **people keep asking what memory system bise uses. there isn't one. here's why it doesn't need one.**

beats:
1. main's thread never ends. when it gets long, bise compacts it. main remembers your project the way a teammate does.
2. and when it needs the details: agents can search every thread in the repo, even the parts from before a compaction, even agents that are long gone (`sb history`, `sb show`).
3. demo: "the thing we did on the divider last week?" → it finds it, quotes it, carries on.
4. the point: you talk about past work in your own words.

to verify: don't say "6 months, 1 year ago" on screen. bise is weeks old, so show a real search from last week and say "months later, same thing" only as the design.

### capsule 3 · "every MCP, always on"

hook: **i have [N] MCP servers on and my agent's context is still tiny. here's the trick.**

beats:
1. the usual problem: every MCP server dumps its tool list in the context. ten servers and the model is drowning.
2. in bise, agents call tools by writing TypeScript. every server becomes functions under `tools.*`, found with a search.
3. demo: one ask that uses web search, GitHub and Slack. ctrl+o to show the script it wrote.
4. why it's powerful: loops, filters, joins in one script, and only the result comes back.
5. your skills and Agent Plugins come along, always on.

conflict: you want to say "we shipped this in Vibe too". that names your employer, and we just took Mistral out of everything. your call: skip it, or say it only after you talk to your manager.
to verify: the real number of MCP servers you have on for the hook.

### capsule 4 · "the little things"

hook: **the best part of bise is the stuff nobody asked for.**

beats (one demo each, 15 s):
1. select lines in the history and just type: they come along as a quote. no shortcut.
2. ctrl+v a screenshot: "why is this ugly?"
3. voice: /voice, then ctrl+r, ramble, it's text.
4. hold ctrl: the shortcuts show up where they work (Ghostty, kitty).
5. ctrl+o: everything folded opens up. ctrl+`: a real shell.
6. your terminal's colors, light or dark.

question for you: by "chips" do you mean something specific in the UI? tell me and i'll add it.

### capsule 5 · "restart whenever"

hook: **i push updates to bise while i'm using it. mid-task. nothing breaks. here's how that's possible.**

beats:
1. demo: agents working, quit bise (or update), come back: everything is where it was, work continues.
2. why that's hard: messages in flight, agents waiting on each other, cards open. restart at the wrong time and you lose one or send it twice.
3. how: the part that passes messages between agents is written in Bend. its state is exactly a replay of a journal, so a restart rebuilds it.
4. and there are proofs, checked on every change: a message is never overwritten, never removed, delivered only once; stopping an agent leaves no message and no card behind.
5. honest line: "bend's checker says ALL PROOFS CHECK. the extra check with Lean hasn't run yet."

facts from docs/bend-laws-report.md. say "proofs checked by Bend", never "formally verified end to end".

---

## 3. cadence

4-5 posts a day works if they're different kinds of posts. five promo links a day, and people mute you. each day: one capsule or clip (the main post), and small posts around it.

| paris time | kind |
|---|---|
| 10:00 | a one-line hook, no link (the pain, a joke, a question) |
| 15:30 | **the main post of the day** (capsule / clip). US morning |
| 18:30 | a short clip or GIF (10-20 s) |
| 22:00 | behind the scenes, a number, or a quote of a user (with their ok) |
| anytime | quote-tweet people who try it. these count most |

| day | main post (15:30) |
|---|---|
| wed | launch thread (16:00) |
| thu | capsule 1 · the thread that runs itself |
| fri | capsule 3 · every MCP, always on |
| sat, sun | 1-2 light posts a day (weekend) |
| mon | capsule 2 · memory without a memory system |
| tue | capsule 4 · the little things |
| wed | capsule 5 · restart whenever (Bend) |

---

## 4. the post bank (for 10:00, 18:30, 22:00)

one-line hooks:
- how many agent sessions do you have open right now? be honest.
- every "should i continue?" costs you your train of thought. bise asks once, in an inbox, when you're ready.
- you're not the router. main answers the obvious questions itself and tells you why.
- no yaml was harmed in the making of this multi-agent setup.
- the models grew up. you can stop babysitting.
- five agents. five tabs. one very tired you.
- the composer is never locked. interrupt all you want.
- ramble. interrupt. change your mind. i run the agents.
- no new chat. ever. one thread per repo.
- your token bill can relax: an agent only starts when a job needs one. no committee of agents reviewing each other.
- bise /beez/ · french, n. 1. a quick kiss on the cheek :* 2. a brisk north wind 3. a terminal where multi-agent coding is painless

clips (10-20 s, one sentence):
- the inbox: a card waits, ctrl+g, one key. "agents wait for you. the other way around was exhausting."
- ✓✓: you steer mid-turn. "yes, it read your message."
- ✉ between agents: "they talk behind your back. in a good way."
- ψ appears, then goes: "a worktree when it helps, gone after."
- hold ctrl: "no cheat sheet."
- the ∿ loader in slow motion: "we spent way too long on this wave."
- the site's hero: agents branching like git.

behind the scenes:
- the repo history as a timelapse: "most of bise was built with bise." (real numbers only)
- the design of :* and ∿ (ask designer for the before/after)
- the desktop screens: "want it? tell me."
- day 1 numbers, day 7 numbers (real ones only).
- "the bug a user found on day 1, and the fix" (with their ok).
