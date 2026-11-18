use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BeaconError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid Beacon API response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Deserialize)]
struct BeaconReply<T> {
    data: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeaconHealth {
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeaconSyncStatus {
    pub head_slot: Option<String>,
    pub sync_distance: Option<String>,
    pub is_syncing: Option<bool>,
    pub is_optimistic: Option<bool>,
    pub el_offline: Option<bool>,
}

/// Peer counts by connection state.
///
/// Used to monitor the stability of the Consensus Layer P2P network.
#[derive(Debug, Clone, Deserialize)]
pub struct PeerCounts {
    pub connected: Option<String>,
    pub connecting: Option<String>,
    pub disconnected: Option<String>,
    pub disconnecting: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeadHeader {
    pub root: Option<String>,
    pub canonical: Option<bool>,
    pub header: Option<HeaderData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderData {
    pub message: Option<HeaderMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderMessage {
    pub slot: Option<String>,
    pub proposer_index: Option<String>,
    pub parent_root: Option<String>,
    pub state_root: Option<String>,
    pub body_root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FinalityCheckpoints {
    pub previous_justified: Option<Checkpoint>,
    pub current_justified: Option<Checkpoint>,
    pub finalized: Option<Checkpoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Checkpoint {
    pub epoch: Option<String>,
    pub root: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BeaconClient {
    client: Client,
    api_url: String,
}

impl BeaconClient {
    /// Creates a Beacon REST API client for one node.
    ///
    /// Surrounding whitespace and trailing slashes are removed from the URL.
    pub fn new(api_url: impl Into<String>, request_timeout_secs: u64) -> Result<Self, BeaconError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;

        let api_url = api_url.into().trim().trim_end_matches('/').to_string();

        Ok(Self { client, api_url })
    }

    /// Generic GET request returning the API response data.
    async fn get<T>(&self, endpoint: &str) -> Result<T, BeaconError>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.api_url, endpoint);

        let response = self.client.get(&url).send().await?.error_for_status()?;

        let reply: BeaconReply<T> = response.json().await?;

        Ok(reply.data)
    }

    /// Checks Beacon Node health.
    ///
    /// Lighthouse may return HTTP status codes directly for this endpoint,
    /// so this method does not expect a JSON "data" wrapper.
    pub async fn health(&self) -> Result<u16, BeaconError> {
        let url = format!("{}/eth/v1/node/health", self.api_url);

        let response = self.client.get(&url).send().await?;
        let status = response.status().as_u16();

        if !response.status().is_success() {
            return Err(BeaconError::InvalidResponse(format!(
                "Health endpoint returned HTTP {}",
                status
            )));
        }

        Ok(status)
    }

    /// Returns Beacon Node version.
    pub async fn version(&self) -> Result<String, BeaconError> {
        #[derive(Deserialize)]
        struct VersionData {
            version: String,
        }

        let data: VersionData = self.get("/eth/v1/node/version").await?;
        Ok(data.version)
    }

    /// Returns Beacon Node synchronization status.
    pub async fn syncing(&self) -> Result<BeaconSyncStatus, BeaconError> {
        self.get("/eth/v1/node/syncing").await
    }

    /// Returns the number of peers by connection state.
    pub async fn peer_count(&self) -> Result<PeerCounts, BeaconError> {
        self.get("/eth/v1/node/peer_count").await
    }

    /// Returns the current Beacon chain head header.
    pub async fn head_header(&self) -> Result<HeadHeader, BeaconError> {
        self.get("/eth/v1/beacon/headers/head").await
    }

    /// Returns the latest finality checkpoints.
    pub async fn finality_checkpoints(&self) -> Result<FinalityCheckpoints, BeaconError> {
        self.get("/eth/v1/beacon/states/head/finality_checkpoints")
            .await
    }

    /// Returns the configured Beacon API URL.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{json, Value};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> BeaconClient {
        BeaconClient::new(server.uri(), 5).expect("client should build")
    }

    /// Serves `body` on GET `route` and expects exactly one request.
    async fn serve_json(server: &MockServer, route: &str, body: Value) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(server)
            .await;
    }

    /// Serves an empty answer with the given status on GET `route`.
    async fn serve_status(server: &MockServer, route: &str, status: u16) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(status))
            .mount(server)
            .await;
    }

    // ---- construction -------------------------------------------------

    #[test]
    fn removes_trailing_slashes_from_url() {
        let client = BeaconClient::new("http://127.0.0.1:32794///", 5).unwrap();

        assert_eq!(client.api_url(), "http://127.0.0.1:32794");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let client = BeaconClient::new("  http://127.0.0.1:32794/\n", 5).unwrap();

        assert_eq!(client.api_url(), "http://127.0.0.1:32794");
    }

    #[test]
    fn keeps_url_without_trailing_slash() {
        let client = BeaconClient::new("http://127.0.0.1:32794", 5).unwrap();

        assert_eq!(client.api_url(), "http://127.0.0.1:32794");
    }

    #[tokio::test]
    async fn url_with_trailing_slash_still_hits_the_right_endpoint() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/node/syncing",
            json!({ "data": { "head_slot": "1" } }),
        )
        .await;

