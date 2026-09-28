# RFC 0003 : Chaque agent peut parler à n'importe quel agent

- Statut : Brouillon
- Date : 2026-09-27
- Auteur : Gabriel Vergnaud
- Étend : Unified Harness, Internal capabilities & Plugins RFC, **D8**
  (sous-agents) et **D9** (notifications asynchrones)
- Utilisée par : [`rfc-0001-switchboard.md`](rfc-0001-switchboard.md)

Les mots **DOIT**, **NE DOIT PAS**, **DEVRAIT** et **PEUT** ont le sens de la
RFC 2119.

## 1. Résumé

D8 définit `tools.agent` : `list`, `spawn`, `wait`, `sendMessage`,
`interrupt`, `close`. Aujourd'hui, un agent ne voit et ne parle qu'à **ses
enfants**.

Cette RFC ouvre la **communication** à tous les agents d'un même groupe : un
agent peut lister les autres et envoyer un message à n'importe lequel,
parent, enfant ou pair. Le **contrôle** reste hiérarchique : seuls le parent
et l'humain peuvent créer, interrompre ou fermer un agent.

Elle ajoute :

- un expéditeur visible sur chaque message ;
- des identifiants de message et de fil, pour répondre et attendre une
  réponse ;
- des règles de livraison selon l'état du destinataire ;
- des garde-fous contre les boucles et les attentes croisées.

## 2. Vocabulaire

| Terme | Définition |
|---|---|
| **Groupe** | L'ensemble des agents qui peuvent se parler. Dans Switchboard : main et toutes les tâches d'un workspace. |
| **Parent** | L'agent qui a créé un agent avec `spawn`. Main n'a pas de parent. |
| **Pair** | Un agent du groupe qui n'est ni le parent ni un enfant de l'appelant. |
| **Humain** | L'utilisateur. Il n'est pas un agent et n'apparaît pas dans `list`. |
| **Message** | Un texte envoyé par un agent à un autre, avec un `messageId`. |
| **Fil** | Une suite de messages liés par `replyTo`. Il a un `threadId`. |

## 3. Principes

1. **Parler est ouvert, contrôler est hiérarchique.** Tout agent peut
   écrire à tout agent du groupe. Seuls le parent et l'humain peuvent
   `interrupt` ou `close` un agent.
2. **Un message d'agent n'a jamais l'autorité de l'humain.** Il est toujours
   marqué avec son expéditeur. Le destinataire le traite comme une demande,
   pas comme un ordre de l'utilisateur.
3. **Envoyer ne bloque jamais.** Attendre une réponse est un choix explicite
   (`wait`).
4. **Tout est journalisé et visible par l'humain.**

## 4. API côté modèle

Compatible avec D8 : les appels existants gardent leur sens.

```ts
namespace tools.agent {
  // D8 : listait les enfants. Maintenant : tout le groupe par défaut.
  declare function list(params?: {
    scope?: "group" | "children"; // défaut "group"
  }): Promise<{ agents: ListedAgent[] }>;

  // Inchangé. L'appelant devient le parent.
  declare function spawn(params: {
    agentName: string;
    message: string;
    agentType?: string;
  }): Promise<Result<void, string>>;

  // D8 : vers un enfant. Maintenant : vers n'importe quel agent du groupe.
  declare function sendMessage(params: {
    agentName: string;
    message: string;
    replyTo?: string;      // messageId auquel on répond
    expectReply?: boolean; // défaut false
  }): Promise<Result<{ messageId: string; threadId: string }, string>>;

  // D8 : attendre la fin du tour d'un enfant (inchangé).
  // Ajout : attendre la réponse à un message.
  declare function wait(
    params:
      | { agentName: string; timeoutMs: number } // enfants seulement
      | { messageId: string; timeoutMs: number },
  ): Promise<Result<string | WaitOutcome, string>>;

  // Nouveau : état déclaré par l'agent lui-même, visible dans list().
  declare function setStatus(params: {
    status: "working" | "done" | "blocked";
    note?: string;
  }): Promise<Result<void, string>>;

  // Parent et humain seulement.
  declare function interrupt(params: { agentName: string }): Promise<Result<void, string>>;
  declare function close(params: { agentName: string }): Promise<Result<void, string>>;
}

interface ListedAgent {
  agentName: string;
  description: string;           // rôle ou objectif, une ligne
  relation: "self" | "parent" | "child" | "peer";
  parentName: string | null;
  status: "starting" | "working" | "waiting" | "needs_approval"
        | "idle" | "done" | "blocked" | "failed" | "stopped" | "archived";
  statusNote?: string;           // dernière note de setStatus
}

type WaitOutcome =
  | { type: "reply"; from: string; messageId: string; message: string; auto: boolean }
  | { type: "incoming_request"; from: string; messageId: string; message: string };
```

