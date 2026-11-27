//! Typed metric groups (Phase 2).
//!
//! Each component (Geth, Lighthouse Beacon Node, Lighthouse Validator Client)
//! has its own metrics struct, built from small groups: one group per
//! collector, so that a collector only fills the group it owns.
//!
//! Rules (shared with the JSON contract of the Python Intelligence Engine):
//! - Every measurement is an `Option`. `None` is written as `null` and means
//!   "not measured"; a missing measurement is never replaced by `0` or `false`.
//! - Non-finite floats (NaN, infinity) are also written as `null` by serde_json.
//! - Inside a component, the JSON is FLAT: `#[serde(flatten)]` merges the groups
//!   into one object, and every key is always present.
//! - Adding a field later is backward compatible: a key absent from old data
//!   deserializes as `None`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Geth
// ---------------------------------------------------------------------------

/// All Geth measurements of one collection cycle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GethMetrics {
    #[serde(flatten)]
    pub p2p: P2pMetrics,
    #[serde(flatten)]
    pub transactions: TransactionMetrics,
    #[serde(flatten)]
    pub rpc: RpcMetrics,
    #[serde(flatten)]
    pub blockchain: BlockchainMetrics,
    #[serde(flatten)]
    pub block_execution: BlockExecutionMetrics,
}

/// Ethereum P2P activity (P2P collector).
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

/// Transaction pool (transactions collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TransactionMetrics {
    /// Executable transactions waiting for inclusion (`txpool_status`).
    pub txpool_pending: Option<u64>,
    /// Transactions not yet executable, e.g. nonce gap (`txpool_status`).
    pub txpool_queued: Option<u64>,
}

/// JSON-RPC availability, responsiveness and load (RPC collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RpcMetrics {
    /// `Some(true/false)` when the RPC collector ran, `None` otherwise.
    pub rpc_available: Option<bool>,
    /// End-to-end latency of a lightweight request, in milliseconds
    /// (`None` when the request failed).
    pub rpc_latency_ms: Option<f64>,
    /// Cumulative number of JSON-RPC requests served (Prometheus counter).
    pub rpc_requests_total: Option<u64>,
    /// Cumulative number of failed JSON-RPC requests (Prometheus counter).
    pub rpc_failed_requests_total: Option<u64>,
    /// Request rate between two cycles; `None` on the first cycle or after a
    /// counter reset (a rate needs two samples).
    pub rpc_requests_per_sec: Option<f64>,
}

/// Blockchain progression and synchronization (blockchain collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockchainMetrics {
    /// Latest block number (`eth_blockNumber`).
    pub block_number: Option<u64>,
    /// Interpreted synchronization state (`None` if `eth_syncing` failed).
    pub is_synced: Option<bool>,
    /// Seconds between the latest block's timestamp and the collection time.
    pub latest_block_age_secs: Option<u64>,
}

/// Block processing (block execution collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockExecutionMetrics {
    /// Time spent executing the latest block, in milliseconds.
    pub block_execution_ms: Option<f64>,
    /// Number of transactions in the latest block.
    pub block_tx_count: Option<u64>,
}

// ---------------------------------------------------------------------------
// Lighthouse Beacon Node
// ---------------------------------------------------------------------------

/// All Beacon Node measurements of one collection cycle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BeaconMetrics {
    #[serde(flatten)]
    pub peers: PeerMetrics,
    #[serde(flatten)]
    pub connections: PeerConnectionMetrics,
    #[serde(flatten)]
    pub diversity: PeerDiversityMetrics,
    #[serde(flatten)]
    pub consensus: ConsensusMetrics,
    #[serde(flatten)]
    pub sync: SyncMetrics,
    #[serde(flatten)]
    pub propagation: BlockPropagationMetrics,
    #[serde(flatten)]
    pub attestation_inclusion: AttestationInclusionMetrics,
}

/// Peer counts (peers collector; `/eth/v1/node/peer_count`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PeerMetrics {
    /// Connected peers.
    pub peer_count: Option<u64>,
    /// Peers currently being dialed.
    pub connecting_peers: Option<u64>,
}

/// Peer connection direction and churn (peer connections collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PeerConnectionMetrics {
    /// Connected peers that dialed us.
    pub inbound_peers: Option<u64>,
    /// Connected peers that we dialed.
    pub outbound_peers: Option<u64>,
    /// Peers present now but not in the previous cycle (`None` on the first cycle).
    pub new_peers: Option<u64>,
    /// Peers present in the previous cycle but not now (`None` on the first cycle).
    pub disconnected_peers: Option<u64>,
    /// `new_peers + disconnected_peers` (`None` on the first cycle).
    pub peer_churn_count: Option<u64>,
}

/// How varied the connected peers are (peer diversity collector).
/// A low diversity is a typical eclipse-attack signal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PeerDiversityMetrics {
    /// Number of distinct IP addresses among connected peers.
    pub unique_peer_ips: Option<u64>,
    /// Number of distinct client implementations among connected peers.
    pub unique_peer_clients: Option<u64>,
    /// Share (0-1) of the most common client among connected peers.
    pub top_client_ratio: Option<f64>,
}

