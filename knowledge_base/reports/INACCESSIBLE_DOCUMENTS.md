# Documents non recuperes localement

Genere le 2026-11-24. Source : `metadata/documents_manifest.json`.

15 entrees sur 133 n'ont pas de texte integral dans ce depot. Aucune n'a ete remplacee par une publication differente : l'entree est conservee avec ses metadonnees verifiees et son statut d'acces.

## Cause

L'environnement de construction n'autorise la sortie reseau que vers GitHub et les registres de paquets. arxiv.org, eprint.iacr.org, ndss-symposium.org, doi.org, faculty.cc.gatech.edu et les sites editeurs ont renvoye une absence de reponse (code 000) : c'est une restriction locale, **pas** un constat d'URL morte.

## Entrees concernees

| document_id                     | categorie | annee | acces                   | DOI                        | URL de recuperation                                                    |
|---------------------------------|-----------|-------|-------------------------|----------------------------|------------------------------------------------------------------------|
| blockchain_amplification_2025   | attacks   | 2025  | open_access             | -                          | https://arxiv.org/pdf/2408.01508                                       |
| doers_ndss_2021                 | attacks   | 2021  | open_access             | -                          | https://www.ndss-symposium.org/wp-content/uploads/2021-108-paper.pdf   |
| eclipse_detect_jnca_2026        | attacks   | 2026  | open_access_unconfirmed | 10.1016/j.jnca.2025.104416 | https://doi.org/10.1016/j.jnca.2025.104416                             |
| eclipse_eth_2018                | attacks   | 2018  | open_access             | -                          | https://eprint.iacr.org/2018/236.pdf                                   |
| eclipse_post_merge_www_2026     | attacks   | 2026  | open_access             | -                          | https://arxiv.org/pdf/2601.16560                                       |
| eth_eds_cose_2020               | attacks   | 2020  | publisher_paywall       | 10.1016/j.cose.2019.101604 | https://doi.org/10.1016/j.cose.2019.101604                             |
| false_friends_2019              | attacks   | 2019  | open_access             | -                          | https://arxiv.org/pdf/1908.10141                                       |
| rpc_exposure_2019               | attacks   | 2019  | open_access             | -                          | https://arxiv.org/pdf/1904.01981                                       |
| beacon_clients_review_2021      | research  | 2021  | open_access             | -                          | https://arxiv.org/pdf/2109.11677                                       |
| deanon_validators_2025          | research  | 2024  | open_access             | -                          | https://arxiv.org/pdf/2409.04366                                       |
| egn_measure_attack_2025         | research  | 2025  | open_access             | -                          | https://eprint.iacr.org/2025/588.pdf                                   |
| eth_security_survey_2020        | research  | 2020  | open_access             | 10.1145/3391195            | https://arxiv.org/pdf/1908.04507                                       |
| gossipsub_2020                  | research  | 2020  | open_access             | -                          | https://arxiv.org/pdf/2007.02754                                       |
| measuring_eth_peers_imc_2018    | research  | 2018  | open_access             | -                          | https://faculty.cc.gatech.edu/~mbailey/publications/imc18_ethereum.pdf |
| p2p_chain_client_diversity_2025 | research  | 2025  | open_access             | -                          | https://arxiv.org/pdf/2501.16236                                       |

## Marche a suivre

```bash
bash scripts/download_papers.sh          # acces libre uniquement
python ingestion/verify_corpus.py --update   # bascule en 'downloaded' + sha256
python ingestion/indexer.py --build --reset  # reindexation
```

## Acces non automatisable

### eclipse_detect_jnca_2026

- Titre : A robust eclipse attack detection framework for Ethereum networks
- Acces : open_access_unconfirmed
- DOI : 10.1016/j.jnca.2025.104416
- Licence / conditions : a verifier sur la page editeur (indique Open Access par certains index)
- Depot ou page editeur : https://www.sciencedirect.com/science/article/pii/S1084804525003133
- Deposer le PDF ici : `documents/attacks/eclipse_detect_jnca_2026.pdf`

### eth_eds_cose_2020

- Titre : Am I eclipsed? A smart detector of eclipse attacks for Ethereum
- Acces : publisher_paywall
- DOI : 10.1016/j.cose.2019.101604
- Licence / conditions : Elsevier (texte final) ; manuscrit accepte sur NTNU Open sous CC-BY-NC-ND 4.0
- Depot ou page editeur : https://ntnuopen.ntnu.no/ntnu-xmlui/handle/11250/2620806
- Deposer le PDF ici : `documents/attacks/eth_eds_cose_2020.pdf`

Le texte integral sous licence restrictive ne doit pas etre redistribue : les PDF restent locaux et `vectorstore/` comme `documents/attacks/*.pdf` sont ignores par git (voir `.gitignore`).

