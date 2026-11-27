# **Semaine 2 — Architecture Ethereum**

L'objectif de cette semaine est de comprendre **comment un nœud Ethereum fonctionne réellement**, comment il communique avec les autres nœuds et comment il valide les transactions et les blocs.

Pour comprendre cette architecture, il faut retenir une idée fondamentale :

**Ethereum utilise deux couches complémentaires :**

- Execution Layer (EL) : exécute les transactions et gère l'état de la blockchain.
- Consensus Layer (CL) : organise les validateurs et garantit l'accord entre les nœuds sur les blocs.

# **1. Architecture générale d'Ethereum**

Depuis le passage de Proof of Work à Proof of Stake (The Merge, septembre 2022), Ethereum utilise une architecture séparée en deux couches.

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/k-1X9TcAnGnQHoJ_aqeIFmQ4aKRZKjNnd92Efb3sceakcGF8XtBXrrdPJCNCFW1uCXG9K8YLjLtIv8FhiB5_yJpn-DN_K3BMfh01_Fjm0bht1WnYfYZteW6C0nXm8DafOC3QzbgxBP3TEAnWG4pNmXj0820A0Hk0e2UmR3dODpg?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node/?utm_source=chatgpt.com)

## **1.1 Execution Layer (EL)**

L'Execution Layer est responsable de l'exécution des transactions et des Smart Contracts.

Son rôle est de répondre aux questions suivantes :

- Est-ce que cette transaction est valide ?
- L'utilisateur possède-t-il suffisamment d'ETH ?
- Le nonce est-il correct ?
- Combien de gas faut-il consommer ?
- Quel sera le nouvel état de la blockchain après l'exécution ?

**Exemple :**

Alice veut envoyer 1 ETH à Bob.

```
Alice
  |
  | Transaction : envoyer 1 ETH
  v
Execution Layer
  |
  | Vérification :
  | - Signature
  | - Balance
  | - Nonce
  | - Gas
  |
  v
Exécution de la transaction
  |
  v
Mise à jour des balances
```

Après l'exécution :

- Balance Alice : -1 ETH (plus les frais).
- Balance Bob : +1 ETH.

L'Execution Layer maintient également l'état Ethereum (balances, contrats, stockage, etc.).

### **Geth**

