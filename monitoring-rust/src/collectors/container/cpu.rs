use bollard::container::Stats;

/// Collects CPU usage for a Docker container.
pub struct CpuCollector;

impl CpuCollector {
    /// Calculates CPU usage percentage from a Docker stats sample.
    ///
    /// Same formula as `docker stats`: 100.0 means one fully used CPU, so
    /// the value goes up to `100.0 * online_cpus`. It covers the window
    /// between `precpu_stats` and `cpu_stats` (about one second), it is
    /// not an average over the polling interval.
    ///
    /// Returns None when Docker does not provide enough information,
    /// including when there is no previous sample to compare with.
    pub fn collect(stats: &Stats) -> Option<f64> {
        let current = &stats.cpu_stats;
        let previous = &stats.precpu_stats;

        let online_cpus = current
            .online_cpus
            .or_else(|| {
                current
                    .cpu_usage
                    .percpu_usage
                    .as_ref()
                    .map(|cpus| cpus.len() as u64)
            })
            .unwrap_or(1);

        cpu_percent(
            current.cpu_usage.total_usage,
            previous.cpu_usage.total_usage,
            current.system_cpu_usage?,
            previous.system_cpu_usage?,
            online_cpus,
        )
    }
}

/// Calculates CPU percentage from Docker CPU counters.
///
/// - `container_*`: CPU time used by the container (cumulative).
/// - `system_*`: CPU time of the whole host (cumulative).
///
/// Returns None for counters that cannot give a meaningful value:
/// a counter that went backwards (container restart), no host time elapsed,
/// or no previous sample. In the last case `precpu_stats` is all zeros, as
/// when Docker is asked for a one-shot sample, and dividing by the host
/// uptime would give a lifetime average instead of current usage.
fn cpu_percent(
    container_now: u64,
    container_before: u64,
    system_now: u64,
    system_before: u64,
    online_cpus: u64,
) -> Option<f64> {
    if system_before == 0 || online_cpus == 0 {
        return None;
    }

    let container_delta = container_now.checked_sub(container_before)?;
    let system_delta = system_now.checked_sub(system_before)?;

    if system_delta == 0 {
        return None;
    }

    let percent = container_delta as f64 / system_delta as f64 * online_cpus as f64 * 100.0;

    percent.is_finite().then_some(percent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::StatsBuilder;

    fn assert_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a value");

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // ---- cpu_percent -------------------------------------------------------

    #[test]
    fn calculates_cpu_usage() {
        assert_close(cpu_percent(150, 100, 1100, 1000, 2), 100.0);
    }

    #[test]
    fn calculates_single_core_usage() {
        assert_close(cpu_percent(125, 100, 1100, 1000, 1), 25.0);
    }

    #[test]
    fn supports_multiple_cores() {
        assert_close(cpu_percent(175, 100, 1100, 1000, 4), 300.0);
    }

    #[test]
    fn idle_container_is_zero_percent() {
        assert_close(cpu_percent(100, 100, 1100, 1000, 4), 0.0);
    }

    #[test]
    fn brand_new_container_with_zero_cpu_time_is_zero_percent() {
        assert_close(cpu_percent(0, 0, 1100, 1000, 4), 0.0);
    }

    #[test]
    fn no_previous_sample_gives_none() {
        // precpu_stats is all zeros: a lifetime average would be wrong.
        assert_eq!(cpu_percent(5_000_000, 0, 900_000_000, 0, 4), None);
    }

    #[test]
    fn counter_going_backwards_gives_none() {
        assert_eq!(cpu_percent(10, 500, 1100, 1000, 2), None);
        assert_eq!(cpu_percent(150, 100, 900, 1000, 2), None);
    }

    #[test]
    fn no_host_time_elapsed_gives_none() {
        assert_eq!(cpu_percent(150, 100, 1000, 1000, 2), None);
    }

    #[test]
    fn zero_online_cpus_gives_none() {
        assert_eq!(cpu_percent(150, 100, 1100, 1000, 0), None);
    }

    #[test]
    fn huge_counters_stay_finite() {
        let value = cpu_percent(u64::MAX, 1, u64::MAX, 1, 8).expect("a value");

        assert!(value.is_finite());
        assert!((value - 800.0).abs() < 1e-6);
    }

    // ---- collect (from a real bollard Stats) ---------------------------------

    #[test]
    fn collects_from_docker_stats() {
        let stats = StatsBuilder::default().build();

        assert_close(CpuCollector::collect(&stats), 100.0);
    }

    #[test]
    fn falls_back_to_percpu_usage_length_when_online_cpus_is_missing() {
        let stats = StatsBuilder {
            online_cpus: None,
            percpu: Some(vec![10, 20, 30, 40]),
            ..StatsBuilder::default()
        }
        .build();

        // delta 50 / 100 * 4 CPUs * 100
        assert_close(CpuCollector::collect(&stats), 200.0);
    }

    #[test]
    fn assumes_one_cpu_when_nothing_tells_the_count() {
        let stats = StatsBuilder {
            online_cpus: None,
            percpu: None,
            ..StatsBuilder::default()
        }
        .build();

        assert_close(CpuCollector::collect(&stats), 50.0);
    }

    #[test]
    fn empty_percpu_list_gives_none() {
        let stats = StatsBuilder {
            online_cpus: None,
            percpu: Some(vec![]),
            ..StatsBuilder::default()
        }
        .build();

        assert_eq!(CpuCollector::collect(&stats), None);
    }

    #[test]
    fn missing_system_cpu_usage_gives_none() {
        let current_missing = StatsBuilder {
            system: None,
            ..StatsBuilder::default()
        }
        .build();

        let previous_missing = StatsBuilder {
            presystem: None,
            ..StatsBuilder::default()
        }
        .build();

        assert_eq!(CpuCollector::collect(&current_missing), None);
        assert_eq!(CpuCollector::collect(&previous_missing), None);
    }

    #[test]
    fn one_shot_sample_without_previous_stats_gives_none() {
        let stats = StatsBuilder {
            precpu_total: 0,
            presystem: Some(0),
            cpu_total: 5_000_000,
            system: Some(900_000_000),
            ..StatsBuilder::default()
        }
        .build();

        assert_eq!(CpuCollector::collect(&stats), None);
    }
}
