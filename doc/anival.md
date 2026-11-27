**Anvil** est un outil de **Foundry** qui permet de créer une blockchain Ethereum locale sur ta machine pour développer, tester et déployer des Smart Contracts sans utiliser le réseau Ethereum réel.

Dans ton terminal, tu viens de démarrer un réseau Ethereum local.

## **1. À quoi sert Anvil ?**

Anvil joue le rôle d'un réseau Ethereum de développement.

Il simule le fonctionnement d'un véritable réseau Ethereum :

- Création de comptes Ethereum (EOA).
- Envoi et réception de transactions.
- Déploiement de Smart Contracts.
- Exécution de fonctions Solidity.
- Gestion du gas et des blocs.
- Interaction avec le réseau via JSON-RPC.

[Intro to Building and Testing with Foundry - HackMD](https://images.openai.com/static-rsc-4/oELbL9Pi_9f8ZkKl-sEwB7Ac5ao_S7AjaYMpOii4T3c-Cf0byYhICbdX6kfXPwW89JXh7m4Ag2Np4BvexqJTEbaaXd91JZLE2GXRmlhyjLf0vessF-nOnZ7yfghRV6Z4MhCGRPNUcoFRFUHTYRCcJ0xLB7hGcck4Cd_0DzMXEDo?purpose=inline)

[hackmd.io](https://hackmd.io/%40meowy/ryaGG5Vaj?utm_source=chatgpt.com)

## **2. Comprendre ton résultat Anvil**

Analysons les informations affichées.

### **A. Available Accounts**

```
(0) 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    10000 ETH
```

Anvil crée automatiquement 10 comptes Ethereum de test.

Chaque compte possède :

- Une adresse publique.
- Une clé privée.
- Un solde initial de 10 000 ETH fictifs.

Par exemple :

| Élément     | Valeur                       |
| ----------- | ---------------------------- |
| Adresse     | `0xf39Fd6e...`               |
| Balance     | 10 000 ETH                   |
| Type        | EOA                          |
| Utilisation | Transactions et déploiements |

**Attention :** les clés privées affichées sont des clés de test publiques et connues. Ne les utilise jamais avec de vrais fonds.

### **B. Private Keys**

```
(0) 0xac0974bec39a17e36...
```

Chaque adresse Ethereum est associée à une clé privée.

La clé privée permet de :

- Signer une transaction.
- Envoyer des ETH.
- Déployer un Smart Contract.
- Interagir avec un contrat nécessitant une signature.

La relation est :

**Private Key**

Cryptographie elliptique secp256k1

**Public Key**

Keccak-256

**Ethereum Address**

### **C. Wallet / Mnemonic**

```
Mnemonic:
test test test test test test test test test test test junk
```

C'est une phrase de récupération composée de 12 mots.

Elle permet de dériver plusieurs comptes Ethereum.

Le chemin de dérivation est :

```
m/44'/60'/0'/0/
```

Il utilise la norme BIP-44 avec le coin type Ethereum (60).

### **D. Chain ID**

```
Chain ID: 31337
```

C'est l'identifiant du réseau Ethereum local.

Quelques exemples :

| Réseau           | Chain ID |
| ---------------- | -------- |
| Ethereum Mainnet | 1        |
| Sepolia          | 11155111 |
| Anvil            | 31337    |

Le Chain ID permet notamment de distinguer les réseaux lors de la signature des transactions.

### **E. Base Fee**

```
Base Fee: 1000000000
```

C'est le coût de base du gas exprimé en wei.

Ici :

\\[ 10^9 \text{ wei} = 1 \text{ Gwei} \\]

### **F. Gas Limit**

```
Gas Limit: 30000000
```

C'est la limite maximale de gas autorisée pour l'exécution d'un bloc.

### **G. Listening**

```
Listening on 127.0.0.1:8545
```

C'est très important !

Ton réseau Ethereum local expose une API JSON-RPC à cette adresse :

```
http://127.0.0.1:8545
```

Tu peux utiliser cette URL avec :

- MetaMask.
- Foundry (Cast).
- Web3.py.
- Ethers.js.
- Applications FastAPI.

## **3. Tester ton réseau Ethereum local**

Ouvre un deuxième terminal sans arrêter Anvil.

**Étape 1 : vérifier le réseau**

```
cast chain-id --rpc-url http://127.0.0.1:8545
```

Résultat attendu :

```
31337
```

**Étape 2 : consulter le solde du compte 0**

```
cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
--ether \
--rpc-url http://127.0.0.1:8545
```

Résultat :

```
10000.0
```

**Étape 3 : consulter le dernier bloc**

```
cast block latest --rpc-url http://127.0.0.1:8545
```

**Étape 4 : envoyer une transaction**

On utilise le compte 0 pour envoyer 1 ETH au compte 1 :

```
cast send \
0x70997970C51812dc3A010C7d01b50e0d17dc79C8 \
--value 1ether \
--private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
--rpc-url http://127.0.0.1:8545
```

Puis vérifie le solde du compte 1 :

```
cast balance \
0x70997970C51812dc3A010C7d01b50e0d17dc79C8 \
--ether \
--rpc-url http://127.0.0.1:8545
```

Tu devrais obtenir 10001 ETH.

## **4. Anvil vs Geth vs Beacon Node**

C'est important pour comprendre l'architecture Ethereum.

| Outil      | Rôle                                           |
| ---------- | ---------------------------------------------- |
| Anvil      | Blockchain locale pour développement et tests  |
| Geth       | Client d'exécution Ethereum (Execution Layer)  |
| Lighthouse | Client de consensus Ethereum (Consensus Layer) |
| Prysm      | Autre client de consensus                      |
| Nethermind | Autre client d'exécution                       |

[Spin up your own Ethereum node | ethereum.org](https://images.openai.com/static-rsc-4/k-1X9TcAnGnQHoJ_aqeIFmQ4aKRZKjNnd92Efb3sceakcGF8XtBXrrdPJCNCFW1uCXG9K8YLjLtIv8FhiB5_yJpn-DN_K3BMfh01_Fjm0bht1WnYfYZteW6C0nXm8DafOC3QzbgxBP3TEAnWG4pNmXj0820A0Hk0e2UmR3dODpg?purpose=inline)

[ethereum.org](https://ethereum.org/developers/docs/nodes-and-clients/run-a-node/?utm_source=chatgpt.com)

**Attention à une différence fondamentale :** Anvil simule un environnement blockchain local. Il ne constitue pas un véritable nœud Ethereum Proof of Stake avec un Execution Client et un Consensus Client connectés.

## **5. Lien avec ton projet EVA-Guard**

Dans ton projet EVA-Guard, Anvil peut être utile pour tester certains scénarios d'attaques et les comportements des transactions.

Par exemple :

- RPC Flooding : envoyer un grand nombre de requêtes RPC.
- Transaction Flooding : générer beaucoup de transactions.
- Smart Contract Attacks : tester des contrats vulnérables.
- Monitoring : mesurer la latence RPC, le nombre de transactions et les ressources système.

Mais pour tester réellement les attaques Eclipse contre des nœuds Ethereum, conserve ton environnement multi-nœuds Geth + Lighthouse sous Kurtosis.

## **6. Ce que je te conseille de maîtriser ensuite**

Pour maîtriser Ethereum, suis cet ordre :

1. **Anvil** : comprendre et manipuler une blockchain locale.
2. **Cast** : envoyer des transactions et interroger Ethereum.
3. **Forge** : compiler, tester et déployer des Smart Contracts.
4. **Geth** : comprendre un Execution Client.
5. **Lighthouse** : comprendre le Consensus Client et Beacon Chain.
6. **JSON-RPC** : comprendre la communication entre les applications et les nœuds.

**Prochaine étape :** apprendre à déployer un Smart Contract Solidity avec Forge sur ton Anvil local, puis interagir avec lui avec Cast.
---------------------
Pour intégrer **Foundry (Anvil) avec MetaMask**, tu dois connecter MetaMask à ton réseau Ethereum local lancé par Anvil.

L'objectif est de pouvoir :

- Voir tes comptes Anvil dans MetaMask.
- Consulter leurs soldes en ETH.
- Envoyer des transactions depuis MetaMask.
- Déployer et interagir avec des Smart Contracts Solidity.
- Observer les transactions directement dans Anvil.

## **1. Démarrer Anvil**

Dans ton terminal Ubuntu :

```
anvil
```

Tu as déjà lancé Anvil. Il écoute sur :

```
RPC URL: http://127.0.0.1:8545
Chain ID: 31337
Network: Anvil
Currency: ETH
```

**Important :** laisse ce terminal ouvert pendant toute la manipulation.

## **2. Ajouter Anvil dans MetaMask**

Ouvre l'extension MetaMask dans Chrome.

[How to Add Scroll to MetaMask | CoinGecko](https://images.openai.com/static-rsc-4/0n0mjN01gxKDwoKaCo42CoVN0hiAJ5q7SgCZ92_pFEziy2o0-Xiol1cHKvDRwPpP6wLy7duh8Y4sc_f-gsf9Zh02dAQ2ynSst4xSFY2YR9601RtVf_UhxMwY9SG7iSgdUzZwOcX3yaZIlKCrOKhWRcNwvVG4DI8y6JHH74irIZs?purpose=inline)

[coingecko.com](https://www.coingecko.com/learn/add-scroll-to-metamask?utm_source=chatgpt.com)

1. Clique sur le sélecteur de réseau en haut.
2. Clique sur « Add a custom network » ou « Ajouter un réseau personnalisé ».
3. Renseigne les informations suivantes.

## **Configuration du réseau Anvil**

Network Name

**Anvil Local**

Default RPC URL

[http://127.0.0.1:8545](http://127.0.0.1:8545?utm_source=chatgpt.com)

Chain ID

**31337**

Currency Symbol

**ETH**

Block Explorer URL

Laisser vide

4. Clique sur Save.
5. Sélectionne le réseau Anvil Local.

## **3. Importer un compte Anvil dans MetaMask**

Ajouter le réseau ne suffit pas. Il faut également importer un compte Anvil pour accéder aux 10 000 ETH de test.

Dans ton terminal Anvil, récupère la clé privée du compte 0 :

```
Address:
0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

Dans MetaMask :

1. Clique sur le sélecteur de compte.
2. Sélectionne « Add account or hardware wallet ».
3. Choisis « Import account ».
4. Colle la clé privée du compte 0 affichée par Anvil.
5. Confirme l'importation.

[How to Load Unlimited Free Ethers to Metamask Wallet - DEV Community](https://images.openai.com/static-rsc-4/yySWxwkW_aG18m8kJWfZtGH2gGhb9HA1DXKYIndZENRgUN5DCtgYXVHL0PDhjTX5UTYzJJX1I7CIBQH4i2dOA_Kgp7XLLKEHSFXb1rfDi5f8cJj4L2DllDuJVBSixmtKUU5D7mwzvxckY3dOa4jP4gDTzV4B6xU1mznE2A5_0tc?purpose=inline)

[dev.to](https://dev.to/daltonic/how-to-load-unlimited-free-ethers-to-metamask-wallet-86k?utm_source=chatgpt.com)

Résultat attendu

Tu dois retrouver :

**Account Anvil 0**

Connecté au réseau local

# **10,000 ETH**

Solde fictif sur Anvil – Chain ID 31337

Adresse

`0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`

Source : [Cyfrin – Deploy Smart Contract Locally Using Anvil](https://updraft.cyfrin.io/courses/foundry/foundry-simple-storage/deploy-smart-contract-locally?lesson_format=video\&utm_source=chatgpt.com).

**Attention :** les clés privées Anvil sont publiques et prévisibles. Ne les utilise jamais pour un portefeuille réel ou sur Mainnet.

## **4. Tester une transaction depuis MetaMask**

Maintenant :

1. Sélectionne le réseau Anvil Local.
2. Sélectionne le compte 0.
3. Clique sur Send.
4. Mets l'adresse du compte 1 :

```
0x70997970C51812dc3A010C7d01b50e0d17dc79C8
```

5. Envoie 1 ETH.
6. Confirme la transaction.

Dans ton terminal Anvil, tu verras les informations de la transaction : hash, gas utilisé, bloc miné, etc.

Tu peux aussi vérifier avec :

```
cast balance 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 \
--ether \
--rpc-url http://127.0.0.1:8545
```

Résultat attendu : `10001.0`.

## **5. Comprendre l'architecture**

**MetaMask**

Wallet – Signature des transactions

**JSON-RPC**

127.0.0.1:8545

**Anvil – Foundry**

Blockchain Ethereum locale

**Smart Contracts Solidity**

Contrats déployés localement

## **6. Quelle est la prochaine étape ?**

Je te conseille de faire un petit TP pratique :

- Écrire un Smart Contract Solidity.
- Le compiler avec `forge build`.
- Le déployer sur Anvil avec `forge script`.
- Ajouter le contrat dans MetaMask si c'est un token ERC-20.
- Interagir avec le contrat depuis MetaMask.

**Que veux-tu apprendre maintenant ?**

Déployer mon premier Smart Contract avec Foundry

Créer un token ERC-20 et l'afficher dans MetaMask

Comprendre les transactions et signatures MetaMask

Connecter MetaMask à une application React/Web3

Commencer le TP



----------------------------------------------------------
**Foundry** est un framework complet pour développer, tester, déployer et analyser des Smart Contracts Ethereum en Solidity.

Tu peux l'utiliser pour faire beaucoup de choses, depuis la création d'un token ERC-20 jusqu'à l'audit de sécurité de contrats intelligents.

Je vais te présenter les possibilités avec des exemples pratiques.

# **1. Les 4 outils principaux de Foundry**

[Heather Robertson: Free Workouts to Paid Fitness App](https://images.openai.com/static-rsc-4/_Gq1wrcHGocahy9lCuj6HIbxM228V-xPBo7IE0j-Jy0NPBAd0ds5-Uh8oaMoc5VtL3q3uOsLsqrBjJtx7YsnpjhKKfFaknfzGF-YcV9EKgDr0l4OmZS-iZmxF81HUQ-QPJSLur0C-EE3I_RH00aSPWJUo5MAyLRD8UuZ7KHeonk?purpose=inline)

**Forge — Développement**

Compiler et tester

- Compiler des contrats Solidity.
- Écrire des tests unitaires.
- Tester les interactions entre contrats.
- Déployer des Smart Contracts.

[Video: Foundry Simple Storage - Deploy a Smart Contract Locally Using Anvil - Foundry Fundamentals](https://images.openai.com/static-rsc-4/CR7bazMaoS5tyWG-HOtPOj-0-Lu-SDTk8TGJ01dNLaWwvZORd9SUlePykrFEYdvFk_JnqKS5vlq960KrZtFDMTDh4PGihiVu37gSr85zfW9o_6BfJh1zmhF6O_S2wCkGMbVddd40cmicsuMUrOSKtcmqGndgFWc25lKRwk2StsA?purpose=inline)

**Anvil — Blockchain locale**

Simulation

- Créer une blockchain locale.
- Simuler des transactions.
- Créer des comptes avec des ETH de test.
- Tester des attaques et des comportements anormaux.

[Raycast Store: Foundry Cast CLI](https://images.openai.com/static-rsc-4/-NhfBSRjSLAJS3ZaGWqdz9wQDcE53ixVY69JVmIdLRyP8y8bbUNsXAPe94-3_Wb3vrXnNH846EtCYrJLtKmyaL1iIadBFEZcbodaVvBWKoJjc3tFtJ90T2k69A9Cmefipmb4FC9nvjwK0viqBnuCuhFkqkWamk7vj-UNiQr0mj4?purpose=inline)

**Cast — Interaction Ethereum**

Communication

- Consulter les soldes.
- Envoyer des transactions.
- Lire les données d'un contrat.
- Interroger un nœud Ethereum via RPC.

[GitHub - foundry-rs/foundry: Foundry is a blazing fast, portable and modular toolkit for Ethereum application development written in Rust. · GitHub](https://images.openai.com/static-rsc-4/MdzMVyhk1jzcJiQ-PDFQzukKNhSaFHy8IWb_8Kxi_A1cBKDjH6XRUTJSEIByFBkzAMXYQpmJRtNhpBdQwjTzUGgwlk5j47olO296glMEuR3BYT4EKvP77FlCOCQ7nkBu5qkCWgFH-ikXNJwB__fCNvLh2B-eeVXAeat6o8S67Lg?purpose=inline)

**Chisel — Solidity interactif**

Expérimentation

- Exécuter du Solidity directement dans le terminal.
- Tester des expressions.
- Comprendre le comportement du langage.

# **2. Les 10 choses que tu peux faire avec Foundry**

## **1. Créer un Smart Contract**

Exemple : créer un contrat qui stocke un message.

```
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract MyContract {

    string public message = "Hello Ethereum";

    function setMessage(string memory newMessage) public {
        message = newMessage;
    }
}
```

## **2. Compiler tes Smart Contracts**

```
forge build
```

Foundry compile tes fichiers Solidity et génère :

- ABI.
- Bytecode.
- Artifacts.
- Informations de compilation.

## **3. Tester automatiquement les contrats**

Tu peux écrire des tests pour vérifier que tes contrats fonctionnent correctement.

```
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/MyContract.sol";

contract MyContractTest is Test {

    MyContract contractInstance;

    function setUp() public {
        contractInstance = new MyContract();
    }

    function testInitialMessage() public {
        assertEq(
            contractInstance.message(),
            "Hello Ethereum"
        );
    }
}
```

Exécuter :

```
forge test -vvv
```

Tu peux aussi tester les cas limites, les erreurs et les tentatives d'exploitation.

## **4. Créer un token ERC-20**

Tu peux développer ton propre token Ethereum.

Exemples :

- Token de paiement.
- Token de gouvernance.
- Token de récompense.
- Token de test.

[Building Upgradeable Smart Contracts with Foundry & OpenZeppelin: An ERC20 Step-by-Step Guide | by Régis | Coinmonks | Medium](https://images.openai.com/static-rsc-4/XlydQmHMtsG-VWfebmsmJ2nHW8Fzp_izDdPcQtM7gwxFF_dXmSkGxsU_OG4IRKGLvLqf5CFEm5QiM0t5xDICK4RVP-v46QUIVw8bdnnDrx8C-sLyzXmX75R1JkIU-0i6kxXYUYtjTUrmJVtvCK-lt464x_uQG_eqlZjtGx06PKI?purpose=inline)

[medium.com](https://medium.com/coinmonks/building-upgradeable-smart-contracts-with-foundry-openzeppelin-an-erc20-step-by-step-guide-045337eb1559?utm_source=chatgpt.com)

Avec Foundry et OpenZeppelin :

```
forge install OpenZeppelin/openzeppelin-contracts
```

Tu peux ensuite créer un contrat ERC-20 et le déployer sur Anvil.

## **5. Déployer des Smart Contracts**

Sur Anvil :

```
anvil
```

Dans un autre terminal :

```
forge create src/MyContract.sol:MyContract \
--rpc-url http://127.0.0.1:8545 \
--private-key YOUR_PRIVATE_KEY
```

Foundry envoie une transaction de déploiement à Anvil.

Tu récupères ensuite l'adresse du contrat.

## **6. Interagir avec Ethereum grâce à Cast**

Consulter un solde :

```
cast balance ADDRESS \
--ether \
--rpc-url http://127.0.0.1:8545
```

Consulter le Chain ID :

```
cast chain-id \
--rpc-url http://127.0.0.1:8545
```

Consulter le dernier bloc :

```
cast block latest \
--rpc-url http://127.0.0.1:8545
```

Lire une fonction publique d'un contrat :

```
cast call CONTRACT_ADDRESS \
"message()(string)" \
--rpc-url http://127.0.0.1:8545
```

## **7. Tester les attaques contre les Smart Contracts**

Très intéressant pour la cybersécurité

Foundry permet de tester des vulnérabilités dans un environnement contrôlé.

[Attacks on Smart Contracts: A Dartmouth Blockchain Dive | by Dartmouth Blockchain | Medium](https://images.openai.com/static-rsc-4/ZVw1QGIqR4LzPNgJ3q0En38JcZhOS-YP-yVr9bAmTY3sySAg-8Fq8wJ61cQRC2on4qDFhdU-sStxwwffyTRvAgEIipvFccPPIUsOXd_PzfqMRRdx2NeZpYchSj7q2BFy6JXoWbOwZ1FohRZcmALifUEL6S4T4SOKKZjRTZvV0NQ?purpose=inline)

[Web3 Security Vulnerabilities 2: Comprehensive Guide to Protecting Digital Assets](https://images.openai.com/static-rsc-4/IXXqscYthsmEXQo7Njhl_IN5nezsTzOEtab3_lGqsU1LTUO9fLVz5FsvOfkGrxwUCgCJeDDLulPxV56AyS4yCKyiGSe9IYSDOrERAq6jQZVoxIwkcRhQmal_2J85c2RwY2L23ZvHP6E37-F6-Qbwjx-MC5AamtX02d4PqEOE9W8?purpose=inline)

[Solidity Security By Example #07: Phishing With Improper Authorization | by Phuwanai Thummavet | Valix Consulting | Medium](https://images.openai.com/static-rsc-4/3k0OluUJTubpMhWa-ZExYOTYMW_As1Lda-tfpWLwkSKmpN-CqR6kNO3cVEQ0Ik3M-aVpcQtNoo1lJ3WVDL7m2OGQGthUVqVwrCCC_6rwsfBKz7nWAYbRvEXjqqpzuTj5utTBBtZuWryyZ_1W2cyECSZnEuw0UL5HlDU46x_V5rY?purpose=inline)

9

Tu peux étudier :

| Vulnérabilité     | Ce que tu peux tester      |
| ----------------- | -------------------------- |
| Reentrancy        | Réentrance dans un contrat |
| Integer Overflow  | Débordement arithmétique   |
| Access Control    | Accès non autorisé         |
| Front-running     | Ordre des transactions     |
| Flash Loan        | Attaques par prêts flash   |
| Denial of Service | Blocage de fonctions       |

Foundry propose notamment :

- Fuzz Testing.
- Invariant Testing.
- Fork Testing.
- Gas Profiling.

## **8. Faire du Fork Testing**

C'est une fonctionnalité très intéressante.

Tu peux créer une copie locale de l'état d'un réseau Ethereum existant.

Par exemple :

```
anvil --fork-url YOUR_ETHEREUM_RPC_URL
```

Tu peux ainsi tester des transactions sur une copie locale de Mainnet sans modifier la vraie blockchain.

## **9. Analyser les transactions et le Gas**

Tu peux mesurer le coût d'exécution de tes fonctions.

```
forge test --gas-report
```

Tu peux également inspecter une transaction :

```
cast receipt TRANSACTION_HASH \
--rpc-url http://127.0.0.1:8545
```

## **10. Automatiser les déploiements**

Avec les scripts Foundry, tu peux automatiser :

- Le déploiement de plusieurs contrats.
- L'initialisation des contrats.
- La configuration des rôles.
- Les interactions entre plusieurs contrats.

# **3. Les commandes Foundry à connaître**

Voici une fiche de référence :

| Commande         | Fonction                       |
| ---------------- | ------------------------------ |
| `forge init`     | Créer un projet                |
| `forge build`    | Compiler                       |
| `forge test`     | Exécuter les tests             |
| `forge create`   | Déployer un contrat            |
| `forge script`   | Exécuter un script             |
| `forge install`  | Installer une dépendance       |
| `forge fmt`      | Formater Solidity              |
| `forge snapshot` | Mesurer le gas des tests       |
| `anvil`          | Démarrer une blockchain locale |
| `cast call`      | Lire un contrat                |
| `cast send`      | Envoyer une transaction        |
| `cast block`     | Consulter un bloc              |
| `cast tx`        | Consulter une transaction      |
| `chisel`         | Solidity interactif            |

# **4. Un projet pratique pour maîtriser Foundry**

Je te propose de réaliser un projet appelé **Ethereum Security Lab**.

[Smart Contract Auditor - Static & Dynamic Analysis Engine (2026) - itch.io](https://images.openai.com/static-rsc-4/hQkLKvIRngx5N3tvh5Eym5mkW4sEL1EcbvwdxKkNNaOzYjl5j75ioqb9oI4o_-EmsL4_ugztRWMp3LG-m0uKbjCoi5HR4YWDUQPacJ7PstH9pwrTHAkdcU41xsSUW7bo7pwkvYThQmD7DnOucN1ZG_dus29chuHU7UDiSUBUlzs?purpose=inline)

[itch.io](https://itch.io/blog/1516189/smart-contract-auditor-static-dynamic-analysis-engine-2026?utm_source=chatgpt.com)

# **Ethereum Security Lab**

Projet pratique

**Objectif :** développer un environnement pour tester, analyser et sécuriser les Smart Contracts Ethereum.

1. **Phase 1 — Blockchain locale**

   Démarrer Anvil, créer les comptes et envoyer des transactions.
2. **Phase 2 — Smart Contracts**

   Créer un ERC-20, un contrat de paiement et un contrat de gestion des utilisateurs.
3. **Phase 3 — Testing**

   Écrire des tests unitaires, fuzzing et invariant testing.
4. **Phase 4 — Security**

   Simuler des vulnérabilités et vérifier les protections.
5. **Phase 5 — MetaMask**

   Connecter MetaMask, déployer et interagir avec les contrats.
6. **Phase 6 — Monitoring**

   Collecter les transactions, les événements, le gas et les erreurs.

# **5. Lien avec EVA-Guard**

Tu peux également utiliser Foundry pour compléter EVA-Guard.

Par exemple :

| Composant  | Utilisation dans EVA-Guard               |
| ---------- | ---------------------------------------- |
| Anvil      | Simuler un réseau local                  |
| Forge      | Créer des scénarios de test              |
| Cast       | Générer des transactions et requêtes RPC |
| Geth       | Surveiller l'Execution Layer             |
| Lighthouse | Surveiller le Consensus Layer            |
| Rust       | Collecter les métriques                  |
| ML         | Détecter les attaques                    |

Attention : Anvil ne remplace pas ton environnement Kurtosis avec Geth et Lighthouse pour les expérimentations portant sur les attaques réseau Ethereum.

# **6. Par quoi commencer ?**

Je te conseille de maîtriser Foundry à travers des travaux pratiques progressifs.

**Ton parcours Foundry**

0/7 terminés

**TP 1 — Installer et initialiser un projet Foundry**

forge init, structure et configuration

**TP 2 — Manipuler Anvil et les comptes**

Comptes, clés privées, transactions et blocs

**TP 3 — Développer un Smart Contract Solidity**

Variables, fonctions, événements et mappings

**TP 4 — Écrire des tests avec Forge**

Tests unitaires, assertions et erreurs

**TP 5 — Déployer et connecter MetaMask**

Déploiement et interaction avec le contrat

**TP 6 — Tester les vulnérabilités**

Reentrancy, fuzzing et invariant testing

**TP 7 — Fork Ethereum et analyser le gas**

Fork testing et optimisation

Commencer le TP 1

Mon conseil : ne te limite pas à apprendre les commandes. Apprends surtout comment une transaction est signée, propagée, exécutée par l'EVM, puis intégrée dans un bloc validé par le consensus Ethereum. C'est ce qui te permettra de vraiment maîtriser Foundry et l'architecture Ethereum.



https://chatgpt.com/c/6b07fd8f-6438-83ea-bf9a-6c9a543756ca