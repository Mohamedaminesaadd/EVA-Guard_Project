use super::{signing_totals, SIGNED_BLOCKS};
use crate::clients::validator_client::{PrometheusMetric, ValidatorClient};
use crate::models::metrics::BlockProposalMetrics;

/// Collects block proposal metrics.
pub struct ProposalsCollector;

impl ProposalsCollector {
    /// Collects block proposal metrics from one Validator Client.
    ///
    /// Scrapes the Prometheus endpoint. To share one scrape between several
    /// collectors, call `collect_metrics` once and use `from_metrics`.
    pub async fn collect(client: &ValidatorClient) -> BlockProposalMetrics {
        match client.collect_metrics().await {
            Ok(samples) => Self::from_metrics(&samples),
            Err(_) => BlockProposalMetrics::default(),
        }
    }

    /// Computes the metrics from already scraped Prometheus samples.
    pub fn from_metrics(samples: &[PrometheusMetric]) -> BlockProposalMetrics {
        let mut metrics = BlockProposalMetrics::default();
        apply_proposals(&mut metrics, samples);
        metrics
    }
}

/// Fills the block proposal metrics from `vc_signed_beacon_blocks_total`.
///
/// `blocks_proposed_total` is the number of blocks the Validator Client
/// signed successfully (`status="success"`); a signed block is not
/// necessarily accepted by the network. `block_proposal_failures_total`
/// counts the refused signings (any other status). Both are None while the
/// counter has not been exposed, which is normal until the first proposal.
fn apply_proposals(metrics: &mut BlockProposalMetrics, samples: &[PrometheusMetric]) {
    let totals = signing_totals(samples, SIGNED_BLOCKS);

    metrics.blocks_proposed_total = totals.success;
    metrics.block_proposal_failures_total = totals.refused;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::validator::test_support::{lighthouse_scrape, sample, signed};

    const NAME: &str = "vc_signed_beacon_blocks_total";

    #[test]
    fn collects_successful_proposals() {
        let metrics = ProposalsCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.blocks_proposed_total, Some(3));
    }

    #[test]
    fn collects_failed_proposals() {
        let samples = vec![
            signed(NAME, "success", 12.0),
            signed(NAME, "slashable", 2.0),
            signed(NAME, "unregistered", 1.0),
        ];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, Some(12));
        assert_eq!(metrics.block_proposal_failures_total, Some(3));
    }

    #[test]
    fn healthy_validator_has_zero_failed_proposals() {
        let samples = vec![signed(NAME, "success", 4.0)];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, Some(4));
        assert_eq!(metrics.block_proposal_failures_total, Some(0));
    }

    #[test]
    fn only_failures_means_zero_proposed_blocks() {
        let samples = vec![signed(NAME, "slashable", 1.0)];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, Some(0));
        assert_eq!(metrics.block_proposal_failures_total, Some(1));
    }

    #[test]
    fn returns_none_before_the_first_proposal() {
        // The counter vector has no sample until the first increment.
        let samples = vec![sample("vc_validators_total_count", &[], 4.0)];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, None);
        assert_eq!(metrics.block_proposal_failures_total, None);
    }

    #[test]
    fn rejects_negative_values() {
        let samples = vec![signed(NAME, "success", -1.0)];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, None);
    }

    #[test]
    fn attestation_counters_are_not_mixed_in() {
        let samples = vec![
            signed("vc_signed_attestations_total", "success", 999.0),
            signed(NAME, "success", 2.0),
        ];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, Some(2));
    }

    #[test]
    fn old_guessed_metric_names_are_not_used() {
        let samples = vec![
            sample("validator_blocks_proposed_total", &[], 12.0),
            sample("validator_block_proposal_failures_total", &[], 2.0),
        ];

        let metrics = ProposalsCollector::from_metrics(&samples);

        assert_eq!(metrics.blocks_proposed_total, None);
        assert_eq!(metrics.block_proposal_failures_total, None);
    }
}
