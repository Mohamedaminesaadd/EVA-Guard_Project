//! Start-up check of every endpoint the collectors depend on.
//!
//! The collectors turn any failure into `None` ("not measured"), which keeps
//! the dataset honest but hides the cause: a stopped container, a Docker port
//! that changed after a restart, and a Geth API that was not enabled all give
//! the same `null` column. [`check_nodes`] probes each endpoint once, with the
//! same clients the collectors use, and says which one fails and why.
//!
//! Six endpoints are checked per node:
//!
//! | endpoint          | used for                                          |
//! |-------------------|---------------------------------------------------|
//! | Geth RPC          | block number, sync, peer count, RPC latency       |
//! | Geth `admin` API  | peer details and peer churn                       |
//! | Geth `txpool` API | pending and queued transactions                   |
//! | Geth metrics      | RPC request counters                              |
//! | Beacon API        | every Beacon metric                               |
//! | Validator metrics | every Validator metric                            |
//!
//! A failed check never stops the monitoring: a node that is down is itself
//! something the dataset must record.

use std::error::Error as _;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use serde_json::Value;

use crate::clients::beacon_client::BeaconError;
use crate::clients::geth_client::GethError;
use crate::clients::validator_client::ValidatorError;
use crate::monitoring::node_manager::ManagedNode;

/// JSON-RPC "method not found": the API namespace is not enabled.
const RPC_METHOD_NOT_FOUND: i64 = -32601;

/// Time allowed on top of the request timeout before a probe is abandoned.
const PROBE_SLACK: Duration = Duration::from_secs(1);

/// An endpoint of a node set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    GethRpc,
    GethAdminApi,
    GethTxpoolApi,
    GethMetrics,
    BeaconApi,
    ValidatorMetrics,
}

impl Endpoint {
    pub const ALL: [Endpoint; 6] = [
        Endpoint::GethRpc,
        Endpoint::GethAdminApi,
        Endpoint::GethTxpoolApi,
        Endpoint::GethMetrics,
        Endpoint::BeaconApi,
        Endpoint::ValidatorMetrics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Endpoint::GethRpc => "geth rpc",
            Endpoint::GethAdminApi => "geth admin api",
            Endpoint::GethTxpoolApi => "geth txpool api",
            Endpoint::GethMetrics => "geth metrics",
            Endpoint::BeaconApi => "beacon api",
            Endpoint::ValidatorMetrics => "validator metrics",
        }
    }

    /// What is `null` in the dataset while this endpoint fails.
    pub fn consequence(self) -> &'static str {
        match self {
            Endpoint::GethRpc => {
                "geth block, sync, peer count and RPC latency (rpc_available is false)"
            }
            Endpoint::GethAdminApi => "geth peer details and peer churn",
            Endpoint::GethTxpoolApi => "geth txpool_pending and txpool_queued",
            Endpoint::GethMetrics => "geth RPC request counters and rates",
            Endpoint::BeaconApi => "every Beacon Node metric",
            Endpoint::ValidatorMetrics => "every Validator Client metric",
        }
    }
}

/// The result of probing one endpoint of one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointCheck {
    pub node_id: String,
    pub endpoint: Endpoint,
    pub url: String,
    /// `Err` carries the reason, in plain words.
    pub outcome: Result<(), String>,
}

impl EndpointCheck {
    pub fn is_ok(&self) -> bool {
        self.outcome.is_ok()
    }
}

/// `true` when every probe succeeded.
pub fn all_ok(checks: &[EndpointCheck]) -> bool {
    checks.iter().all(EndpointCheck::is_ok)
}

/// Probes the six endpoints of every node, all at once.
///
/// The result is ordered by node, then by [`Endpoint::ALL`]. Takes at most
/// `request_timeout` plus one second.
pub async fn check_nodes(
    nodes: &[Arc<ManagedNode>],
    request_timeout: Duration,
) -> Vec<EndpointCheck> {
    let probes = nodes.iter().flat_map(|node| {
        Endpoint::ALL
            .into_iter()
            .map(move |endpoint| probe(node, endpoint, request_timeout))
    });

    join_all(probes).await
}

