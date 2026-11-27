# Publications et sources recommandees pour les prochaines iterations

Genere le 2026-11-24.

Ces references ne sont **pas** dans le corpus. Le niveau de verification est indique pour chacune : `metadonnees confirmees` signifie titre, auteurs et identifiant releves sur une page faisant autorite ; `a verifier` signifie que l'existence de l'article est attestee mais qu'au moins un champ (auteurs complets, annee, revue) n'a pas ete controle. Ne jamais integrer une reference `a verifier` au manifeste sans controle.

## 1. Priorite haute -- comblent une lacune identifiee

### Routing Attacks in Ethereum PoS: A Systematic Exploration

- Identifiant : arXiv 2505.07713
- Verification : a verifier (auteurs non controles)
- Interet pour EVA-Guard : Attaques BGP contre Ethereum PoS (StakeBleed, KnockBlock) : seule lacune majeure du corpus cote couche reseau inferieure. Utile pour distinguer eclipse locale et detournement de routage.
- URL : https://arxiv.org/abs/2505.07713

### Multiple Sides of 36 Coins: a multi-chain P2P measurement study

- Identifiant : arXiv 2511.15388
- Verification : a verifier (liste d'auteurs partielle : Kiffer, Heimbach, Trautwein, et al.)
- Interet pour EVA-Guard : Mesure longitudinale multi-chaines incluant discv4 et discv5 : fournit des ordres de grandeur recents pour calibrer un referentiel NORMAL.
- URL : https://arxiv.org/abs/2511.15388

### Total Eclipse of the Enclave: Detecting Eclipse Attacks From Inside TEEs

- Identifiant : Zheng, Tran, Arden (UC Santa Cruz)
- Verification : a verifier (annee et venue non controlees)
- Interet pour EVA-Guard : Detection d'eclipse par observation de la difficulte et des blocs : approche complementaire a la detection par caracteristiques de trafic.
- URL : https://par.nsf.gov/servlets/purl/10407126

### Decentralized Lightweight Detection of Eclipse Attacks on Bitcoin Clients

- Identifiant : arXiv 2007.02287
- Verification : metadonnees partielles (titre et identifiant confirmes, 2020)
- Interet pour EVA-Guard : Methodes de detection cote client transposables ; utile pour comparer les strategies de detection hors apprentissage.
- URL : https://arxiv.org/abs/2007.02287

### Ethereum Eclipse Attack Detection based on BG-XGBoost

- Identifiant : reference ResearchGate
- Verification : a verifier (auteurs, revue et annee non controles)
- Interet pour EVA-Guard : Detection d'eclipse par XGBoost : directement comparable a votre modele. A valider sur la page editeur avant toute citation.
- URL : https://www.researchgate.net/publication/380804497_Ethereum_Eclipse_Attack_Detection_based_on_BG-XGBoost

### Eclipse attack detection for blockchain network layer based on deep feature extraction

- Identifiant : Dai, Zhang, Dong
- Verification : a verifier (revue et annee incertaines : cite comme 2022 par un tiers)
- Interet pour EVA-Guard : Extraction de caracteristiques profondes pour la detection d'eclipse : alternative aux caracteristiques manuelles de votre pipeline.
- URL : -

## 2. Mesure et topologie du reseau Ethereum

- **DEthna: Accurate Ethereum Network Topology Discovery with Marked Transactions** -- arXiv 2402.03881 -- *a verifier (auteurs non controles)*. Decouverte de topologie par transactions marquees : methode de validation d'une hypothese d'isolement.
- **Ethna: Analyzing the Underlying Peer-to-Peer Network of the Ethereum Blockchain** -- arXiv 2010.01373 -- *a verifier (auteurs non controles)*. Mesure de distribution de degre des noeuds : base de comparaison pour un referentiel de connectivite.
- **EGNInfoLeaker: public key reuse and identity leakage in blockchain** -- arXiv 2507.01635 -- *a verifier (auteurs non controles)*. Reutilisation de cles publiques dans discv4/discv5 : utile pour detecter des pairs artificiels partageant une identite cryptographique (indice de Sybil).
- **Decentralization in Bitcoin and Ethereum Networks** -- arXiv 1801.03998 -- *a verifier (auteurs non controles)*. Comparaison de robustesse ; valeurs anciennes (2018), a n'utiliser que pour la methode.

## 3. Sources officielles a ajouter au prochain cycle

- **Code source go-ethereum** (`p2p/`, `p2p/discover/`, `metrics/`, `internal/ethapi/`) : seule source fiable pour la liste exacte des metriques Prometheus et le comportement de peering de la version deployee. Palliatif direct a la doc Geth figee en 2023.
- **`ethereum/go-ethereum` : `docs/postmortems/` et `docs/audits/`** : incidents reels et audits, deja clones mais non integres ; valeur forte pour un agent de confirmation.
- **Livre Lighthouse complet** (`book/src/`, 60+ pages) : seules 23 pages ciblees ont ete integrees ; le reste (migrations base de donnees, builders, checkpoint sync) peut servir a ecarter des causes benignes.
- **`ethereum/beacon-APIs` -- repertoires `beacon/`, `debug/`, `config/`** : seuls `node/`, `eventstream/` et la liveness validateur sont integres.
- **`ethereum/execution-specs`** : specifications de la couche execution (non integrees).
- **Notes de version de Geth et de Lighthouse** : indispensables pour rattacher une observation a une version de client et pour marquer les sources obsoletes.
- **EIP-778 (ENR), EIP-868, EIP-2124 (fork id)** depuis `ethereum/EIPs` : references normatives citees par les specifications devp2p deja presentes.

## 4. Methodologie de detection (hors Ethereum)

Lacune signalee a la section 7 du rapport de corpus : aucune reference methodologique ne justifie actuellement les choix du pipeline ML. Pistes a verifier avant integration : litterature sur la detection d'anomalies en series temporelles, la derive de distribution, la calibration de seuils et l'evaluation en contexte desequilibre. Aucun titre n'est recommande ici faute de verification.

## 5. Procedure d'ajout

```bash
# 1. verifier titre, auteurs, annee, venue, DOI sur la page editeur
# 2. ajouter l'entree dans la table de references du generateur de corpus
# 3. regenerer manifeste et rapports, puis reindexer
python ingestion/verify_corpus.py --check-urls
python ingestion/indexer.py --build
```

