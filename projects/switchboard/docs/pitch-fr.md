# Switchboard : pitch et go-to-market

*English version (version principale) : [pitch.md](pitch.md).*

Brouillon, 2026-09. Point de départ : ce que Gabriel dit de Switchboard après
l'avoir utilisé tous les jours pour le construire. Les affirmations pas encore
vraies sont marquées **⚠**.

## 1. Le problème

Les agents de code sont bons. Les travailler en parallèle, non.

- **On attend.** Un tour dure 1 à 10 minutes. Pendant ce temps, le terminal
  est pris. On regarde défiler, ou on change de contexte et on perd le fil.
- **On fait la nounou.** Avec 3 agents, on devient le routeur : quel terminal
  fait quoi, qui attend une réponse, qui a fini à moitié, qui faut-il relancer.
  La charge mentale monte avec chaque agent. En pratique, on plafonne à 2 ou 3.
- **On jongle avec les worktrees.** Pour que les agents ne se marchent pas
  dessus, chaque outil pousse vers un worktree ou une VM par tâche. Résultat :
  des branches à merger, des conflits à la fin, des dépendances à réinstaller,
  et des agents qui ne savent rien les uns des autres.

## 2. La promesse

> **Tu parles à un seul agent. Il en fait travailler dix. Tu n'attends jamais.**

Taglines (choisies par Gabriel, gardées en anglais) :

> **multi-agent coding, made human.**
> *ramble. interrupt. change your mind. i run the agents. you stay in flow.*

« kiss your backlog goodbye. » descend dans la page : c'est le titre de la
section qui montre ce que bise sait faire (landing, features).

**Règle de style (Gabriel) : jamais de majuscule en début de mot, sur tous
les supports marketing et tous les textes du site.** Ça fait humain qui tape
au clavier. Ton : humain, décontracté, à la première personne quand ça colle ;
éviter la structure lisse « X. None of the Y. » qui sonne LLM. Exceptions : les noms
propres gardent leurs majuscules (Mistral, Vibe, Claude Code, GitHub) ; les
sigles en majuscules (API, MCP), mais « cli » en minuscules passe bien ; les MAJUSCULES pour crier sont
permises, extrêmement rarement. Notre nom reste en minuscules : `bise`. (Les docs
internes comme celle-ci gardent les majuscules, sauf dans la copy elle-même.)

Les deux choisies par Gabriel. Ancienne principale, gardée en alternative :
« your team is as big as your ideas. »

Public et message central : les gens qui ont plein d'idées, qui vont vite, et
qui ont besoin que la tech disparaisse et les suive. La valeur est humaine : le
flow, pas de jonglage entre agents, pas de coût cognitif. Tu es plus efficace,
tu prends plus de plaisir, et tu ships plus. Éviter les chiffres (« dix »,
« 10x ») : ça sonne comme un plafond, et le produit n'en a pas. **⚠** Pas de
limite dans le produit (RFC 0001 §10.1), mais les tokens, le coût et la
vitesse du provider sont la vraie limite ; le dire.

**Nom : `bise`** (choisi par Gabriel). Court à taper, avec un double sens : la
bise est un vent froid du nord (le vent, c'est le
flow), et *faire la bise*. Vérifié libre sur npm, crates.io, PyPI et Homebrew.
**⚠** `bise.sh` et `bise.ai` sont pris ; `bise.dev`, l'org GitHub, les marques
et les paquets Linux ne sont pas encore vérifiés. Homonyme connu : BISE, le
système d'information européen sur la biodiversité (autre domaine).

**Signe du logo : `:*`** (Gabriel), le bisou ASCII : `bise :*`. Il marche
dans tous les terminaux et toutes les polices, et fait tapé par un humain.

**Marque : autonome** (décision de Gabriel). bise est sa propre marque pour
l'instant, sans lien avec Mistral : pas d'identité visuelle Mistral, pas de
mention de Mistral dans la copy. **⚠** Les sections 4 et 7 (cible = devs
Mistral, intégration Vibe) datent d'avant cette décision et sont à repenser.
Question ouverte : le code tourne sur le Unified Harness et Vibe de Mistral ;
qu'est-ce qui peut sortir sous une marque autonome ?

Taglines :

