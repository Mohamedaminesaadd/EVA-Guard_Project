//! Collection of ONE node set (Geth + Beacon Node + Validator Client).
//!
//! A [`NodeCollector`] runs, for one node, every collector of the three
//! components **concurrently** and returns a [`NodeSnapshot`]: the metrics of
//! the three components for one cycle. It owns the collectors that keep state
//! between two cycles (peer churn, request rate), so it must be built once
//! and reused for the whole run, never rebuilt per cycle.
//!
//! # Time budget
//!
//! Every collector is bounded by `budget`. A collector that exceeds it is
//! cancelled and its metrics stay `None` ("not measured"), while the other
//! collectors of the node keep their answer: a single hanging endpoint
//! cannot blank a whole node, and cannot make the cycle overrun. A collector
//! cancelled this way does not update its own state, so the next peer churn
//! or request rate is computed over the last cycle that did complete.
//!
//! # What is NOT collected here
//!
//! - `beacon.propagation` stays `None`: it compares the heads of **all** the
//!   Beacon Nodes, so the scheduler computes it once per cycle and merges it.
//!
//! # System metrics
//!
//! With a [`NodeSystemCollector`] attached ([`NodeCollector::with_system`]),
//! the CPU, memory, network and TCP metrics of the three containers of the
//! node are collected concurrently with the application metrics, under the
//! same budget. Each observation receives the metrics of its own container
//! (see `system_collection`). Without one, the `system` groups stay `null`.
//!
//! [`NodeSource`] is the seam the scheduler works through, which keeps the
//! scheduling logic testable without any network.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::future::BoxFuture;
use thiserror::Error;

use crate::clients::beacon_client::BeaconClient;
use crate::clients::validator_client::PrometheusMetric;
use crate::collectors::beacon::{
    AttestationInclusionCollector, ConnectionsCollector as PeerConnectionsCollector,
    ConsensusCollector, DiversityCollector, PeersCollector, SyncCollector,
};
use crate::collectors::geth::block_execution::BlockExecutionCollector;
use crate::collectors::geth::blockchain::BlockchainCollector;
use crate::collectors::geth::p2p::P2pCollector;
use crate::collectors::geth::rpc::RpcCollector;
use crate::collectors::geth::transactions::TransactionCollector;
use crate::collectors::validator::{
    AttestationsCollector, DutiesCollector, EffectivenessCollector, MissedAttestationsCollector,
    ProposalsCollector,
};
use crate::models::observation::{
    BeaconMetrics, GethMetrics, MissedProposalMetrics, Observation, Scenario, ValidatorMetrics,
};
use crate::monitoring::node_manager::ManagedNode;
use crate::monitoring::system_collection::{ComponentSystems, NodeSystemCollector};

/// The metrics of the three components of one node, for one cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSnapshot {
    pub node_id: String,
    pub geth: GethMetrics,
    pub beacon: BeaconMetrics,
    pub validator: ValidatorMetrics,
    /// The resources of each component's own container.
    pub system: ComponentSystems,
}

impl NodeSnapshot {
    /// A snapshot where nothing was measured (every metric is `None`).
    ///
    /// Used when a node could not be collected at all in time.
    pub fn unmeasured(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            geth: GethMetrics::default(),
            beacon: BeaconMetrics::default(),
            validator: ValidatorMetrics::default(),
            system: ComponentSystems::default(),
        }
    }

    /// The three observations of the cycle, in the order geth, beacon,
    /// validator, all stamped with the same `timestamp`. Each one carries
    /// the `system` group of its own container.
    pub fn into_observations(
        self,
        timestamp: DateTime<Utc>,
        scenario: Scenario,
        interval: Duration,
    ) -> [Observation; 3] {
        let Self {
            node_id,
            geth,
            beacon,
            validator,
            system,
        } = self;

        let with_system = |mut observation: Observation, metrics| {
            observation.system = metrics;
            observation
        };

        [
            with_system(
                Observation::new_at(timestamp, node_id.clone(), scenario, interval, geth),
                system.geth,
            ),
            with_system(
                Observation::new_at(timestamp, node_id.clone(), scenario, interval, beacon),
                system.beacon,
            ),
            with_system(
                Observation::new_at(timestamp, node_id, scenario, interval, validator),
                system.validator,
            ),
        ]
    }
}

