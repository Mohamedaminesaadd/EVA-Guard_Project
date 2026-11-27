use super::{fetch_latest_block, hex_field};
use crate::clients::geth_client::GethClient;
use crate::models::metrics::BlockchainMetrics;

use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Collects Ethereum blockchain progression and synchronization metrics.
pub struct BlockchainCollector;

impl BlockchainCollector {
    /// Collects blockchain metrics from one Geth node.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(client: &GethClient) -> BlockchainMetrics {
        let mut metrics = BlockchainMetrics::default();

        // 1. Read number and timestamp from one single `latest` block,
        //    so both values always describe the same block.
        if let Some(block) = fetch_latest_block(client).await {
            apply_head_block(&mut metrics, &block, current_unix_secs());
        }

        // 2. Determine whether Geth is synchronized.
        //    `eth_syncing` returns false (None here) when not syncing.
        if let Ok(sync_result) = client.syncing_status().await {
            metrics.is_synced = Some(sync_result.value.is_none());
        }

        metrics
    }
}

/// Fills block number and block age from a block object.
///
/// `now_secs` is injected so the logic is deterministic and testable.
fn apply_head_block(metrics: &mut BlockchainMetrics, block: &Value, now_secs: Option<u64>) {
    metrics.block_number = hex_field(block, "number");

    metrics.latest_block_age_secs = hex_field(block, "timestamp")
        .zip(now_secs)
        .and_then(|(timestamp, now)| block_age_secs(now, timestamp));
}

/// Returns the current Unix time in seconds.
fn current_unix_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

/// Calculates the age of a block in seconds.
///
/// Returns None if the block timestamp is in the future
/// (local clock behind the network).
fn block_age_secs(now_secs: u64, block_timestamp: u64) -> Option<u64> {
    now_secs.checked_sub(block_timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    const BLOCK_TIMESTAMP: u64 = 1_700_000_000;

    #[test]
    fn parses_block_timestamp() {
        let block = json!({ "timestamp": "0x6553f100" });

        assert_eq!(hex_field(&block, "timestamp"), Some(BLOCK_TIMESTAMP));
    }

    #[test]
    fn computes_block_age() {
        assert_eq!(
            block_age_secs(BLOCK_TIMESTAMP + 10, BLOCK_TIMESTAMP),
            Some(10)
        );
    }

    #[test]
    fn block_age_is_zero_for_current_block() {
        assert_eq!(block_age_secs(BLOCK_TIMESTAMP, BLOCK_TIMESTAMP), Some(0));
    }

    #[test]
    fn future_block_timestamp_returns_none() {
        assert_eq!(block_age_secs(BLOCK_TIMESTAMP, BLOCK_TIMESTAMP + 100), None);
    }

    #[test]
    fn current_unix_secs_is_plausible() {
        assert!(current_unix_secs().unwrap() > BLOCK_TIMESTAMP);
    }

    #[test]
    fn applies_number_and_age_from_head_block() {
        let block = json!({
            "number": "0x10",
            "timestamp": "0x6553f100"
        });
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &block, Some(BLOCK_TIMESTAMP + 7));

        assert_eq!(metrics.block_number, Some(16));
        assert_eq!(metrics.latest_block_age_secs, Some(7));
    }

    #[test]
    fn keeps_number_when_clock_is_unavailable() {
        let block = json!({
            "number": "0x10",
            "timestamp": "0x6553f100"
        });
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &block, None);

        assert_eq!(metrics.block_number, Some(16));
        assert_eq!(metrics.latest_block_age_secs, None);
    }

    #[test]
    fn keeps_number_when_timestamp_is_in_the_future() {
        let block = json!({
            "number": "0x10",
            "timestamp": "0x6553f100"
        });
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &block, Some(BLOCK_TIMESTAMP - 1));

        assert_eq!(metrics.block_number, Some(16));
        assert_eq!(metrics.latest_block_age_secs, None);
    }

    #[test]
    fn rejects_invalid_timestamp_but_keeps_number() {
        let block = json!({
            "number": "0x10",
            "timestamp": "0xxyz"
        });
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &block, Some(BLOCK_TIMESTAMP));

        assert_eq!(metrics.block_number, Some(16));
        assert_eq!(metrics.latest_block_age_secs, None);
    }

    #[test]
    fn handles_block_without_fields() {
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &json!({}), Some(BLOCK_TIMESTAMP));

        assert_eq!(metrics.block_number, None);
        assert_eq!(metrics.latest_block_age_secs, None);
    }

    #[test]
    fn handles_non_object_block() {
        let mut metrics = BlockchainMetrics::default();

        apply_head_block(&mut metrics, &Value::Null, Some(BLOCK_TIMESTAMP));

        assert_eq!(metrics.block_number, None);
        assert_eq!(metrics.latest_block_age_secs, None);
    }
}
