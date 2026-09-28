# Switchboard: pitch and go-to-market

*Version française : [pitch-fr.md](pitch-fr.md).*

Draft, 2026-09. Starting point: what Gabriel says about Switchboard after using
it every day to build it. Claims that aren't true yet are flagged **⚠**.

## 1. The problem

Coding agents are good. Running several of them in parallel is not.

- **You wait.** A turn takes 1 to 10 minutes, and your terminal is stuck the
  whole time. You either watch the output scroll by, or you switch context and
  lose the thread.
- **You babysit.** With 3 agents, you become the router: which terminal is
  doing what, who is waiting on an answer, who half-finished, who needs a
  nudge. The mental load grows with every agent. In practice, you top out at
  2 or 3.
- **You juggle worktrees.** To keep agents from stepping on each other, every
  tool pushes you toward one worktree or VM per task. The result: branches to
  merge, conflicts at the end, dependencies to reinstall, and agents that know
  nothing about each other.

## 2. The promise

> **You talk to one agent. It runs ten. You never wait.**

Taglines (picked by Gabriel):

> **you, but with way more hands.**
> *built for people who think faster than they type.*

**Style rule (Gabriel): no capital letter at the start of any word, ever,
on every marketing piece and all website text.** It reads like a human typing
on a keyboard. Tone: human, casual, first person when it fits; avoid the
polished "X. None of the Y." LLM pattern. Exceptions: proper nouns keep
their capitals (Mistral, Vibe, Claude Code, GitHub); acronyms in caps
(API, MCP), though "cli" in lowercase is fine; ALL CAPS is allowed to shout, extremely rarely. Our own name
stays lowercase: `bise`. (Internal docs like this one keep
normal capitals, except for the copy itself.)

Both picked by Gabriel. Previous headline, kept as an alternative: "your
team is as big as your ideas."

Audience and core message: people with lots of ideas who move fast and need
the tech to disappear and keep up with them. The value is human: flow, no
juggling between agents, no cognitive cost. You're more efficient, you have
more fun, and you ship more. Avoid a number ("ten", "10x"): it sounds like a
cap, and the product has none. **⚠** No limit in the product (RFC 0001
§10.1), but tokens, cost and provider speed are the real limit; say so.

**Name: `bise`** (picked by Gabriel). Short to type, and a double meaning: the
*bise* is a cold north wind (wind = flow), and *faire la
bise* is the French cheek kiss. Checked free on npm, crates.io, PyPI and
Homebrew. **⚠** `bise.sh` and `bise.ai` are taken; `bise.dev`, GitHub org,
trademarks and Linux packages not checked yet. Known namesake: BISE, the EU
Biodiversity Information System for Europe (different field).

**Logo mark: `:*`** (Gabriel), the ASCII kiss: `bise :*`. It works in any
terminal and any font, and looks typed by a human.

