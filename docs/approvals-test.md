# Approvals: the user's test (plan §5)

The 10 steps of [approvals-plan.md](approvals-plan.md) §5, written for
what the `approvals` branch builds today: macOS with the sandbox on (the
default), and the checker as `/models` sets it. Each step says what to do
and what you should see. Something else = a bug: note the step and what
you saw.

## Before you start

- Run the build from `approvals` (main gives you its path:
  `scripts/versions.sh build approvals` prints it) in a repo where no other
  bise runs: a fresh clone is best (`git clone <repo> ~/lab/approvals-try`).
  Your daily bise keeps running elsewhere.
- The checker: with a TypeSafe or OpenRouter key it is Jev; with neither,
  `auto` uses your `small` model (`/models` shows it on the `checker`
  row). Both work for this test; step 9 switches between them.
- The sandbox is on in `auto` on macOS. `BISE_SANDBOX=0` turns it off
  (then bash edits the parser cannot read are denied once, then a card).

## The steps

1. **yolo first.** Start bise. The key bar ends with `⇧⇥ yolo`. Ask an
   agent for anything, even `git push --force` to a scratch branch: nothing
   asks.
2. **Switch to auto.** Press `shift+tab`. For 3 s the key bar says
   `auto · safe calls run, risky ones ask you`; the first time, a tip says
   what leaves the machine (Jev: "the command, the script it runs, and your
   request"; a chat model: "<model> checks the commands that aren't
   clearly safe"). Quit and start bise again: the bar still says
   `⇧⇥ auto` (`approvals = "auto"` in `~/.bise/config.toml`).
3. **Work in the repo.** Ask an agent to read and edit files, then to run
   `ls`, `rg`, `git status`, a `sed -i` on a repo file, and a commit on a
   private index (`GIT_INDEX_FILE=… git commit-tree …`). No card, and they
   all ran.
4. **The checker.** Ask for a command that uses the network, e.g.
   `curl -sI https://example.com` or `gh pr list`. The tool row says
   `checking…`, then it runs. Ask for the same command again: it runs with
   no check (cached for this repo).
   (`cargo test` alone never reaches the checker with the sandbox on: it
   runs contained, with no network, and writes only inside the repo.)
5. **A write outside the repo.** Ask for `echo hi > ~/Desktop/x.txt`. The
   sandbox stops it; a card comes: `<agent> wants to run it outside the
   sandbox`, reason `the sandbox stopped a write outside the repo:
   ~/Desktop/x.txt.`, options `1 run it again without the sandbox`,
   `2 always run it outside the sandbox here`, `3 no`. Type a note
   ("keep it in the repo") and ⏎: the agent reads `stopped by the sandbox`
   and your note; the file is not there. Ask again and pick `1`: the fold
   says `you let <agent> run it outside the sandbox: …` and the file is
   written.
6. **A hard rule.** Ask for `git push --force` to main (`git push origin
   main --force`). A card with no option 2 (no "always"), its reason ends
   with `this one always asks.` Say no.
7. **ctrl+c on a card.** Get a card (step 6's command again), then go to
   the agent's view and press `ctrl+c`. The call does not run and the card
   closes.
8. **Always allow.** Cards are rare with the checker on, so turn it off
   first: `/models` → `checker` → `off · every command asks you`. Ask for
   `curl -sI https://example.com`: a card with `2 always allow … here`.
   Pick `2`. Ask for it again: no card. `/approvals` lists the rule with
   `today · from <agent>`; `backspace` on it asks `remove it? enter yes ·
   esc no`, `enter` removes it.
9. **The checker row.** Still off from step 8: any command past the safe
   ones asks. `/models` → `checker` → a chat provider and model: that model
   checks (step 4's `checking…` again). → `TypeSafe` (needs its key,
   `/provider`): back to Jev.
10. **Two agents.** Start two agents; get one to wait on a card (step 6's
    command). The other keeps working; its replies keep coming. Answer the
    card: the first one goes on.

Also worth a look: quit bise while a card is open and start it again. The
card is still there and answering it reaches the agent.

## What the automated tests already cover

`tests/approvals_e2e.py` (steps 1, 2, 6, 7, 8, 10, sandbox off),
`tests/approvals_restart_e2e.py` (a restart keeps the card, rule removal),
`tests/approvals_sandbox_e2e.py` (steps 3-5 with the sandbox),
`tests/edit_tools_e2e.py`, `tests/agent_tmp_e2e.py`,
`tests/tui_approvals_tmux.py` (the key bar, the flash, `/approvals`, the
cards and folds on screen, the sandbox card), `tests/tui_checker_tmux.py`
(the `checker` row in `/models`). Not covered: a real Jev call (no
TypeSafe or OpenRouter key on the build machine) and a real chat model as
the checker.
