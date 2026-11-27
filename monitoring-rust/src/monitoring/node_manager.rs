//! Registry of the monitored Ethereum nodes.
//!
//! A [`NodeManager`] turns the validated [`NodesConfig`] into one
//! [`ManagedNode`] per `[[nodes]]` entry: the `node_id` and the three clients
//! (Geth, Beacon Node, Validator Client) that talk to that node set. It does
//! not collect anything and does not touch the network: building a client only
//! configures an HTTP client, so a node that is down does not prevent start-up.
//!
//! Nodes are kept in the order of the configuration file, and each one is
//! shared through an [`Arc`] so that the scheduler can hand it to Tokio tasks.

use std::collections::HashSet;
use std::sync::Arc;

use thiserror::Error;

use crate::clients::{
    beacon_client::{BeaconClient, BeaconError},
    geth_client::{GethClient, GethError},
    validator_client::{ValidatorClient, ValidatorError},
};
use crate::config::{NodeConfig, NodesConfig};

/// One Ethereum node set and its associated clients.
#[derive(Debug, Clone)]
pub struct ManagedNode {
    pub node_id: String,
    pub geth: GethClient,
    pub beacon: BeaconClient,
    pub validator: ValidatorClient,
}

impl ManagedNode {
    /// Builds the clients of one node set.
    ///
    /// `request_timeout_secs` is the maximum duration of one HTTP request.
    pub fn new(config: &NodeConfig, request_timeout_secs: u64) -> Result<Self, NodeManagerError> {
        let node = |config: &NodeConfig| config.id.clone();

        let geth = GethClient::new(
            &config.geth.rpc_url,
            &config.geth.metrics_url,
            request_timeout_secs,
        )
        .map_err(|source| NodeManagerError::Geth {
            node: node(config),
            source,
        })?;

        let beacon =
            BeaconClient::new(&config.beacon.api_url, request_timeout_secs).map_err(|source| {
                NodeManagerError::Beacon {
                    node: node(config),
                    source,
                }
            })?;

        let validator = ValidatorClient::new(&config.validator.metrics_url, request_timeout_secs)
            .map_err(|source| NodeManagerError::Validator {
            node: node(config),
            source,
        })?;

        Ok(Self {
            node_id: config.id.clone(),
            geth,
            beacon,
            validator,
        })
    }
}

/// Errors raised during node initialization.
#[derive(Debug, Error)]
pub enum NodeManagerError {
    #[error("node `{node}`: Geth initialization failed: {source}")]
    Geth {
        node: String,
        #[source]
        source: GethError,
    },

    #[error("node `{node}`: Beacon initialization failed: {source}")]
    Beacon {
        node: String,
        #[source]
        source: BeaconError,
    },

    #[error("node `{node}`: Validator initialization failed: {source}")]
    Validator {
        node: String,
        #[source]
        source: ValidatorError,
    },

    #[error("Duplicate node ID: {0}")]
    DuplicateNode(String),
}

/// Manages all Ethereum nodes.
#[derive(Debug)]
pub struct NodeManager {
    /// In configuration order.
    nodes: Vec<Arc<ManagedNode>>,
}

impl NodeManager {
    /// Initialize all nodes from the multi-node configuration.
    ///
    /// Fails on a duplicate node id even if the configuration was built by
    /// hand and did not go through [`NodesConfig::from_toml_str`].
    pub fn new(config: &NodesConfig) -> Result<Self, NodeManagerError> {
        let timeout = config.settings.request_timeout_secs;

        let mut seen: HashSet<&str> = HashSet::new();
        let mut nodes = Vec::with_capacity(config.nodes.len());

        for node_config in &config.nodes {
            if !seen.insert(node_config.id.as_str()) {
                return Err(NodeManagerError::DuplicateNode(node_config.id.clone()));
            }

            nodes.push(Arc::new(ManagedNode::new(node_config, timeout)?));
        }

        Ok(Self { nodes })
    }

    /// Get a node using its ID.
    pub fn get_node(&self, node_id: &str) -> Option<&ManagedNode> {
        self.nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .map(Arc::as_ref)
    }

    /// Return the number of managed nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check whether there are no managed nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over all managed nodes, in configuration order.
    pub fn iter(&self) -> impl Iterator<Item = &ManagedNode> {
        self.nodes.iter().map(Arc::as_ref)
    }