`wait({ agentName })` sur un pair est refusé : le tour d'un pair peut servir
à quelqu'un d'autre. Pour un pair, on attend une réponse avec
`wait({ messageId })`.

## 5. Format dans le contexte du destinataire

Un message d'agent entre dans le contexte comme un message de rôle `user`
qui contient une balise. C'est le motif discuté sous D9 : le rôle `tool` est
impossible (pas de résultat d'outil différé), le rôle `system` donnerait trop
d'autorité.

```
<agent_message from="docs" relation="peer" id="m_42" thread="t_9"
               reply_to="m_40" expects_reply="true">
Tu utilises la v1 ou la v2 de l'API d'auth ?
</agent_message>
```

Niveaux d'autorité, que le prompt système de chaque agent DOIT expliquer :

| Source | Forme | Autorité |
|---|---|---|
| Humain (checkout, ou réponse à une carte) | message `user` sans balise, ou `from="user"` | Instruction |
| Parent | `relation="parent"` | Instruction dans le cadre de la tâche |
| Enfant ou pair | `relation="child"` ou `"peer"` | Demande. Le destinataire PEUT refuser si elle contredit sa tâche. |

Un message d'agent NE DOIT PAS pouvoir approuver un appel d'outil, changer
des permissions, ni modifier le rôle du destinataire. Les approbations
restent à l'humain (D8 : les sous-agents héritent de la politique de
permissions).

## 6. Livraison

### 6.1 Selon l'état du destinataire

| État du destinataire | Effet |
|---|---|
| `working` | Entrée en attente, insérée au prochain point sûr du tour (ADR 0005, D9). |
| `idle`, `done`, `blocked` | Démarre un nouveau tour. |
| `waiting` (dans un `wait`) | Voir §6.2. |
| `needs_approval` | Mis en file. Livré après la décision de l'humain. Sinon, il répondrait à la place de l'humain. |
| `starting` | Mis en file. Livré après le premier message du parent. |
| `failed`, `stopped`, `archived` | `sendMessage` renvoie l'erreur `recipient_unavailable` avec l'état. Rien n'est réveillé. |

Plusieurs messages en attente pour un même agent sont livrés ensemble, dans
l'ordre d'arrivée au runtime. L'ordre est garanti pour un même couple
(expéditeur, destinataire).

Un `messageId` déjà livré n'est jamais livré deux fois (même règle que
`HarnessNotification.id`).

### 6.2 Messages reçus pendant un `wait`

- Si le message est la réponse attendue, `wait` se termine avec
  `{ type: "reply" }`.
- Si le message attend une réponse (`expectReply: true`), `wait` se termine
  tout de suite avec `{ type: "incoming_request" }`. L'agent répond, puis
  PEUT relancer `wait`.
- Sinon, le message reste en file jusqu'à la fin du `wait`.

La deuxième règle empêche les attentes croisées : si A attend B pendant que
B attend A, et que les deux ont posé une question, les deux `wait` se
terminent et chacun peut répondre. Le runtime n'a pas besoin de détecter les
cycles.

## 7. Réponses

- Répondre, c'est `sendMessage` avec `replyTo`. La réponse reste dans le
  fil du message d'origine.
- **Réponse automatique.** Si un message avec `expectReply: true` a été livré
  et que le tour du destinataire se termine sans réponse à ce message, le
  runtime envoie le dernier message de l'assistant comme réponse, avec
  `auto: true`. Une question reçoit donc toujours une réponse.
- Une réponse automatique a toujours `expectReply: false`. Elle ne peut pas
  lancer une chaîne de réponses.
- Seule la première réponse termine un `wait`. Les suivantes arrivent comme
  des messages normaux.
- Si `wait` expire, il renvoie l'erreur `timeout`. Le message reste valable :
  une réponse tardive arrive comme un message normal.

## 8. Garde-fous

| Règle | Défaut | Effet quand la limite est atteinte |
|---|---|---|
| Messages d'agents consécutifs dans un fil, sans intervention humaine | 12 | `sendMessage` renvoie `thread_limit`. Une carte d'attention s'ouvre. Un message de l'humain à un participant du fil remet le compteur à zéro. |
| Tours réveillés par des pairs, par agent et par heure | 20 | Les messages suivants restent en file jusqu'au prochain tour démarré par l'humain ou le parent. |
| Envoi à soi-même | interdit | Erreur `invalid_recipient`. |
| Envoi à tout le groupe | absent | Pas de diffusion en v1. |