[Go-ethereum | Learn Netdata](https://images.openai.com/static-rsc-4/5YGaHuuemvsMJLPPerO8qFXtgrBMWWB4EcKbzhoXiVDtQMFEgNDu1TkSYBgq1AZixOu9S2TbCeHWtb_pJvYH6S_eUOvXIkK18SR4-NL0W8-3f6wH90DRXekUNZ2fgeYPdle7UNRv2umOopfB6iPBJXFQTWy7Y5_TGHxtgk8oREs?purpose=inline)

**Geth (Go Ethereum)**

Client d'exécution Ethereum écrit en Go.

Execution Layer Client

[Documentation officielle Geth](https://geth.ethereum.org/?utm_source=chatgpt.com)

Geth permet de :

- Recevoir des transactions.
- Vérifier les transactions.
- Exécuter les Smart Contracts.
- Maintenir l'état Ethereum.
- Construire et valider des blocs d'exécution.
- Exposer une API JSON-RPC.

Quelques commandes :

```
geth version
```

Lancer un nœud de développement :

```
geth --dev
```

Afficher les informations RPC :

```
curl -X POST http://127.0.0.1:8545 \
-H "Content-Type: application/json" \
-d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

## **1.2 Consensus Layer (CL)**

Le Consensus Layer est responsable du consensus entre les validateurs Ethereum.

Il répond à une question fondamentale :

**Comment les nœuds Ethereum arrivent-ils à se mettre d'accord sur les blocs qui seront ajoutés à la blockchain ?**

Ethereum utilise le mécanisme Proof of Stake (PoS).

Les validateurs doivent déposer 32 ETH pour participer directement à la validation en tant que validateurs indépendants.

Le Consensus Layer gère notamment :

- Les validateurs.
- Les propositions de blocs.
- Les attestations.
- Les finalités (Finality).
- La synchronisation de la Beacon Chain.
- Le choix de la chaîne canonique.

### **Exemple simplifié**

Imaginons trois validateurs :

```
Validator A
Validator B
Validator C
```

Un validateur propose un bloc.

Les autres validateurs vérifient ce bloc et envoient des attestations.

```
           Proposer un bloc
                  |
                  v
             Block #100
                  |
          +-------+-------+
          |       |       |
          v       v       v
       Node A  Node B  Node C
          |       |       |
          +-------+-------+
                  |
                  v
          Consensus Ethereum
                  |
                  v
             Finalisation
```

### **Lighthouse**

[GitHub - sigp/lighthouse: Ethereum consensus client in Rust · GitHub](https://images.openai.com/static-rsc-4/uqQTVdrl55m90gupkroT9rpJQOdBAET7VzGvzl1b4gswxAI812Aus0oLMmuqAcp0CP419DZERsK1S1uH3ta1fwT1LzJC_NgoCDQpk_r0CiOMWl4ES3sumcUjDleS7c9ZILQD-wQ5yikG4g-s_xk7AxNNtF1Im_lrGQWVkv7mq5A?purpose=inline)

**Lighthouse**

Client Consensus Layer développé en Rust par Sigma Prime.

Consensus Layer Client

[Documentation officielle Lighthouse](https://lighthouse-book.sigmaprime.io/?utm_source=chatgpt.com)

Lighthouse permet de :

- Synchroniser la Beacon Chain.
- Gérer les informations des validateurs.
- Recevoir et vérifier les blocs Beacon.
- Participer au consensus Proof of Stake.
- Envoyer et vérifier les attestations.
- Communiquer avec le client d'exécution.

Quelques commandes :

```
lighthouse --version
```

Lancer un nœud Beacon :

```
lighthouse bn \
  --network mainnet \
  --execution-endpoint http://127.0.0.1:8551 \
  --execution-jwt /path/to/jwt.hex
```

Cette commande est un exemple de configuration : le client d'exécution doit également être configuré pour communiquer avec Lighthouse.

# **2. Geth et Lighthouse : comment travaillent-ils ensemble ?**

C'est une partie très importante pour comprendre ton projet EVA-Guard.

Geth et Lighthouse ne sont pas deux alternatives. Ils travaillent ensemble.

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/t5PHAFvPXBLjryYLjPYCbxFZGF-S4SJezwHaf4BsLoBmd23ofRAGqWqo8HqllYgwkhUUSlZcEMpPubJ9Jp7mZpYzQixmo6PzK3TGZNs_8ngMWQv9Db17ZhJbgVn66v1-E06F_xH56tkPXJ99dwFBceEMRx2kld4FNV0enIM18lY?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node?utm_source=chatgpt.com)

Considérons un nœud Ethereum composé de deux processus.

| Composant  | Responsabilité                               |
| ---------- | -------------------------------------------- |
| Geth       | Exécution des transactions                   |
| Lighthouse | Consensus et Beacon Chain                    |
| Engine API | Communication entre Geth et Lighthouse       |
| JSON-RPC   | Communication avec les applications externes |

### **Exemple concret**

Supposons qu'un utilisateur envoie une transaction :

```
Alice envoie 2 ETH à Bob
```

Voici le déroulement :

**Étape 1 : réception**

Geth reçoit la transaction via JSON-RPC ou le réseau P2P.

**Étape 2 : vérification et exécution**

Geth vérifie la transaction et peut l'exécuter pour préparer un bloc.

**Étape 3 : consensus**

Lighthouse organise la proposition du bloc dans le cadre du consensus Proof of Stake.

**Étape 4 : communication**

Lighthouse demande à Geth de construire un bloc d'exécution via Engine API.

**Étape 5 : validation**

Lighthouse vérifie les informations nécessaires et participe au processus de consensus.

**Étape 6 : mise à jour**

Le bloc accepté est intégré à la chaîne canonique et les deux clients mettent à jour leurs données respectives.

# **3. Beacon Chain**

[How to run an Ethereum node after the merge | by Ariel Sandez | Truebit | Medium](https://images.openai.com/static-rsc-4/JaiJfOoVqYuYIKfoW2qCMTghYOYgEMIW043ergnopqhzCNY45YEVSh6bRalgOLHMJRchxJSN0FEMDMwSQrCtvsLegzz-B_ra7xdOA7A-j7tVi6NqEkEK7qLRmKVB7FIkDq-WLVAg6XfVza0ShpzvcHbM7Hi3Jpf2FbjU_Vqfs7w?purpose=inline)

[medium.com](https://medium.com/truebit/how-to-run-an-ethereum-node-after-the-merge-da20a239d007?utm_source=chatgpt.com)

La Beacon Chain est la blockchain du Consensus Layer.

Elle a été introduite avant The Merge pour permettre à Ethereum de fonctionner avec Proof of Stake.

Elle est maintenant intégrée à l'architecture Ethereum actuelle.

### **3.1 Slot**

Un slot est une période de 12 secondes.

Pendant chaque slot, un validateur est sélectionné pour proposer un bloc.

```
Slot 0       Slot 1       Slot 2       Slot 3
  |            |            |            |
  v            v            v            v
 12 sec       12 sec       12 sec       12 sec
```

Attention : un bloc n'est pas nécessairement proposé à chaque slot.

### **3.2 Epoch**

Une epoch contient 32 slots.

Donc :

\\[ 32 \times 12 = 384 \text{ secondes} \\]

Une epoch dure environ 6 minutes et 24 secondes.

**Organisation temporelle**

**1 Slot**

12 secondes

**1 Epoch**

32 slots

32 × 12 secondes = 6 min 24 s

### **3.3 Attestation**

Une attestation est un vote effectué par un validateur concernant la chaîne qu'il considère comme correcte.

Les validateurs contribuent ainsi au consensus.

### **3.4 Finality**

La finalité signifie qu'un bloc est considéré comme finalisé par le protocole.

Une fois finalisé, revenir sur ce bloc nécessiterait des conditions extrêmement coûteuses et pénalisantes pour les validateurs.

**À retenir :**

- Slot : période de 12 secondes.
- Epoch : 32 slots.
- Attestation : vote d'un validateur.
- Finality : finalisation cryptoeconomique de la chaîne.

# **4. Engine API**

C'est probablement le concept le plus important pour comprendre la communication entre Geth et Lighthouse.

L'Engine API est une interface interne permettant au client Consensus de communiquer avec le client Execution.

[Ethereum 101: Node Architecture, Explained | by 0s & 1s — All About Software ✨ | Medium](https://images.openai.com/static-rsc-4/4QIKRLCRbguiGkljhcM-BtsHa7xjVmxdhE8gJf_u7G6Zl9bgma5ESZ-n08XVdx8oo7Ao1cMg7gojRPIz_AzVmWF7FGBvDSrTqVsh0XAWbtPWA7j0-5nxCPMyGo4KBZDp5CUA86pTmCbZXiF-m9TABjT22FPRofCDI-MetPhg5aA?purpose=inline)

[medium.com](https://medium.com/%400s.and.1s/ethereum-101-node-architecture-explained-e9012239afc2?utm_source=chatgpt.com)

### **Pourquoi Engine API ?**

Lighthouse ne sait pas exécuter directement les transactions Ethereum.

Geth ne gère pas seul le consensus Proof of Stake.

Il faut donc une interface de communication entre les deux.

Cette interface est Engine API.

### **Comment fonctionne-t-elle ?**

Voici quelques méthodes importantes :

| Méthode                      | Fonction                                                                              |
| ---------------------------- | ------------------------------------------------------------------------------------- |
| `engine_forkchoiceUpdatedV3` | Communique le choix de la chaîne canonique et peut demander la construction d'un bloc |
| `engine_getPayloadV3`        | Récupère un bloc d'exécution préparé                                                  |
| `engine_newPayloadV3`        | Soumet un nouveau payload d'exécution pour validation                                 |

Le suffixe V3 correspond à une version de l'interface ; d'autres versions peuvent être utilisées selon les forks Ethereum et les capacités des clients.

### **Exemple simplifié**

```
         Lighthouse
              |
              |
     Engine API (JWT)
              |
              |
             Geth
              |
              v
      Execution Payload
              |
              v
       Transactions
```

L'Engine API utilise généralement :

- HTTP.
- JSON-RPC.
- Authentification JWT.
- Un endpoint dédié, souvent sur le port 8551.

Exemple de configuration Geth :

```
geth \
  --http \
  --http.addr 127.0.0.1 \
  --http.port 8545 \
  --authrpc.addr 127.0.0.1 \
  --authrpc.port 8551 \
  --authrpc.jwtsecret /path/to/jwt.hex
```

Le port 8545 est généralement utilisé pour JSON-RPC et le port 8551 pour Engine API.

# **5. JSON-RPC**

JSON-RPC est une interface qui permet aux applications de communiquer avec Geth.

Par exemple :

- MetaMask.
- Foundry.
- DApps.
- Scripts Python.
- Outils de monitoring.

Contrairement à Engine API, JSON-RPC est principalement destiné aux applications et aux utilisateurs qui souhaitent interagir avec le client d'exécution.

### **Exemple 1 : récupérer le dernier bloc**

```
curl -X POST http://127.0.0.1:8545 \
-H "Content-Type: application/json" \
-d '{
  "jsonrpc": "2.0",
  "method": "eth_blockNumber",
  "params": [],
  "id": 1
}'
```

### **Exemple 2 : récupérer le Chain ID**

```
curl -X POST http://127.0.0.1:8545 \
-H "Content-Type: application/json" \
-d '{
  "jsonrpc": "2.0",
  "method": "eth_chainId",
  "params": [],
  "id": 1
}'
```

### **Exemple 3 : récupérer le solde d'une adresse**

```
curl -X POST http://127.0.0.1:8545 \
-H "Content-Type: application/json" \
-d '{
  "jsonrpc": "2.0",
  "method": "eth_getBalance",
  "params": [
    "0x0000000000000000000000000000000000000000",
    "latest"
  ],
  "id": 1
}'
```

### **Différence entre JSON-RPC et Engine API**

| Critère          | JSON-RPC                 | Engine API          |
| ---------------- | ------------------------ | ------------------- |
| Communication    | Application → Geth       | Lighthouse → Geth   |
| Utilisateur      | DApps, MetaMask, Foundry | Client Consensus    |
| Authentification | Selon configuration      | JWT                 |
| Port habituel    | 8545                     | 8551                |
| Objectif         | Interagir avec Ethereum  | Coordonner EL et CL |

# **6. Architecture complète d'un nœud Ethereum**

Voici l'architecture que tu dois maîtriser.

[The Gem of Pectra: EIP-7702, the SetCode Transaction | by Gonzalo Bustos | Medium](https://images.openai.com/static-rsc-4/C98wqFDdfd3D-lRFVzaPP7dzLGhT9lOoFrfKZCwqfY5KL2Zk0o6mf6EKG6TbZnYJ1m8_74aw1V1ukxSF724BH-rQIX3y6UepunGZACIXGigoLTV4uOx6PG0dotHoqhqCvyE7tEoyx3hF_1eprEMgXmm1oOX6bxqdzC51DGKJ4l0?purpose=inline)

[medium.com](https://medium.com/%40bustosgonzalom/the-gem-of-pectra-eip-7702-the-setcode-transaction-e55593b0533a?utm_source=chatgpt.com)

On peut la représenter ainsi :

```
                  USER / DAPP
                       |
                       |
                   JSON-RPC
                       |
                       v
              +----------------+
              |      GETH      |
              | Execution Layer|
              +----------------+
                       |
                       |
                   Engine API
                       |
                       |
              +----------------+
              |   LIGHTHOUSE   |
              | Consensus Layer|
              +----------------+
                       |
                       |
                BEACON CHAIN
                       |
                       |
                PROOF OF STAKE
                       |
                       v
                   VALIDATORS
