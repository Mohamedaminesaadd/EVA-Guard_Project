use crate::clients::beacon_client::BeaconClient;
use crate::models::metrics::BlockPropagationMetrics;

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::task::JoinSet;

/// One node's observation of a head block root.
#[derive(Debug, Clone)]
struct HeadObservation {
    node_id: String,
    observed_at: Instant,
}

/// Observation spread of the head roots seen by at least two nodes.
#[derive(Debug)]
struct ObservationSpread {
    /// Largest spread between the first and the last node that observed
    /// the same root.
    max_delay_ms: f64,
    /// Nodes that took part in at least one shared root.
    participants: HashSet<String>,
}

/// Collects block propagation observations across multiple Beacon Nodes.
pub struct PropagationCollector;

impl PropagationCollector {
    /// Compares the latest block observed by each Beacon Node.
    ///
    /// Each tuple contains:
    /// - Node ID
    /// - Beacon client
    ///
    /// Every node is queried concurrently, so the observation times
    /// reflect when each node answered and not the order of the loop.
    ///
    /// Returns one propagation metric per node. A node gets a value only
    /// if it observed a block root that at least one other node also
    /// observed; unreachable nodes and nodes on a different head stay None.
    pub async fn collect(
        nodes: &[(&str, &BeaconClient)],
    ) -> HashMap<String, BlockPropagationMetrics> {
        let mut results: HashMap<String, BlockPropagationMetrics> = nodes
            .iter()
            .map(|(node_id, _)| ((*node_id).to_string(), BlockPropagationMetrics::default()))
            .collect();

        // 1. Retrieve the latest head from every node, concurrently.
        let mut tasks = JoinSet::new();

        for (node_id, client) in nodes {
            let node_id = (*node_id).to_string();
            let client = (*client).clone();

            tasks.spawn(async move {
                let root = client
                    .head_header()
                    .await
                    .ok()
                    .and_then(|header| header.root)
                    .filter(|root| !root.is_empty());

                // Local observation time, taken as soon as the node answered.
                (node_id, root, Instant::now())
            });
        }

        // Store observed block roots and their local observation times.
        let mut observations: HashMap<String, Vec<HeadObservation>> = HashMap::new();

        while let Some(joined) = tasks.join_next().await {
            // Failed requests and panicked tasks leave the node unmeasured.
            let Ok((node_id, Some(root), observed_at)) = joined else {
                continue;
            };

            observations.entry(root).or_default().push(HeadObservation {
                node_id,
                observed_at,
            });
        }

        // 2. Calculate observation spread for blocks seen by
        // at least two different nodes.
        let Some(spread) = observation_spread(&observations) else {
            return results;
        };

        // 3. Assign the observation spread to the nodes that produced it.
        for node_id in &spread.participants {
            if let Some(metrics) = results.get_mut(node_id) {
                metrics.block_propagation_delay_ms = Some(spread.max_delay_ms);
            }
        }

        results
    }
}

