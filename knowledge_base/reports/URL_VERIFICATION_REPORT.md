# Rapport de verification des sources

Genere le 2026-11-24.

## 1. Methode

Deux mecanismes de verification distincts ont ete employes.

**Documentation officielle et specifications** : recuperee par `git clone` du depot amont qui alimente le site publie. La verification porte donc sur le contenu lui-meme (fichier present au commit indique, empreinte sha256 enregistree au manifeste), ce qui est plus fort qu'un simple code HTTP 200 sur une URL.

**Publications** : chaque champ bibliographique (titre exact, liste complete des auteurs, annee, revue ou conference, volume, numero d'article, DOI) a ete controle sur la page editeur, la page de listing arXiv, l'archive ePrint, dblp ou un depot institutionnel. Aucun auteur, aucune date et aucun resultat n'a ete deduit ou complete de memoire.

## 2. Limite de l'environnement de construction

La sortie reseau de l'environnement n'autorise que GitHub et les registres de paquets. Les URLs hors GitHub n'ont donc **pas** pu etre testees en HTTP : elles sont marquees `non_testee`, et non `cassee`. Mesures effectivement relevees :

| URL ou operation                                                                                       | Code | Resultat           |
|--------------------------------------------------------------------------------------------------------|------|--------------------|
| https://raw.githubusercontent.com/ethereum/devp2p/master/README.md                                     | 200  | accessible         |
| git clone github.com/ethereum/{devp2p,go-ethereum,ethereum-org-website,consensus-specs,execution-apis} | OK   | accessible         |
| git clone github.com/{sigp/lighthouse,ethereum/beacon-APIs}                                            | OK   | accessible         |
| https://ethereum.org/developers/docs/networking-layer/                                                 | 000  | bloquee localement |
| https://geth.ethereum.org/docs/fundamentals                                                            | 000  | bloquee localement |
| https://arxiv.org/abs/2601.16560                                                                       | 000  | bloquee localement |
| https://cs-web.bu.edu/~goldbe/projects/eclipseEth.pdf                                                  | 000  | bloquee localement |
| https://doi.org/10.1016/j.cose.2019.101604                                                             | 000  | bloquee localement |

Pour completer la verification depuis votre machine :

```bash
python ingestion/verify_corpus.py --check-urls
```

Le script teste chaque `source_url` et `pdf_url` (HEAD puis GET), classe le resultat en `ok` / `paywall_or_forbidden` / `unreachable` / `error` et ecrit `reports/verification_report.json`.

## 3. Ecarts releves sur les URLs fournies dans la demande

- `https://cs-web.bu.edu/~goldbe/projects/eclipseEth.pdf` : la copie auteur retrouvee est servie par l'hote `www.cs.bu.edu` (meme chemin). Comme URL canonique du manifeste, `https://eprint.iacr.org/2018/236` a ete retenue : c'est la reference de publication citee par les auteurs eux-memes (IACR ePrint 2018/236). Les deux copies restent listees en miroirs.
- `https://doi.org/10.1016/j.cose.2019.101604` : DOI correct. Article Elsevier, *Computers & Security*, vol. 88, art. 101604, publie en janvier 2020 (en ligne le 3 septembre 2019). Sous paywall ; manuscrit accepte disponible en depot institutionnel NTNU sous CC-BY-NC-ND 4.0.
- `https://arxiv.org/abs/2601.16560` : identifiant valide et correspondant bien au titre annonce. Precision ajoutee au manifeste : article accepte a *The ACM Web Conference (WWW) 2026*, soumission du 23 janvier 2026, 7 auteurs (BUPT et UNSW Sydney).
- `https://doi.org/10.1016/j.jnca.2025.104416` : DOI correct malgre le segment `2025`. Publication dans *Journal of Network and Computer Applications*, vol. 246, art. 104416, fevrier 2026 (mise en ligne decembre 2025) ; cette double datation est conservee telle quelle dans le champ `venue`.
- `https://geth.ethereum.org/docs/fundamentals` et `https://geth.ethereum.org/docs/interacting-with-geth/rpc` : URLs conservees comme `source_url`, mais le contenu provient du depot `ethereum/geth-website`, dont la branche par defaut n'a plus de commit depuis le 16 janvier 2023. Ecart de fraicheur signale au rapport de corpus (section 8).
- `https://ethereum.org/developers/docs/` : les URLs du manifeste utilisent la forme localisee effectivement servie (`https://ethereum.org/en/developers/docs/...`).

## 4. Etat par document

| document_id                      | categorie  | Methode de verification                        | Verdict                                |
|----------------------------------|------------|------------------------------------------------|----------------------------------------|
| blockchain_amplification_2025    | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| doers_ndss_2021                  | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| eclipse_detect_jnca_2026         | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| eclipse_eth_2018                 | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| eclipse_post_merge_www_2026      | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| eth_eds_cose_2020                | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| false_friends_2019               | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| rpc_exposure_2019                | attacks    | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| devp2p_caps_eth                  | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_caps_les                  | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_caps_snap                 | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_caps_wit                  | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_discv4                    | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_discv5                    | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_discv5_rationale          | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_discv5_theory             | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_discv5_wire               | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_dnsdisc                   | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_enr                       | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_enr_entries_eth           | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_readme                    | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| devp2p_rlpx                      | devp2p     | clonage ethereum/devp2p@277ceb39               | contenu verifie                        |
| beacon_api_eventstream           | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_health           | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_identity         | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_peer             | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_peer_count       | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_peers            | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_syncing          | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_node_version          | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_readme                | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_validator_flow        | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| beacon_api_validator_liveness    | ethereum   | clonage ethereum/beacon-APIs@76f2ca67          | contenu verifie                        |
| cl_beacon_chain_phase0           | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_altair                    | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_bellatrix                 | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_capella                   | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_deneb                     | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_electra                   | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_fulu                      | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_p2p_phase0                    | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| cl_validator_phase0              | ethereum   | clonage ethereum/consensus-specs@d2021d27      | contenu verifie                        |
| ea_debug_getters                 | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_authentication         | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_cancun                 | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_common                 | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_paris                  | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_prague                 | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_engine_readme                 | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_block                     | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_client                    | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_fee_market                | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_filter                    | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_state                     | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_eth_submit                    | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| ea_txpool_pool                   | ethereum   | clonage ethereum/execution-apis@1ca08185       | contenu verifie                        |
| eth_bootnodes                    | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_client_diversity             | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_consensus_mechanisms         | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_ethereum_stack               | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_intro_to_ethereum            | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_json_rpc_api                 | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_light_clients                | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_network_addresses            | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_networking_layer             | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_node_architecture            | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_nodes_and_clients            | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_portal_network               | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_pos                          | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_pos_attack_defense           | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_pos_rewards_penalties        | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| eth_run_a_node                   | ethereum   | clonage ethereum/ethereum-org-website@fef1cdf6 | contenu verifie                        |
| geth_command_line_options        | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_consensus_clients           | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_faq                         | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_fundamentals_index          | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_hardware_requirements       | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_les                         | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_logs                        | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_monitoring_dashboards       | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_monitoring_ethstats         | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_monitoring_metrics          | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_node_architecture           | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_peer_to_peer                | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_private_network             | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_pruning                     | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_batch                   | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_graphql                 | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_index                   | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_admin                | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_clique               | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_debug                | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_eth                  | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_les                  | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_miner                | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_net                  | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_personal             | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_personal_deprecation | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_ns_txpool               | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_rpc_pubsub                  | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_security                    | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_sync_modes                  | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| geth_tools_devp2p                | geth       | clonage ethereum/geth-website@e74924e5         | contenu verifie                        |
| lh_advanced_database             | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_advanced_networking           | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_advanced_redundancy           | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_advanced_slasher              | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api                           | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_bn                        | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_lighthouse                | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_metrics                   | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_validator_inclusion       | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_vc                        | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_vc_auth_header            | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_api_vc_endpoints              | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_developers_architecture       | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_faq                           | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_help_bn                       | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_help_vc                       | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_intro                         | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_run_a_node                    | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_security                      | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_validator_doppelganger        | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_validator_management          | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_validator_monitoring          | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| lh_validator_slashing_protection | lighthouse | clonage sigp/lighthouse@57c2abc1               | contenu verifie                        |
| beacon_clients_review_2021       | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| deanon_validators_2025           | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| egn_measure_attack_2025          | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| eth_security_survey_2020         | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| gossipsub_2020                   | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| measuring_eth_peers_imc_2018     | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |
| p2p_chain_client_diversity_2025  | research   | metadonnees bibliographiques                   | metadonnees verifiees / URL non testee |

## 5. Detail des verifications bibliographiques

### beacon_clients_review_2021

- Titre retenu : Security Review of Ethereum Beacon Clients
- Auteurs retenus : Jean-Philippe Aumasson, Denis Kolegov, Evangelia Stathopoulou
- Publication retenue : preprint arXiv 2109.11677 (23 sept. 2021)
- Verifie via : Page de listing arXiv 2109.11677 ; page de garde du PDF (3 auteurs et affiliations) ; support de presentation de l'auteur principal
- URL canonique : https://arxiv.org/abs/2109.11677

### blockchain_amplification_2025

- Titre retenu : Blockchain Amplification Attack
- Auteurs retenus : Taro Tsuchiya, Liyi Zhou, Kaihua Qin, Arthur Gervais, Nicolas Christin
- Publication retenue : ACM SIGMETRICS 2025 ; preprint arXiv 2408.01508 (v1 aout 2024, v3 fevrier 2025)
- Verifie via : Page de listing arXiv 2408.01508 (5 auteurs, v1 aout 2024, v3 fevrier 2025) ; copie auteur CMU indiquant SIGMETRICS 2025
- URL canonique : https://arxiv.org/abs/2408.01508

### deanon_validators_2025

- Titre retenu : Deanonymizing Ethereum Validators: The P2P Network Has a Privacy Issue
- Auteurs retenus : Lioba Heimbach, Yann Vonlanthen, Juan Villacis, Lucianna Kiffer, Roger Wattenhofer
- Publication retenue : preprint arXiv 2409.04366
- Verifie via : PDF arXiv 2409.04366 ; liste d'auteurs confirmee par la bibliographie de arXiv 2601.16560
- URL canonique : https://arxiv.org/abs/2409.04366

### doers_ndss_2021

- Titre retenu : As Strong As Its Weakest Link: How to Break Blockchain DApps at RPC Service
- Auteurs retenus : Kai Li, Jiaqi Chen, Xianghong Liu, Yuzhe Tang, XiaoFeng Wang, Xiapu Luo
- Publication retenue : 28th Network and Distributed System Security Symposium (NDSS) 2021
- Verifie via : Page de l'article sur ndss-symposium.org (auteurs et affiliations) ; notice institutionnelle Stevens ; PDF des actes NDSS 2021
- URL canonique : https://www.ndss-symposium.org/ndss-paper/as-strong-as-its-weakest-link-how-to-break-blockchain-dapps-at-rpc-service/

### eclipse_detect_jnca_2026

- Titre retenu : A robust eclipse attack detection framework for Ethereum networks
- Auteurs retenus : Zubaida Rehman, Iqbal Gondal, Hai Dong, Mengmeng Ge, Mark A. Gregory, Ikram Ul Haq
- Publication retenue : Journal of Network and Computer Applications, vol. 246, art. 104416 (fevrier 2026)
- Verifie via : Page editeur ScienceDirect (volume 246, article 104416, fevrier 2026) ; notices dblp, ACM profil auteur et sciprofiles (6 auteurs concordants)
- URL canonique : https://doi.org/10.1016/j.jnca.2025.104416

### eclipse_eth_2018

- Titre retenu : Low-Resource Eclipse Attacks on Ethereum's Peer-to-Peer Network
- Auteurs retenus : Yuval Marcus, Ethan Heilman, Sharon Goldberg
- Publication retenue : Cryptology ePrint Archive, Report 2018/236
- Verifie via : Depot institutionnel Boston University (open.bu.edu, notice complete et citation IACR ePrint 2018/236) ; copie auteur cs.bu.edu ; page projet de l'auteure
- URL canonique : https://eprint.iacr.org/2018/236

### eclipse_post_merge_www_2026

- Titre retenu : Eclipse Attacks on Ethereum's Peer-to-Peer Network
- Auteurs retenus : Ruisheng Shi, Yuxuan Liang, Zijun Guo, Qin Wang, Lina Lan, Chenfeng Wang, Zhuoyi Zheng
- Publication retenue : The ACM Web Conference (WWW) 2026 -- accepte ; preprint arXiv 2601.16560 (23 janv. 2026)
- Verifie via : Page de listing arXiv 2601.16560 (titre, 7 auteurs, soumission 23 janv. 2026) ; en-tete du preprint mentionnant l'acceptation a WWW 2026
- URL canonique : https://arxiv.org/abs/2601.16560

### egn_measure_attack_2025

- Titre retenu : A Place for Everyone vs Everyone in its Place: Measuring and Attacking the Ethereum Global Network
- Auteurs retenus : Chenyu Li, Ren Zhang, Xiaorui Gong
- Publication retenue : Cryptology ePrint Archive, Paper 2025/588 (avril 2025)
- Verifie via : Page Cryptology ePrint Archive 2025/588 (licence CC BY, dates de soumission) ; auteurs confirmes par la bibliographie de arXiv 2601.16560
- URL canonique : https://eprint.iacr.org/2025/588

### eth_eds_cose_2020

- Titre retenu : Am I eclipsed? A smart detector of eclipse attacks for Ethereum
- Auteurs retenus : Guangquan Xu, Bingjiang Guo, Chunhua Su, Xi Zheng, Kaitai Liang, Duncan S. Wong, Hao Wang
- Publication retenue : Computers & Security, vol. 88, art. 101604 (en ligne 3 sept. 2019)
- Verifie via : Notices editeur et universitaires (TU Delft, Macquarie, Surrey) ; page de depot NTNU Open avec DOI, volume, licence du manuscrit accepte
- URL canonique : https://doi.org/10.1016/j.cose.2019.101604

### eth_security_survey_2020

- Titre retenu : A Survey on Ethereum Systems Security: Vulnerabilities, Attacks, and Defenses
- Auteurs retenus : Huashan Chen, Marcus Pendleton, Laurent Njilla, Shouhuai Xu
- Publication retenue : ACM Computing Surveys, vol. 53(3), art. 67 (juin 2020) ; preprint arXiv 1908.04507
- Verifie via : Copie auteur UTSA avec reference ACM complete (CSUR 53(3), art. 67, juin 2020, DOI 10.1145/3391195) ; notice ADS pour le preprint arXiv
- URL canonique : https://doi.org/10.1145/3391195

### false_friends_2019

- Titre retenu : Eclipsing Ethereum Peers with False Friends
- Auteurs retenus : Sebastian Henningsen, Daniel Teunis, Martin Florian, Bjoern Scheuermann
- Publication retenue : IEEE Security & Privacy on the Blockchain (IEEE S&B) 2019 / EuroS&PW 2019
- Verifie via : Page de listing arXiv 1908.10141 (titre, auteurs, date de soumission, mention EuroS&PW) ; notices dblp et HU Berlin / TU Darmstadt
- URL canonique : https://arxiv.org/abs/1908.10141

### gossipsub_2020

- Titre retenu : GossipSub: Attack-Resilient Message Propagation in the Filecoin and ETH2.0 Networks
- Auteurs retenus : Dimitris Vyzovitis, Yusef Napora, Dirk McCormick, David Dias, Yiannis Psaras
- Publication retenue : Protocol Labs TechRep PL-TechRep-2020-002 ; preprint arXiv 2007.02754 (6 juil. 2020)
- Verifie via : Page de listing arXiv 2007.02754 (5 auteurs, 6 juil. 2020) ; reference Protocol Labs PL-TechRep-2020-002 sur le PDF
- URL canonique : https://arxiv.org/abs/2007.02754

### measuring_eth_peers_imc_2018

- Titre retenu : Measuring Ethereum Network Peers
- Auteurs retenus : Seoung Kyun Kim, Zane Ma, Siddharth Murali, Joshua Mason, Andrew Miller, Michael Bailey
- Publication retenue : Internet Measurement Conference (IMC) 2018, p. 91-104
- Verifie via : Copie auteur Georgia Tech (titre, 6 auteurs, reference ACM IMC 2018) ; pagination 91-104 confirmee par la bibliographie d'un article ulterieur
- URL canonique : https://faculty.cc.gatech.edu/~mbailey/publications/imc18_ethereum.pdf

### p2p_chain_client_diversity_2025

- Titre retenu : Unveiling Ethereum's P2P Network: The Role of Chain and Client Diversity
- Auteurs retenus : Jiahao Luo
- Publication retenue : preprint arXiv 2501.16236 (27 janv. 2025)
- Verifie via : Page de listing arXiv 2501.16236 (auteur unique, 27 janv. 2025) confirmee par deux index secondaires
- URL canonique : https://arxiv.org/abs/2501.16236

### rpc_exposure_2019

- Titre retenu : Towards a First Step to Understand the Cryptocurrency Stealing Attack on Ethereum
- Auteurs retenus : Zhen Cheng, Xinrui Hou, Runhuai Li, Yajin Zhou, Xiapu Luo, Jinku Li, Kui Ren
- Publication retenue : preprint arXiv 1904.01981 (v1 avril 2019, v2 juillet 2019)
- Verifie via : Page de listing arXiv 1904.01981 (7 auteurs, v1 avril 2019, v2 juillet 2019)
- URL canonique : https://arxiv.org/abs/1904.01981