    /// The node ids, in configuration order.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|node| node.node_id.as_str())
    }

    /// The shared nodes, in configuration order.
    pub fn nodes(&self) -> &[Arc<ManagedNode>] {
        &self.nodes
    }

    /// Hands the shared nodes over (to the scheduler's tasks).
    pub fn into_nodes(self) -> Vec<Arc<ManagedNode>> {
        self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// One `[[nodes]]` block; the five ports are `base..base + 5`.
    fn node_toml(id: &str, base: u16) -> String {
        format!(
            r#"
[[nodes]]
id = "{id}"

[nodes.geth]
rpc_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/debug/metrics/prometheus"

[nodes.beacon]
api_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/metrics"

[nodes.validator]
metrics_url = "http://127.0.0.1:{}/metrics"
"#,
            base,
            base + 1,
            base + 2,
            base + 3,
            base + 4
        )
    }

    fn config_of(ids: &[&str]) -> NodesConfig {
        let content: String = ids
            .iter()
            .enumerate()
            .map(|(index, id)| node_toml(id, 9000 + 100 * index as u16))
            .collect();

        NodesConfig::from_toml_str(&content).unwrap()
    }

    #[test]
    fn builds_one_node_per_configured_entry_in_order() {
        let config = config_of(&["node-03", "node-01", "node-02"]);

        let manager = NodeManager::new(&config).unwrap();

        assert_eq!(manager.len(), 3);
        assert!(!manager.is_empty());
        assert_eq!(
            manager.node_ids().collect::<Vec<_>>(),
            ["node-03", "node-01", "node-02"]
        );
        assert_eq!(
            manager
                .iter()
                .map(|n| n.node_id.as_str())
                .collect::<Vec<_>>(),
            ["node-03", "node-01", "node-02"]
        );
    }

    #[test]
    fn every_node_gets_its_own_endpoints() {
        let config = config_of(&["node-01", "node-02"]);
        let manager = NodeManager::new(&config).unwrap();

        let second = manager.get_node("node-02").unwrap();

        assert_eq!(second.geth.rpc_url(), "http://127.0.0.1:9100");
        assert_eq!(
            second.geth.metrics_url(),
            "http://127.0.0.1:9101/debug/metrics/prometheus"
        );
        assert_eq!(second.beacon.api_url(), "http://127.0.0.1:9102");
        assert_eq!(
            second.validator.metrics_url(),
            "http://127.0.0.1:9104/metrics"
        );
    }

    #[test]
    fn get_node_finds_by_id() {
        let manager = NodeManager::new(&config_of(&["node-01", "node-02"])).unwrap();

        assert_eq!(manager.get_node("node-01").unwrap().node_id, "node-01");
        assert!(manager.get_node("node-99").is_none());
        assert!(manager.get_node("").is_none());
    }

    #[test]
    fn duplicate_ids_are_rejected_even_without_config_validation() {
        let mut config = config_of(&["node-01", "node-02"]);
        config.nodes[1].id = "node-01".to_string();

        let error = NodeManager::new(&config).unwrap_err();

        assert!(matches!(&error, NodeManagerError::DuplicateNode(id) if id == "node-01"));
        assert_eq!(error.to_string(), "Duplicate node ID: node-01");
    }

    #[test]
    fn an_empty_configuration_gives_an_empty_manager() {
        let config = NodesConfig {
            settings: Default::default(),
            nodes: Vec::new(),
        };

        let manager = NodeManager::new(&config).unwrap();

        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert_eq!(manager.iter().count(), 0);
    }

    #[test]
    fn building_the_manager_does_not_touch_the_network() {
        // Nothing listens on these ports: start-up must still be instantaneous.
        let started = Instant::now();

        let manager = NodeManager::new(&config_of(&["node-01", "node-02", "node-03"])).unwrap();

        assert_eq!(manager.len(), 3);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn five_nodes_build_five_managed_nodes() {
        let ids = ["node-01", "node-02", "node-03", "node-04", "node-05"];

        let manager = NodeManager::new(&config_of(&ids)).unwrap();

        assert_eq!(manager.len(), 5);
        assert_eq!(manager.node_ids().collect::<Vec<_>>(), ids);
    }

    #[test]
    fn into_nodes_keeps_the_order_and_the_sharing() {
        let manager = NodeManager::new(&config_of(&["node-01", "node-02"])).unwrap();

        let nodes = manager.into_nodes();
        let clone = Arc::clone(&nodes[1]);

        assert_eq!(nodes[0].node_id, "node-01");
        assert_eq!(clone.node_id, "node-02");
        assert_eq!(Arc::strong_count(&nodes[1]), 2);
    }

    #[test]
    fn errors_name_the_node_and_keep_their_source() {
        use std::error::Error as _;

        let error = NodeManagerError::Validator {
            node: "node-04".to_string(),
            source: ValidatorError::InvalidMetrics("bad".to_string()),
        };

        assert_eq!(
            error.to_string(),
            "node `node-04`: Validator initialization failed: Invalid Prometheus metrics format: bad"
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn a_managed_node_is_cheap_to_clone_and_can_move_between_tasks() {
        fn needs<T: Send + Sync + 'static>() {}
        needs::<ManagedNode>();
        needs::<NodeManager>();
    }
}
