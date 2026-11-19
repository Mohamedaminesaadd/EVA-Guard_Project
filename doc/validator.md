# **Étude de&#x20;**`validator_client.rs`**&#x20;— EVA-Guard**

On passe maintenant au troisième client de ton architecture : `ValidatorClient`.

Après avoir étudié `GethClient` et `BeaconClient`, ce client a un rôle différent : il récupère les métriques Prometheus exposées par le service Validator d'un nœud Ethereum et les transforme en données exploitables par EVA-Guard.

## **1. Quel est le rôle de ce fichier ?**

Ethereum utilise des validateurs pour participer au consensus : ils peuvent notamment proposer des blocs et participer aux attestations.

Le service Validator expose des métriques qui permettent d'observer son fonctionnement.

Par exemple :

- Le nombre d'attestations effectuées.
- Le nombre de tâches asynchrones actives.
- Certaines métriques liées aux services internes du validateur.

Ton `ValidatorClient` permet de :

1. Se connecter à l'endpoint Prometheus du Validator.
2. Récupérer les métriques sous forme de texte.
3. Analyser ce texte.
4. Transformer chaque métrique en structure Rust.
5. Récupérer uniquement les métriques qui nous intéressent.

**Point important :** ce client ne détecte pas directement les attaques. Il collecte et structure les données qui seront ensuite exploitées par les collecteurs et le moteur de monitoring.

## **2. Où se situe-t-il dans EVA-Guard ?**

**Ethereum Validator**

Expose des métriques Prometheus

**Prometheus HTTP Endpoint**

Exemple : /metrics

**ValidatorClient**

HTTP GET + récupération des métriques + parsing

**Validator Collectors**

Sélection et interprétation des métriques utiles

**Observations → Monitoring**

Analyse des anomalies et détection

## **3. Les imports : les outils utilisés**

```
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
```

| Import             | Rôle                                                     |
| ------------------ | -------------------------------------------------------- |
| `reqwest::Client`  | Envoyer des requêtes HTTP vers le serveur Prometheus.    |
| `HashMap`          | Stocker les labels d'une métrique sous forme clé-valeur. |
| `Duration`         | Définir le délai maximal d'une requête HTTP.             |
| `thiserror::Error` | Définir facilement les erreurs personnalisées.           |

Pourquoi `HashMap` ?

Parce qu'une métrique Prometheus peut avoir plusieurs labels.

Par exemple :

```
async_tasks_count{async_task_count="attestation_service"} 1
```

On peut représenter les labels comme ceci :

```
async_task_count → attestation_service
```

## **4. Les structures de données**

### **4.1&#x20;**`ValidatorError`

```
#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid Prometheus metrics format: {0}")]
    InvalidMetrics(String),
}
```

Cette énumération représente les erreurs possibles dans le client.

Elle contient deux variantes :

**A.&#x20;**`Http`

```
Http(#[from] reqwest::Error)
```

Elle représente les erreurs de communication HTTP :

- Le serveur Validator est inaccessible.
- L'URL est incorrecte.
- La connexion est interrompue.
- Le délai maximal est dépassé.

Le `#[from]` permet de convertir automatiquement une erreur `reqwest::Error` en `ValidatorError`.

**B.&#x20;**`InvalidMetrics`

```
InvalidMetrics(String)
```

Elle représente une erreur pendant l'analyse du texte Prometheus.

Par exemple :

```
validator_attestations_total abc
```

La valeur `abc` ne peut pas être convertie en nombre `f64`.

Le parser renvoie donc une erreur `InvalidMetrics`.

### **4.2&#x20;**`PrometheusMetric`

```
#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
}
```

C'est la structure centrale de ce fichier.

Elle représente une métrique Prometheus après son analyse.

Elle contient trois informations :

| Attribut | Type                      | Signification                    |
| -------- | ------------------------- | -------------------------------- |
| `name`   | `String`                  | Nom de la métrique.              |
| `labels` | `HashMap<String, String>` | Labels associés à la métrique.   |
| `value`  | `f64`                     | Valeur numérique de la métrique. |

Exemple de métrique :

```
async_tasks_count{async_task_count="attestation_service"} 1
```

Après parsing, on obtient conceptuellement :

```
name   = "async_tasks_count"
labels = {"async_task_count": "attestation_service"}
value  = 1.0
```

Pourquoi utiliser `f64` ?

