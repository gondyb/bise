<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/hero-dark.svg">
    <img src="projects/switchboard/docs/brand/readme/hero-light.svg" width="860" alt="bise :* · multi-agent coding, made human.">
  </picture>
</p>

<p align="center">
  <b>bise</b> /beez/ · french, n.<br>
  1. a quick kiss on the cheek :*<br>
  2. a brisk north wind<br>
  3. a terminal where multi-agent coding is painless
</p>

<p align="center">
  <a href="https://bise.dev">bise.dev</a> ·
  <a href="https://bise.dev/book/">the brand book</a> ·
  <a href="#install">install</a>
</p>

<br>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/demo-dark.svg">
    <img src="projects/switchboard/docs/brand/readme/demo-light.svg" width="860" alt="a bise session: five ideas in a row to main, five agents start, they sync, you change your mind, one card asks you, everything ships.">
  </picture>
</p>

## you didn't become an engineer to babysit robots

at 6pm you'd shipped more than ever, and you felt like you did nothing.

it's not you. it's agent fatigue. you jump between tabs a hundred times a day. every agent talks to you, about everything, all the time.

**with bise, you only talk to one thread. it runs the agents, and taps your shoulder when it matters.**

## five agents. five tabs. one very tired you

every session waits for you. you're the router, the memory, and the merge tool. you ship a lot, but you never get to think.

## one thread. a whole team

bise is a terminal app. you talk to **main**, bise's main agent. main splits your ideas into jobs and runs the other agents.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/team-dark.svg">
    <img src="projects/switchboard/docs/brand/readme/team-light.svg" width="860" alt="you talk to main; main splits one idea into four jobs; the agents tell each other what they touch.">
  </picture>
</p>

| | |
|---|---|
| **one thread per repo** | one thread to talk to. peek into any agent when you feel like it. no tabs to juggle, no "wait, which session was that?". main knows everything going on in the repo, even what you did last month. |
| **main is always listening** | the team is busy. main never is. ask, add a thing, change your mind while the agents work. main answers right away. |
| **a team that runs itself** | they talk behind your back. in a good way. agents message each other before they collide. when one needs its own copy of the repo, it makes a worktree and cleans it up after. each one commits its own work. |
| **forget there are agents** | no yaml was harmed. no roles, no graph, no config. bise has opinions, so you don't need any. you just dump your ideas in the thread. |

## kiss your backlog goodbye

the models grew up. you can stop babysitting. ten things you'll notice in the first hour:

