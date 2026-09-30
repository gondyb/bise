# outreach · heads-up DMs (heads-ups: before launch · Josh: after)

rules: one personal DM each, no repost ask, no upvote ask. send only to people you actually know (mark them below). edit each one so it sounds like you.

| who | know them? | channel | sent |
|---|---|---|---|
| Matt Pocock | a little | X DM | [ ] |
| Simon Willison | ? | email / X | [ ] |
| Mitchell Hashimoto | ? | X / Ghostty discord | [ ] |
| Armin Ronacher | ? | X / Bluesky | [ ] |
| Peter Steinberger | ? | X | [ ] |
| Geoffrey Huntley | ? | X | [ ] |
| swyx | ? | X | [ ] |
| Josh Comeau | ? | X / email | [ ] |
| Grafikart, Underscore_, Korben | ? | X / email (FR) | [ ] |
| Mistral colleagues | warm | Slack perso / DM, after the manager heads-up | [ ] |
| ThePrimeagen, Theo | cold | nothing. only if it takes off | – |

---

**Matt Pocock** (cold-ish, informal, before 14:30: an early look)
> hey Matt! how's it going?
>
> i've been working on a side project. multi-agent coding always drove me nuts (juggling sessions, being the router between them), so i built my own thing: bise.dev
>
> honestly it's the first setup that feels natural to me. I want to make it public pretty soon. want to give it a spin and tell me what you think?

**Matt · second message, if he says yes** (add him first: github.com/gvergnaud/bise → settings → collaborators, his handle is `mattpocock`: checked, GitHub "Matt Pocock", X @mattpocockuk)
> amazing! the repo is still private, so i just added you as a collaborator. accept the invite (github.com/gvergnaud/bise/invitations), then:
>
> brew install gh && gh auth login   (if you don't have it)
> curl -fsSL bise.dev/install | sh
>
> then cd into any repo and run bise. you bring your own key (Anthropic, OpenAI, …).
>
> heads-up: there's no approval mode yet, agents run commands without asking. so pick a repo you trust :)

install path checked by main on the real private release (with a token; the `gh` branch is covered by tests, the full run through real `gh` couldn't run on a temp HOME). after the repo is public, he just needs the curl line.

why collaborator over a zip: the installer already handles a private repo through `gh` (install.sh, BISE-217), and he gets the updates. a zip downloaded in a browser gets the macOS quarantine flag, and an ad-hoc signed binary then gets blocked by Gatekeeper.

**Simon Willison**
> hi Simon, heads-up: i'm open-sourcing bise very soon. it's a terminal app for running many coding agents from one thread per repo. two details you might like: tools are called by writing TypeScript (every MCP becomes functions), and it's honest about its limits (no approval mode yet, macOS only). no ask. bise.dev

**Mitchell Hashimoto**
> hi Mitchell, a thank-you more than anything: i built bise, a terminal app for coding with agents, and it feels most at home in Ghostty (it sets up the keys for you, hold ctrl shows the shortcuts). it goes public very soon. no ask. bise.dev

**Armin Ronacher**
> hi Armin, you write a lot about working with many agents, so a heads-up: i'm putting out bise very soon. one thread per repo, it runs the agents, they message each other before they collide. would love your honest take whenever, no rush. bise.dev

**Peter Steinberger**
> hi Peter, heads-up since you run a lot of agents in parallel: i'm open-sourcing bise very soon: one thread per repo, it runs the agents for you. i'd love your honest take whenever. bise.dev

**Geoffrey Huntley**
> hi Geoffrey, heads-up: bise goes public very soon. it's a terminal app where you talk to one thread and it runs the coding agents, worktrees only when one needs its own copy. honest feedback welcome whenever. bise.dev

**swyx**
> hey swyx, heads-up: i'm launching bise very soon. it's my take on agent UX: one thread per repo, the agents talk to each other, and the decisions that need you wait in an inbox. if it ever fits Latent Space, i'd love to talk about the design. bise.dev

**Josh Comeau**
> hi Josh, just a thank-you: i just launched bise, and the site's animations owe a lot to what i learned from you. no ask at all. bise.dev :*

**Grafikart / Underscore_ / Korben (FR)**
> salut ! petit message avant que ce soit public : je sors bientôt bise :* , un terminal open source pour coder avec plein d'agents IA depuis un seul fil par repo. fait à Paris, nom français, bien sûr. aucune demande, je voulais juste que tu l'apprennes de moi. si un jour ça t'intéresse d'en parler, avec plaisir. bise.dev

**Mistral colleagues (after the manager heads-up)**
> hey ! je sors bientôt mon projet perso : bise, un terminal pour coder avec plein d'agents. rien à voir avec Mistral officiellement, c'est mon side project. si tu as 5 min pour l'essayer ce soir, ton retour m'aiderait beaucoup. bise.dev
