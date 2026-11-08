use crate::geth::client::{GethClient, GethError, SyncStatus};
use crate::models::observation::BlockchainMetrics;

/// Collects blockchain metrics from a Geth node.
pub struct BlockchainCollector<'a> {
    client: &'a GethClient,
}

impl<'a> BlockchainCollector<'a> {
    /// Creates a new BlockchainCollector.
    pub fn new(client: &'a GethClient) -> Self {
        Self { client }
    }

    /// Collects blockchain metrics.
    pub async fn collect(&self) -> Result<BlockchainMetrics, GethError> {
        // Retrieve the latest block number.
        let block_number = self.client.block_number().await?;

        // Retrieve the synchronization status.
        let sync_status = self.client.syncing().await?;

        // Convert Geth synchronization status into a boolean.
        let is_synced = match sync_status.value {
            SyncStatus::NotSyncing => true,
            SyncStatus::Syncing { .. } => false,
        };

        // Build the metrics structure.
        Ok(BlockchainMetrics {
            block_number: Some(block_number.value),
            is_synced: Some(is_synced),
        })
    }
}