Parce que les métriques Prometheus peuvent être des nombres entiers ou décimaux.

Par exemple :

```
validator_attestations_total 42
validator_cpu_usage 23.75
```

Les deux peuvent être stockés dans un `f64`.

Pourquoi `Debug` et `Clone` ?

- `Debug` : permet d'afficher facilement une métrique pendant le développement.
- `Clone` : permet de copier une métrique lorsque nécessaire.

### **4.3&#x20;**`ValidatorClient`

```
#[derive(Debug, Clone)]
pub struct ValidatorClient {
    client: Client,
    metrics_url: String,
}
```

Cette structure représente le client HTTP qui communique avec l'endpoint Prometheus.

Elle possède deux attributs :

- `client` : le client HTTP `reqwest` qui réalise les requêtes.
- `metrics_url` : l'adresse de l'endpoint Prometheus.

Exemple :

```
http://127.0.0.1:5064/metrics
```

Attention : cette adresse est un exemple. Le port réel dépend de la configuration de ton service Validator.

Les champs sont privés : les autres modules ne peuvent pas les modifier directement. Ils utilisent les méthodes publiques du client.

## **5. Explication de l'implémentation**

### **5.1 Constructeur&#x20;**`new()`

```
pub fn new(
    metrics_url: impl Into<String>,
    request_timeout_secs: u64,
) -> Result<Self, ValidatorError>
```

Cette fonction construit une instance de `ValidatorClient`.

Ses paramètres :

- `metrics_url` : adresse du serveur Prometheus.
- `request_timeout_secs` : durée maximale d'une requête, en secondes.

Pourquoi `impl Into<String>` ?

Cela permet de passer plusieurs types de valeurs convertibles en `String`, par exemple :

```
" http://localhost:5064/metrics "
```

ou une `String` déjà construite.

Le type `u64` représente ici le nombre de secondes.

Regardons son contenu :

```
let client = Client::builder()
    .timeout(Duration::from_secs(request_timeout_secs))
    .build()?;
```

Étapes :

1. `Client::builder()` prépare la configuration du client HTTP.
2. `.timeout(...)` configure le délai maximal.
3. `Duration::from_secs(...)` transforme le nombre de secondes en durée Rust.
4. `.build()` construit le client HTTP.
5. `?` propage l'erreur si la construction échoue.

Ensuite :

```
Ok(Self {
    client,
    metrics_url: metrics_url.into(),
})
```

On crée et retourne l'objet `ValidatorClient`.

`Self` désigne ici le type `ValidatorClient`.

`Ok(...)` signifie que la construction s'est terminée correctement.

**Exemple d'utilisation :**

```
let validator = ValidatorClient::new(
    "http://127.0.0.1:5064/metrics",
    3,
)?;
```

Cela crée un client configuré avec un timeout de trois secondes.

### **5.2&#x20;**`get_metrics()`

```
pub async fn get_metrics(&self) -> Result<String, ValidatorError>
```

Son objectif : récupérer le contenu brut de l'endpoint Prometheus.

Regardons les instructions :

```
let response = self
    .client
    .get(&self.metrics_url)
    .send()
    .await?
    .error_for_status()?;
```

- `self.client` : utilise le client HTTP.
- `.get(...)` : prépare une requête HTTP GET.
- `.send()` : envoie la requête.
- `.await` : attend la réponse, car l'opération est asynchrone.
- `?` : propage une éventuelle erreur réseau.
- `.error_for_status()` : transforme les réponses HTTP d'erreur (4xx et 5xx) en erreurs.

Puis :

```
Ok(response.text().await?)
```

Cette instruction récupère le corps de la réponse sous forme de texte.

Par exemple, le serveur pourrait retourner :

```
# HELP validator_attestations_total Total attestations
# TYPE validator_attestations_total counter
validator_attestations_total 42

async_tasks_count{async_task_count="attestation_service"} 1
```

La fonction retourne ce texte sans encore l'interpréter.

### **5.3&#x20;**`collect_metrics()`

```
pub async fn collect_metrics(
    &self,
) -> Result<Vec<PrometheusMetric>, ValidatorError> {
    let raw_metrics = self.get_metrics().await?;

    parse_prometheus_metrics(&raw_metrics)
}
```

Cette fonction combine les deux opérations :

1. Récupérer les métriques brutes.
2. Les analyser pour produire des structures Rust.

