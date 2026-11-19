use super::fetch_latest_block;
use crate::clients::geth_client::GethClient;
use crate::models::metrics::BlockExecutionMetrics;

use serde_json::Value;

/// Collects execution-related information about the latest Ethereum block.
pub struct BlockExecutionCollector;

impl BlockExecutionCollector {
    /// Collects metrics for the latest execution block.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(client: &GethClient) -> BlockExecutionMetrics {
        let mut metrics = BlockExecutionMetrics::default();

        // One request: the latest block with transaction hashes only.
        if let Some(block) = fetch_latest_block(client).await {
            apply_block(&mut metrics, &block);
        }

        metrics
    }
}

/// Fills the metrics that can be derived from a block object.
///
/// `eth_getBlockByNumber` does not expose the actual execution duration,
/// so `block_execution_ms` stays None.
fn apply_block(metrics: &mut BlockExecutionMetrics, block: &Value) {
    metrics.block_tx_count = count_transactions(block);
}

/// Counts the transactions listed in a block object.
///
/// Returns None when the `transactions` field is missing or is not an array.
fn count_transactions(block: &Value) -> Option<u64> {
    block
        .get("transactions")
        .and_then(Value::as_array)
        .map(|transactions| transactions.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn counts_block_transactions() {
        let block = json!({
            "transactions": ["0xaaa", "0xbbb", "0xccc"]
        });

        assert_eq!(count_transactions(&block), Some(3));
    }

    #[test]
    fn handles_empty_block() {
        let block = json!({ "transactions": [] });

        assert_eq!(count_transactions(&block), Some(0));
    }

    #[test]
    fn handles_missing_transactions() {
        assert_eq!(count_transactions(&json!({})), None);
    }

    #[test]
    fn handles_non_array_transactions() {
        assert_eq!(count_transactions(&json!({ "transactions": null })), None);
        assert_eq!(count_transactions(&json!({ "transactions": "0x1" })), None);
    }

    #[test]
    fn handles_null_block() {
        assert_eq!(count_transactions(&Value::Null), None);
    }

    #[test]
    fn apply_block_sets_tx_count_and_leaves_execution_time_unset() {
        let block = json!({ "transactions": ["0xaaa", "0xbbb"] });
        let mut metrics = BlockExecutionMetrics::default();

        apply_block(&mut metrics, &block);

        assert_eq!(metrics.block_tx_count, Some(2));
        assert!(metrics.block_execution_ms.is_none());
    }
}
