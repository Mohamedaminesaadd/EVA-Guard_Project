Pour bien comprendre ton projet **EVA-Guard**, il est essentiel de comprendre ce qu'est un **nœud Ethereum (Ethereum Node)**, comment il fonctionne, comment il communique avec les autres nœuds et comment EVA-Guard peut surveiller son comportement pour détecter les attaques.

Je vais t'expliquer progressivement, avec une approche technique adaptée à ton niveau d'ingénieur informatique.

# **1. Qu'est-ce qu'un nœud Ethereum ?**

[Architecture du noeud | ethereum.org](https://images.openai.com/static-rsc-4/TcHiDsbeOAHeSw3QIpljqwUDPXKyNeFcv4QA9mwYAerJyi9ZQuevmioiv8JOBgVrC36-tlIp5Y7xeG3J8giDLKHDVaWogqruXcErQm7WYkWdFBGDHKtht1LJnvmFUaFU_weRGehVcBoKnC-GRU2Ob18Crw2VuFv0wFrKT35kQ_E?purpose=inline)

[ethereum.org](https://ethereum.org/fr/developers/docs/nodes-and-clients/node-architecture/?utm_source=chatgpt.com)

Un nœud Ethereum est une machine qui exécute un logiciel Ethereum et participe au fonctionnement du réseau blockchain.

Il peut avoir plusieurs responsabilités :

- Communiquer avec les autres nœuds.
- Recevoir et transmettre les transactions.
- Vérifier les blocs.
- Maintenir une copie des données de la blockchain.
- Participer à la synchronisation du réseau.
- Exposer des interfaces RPC pour permettre aux applications de communiquer avec lui.

**Exemple :** imaginons un réseau Ethereum constitué de 5 machines.

- Node 1
- Node 2
- Node 3
- Node 4
- Node 5

Ces machines communiquent entre elles. Si Node 1 reçoit une nouvelle transaction, il peut la transmettre aux autres nœuds.

# **2. Les deux composants principaux d'un nœud Ethereum**

Un nœud Ethereum moderne utilise généralement deux clients logiciels complémentaires.

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/k-1X9TcAnGnQHoJ_aqeIFmQ4aKRZKjNnd92Efb3sceakcGF8XtBXrrdPJCNCFW1uCXG9K8YLjLtIv8FhiB5_yJpn-DN_K3BMfh01_Fjm0bht1WnYfYZteW6C0nXm8DafOC3QzbgxBP3TEAnWG4pNmXj0820A0Hk0e2UmR3dODpg?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node/?utm_source=chatgpt.com)

## **A. Execution Layer (EL)**

C'est la couche responsable de l'exécution des transactions.

Dans ton projet, tu utilises **Geth** (Go Ethereum (Geth)).

Ses responsabilités :

1. Recevoir les transactions.
2. Vérifier les transactions.
3. Exécuter les Smart Contracts.
4. Maintenir l'état de la blockchain.
5. Gérer les comptes et les soldes.
6. Exposer une API RPC.

Exemple :

Alice possède 5 ETH et souhaite envoyer 2 ETH à Bob.

Geth participe à l'exécution de cette transaction et au calcul du nouvel état :

- Alice : 3 ETH.
- Bob : 2 ETH.

## **B. Consensus Layer (CL)**

La couche de consensus permet aux validateurs de se mettre d'accord sur les blocs qui doivent être ajoutés à la blockchain.

Dans ton projet, tu utilises **Lighthouse** (Lighthouse).

Ses responsabilités :

1. Participer au protocole de consensus Proof of Stake.
2. Communiquer avec les autres clients de consensus.
3. Vérifier les attestations.
4. Participer à la sélection des blocs.
5. Gérer la synchronisation de la couche consensus.

**Différence fondamentale :**

| Geth (EL)                   | Lighthouse (CL)          |
| --------------------------- | ------------------------ |
| Exécute les transactions    | Gère le consensus        |
| Gère l'état Ethereum        | Gère la Beacon Chain     |
| Vérifie les Smart Contracts | Vérifie les attestations |
| Expose JSON-RPC             | Expose Beacon API        |

Les deux clients communiquent entre eux via Engine API.

# **3. Architecture interne d'un nœud**

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/t5PHAFvPXBLjryYLjPYCbxFZGF-S4SJezwHaf4BsLoBmd23ofRAGqWqo8HqllYgwkhUUSlZcEMpPubJ9Jp7mZpYzQixmo6PzK3TGZNs_8ngMWQv9Db17ZhJbgVn66v1-E06F_xH56tkPXJ99dwFBceEMRx2kld4FNV0enIM18lY?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node?utm_source=chatgpt.com)

