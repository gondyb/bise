# handoff · bise, its brand, and its launch

You are picking up from another agent (the "designer") who worked with me, Gabriel Vergnaud, for several days on the brand, the website and the launch of **bise**, my open-source terminal app. I want to keep brainstorming the release with you.

**Your job, in order:**
1. Build a **knowledge base** about bise from everything below: product, brand, voice, site, launch. Organize it into clear pages: product facts; brand and voice; copy bank; launch plan; people and channels; open decisions. Keep every fact exactly as written here. Don't invent features, numbers or quotes.
2. Then **brainstorm release ideas with me**: posts, hooks, video ideas, stunts, collaborations, content series, ways to get the right people talking.

**Ground rules:**
- Talk to me in **French**, and keep it short. Write all marketing copy in **English**, in the bise voice (see §4).
- Stay honest. If an idea needs a claim that isn't in the facts below, flag it as "to verify". Never present something unfinished as done.
- Give me options with a pick, not essays. I like playful, human, a bit risky. I hate anything that sounds like an LLM wrote it.

---

## 1. what bise is

**One line:** bise is a terminal app for coding with many AI agents at once, without the mental load. You talk to one thread per repo. It runs the agents for you, and only taps your shoulder when it matters.

**Name:** *bise* /beez/ · french, n.
1. a quick kiss on the cheek :*
2. a brisk north wind
3. a terminal where multi-agent coding is painless

The double meaning (a kiss, a wind) runs through the whole brand: the `:*` kiss emoticon, soft wind and ASCII animations.

**Who made it:** Gabriel Vergnaud. He builds coding agents for a living. (No employer name anywhere: bise is its own indie brand.) He's the author of Type-Level TypeScript (a course with a newsletter). bise is **its own indie brand, not a Mistral product**. Most of bise was built with bise.

**Links:**
- site: https://bise.dev
- brand book: https://bise.dev/book/
- every screen: https://bise.dev/book/screens
- desktop app proposal: https://bise.dev/book/desktop
- repo: https://github.com/gvergnaud/bise (private until launch, goes public on launch day)
- install: `curl -fsSL bise.dev/install | sh`, then `cd` into a repo and run `bise`

## 2. product facts (true today)

**The model:** one thread per repo. You talk to **main**, bise's main agent (the orchestrator, your team lead). Main splits your ideas into jobs and starts **agents** in the background. The thread never ends: when it gets long, bise compacts it, so there are no sessions to manage.

**The four words a user learns:** you, main, agents, cards.

**The 4 big ideas (landing "how" section):**
1. **one thread per repo.** peek into any agent when you feel like it. no tabs, no "wait, which session was that?". main knows everything going on in the repo.
2. **main is always listening.** the team is busy, main never is. ask, add a thing, change your mind while agents work.
3. **a team that runs itself.** "they talk behind your back. in a good way." Agents message each other before they collide. They make a git worktree only when one needs its own copy, and clean it up after. Each one commits its own work.
4. **forget there are agents.** "no yaml was harmed." No roles, no graph, no config. bise is opinionated.

