use bollard::container::Stats;

use crate::collectors::container::{
    ConnectionsCollector, CpuCollector, DockerClient, MemoryCollector, NetworkCollector,
};
use crate::models::metrics::{NetworkMetrics as NetworkGroup, SystemMetrics};

/// 2^64 as f64 (exactly representable). Any value >= this overflows u64.
const U64_OVERFLOW_F64: f64 = 18_446_744_073_709_551_616.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerSystemMetrics {
    pub container_id: String,

    // CPU
    pub cpu_usage_percent: Option<f64>,

    // Memory
    pub memory_used_mb: Option<f64>,
    pub memory_limit_mb: Option<f64>,
    pub memory_usage_percent: Option<f64>,

    // Network
    pub network_rx_bytes_total: Option<u64>,
    pub network_tx_bytes_total: Option<u64>,
    pub network_rx_bytes_per_sec: Option<f64>,
    pub network_tx_bytes_per_sec: Option<f64>,

    // TCP
    pub tcp_established_connections: Option<u64>,
}

impl ContainerSystemMetrics {
    /// Every metric unavailable.
    fn unavailable(container_id: &str) -> Self {
        Self {
            container_id: container_id.to_string(),
            cpu_usage_percent: None,
            memory_used_mb: None,
            memory_limit_mb: None,
            memory_usage_percent: None,
            network_rx_bytes_total: None,
            network_tx_bytes_total: None,
            network_rx_bytes_per_sec: None,
            network_tx_bytes_per_sec: None,
            tcp_established_connections: None,
        }
    }
}

/// Converts the metrics of a container into the `system` group of an
/// observation.
///
/// - `cpu_usage_percent` keeps the Docker convention: 100.0 is one fully
///   used CPU, so it can exceed 100 on a multi-core container.
/// - `memory_used_mb` and `memory_total_mb` are whole MiB (rounded). The
///   total is the container memory limit, which Docker sets to the host
///   memory when no limit is configured.
/// - `memory_usage_percent` has no place in the model and is dropped, as
///   is the container id.
/// - Values that are not finite, or not a valid size, become None.
impl From<ContainerSystemMetrics> for SystemMetrics {
    fn from(container: ContainerSystemMetrics) -> Self {
        SystemMetrics {
            cpu_usage_percent: finite(container.cpu_usage_percent),
            memory_used_mb: whole_mib(container.memory_used_mb),
            memory_total_mb: whole_mib(container.memory_limit_mb),
            network: NetworkGroup {
                network_rx_bytes_total: container.network_rx_bytes_total,
                network_tx_bytes_total: container.network_tx_bytes_total,
                network_rx_bytes_per_sec: finite(container.network_rx_bytes_per_sec),
                network_tx_bytes_per_sec: finite(container.network_tx_bytes_per_sec),
                tcp_established_connections: container.tcp_established_connections,
            },
        }
    }
}

/// The value, unless it is NaN or infinite.
fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}

/// A size in MiB as a whole number, rounded to the nearest.
///
/// None for a negative, non-finite or u64-overflowing value.
fn whole_mib(mib: Option<f64>) -> Option<u64> {
    mib.filter(|mib| mib.is_finite() && *mib >= 0.0 && *mib < U64_OVERFLOW_F64)
        .map(|mib| mib.round() as u64)
}

pub struct SystemCollector {
    network_collector: NetworkCollector,
}

impl SystemCollector {
    pub fn new() -> Self {
        Self {
            network_collector: NetworkCollector::new(),
        }
    }

    /// Collects system metrics for one Docker container.
    ///
    /// container_pid is the PID of the container's main process
    /// as seen from the host.
    ///
    /// Fetching the statistics takes one to two seconds. To monitor several
    /// containers at once, call `DockerClient::get_stats` concurrently and
    /// feed each result to `collect_from_stats`, which does not wait.
    pub async fn collect(
        &mut self,
        docker_client: &DockerClient,
        container_id: &str,
        container_pid: u64,
    ) -> ContainerSystemMetrics {
        // Retrieve Docker statistics.
        let stats = match docker_client.get_stats(container_id).await {
            Ok(stats) => stats,
            Err(error) => {
                tracing::warn!(
                    container_id = %container_id,
                    error = %error,
                    "Could not collect Docker statistics"
                );

                // The PID is not read either: when Docker cannot see the
                // container, the PID is probably stale and may already
                // belong to another process.
                return ContainerSystemMetrics::unavailable(container_id);
            }
        };

        let tcp_established_connections =
            Self::count_tcp_connections(container_id, container_pid).await;

        self.collect_from_stats(container_id, &stats, tcp_established_connections)
    }