Un nœud peut être représenté ainsi :

```
             ETHEREUM NODE
                   |
        +----------+----------+
        |                     |
   EXECUTION LAYER      CONSENSUS LAYER
        |                     |
       GETH               LIGHTHOUSE
        |                     |
   Transaction           Attestations
   Smart Contracts       Validators
   State                 Beacon Chain
        |                     |
        +---------+-----------+
                  |
              Engine API
                  |
           Communication
           entre les clients
```

Attention : dans ton réseau Kurtosis, chaque participant peut posséder ses propres clients Geth et Lighthouse.

# **4. Comment les nœuds communiquent-ils ?**

Ethereum utilise une architecture P2P (Peer-to-Peer).

[波特尔网络 | ethereum.org](https://images.openai.com/static-rsc-4/r7QFp7mdzhD44bwf0_pSXH-s-Phx3vh1ztpEoe106-hXmQtA3Uj32uC_p1tAkbi-P9-vlSq1bgz_OQXq270kuXW8Wbub6VCb_Q_APs8OPWtbFEs75HATb5OYD3kVDycamtL1BcE1xjFMdbBzODhiBUcO5PNE6yVs0RLTQcMJNZA?purpose=inline)

[ethereum.org](https://ethereum.org/zh/developers/docs/networking-layer/portal-network/?utm_source=chatgpt.com)

Chaque nœud peut avoir plusieurs connexions avec d'autres nœuds.

Par exemple :

```
             Node 1
            /      \
         Node 2    Node 3
          /  \       \
       Node 4 ------ Node 5
```

Les nœuds communiquent pour :

- Découvrir de nouveaux peers.
- Échanger des informations.
- Propager les transactions.
- Propager les blocs.
- Synchroniser leurs données.

Dans Ethereum, il faut distinguer deux réseaux :

- **Execution P2P :** communication entre clients d'exécution.
- **Consensus P2P :** communication entre clients de consensus.

# **5. Comment fonctionne ton réseau avec Kurtosis ?**

Dans EVA-Guard, tu utilises Kurtosis pour déployer un réseau Ethereum de cinq participants.

[Guide to Deploying your own Local Ethereum PoS Testnet | by Rez | Medium](https://images.openai.com/static-rsc-4/btZhHLrJY00VQ5_unxQ87zxHrCC_8dL-6z5Yo5xVPqLU1ccB2TuWoHKTgItCIV71-gQrRJBFgESdflBkzg2gpSqjPfjdrs6J_QxjKxHd7jwX35ElzjIMpQeZhNclz0DrvCb4JT6HJaJYq_RZZRCmoNsF0DXJxV-YN6gQb6-tExo?purpose=inline)

[medium.com](https://medium.com/%400xZorz/guide-to-deploying-your-own-local-ethereum-pos-testnet-95b5a0b0ca87?utm_source=chatgpt.com)

Ton architecture ressemble à ceci :

```
             KURTOSIS
                 |
        ETHEREUM NETWORK
                 |
     +-----------+-----------+
     |           |           |
   Node 1      Node 2      Node 3
     |           |           |
   Geth        Geth        Geth
 Lighthouse  Lighthouse  Lighthouse
     |           |           |
     +-----------+-----------+
                 |
          Node 4 / Node 5
```

Chaque participant représente une instance indépendante du réseau.

Les ports que tu as configurés dans ton fichier TOML permettent à ton moteur Rust d'accéder aux métriques et aux API de chaque client.

# **6. Comment EVA-Guard surveille les nœuds ?**

C'est ici que ton projet devient intéressant.

Tu as développé un moteur de monitoring en Rust qui collecte des informations sur les nœuds Ethereum.

Voici les principales métriques surveillées :

| Métrique                 | Signification             |
| ------------------------ | ------------------------- |
| `peer_count`             | Nombre de pairs connectés |
| `rpc_latency_ms`         | Temps de réponse RPC      |
| `rpc_requests_total`     | Nombre de requêtes RPC    |
| `block_number`           | Numéro du dernier bloc    |
| `is_synced`              | État de synchronisation   |
| `cpu_usage`              | Utilisation CPU           |
| `memory_usage`           | Utilisation mémoire       |
| `network_rx_bytes_total` | Données reçues            |
| `network_tx_bytes_total` | Données envoyées          |

Par exemple :

```
{
  "node_id": "node-01",
  "scenario": "NORMAL",
  "peer_count": 8,
  "rpc_latency_ms": 25,
  "rpc_requests_total": 150,
  "block_number": 120,
  "is_synced": true,
  "cpu_usage": 25.5,
  "memory_usage": 45.2
}
```

Ce JSON représente un exemple illustratif de métriques collectées.

# **7. Pourquoi surveiller ces métriques ?**

Parce qu'un attaquant peut essayer de perturber le fonctionnement du nœud.

Voici trois scénarios importants dans EVA-Guard.

[Securing Blockchain Systems: A Layer-Oriented Survey of Threats, Vulnerability Taxonomy, and Detection Methods](https://images.openai.com/static-rsc-4/rGb6MWlPPIRriFRiUhqN1Ey-lzxFUBpQYLyMgeEuSXQmJYoLKWjC-nnSd93fz1FTslpLvd-b-k4gWs0MjSChdUilUWNuPxBvRq99j6zSOLgtskNoeAe-RGIHkVNYJDLGuFMSBMGawMpetiQKabW2zK5Qpnea0KgmCgAS4ifehT8?purpose=inline)

**1. Eclipse Attack**

L'attaquant tente d'isoler un nœud de ses connexions légitimes et de contrôler les informations qu'il reçoit.

Indicateurs possibles : diminution du nombre de peers, connexions suspectes et problèmes de synchronisation.

[DDoS攻击如何防御？](https://images.openai.com/static-rsc-4/3sbeefLzLv4HzbrnMansGBNuQ9Wh0OYH-X-Kntzl-0Aps2YWZ4RevpJt8TVwWgFC4V4cpWeGvwxfYEKca0kYjgkkydi_BG-4s43u5vgpLIpOglbm_GzAV9cqVxED7EUVNwLtm0viDVFpIfSqMTcZQJOxYImXzlygQx56AKLFkyI?purpose=inline)

**2. RPC Flooding**

L'attaquant envoie un très grand nombre de requêtes RPC pour surcharger le nœud.

Indicateurs possibles : augmentation des requêtes RPC, latence élevée et consommation CPU importante.

[Open Source Ethereum Infrastructure: A Beginner’s Guide to Essential Resources | by Offchain Labs | Offchain Labs | Medium](https://images.openai.com/static-rsc-4/PpF-HR5gBRoILQdaJGoZq7-n_k7j3NrsBd6vxjJLqgVYfmgpEbc2WUW4Mjv3389uYN2s6pJ08GuRmnvgAp5QxzxnD2SECuCLUg24slltIsqUDH8QGg4MUCdRrdrGmdrtpZPo9GvsBgJt6nGt_LMzGnQQR6_FX2GA7O_0_obTV3M?purpose=inline)

**3. Normal**

Le nœud fonctionne normalement, communique avec ses peers et traite les requêtes sans anomalie particulière.

# **8. Le fonctionnement complet dans EVA-Guard**

Voici le pipeline global :

**Réseau Ethereum**

5 nœuds Geth + Lighthouse

**Rust Monitoring Engine**

Collecte des métriques EL / CL / système

**JSONL Observations**

Historique des observations des nœuds

**ML Prediction Engine**

Classification NORMAL / ECLIPSE / RPC_FLOODING

**Agent 1 – Detection**

Analyse des prédictions et des anomalies

**RAG + Agent 2 – Confirmation**

Investigation et confirmation de l'attaque

**Security Report**

Rapport final de sécurité

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