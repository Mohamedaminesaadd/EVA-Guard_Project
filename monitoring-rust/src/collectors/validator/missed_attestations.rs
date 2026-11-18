use crate::clients::validator_client::{PrometheusMetric, ValidatorClient};
use crate::models::metrics::MissedAttestationMetrics;

/// Collects missed attestation metrics.
///
/// The Lighthouse Validator Client cannot tell how many attestations were
/// due but not produced: a missed duty is only visible on chain, not in
/// its own metrics. Both measurements stay None (never 0, which would claim
/// "nothing was missed"):
/// - `missed_attestations_total`
/// - `missed_attestations_last_epoch`
///
/// The data exists on the Beacon Node when it runs with the validator
/// monitor (`validator_monitor_prev_epoch_on_chain_attester_miss`).
///
/// Refused signings, which the Validator Client does see, are reported as
/// `attestation_failures_total` by `AttestationsCollector`.
pub struct MissedAttestationsCollector;

impl MissedAttestationsCollector {
    /// Returns the (unavailable) metrics. No request is made.
    pub async fn collect(_client: &ValidatorClient) -> MissedAttestationMetrics {
        MissedAttestationMetrics::default()
    }

    /// Computes the metrics from already scraped Prometheus samples.
    pub fn from_metrics(_samples: &[PrometheusMetric]) -> MissedAttestationMetrics {
        MissedAttestationMetrics::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::validator::test_support::{lighthouse_scrape, sample, signed};

    #[test]
    fn nothing_is_measured_from_a_realistic_scrape() {
        let metrics = MissedAttestationsCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.missed_attestations_total, None);
        assert_eq!(metrics.missed_attestations_last_epoch, None);
    }

    #[test]
    fn refused_signings_are_not_reported_as_missed() {
        let samples = vec![
            signed("vc_signed_attestations_total", "success", 100.0),
            signed("vc_signed_attestations_total", "slashable", 5.0),
        ];

        let metrics = MissedAttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics, MissedAttestationMetrics::default());
    }

    #[test]
    fn empty_scrape_gives_none() {
        assert_eq!(
            MissedAttestationsCollector::from_metrics(&[]),
            MissedAttestationMetrics::default()
        );
    }

    #[test]
    fn old_guessed_metric_names_are_not_used() {
        let samples = vec![
            sample("validator_missed_attestations_total", &[], 15.0),
            sample("validator_missed_attestations_last_epoch", &[], 1.0),
        ];

        let metrics = MissedAttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics, MissedAttestationMetrics::default());
    }
}