```

Il faut également comprendre que Geth et Lighthouse disposent chacun de communications P2P avec leurs réseaux respectifs.

# **7. Application directe à ton projet EVA-Guard**

Maintenant, relions ces connaissances à ton architecture de détection des attaques Ethereum.

Tu utilises Geth et Lighthouse comme sources de données pour ton moteur de monitoring Rust.

[Guidance for Automating Ethereum Node Validator using AWS Graviton](https://images.openai.com/static-rsc-4/XC0zJEbI_Cgy-gYO1_Ti7mI3ssvXuTUYfeJmwZGGgKj3zUtDLmiSeDCNL2TnlqmCp7Dw54hYgm8aiF5i8fXQO5_Byaw41Wwh54istoYDCoBbMqIW7iVa3SovifS-zEfQBVS4TQxb13APH0-oEj2rJ3WlXGmF0cQjDxKXY5ijTx0?purpose=inline)

[aws.amazon.com](https://aws.amazon.com/solutions/guidance/automating-ethereum-node-validator-using-aws-graviton/?utm_source=chatgpt.com)

Dans ton projet :

| Composant              | Rôle dans EVA-Guard                        |
| ---------------------- | ------------------------------------------ |
| Geth                   | Fournit les métriques Execution Layer      |
| Lighthouse             | Fournit les métriques Consensus Layer      |
| Rust Monitoring Engine | Collecte les métriques des nœuds           |
| ML Model               | Détecte NORMAL, ECLIPSE, RPC_FLOODING      |
| Agent 1                | Analyse les alertes et hypothèses          |
| RAG                    | Fournit les connaissances sur les attaques |
| Agent 2                | Confirme et explique les attaques          |

### **Exemple : attaque RPC Flooding**

Un attaquant envoie un grand nombre de requêtes RPC à Geth.

```
Attacker
   |
   | 10000 RPC requests
   v
