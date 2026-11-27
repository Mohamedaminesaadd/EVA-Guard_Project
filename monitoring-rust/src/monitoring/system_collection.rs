//! System metrics (CPU, memory, network, TCP) of the containers of each node.
//!
//! Every observation carries the resources of the container that runs its
//! own component: the Geth observation gets the execution container, the
//! Beacon observation the Lighthouse Beacon Node container, the Validator
//! observation the Lighthouse Validator Client container.
//!
//! # Finding the containers
//!
//! A [`ContainerDirectory`], shared by every node, maps `(node, role)` to a
//! container ID with one `docker ps -a` call:
//!
//! 1. the Kurtosis Ethereum-package labels give the node (`node-index` →
//!    `node-NN`) and the role (`client-type` + `client`); container names
//!    are never used;
//! 2. among the containers carrying those labels, the one kept is the
//!    *running* container that publishes a host port configured for that
//!    component in `nodes.toml`. This proves that the system metrics come
//!    from the very container the application metrics are read from, and
//!    sets aside the containers of another (old or parallel) enclave, which
//!    carry the same labels.
//!
//! The mapping is resolved per role: a missing, stopped or ambiguous
//! container only blanks its own `system` group. It is refreshed every
//! [`REFRESH_EVERY`], and sooner (at most every [`MIN_REFRESH_GAP`]) after a
//! sign that it went stale: a failed stats request, a container that stopped,
//! a PID that no longer belongs to the container.
//!
//! # Collecting
//!
//! Each node's [`NodeSystemCollector`] asks Docker for the statistics of its
//! three containers concurrently (one shared Docker connection, a bounded
//! timeout per request) and counts the TCP connections in each container's
//! network namespace through `/proc/<pid>/net/tcp{,6}`, after checking that
//! the PID still belongs to the container. It keeps one stateful
//! [`SystemCollector`] for the whole run, so network rates appear from the
//! second sample.
//!
//! A metric that could not be measured is `None` (JSON `null`) and the reason
//! is logged at WARN: a failure is never written as `0`.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bollard::container::Stats;
use tokio::time::Instant;

use crate::collectors::container::{
    classify, ComponentRole, ConnectionsCollector, DockerClient, DockerClientError,
    DockerContainer,
};
use crate::collectors::system::{ContainerSystemMetrics, SystemCollector};
use crate::config::NodeConfig;
use crate::models::metrics::SystemMetrics;

/// Periodic re-discovery, to follow containers recreated by Kurtosis.
pub const REFRESH_EVERY: Duration = Duration::from_secs(60);

/// Minimum delay between two re-discoveries triggered by a failure, so a
/// container that stays down does not cost a Docker listing every cycle.
pub const MIN_REFRESH_GAP: Duration = Duration::from_secs(10);

/// Timeout of one Docker stats request for a collector budget.
///
/// A sample takes about one second (Docker measures CPU over a one second
/// window), two when the 15 containers are sampled together. 70 % of the
/// budget leaves room for a re-discovery in the same cycle, so a slow
/// daemon produces a logged timeout rather than a silent budget cut.
pub fn stats_timeout_for(collector_budget: Duration) -> Duration {
    collector_budget.mul_f64(0.7)
}

/// Timeout of one re-discovery (list + inspections) for a collector budget.
pub fn refresh_timeout_for(collector_budget: Duration) -> Duration {
    collector_budget.mul_f64(0.2)
}

/// The `system` group of each component observation of one node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentSystems {
    pub geth: SystemMetrics,
    pub beacon: SystemMetrics,
    pub validator: SystemMetrics,
}

/// A running container selected for a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub container_id: String,
    pub container_name: Option<String>,
    /// Host PID of the container's main process (from `docker inspect`);
    /// `None` when the inspection failed or reported no PID.
    pub host_pid: Option<u64>,
}

/// What the directory knows about the container of one component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Running(Target),
    /// Only stopped containers carry the labels of this component.
    Stopped { container_id: String, state: String },
    /// No usable container; the reason is logged.
    Missing { reason: String },
}

type Key = (String, ComponentRole);

/// Host ports configured in `nodes.toml` for each component of each node.
fn expected_ports(nodes: &[NodeConfig]) -> HashMap<Key, Vec<u16>> {
    let mut expected = HashMap::new();

    for node in nodes {
        let urls: [(ComponentRole, Vec<&str>); 3] = [
            (
                ComponentRole::Geth,
                vec![node.geth.rpc_url.as_str(), node.geth.metrics_url.as_str()],
            ),
            (
                ComponentRole::Beacon,
                vec![
                    node.beacon.api_url.as_str(),
                    node.beacon.metrics_url.as_str(),
                ],
            ),
            (
                ComponentRole::Validator,
                std::iter::once(node.validator.metrics_url.as_str())
                    .chain(node.validator.api_url.as_deref())
                    .collect(),
            ),
        ];

        for (role, urls) in urls {
            let mut ports: Vec<u16> = urls.into_iter().filter_map(url_port).collect();
            ports.sort_unstable();
            ports.dedup();
            expected.insert((node.id.clone(), role), ports);
        }
    }

    expected
}