- **talk whenever. you never wait** · say the next thing while the last one runs. your composer is never locked.
- **agents sync on their own** · every agent can talk to every other one. they ask, share and hand off quietly, folded out of your way.
- **all your MCPs. all your skills. always on** · GitHub, Linear, Sentry, Slack, your docs, your database: connect as many MCP servers as you want, and never pick which ones to turn on. bise calls tools through code, so a hundred servers don't fill its context.
- **only the real decisions reach you** · one card, a couple of choices, one key. then back to what you were doing.
- **you're not the router** · main answers the obvious questions itself, the way you would, and tells you why.
- **talk to any agent, anytime** · `⌥` + a number, or `@name`. ask it why, push it, then go back to main. nobody has to stop.
- **change your mind mid-run** · a correction lands in the running agent. ✓ it got it, ✓✓ it read it.
- **nothing gets dropped** · when an agent stops half-way, main picks the work back up from where it was.
- **worktrees, only when they help** · agents share your folder. when one really needs isolation, it makes a worktree, and cleans it up after.
- **bring your Agent Plugins** · skills, MCP servers, hooks: plugins in the [Agent Plugins](https://agent-plugins.org) format load as they are, from `~/.agents/plugins` or your repo. Vibe plugins too.

<details>
<summary><b>and so much more</b> · polished down to the last character</summary>

<br>

hundreds of tiny details. you'll feel them before you see them.

- **your token bill can relax** · bise starts an agent when a job needs one. that's the whole rule. there's no committee of agents reviewing each other in circles.
- **it's quiet. open it up whenever** · what matters to you shows. the rest is one key away (`ctrl+o`): every message between agents, every tool call, every script, in full.
- **or just talk** · turn on `/voice`, press `ctrl+r`, say it.
- **show it a screenshot** · paste it with `ctrl+v` or drag it in. an image is one chip in your text.
- **ask about anything on screen** · select a few lines in the history and start typing: they come along as a quote.
- **a model per agent** · the big one for the hard job, a fast one for the chores. `/model` and `/reasoning`.
- **a real shell, one key away** · ``ctrl+` `` opens a terminal in your repo. it keeps running while hidden.
- **hold ctrl to see the shortcuts** · each one shows up where it works. in Ghostty and kitty.
- **restart whenever you like** · update to the latest version mid-work, or quit and come back tomorrow. no work is ever lost: your agents resume right away, and your thread, your draft and your queue come back too.
- **agent communication is proven correct** · we have the proof, thanks to [Bend](https://github.com/HigherOrderCO/Bend): no message lost, none sent twice. see `bend/PROOF.bend`.
- **your terminal** · your colors, light or dark, text at a reading width.

</details>

<details>
<summary><b>questions</b></summary>

<br>

**but i already have Claude Code / Codex / Vibe**<br>
keep your models, MCP servers and skills. bise changes how you work with them: one thread, and a team behind it.

**won't the agents step on each other?**<br>
they tell each other what they touch before they touch it. worktrees only when one needs its own copy, cleaned up after.

**won't my context overflow? i don't want to live in the dumb zone**<br>
no need to overthink it. when a thread gets long, bise compacts it. the latest models are trained for that and they keep their trajectory. your thread just keeps going.

**do i need a memory system?**<br>
nope. compaction carries the thread, and any agent can go back and read the full history: every message, every tool call. nothing gets lost.

**how many agents can i run?**<br>
as many as you want. the real limits are your tokens, your budget and your provider.

**which models?**<br>
bring your own key.

**do i need to learn anything?**<br>
four words: you, main, agents, cards. main is the one you talk to. the rest you pick up in the first hour.

**open source?**<br>
yes, under Apache-2.0. read it, fork it, send a :*.

</details>

## install

> [!NOTE]
> bise is pre-release. macOS only for now (Apple Silicon and Intel).
> there's no approval mode yet: agents run commands without asking you. git is your safety net, so commit often. approvals are next.

```sh
curl -fsSL bise.dev/install | sh
```

then go to the repo you want to work on, and start it there:

```sh
cd ~/your-repo
bise            # the first run walks you through a theme, a model key and the folder
bise doctor     # checks your install, keys and running hubs
```

from source:

```sh
git clone https://github.com/gvergnaud/bise && cd bise
./run.sh                            # builds and starts bise in the current folder
```

## what's in here

| | |
|---|---|
| [`rust/`](rust/) | the app: the terminal UI, the agent harness, plugins, sessions |
| [`bend/`](bend/) | the agent runtime and the Switchboard hub, written in [Bend](https://github.com/HigherOrderCO/Bend), and their laws (`LAWS.bend`, `PROOF.bend`) |
| [`prompts/`](prompts/) | the system prompts and tool descriptions the agents read |
| [`scripts/`](scripts/) | dev scripts: build the Bend binaries, build and switch versions |
| [`projects/switchboard/`](projects/switchboard/) | design docs, packaging, tests |
| [`projects/switchboard/docs/brand/`](projects/switchboard/docs/brand/) | the brand book, the site, these images |

## made by a human. shipped with bise

hi, i'm gabriel. i build coding agents for a living.

agents made me faster. they also made my days feel empty. i was switching tabs, answering "should i continue?", pasting context around. i shipped more than ever and i didn't enjoy it.

so i built the tool i wanted: one thread, a team behind it, and me in flow. most of bise was built with bise.

i want everyone to ship like a team of a hundred, and to love building again. bise is open source and indie. i read every issue.

— [Gabriel Vergnaud](https://github.com/gvergnaud)

## next, maybe: a desktop app

the same bise, in a window. your repos in a sidebar, one thread each. the agents on the right. a notification only when something needs you. [i drew the screens](https://bise.dev/book/desktop). if enough of you want it, i'll build it: tell me on [X](https://x.com/GabrielVergnaud) or in an [issue](https://github.com/gvergnaud/bise/issues).

## license

Apache-2.0, see [LICENSE](LICENSE). third-party components: [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).

<p align="center"><br><b>ideas in. little kisses out. also pull requests.</b> :*</p>