Geth RPC
   |
   | Latence augmente
   | CPU augmente
   | RPC requests/sec augmente
   v
Rust Monitoring
   |
   v
ML Detection
   |
   v
RPC_FLOODING
```

### **Exemple : attaque Eclipse**

Un nœud Ethereum est isolé de ses pairs honnêtes et ses connexions P2P sont manipulées.

Les métriques intéressantes à surveiller comprennent :

- `geth_peer_count`
- `geth_new_peers`
- `geth_disconnected_peers`
- `geth_peer_churn_count`
- `beacon_peer_count`
- `beacon_is_syncing`

C'est pourquoi il est important de surveiller les deux couches et pas uniquement Geth.

# **8. Travaux pratiques : maîtriser cette architecture**

Je te propose de réaliser ces exercices dans l'ordre.

## **TP — Architecture Ethereum**

Objectif : comprendre et observer un nœud Ethereum réel.

**Progression**

# **0%**

**TP 1 — Découvrir Geth**

Installer Geth, vérifier sa version et tester JSON-RPC.

**TP 2 — Découvrir Lighthouse**

Installer Lighthouse et comprendre le fonctionnement du Beacon Node.

**TP 3 — Connecter Geth et Lighthouse**

Configurer Engine API, JWT et les endpoints.

**TP 4 — Observer la Beacon Chain**

Étudier les slots, epochs, validateurs et attestations.

**TP 5 — Tester JSON-RPC**

Interroger eth_blockNumber, eth_chainId et eth_getBalance.

**TP 6 — Monitoring**

Collecter les métriques Geth et Lighthouse avec ton moteur Rust.

# **9. Questions pour vérifier ta compréhension**

Essaie de répondre sans regarder les explications.

---------------------------------------------------------
Très bonne question ! Il y a une nuance importante à comprendre : **Geth ne propose pas directement les blocs à Lighthouse. C'est Lighthouse qui organise la proposition du bloc, tandis que Geth construit la partie Execution du bloc.**

Pour comprendre, il faut distinguer deux types de blocs.

# **1. Les deux types de blocs dans Ethereum**

Ethereum Proof of Stake possède deux structures complémentaires.

## **A. Execution Block (bloc d'exécution)**

C'est le bloc construit par Geth.

Il contient notamment :

- Les transactions Ethereum.
- Le résultat de l'exécution des transactions.
- Le State Root (état résultant de la blockchain).
- Le Transactions Root.
- Le Receipts Root.
- Les informations relatives au gas.

Exemple :

```
Execution Block #100
|
|-- Transaction 1 : Alice -> Bob (1 ETH)
|-- Transaction 2 : Bob -> Charlie (0.5 ETH)
|-- Transaction 3 : Smart Contract execution
|
|-- State Root
|-- Transactions Root
|-- Receipts Root
|-- Gas Used
```

Geth est responsable de construire et vérifier ce bloc d'exécution.

## **B. Beacon Block (bloc de consensus)**

C'est Lighthouse qui organise et propose le bloc Beacon.

Il contient notamment :

- Les informations de consensus.
- Le slot.
- Le proposer index.
- Les attestations.
- Les informations relatives aux validateurs.
- Un Execution Payload contenant le bloc d'exécution fourni par Geth.

Autrement dit, le Beacon Block contient le résultat de la partie consensus et embarque les données du bloc d'exécution.

[proof of stake - What exactly is the content of blocks on the beacon chain? - Ethereum Stack Exchange](https://images.openai.com/static-rsc-4/GoZrMkqmoxk7pUGZ0pI8ejlTutAbJjAK7KGh7V7nJUFFnVmbTxn7NmyYyCSFkWXB24BojPCSKKK4nAKG4ozXIF96gJHPlrfi5oFwwi2qormITDXzLTUpFgmy1-xBdkHjIBDeBYesD5C5IW75mVmSC1fQPvTwsixKu8Ih1sLuIMY?purpose=inline)

[ethereum.stackexchange.com](https://ethereum.stackexchange.com/questions/115594/what-exactly-is-the-content-of-blocks-on-the-beacon-chain?utm_source=chatgpt.com)

# **2. Comment Geth et Lighthouse construisent-ils un bloc ensemble ?**

Prenons un exemple : Ethereum doit produire le bloc du slot 1000.

## **Processus de construction d'un bloc**

1. **Lighthouse sélectionne le proposant**

   Le protocole sélectionne un validateur pour proposer le bloc du slot 1000.
2. **Lighthouse demande à Geth de construire le bloc d'exécution**

   Il utilise l'Engine API :

   `engine_forkchoiceUpdatedV3`

   Geth commence à préparer un Execution Payload.
3. **Geth construit l'Execution Payload**

   Geth sélectionne les transactions de son mempool, les exécute et prépare le bloc d'exécution.
4. **Lighthouse récupère le payload**

   Il appelle :

   `engine_getPayloadV3`

   Geth retourne le payload construit.
5. **Lighthouse construit le Beacon Block**

   Il intègre le payload d'exécution dans le bloc Beacon avec les informations de consensus nécessaires.
6. **Le bloc est proposé au réseau**

   Lighthouse diffuse le Beacon Block aux autres nœuds via le réseau P2P.

# **3. Architecture de communication**

Voici la relation exacte entre les deux clients :

[Lido Explained, Part 2: Consensus & Withdraw](https://images.openai.com/static-rsc-4/W7VtV0h2m0YTcIi9tGSXRwhn57iuDPXg1dvIcuZ6Ckia6vzf8vDxACPy_S7os7I1eAwWHVNVwA8jtN6J7qDfusMIFkimMMnhhVM8LYhZNImOwFcR_t6AjyL_5KcmClSb2DMis_zcxzlLJX3ZmbMCJLmklnZs6We6-MDsqi4iVK0?purpose=inline)

[oxor.io](https://oxor.io/blog/2024-02-22-lido-explained-part-2-consensus-withdraw/?utm_source=chatgpt.com)

```
             VALIDATOR
                 |
                 v
             LIGHTHOUSE
          (Consensus Layer)
                 |
                 | 1. forkchoiceUpdated
                 v
               GETH
          (Execution Layer)
                 |
                 | 2. Construire le bloc
                 |
                 v
          EXECUTION PAYLOAD
                 |
                 | 3. getPayload
                 v
             LIGHTHOUSE
                 |
                 | 4. Construire Beacon Block
                 |
                 v
             BEACON BLOCK
                 |
                 | 5. Broadcast P2P
                 v
          OTHER ETHEREUM NODES