fn url_port(url: &str) -> Option<u16> {
    reqwest::Url::parse(url).ok()?.port_or_known_default()
}

/// Maps every expected component to a container. Pure: no Docker call.
pub(crate) fn resolve(
    containers: &[DockerContainer],
    expected: &HashMap<Key, Vec<u16>>,
) -> HashMap<Key, Resolution> {
    let mut candidates: HashMap<Key, Vec<&DockerContainer>> = HashMap::new();

    for container in containers {
        match classify(container) {
            Ok(Some(key)) => candidates.entry(key).or_default().push(container),
            Ok(None) => {}
            Err(error) => tracing::warn!(
                %error,
                "container ignored for system metrics: unusable Ethereum-package labels"
            ),
        }
    }

    expected
        .iter()
        .map(|(key, ports)| {
            let found = candidates.get(key).map(Vec::as_slice).unwrap_or_default();
            (key.clone(), resolve_one(found, ports))
        })
        .collect()
}

fn resolve_one(found: &[&DockerContainer], ports: &[u16]) -> Resolution {
    let running: Vec<&DockerContainer> = found
        .iter()
        .copied()
        .filter(|container| container.is_running())
        .collect();
    let matching: Vec<&DockerContainer> = running
        .iter()
        .copied()
        .filter(|container| {
            container
                .public_ports
                .iter()
                .any(|port| ports.contains(port))
        })
        .collect();

    match matching.as_slice() {
        [container] => Resolution::Running(Target {
            container_id: container.id.clone(),
            container_name: container.name.clone(),
            host_pid: None,
        }),
        [] if !running.is_empty() => Resolution::Missing {
            reason: format!(
                "{} running container(s) carry the labels of this component, but none publishes \
                 a configured port {ports:?}: they belong to another enclave, or nodes.toml is \
                 out of date",
                running.len()
            ),
        },
        [] => match found.first() {
            Some(stopped) => Resolution::Stopped {
                container_id: stopped.id.clone(),
                state: stopped
                    .state
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            },
            None => Resolution::Missing {
                reason: "no container carries the labels of this component".to_string(),
            },
        },
        many => Resolution::Missing {
            reason: format!(
                "{} running containers publish a configured port {ports:?}: ambiguous",
                many.len()
            ),
        },
    }
}

/// The first 12 characters of a container ID, as `docker ps` shows them.
fn short(container_id: &str) -> &str {
    container_id.get(..12).unwrap_or(container_id)
}

#[derive(Default)]
struct DirectoryState {
    resolutions: HashMap<Key, Resolution>,
    attempted_at: Option<Instant>,
    last_error: Option<String>,
}

/// Container mapping shared by every node. See the module documentation.
pub struct ContainerDirectory {
    docker: DockerClient,
    expected: HashMap<Key, Vec<u16>>,
    refresh_timeout: Duration,
    refresh_every: Duration,
    min_refresh_gap: Duration,
    stale: AtomicBool,
    state: tokio::sync::Mutex<DirectoryState>,
}

impl ContainerDirectory {
    /// A directory for the nodes of `nodes.toml`. Does not touch Docker:
    /// the first lookup discovers the containers.
    pub fn new(docker: DockerClient, nodes: &[NodeConfig], refresh_timeout: Duration) -> Self {
        Self {
            docker,
            expected: expected_ports(nodes),
            refresh_timeout,
            refresh_every: REFRESH_EVERY,
            min_refresh_gap: MIN_REFRESH_GAP,
            stale: AtomicBool::new(false),
            state: tokio::sync::Mutex::new(DirectoryState::default()),
        }
    }

    /// Overrides the re-discovery delays (tests).
    pub fn with_refresh_policy(mut self, every: Duration, min_gap: Duration) -> Self {
        self.refresh_every = every;
        self.min_refresh_gap = min_gap;
        self
    }

    pub fn docker(&self) -> &DockerClient {
        &self.docker
    }

    /// Asks for a re-discovery at the next lookup (rate limited).
    pub fn mark_stale(&self) {
        self.stale.store(true, Ordering::Relaxed);
    }

    /// The containers of the geth, beacon and validator components of a
    /// node, in that order. Re-discovers first when due; concurrent lookups
    /// share one discovery.
    pub async fn lookup(&self, node_id: &str) -> [Resolution; 3] {
        let mut state = self.state.lock().await;

        if self.needs_refresh(&state) {
            self.stale.store(false, Ordering::Relaxed);
            self.refresh(&mut state).await;
        }

        ComponentRole::ALL.map(|role| {
            state
                .resolutions
                .get(&(node_id.to_string(), role))
                .cloned()
                .unwrap_or_else(|| Resolution::Missing {
                    reason: match &state.last_error {
                        Some(error) => format!("Docker container discovery failed: {error}"),
                        None => "node absent from the container directory".to_string(),
                    },
                })
        })
    }

