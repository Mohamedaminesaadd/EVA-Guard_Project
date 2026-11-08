Dans Geth (Ethereum), `admin` est un namespace JSON-RPC qui fournit des méthodes d'administration et de diagnostic du nœud Ethereum.

Il permet notamment d'obtenir des informations sur les connexions P2P et de gérer certains aspects du nœud.

## 1. À quoi sert `admin` ?

Quelques méthodes importantes :

| Méthode            | Rôle                                                            |
| ------------------ | --------------------------------------------------------------- |
| `admin_peers`      | Retourne la liste des nœuds Ethereum auxquels Geth est connecté |
| `admin_nodeInfo`   | Donne des informations sur le nœud local                        |
| `admin_addPeer`    | Ajoute manuellement un pair                                     |
| `admin_removePeer` | Supprime un pair                                                |

Par exemple, `admin_peers` retourne des informations comme :

- L'identifiant du peer (`id`).
- Son adresse réseau.
- Son état de connexion.
- Les informations sur les protocoles utilisés.

## 2. Différence entre `net_peerCount` et `admin_peers`

Dans notre projet EVA-Guard, cette distinction est importante.

`net_peerCount`

Retourne uniquement le nombre de peers connectés.

Exemple :

```
{
  "result": "0x3"
}
```

Cela signifie que Geth est connecté à 3 peers.

`admin_peers`

Retourne les détails de chaque peer.

Exemple simplifié :

```
{
  "result": [
    {
      "id": "peer-id-123",
      "name": "Geth",
      "network": {
        "remoteAddress": "192.168.1.10:30303"
      }
    }
  ]
}
```

## 3. Pourquoi avons-nous besoin de `admin` dans EVA-Guard ?

Notre `P2pCollector` collecte plusieurs métriques :

| Métrique             | Source                                         |
| -------------------- | ---------------------------------------------- |
| `peer_count`         | `net_peerCount`                                |
| `network_listening`  | `net_listening`                                |
| `admin_peer_count`   | `admin_peers`                                  |
| `new_peers`          | Comparaison entre deux appels `admin_peers`    |
| `disconnected_peers` | Comparaison entre deux appels `admin_peers`    |
| `peer_churn_count`   | Nombre de connexions et déconnexions détectées |

Sans `admin_peers`, nous pouvons toujours compter les peers, mais nous ne pouvons pas identifier précisément lesquels se sont connectés ou déconnectés.

## 4. Pourquoi `admin_peers` ne fonctionne pas chez toi ?

Tu as obtenu :

```
{
  "error": {
    "code": -32601,
    "message": "the method admin_peers does not exist/is not available"
  }
}
```

Cela signifie que cette méthode n'est pas accessible via ton endpoint HTTP actuel.

Attention : `admin` est une API sensible. Il faut éviter de l'exposer publiquement. Pour notre laboratoire, nous pouvons la garder accessible uniquement sur `127.0.0.1`.

La prochaine vérification : nous allons tester si l'API `admin` est disponible dans la console Geth avec :

```
geth attach http://127.0.0.1:8545
```

Puis :

```
admin.peers
```

Envoie-moi le résultat. Nous saurons ainsi si le problème vient de l'activation de l'API ou de son exposition via HTTP.