async fn probe(node: &ManagedNode, endpoint: Endpoint, request_timeout: Duration) -> EndpointCheck {
    let outcome =
        match tokio::time::timeout(request_timeout + PROBE_SLACK, request(node, endpoint)).await {
            Ok(outcome) => outcome,
            Err(_elapsed) => Err("timed out".to_string()),
        };

    EndpointCheck {
        node_id: node.node_id.clone(),
        endpoint,
        url: url_of(node, endpoint),
        outcome,
    }
}

fn url_of(node: &ManagedNode, endpoint: Endpoint) -> String {
    match endpoint {
        Endpoint::GethRpc | Endpoint::GethAdminApi | Endpoint::GethTxpoolApi => {
            node.geth.rpc_url().to_string()
        }
        Endpoint::GethMetrics => node.geth.metrics_url().to_string(),
        Endpoint::BeaconApi => node.beacon.api_url().to_string(),
        Endpoint::ValidatorMetrics => node.validator.metrics_url().to_string(),
    }
}

async fn request(node: &ManagedNode, endpoint: Endpoint) -> Result<(), String> {
    match endpoint {
        Endpoint::GethRpc => node
            .geth
            .block_number()
            .await
            .map(drop)
            .map_err(geth_reason),
        Endpoint::GethAdminApi => node.geth.admin_peers().await.map(drop).map_err(geth_reason),
        Endpoint::GethTxpoolApi => node
            .geth
            .call::<Value>("txpool_status", vec![])
            .await
            .map(drop)
            .map_err(geth_reason),
        Endpoint::GethMetrics => node
            .geth
            .get_prometheus_metrics()
            .await
            .map(drop)
            .map_err(geth_reason),
        Endpoint::BeaconApi => node.beacon.syncing().await.map(drop).map_err(beacon_reason),
        Endpoint::ValidatorMetrics => node
            .validator
            .collect_metrics()
            .await
            .map(drop)
            .map_err(validator_reason),
    }
}

fn geth_reason(error: GethError) -> String {
    match error {
        GethError::Http(error) => http_reason(&error),
        GethError::Rpc {
            code: RPC_METHOD_NOT_FOUND,
            message,
        } => format!("API not enabled on the node (JSON-RPC {RPC_METHOD_NOT_FOUND}: {message})"),
        GethError::Rpc { code, message } => format!("JSON-RPC error {code}: {message}"),
        other => other.to_string(),
    }
}

fn beacon_reason(error: BeaconError) -> String {
    match error {
        BeaconError::Http(error) => http_reason(&error),
        other => other.to_string(),
    }
}

fn validator_reason(error: ValidatorError) -> String {
    match error {
        ValidatorError::Http(error) => http_reason(&error),
        other => other.to_string(),
    }
}

/// A transport error in plain words: refused, timed out, HTTP status...
fn http_reason(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return "timed out".to_string();
    }

    if let Some(status) = error.status() {
        return format!("HTTP {status}");
    }

    // The useful part ("Connection refused") is the deepest cause.
    let mut root = String::new();
    let mut source = error.source();
    while let Some(cause) = source {
        root = cause.to_string();
        source = cause.source();
    }

    if root.to_ascii_lowercase().contains("refused") {
        "connection refused".to_string()
    } else if error.is_connect() {
        format!("cannot connect: {root}")
    } else if root.is_empty() {
        error.to_string()
    } else {
        format!("{error}: {root}")
    }
}

