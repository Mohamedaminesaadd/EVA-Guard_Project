use crate::clients::geth_client::GethClient;
use crate::models::metrics::RpcMetrics;

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// Prometheus names of the total RPC request counter, by priority.
///
/// Geth registers `rpc/requests`, exported by its Prometheus endpoint
/// (`/debug/metrics/prometheus`) as `rpc_requests`: `/` becomes `_` and
/// no `_total` suffix is added. The `_total` form is kept as a fallback.
const REQUEST_COUNTER_NAMES: &[&str] = &["rpc_requests", "rpc_requests_total"];

/// Prometheus names of the failed RPC request counter, by priority.
///
/// Geth registers `rpc/failure`, exported as `rpc_failure`.
const FAILURE_COUNTER_NAMES: &[&str] = &["rpc_failure", "rpc_failure_total"];

/// 2^64 as f64 (exactly representable). Any value >= this overflows u64.
const U64_OVERFLOW_F64: f64 = 18_446_744_073_709_551_616.0;

/// Stores the previous RPC counter sample.
#[derive(Debug, Clone, Copy)]
struct RpcCounterSample {
    total: u64,
    collected_at: Instant,
}

/// Collects Geth JSON-RPC availability, latency and request rate.
///
/// Keeps the previous request counter to calculate a rate between two
/// cycles. One collector instance must be used per node.
pub struct RpcCollector {
    previous_sample: Mutex<Option<RpcCounterSample>>,
}

impl RpcCollector {
    pub fn new() -> Self {
        Self {
            previous_sample: Mutex::new(None),
        }
    }

    /// Collects RPC metrics from one Geth node.
    pub async fn collect(&self, client: &GethClient) -> RpcMetrics {
        let mut metrics = RpcMetrics::default();

        // 1. Test JSON-RPC availability and measure latency.
        match client.block_number().await {
            Ok(result) => {
                metrics.rpc_available = Some(true);
                metrics.rpc_latency_ms = Some(result.latency_ms);
            }
            Err(_) => {
                metrics.rpc_available = Some(false);
            }
        }

        // 2. Retrieve Prometheus metrics.
        let raw_metrics = match client.get_prometheus_metrics().await {
            Ok(data) => data,
            Err(_) => return metrics,
        };

        // Timestamp of the sample, taken as close as possible to the answer.
        let collected_at = Instant::now();

        // 3. Retrieve total RPC requests.
        let total = match find_counter(&raw_metrics, REQUEST_COUNTER_NAMES) {
            Some(value) => value,
            None => return metrics,
        };

        // 4. Retrieve failed RPC requests.
        let failed = find_counter(&raw_metrics, FAILURE_COUNTER_NAMES);

        metrics.rpc_requests_total = Some(total);
        metrics.rpc_failed_requests_total = failed;

        // 5. Calculate requests per second.
        self.update_rate(&mut metrics, total, collected_at);

        metrics
    }

    /// Calculates the request rate using two consecutive samples.
    ///
    /// The first call only stores the baseline and measures nothing.
    fn update_rate(&self, metrics: &mut RpcMetrics, total: u64, collected_at: Instant) {
        let mut previous = self.lock_previous();

        if let Some(old) = *previous {
            let elapsed = collected_at
                .saturating_duration_since(old.collected_at)
                .as_secs_f64();

            // A lower counter value indicates a reset or restart.
            if elapsed > 0.0 && total >= old.total {
                metrics.rpc_requests_per_sec = Some((total - old.total) as f64 / elapsed);
            }
        }

        *previous = Some(RpcCounterSample {
            total,
            collected_at,
        });
    }

