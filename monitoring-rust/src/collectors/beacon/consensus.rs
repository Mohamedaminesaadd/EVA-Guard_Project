use super::parse_optional_decimal;
use crate::clients::beacon_client::{BeaconClient, FinalityCheckpoints};
use crate::models::metrics::ConsensusMetrics;

/// Collects Ethereum consensus and finality metrics.
pub struct ConsensusCollector;

impl ConsensusCollector {
    /// Collects finality checkpoints from one Beacon Node.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(client: &BeaconClient) -> ConsensusMetrics {
        let mut metrics = ConsensusMetrics::default();

        // Retrieve finality checkpoints.
        if let Ok(checkpoints) = client.finality_checkpoints().await {
            apply_checkpoints(&mut metrics, &checkpoints);
        }

        metrics
    }
}

/// Fills the metrics from a `finality_checkpoints` answer.
///
/// `finality_distance_epochs` is `current_justified - finalized`. It stays
/// None when either epoch is unavailable or when the order is invalid
/// (justified older than finalized).
fn apply_checkpoints(metrics: &mut ConsensusMetrics, checkpoints: &FinalityCheckpoints) {
    let justified_epoch = checkpoints
        .current_justified
        .as_ref()
        .and_then(|checkpoint| parse_optional_decimal(checkpoint.epoch.as_deref()));

    let finalized_epoch = checkpoints
        .finalized
        .as_ref()
        .and_then(|checkpoint| parse_optional_decimal(checkpoint.epoch.as_deref()));

    metrics.justified_epoch = justified_epoch;
    metrics.finalized_epoch = finalized_epoch;

    if let (Some(justified), Some(finalized)) = (justified_epoch, finalized_epoch) {
        metrics.finality_distance_epochs = justified.checked_sub(finalized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::beacon_client::Checkpoint;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::json;
    use wiremock::MockServer;

    const ROUTE: &str = "/eth/v1/beacon/states/head/finality_checkpoints";

    fn checkpoint(epoch: Option<&str>) -> Option<Checkpoint> {
        Some(Checkpoint {
            epoch: epoch.map(str::to_string),
            root: Some("0xroot".to_string()),
        })
    }

    fn checkpoints(justified: Option<&str>, finalized: Option<&str>) -> FinalityCheckpoints {
        FinalityCheckpoints {
            previous_justified: checkpoint(Some("1")),
            current_justified: checkpoint(justified),
            finalized: checkpoint(finalized),
        }
    }

    // ---- apply_checkpoints -------------------------------------------------

    #[test]
    fn calculates_finality_distance() {
        let mut metrics = ConsensusMetrics::default();

        apply_checkpoints(&mut metrics, &checkpoints(Some("15"), Some("12")));

        assert_eq!(metrics.justified_epoch, Some(15));
        assert_eq!(metrics.finalized_epoch, Some(12));
        assert_eq!(metrics.finality_distance_epochs, Some(3));
    }

    #[test]
    fn distance_is_zero_when_justified_equals_finalized() {
        let mut metrics = ConsensusMetrics::default();

        apply_checkpoints(&mut metrics, &checkpoints(Some("0"), Some("0")));

        assert_eq!(metrics.justified_epoch, Some(0));
        assert_eq!(metrics.finalized_epoch, Some(0));
        assert_eq!(metrics.finality_distance_epochs, Some(0));
    }

    #[test]
    fn rejects_invalid_finality_order() {
        let mut metrics = ConsensusMetrics::default();

        apply_checkpoints(&mut metrics, &checkpoints(Some("10"), Some("12")));

        assert_eq!(metrics.justified_epoch, Some(10));
        assert_eq!(metrics.finalized_epoch, Some(12));
        assert_eq!(metrics.finality_distance_epochs, None);
    }

    #[test]
    fn invalid_finalized_epoch_keeps_justified_epoch() {
        let mut metrics = ConsensusMetrics::default();

        apply_checkpoints(&mut metrics, &checkpoints(Some("15"), Some("invalid")));

        assert_eq!(metrics.justified_epoch, Some(15));
        assert_eq!(metrics.finalized_epoch, None);
        assert_eq!(metrics.finality_distance_epochs, None);
    }

    #[test]
    fn missing_checkpoint_objects_stay_none() {
        let mut metrics = ConsensusMetrics::default();
        let empty = FinalityCheckpoints {
            previous_justified: None,
            current_justified: None,
            finalized: None,
        };

        apply_checkpoints(&mut metrics, &empty);

        assert_eq!(metrics.justified_epoch, None);
        assert_eq!(metrics.finalized_epoch, None);
        assert_eq!(metrics.finality_distance_epochs, None);
    }

    #[test]
    fn missing_epoch_inside_a_checkpoint_stays_none() {
        let mut metrics = ConsensusMetrics::default();

        apply_checkpoints(&mut metrics, &checkpoints(None, Some("4")));

        assert_eq!(metrics.justified_epoch, None);
        assert_eq!(metrics.finalized_epoch, Some(4));
        assert_eq!(metrics.finality_distance_epochs, None);
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_from_the_beacon_api() {
        let server = MockServer::start().await;
        mock_json(
            &server,
            ROUTE,
            json!({ "data": {
                "previous_justified": { "epoch": "10", "root": "0xa" },
                "current_justified": { "epoch": "11", "root": "0xb" },
                "finalized": { "epoch": "9", "root": "0xc" }
            }}),
        )
        .await;

        let metrics = ConsensusCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.justified_epoch, Some(11));
        assert_eq!(metrics.finalized_epoch, Some(9));
        assert_eq!(metrics.finality_distance_epochs, Some(2));
    }

    #[tokio::test]
    async fn http_error_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_status(&server, ROUTE, 500).await;

        let metrics = ConsensusCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.justified_epoch, None);
        assert_eq!(metrics.finalized_epoch, None);
        assert_eq!(metrics.finality_distance_epochs, None);
    }

    #[tokio::test]
    async fn malformed_json_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_garbage(&server, ROUTE).await;

        let metrics = ConsensusCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.finalized_epoch, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_empty_metrics() {
        let metrics = ConsensusCollector::collect(&unreachable_client()).await;

        assert_eq!(metrics.finalized_epoch, None);
    }
}
