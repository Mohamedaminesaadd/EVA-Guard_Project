Parfait. Nous passons maintenant au RPC Collector dans `src/collectors/rpc.rs`.

Son objectif est de surveiller la disponibilité et les performances de l'API JSON-RPC de Geth.

## 1. Ce que nous allons développer

Le RPC Collector devra mesurer :

- Disponibilité : est-ce que le nœud répond aux requêtes RPC ?
- Latence : combien de temps prend une requête RPC ?
- Erreurs : est-ce que les requêtes échouent ?
- Timeouts : est-ce que Geth ne répond pas dans le délai prévu ?

Il utilisera `GethClient` pour communiquer avec le nœud. Il ne fera pas de détection d'attaque : cette responsabilité appartient à une autre partie d'EVA-Guard.

## 2. Ordre de développement

Nous allons avancer progressivement :

1. Examiner `RpcMetrics` et les méthodes disponibles dans `GethClient`.
2. Définir la structure `RpcCollector`.
3. Implémenter la mesure de disponibilité et de latence.
4. Gérer les erreurs et les timeouts.
5. Tester avec le vrai Geth.
6. Ajouter des tests unitaires spécifiques.

## 3. Première étape — Examiner le code existant

Avant d'écrire `rpc.rs`, je veux vérifier les champs exacts de ton modèle et les méthodes disponibles dans ton client.

Exécute ces deux commandes depuis `monitoring-rust` :

Commande 1 — Examiner RpcMetrics

```
grep -n -A25 "pub struct RpcMetrics" src/models/observation.rs
```

Commande 2 — Examiner les méthodes de GethClient

```
grep -n "pub async fn" src/geth/client.rs
```

Envoie-moi les deux résultats. Nous définirons ensuite précisément le comportement du `RpcCollector`, sans dupliquer inutilement le code déjà présent.




RPC Collector — test réel réussi

Excellent ! Ton résultat confirme que le `RpcCollector` communique correctement avec Geth.

```
RpcMetrics {
    rpc_available: Some(true),
    rpc_latency_ms: Some(1.07959),
}
```

Interprétation :

- `rpc_available = Some(true)` : Geth répond correctement.
- `rpc_latency_ms = Some(1.07959)` : la requête `eth_blockNumber` a pris environ 1,08 ms.

Petite correction : `cargo main` n'est pas une commande Cargo. Pour exécuter ton programme, utilise `cargo run`.

Nous passons au Network Collector (`src/collectors/network.rs`).

## 1. Objectif du Network Collector

Il surveillera l'activité réseau de la machine qui héberge Geth.

Notre modèle `NetworkMetrics` prévoit cinq métriques :

| Métrique                      | Description                       |
| ----------------------------- | --------------------------------- |
| `network_rx_bytes_total`      | Total des octets reçus            |
| `network_tx_bytes_total`      | Total des octets envoyés          |
| `network_rx_bytes_per_sec`    | Débit de réception en octets/s    |
| `network_tx_bytes_per_sec`    | Débit d'émission en octets/s      |
| `tcp_established_connections` | Nombre de connexions TCP établies |

Attention : ces métriques concernent la machine entière, pas uniquement Geth. L'interface loopback (`lo`) sera exclue.

## 2. Architecture prévue

Sur Ubuntu, nous pouvons exploiter les informations du système Linux :

- `/proc/net/dev` : compteurs réseau RX/TX des interfaces.
- `/proc/net/tcp` et `/proc/net/tcp6` : connexions TCP.
- `Instant` : mesure du temps entre deux collectes pour calculer les débits.

## 3. Ordre de développement

1. Lire les compteurs réseau.
2. Exclure l'interface loopback.
3. Compter les connexions TCP établies.
4. Calculer les débits entre deux collectes.
5. Tester avec les interfaces réseau réelles.
6. Ajouter les tests unitaires.

## 4. Première étape — Examiner le projet

Exécute :

```
cat src/collectors/mod.rs
```

Puis :

```
grep -n -A30 "pub struct NetworkMetrics" src/models/observation.rs
```

Envoie-moi les résultats. Nous commencerons par développer la lecture de `/proc/net/dev`, une étape à la fois.


---------------------------------------------------------------
Très bien. Nous passons au dernier collector de ressources système : `src/collectors/system.rs`.

L'objectif est de surveiller la consommation des ressources de la machine qui héberge Geth.

## 1. Objectif du System Collector

D'après ton modèle `SystemMetrics`, nous avons trois métriques à collecter :&#x20;

Pasted text(20261108-143319).txt



| Métrique            | Description                       | Unité |
| ------------------- | --------------------------------- | ----- |
| `cpu_usage_percent` | Utilisation globale du processeur | %     |
| `memory_used_mb`    | Mémoire RAM utilisée              | MiB   |
| `memory_total_mb`   | Mémoire RAM totale                | MiB   |

## 2. Sources des données sous Ubuntu

Nous allons exploiter les fichiers virtuels Linux :

| Fichier         | Informations                    |
| --------------- | ------------------------------- |
| `/proc/stat`    | Compteurs CPU                   |
| `/proc/meminfo` | Informations sur la mémoire RAM |

Calcul CPU : nous devons comparer deux lectures successives de `/proc/stat`, car les compteurs CPU sont cumulatifs.

\\[ CPU\ Usage = \frac{\Delta Total-\Delta Idle}{\Delta Total}\times100 \\]

La première collecte donnera donc `cpu_usage_percent = None`.

Calcul mémoire :

\\[ MemoryUsed = MemoryTotal - MemoryAvailable \\]

## 3. Architecture prévue

SystemCollector

/proc/stat

CPU counters

/proc/meminfo

RAM information

SystemMetrics

## 4. Première étape — Examiner les données Linux

Avant de coder, vérifions les données disponibles sur ta machine.

Exécute ces deux commandes :

Commande 1 — CPU

```
head -n 5 /proc/stat
```

Commande 2 — RAM

```
grep -E 'MemTotal|MemAvailable' /proc/meminfo
```

Envoie-moi les résultats.

Nous allons ensuite développer `system.rs` progressivement, avec les mêmes étapes que les autres collectors : implémentation, compilation, test réel et tests unitaires.