        let client = BeaconClient::new(format!("{}/", server.uri()), 5).unwrap();

        assert!(client.syncing().await.is_ok());
    }

    // ---- endpoints ------------------------------------------------------

    #[tokio::test]
    async fn version_reads_the_version_string() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/node/version",
            json!({ "data": { "version": "Lighthouse/v7.0.0-abc/x86_64-linux" } }),
        )
        .await;

        let version = client_for(&server).version().await.unwrap();

        assert_eq!(version, "Lighthouse/v7.0.0-abc/x86_64-linux");
    }

    #[tokio::test]
    async fn syncing_decodes_all_fields() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/node/syncing",
            json!({ "data": {
                "head_slot": "4096",
                "sync_distance": "12",
                "is_syncing": true,
                "is_optimistic": false,
                "el_offline": true
            }}),
        )
        .await;

        let status = client_for(&server).syncing().await.unwrap();

        assert_eq!(status.head_slot.as_deref(), Some("4096"));
        assert_eq!(status.sync_distance.as_deref(), Some("12"));
        assert_eq!(status.is_syncing, Some(true));
        assert_eq!(status.is_optimistic, Some(false));
        assert_eq!(status.el_offline, Some(true));
    }

    #[tokio::test]
    async fn syncing_tolerates_missing_fields() {
        let server = MockServer::start().await;
        serve_json(&server, "/eth/v1/node/syncing", json!({ "data": {} })).await;

        let status = client_for(&server).syncing().await.unwrap();

        assert!(status.head_slot.is_none());
        assert!(status.sync_distance.is_none());
        assert!(status.is_syncing.is_none());
        assert!(status.is_optimistic.is_none());
        assert!(status.el_offline.is_none());
    }

    #[tokio::test]
    async fn peer_count_decodes_counts() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/node/peer_count",
            json!({ "data": {
                "disconnected": "1",
                "connecting": "2",
                "connected": "3",
                "disconnecting": "4"
            }}),
        )
        .await;

        let peers = client_for(&server).peer_count().await.unwrap();

        assert_eq!(peers.disconnected.as_deref(), Some("1"));
        assert_eq!(peers.connecting.as_deref(), Some("2"));
        assert_eq!(peers.connected.as_deref(), Some("3"));
        assert_eq!(peers.disconnecting.as_deref(), Some("4"));
    }

    #[tokio::test]
    async fn head_header_decodes_nested_message() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/beacon/headers/head",
            json!({
                "execution_optimistic": false,
                "finalized": false,
                "data": {
                    "root": "0xabc",
                    "canonical": true,
                    "header": {
                        "message": {
                            "slot": "100",
                            "proposer_index": "7",
                            "parent_root": "0xparent",
                            "state_root": "0xstate",
                            "body_root": "0xbody"
                        },
                        "signature": "0xsig"
                    }
                }
            }),
        )
        .await;

        let header = client_for(&server).head_header().await.unwrap();
        let message = header.header.unwrap().message.unwrap();

        assert_eq!(header.root.as_deref(), Some("0xabc"));
        assert_eq!(header.canonical, Some(true));
        assert_eq!(message.slot.as_deref(), Some("100"));
        assert_eq!(message.proposer_index.as_deref(), Some("7"));
        assert_eq!(message.parent_root.as_deref(), Some("0xparent"));
    }

    #[tokio::test]
    async fn finality_checkpoints_decode_all_checkpoints() {
        let server = MockServer::start().await;
        serve_json(
            &server,
            "/eth/v1/beacon/states/head/finality_checkpoints",
            json!({ "data": {
                "previous_justified": { "epoch": "10", "root": "0xa" },
                "current_justified": { "epoch": "11", "root": "0xb" },
                "finalized": { "epoch": "9", "root": "0xc" }
            }}),
        )
        .await;

        let checkpoints = client_for(&server).finality_checkpoints().await.unwrap();

        let epoch = |checkpoint: Option<Checkpoint>| checkpoint.and_then(|c| c.epoch);

        assert_eq!(epoch(checkpoints.previous_justified).as_deref(), Some("10"));
        assert_eq!(epoch(checkpoints.current_justified).as_deref(), Some("11"));
        assert_eq!(epoch(checkpoints.finalized).as_deref(), Some("9"));
    }

    // ---- health -----------------------------------------------------------

    async fn health_for(status: u16) -> Result<u16, BeaconError> {
        let server = MockServer::start().await;
        serve_status(&server, "/eth/v1/node/health", status).await;

        client_for(&server).health().await
    }

    #[tokio::test]
    async fn health_accepts_ready_node() {
        assert_eq!(health_for(200).await.unwrap(), 200);
    }

    #[tokio::test]
    async fn health_reports_syncing_node_as_partial_content() {
        assert_eq!(health_for(206).await.unwrap(), 206);
    }

    #[tokio::test]
    async fn health_rejects_uninitialized_node() {
        let error = health_for(503).await.unwrap_err();

        assert!(matches!(error, BeaconError::InvalidResponse(_)));
        assert!(error.to_string().contains("503"));
    }

    // ---- failures ---------------------------------------------------------

    #[tokio::test]
    async fn error_status_is_an_http_error() {
        let server = MockServer::start().await;
        serve_status(&server, "/eth/v1/node/syncing", 500).await;

        let error = client_for(&server).syncing().await.unwrap_err();

        assert!(matches!(error, BeaconError::Http(_)));
    }

    #[tokio::test]
    async fn malformed_json_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/syncing"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let error = client_for(&server).syncing().await.unwrap_err();

        assert!(matches!(error, BeaconError::Http(_)));
    }

    #[tokio::test]
    async fn missing_data_wrapper_is_an_error() {
        let server = MockServer::start().await;
        serve_json(&server, "/eth/v1/node/syncing", json!({ "head_slot": "1" })).await;

        let error = client_for(&server).syncing().await.unwrap_err();

        assert!(matches!(error, BeaconError::Http(_)));
    }

    #[tokio::test]
    async fn slow_node_times_out() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/eth/v1/node/syncing"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "data": {} }))
                    .set_delay(Duration::from_secs(3)),
            )
            .mount(&server)
            .await;

        let client = BeaconClient::new(server.uri(), 1).unwrap();

        match client.syncing().await.unwrap_err() {
            BeaconError::Http(error) => assert!(error.is_timeout()),
            other => panic!("expected a timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unreachable_node_is_a_connection_error() {
        // Reserve a free port, then close it so nothing listens there.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let client = BeaconClient::new(format!("http://127.0.0.1:{port}"), 2).unwrap();

        match client.syncing().await.unwrap_err() {
            BeaconError::Http(error) => assert!(error.is_connect()),
            other => panic!("expected a connection error, got {other:?}"),
        }
    }
}
