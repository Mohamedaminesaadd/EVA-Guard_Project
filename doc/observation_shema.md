Dans ton projet EVA-Guard, la structure `Observation` joue un rôle central : elle représente un instantané de l'état du nœud Ethereum à un moment précis.

Autrement dit, les collectors récupèrent les informations depuis Geth et le système Linux, puis toutes ces informations sont regroupées dans une seule `Observation`.

## 1. Le concept : qu'est-ce qu'une Observation ?

Imagine que ton nœud Geth fonctionne en continu.

Toutes les 5 secondes, EVA-Guard collecte des informations :

- Combien de peers sont connectés ?
- Quel est le dernier bloc reçu ?
- Le nœud est-il synchronisé ?
- Le RPC est-il disponible ?
- Quelle est la consommation CPU et mémoire ?
- Combien de données réseau sont reçues et envoyées ?

Une `Observation` rassemble toutes ces mesures dans un seul objet.

Cycle de collecte EVA-Guard

Geth Node + Linux System

Sources des métriques

Blockchain

Block number, sync

P2P

Peers, listening

RPC

Availability, latency

Network / System

Traffic, CPU, RAM

Observation

Regroupe toutes les métriques collectées

JSONL Storage

Historique des observations

## 2. Pourquoi plusieurs structures de métriques ?

Dans ton fichier `observation.rs`, tu as une structure principale et cinq sous-structures.

| Structure           | Responsabilité        | Exemples                      |
| ------------------- | --------------------- | ----------------------------- |
| `Observation`       | Regrouper les mesures | Timestamp, node ID            |
| `P2pMetrics`        | État du réseau P2P    | Peer count, nouveaux peers    |
| `BlockchainMetrics` | État de la blockchain | Block number, synchronisation |
| `RpcMetrics`        | État de l'API RPC     | Disponibilité, latence        |
| `NetworkMetrics`    | Trafic réseau         | RX/TX, connexions TCP         |
| `SystemMetrics`     | Ressources système    | CPU, mémoire                  |

Cette séparation permet de développer chaque collector indépendamment.

Par exemple :

- `BlockchainCollector` remplit `BlockchainMetrics`.
- `P2pCollector` remplit `P2pMetrics`.
- `RpcCollector` remplit `RpcMetrics`.
- `NetworkCollector` remplit `NetworkMetrics`.
- `SystemCollector` remplit `SystemMetrics`.

## 3. Comprendre le code Observation

Prenons quelques champs de ta structure.

A. Les métadonnées

```
pub struct Observation {
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub collection_interval: u64,
    // ...
}
```

- `timestamp` : moment où l'observation est créée.
- `node_id` : identifiant du nœud surveillé.
- `collection_interval` : intervalle de collecte en secondes.

B. Les métriques

```
pub struct BlockchainMetrics {
    pub block_number: Option<u64>,
    pub is_synced: Option<bool>,
}
```

Pourquoi utiliser `Option` ?

Parce qu'une mesure peut être indisponible.

Par exemple :

```
BlockchainMetrics {
    block_number: Some(150000),
    is_synced: Some(true),
}
```

Signifie que les deux mesures sont disponibles.

Mais :

```
BlockchainMetrics {
    block_number: None,
    is_synced: None,
}
```

Signifie qu'on n'a pas obtenu ces mesures.

C'est important : une valeur inconnue ne doit pas être confondue avec une valeur égale à zéro ou `false`.

C. Le constructeur `new()`

```
let observation = Observation::new(
    "geth-node-01",
    Duration::from_secs(5),
);
```

Cette fonction crée une observation initialisée :

- Elle définit le timestamp actuel.
- Elle définit l'identifiant du nœud.
- Elle définit l'intervalle de collecte.
- Elle initialise toutes les métriques à `None`.

Les collectors rempliront ensuite les champs disponibles.

## 4. Exemple pratique

Imaginons une collecte à 14:00:00.

L'objet `Observation` pourrait contenir :

```
{
  "timestamp": "2026-11-08T14:00:00Z",
  "node_id": "geth-node-01",
  "collection_interval": 5,
  "peer_count": 12,
  "block_number": 150000,
  "is_synced": true,
  "rpc_available": true,
  "rpc_latency_ms": 12.5,
  "cpu_usage_percent": 35.2,
  "memory_used_mb": 2048
}
```

Grâce à `#[serde(flatten)]`, les métriques des sous-structures sont directement intégrées dans le JSON, sans objets imbriqués supplémentaires.

## 5. Pourquoi cette structure est importante pour EVA-Guard ?

L'objectif final d'EVA-Guard est de détecter des comportements anormaux sur un nœud Ethereum.

Par exemple, supposons que tu observes :

| Temps | Peers | CPU  | RPC latency |
| ----- | ----- | ---- | ----------- |
| 14:00 | 12    | 30 % | 10 ms       |
| 14:05 | 11    | 32 % | 12 ms       |
| 14:10 | 3     | 85 % | 250 ms      |

La troisième observation peut représenter un comportement suspect.

L'agent d'analyse pourra utiliser l'historique des observations pour identifier des anomalies et formuler une hypothèse d'attaque.

La distinction importante : `Observation` ne détecte pas elle-même les attaques. Elle collecte et organise les données nécessaires aux composants de détection et d'analyse.

## 6. Ce que tu dois retenir

- `Observation` = photographie de l'état du nœud à un instant donné.
- Les `Collectors` = composants qui récupèrent les mesures.
- Les structures `Metrics` = organisent les mesures par catégorie.
- `JSONL Storage` = conserve l'historique.
- Les agents EVA-Guard = exploitent ensuite cet historique pour analyser les comportements.