```

# **4. Et si Lighthouse reçoit un bloc d'un autre nœud ?**

C'est le processus inverse : la validation.

Supposons que le nœud A propose un Beacon Block.

Le nœud B reçoit ce bloc via Lighthouse.

Voici ce qui se passe :

1. Lighthouse vérifie les règles du consensus.
2. Il extrait l'Execution Payload.
3. Il transmet ce payload à Geth via `engine_newPayloadV3`.
4. Geth vérifie et exécute le payload.
5. Geth retourne le résultat de validation.
6. Lighthouse utilise ce résultat dans le traitement du bloc Beacon.

Donc :

- Lighthouse vérifie les règles du consensus.
- Geth vérifie les règles d'exécution.
- Les deux doivent coopérer pour accepter correctement le bloc.

# **5. Différence entre les méthodes Engine API**

| Méthode                      | Fonction                                                                           |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| `engine_forkchoiceUpdatedV3` | Informe Geth du choix de la chaîne et peut déclencher la construction d'un payload |
| `engine_getPayloadV3`        | Récupère le payload construit par Geth                                             |
| `engine_newPayloadV3`        | Soumet un payload reçu pour validation par Geth                                    |

Attention : ces méthodes sont utilisées dans différents moments du cycle de vie des blocs. Elles ne constituent pas simplement une séquence unique dans tous les cas.

# **6. Application à EVA-Guard**

Cette distinction est particulièrement importante pour ton système de monitoring.

Tu dois surveiller les deux couches :

| Couche     | Métriques intéressantes                                         |
| ---------- | --------------------------------------------------------------- |
| Geth       | block number, transactions, gas, RPC latency, peer count        |
| Lighthouse | slot, epoch, beacon head, attestations, sync status, peer count |

Par exemple :

- Si Geth ne produit plus de payloads correctement, tu peux avoir un problème Execution Layer.
- Si Lighthouse ne parvient plus à proposer ou synchroniser les Beacon Blocks, tu peux avoir un problème Consensus Layer.
- Si les deux clients ne communiquent plus via Engine API, le nœud peut rencontrer un problème de coordination EL/CL.

----------------------------------------------------------
La **Beacon Chain** est une composante fondamentale de l'architecture Ethereum. Elle est responsable de la gestion du **Consensus Layer**, c'est-à-dire le mécanisme qui permet aux nœuds Ethereum de se mettre d'accord sur les blocs et l'état de la blockchain.

Pour bien comprendre, imagine que la Beacon Chain est le cerveau qui coordonne les validateurs d'Ethereum.

## **1. Pourquoi Ethereum a créé la Beacon Chain ?**

Avant septembre 2022, Ethereum utilisait le mécanisme **Proof of Work (PoW)** :

- Les mineurs validaient les transactions.
- Ils utilisaient leur puissance de calcul.
- Le réseau consommait beaucoup d'énergie.

En septembre 2022, Ethereum a réalisé *The Merge* et est passé à **Proof of Stake (PoS)**.

La Beacon Chain a été introduite pour gérer ce nouveau mécanisme de consensus.

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/k-1X9TcAnGnQHoJ_aqeIFmQ4aKRZKjNnd92Efb3sceakcGF8XtBXrrdPJCNCFW1uCXG9K8YLjLtIv8FhiB5_yJpn-DN_K3BMfh01_Fjm0bht1WnYfYZteW6C0nXm8DafOC3QzbgxBP3TEAnWG4pNmXj0820A0Hk0e2UmR3dODpg?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node/?utm_source=chatgpt.com)

## **2. Quel est le rôle de la Beacon Chain ?**

Elle possède plusieurs responsabilités.

| Responsabilité        | Explication                        |
| --------------------- | ---------------------------------- |
| Validators management | Gérer les validateurs Ethereum     |
| Proof of Stake        | Gérer le mécanisme PoS             |
| Block proposals       | Organiser la proposition des blocs |
| Attestations          | Recevoir les votes des validateurs |
| Fork choice           | Déterminer la chaîne canonique     |
| Finality              | Déterminer les blocs finalisés     |

### **A. Gestion des validateurs**

Un validateur est un participant qui contribue au consensus Ethereum.

Pour devenir validateur sur le réseau principal, il faut déposer 32 ETH (pour un validateur standard).

Son rôle :

- Proposer des blocs lorsqu'il est sélectionné.
- Voter (attester) pour les blocs proposés par d'autres validateurs.
- Participer à la sécurité du réseau.

### **B. Les Slots et les Epochs**

La Beacon Chain organise le temps en deux unités :

**Slot**

# **12 secondes**

Un slot est une période pendant laquelle un validateur peut proposer un bloc.

**Epoch**

# **32 slots**

Une epoch contient 32 slots, soit 6 minutes et 24 secondes.

Epoch

32 slots

Chaque petit rectangle représente un slot.

## **3. Comment fonctionne la Beacon Chain ?**

Voici un scénario simplifié :

**1. Transactions**

Les utilisateurs envoient des transactions Ethereum.

**2. Geth – Execution Layer**

Exécute les transactions et construit le bloc d'exécution.

**3. Lighthouse – Consensus Layer**

Gère les validateurs, les propositions et les attestations.

**4. Beacon Chain**

Coordonne le consensus et le choix de la chaîne canonique.

**5. Finality**

Les blocs deviennent finalisés selon les règles du consensus.

Attention : ce schéma est conceptuel. Geth et Lighthouse communiquent via l'Engine API. La Beacon Chain est aujourd'hui intégrée au fonctionnement du Consensus Layer, et non un service indépendant qui se trouve simplement après Lighthouse.

## **4. Quel est le rapport entre Beacon Chain, Geth et Lighthouse ?**

C'est particulièrement important pour ton projet EVA-Guard.

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/t5PHAFvPXBLjryYLjPYCbxFZGF-S4SJezwHaf4BsLoBmd23ofRAGqWqo8HqllYgwkhUUSlZcEMpPubJ9Jp7mZpYzQixmo6PzK3TGZNs_8ngMWQv9Db17ZhJbgVn66v1-E06F_xH56tkPXJ99dwFBceEMRx2kld4FNV0enIM18lY?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node?utm_source=chatgpt.com)

| Composant    | Fonction                               |
| ------------ | -------------------------------------- |
| Geth         | Execution Client                       |
| Lighthouse   | Consensus Client                       |
| Beacon Chain | Logique de consensus Proof of Stake    |
| Engine API   | Communication entre Geth et Lighthouse |
| JSON-RPC     | Interface pour interagir avec Geth     |

**Exemple concret :**

1. Geth reçoit et exécute les transactions.
2. Lighthouse prépare une proposition de bloc.
3. Lighthouse communique avec Geth via Engine API.
4. Geth exécute les transactions du payload.
5. Lighthouse organise les attestations des validateurs.
6. Le consensus détermine la chaîne canonique et sa finalité.

## **5. Dans EVA-Guard, pourquoi surveiller la Beacon Chain ?**

Tu surveilles déjà Geth et Lighthouse avec ton moteur Rust.

Voici les métriques importantes à surveiller :

| Métrique                  | Ce qu'elle permet de détecter        |
| ------------------------- | ------------------------------------ |
| Beacon peer count         | Isolement réseau                     |
| Head slot                 | Retard de synchronisation            |
| Finalized epoch           | Problèmes de finalité                |
| Attestation participation | Faible participation des validateurs |
| Sync status               | Désynchronisation                    |
| Beacon API latency        | Dégradation des performances         |

Par exemple, si un nœud possède un `head_slot` très inférieur aux autres nœuds, cela peut indiquer un problème de synchronisation ou une attaque de type Eclipse.

## **6. Comment explorer la Beacon Chain avec Lighthouse ?**

Si ton nœud Lighthouse expose son API Beacon sur le port `5052`, tu peux exécuter :

**1. Vérifier la synchronisation**

```
curl http://127.0.0.1:5052/eth/v1/node/syncing
```

**2. Consulter les informations du nœud**

```
curl http://127.0.0.1:5052/eth/v1/node/identity
```

**3. Consulter la tête actuelle de la Beacon Chain**

```
curl http://127.0.0.1:5052/eth/v1/beacon/headers/head
```

**4. Consulter les validateurs**

```
curl http://127.0.0.1:5052/eth/v1/beacon/states/head/validators
```

## **7. Ce que tu dois retenir**

- **Beacon Chain** : coordonne le consensus Ethereum Proof of Stake.
- **Lighthouse** : implémente le Consensus Client et les protocoles associés.
- **Geth** : implémente l'Execution Client.
- **Engine API** : permet à Lighthouse et Geth de communiquer.
- **Validators** : participent au consensus en proposant et en attestant les blocs.

**Pour maîtriser Ethereum**, je te conseille ensuite d'étudier en profondeur les trois concepts suivants :

1. Slots, Epochs et Validators.
2. Attestations, Fork Choice et LMD-GHOST.
3. Casper FFG et Finality.

Ce sont les mécanismes essentiels pour comprendre comment Ethereum parvient à un consensus distribué sans Proof of Work.
----------------------------------------------------
Excellente question. Il faut distinguer deux choses : **où les données de la blockchain Ethereum sont stockées et quel composant est responsable de leur enregistrement.**

La blockchain Ethereum n'est pas stockée dans un seul serveur. Elle est répliquée et maintenue par les différents nœuds du réseau.

## **1. Où sont enregistrés les événements Ethereum ?**

Dans Ethereum, les données sont principalement conservées par les Execution Clients comme **Geth**.

Geth stocke notamment :

- Les blocs.
- Les transactions.
- Les reçus de transactions (Transaction Receipts).
- Les logs des Smart Contracts.
- L'état Ethereum (balances, storage des contrats, etc.).

[Ethereum’s Hidden Backbone: A Deep Dive Into Merkle Patricia Tries | HackerNoon](https://images.openai.com/static-rsc-4/no_yxG7XesU8nZq5vIH6LKRQ8USI_sHSC3VKWO6enB0VnWGJpK8PeMCVbD-5-ZonI-EA0sR2D3QR2gYfphVeiVKZ8sX6fhHWcPapMHGpB8ElDfld8ALsbb-83oSOtV0sePfgDz1TW6lg76RVnkqRVPe0_1sFxMVLY3VwXj4jsoQ?purpose=inline)

[hackernoon.com](https://hackernoon.com/ethereums-hidden-backbone-a-deep-dive-into-merkle-patricia-tries?utm_source=chatgpt.com)

## **2. Comment un événement est-il enregistré ?**

Prenons un Smart Contract qui émet un événement :

```
event Transfer(
    address indexed from,
    address indexed to,
    uint256 value
);

