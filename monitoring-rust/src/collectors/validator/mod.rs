pub mod attestations;
pub mod duties;
pub mod effectiveness;
pub mod missed_attestations;
pub mod proposals;

pub use attestations::AttestationsCollector;
pub use duties::DutiesCollector;
pub use effectiveness::EffectivenessCollector;
pub use missed_attestations::MissedAttestationsCollector;
pub use proposals::ProposalsCollector;

use crate::clients::validator_client::PrometheusMetric;

// ---------------------------------------------------------------------------
// Lighthouse Validator Client metric names.
//
// Taken from sigp/lighthouse (`validator_client/validator_metrics`).
// Counter vectors (`*_total`) only get a sample for a `status` value after
// its first increment, so a status that never happened has no sample at all.
// ---------------------------------------------------------------------------

/// Counter vector, label `status`: success | slashable | same_data | unregistered.
pub(crate) const SIGNED_ATTESTATIONS: &str = "vc_signed_attestations_total";

/// Counter vector, label `status`: success | slashable | same_data | unregistered.
pub(crate) const SIGNED_BLOCKS: &str = "vc_signed_beacon_blocks_total";

/// Gauge: enabled and disabled validators.
pub(crate) const VALIDATORS_TOTAL: &str = "vc_validators_total_count";

/// Gauge: enabled validators.
pub(crate) const VALIDATORS_ENABLED: &str = "vc_validators_enabled_count";

/// Gauge, label `task`: attesters on this host for `current_epoch` / `next_epoch`.
pub(crate) const ATTESTER_COUNT: &str = "vc_beacon_attester_count";

/// Gauge, label `task`: block proposers on this host for `current_epoch`.
pub(crate) const PROPOSER_COUNT: &str = "vc_beacon_block_proposer_count";

/// Gauge: Beacon Nodes the Validator Client currently considers available.
pub(crate) const BEACON_NODES_AVAILABLE: &str = "vc_beacon_nodes_available_count";

const STATUS_LABEL: &str = "status";
const STATUS_SUCCESS: &str = "success";
const TASK_LABEL: &str = "task";
pub(crate) const TASK_CURRENT_EPOCH: &str = "current_epoch";

/// 2^64 as f64 (exactly representable). Any value >= this overflows u64.
const U64_OVERFLOW_F64: f64 = 18_446_744_073_709_551_616.0;

/// Outcome of summing the samples of one metric.
#[derive(Debug, PartialEq, Eq)]
enum Sum {
    /// No sample matched.
    Absent,
    /// A matching sample has an invalid value, or the total overflows.
    Invalid,
    Total(u64),
}

/// Reads the value of a label, if the sample carries it.
fn label<'a>(sample: &'a PrometheusMetric, key: &str) -> Option<&'a str> {
    sample.labels.get(key).map(String::as_str)
}

/// Converts a sample value into an exact non-negative integer.
fn sample_value_u64(sample: &PrometheusMetric) -> Option<u64> {
    let value = sample.value;

    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value >= U64_OVERFLOW_F64 {
        return None;
    }

    Some(value as u64)
}

/// Sums the samples named `name` accepted by `keep`.
fn sum_samples(
    metrics: &[PrometheusMetric],
    name: &str,
    keep: impl Fn(&PrometheusMetric) -> bool,
) -> Sum {
    let mut total: Option<u64> = None;

    for sample in metrics
        .iter()
        .filter(|sample| sample.name == name && keep(sample))
    {
        let Some(value) = sample_value_u64(sample) else {
            return Sum::Invalid;
        };

        let Some(next) = total.unwrap_or(0).checked_add(value) else {
            return Sum::Invalid;
        };

        total = Some(next);
    }

    match total {
        Some(total) => Sum::Total(total),
        None => Sum::Absent,
    }
}

/// Sum of every sample named `name`.
///
/// Meant for unlabelled gauges. Returns None when the metric is missing
/// or invalid.
pub(crate) fn sum_family(metrics: &[PrometheusMetric], name: &str) -> Option<u64> {
    match sum_samples(metrics, name, |_| true) {
        Sum::Total(total) => Some(total),
        Sum::Absent | Sum::Invalid => None,
    }
}