## 9. Visibilité

- Chaque message est un événement du journal (§10).
- Dans la vue d'un agent, les messages reçus et envoyés apparaissent à leur
  place, avec l'expéditeur ou le destinataire.
- Main n'est **pas** en copie des messages entre pairs, pour éviter le coût
  et le bruit. Il voit un résumé des fils actifs, calculé sans LLM, dans
  son tableau des tâches (RFC 0001, §8.2) :

```
<agent_threads>
t_9  docs ↔ auth-fix   4 messages  ouvert   "v1 ou v2 de l'API d'auth ?"
t_11 bench → docs      1 message   répondu  "Chiffres du parseur"
</agent_threads>
```

- Main PEUT lire un fil complet avec `task_inspect` (RFC 0001).

## 10. Événements

```ts
type AgentMessageEvent =
  | { type: "agent_message_sent"; message_id: string; thread_id: string; from: string;
      to: string; reply_to?: string; expects_reply: boolean; auto: boolean; text: string }
  | { type: "agent_message_queued"; message_id: string; reason: "recipient_busy" | "needs_approval" | "starting" | "wake_limit" }
  | { type: "agent_message_delivered"; message_id: string }
  | { type: "agent_message_rejected"; message_id: string; error: "recipient_unavailable" | "thread_limit" | "invalid_recipient" }
  | { type: "agent_status_declared"; agent: string; status: "working" | "done" | "blocked"; note?: string };
```

## 11. Changements au protocole du Unified Harness

Noms de `spec/step-protocol.ts`. D8 dit `close`, la spec dit `subagent.stop`.

| Élément | Changement |
|---|---|
| `AgentListToolInput` | Ajouter `scope?: "group" \| "children"`. |
| `AgentRuntimeState` | Ajouter `parentName`, `relation`, `description`, `statusNote`, et les états `waiting`, `needs_approval`, `done`, `blocked`, `archived`. |
| `AgentMessageToolInput` | Ajouter `replyTo?`, `expectReply?`. La sortie en succès renvoie `messageId` et `threadId`. |
| `AgentWaitToolInput` | Union avec `{ messageId, timeoutMs }`. |
| Nouvel outil | `subagent.set_status`. |
| `HarnessNotificationSource` | Ajouter `{ type: "agent_message"; from; message_id; thread_id; reply_to?; expects_reply }`. Le Core rend la notification comme le message balisé du §5. |
| Runtime | Un annuaire du groupe (toutes les sessions, pas seulement les enfants), le routage entre sessions, les compteurs du §8, la réponse automatique du §7. |

Le Core ne change presque pas : il insère déjà les notifications aux points
sûrs. Le travail est surtout dans le Runtime, qui doit connaître tout le
groupe.

## 12. Cas limites

| Cas | Comportement |
|---|---|
| A et B se posent une question en même temps, puis attendent | Les deux `wait` se terminent avec `incoming_request`. Chacun répond. |
| Le destinataire est renommé | L'ancien nom reste un alias (RFC 0001, §9.3). |
| Le runtime redémarre avec des messages en file | Ils sont dans le journal et livrés après la reprise. Les `wait` en cours se terminent avec `interrupted_by_restart`. |
| L'humain ouvre le checkout de B pendant qu'un message de A est en file | La vue de B affiche le message en attente et sa raison. |
| Un pair demande à B de fermer C | B ne peut pas : seul le parent de C ou l'humain le peut. B peut transmettre la demande à main. |
| Réponse à un message inconnu (`replyTo` invalide) | Erreur. Le message n'est pas envoyé. |

## 13. Alternatives étudiées

- **Main en copie de tous les messages.** Rejeté : chaque message coûterait
  un tour de main.
- **Canal de groupe partagé (style Grok Bot).** Rejeté pour la v1 : tous les
  agents lisent tout, et plusieurs répondent au même message.
- **Messages sans corrélation (style hcom).** Plus simple, mais un agent ne
  peut pas attendre une réponse précise. Rejeté.
- **A2A.** Fait pour des agents sur des services différents. Ici, tous les
  agents sont dans le même runtime.

## 14. Questions ouvertes

1. Faut-il que l'humain soit adressable (`agentName: "user"`) ? Aujourd'hui,
   un agent passe par main, qui ouvre une carte d'attention.
2. Les limites du §8 (12 messages, 20 réveils par heure) : bonnes valeurs ?
3. Un pair doit-il pouvoir réveiller une tâche `done` ? Aujourd'hui : oui,
   comme l'humain.
4. Faut-il autoriser un pair à créer des agents (profondeur > 1) ?
