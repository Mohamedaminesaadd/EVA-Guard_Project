use std::collections::HashSet;

use crate::geth::client::{GethClient, GethError};
use crate::models::observation::P2pMetrics;

/// Collects P2P metrics from a Geth node.
pub struct P2pCollector<'a> {
    client: &'a GethClient,
    previous_peer_ids: Option<HashSet<String>>,
}

impl<'a> P2pCollector<'a> {
    /// Creates a new P2P collector.
    pub fn new(client: &'a GethClient) -> Self {
        Self {
            client,
            previous_peer_ids: None,
        }
    }

    /// Collects P2P metrics and tracks peer changes.
    pub async fn collect(&mut self) -> Result<P2pMetrics, GethError> {
        // Get the total number of connected peers.
        let peer_count = self.client.peer_count().await?;

        // Check whether the node is listening for P2P connections.
        let listening = self.client.listening().await?;

        // Retrieve detailed peer information.
        let admin_reply = self.client.admin_peers().await;

        let (
            admin_peer_count,
            peer_details_available,
            new_peers,
            disconnected_peers,
            peer_churn_count,
        ) = match admin_reply {
            Ok(reply) => {
                let current_peer_ids: HashSet<String> =
                    reply.value.into_iter().map(|peer| peer.id).collect();

                let admin_count = current_peer_ids.len() as u64;

                let changes = self.previous_peer_ids.as_ref().map(|previous| {
                    let new_count = current_peer_ids.difference(previous).count() as u64;

                    let disconnected_count = previous.difference(&current_peer_ids).count() as u64;

                    (
                        new_count,
                        disconnected_count,
                        new_count + disconnected_count,
                    )
                });

                self.previous_peer_ids = Some(current_peer_ids);

                (
                    Some(admin_count),
                    Some(true),
                    changes.map(|value| value.0),
                    changes.map(|value| value.1),
                    changes.map(|value| value.2),
                )
            }

            Err(error) if error.is_method_unavailable() => (None, Some(false), None, None, None),

            Err(_) => (None, None, None, None, None),
        };

        Ok(P2pMetrics {
            peer_count: Some(peer_count.value),
            admin_peer_count,
            peer_details_available,
            new_peers,
            disconnected_peers,
            peer_churn_count,
            network_listening: Some(listening.value),
        })
    }
}