| FR | EN |
|---|---|
| Ton équipe est aussi grande que tes idées. | Your team is as big as your ideas. |
| divague. interromps. change d'avis. je gère les agents. tu restes dans le flow. | ramble. interrupt. change your mind. i run the agents. you stay in flow. (sous-titre du site, choisi par Gabriel) |
| Fait pour les ingénieurs qui pensent plus vite qu'ils ne tapent. | built for engineers who think faster than they type. (ancien sous-titre, remplacé) |
| des idées entrent. des bisous sortent. et des pull requests. | ideas in. little kisses out. also pull requests. (tagline dans le produit, choisie par Gabriel) |
| je souffle sur les backlogs. | i blow through backlogs. (one-liner pro : README, bios) |
| bise : un bisou sur la joue. aussi un vent du nord. | bise /beez/ · french, n. 1. a quick kiss on the cheek. 2. a brisk north wind. 3. a terminal where your agents ship while you think. (glose pour les anglophones) |
| trop d'idées ? tant mieux. | too many ideas? good. |
| dis-le une fois. oublie. c'est fait. | say it once. forget about it. it's done. |
| enfin un truc qui suit mon cerveau. | finally, something that keeps up with my brain. |
| le multi-agent, version humaine. | multi-agent coding, made human. (tagline principale, choisie par Gabriel) |
| dis adieu à ton backlog (et un bisou au passage). | kiss your backlog goodbye. (titre de la section features) |
| toi, mais avec beaucoup plus de mains. | you, but with way more hands. (ancienne principale, en réserve) |
| Ne jamais attendre un agent. | Never wait on an agent again. |
| Un interlocuteur. Autant d'agents que tu veux. | One conversation. As many agents as you want. |
| Reste dans le flow, main s'occupe du reste. | Stay in flow. Main handles the rest. |
| Le standard téléphonique de tes agents. | The switchboard for your agents. |
| Plus d'agents, pas plus de charge mentale. | More agents, not more overhead. |

## 3. Les 5 piliers

**1. Tu restes en flow : tu n'attends jamais la fin d'un tour.**
Main est toujours disponible. Chaque demande part dans une tâche qui tourne en
parallèle.
- Avant : « lance les tests » → 6 minutes à regarder le terminal.
- Après : « lance les tests », puis tout de suite « et regarde ce bug Safari »,
  puis « écris la release note ». Trois tâches tournent, tu as déjà enchaîné.

**2. Tu ne sais pas qui fait quoi, et tu n'as pas besoin de le savoir.**
Main route tes messages vers la bonne tâche et garde le tableau de toutes les
tâches, même après compaction. Tu peux entrer dans une tâche (`Enter`) et
revenir (`Esc`), mais tu n'es pas obligé.
- Avant : 5 onglets de terminal, un post-it mental par agent.
- Après : « où en est le fix Safari ? » → main répond, avec l'état réel de la
  tâche.

**3. Main répond seul aux questions évidentes.**
Quand une tâche demande « v1 ou v2 de l'API ? » et que la réponse est dans le
brief ou le repo, main répond. Seules les vraies décisions arrivent jusqu'à toi,
sous forme de carte.
- Avant : 12 interruptions par heure, dont 10 triviales.
- Après : 2 cartes, les 2 qui comptent.

**4. Main relance ce qui n'est pas fini.**
Main relit le rapport de la tâche. Si c'est incomplet ou faux, il renvoie la
tâche au travail avec une consigne précise, avant de te déranger.
- Avant : « done ! » … sauf que les tests ne passent pas. Tu découvres ça 20
  minutes plus tard.
- Après : main a vu que les tests échouaient et a relancé. Tu reçois un vrai
  « fini ».

**5. Un seul dossier, des worktrees seulement quand ça aide.**
Les agents se connaissent. Ils se voient (`sb list`, `sb tasks`), lisent le fil
des autres, et se préviennent avant de toucher un fichier partagé. Quand un
agent juge qu'il a vraiment besoin d'isolation, il crée son propre worktree
(marqué `ψ`) et le nettoie après. Il n'en crée pas des centaines. Argument de
vente (Gabriel) : les worktrees sont supportés, mais tu ne les gères jamais.
- Avant : 5 branches, 5 `npm install`, 5 merges.
- Après : un repo, un dossier, un historique git linéaire.