    fn needs_refresh(&self, state: &DirectoryState) -> bool {
        let Some(attempted_at) = state.attempted_at else {
            return true;
        };
        let age = attempted_at.elapsed();

        age >= self.refresh_every
            || (age >= self.min_refresh_gap
                && (self.stale.load(Ordering::Relaxed) || state.last_error.is_some()))
    }

    async fn refresh(&self, state: &mut DirectoryState) {
        let started = Instant::now();
        let outcome = tokio::time::timeout(self.refresh_timeout, self.discover()).await;
        state.attempted_at = Some(Instant::now());
        let elapsed_ms = started.elapsed().as_millis() as u64;

        let resolutions = match outcome {
            Ok(Ok(resolutions)) => resolutions,
            Ok(Err(error)) => {
                tracing::warn!(
                    %error,
                    elapsed_ms,
                    "Docker container discovery failed: system metrics stay null until it succeeds"
                );
                state.last_error = Some(error.to_string());
                return;
            }
            Err(_) => {
                let error = format!("timed out after {:?}", self.refresh_timeout);
                tracing::warn!(
                    %error,
                    "Docker container discovery failed: system metrics stay null until it succeeds"
                );
                state.last_error = Some(error);
                return;
            }
        };

        let mut keys: Vec<&Key> = resolutions.keys().collect();
        keys.sort();
        for key in keys {
            let new = &resolutions[key];
            if state.resolutions.get(key) != Some(new) {
                log_resolution(key, new);
            }
        }

        let running = resolutions
            .values()
            .filter(|resolution| matches!(resolution, Resolution::Running(_)))
            .count();
        tracing::debug!(
            running,
            expected = resolutions.len(),
            elapsed_ms,
            "container directory refreshed"
        );

        state.resolutions = resolutions;
        state.last_error = None;
    }

    /// One listing, then one inspection per running target (for its PID).
    async fn discover(&self) -> Result<HashMap<Key, Resolution>, DockerClientError> {
        let containers = self.docker.list_kurtosis_services_all().await?;
        let mut resolutions = resolve(&containers, &self.expected);

        let to_inspect: Vec<(Key, String)> = resolutions
            .iter()
            .filter_map(|(key, resolution)| match resolution {
                Resolution::Running(target) => Some((key.clone(), target.container_id.clone())),
                _ => None,
            })
            .collect();

        let inspections =
            futures_util::future::join_all(to_inspect.into_iter().map(|(key, id)| async move {
                let inspection = self.docker.inspect_container(&id).await;
                (key, inspection)
            }))
            .await;

        for (key, inspection) in inspections {
            if let Some(Resolution::Running(target)) = resolutions.get_mut(&key) {
                match inspection {
                    Ok(inspection) => target.host_pid = inspection.host_pid,
                    Err(error) => tracing::warn!(
                        node = %key.0,
                        role = key.1.as_str(),
                        container_id = short(&target.container_id),
                        %error,
                        "container inspection failed: its TCP connections cannot be counted"
                    ),
                }
            }
        }

        Ok(resolutions)
    }
}

fn log_resolution((node, role): &Key, resolution: &Resolution) {
    match resolution {
        Resolution::Running(target) => tracing::info!(
            node = %node,
            role = role.as_str(),
            container_id = short(&target.container_id),
            container_name = target.container_name.as_deref().unwrap_or("-"),
            state = "running",
            host_pid = ?target.host_pid,
            "container mapped for system metrics"
        ),
        Resolution::Stopped {
            container_id,
            state,
        } => tracing::warn!(
            node = %node,
            role = role.as_str(),
            container_id = short(container_id),
            state = %state,
            "container is not running: its system metrics will be null"
        ),
        Resolution::Missing { reason } => tracing::warn!(
            node = %node,
            role = role.as_str(),
            reason = %reason,
            "no usable container: system metrics of this component will be null"
        ),
    }
}

struct CollectorState {
    collector: SystemCollector,
    /// The container measured last for each role, to drop its network
    /// baseline when the role moves to another container.
    last_container: HashMap<ComponentRole, String>,
}

/// Collects the system metrics of the three containers of one node.
///
/// Built once per node and reused for every cycle: it keeps the previous
/// network sample of each container.
pub struct NodeSystemCollector {
    node_id: String,
    directory: Arc<ContainerDirectory>,
    proc_root: PathBuf,
    state: Mutex<CollectorState>,
}

impl NodeSystemCollector {
    pub fn new(node_id: impl Into<String>, directory: Arc<ContainerDirectory>) -> Self {
        Self {
            node_id: node_id.into(),
            directory,
            proc_root: PathBuf::from("/proc"),
            state: Mutex::new(CollectorState {
                collector: SystemCollector::new(),
                last_container: HashMap::new(),
            }),
        }
    }

    /// Reads `/proc` from another root (tests).
    pub fn with_proc_root(mut self, proc_root: impl Into<PathBuf>) -> Self {
        self.proc_root = proc_root.into();
        self
    }