Question pour vérifier ta compréhension :

Si `BlockchainCollector` récupère le `block_number`, est-ce qu'il doit modifier directement le fichier JSONL, ou simplement retourner un `BlockchainMetrics` que le système central ajoutera à l'`Observation` ?
------------------------------------------------------
Avant de coder le Storage, il faut bien comprendre l'architecture de `Observation`, car c'est la structure centrale qui transporte toutes les données collectées par EVA-Guard.

# 1. Architecture de l'Observation

Le principe est simple : chaque cycle de monitoring produit une seule `Observation`, qui regroupe les résultats de tous les collectors.

Observation

Snapshot complet du nœud Ethereum et de son système

Metadata

timestamp

node_id

collection_interval

P2pMetrics

peer_count

new_peers

disconnected_peers

network_listening

BlockchainMetrics

block_number

is_synced

RpcMetrics

rpc_available

rpc_latency_ms

NetworkMetrics

network_rx_bytes_total

network_tx_bytes_total

network_rx_bytes_per_sec

tcp_established_connections

SystemMetrics

cpu_usage_percent

memory_used_mb

memory_total_mb

Architecture correspondant à ton fichier observation.rs. Les groupes sont aplatis lors de la sérialisation JSON.

# 2. Architecture logicielle

Dans le code Rust, nous avons une structure principale et cinq structures spécialisées.

```
src/
│
├── models/
│   ├── mod.rs
│   └── observation.rs
│       │
│       ├── Observation
│       │   ├── timestamp
│       │   ├── node_id
│       │   ├── collection_interval
│       │   │
│       │   ├── P2pMetrics
│       │   ├── BlockchainMetrics
│       │   ├── RpcMetrics
│       │   ├── NetworkMetrics
│       │   └── SystemMetrics
```

Chaque structure a une responsabilité :

| Structure           | Responsabilité                   |
| ------------------- | -------------------------------- |
| `Observation`       | Regrouper un cycle de monitoring |
| `P2pMetrics`        | Décrire l'activité P2P           |
| `BlockchainMetrics` | Décrire l'état Blockchain        |
| `RpcMetrics`        | Décrire l'état RPC               |
| `NetworkMetrics`    | Décrire le réseau de la machine  |
| `SystemMetrics`     | Décrire les ressources système   |

# 3. Comment les collectors remplissent Observation ?

Chaque collector retourne son propre groupe de métriques.

```
BlockchainCollector ──> BlockchainMetrics ──┐
P2pCollector ──────────> P2pMetrics ─────────┤
RpcCollector ──────────> RpcMetrics ────────┤
NetworkCollector ──────> NetworkMetrics ────┤
SystemCollector ───────> SystemMetrics ─────┤
                                            v
                                       Observation
```

Exemple simplifié :

```
let mut observation = Observation::new(
    "geth-node-01",
    Duration::from_secs(5),
);

observation.blockchain.block_number = Some(1200);
observation.p2p.peer_count = Some(10);
observation.rpc.rpc_available = Some(true);
observation.system.cpu_usage_percent = Some(25.5);
```

Chaque collector remplit uniquement ses propres métriques.

# 4. Pourquoi utiliser `Option<T>` ?

C'est une décision architecturale importante dans EVA-Guard.

Supposons que Geth soit arrêté :

| Métrique            | Valeur        |
| ------------------- | ------------- |
| `peer_count`        | `None`        |
| `block_number`      | `None`        |
| `rpc_available`     | `Some(false)` |
| `cpu_usage_percent` | `Some(25.5)`  |
| `memory_used_mb`    | `Some(4096)`  |

Cela permet de distinguer :

- `Some(0)` : mesure disponible, valeur zéro.
- `None` : mesure non disponible ou non effectuée.
- `Some(false)` : mesure disponible, résultat négatif.

Cette distinction est essentielle pour le futur moteur d'intelligence qui analysera les observations.

# 5. Transformation en JSON

Grâce à Serde et `#[serde(flatten)]`, les groupes Rust deviennent un seul objet JSON plat.

Structure Rust

```
Observation {
    p2p: P2pMetrics { ... },
    blockchain: BlockchainMetrics { ... },
    system: SystemMetrics { ... },
}
```

JSON produit

```
{
  "timestamp": "2026-11-08T12:00:00Z",
  "node_id": "geth-node-01",
  "collection_interval": 5,
  "peer_count": 10,
  "block_number": 1200,
  "cpu_usage_percent": 25.5,
  "memory_used_mb": 4096
}
```

Les 22 clés définies dans ton modèle restent présentes, même lorsque certaines valeurs sont `null`.

# 6. Pourquoi cette architecture est importante ?

Elle sépare trois responsabilités :

1. Collectors : récupérer les données.
2. Observation : organiser les données dans un contrat commun.
3. Storage : sauvegarder les données.

Ainsi, le Storage ne connaît pas Geth, ne connaît pas les collectors et ne calcule aucune métrique. Il reçoit simplement une `Observation` et la sérialise.

C'est une bonne séparation des responsabilités.

## Validation de compréhension avant de coder

Trois questions importantes pour la suite :

1. Pourquoi `Observation` regroupe-t-elle cinq structures de métriques au lieu de mettre tous les champs directement dans les collectors ?
2. Quelle est la différence entre `Some(0)` et `None` ?
3. Pourquoi `serde(flatten)` est-il utile pour notre futur moteur d'intelligence Python ?

Tu peux répondre ou simplement me dire de continuer. Notre prochaine étape sera la création de `src/storage/mod.rs`, sans toucher à `observation.rs`.