**⚠ Honnêteté sur les piliers.** Les piliers 3, 4 et 5 viennent du prompt et
du modèle, pas d'une garantie du système. Ils marchent bien avec un bon modèle,
pas toujours. Le pilier 5 n'a pas de filet : la détection de fichiers modifiés
par deux tâches (RFC 0001 §10.3) n'est pas implémentée, et on a déjà eu un
commit qui a embarqué le travail d'une autre tâche. « À l'infini » est faux :
le coût en tokens et la vitesse du provider sont la vraie limite.

## 4. Pour qui, et par où commencer

- **Cible :** développeurs qui utilisent déjà un agent CLI tous les jours et
  qui en veulent plusieurs en parallèle. Ils ont déjà ressenti la douleur
  « j'attends / je fais la nounou ».
- **Premier coin (wedge) :** les utilisateurs de Vibe CLI chez Mistral, puis
  les power users de Vibe dehors. Même stack, même modèle, feedback en 1 jour.
- **Pas maintenant :** les non-développeurs, les équipes (multi-utilisateurs),
  le cloud.

## 5. Preuves et démo

Preuves à montrer :
- **Construit avec lui-même.** Switchboard a été développé dans Switchboard :
  7 tâches en parallèle sur le même repo au moment où ce doc est écrit (hub en
  Bend, versions, voix, erreurs, éditeur, tokens, ce pitch).
- **Un cœur prouvé.** Le hub est écrit en Bend, avec des lois vérifiées par la
  machine : aucun message perdu, livré une seule fois, aucun agent ne reste
  inactif avec du courrier en attente. Les preuves ont trouvé de vrais bugs
  (ex. une tâche restaurée qui ne recevait pas son courrier). **⚠** Les preuves
  couvrent la livraison des messages, pas le comportement des agents.
- **On le met à jour sans rien arrêter.** `/restart` reconstruit, relance
  le hub en période d'essai et revient en arrière si ça casse. Les agents
  continuent leur tour.

Storyboard vidéo, 2 minutes :

| Temps | Écran | Voix off |
|---|---|---|
| 0:00 | Un dev devant Claude Code, une barre de progression. Il attend. | « Un agent, c'est génial. Attendre un agent, non. » |
| 0:10 | Switchboard vide. Il tape 3 demandes à la suite, sans attendre. | « Tu parles à main. Main crée les tâches. » |
| 0:30 | Panneau des tâches : 3 `working`. Une 4e demande à la voix **⚠ (voix en cours)**. | « Tu n'attends jamais. » |
| 0:45 | Une tâche pose une question ; main répond seul, ligne `sb route`. | « Les questions évidentes, main y répond. » |
| 1:00 | Une carte arrive : vraie décision. Il répond en un mot (`Ctrl+A`). | « Toi, tu ne gardes que les vraies décisions. » |
| 1:15 | Une tâche dit « fini », main voit un test rouge et la relance. | « Main vérifie avant de te déranger. » |
| 1:30 | Deux tâches sur le même fichier : message de pair « je touche router.rs ». `git log` linéaire. | « Même repo, même dossier. Pas de worktrees. » |
| 1:45 | Il demande « où on en est ? » : résumé propre. Il ferme le laptop, le rouvre, tout est là. | « Switchboard. Plus d'agents, pas plus de charge mentale. » |

## 6. Positionnement

| | Point fort | Ce que Switchboard fait autrement |
|---|---|---|
| **Claude Code** | Meilleur agent CLI solo ; sous-agents et Agent Teams. | Les sous-agents de Claude Code servent un tour ; le lead d'une équipe disparaît avec elle. Ici main est permanent (fil infini) et tu lui parles pendant que tout tourne. |
| **Codex (CLI + cloud)** | Tâches parallèles dans le cloud, une PR par tâche. | Local, même dossier, pas de PR à merger ; les tâches se parlent. |
| **Cursor background agents** | Intégré à l'IDE, VM distante par agent. | Terminal, local, pas de VM ; un orchestrateur au lieu d'une liste d'agents à surveiller. |
| **Devin** | Ingénieur autonome dans le cloud, via Slack. | Tu restes aux commandes, en local, et tu peux entrer dans chaque tâche en une touche. |
| **claude-squad, Conductor, tmux** | Voir plusieurs agents côte à côte. | Ils affichent ; ils ne routent pas. Avec eux, tu restes le routeur. |

En une phrase : **les autres parallélisent les agents ; Switchboard
parallélise sans te transformer en chef de projet.**