    /// One cycle: the three containers are measured concurrently, and a
    /// failure of one leaves the two others untouched.
    pub async fn collect(&self) -> ComponentSystems {
        let [geth, beacon, validator] = self.directory.lookup(&self.node_id).await;

        let (geth, beacon, validator) = tokio::join!(
            self.collect_one(ComponentRole::Geth, geth),
            self.collect_one(ComponentRole::Beacon, beacon),
            self.collect_one(ComponentRole::Validator, validator),
        );

        ComponentSystems {
            geth,
            beacon,
            validator,
        }
    }

    async fn collect_one(&self, role: ComponentRole, resolution: Resolution) -> SystemMetrics {
        match resolution {
            Resolution::Running(target) => self.collect_running(role, &target).await,
            Resolution::Stopped {
                container_id,
                state,
            } => {
                self.forget(role);
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    container_id = short(&container_id),
                    state = %state,
                    "system metrics not collected: the container is not running"
                );
                SystemMetrics::default()
            }
            Resolution::Missing { reason } => {
                self.forget(role);
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    reason = %reason,
                    "system metrics not collected: no container for this component"
                );
                SystemMetrics::default()
            }
        }
    }

    async fn collect_running(&self, role: ComponentRole, target: &Target) -> SystemMetrics {
        let started = Instant::now();
        let id = target.container_id.as_str();

        let stats = match self.directory.docker().get_stats(id).await {
            Ok(stats) => stats,
            Err(error) => {
                self.directory.mark_stale();
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    container_id = short(id),
                    state = "running",
                    %error,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Docker stats request failed: system metrics left null"
                );
                return SystemMetrics::default();
            }
        };

        // A container that stopped since the discovery still gets HTTP 200,
        // with an empty sample: the collectors turn it into nulls.
        if looks_stopped(&stats) {
            self.directory.mark_stale();
            tracing::warn!(
                node = %self.node_id,
                role = role.as_str(),
                container_id = short(id),
                "Docker returned an empty sample (container stopped?): system metrics left null"
            );
        }

        let tcp = self.count_tcp(role, target).await;

        let metrics = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let state = &mut *state;
            if let Some(previous) = state.last_container.insert(role, id.to_string()) {
                if previous != id {
                    state.collector.forget_container(&previous);
                }
            }
            state.collector.collect_from_stats(id, &stats, tcp)
        };

        self.log_extraction(role, &metrics, started);

        SystemMetrics::from(metrics)
    }

    /// Established TCP connections in the container's network namespace,
    /// once the PID is proven to still belong to the container.
    async fn count_tcp(&self, role: ComponentRole, target: &Target) -> Option<u64> {
        let id = target.container_id.as_str();
        let Some(pid) = target.host_pid else {
            self.directory.mark_stale();
            tracing::warn!(
                node = %self.node_id,
                role = role.as_str(),
                container_id = short(id),
                "no host PID known for the container: TCP connections not counted"
            );
            return None;
        };

        let proc_root = self.proc_root.clone();
        let container_id = id.to_string();
        let counted = tokio::task::spawn_blocking(move || -> io::Result<Option<u64>> {
            if !ConnectionsCollector::pid_belongs_to_container(&proc_root, pid, &container_id)? {
                return Ok(None);
            }
            ConnectionsCollector::collect_from(&proc_root, pid).map(Some)
        })
        .await;

        match counted {
            Ok(Ok(Some(count))) => Some(count),
            Ok(Ok(None)) => {
                self.directory.mark_stale();
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    container_id = short(id),
                    pid,
                    "the PID no longer belongs to the container (restarted?): TCP connections not counted"
                );
                None
            }
            Ok(Err(error)) => {
                if error.kind() == io::ErrorKind::NotFound {
                    self.directory.mark_stale();
                }
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    container_id = short(id),
                    pid,
                    %error,
                    "could not count TCP connections"
                );
                None
            }
            Err(error) => {
                tracing::warn!(
                    node = %self.node_id,
                    role = role.as_str(),
                    container_id = short(id),
                    %error,
                    "TCP counting task failed"
                );
                None
            }
        }
    }

    fn log_extraction(&self, role: ComponentRole, metrics: &ContainerSystemMetrics, started: Instant) {
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let mut missing = Vec::new();
        if metrics.cpu_usage_percent.is_none() {
            missing.push("cpu");
        }
        if metrics.memory_used_mb.is_none() || metrics.memory_limit_mb.is_none() {
            missing.push("memory");
        }
        if metrics.network_rx_bytes_total.is_none() {
            missing.push("network");
        }
        if metrics.tcp_established_connections.is_none() {
            missing.push("tcp");
        }

        if missing.is_empty() {
            tracing::debug!(
                node = %self.node_id,
                role = role.as_str(),
                container_id = short(&metrics.container_id),
                state = "running",
                cpu_percent = ?metrics.cpu_usage_percent,
                memory_used_mb = ?metrics.memory_used_mb,
                rx_total = ?metrics.network_rx_bytes_total,
                rx_per_sec = ?metrics.network_rx_bytes_per_sec,
                tcp = ?metrics.tcp_established_connections,
                elapsed_ms,
                "system metrics collected"
            );
        } else {
            tracing::warn!(
                node = %self.node_id,
                role = role.as_str(),
                container_id = short(&metrics.container_id),
                missing = ?missing,
                elapsed_ms,
                "system metrics partially collected: missing values left null"
            );
        }
    }

    fn forget(&self, role: ComponentRole) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let state = &mut *state;
        if let Some(previous) = state.last_container.remove(&role) {
            state.collector.forget_container(&previous);
        }
    }
}

