use crate::clients::geth_client::{GethClient, PeerInfo};
use crate::models::metrics::P2pMetrics;

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Collects Geth P2P network metrics.
///
/// Keeps the previous peer IDs to calculate peer churn between two
/// consecutive valid cycles. One collector instance must be used per node.
pub struct P2pCollector {
    previous_peers: Mutex<Option<HashSet<String>>>,
}

impl P2pCollector {
    pub fn new() -> Self {
        Self {
            previous_peers: Mutex::new(None),
        }
    }

    /// Collects P2P metrics from one Geth node.
    pub async fn collect(&self, client: &GethClient) -> P2pMetrics {
        let mut metrics = P2pMetrics::default();

        // 1. Connected peer count.
        if let Ok(result) = client.peer_count().await {
            metrics.peer_count = Some(result.value);
        }

        // 2. Check whether P2P networking is listening.
        if let Ok(result) = client.listening().await {
            metrics.network_listening = Some(result.value);
        }

        // 3. Retrieve detailed peer information.
        match client.admin_peers().await {
            Ok(result) => self.apply_peers(&mut metrics, &result.value),

            Err(_) => {
                metrics.peer_details_available = Some(false);

                // The next churn must compare two consecutive valid
                // samples, not a sample taken several cycles ago.
                self.reset_baseline();
            }
        }

        metrics
    }

    /// Fills peer details and churn from a successful `admin_peers` answer.
    fn apply_peers(&self, metrics: &mut P2pMetrics, peers: &[PeerInfo]) {
        metrics.peer_details_available = Some(true);
        metrics.admin_peer_count = Some(peers.len() as u64);

        // Extract stable peer IDs.
        let current_peers = extract_peer_ids(peers);

        if current_peers.len() == peers.len() {
            // Peer details are complete: churn can be calculated.
            self.update_peer_churn(metrics, current_peers);
        } else {
            // Missing, empty or duplicated IDs: the set is unreliable,
            // so do not use it as a churn baseline either.
            self.reset_baseline();
        }
    }

    /// Calculates new peers, disconnected peers and churn.
    ///
    /// The first call only stores the baseline and measures nothing.
    fn update_peer_churn(&self, metrics: &mut P2pMetrics, current_peers: HashSet<String>) {
        let mut previous = self.lock_previous();

        if let Some(previous_peers) = previous.as_ref() {
            let new_peers = current_peers.difference(previous_peers).count() as u64;

            let disconnected_peers = previous_peers.difference(&current_peers).count() as u64;

            metrics.new_peers = Some(new_peers);
            metrics.disconnected_peers = Some(disconnected_peers);
            metrics.peer_churn_count = Some(new_peers + disconnected_peers);
        }

        *previous = Some(current_peers);
    }

    /// Forgets the previous peer set.
    fn reset_baseline(&self) {
        *self.lock_previous() = None;
    }

    /// Locks the previous peer set, recovering from a poisoned mutex.
    ///
    /// The protected value is replaced atomically and can never be left
    /// half-updated, so continuing after a panic elsewhere is safe and
    /// keeps churn measurement alive.
    fn lock_previous(&self) -> MutexGuard<'_, Option<HashSet<String>>> {
        self.previous_peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for P2pCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts stable peer IDs from admin_peers response.
///
/// Peers without an ID, or with an empty one, are skipped.
fn extract_peer_ids(peers: &[PeerInfo]) -> HashSet<String> {
    peers
        .iter()
        .filter_map(|peer| peer.id.as_ref().filter(|id| !id.is_empty()).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    fn peer_set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    fn peer(id: Option<&str>) -> PeerInfo {
        PeerInfo {
            id: id.map(str::to_string),
            name: None,
            network: None,
        }
    }

    fn peers(ids: &[&str]) -> Vec<PeerInfo> {
        ids.iter().map(|id| peer(Some(id))).collect()
    }

    /// Runs one churn cycle and returns the resulting metrics.
    fn churn_cycle(collector: &P2pCollector, ids: &[&str]) -> P2pMetrics {
        let mut metrics = P2pMetrics::default();
        collector.update_peer_churn(&mut metrics, peer_set(ids));
        metrics
    }

    /// Runs one full `admin_peers` cycle and returns the resulting metrics.
    fn peers_cycle(collector: &P2pCollector, list: &[PeerInfo]) -> P2pMetrics {
        let mut metrics = P2pMetrics::default();
        collector.apply_peers(&mut metrics, list);
        metrics
    }

    #[test]
    fn first_cycle_has_no_churn_measurement() {
        let collector = P2pCollector::new();

        let metrics = churn_cycle(&collector, &["peer-1", "peer-2"]);

        assert_eq!(metrics.new_peers, None);
        assert_eq!(metrics.disconnected_peers, None);
        assert_eq!(metrics.peer_churn_count, None);
    }

    #[test]
    fn detects_new_peers() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        let metrics = churn_cycle(&collector, &["peer-1", "peer-2", "peer-3"]);

        assert_eq!(metrics.new_peers, Some(1));
        assert_eq!(metrics.disconnected_peers, Some(0));
        assert_eq!(metrics.peer_churn_count, Some(1));
    }

    #[test]
    fn detects_disconnected_peers() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2", "peer-3"]);

        let metrics = churn_cycle(&collector, &["peer-1", "peer-2"]);

        assert_eq!(metrics.new_peers, Some(0));
        assert_eq!(metrics.disconnected_peers, Some(1));
        assert_eq!(metrics.peer_churn_count, Some(1));
    }

