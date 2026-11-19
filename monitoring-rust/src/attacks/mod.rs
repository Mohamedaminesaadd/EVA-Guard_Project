//! Controlled perturbations used to produce the labelled attack datasets.
//!
//! An [`Attack`] runs against **one victim node** of the local Kurtosis
//! testnet while the monitoring engine records every node. It is started at
//! the beginning of a scenario phase and told to stop at the end; it then
//! undoes whatever it changed, so the next phase starts from a clean state.
//!
//! These are meant for a researcher's own enclave, to generate training data
//! for the anomaly detector. They are deliberately coarse and reversible:
//! none of them corrupts state, exfiltrates anything, or persists after the
//! phase.
//!
//! - [`EclipseAttack`]: repeatedly disconnects the victim geth's peers
//!   (`admin_removePeer`), so its peer set collapses while its RPC stays
//!   observable. Signature: `peer_count` → 0, the head stops advancing,
//!   finality stalls.
//! - [`RpcFloodingAttack`]: floods the victim geth's JSON-RPC endpoint with
//!   concurrent requests. Signature: `rpc_latency_ms` and
//!   `rpc_requests_per_sec` spike, failures rise.

pub mod eclipse;
pub mod rpc_flooding;

use std::sync::Arc;

use futures_util::future::BoxFuture;
use tokio::sync::oneshot;

pub use eclipse::EclipseAttack;
pub use rpc_flooding::RpcFloodingAttack;

/// What an attack did during one phase, for the run report and the logs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttackReport {
    /// Perturbation rounds performed (an attack that works in rounds).
    pub rounds: u64,
    /// Successful actions: peers removed, or requests sent.
    pub actions: u64,
    /// Actions that failed (logged, never fatal).
    pub errors: u64,
}

/// A reversible perturbation of one victim node.
///
/// `run` keeps perturbing until `cancel` fires, then restores and returns.
/// It must never panic: a failed action is counted, logged and skipped.
pub trait Attack: Send + Sync + 'static {
    /// Short name for logs and the report (`eclipse`, `rpc_flooding`).
    fn name(&self) -> &'static str;

    /// The victim node id.
    fn target(&self) -> &str;

    /// Runs until `cancel` resolves, then restores and reports.
    fn run(self: Arc<Self>, cancel: oneshot::Receiver<()>) -> BoxFuture<'static, AttackReport>;
}
