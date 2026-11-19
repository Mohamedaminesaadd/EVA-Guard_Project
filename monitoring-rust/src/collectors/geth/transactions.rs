use super::{call_value, hex_field};
use crate::clients::geth_client::GethClient;
use crate::models::metrics::TransactionMetrics;

use serde_json::Value;

/// Collects transaction pool metrics from one Geth node.
pub struct TransactionCollector;

impl TransactionCollector {
    /// Collects pending and queued transaction counts from Geth.
    ///
    /// Requires the Geth `txpool` namespace to be enabled
    /// (e.g. `--http.api eth,net,web3,txpool`).
    /// Unavailable measurements remain None.
    pub async fn collect(client: &GethClient) -> TransactionMetrics {
        let mut metrics = TransactionMetrics::default();

        if let Some(status) = call_value(client, "txpool_status", Vec::new()).await {
            apply_txpool_status(&mut metrics, &status);
        }

        metrics
    }
}

/// Fills pool counters from a `txpool_status` result.
///
/// Geth answers `{"pending": "0x10", "queued": "0x0"}`. Each counter is
/// parsed independently so one bad field does not discard the other.
fn apply_txpool_status(metrics: &mut TransactionMetrics, status: &Value) {
    metrics.txpool_pending = hex_field(status, "pending");
    metrics.txpool_queued = hex_field(status, "queued");
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn parses_pending_and_queued() {
        let status = json!({ "pending": "0x10", "queued": "0x2" });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, Some(16));
        assert_eq!(metrics.txpool_queued, Some(2));
    }

    #[test]
    fn parses_zero_transactions() {
        let status = json!({ "pending": "0x0", "queued": "0x0" });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, Some(0));
        assert_eq!(metrics.txpool_queued, Some(0));
    }

    #[test]
    fn invalid_pending_does_not_discard_queued() {
        let status = json!({ "pending": "0xxyz", "queued": "0x5" });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, None);
        assert_eq!(metrics.txpool_queued, Some(5));
    }

    #[test]
    fn missing_queued_does_not_discard_pending() {
        let status = json!({ "pending": "0x7" });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, Some(7));
        assert_eq!(metrics.txpool_queued, None);
    }

    #[test]
    fn rejects_empty_and_unprefixed_values() {
        let status = json!({ "pending": "0x", "queued": "10" });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, None);
        assert_eq!(metrics.txpool_queued, None);
    }

    #[test]
    fn rejects_non_string_values() {
        let status = json!({ "pending": 16, "queued": null });
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &status);

        assert_eq!(metrics.txpool_pending, None);
        assert_eq!(metrics.txpool_queued, None);
    }

    #[test]
    fn handles_non_object_status() {
        let mut metrics = TransactionMetrics::default();

        apply_txpool_status(&mut metrics, &Value::Null);

        assert_eq!(metrics.txpool_pending, None);
        assert_eq!(metrics.txpool_queued, None);
    }
}
