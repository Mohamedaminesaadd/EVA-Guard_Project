use super::{signing_totals, SIGNED_ATTESTATIONS, SIGNED_BLOCKS};
use crate::clients::validator_client::{PrometheusMetric, ValidatorClient};
use crate::models::metrics::EffectivenessMetrics;

/// Collects attestation and proposal effectiveness.
///
/// Effectiveness is `successful / expected`, a ratio between 0 and 1 (the
/// unit of `EffectivenessMetrics`; 1.0 means every duty was fulfilled). The
/// Lighthouse Validator Client exports the successful signings but no
/// counter of the expected duties, so on Lighthouse both values stay None.
/// Counting the duties that were actually missed needs the Beacon Node
/// validator monitor.
pub struct EffectivenessCollector;

impl EffectivenessCollector {
    /// Collects effectiveness metrics from one Validator Client.
    ///
    /// Scrapes the Prometheus endpoint. To share one scrape between several
    /// collectors, call `collect_metrics` once and use `from_metrics`.
    pub async fn collect(client: &ValidatorClient) -> EffectivenessMetrics {
        match client.collect_metrics().await {
            Ok(samples) => Self::from_metrics(&samples),
            Err(_) => EffectivenessMetrics::default(),
        }
    }

    /// Computes the metrics from already scraped Prometheus samples.
    pub fn from_metrics(samples: &[PrometheusMetric]) -> EffectivenessMetrics {
        let mut metrics = EffectivenessMetrics::default();
        apply_effectiveness(&mut metrics, samples);
        metrics
    }
}

/// Fills the effectiveness ratios.
fn apply_effectiveness(metrics: &mut EffectivenessMetrics, samples: &[PrometheusMetric]) {
    let successful_attestations = signing_totals(samples, SIGNED_ATTESTATIONS).success;
    let successful_proposals = signing_totals(samples, SIGNED_BLOCKS).success;

    metrics.attestation_effectiveness =
        calculate_ratio(successful_attestations, expected_attestations(samples));

    metrics.proposal_effectiveness =
        calculate_ratio(successful_proposals, expected_proposals(samples));
}

/// Cumulative number of attestation duties.
///
/// The Lighthouse Validator Client does not export it
/// (`vc_beacon_attester_count` is a per-epoch gauge, not a counter).
fn expected_attestations(_samples: &[PrometheusMetric]) -> Option<u64> {
    None
}

/// Cumulative number of block proposal duties.
///
/// The Lighthouse Validator Client does not export it
/// (`vc_beacon_block_proposer_count` is a per-epoch gauge, not a counter).
fn expected_proposals(_samples: &[PrometheusMetric]) -> Option<u64> {
    None
}

/// Calculates effectiveness as a ratio between 0 and 1.
///
/// Returns None if either counter is unavailable, if the expected number
/// of duties is zero, or if more duties were fulfilled than expected
/// (inconsistent counters).
fn calculate_ratio(successful: Option<u64>, expected: Option<u64>) -> Option<f64> {
    let successful = successful?;
    let expected = expected?;

    if expected == 0 || successful > expected {
        return None;
    }

    Some(successful as f64 / expected as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::validator::test_support::{lighthouse_scrape, signed};

    #[test]
    fn calculates_attestation_effectiveness() {
        assert_eq!(calculate_ratio(Some(90), Some(100)), Some(0.9));
    }

    #[test]
    fn calculates_proposal_effectiveness() {
        assert_eq!(calculate_ratio(Some(8), Some(10)), Some(0.8));
    }

    #[test]
    fn perfect_and_zero_effectiveness() {
        assert_eq!(calculate_ratio(Some(50), Some(50)), Some(1.0));
        assert_eq!(calculate_ratio(Some(0), Some(50)), Some(0.0));
    }

    #[test]
    fn ratio_is_never_a_percentage() {
        let ratio = calculate_ratio(Some(99), Some(100)).unwrap();

        assert!((0.0..=1.0).contains(&ratio), "{ratio}");
    }

    #[test]
    fn non_round_ratios_are_accurate() {
        let value = calculate_ratio(Some(1), Some(3)).unwrap();

        assert!((value - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn returns_none_when_expected_is_zero() {
        assert_eq!(calculate_ratio(Some(0), Some(0)), None);
    }

    #[test]
    fn returns_none_when_counters_are_missing() {
        assert_eq!(calculate_ratio(None, Some(100)), None);
        assert_eq!(calculate_ratio(Some(100), None), None);
        assert_eq!(calculate_ratio(None, None), None);
    }

    #[test]
    fn rejects_success_greater_than_expected() {
        assert_eq!(calculate_ratio(Some(110), Some(100)), None);
    }

    #[test]
    fn handles_very_large_counters() {
        let value = calculate_ratio(Some(u64::MAX / 2), Some(u64::MAX)).unwrap();

        assert!((value - 0.5).abs() < 1e-9);
    }

    #[test]
    fn effectiveness_is_unavailable_on_a_lighthouse_validator_client() {
        // Documents a known limitation: the signed counters are there, but
        // there is no cumulative "expected duties" counter to divide by.
        let metrics = EffectivenessCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.attestation_effectiveness, None);
        assert_eq!(metrics.proposal_effectiveness, None);
    }

    #[test]
    fn empty_scrape_gives_no_effectiveness() {
        let metrics = EffectivenessCollector::from_metrics(&[]);

        assert_eq!(metrics, EffectivenessMetrics::default());
    }

    #[test]
    fn successful_counters_are_read_from_the_real_metric_names() {
        let samples = vec![
            signed("vc_signed_attestations_total", "success", 90.0),
            signed("vc_signed_beacon_blocks_total", "success", 8.0),
        ];

        let attestations = signing_totals(&samples, SIGNED_ATTESTATIONS).success;
        let proposals = signing_totals(&samples, SIGNED_BLOCKS).success;

        assert_eq!(calculate_ratio(attestations, Some(100)), Some(0.9));
        assert_eq!(calculate_ratio(proposals, Some(10)), Some(0.8));
    }
}