/// Docker's sample of a container that is not running: no host CPU counter
/// and no memory usage.
fn looks_stopped(stats: &Stats) -> bool {
    stats.cpu_stats.system_cpu_usage.is_none() && stats.memory_stats.usage.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::discovery::{
        CLIENT_LABEL, CLIENT_TYPE_LABEL, NODE_INDEX_LABEL,
    };
    use crate::collectors::container::test_support::{
        spawn_fake_docker, FakeDocker, Reply, StatsBuilder, TempDir, MIB,
    };
    use serde_json::{json, Value};

    // ---- fixtures ------------------------------------------------------------------

    fn labels(index: u8, role: ComponentRole) -> HashMap<String, String> {
        let (client_type, client) = match role {
            ComponentRole::Geth => ("execution", "geth"),
            ComponentRole::Beacon => ("beacon", "lighthouse"),
            ComponentRole::Validator => ("validator", "lighthouse"),
        };
        HashMap::from([
            (NODE_INDEX_LABEL.to_string(), index.to_string()),
            (CLIENT_TYPE_LABEL.to_string(), client_type.to_string()),
            (CLIENT_LABEL.to_string(), client.to_string()),
        ])
    }

    /// Host port of a component in the fixtures: 30000 + 100*node + role.
    fn port(index: u8, role: ComponentRole) -> u16 {
        30_000 + 100 * u16::from(index) + role as u16
    }

    fn container(id: &str, index: u8, role: ComponentRole, state: &str) -> DockerContainer {
        let running = state == "running";
        DockerContainer {
            id: id.to_string(),
            name: Some(format!("{id}-name")),
            labels: labels(index, role),
            state: Some(state.to_string()),
            public_ports: if running { vec![port(index, role)] } else { vec![] },
        }
    }

    fn expectations(nodes: u8) -> HashMap<Key, Vec<u16>> {
        (1..=nodes)
            .flat_map(|index| {
                ComponentRole::ALL
                    .map(|role| ((format!("node-{index:02}"), role), vec![port(index, role)]))
            })
            .collect()
    }

    fn id(index: u8, role: ComponentRole) -> String {
        format!("{}{index}", role.as_str())
    }

    fn running_id(resolution: &Resolution) -> &str {
        match resolution {
            Resolution::Running(target) => &target.container_id,
            other => panic!("expected a running container, got {other:?}"),
        }
    }

    // ---- 1, 2: node-to-container and role mapping ---------------------------------

    #[test]
    fn maps_fifteen_containers_to_their_node_and_role() {
        let containers: Vec<_> = (1..=5)
            .flat_map(|index| {
                ComponentRole::ALL.map(|role| container(&id(index, role), index, role, "running"))
            })
            .collect();

        let resolved = resolve(&containers, &expectations(5));

        assert_eq!(resolved.len(), 15);
        for index in 1..=5 {
            for role in ComponentRole::ALL {
                let key = (format!("node-{index:02}"), role);
                assert_eq!(running_id(&resolved[&key]), id(index, role), "{key:?}");
            }
        }
    }

    #[test]
    fn a_stopped_enclave_with_the_same_labels_is_ignored() {
        let containers = vec![
            container("old-geth1", 1, ComponentRole::Geth, "exited"),
            container("geth1", 1, ComponentRole::Geth, "running"),
        ];

        let resolved = resolve(&containers, &expectations(1));

        assert_eq!(
            running_id(&resolved[&("node-01".to_string(), ComponentRole::Geth)]),
            "geth1"
        );
    }

    #[test]
    fn a_running_container_of_another_enclave_is_rejected_by_its_ports() {
        let mut other_enclave = container("other-geth1", 1, ComponentRole::Geth, "running");
        other_enclave.public_ports = vec![45_000];
        let containers = vec![other_enclave, container("geth1", 1, ComponentRole::Geth, "running")];

        let resolved = resolve(&containers, &expectations(1));
        assert_eq!(
            running_id(&resolved[&("node-01".to_string(), ComponentRole::Geth)]),
            "geth1"
        );

        // Alone, it is reported missing rather than used.
        let mut alone = container("other-geth1", 1, ComponentRole::Geth, "running");
        alone.public_ports = vec![45_000];
        let resolved = resolve(&[alone], &expectations(1));
        assert!(matches!(
            &resolved[&("node-01".to_string(), ComponentRole::Geth)],
            Resolution::Missing { reason } if reason.contains("another enclave")
        ));
    }

    #[test]
    fn roles_are_never_swapped_even_when_ports_cross() {
        // The beacon container publishes the port configured for geth.
        let mut beacon = container("beacon1", 1, ComponentRole::Beacon, "running");
        beacon.public_ports = vec![port(1, ComponentRole::Geth)];

        let resolved = resolve(&[beacon], &expectations(1));

        assert!(matches!(
            resolved[&("node-01".to_string(), ComponentRole::Geth)],
            Resolution::Missing { .. }
        ));
        assert!(matches!(
            resolved[&("node-01".to_string(), ComponentRole::Beacon)],
            Resolution::Missing { .. }
        ));
    }

    // ---- 9, 10: missing and stopped containers ---------------------------------------

    #[test]
    fn a_missing_or_stopped_container_only_affects_its_role() {
        let containers = vec![
            container("geth1", 1, ComponentRole::Geth, "running"),
            container("beacon1", 1, ComponentRole::Beacon, "exited"),
            // No validator container at all.
        ];

        let resolved = resolve(&containers, &expectations(1));

        assert_eq!(
            running_id(&resolved[&("node-01".to_string(), ComponentRole::Geth)]),
            "geth1"
        );
        assert_eq!(
            resolved[&("node-01".to_string(), ComponentRole::Beacon)],
            Resolution::Stopped {
                container_id: "beacon1".to_string(),
                state: "exited".to_string()
            }
        );
        assert!(matches!(
            resolved[&("node-01".to_string(), ComponentRole::Validator)],
            Resolution::Missing { .. }
        ));
    }

    #[test]
    fn expected_ports_come_from_every_configured_url() {
        let config = crate::config::NodesConfig::from_toml_str(
            r#"
[[nodes]]
id = "node-01"
[nodes.geth]
rpc_url = "http://127.0.0.1:32770"
metrics_url = "http://127.0.0.1:32773/debug/metrics/prometheus"
[nodes.beacon]
api_url = "http://127.0.0.1:32795"
metrics_url = "http://127.0.0.1:32796/metrics"
[nodes.validator]
metrics_url = "http://127.0.0.1:32810/metrics"
"#,
        )
        .unwrap();

        let expected = expected_ports(&config.nodes);

        let key = |role| ("node-01".to_string(), role);
        assert_eq!(expected[&key(ComponentRole::Geth)], vec![32770, 32773]);
        assert_eq!(expected[&key(ComponentRole::Beacon)], vec![32795, 32796]);
        assert_eq!(expected[&key(ComponentRole::Validator)], vec![32810]);
    }

    // ---- fake Docker daemon serving one node --------------------------------------

    const PIDS: [u64; 3] = [4101, 4102, 4103];

    fn list_entry(container: &DockerContainer) -> Value {
        json!({
            "Id": container.id,
            "Names": [format!("/{}", container.id)],
            "State": container.state,
            "Labels": container.labels,
            "Ports": container.public_ports.iter()
                .map(|port| json!({"PrivatePort": 8545, "PublicPort": port, "Type": "tcp"}))
                .collect::<Vec<_>>(),
        })
    }

    /// Distinct memory per role, to prove each observation gets its own container.
    fn stats_of(role: ComponentRole, rx: u64) -> Reply {
        let memory = match role {
            ComponentRole::Geth => 110,
            ComponentRole::Beacon => 320,
            ComponentRole::Validator => 30,
        };
        Reply::Json(
            StatsBuilder {
                memory_usage: Some(memory * MIB),
                memory_stat: crate::collectors::container::test_support::MemoryStat::V2 {
                    inactive_file: 0,
                },
                networks: Some(vec![("eth0", rx, 2 * rx)]),
                ..StatsBuilder::default()
            }
            .json()
            .to_string(),
        )
    }

    /// Routes for the three containers of node-01; `override_stats` replaces
    /// the stats reply of one role.
    fn node_routes(
        containers: &[DockerContainer],
        override_stats: Option<(ComponentRole, Reply)>,
    ) -> Reply {
        let list = Value::Array(containers.iter().map(list_entry).collect()).to_string();
        let mut routes = vec![("/containers/json".to_string(), Reply::Json(list))];

        for (role, pid) in ComponentRole::ALL.into_iter().zip(PIDS) {
            let id = id(1, role);
            routes.push((
                format!("/containers/{id}/json"),
                Reply::Json(json!({"Id": id, "State": {"Pid": pid}}).to_string()),
            ));
            let stats = match &override_stats {
                Some((failing, reply)) if *failing == role => reply.clone(),
                _ => stats_of(role, 1_000),
            };
            routes.push((format!("/containers/{id}/stats"), stats));
        }

        Reply::Routed(routes)
    }

    fn healthy_containers() -> Vec<DockerContainer> {
        ComponentRole::ALL
            .map(|role| container(&id(1, role), 1, role, "running"))
            .to_vec()
    }

    /// `<root>/<pid>/cgroup` naming the container and `<root>/<pid>/net/tcp`
    /// with `established` ESTABLISHED sockets (plus one LISTEN).
    fn fake_proc() -> TempDir {
        let root = TempDir::new("system-proc");
        let header = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        for ((role, pid), established) in ComponentRole::ALL.into_iter().zip(PIDS).zip([5, 7, 3]) {
            let dir = root.path().join(pid.to_string());
            std::fs::create_dir_all(dir.join("net")).unwrap();
            std::fs::write(
                dir.join("cgroup"),
                format!("0::/system.slice/docker-{}.scope\n", id(1, role)),
            )
            .unwrap();
            let mut table = header.to_string();
            table.push_str("   0: 00000000:1F90 00000000:0000 0A 0 0 0 0 0 0 0\n");
            for row in 0..established {
                table.push_str(&format!("   {row}: 0100007F:1F90 0100007F:E2A4 01 0 0 0 0 0 0 0\n"));
            }
            std::fs::write(dir.join("net").join("tcp"), table).unwrap();
        }
        root
    }

    struct Fixture {
        fake: FakeDocker,
        proc_root: TempDir,
        collector: NodeSystemCollector,
    }

    fn fixture(reply: Reply, stats_timeout: Duration) -> Fixture {
        let fake = spawn_fake_docker(reply);
        let proc_root = fake_proc();
        let docker =
            DockerClient::from_docker(fake.docker.clone()).with_stats_timeout(stats_timeout);
        let directory = ContainerDirectory {
            docker,
            expected: expectations(1),
            refresh_timeout: Duration::from_secs(2),
            refresh_every: REFRESH_EVERY,
            min_refresh_gap: Duration::ZERO,
            stale: AtomicBool::new(false),
            state: tokio::sync::Mutex::new(DirectoryState::default()),
        };
        let collector = NodeSystemCollector::new("node-01", Arc::new(directory))
            .with_proc_root(proc_root.path());

        Fixture {
            fake,
            proc_root,
            collector,
        }
    }

    fn assert_all_present(system: &SystemMetrics) {
        assert!(system.cpu_usage_percent.is_some(), "{system:?}");
        assert!(system.memory_used_mb.is_some(), "{system:?}");
        assert!(system.memory_total_mb.is_some(), "{system:?}");
        assert!(system.network.network_rx_bytes_total.is_some(), "{system:?}");
        assert!(system.network.network_tx_bytes_total.is_some(), "{system:?}");
        assert!(system.network.tcp_established_connections.is_some(), "{system:?}");
    }

    // ---- 3-6, 8, 14: retrieval, extraction, TCP, per-component assignment ---------

    #[tokio::test]
    async fn every_component_gets_the_metrics_of_its_own_container() {
        let f = fixture(node_routes(&healthy_containers(), None), Duration::from_secs(2));

        let systems = f.collector.collect().await;

        for system in [&systems.geth, &systems.beacon, &systems.validator] {
            assert_all_present(system);
            assert_eq!(system.cpu_usage_percent, Some(100.0));
            assert_eq!(system.memory_total_mb, Some(1024));
            assert_eq!(system.network.network_rx_bytes_total, Some(1_000));
            assert_eq!(system.network.network_tx_bytes_total, Some(2_000));
            // A rate needs two samples.
            assert_eq!(system.network.network_rx_bytes_per_sec, None);
        }
        assert_eq!(systems.geth.memory_used_mb, Some(110));
        assert_eq!(systems.beacon.memory_used_mb, Some(320));
        assert_eq!(systems.validator.memory_used_mb, Some(30));
        assert_eq!(systems.geth.network.tcp_established_connections, Some(5));
        assert_eq!(systems.beacon.network.tcp_established_connections, Some(7));
        assert_eq!(systems.validator.network.tcp_established_connections, Some(3));

        // Each stats request went to the right container.
        let requests = f.fake.requests();
        for role in ComponentRole::ALL {
            let path = format!("/containers/{}/stats", id(1, role));
            assert_eq!(requests.iter().filter(|r| r.contains(&path)).count(), 1, "{requests:?}");
        }
    }

    // ---- 7: network rates across two cycles ----------------------------------------

    #[tokio::test]
    async fn network_rates_appear_on_the_second_cycle_and_discovery_is_reused() {
        let f = fixture(node_routes(&healthy_containers(), None), Duration::from_secs(2));

        let first = f.collector.collect().await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        let second = f.collector.collect().await;

        assert_eq!(first.geth.network.network_rx_bytes_per_sec, None);
        // The fake daemon repeats the same counters: the traffic is zero,
        // a measured zero (Some), not a missing value.
        assert_eq!(second.geth.network.network_rx_bytes_per_sec, Some(0.0));
        assert_eq!(second.validator.network.network_tx_bytes_per_sec, Some(0.0));

        // One listing for two cycles: the Docker client and mapping are reused.
        let lists = f.fake.requests().iter().filter(|r| r.contains("/containers/json")).count();
        assert_eq!(lists, 1);
    }

    // ---- 11, 13: Docker API error on one container ------------------------------------

    #[tokio::test]
    async fn a_docker_error_on_one_container_leaves_the_others_untouched() {
        let f = fixture(
            node_routes(
                &healthy_containers(),
                Some((ComponentRole::Beacon, Reply::Status(500, "daemon error"))),
            ),
            Duration::from_secs(2),
        );

        let systems = f.collector.collect().await;

        assert_eq!(systems.beacon, SystemMetrics::default());
        assert_all_present(&systems.geth);
        assert_all_present(&systems.validator);

        // The failure asks for a re-discovery on the next cycle.
        f.collector.collect().await;
        let lists = f.fake.requests().iter().filter(|r| r.contains("/containers/json")).count();
        assert_eq!(lists, 2);
    }

    // ---- 12: timeout ----------------------------------------------------------------

    #[tokio::test]
    async fn a_hanging_stats_request_times_out_without_blocking_the_others() {
        let f = fixture(
            node_routes(&healthy_containers(), Some((ComponentRole::Geth, Reply::Hang))),
            Duration::from_millis(300),
        );

        let started = std::time::Instant::now();
        let systems = f.collector.collect().await;

        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(systems.geth, SystemMetrics::default());
        assert_all_present(&systems.beacon);
        assert_all_present(&systems.validator);
    }

    // ---- 9, 10: missing and stopped containers, end to end ------------------------------

    #[tokio::test]
    async fn stopped_and_missing_containers_give_null_groups_only_for_their_role() {
        let containers = vec![
            container(&id(1, ComponentRole::Geth), 1, ComponentRole::Geth, "running"),
            container(&id(1, ComponentRole::Beacon), 1, ComponentRole::Beacon, "exited"),
        ];
        let f = fixture(node_routes(&containers, None), Duration::from_secs(2));

        let systems = f.collector.collect().await;

        assert_all_present(&systems.geth);
        assert_eq!(systems.beacon, SystemMetrics::default());
        assert_eq!(systems.validator, SystemMetrics::default());
        // No stats request is made for a container known to be stopped.
        assert!(!f
            .fake
            .requests()
            .iter()
            .any(|r| r.contains(&format!("/containers/{}/stats", id(1, ComponentRole::Beacon)))));
    }

    #[tokio::test]
    async fn a_container_stopped_since_discovery_gives_nulls_not_zeros() {
        let empty_sample = Reply::Json(
            json!({
                "read": "0001-01-01T00:00:00Z", "preread": "0001-01-01T00:00:00Z",
                "num_procs": 0, "pids_stats": {}, "blkio_stats": {}, "storage_stats": {},
                "cpu_stats": {"cpu_usage": {"total_usage": 0, "usage_in_kernelmode": 0, "usage_in_usermode": 0},
                              "throttling_data": {"periods": 0, "throttled_periods": 0, "throttled_time": 0}},
                "precpu_stats": {"cpu_usage": {"total_usage": 0, "usage_in_kernelmode": 0, "usage_in_usermode": 0},
                                 "throttling_data": {"periods": 0, "throttled_periods": 0, "throttled_time": 0}},
                "memory_stats": {}, "name": "/x", "id": "x"
            })
            .to_string(),
        );
        let f = fixture(
            node_routes(&healthy_containers(), Some((ComponentRole::Validator, empty_sample))),
            Duration::from_secs(2),
        );

        let systems = f.collector.collect().await;

        assert_eq!(systems.validator.cpu_usage_percent, None);
        assert_eq!(systems.validator.memory_used_mb, None);
        assert_eq!(systems.validator.network.network_rx_bytes_total, None);
        assert_all_present(&systems.geth);
    }

    // ---- 8: TCP safety -----------------------------------------------------------------

    #[tokio::test]
    async fn a_pid_reused_by_another_process_is_not_counted() {
        let f = fixture(node_routes(&healthy_containers(), None), Duration::from_secs(2));
        // The geth PID now belongs to another cgroup.
        std::fs::write(
            f.proc_root.path().join(PIDS[0].to_string()).join("cgroup"),
            "0::/user.slice/session-1.scope\n",
        )
        .unwrap();

        let systems = f.collector.collect().await;

        assert_eq!(systems.geth.network.tcp_established_connections, None);
        // Everything else of the container is still measured.
        assert_eq!(systems.geth.memory_used_mb, Some(110));
        assert_eq!(systems.beacon.network.tcp_established_connections, Some(7));
    }

    #[tokio::test]
    async fn docker_unreachable_gives_null_groups_everywhere() {
        let f = fixture(Reply::Status(500, "daemon down"), Duration::from_secs(2));

        let systems = f.collector.collect().await;

        assert_eq!(systems, ComponentSystems::default());
    }

    #[test]
    fn timeouts_fit_the_collector_budget() {
        let budget = Duration::from_secs(4);

        assert_eq!(stats_timeout_for(budget), Duration::from_millis(2800));
        assert_eq!(refresh_timeout_for(budget), Duration::from_millis(800));
        assert!(stats_timeout_for(budget) + refresh_timeout_for(budget) < budget);
    }

    #[test]
    fn short_ids_are_twelve_characters() {
        assert_eq!(short("57a01b6f3a499c43d4d2"), "57a01b6f3a49");
        assert_eq!(short("abc"), "abc");
    }
}