/// Consensus health: justification and finality (consensus collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsensusMetrics {
    pub finalized_epoch: Option<u64>,
    pub justified_epoch: Option<u64>,
    /// Current epoch minus finalized epoch (healthy chain: about 2).
    pub finality_distance_epochs: Option<u64>,
    /// Cumulative number of observed chain reorganizations.
    pub reorgs_total: Option<u64>,
}

/// Synchronization state (synchronization collector; `/eth/v1/node/syncing`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncMetrics {
    pub head_slot: Option<u64>,
    /// Slots between the node's head and the network head.
    pub sync_distance: Option<u64>,
    pub is_syncing: Option<bool>,
    /// The head block was imported before its execution payload was verified.
    pub is_optimistic: Option<bool>,
    /// The Beacon Node reports its execution layer (Geth) as offline.
    pub el_offline: Option<bool>,
}

/// Block gossip timing (block propagation collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockPropagationMetrics {
    /// Average delay between the start of a slot and the arrival of its block,
    /// in milliseconds.
    pub block_propagation_delay_ms: Option<f64>,
}

/// Attestation inclusion (attestation inclusion collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttestationInclusionMetrics {
    /// Average number of slots between an attestation's slot and its inclusion.
    pub attestation_inclusion_distance: Option<f64>,
}

// ---------------------------------------------------------------------------
// Lighthouse Validator Client
// ---------------------------------------------------------------------------

/// All Validator Client measurements of one collection cycle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidatorMetrics {
    #[serde(flatten)]
    pub attestations: AttestationMetrics,
    #[serde(flatten)]
    pub missed_attestations: MissedAttestationMetrics,
    #[serde(flatten)]
    pub block_proposals: BlockProposalMetrics,
    #[serde(flatten)]
    pub missed_proposals: MissedProposalMetrics,
    #[serde(flatten)]
    pub duties: ValidatorDutiesMetrics,
    #[serde(flatten)]
    pub effectiveness: EffectivenessMetrics,
}

/// Attestations produced by the validators (attestation collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttestationMetrics {
    /// Cumulative number of attestations successfully signed and published.
    pub attestations_total: Option<u64>,
    /// Cumulative number of attestations that failed.
    pub attestation_failures_total: Option<u64>,
}

/// Attestations that were due but not produced (missed attestation collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MissedAttestationMetrics {
    pub missed_attestations_total: Option<u64>,
    pub missed_attestations_last_epoch: Option<u64>,
}

/// Blocks proposed by the validators (block proposal collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockProposalMetrics {
    pub blocks_proposed_total: Option<u64>,
    pub block_proposal_failures_total: Option<u64>,
}

/// Proposals that were due but not produced (missed proposal collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MissedProposalMetrics {
    pub missed_proposals_total: Option<u64>,
}

/// Validator set and duties (validator duties collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidatorDutiesMetrics {
    pub validators_total: Option<u64>,
    pub validators_enabled: Option<u64>,
    /// Attester duties scheduled for the current epoch.
    pub attester_duties: Option<u64>,
    /// Proposer duties scheduled for the current epoch.
    pub proposer_duties: Option<u64>,
    /// The Validator Client can reach its Beacon Node.
    pub beacon_node_online: Option<bool>,
}

/// Derived performance ratios (validator effectiveness collector).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectivenessMetrics {
    /// Successful attestations / expected attestations, 0-1.
    pub attestation_effectiveness: Option<f64>,
    /// Successful proposals / expected proposals, 0-1.
    pub proposal_effectiveness: Option<f64>,
}

// ---------------------------------------------------------------------------
// System (common to every component)
// ---------------------------------------------------------------------------

/// Resource consumption of the container that runs the monitored component
/// (system collector), attached to every observation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage of the container, Docker convention: 100.0 is one fully used
    /// CPU, so the value can exceed 100 on a multi-core container.
    pub cpu_usage_percent: Option<f64>,
    /// Memory used by the container in MiB (1 MiB = 1024 * 1024 bytes), the
    /// inactive file cache excluded as in `docker stats`.
    pub memory_used_mb: Option<u64>,
    /// Memory limit of the container in MiB (1 MiB = 1024 * 1024 bytes).
    /// Docker reports the host memory when no limit is configured.
    pub memory_total_mb: Option<u64>,
    #[serde(flatten)]
    pub network: NetworkMetrics,
}

/// Network activity of the container: all the interfaces Docker reports,
/// added together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub network_rx_bytes_total: Option<u64>,
    pub network_tx_bytes_total: Option<u64>,
    /// `None` on the first cycle (a rate needs two samples) or after a counter reset.
    pub network_rx_bytes_per_sec: Option<f64>,
    /// `None` on the first cycle (a rate needs two samples) or after a counter reset.
    pub network_tx_bytes_per_sec: Option<f64>,
    /// Established TCP connections (IPv4 and IPv6) in the network namespace of
    /// the container.
    pub tcp_established_connections: Option<u64>,
}
