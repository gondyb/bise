# Notes UX : à brainstormer

Questions à trancher. Le comportement est décrit dans
[`rfc-0001-switchboard.md`](rfc-0001-switchboard.md). Ici, on décide comment
il se voit et se contrôle.

## Proposition de départ

```
┌ main ─────────────────────────────────────┐┌ tâches ────────────────┐
│ toi : le login casse sur Safari            ││ ● auth-fix   working 12m│
│ main → nouvelle tâche @auth-fix            ││ ? docs       waiting   │
│ toi : et la doc ?                          ││ ✓ bench      done      │
│ main : @docs attend une réponse (v1 ou v2) ││                        │
│ ┌ @docs demande ───────────────────────┐   ││                        │
│ │ v1 ou v2 de l'API ?        [répondre]│   ││                        │
│ └──────────────────────────────────────┘   ││                        │
├────────────────────────────────────────────┴┴────────────────────────┤
│ > _                                        2 cartes · 3 tâches · 1,2 $│
└──────────────────────────────────────────────────────────────────────┘
```

Touches de départ :

| Touche | Action |
|---|---|
| `Esc` | Retour à main (compositeur vide) |
| `Alt+↓` / `Alt+↑` | Tâche suivante / précédente dans la liste (Ctrl+J/K retirés, BISE-302) |
| `Enter` sur une tâche | Checkout |
| `Space` sur une tâche | Aperçu sans changer le focus |
| `@nom …` | Route explicite |
| `Ctrl+A` | Carte d'attention suivante |
| `Ctrl+Z` | Annuler le dernier routage (si pas encore livré) |

## Questions

### Navigation

1. La liste des tâches : panneau toujours visible, ou affiché à la demande ?
2. Aller à une tâche par numéro (`Alt+1…9`), par nom (palette `Ctrl+P`), ou
   les deux ?
3. Quand le compositeur a le focus, comment on navigue dans la liste sans
   conflit avec l'édition de texte ?
4. Le checkout remplace-t-il tout l'écran, ou s'ouvre-t-il dans un panneau à
   côté de main ?
5. Faut-il un moyen de revenir à la **dernière** tâche (comme `cd -`) en plus
   de `Esc` vers main ?

### Routage

6. Comment montrer la décision de routage de main : ligne dans le fil,
   animation, notification ?
7. Combien de temps l'annulation reste-t-elle visible ?
8. `@nom` doit-il avoir de l'autocomplétion ?
9. Faut-il un mode « collé » : tout ce que je tape va à `@nom` jusqu'à ce que
   je sorte, sans faire de checkout complet ?

### Fil de main

10. Les rapports `progress` des tâches apparaissent-ils dans le fil de main,
    ou seulement dans le tableau ?
11. Comment afficher un fil qui a des milliers de messages : repli par jour,
    par tâche, recherche ?
12. Les cartes d'attention restent-elles à leur place dans le fil, ou
    remontent-elles en bas tant qu'elles sont ouvertes ?

### Vue checkout

13. Afficher un bandeau « tu parles directement à @nom, main n'est pas dans
    la boucle » ?
14. Montrer le brief et le dernier rapport en haut de la vue ?
15. Afficher dans la vue checkout les cartes qui arrivent des autres tâches ?

### Notifications hors de l'app

16. Notification système (macOS) quand une carte d'attention s'ouvre ?
17. Un son, une cloche de terminal, rien ?

## Décisions

(à remplir pendant le brainstorming)

- 2026-09-27 : « backtrack » = retour à main. Pas de retour en arrière dans
  le fil de main.
- 2026-09-27 : une tâche est une session d'agent, pas forcément liée à git,
  et peut durer longtemps.
- 2026-09-27 : worktree seulement sur demande de l'utilisateur. Pas de
  `/land` (merge via GitHub), pas de worktrees préparés à l'avance.
- 2026-09-27 : aucune expiration (sessions, tâches, sauvegardes).
- 2026-09-27 : pas de `/push` ; on demande à l'agent dans la conversation.
