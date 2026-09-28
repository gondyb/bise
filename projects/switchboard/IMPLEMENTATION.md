# Switchboard — implémentation (journal de travail)

Branche `switchboard`, worktree `~/lab/bend-lab/harness-switchboard`.
Ce fichier sert aussi de mémoire de travail : il dit où en est le code.

## Architecture retenue (option B de la RFC 0001, §13)

- **Crate Rust `rust/switchboard`** (lib), branchée dans `bend-harness` :
  - `bend-harness switchboard` : client (TUI ou mode ligne), lance le hub
    si besoin ;
  - `bend-harness sbd --workspace DIR` : le hub (daemon, un par workspace) ;
  - `bend-harness sb …` : la CLI des agents (appelée via le shim
    `<state>/bin/sb`, dans le PATH de chaque REPL).
- **Une REPL Bend (`repl-live`) par agent** (main + chaque tâche), lancée
  par le hub. Le hub est le seul client TCP de chaque REPL.
  - message à un agent inactif : `say <texte>` sur le socket ;
  - message à un agent en plein tour : fichier de steering (ADR 0005) ;
  - interruption : fichier d'interruption.
- **Outils des agents = la CLI `sb` via le tool bash** (pas de nouveaux
  outils natifs) : `sb list/send/wait/ask/report/status/spawn/inspect/
  interrupt/stop/drop/card/history`.
- **État du hub** : journal `journal.jsonl` (ajout seul) + projection en
  mémoire. (La RFC dit SQLite ; JSONL suffit et reste lisible.)
- **Répertoire d'état** : `SB_STATE_DIR` ou
  `$XDG_STATE_HOME/switchboard/<nom>-<hash>` (défaut
  `~/.local/state/switchboard/…`).

### Changements Bend (petits, avec lois)

1. `BEND_WORKDIR` : le tool bash fait `cd` dans ce dossier (workspace ou
   worktree). La REPL reste lancée depuis la racine de l'app.
2. `BEND_EXTRA_PROMPT` : fichier ajouté au prompt système live (rôle de
   main ou de la tâche).
3. `BEND_CONTEXT_FILE` : fichier relu à chaque appel au modèle (appels
   agent seulement, pas la compaction) et ajouté en dernier message
   `user`, jamais stocké dans l'historique (tableau des tâches, §8.2).

## Écarts assumés par rapport aux RFC

- Journal JSONL au lieu de SQLite.
- `wait` borné à `BEND_BG_AFTER - 5` s (le tool bash passe en arrière-plan
  après `BEND_BG_AFTER`). Une réponse plus tardive arrive comme un nouveau
  message et réveille l'agent.
- `needs_approval` n'existe pas : ce harness n'a pas de porte
  d'approbation sur bash.
- Le rapport automatique de fin de tour (RFC 0001 §7.2) met à jour le
  tableau sans réveiller main. Main est réveillé par les réponses
  automatiques (RFC 0003 §7) aux messages qu'il a envoyés.
- Le résumé d'échange direct est livré avec le prochain message à main
  (il ne démarre pas de tour à lui seul).
- `/new` sans nom : le hub dérive le nom du brief (pas de tour de main).
- Budget de tokens : non appliqué si le runtime ne publie pas l'usage.

## Plan

- [x] P0 Bend : BEND_WORKDIR, BEND_EXTRA_PROMPT, BEND_CONTEXT_FILE + lois,
      PROOF vert, repl-live recompilé.
- [x] P1 Rust pur : modèle, journal/projection, routeur, règles de
      livraison, tableau ; tests unitaires.
- [x] P2 Hub : superviseur des REPL, livraison, socket client, socket CLI.
- [x] P3 CLI `sb` + prompts de main et des tâches.
- [x] P4 Worktrees : création, drop avec sauvegarde, restore, isolate.
- [ ] P5 TUI : vues par agent, liste des tâches, checkout/Esc, aperçu,
      cartes, compteurs.
- [ ] P6 Tests E2E : faux provider (Python), client headless ; test live ;
      test TUI sous pty.
- [ ] P7 Docs et commit.

## État

- P0 fait (commit 179e72d) : 8 lois ajoutées, PROOF vert, demo déterministe.
- P1-P4 faits : `rust/switchboard` (core + 22 tests de scénario, worktree
  + tests git, daemon, cli, client), sous-commandes `bend-harness sb|sbd|
  switchboard`. 55 tests unitaires.
- P5 : mode switchboard du TUI (`rust/tui/src/sb.rs`) écrit ; à tester
  sous tmux.
- P6 : `tests/e2e.py` + `tests/fake_provider.py` : 7 scénarios verts
  (spawn + réponse auto, message direct + note, ask/wait, carte, worktree
  drop/restore, CLI refusée, redémarrage du hub).

### Notes de conception du core (pour reprendre après compaction)

- `Hub::handle(input, env) -> Vec<Effect>` ; `env` = trait (now, git).
- Entrées : ReplReady, ReplLine, ReplIdle{leftover_steer}, ReplExited,
  ClientHello/Input/Focus/Gone/Confirm/Cancel/Interrupt, Agent{token,
  from, req}, Tick.
- Effets : Journal(Event), Spawn{resume, crash_note}, Kill, Say, Steer,
  Passthrough(/compact seulement), Interrupt, Context{text}, Line
  (ligne synthétique `sb …` dans le flux d'un agent), Reply{token},
  ToClient, Renamed, State.
- Livraison (`pump`) : Down/Starting → file ; Busy → steer (ids notés) ;
  Idle → `say` (notes de main en préfixe), sauf limite de 4 tâches
  occupées ou limite de réveils par des pairs (20/h).
- À `--- idle` : le shell lit+vide le fichier steer ; s'il restait du
  contenu, les messages steerés sont remis en file. Puis réponses
  automatiques (RFC 0003 §7) et rapport auto (tableau seulement).
- `sb wait` : waiter {token, agent, msg, deadline} ; un message
  expect_reply entrant termine le wait (`incoming_request`).
- Lignes synthétiques : `sb you : <texte>` (message de l'utilisateur),
  `sb msg-in : …`, `sb route : …`, `sb spawn : …`, `sb card : #n …`,
  `sb direct : …`, `sb info : …`, `sb warn : …`.
- Dossiers par agent : `agents/<dir>/` où `dir` = nom à la création
  (un rename ne déplace rien).