Pourquoi `Vec<PrometheusMetric>` ?

Parce que l'endpoint peut retourner plusieurs métriques.

Exemple :

```
validator_attestations_total 42
validator_proposals_total 5
async_tasks_count{async_task_count="attestation_service"} 1
```

Le résultat sera un vecteur contenant trois objets `PrometheusMetric`.

**Différence importante :**

- `get_metrics()` retourne du texte brut (`String`).
- `collect_metrics()` retourne des métriques structurées (`Vec<PrometheusMetric>`).

### **5.4&#x20;**`get_metric()`

```
pub async fn get_metric(
    &self,
    metric_name: &str,
) -> Result<Vec<PrometheusMetric>, ValidatorError>
```

Son rôle est de récupérer uniquement les métriques correspondant à un nom précis.

D'abord :

```
let metrics = self.collect_metrics().await?;
```

On récupère toutes les métriques.

Ensuite :

```
Ok(metrics
    .into_iter()
    .filter(|metric| metric.name == metric_name)
    .collect())
```

Explication :

- `into_iter()` : transforme le vecteur en itérateur consommant ses éléments.
- `filter(...)` : conserve uniquement les métriques dont le nom correspond.
- `metric.name == metric_name` : compare le nom de chaque métrique avec le nom recherché.
- `collect()` : rassemble les résultats dans un nouveau vecteur.

Exemple :

```
let metrics = validator
    .get_metric("validator_attestations_total")
    .await?;
```

Cette fonction retourne uniquement les métriques nommées `validator_attestations_total`.

Attention : elle télécharge et analyse toutes les métriques à chaque appel, puis filtre localement. Elle ne demande pas au serveur Prometheus de filtrer.

### **5.5&#x20;**`metrics_url()`

```
pub fn metrics_url(&self) -> &str {
    &self.metrics_url
}
```

C'est simplement un getter.

Il permet aux autres modules de lire l'URL configurée sans modifier sa valeur.

Le `&str` est une référence vers une chaîne de caractères : aucune nouvelle `String` n'est créée.

## **6. Le parser Prometheus : la partie la plus importante**

Maintenant, nous arrivons à la partie centrale de ton fichier.

Le serveur Prometheus retourne du texte. Ton programme doit comprendre ce texte et le transformer en objets Rust.

C'est exactement le rôle de :

```
pub fn parse_prometheus_metrics(
    input: &str,
) -> Result<Vec<PrometheusMetric>, ValidatorError>
```

### **6.1 Initialisation**

```
let mut metrics = Vec::new();
```

On crée un vecteur vide qui va contenir toutes les métriques analysées.

`mut` signifie que le vecteur peut être modifié.

### **6.2 Parcourir les lignes**

```
for line in input.lines() {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') {
        continue;
    }
```

On parcourt chaque ligne du texte.

Deux cas sont ignorés :

- Les lignes vides.
- Les lignes qui commencent par `#`.

Pourquoi ignorer `#` ?

Parce que Prometheus utilise notamment ces lignes pour fournir des descriptions et des types :

```
# HELP validator_attestations_total Total attestations
# TYPE validator_attestations_total counter
```

Ces lignes ne représentent pas directement des valeurs de métriques.

`continue` permet de passer à la ligne suivante.

### **6.3 Séparer le nom de la métrique et sa valeur**

```
let (metric_part, value_part) = line
    .rsplit_once(char::is_whitespace)
    .ok_or_else(|| {
        ValidatorError::InvalidMetrics(line.to_string())
    })?;
```

C'est une instruction importante.

`rsplit_once()` sépare une chaîne à partir de la dernière occurrence d'un caractère correspondant au prédicat fourni.

Prenons :

```
validator_attestations_total 42
```

On obtient :

```
metric_part = "validator_attestations_total"
value_part  = "42"
```

Autre exemple :

```
async_tasks_count{async_task_count="attestation_service"} 1
```

On obtient :

```
metric_part = "async_tasks_count{async_task_count="attestation_service"}"
value_part  = "1"
```

Si aucune séparation n'est trouvée, `ok_or_else()` transforme le résultat en erreur `InvalidMetrics`.

Puis `?` propage cette erreur.

### **6.4 Convertir la valeur en nombre**

```
let value: f64 = value_part.parse().map_err(|_| {
    ValidatorError::InvalidMetrics(line.to_string())
})?;
```

La valeur récupérée est initialement une chaîne de caractères.

