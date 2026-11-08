
use std::time::Duration;

use thiserror::Error;

use crate::collectors::blockchain::BlockchainCollector;
use crate::collectors::network::NetworkCollector;
use crate::collectors::p2p::P2pCollector;
use crate::collectors::rpc::RpcCollector;
use crate::collectors::system::SystemCollector;

use crate::config::Config;
use crate::geth::client::GethClient;
use crate::models::observation::Observation;
use crate::storage::jsonl::{JsonlStorage, JsonlStorageError};

/// Errors raised by the monitoring scheduler.
#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Storage error: {0}")]
    Storage(#[from] JsonlStorageError),
}

/// Coordinates all monitoring collectors and JSONL persistence.
pub struct MonitoringScheduler<'a> {
    node_id: String,
    collection_interval: Duration,

    blockchain_collector: BlockchainCollector<'a>,
    p2p_collector: P2pCollector<'a>,
    rpc_collector: RpcCollector<'a>,

    network_collector: NetworkCollector,
    system_collector: SystemCollector,

    storage: JsonlStorage,
}

impl<'a> MonitoringScheduler<'a> {
    pub fn new(
        client: &'a GethClient,
        config: &Config,
        node_id: impl Into<String>,
    ) -> Result<Self, SchedulerError> {
        let storage = JsonlStorage::new(&config.output_file)
            .map_err(JsonlStorageError::Io)?;

        Ok(Self {
            node_id: node_id.into(),
            collection_interval: config.collection_interval,

            blockchain_collector: BlockchainCollector::new(client),
            p2p_collector: P2pCollector::new(client),
            rpc_collector: RpcCollector::new(client),

            network_collector: NetworkCollector::new(),
            system_collector: SystemCollector::new(),

            storage,
        })
    }

    /// Execute one complete monitoring cycle.
    pub async fn collect_once(
        &mut self,
    ) -> Result<Observation, SchedulerError> {
        let mut observation =
            Observation::new(&self.node_id, self.collection_interval);

        // 1. Blockchain metrics
        match self.blockchain_collector.collect().await {
            Ok(metrics) => observation.blockchain = metrics,
            Err(error) => eprintln!("Blockchain collector error: {error}"),
        }

        // 2. P2P metrics
        match self.p2p_collector.collect().await {
            Ok(metrics) => observation.p2p = metrics,
            Err(error) => eprintln!("P2P collector error: {error}"),
        }

        // 3. RPC metrics
        observation.rpc = self.rpc_collector.collect().await;

        // 4. Network metrics
        match self.network_collector.collect() {
            Ok(metrics) => observation.network = metrics,
            Err(error) => eprintln!("Network collector error: {error}"),
        }

        // 5. System metrics
        match self.system_collector.collect() {
            Ok(metrics) => observation.system = metrics,
            Err(error) => eprintln!("System collector error: {error}"),
        }

        // 6. Persist observation
        self.storage.append(&observation)?;

        Ok(observation)
    }

    /// Start continuous monitoring.
    pub async fn run(&mut self) -> Result<(), SchedulerError> {
        let mut ticker = tokio::time::interval(self.collection_interval);

        ticker.set_missed_tick_behavior(
            tokio::time::MissedTickBehavior::Skip,
        );

        loop {
            ticker.tick().await;

            match self.collect_once().await {
                Ok(observation) => {
                    println!(
                        "Monitoring cycle completed: node={}, timestamp={}",
                        observation.node_id,
                        observation.timestamp
                    );
                }
                Err(error) => {
                    eprintln!("Monitoring cycle failed: {error}");
                    return Err(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::observation::Observation;

    #[test]
    fn test_scheduler_type_exists() {
        // Compile-time verification of the Observation type.
        let _observation: Option<Observation> = None;

        // Verify that the scheduler type is accessible.
        let _scheduler: Option<MonitoringScheduler<'static>> = None;
    }
}
