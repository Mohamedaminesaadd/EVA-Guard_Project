use super::{signing_totals, SIGNED_ATTESTATIONS};
use crate::clients::validator_client::{PrometheusMetric, ValidatorClient};
use crate::models::metrics::AttestationMetrics;

/// Collects validator attestation metrics.
pub struct AttestationsCollector;

impl AttestationsCollector {
    /// Collects attestation metrics from one Validator Client.
    ///
    /// Scrapes the Prometheus endpoint. To share one scrape between several
    /// collectors, call `collect_metrics` once and use `from_metrics`.
    pub async fn collect(client: &ValidatorClient) -> AttestationMetrics {
        match client.collect_metrics().await {
            Ok(samples) => Self::from_metrics(&samples),
            Err(_) => AttestationMetrics::default(),
        }
    }

    /// Computes the metrics from already scraped Prometheus samples.
    pub fn from_metrics(samples: &[PrometheusMetric]) -> AttestationMetrics {
        let mut metrics = AttestationMetrics::default();
        apply_attestations(&mut metrics, samples);
        metrics
    }
}

/// Fills the attestation counters from `vc_signed_attestations_total`.
///
/// - `attestations_total`: attestations signed successfully
///   (`status="success"`).
/// - `attestation_failures_total`: attestations the Validator Client
///   refused to sign, i.e. every other status (slashable, same_data,
///   unregistered).
///
/// Both are None while the counter has not been exposed yet. Once it is,
/// a status without sample has never happened and counts as zero.
fn apply_attestations(metrics: &mut AttestationMetrics, samples: &[PrometheusMetric]) {
    let totals = signing_totals(samples, SIGNED_ATTESTATIONS);

    metrics.attestations_total = totals.success;
    metrics.attestation_failures_total = totals.refused;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::validator::test_support::{lighthouse_scrape, sample, signed};

    const NAME: &str = "vc_signed_attestations_total";

    #[test]
    fn collects_successful_attestations() {
        let metrics = AttestationsCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.attestations_total, Some(120));
    }

    #[test]
    fn collects_failures_from_a_realistic_scrape() {
        // One `same_data` refusal in the scrape.
        let metrics = AttestationsCollector::from_metrics(&lighthouse_scrape());

        assert_eq!(metrics.attestation_failures_total, Some(1));
    }

    #[test]
    fn refused_signings_are_failures_not_attestations() {
        let samples = vec![
            signed(NAME, "success", 10.0),
            signed(NAME, "same_data", 4.0),
            signed(NAME, "slashable", 2.0),
            signed(NAME, "unregistered", 1.0),
        ];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, Some(10));
        assert_eq!(metrics.attestation_failures_total, Some(7));
    }

    #[test]
    fn healthy_validator_has_zero_failures() {
        let samples = vec![signed(NAME, "success", 100.0)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, Some(100));
        assert_eq!(metrics.attestation_failures_total, Some(0));
    }

    #[test]
    fn only_refused_signings_means_zero_attestations() {
        let samples = vec![signed(NAME, "unregistered", 3.0)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, Some(0));
        assert_eq!(metrics.attestation_failures_total, Some(3));
    }

    #[test]
    fn returns_none_when_metric_is_missing() {
        let samples = vec![sample("some_other_metric", &[], 10.0)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics, AttestationMetrics::default());
    }

    #[test]
    fn empty_scrape_gives_none() {
        let metrics = AttestationsCollector::from_metrics(&[]);

        assert_eq!(metrics, AttestationMetrics::default());
    }

    #[test]
    fn rejects_negative_values() {
        let samples = vec![signed(NAME, "success", -5.0)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, None);
    }

    #[test]
    fn rejects_non_integer_values() {
        let samples = vec![signed(NAME, "success", 12.5)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, None);
    }

    #[test]
    fn an_invalid_success_value_does_not_hide_failures() {
        let samples = vec![
            signed(NAME, "success", f64::NAN),
            signed(NAME, "slashable", 2.0),
        ];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, None);
        assert_eq!(metrics.attestation_failures_total, Some(2));
    }

    #[test]
    fn an_invalid_failure_value_does_not_hide_attestations() {
        let samples = vec![
            signed(NAME, "success", 9.0),
            signed(NAME, "slashable", -1.0),
        ];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics.attestations_total, Some(9));
        assert_eq!(metrics.attestation_failures_total, None);
    }

    #[test]
    fn ignores_samples_without_a_status_label() {
        let samples = vec![sample(NAME, &[], 42.0)];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics, AttestationMetrics::default());
    }

    #[test]
    fn old_guessed_metric_names_are_not_used() {
        let samples = vec![
            sample("validator_attestations_total", &[], 42.0),
            sample("attestations_total", &[], 42.0),
            sample("validator_attestation_failures_total", &[], 3.0),
        ];

        let metrics = AttestationsCollector::from_metrics(&samples);

        assert_eq!(metrics, AttestationMetrics::default());
    }
}
