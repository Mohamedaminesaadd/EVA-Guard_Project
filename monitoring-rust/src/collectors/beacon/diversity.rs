use crate::clients::beacon_client::BeaconClient;
use crate::models::metrics::PeerDiversityMetrics;

use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct PeersResponse {
    data: Vec<PeerData>,
}

/// One entry of `/eth/v1/node/peers` (only the fields used here).
#[derive(Debug, Clone, Deserialize)]
struct PeerData {
    state: Option<String>,
    last_seen_p2p_address: Option<String>,
}

/// Collects Beacon Node peer diversity metrics.
pub struct DiversityCollector {
    client: Client,
}

impl DiversityCollector {
    pub fn new(request_timeout_secs: u64) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;

        Ok(Self { client })
    }

    /// Collects IP and client diversity from one Beacon Node.
    ///
    /// Only connected peers are counted. Unavailable measurements remain
    /// None.
    pub async fn collect(&self, beacon: &BeaconClient) -> PeerDiversityMetrics {
        let mut metrics = PeerDiversityMetrics::default();

        // 1. Retrieve detailed peer information.
        let Some(peers) = self.fetch_peers(beacon).await else {
            return metrics;
        };

        // 2. Compute the diversity metrics.
        apply_diversity(&mut metrics, &peers);

        metrics
    }

    /// Fetches `/eth/v1/node/peers`. Any failure gives None.
    async fn fetch_peers(&self, beacon: &BeaconClient) -> Option<Vec<PeerData>> {
        let url = format!("{}/eth/v1/node/peers", beacon.api_url());

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;

        let peers: PeersResponse = response.json().await.ok()?;

        Some(peers.data)
    }
}

/// Fills the diversity metrics from the peer list.
fn apply_diversity(metrics: &mut PeerDiversityMetrics, peers: &[PeerData]) {
    // Extract unique IP addresses of connected peers.
    //
    // The peer list also contains disconnected, connecting and
    // disconnecting peers that must not be counted.
    let unique_ips: HashSet<IpAddr> = peers
        .iter()
        .filter(|peer| is_connected(peer))
        .filter_map(|peer| peer.last_seen_p2p_address.as_deref().and_then(extract_ip))
        .collect();

    metrics.unique_peer_ips = Some(unique_ips.len() as u64);

    // Client implementations are not available from this endpoint (an ENR
    // is not a reliable source), so client diversity stays unavailable.
    metrics.unique_peer_clients = None;
    metrics.top_client_ratio = None;
}

/// Tells whether a peer is connected.
///
/// A peer without a `state` field is kept, since its state is unknown.
fn is_connected(peer: &PeerData) -> bool {
    match peer.state.as_deref() {
        Some(state) => state.eq_ignore_ascii_case("connected"),
        None => true,
    }
}

/// Extracts an IP address from a multiaddress-like string.
///
/// Examples:
/// /ip4/192.168.1.10/tcp/9000
/// /ip6/2001:db8::1/tcp/9000
///
/// Returns None for DNS addresses and for values that are not valid IPs.
fn extract_ip(address: &str) -> Option<IpAddr> {
    let segments: Vec<&str> = address.split('/').collect();

    segments.windows(2).find_map(|pair| match pair[0] {
        "ip4" => pair[1].parse::<Ipv4Addr>().ok().map(IpAddr::V4),
        "ip6" => pair[1].parse::<Ipv6Addr>().ok().map(IpAddr::V6),
        _ => None,
    })
}