function transfer(address to, uint256 amount) public {
    // Transfert de tokens...

    emit Transfer(msg.sender, to, amount);
}
```

Voici le processus :

**1. Smart Contract**

Émet l'événement Transfer.

**2. Transaction**

La transaction est envoyée au réseau.

**3. Geth – Execution Layer**

Exécute le contrat et produit les logs.

**4. Transaction Receipt**

Contient les logs de l'événement.

**5. Bloc Ethereum**

Les logs sont intégrés au bloc via le reçu de transaction et engagés cryptographiquement dans le receiptsRoot.

**Point important :** les événements ne sont pas stockés comme des variables dans le Smart Contract. Ils sont enregistrés dans les logs des reçus de transactions.

## **3. Où se trouve physiquement la blockchain dans Geth ?**

Sur ta machine Ubuntu, Geth utilise un répertoire de données appelé `datadir`.

Par défaut, sous Linux :

```
~/.ethereum/geth/
```

Tu peux examiner son contenu :

```
ls -lah ~/.ethereum/
```

Et :

```
ls -lah ~/.ethereum/geth/
```

Geth utilise une base de données locale (Pebble, notamment) pour conserver les données nécessaires au fonctionnement du nœud.

Dans ton environnement Kurtosis, les données peuvent être stockées dans les volumes Docker associés aux conteneurs Geth, et non dans `~/.ethereum/`.

## **4. Et Lighthouse, où stocke-t-il les données ?**

Lighthouse possède également son propre stockage.

Il conserve les données relatives au Consensus Layer :

- Beacon blocks.
- Attestations.
- États de la Beacon Chain.
- Informations des validateurs.
- Historique du consensus.

Il ne remplace pas le stockage de Geth : les deux clients ont des responsabilités différentes.

## **5. Comment consulter les événements enregistrés ?**

Avec Geth et JSON-RPC, tu peux récupérer les logs.

Exemple :

```
curl -X POST http://127.0.0.1:8545 \
-H "Content-Type: application/json" \
-d '{
  "jsonrpc":"2.0",
  "method":"eth_getLogs",
  "params":[{
    "fromBlock":"latest",
    "toBlock":"latest"
  }],
  "id":1
}'
```

Tu peux aussi récupérer le reçu d'une transaction :

```
eth_getTransactionReceipt
```

Il contient notamment :

- `transactionHash`
- `blockNumber`
- `status`
- `logs`
- `logsBloom`

## **6. Application à EVA-Guard**

Voici une distinction essentielle pour ton architecture :

| Élément                       | Où le trouver ?          |
| ----------------------------- | ------------------------ |
| Transactions                  | Geth                     |
| Blocs d'exécution             | Geth                     |
| Smart Contract Events         | Geth (logs des receipts) |
| Balances et état des contrats | Geth                     |
| Attestations                  | Lighthouse               |
| Beacon Blocks                 | Lighthouse               |
| Finality                      | Lighthouse               |

**Architecture de stockage d'EVA-Guard**

**Geth + Lighthouse**

Données blockchain et consensus

**Rust Monitoring Engine**

Collecte des métriques et observations JSONL

**ML + Agents IA**

Détection et analyse des attaques

**Rapports de sécurité**

**Une distinction fondamentale à retenir :** EVA-Guard collecte des observations sur les nœuds Ethereum. Ces observations JSONL ne sont pas des événements enregistrés dans la blockchain. Ce sont des données externes de monitoring.

La prochaine notion importante à comprendre est la différence entre **Block, Transaction, Receipt, Logs et State** : ce sont cinq concepts différents qui constituent le fonctionnement interne d'Ethereum.


https://chatgpt.com/c/6b080948-2948-83ea-a3e3-54e719a2b10b

https://chatgpt.com/c/6b080798-b750-83ea-8a89-ec83c997a25b