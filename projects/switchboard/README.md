# Switchboard

Nom de travail. Une app terminal où tu parles à un seul agent, **main**, qui
route ton message vers le bon sous-agent. Il y a un sous-agent par tâche. Tu
peux entrer dans un sous-agent (checkout), voir ce qu'il fait et lui parler en
direct, puis revenir à main avec `Esc`. Le fil de main ne se termine jamais.

## Documents

Design :

- [`docs/rfc-0001-switchboard.md`](docs/rfc-0001-switchboard.md) : design
  global et comportement attendu (RFC, brouillon).
- [`docs/rfc-0002-worktrees.md`](docs/rfc-0002-worktrees.md) : worktree
  optionnel par tâche, et suppression de la tâche avec son worktree en une
  commande.
- [`docs/rfc-0003-agent-messaging.md`](docs/rfc-0003-agent-messaging.md) :
  chaque agent peut parler à n'importe quel agent du groupe.
- [`docs/ux-notes.md`](docs/ux-notes.md) : questions UX à trancher, pour le
  brainstorming.
- [`docs/bend-laws-report.md`](docs/bend-laws-report.md) : les interactions
  du hub en Bend (`hub/*.bend`) et leurs lois.

Distribution et produit :

- [`docs/packaging.md`](docs/packaging.md) (EN) : packaging et
  installation ; les scripts sont dans [`packaging/`](packaging/)
  (`build-dist.sh`, `install.sh`, `test-install.sh`).
- [`docs/pitch.md`](docs/pitch.md) (EN) / [`docs/pitch-fr.md`](docs/pitch-fr.md) :
  pitch et go-to-market.

Code et tests :

- [`IMPLEMENTATION.md`](IMPLEMENTATION.md) : architecture, lancer, tester,
  touches, limites.
- [`tests/`](tests/) : `run_all.sh` lance les contrôles déterministes
  (lois Bend, Rust, e2e, TUI sous tmux) ; `--live` ajoute un vrai modèle.

## État

Lancer : `bise` dans n'importe quel dossier (installé une fois avec
`sh projects/switchboard/packaging/install.sh --dev` sur la machine de dev ;
voir [`IMPLEMENTATION.md`](IMPLEMENTATION.md) § Lancer).

Implémenté dans le harness (branche `main`) : voir
[`IMPLEMENTATION.md`](IMPLEMENTATION.md) (lancer, tester, touches, limites).