**Brand: standalone** (Gabriel's decision). bise is its own brand for now,
unrelated to Mistral: no Mistral visual identity, no Mistral mention in the
copy. **⚠** Sections 4 and 7 (wedge = Mistral devs, Vibe integration) predate
this decision and need a rethink. Open question: the code runs on Mistral's
Unified Harness and Vibe; what can ship under a standalone brand?

Taglines:

Rejected: "All the agents. None of the overhead." ("overhead" is unclear);
"All the agents. Stay in flow."; "Code like a team of ten." (a number).
"All your ideas. None of the juggling." (sounds like an LLM).
Other candidates, human tone: "too many ideas? good."; "say it once. forget
about it. it's done."; "finally, something that keeps up with my brain."

- Never wait on an agent again.
- One conversation. As many agents as you want.
- Stay in flow. Main handles the rest.
- The switchboard for your agents.
- More agents, not more overhead.

## 3. The 5 pillars

**1. You stay in flow: you never wait for a turn to end.**
Main is always available. Every request becomes a task that runs in parallel.
- Before: "run the tests" → 6 minutes staring at the terminal.
- After: "run the tests", then right away "look at that Safari bug", then
  "write the release note". Three tasks are running, and you've already moved
  on.

**2. You don't know who's doing what, and you don't need to.**
Main routes your messages to the right task and keeps the board of every task,
even across compaction. You can step into a task (`Enter`) and back out
(`Esc`), but you don't have to.
- Before: 5 terminal tabs, a mental sticky note per agent.
- After: "where's the Safari fix at?" → main answers, with the task's real
  state.

**3. Main answers the obvious questions itself.**
When a task asks "API v1 or v2?" and the answer is in the brief or the repo,
main answers. Only real decisions reach you, as a card.
- Before: 12 interruptions an hour, 10 of them trivial.
- After: 2 cards, the 2 that matter.

**4. Main sends unfinished work back.**
Main reads the task's report. If it's incomplete or wrong, main sends the task
back to work with precise instructions, before bothering you.
- Before: "done!" … except the tests fail. You find out 20 minutes later.
- After: main saw the failing tests and sent the task back. You get a real
  "done".

**5. No worktrees: every agent works in the same folder.**
The agents know about each other. They see each other (`sb list`, `sb tasks`),
read each other's threads, and give a heads-up before touching a shared file.
A worktree is still available on request.
- Before: 5 branches, 5 `npm install`s, 5 merges.
- After: one repo, one folder, one linear git history.

**⚠ Honesty about the pillars.** Pillars 3, 4 and 5 come from the prompt and
the model, not from a system guarantee. They work well with a good model, not
always. Pillar 5 has no safety net: detecting files changed by two tasks
(RFC 0001 §10.3) isn't implemented, and we've already had a commit that swept
up another task's work. "As many as you want" is not literally true: token cost
and provider speed are the real limit.

## 4. Who it's for, and where to start

- **Target:** developers who already use a CLI agent every day and want
  several running in parallel. They've already felt the "I'm waiting / I'm
  babysitting" pain.
- **Wedge:** Vibe CLI users at Mistral, then Vibe power users outside. Same
  stack, same model, feedback within a day.
- **Not now:** non-developers, teams (multi-user), cloud.

## 5. Proof and demo

Proof points:
- **Built with itself.** Switchboard was developed inside Switchboard: 7 tasks
  running in parallel on the same repo as this doc is written (Bend hub,
  versions, voice, errors, editor, tokens, this pitch).
- **A proven core.** The hub is written in Bend, with machine-checked laws: no
  message is lost, each is delivered exactly once, and no agent sits idle with
  mail waiting. The proofs found real bugs (e.g. a restored task that never
  received its mail). **⚠** The proofs cover message delivery, not agent
  behavior.
- **Updates without stopping anything.** `/restart` rebuilds, restarts
  the hub on probation, and rolls back if it breaks. The agents keep going
  mid-turn.

Video storyboard, 2 minutes:

| Time | Screen | Voiceover |
|---|---|---|
| 0:00 | A dev in front of Claude Code, a progress bar. He waits. | "One agent is great. Waiting on one isn't." |
| 0:10 | Empty Switchboard. He types 3 requests back to back, no waiting. | "You talk to main. Main creates the tasks." |
| 0:30 | Task panel: 3 `working`. A 4th request by voice **⚠ (voice in progress)**. | "You never wait." |
| 0:45 | A task asks a question; main answers it alone, `sb route` line. | "The obvious questions? Main answers them." |
| 1:00 | A card arrives: a real decision. He answers in one word (`Ctrl+A`). | "You only keep the real decisions." |
| 1:15 | A task says "done"; main spots a red test and sends it back. | "Main checks before it bothers you." |
| 1:30 | Two tasks on the same file: peer message "I'm touching router.rs". Linear `git log`. | "Same repo, same folder. No worktrees." |
| 1:45 | He asks "where are we?": a clean summary. He closes the laptop, reopens it, everything's still there. | "Switchboard. More agents, not more overhead." |

## 6. Positioning

| | Strength | What Switchboard does differently |
|---|---|---|
| **Claude Code** | Best solo CLI agent; subagents and Agent Teams. | Claude Code's subagents serve a single turn; a team's lead goes away with the team. Here main is permanent (infinite thread) and you talk to it while everything runs. |
| **Codex (CLI + cloud)** | Parallel tasks in the cloud, one PR per task. | Local, same folder, no PRs to merge; tasks talk to each other. |
| **Cursor background agents** | Built into the IDE, a remote VM per agent. | Terminal, local, no VMs; one orchestrator instead of a list of agents to watch. |
| **Devin** | Autonomous cloud engineer, driven from Slack. | You stay in control, locally, and can step into any task with one key. |
| **claude-squad, Conductor, tmux** | See several agents side by side. | They display; they don't route. With them, you're still the router. |

In one sentence: **others parallelize agents; Switchboard parallelizes them
without turning you into a project manager.**

**⚠** These tools move fast (Claude Code already has background tasks and
teams). The edge is a permanent main plus agents that talk to each other, not
"parallelism" itself.

## 7. Go-to-market

1. **Dogfooding (now → +1 month).** 5 to 10 Mistral devs who use Vibe.
   Metrics: parallel tasks per day, waiting time avoided, questions resolved
   by main without the human, "two agents on the same file" incidents.
2. **Vibe integration (+1 to 3 months).** Switchboard becomes a Vibe mode
   (`vibe --switchboard` or `/switchboard`), not a separate product. This is
   option A of RFC 0001: the hub as the Unified Harness runtime. The same
   concept can then show up in Le Chat (background tasks driven by a
   conversation).
3. **Open source?** Recommendation: open the hub and the `sb` protocol (the
   proven Bend core is a strong technical argument and a good article), and
   keep the Vibe integration as the front door. To settle with Mistral:
   license, and whether to open up driving other CLIs (Claude Code, Codex) as
   subagents.
4. **Launch.** 2-minute video + "We built Switchboard with Switchboard"
   article + technical article "Proving an agent orchestrator in Bend".
   Channels: X/HN, Mistral blog, Vibe docs.

## 8. What to fix before anyone else uses it

1. **Install.** Today: `run.sh` from a dev worktree, with a Rust binary and a
   Bend REPL to build. It needs a binary (brew / curl | sh) or, better, to
   ship inside Vibe.
2. **Onboarding.** A guided first run: what main is, what a task is,
   `Enter`/`Esc`, cards. A 60-second played-out example.
3. **Visible cost and tokens.** Per task and in total, live, with a budget
   alert (in progress: `token-usage` task). Without it, "as many agents as you
   want" is scary, and rightly so.
4. **Trust and safety.** No approval gate: agents run bash commands without
   asking. At minimum it needs an approval mode, a deny-list (push, rm outside
   the repo), and a clear log of what each agent changed.
5. **Collisions in the shared folder.** Detect two tasks on the same file
   (RFC 0001 §10.3) and stop a commit from sweeping up another task's work
   (enforced targeted `git add`, or a warning).
6. **Visible errors.** A provider outage must show up immediately, not look
   like a silent task (in progress: `error-report` task).
7. **Portability.** Mostly tested on macOS + Ghostty; needs checking on Linux,
   other terminals, and outside the Bend harness.

## 9. Risks

- **Quality depends on the model.** Pillars 3 and 4 are model behaviors. A
  weaker model means a main that answers wrong on your behalf. We need evals
  on "main answers alone" and "main sends work back".
- **Trust.** Main answering for you saves time, right up to the first bad
  silent decision. Every answer main gives a task must be visible and
  reversible.
- **Cost.** 7 agents in parallel cost 7 times as much. Users must see that
  before they discover it on the bill.
- **Catch-up.** Anthropic, OpenAI and Cursor can add a "permanent main"
  quickly. The edge has to come from the Vibe integration and reliability
  (the proven core), not just the idea.
- **No worktrees is a bet.** It works for one repo and one human. On a large
  monorepo with heavy parallel builds (caches, ports, lockfiles), it can
  break. Keep on-demand worktrees and say so clearly.
- **Name.** The CLI is now `bise` ("Switchboard" was a working name). Check
  trademarks, a domain and the GitHub org before launch.
