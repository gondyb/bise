# Switchboard

Nom de travail. Une app terminal où tu parles à un seul agent, **main**, qui
route ton message vers le bon sous-agent. Il y a un sous-agent par tâche. Tu
peux entrer dans un sous-agent (checkout), voir ce qu'il fait et lui parler en
direct, puis revenir à main avec `Esc`. Le fil de main ne se termine jamais.

## Documents

- [`docs/rfc-0001-switchboard.md`](docs/rfc-0001-switchboard.md) : design
  global et comportement attendu (RFC, brouillon).
- [`docs/rfc-0002-worktrees.md`](docs/rfc-0002-worktrees.md) : worktree
  optionnel par tâche, et suppression de la tâche avec son worktree en une
  commande.
- [`docs/rfc-0003-agent-messaging.md`](docs/rfc-0003-agent-messaging.md) :
  chaque agent peut parler à n'importe quel agent du groupe.
- [`docs/ux-notes.md`](docs/ux-notes.md) : questions UX à trancher, pour le
  brainstorming.

## État

Implémenté sur la branche `switchboard` du harness : voir
[`IMPLEMENTATION.md`](IMPLEMENTATION.md) (lancer, tester, touches, limites).
