# bise · launch plan · v1

one main post, then a steady stream: a feature, the big idea told another way, a look behind the scenes. every post lands on the same promise, so people hear it ten times without reading the same thing twice.

**the promise, in one line:** one thread per repo. you talk, bise runs the agents. you stay in flow.
**the pain, in one line:** you didn't become an engineer to babysit robots.
**its name:** agent fatigue. five sessions talking at once, logs you never asked for, "should i continue?" all day. most of it isn't for you, all of it lands in your head. use the name everywhere: people will say it back.
**the proof:** the 60 s demo. one thread, three agents, one question waiting in the inbox.

rule for every post: pain → promise → proof. never the tech first ("multi-agent orchestration harness" loses people in 3 words).

---

## launch today · wed sep 30 (paris time)

the 8-week plan below still holds from tomorrow. today is the short version.

### decisions only you can make (before 13:00)

1. **approvals are not built.** agents run commands without asking. my pick: launch and say it plainly (site faq, README, HN comment): "agents run commands without asking today. commit often. approvals are next." devs know yolo modes.
2. **the LGPL glibc bit in the prebuilt V8** (main's note): main's pick is accept for launch, rebuild later.
3. **Mistral's side-project policy**: a quick yes from your manager, today, before any public post.
4. **your X handle** (the site says x.com/GabrielVergnaud).
5. **who you know** on the list in §2: today it's a heads-up, not early access.

### timeline

| time | what | who |
|---|---|---|
| 10:30-12:00 | decisions above. honesty pass on site + README (macOS only, no approvals yet) | you, designer |
| 11:00-13:00 | record the 60 s video (script below). fallback: a screen capture of the landing's flow demo | you |
| 12:00-13:00 | heads-up DMs: Matt Pocock + the people you know. "launching today at 16:00, here's the link, no ask" | you (designer writes each DM) |
| 14:30 | repo public (your click) | you |
| 14:30-15:30 | clean-machine install, no gh token: curl → onboarding with a real key → first message | main |
| 15:30 | go / no-go. site indexable (noindex removed), deploy | designer |
| 16:00 | X thread with the video, then Bluesky, LinkedIn (EN) | you |
| 16:05 | Show HN + your first comment | you |
| 16:30 | r/ClaudeAI, r/commandline: a short story + the video | you |
| 16:00-22:00 | answer every reply within the hour. bugs go to main, live | you, main |
| 22:00 | thank-you post with the first numbers | you |
| thu | LinkedIn in French. first "feature" post (the inbox) | you |

### the 60 s video

0-5 s: `cd ~/app && bise`. the gloss appears.
5-25 s: three ideas typed one after the other, without waiting ("signup is slow on mobile", "the 404 page is sad", "csv export breaks on emoji"). three agents start in the panel.
25-40 s: ⌥2 into one agent, one question, ⌥0 back. main answers a question for you in the history.
40-55 s: ✓ done lines come in. one card in the inbox, answered with one key.
55-60 s: "bise.dev". no voice-over needed, captions only.

## 0. must be true before launch day

| what | today | to do |
|---|---|---|
| the repo is public | private (github.com/gvergnaud/bise is a 404) | make it public on the morning of launch |
| the installer works | bise.dev/install is served; the README says "goes live with the first release" | publish the release, test the one-liner on a clean mac |
| **the site can be found by Google** | **vercel.json sends `X-Robots-Tag: noindex` on every page** | remove that header the day before |
| where it runs | macOS (Apple Silicon) first | say "macOS first" in every post. no surprise, no angry replies |
| cost | open source (Apache-2.0), bring your own key | say it in the main post |
| your employer | you work on Mistral's agent tools; the landing says "one of the people behind Mistral's agent tools, like Vibe" | check the side-project / open-source policy, and ok that line with Mistral comms. then say it once, plainly: a personal project, not a Mistral product |
| "built with bise" numbers | 872 commits in the repo today | pull the real numbers the day before: weeks, commits, agents started. never an estimate |

---

## 1. who should hear about it

| who | where | the angle that lands | hook |
|---|---|---|---|
| people with 5 Claude Code / Codex tabs | X, r/ClaudeAI, HN | the tab pain | "wait, which session was that?" |
| TypeScript devs (your type-level-typescript audience) | X, Bluesky | the TS person built a calm tool for agents | "i spent years on types. now i spend my days babysitting agents. so i fixed that" |
| people who build agents | X, HN, Latent Space | the design choices: one thread, programmatic tool calling, every MCP on, no yaml | "no yaml was harmed" |
| solo founders, indie hackers | X, Indie Hackers | a whole product, alone | "a team of a hundred that doesn't need you" |
| terminal lovers | r/commandline, the Ghostty community, HN | a TUI that feels designed | the ∿ loader, zen mode, hold ctrl |
| French tech | LinkedIn, X in French | bise = a kiss on the cheek, made in Paris | "on dit bise, pas bees" |
| design and craft people | X | the brand, the site's animations | the hero clip, the git branches |

---

## 2. people to reach, by hand

rules: a personal DM, never a template. one week before launch. early access, and an ask for honest feedback, not a repost. a 60 s video made for them (a Loom that says their name). on launch day, no ask; if they liked it, they'll say it.

| who | why they'd care | what you offer | warm? |
|---|---|---|---|
| **Matt Pocock** | a huge TS audience, and he now teaches AI engineering (AI Hero). agents that don't eat your flow is his audience's pain | early access, then maybe a live session together: "i built this with it in 3 weeks". ask what he thinks of the one-thread idea | you know him a little |
| Simon Willison | writes up every serious agent tool, loves honest details | the why, plus how it works (programmatic tool calling, every MCP on). he may blog it | ? |
| Mitchell Hashimoto (Ghostty) | bise sets up Ghostty's keys for you and looks its best there | a short note: "we made bise feel at home in Ghostty". no ask | ? |
| Armin Ronacher, Peter Steinberger, Geoffrey Huntley | they post about working with many agents every week: the exact pain | early access, feedback | ? |
| swyx / Latent Space | the "agent UX" story | a guest spot or a mention in the newsletter | ? |
| ThePrimeagen, Theo | huge reach, and they love to roast. only once it's solid | nothing before launch. if it takes off, they'll find it | cold |
| Josh Comeau | the site's animations owe him a lot | a thank-you, no ask | ? |
| French: Grafikart, Underscore_, Korben | the French dev scene; bise is a French word | early access, an interview in French | ? |
| Mistral colleagues | a warm first circle on launch day | a heads-up the day before, from their personal accounts (after the policy check) | warm |

tell me who you know on this list: i'll write each DM.

---

## 3. before launch (T-14 → T-1)

- **T-14 · private beta.** 20-30 devs: friends, colleagues, 5 names from the list. goals: 3 quotes you can use, 3 bugs fixed, 1 clip that makes people say "wow".
- **T-10 · teasers, no link.** the gloss card ("bise /beez/ · french, n. 1. a quick kiss on the cheek :* 2. a brisk north wind 3. …"), the ∿ loader, a poll: "how many agent sessions do you have open right now?"
- **T-7 · the DMs.** early access for the people above.
- **T-3 · record and write.** the video, the gifs, every launch-day post, all scheduled.
- **T-1 · the checks.** pull the numbers, remove noindex, test the install on a clean mac, write the HN comment.

---

## 4. launch day

a tuesday or a wednesday. check the calendar first: no big AI launch that day (OpenAI, Anthropic, Google dev days).

| paris time | what |
|---|---|
| 15:00 | repo public, release published, site indexable |
| 16:00 (10:00 in New York) | the main X thread with the video. the same day: LinkedIn, Bluesky |
| 16:05 | Show HN, then your first comment (the why, what isn't done yet, what you'd love feedback on) |
| 16:30 | r/ClaudeAI and r/ChatGPTCoding: a story post with the video, not a link dump |
| all day | answer every reply and comment within the hour. that is the real launch |
| 22:00 | a thank-you post with the first numbers |

Product Hunt: not on day 0. in week 2 or 3, with the desktop app as a second moment.

---

## 5. the main post · drafts

### X thread

> 1/ i built the terminal app i wanted for coding with agents.
> one thread per repo. you talk, it runs the agents. you stay in flow.
> it's called bise :* (french for a quick kiss on the cheek)
> [the 60 s video]

> 2/ agents made me faster and my days emptier. five tabs. "should i continue?" eleven times a day. the same context pasted three times. i shipped more than ever and felt like i did nothing.

> 3/ with bise you only talk to main. main starts an agent when a job needs one, follows it, and taps your shoulder only when it matters. you keep talking while they work.

> 4/ the agents talk to each other before they collide. when one needs its own copy of the repo, it makes a worktree and cleans it up after.

> 5/ what needs you waits in an inbox above your message. never a popup in the middle of your sentence.

> 6/ every MCP and every skill, always on (programmatic tool calling keeps the context small). bring your own key. open source, Apache-2.0. macOS only for now. no approval mode yet: agents run commands without asking, so commit often.

> 7/ most of bise was built with bise.
> curl -fsSL bise.dev/install | sh
> bise.dev

### the Type-Level TypeScript newsletter

the warmest list you have: people who already trust you, and who write TypeScript all day. send it on launch day at 16:15, right after the X thread. ask them to try it and to reply. never ask for HN upvotes (HN buries posts that get vote rings).

subject: **i built something new: bise :***

preview text (Kit shows it after the subject; keep it under ~90 characters):
- a terminal where multi-agent coding is painless. open source, out today. ← pick
- one thread per repo. it runs the agents, you stay in flow.
- i stopped babysitting my agents. here's what i built instead.

> hi! it's Gabriel, from Type-Level TypeScript.
>
> it's been a while. i've been building something, and it's out today.
>
> it's called bise :* (french for a quick kiss on the cheek). it's a terminal app for coding with agents.
>
> agents made me faster, and my days emptier. five tabs, "should i continue?" all day, the same context pasted three times. at 6pm i'd shipped more than ever and felt like i did nothing.
>
> with bise, you talk to one thread per repo. it runs the agents for you, and only taps your shoulder when it matters. you keep talking while they work. the agents talk to each other before they collide.
>
> a detail you'll like: bise's agents call their tools by writing TypeScript. every MCP server you connect becomes functions under `tools.*`, and the agent writes a small script to use them. that's how you can keep a hundred servers on without filling its context.
>
> it's open source (Apache-2.0), macOS only for now, and early: there's no approval mode yet, so commit often.
>
> curl -fsSL bise.dev/install | sh
> then `cd` into a repo and run `bise`.
>
> if you try it, hit reply and tell me what felt good and what broke. i read every answer.
>
> :*
> Gabriel
>
> p.s. most of bise was built with bise.

### Show HN

title: **Show HN: bise – one thread per repo, it runs the coding agents for you**

first comment: who you are (one line), the pain (three lines), the 3 design choices (one thread and an always-listening main, agents that message each other, an inbox instead of interruptions), what's not done (macOS only, no approval mode yet, pre-release), what you want to hear. no superlatives.

### LinkedIn

the same story, longer and in first person: the day at 6pm when you'd shipped a lot and felt empty, then the fix. end on the desktop question ("want a desktop app? tell me") to get comments. one version in English, one in French a day later.

---

## 6. after launch · 8 weeks, 3 posts a week

rhythm:
- **monday · the big idea, told another way.** one audience per post.
- **wednesday · one feature.** a 15-30 s clip, one sentence, no thread.
- **friday · behind the scenes or the community.** a number, a design decision, a user's quote, a question.

### the big idea, 10 ways

| # | angle | for | hook |
|---|---|---|---|
| 1 | flow | everyone | "every 'should i continue?' costs you 20 minutes of focus. bise asks once, in an inbox, when you're ready" |
| 2 | the tabs | Claude Code / Codex users | "wait, which session was that?" · the landing's before/after animation |
| 3 | the boss | managers, leads | "you're the boss. main is your team lead. the agents never ping you directly" |
| 4 | tokens | the cost-conscious | "no committee of agents reviewing each other. bise starts an agent when a job needs one. that's the whole rule" |
| 5 | solo founder | indie hackers | "you can build a whole product on your own now. you just need your head back" |
| 6 | a day | everyone | 09:02 → 18:00 in one thread, told as a story |
| 7 | no config | agent builders | "no yaml was harmed. no roles to write, no graph" |
| 8 | the craft | designers | "made by a human": the brand book, the ∿, the kiss |
| 9 | the word | the French and the curious | "bise /beez/: a kiss on the cheek. also a brisk north wind" |
| 10 | built with itself | builders | the repo's history as a timelapse (gource), and the numbers |

### features, one clip each

| feature | the clip shows | hook |
|---|---|---|
| the inbox | an agent's question waits above your message; ctrl+g, 2, done | "agents wait for you. not the other way around" |
| agents talking to each other | ✉ perf → dark-mode: "are you touching tokens.css?" | "they talk behind your back. in a good way" |
| worktrees only when needed | ψ appears, the agent finishes, ψ goes | "a worktree when it helps, cleaned up after" |
| ✓✓ read receipts | you steer mid-turn, ✓ then ✓✓ | "yes, it read your message" |
| calls in one line | $ and ƒ rows written in your language; ctrl+o opens them all | "it tells you what it's doing, in your words" |
| select and ask | select a line in the history, type your question | "point at it and ask" |
| paste a screenshot | ctrl+v, "why is this ugly?" | |
| voice | ctrl+r, you ramble, it's text | "ramble. interrupt. change your mind" |
| zen | the edges fade while you type | "the interface steps back while you think" |
| hold ctrl | the shortcuts show up where they work | "no cheat sheet" |
| every MCP on | web search, GitHub, Slack in one ask | "no menu. everything's on" |
| the forever thread | the same thread on monday and in june; compaction | "no new chat. ever" |
| setup, by the hand | the first inbox item sets up Ghostty and an AGENTS.md | "it sets itself up, and asks first" |
| the ∿ | the loader in slow motion, the brand story | the design angle |
| the site | the hero: agents branching like git | a clip of bise.dev |
| the desktop app | the screens | "want it? tell me" |

### behind the scenes

the numbers every friday (stars, installs, agents started), a design decision with its before/after (✓ instead of ♡, the inbox keys), a user's clip, the week's bugs and fixes, the desktop app's replies.

### 8-week calendar

| week | mon (idea) | wed (feature) | fri (behind the scenes) |
|---|---|---|---|
| 1 | launch day (tue) | the inbox | first numbers + thanks |
| 2 | #2 the tabs | agents talking to each other | the "built with bise" timelapse |
| 3 | #4 tokens | calls in one line | the desktop app screens + "tell me" |
| 4 | #5 solo founder | select and ask | a user's clip |
| 5 | #3 the boss | voice | the design of the ∿ |
| 6 | #6 a day | every MCP on | Product Hunt? or the long essay |
| 7 | #7 no config | worktrees | numbers + roadmap |
| 8 | #1 flow | zen + hold ctrl | "what's next": the desktop decision |

---

## 7. to keep the buzz going

- **the site is a clip.** the hero, agents branching off like git and coming back as ✓, is a 10 s loop that explains the product without a word. post it alone.
- **the timelapse.** a gource video of the repo's history: "most of bise was built with bise", with real numbers.
- **the poll.** "how many agent sessions do you have open right now?" the answers are the pain, in public.
- **the desktop app as a second launch.** week 3: the screens and "want it? tell me". the replies are both reach and a signal.
- **make :* a thing.** people reply with :* ; "send a bise"; an animated :* sticker / emoji.
- **a talk.** submit "you didn't become an engineer to babysit robots" to Paris TypeScript and AI Tinkerers Paris; clip it after.
- **the long essay.** "why i built bise" on your blog or dev.to, a week after launch: a second shot at HN.
- **newsletters.** Console.dev (they review dev tools), Changelog News, Bytes.dev, JavaScript Weekly, TLDR, Latent Space.
- **open source done right.** "good first issue" labels, thank every contributor by name in the release notes, an answer to every issue within 24 h (the landing promises "i read every issue").

---

## 8. assets to make

| asset | length | note |
|---|---|---|
| the main video | 60-90 s | script: the 6pm feeling (10 s) → one message, three agents start (20 s) → you keep talking, they talk to each other (20 s) → the inbox question, 2, done (10 s) → ✓ perf done (5 s) → "one thread per repo. you stay in flow. bise.dev" |
| gifs | 6 × 10-20 s | inbox, ✉ between agents, ✓✓, zen, hold ctrl, the site's hero |
| images | 3 | the gloss card, light and dark screenshots, the og image (exists) |
| DM videos | 60 s each | one per person, with their name |
| the timelapse | 30 s | gource on the repo |

---

## 9. what we measure

GitHub stars, installs (hits on bise.dev/install in Vercel), HN rank and comments, X impressions and replies, replies to the desktop question, issues opened. a short note every friday: what worked, what we change.

## 10. risks

- **drowned in AI news** (your friend's point). the answer: one sentence, pain first, the video does the talking. calm is rare in AI launches: that's the edge.
- **"another wrapper".** lead with the design choices, and be honest in the first HN comment.
- **the install breaks on day 0.** test on a clean mac the day before; `bise doctor` in the README.
- **claims.** macOS first, pre-release, bring your own key. no number we didn't measure.
- **your employer.** check the policy first (§0).

## questions for you

1. the launch date?
2. who on the list (§2) do you know, and how well? i write each DM.
3. your handles: X (the site links x.com/GabrielVergnaud), Bluesky, LinkedIn?
4. posts in French too, or English only with one French moment (LinkedIn)?
5. the video: you on camera for 10 s at the start, or screen only?
