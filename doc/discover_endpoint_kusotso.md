# EVA-Guard — Documentation : Découverte et validation des endpoints Ethereum

## 1. Objectif

Avant de développer le moteur de monitoring Rust d'EVA-Guard, il est nécessaire d'identifier et de vérifier les interfaces de communication exposées par les composants du réseau Ethereum.

Notre environnement de test utilise Kurtosis pour déployer un réseau Ethereum privé composé de cinq ensembles de nœuds.

Chaque ensemble contient :

- Un client d'exécution Geth.
- Un Lighthouse Beacon Node.
- Un Lighthouse Validator Client.

L'objectif est de vérifier que le moteur EVA-Guard pourra communiquer avec les 15 services et récupérer leurs métriques.

Les vérifications concernent trois types d'interfaces :

1. JSON-RPC : communication avec Geth.
2. REST API : communication avec Lighthouse Beacon Node.
3. Prometheus Metrics : récupération des métriques internes de Geth, Beacon Node et Validator Client.

## 2. Découverte des endpoints avec Kurtosis

### 2.1 Qu'est-ce qu'un endpoint ?

Un endpoint est une adresse permettant à une application de communiquer avec un service.

Il est généralement composé de :

- Une adresse IP ou un nom d'hôte.
- Un port.
- Éventuellement un chemin HTTP.

Exemple :

`http://127.0.0.1:32774`

Dans cet exemple :

- `http` : protocole de communication.
- `127.0.0.1` : adresse locale de la machine.
- `32774` : port exposé pour accéder au service.

Un endpoint peut également inclure un chemin :

`http://127.0.0.1:32777/debug/metrics/prometheus`

### 2.2 Pourquoi utiliser Kurtosis ?

Kurtosis déploie les composants Ethereum dans des conteneurs Docker.

Les services utilisent des ports internes dans leurs conteneurs. Kurtosis peut publier ces ports sur des ports différents de la machine hôte.

Par exemple :

|   | Élément                |  Port |
| - | ---------------------- | ----: |
|   | Port interne Geth RPC  |  8545 |
|   | Port publié sur Ubuntu | 32774 |

Il faut donc utiliser le port publié par Kurtosis depuis le moteur Rust exécuté sur Ubuntu.

### 2.3 Comment découvrir les ports ?

La commande utilisée est :

```bash
kurtosis enclave inspect eva-ethereum
```

Cette commande affiche :

- Les conteneurs et services déployés.
- Les identifiants des services.
- Les ports internes.
- Les ports publiés sur la machine hôte.
- L'état de chaque service.

Exemple de résultat :

```text
el-1-geth-lighthouse

engine-rpc: 8551/tcp -> 127.0.0.1:32776
metrics:    9001/tcp -> http://127.0.0.1:32777
rpc:        8545/tcp -> 127.0.0.1:32774
ws:         8546/tcp -> 127.0.0.1:32775
```

Interprétation :

- `8551` : endpoint Engine API utilisé pour la communication entre le client d'exécution et le client consensus.
- `9001` : endpoint Prometheus interne de Geth.
- `8545` : endpoint JSON-RPC de Geth.
- `8546` : endpoint WebSocket de Geth.

Le port publié `32774` permet à EVA-Guard d'accéder au JSON-RPC depuis Ubuntu.

**Important :** il ne faut pas supposer que le port interne et le port externe sont identiques.

## 3. Identification des endpoints des cinq nœuds

Après l'exécution de `kurtosis enclave inspect`, nous avons identifié les ports suivants.

### 3.1 Geth

Geth est le client d'exécution Ethereum. Il gère notamment les transactions, les blocs, l'état de la blockchain et les communications P2P.

Deux interfaces sont utilisées.

**A. JSON-RPC**

Cette interface permet d'envoyer des requêtes au client Geth.

Exemples de méthodes :

- `eth_blockNumber` : récupérer le dernier numéro de bloc.
- `eth_syncing` : vérifier l'état de synchronisation.
- `net_peerCount` : récupérer le nombre de pairs connectés.
- `admin_peers` : obtenir des informations détaillées sur les pairs, si le namespace admin est activé.

