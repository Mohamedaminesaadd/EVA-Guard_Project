Oui ! Et bonne nouvelle : **ton installation fonctionne maintenant.** Tu as réussi à démarrer ton réseau Ethereum privé avec :

- 5 nœuds Geth (Execution Layer).
- 5 nœuds Lighthouse Beacon (Consensus Layer).
- 5 Validator Clients Lighthouse.
- 5 participants, chacun avec son propre ensemble de services.

C'est confirmé par le résultat de ton exécution : les 15 services principaux sont indiqués `RUNNING`. Attention : cela confirme le démarrage des services, pas encore à lui seul la bonne production de blocs et la finalisation du réseau.

Je te propose de préparer une procédure reproductible pour pouvoir refaire l'installation sur un autre PC, **sans dépendre de ChatGPT**.

# **1. Préparer le deuxième PC**

Pour ton laboratoire avec 5 participants, voici les ressources que je recommande :

| Ressource | Recommandation pratique                   |
| --------- | ----------------------------------------- |
| OS        | Ubuntu 24.04 LTS                          |
| CPU       | 8 cœurs recommandés                       |
| RAM       | 16 Go minimum, 32 Go recommandés          |
| Stockage  | 50 Go libres minimum, davantage conseillé |
| Internet  | Connexion stable                          |
| GPU       | Pas nécessaire                            |

Ces chiffres sont des recommandations pratiques pour ton laboratoire, pas des exigences officielles du projet.

# **2. Installer les dépendances**

Tu as besoin de seulement deux outils principaux :

1. Docker : exécuter les conteneurs Ethereum.
2. Kurtosis : déployer et orchestrer le réseau.

## **Étape 2.1 — Installer Docker**

