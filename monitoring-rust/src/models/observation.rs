//! Observation data model (Phase 2).
//!
//! One `Observation` = one JSONL line = the metrics of ONE component (Geth,
//! Lighthouse Beacon Node or Validator Client) of ONE node, for one collection
//! cycle. `scenario` is present for labelled training data and omitted for live detection.
//!
//! JSON layout in training mode (the `scenario` key is omitted in live mode):
//!
//! ```json
//! {
//!   "timestamp": "2026-11-16T08:00:05Z",
//!   "node_id": "node-01",
//!   "scenario": "NORMAL",
//!   "collection_interval": 5,
//!   "component": "geth",
//!   "metrics": { "peer_count": 8, "block_number": 1200, "...": null },
//!   "system":  { "cpu_usage_percent": 37.5, "...": null }
//! }
//! ```
//!
//! - `component` and `metrics` come from the same enum variant
//!   ([`ComponentMetrics`]), so a `geth` observation can never carry beacon
//!   metrics.
//! - `metrics` is a flat object whose keys depend on the component.
//! - A measurement that is not available is `null`, never `0` or `false`.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// The metric groups used to live in this module: keep the old import paths valid.
pub use super::metrics::*;

/// Experimental scenario during which an observation was collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Scenario {
    /// No malicious behavior.
    Normal,
    /// Controlled isolation of a node.
    Eclipse,
    /// Controlled RPC overload.
    RpcFlooding,
}

impl Scenario {
    pub const ALL: [Scenario; 3] = [Scenario::Normal, Scenario::Eclipse, Scenario::RpcFlooding];

    /// Value written in the JSON (`NORMAL`, `ECLIPSE`, `RPC_FLOODING`).
    pub fn as_str(self) -> &'static str {
        match self {
            Scenario::Normal => "NORMAL",
            Scenario::Eclipse => "ECLIPSE",
            Scenario::RpcFlooding => "RPC_FLOODING",
        }
    }

    /// Name of the dataset file stem (`data/<dataset_name>.jsonl`).
    pub fn dataset_name(self) -> &'static str {
        match self {
            Scenario::Normal => "normal",
            Scenario::Eclipse => "eclipse",
            Scenario::RpcFlooding => "rpc_flooding",
        }
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when a string is not a known scenario.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown scenario `{0}` (expected NORMAL, ECLIPSE or RPC_FLOODING)")]
pub struct ParseScenarioError(pub String);

impl FromStr for Scenario {
    type Err = ParseScenarioError;

    /// Case-insensitive; `-` is accepted in place of `_` (`rpc-flooding`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_uppercase().replace('-', "_");
        Scenario::ALL
            .into_iter()
            .find(|scenario| scenario.as_str() == normalized)
            .ok_or_else(|| ParseScenarioError(s.trim().to_string()))
    }
}

/// Monitored component of a node set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Geth,
    Beacon,
    Validator,
}

impl Component {
    pub const ALL: [Component; 3] = [Component::Geth, Component::Beacon, Component::Validator];

    /// Value written in the JSON (`geth`, `beacon`, `validator`).
    pub fn as_str(self) -> &'static str {
        match self {
            Component::Geth => "geth",
            Component::Beacon => "beacon",
            Component::Validator => "validator",
        }
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The metrics of one component. Serialized as the two keys `component`
/// (variant name) and `metrics` (variant content).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "component", content = "metrics", rename_all = "lowercase")]
pub enum ComponentMetrics {
    Geth(GethMetrics),
    Beacon(BeaconMetrics),
    Validator(ValidatorMetrics),
}

impl ComponentMetrics {
    /// Metrics of `component` with no measurement filled in yet.
    pub fn empty(component: Component) -> Self {
        match component {
            Component::Geth => Self::Geth(GethMetrics::default()),
            Component::Beacon => Self::Beacon(BeaconMetrics::default()),
            Component::Validator => Self::Validator(ValidatorMetrics::default()),
        }
    }