/// What the scheduler needs from a node: an id, a collection, and the Beacon
/// client used for the cross-node propagation measurement.
pub trait NodeSource: Send + Sync + 'static {
    /// The node id (`nodes.toml` `id`).
    fn node_id(&self) -> &str;

    /// Collects the node for one cycle. Never fails: whatever could not be
    /// measured is `None`.
    fn collect(&self) -> BoxFuture<'_, NodeSnapshot>;

    /// The Beacon API client of the node, if it takes part in the
    /// block propagation comparison.
    fn beacon_client(&self) -> Option<&BeaconClient> {
        None
    }
}

/// Errors raised while building a [`NodeCollector`].
#[derive(Debug, Error)]
pub enum NodeCollectorError {
    #[error("node `{node}`: HTTP client initialization failed: {source}")]
    Http {
        node: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("node `{node}`: the collector time budget must be greater than zero")]
    ZeroBudget { node: String },
}

/// Collects one node set. See the module documentation.
pub struct NodeCollector {
    node: Arc<ManagedNode>,
    budget: Duration,

    // Geth: the two collectors that compare two consecutive cycles.
    p2p: P2pCollector,
    rpc: RpcCollector,

    // Beacon Node: one stateful collector, two that own an HTTP client.
    peer_connections: PeerConnectionsCollector,
    diversity: DiversityCollector,
    attestation_inclusion: AttestationInclusionCollector,

    // Containers: CPU, memory, network, TCP (stateful: network rates).
    system: Option<NodeSystemCollector>,
}

impl NodeCollector {
    /// Builds the collectors of one node.
    ///
    /// `request_timeout_secs` bounds one HTTP request, `budget` bounds one
    /// collector (which may chain several requests).
    pub fn new(
        node: Arc<ManagedNode>,
        request_timeout_secs: u64,
        budget: Duration,
    ) -> Result<Self, NodeCollectorError> {
        if budget.is_zero() {
            return Err(NodeCollectorError::ZeroBudget {
                node: node.node_id.clone(),
            });
        }

        let http = |source| NodeCollectorError::Http {
            node: node.node_id.clone(),
            source,
        };

        let diversity = DiversityCollector::new(request_timeout_secs).map_err(http)?;
        let attestation_inclusion =
            AttestationInclusionCollector::new(request_timeout_secs).map_err(http)?;

        Ok(Self {
            budget,
            p2p: P2pCollector::new(),
            rpc: RpcCollector::new(),
            peer_connections: PeerConnectionsCollector::new(),
            diversity,
            attestation_inclusion,
            node,
            system: None,
        })
    }

    /// Attaches the collector of the node's container metrics.
    pub fn with_system(mut self, system: NodeSystemCollector) -> Self {
        self.system = Some(system);
        self
    }

    /// The collector time budget.
    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// Collects the three components of the node, and their containers,
    /// concurrently.
    pub async fn collect_snapshot(&self) -> NodeSnapshot {
        let (geth, beacon, validator, system) = tokio::join!(
            self.collect_geth(),
            self.collect_beacon(),
            self.collect_validator(),
            self.collect_system(),
        );

        NodeSnapshot {
            node_id: self.node.node_id.clone(),
            geth,
            beacon,
            validator,
            system,
        }
    }

    /// The three containers; `null` groups when no Docker is attached or the
    /// budget runs out.
    async fn collect_system(&self) -> ComponentSystems {
        match &self.system {
            Some(system) => self.bounded("system", system.collect()).await,
            None => ComponentSystems::default(),
        }
    }

    async fn collect_geth(&self) -> GethMetrics {
        let client = &self.node.geth;

        let (p2p, transactions, rpc, blockchain, block_execution) = tokio::join!(
            self.bounded("geth.p2p", self.p2p.collect(client)),
            self.bounded("geth.transactions", TransactionCollector::collect(client)),
            self.bounded("geth.rpc", self.rpc.collect(client)),
            self.bounded("geth.blockchain", BlockchainCollector::collect(client)),
            self.bounded(
                "geth.block_execution",
                BlockExecutionCollector::collect(client)
            ),
        );

        GethMetrics {
            p2p,
            transactions,
            rpc,
            blockchain,
            block_execution,
        }
    }