On doit la convertir en `f64`.

Exemple :

```
"42" → 42.0
"23.75" → 23.75
```

Si la conversion échoue, on retourne une erreur.

### **6.5 Analyser le nom et les labels**

```
let (name, labels) = parse_metric_name_and_labels(metric_part)?;
```

Cette fonction secondaire analyse la partie gauche de la ligne.

Elle récupère :

- Le nom de la métrique.
- Les labels éventuels.

Nous allons voir son fonctionnement juste après.

### **6.6 Construire l'objet final**

```
metrics.push(PrometheusMetric {
    name,
    labels,
    value,
});
```

Une fois toutes les informations extraites, on construit un objet `PrometheusMetric` et on l'ajoute au vecteur.

À la fin :

```
Ok(metrics)
```

On retourne toutes les métriques analysées.

## **7. Comprendre&#x20;**`parse_metric_name_and_labels()`

Cette fonction est chargée de séparer le nom d'une métrique de ses labels.

```
fn parse_metric_name_and_labels(
    input: &str,
) -> Result<(String, HashMap<String, String>), ValidatorError>
```

Elle retourne un tuple :

```
(String, HashMap<String, String>)
```

C'est-à-dire :

- Le nom de la métrique.
- Une collection de labels.

### **7.1 Vérifier la présence des labels**

```
let Some(start) = input.find('{') else {
    return Ok((input.to_string(), HashMap::new()));
};
```

On recherche le caractère `{`.

Deux possibilités :

**Cas 1 : la métrique ne contient aucun label.**

```
validator_attestations_total
```

On retourne :

```
name   = "validator_attestations_total"
labels = {}
```

**Cas 2 : la métrique contient des labels.**

```
async_tasks_count{async_task_count="attestation_service"}
```

On continue l'analyse.

### **7.2 Vérifier la fermeture des accolades**

```
if !input.ends_with('}') {
    return Err(ValidatorError::InvalidMetrics(input.to_string()));
}
```

Une métrique avec labels doit se terminer par `}` dans le format pris en charge par ce parser.

Exemple incorrect :

```
async_tasks_count{async_task_count="attestation_service"
```

Le parser retourne une erreur.

### **7.3 Extraire le nom**

```
let name = input[..start].to_string();
```

On récupère tout ce qui se trouve avant `{`.

Exemple :

```
async_tasks_count{async_task_count="attestation_service"}
```

Résultat :

```
name = "async_tasks_count"
```

### **7.4 Extraire les labels**

```
let labels_part = &input[start + 1..input.len() - 1];
```

On retire les accolades.

Résultat :

```
async_task_count="attestation_service"
```

Puis :

```
let mut labels = HashMap::new();
```

On crée une collection vide pour stocker les labels.

### **7.5 Parcourir les labels**

```
for label in labels_part.split(',') {
```

On sépare les labels avec une virgule.

Exemple :

```
service="attestation",status="active"
```

On obtient deux éléments :

```
service="attestation"
status="active"
```

Puis :

```
let Some((key, value)) = label.split_once('=') else {
    return Err(ValidatorError::InvalidMetrics(
        input.to_string(),
    ));
};
```

On sépare chaque label en deux parties :

- `key` : nom du label.
- `value` : valeur du label.

Ensuite :

```
let value = value
    .trim()
    .trim_matches('"')
    .to_string();
```

On retire les espaces au début et à la fin, puis les guillemets autour de la valeur.

Exemple :

```
"attestation_service" → attestation_service
```

Enfin :

```
labels.insert(key.trim().to_string(), value);
```

On ajoute le label dans le `HashMap`.

Et à la fin :

```
Ok((name, labels))
```

On retourne le nom et les labels.

## **8. Les tests unitaires**

Ton fichier contient trois tests.

### **Test 1 : métrique simple**

```
#[test]
fn parses_simple_metric() {
    let input = "validator_attestations_total 42";

    let metrics = parse_prometheus_metrics(input).unwrap();

    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].name, "validator_attestations_total");
    assert_eq!(metrics[0].value, 42.0);
}
```

Objectif : vérifier qu'une métrique sans labels est correctement analysée.

On vérifie :

- Une seule métrique est retournée.
- Son nom est correct.
- Sa valeur est égale à `42.0`.

### **Test 2 : métrique avec labels**

