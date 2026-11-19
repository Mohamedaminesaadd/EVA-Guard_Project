use super::parse_decimal_u64;
use crate::clients::beacon_client::BeaconClient;
use crate::models::metrics::AttestationInclusionMetrics;

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// Collects attestation inclusion delay metrics.
pub struct AttestationInclusionCollector {
    client: Client,
}

impl AttestationInclusionCollector {
    pub fn new(request_timeout_secs: u64) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;

        Ok(Self { client })
    }

    /// Collects the average attestation inclusion distance
    /// from the latest Beacon block.
    ///
    /// Unavailable measurements remain None.
    pub async fn collect(&self, beacon: &BeaconClient) -> AttestationInclusionMetrics {
        let mut metrics = AttestationInclusionMetrics::default();

        // 1. Retrieve the slot of the latest Beacon block.
        let Some(inclusion_slot) = head_slot(beacon).await else {
            return metrics;
        };

        // 2. Retrieve the latest Beacon block.
        let Some(block) = self.fetch_block(beacon, inclusion_slot).await else {
            return metrics;
        };

        // 3. Calculate the average inclusion distance of its attestations.
        metrics.attestation_inclusion_distance = extract_attestations(&block)
            .and_then(|attestations| average_inclusion_distance(attestations, inclusion_slot));

        metrics
    }

    /// Fetches the Beacon block of a slot. Any failure gives None.
    async fn fetch_block(&self, beacon: &BeaconClient, slot: u64) -> Option<Value> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", beacon.api_url(), slot);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;

        response.json::<Value>().await.ok()
    }
}

/// Returns the slot of the current head block, if available.
async fn head_slot(beacon: &BeaconClient) -> Option<u64> {
    let header = beacon.head_header().await.ok()?;

    header
        .header?
        .message?
        .slot
        .as_deref()
        .and_then(parse_decimal_u64)
}

/// Returns the attestations of a `/eth/v2/beacon/blocks/{id}` answer.
fn extract_attestations(block: &Value) -> Option<&[Value]> {
    block
        .get("data")?
        .get("message")?
        .get("body")?
        .get("attestations")?
        .as_array()
        .map(Vec::as_slice)
}