Référence officielle : [Docker Engine – Ubuntu installation](https://docs.docker.com/engine/install/ubuntu/?utm_source=chatgpt.com).

Après installation, vérifie :

```
docker --version
docker compose version
sudo systemctl status docker
```

Démarre Docker si nécessaire :

```
sudo systemctl enable --now docker
```

Teste son fonctionnement :

```
sudo docker run hello-world
```

## **Étape 2.2 — Installer Kurtosis**

Référence officielle : [Kurtosis – Installation](https://docs.kurtosis.com/install/?utm_source=chatgpt.com).

Sur Ubuntu :

```
echo "deb [trusted=yes] https://sdk.kurtosis.com/kurtosis-cli-release-artifacts/ /" | sudo tee /etc/apt/sources.list.d/kurtosis.list

sudo apt update

sudo apt install kurtosis-cli
```

Vérifie :

```
kurtosis version
```

Démarre le moteur :

```
kurtosis engine start
```

Vérifie :

```
kurtosis engine status
```

# **3. Récupérer le projet Ethereum**

Tu dois récupérer le package officiel utilisé pour construire ton réseau.

Référence : [ethpandaops/ethereum-package](https://github.com/ethpandaops/ethereum-package?utm_source=chatgpt.com).

Sur le deuxième PC :

```
mkdir -p ~/EVA_ethereum
cd ~/EVA_ethereum
```

Clone le dépôt :

```
git clone https://github.com/ethpandaops/ethereum-package.git
```

Pour reproduire exactement la version que tu as utilisée, tu peux aussi verrouiller le dépôt sur le commit que tu as testé :

```
cd ethereum-package

git checkout 74cdb681a14a46c7177df17c0ea186f0aa184015
```

C'est important : la branche `main` peut évoluer et modifier les paramètres disponibles.

# **4. Préparer la configuration du réseau**

Tu dois récupérer ton fichier :

```
ethereum-network/
└── kurtosis/
    └── network_params.yaml
```

C'est ce fichier qui définit notamment :

- Les clients Execution Layer.
- Les clients Consensus Layer.
- Le nombre de participants.
- Les paramètres des validateurs.
- Les paramètres du réseau et des forks Ethereum.

**Ne recrée pas ce fichier à partir de zéro.** Copie celui que tu as utilisé lors du lancement réussi.

Pour comprendre les paramètres disponibles :

- [Configuration officielle network_params.yaml](https://github.com/ethpandaops/ethereum-package/blob/main/network_params.yaml?utm_source=chatgpt.com).
- [Documentation du package Ethereum](https://github.com/ethpandaops/ethereum-package/blob/main/README.md?utm_source=chatgpt.com).

# **5. Lancer le réseau**

Depuis le dossier principal `EVA_ethereum` :

```
kurtosis run \
  --enclave eva-ethereum \
  ./ethereum-package/kurtosis.yml \
  --args-file ./ethereum-network/kurtosis/network_params.yaml
```

Le package va automatiquement :

1. Générer les données Genesis.
2. Configurer les clients Geth.
3. Configurer les Beacon Nodes Lighthouse.
4. Générer les clés des validateurs.
5. Démarrer les Validator Clients.
6. Connecter les différents participants.

C'est le fonctionnement prévu par le package officiel.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://github.com\&sz=32)

GitHub+1



# **6. Vérifier que le réseau fonctionne**

Après le lancement, utilise les commandes suivantes.

**Voir les enclaves :**

```
kurtosis enclave ls
```

**Inspecter ton réseau :**

```
kurtosis enclave inspect eva-ethereum
```

**Consulter les services :**

```
kurtosis enclave inspect eva-ethereum
```

Tu dois retrouver les services suivants :

| Service                     | Nombre attendu |
| --------------------------- | -------------- |
| Geth                        | 5              |
| Lighthouse Beacon Node      | 5              |
| Lighthouse Validator Client | 5              |

**Consulter les logs :**

```
kurtosis service logs eva-ethereum el-1-geth-lighthouse
```

Tu peux aussi consulter les ports RPC publiés dans le résultat d'`enclave inspect`, puis tester le JSON-RPC de Geth avec `eth_blockNumber`.

# **7. Organiser ton projet pour le transférer**

Je te conseille de conserver cette organisation :

```
EVA_ethereum/
│
├── ethereum-package/
│
├── ethereum-network/
│   ├── kurtosis/
│   │   ├── network_params.yaml
│   │   └── README.md
│   └── scripts/
│
├── monitoring-rust/
│
├── experiments/
│   ├── baseline/
│   ├── eclipse/
│   └── rpc_flooding/
│
├── datasets/
│   ├── normal/
│   ├── eclipse/
│   └── rpc_flooding/
│
└── README.md
```

Tu peux transférer les fichiers avec Git ou une clé USB.

**Attention aux données sensibles :** ton résultat contient des clés privées de comptes de test et des informations générées pour les validateurs. Ne publie pas ces clés, les keystores ou les fichiers JWT. Elles ne doivent jamais être utilisées pour des fonds réels.

# **8. Les références à conserver**

Voici les cinq ressources que je te conseille de garder dans ton `README.md`.

[image](https://www.google.com/s2/favicons?domain=https://docs.docker.com\&sz=32)

**1. Docker Documentation**

[Installation Docker Ubuntu](https://docs.docker.com/engine/install/ubuntu/?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://docs.kurtosis.com\&sz=32)

**2. Kurtosis Documentation**

[Installation de Kurtosis](https://docs.kurtosis.com/install/?utm_source=chatgpt.com)[Construire un réseau Ethereum privé](https://docs.kurtosis.com/how-to-compose-your-own-testnet/?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://github.com\&sz=32)

**3. Ethereum Package**

[Dépôt officiel](https://github.com/ethpandaops/ethereum-package?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://geth.ethereum.org\&sz=32)

**4. Geth Documentation**

[Private Networks via Kurtosis](https://geth.ethereum.org/docs/fundamentals/kurtosis?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://github.com\&sz=32)

**5. Lighthouse**

[Dépôt officiel Lighthouse](https://github.com/sigp/lighthouse?utm_source=chatgpt.com)

## **Mon conseil : apprendre à maîtriser les outils indépendamment**

Pour être autonome, ne mémorise pas seulement les commandes. Comprends ces cinq éléments :

1. **Docker** : images, conteneurs, réseaux, volumes et ports.
2. **Kurtosis** : packages, enclaves, services et artifacts.
3. **Ethereum Package** : `main.star`, `network_params.yaml` et les paramètres des participants.
4. **Ethereum PoS** : relation entre Geth, Beacon Node et Validator Client.
5. **Monitoring** : JSON-RPC, Beacon API, Prometheus et logs.

Pour ton projet EVA-Guard, le plus important sera ensuite de comprendre comment récupérer les métriques des cinq nœuds et comment isoler un participant pour expérimenter une attaque Eclipse dans ton réseau privé.

-------------------------------------------------------------
Voici une documentation que tu peux intégrer directement dans ton rapport de projet **EVA-Guard**. Elle présente, pour chaque vérification, son objectif, la méthode d'exécution, les commandes à utiliser et les résultats attendus.

# **Phase 1 — Vérification et validation de l'environnement Ethereum**

## **1. Objectif général**

Avant de simuler les attaques Eclipse et RPC Flooding, nous devons vérifier que notre réseau Ethereum privé fonctionne correctement.

Cette phase permet de :

- Vérifier que les nœuds Ethereum sont accessibles.
- Contrôler la synchronisation entre les nœuds.
- Vérifier le fonctionnement du consensus Proof of Stake.
- Vérifier la participation des validateurs.
- Tester les interfaces JSON-RPC.
- Préparer l'environnement pour connecter le moteur de monitoring Rust.

**Pourquoi cette phase est importante ?**

Elle permet de construire un état de référence (*Baseline NORMAL*). Ainsi, les anomalies observées pendant les attaques pourront être comparées au comportement normal du réseau.

## **2. Vérification des nœuds Lighthouse**

### **Objectif**

Vérifier que les cinq nœuds Lighthouse sont démarrés et que leurs API Beacon REST sont accessibles.

### **Méthode**

Nous envoyons une requête HTTP à chaque nœud pour récupérer son bloc courant (*Head Block*).

### **Commande**

```
for port in 32794 32806 32803 32800 32797; do
    echo "===== LIGHTHOUSE $port ====="
    curl -s http://127.0.0.1:$port/eth/v1/beacon/headers/head
    echo
done
```

### **Résultat attendu**

Pour chaque nœud, nous devons obtenir :

- Une réponse HTTP contenant les informations du bloc courant.
- Un `root` représentant l'identifiant du bloc.
- Un `slot` représentant sa position dans la chaîne.

### **Interprétation**

- Si le nœud répond : son API est accessible.
- Si plusieurs nœuds ont le même `root` : ils observent le même bloc courant.
- Si un nœud possède un slot inférieur : il peut être en retard de synchronisation.

**Résultat obtenu :** les cinq nœuds ont retourné le même bloc, avec le slot 70.

## **3. Vérification de la synchronisation et de la finalité**

### **Objectif**

Vérifier que les nœuds partagent le même état de consensus et que le réseau arrive à finaliser des blocs.

### **Méthode**

Nous interrogeons l'API Beacon REST pour récupérer les checkpoints de finalité.

### **Commande**

```
for port in 32794 32806 32803 32800 32797; do
    echo "===== NODE $port ====="
    curl -s http://127.0.0.1:$port/eth/v1/beacon/states/head/finality_checkpoints
    echo
done
```

### **Résultat attendu**

La réponse contient trois informations importantes :

| Champ                | Signification                |
| -------------------- | ---------------------------- |
| `previous_justified` | Epoch précédemment justifiée |
| `current_justified`  | Epoch actuellement justifiée |
| `finalized`          | Epoch finalisée              |

### **Interprétation**

- Une epoch justifiée indique qu'un checkpoint a obtenu le niveau de soutien nécessaire du consensus.
- Une epoch finalisée indique qu'un checkpoint est définitivement confirmé selon les règles du consensus.
- Si les valeurs restent à zéro, la finalité n'est pas encore démontrée.

**Résultat obtenu précédemment :** les cinq nœuds retournaient des epochs justifiées et finalisées égales à 0. Cette vérification doit donc être poursuivie.

## **4. Vérification des Validator Clients**

### **Objectif**

Vérifier que les Validator Clients sont actifs et qu'ils participent aux opérations de validation.

### **Méthode**

Nous consultons les métriques Prometheus exposées par chaque Validator Client.

### **Commande**

```
for port in 32809 32813 32810 32812 32811; do
    echo "===== VALIDATOR CLIENT $port ====="
    curl -s http://127.0.0.1:$port/metrics \
    | grep -Ei 'attestation|validator'
    echo
done
```

### **Résultat attendu**

Nous recherchons notamment :

- `vc_validators_enabled_count` : nombre de validateurs activés.
- `vc_validators_total_count` : nombre total de validateurs connus.
- `vc_signed_attestations_total` : nombre d'attestations signées.

### **Interprétation**

- Un Validator Client actif doit posséder des validateurs configurés.
- Les attestations signées avec succès montrent que le client effectue des opérations de signature.
- Il faut également vérifier leur inclusion dans la chaîne pour confirmer leur participation effective au consensus.

**Résultat obtenu précédemment :** les métriques indiquaient un validateur activé par client et des attestations signées avec succès. Leur inclusion dans la chaîne reste à confirmer.

## **5. Vérification des nœuds Geth et JSON-RPC**

### **Objectif**

Vérifier que les clients d'exécution Geth répondent correctement aux requêtes JSON-RPC.

Cette vérification est importante car l'attaque RPC Flooding ciblera l'interface RPC.

### **Méthode**

Nous envoyons une requête JSON-RPC `eth_blockNumber` à chaque endpoint Geth.

### **Commande**

Les ports doivent être remplacés par les ports RPC réellement publiés pour les cinq services Geth.

```
curl -s -X POST http://127.0.0.1:PORT_RPC \
-H "Content-Type: application/json" \
-d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

### **Résultat attendu**

Une réponse JSON contenant :

```
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x..."
}
```

### **Interprétation**

- Une réponse correcte confirme que l'API RPC fonctionne.
- Le numéro de bloc permet de suivre la progression de la blockchain.
- Le temps de réponse peut être mesuré pour construire une référence de latence normale.

## **6. Vérification du moteur de monitoring Rust**

### **Objectif**

Vérifier que notre moteur Rust peut collecter et enregistrer les observations provenant du réseau Ethereum.

### **Méthode**

Nous démarrons le moteur de monitoring existant et vérifions les observations enregistrées.

### **Commandes**

```
cd ~/Downloads/EVA_ethereum/monitoring-rust
cargo test
```

Puis :

```
cargo run
```

Dans un autre terminal, consulter les observations :

```
tail -n 10 data/observations.jsonl
```

### **Résultat attendu**

Le fichier doit contenir des observations structurées en JSON, correspondant aux métriques collectées.

### **Interprétation**

- Les tests vérifient le bon fonctionnement des modules Rust.
- L'exécution vérifie le fonctionnement du moteur.
- Le fichier JSONL permet de conserver les observations pour constituer notre dataset.

## **7. Synthèse des vérifications**

| Vérification      | Objectif                      | État actuel                                |
| ----------------- | ----------------------------- | ------------------------------------------ |
| Lighthouse        | Accessibilité des 5 nœuds     | Vérifié                                    |
| Synchronisation   | Même bloc courant             | Vérifié                                    |
| Finalité          | Consensus et finalisation     | À poursuivre                               |
| Validator Clients | Participation des validateurs | Partiellement vérifié                      |
| Geth / RPC        | Fonctionnement des API RPC    | À vérifier                                 |
| Monitoring Rust   | Collecte des métriques        | Tests déjà réussis, intégration à vérifier |

## **8. Étape suivante : construction du dataset NORMAL**

Après validation de l'environnement, nous enregistrerons les métriques du réseau sans aucune attaque.

Ce dataset constituera notre référence pour les expériences suivantes.

Les classes de données prévues sont :

| Classe         | Description                                                |
| -------------- | ---------------------------------------------------------- |
| `NORMAL`       | Fonctionnement normal du réseau                            |
| `ECLIPSE`      | Comportement du réseau pendant une simulation Eclipse      |
| `RPC_FLOODING` | Comportement du réseau pendant une simulation RPC Flooding |

**Livrable de cette phase :** un environnement Ethereum privé vérifié, un ensemble de métriques de référence et un moteur Rust prêt à collecter les observations.

**Notre prochaine action pratique :** terminer la vérification de la finalité (section 3). Ne lance pas encore les attaques : nous devons d'abord établir que le réseau fonctionne correctement sans perturbation.