```
#[test]
fn parses_metric_with_labels() {
    let input =
        r#"async_tasks_count{async_task_count="attestation_service"} 1"#;

    let metrics = parse_prometheus_metrics(input).unwrap();

    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].name, "async_tasks_count");
    assert_eq!(
        metrics[0].labels.get("async_task_count"),
        Some(&"attestation_service".to_string())
    );
    assert_eq!(metrics[0].value, 1.0);
}
```

Objectif : vérifier que le parser extrait correctement les labels.

`r#"... "#` est une chaîne brute Rust : elle permet d'utiliser les guillemets sans les échapper.

### **Test 3 : ignorer les commentaires**

```
#[test]
fn ignores_prometheus_comments() {
    let input = r#"
# HELP validator_attestations_total Total attestations
# TYPE validator_attestations_total counter
validator_attestations_total 42
"#;

    let metrics = parse_prometheus_metrics(input).unwrap();

    assert_eq!(metrics.len(), 1);
}
```

Objectif : vérifier que les commentaires Prometheus ne sont pas interprétés comme des métriques.

3 tests présents

Ces tests vérifient le comportement du parser, mais ne testent pas encore la communication HTTP réelle avec un service Validator.

## **9. Limites techniques à connaître**

Ton implémentation constitue un parser de base, mais elle ne prend pas en charge toutes les possibilités du format Prometheus.

| Limitation          | Explication                                                                          |
| ------------------- | ------------------------------------------------------------------------------------ |
| Labels complexes    | `split(',')` ne gère pas correctement les virgules échappées dans les valeurs.       |
| Guillemets échappés | Les valeurs contenant des guillemets échappés ne sont pas correctement interprétées. |
| Timestamp           | Les lignes avec timestamp supplémentaire ne sont pas correctement prises en charge.  |
| Validation          | Les noms de métriques et les clés des labels ne sont pas entièrement validés.        |
| Labels dupliqués    | Un label répété écrase la valeur précédente dans le `HashMap`.                       |
| Performance         | `get_metric()` récupère toutes les métriques avant de filtrer.                       |

Il faut aussi noter que le parser ignore les déclarations `HELP` et `TYPE`, donc il ne conserve pas les métadonnées indiquant si une métrique est un compteur, une jauge ou un histogramme.

Pour EVA-Guard, ce parser peut être suffisant pour commencer avec des métriques simples, mais il faudra envisager un parser Prometheus plus complet si tu exploites des métriques complexes en production.

## **10. Différence entre les trois clients EVA-Guard**

| Client            | Source            | Protocole         | Données récupérées                       |
| ----------------- | ----------------- | ----------------- | ---------------------------------------- |
| `GethClient`      | Geth              | JSON-RPC + HTTP   | Bloc, peers, synchronisation, métriques  |
| `BeaconClient`    | Lighthouse Beacon | REST API + HTTP   | Slots, peers, synchronisation, finalité  |
| `ValidatorClient` | Validator         | Prometheus + HTTP | Attestations, tâches, métriques internes |

**Ce que tu as implémenté dans ce fichier**

1. Un client HTTP pour accéder aux métriques du Validator.
2. Une structure standardisée `PrometheusMetric`.
3. Un mécanisme de récupération des métriques brutes.
4. Un parser pour transformer le texte Prometheus en objets Rust.
5. Un filtre pour récupérer une métrique spécifique.
6. Trois tests unitaires pour vérifier le parsing.

## **11. Vérifie ta compréhension**

Voici trois petites questions pour vérifier que tu as compris le fonctionnement.

**Question 1. Quel est le rôle principal de ValidatorClient ?**

Détecter directement les attaques Ethereum

Récupérer et parser les métriques Prometheus

Créer des blocs Ethereum

**Question 2. Quelle est la différence entre get_metrics() et collect_metrics() ?**

Aucune différence

get_metrics retourne du texte brut, collect_metrics retourne des objets Rust

get_metrics utilise JSON-RPC

**Question 3. Pourquoi utilise-t-on HashMap dans PrometheusMetric ?**

Pour stocker les labels sous forme clé-valeur

Pour mesurer le temps HTTP

Pour stocker les blocs Ethereum

Vérifier mes réponses

**Prochaine étape dans ton architecture :** étudier `collectors/validator/attestations.rs`, pour comprendre comment EVA-Guard exploite les métriques récupérées par ce client afin de construire des observations sur le comportement des validateurs.