/// Calculates the average inclusion distance of a block's attestations.
///
/// The distance is `inclusion_slot - attestation.data.slot`. Attestations
/// whose slot is missing, invalid or later than the inclusion slot are
/// ignored. Returns None when no attestation is usable.
fn average_inclusion_distance(attestations: &[Value], inclusion_slot: u64) -> Option<f64> {
    let mut total_distance: u128 = 0;
    let mut valid_attestations: u64 = 0;

    for attestation in attestations {
        let Some(attestation_slot) = attestation
            .get("data")
            .and_then(|data| data.get("slot"))
            .and_then(Value::as_str)
            .and_then(parse_decimal_u64)
        else {
            continue;
        };

        // Ignore invalid future attestation slots.
        let Some(distance) = inclusion_slot.checked_sub(attestation_slot) else {
            continue;
        };

        total_distance += u128::from(distance);
        valid_attestations += 1;
    }

    if valid_attestations == 0 {
        return None;
    }

    Some(total_distance as f64 / valid_attestations as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::beacon::test_support::{
        client_for, mock_garbage, mock_json, mock_status, unreachable_client,
    };

    use serde_json::json;
    use wiremock::MockServer;

    const HEAD_ROUTE: &str = "/eth/v1/beacon/headers/head";

    fn attestation(slot: &str) -> Value {
        json!({ "aggregation_bits": "0x01", "data": { "slot": slot, "index": "0" } })
    }

    fn attestations(slots: &[&str]) -> Vec<Value> {
        slots.iter().map(|slot| attestation(slot)).collect()
    }

    fn block_body(attestations: Vec<Value>) -> Value {
        json!({
            "version": "electra",
            "data": { "message": { "slot": "100", "body": { "attestations": attestations } } }
        })
    }

    fn head_body(slot: &str) -> Value {
        json!({ "data": {
            "root": "0xabc",
            "canonical": true,
            "header": { "message": { "slot": slot } }
        }})
    }

    // ---- average_inclusion_distance --------------------------------------

    #[test]
    fn calculates_inclusion_distance() {
        let list = attestations(&["96"]);

        assert_eq!(average_inclusion_distance(&list, 100), Some(4.0));
    }

    #[test]
    fn calculates_average_inclusion_distance() {
        // Distances 2, 4 and 6.
        let list = attestations(&["98", "96", "94"]);

        assert_eq!(average_inclusion_distance(&list, 100), Some(4.0));
    }

    #[test]
    fn optimal_inclusion_has_distance_one() {
        let list = attestations(&["99", "99", "99"]);

        assert_eq!(average_inclusion_distance(&list, 100), Some(1.0));
    }

    #[test]
    fn ignores_future_attestations() {
        let list = attestations(&["105", "98"]);

        assert_eq!(average_inclusion_distance(&list, 100), Some(2.0));
    }

    #[test]
    fn only_future_attestations_give_none() {
        let list = attestations(&["105", "101"]);

        assert_eq!(average_inclusion_distance(&list, 100), None);
    }

    #[test]
    fn ignores_attestations_with_invalid_slots() {
        let list = vec![
            attestation("invalid"),
            attestation("-3"),
            json!({ "data": {} }),
            json!({ "data": { "slot": 98 } }),
            json!({}),
            attestation("96"),
        ];

        assert_eq!(average_inclusion_distance(&list, 100), Some(4.0));
    }

    #[test]
    fn handles_empty_attestations() {
        assert_eq!(average_inclusion_distance(&[], 100), None);
    }

    #[test]
    fn huge_slots_do_not_overflow() {
        let max = u64::MAX.to_string();
        let list = attestations(&["0", "0", max.as_str()]);

        let average = average_inclusion_distance(&list, u64::MAX).unwrap();

        // (MAX + MAX + 0) / 3
        assert!((average - (u64::MAX as f64 * 2.0 / 3.0)).abs() / average < 1e-9);
    }

    // ---- extract_attestations -----------------------------------------------

    #[test]
    fn extracts_attestations_from_a_block() {
        let block = block_body(attestations(&["96", "97"]));

        assert_eq!(extract_attestations(&block).map(<[Value]>::len), Some(2));
    }

    #[test]
    fn extracts_an_empty_attestation_list() {
        let block = block_body(vec![]);

        assert_eq!(extract_attestations(&block).map(<[Value]>::len), Some(0));
    }

    #[test]
    fn missing_or_malformed_body_gives_none() {
        assert!(extract_attestations(&json!({})).is_none());
        assert!(extract_attestations(&json!({ "data": { "message": {} } })).is_none());
        assert!(extract_attestations(&json!({
            "data": { "message": { "body": { "attestations": "x" } } }
        }))
        .is_none());
        assert!(extract_attestations(&Value::Null).is_none());
    }

    // ---- collect (HTTP) ---------------------------------------------------

    #[tokio::test]
    async fn collects_average_distance_from_the_head_block() {
        let server = MockServer::start().await;
        mock_json(&server, HEAD_ROUTE, head_body("100")).await;
        // Distances 1, 2 and 4.
        mock_json(
            &server,
            "/eth/v2/beacon/blocks/100",
            block_body(attestations(&["99", "98", "96"])),
        )
        .await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        let distance = metrics.attestation_inclusion_distance.unwrap();
        assert!((distance - 7.0 / 3.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn block_without_attestations_gives_none() {
        let server = MockServer::start().await;
        mock_json(&server, HEAD_ROUTE, head_body("100")).await;
        mock_json(&server, "/eth/v2/beacon/blocks/100", block_body(vec![])).await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }

    #[tokio::test]
    async fn head_without_a_slot_gives_none() {
        let server = MockServer::start().await;
        mock_json(&server, HEAD_ROUTE, json!({ "data": { "root": "0xabc" } })).await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }

    #[tokio::test]
    async fn head_error_gives_none() {
        let server = MockServer::start().await;
        mock_status(&server, HEAD_ROUTE, 500).await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }

    #[tokio::test]
    async fn missing_block_gives_none() {
        let server = MockServer::start().await;
        mock_json(&server, HEAD_ROUTE, head_body("100")).await;
        mock_status(&server, "/eth/v2/beacon/blocks/100", 404).await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }

    #[tokio::test]
    async fn malformed_block_json_gives_none() {
        let server = MockServer::start().await;
        mock_json(&server, HEAD_ROUTE, head_body("100")).await;
        mock_garbage(&server, "/eth/v2/beacon/blocks/100").await;

        let collector = AttestationInclusionCollector::new(5).unwrap();
        let metrics = collector.collect(&client_for(&server)).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }

    #[tokio::test]
    async fn unreachable_node_gives_none() {
        let collector = AttestationInclusionCollector::new(2).unwrap();
        let metrics = collector.collect(&unreachable_client()).await;

        assert_eq!(metrics.attestation_inclusion_distance, None);
    }
}
