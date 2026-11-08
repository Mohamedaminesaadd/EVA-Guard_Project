Dans Rust, les modules (Modules) permettent d'organiser le code en plusieurs fichiers et dossiers pour séparer les responsabilités, éviter un fichier géant et faciliter la maintenance.

Dans EVA-Guard, c'est essentiel parce que nous avons plusieurs composants : communication avec Geth, collecte des métriques, stockage et orchestration.

# 1. Concept — Comment Rust organise les modules ?

Rust utilise trois mécanismes importants :

- `mod` : déclarer un module.
- `pub` : rendre un élément accessible depuis l'extérieur de son module.
- `use` : importer un élément pour l'utiliser plus facilement.

Il faut distinguer déclarer un module et utiliser un module.

## 2. Premier exemple pratique

Imaginons un petit projet :

```
my_project/
│
└── src/
    ├── main.rs
    ├── lib.rs
    └── database.rs
```

Dans `lib.rs` :

```
pub mod database;
```

Dans `database.rs` :

```
pub fn connect() {
    println!("Database connected!");
}
```

Dans `main.rs` :

```
fn main() {
    my_project::database::connect();
}
```

Que se passe-t-il ?

1. `lib.rs` déclare le module `database`.
2. Rust cherche son implémentation dans `database.rs`.
3. `pub mod` rend le module accessible depuis l'extérieur de la bibliothèque.
4. `pub fn` rend la fonction `connect()` accessible.
5. `main.rs` appelle cette fonction.

Résultat :

```
Database connected!
```

# 3. Organisation avec des sous-modules

Maintenant, imaginons que notre module database possède plusieurs fonctionnalités.

Nous pouvons organiser le code ainsi :

```
src/
│
├── lib.rs
│
└── database/
    ├── mod.rs
    ├── connection.rs
    └── query.rs
```

Dans `lib.rs` :

```
pub mod database;
```

Dans `database/mod.rs` :

```
pub mod connection;
pub mod query;
```

Dans `database/connection.rs` :

```
pub fn connect() {
    println!("Connecting to database...");
}
```

Dans `database/query.rs` :

```
pub fn execute() {
    println!("Executing query...");
}
```

Pour utiliser les fonctions :

```
my_project::database::connection::connect();

my_project::database::query::execute();
```

Comprendre la hiérarchie

lib.rs

Racine de la bibliothèque

database

database/mod.rs

connection.rs

Connexion

query.rs

Requêtes

# 4. Différence entre mod.rs et les autres fichiers

Dans notre exemple :

| Fichier           | Rôle                      |
| ----------------- | ------------------------- |
| `lib.rs`          | Racine de la bibliothèque |
| `database/mod.rs` | Déclare les sous-modules  |
| `connection.rs`   | Implémente la connexion   |
| `query.rs`        | Implémente les requêtes   |

Attention : depuis Rust 2018, il existe deux façons d'organiser un module.

Ancienne convention :

```
database/
├── mod.rs
├── connection.rs
└── query.rs
```

Convention moderne :

```
database.rs
database/
├── connection.rs
└── query.rs
```

Les deux sont valides. Nous pouvons conserver `mod.rs` pour respecter l'architecture de ton document EVA-Guard.

# 5. Application à EVA-Guard

Regardons maintenant comment cela s'applique à ton projet.

Dans `lib.rs` :

```
pub mod collectors;
pub mod config;
pub mod geth;
pub mod models;
pub mod monitoring;
pub mod storage;
```

Prenons le module `geth`.

```
geth/
├── mod.rs
└── client.rs
```

Dans `geth/mod.rs` :

```
pub mod client;
```

Dans `geth/client.rs`, nous implémenterons plus tard le client JSON-RPC.

Pour l'utiliser ailleurs :

```
use crate::geth::client;
```

Ici :

- `crate` représente la racine de notre crate actuelle.
- `geth` représente le module Geth.
- `client` représente le sous-module client.

C'est ce mécanisme qui permettra aux collecteurs d'utiliser notre client Geth.

# 6. Les trois mots-clés à retenir

| Mot-clé | Fonction               |
| ------- | ---------------------- |
| `mod`   | Déclare un module      |
| `pub`   | Rend un élément public |
| `use`   | Importe un chemin      |

Exemple :

```
pub mod geth;

use crate::geth::client;
```

# 7. Questions pour vérifier ta compréhension

Essaie de répondre avant de regarder les explications dans une autre LLM.

Mini-quiz Rust Modules

0/4 réponses

1\. Quel est le rôle de \`mod\` ?

Exécuter une fonction

Déclarer un module

Compiler le projet

2\. Quel est le rôle de \`pub\` ?

Rendre un élément accessible

Créer un fichier

Importer une bibliothèque

3\. Quel fichier déclare les sous-modules de database dans notre architecture ?

main.rs

Cargo.toml

database/mod.rs

4\. Que représente \`crate\` dans \`crate::geth::client\` ?

Le dossier du projet

La racine du crate actuel

Une dépendance externe

Vérifier mes réponses

## 8. Questions à poser à une autre LLM

Pour approfondir :

1. Explain Rust module resolution step by step.
2. What is the difference between `crate`, `self` and `super`?
3. Explain Rust visibility rules with practical examples.
4. What happens if a module is declared with `mod` but not `pub mod`?
5. How should I organize modules in a medium-sized Rust application?

Prochaine étape pratique : nous allons créer réellement l'architecture des modules EVA-Guard, en commençant par `config.rs`, puis `geth/mod.rs` et `geth/client.rs`. Nous n'implémenterons pas encore le JSON-RPC.