    /// Everything except the propagation, which needs the other nodes.
    async fn collect_beacon(&self) -> BeaconMetrics {
        let client = &self.node.beacon;

        let (peers, connections, diversity, consensus, sync, attestation_inclusion) = tokio::join!(
            self.bounded("beacon.peers", PeersCollector::collect(client)),
            self.bounded("beacon.connections", self.peer_connections.collect(client)),
            self.bounded("beacon.diversity", self.diversity.collect(client)),
            self.bounded("beacon.consensus", ConsensusCollector::collect(client)),
            self.bounded("beacon.sync", SyncCollector::collect(client)),
            self.bounded(
                "beacon.attestation_inclusion",
                self.attestation_inclusion.collect(client)
            ),
        );

        BeaconMetrics {
            peers,
            connections,
            diversity,
            consensus,
            sync,
            attestation_inclusion,
            ..BeaconMetrics::default()
        }
    }

    /// One Prometheus scrape feeds every Validator collector.
    async fn collect_validator(&self) -> ValidatorMetrics {
        let client = &self.node.validator;

        self.bounded("validator", async {
            match client.collect_metrics().await {
                Ok(samples) => validator_metrics_from_samples(&samples),
                Err(_) => ValidatorMetrics::default(),
            }
        })
        .await
    }

    /// Runs `work` within the budget; on timeout, returns "not measured".
    async fn bounded<T: Default>(
        &self,
        collector: &'static str,
        work: impl Future<Output = T>,
    ) -> T {
        bounded(&self.node.node_id, collector, self.budget, work).await
    }
}

impl NodeSource for NodeCollector {
    fn node_id(&self) -> &str {
        &self.node.node_id
    }

    fn collect(&self) -> BoxFuture<'_, NodeSnapshot> {
        Box::pin(self.collect_snapshot())
    }

    fn beacon_client(&self) -> Option<&BeaconClient> {
        Some(&self.node.beacon)
    }
}

/// Runs `work` for at most `budget`; on timeout returns `T::default()`
/// (every metric `None`) and logs which collector was cut.
async fn bounded<T: Default>(
    node_id: &str,
    collector: &'static str,
    budget: Duration,
    work: impl Future<Output = T>,
) -> T {
    match tokio::time::timeout(budget, work).await {
        Ok(value) => value,
        Err(_elapsed) => {
            tracing::warn!(
                node = node_id,
                collector,
                budget_ms = budget.as_millis() as u64,
                "collector exceeded its time budget: metrics left unmeasured"
            );
            T::default()
        }
    }
}

/// Builds the Validator metrics from one Prometheus scrape.
///
/// `missed_proposals` is never measured: the Lighthouse Validator Client does
/// not expose it (see `MissedAttestationsCollector` for the same limit).
pub(crate) fn validator_metrics_from_samples(samples: &[PrometheusMetric]) -> ValidatorMetrics {
    ValidatorMetrics {
        attestations: AttestationsCollector::from_metrics(samples),
        missed_attestations: MissedAttestationsCollector::from_metrics(samples),
        block_proposals: ProposalsCollector::from_metrics(samples),
        missed_proposals: MissedProposalMetrics::default(),
        duties: DutiesCollector::from_metrics(samples),
        effectiveness: EffectivenessCollector::from_metrics(samples),
    }
}

