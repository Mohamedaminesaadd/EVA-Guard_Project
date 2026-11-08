Bien sûr. Avant de commencer à coder, il est important de comprendre pourquoi nous avons besoin de la PHASE 10 — JSONL Storage et de la PHASE 11 — Monitoring Scheduler, et comment elles fonctionnent ensemble dans EVA-Guard.

# 1. Où sommes-nous dans le développement d'EVA-Guard ?

Jusqu'à maintenant, nous avons développé les collectors.

Un collector est un composant responsable de récupérer une catégorie de métriques depuis Geth ou depuis le système Linux.

Par exemple :

| Collector  | Informations récupérées                 |
| ---------- | --------------------------------------- |
| Blockchain | Numéro du dernier bloc, synchronisation |
| P2P        | Nombre de peers, connexions             |
| RPC        | Disponibilité et latence RPC            |
| Network    | Trafic réseau, connexions TCP           |
| System     | CPU, RAM                                |

Mais il manque deux éléments essentiels :

- Où allons-nous enregistrer les informations collectées ?
- Qui va déclencher les collectors régulièrement ?

C'est exactement le rôle des phases 10 et 11.

# 2. PHASE 10 — JSONL Storage

## A. Le concept

Imagine que ton système collecte des informations toutes les 5 secondes.

À chaque collecte, il produit un objet `Observation`.

Par exemple :

```
Observation {
    timestamp: "...",
    node_id: "geth-node-01",
    collection_interval: 5,
    peer_count: Some(10),
    block_number: Some(1200),
    cpu_usage_percent: Some(25.5),
    memory_used_mb: Some(4096),
    // ...
}
```

Cette observation représente l'état du nœud Ethereum à un instant donné.

Nous voulons conserver cet historique dans un fichier.

C'est le rôle du Storage.

## B. Pourquoi JSONL ?

JSONL signifie JSON Lines.

Contrairement à un fichier JSON classique qui contient un grand tableau, JSONL contient un objet JSON par ligne.

Fichier classique JSON

```
[
  {"peer_count": 10},
  {"peer_count": 9},
  {"peer_count": 3}
]
```

Fichier JSONL

```
{"peer_count": 10}
{"peer_count": 9}
{"peer_count": 3}
```

Pourquoi préférer JSONL dans notre projet ?

1. On peut ajouter une nouvelle observation sans réécrire tout le fichier.
2. Chaque observation est indépendante.
3. Le format est facile à lire avec Python.
4. Il sera facile d'analyser les données plus tard pour détecter des anomalies.

## C. Comment fonctionne le Storage ?

Imaginons que le fichier s'appelle `monitoring.jsonl`.

Le fonctionnement est :

Observation Rust

Données collectées par les collectors

Serde Serialization

Convertir Observation en JSON

JsonlStorage

Ajouter une ligne au fichier

monitoring.jsonl

Historique des observations

### Les responsabilités de `jsonl.rs`

Nous allons créer une structure :

```
pub struct JsonlStorage {
    // Gestion du fichier et du buffer
}
```

Elle aura principalement une méthode :

```
pub fn write_observation(
    &mut self,
    observation: &Observation
) -> std::io::Result<()>
```

Cette méthode devra :

1. Recevoir une observation.
2. La convertir en JSON avec Serde.
3. Écrire le JSON dans le fichier.
4. Ajouter un retour à la ligne.
5. Gérer les erreurs.

Le `flush` permet de vider le buffer vers le système de fichiers. Il est important pour éviter de conserver trop longtemps des observations uniquement en mémoire.

# 3. PHASE 11 — Monitoring Scheduler

## A. Le problème

Maintenant, nous savons collecter des métriques et nous savons où les sauvegarder.

Mais qui va exécuter tout cela ?

Nous ne voulons pas lancer manuellement chaque collector.

Nous voulons un système automatique qui fonctionne continuellement.

C'est le rôle du Scheduler.

## B. Le concept

Un Scheduler est un composant qui organise l'exécution périodique des tâches.

Dans notre cas :

- Toutes les 5 secondes.
- Exécuter les collectors.
- Rassembler leurs résultats.
- Construire une observation.
- Envoyer cette observation au Storage.
- Recommencer.

## C. Architecture du Scheduler

