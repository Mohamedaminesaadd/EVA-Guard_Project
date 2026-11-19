use super::parse_optional_decimal;
use crate::clients::beacon_client::{BeaconClient, PeerCounts};
use crate::models::metrics::PeerMetrics;

/// Collects Beacon Node peer metrics.
pub struct PeersCollector;

impl PeersCollector {
    /// Collects peer counts from one Beacon Node.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(client: &BeaconClient) -> PeerMetrics {
        let mut metrics = PeerMetrics::default();

        // Retrieve peer connection states.
        if let Ok(peers) = client.peer_count().await {
            apply_peer_counts(&mut metrics, &peers);
        }

        metrics
    }
}

/// Fills the metrics from a `/eth/v1/node/peer_count` answer.
fn apply_peer_counts(metrics: &mut PeerMetrics, peers: &PeerCounts) {
    metrics.peer_count = parse_optional_decimal(peers.connected.as_deref());
    metrics.connecting_peers = parse_optional_decimal(peers.connecting.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::json;
    use wiremock::MockServer;

    const ROUTE: &str = "/eth/v1/node/peer_count";

    fn counts(connected: Option<&str>, connecting: Option<&str>) -> PeerCounts {
        PeerCounts {
            connected: connected.map(str::to_string),
            connecting: connecting.map(str::to_string),
            disconnected: Some("7".to_string()),
            disconnecting: Some("0".to_string()),
        }
    }

    // ---- apply_peer_counts --------------------------------------------------

    #[test]
    fn parses_connected_and_connecting_peers() {
        let mut metrics = PeerMetrics::default();

        apply_peer_counts(&mut metrics, &counts(Some("15"), Some("2")));

        assert_eq!(metrics.peer_count, Some(15));
        assert_eq!(metrics.connecting_peers, Some(2));
    }

    #[test]
    fn parses_zero_peers() {
        let mut metrics = PeerMetrics::default();

        apply_peer_counts(&mut metrics, &counts(Some("0"), Some("0")));

        assert_eq!(metrics.peer_count, Some(0));
        assert_eq!(metrics.connecting_peers, Some(0));
    }

    #[test]
    fn missing_counts_stay_none() {
        let mut metrics = PeerMetrics::default();

        apply_peer_counts(&mut metrics, &counts(None, None));

        assert_eq!(metrics.peer_count, None);
        assert_eq!(metrics.connecting_peers, None);
    }

    #[test]
    fn invalid_connected_does_not_discard_connecting() {
        let mut metrics = PeerMetrics::default();

        apply_peer_counts(&mut metrics, &counts(Some("invalid"), Some("4")));

        assert_eq!(metrics.peer_count, None);
        assert_eq!(metrics.connecting_peers, Some(4));
    }

    #[test]
    fn parses_large_peer_count() {
        let mut metrics = PeerMetrics::default();

        apply_peer_counts(&mut metrics, &counts(Some("1000"), None));

        assert_eq!(metrics.peer_count, Some(1000));
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_from_the_beacon_api() {
        let server = MockServer::start().await;
        mock_json(
            &server,
            ROUTE,
            json!({ "data": {
                "disconnected": "30",
                "connecting": "2",
                "connected": "15",
                "disconnecting": "0"
            }}),
        )
        .await;

        let metrics = PeersCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.peer_count, Some(15));
        assert_eq!(metrics.connecting_peers, Some(2));
    }

    #[tokio::test]
    async fn http_error_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_status(&server, ROUTE, 503).await;

        let metrics = PeersCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.peer_count, None);
        assert_eq!(metrics.connecting_peers, None);
    }

    #[tokio::test]
    async fn malformed_json_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_garbage(&server, ROUTE).await;

        let metrics = PeersCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.peer_count, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_empty_metrics() {
        let metrics = PeersCollector::collect(&unreachable_client()).await;

        assert_eq!(metrics.peer_count, None);
    }
}
