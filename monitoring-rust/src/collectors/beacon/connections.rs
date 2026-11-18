use super::parse_optional_decimal;
use crate::clients::beacon_client::BeaconClient;
use crate::models::metrics::PeerConnectionMetrics;

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Collects Beacon Node peer connection metrics.
///
/// Tracks the *net* change of the connected peer count between two
/// consecutive valid cycles. It cannot see peers being replaced one for
/// one: 5 peers leaving while 5 others join gives a net change of zero.
/// One collector instance must be used per node.
pub struct ConnectionsCollector {
    previous_connected: Mutex<Option<u64>>,
}

impl ConnectionsCollector {
    pub fn new() -> Self {
        Self {
            previous_connected: Mutex::new(None),
        }
    }

    /// Collects connection metrics from one Beacon Node.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(&self, client: &BeaconClient) -> PeerConnectionMetrics {
        let mut metrics = PeerConnectionMetrics::default();

        // 1. Retrieve peer connection states.
        let Ok(peers) = client.peer_count().await else {
            self.reset_baseline();
            return metrics;
        };

        // 2. Parse connected peers.
        let Some(connected) = parse_optional_decimal(peers.connected.as_deref()) else {
            self.reset_baseline();
            return metrics;
        };

        // 3. Calculate connection changes.
        self.update_connection_changes(&mut metrics, connected);