    /// Locks the previous sample, recovering from a poisoned mutex.
    ///
    /// The protected value is replaced atomically and can never be left
    /// half-updated, so continuing after a panic elsewhere is safe.
    fn lock_previous(&self) -> MutexGuard<'_, Option<RpcCounterSample>> {
        self.previous_sample
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for RpcCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of looking for one counter name in Prometheus text.
#[derive(Debug, PartialEq, Eq)]
enum CounterLookup {
    Missing,
    Found(u64),
    Ambiguous,
}

/// Searches Prometheus exposition text for an unlabelled counter.
///
/// `candidate_names` are tried in priority order: the first name that
/// exists wins. Returns None when no name is found, or when the winning
/// name appears several times (ambiguous). Labelled samples, lines with
/// extra fields (e.g. timestamps), invalid values and values outside the
/// u64 integer range are ignored.
fn find_counter(input: &str, candidate_names: &[&str]) -> Option<u64> {
    for name in candidate_names {
        match lookup_counter(input, name) {
            CounterLookup::Found(value) => return Some(value),
            CounterLookup::Ambiguous => return None,
            CounterLookup::Missing => {}
        }
    }

    None
}

/// Looks for one exact metric name, refusing to pick between duplicates.
fn lookup_counter(input: &str, wanted: &str) -> CounterLookup {
    let mut found = None;

    for line in input.lines() {
        let Some(value) = sample_value(line, wanted) else {
            continue;
        };

        // Avoid silently selecting one of multiple matching samples.
        if found.is_some() {
            return CounterLookup::Ambiguous;
        }

        found = Some(value);
    }

    match found {
        Some(value) => CounterLookup::Found(value),
        None => CounterLookup::Missing,
    }
}

/// Returns the value of a line that is exactly `<wanted> <value>`.
///
/// Comments, blank lines, labelled samples (`name{...} v`) and lines with
/// extra fields do not match.
fn sample_value(line: &str, wanted: &str) -> Option<u64> {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let mut parts = line.split_whitespace();

    let name = parts.next()?;
    let value = parts.next()?;

    if parts.next().is_some() || name != wanted {
        return None;
    }

    parse_counter_value(value)
}

/// Parses a counter value as an exact non-negative integer.
///
/// Plain integers are parsed exactly (no f64 precision loss above 2^53).
/// Float notations such as `1.2e+06` are accepted only when they are whole.
fn parse_counter_value(raw: &str) -> Option<u64> {
    if let Ok(value) = raw.parse::<u64>() {
        return Some(value);
    }

    let parsed = raw.parse::<f64>().ok()?;

    if !parsed.is_finite() || parsed < 0.0 || parsed.fract() != 0.0 || parsed >= U64_OVERFLOW_F64 {
        return None;
    }

    Some(parsed as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::Duration;

    fn rate_of(metrics: &RpcMetrics) -> f64 {
        metrics.rpc_requests_per_sec.expect("a rate was expected")
    }

    // ---- find_counter -------------------------------------------------

    #[test]
    fn finds_prometheus_counter() {
        let input = r#"
# HELP rpc_requests_total Total requests
# TYPE rpc_requests_total counter
rpc_requests_total 1200
"#;

        assert_eq!(find_counter(input, &["rpc_requests_total"]), Some(1200));
    }

    #[test]
    fn finds_real_geth_metric_names() {
        // Shape of the output of Geth's /debug/metrics/prometheus endpoint.
        let input = r#"
# TYPE rpc_requests gauge
rpc_requests 4821
# TYPE rpc_success gauge
rpc_success 4800
# TYPE rpc_failure gauge
rpc_failure 21
# TYPE rpc_duration_all summary
rpc_duration_all{quantile="0.5"} 183000
rpc_duration_all_count 4821
"#;

        assert_eq!(find_counter(input, REQUEST_COUNTER_NAMES), Some(4821));
        assert_eq!(find_counter(input, FAILURE_COUNTER_NAMES), Some(21));
    }

    #[test]
    fn does_not_match_longer_metric_names() {
        let input = "rpc_requests_per_method 7\nrpc_duration_all_count 9";

        assert_eq!(find_counter(input, REQUEST_COUNTER_NAMES), None);
    }

    #[test]
    fn returns_none_when_counter_is_missing() {
        let input = "some_other_metric 42";

        assert_eq!(find_counter(input, &["rpc_requests_total"]), None);
    }

    #[test]
    fn returns_none_for_empty_input_and_no_candidates() {
        assert_eq!(find_counter("", REQUEST_COUNTER_NAMES), None);
        assert_eq!(find_counter("rpc_requests 5", &[]), None);
    }

    #[test]
    fn ignores_labelled_samples() {
        let input = r#"rpc_requests_total{method="eth_call"} 100"#;

        assert_eq!(find_counter(input, &["rpc_requests_total"]), None);
    }

    #[test]
    fn ignores_lines_with_timestamps() {
        let input = "rpc_requests 100 1700000000000";

        assert_eq!(find_counter(input, &["rpc_requests"]), None);
    }

    #[test]
    fn rejects_non_integer_counter() {
        let input = "rpc_requests_total 12.5";

        assert_eq!(find_counter(input, &["rpc_requests_total"]), None);
    }

    #[test]
    fn rejects_negative_counter() {
        let input = "rpc_requests_total -10";

        assert_eq!(find_counter(input, &["rpc_requests_total"]), None);
    }

    #[test]
    fn rejects_non_finite_counters() {
        for value in ["NaN", "+Inf", "-Inf", "inf"] {
            let input = format!("rpc_requests {value}");

            assert_eq!(find_counter(&input, &["rpc_requests"]), None, "{value}");
        }
    }

    #[test]
    fn accepts_whole_float_notation() {
        assert_eq!(
            find_counter("rpc_requests 1200.0", &["rpc_requests"]),
            Some(1200)
        );
        assert_eq!(
            find_counter("rpc_requests 1.2e+03", &["rpc_requests"]),
            Some(1200)
        );
    }

    #[test]
    fn parses_large_counters_exactly() {
        // 2^53 + 1 is not representable as f64.
        let input = "rpc_requests 9007199254740993";

        assert_eq!(
            find_counter(input, &["rpc_requests"]),
            Some(9_007_199_254_740_993)
        );
    }

    #[test]
    fn accepts_u64_max_and_rejects_overflow() {
        assert_eq!(
            find_counter("rpc_requests 18446744073709551615", &["rpc_requests"]),
            Some(u64::MAX)
        );
        assert_eq!(
            find_counter("rpc_requests 18446744073709551616", &["rpc_requests"]),
            None
        );
        assert_eq!(find_counter("rpc_requests 1e30", &["rpc_requests"]), None);
    }

    #[test]
    fn ignores_invalid_lines_and_continues_search() {
        let input = r#"
invalid_metric
rpc_requests_total 1200
"#;

        assert_eq!(find_counter(input, &["rpc_requests_total"]), Some(1200));
    }

    #[test]
    fn skips_an_invalid_sample_and_uses_a_later_valid_one() {
        let input = "rpc_requests abc\nrpc_requests 15";

        assert_eq!(find_counter(input, &["rpc_requests"]), Some(15));
    }

    #[test]
    fn rejects_duplicate_samples_of_the_same_name() {
        let input = "rpc_requests 10\nrpc_requests 20";

        assert_eq!(find_counter(input, &["rpc_requests"]), None);
    }

    #[test]
    fn prefers_the_first_candidate_name() {
        let input = r#"
rpc_requests_total 1500
rpc_requests 1200
"#;

        assert_eq!(
            find_counter(input, &["rpc_requests", "rpc_requests_total"]),
            Some(1200)
        );
    }

    #[test]
    fn falls_back_to_the_next_candidate_name() {
        let input = "rpc_requests_total 1500";

        assert_eq!(
            find_counter(input, &["rpc_requests", "rpc_requests_total"]),
            Some(1500)
        );
    }

    #[test]
    fn duplicates_of_the_winning_name_are_not_hidden_by_fallbacks() {
        let input = "rpc_requests 1\nrpc_requests 2\nrpc_requests_total 3";

        assert_eq!(
            find_counter(input, &["rpc_requests", "rpc_requests_total"]),
            None
        );
    }

    // ---- update_rate ----------------------------------------------------

    #[test]
    fn first_sample_has_no_rate() {
        let collector = RpcCollector::new();
        let mut metrics = RpcMetrics::default();

        collector.update_rate(&mut metrics, 100, Instant::now());

        assert!(metrics.rpc_requests_per_sec.is_none());
    }

    #[test]
    fn computes_requests_per_second() {
        let collector = RpcCollector::new();
        let t0 = Instant::now();

        collector.update_rate(&mut RpcMetrics::default(), 100, t0);

        let mut metrics = RpcMetrics::default();
        collector.update_rate(&mut metrics, 160, t0 + Duration::from_secs(10));

        assert!((rate_of(&metrics) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn idle_node_has_a_zero_rate() {
        let collector = RpcCollector::new();
        let t0 = Instant::now();

        collector.update_rate(&mut RpcMetrics::default(), 100, t0);

        let mut metrics = RpcMetrics::default();
        collector.update_rate(&mut metrics, 100, t0 + Duration::from_secs(5));

        assert!(rate_of(&metrics).abs() < 1e-9);
    }

    #[test]
    fn counter_reset_has_no_rate_and_replaces_the_baseline() {
        let collector = RpcCollector::new();
        let t0 = Instant::now();

        collector.update_rate(&mut RpcMetrics::default(), 1000, t0);

        // Node restarted: the counter went down.
        let mut after_reset = RpcMetrics::default();
        collector.update_rate(&mut after_reset, 5, t0 + Duration::from_secs(10));
        assert!(after_reset.rpc_requests_per_sec.is_none());

        // The next sample is measured against the post-reset baseline.
        let mut next = RpcMetrics::default();
        collector.update_rate(&mut next, 105, t0 + Duration::from_secs(20));
        assert!((rate_of(&next) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn zero_elapsed_time_has_no_rate() {
        let collector = RpcCollector::new();
        let t0 = Instant::now();

        collector.update_rate(&mut RpcMetrics::default(), 100, t0);

        let mut metrics = RpcMetrics::default();
        collector.update_rate(&mut metrics, 200, t0);

        assert!(metrics.rpc_requests_per_sec.is_none());
    }

    #[test]
    fn older_timestamp_does_not_panic_or_produce_a_rate() {
        let collector = RpcCollector::new();
        let t0 = Instant::now() + Duration::from_secs(10);

        collector.update_rate(&mut RpcMetrics::default(), 100, t0);

        let mut metrics = RpcMetrics::default();
        collector.update_rate(&mut metrics, 200, t0 - Duration::from_secs(5));

        assert!(metrics.rpc_requests_per_sec.is_none());
    }

    #[test]
    fn recovers_from_a_poisoned_mutex() {
        let collector = Arc::new(RpcCollector::new());
        let t0 = Instant::now();

        collector.update_rate(&mut RpcMetrics::default(), 100, t0);

        let poisoner = Arc::clone(&collector);
        let result = std::thread::spawn(move || {
            let _guard = poisoner.previous_sample.lock().unwrap();
            panic!("poison the mutex on purpose");
        })
        .join();

        assert!(result.is_err());
        assert!(collector.previous_sample.is_poisoned());

        let mut metrics = RpcMetrics::default();
        collector.update_rate(&mut metrics, 150, t0 + Duration::from_secs(10));

        assert!((rate_of(&metrics) - 5.0).abs() < 1e-9);
    }
}