**⚠** Ces outils bougent vite (Claude Code a déjà des tâches en arrière-plan et
des équipes). L'avance tient à main permanent + agents qui se parlent, pas à
« le parallèle ».

## 7. Go-to-market

1. **Dogfooding (maintenant → +1 mois).** 5 à 10 devs Mistral, Vibe
   utilisateurs. Mesures : nombre de tâches en parallèle par jour, temps
   d'attente évité, nombre de questions résolues par main sans l'humain,
   incidents « deux agents sur le même fichier ».
2. **Intégration Vibe (+1 à 3 mois).** Switchboard devient un mode de Vibe
   (`vibe --switchboard` ou `/switchboard`), pas un produit à part. C'est
   l'option A de la RFC 0001 : le hub comme runtime du Unified Harness. Le
   même concept peut ensuite exister dans Le Chat (tâches d'arrière-plan
   pilotées par une conversation).
3. **Open source ?** Recommandation : ouvrir le hub et le protocole `sb`
   (le cœur Bend prouvé est un bon argument technique et un bon article),
   garder l'intégration Vibe comme porte d'entrée. À trancher avec Mistral :
   licence, et si on ouvre le pilotage d'autres CLI (Claude Code, Codex)
   comme sous-agents.
4. **Lancement.** Vidéo de 2 min + article « On a construit Switchboard avec
   Switchboard » + article technique « Prouver un orchestrateur d'agents en
   Bend ». Canaux : X/HN, blog Mistral, doc Vibe.

## 8. À corriger avant que quelqu'un d'autre l'utilise

1. **Installation.** Aujourd'hui : `bise`, par le canal dev
   (`install.sh --dev` : il lance la version du hub du dépôt de dev), donc
   toujours un dépôt de dev, binaire Rust + REPL Bend à compiler. Il faut un binaire (brew / curl |
   sh) ou, mieux, livré dans Vibe.
2. **Onboarding.** Premier lancement guidé : ce qu'est main, ce qu'est une
   tâche, `Enter`/`Esc`, les cartes. Un exemple joué en 60 secondes.
3. **Coût et tokens visibles.** Par tâche et au total, en direct, avec une
   alerte de budget (en cours : tâche `token-usage`). Sans ça, « autant
   d'agents que tu veux » fait peur, à raison.
4. **Confiance et sécurité.** Pas de porte d'approbation : les agents lancent
   des commandes bash sans demander. Il faut au minimum un mode approbation,
   une liste de commandes interdites (push, rm hors repo) et un journal clair
   de ce que chaque agent a changé.
5. **Collisions dans le dossier partagé.** Détecter deux tâches sur le même
   fichier (RFC 0001 §10.3) et empêcher un commit d'embarquer le travail d'une
   autre tâche (`git add` ciblé imposé, ou alerte).
6. **Erreurs visibles.** Une panne de provider doit se voir tout de suite, et
   pas passer pour une tâche silencieuse (en cours : tâche `error-report`).
7. **Portabilité.** Testé surtout sur macOS + Ghostty ; à vérifier sur Linux,
   autres terminaux, et hors du harness Bend.

## 9. Risques

- **La qualité dépend du modèle.** Les piliers 3 et 4 sont des comportements
  du modèle. Un modèle plus faible = un main qui répond faux à ta place. Il
  faut des évals sur « main répond seul » et « main relance ».
- **Confiance.** Main qui répond à ta place, c'est un gain de temps jusqu'à la
  première mauvaise décision silencieuse. Il faut que chaque réponse de main
  à une tâche soit visible et annulable.
- **Coût.** 7 agents en parallèle coûtent 7 fois plus. Le public doit le voir
  avant de le découvrir sur la facture.
- **Rattrapage.** Anthropic, OpenAI et Cursor peuvent ajouter un « main
  permanent » vite. L'avance doit venir de l'intégration Vibe et de la
  fiabilité (cœur prouvé), pas seulement de l'idée.
- **Pas de worktrees = pari.** Ça marche sur un repo et un humain. Sur un gros
  monorepo avec des builds lourds et parallèles (caches, ports, lockfiles), ça
  peut casser. Garder le worktree à la demande et le dire clairement.
- **Nom.** Le CLI s'appelle maintenant `bise` (« Switchboard » était un nom
  de travail). Vérifier les marques, un domaine et l'org GitHub avant le
  lancement.