Ports identifiés :

| Nœud    | Port JSON-RPC |
| ------- | ------------: |
| node-01 |         32774 |
| node-02 |         32790 |
| node-03 |         32786 |
| node-04 |         32782 |
| node-05 |         32778 |

**B. Prometheus**

Geth expose également ses métriques internes via une interface Prometheus.

| Nœud    | Port Prometheus |
| ------- | --------------: |
| node-01 |           32777 |
| node-02 |           32793 |
| node-03 |           32789 |
| node-04 |           32785 |
| node-05 |           32781 |

Chemin utilisé :

`/debug/metrics/prometheus`

### 3.2 Lighthouse Beacon Node

Le Beacon Node est le client consensus Ethereum.

Il gère notamment :

- Les informations de consensus.
- Les slots et les epochs.
- Les pairs du réseau consensus.
- Les attestations.
- La synchronisation et la finalité.

Deux interfaces sont utilisées.

**A. Beacon REST API**

Cette interface permet de récupérer des informations sur le consensus.

Exemples de routes :

- `/eth/v1/node/syncing` : état de synchronisation.
- `/eth/v1/node/identity` : informations d'identité du nœud.
- `/eth/v1/beacon/states/head/finality_checkpoints` : checkpoints de finalité.

Ports identifiés :

| Nœud    | Port REST API |
| ------- | ------------: |
| node-01 |         32794 |
| node-02 |         32806 |
| node-03 |         32803 |
| node-04 |         32800 |
| node-05 |         32797 |

**B. Prometheus Beacon**

Lighthouse expose ses métriques internes via Prometheus.

| Nœud    | Port Prometheus |
| ------- | --------------: |
| node-01 |           32795 |
| node-02 |           32807 |
| node-03 |           32804 |
| node-04 |           32801 |
| node-05 |           32798 |

Chemin utilisé :

`/metrics`

### 3.3 Lighthouse Validator Client

Le Validator Client participe aux activités des validateurs Ethereum.

Il est notamment responsable de :

- La production des attestations.
- La participation aux tâches de validation.
- La proposition de blocs lorsqu'un validateur est sélectionné.
- La publication des attestations et autres messages de validation.

Les métriques sont exposées via Prometheus.

| Nœud    | Port Prometheus |
| ------- | --------------: |
| node-01 |           32809 |
| node-02 |           32813 |
| node-03 |           32810 |
| node-04 |           32812 |
| node-05 |           32811 |

Chemin utilisé :

`/metrics`

Aucune API HTTP distincte du Validator Client n'a été identifiée dans la sortie Kurtosis inspectée. Nous utilisons donc uniquement son endpoint Prometheus dans la configuration actuelle.

## 4. Qu'est-ce que Prometheus ?

Prometheus est un système de collecte et de surveillance de métriques.

Il permet aux applications d'exposer des données numériques décrivant leur fonctionnement.

Exemples :

- Nombre de connexions.
- Nombre de pairs.
- Nombre de requêtes.
- Durée des opérations.
- Utilisation des ressources.
- Nombre d'erreurs.
- État de certains composants internes.

### 4.1 Comment fonctionne Prometheus ?

Un service expose ses métriques via une URL HTTP.

Exemple :

`http://127.0.0.1:32777/debug/metrics/prometheus`

Un système de monitoring peut envoyer une requête HTTP GET vers cette URL et récupérer les métriques.

La réponse utilise généralement le format texte Prometheus.

Exemple :

```text
# TYPE blobpool_add_gapped gauge
blobpool_add_gapped 0

# TYPE blobpool_add_gappedfull gauge
blobpool_add_gappedfull 0
```

Interprétation :

- `# TYPE` indique le type de métrique.
- `blobpool_add_gapped` représente le nom d'une métrique.
- `0` représente sa valeur actuelle.

### 4.2 Les principaux types de métriques

**Counter**

Un compteur qui augmente ou reste constant. Il est utilisé pour mesurer un nombre cumulatif d'événements.

Exemples :

- Nombre de requêtes.
- Nombre d'erreurs.
- Nombre de connexions établies au cours d'une période cumulée.

**Gauge**