/// Fake nodes served by wiremock, shared by the tests of the monitoring layer.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::config::NodesConfig;

    use serde_json::{json, Value};
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Which components of a [`FakeNode`] answer. A component that does not
    /// answer returns HTTP 404 on every route.
    #[derive(Clone, Copy)]
    pub(crate) struct Alive {
        pub geth: bool,
        pub beacon: bool,
        pub validator: bool,
    }

    impl Alive {
        pub(crate) const ALL: Alive = Alive {
            geth: true,
            beacon: true,
            validator: true,
        };
    }

    /// A TCP port on which nothing listens (connections are refused).
    fn closed_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    /// The `[[nodes]]` block of a node whose three components refuse every
    /// connection.
    pub(crate) fn refused_toml(id: &str) -> String {
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
            closed_port(),
            closed_port(),
            closed_port(),
            closed_port(),
            closed_port(),
        )
    }

    /// One node set made of three mock servers.
    pub(crate) struct FakeNode {
        pub geth: MockServer,
        pub beacon: MockServer,
        pub validator: MockServer,
    }

    impl FakeNode {
        /// A healthy node whose Beacon head is `head_root`.
        pub(crate) async fn healthy(head_root: &str) -> Self {
            Self::start(head_root, Alive::ALL).await
        }

        pub(crate) async fn start(head_root: &str, alive: Alive) -> Self {
            let node = Self {
                geth: MockServer::start().await,
                beacon: MockServer::start().await,
                validator: MockServer::start().await,
            };

            if alive.geth {
                node.mount_geth().await;
            }
            if alive.beacon {
                node.mount_beacon(head_root).await;
            }
            if alive.validator {
                node.mount_validator().await;
            }

            node
        }

        async fn rpc(&self, rpc_method: &str, result: Value) {
            Mock::given(method("POST"))
                .and(body_partial_json(json!({ "method": rpc_method })))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": result,
                })))
                .mount(&self.geth)
                .await;
        }

        async fn mount_geth(&self) {
            self.rpc("net_peerCount", json!("0x8")).await;
            self.rpc("net_listening", json!(true)).await;
            self.rpc(
                "admin_peers",
                json!([{ "id": "peer-a" }, { "id": "peer-b" }]),
            )
            .await;
            self.rpc(
                "txpool_status",
                json!({ "pending": "0x10", "queued": "0x2" }),
            )
            .await;
            self.rpc("eth_blockNumber", json!("0x4b0")).await;
            self.rpc("eth_syncing", json!(false)).await;
            self.rpc(
                "eth_getBlockByNumber",
                json!({ "number": "0x4b0", "timestamp": "0x1", "transactions": ["0xa", "0xb"] }),
            )
            .await;

            Mock::given(method("GET"))
                .and(path("/debug/metrics/prometheus"))
                .respond_with(ResponseTemplate::new(200).set_body_string(
                    "# TYPE rpc_requests counter\nrpc_requests 1000\nrpc_failure 5\n",
                ))
                .mount(&self.geth)
                .await;
        }

        async fn get(&self, route: &str, body: Value) {
            Mock::given(method("GET"))
                .and(path(route))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": body })))
                .mount(&self.beacon)
                .await;
        }

        async fn mount_beacon(&self, head_root: &str) {
            self.get(
                "/eth/v1/node/peer_count",
                json!({ "connected": "8", "connecting": "1", "disconnected": "3", "disconnecting": "0" }),
            )
            .await;
            self.get(
                "/eth/v1/node/peers",
                json!([
                    { "state": "connected", "last_seen_p2p_address": "/ip4/1.2.3.4/tcp/9000" },
                    { "state": "connected", "last_seen_p2p_address": "/ip4/5.6.7.8/tcp/9000" },
                    { "state": "disconnected", "last_seen_p2p_address": "/ip4/9.9.9.9/tcp/9000" },
                ]),
            )
            .await;
            self.get(
                "/eth/v1/node/syncing",
                json!({ "head_slot": "1200", "sync_distance": "0", "is_syncing": false,
                        "is_optimistic": false, "el_offline": false }),
            )
            .await;
            self.get(
                "/eth/v1/beacon/headers/head",
                json!({ "root": head_root, "canonical": true,
                        "header": { "message": { "slot": "1200" } } }),
            )
            .await;
            self.get(
                "/eth/v1/beacon/states/head/finality_checkpoints",
                json!({ "previous_justified": { "epoch": "36", "root": "0x1" },
                        "current_justified": { "epoch": "37", "root": "0x2" },
                        "finalized": { "epoch": "35", "root": "0x3" } }),
            )
            .await;

            // Attestations included 1 and 2 slots late: average distance 1.5.
            Mock::given(method("GET"))
                .and(path("/eth/v2/beacon/blocks/1200"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data": { "message": { "body": { "attestations": [
                        { "data": { "slot": "1199" } },
                        { "data": { "slot": "1198" } },
                    ] } } }
                })))
                .mount(&self.beacon)
                .await;
        }

        async fn mount_validator(&self) {
            Mock::given(method("GET"))
                .and(path("/metrics"))
                .respond_with(ResponseTemplate::new(200).set_body_string(
                    "vc_validators_total_count 4\n\
                     vc_validators_enabled_count 3\n\
                     vc_beacon_nodes_available_count 1\n\
                     vc_signed_attestations_total{status=\"success\"} 120\n\
                     vc_signed_attestations_total{status=\"same_data\"} 1\n\
                     vc_signed_beacon_blocks_total{status=\"success\"} 3\n\
                     vc_beacon_attester_count{task=\"current_epoch\"} 4\n\
                     vc_beacon_block_proposer_count{task=\"current_epoch\"} 1\n",
                ))
                .mount(&self.validator)
                .await;
        }

        /// The `[[nodes]]` block of this node.
        pub(crate) fn toml(&self, id: &str) -> String {
            format!(
                r#"
[[nodes]]
id = "{id}"

[nodes.geth]
rpc_url = "{geth}"
metrics_url = "{geth}/debug/metrics/prometheus"

[nodes.beacon]
api_url = "{beacon}"
metrics_url = "{beacon}/metrics"

[nodes.validator]
metrics_url = "{validator}/metrics"
"#,
                geth = self.geth.uri(),
                beacon = self.beacon.uri(),
                validator = self.validator.uri(),
            )
        }

        /// The clients of this node.
        pub(crate) fn managed(&self, id: &str, request_timeout_secs: u64) -> Arc<ManagedNode> {
            let config = NodesConfig::from_toml_str(&self.toml(id)).unwrap();

            Arc::new(ManagedNode::new(&config.nodes[0], request_timeout_secs).unwrap())
        }

        /// A collector for this node.
        pub(crate) fn collector(&self, id: &str, budget: Duration) -> NodeCollector {
            NodeCollector::new(self.managed(id, 5), 5, budget).unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{refused_toml, Alive, FakeNode};
    use super::*;
    use crate::collectors::validator::test_support::lighthouse_scrape;

    use std::time::Instant;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    const BUDGET: Duration = Duration::from_secs(4);

    fn stamp() -> DateTime<Utc> {
        "2026-11-16T08:00:05Z".parse().unwrap()
    }

    // ---- Pure parts ----

    #[test]
    fn an_unmeasured_snapshot_serializes_to_nulls_only() {
        let snapshot = NodeSnapshot::unmeasured("node-02");

        for observation in snapshot.into_observations(stamp(), Scenario::Normal, BUDGET) {
            let json = serde_json::to_value(&observation).unwrap();
            let metrics = json["metrics"].as_object().unwrap();

            assert!(!metrics.is_empty());
            assert!(
                metrics.values().all(Value::is_null),
                "{}: {metrics:?}",
                observation.component()
            );
            assert!(json["system"]
                .as_object()
                .unwrap()
                .values()
                .all(Value::is_null));
        }
    }

    use serde_json::Value;

    #[test]
    fn a_snapshot_gives_three_observations_sharing_one_stamp() {
        let mut snapshot = NodeSnapshot::unmeasured("node-03");
        snapshot.geth.p2p.peer_count = Some(8);
        snapshot.beacon.sync.head_slot = Some(1200);
        snapshot.validator.duties.validators_total = Some(4);

        let observations =
            snapshot.into_observations(stamp(), Scenario::Eclipse, Duration::from_secs(5));

        let components: Vec<_> = observations.iter().map(Observation::component).collect();
        assert_eq!(
            components.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            ["geth", "beacon", "validator"]
        );

        for observation in &observations {
            assert_eq!(observation.timestamp, stamp());
            assert_eq!(observation.node_id, "node-03");
            assert_eq!(observation.scenario, Scenario::Eclipse);
            assert_eq!(observation.collection_interval, 5);
        }

        let json = serde_json::to_value(&observations[0]).unwrap();
        assert_eq!(json["metrics"]["peer_count"], 8);
        let json = serde_json::to_value(&observations[1]).unwrap();
        assert_eq!(json["metrics"]["head_slot"], 1200);
        let json = serde_json::to_value(&observations[2]).unwrap();
        assert_eq!(json["metrics"]["validators_total"], 4);
    }

    fn system_of(cpu: f64, memory: u64) -> crate::models::metrics::SystemMetrics {
        crate::models::metrics::SystemMetrics {
            cpu_usage_percent: Some(cpu),
            memory_used_mb: Some(memory),
            memory_total_mb: Some(31_793),
            network: crate::models::metrics::NetworkMetrics {
                network_rx_bytes_total: Some(48_000_000),
                network_tx_bytes_total: Some(14_000_000),
                network_rx_bytes_per_sec: Some(18_490.5),
                network_tx_bytes_per_sec: Some(0.0),
                tcp_established_connections: Some(5),
            },
        }
    }

    fn snapshot_with_systems() -> NodeSnapshot {
        let mut snapshot = NodeSnapshot::unmeasured("node-01");
        snapshot.system = ComponentSystems {
            geth: system_of(1.5, 106),
            beacon: system_of(14.8, 322),
            validator: system_of(0.2, 30),
        };
        snapshot
    }

    #[test]
    fn each_observation_carries_the_system_metrics_of_its_own_container() {
        let [geth, beacon, validator] =
            snapshot_with_systems().into_observations(stamp(), Scenario::Normal, BUDGET);

        assert_eq!(geth.system, system_of(1.5, 106));
        assert_eq!(beacon.system, system_of(14.8, 322));
        assert_eq!(validator.system, system_of(0.2, 30));
    }

    #[test]
    fn the_jsonl_dataset_contains_the_measured_system_metrics() {
        use crate::collectors::container::test_support::TempDir;
        use crate::storage::JsonlStorage;

        let dir = TempDir::new("system-jsonl");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        for observation in
            snapshot_with_systems().into_observations(stamp(), Scenario::Normal, BUDGET)
        {
            storage.append(&observation).unwrap();
        }
        storage.flush().unwrap();

        let content = std::fs::read_to_string(storage.scenario_path(Scenario::Normal)).unwrap();
        let lines: Vec<Value> = content
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(lines.len(), 3);
        for line in &lines {
            let system = line["system"].as_object().unwrap();
            assert_eq!(system.len(), 8, "{system:?}");
            assert!(system.values().all(|value| !value.is_null()), "{system:?}");
            // A measured zero stays a zero, distinct from null.
            assert_eq!(line["system"]["network_tx_bytes_per_sec"], 0.0);
        }
        assert_eq!(lines[0]["component"], "geth");
        assert_eq!(lines[0]["system"]["memory_used_mb"], 106);
        assert_eq!(lines[1]["component"], "beacon");
        assert_eq!(lines[1]["system"]["memory_used_mb"], 322);
        assert_eq!(lines[2]["component"], "validator");
        assert_eq!(lines[2]["system"]["memory_used_mb"], 30);
    }

    #[tokio::test]
    async fn without_docker_the_system_groups_stay_null() {
        let fake = FakeNode::healthy("0xaaa").await;

        let snapshot = fake.collector("node-01", BUDGET).collect_snapshot().await;

        assert_eq!(snapshot.system, ComponentSystems::default());
    }

    #[test]
    fn one_scrape_feeds_every_validator_collector() {
        let metrics = validator_metrics_from_samples(&lighthouse_scrape());

        assert_eq!(metrics.attestations.attestations_total, Some(120));
        assert_eq!(metrics.attestations.attestation_failures_total, Some(1));
        assert_eq!(metrics.block_proposals.blocks_proposed_total, Some(3));
        assert_eq!(metrics.duties.validators_total, Some(4));
        assert_eq!(metrics.duties.validators_enabled, Some(3));
        assert_eq!(metrics.duties.attester_duties, Some(4));
        assert_eq!(metrics.duties.proposer_duties, Some(1));
        assert_eq!(metrics.duties.beacon_node_online, Some(true));
    }

    #[test]
    fn what_lighthouse_does_not_expose_stays_unmeasured() {
        let metrics = validator_metrics_from_samples(&lighthouse_scrape());

        assert_eq!(metrics.missed_attestations.missed_attestations_total, None);
        assert_eq!(
            metrics.missed_attestations.missed_attestations_last_epoch,
            None
        );
        assert_eq!(metrics.missed_proposals.missed_proposals_total, None);
        assert_eq!(metrics.effectiveness.attestation_effectiveness, None);
        assert_eq!(metrics.effectiveness.proposal_effectiveness, None);
    }

    // ---- The time budget (virtual time) ----

    #[tokio::test(start_paused = true)]
    async fn bounded_returns_the_value_of_work_that_fits_the_budget() {
        let started = tokio::time::Instant::now();

        let value = bounded("node-01", "test", Duration::from_secs(4), async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Some(7_u64)
        })
        .await;

        assert_eq!(value, Some(7));
        assert_eq!(started.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_cuts_work_at_the_budget_and_returns_the_default() {
        let started = tokio::time::Instant::now();

        let value = bounded("node-01", "test", Duration::from_secs(4), async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Some(7_u64)
        })
        .await;

        assert_eq!(value, None);
        assert_eq!(started.elapsed(), Duration::from_secs(4));
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_work_is_really_cancelled_not_just_ignored() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let finished = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&finished);

        let _ = bounded("node-01", "test", Duration::from_secs(1), async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            flag.store(true, Ordering::SeqCst);
            0_u64
        })
        .await;

        tokio::time::sleep(Duration::from_secs(10)).await;
        assert!(!finished.load(Ordering::SeqCst));
    }

    #[test]
    fn a_zero_budget_is_rejected() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let node = runtime.block_on(async {
            let fake = FakeNode::start("0xroot", Alive::ALL).await;
            fake.managed("node-01", 5)
        });

        let error = NodeCollector::new(node, 5, Duration::ZERO)
            .err()
            .expect("zero budget must be rejected");

        assert!(matches!(&error, NodeCollectorError::ZeroBudget { node } if node == "node-01"));
    }

    // ---- Against real clients and fake servers ----

    #[tokio::test]
    async fn a_healthy_node_fills_the_three_components() {
        let fake = FakeNode::healthy("0xaaa").await;
        let collector = fake.collector("node-01", BUDGET);

        let snapshot = collector.collect_snapshot().await;

        assert_eq!(snapshot.node_id, "node-01");

        // Geth.
        let geth = &snapshot.geth;
        assert_eq!(geth.p2p.peer_count, Some(8));
        assert_eq!(geth.p2p.admin_peer_count, Some(2));
        assert_eq!(geth.p2p.peer_details_available, Some(true));
        assert_eq!(geth.p2p.network_listening, Some(true));
        assert_eq!(geth.transactions.txpool_pending, Some(16));
        assert_eq!(geth.transactions.txpool_queued, Some(2));
        assert_eq!(geth.rpc.rpc_available, Some(true));
        assert!(geth.rpc.rpc_latency_ms.is_some());
        assert_eq!(geth.rpc.rpc_requests_total, Some(1000));
        assert_eq!(geth.rpc.rpc_failed_requests_total, Some(5));
        assert_eq!(geth.blockchain.block_number, Some(1200));
        assert_eq!(geth.blockchain.is_synced, Some(true));
        assert_eq!(geth.block_execution.block_tx_count, Some(2));

        // Beacon Node.
        let beacon = &snapshot.beacon;
        assert_eq!(beacon.peers.peer_count, Some(8));
        assert_eq!(beacon.peers.connecting_peers, Some(1));
        assert_eq!(beacon.diversity.unique_peer_ips, Some(2));
        assert_eq!(beacon.consensus.finalized_epoch, Some(35));
        assert_eq!(beacon.consensus.justified_epoch, Some(37));
        assert_eq!(beacon.sync.head_slot, Some(1200));
        assert_eq!(
            beacon.attestation_inclusion.attestation_inclusion_distance,
            Some(1.5)
        );

        // Validator Client.
        let validator = &snapshot.validator;
        assert_eq!(validator.attestations.attestations_total, Some(120));
        assert_eq!(validator.block_proposals.blocks_proposed_total, Some(3));
        assert_eq!(validator.duties.validators_total, Some(4));
        assert_eq!(validator.duties.beacon_node_online, Some(true));
    }

    #[tokio::test]
    async fn propagation_is_left_to_the_scheduler() {
        let fake = FakeNode::healthy("0xaaa").await;

        let snapshot = fake.collector("node-01", BUDGET).collect_snapshot().await;

        assert_eq!(snapshot.beacon.propagation, Default::default());
    }

    #[tokio::test]
    async fn a_component_that_errors_leaves_the_others_untouched() {
        let fake = FakeNode::start(
            "0xaaa",
            Alive {
                geth: false,
                beacon: true,
                validator: true,
            },
        )
        .await;

        let snapshot = fake.collector("node-01", BUDGET).collect_snapshot().await;

        // Measured as down: `Some(false)`, which is information.
        assert_eq!(snapshot.geth.rpc.rpc_available, Some(false));
        // Not measurable: `None`, never 0.
        assert_eq!(snapshot.geth.p2p.peer_count, None);
        assert_eq!(snapshot.geth.blockchain.block_number, None);

        assert_eq!(snapshot.beacon.sync.head_slot, Some(1200));
        assert_eq!(snapshot.validator.duties.validators_total, Some(4));
    }

    #[tokio::test]
    async fn an_unreachable_node_is_all_unmeasured_except_the_availability_flags() {
        // Nothing listens on any of the five ports: connections are refused.
        let config = crate::config::NodesConfig::from_toml_str(&refused_toml("node-01")).unwrap();
        let node = Arc::new(ManagedNode::new(&config.nodes[0], 1).unwrap());
        let collector = NodeCollector::new(node, 1, BUDGET).unwrap();

        let snapshot = collector.collect_snapshot().await;

        // `Some(false)` is a measurement ("the RPC is down"); the rest is
        // `None`, never 0.
        let mut expected = NodeSnapshot::unmeasured("node-01");
        expected.geth.rpc.rpc_available = Some(false);
        expected.geth.p2p.peer_details_available = Some(false);
        assert_eq!(snapshot, expected);
    }

    #[tokio::test]
    async fn a_hanging_endpoint_is_cut_at_the_budget_and_nothing_else_is_lost() {
        let fake = FakeNode::healthy("0xaaa").await;

        // `peer_count` never answers in time; it feeds two collectors.
        fake.beacon.reset().await;
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/peer_count"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&fake.beacon)
            .await;
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/syncing"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "head_slot": "1200", "sync_distance": "0", "is_syncing": false }
            })))
            .mount(&fake.beacon)
            .await;

        let collector =
            NodeCollector::new(fake.managed("node-01", 60), 60, Duration::from_millis(300))
                .unwrap();

        let started = Instant::now();
        let snapshot = collector.collect_snapshot().await;

        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(snapshot.beacon.peers.peer_count, None);
        assert_eq!(snapshot.beacon.connections.peer_churn_count, None);
        assert_eq!(snapshot.beacon.sync.head_slot, Some(1200));
        assert_eq!(snapshot.geth.p2p.peer_count, Some(8));
        assert_eq!(snapshot.validator.duties.validators_total, Some(4));
    }

    #[tokio::test]
    async fn state_survives_between_cycles_so_rates_and_churn_appear() {
        let fake = FakeNode::healthy("0xaaa").await;
        let collector = fake.collector("node-01", BUDGET);

        let first = collector.collect_snapshot().await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        let second = collector.collect_snapshot().await;

        // A rate or a churn needs two samples.
        assert_eq!(first.geth.rpc.rpc_requests_per_sec, None);
        assert_eq!(first.geth.p2p.peer_churn_count, None);
        assert_eq!(first.beacon.connections.peer_churn_count, None);

        assert_eq!(second.geth.rpc.rpc_requests_per_sec, Some(0.0));
        assert_eq!(second.geth.p2p.peer_churn_count, Some(0));
        assert_eq!(second.beacon.connections.peer_churn_count, Some(0));
    }

    #[tokio::test]
    async fn two_nodes_keep_separate_state() {
        let first_fake = FakeNode::healthy("0xaaa").await;
        let second_fake = FakeNode::healthy("0xaaa").await;
        let first = first_fake.collector("node-01", BUDGET);
        let second = second_fake.collector("node-02", BUDGET);

        // Two cycles for the first node, only one for the second.
        first.collect_snapshot().await;
        let first_again = first.collect_snapshot().await;
        let second_once = second.collect_snapshot().await;

        assert_eq!(first_again.beacon.connections.peer_churn_count, Some(0));
        assert_eq!(second_once.beacon.connections.peer_churn_count, None);
    }

    #[tokio::test]
    async fn the_node_source_exposes_the_id_and_the_beacon_client() {
        let fake = FakeNode::healthy("0xaaa").await;
        let collector = fake.collector("node-07", BUDGET);
        let source: &dyn NodeSource = &collector;

        assert_eq!(source.node_id(), "node-07");
        assert_eq!(source.beacon_client().unwrap().api_url(), fake.beacon.uri());
        assert_eq!(source.collect().await.node_id, "node-07");
    }

    #[test]
    fn errors_name_the_node() {
        let error = NodeCollectorError::ZeroBudget {
            node: "node-04".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "node `node-04`: the collector time budget must be greater than zero"
        );
    }

    #[test]
    fn a_node_source_default_has_no_beacon_client() {
        struct Bare;
        impl NodeSource for Bare {
            fn node_id(&self) -> &str {
                "bare"
            }
            fn collect(&self) -> BoxFuture<'_, NodeSnapshot> {
                Box::pin(async { NodeSnapshot::unmeasured("bare") })
            }
        }

        assert!(Bare.beacon_client().is_none());
    }
}
