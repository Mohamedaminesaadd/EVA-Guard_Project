use super::{
    sum_family, sum_labelled, ATTESTER_COUNT, BEACON_NODES_AVAILABLE, PROPOSER_COUNT,
    TASK_CURRENT_EPOCH, TASK_LABEL, VALIDATORS_ENABLED, VALIDATORS_TOTAL,
};
use crate::clients::validator_client::{PrometheusMetric, ValidatorClient};
use crate::models::metrics::ValidatorDutiesMetrics;

/// Collects validator and duty counts.
pub struct DutiesCollector;

impl DutiesCollector {
    /// Collects validator and duty counts from one Validator Client.
    ///
    /// Scrapes the Prometheus endpoint. To share one scrape between several
    /// collectors, call `collect_metrics` once and use `from_metrics`.
    pub async fn collect(client: &ValidatorClient) -> ValidatorDutiesMetrics {
        match client.collect_metrics().await {
            Ok(samples) => Self::from_metrics(&samples),
            Err(_) => ValidatorDutiesMetrics::default(),
        }
    }

    /// Computes the metrics from already scraped Prometheus samples.
    pub fn from_metrics(samples: &[PrometheusMetric]) -> ValidatorDutiesMetrics {
        let mut metrics = ValidatorDutiesMetrics::default();
        apply_duties(&mut metrics, samples);
        metrics
    }
}

/// Fills the validator and duty counts.
///
/// All values are gauges, not cumulative counters:
/// - `validators_total`   <- `vc_validators_total_count`
/// - `validators_enabled` <- `vc_validators_enabled_count`
/// - `attester_duties`    <- `vc_beacon_attester_count{task="current_epoch"}`
/// - `proposer_duties`    <- `vc_beacon_block_proposer_count{task="current_epoch"}`
/// - `beacon_node_online` <- `vc_beacon_nodes_available_count > 0`
///
/// The duty counts are the number of duties of this host in the current
/// epoch; the `next_epoch` sample is deliberately ignored, since adding
/// the two epochs together would double the count.
///
/// The Validator Client refreshes the availability gauge once per slot and
/// starts it at 0, so `beacon_node_online` can be `Some(false)` during the
/// first seconds after a start.
fn apply_duties(metrics: &mut ValidatorDutiesMetrics, samples: &[PrometheusMetric]) {
    metrics.validators_total = sum_family(samples, VALIDATORS_TOTAL);
    metrics.validators_enabled = sum_family(samples, VALIDATORS_ENABLED);

    metrics.beacon_node_online =
        sum_family(samples, BEACON_NODES_AVAILABLE).map(|available| available > 0);

    metrics.attester_duties = sum_labelled(samples, ATTESTER_COUNT, TASK_LABEL, TASK_CURRENT_EPOCH);

    metrics.proposer_duties = sum_labelled(samples, PROPOSER_COUNT, TASK_LABEL, TASK_CURRENT_EPOCH);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::validator::test_support::{lighthouse_scrape, sample};

    #[test]
    fn collects_all_counts_from_a_realistic_scrape() {
        let metrics = DutiesCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.validators_total, Some(4));
        assert_eq!(metrics.validators_enabled, Some(3));
        assert_eq!(metrics.attester_duties, Some(4));
        assert_eq!(metrics.proposer_duties, Some(1));
        assert_eq!(metrics.beacon_node_online, Some(true));
    }

    #[test]
    fn beacon_node_is_online_when_at_least_one_is_available() {
        let samples = vec![sample("vc_beacon_nodes_available_count", &[], 2.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.beacon_node_online, Some(true));
    }

    #[test]
    fn beacon_node_is_offline_when_none_is_available() {
        let samples = vec![
            sample("vc_beacon_nodes_total_count", &[], 1.0),
            sample("vc_beacon_nodes_available_count", &[], 0.0),
        ];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.beacon_node_online, Some(false));
    }

    #[test]
    fn unknown_beacon_node_state_is_not_reported_as_offline() {
        // Only the total is exposed: nothing says whether a node is available.
        let samples = vec![sample("vc_beacon_nodes_total_count", &[], 1.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.beacon_node_online, None);
    }

    #[test]
    fn invalid_availability_is_not_reported_as_offline() {
        let samples = vec![sample("vc_beacon_nodes_available_count", &[], -1.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.beacon_node_online, None);
    }

    #[test]
    fn collects_validator_count() {
        let samples = vec![sample("vc_validators_total_count", &[], 10.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.validators_total, Some(10));
        assert_eq!(metrics.validators_enabled, None);
    }

    #[test]
    fn collects_enabled_validators() {
        let samples = vec![sample("vc_validators_enabled_count", &[], 8.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.validators_enabled, Some(8));
        assert_eq!(metrics.validators_total, None);
    }

    #[test]
    fn collects_attester_duties_of_the_current_epoch() {
        let samples = vec![
            sample(
                "vc_beacon_attester_count",
                &[("task", "current_epoch")],
                32.0,
            ),
            sample("vc_beacon_attester_count", &[("task", "next_epoch")], 31.0),
        ];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.attester_duties, Some(32));
    }

    #[test]
    fn collects_proposer_duties_of_the_current_epoch() {
        let samples = vec![sample(
            "vc_beacon_block_proposer_count",
            &[("task", "current_epoch")],
            4.0,
        )];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.proposer_duties, Some(4));
    }

    #[test]
    fn next_epoch_alone_is_not_reported_as_current() {
        let samples = vec![sample(
            "vc_beacon_attester_count",
            &[("task", "next_epoch")],
            9.0,
        )];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.attester_duties, None);
    }

    #[test]
    fn zero_duties_are_reported_as_zero() {
        let samples = vec![
            sample(
                "vc_beacon_attester_count",
                &[("task", "current_epoch")],
                0.0,
            ),
            sample(
                "vc_beacon_block_proposer_count",
                &[("task", "current_epoch")],
                0.0,
            ),
        ];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.attester_duties, Some(0));
        assert_eq!(metrics.proposer_duties, Some(0));
    }

    #[test]
    fn returns_none_when_metrics_are_missing() {
        let samples = vec![sample("some_other_metric", &[], 10.0)];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics, ValidatorDutiesMetrics::default());
    }

    #[test]
    fn invalid_value_only_affects_its_own_field() {
        let samples = vec![
            sample("vc_validators_total_count", &[], -1.0),
            sample("vc_validators_enabled_count", &[], 8.0),
        ];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics.validators_total, None);
        assert_eq!(metrics.validators_enabled, Some(8));
    }

    #[test]
    fn old_guessed_metric_names_are_not_used() {
        let samples = vec![
            sample("validator_count", &[], 10.0),
            sample("validator_enabled_count", &[], 8.0),
            sample("validator_attester_duties_total", &[], 32.0),
            sample("validator_proposer_duties_total", &[], 4.0),
            sample("validator_beacon_node_online", &[], 1.0),
        ];

        let metrics = DutiesCollector::from_metrics(&samples);

        assert_eq!(metrics, ValidatorDutiesMetrics::default());
    }
}