/// Sum of the samples named `name` that carry `key="value"`.
///
/// Returns None when no such sample exists or when one is invalid.
pub(crate) fn sum_labelled(
    metrics: &[PrometheusMetric],
    name: &str,
    key: &str,
    value: &str,
) -> Option<u64> {
    match sum_samples(metrics, name, |sample| label(sample, key) == Some(value)) {
        Sum::Total(total) => Some(total),
        Sum::Absent | Sum::Invalid => None,
    }
}

/// Totals of a Lighthouse signing counter, split by outcome.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SigningTotals {
    /// Signings accepted (`status="success"`).
    pub(crate) success: Option<u64>,
    /// Signings refused (any other status: slashable, same_data, unregistered).
    pub(crate) refused: Option<u64>,
}

/// Splits a Lighthouse signing counter vector by `status`.
///
/// Once the counter family is live (at least one sample with a `status`
/// label), a status without sample has never been incremented and counts
/// as zero. If the family is absent, nothing is known and both totals are
/// None. Samples without a `status` label are ignored. A total is None
/// when one of its samples is invalid.
pub(crate) fn signing_totals(metrics: &[PrometheusMetric], name: &str) -> SigningTotals {
    let family_is_live = metrics
        .iter()
        .any(|sample| sample.name == name && label(sample, STATUS_LABEL).is_some());

    if !family_is_live {
        return SigningTotals {
            success: None,
            refused: None,
        };
    }

    let success = sum_samples(metrics, name, |sample| {
        label(sample, STATUS_LABEL) == Some(STATUS_SUCCESS)
    });

    let refused = sum_samples(
        metrics,
        name,
        |sample| matches!(label(sample, STATUS_LABEL), Some(status) if status != STATUS_SUCCESS),
    );

    SigningTotals {
        success: counter_or_zero(success),
        refused: counter_or_zero(refused),
    }
}

/// Maps a counter-vector sum: no sample in a live family means zero.
fn counter_or_zero(sum: Sum) -> Option<u64> {
    match sum {
        Sum::Total(total) => Some(total),
        Sum::Absent => Some(0),
        Sum::Invalid => None,
    }
}

