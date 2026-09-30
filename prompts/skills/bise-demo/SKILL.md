---
name: bise-demo
description: Run bise's built-in demo, a short role-play where a small product team (a pm, a designer, a dev) starts, talks like colleagues, asks the user one question and one agent is dropped, with no file or git change. Use when the user asks to see what bise or you can do, for a demo, a tour or your features (show me what you can do, give me a demo, what can you do?, demo all your features), in any language.
---

# bise demo: a small team

You run a short demo of bise, played as a small product team at a company: a pm, a designer and a dev build a CSV export of the invoices page. They only role-play the work. The user sees the real mechanics with nothing at stake, and the TUI shows tips that tell them what to press at each step (they come on their own: the agents' objectives start with `bise demo`). Speak the user's language all along (the agents too: put it in their briefs).

The rules of the demo, never broken:
- Nothing in the workspace changes: no file written, edited, created or deleted, no git command, no worktree. The agents run `sb` and nothing else; their briefs say so.
- It stays short and cheap: it runs on the user's own key.
- At the end one agent is dropped and two stay idle, for the user to open and read.

## 1. Ask before you start

One short message, then end your turn and wait for the answer:

`a quick demo: a small team (a pm, a designer, a dev) builds a pretend feature, talks, and asks you one question. no files touched. about 3 minutes and ~300k tokens on your key (a few cents on a mid-size model). go?`

Adapt the numbers to the model you run on if you know better. The user says no or asks something else: drop the demo, answer them.

## 2. Start the team

Run the three spawns in one bash call. Names exactly `pm`, `designer`, `dev-api`; a name taken: add `-2` to it, and use the new names everywhere (the briefs too). Every objective starts with `bise demo`: the TUI's tips key on it. Replace `<lang>` with the user's language.

```
sb spawn pm --objective "bise demo (role-play, not real work): you are the pm of a small product team; the feature: a CSV export of the invoices page. Step 1, run: sb send designer \"hi! new one for this sprint: a CSV export on the invoices page. v1 exports what the current filters show. can you place the button?\" Step 2, run: sb send dev-api \"hey, the CSV export is yours: current filters, up to 10k rows. ping me if something is unclear.\" Step 3, run: sb report done \"<2 lines: the scope, who does what>\"" --constraint "Every step is one call of your bash tool, the last one too (sb report done): an sb command written in your reply runs nothing and reaches nobody." --constraint "This is a demo: never create, edit or delete a file, never run git or any command other than sb." --constraint "Write in <lang>. Short, like a Slack message between colleagues." --done-when "you reported done with the scope"
sb spawn designer --objective "bise demo (role-play, not real work): you are the designer of a small product team; the feature: a CSV export of the invoices page. The user picks where the button goes, never you. Step 1, run: sb send main --expect-reply \"quick one for the CSV export: where does the button go? 1. next to the filters 2. in the ⋯ menu\" Step 2: end your turn; the answer (the user's call) comes as a message and wakes you up. Step 3, once you have it, run: sb send dev-api \"button goes <the answer>, label 'Export CSV'.\" Step 4, run: sb report done \"<one line: where the button goes>\"" --constraint "Every step is one call of your bash tool, the last one too (sb report done): an sb command written in your reply runs nothing and reaches nobody." --constraint "Never choose the place yourself: wait for the answer." --constraint "This is a demo: never create, edit or delete a file, never run git or any command other than sb." --constraint "Write in <lang>. Short, like a Slack message between colleagues." --done-when "you reported done with the user's choice"
sb spawn dev-api --objective "bise demo (role-play, not real work): you are the backend dev of a small product team; the feature: a CSV export of the invoices page. Step 1, run: sb send pm \"on it. i'll stream the file so 10k rows don't time out.\" Step 2, run: sb send main --expect-reply \"should the export include cancelled invoices?\" Step 3: end your turn. You wait for two things, in any order, each one wakes you up: main's answer (a message), and a word from the user, who writes to you directly in your thread. When the user writes, answer them in one short line, as a colleague would, then end your turn. The user's word is a message from the user in your thread, never one you imagine: without it, you end your turn and wait. Step 4, once you have BOTH main's answer and the user's word, run: sb report done \"<one line: the export shipped (role-play), with main's answer and the user's word>\"" --constraint "Every step is one call of your bash tool, the last one too (sb report done): an sb command written in your reply runs nothing and reaches nobody." --constraint "This is a demo: never create, edit or delete a file, never run git or any command other than sb." --constraint "Write in <lang>. Short, like a Slack message between colleagues." --done-when "you reported done after main's answer and the user's word"
```

Then, in the same turn:
1. Steer dev-api while it works: `sb send dev-api "heads-up: finance wants the dates in ISO 8601 in the CSV."`
2. Tell the user in one line: `your team is on it: pm scopes the CSV export, designer places the button, dev-api writes it. follow the tips on screen.`
3. End your turn. Do not poll: messages and reports wake you up.

## 3. The designer's question: the user's card

designer asks you where the button goes: this is the user's call, not yours. Open a card tied to its message, the options as numbered lines so one key answers (the answer goes straight to designer):
`sb card --for <its message id> "designer asks: where does the export button go?
1. next to the filters
2. in the ⋯ menu"`
Then tell the user in one line: `designer needs you: the question is in your inbox.`

## 4. dev-api's question

dev-api asks about cancelled invoices: answer it yourself, on the user's behalf, with a why:
`sb send dev-api --reply-to <its id> --why "a demo: i answer what i can so you get one question" "no: finance tracks those apart."`
Then tell the user in one line: `dev-api asked about cancelled invoices; i answered it myself. it now waits for a word from you: ⌥ and its number, type, ⏎.`

The user writes to you instead of dev-api while it waits: tell them in one line how to reach it (`⌥` and its number, type, ⏎), or, if they want to stop, go to step 5 now with who is done.

## 5. The end

pm finishes first, dev-api last (it waits for the user's word). An agent is finished when `sb tasks` shows its done report, or when it is idle and its last message gives the result of its last step (pm: the scope; designer: where the button goes, from the user's answer; dev-api: the export, with the user's word). An agent still waiting for an answer is not finished. The card must be answered too (it left the inbox, designer has the user's choice): the end never comes before the user's answer. When all three are finished, or the user asks to stop, go on; else end your turn and wait (each message wakes you up, and `sb tasks` tells you where they are).
1. Drop pm only: run `sb drop pm` with your bash tool, now. Never drop designer or dev-api: the user reads them next. A drop is refused or asks the user: tell the user in one line and stop there; never run `sb close` for it, never retry a failing command.
2. Run `sb list`: pm is gone, designer and dev-api are there. No recap before this: the user's tips wait for pm to be archived.
3. Close in 3 short lines, in the user's language (no ⌥ numbers: you do not see the panel; the tip on screen gives them):
   `done: pm scoped it, designer placed the button, dev-api wrote the export. nothing in the repo changed.`
   `designer and dev-api stay in the panel for you to open; ctrl+s finds pm. ask me to drop them when you're done.`
   `now, what do you actually want done?`
