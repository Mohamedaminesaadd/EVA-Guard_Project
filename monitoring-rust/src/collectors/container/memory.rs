use bollard::container::{MemoryStatsStats, Stats};

const BYTES_PER_MIB: f64 = 1_048_576.0;

/// Memory usage of a container.
///
/// Despite the `_mb` suffix the values are MiB (1024 * 1024 bytes), like
/// the figures of `docker stats`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemoryMetrics {
    pub memory_used_mb: Option<f64>,
    pub memory_limit_mb: Option<f64>,
    pub memory_usage_percent: Option<f64>,
}

pub struct MemoryCollector;

impl MemoryCollector {
    /// Collects memory usage from a Docker stats sample.
    ///
    /// Memory used follows `docker stats`: the cgroup usage minus the
    /// inactive file cache, which the kernel reclaims under pressure.
    pub fn collect(stats: &Stats) -> MemoryMetrics {
        let memory_stats = &stats.memory_stats;

        memory_metrics(
            memory_stats.usage,
            inactive_file(memory_stats.stats.as_ref()),
            memory_stats.limit,
        )
    }
}

/// Inactive file cache of the container, in bytes.
///
/// cgroup v1 reports it as `total_inactive_file`, cgroup v2 as
/// `inactive_file`. Zero when Docker does not send the breakdown.
fn inactive_file(stats: Option<&MemoryStatsStats>) -> u64 {
    match stats {
        Some(MemoryStatsStats::V1(v1)) => v1.total_inactive_file,
        Some(MemoryStatsStats::V2(v2)) => v2.inactive_file,
        None => 0,
    }
}

/// Memory in use once the inactive file cache is left out.
///
/// An inactive cache larger than the usage is inconsistent: the raw usage
/// is kept, as `docker stats` does.
fn working_set(usage: u64, inactive_file: u64) -> u64 {
    if inactive_file < usage {
        usage - inactive_file
    } else {
        usage
    }
}

fn to_mib(bytes: u64) -> f64 {
    bytes as f64 / BYTES_PER_MIB
}

/// Builds the metrics from raw Docker values (bytes).
///
/// A missing or zero limit means "no limit known": no limit and no
/// percentage are reported.
fn memory_metrics(usage: Option<u64>, inactive_file: u64, limit: Option<u64>) -> MemoryMetrics {
    let used = usage.map(|usage| working_set(usage, inactive_file));
    let limit = limit.filter(|limit| *limit > 0);

    MemoryMetrics {
        memory_used_mb: used.map(to_mib),
        memory_limit_mb: limit.map(to_mib),
        memory_usage_percent: used
            .zip(limit)
            .map(|(used, limit)| used as f64 / limit as f64 * 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::{MemoryStat, StatsBuilder, MIB};

    fn assert_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a value");

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // ---- memory_metrics ------------------------------------------------------

    #[test]
    fn subtracts_inactive_file_cache() {
        let metrics = memory_metrics(Some(300 * MIB), 100 * MIB, Some(1024 * MIB));

        assert_close(metrics.memory_used_mb, 200.0);
        assert_close(metrics.memory_limit_mb, 1024.0);
        assert_close(metrics.memory_usage_percent, 200.0 / 1024.0 * 100.0);
    }

    #[test]
    fn reports_raw_usage_when_there_is_no_cache() {
        let metrics = memory_metrics(Some(512 * MIB), 0, Some(1024 * MIB));

        assert_close(metrics.memory_used_mb, 512.0);
        assert_close(metrics.memory_usage_percent, 50.0);
    }

    #[test]
    fn keeps_raw_usage_when_cache_is_not_smaller_than_usage() {
        let equal = memory_metrics(Some(100 * MIB), 100 * MIB, Some(1024 * MIB));
        let larger = memory_metrics(Some(100 * MIB), 400 * MIB, Some(1024 * MIB));

        assert_close(equal.memory_used_mb, 100.0);
        assert_close(larger.memory_used_mb, 100.0);
    }

    #[test]
    fn zero_limit_means_no_limit_and_no_percentage() {
        let metrics = memory_metrics(Some(100 * MIB), 0, Some(0));

        assert_close(metrics.memory_used_mb, 100.0);
        assert_eq!(metrics.memory_limit_mb, None);
        assert_eq!(metrics.memory_usage_percent, None);
    }

    #[test]
    fn missing_limit_means_no_limit_and_no_percentage() {
        let metrics = memory_metrics(Some(100 * MIB), 0, None);

        assert_close(metrics.memory_used_mb, 100.0);
        assert_eq!(metrics.memory_limit_mb, None);
        assert_eq!(metrics.memory_usage_percent, None);
    }

    #[test]
    fn missing_usage_keeps_the_limit_only() {
        let metrics = memory_metrics(None, 50 * MIB, Some(1024 * MIB));

        assert_eq!(metrics.memory_used_mb, None);
        assert_eq!(metrics.memory_usage_percent, None);
        assert_close(metrics.memory_limit_mb, 1024.0);
    }

    #[test]
    fn nothing_known_gives_default_metrics() {
        assert_eq!(memory_metrics(None, 0, None), MemoryMetrics::default());
    }

    #[test]
    fn usage_above_limit_is_reported_as_is() {
        let metrics = memory_metrics(Some(2048 * MIB), 0, Some(1024 * MIB));

        assert_close(metrics.memory_usage_percent, 200.0);
    }

    #[test]
    fn huge_values_stay_finite() {
        let metrics = memory_metrics(Some(u64::MAX), 0, Some(u64::MAX));

        assert_close(metrics.memory_usage_percent, 100.0);
        assert!(metrics.memory_used_mb.expect("a value").is_finite());
    }

    // ---- collect (from a real bollard Stats) ------------------------------------

    #[test]
    fn collects_with_cgroup_v2_stats() {
        let stats = StatsBuilder::default().build();

        let metrics = MemoryCollector::collect(&stats);

        assert_close(metrics.memory_used_mb, 200.0);
        assert_close(metrics.memory_limit_mb, 1024.0);
        assert_close(metrics.memory_usage_percent, 200.0 / 1024.0 * 100.0);
    }

    #[test]
    fn cgroup_v1_uses_total_inactive_file() {
        // `inactive_file` is deliberately different: only the hierarchical
        // `total_inactive_file` must be subtracted on cgroup v1.
        let stats = StatsBuilder {
            memory_stat: MemoryStat::V1 {
                total_inactive_file: 50 * MIB,
                inactive_file: 10 * MIB,
            },
            ..StatsBuilder::default()
        }
        .build();

        let metrics = MemoryCollector::collect(&stats);

        assert_close(metrics.memory_used_mb, 250.0);
    }

    #[test]
    fn without_breakdown_the_raw_usage_is_reported() {
        let stats = StatsBuilder {
            memory_stat: MemoryStat::Missing,
            ..StatsBuilder::default()
        }
        .build();

        let metrics = MemoryCollector::collect(&stats);

        assert_close(metrics.memory_used_mb, 300.0);
    }

    #[test]
    fn stopped_container_without_memory_data_gives_default_metrics() {
        let stats = StatsBuilder {
            memory_usage: None,
            memory_limit: None,
            memory_stat: MemoryStat::Missing,
            ..StatsBuilder::default()
        }
        .build();

        assert_eq!(MemoryCollector::collect(&stats), MemoryMetrics::default());
    }
}