/// Helpers shared by the tests of the validator collectors.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::clients::validator_client::PrometheusMetric;

    /// Builds one Prometheus sample.
    pub(crate) fn sample(name: &str, labels: &[(&str, &str)], value: f64) -> PrometheusMetric {
        PrometheusMetric {
            name: name.to_string(),
            labels: labels
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            value,
        }
    }

    /// Builds one sample of a signing counter (`status` label).
    pub(crate) fn signed(name: &str, status: &str, value: f64) -> PrometheusMetric {
        sample(name, &[("status", status)], value)
    }

    /// Samples shaped like a real Lighthouse Validator Client scrape.
    pub(crate) fn lighthouse_scrape() -> Vec<PrometheusMetric> {
        vec![
            sample("vc_validators_total_count", &[], 4.0),
            sample("vc_validators_enabled_count", &[], 3.0),
            sample("vc_beacon_nodes_total_count", &[], 2.0),
            sample("vc_beacon_nodes_available_count", &[], 1.0),
            sample("vc_beacon_nodes_synced_count", &[], 1.0),
            signed("vc_signed_attestations_total", "success", 120.0),
            signed("vc_signed_attestations_total", "same_data", 1.0),
            signed("vc_signed_beacon_blocks_total", "success", 3.0),
            sample(
                "vc_beacon_attester_count",
                &[("task", "current_epoch")],
                4.0,
            ),
            sample("vc_beacon_attester_count", &[("task", "next_epoch")], 7.0),
            sample(
                "vc_beacon_block_proposer_count",
                &[("task", "current_epoch")],
                1.0,
            ),
            // Unrelated families must be ignored.
            sample(
                "vc_signing_times_seconds_count",
                &[("task", "attestations")],
                99.0,
            ),
            sample(
                "bn_endpoint_errors",
                &[("endpoint", "/eth/v1/node/syncing")],
                5.0,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{lighthouse_scrape, sample, signed};
    use super::*;

    const ATT: &str = SIGNED_ATTESTATIONS;

    // ---- sample_value_u64 / sum_family ------------------------------------

    #[test]
    fn sums_all_samples_of_a_family() {
        let metrics = vec![
            sample("m", &[("a", "1")], 3.0),
            sample("m", &[("a", "2")], 4.0),
            sample("other", &[], 100.0),
        ];

        assert_eq!(sum_family(&metrics, "m"), Some(7));
    }

    #[test]
    fn missing_family_gives_none() {
        let metrics = vec![sample("some_other_metric", &[], 5.0)];

        assert_eq!(sum_family(&metrics, "m"), None);
        assert_eq!(sum_family(&[], "m"), None);
    }

    #[test]
    fn accepts_zero_and_whole_float_values() {
        assert_eq!(sum_family(&[sample("m", &[], 0.0)], "m"), Some(0));
        assert_eq!(sum_family(&[sample("m", &[], 12.0)], "m"), Some(12));
    }

    #[test]
    fn rejects_invalid_values() {
        for value in [-1.0, 12.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(sum_family(&[sample("m", &[], value)], "m"), None, "{value}");
        }
    }

    #[test]
    fn rejects_values_that_overflow_u64() {
        assert_eq!(sum_family(&[sample("m", &[], 1e30)], "m"), None);
        assert_eq!(
            sum_family(&[sample("m", &[], 18_446_744_073_709_551_616.0)], "m"),
            None
        );
    }

    #[test]
    fn rejects_a_total_that_overflows_u64() {
        let metrics = vec![
            sample("m", &[("a", "1")], 18_446_744_073_709_549_568.0),
            sample("m", &[("a", "2")], 18_446_744_073_709_549_568.0),
        ];

        assert_eq!(sum_family(&metrics, "m"), None);
    }

    #[test]
    fn one_invalid_sample_invalidates_the_whole_sum() {
        let metrics = vec![
            sample("m", &[("a", "1")], 3.0),
            sample("m", &[("a", "2")], -1.0),
        ];

        assert_eq!(sum_family(&metrics, "m"), None);
    }

    // ---- sum_labelled ------------------------------------------------------

    #[test]
    fn sums_only_samples_with_the_wanted_label() {
        let metrics = vec![
            sample("m", &[("task", "current_epoch")], 4.0),
            sample("m", &[("task", "next_epoch")], 7.0),
            sample("m", &[], 100.0),
        ];

        assert_eq!(
            sum_labelled(&metrics, "m", "task", "current_epoch"),
            Some(4)
        );
        assert_eq!(sum_labelled(&metrics, "m", "task", "next_epoch"), Some(7));
    }

    #[test]
    fn missing_label_value_gives_none() {
        let metrics = vec![sample("m", &[("task", "current_epoch")], 4.0)];

        assert_eq!(sum_labelled(&metrics, "m", "task", "next_epoch"), None);
        assert_eq!(sum_labelled(&metrics, "m", "other", "current_epoch"), None);
    }

    #[test]
    fn invalid_sample_with_the_wanted_label_gives_none() {
        let metrics = vec![sample("m", &[("task", "current_epoch")], -2.0)];

        assert_eq!(sum_labelled(&metrics, "m", "task", "current_epoch"), None);
    }

    #[test]
    fn invalid_sample_with_another_label_is_ignored() {
        let metrics = vec![
            sample("m", &[("task", "current_epoch")], 4.0),
            sample("m", &[("task", "next_epoch")], -2.0),
        ];

        assert_eq!(
            sum_labelled(&metrics, "m", "task", "current_epoch"),
            Some(4)
        );
    }

    // ---- signing_totals ----------------------------------------------------

    #[test]
    fn splits_success_and_refused_signings() {
        let metrics = vec![
            signed(ATT, "success", 120.0),
            signed(ATT, "same_data", 1.0),
            signed(ATT, "slashable", 2.0),
            signed(ATT, "unregistered", 3.0),
        ];

        let totals = signing_totals(&metrics, ATT);

        assert_eq!(totals.success, Some(120));
        assert_eq!(totals.refused, Some(6));
    }

    #[test]
    fn healthy_family_has_zero_refused_signings() {
        let metrics = vec![signed(ATT, "success", 50.0)];

        let totals = signing_totals(&metrics, ATT);

        assert_eq!(totals.success, Some(50));
        assert_eq!(totals.refused, Some(0));
    }

    #[test]
    fn only_refused_signings_means_zero_successes() {
        let metrics = vec![signed(ATT, "slashable", 2.0)];

        let totals = signing_totals(&metrics, ATT);

        assert_eq!(totals.success, Some(0));
        assert_eq!(totals.refused, Some(2));
    }

    #[test]
    fn absent_family_is_unknown() {
        let metrics = vec![sample("some_other_metric", &[], 5.0)];

        assert_eq!(
            signing_totals(&metrics, ATT),
            SigningTotals {
                success: None,
                refused: None
            }
        );
    }

    #[test]
    fn samples_without_a_status_label_are_ignored() {
        let metrics = vec![sample(ATT, &[], 99.0)];

        assert_eq!(
            signing_totals(&metrics, ATT),
            SigningTotals {
                success: None,
                refused: None
            }
        );
    }

    #[test]
    fn invalid_success_does_not_discard_refused() {
        let metrics = vec![signed(ATT, "success", -1.0), signed(ATT, "slashable", 2.0)];

        let totals = signing_totals(&metrics, ATT);

        assert_eq!(totals.success, None);
        assert_eq!(totals.refused, Some(2));
    }

    #[test]
    fn invalid_refused_does_not_discard_success() {
        let metrics = vec![signed(ATT, "success", 9.0), signed(ATT, "slashable", 1.5)];

        let totals = signing_totals(&metrics, ATT);

        assert_eq!(totals.success, Some(9));
        assert_eq!(totals.refused, None);
    }

    #[test]
    fn other_families_are_not_mixed_in() {
        let metrics = vec![
            signed(SIGNED_BLOCKS, "success", 3.0),
            signed(ATT, "success", 120.0),
        ];

        assert_eq!(signing_totals(&metrics, ATT).success, Some(120));
        assert_eq!(signing_totals(&metrics, SIGNED_BLOCKS).success, Some(3));
    }

    // ---- the five collectors against the observation contract ----------------

    /// One scrape fed to the five collectors, as the monitoring loop does.
    fn validator_metrics(scrape: &[PrometheusMetric]) -> crate::models::metrics::ValidatorMetrics {
        crate::models::metrics::ValidatorMetrics {
            attestations: AttestationsCollector::from_metrics(scrape),
            missed_attestations: MissedAttestationsCollector::from_metrics(scrape),
            block_proposals: ProposalsCollector::from_metrics(scrape),
            missed_proposals: Default::default(),
            duties: DutiesCollector::from_metrics(scrape),
            effectiveness: EffectivenessCollector::from_metrics(scrape),
        }
    }

    fn observation_of(
        metrics: crate::models::metrics::ValidatorMetrics,
    ) -> serde_json::Map<String, serde_json::Value> {
        use crate::models::observation::{Observation, Scenario};

        let observation = Observation::new_at(
            "2026-11-16T08:00:05Z".parse().unwrap(),
            "node-01",
            Scenario::Normal,
            std::time::Duration::from_secs(5),
            metrics,
        );

        let value = serde_json::to_value(observation).unwrap();
        value["metrics"].as_object().unwrap().clone()
    }

    #[test]
    fn a_lighthouse_scrape_fills_the_contract_fields_it_can() {
        let metrics = observation_of(validator_metrics(&lighthouse_scrape()));

        // Exactly the 14 keys of the contract, always present.
        assert_eq!(metrics.len(), 14, "{metrics:?}");

        for (key, expected) in [
            ("attestations_total", serde_json::json!(120)),
            ("attestation_failures_total", serde_json::json!(1)),
            ("blocks_proposed_total", serde_json::json!(3)),
            ("block_proposal_failures_total", serde_json::json!(0)),
            ("validators_total", serde_json::json!(4)),
            ("validators_enabled", serde_json::json!(3)),
            ("attester_duties", serde_json::json!(4)),
            ("proposer_duties", serde_json::json!(1)),
            ("beacon_node_online", serde_json::json!(true)),
        ] {
            assert_eq!(metrics[key], expected, "{key}");
        }
    }

    #[test]
    fn what_a_validator_client_cannot_see_is_null_not_zero() {
        let metrics = observation_of(validator_metrics(&lighthouse_scrape()));

        for key in [
            "missed_attestations_total",
            "missed_attestations_last_epoch",
            "missed_proposals_total",
            "attestation_effectiveness",
            "proposal_effectiveness",
        ] {
            assert!(metrics[key].is_null(), "{key} = {}", metrics[key]);
        }
    }

    #[test]
    fn an_unreachable_validator_client_gives_only_nulls() {
        let metrics = observation_of(validator_metrics(&[]));

        assert_eq!(metrics.len(), 14);
        assert!(
            metrics.values().all(serde_json::Value::is_null),
            "{metrics:?}"
        );
    }
}
