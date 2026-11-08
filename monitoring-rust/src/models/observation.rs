//! Observation data model (Phase 4).
//!
//! One `Observation` = one JSONL line. The JSON is FLAT (no nested objects) and
//! every key is always present, so the schema is stable for the Python
//! Intelligence Engine.
//!
//! Rules:
//! - Every measurement is an `Option`. `None` is written as `null` and means
//!   "not measured"; a missing measurement is never replaced by `0` or `false`.
//! - Non-finite floats (NaN, infinity) are also written as `null` by serde_json.
//! - The Rust groups (`P2pMetrics`, ...) exist so that each collector fills its
//!   own group; `#[serde(flatten)]` merges them into the single flat JSON object.

use std::time::Duration;

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};

/// One complete snapshot of the node and its host for a single collection cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// UTC time of the collection cycle (RFC 3339; whole seconds when built by `new`).
    pub timestamp: DateTime<Utc>,
    /// Identifier of the monitored node.
    pub node_id: String,
    /// Configured delay between two collection cycles, in seconds.
    pub collection_interval: u64,

    #[serde(flatten)]
    pub p2p: P2pMetrics,
    #[serde(flatten)]
    pub blockchain: BlockchainMetrics,
    #[serde(flatten)]
    pub rpc: RpcMetrics,
    #[serde(flatten)]
    pub network: NetworkMetrics,
    #[serde(flatten)]
    pub system: SystemMetrics,
}

impl Observation {
    /// New observation stamped with the current time (truncated to whole seconds)
    /// and with no measurement filled in yet.
    pub fn new(node_id: impl Into<String>, collection_interval: Duration) -> Self {
        let now = Utc::now();
        Self {
            timestamp: now.with_nanosecond(0).unwrap_or(now),
            node_id: node_id.into(),
            collection_interval: collection_interval.as_secs(),
            p2p: P2pMetrics::default(),
            blockchain: BlockchainMetrics::default(),
            rpc: RpcMetrics::default(),
            network: NetworkMetrics::default(),
            system: SystemMetrics::default(),
        }
    }
}

/// Ethereum P2P activity (filled by the P2P collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct P2pMetrics {
    /// Peers reported by `net_peerCount`.
    pub peer_count: Option<u64>,
    /// Number of entries returned by `admin_peers` (`None` if unavailable).
    pub admin_peer_count: Option<u64>,
    /// `Some(true)` if `admin_peers` answered, `Some(false)` if the method is
    /// unavailable, `None` if it could not be determined (e.g. RPC unreachable).
    pub peer_details_available: Option<bool>,
    /// Peers present now but not in the previous cycle (`None` on the first cycle).
    pub new_peers: Option<u64>,
    /// Peers present in the previous cycle but not now (`None` on the first cycle).
    pub disconnected_peers: Option<u64>,
    /// `new_peers + disconnected_peers` (`None` on the first cycle).
    pub peer_churn_count: Option<u64>,
    /// `net_listening`.
    pub network_listening: Option<bool>,
}

/// Blockchain progression and synchronization (filled by the blockchain collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockchainMetrics {
    /// Latest block number (`eth_blockNumber`).
    pub block_number: Option<u64>,
    /// Interpreted synchronization state (`None` if `eth_syncing` failed).
    pub is_synced: Option<bool>,
}

/// JSON-RPC availability and responsiveness (filled by the RPC collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RpcMetrics {
    /// `Some(true/false)` when the RPC collector ran, `None` otherwise.
    pub rpc_available: Option<bool>,
    /// End-to-end latency of a lightweight request, in milliseconds
    /// (`None` when the request failed).
    pub rpc_latency_ms: Option<f64>,
}

/// Host network activity, loopback excluded (filled by the network collector).
/// Host-wide values: they are not specific to Geth.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub network_rx_bytes_total: Option<u64>,
    pub network_tx_bytes_total: Option<u64>,
    /// `None` on the first cycle (a rate needs two samples) or after a counter reset.
    pub network_rx_bytes_per_sec: Option<f64>,
    /// `None` on the first cycle (a rate needs two samples) or after a counter reset.
    pub network_tx_bytes_per_sec: Option<f64>,
    /// System-wide established TCP connections (not only Geth's).
    pub tcp_established_connections: Option<u64>,
}

