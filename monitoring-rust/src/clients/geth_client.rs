
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GethError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Geth JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("Invalid JSON-RPC response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: Vec<Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct RpcReply<T> {
    jsonrpc: Option<String>,
    id: Option<Value>,
    result: Option<T>,
    error: Option<RpcResponseError>,
}

#[derive(Debug, Deserialize)]
struct RpcResponseError {
    code: i64,
    message: String,
}

#[derive(Debug, Clone)]
pub struct RpcResult<T> {
    pub value: T,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub starting_block: Option<String>,
    pub current_block: Option<String>,
    pub highest_block: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub network: Option<PeerNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerNetwork {
    #[serde(rename = "localAddress")]
    pub local_address: Option<String>,

    #[serde(rename = "remoteAddress")]
    pub remote_address: Option<String>,

    pub inbound: Option<bool>,
    pub trusted: Option<bool>,

    #[serde(rename = "static")]
    pub static_: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GethClient {
    client: Client,
    rpc_url: String,
    metrics_url: String,
}

impl GethClient {
    /// Creates a Geth client with JSON-RPC and Prometheus endpoints.
    pub fn new(
        rpc_url: impl Into<String>,
        metrics_url: impl Into<String>,
        request_timeout_secs: u64,
    ) -> Result<Self, GethError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;

        Ok(Self {
            client,
            rpc_url: rpc_url.into(),
            metrics_url: metrics_url.into(),
        })
    }

    /// Executes a generic JSON-RPC request and measures its latency.
    pub async fn call<T>(
        &self,
        method: &str,
        params: Vec<Value>,
    ) -> Result<RpcResult<T>, GethError>
    where
        T: DeserializeOwned,
    {
        let request = RpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id: 1,
        };

        let start = Instant::now();

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?;

        let reply: RpcReply<T> = response.json().await?;
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        if let Some(error) = reply.error {
            return Err(GethError::Rpc {
                code: error.code,
                message: error.message,
            });
        }

        if reply.jsonrpc.as_deref() != Some("2.0") {
            return Err(GethError::InvalidResponse(
                "Missing or invalid JSON-RPC version".to_string(),
            ));
        }

        if reply.id.as_ref() != Some(&json!(1)) {
            return Err(GethError::InvalidResponse(
                "JSON-RPC response ID does not match request ID".to_string(),
            ));
        }

        let value = reply.result.ok_or_else(|| {
            GethError::InvalidResponse(
                "Response contains neither result nor error".to_string(),
            )
        })?;

        Ok(RpcResult {
            value,
            latency_ms,
        })
    }

    /// Returns the current execution block number.
    pub async fn block_number(&self) -> Result<RpcResult<u64>, GethError> {
        let result: RpcResult<String> =
            self.call("eth_blockNumber", vec![]).await?;

        Ok(RpcResult {
            value: parse_hex_u64(&result.value)?,
            latency_ms: result.latency_ms,
        })
    }

    /// Returns the number of connected P2P peers.
    pub async fn peer_count(&self) -> Result<RpcResult<u64>, GethError> {
        let result: RpcResult<String> =
            self.call("net_peerCount", vec![]).await?;

        Ok(RpcResult {
            value: parse_hex_u64(&result.value)?,
            latency_ms: result.latency_ms,
        })
    }

    /// Checks whether P2P networking is listening.
    pub async fn listening(&self) -> Result<RpcResult<bool>, GethError> {
        self.call("net_listening", vec![]).await
    }

    /// Returns the raw eth_syncing result.
    pub async fn syncing(&self) -> Result<RpcResult<Value>, GethError> {
        self.call("eth_syncing", vec![]).await
    }

    /// Returns None when synchronized, Some(status) when syncing.
    pub async fn syncing_status(
        &self,
    ) -> Result<RpcResult<Option<SyncStatus>>, GethError> {
        let result: RpcResult<Value> =
            self.call("eth_syncing", vec![]).await?;

        let status = match result.value {
            Value::Bool(false) => None,
            Value::Object(_) => {
                Some(serde_json::from_value(result.value)?)
            }
            other => {
                return Err(GethError::InvalidResponse(format!(
                    "Unexpected eth_syncing response: {}",
                    other
                )));
            }
        };

        Ok(RpcResult {
            value: status,
            latency_ms: result.latency_ms,
        })
    }

    /// Returns connected peers.
    /// Requires the admin namespace to be enabled in Geth.
    pub async fn admin_peers(
        &self,
    ) -> Result<RpcResult<Vec<PeerInfo>>, GethError> {
        self.call("admin_peers", vec![]).await
    }

    /// Retrieves raw Prometheus metrics exposed by Geth.
    pub async fn get_prometheus_metrics(&self) -> Result<String, GethError> {
        let response = self
            .client
            .get(&self.metrics_url)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.text().await?)
    }

    /// Returns the configured JSON-RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Returns the configured Prometheus URL.
    pub fn metrics_url(&self) -> &str {
        &self.metrics_url
    }
}

/// Converts a hexadecimal quantity such as "0x1b7" into u64.
pub fn parse_hex_u64(value: &str) -> Result<u64, GethError> {
    let hex = value.strip_prefix("0x").unwrap_or(value);

    if hex.is_empty() {
        return Err(GethError::InvalidResponse(
            "Empty hexadecimal quantity".to_string(),
        ));
    }

    u64::from_str_radix(hex, 16).map_err(|error| {
        GethError::InvalidResponse(format!(
            "Invalid hexadecimal quantity '{}': {}",
            value, error
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_block_number() {
        assert_eq!(parse_hex_u64("0x1b7").unwrap(), 439);
    }

    #[test]
    fn parses_zero() {
        assert_eq!(parse_hex_u64("0x0").unwrap(), 0);
    }

    #[test]
    fn rejects_invalid_hex() {
        assert!(parse_hex_u64("0xxyz").is_err());
    }

    #[test]
    fn parses_hex_without_prefix() {
        assert_eq!(parse_hex_u64("1b7").unwrap(), 439);
    }

    #[test]
    fn rejects_empty_hex() {
        assert!(parse_hex_u64("0x").is_err());
    }
}
