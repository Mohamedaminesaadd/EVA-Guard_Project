use bollard::container::{NetworkStats, Stats};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkMetrics {
    pub rx_bytes_total: Option<u64>,
    pub tx_bytes_total: Option<u64>,
    pub rx_bytes_per_sec: Option<f64>,
    pub tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone)]
struct NetworkSample {
    rx_bytes: u64,
    tx_bytes: u64,
    collected_at: Instant,
}

/// Collects network traffic of Docker containers.
///
/// Totals come straight from Docker. Rates need two samples of the same
/// container, so the first sample of a container only has totals.
pub struct NetworkCollector {
    previous: HashMap<String, NetworkSample>,
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
        }
    }

    /// Collects traffic totals and, from the second call on, rates.
    ///
    /// All interfaces of the container are added together. Everything is
    /// None when Docker reports no interface (host network mode, stopped
    /// container) or when the totals overflow. A counter that went
    /// backwards (container restart) only loses the rate of that sample.
    pub fn collect(&mut self, container_id: &str, stats: &Stats) -> NetworkMetrics {
        self.collect_at(container_id, stats, Instant::now())
    }

    /// Drops the previous sample of a container that is no longer monitored.
    pub fn forget(&mut self, container_id: &str) {
        self.previous.remove(container_id);
    }

    fn collect_at(&mut self, container_id: &str, stats: &Stats, now: Instant) -> NetworkMetrics {
        let Some((rx_bytes, tx_bytes)) = stats.networks.as_ref().and_then(network_totals) else {
            // A gap invalidates the baseline: a rate computed across it
            // would average over a period without data.
            self.previous.remove(container_id);

            return NetworkMetrics::default();
        };

        let sample = NetworkSample {
            rx_bytes,
            tx_bytes,
            collected_at: now,
        };

        let previous = match self.previous.get_mut(container_id) {
            Some(slot) => Some(std::mem::replace(slot, sample)),
            None => {
                self.previous.insert(container_id.to_string(), sample);
                None
            }
        };

        let (rx_rate, tx_rate) = match previous {
            Some(previous) => {
                let elapsed = now
                    .saturating_duration_since(previous.collected_at)
                    .as_secs_f64();

                (
                    bytes_per_sec(rx_bytes, previous.rx_bytes, elapsed),
                    bytes_per_sec(tx_bytes, previous.tx_bytes, elapsed),
                )
            }
            None => (None, None),
        };

        NetworkMetrics {
            rx_bytes_total: Some(rx_bytes),
            tx_bytes_total: Some(tx_bytes),
            rx_bytes_per_sec: rx_rate,
            tx_bytes_per_sec: tx_rate,
        }
    }
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Received and transmitted bytes of all interfaces.
///
/// None without interface, or when the sums overflow.
fn network_totals(networks: &HashMap<String, NetworkStats>) -> Option<(u64, u64)> {
    if networks.is_empty() {
        return None;
    }

    let mut rx_bytes = 0u64;
    let mut tx_bytes = 0u64;

    for network in networks.values() {
        rx_bytes = rx_bytes.checked_add(network.rx_bytes)?;
        tx_bytes = tx_bytes.checked_add(network.tx_bytes)?;
    }

    Some((rx_bytes, tx_bytes))
}

