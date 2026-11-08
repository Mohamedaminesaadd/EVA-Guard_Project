//! Geth JSON-RPC client (Phase 3).
//!
//! One method = one HTTP POST. Every successful call returns the decoded value
//! together with its end-to-end latency (request sent -> body fully received).

use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::Config;

const JSON_RPC_VERSION: &str = "2.0";
/// Each HTTP request carries exactly one JSON-RPC call, so a fixed id is enough.
const REQUEST_ID: u64 = 1;
/// JSON-RPC "Method not found" (returned by Geth when an API namespace is disabled).
const METHOD_NOT_FOUND: i64 = -32601;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum GethError {
    #[error("failed to build HTTP client: {0}")]
    ClientBuild(#[source] reqwest::Error),

    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    #[error("cannot connect to Geth: {0}")]
    Connection(#[source] reqwest::Error),

    #[error("unexpected HTTP status {0}")]
    HttpStatus(StatusCode),

    #[error("HTTP transport error: {0}")]
    Transport(#[source] reqwest::Error),

    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("invalid JSON-RPC response: {0}")]
    InvalidResponse(String),
}

impl GethError {
    /// True when Geth answered "method not found" (e.g. the `admin` namespace is disabled).
    pub fn is_method_unavailable(&self) -> bool {
        matches!(self, GethError::Rpc { code, .. } if *code == METHOD_NOT_FOUND)
    }
}

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A decoded RPC value plus the latency of the call that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcReply<T> {
    pub value: T,
    pub latency: Duration,
}

/// Raw content of `eth_syncing`. Interpretation is left to the blockchain collector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// `eth_syncing` returned `false`.
    NotSyncing,
    /// `eth_syncing` returned a progress object. Fields absent from the object are `None`.
    Syncing {
        starting_block: Option<u64>,
        current_block: Option<u64>,
        highest_block: Option<u64>,
    },
}

/// One entry of `admin_peers`. Only the fields EVA-Guard needs are decoded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PeerInfo {
    /// Node id: stable across reconnections, so it is the peer identity.
    pub id: String,
    pub name: Option<String>,
    pub enode: Option<String>,
    pub network: Option<PeerNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PeerNetwork {
    #[serde(rename = "remoteAddress")]
    pub remote_address: Option<String>,
    pub inbound: Option<bool>,
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: &'a [Value],
    id: u64,
}

#[derive(Deserialize)]
struct RpcResponse {
    #[serde(default)]
    id: Value,
    result: Option<Value>,
    error: Option<RpcErrorObject>,
}

#[derive(Deserialize)]
struct RpcErrorObject {
    code: i64,
    message: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct GethClient {
    http: reqwest::Client,
    url: String,
    timeout: Duration,
}

impl GethClient {
    pub fn new(config: &Config) -> Result<Self, GethError> {
        let http = reqwest::Client::builder()
            .timeout(config.rpc_timeout)
            // The node is monitored directly: never route this traffic through a system proxy.
            .no_proxy()
            .build()
            .map_err(GethError::ClientBuild)?;
        Ok(Self {
            http,
            url: config.rpc_url.clone(),
            timeout: config.rpc_timeout,
        })
    }

    /// `eth_blockNumber`: latest block number.
    pub async fn block_number(&self) -> Result<RpcReply<u64>, GethError> {
        self.quantity("eth_blockNumber").await
    }

    /// `net_peerCount`: number of connected peers.
    pub async fn peer_count(&self) -> Result<RpcReply<u64>, GethError> {
        self.quantity("net_peerCount").await
    }

    /// `net_listening`: whether the node accepts incoming P2P connections.
    pub async fn listening(&self) -> Result<RpcReply<bool>, GethError> {
        self.call("net_listening", &[]).await
    }

    /// `eth_syncing`: synchronization progress, uninterpreted.
    pub async fn syncing(&self) -> Result<RpcReply<SyncStatus>, GethError> {
        let reply: RpcReply<Value> = self.call("eth_syncing", &[]).await?;
        Ok(RpcReply {
            value: parse_sync_status(&reply.value)?,
            latency: reply.latency,
        })
    }

    /// `admin_peers`: detailed peer list. Requires the `admin` API namespace on Geth.
    pub async fn admin_peers(&self) -> Result<RpcReply<Vec<PeerInfo>>, GethError> {
        self.call("admin_peers", &[]).await
    }

    /// Generic JSON-RPC call: serialize, send, check HTTP status, decode, measure latency.
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<RpcReply<T>, GethError> {
        let request = RpcRequest {
            jsonrpc: JSON_RPC_VERSION,
            method,
            params,
            id: REQUEST_ID,
        };

        let started = Instant::now();
        let response = self
            .http
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.classify(e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(GethError::HttpStatus(status));
        }
        let body = response.bytes().await.map_err(|e| self.classify(e))?;
        let latency = started.elapsed();

        let parsed: RpcResponse = serde_json::from_slice(&body)
            .map_err(|e| GethError::InvalidResponse(format!("malformed JSON: {e}")))?;

        // An error object takes precedence: JSON-RPC allows `id: null` on errors.
        if let Some(error) = parsed.error {
            return Err(GethError::Rpc {
                code: error.code,
                message: error.message,
            });
        }
        if parsed.id != Value::from(REQUEST_ID) {
            return Err(GethError::InvalidResponse(format!(
                "response id {} does not match request id {REQUEST_ID}",
                parsed.id
            )));
        }
        let result = parsed
            .result
            .ok_or_else(|| GethError::InvalidResponse("missing `result` field".to_string()))?;
        let value = serde_json::from_value(result)
            .map_err(|e| GethError::InvalidResponse(format!("unexpected `result` shape: {e}")))?;

        Ok(RpcReply { value, latency })
    }

    async fn quantity(&self, method: &str) -> Result<RpcReply<u64>, GethError> {
        let reply: RpcReply<String> = self.call(method, &[]).await?;
        Ok(RpcReply {
            value: parse_quantity(&reply.value)?,
            latency: reply.latency,
        })
    }

    fn classify(&self, err: reqwest::Error) -> GethError {
        if err.is_timeout() {
            GethError::Timeout(self.timeout)
        } else if err.is_connect() {
            GethError::Connection(err)
        } else {
            GethError::Transport(err)
        }
    }
}

// ---------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------

/// Decode an Ethereum JSON-RPC quantity: `0x` followed by at least one hex digit.
fn parse_quantity(raw: &str) -> Result<u64, GethError> {
    let invalid = || GethError::InvalidResponse(format!("not a valid hex quantity: `{raw}`"));
    let digits = raw.strip_prefix("0x").ok_or_else(invalid)?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    u64::from_str_radix(digits, 16).map_err(|_| invalid())
}

fn parse_quantity_value(field: &str, value: &Value) -> Result<u64, GethError> {
    let text = value
        .as_str()
        .ok_or_else(|| GethError::InvalidResponse(format!("`{field}` is not a string: {value}")))?;
    parse_quantity(text)
}

fn parse_sync_status(value: &Value) -> Result<SyncStatus, GethError> {
    match value {
        Value::Bool(false) => Ok(SyncStatus::NotSyncing),
        Value::Object(map) => {
            let field = |name: &str| -> Result<Option<u64>, GethError> {
                map.get(name)
                    .map(|v| parse_quantity_value(name, v))
                    .transpose()
            };
            Ok(SyncStatus::Syncing {
                starting_block: field("startingBlock")?,
                current_block: field("currentBlock")?,
                highest_block: field("highestBlock")?,
            })
        }
        other => Err(GethError::InvalidResponse(format!(
            "unexpected eth_syncing result: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn is_invalid_response<T: std::fmt::Debug>(result: Result<T, GethError>) -> bool {
        matches!(result, Err(GethError::InvalidResponse(_)))
    }

    #[test]
    fn parses_valid_quantities() {
        assert_eq!(parse_quantity("0x0").unwrap(), 0);
        assert_eq!(parse_quantity("0x1b4").unwrap(), 436);
        assert_eq!(parse_quantity("0xFF").unwrap(), 255);
        assert_eq!(parse_quantity("0xffffffffffffffff").unwrap(), u64::MAX);
    }

    #[test]
    fn rejects_invalid_quantities() {
        assert!(is_invalid_response(parse_quantity("1b4")));
        assert!(is_invalid_response(parse_quantity("0x")));
        assert!(is_invalid_response(parse_quantity("0xzz")));
        assert!(is_invalid_response(parse_quantity("0x+1")));
        assert!(is_invalid_response(parse_quantity("")));
        // 17 hex digits overflow u64
        assert!(is_invalid_response(parse_quantity("0x10000000000000000")));
    }

    #[test]
    fn sync_false_means_not_syncing() {
        assert_eq!(
            parse_sync_status(&json!(false)).unwrap(),
            SyncStatus::NotSyncing
        );
    }

    #[test]
    fn sync_object_is_decoded() {
        let status = parse_sync_status(&json!({
            "startingBlock": "0x0",
            "currentBlock": "0x64",
            "highestBlock": "0xc8",
            "pulledStates": "0x10"
        }))
        .unwrap();
        assert_eq!(
            status,
            SyncStatus::Syncing {
                starting_block: Some(0),
                current_block: Some(100),
                highest_block: Some(200),
            }
        );
    }

    #[test]
    fn sync_object_with_missing_fields_keeps_none() {
        let status = parse_sync_status(&json!({ "currentBlock": "0x5" })).unwrap();
        assert_eq!(
            status,
            SyncStatus::Syncing {
                starting_block: None,
                current_block: Some(5),
                highest_block: None,
            }
        );
    }

    #[test]
    fn sync_rejects_unexpected_shapes() {
        assert!(is_invalid_response(parse_sync_status(&json!(true))));
        assert!(is_invalid_response(parse_sync_status(&json!("0x1"))));
        assert!(is_invalid_response(parse_sync_status(
            &json!({ "currentBlock": "oops" })
        )));
        assert!(is_invalid_response(parse_sync_status(
            &json!({ "currentBlock": 5 })
        )));
    }

    #[test]
    fn peer_info_is_decoded_and_ignores_extra_fields() {
        let peers: Vec<PeerInfo> = serde_json::from_value(json!([
            {
                "id": "aaaa",
                "name": "Geth/v1.14.0",
                "enode": "enode://aaaa@1.2.3.4:30303",
                "caps": ["eth/68"],
                "network": {
                    "localAddress": "10.0.0.1:30303",
                    "remoteAddress": "1.2.3.4:30303",
                    "inbound": true,
                    "trusted": false,
                    "static": false
                },
                "protocols": {}
            },
            { "id": "bbbb" }
        ]))
        .unwrap();

        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].id, "aaaa");
        assert_eq!(
            peers[0].network.as_ref().unwrap().remote_address.as_deref(),
            Some("1.2.3.4:30303")
        );
        assert_eq!(peers[0].network.as_ref().unwrap().inbound, Some(true));
        assert_eq!(peers[1].name, None);
        assert_eq!(peers[1].network, None);
    }

    #[test]
    fn method_not_found_is_reported_as_unavailable() {
        let unavailable = GethError::Rpc {
            code: -32601,
            message: "the method admin_peers does not exist/is not available".to_string(),
        };
        let other = GethError::Rpc {
            code: -32000,
            message: "boom".to_string(),
        };
        assert!(unavailable.is_method_unavailable());
        assert!(!other.is_method_unavailable());
        assert!(!GethError::Timeout(Duration::from_secs(1)).is_method_unavailable());
    }
}
