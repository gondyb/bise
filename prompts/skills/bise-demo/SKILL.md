---
name: bise-demo
description: Run bise's built-in demo, a tiny scripted quest where two agents start, message each other, ask the user one question and are dropped, with no file or git change. Use when the user asks to see what bise or you can do, for a demo, a tour or your features (show me what you can do, give me a demo, what can you do?, demo all your features), in any language.
---

# bise demo: a tiny quest

You run a short, scripted demo of bise, played as a tiny RPG: two agents go on a quest, talk to each other, and you and the user each answer one question. The user sees the real mechanics with nothing at stake. Speak the user's language all along (the agents too: put it in their briefs).

The rules of the demo, never broken:
- Nothing in the workspace changes: no file written, edited, created or deleted, no git command, no worktree. The agents run `sb` and nothing else; their briefs say so.
- It stays short and cheap: it runs on the user's own key.
- Every demo agent is dropped at the end: the list is as it was.

## 1. Ask before you start

One short message, then end your turn and wait for the answer:

`a quick demo: two agents go on a tiny quest, message each other and ask you one question. no files touched, nothing left behind. about 2 minutes and ~200k tokens on your key (a few cents on a mid-size model). go?`

Adapt the numbers to the model you run on if you know better. The user says no or asks something else: drop the demo, answer them.

## 2. Start the party

Run both spawns in one bash call (names exactly `quest-scout` and `quest-smith`; a name taken: add `-2`). Replace `<lang>` with the user's language.

```
sb spawn quest-scout --objective "Demo quest (a game, not real work): you are the scout. Step 1, run: sb send quest-smith \"the dragon of the old repo wakes up: forge me a weapon!\" Step 2, run: sb send main --expect-reply \"two paths to the dragon: 1. the dark forest 2. the mountain pass. which one?\" Step 3: end your turn; main's answer comes as a message and wakes you up. Step 4, once you have the path, run: sb report done \"<a 2-line story: the path, the twist, the ending>\"" --constraint "Run every step with your bash tool: an sb command written as text reaches nobody." --constraint "This is a demo: never create, edit or delete a file, never run git or any command other than sb." --constraint "Write in <lang>. One or two short, playful sentences per message." --done-when "you reported done with the story"
sb spawn quest-smith --objective "Demo quest (a game, not real work): you are the blacksmith. The user picks the blade, never you. Step 1, run: sb send quest-scout \"i hear you! the user picks the blade, the forge is hot.\" Step 2, run: sb send main --expect-reply \"which blade do i forge?\" Step 3: end your turn; the user's answer comes as a message and wakes you up. Step 4, once you have it, run: sb report done \"<the blade the user chose, forged, one line>\"" --constraint "Run every step with your bash tool: an sb command written as text reaches nobody." --constraint "Never choose the blade yourself: wait for the user's answer." --constraint "This is a demo: never create, edit or delete a file, never run git or any command other than sb." --constraint "Write in <lang>. One or two short, playful sentences per message." --done-when "you reported done with the user's blade"
```

Then, in the same turn:
1. Steer quest-scout while it works: `sb send quest-scout "twist: the dragon is really a giant rubber duck. put it in your story."`
2. Tell the user, in one short message:
   - who took what: `on it: quest-scout goes after the dragon, quest-smith forges the weapon.`
   - `i just steered quest-scout mid-run: my message turns ✓✓ once it has read it.`
   - how to peek: `press ⌥1 to peek at quest-scout, ⌥2 at quest-smith, ⌥0 to come back here.` (the numbers are the agents' places in the list, main is 0; check `sb list` if other tasks exist)
   - the ✉ lines are the agents messaging each other.
3. End your turn. Do not poll: messages and answers wake you up.

## 3. The smith's question: the user's card

quest-smith asks you which blade: this is the user's call, not yours. Open a card tied to its message, the options as numbered lines so one key answers (the user's answer goes straight to quest-smith):
`sb card --for <its message id> "quest-smith asks: which blade do i forge?
1. a sword of clean commits
2. an axe of fast tests"`
Then tell the user in one line: `a question for you is in the inbox: ctrl+g, ⏎, then 1 or 2.`

## 4. The scout's question

quest-scout asks you which path: answer it yourself, on the user's behalf, with a why:
`sb send quest-scout --reply-to <its id> --why "a demo: i pick so you only answer one question" "2. the mountain pass"`
Then tell the user in one line: `quest-scout asked which path; i answered the mountain pass myself, so you only get one question.`

An agent's end-of-turn message (`auto="true"`) is not a report: an agent is done only when `sb tasks` shows its done report. Never announce the end before that.

## 5. The end

quest-smith finishes last: it waits for the user's card. Wait until BOTH reported done (their `done` reports, visible in `sb tasks`; an `auto="true"` end-of-turn message is not a report) and both are idle, or ~5 minutes passed. One done is not the end: end your turn and wait.
1. Drop both in one bash call, exactly: `sb drop quest-scout; sb drop quest-smith`. A drop is refused or asks the user: tell the user in one line and stop there; never run `sb close` for it, never retry a failing command.
2. Check `sb list`: no quest agent is left.
3. Close in 3 or 4 short lines: the story in one line (quest-scout's path and quest-smith's blade), then what they just saw: agents start with `sb spawn`, talk with ✉ messages, ask you through the inbox, can be steered mid-run, and main answers what it can. Nothing in the repo changed. End with one line on what to do next: `now tell me something real you want done.`