/// Finds the largest observation spread among roots seen by 2+ nodes.
///
/// Returns None when no root was observed by at least two nodes.
fn observation_spread(
    observations: &HashMap<String, Vec<HeadObservation>>,
) -> Option<ObservationSpread> {
    let mut max_delay_ms: Option<f64> = None;
    let mut participants = HashSet::new();

    for seen_by in observations.values() {
        if seen_by.len() < 2 {
            continue;
        }

        let first = seen_by
            .iter()
            .map(|observation| observation.observed_at)
            .min();
        let last = seen_by
            .iter()
            .map(|observation| observation.observed_at)
            .max();

        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };

        let delay_ms = last.saturating_duration_since(first).as_secs_f64() * 1000.0;

        max_delay_ms = Some(max_delay_ms.map_or(delay_ms, |current| current.max(delay_ms)));

        participants.extend(
            seen_by
                .iter()
                .map(|observation| observation.node_id.clone()),
        );
    }

    max_delay_ms.map(|max_delay_ms| ObservationSpread {
        max_delay_ms,
        participants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::client_for;

    use serde_json::{json, Value};
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const HEAD_ROUTE: &str = "/eth/v1/beacon/headers/head";

    fn observation(node_id: &str, observed_at: Instant) -> HeadObservation {
        HeadObservation {
            node_id: node_id.to_string(),
            observed_at,
        }
    }

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn head_body(root: &str) -> Value {
        json!({ "data": {
            "root": root,
            "canonical": true,
            "header": { "message": { "slot": "5" } }
        }})
    }

    /// Starts a node that answers with `root` after `delay`
    /// (or with HTTP 500 when `root` is None).
    async fn node(root: Option<&str>, delay: Duration) -> MockServer {
        let server = MockServer::start().await;

        let response = match root {
            Some(root) => ResponseTemplate::new(200)
                .set_body_json(head_body(root))
                .set_delay(delay),
            None => ResponseTemplate::new(500),
        };

        Mock::given(method("GET"))
            .and(path(HEAD_ROUTE))
            .respond_with(response)
            .mount(&server)
            .await;

        server
    }

    async fn collect_from(
        ids: &[&str],
        servers: &[MockServer],
    ) -> HashMap<String, BlockPropagationMetrics> {
        let clients: Vec<BeaconClient> = servers.iter().map(client_for).collect();
        let nodes: Vec<(&str, &BeaconClient)> = ids.iter().copied().zip(clients.iter()).collect();

        PropagationCollector::collect(&nodes).await
    }

    // ---- observation_spread -------------------------------------------------

    #[test]
    fn calculates_observation_spread() {
        let start = Instant::now();
        let mut observations = HashMap::new();
        observations.insert(
            "0xroot".to_string(),
            vec![
                observation("a", start),
                observation("b", start + ms(100)),
                observation("c", start + ms(300)),
            ],
        );

        let spread = observation_spread(&observations).unwrap();

        assert!((spread.max_delay_ms - 300.0).abs() < 0.001);
        assert_eq!(spread.participants.len(), 3);
    }

    #[test]
    fn spread_does_not_depend_on_insertion_order() {
        let start = Instant::now();
        let mut observations = HashMap::new();
        observations.insert(
            "0xroot".to_string(),
            vec![
                observation("c", start + ms(300)),
                observation("a", start),
                observation("b", start + ms(100)),
            ],
        );

        let spread = observation_spread(&observations).unwrap();

        assert!((spread.max_delay_ms - 300.0).abs() < 0.001);
    }

    #[test]
    fn requires_at_least_two_observations() {
        let mut observations = HashMap::new();
        observations.insert("0xroot".to_string(), vec![observation("a", Instant::now())]);

        assert!(observation_spread(&observations).is_none());
    }

    #[test]
    fn no_observations_give_no_spread() {
        assert!(observation_spread(&HashMap::new()).is_none());
    }

    #[test]
    fn identical_roots_are_grouped_and_different_roots_are_not_mixed() {
        let start = Instant::now();
        let mut observations = HashMap::new();
        // Seen by two nodes 40 ms apart.
        observations.insert(
            "0xroot-a".to_string(),
            vec![observation("a", start), observation("b", start + ms(40))],
        );
        // A different root seen by one node much later must not widen it.
        observations.insert(
            "0xroot-b".to_string(),
            vec![observation("c", start + ms(5_000))],
        );

        let spread = observation_spread(&observations).unwrap();

        assert!((spread.max_delay_ms - 40.0).abs() < 0.001);
        assert!(spread.participants.contains("a"));
        assert!(spread.participants.contains("b"));
        assert!(!spread.participants.contains("c"));
    }

    #[test]
    fn uses_the_largest_spread_across_roots() {
        let start = Instant::now();
        let mut observations = HashMap::new();
        observations.insert(
            "0xroot-a".to_string(),
            vec![observation("a", start), observation("b", start + ms(40))],
        );
        observations.insert(
            "0xroot-b".to_string(),
            vec![observation("c", start), observation("d", start + ms(250))],
        );

        let spread = observation_spread(&observations).unwrap();

        assert!((spread.max_delay_ms - 250.0).abs() < 0.001);
        assert_eq!(spread.participants.len(), 4);
    }

    #[test]
    fn simultaneous_observations_have_zero_spread() {
        let now = Instant::now();
        let mut observations = HashMap::new();
        observations.insert(
            "0xroot".to_string(),
            vec![observation("a", now), observation("b", now)],
        );

        let spread = observation_spread(&observations).unwrap();

        assert!(spread.max_delay_ms.abs() < 0.001);
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn empty_node_list_gives_empty_result() {
        let results = PropagationCollector::collect(&[]).await;

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn nodes_on_the_same_head_get_a_small_delay() {
        let servers = vec![
            node(Some("0xabc"), ms(0)).await,
            node(Some("0xabc"), ms(0)).await,
            node(Some("0xabc"), ms(0)).await,
        ];

        let results = collect_from(&["n1", "n2", "n3"], &servers).await;

        assert_eq!(results.len(), 3);
        for id in ["n1", "n2", "n3"] {
            let delay = results[id].block_propagation_delay_ms.expect("a delay");
            assert!((0.0..500.0).contains(&delay), "{id}: {delay}");
        }
    }

    #[tokio::test]
    async fn unreachable_node_gets_no_measurement() {
        let servers = vec![
            node(Some("0xabc"), ms(0)).await,
            node(Some("0xabc"), ms(0)).await,
            node(None, ms(0)).await,
        ];

        let results = collect_from(&["n1", "n2", "down"], &servers).await;

        assert!(results["n1"].block_propagation_delay_ms.is_some());
        assert!(results["n2"].block_propagation_delay_ms.is_some());
        assert!(results["down"].block_propagation_delay_ms.is_none());
        // The unreachable node is still part of the result.
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn node_on_a_different_head_gets_no_measurement() {
        let servers = vec![
            node(Some("0xabc"), ms(0)).await,
            node(Some("0xabc"), ms(0)).await,
            node(Some("0xfork"), ms(0)).await,
        ];

        let results = collect_from(&["n1", "n2", "isolated"], &servers).await;

        assert!(results["n1"].block_propagation_delay_ms.is_some());
        assert!(results["n2"].block_propagation_delay_ms.is_some());
        assert!(results["isolated"].block_propagation_delay_ms.is_none());
    }

    #[tokio::test]
    async fn single_responding_node_gets_no_measurement() {
        let servers = vec![node(Some("0xabc"), ms(0)).await, node(None, ms(0)).await];

        let results = collect_from(&["n1", "down"], &servers).await;

        assert!(results["n1"].block_propagation_delay_ms.is_none());
        assert!(results["down"].block_propagation_delay_ms.is_none());
    }

    #[tokio::test]
    async fn all_nodes_down_gives_no_measurement() {
        let servers = vec![node(None, ms(0)).await, node(None, ms(0)).await];

        let results = collect_from(&["a", "b"], &servers).await;

        assert_eq!(results.len(), 2);
        assert!(results
            .values()
            .all(|m| m.block_propagation_delay_ms.is_none()));
    }

    #[tokio::test]
    async fn empty_root_is_not_an_observation() {
        let servers = vec![node(Some(""), ms(0)).await, node(Some(""), ms(0)).await];

        let results = collect_from(&["a", "b"], &servers).await;

        assert!(results
            .values()
            .all(|m| m.block_propagation_delay_ms.is_none()));
    }

    #[tokio::test]
    async fn nodes_are_queried_concurrently() {
        // Three nodes answering after 300 ms each. Queried one after the
        // other this would take 900 ms and report a ~600 ms "delay" that is
        // only an artifact of the polling order.
        let servers = vec![
            node(Some("0xabc"), ms(300)).await,
            node(Some("0xabc"), ms(300)).await,
            node(Some("0xabc"), ms(300)).await,
        ];

        let started = Instant::now();
        let results = collect_from(&["n1", "n2", "n3"], &servers).await;
        let elapsed = started.elapsed();

        assert!(
            elapsed < ms(700),
            "took {elapsed:?}, nodes were queried in sequence"
        );

        let delay = results["n1"].block_propagation_delay_ms.expect("a delay");
        assert!(
            delay < 250.0,
            "delay {delay} ms looks like a polling artifact"
        );
    }
}