[Decoding the Observability Pipeline: A Java Architect's Guide to Metrics, Logs, and Traces - DEV Community](https://images.openai.com/static-rsc-4/ZUzoMt7QZ81dnzx0OWInDRavr0N_oFLAZfp0zTdhZQ44GsPyiWIy9-EjOjsNPRt5vbZkbtcam42y9NeWDJw3MUdq5w66lCHfNuYmp8Cq9zRsi7kgEEw75A5FZLD5hM8yf-xvgfswYPnhqaPQPK_FT8ieM1VuWEEW6X3kzQmStHs?purpose=inline)

[dev.to](https://dev.to/mohamadawwaad/decoding-the-observability-pipeline-a-java-architects-guide-to-metrics-logs-and-traces-4e?utm_source=chatgpt.com)

Voici le fonctionnement logique :

```
              MAIN
               |
               v
        MonitoringScheduler
               |
       Initialize components
               |
       +-------+-------+
       |       |       |
      Geth   Collectors Storage
       |       |       |
       +-------+-------+
               |
               v
          Start Loop
               |
          Every 5 sec
               |
       Execute Collectors
               |
       Build Observation
               |
        Write JSONL
               |
         Wait 5 sec
               |
          Repeat...
```

## D. Pourquoi utiliser Tokio ?

Tokio est un runtime asynchrone pour Rust.

Il permet notamment de gérer plusieurs opérations asynchrones sans bloquer inutilement le programme.

Dans notre projet :

- Les appels RPC à Geth sont asynchrones.
- Plusieurs collectors peuvent être exécutés indépendamment.
- Le Scheduler peut gérer une boucle périodique.
- L'arrêt du programme peut être géré proprement.

Par exemple, nous pourrons utiliser :

```
tokio::time::interval(
    Duration::from_secs(5)
);
```

Cela permettra de déclencher une collecte périodiquement.

## E. Un point architectural important : les erreurs

Imaginons que Geth ne réponde plus.

Le collector RPC retourne une erreur ou indique que le RPC est indisponible.

Est-ce qu'on doit arrêter tous les autres collectors ?

Non.

Le CPU, la RAM et les statistiques réseau restent accessibles même si Geth est arrêté.

C'est pourquoi chaque collector doit être traité indépendamment.

Exemple :

| Collector  | Résultat     |
| ---------- | ------------ |
| Blockchain | Échec        |
| P2P        | Échec        |
| RPC        | Indisponible |
| Network    | Réussi       |
| System     | Réussi       |

Le Scheduler doit tout de même construire une observation avec les métriques disponibles et les valeurs `None` pour celles qui n'ont pas pu être mesurées.

# 4. Différence entre les deux phases

| PHASE 10 — Storage       | PHASE 11 — Scheduler         |
| ------------------------ | ---------------------------- |
| Sauvegarder les données  | Organiser les collectes      |
| Gérer les fichiers       | Gérer le temps               |
| Sérialiser en JSON       | Exécuter les collectors      |
| Ajouter les observations | Construire les observations  |
| Garantir la persistance  | Gérer les erreurs et l'arrêt |

En résumé :

- Storage répond à la question : « Où conserver les observations ? »
- Scheduler répond à la question : « Quand et comment collecter les observations ? »

# 5. Comment ces phases s'intègrent dans EVA-Guard ?

Notre architecture globale devient :

```
                Ethereum Geth Node
                       |
                       v
                Geth JSON-RPC
                       |
                       v
                GethClient (Rust)
                       |
                       v
               Monitoring Engine
                       |
             +---------+---------+
             |         |         |
          Blockchain  P2P       RPC
          Collector Collector Collector
             |         |         |
          Network    System      ...
          Collector Collector
             |         |         |
             +---------+---------+
                       |
                       v
                  Observation
                       |
                       v
                  JSONL Storage
                       |
                       v
                 monitoring.jsonl
                       |
                       v
             Intelligence Engine
             (Future processing)
```

Le fichier JSONL constituera une première source de données historiques pour le futur moteur d'intelligence d'EVA-Guard.

# 6. Ordre de développement que je recommande

Nous allons travailler de manière progressive :

1. Comprendre et vérifier le modèle `Observation`.
2. Créer `storage/mod.rs`.
3. Implémenter `storage/jsonl.rs`.
4. Tester l'écriture d'une observation.
5. Tester plusieurs observations.
6. Vérifier la persistance après redémarrage.
7. Créer `monitoring/mod.rs`.
8. Implémenter `monitoring/scheduler.rs`.
9. Intégrer tous les collectors.
10. Tester le monitoring continu avec Geth.

La prochaine étape sera le Storage, pas encore le Scheduler. Il faut d'abord avoir un composant de persistance fiable pour que le Scheduler puisse l'utiliser.