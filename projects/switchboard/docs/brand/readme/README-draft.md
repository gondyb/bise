<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/hero-dark.svg">
    <img src="projects/switchboard/docs/brand/readme/hero-light.svg" width="860" alt="bise :* · a multi-agent harness, made for humans.">
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
  <a href="#install">install</a> ·
  <a href="#features">features</a> ·
  <a href="https://bise.dev/book/">brand book</a>
</p>

<br>

**meet your team lead. you stay in flow, it runs the agents.**

bise is a terminal app for multi-agent coding. there's one thread per repo, and in it you talk to **main**, your team lead. main splits your ideas into jobs, starts an agent when a job needs one, keeps them in sync, and only comes back to you when a decision is yours.

you stop micromanaging agents. no sessions to juggle, no workflow to design, nothing to configure. the multi-agent part is built in, and it has opinions.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/DEMO-dark.svg">
    <img src="projects/switchboard/docs/brand/readme/DEMO-light.svg" width="860" alt="a bise session in the terminal: you send ideas one after the other, main starts an agent for each, they ship.">
  </picture>
</p>

## install

```sh
curl -fsSL bise.dev/install | sh
```

then start it in the repo you want to work on:

```sh
cd ~/your-repo
bise
```

the first run walks you through a theme and a model key. `bise doctor` checks your install, your keys and the running hubs.

switching from Claude Code or Codex? paste this into it:

```text
read https://bise.dev/setup.md and set bise up for me
```

your agent installs bise and brings over what you already have: your API key, your model, your `CLAUDE.md`, skills and MCP servers. it shows you one plan and waits for your yes. it never prints a key. if your agent can't open links, paste [the full prompt](https://bise.dev/setup) instead. a Claude Pro/Max or ChatGPT subscription isn't an API key: bise needs a key.

> [!NOTE]
> bise is pre-release. **macOS only for now** (Apple silicon and Intel). linux is next.
> there's **no approval mode yet**: agents run shell commands in your repo without asking. use it on repos you trust, and commit often. approvals are next.
> bring your own key: Anthropic, OpenAI, Mistral and more.

## how it works

- **one thread per repo.** you always talk to the same thread. it never ends: when it gets long, bise compacts it and keeps going.
- **main is the team lead.** it answers what it can, starts an agent when a job needs one, follows up, and picks up work that stopped half-way.
- **agents work in the background,** in your checkout. they message each other before they touch the same files. one makes a git worktree only when it needs its own copy, and cleans it up after.
- **cards are the decisions that need you.** only the real ones reach you. they wait in an inbox above your message (`ctrl+g`), never in the middle of your sentence.

four words to learn: you, main, agents, cards.

## features

### talk whenever

send the next idea while the last one runs. the composer is never locked.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/talk-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/talk-light.svg" width="680" alt="you send a second idea while the first agent works."></picture>

### agents sync on their own

every agent can message every other one. they ask, share and hand off, folded out of your way.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/sync-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/sync-light.svg" width="680" alt="two agents agree on who edits a shared file."></picture>

### every MCP server, always on

connect as many MCP servers and skills as you want, and never pick which ones to turn on. agents call tools by writing small TypeScript scripts, so a hundred servers don't fill the context.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/tools-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/tools-light.svg" width="680" alt="an agent calls Sentry, Linear and GitHub tools, one row each."></picture>

### only the real decisions reach you

one card, a couple of choices, one key. `ctrl+g` opens the inbox.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/card-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/card-light.svg" width="680" alt="a card asks one question and you answer with one key."></picture>

### quiet by default

what matters to you shows. tool calls and messages between agents fold into one line. `ctrl+o` opens them all, in full, and folds them back.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/quiet-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/quiet-light.svg" width="680" alt="the work between agents stays folded; ctrl+o opens every tool call and message."></picture>

### talk to any agent, anytime

`⌥` + a number, or `@name`. ask it why, push it, then go back to main. nobody has to stop.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/direct-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/direct-light.svg" width="680" alt="you switch to one agent, ask it a question, and go back to main."></picture>

### change your mind mid-run

a correction lands in the running agent. ✓ it got it, ✓✓ it read it.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/steer-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/steer-light.svg" width="680" alt="you correct a running agent and it changes course."></picture>

### nothing gets dropped

when an agent stops half-way, main starts it again from where it was. quit bise, update it, come back tomorrow: your agents, your thread and your draft come back.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/resume-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/resume-light.svg" width="680" alt="an agent stops on a provider error and main restarts it."></picture>

### worktrees, only when they help

agents share your folder. when one really needs isolation, it makes a worktree and cleans it up after.

<picture><source media="(prefers-color-scheme: dark)" srcset="projects/switchboard/docs/brand/readme/feat/worktree-dark.svg"><img src="projects/switchboard/docs/brand/readme/feat/worktree-light.svg" width="680" alt="one agent gets its own worktree; the others share the folder."></picture>

### and also

- **your token bill can relax.** an agent starts when a job needs one. no committee of agents reviewing each other.
- **Agent Plugins.** skills, MCP servers and hooks in the [Agent Plugins](https://agent-plugins.org) format load as they are, from `~/.agents/plugins` or your repo. Vibe plugins too.
- **a model per agent.** `/model` and `/reasoning`.
- **voice.** `/voice`, then `ctrl+r`.
- **images.** paste a screenshot with `ctrl+v`, or drag it in.
- **quotes.** select lines in the history and start typing: they come along as a quote.
- **a real shell.** ``ctrl+` `` opens a terminal in your repo.
- **hold ctrl** to see the shortcuts, in Ghostty and kitty.
- **message passing, proven.** the hub that carries messages between agents is written in [Bend](https://github.com/HigherOrderCO/Bend), with proofs that no message is lost or sent twice (`bend/PROOF.bend`).

## from source

```sh
git clone https://github.com/gvergnaud/bise && cd bise
./run.sh    # builds and starts bise in the current folder
```

## what's in here

- [`rust/`](rust/) · the app: the terminal UI, the agent harness, plugins, sessions
- [`bend/`](bend/) · the agent runtime and the Switchboard hub, written in [Bend](https://github.com/HigherOrderCO/Bend), and their laws (`LAWS.bend`, `PROOF.bend`)
- [`prompts/`](prompts/) · the system prompts and tool descriptions the agents read
- [`scripts/`](scripts/) · dev scripts: build the Bend binaries, build and switch versions
- [`projects/switchboard/`](projects/switchboard/) · design docs, packaging, tests
- [`projects/switchboard/docs/brand/`](projects/switchboard/docs/brand/) · the brand book, the site, these images

## license

Apache-2.0, see [LICENSE](LICENSE). third-party components: [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).

<p align="center"><br>ideas in. little kisses out. also pull requests. :*</p>