/// Host resource consumption (filled by the system collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Global CPU usage, 0-100.
    pub cpu_usage_percent: Option<f64>,
    /// Used memory in MiB (1 MiB = 1024 * 1024 bytes).
    pub memory_used_mb: Option<u64>,
    /// Total memory in MiB (1 MiB = 1024 * 1024 bytes).
    pub memory_total_mb: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::BTreeSet;

    /// The JSON contract shared with the Python Intelligence Engine.
    const EXPECTED_KEYS: [&str; 22] = [
        "timestamp",
        "node_id",
        "collection_interval",
        "peer_count",
        "admin_peer_count",
        "peer_details_available",
        "new_peers",
        "disconnected_peers",
        "peer_churn_count",
        "network_listening",
        "block_number",
        "is_synced",
        "rpc_available",
        "rpc_latency_ms",
        "network_rx_bytes_total",
        "network_tx_bytes_total",
        "network_rx_bytes_per_sec",
        "network_tx_bytes_per_sec",
        "tcp_established_connections",
        "cpu_usage_percent",
        "memory_used_mb",
        "memory_total_mb",
    ];

    fn full_observation() -> Observation {
        Observation {
            timestamp: "2026-11-08T12:00:05Z".parse().unwrap(),
            node_id: "geth-test".to_string(),
            collection_interval: 5,
            p2p: P2pMetrics {
                peer_count: Some(10),
                admin_peer_count: Some(10),
                peer_details_available: Some(true),
                new_peers: Some(1),
                disconnected_peers: Some(2),
                peer_churn_count: Some(3),
                network_listening: Some(true),
            },
            blockchain: BlockchainMetrics {
                block_number: Some(1201),
                is_synced: Some(true),
            },
            rpc: RpcMetrics {
                rpc_available: Some(true),
                rpc_latency_ms: Some(12.5),
            },
            network: NetworkMetrics {
                network_rx_bytes_total: Some(1_000_000),
                network_tx_bytes_total: Some(2_000_000),
                network_rx_bytes_per_sec: Some(1500.25),
                network_tx_bytes_per_sec: Some(300.5),
                tcp_established_connections: Some(42),
            },
            system: SystemMetrics {
                cpu_usage_percent: Some(37.5),
                memory_used_mb: Some(4096),
                memory_total_mb: Some(16384),
            },
        }
    }

    fn key_set(value: &Value) -> BTreeSet<&str> {
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn full_observation_round_trips_through_json() {
        let original = full_observation();
        let line = serde_json::to_string(&original).unwrap();
        println!("{line}");
        let decoded: Observation = serde_json::from_str(&line).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn json_is_flat_and_has_exactly_the_contract_keys() {
        let value = serde_json::to_value(full_observation()).unwrap();
        let expected: BTreeSet<&str> = EXPECTED_KEYS.into_iter().collect();
        assert_eq!(key_set(&value), expected);
        assert!(value.as_object().unwrap().values().all(|v| !v.is_object()));
    }

    #[test]
    fn new_observation_has_no_measurements_and_all_keys() {
        let observation = Observation::new("geth-test", Duration::from_secs(5));
        assert_eq!(observation.collection_interval, 5);
        assert_eq!(observation.node_id, "geth-test");

        let value = serde_json::to_value(&observation).unwrap();
        let expected: BTreeSet<&str> = EXPECTED_KEYS.into_iter().collect();
        assert_eq!(key_set(&value), expected);

        // Everything except the three metadata keys is null: missing is never 0.
        for key in EXPECTED_KEYS.iter().skip(3) {
            assert!(value[key].is_null(), "{key} should be null");
        }
    }

    #[test]
    fn missing_measurements_round_trip_as_none() {
        let original = Observation::new("geth-test", Duration::from_secs(5));
        let line = serde_json::to_string(&original).unwrap();
        let decoded: Observation = serde_json::from_str(&line).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(decoded.p2p.peer_count, None);
        assert_eq!(decoded.rpc.rpc_latency_ms, None);
    }

    #[test]
    fn absent_measurement_keys_deserialize_as_none() {
        let line = r#"{"timestamp":"2026-11-08T12:00:00Z","node_id":"n1","collection_interval":5}"#;
        let decoded: Observation = serde_json::from_str(line).unwrap();
        assert_eq!(decoded.p2p, P2pMetrics::default());
        assert_eq!(decoded.blockchain, BlockchainMetrics::default());
        assert_eq!(decoded.rpc, RpcMetrics::default());
        assert_eq!(decoded.network, NetworkMetrics::default());
        assert_eq!(decoded.system, SystemMetrics::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let mut value = serde_json::to_value(full_observation()).unwrap();
        value["field_added_by_a_newer_version"] = json!(123);
        let decoded: Observation = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, full_observation());
    }

    #[test]
    fn metadata_is_required() {
        assert!(serde_json::from_str::<Observation>(r#"{"peer_count":10}"#).is_err());
    }

    #[test]
    fn timestamp_is_rfc3339_utc() {
        let line = serde_json::to_string(&full_observation()).unwrap();
        assert!(
            line.contains(r#""timestamp":"2026-11-08T12:00:05Z""#),
            "{line}"
        );

        let fresh = serde_json::to_value(Observation::new("n", Duration::from_secs(1))).unwrap();
        let text = fresh["timestamp"].as_str().unwrap();
        assert!(text.ends_with('Z') && !text.contains('.'), "{text}");
    }

    #[test]
    fn serialized_line_never_contains_a_raw_newline() {
        let mut observation = full_observation();
        observation.node_id = "line1\nline2".to_string();
        let line = serde_json::to_string(&observation).unwrap();
        assert!(!line.contains('\n'));
        let decoded: Observation = serde_json::from_str(&line).unwrap();
        assert_eq!(decoded.node_id, "line1\nline2");
    }

    #[test]
    fn non_finite_floats_are_written_as_null() {
        let mut observation = full_observation();
        observation.system.cpu_usage_percent = Some(f64::NAN);
        observation.rpc.rpc_latency_ms = Some(f64::INFINITY);
        let value = serde_json::to_value(&observation).unwrap();
        assert!(value["cpu_usage_percent"].is_null());
        assert!(value["rpc_latency_ms"].is_null());
    }
}
