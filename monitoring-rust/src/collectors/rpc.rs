use crate::geth::client::GethClient;
use crate::models::observation::RpcMetrics;

/// Collects JSON-RPC availability and latency metrics.
pub struct RpcCollector<'a> {
    client: &'a GethClient,
}

impl<'a> RpcCollector<'a> {
    /// Creates a new RPC collector.
    pub fn new(client: &'a GethClient) -> Self {
        Self { client }
    }

    /// Collects RPC availability and latency.
    pub async fn collect(&self) -> RpcMetrics {
        match self.client.block_number().await {
            Ok(reply) => RpcMetrics {
                rpc_available: Some(true),
                rpc_latency_ms: Some(reply.latency.as_secs_f64() * 1000.0),
            },
            Err(_) => RpcMetrics {
                rpc_available: Some(false),
                rpc_latency_ms: None,
            },
        }
    }
}