/// A table of the checks, followed by what to do about the failures.
pub fn render_report(checks: &[EndpointCheck], request_timeout: Duration) -> String {
    let mut out = format!("Endpoint check (timeout {request_timeout:?} per request)\n");

    let node_width = checks
        .iter()
        .map(|c| c.node_id.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let endpoint_width = Endpoint::ALL
        .iter()
        .map(|endpoint| endpoint.label().len())
        .max()
        .unwrap_or(8);

    let _ = writeln!(
        out,
        "  {:<node_width$}  {:<endpoint_width$}  RESULT  DETAIL",
        "NODE", "ENDPOINT"
    );

    for check in checks {
        let (result, detail) = match &check.outcome {
            Ok(()) => ("OK", String::new()),
            Err(reason) => ("FAIL", format!("{reason} - {}", check.url)),
        };

        let _ = writeln!(
            out,
            "  {:<node_width$}  {:<endpoint_width$}  {:<6}  {}",
            check.node_id,
            check.endpoint.label(),
            result,
            detail
        );
    }

    let failures = checks.iter().filter(|check| !check.is_ok()).count();
    if failures == 0 {
        out.push_str("\nAll endpoints answer.\n");
        return out;
    }

    let _ = writeln!(out, "\n{failures} of {} checks failed.", checks.len());
    out.push_str("While an endpoint fails, this is null in the dataset:\n");

    for endpoint in Endpoint::ALL {
        if checks.iter().any(|c| c.endpoint == endpoint && !c.is_ok()) {
            let _ = writeln!(out, "  - {}: {}", endpoint.label(), endpoint.consequence());
        }
    }

    let hints = hints(checks);
    if !hints.is_empty() {
        out.push_str("\nWhat to check:\n");
        for hint in hints {
            let _ = writeln!(out, "  - {hint}");
        }
    }

    out
}

fn hints(checks: &[EndpointCheck]) -> Vec<String> {
    let failed = |endpoint: Endpoint| checks.iter().any(|c| c.endpoint == endpoint && !c.is_ok());
    let reason_has = |needle: &str| {
        checks
            .iter()
            .any(|c| matches!(&c.outcome, Err(reason) if reason.contains(needle)))
    };

    let mut hints = Vec::new();

    if reason_has("connection refused") {
        hints.push(
            "connection refused: the service is stopped, or its port changed. Docker assigns \
             random host ports and they change when a container is recreated: compare \
             config/nodes.toml with `docker ps --format 'table {{.Names}}\\t{{.Ports}}'`."
                .to_string(),
        );
    }

    if reason_has("timed out") {
        hints.push(
            "timed out: the port answers but the service does not (still starting, overloaded, \
             or a firewall dropping packets)."
                .to_string(),
        );
    }

    if reason_has("API not enabled") {
        hints.push(
            "API not enabled: start Geth with --http.api eth,net,web3,admin,txpool (the admin and \
             txpool namespaces are off by default)."
                .to_string(),
        );
    }

    if failed(Endpoint::GethMetrics) && reason_has("HTTP 404") {
        hints.push(
            "geth metrics HTTP 404: start Geth with --metrics and --metrics.addr 0.0.0.0, and \
             check the path (/debug/metrics/prometheus)."
                .to_string(),
        );
    }

    if reason_has("Invalid Prometheus") {
        hints.push(
            "the Validator Client answers but its output could not be parsed: the detail shows \
             the rejected line."
                .to_string(),
        );
    }

    if (failed(Endpoint::BeaconApi) || failed(Endpoint::ValidatorMetrics)) && reason_has("HTTP 404")
    {
        hints.push(
            "HTTP 404 on the Beacon API or Validator metrics: the port answers but is not that \
             service (wrong port), or the HTTP API / metrics server is not enabled (--http, \
             --metrics)."
                .to_string(),
        );
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::node_collector::test_support::{refused_toml, Alive, FakeNode};

    use serde_json::json;
    use std::time::Instant;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, ResponseTemplate};

    const TIMEOUT: Duration = Duration::from_secs(3);

    fn check(node: &str, endpoint: Endpoint, outcome: Result<(), &str>) -> EndpointCheck {
        EndpointCheck {
            node_id: node.to_string(),
            endpoint,
            url: format!("http://127.0.0.1:1/{}", endpoint.label().replace(' ', "-")),
            outcome: outcome.map_err(str::to_string),
        }
    }

    fn reason(checks: &[EndpointCheck], endpoint: Endpoint) -> &str {
        match &checks
            .iter()
            .find(|c| c.endpoint == endpoint)
            .unwrap()
            .outcome
        {
            Ok(()) => "ok",
            Err(reason) => reason,
        }
    }

    async fn run(fake: &FakeNode, request_timeout_secs: u64) -> Vec<EndpointCheck> {
        let node = fake.managed("node-01", request_timeout_secs);
        check_nodes(&[node], Duration::from_secs(request_timeout_secs)).await
    }

    // ---- Against fake servers ----

    #[tokio::test]
    async fn a_healthy_node_passes_the_six_checks_in_order() {
        let fake = FakeNode::healthy("0xaaa").await;

        let checks = run(&fake, 3).await;

        assert!(all_ok(&checks), "{checks:?}");
        let endpoints: Vec<_> = checks.iter().map(|c| c.endpoint).collect();
        assert_eq!(endpoints, Endpoint::ALL);
        assert!(checks.iter().all(|c| c.node_id == "node-01"));
    }

    #[tokio::test]
    async fn checks_are_grouped_by_node_in_node_order() {
        let first = FakeNode::healthy("0xaaa").await;
        let second = FakeNode::healthy("0xaaa").await;
        let nodes = [first.managed("node-01", 3), second.managed("node-02", 3)];

        let checks = check_nodes(&nodes, TIMEOUT).await;

        let ids: Vec<_> = checks.iter().map(|c| c.node_id.as_str()).collect();
        assert_eq!(checks.len(), 12);
        assert_eq!(&ids[..6], ["node-01"; 6]);
        assert_eq!(&ids[6..], ["node-02"; 6]);
    }

    #[tokio::test]
    async fn the_url_of_each_check_is_the_endpoint_that_was_probed() {
        let fake = FakeNode::healthy("0xaaa").await;

        let checks = run(&fake, 3).await;

        let url = |endpoint| {
            checks
                .iter()
                .find(|c| c.endpoint == endpoint)
                .unwrap()
                .url
                .clone()
        };
        assert_eq!(url(Endpoint::GethRpc), fake.geth.uri());
        assert_eq!(url(Endpoint::GethAdminApi), fake.geth.uri());
        assert_eq!(
            url(Endpoint::GethMetrics),
            format!("{}/debug/metrics/prometheus", fake.geth.uri())
        );
        assert_eq!(url(Endpoint::BeaconApi), fake.beacon.uri());
        assert_eq!(
            url(Endpoint::ValidatorMetrics),
            format!("{}/metrics", fake.validator.uri())
        );
    }

    #[tokio::test]
    async fn nothing_listening_is_reported_as_connection_refused() {
        let config = crate::config::NodesConfig::from_toml_str(&refused_toml("node-01")).unwrap();
        let node = Arc::new(ManagedNode::new(&config.nodes[0], 3).unwrap());

        let checks = check_nodes(&[node], TIMEOUT).await;

        assert_eq!(checks.len(), 6);
        for check in &checks {
            assert_eq!(
                check.outcome,
                Err("connection refused".to_string()),
                "{:?}",
                check.endpoint
            );
        }
    }

    #[tokio::test]
    async fn a_geth_without_the_admin_and_txpool_apis_is_told_so() {
        let fake = FakeNode::start(
            "0xaaa",
            Alive {
                geth: false,
                beacon: true,
                validator: true,
            },
        )
        .await;

        // Only the default namespaces answer; the others are "not found".
        Mock::given(method("POST"))
            .and(body_partial_json(json!({ "method": "eth_blockNumber" })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "jsonrpc": "2.0", "id": 1, "result": "0x10" })),
            )
            .mount(&fake.geth)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": 1,
                "error": { "code": -32601, "message": "the method does not exist/is not available" }
            })))
            .mount(&fake.geth)
            .await;

        let checks = run(&fake, 3).await;

        assert_eq!(reason(&checks, Endpoint::GethRpc), "ok");
        assert!(
            reason(&checks, Endpoint::GethAdminApi).starts_with("API not enabled"),
            "{}",
            reason(&checks, Endpoint::GethAdminApi)
        );
        assert!(reason(&checks, Endpoint::GethTxpoolApi).starts_with("API not enabled"));
        // The other components are unaffected.
        assert_eq!(reason(&checks, Endpoint::BeaconApi), "ok");
        assert_eq!(reason(&checks, Endpoint::ValidatorMetrics), "ok");
    }

    #[tokio::test]
    async fn a_service_answering_404_is_reported_with_its_status() {
        let fake = FakeNode::start(
            "0xaaa",
            Alive {
                geth: true,
                beacon: false,
                validator: false,
            },
        )
        .await;

        let checks = run(&fake, 3).await;

        assert_eq!(reason(&checks, Endpoint::BeaconApi), "HTTP 404 Not Found");
        assert_eq!(
            reason(&checks, Endpoint::ValidatorMetrics),
            "HTTP 404 Not Found"
        );
        assert_eq!(reason(&checks, Endpoint::GethRpc), "ok");
    }

    #[tokio::test]
    async fn a_hanging_endpoint_times_out_without_slowing_the_others() {
        let fake = FakeNode::healthy("0xaaa").await;
        fake.beacon.reset().await;
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/syncing"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&fake.beacon)
            .await;

        let started = Instant::now();
        let checks = run(&fake, 1).await;

        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(reason(&checks, Endpoint::BeaconApi), "timed out");
        assert_eq!(reason(&checks, Endpoint::GethRpc), "ok");
        assert_eq!(reason(&checks, Endpoint::ValidatorMetrics), "ok");
    }

    #[tokio::test]
    async fn a_validator_scrape_the_parser_rejects_shows_the_offending_line() {
        let fake = FakeNode::healthy("0xaaa").await;
        fake.validator.reset().await;
        Mock::given(method("GET"))
            .and(path("/metrics"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("good_metric 1\nthis is not a metric\n"),
            )
            .mount(&fake.validator)
            .await;

        let checks = run(&fake, 3).await;

        let reason = reason(&checks, Endpoint::ValidatorMetrics);
        assert!(
            reason.contains("Invalid Prometheus metrics format"),
            "{reason}"
        );
        assert!(reason.contains("this is not a metric"), "{reason}");
    }

    // ---- The report ----

    #[test]
    fn a_report_without_failures_says_so_and_gives_no_advice() {
        let checks: Vec<_> = Endpoint::ALL
            .into_iter()
            .map(|endpoint| check("node-01", endpoint, Ok(())))
            .collect();

        let report = render_report(&checks, TIMEOUT);

        assert!(report.contains("All endpoints answer."));
        assert!(!report.contains("What to check"));
        assert!(!report.contains("FAIL"));
        assert!(report.contains("geth rpc"));
    }

    #[test]
    fn a_report_lists_failures_their_consequences_and_the_docker_port_hint() {
        let mut checks: Vec<_> = Endpoint::ALL
            .into_iter()
            .map(|endpoint| check("node-02", endpoint, Err("connection refused")))
            .collect();
        checks.push(check("node-01", Endpoint::GethRpc, Ok(())));

        let report = render_report(&checks, TIMEOUT);

        assert!(report.contains("6 of 7 checks failed."), "{report}");
        assert!(report.contains("node-02  geth rpc"));
        assert!(report.contains("FAIL    connection refused - http://127.0.0.1:1/geth-rpc"));
        assert!(report.contains("- beacon api: every Beacon Node metric"));
        assert!(report.contains("- geth txpool api: geth txpool_pending and txpool_queued"));
        assert!(report.contains("docker ps"));
    }

    #[test]
    fn only_the_hints_that_apply_are_given() {
        let checks = [
            check(
                "node-01",
                Endpoint::GethAdminApi,
                Err("API not enabled on the node (JSON-RPC -32601: x)"),
            ),
            check("node-01", Endpoint::GethRpc, Ok(())),
        ];

        let report = render_report(&checks, TIMEOUT);

        assert!(report.contains("--http.api eth,net,web3,admin,txpool"));
        assert!(!report.contains("docker ps"), "{report}");
        assert!(!report.contains("--metrics.addr"));
    }

    #[test]
    fn all_ok_is_false_as_soon_as_one_check_fails() {
        assert!(all_ok(&[]));
        assert!(all_ok(&[check("n", Endpoint::GethRpc, Ok(()))]));
        assert!(!all_ok(&[
            check("n", Endpoint::GethRpc, Ok(())),
            check("n", Endpoint::BeaconApi, Err("timed out")),
        ]));
    }

    #[test]
    fn every_endpoint_has_a_label_and_a_consequence() {
        for endpoint in Endpoint::ALL {
            assert!(!endpoint.label().is_empty());
            assert!(!endpoint.consequence().is_empty());
        }
        assert_eq!(Endpoint::ALL.len(), 6);
    }
}