    /// Builds the metrics of one container from a statistics sample.
    ///
    /// Network rates are computed against the previous sample of the same
    /// container, so keep calling this on the same `SystemCollector`.
    pub fn collect_from_stats(
        &mut self,
        container_id: &str,
        stats: &Stats,
        tcp_established_connections: Option<u64>,
    ) -> ContainerSystemMetrics {
        let memory = MemoryCollector::collect(stats);
        let network = self.network_collector.collect(container_id, stats);

        ContainerSystemMetrics {
            container_id: container_id.to_string(),

            cpu_usage_percent: CpuCollector::collect(stats),

            memory_used_mb: memory.memory_used_mb,
            memory_limit_mb: memory.memory_limit_mb,
            memory_usage_percent: memory.memory_usage_percent,

            network_rx_bytes_total: network.rx_bytes_total,
            network_tx_bytes_total: network.tx_bytes_total,
            network_rx_bytes_per_sec: network.rx_bytes_per_sec,
            network_tx_bytes_per_sec: network.tx_bytes_per_sec,

            tcp_established_connections,
        }
    }

    /// Forgets the network baseline of a container that is no longer monitored.
    pub fn forget_container(&mut self, container_id: &str) {
        self.network_collector.forget(container_id);
    }

    /// Counts TCP connections without blocking the async runtime.
    async fn count_tcp_connections(container_id: &str, container_pid: u64) -> Option<u64> {
        let result =
            tokio::task::spawn_blocking(move || ConnectionsCollector::collect(container_pid)).await;

        match result {
            Ok(Ok(count)) => Some(count),
            Ok(Err(error)) => {
                tracing::warn!(
                    container_id = %container_id,
                    container_pid,
                    error = %error,
                    "Could not count TCP connections"
                );

                None
            }
            Err(error) => {
                tracing::warn!(
                    container_id = %container_id,
                    error = %error,
                    "TCP connection counting task failed"
                );

                None
            }
        }
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::{
        spawn_fake_docker, MemoryStat, Reply, StatsBuilder, MIB,
    };
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    fn own_pid() -> u64 {
        u64::from(std::process::id())
    }

    fn assert_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a value");

        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    // ---- collect_from_stats ----------------------------------------------------

    #[test]
    fn maps_every_collector_into_the_result() {
        let mut collector = SystemCollector::new();
        let stats = StatsBuilder::default().build();

        let metrics = collector.collect_from_stats("web", &stats, Some(7));

        assert_eq!(metrics.container_id, "web");
        assert_close(metrics.cpu_usage_percent, 100.0);
        assert_close(metrics.memory_used_mb, 200.0);
        assert_close(metrics.memory_limit_mb, 1024.0);
        assert_close(metrics.memory_usage_percent, 200.0 / 1024.0 * 100.0);
        assert_eq!(metrics.network_rx_bytes_total, Some(1_000));
        assert_eq!(metrics.network_tx_bytes_total, Some(2_000));
        assert_eq!(metrics.network_rx_bytes_per_sec, None);
        assert_eq!(metrics.network_tx_bytes_per_sec, None);
        assert_eq!(metrics.tcp_established_connections, Some(7));
    }

    #[test]
    fn unavailable_tcp_count_is_passed_through() {
        let mut collector = SystemCollector::new();
        let stats = StatsBuilder::default().build();

        let metrics = collector.collect_from_stats("web", &stats, None);

        assert_eq!(metrics.tcp_established_connections, None);
        assert!(metrics.cpu_usage_percent.is_some());
    }

    #[test]
    fn missing_pieces_of_a_sample_stay_none_independently() {
        let mut collector = SystemCollector::new();
        let stats = StatsBuilder {
            system: None,
            memory_usage: None,
            memory_stat: MemoryStat::Missing,
            networks: None,
            ..StatsBuilder::default()
        }
        .build();

        let metrics = collector.collect_from_stats("web", &stats, Some(1));

        assert_eq!(metrics.cpu_usage_percent, None);
        assert_eq!(metrics.memory_used_mb, None);
        assert_eq!(metrics.network_rx_bytes_total, None);
        // The memory limit and the TCP count do not depend on those.
        assert_close(metrics.memory_limit_mb, 1024.0);
        assert_eq!(metrics.tcp_established_connections, Some(1));
    }

    #[test]
    fn network_rates_appear_on_the_second_sample() {
        let mut collector = SystemCollector::new();

        let first = collector.collect_from_stats(
            "web",
            &StatsBuilder {
                networks: Some(vec![("eth0", 1_000, 1_000)]),
                ..StatsBuilder::default()
            }
            .build(),
            None,
        );

        std::thread::sleep(Duration::from_millis(10));

        let second = collector.collect_from_stats(
            "web",
            &StatsBuilder {
                networks: Some(vec![("eth0", 3_000, 1_000)]),
                ..StatsBuilder::default()
            }
            .build(),
            None,
        );

        assert_eq!(first.network_rx_bytes_per_sec, None);
        assert!(second.network_rx_bytes_per_sec.expect("rx rate") > 0.0);
        assert_close(second.network_tx_bytes_per_sec, 0.0);
    }

    #[test]
    fn containers_keep_separate_network_baselines() {
        let mut collector = SystemCollector::new();
        let stats = StatsBuilder::default().build();

        collector.collect_from_stats("a", &stats, None);
        std::thread::sleep(Duration::from_millis(5));

        let other = collector.collect_from_stats("b", &stats, None);
        let again = collector.collect_from_stats("a", &stats, None);

        assert_eq!(other.network_rx_bytes_per_sec, None);
        assert!(again.network_rx_bytes_per_sec.is_some());
    }

    #[test]
    fn forgetting_a_container_resets_its_rates() {
        let mut collector = SystemCollector::new();
        let stats = StatsBuilder::default().build();

        collector.collect_from_stats("a", &stats, None);
        collector.forget_container("a");

        let metrics = collector.collect_from_stats("a", &stats, None);

        assert_eq!(metrics.network_rx_bytes_per_sec, None);
    }

    // ---- collect (against a fake Docker daemon) -----------------------------------

    #[tokio::test]
    async fn collects_everything_for_a_running_container() {
        let fake = spawn_fake_docker(Reply::Json(StatsBuilder::default().json().to_string()));
        let client = DockerClient::from_docker(fake.docker.clone());
        let mut collector = SystemCollector::new();

        // Two connected sockets in this process: the PID of the "container"
        // is the test process itself.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let peer = TcpStream::connect(listener.local_addr().unwrap()).expect("connect");
        let (accepted, _) = listener.accept().expect("accept");

        let metrics = collector.collect(&client, "web", own_pid()).await;

        assert_eq!(metrics.container_id, "web");
        assert_close(metrics.cpu_usage_percent, 100.0);
        assert_close(metrics.memory_used_mb, 200.0);
        assert_eq!(metrics.network_rx_bytes_total, Some(1_000));
        assert!(
            metrics.tcp_established_connections.expect("tcp count") >= 2,
            "{metrics:?}"
        );

        drop((peer, accepted));
    }

    #[tokio::test]
    async fn memory_of_a_container_with_a_big_cache_is_reported_without_it() {
        let stats = StatsBuilder {
            memory_usage: Some(900 * MIB),
            memory_stat: MemoryStat::V2 {
                inactive_file: 800 * MIB,
            },
            ..StatsBuilder::default()
        };
        let fake = spawn_fake_docker(Reply::Json(stats.json().to_string()));
        let client = DockerClient::from_docker(fake.docker.clone());

        let metrics = SystemCollector::new().collect(&client, "web", 0).await;

        assert_close(metrics.memory_used_mb, 100.0);
    }

    #[tokio::test]
    async fn docker_failure_gives_all_none_and_skips_the_pid() {
        let fake = spawn_fake_docker(Reply::Status(404, "No such container"));
        let client = DockerClient::from_docker(fake.docker.clone());

        // A perfectly readable PID: it must still not be used.
        let metrics = SystemCollector::new()
            .collect(&client, "gone", own_pid())
            .await;

        assert_eq!(metrics, ContainerSystemMetrics::unavailable("gone"));
        assert_eq!(metrics.container_id, "gone");
    }

    #[tokio::test]
    async fn docker_timeout_gives_all_none() {
        let fake = spawn_fake_docker(Reply::Hang);
        let client = DockerClient::from_docker(fake.docker.clone())
            .with_stats_timeout(Duration::from_millis(100));

        let metrics = SystemCollector::new()
            .collect(&client, "slow", own_pid())
            .await;

        assert_eq!(metrics, ContainerSystemMetrics::unavailable("slow"));
    }

    #[tokio::test]
    async fn unreadable_pid_only_loses_the_tcp_count() {
        let fake = spawn_fake_docker(Reply::Json(StatsBuilder::default().json().to_string()));
        let client = DockerClient::from_docker(fake.docker.clone());

        // PID 0 is what Docker reports for a stopped container.
        let metrics = SystemCollector::new().collect(&client, "web", 0).await;

        assert_eq!(metrics.tcp_established_connections, None);
        assert_close(metrics.cpu_usage_percent, 100.0);
        assert_eq!(metrics.network_rx_bytes_total, Some(1_000));
    }

    #[tokio::test]
    async fn rates_work_across_collect_calls() {
        let fake = spawn_fake_docker(Reply::Json(StatsBuilder::default().json().to_string()));
        let client = DockerClient::from_docker(fake.docker.clone());
        let mut collector = SystemCollector::default();

        let first = collector.collect(&client, "web", 0).await;
        let second = collector.collect(&client, "web", 0).await;

        assert_eq!(first.network_rx_bytes_per_sec, None);
        // The fake daemon repeats the same counters: traffic is zero.
        assert_close(second.network_rx_bytes_per_sec, 0.0);
    }

    // ---- conversion into the observation model ------------------------------------

    fn container_metrics() -> ContainerSystemMetrics {
        ContainerSystemMetrics {
            container_id: "web".to_string(),
            cpu_usage_percent: Some(150.0),
            memory_used_mb: Some(200.0),
            memory_limit_mb: Some(1024.0),
            memory_usage_percent: Some(19.5),
            network_rx_bytes_total: Some(1_000),
            network_tx_bytes_total: Some(2_000),
            network_rx_bytes_per_sec: Some(10.5),
            network_tx_bytes_per_sec: Some(20.25),
            tcp_established_connections: Some(7),
        }
    }

    #[test]
    fn converts_every_field_into_the_system_group() {
        let system = SystemMetrics::from(container_metrics());

        assert_eq!(system.cpu_usage_percent, Some(150.0));
        assert_eq!(system.memory_used_mb, Some(200));
        assert_eq!(system.memory_total_mb, Some(1024));
        assert_eq!(system.network.network_rx_bytes_total, Some(1_000));
        assert_eq!(system.network.network_tx_bytes_total, Some(2_000));
        assert_eq!(system.network.network_rx_bytes_per_sec, Some(10.5));
        assert_eq!(system.network.network_tx_bytes_per_sec, Some(20.25));
        assert_eq!(system.network.tcp_established_connections, Some(7));
    }

    #[test]
    fn unavailable_container_gives_an_empty_system_group() {
        let system = SystemMetrics::from(ContainerSystemMetrics::unavailable("gone"));

        assert_eq!(system, SystemMetrics::default());
    }

    #[test]
    fn memory_is_rounded_to_whole_mib() {
        let mut container = container_metrics();
        container.memory_used_mb = Some(200.4);
        container.memory_limit_mb = Some(1023.5);

        let system = SystemMetrics::from(container);

        assert_eq!(system.memory_used_mb, Some(200));
        assert_eq!(system.memory_total_mb, Some(1024));
    }

    #[test]
    fn invalid_values_become_none_instead_of_nonsense() {
        let mut container = container_metrics();
        container.cpu_usage_percent = Some(f64::NAN);
        container.memory_used_mb = Some(-1.0);
        container.memory_limit_mb = Some(f64::INFINITY);
        container.network_rx_bytes_per_sec = Some(f64::NEG_INFINITY);

        let system = SystemMetrics::from(container);

        assert_eq!(system.cpu_usage_percent, None);
        assert_eq!(system.memory_used_mb, None);
        assert_eq!(system.memory_total_mb, None);
        assert_eq!(system.network.network_rx_bytes_per_sec, None);
        // Valid neighbours are untouched.
        assert_eq!(system.network.network_tx_bytes_per_sec, Some(20.25));
    }

    #[test]
    fn sizes_that_overflow_u64_become_none() {
        assert_eq!(whole_mib(Some(1e30)), None);
        assert_eq!(whole_mib(Some(18_446_744_073_709_551_616.0)), None);
        assert_eq!(whole_mib(Some(0.0)), Some(0));
    }

    #[test]
    fn no_limit_means_no_total_memory() {
        let mut container = container_metrics();
        container.memory_limit_mb = None;

        let system = SystemMetrics::from(container);

        assert_eq!(system.memory_used_mb, Some(200));
        assert_eq!(system.memory_total_mb, None);
    }

    #[test]
    fn a_collected_sample_converts_end_to_end() {
        let stats = StatsBuilder::default().build();
        let mut collector = SystemCollector::new();

        let system = SystemMetrics::from(collector.collect_from_stats("web", &stats, Some(3)));

        assert_eq!(system.cpu_usage_percent, Some(100.0));
        assert_eq!(system.memory_used_mb, Some(200));
        assert_eq!(system.memory_total_mb, Some(1024));
        assert_eq!(system.network.network_rx_bytes_total, Some(1_000));
        assert_eq!(system.network.tcp_established_connections, Some(3));
    }

    #[test]
    fn converted_group_keeps_the_eight_contract_keys() {
        let value = serde_json::to_value(SystemMetrics::from(container_metrics())).unwrap();
        let object = value.as_object().expect("a JSON object");

        assert_eq!(object.len(), 8, "{object:?}");
        assert!(object.values().all(|value| !value.is_null()), "{object:?}");
    }
}