/// Calculates the most represented client's ratio.
///
/// Returns None when there are no identifiable client names.
/// Not used yet: it is meant for when client names become available.
#[allow(dead_code)]
fn calculate_top_client_ratio(clients: &[String]) -> Option<f64> {
    if clients.is_empty() {
        return None;
    }

    let mut counts: HashMap<&str, usize> = HashMap::new();

    for client in clients {
        *counts.entry(client.as_str()).or_insert(0) += 1;
    }

    let maximum = counts.values().copied().max()?;

    Some(maximum as f64 / clients.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::{json, Value};
    use wiremock::MockServer;

    const ROUTE: &str = "/eth/v1/node/peers";

    fn peer(state: Option<&str>, address: Option<&str>) -> PeerData {
        PeerData {
            state: state.map(str::to_string),
            last_seen_p2p_address: address.map(str::to_string),
        }
    }

    fn json_peer(ip: &str, state: &str) -> Value {
        json!({
            "peer_id": format!("16Uiu2HAm-{ip}"),
            "enr": "enr:-xyz",
            "last_seen_p2p_address": format!("/ip4/{ip}/tcp/9000/p2p/16Uiu2HAmX"),
            "state": state,
            "direction": "inbound"
        })
    }

    fn diversity_of(peers: &[PeerData]) -> PeerDiversityMetrics {
        let mut metrics = PeerDiversityMetrics::default();
        apply_diversity(&mut metrics, peers);
        metrics
    }

    // ---- extract_ip ---------------------------------------------------------

    #[test]
    fn extracts_ipv4_address() {
        assert_eq!(
            extract_ip("/ip4/192.168.1.10/tcp/9000"),
            Some("192.168.1.10".parse().unwrap())
        );
    }

    #[test]
    fn extracts_ipv6_address() {
        assert_eq!(
            extract_ip("/ip6/2001:db8::1/tcp/9000"),
            Some("2001:db8::1".parse().unwrap())
        );
    }

    #[test]
    fn extracts_ip_when_a_peer_id_follows() {
        assert_eq!(
            extract_ip("/ip4/10.1.2.3/udp/9000/quic-v1/p2p/16Uiu2HAmX"),
            Some("10.1.2.3".parse().unwrap())
        );
    }

    #[test]
    fn rejects_address_without_ip() {
        assert_eq!(extract_ip("/dns4/example.com/tcp/9000"), None);
    }

    #[test]
    fn rejects_invalid_ip_values() {
        assert_eq!(extract_ip("/ip4/not-an-ip/tcp/9000"), None);
        assert_eq!(extract_ip("/ip4/999.1.1.1/tcp/9000"), None);
        // An IPv6 literal under an ip4 segment is invalid.
        assert_eq!(extract_ip("/ip4/::1/tcp/9000"), None);
    }

    #[test]
    fn rejects_empty_and_truncated_addresses() {
        assert_eq!(extract_ip(""), None);
        assert_eq!(extract_ip("/ip4"), None);
        assert_eq!(extract_ip("/ip4/"), None);
    }

    #[test]
    fn different_ipv6_spellings_are_the_same_address() {
        assert_eq!(
            extract_ip("/ip6/2001:db8::1/tcp/9000"),
            extract_ip("/ip6/2001:0DB8:0:0:0:0:0:1/tcp/9001")
        );
    }

    // ---- apply_diversity ----------------------------------------------------

    #[test]
    fn counts_unique_ips_of_connected_peers() {
        let metrics = diversity_of(&[
            peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9000")),
            peer(Some("connected"), Some("/ip4/10.0.0.2/tcp/9000")),
            peer(Some("connected"), Some("/ip6/2001:db8::1/tcp/9000")),
        ]);

        assert_eq!(metrics.unique_peer_ips, Some(3));
    }

    #[test]
    fn peers_sharing_an_ip_count_once() {
        let metrics = diversity_of(&[
            peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9000")),
            peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9001")),
            peer(Some("connected"), Some("/ip4/10.0.0.1/udp/9000")),
        ]);

        assert_eq!(metrics.unique_peer_ips, Some(1));
    }

    #[test]
    fn ignores_peers_that_are_not_connected() {
        let metrics = diversity_of(&[
            peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9000")),
            peer(Some("disconnected"), Some("/ip4/10.0.0.2/tcp/9000")),
            peer(Some("connecting"), Some("/ip4/10.0.0.3/tcp/9000")),
            peer(Some("disconnecting"), Some("/ip4/10.0.0.4/tcp/9000")),
        ]);

        assert_eq!(metrics.unique_peer_ips, Some(1));
    }

    #[test]
    fn state_comparison_ignores_case() {
        let metrics = diversity_of(&[peer(Some("Connected"), Some("/ip4/10.0.0.1/tcp/9000"))]);

        assert_eq!(metrics.unique_peer_ips, Some(1));
    }

    #[test]
    fn peer_without_state_is_kept() {
        let metrics = diversity_of(&[peer(None, Some("/ip4/10.0.0.1/tcp/9000"))]);

        assert_eq!(metrics.unique_peer_ips, Some(1));
    }

    #[test]
    fn ignores_peers_without_a_usable_address() {
        let metrics = diversity_of(&[
            peer(Some("connected"), None),
            peer(Some("connected"), Some("/dns4/example.com/tcp/9000")),
            peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9000")),
        ]);

        assert_eq!(metrics.unique_peer_ips, Some(1));
    }

    #[test]
    fn no_peers_gives_zero_ips_and_unavailable_clients() {
        let metrics = diversity_of(&[]);

        assert_eq!(metrics.unique_peer_ips, Some(0));
        assert_eq!(metrics.unique_peer_clients, None);
        assert_eq!(metrics.top_client_ratio, None);
    }

    #[test]
    fn client_diversity_is_always_unavailable() {
        let metrics = diversity_of(&[peer(Some("connected"), Some("/ip4/10.0.0.1/tcp/9000"))]);

        assert_eq!(metrics.unique_peer_clients, None);
        assert_eq!(metrics.top_client_ratio, None);
    }

    // ---- calculate_top_client_ratio --------------------------------------

    #[test]
    fn calculates_client_ratio() {
        let clients = vec![
            "Lighthouse".to_string(),
            "Lighthouse".to_string(),
            "Lighthouse".to_string(),
            "Prysm".to_string(),
        ];

        assert_eq!(calculate_top_client_ratio(&clients), Some(0.75));
    }

    #[test]
    fn single_client_ratio_is_one() {
        let clients = vec!["Teku".to_string(), "Teku".to_string()];

        assert_eq!(calculate_top_client_ratio(&clients), Some(1.0));
    }

    #[test]
    fn handles_empty_client_list() {
        let clients: Vec<String> = vec![];

        assert_eq!(calculate_top_client_ratio(&clients), None);
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_only_connected_peers_from_the_beacon_api() {
        let server = MockServer::start().await;
        mock_json(
            &server,
            ROUTE,
            json!({
                "data": [
                    json_peer("10.0.0.1", "connected"),
                    json_peer("10.0.0.2", "connected"),
                    json_peer("10.0.0.3", "disconnected"),
                    json_peer("10.0.0.4", "disconnected"),
                    json_peer("10.0.0.5", "disconnected")
                ],
                "meta": { "count": 5 }
            }),
        )
        .await;

        let collector = DiversityCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.unique_peer_ips, Some(2));
        assert_eq!(metrics.unique_peer_clients, None);
        assert_eq!(metrics.top_client_ratio, None);
    }

    #[tokio::test]
    async fn empty_peer_list_gives_zero_ips() {
        let server = MockServer::start().await;
        mock_json(&server, ROUTE, json!({ "data": [] })).await;

        let collector = DiversityCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.unique_peer_ips, Some(0));
    }

    #[tokio::test]
    async fn http_error_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_status(&server, ROUTE, 500).await;

        let collector = DiversityCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.unique_peer_ips, None);
    }

    #[tokio::test]
    async fn malformed_json_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_garbage(&server, ROUTE).await;

        let collector = DiversityCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.unique_peer_ips, None);
    }

    #[tokio::test]
    async fn missing_data_field_gives_empty_metrics() {
        let server = MockServer::start().await;
        mock_json(&server, ROUTE, json!({ "meta": { "count": 0 } })).await;

        let collector = DiversityCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.unique_peer_ips, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_empty_metrics() {
        let collector = DiversityCollector::new(2).unwrap();
        let metrics = collector.collect(&unreachable_client()).await;

        assert_eq!(metrics.unique_peer_ips, None);
    }
}