    #[test]
    fn detects_simultaneous_peer_changes() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        let metrics = churn_cycle(&collector, &["peer-2", "peer-3"]);

        assert_eq!(metrics.new_peers, Some(1));
        assert_eq!(metrics.disconnected_peers, Some(1));
        assert_eq!(metrics.peer_churn_count, Some(2));
    }

    #[test]
    fn stable_peer_set_has_zero_churn() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        let metrics = churn_cycle(&collector, &["peer-2", "peer-1"]);

        assert_eq!(metrics.new_peers, Some(0));
        assert_eq!(metrics.disconnected_peers, Some(0));
        assert_eq!(metrics.peer_churn_count, Some(0));
    }

    #[test]
    fn losing_every_peer_is_full_churn() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        let metrics = churn_cycle(&collector, &[]);

        assert_eq!(metrics.new_peers, Some(0));
        assert_eq!(metrics.disconnected_peers, Some(2));
        assert_eq!(metrics.peer_churn_count, Some(2));
    }

    #[test]
    fn churn_uses_the_latest_sample_as_baseline() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1"]);
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        let metrics = churn_cycle(&collector, &["peer-1", "peer-2"]);

        assert_eq!(metrics.peer_churn_count, Some(0));
    }

    #[test]
    fn ignores_missing_peer_ids() {
        let list = vec![peer(Some("peer-1")), peer(None)];

        assert_eq!(extract_peer_ids(&list), peer_set(&["peer-1"]));
    }

    #[test]
    fn ignores_empty_peer_ids() {
        let list = vec![peer(Some("peer-1")), peer(Some(""))];

        assert_eq!(extract_peer_ids(&list), peer_set(&["peer-1"]));
    }

    #[test]
    fn apply_peers_reports_counts_and_churn() {
        let collector = P2pCollector::new();

        let first = peers_cycle(&collector, &peers(&["peer-1", "peer-2"]));
        assert_eq!(first.peer_details_available, Some(true));
        assert_eq!(first.admin_peer_count, Some(2));
        assert_eq!(first.peer_churn_count, None);

        let second = peers_cycle(&collector, &peers(&["peer-2", "peer-3", "peer-4"]));
        assert_eq!(second.peer_details_available, Some(true));
        assert_eq!(second.admin_peer_count, Some(3));
        assert_eq!(second.new_peers, Some(2));
        assert_eq!(second.disconnected_peers, Some(1));
        assert_eq!(second.peer_churn_count, Some(3));
    }

    #[test]
    fn incomplete_peer_details_skip_churn_and_reset_baseline() {
        let collector = P2pCollector::new();
        peers_cycle(&collector, &peers(&["peer-1", "peer-2"]));

        let incomplete = vec![peer(Some("peer-1")), peer(None)];
        let metrics = peers_cycle(&collector, &incomplete);

        // Details were returned, but they cannot be trusted for churn.
        assert_eq!(metrics.peer_details_available, Some(true));
        assert_eq!(metrics.admin_peer_count, Some(2));
        assert_eq!(metrics.peer_churn_count, None);

        // The next valid sample only becomes the new baseline...
        let after = peers_cycle(&collector, &peers(&["peer-1", "peer-2", "peer-3"]));
        assert_eq!(after.peer_churn_count, None);

        // ...and churn resumes on the cycle after that.
        let resumed = peers_cycle(&collector, &peers(&["peer-1", "peer-2"]));
        assert_eq!(resumed.new_peers, Some(0));
        assert_eq!(resumed.disconnected_peers, Some(1));
        assert_eq!(resumed.peer_churn_count, Some(1));
    }

    #[test]
    fn duplicate_peer_ids_are_treated_as_incomplete() {
        let collector = P2pCollector::new();

        let duplicated = peers(&["peer-1", "peer-1"]);
        let metrics = peers_cycle(&collector, &duplicated);

        assert_eq!(metrics.admin_peer_count, Some(2));
        assert_eq!(metrics.peer_churn_count, None);

        // Nothing was stored as baseline, so the next cycle measures nothing.
        let next = peers_cycle(&collector, &peers(&["peer-1", "peer-2"]));
        assert_eq!(next.peer_churn_count, None);
    }

    #[test]
    fn reset_baseline_prevents_stale_churn_after_failures() {
        let collector = P2pCollector::new();
        churn_cycle(&collector, &["peer-1", "peer-2"]);

        // What `collect` does when `admin_peers` fails.
        collector.reset_baseline();

        let metrics = churn_cycle(&collector, &["peer-9"]);

        assert_eq!(metrics.new_peers, None);
        assert_eq!(metrics.disconnected_peers, None);
        assert_eq!(metrics.peer_churn_count, None);
    }

    #[test]
    fn recovers_from_a_poisoned_mutex() {
        let collector = Arc::new(P2pCollector::new());
        churn_cycle(&collector, &["peer-1"]);

        let poisoner = Arc::clone(&collector);
        let result = std::thread::spawn(move || {
            let _guard = poisoner.previous_peers.lock().unwrap();
            panic!("poison the mutex on purpose");
        })
        .join();

        assert!(result.is_err());
        assert!(collector.previous_peers.is_poisoned());

        // Churn measurement keeps working with the previous baseline.
        let metrics = churn_cycle(&collector, &["peer-1", "peer-2"]);

        assert_eq!(metrics.new_peers, Some(1));
        assert_eq!(metrics.disconnected_peers, Some(0));
        assert_eq!(metrics.peer_churn_count, Some(1));
    }
}