/// Bytes per second between two counter readings.
///
/// None when no time elapsed or when the counter went backwards.
fn bytes_per_sec(current: u64, previous: u64, elapsed_secs: f64) -> Option<f64> {
    if elapsed_secs <= 0.0 {
        return None;
    }

    current
        .checked_sub(previous)
        .map(|delta| delta as f64 / elapsed_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::StatsBuilder;
    use std::time::Duration;

    fn stats(networks: &[(&'static str, u64, u64)]) -> Stats {
        StatsBuilder {
            networks: Some(networks.to_vec()),
            ..StatsBuilder::default()
        }
        .build()
    }

    fn assert_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a value");

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // ---- pure helpers -----------------------------------------------------------

    #[test]
    fn rate_is_delta_over_elapsed_time() {
        assert_close(bytes_per_sec(3_000, 1_000, 2.0), 1_000.0);
        assert_close(bytes_per_sec(1_000, 1_000, 2.0), 0.0);
    }

    #[test]
    fn rate_needs_elapsed_time() {
        assert_eq!(bytes_per_sec(3_000, 1_000, 0.0), None);
        assert_eq!(bytes_per_sec(3_000, 1_000, -1.0), None);
    }

    #[test]
    fn rate_rejects_a_counter_going_backwards() {
        assert_eq!(bytes_per_sec(100, 1_000, 2.0), None);
    }

    // ---- collect ----------------------------------------------------------------

    #[test]
    fn first_sample_has_totals_but_no_rates() {
        let mut collector = NetworkCollector::new();

        let metrics = collector.collect_at("c1", &stats(&[("eth0", 1_000, 2_000)]), Instant::now());

        assert_eq!(metrics.rx_bytes_total, Some(1_000));
        assert_eq!(metrics.tx_bytes_total, Some(2_000));
        assert_eq!(metrics.rx_bytes_per_sec, None);
        assert_eq!(metrics.tx_bytes_per_sec, None);
    }

    #[test]
    fn second_sample_has_rates() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 1_000, 2_000)]), start);
        let metrics = collector.collect_at(
            "c1",
            &stats(&[("eth0", 5_000, 2_500)]),
            start + Duration::from_secs(2),
        );

        assert_eq!(metrics.rx_bytes_total, Some(5_000));
        assert_eq!(metrics.tx_bytes_total, Some(2_500));
        assert_close(metrics.rx_bytes_per_sec, 2_000.0);
        assert_close(metrics.tx_bytes_per_sec, 250.0);
    }

    #[test]
    fn rates_follow_the_latest_sample() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 0, 0)]), start);
        collector.collect_at(
            "c1",
            &stats(&[("eth0", 1_000, 0)]),
            start + Duration::from_secs(1),
        );
        let metrics = collector.collect_at(
            "c1",
            &stats(&[("eth0", 1_500, 0)]),
            start + Duration::from_secs(2),
        );

        // 500 bytes in the last second, not 1500 over two.
        assert_close(metrics.rx_bytes_per_sec, 500.0);
    }

    #[test]
    fn adds_all_interfaces_together() {
        let mut collector = NetworkCollector::new();

        let metrics = collector.collect_at(
            "c1",
            &stats(&[("eth0", 1_000, 10), ("eth1", 500, 20)]),
            Instant::now(),
        );

        assert_eq!(metrics.rx_bytes_total, Some(1_500));
        assert_eq!(metrics.tx_bytes_total, Some(30));
    }

    #[test]
    fn counter_reset_keeps_totals_and_drops_rates_for_one_sample() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 9_000, 9_000)]), start);

        // Container restarted: counters are back near zero.
        let reset = collector.collect_at(
            "c1",
            &stats(&[("eth0", 100, 100)]),
            start + Duration::from_secs(1),
        );

        assert_eq!(reset.rx_bytes_total, Some(100));
        assert_eq!(reset.rx_bytes_per_sec, None);
        assert_eq!(reset.tx_bytes_per_sec, None);

        // The new counters became the baseline.
        let next = collector.collect_at(
            "c1",
            &stats(&[("eth0", 300, 100)]),
            start + Duration::from_secs(2),
        );

        assert_close(next.rx_bytes_per_sec, 200.0);
        assert_close(next.tx_bytes_per_sec, 0.0);
    }

    #[test]
    fn same_instant_gives_no_rate() {
        let mut collector = NetworkCollector::new();
        let now = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 1_000, 0)]), now);
        let metrics = collector.collect_at("c1", &stats(&[("eth0", 2_000, 0)]), now);

        assert_eq!(metrics.rx_bytes_total, Some(2_000));
        assert_eq!(metrics.rx_bytes_per_sec, None);
    }

    #[test]
    fn containers_do_not_share_baselines() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 1_000, 0)]), start);
        let other = collector.collect_at(
            "c2",
            &stats(&[("eth0", 9_000, 0)]),
            start + Duration::from_secs(1),
        );

        // First sample of c2: no rate, even though c1 was sampled before.
        assert_eq!(other.rx_bytes_per_sec, None);
    }

    #[test]
    fn overflowing_totals_give_default_metrics() {
        let mut collector = NetworkCollector::new();

        let metrics = collector.collect_at(
            "c1",
            &stats(&[("eth0", u64::MAX, 0), ("eth1", 1, 0)]),
            Instant::now(),
        );

        assert_eq!(metrics, NetworkMetrics::default());
    }

    #[test]
    fn missing_networks_give_default_metrics() {
        let mut collector = NetworkCollector::new();
        let sample = StatsBuilder {
            networks: None,
            ..StatsBuilder::default()
        }
        .build();

        assert_eq!(
            collector.collect_at("c1", &sample, Instant::now()),
            NetworkMetrics::default()
        );
    }

    #[test]
    fn empty_networks_are_not_reported_as_zero_traffic() {
        // Host network mode: Docker has no interface to report.
        let mut collector = NetworkCollector::new();

        assert_eq!(
            collector.collect_at("c1", &stats(&[]), Instant::now()),
            NetworkMetrics::default()
        );
    }

    #[test]
    fn a_gap_without_data_resets_the_baseline() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 1_000, 0)]), start);
        collector.collect_at("c1", &stats(&[]), start + Duration::from_secs(10));

        let after_gap = collector.collect_at(
            "c1",
            &stats(&[("eth0", 5_000, 0)]),
            start + Duration::from_secs(20),
        );

        assert_eq!(after_gap.rx_bytes_total, Some(5_000));
        assert_eq!(after_gap.rx_bytes_per_sec, None);
    }

    #[test]
    fn forget_drops_the_baseline() {
        let mut collector = NetworkCollector::new();
        let start = Instant::now();

        collector.collect_at("c1", &stats(&[("eth0", 1_000, 0)]), start);
        collector.forget("c1");

        let metrics = collector.collect_at(
            "c1",
            &stats(&[("eth0", 2_000, 0)]),
            start + Duration::from_secs(1),
        );

        assert_eq!(metrics.rx_bytes_per_sec, None);
    }

    #[test]
    fn forgetting_an_unknown_container_is_harmless() {
        let mut collector = NetworkCollector::default();

        collector.forget("never-seen");
    }

    #[test]
    fn public_collect_uses_the_real_clock() {
        let mut collector = NetworkCollector::new();

        let first = collector.collect("c1", &stats(&[("eth0", 1_000, 0)]));
        std::thread::sleep(Duration::from_millis(10));
        let second = collector.collect("c1", &stats(&[("eth0", 2_000, 0)]));

        assert_eq!(first.rx_bytes_per_sec, None);

        let rate = second.rx_bytes_per_sec.expect("a rate");
        assert!(rate.is_finite() && rate > 0.0);
    }
}