        metrics
    }

    /// Calculates changes between consecutive collection cycles.
    ///
    /// The first call only stores the baseline and measures nothing.
    fn update_connection_changes(&self, metrics: &mut PeerConnectionMetrics, connected: u64) {
        let mut previous = self.lock_previous();

        if let Some(old) = *previous {
            let (new_peers, disconnected_peers) = if connected >= old {
                (connected - old, 0)
            } else {
                (0, old - connected)
            };

            metrics.new_peers = Some(new_peers);
            metrics.disconnected_peers = Some(disconnected_peers);
            metrics.peer_churn_count = Some(new_peers + disconnected_peers);
        }

        // Save the current sample.
        *previous = Some(connected);
    }

    /// Forgets the previous sample, so that the next change is measured
    /// between two consecutive valid cycles and not across a failure.
    fn reset_baseline(&self) {
        *self.lock_previous() = None;
    }

    /// Locks the previous sample, recovering from a poisoned mutex.
    ///
    /// The protected value is replaced atomically and can never be left
    /// half-updated, so continuing after a panic elsewhere is safe.
    fn lock_previous(&self) -> MutexGuard<'_, Option<u64>> {
        self.previous_connected
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for ConnectionsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::json;
    use std::sync::Arc;
    use wiremock::MockServer;

    const ROUTE: &str = "/eth/v1/node/peer_count";

    fn cycle(collector: &ConnectionsCollector, connected: u64) -> PeerConnectionMetrics {
        let mut metrics = PeerConnectionMetrics::default();
        collector.update_connection_changes(&mut metrics, connected);
        metrics
    }

    fn peer_count_body(connected: &str) -> serde_json::Value {
        json!({ "data": {
            "disconnected": "0",
            "connecting": "0",
            "connected": connected,
            "disconnecting": "0"
        }})
    }

    // ---- update_connection_changes ---------------------------------------

    #[test]
    fn first_cycle_has_no_churn_measurement() {
        let collector = ConnectionsCollector::new();

        let metrics = cycle(&collector, 5);

        assert_eq!(metrics.new_peers, None);
        assert_eq!(metrics.disconnected_peers, None);
        assert_eq!(metrics.peer_churn_count, None);
    }

    #[test]
    fn detects_new_connections() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 5);

        let metrics = cycle(&collector, 8);

        assert_eq!(metrics.new_peers, Some(3));
        assert_eq!(metrics.disconnected_peers, Some(0));
        assert_eq!(metrics.peer_churn_count, Some(3));
    }

    #[test]
    fn detects_disconnections() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 8);

        let metrics = cycle(&collector, 5);

        assert_eq!(metrics.new_peers, Some(0));
        assert_eq!(metrics.disconnected_peers, Some(3));
        assert_eq!(metrics.peer_churn_count, Some(3));
    }

    #[test]
    fn stable_count_has_zero_churn() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 5);

        let metrics = cycle(&collector, 5);

        assert_eq!(metrics.new_peers, Some(0));
        assert_eq!(metrics.disconnected_peers, Some(0));
        assert_eq!(metrics.peer_churn_count, Some(0));
    }

    #[test]
    fn losing_every_peer_is_reported() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 12);

        let metrics = cycle(&collector, 0);

        assert_eq!(metrics.disconnected_peers, Some(12));
        assert_eq!(metrics.peer_churn_count, Some(12));
    }

    #[test]
    fn uses_the_latest_sample_as_baseline() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 2);
        cycle(&collector, 10);

        let metrics = cycle(&collector, 12);

        assert_eq!(metrics.new_peers, Some(2));
        assert_eq!(metrics.peer_churn_count, Some(2));
    }

    #[test]
    fn net_change_does_not_see_one_for_one_peer_replacement() {
        // Documents a known limitation: only counts are compared, so a full
        // replacement of the peer set (e.g. an eclipse attempt) is invisible.
        let collector = ConnectionsCollector::new();
        cycle(&collector, 5);

        let metrics = cycle(&collector, 5);

        assert_eq!(metrics.peer_churn_count, Some(0));
    }

    #[test]
    fn reset_baseline_prevents_changes_across_a_failure() {
        let collector = ConnectionsCollector::new();
        cycle(&collector, 5);

        collector.reset_baseline();

        let metrics = cycle(&collector, 50);

        assert_eq!(metrics.new_peers, None);
        assert_eq!(metrics.peer_churn_count, None);
    }

    #[test]
    fn recovers_from_a_poisoned_mutex() {
        let collector = Arc::new(ConnectionsCollector::new());
        cycle(&collector, 5);

        let poisoner = Arc::clone(&collector);
        let result = std::thread::spawn(move || {
            let _guard = poisoner.previous_connected.lock().unwrap();
            panic!("poison the mutex on purpose");
        })
        .join();

        assert!(result.is_err());
        assert!(collector.previous_connected.is_poisoned());

        let metrics = cycle(&collector, 7);

        assert_eq!(metrics.new_peers, Some(2));
        assert_eq!(metrics.peer_churn_count, Some(2));
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_changes_between_two_cycles() {
        let first_node = MockServer::start().await;
        mock_json(&first_node, ROUTE, peer_count_body("5")).await;
        let second_node = MockServer::start().await;
        mock_json(&second_node, ROUTE, peer_count_body("8")).await;

        let collector = ConnectionsCollector::new();

        let first = collector.collect(&client_for(&first_node)).await;
        assert_eq!(first.peer_churn_count, None);

        let second = collector.collect(&client_for(&second_node)).await;
        assert_eq!(second.new_peers, Some(3));
        assert_eq!(second.disconnected_peers, Some(0));
        assert_eq!(second.peer_churn_count, Some(3));
    }

    #[tokio::test]
    async fn failed_cycle_resets_the_baseline() {
        let healthy = MockServer::start().await;
        mock_json(&healthy, ROUTE, peer_count_body("5")).await;
        let failing = MockServer::start().await;
        mock_status(&failing, ROUTE, 500).await;
        let recovered = MockServer::start().await;
        mock_json(&recovered, ROUTE, peer_count_body("9")).await;

        let collector = ConnectionsCollector::new();
        collector.collect(&client_for(&healthy)).await;

        let failed = collector.collect(&client_for(&failing)).await;
        assert_eq!(failed.peer_churn_count, None);

        // First valid sample after the failure: baseline only.
        let after = collector.collect(&client_for(&recovered)).await;
        assert_eq!(after.new_peers, None);
        assert_eq!(after.peer_churn_count, None);
    }

    #[tokio::test]
    async fn invalid_connected_value_is_a_failed_cycle() {
        let healthy = MockServer::start().await;
        mock_json(&healthy, ROUTE, peer_count_body("5")).await;
        let invalid = MockServer::start().await;
        mock_json(&invalid, ROUTE, peer_count_body("many")).await;
        let recovered = MockServer::start().await;
        mock_json(&recovered, ROUTE, peer_count_body("6")).await;

        let collector = ConnectionsCollector::new();
        collector.collect(&client_for(&healthy)).await;

        let bad = collector.collect(&client_for(&invalid)).await;
        assert_eq!(bad.peer_churn_count, None);

        let after = collector.collect(&client_for(&recovered)).await;
        assert_eq!(after.peer_churn_count, None);
    }

    #[tokio::test]
    async fn malformed_json_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_garbage(&server, ROUTE).await;

        let metrics = ConnectionsCollector::new()
            .collect(&client_for(&server))
            .await;

        assert_eq!(metrics.peer_churn_count, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_empty_metrics() {
        let metrics = ConnectionsCollector::new()
            .collect(&unreachable_client())
            .await;

        assert_eq!(metrics.new_peers, None);
        assert_eq!(metrics.disconnected_peers, None);
    }
}
