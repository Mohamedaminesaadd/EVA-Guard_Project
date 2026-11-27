use super::parse_optional_decimal;
use crate::clients::beacon_client::{BeaconClient, BeaconSyncStatus};
use crate::models::metrics::SyncMetrics;

/// Collects Beacon Node synchronization metrics.
pub struct SyncCollector;

impl SyncCollector {
    /// Collects synchronization information from one Beacon Node.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(client: &BeaconClient) -> SyncMetrics {
        let mut metrics = SyncMetrics::default();

        // Retrieve Beacon Node synchronization status.
        if let Ok(status) = client.syncing().await {
            apply_sync_status(&mut metrics, &status);
        }

        metrics
    }
}

/// Fills the metrics from a `/eth/v1/node/syncing` answer.
///
/// Numeric fields are decimal strings in the Beacon API; a value that
/// cannot be parsed stays None without affecting the other fields.
fn apply_sync_status(metrics: &mut SyncMetrics, status: &BeaconSyncStatus) {
    metrics.head_slot = parse_optional_decimal(status.head_slot.as_deref());
    metrics.sync_distance = parse_optional_decimal(status.sync_distance.as_deref());
    metrics.is_syncing = status.is_syncing;
    metrics.is_optimistic = status.is_optimistic;
    metrics.el_offline = status.el_offline;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::json;
    use wiremock::MockServer;

    const ROUTE: &str = "/eth/v1/node/syncing";

    fn status(
        head_slot: Option<&str>,
        sync_distance: Option<&str>,
        is_syncing: Option<bool>,
        is_optimistic: Option<bool>,
        el_offline: Option<bool>,
    ) -> BeaconSyncStatus {
        BeaconSyncStatus {
            head_slot: head_slot.map(str::to_string),
            sync_distance: sync_distance.map(str::to_string),
            is_syncing,
            is_optimistic,
            el_offline,
        }
    }

    // ---- apply_sync_status -------------------------------------------------

    #[test]
    fn synced_node_is_reported() {
        let mut metrics = SyncMetrics::default();

        apply_sync_status(
            &mut metrics,
            &status(
                Some("1200"),
                Some("0"),
                Some(false),
                Some(false),
                Some(false),
            ),
        );

        assert_eq!(metrics.head_slot, Some(1200));
        assert_eq!(metrics.sync_distance, Some(0));
        assert_eq!(metrics.is_syncing, Some(false));
        assert_eq!(metrics.is_optimistic, Some(false));
        assert_eq!(metrics.el_offline, Some(false));
    }

    #[test]
    fn syncing_node_is_reported() {
        let mut metrics = SyncMetrics::default();

        apply_sync_status(
            &mut metrics,
            &status(Some("900"), Some("300"), Some(true), Some(true), Some(true)),
        );

        assert_eq!(metrics.head_slot, Some(900));
        assert_eq!(metrics.sync_distance, Some(300));
        assert_eq!(metrics.is_syncing, Some(true));
        assert_eq!(metrics.is_optimistic, Some(true));
        assert_eq!(metrics.el_offline, Some(true));
    }

    #[test]
    fn missing_fields_stay_none() {
        let mut metrics = SyncMetrics::default();

        apply_sync_status(&mut metrics, &status(None, None, None, None, None));

        assert_eq!(metrics.head_slot, None);
        assert_eq!(metrics.sync_distance, None);
        assert_eq!(metrics.is_syncing, None);
        assert_eq!(metrics.is_optimistic, None);
        assert_eq!(metrics.el_offline, None);
    }

    #[test]
    fn invalid_numbers_do_not_affect_other_fields() {
        let mut metrics = SyncMetrics::default();

        apply_sync_status(
            &mut metrics,
            &status(Some("invalid"), Some("-5"), Some(true), None, Some(false)),
        );

        assert_eq!(metrics.head_slot, None);
        assert_eq!(metrics.sync_distance, None);
        assert_eq!(metrics.is_syncing, Some(true));
        assert_eq!(metrics.el_offline, Some(false));
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_from_the_beacon_api() {
        let server = MockServer::start().await;
        mock_json(
            &server,
            ROUTE,
            json!({ "data": {
                "head_slot": "4096",
                "sync_distance": "2",
                "is_syncing": false,
                "is_optimistic": false,
                "el_offline": false
            }}),
        )
        .await;

        let metrics = SyncCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.head_slot, Some(4096));
        assert_eq!(metrics.sync_distance, Some(2));
        assert_eq!(metrics.is_syncing, Some(false));
        assert_eq!(metrics.is_optimistic, Some(false));
        assert_eq!(metrics.el_offline, Some(false));
    }

    #[tokio::test]
    async fn http_error_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_status(&server, ROUTE, 500).await;

        let metrics = SyncCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.head_slot, None);
        assert_eq!(metrics.is_syncing, None);
    }

    #[tokio::test]
    async fn malformed_json_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_garbage(&server, ROUTE).await;

        let metrics = SyncCollector::collect(&client_for(&server)).await;

        assert_eq!(metrics.head_slot, None);
        assert_eq!(metrics.sync_distance, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_empty_metrics() {
        let metrics = SyncCollector::collect(&unreachable_client()).await;

        assert_eq!(metrics.head_slot, None);
        assert_eq!(metrics.is_syncing, None);
    }
}
