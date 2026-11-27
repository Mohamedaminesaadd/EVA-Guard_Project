Ces informations proviennent de l'endpoint `/eth/v1/beacon/headers/head` de Lighthouse. Elles décrivent l'en-tête (*Header*) du dernier bloc de la Beacon Chain Ethereum.

L'objectif est de savoir **quel bloc le nœud considère comme sa tête actuelle**, qui l'a proposé et à quel bloc précédent il est lié.

## **1. Explication de chaque information**

**1. Root**

Identifiant cryptographique unique du bloc, calculé à partir de son contenu (hash du header).

Exemple : `0xabcd...1234`

Il permet d'identifier précisément un bloc.

**2. Canonical**

Indique si ce bloc appartient à la chaîne canonique, c'est-à-dire la chaîne actuellement reconnue par le consensus.

- `true` : bloc appartenant à la chaîne canonique.
- `false` : bloc non canonique, par exemple après un réarrangement (*reorg*).

**3. Header**

Contient les métadonnées principales du bloc Beacon. Dans ton code, il contient notamment `message`, qui regroupe les champs suivants.

**4. Slot**

Numéro du créneau temporel auquel le bloc appartient.

Ethereum utilise des slots de 12 secondes.

Exemple : `1000` correspond au slot numéro 1000.

**5. Proposer index**

Index du validateur désigné pour proposer le bloc pendant ce slot.

Exemple : `125` signifie que le validateur portant l'index 125 est le proposant désigné.

**6. Parent root**

Hash du bloc parent, c'est-à-dire le bloc précédent dans la chaîne.

Il permet de vérifier la relation entre deux blocs successifs.

**7. State root**

Hash de l'état du consensus après l'application du bloc.

Il engage cryptographiquement l'état résultant, notamment les informations relatives aux validateurs et au consensus.

**8. Body root**

Hash du contenu du corps du bloc Beacon.

Il permet de vérifier l'intégrité des données contenues dans ce corps, notamment les attestations et autres opérations du consensus.

## **2. Comment ces informations sont liées ?**

Voici une représentation simplifiée :

**Bloc précédent**

Parent root = 0x111...

Canonical = true

**Bloc actuel**

- Root : `0x222...`
- Slot : `1000`
- Proposer index : `125`
- Parent root : `0x111...`
- State root : `0x333...`
- Body root : `0x444...`

## **3. Quel est l'intérêt pour EVA-Guard ?**

Ces données peuvent aider ton système à détecter des anomalies liées au consensus Ethereum.

| Information    | Utilisation possible                                  |
| -------------- | ----------------------------------------------------- |
| Root           | Identifier un bloc                                    |
| Canonical      | Détecter des blocs non canoniques                     |
| Slot           | Détecter des slots manqués ou des retards             |
| Proposer index | Analyser le comportement des proposants               |
| Parent root    | Repérer des changements de chaîne ou des incohérences |
| State root     | Comparer l'état de consensus                          |
| Body root      | Vérifier l'intégrité du contenu du bloc               |

**Exemple concret :** si ton collecteur observe plusieurs changements de `root` ou de `parent_root` pour un même slot, cela peut justifier une investigation sur un éventuel *fork* ou *chain reorganization*. Ce n'est pas automatiquement une attaque : il faut corréler ces observations avec d'autres métriques.

**À retenir :** ces champs décrivent l'identité, la position et la structure d'un bloc Beacon. Ils constituent des données de base pour le monitoring de la couche Consensus dans EVA-Guard.