Une valeur qui peut augmenter ou diminuer.

Exemples :

- Nombre de pairs actuellement connectés.
- Utilisation mémoire.
- Nombre de tâches actives.

**Histogram**

Permet de mesurer la distribution de valeurs, notamment les durées et latences.

Exemples :

- Latence des requêtes.
- Durée d'exécution des opérations.

### 4.3 Pourquoi Prometheus est important pour EVA-Guard ?

JSON-RPC et REST API permettent de demander des informations précises aux services.

Prometheus permet de récupérer un ensemble de métriques internes déjà exposées par les composants.

Cette complémentarité est importante pour détecter les comportements anormaux.

Par exemple :

- Une augmentation inhabituelle du nombre de requêtes RPC.
- Une diminution soudaine du nombre de pairs.
- Une augmentation de la latence.
- Une activité anormale des tâches de validation.

EVA-Guard pourra récupérer ces informations, les associer à un nœud et les exploiter dans son système de détection d'anomalies.

**Prometheus n'est pas le moteur de détection d'EVA-Guard.** Il fournit les métriques. EVA-Guard est responsable de leur collecte, de leur analyse et de la détection des anomalies.

## 5. Procédure de vérification des connexions

Après avoir identifié les ports, nous avons effectué des tests HTTP avec `curl`.

L'objectif est de vérifier que les services répondent réellement.

### 5.1 Test de Geth JSON-RPC

Commande :

```bash
for port in 32774 32790 32786 32782 32778; do
    echo "===== GETH RPC PORT $port ====="

    curl -s -X POST "http://127.0.0.1:$port" \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

    echo
done
```

Méthode utilisée : `eth_blockNumber`.

Résultat obtenu :

```json
{"jsonrpc":"2.0","id":1,"result":"0x1b7"}
```

Interprétation :

- La connexion HTTP fonctionne.
- Geth reçoit la requête JSON-RPC.
- Geth retourne le numéro du dernier bloc.

Les cinq nœuds ont retourné le même résultat : `0x1b7`, soit le bloc 439 en décimal.

### 5.2 Test de Geth Prometheus

Commande :

```bash
for port in 32777 32793 32789 32785 32781; do
    echo "===== GETH METRICS PORT $port ====="

    curl -s -o /tmp/geth_metrics.txt \
      -w "HTTP %{http_code}\n" \
      "http://127.0.0.1:$port/debug/metrics/prometheus"

    head -5 /tmp/geth_metrics.txt
    echo
done
```

Résultat obtenu :

```text
HTTP 200
# TYPE blobpool_add_gapped gauge
blobpool_add_gapped 0
```

Interprétation :

- HTTP 200 signifie que le serveur répond correctement.
- Les métriques sont effectivement exposées.
- Le chemin Prometheus est valide.

Résultat : 5 endpoints sur 5 validés.

### 5.3 Test de Beacon REST API

Commande :

```bash
for port in 32794 32806 32803 32800 32797; do
    echo "===== BEACON API PORT $port ====="

    curl -s "http://127.0.0.1:$port/eth/v1/node/syncing"
    echo
done
```

Une autre route utilisée pour vérifier les checkpoints de consensus :

```bash
for port in 32794 32806 32803 32800 32797; do
    echo "===== FINALITY PORT $port ====="

    curl -s \
      "http://127.0.0.1:$port/eth/v1/beacon/states/head/finality_checkpoints"

    echo
done
```

Résultat obtenu :

Les cinq Beacon Nodes ont retourné les informations de finalité.

Les checkpoints observés étaient :

- Previous justified epoch : 11.
- Current justified epoch : 12.
- Finalized epoch : 11.

Le champ `finalized` était `false` dans la réponse. La présence d'un checkpoint finalized ne signifie donc pas que le nœud déclare l'état courant comme finalisé.

### 5.4 Test de Beacon Prometheus

Commande :

```bash
for port in 32795 32807 32804 32801 32798; do
    echo "===== BEACON METRICS PORT $port ====="

    curl -s -o /tmp/beacon_metrics.txt \
      -w "HTTP %{http_code}\n" \
      "http://127.0.0.1:$port/metrics"

    head -5 /tmp/beacon_metrics.txt
    echo
done
```