    pub fn component(&self) -> Component {
        match self {
            Self::Geth(_) => Component::Geth,
            Self::Beacon(_) => Component::Beacon,
            Self::Validator(_) => Component::Validator,
        }
    }
}

impl From<GethMetrics> for ComponentMetrics {
    fn from(metrics: GethMetrics) -> Self {
        Self::Geth(metrics)
    }
}

impl From<BeaconMetrics> for ComponentMetrics {
    fn from(metrics: BeaconMetrics) -> Self {
        Self::Beacon(metrics)
    }
}

impl From<ValidatorMetrics> for ComponentMetrics {
    fn from(metrics: ValidatorMetrics) -> Self {
        Self::Validator(metrics)
    }
}

/// One snapshot of one component and its host for a single collection cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// UTC time of the collection cycle (RFC 3339; whole seconds when built by `new`).
    pub timestamp: DateTime<Utc>,
    /// Identifier of the monitored node set (`nodes.toml` `id`).
    pub node_id: String,
    /// Experimental scenario in progress. None in live detection mode.
    /// The field is omitted from JSON when no label is available.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scenario: Option<Scenario>,
    /// Configured delay between two collection cycles, in seconds.
    pub collection_interval: u64,
    /// `component` + `metrics` keys.
    #[serde(flatten)]
    pub metrics: ComponentMetrics,
    /// Host resource consumption.
    pub system: SystemMetrics,
}

impl Observation {
    /// New observation stamped with the current time (truncated to whole seconds)
    /// and with an empty `system` group.
    ///
    /// The Geth, Beacon and Validator observations of one cycle should share
    /// one timestamp so that they can be joined: read the clock once per cycle
    /// and use [`Observation::new_at`] for each component.
    pub fn new(
        node_id: impl Into<String>,
        scenario: impl Into<Option<Scenario>>,
        collection_interval: Duration,
        metrics: impl Into<ComponentMetrics>,
    ) -> Self {
        Self::new_at(Utc::now(), node_id, scenario, collection_interval, metrics)
    }

    /// Same as [`Observation::new`] with an explicit time, truncated to whole
    /// seconds like the current time is.
    pub fn new_at(
        timestamp: DateTime<Utc>,
        node_id: impl Into<String>,
        scenario: impl Into<Option<Scenario>>,
        collection_interval: Duration,
        metrics: impl Into<ComponentMetrics>,
    ) -> Self {
        Self {
            timestamp: timestamp.with_nanosecond(0).unwrap_or(timestamp),
            node_id: node_id.into(),
            scenario: scenario.into(),
            collection_interval: collection_interval.as_secs(),
            metrics: metrics.into(),
            system: SystemMetrics::default(),
        }
    }

    pub fn component(&self) -> Component {
        self.metrics.component()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};
    use std::collections::BTreeSet;

    // ---- The JSON contract shared with the Python Intelligence Engine ----

    const TOP_KEYS: [&str; 7] = [
        "timestamp",
        "node_id",
        "scenario",
        "collection_interval",
        "component",
        "metrics",
        "system",
    ];

    const GETH_KEYS: [&str; 19] = [
        "peer_count",
        "admin_peer_count",
        "peer_details_available",
        "new_peers",
        "disconnected_peers",
        "peer_churn_count",
        "network_listening",
        "txpool_pending",
        "txpool_queued",
        "rpc_available",
        "rpc_latency_ms",
        "rpc_requests_total",
        "rpc_failed_requests_total",
        "rpc_requests_per_sec",
        "block_number",
        "is_synced",
        "latest_block_age_secs",
        "block_execution_ms",
        "block_tx_count",
    ];

    const BEACON_KEYS: [&str; 21] = [
        "peer_count",
        "connecting_peers",
        "inbound_peers",
        "outbound_peers",
        "new_peers",
        "disconnected_peers",
        "peer_churn_count",
        "unique_peer_ips",
        "unique_peer_clients",
        "top_client_ratio",
        "finalized_epoch",
        "justified_epoch",
        "finality_distance_epochs",
        "reorgs_total",
        "head_slot",
        "sync_distance",
        "is_syncing",
        "is_optimistic",
        "el_offline",
        "block_propagation_delay_ms",
        "attestation_inclusion_distance",
    ];