**Features (the landing's "ten things you'll notice in the first hour"):**
- talk whenever: the composer is never locked
- agents sync on their own
- all your MCPs, all your skills, always on: agents call tools by writing TypeScript (programmatic tool calling), so a hundred MCP servers don't fill the context
- only the real decisions reach you: cards in an inbox above your message, answered with one key (ctrl+g opens the inbox)
- you're not the router: main answers the obvious questions itself and tells you why
- talk to any agent, anytime: ⌥ + a number, or @name
- change your mind mid-run: a correction lands in the running agent (✓ got it, ✓✓ read it)
- nothing gets dropped: when an agent stops half-way, main picks the work back up
- worktrees only when they help
- bring your Agent Plugins: skills, MCP servers, hooks; Vibe plugins too

**The small things ("polished down to the last character"):**
- your token bill can relax: an agent only starts when a job needs one, there's no committee of agents reviewing each other
- it's quiet, open it up whenever: ctrl+o shows everything folded (every message between agents, every tool call, every script, in full)
- voice: /voice, then ctrl+r
- paste screenshots with ctrl+v
- select lines in the history and start typing: they come along as a quote
- a model per agent (/model, /reasoning)
- a real shell one key away (ctrl+`)
- hold ctrl to see the shortcuts (Ghostty, kitty)
- restart whenever you like: update mid-work or quit and come back tomorrow, and no work is ever lost
- agent communication is proven correct: the part that passes messages between agents is written in Bend, with proofs (no message lost, none sent twice). *To verify before hyping it: `bend PROOF.bend` prints ALL PROOFS CHECK, but the full check with Lean has never run.*
- your terminal: your colors, light or dark, text at a reading width

**Limits, said plainly everywhere:**
- macOS only for now (Apple silicon and Intel). Linux next, no date. Windows later, maybe through WSL.
- **No approval mode yet:** agents run shell commands in your repo without asking. Use it on repos you trust, and commit often. Approvals are next.
- Bring your own API key: Anthropic, OpenAI, Mistral and more.
- Needs git. Pre-release. Open source, Apache-2.0.

**FAQ answers already on the site:**
- "but i already have Claude Code / Codex / Vibe": keep your models, MCPs and skills. bise changes how you work with them.
- context overflow: bise compacts, and the thread keeps going.
- memory system: not needed.
- how many agents: as many as you want. The limits are your tokens, your budget and your provider.

## 3. the story (positioning)

**The pain has a name: agent fatigue.** Agents made engineers faster and their days emptier. You juggle five tabs. You answer "should i continue?" eleven times a day. You paste the same context three times. Two agents edit the same file. At 6pm you've shipped more than ever and feel like you did nothing. You're the router, the memory and the merge tool, and you never get to think. You never reach flow.

**The promise:** you stay in flow. You ramble, interrupt, change your mind. bise runs the agents. You ship like a team of a hundred and love building again.

**Landing "why" title:** "you didn't become an engineer to babysit robots."
Its closing line: "with bise, you only talk to one thread. it runs the agents, and taps your shoulder when it matters."

**Against the others:** Claude Code, Codex, Cursor and Vibe are one agent per session. bise is one thread and a team behind it. It isn't a new model: you keep yours.

## 4. brand and voice

**Voice:** a human typing fast. Warm, playful, a bit cheeky, confident, never corporate.
- **All lowercase,** always. Proper nouns and acronyms keep their capitals (MCP, Ghostty, Claude Code, macOS). ALL CAPS only to shout, very rarely.
- No final periods on titles.
- Never "X, not Y" constructions. No LLM-isms ("seamless", "unleash", "never deleted", neat triplets that sound generated).
- bise speaks as "i": "i run the agents".
- Short sentences. Concrete examples over abstractions. Small easter eggs for people who read everything.

**Taglines:**
- **main:** "multi-agent coding, made human."
- **hero lines:** "one thread per repo. you talk, i run the agents. no tabs. no babysitting. you stay in flow."
- **section and secondary lines:**
  - "kiss your backlog goodbye"
  - "the models grew up. you can stop babysitting"
  - "one thread. a whole team"
  - "five agents. five tabs. one very tired you"
- **sign-off:** "ideas in. little kisses out. also pull requests." :*
- **liked, kept in reserve:**
  - "ramble. interrupt. change your mind. i run the agents. you stay in flow."
  - "don't mind me. i'm just shipping your whole roadmap"
  - "i blow through backlogs."
  - "you think. i ship."
  - "shh. you think. i'll ship."
- **rejected:**
  - "your ideas. my hands. lots of them." (doesn't carry the philosophy)
  - "All the agents. None of the overhead." ("overhead" is unclear)
  - "built for engineers who think faster than they type" (not strong enough)
  - anything built on "10x" or a number of agents: there's no limit

**Visual identity:**
- dark palette: bg #141211, text #ece6da, dim #8f887d, faint #3d3935, accent pink #f4a6b0. Also light themes.
- JetBrains Mono everywhere on the site. The desktop app proposal uses Geist for reading text.
- marks:
  - `:*` bise / main
  - `∿` working (animated wave `·~∿≈∿~`)
  - `✓` done (small pink check)
  - `?` needs you
  - `ψ` worktree
  - `✉` a message between agents
  - `$` bash, `ƒ` TypeScript
- hero animation: an ASCII sky with dandelion seeds and wind. A thread runs across it. The `:*` kiss walks it and sends agents out like git branches: grey trails, a breathing ∿ head. When an agent comes back, the arc blows apart into seeds, with small lines like "spawning · feat: dark mode" and "✓ shipped · fix: sad 404 page".
- demo examples (in-brand, a little funny): "signup is slow on mobile", "the 404 page is so sad" → "keep the dog. just give it a hat", "the csv export crashes on emoji 😭" → "it's always unicode", "which gray for the borders?" → "the one in tokens.css, like everywhere else", "OK LAST ONE. release notes for all of this", signup 4.1 s → 0.9 s.

## 5. the site (bise.dev), in order

1. hero
2. the gloss
3. why
4. the problem, with the "this is what flow looks like" animated demo
5. how (the 4 ideas + an org chart: you → main → agents)
6. features
7. and so much more
8. FAQ
9. "made by a human. shipped with bise" ("hi, i'm gabriel…")
10. install
11. "next, maybe · want a desktop app?", with the CTA "i want it · tell me on X"

Other assets:
- the README (animated SVGs, same copy)
- an OG share image
- an X header (the hero sky)
- the brand book site

## 6. launch

**Status:** launching **today, wednesday sep 30** (paris time).

| paris time | what |
|---|---|
| 14:30 | repo goes public |
| 14:30-15:30 | clean-machine install test |
| 15:30 | go / no-go, site becomes indexable |
| 16:00 | X thread with a 60 s video, then Bluesky and LinkedIn |
| 16:05 | Show HN, "Show HN: bise – one thread per repo, it runs the coding agents for you", plus a first comment |
| 16:15 | Type-Level TypeScript newsletter via Kit |
| 16:30 | Reddit (r/ClaudeAI, r/commandline) |
| until 22:00 | answer every reply within the hour |
| 22:00 | thank-you post with the first numbers |
| thursday | LinkedIn in French |

**Newsletter:**
- subject: "i built something new: bise :*"
- preview: "a terminal where multi-agent coding is painless. open source, out today."
- angle for TS devs: agents call their tools by writing TypeScript.
- the ask: reply with feedback. Never ask for HN upvotes.

**60 s video script:**
1. `cd ~/app && bise`
2. three ideas typed back to back, three agents start
3. ⌥2 into one agent, then back
4. ✓ lines arrive, one card answered with one key
5. end on "bise.dev". Captions, no voice-over.

**X thread draft (7 posts):**
1. "i built the terminal app i wanted for coding with agents." + the video
2. the agent fatigue story
3. one thread, main taps your shoulder
4. agents talk to each other, worktrees only when needed
5. the inbox instead of popups
6. every MCP and skill always on, BYO key, Apache-2.0, macOS only, no approvals yet
7. built with bise + the install line

**People to reach (a personal DM each, a heads-up, no ask for reposts):**

| who | notes |
|---|---|
| Matt Pocock | Gabriel knows him a little. TS audience, AI Hero. Idea: a live session together. |
| Simon Willison | |
| Mitchell Hashimoto | bise feels at home in Ghostty |
| Armin Ronacher, Peter Steinberger, Geoffrey Huntley | |
| swyx / Latent Space | |
| Josh Comeau | a thank-you, the site's animations owe him a lot |
| Grafikart, Underscore_, Korben | the French scene |
| Mistral colleagues | a warm circle, from personal accounts |
| ThePrimeagen, Theo | only if it takes off on its own |

**After launch: 8 weeks, 3 posts a week:**
- monday: the big idea, reframed ten ways (agent fatigue, flow, "you're not the router"…)
- wednesday: one feature, one short clip
- friday: behind the scenes (built with bise, the Bend proofs, the design process, the brand book)
- Product Hunt in week 2-3, with the desktop app as a second moment

**Desktop app proposal:** the same bise in a window. Repos in a sidebar, one thread each, agents on the right, notifications only for the inbox, ⌘ shortcuts. The screens are drawn. It gets built only if people ask for it: it's the CTA that sparks conversation.

## 7. open decisions (ask me, don't assume)

- launch without approvals, and say it plainly? (the designer's pick: yes)
- accept the LGPL glibc bit in the prebuilt V8 for launch?
- a green light under Mistral's side-project policy
- my exact X handle (the site uses x.com/GabrielVergnaud, unverified)
- which people on the list I actually know
- whether I appear in the video, and whether to post in French too

## 8. what I want from you now

1. Build the knowledge base (§1-7), and show me its outline first.
2. Then give me **15 release ideas**, sorted by effort and impact. Mix quick hooks for today and content series for the next weeks. For each, give one line on why it would spread and who would share it.
3. Then write the pieces I pick, in the bise voice.