Résultat obtenu :

```text
HTTP 200
# HELP async_tasks_count Total number of async tasks spawned using spawn
# TYPE async_tasks_count gauge
```

Interprétation :

- Les Beacon Nodes exposent leurs métriques.
- Le format Prometheus est accessible.
- Les cinq endpoints sont opérationnels.

### 5.5 Test de Validator Prometheus

Commande :

```bash
for port in 32809 32813 32810 32812 32811; do
    echo "===== VALIDATOR METRICS PORT $port ====="

    curl -s -o /tmp/validator_metrics.txt \
      -w "HTTP %{http_code}\n" \
      "http://127.0.0.1:$port/metrics"

    head -5 /tmp/validator_metrics.txt
    echo
done
```

Résultat obtenu :

```text
HTTP 200
# HELP async_tasks_count Total number of async tasks spawned using spawn
# TYPE async_tasks_count gauge
async_tasks_count{async_task_count="attestation_service"} 1
```

Interprétation :

- Les cinq Validator Clients répondent.
- Leurs métriques sont exposées.
- Les métriques liées aux tâches d'attestation sont notamment visibles.

Cela confirme l'accessibilité des endpoints, mais ne suffit pas à démontrer que les attestations sont correctement incluses dans la blockchain.

## 6. Résultat global de la validation

| Composant | Interface          | Nombre testé | Résultat       |
| --------- | ------------------ | -----------: | -------------- |
| Geth      | JSON-RPC           |            5 | 5/5 validés    |
| Geth      | Prometheus         |            5 | 5/5 validés    |
| Beacon    | REST API           |            5 | 5/5 validés    |
| Beacon    | Prometheus         |            5 | 5/5 validés    |
| Validator | Prometheus         |            5 | 5/5 validés    |
| Validator | API HTTP distincte |            0 | Non identifiée |

Au total, **25 endpoints ont été vérifiés avec succès** :

- 5 endpoints Geth JSON-RPC.
- 5 endpoints Geth Prometheus.
- 5 endpoints Beacon REST.
- 5 endpoints Beacon Prometheus.
- 5 endpoints Validator Prometheus.

## 7. Configuration centrale EVA-Guard

Les endpoints vérifiés sont regroupés dans un seul fichier de configuration :

`monitoring-rust/config/nodes.toml`

Cette configuration centralise les adresses des services et les paramètres de collecte.

Exemple :

```toml
[settings]
request_timeout_secs = 3
collection_interval_secs = 5

[[nodes]]
id = "node-01"

[nodes.geth]
rpc_url = "http://127.0.0.1:32774"
metrics_url = "http://127.0.0.1:32777/debug/metrics/prometheus"

[nodes.beacon]
api_url = "http://127.0.0.1:32794"
metrics_url = "http://127.0.0.1:32795/metrics"

[nodes.validator]
metrics_url = "http://127.0.0.1:32809/metrics"
```

La même structure est répétée pour les quatre autres nœuds.

Cette organisation permet de modifier les endpoints sans modifier le code source des collectors.

## 8. Conclusion et prochaine étape

La phase de découverte et de validation des endpoints est terminée pour les interfaces identifiées.

Les 25 endpoints testés répondent correctement depuis la machine hôte.

Cette étape permet de confirmer que le futur moteur de monitoring Rust peut accéder aux interfaces nécessaires des cinq ensembles de nœuds.

La prochaine étape consiste à intégrer cette configuration dans le programme Rust :

1. Charger `nodes.toml` avec `NodesConfig::from_env()`.
2. Créer les clients HTTP pour Geth et Lighthouse.
3. Adapter le modèle `Observation`.
4. Développer les collectors pour chaque composant.
5. Mettre en place un scheduler multi-nœuds.
6. Enregistrer les observations dans les fichiers JSONL.

**Remarque :** la validation HTTP confirme l'accessibilité des endpoints, pas encore la disponibilité de toutes les métriques nécessaires à la détection des attaques. Il faudra examiner le contenu Prometheus et les API disponibles pendant le développement des collectors.