    const VALIDATOR_KEYS: [&str; 14] = [
        "attestations_total",
        "attestation_failures_total",
        "missed_attestations_total",
        "missed_attestations_last_epoch",
        "blocks_proposed_total",
        "block_proposal_failures_total",
        "missed_proposals_total",
        "validators_total",
        "validators_enabled",
        "attester_duties",
        "proposer_duties",
        "beacon_node_online",
        "attestation_effectiveness",
        "proposal_effectiveness",
    ];

    const SYSTEM_KEYS: [&str; 8] = [
        "cpu_usage_percent",
        "memory_used_mb",
        "memory_total_mb",
        "network_rx_bytes_total",
        "network_tx_bytes_total",
        "network_rx_bytes_per_sec",
        "network_tx_bytes_per_sec",
        "tcp_established_connections",
    ];

    fn metric_keys(component: Component) -> &'static [&'static str] {
        match component {
            Component::Geth => &GETH_KEYS,
            Component::Beacon => &BEACON_KEYS,
            Component::Validator => &VALIDATOR_KEYS,
        }
    }

    // ---- Fully populated fixtures: every single field is `Some` ----

    fn full_geth() -> GethMetrics {
        GethMetrics {
            p2p: P2pMetrics {
                peer_count: Some(10),
                admin_peer_count: Some(10),
                peer_details_available: Some(true),
                new_peers: Some(1),
                disconnected_peers: Some(2),
                peer_churn_count: Some(3),
                network_listening: Some(true),
            },
            transactions: TransactionMetrics {
                txpool_pending: Some(120),
                txpool_queued: Some(4),
            },
            rpc: RpcMetrics {
                rpc_available: Some(true),
                rpc_latency_ms: Some(12.5),
                rpc_requests_total: Some(50_000),
                rpc_failed_requests_total: Some(7),
                rpc_requests_per_sec: Some(33.25),
            },
            blockchain: BlockchainMetrics {
                block_number: Some(1201),
                is_synced: Some(true),
                latest_block_age_secs: Some(4),
            },
            block_execution: BlockExecutionMetrics {
                block_execution_ms: Some(85.5),
                block_tx_count: Some(190),
            },
        }
    }

    fn full_beacon() -> BeaconMetrics {
        BeaconMetrics {
            peers: PeerMetrics {
                peer_count: Some(55),
                connecting_peers: Some(2),
            },
            connections: PeerConnectionMetrics {
                inbound_peers: Some(20),
                outbound_peers: Some(35),
                new_peers: Some(3),
                disconnected_peers: Some(1),
                peer_churn_count: Some(4),
            },
            diversity: PeerDiversityMetrics {
                unique_peer_ips: Some(50),
                unique_peer_clients: Some(4),
                top_client_ratio: Some(0.45),
            },
            consensus: ConsensusMetrics {
                finalized_epoch: Some(98),
                justified_epoch: Some(99),
                finality_distance_epochs: Some(2),
                reorgs_total: Some(0),
            },
            sync: SyncMetrics {
                head_slot: Some(3200),
                sync_distance: Some(0),
                is_syncing: Some(false),
                is_optimistic: Some(false),
                el_offline: Some(false),
            },
            propagation: BlockPropagationMetrics {
                block_propagation_delay_ms: Some(850.5),
            },
            attestation_inclusion: AttestationInclusionMetrics {
                attestation_inclusion_distance: Some(1.25),
            },
        }
    }

    fn full_validator() -> ValidatorMetrics {
        ValidatorMetrics {
            attestations: AttestationMetrics {
                attestations_total: Some(640),
                attestation_failures_total: Some(1),
            },
            missed_attestations: MissedAttestationMetrics {
                missed_attestations_total: Some(2),
                missed_attestations_last_epoch: Some(0),
            },
            block_proposals: BlockProposalMetrics {
                blocks_proposed_total: Some(5),
                block_proposal_failures_total: Some(0),
            },
            missed_proposals: MissedProposalMetrics {
                missed_proposals_total: Some(0),
            },
            duties: ValidatorDutiesMetrics {
                validators_total: Some(32),
                validators_enabled: Some(32),
                attester_duties: Some(32),
                proposer_duties: Some(1),
                beacon_node_online: Some(true),
            },
            effectiveness: EffectivenessMetrics {
                attestation_effectiveness: Some(0.997),
                proposal_effectiveness: Some(1.0),
            },
        }
    }

    fn full_system() -> SystemMetrics {
        SystemMetrics {
            cpu_usage_percent: Some(37.5),
            memory_used_mb: Some(4096),
            memory_total_mb: Some(16384),
            network: NetworkMetrics {
                network_rx_bytes_total: Some(1_000_000),
                network_tx_bytes_total: Some(2_000_000),
                network_rx_bytes_per_sec: Some(1500.25),
                network_tx_bytes_per_sec: Some(300.5),
                tcp_established_connections: Some(42),
            },
        }
    }

    fn full_observation(component: Component) -> Observation {
        let metrics = match component {
            Component::Geth => ComponentMetrics::Geth(full_geth()),
            Component::Beacon => ComponentMetrics::Beacon(full_beacon()),
            Component::Validator => ComponentMetrics::Validator(full_validator()),
        };
        Observation {
            timestamp: "2026-11-16T08:00:05Z".parse().unwrap(),
            node_id: "node-01".to_string(),
            scenario: Some(Scenario::Eclipse),
            collection_interval: 5,
            metrics,
            system: full_system(),
        }
    }

    fn keys_of(object: &Map<String, Value>) -> BTreeSet<&str> {
        object.keys().map(String::as_str).collect()
    }

    fn set_of<'a>(keys: &[&'a str]) -> BTreeSet<&'a str> {
        let set: BTreeSet<&str> = keys.iter().copied().collect();
        assert_eq!(set.len(), keys.len(), "duplicate key in the contract list");
        set
    }

    fn to_object(observation: &Observation) -> Map<String, Value> {
        serde_json::to_value(observation)
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    // ---- Contract tests ----

    #[test]
    fn json_has_exactly_the_contract_keys_for_every_component() {
        for component in Component::ALL {
            let object = to_object(&full_observation(component));
            assert_eq!(keys_of(&object), set_of(&TOP_KEYS), "{component}");

            let metrics = object["metrics"].as_object().unwrap();
            assert_eq!(
                keys_of(metrics),
                set_of(metric_keys(component)),
                "metrics of {component}"
            );
            // Flat: no nested object inside `metrics`.
            assert!(metrics.values().all(|v| !v.is_object()), "{component}");

            let system = object["system"].as_object().unwrap();
            assert_eq!(
                keys_of(system),
                set_of(&SYSTEM_KEYS),
                "system of {component}"
            );
            assert!(system.values().all(|v| !v.is_object()));
        }
    }

    #[test]
    fn full_fixtures_leave_no_null_so_every_field_is_covered() {
        for component in Component::ALL {
            let object = to_object(&full_observation(component));
            for section in ["metrics", "system"] {
                for (key, value) in object[section].as_object().unwrap() {
                    assert!(!value.is_null(), "{component}: {section}.{key} is null");
                }
            }
        }
    }

    #[test]
    fn component_key_matches_the_variant() {
        for component in Component::ALL {
            let object = to_object(&full_observation(component));
            assert_eq!(object["component"], json!(component.as_str()));
        }
    }

    #[test]
    fn every_component_round_trips_through_json() {
        for component in Component::ALL {
            let original = full_observation(component);
            let line = serde_json::to_string(&original).unwrap();
            let decoded: Observation = serde_json::from_str(&line).unwrap();
            assert_eq!(decoded, original, "{component}");
            assert_eq!(decoded.component(), component);
        }
    }

    #[test]
    fn new_observation_has_no_measurements_and_all_keys() {
        for component in Component::ALL {
            let observation = Observation::new(
                "node-01",
                Scenario::Normal,
                Duration::from_secs(5),
                ComponentMetrics::empty(component),
            );
            assert_eq!(observation.collection_interval, 5);
            assert_eq!(observation.node_id, "node-01");
            assert_eq!(observation.scenario, Some(Scenario::Normal));
            assert_eq!(observation.component(), component);

            let object = to_object(&observation);
            assert_eq!(keys_of(&object), set_of(&TOP_KEYS));
            // Missing is never 0 or false.
            for section in ["metrics", "system"] {
                let values = object[section].as_object().unwrap();
                assert!(
                    values.values().all(Value::is_null),
                    "{component}: {section} should be all null"
                );
            }
            let metrics = object["metrics"].as_object().unwrap();
            assert_eq!(keys_of(metrics), set_of(metric_keys(component)));
        }
    }

    #[test]
    fn new_at_truncates_to_whole_seconds() {
        let timestamp: DateTime<Utc> = "2026-11-16T08:00:05.987654321Z".parse().unwrap();

        let observation = Observation::new_at(
            timestamp,
            "node-01",
            Scenario::Normal,
            Duration::from_secs(5),
            ComponentMetrics::empty(Component::Geth),
        );

        assert_eq!(
            observation.timestamp,
            "2026-11-16T08:00:05Z".parse::<DateTime<Utc>>().unwrap()
        );
        let line = serde_json::to_string(&observation).unwrap();
        assert!(
            line.contains(r#""timestamp":"2026-11-16T08:00:05Z""#),
            "{line}"
        );
    }

    #[test]
    fn the_observations_of_one_cycle_share_a_timestamp() {
        // 08:00:05.999 and the next call a few ms later would straddle a
        // second boundary with `new`; `new_at` keeps them equal.
        let cycle_start: DateTime<Utc> = "2026-11-16T08:00:05.999Z".parse().unwrap();

        let timestamps: Vec<_> = Component::ALL
            .into_iter()
            .map(|component| {
                Observation::new_at(
                    cycle_start,
                    "node-01",
                    Scenario::Normal,
                    Duration::from_secs(5),
                    ComponentMetrics::empty(component),
                )
                .timestamp
            })
            .collect();

        assert!(timestamps.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn new_is_new_at_with_the_current_time() {
        let before = Utc::now().with_nanosecond(0).unwrap();
        let observation = Observation::new(
            "n",
            Scenario::Normal,
            Duration::from_secs(1),
            ComponentMetrics::empty(Component::Beacon),
        );
        let after = Utc::now();

        assert!(observation.timestamp >= before && observation.timestamp <= after);
        assert_eq!(observation.timestamp.nanosecond(), 0);
        assert_eq!(observation.system, SystemMetrics::default());
    }

    #[test]
    fn new_accepts_a_typed_metrics_struct() {
        let observation = Observation::new(
            "node-02",
            Scenario::RpcFlooding,
            Duration::from_secs(10),
            full_validator(),
        );
        assert_eq!(observation.component(), Component::Validator);
        assert_eq!(observation.collection_interval, 10);
    }

    #[test]
    fn partial_metrics_deserialize_missing_keys_as_none() {
        let line = r#"{"timestamp":"2026-11-16T08:00:00Z","node_id":"n1","scenario":"NORMAL",
            "collection_interval":5,"component":"beacon","metrics":{"peer_count":55},"system":{}}"#;
        let decoded: Observation = serde_json::from_str(line).unwrap();
        let ComponentMetrics::Beacon(beacon) = &decoded.metrics else {
            panic!("expected beacon metrics");
        };
        assert_eq!(beacon.peers.peer_count, Some(55));
        assert_eq!(beacon.sync, SyncMetrics::default());
        assert_eq!(decoded.system, SystemMetrics::default());
    }

    #[test]
    fn live_observation_omits_scenario_and_deserializes_without_it() {
        let observation = Observation::new(
            "node-live",
            None,
            Duration::from_secs(5),
            ComponentMetrics::empty(Component::Geth),
        );

        assert_eq!(observation.scenario, None);

        let value = serde_json::to_value(&observation).unwrap();
        assert!(value.get("scenario").is_none());

        let decoded: Observation = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.scenario, None);
        assert_eq!(decoded.node_id, "node-live");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let mut value = serde_json::to_value(full_observation(Component::Geth)).unwrap();
        value["metrics"]["field_added_by_a_newer_version"] = json!(123);
        value["system"]["another_new_field"] = json!(true);
        let decoded: Observation = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, full_observation(Component::Geth));
    }

    #[test]
    fn metadata_is_required() {
        let valid = serde_json::to_value(full_observation(Component::Geth)).unwrap();
        for key in TOP_KEYS.into_iter().filter(|key| *key != "scenario") {
            let mut value = valid.clone();
            value.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<Observation>(value).is_err(),
                "removing `{key}` should be rejected"
            );
        }
    }

    #[test]
    fn unknown_component_or_scenario_is_rejected() {
        let valid = serde_json::to_value(full_observation(Component::Geth)).unwrap();

        let mut bad_component = valid.clone();
        bad_component["component"] = json!("miner");
        assert!(serde_json::from_value::<Observation>(bad_component).is_err());

        let mut bad_scenario = valid;
        bad_scenario["scenario"] = json!("DDOS");
        assert!(serde_json::from_value::<Observation>(bad_scenario).is_err());
    }

    #[test]
    fn timestamp_is_rfc3339_utc() {
        let line = serde_json::to_string(&full_observation(Component::Geth)).unwrap();
        assert!(
            line.contains(r#""timestamp":"2026-11-16T08:00:05Z""#),
            "{line}"
        );

        let fresh = Observation::new(
            "n",
            Scenario::Normal,
            Duration::from_secs(1),
            ComponentMetrics::empty(Component::Geth),
        );
        let value = serde_json::to_value(fresh).unwrap();
        let text = value["timestamp"].as_str().unwrap();
        assert!(text.ends_with('Z') && !text.contains('.'), "{text}");
    }

    #[test]
    fn serialized_line_never_contains_a_raw_newline() {
        let mut observation = full_observation(Component::Geth);
        observation.node_id = "line1\nline2".to_string();
        let line = serde_json::to_string(&observation).unwrap();
        assert!(!line.contains('\n'));
        let decoded: Observation = serde_json::from_str(&line).unwrap();
        assert_eq!(decoded.node_id, "line1\nline2");
    }

    #[test]
    fn non_finite_floats_are_written_as_null() {
        let mut observation = full_observation(Component::Geth);
        observation.system.cpu_usage_percent = Some(f64::NAN);
        if let ComponentMetrics::Geth(geth) = &mut observation.metrics {
            geth.rpc.rpc_latency_ms = Some(f64::INFINITY);
        }
        let object = to_object(&observation);
        assert!(object["system"]["cpu_usage_percent"].is_null());
        assert!(object["metrics"]["rpc_latency_ms"].is_null());
    }

    // ---- Scenario / Component ----

    #[test]
    fn scenario_serializes_as_uppercase_names() {
        let values: Vec<String> = Scenario::ALL
            .iter()
            .map(|s| {
                serde_json::to_value(s)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(values, ["NORMAL", "ECLIPSE", "RPC_FLOODING"]);
        for scenario in Scenario::ALL {
            assert_eq!(scenario.to_string(), scenario.as_str());
        }
    }

    #[test]
    fn scenario_dataset_names_match_the_data_files() {
        let names: Vec<&str> = Scenario::ALL.iter().map(|s| s.dataset_name()).collect();
        assert_eq!(names, ["normal", "eclipse", "rpc_flooding"]);
    }

    #[test]
    fn scenario_parses_case_insensitively() {
        assert_eq!("normal".parse::<Scenario>(), Ok(Scenario::Normal));
        assert_eq!(" Eclipse ".parse::<Scenario>(), Ok(Scenario::Eclipse));
        assert_eq!(
            "RPC_FLOODING".parse::<Scenario>(),
            Ok(Scenario::RpcFlooding)
        );
        assert_eq!(
            "rpc-flooding".parse::<Scenario>(),
            Ok(Scenario::RpcFlooding)
        );
    }

    #[test]
    fn unknown_scenario_is_rejected() {
        assert_eq!(
            "ddos".parse::<Scenario>(),
            Err(ParseScenarioError("ddos".to_string()))
        );
        assert!("".parse::<Scenario>().is_err());
    }

    #[test]
    fn component_serializes_as_lowercase_names() {
        let values: Vec<String> = Component::ALL
            .iter()
            .map(|c| {
                serde_json::to_value(c)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(values, ["geth", "beacon", "validator"]);
    }
}
