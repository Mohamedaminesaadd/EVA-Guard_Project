use std::collections::HashMap;
use std::time::Duration;

use bollard::container::{InspectContainerOptions, ListContainersOptions, Stats, StatsOptions};
use bollard::errors::Error as BollardError;
use bollard::Docker;
use futures_util::TryStreamExt;
use thiserror::Error;

use super::discovery::{self, DiscoveredContainer, DiscoveryError};

/// How long one statistics request may take.
///
/// A sample normally takes one to two seconds: Docker measures CPU over a
/// one second window. bollard's own limit is 120 seconds, far too long for
/// a monitoring cycle.
pub const DEFAULT_STATS_TIMEOUT: Duration = Duration::from_secs(10);

/// Metadata returned by Docker's container list endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DockerContainer {
    pub id: String,
    pub name: Option<String>,
    pub labels: HashMap<String, String>,
    /// Docker state (`running`, `exited`, `created`, ...), when reported.
    pub state: Option<String>,
    /// Host ports the container publishes (empty for a stopped container).
    pub public_ports: Vec<u16>,
}

impl DockerContainer {
    /// Docker reports the container as running.
    pub fn is_running(&self) -> bool {
        self.state.as_deref() == Some("running")
    }
}

/// Metadata that requires Docker's per-container inspection endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInspection {
    pub id: String,
    /// `None` means that Docker reported PID 0 or no PID (the container is
    /// not running), so `/proc/<pid>` must not be read.
    pub host_pid: Option<u64>,
}

#[derive(Debug, Error)]
pub enum DockerClientError {
    #[error("Docker error: {0}")]
    Connection(#[from] BollardError),

    #[error("Docker returned a container without an ID while listing containers")]
    MissingListedContainerId,

    #[error("Docker inspection of container `{container_id}` returned no ID")]
    MissingInspectedContainerId { container_id: String },

    #[error("could not inspect Docker container `{container_id}`: {source}")]
    Inspect {
        container_id: String,
        #[source]
        source: BollardError,
    },

    #[error("Docker container discovery failed: {0}")]
    Discovery(#[from] DiscoveryError),

    #[error("No statistics received for container: {0}")]
    NoStats(String),

    #[error("Timed out after {timeout:?} waiting for statistics of container: {container_id}")]
    Timeout {
        container_id: String,
        timeout: Duration,
    },
}

#[derive(Clone)]
pub struct DockerClient {
    docker: Docker,
    stats_timeout: Duration,
}

impl DockerClient {
    /// Connects to the local Docker Engine.
    pub fn new() -> Result<Self, DockerClientError> {
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self::from_docker(docker))
    }

    /// Wraps an already configured Docker connection.
    pub fn from_docker(docker: Docker) -> Self {
        Self {
            docker,
            stats_timeout: DEFAULT_STATS_TIMEOUT,
        }
    }

    /// Sets how long `get_stats` waits for Docker.
    pub fn with_stats_timeout(mut self, timeout: Duration) -> Self {
        self.stats_timeout = timeout;
        self
    }

    /// Lists running Docker containers. Container IDs and labels are kept so
    /// callers can safely discover services without relying on names.
    pub async fn list_containers(&self) -> Result<Vec<DockerContainer>, DockerClientError> {
        self.list_containers_with_filters(HashMap::new(), false).await
    }

    /// Lists every Kurtosis user-service container, stopped ones included,
    /// with its state and published host ports. One API call.
    pub async fn list_kurtosis_services_all(
        &self,
    ) -> Result<Vec<DockerContainer>, DockerClientError> {
        self.list_containers_with_filters(kurtosis_service_filters(), true)
            .await
    }

    /// The stats timeout in use.
    pub fn stats_timeout(&self) -> Duration {
        self.stats_timeout
    }

    /// Inspects a container to retrieve its stable ID and current host PID.
    pub async fn inspect_container(
        &self,
        container_id: &str,
    ) -> Result<ContainerInspection, DockerClientError> {
        let inspected = self
            .docker
            .inspect_container(container_id, Some(InspectContainerOptions { size: false }))
            .await
            .map_err(|source| DockerClientError::Inspect {
                container_id: container_id.to_string(),
                source,
            })?;

        let id = inspected
            .id
            .ok_or_else(|| DockerClientError::MissingInspectedContainerId {
                container_id: container_id.to_string(),
            })?;
        let host_pid = inspected
            .state
            .and_then(|state| state.pid)
            .and_then(|pid| u64::try_from(pid).ok())
            .filter(|pid| *pid != 0);

        Ok(ContainerInspection { id, host_pid })
    }

    /// Discovers all running Kurtosis Ethereum services and inspects each one
    /// concurrently for its host PID. The label mapping is validated before
    /// any result is returned, so an ambiguous component can never be used.
    pub async fn discover_kurtosis_node_containers(
        &self,
    ) -> Result<Vec<DiscoveredContainer>, DockerClientError> {
        let services = self.list_kurtosis_user_services().await?;
        let containers = discovery::discover(services)?;

        futures_util::future::try_join_all(containers.into_iter().map(|container| async move {
            let inspection = self.inspect_container(&container.container_id).await?;
            Ok(DiscoveredContainer {
                container_id: inspection.id,
                host_pid: inspection.host_pid,
                ..container
            })
        }))
        .await
    }

    /// Retrieves one statistics sample for a container.
    ///
    /// `one_shot` stays off on purpose: Docker then also fills
    /// `precpu_stats`, which the CPU percentage needs.
    pub async fn get_stats(&self, container_id: &str) -> Result<Stats, DockerClientError> {
        let request = async {
            let mut stream = self.docker.stats(
                container_id,
                Some(StatsOptions {
                    stream: false,
                    one_shot: false,
                }),
            );

            stream
                .try_next()
                .await?
                .ok_or_else(|| DockerClientError::NoStats(container_id.to_string()))
        };

        tokio::time::timeout(self.stats_timeout, request)
            .await
            .map_err(|_| DockerClientError::Timeout {
                container_id: container_id.to_string(),
                timeout: self.stats_timeout,
            })?
    }

    /// Returns the Docker client.
    pub fn docker(&self) -> &Docker {
        &self.docker
    }

    async fn list_kurtosis_user_services(&self) -> Result<Vec<DockerContainer>, DockerClientError> {
        self.list_containers_with_filters(kurtosis_service_filters(), false)
            .await
    }

    async fn list_containers_with_filters(
        &self,
        filters: HashMap<String, Vec<String>>,
        all: bool,
    ) -> Result<Vec<DockerContainer>, DockerClientError> {
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions {
                all,
                filters,
                ..Default::default()
            }))
            .await?;

        containers
            .into_iter()
            .map(|container| {
                let id = container
                    .id
                    .ok_or(DockerClientError::MissingListedContainerId)?;
                let mut public_ports: Vec<u16> = container
                    .ports
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|port| port.public_port)
                    .collect();
                public_ports.sort_unstable();
                public_ports.dedup();

                Ok(DockerContainer {
                    id,
                    name: container
                        .names
                        .and_then(|mut names| names.drain(..).next())
                        .map(|name| name.trim_start_matches('/').to_string()),
                    labels: container.labels.unwrap_or_default(),
                    state: container.state,
                    public_ports,
                })
            })
            .collect()
    }
}

fn kurtosis_service_filters() -> HashMap<String, Vec<String>> {
    HashMap::from([(
        "label".to_string(),
        vec![
            "com.kurtosistech.app-id=kurtosis".to_string(),
            "com.kurtosistech.container-type=user-service".to_string(),
        ],
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::{spawn_fake_docker, Reply, StatsBuilder};

    fn stats_reply() -> Reply {
        Reply::Json(StatsBuilder::default().json().to_string())
    }

    #[tokio::test]
    async fn returns_the_stats_sent_by_docker() {
        let fake = spawn_fake_docker(stats_reply());
        let client = DockerClient::from_docker(fake.docker.clone());

        let stats = client.get_stats("abc123").await.expect("stats");

        assert_eq!(stats.cpu_stats.cpu_usage.total_usage, 1_050);
        assert_eq!(stats.precpu_stats.cpu_usage.total_usage, 1_000);
        assert_eq!(stats.id, "abc123");
    }

    #[tokio::test]
    async fn lists_container_ids_names_and_labels() {
        let fake = spawn_fake_docker(Reply::Json(
            r#"[{"Id":"abc123","Names":["/el-1-geth-lighthouse"],"Labels":{"key":"value"}}]"#
                .to_string(),
        ));
        let client = DockerClient::from_docker(fake.docker.clone());

        let containers = client.list_containers().await.expect("containers");

        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].id, "abc123");
        assert_eq!(containers[0].name.as_deref(), Some("el-1-geth-lighthouse"));
        assert_eq!(
            containers[0].labels.get("key").map(String::as_str),
            Some("value")
        );
        assert!(fake.requests()[0].contains("/containers/json"));
    }

    #[tokio::test]
    async fn listing_all_services_keeps_state_and_published_ports() {
        let fake = spawn_fake_docker(Reply::Json(
            r#"[{"Id":"geth1","Names":["/el-1"],"State":"running","Labels":{},
                 "Ports":[{"PrivatePort":8545,"PublicPort":32770,"Type":"tcp"},
                          {"PrivatePort":8545,"PublicPort":32770,"Type":"tcp","IP":"::"},
                          {"PrivatePort":30303,"Type":"udp"}]},
                {"Id":"old","Names":["/el-1-old"],"State":"exited","Labels":{},"Ports":[]}]"#
                .to_string(),
        ));
        let client = DockerClient::from_docker(fake.docker.clone());

        let containers = client.list_kurtosis_services_all().await.expect("list");

        assert_eq!(containers[0].public_ports, vec![32770]);
        assert!(containers[0].is_running());
        assert_eq!(containers[1].state.as_deref(), Some("exited"));
        assert!(!containers[1].is_running());
        assert!(containers[1].public_ports.is_empty());

        let request = &fake.requests()[0];
        assert!(request.contains("all=true"), "{request}");
        assert!(request.contains("user-service"), "{request}");
    }

    #[tokio::test]
    async fn inspection_returns_id_and_running_host_pid() {
        let fake = spawn_fake_docker(Reply::Json(
            r#"{"Id":"abc123","State":{"Pid":4242}}"#.to_string(),
        ));
        let client = DockerClient::from_docker(fake.docker.clone());

        let inspection = client
            .inspect_container("abc123")
            .await
            .expect("inspection");

        assert_eq!(inspection.id, "abc123");
        assert_eq!(inspection.host_pid, Some(4242));
        assert!(fake.requests()[0].contains("/containers/abc123/json"));
    }

    #[tokio::test]
    async fn inspection_treats_pid_zero_as_unavailable() {
        let fake = spawn_fake_docker(Reply::Json(
            r#"{"Id":"stopped","State":{"Pid":0}}"#.to_string(),
        ));
        let client = DockerClient::from_docker(fake.docker.clone());

        assert_eq!(
            client.inspect_container("stopped").await.unwrap().host_pid,
            None
        );
    }

    #[tokio::test]
    async fn asks_for_one_non_one_shot_sample_of_the_right_container() {
        let fake = spawn_fake_docker(stats_reply());
        let client = DockerClient::from_docker(fake.docker.clone());

        client.get_stats("my-geth").await.expect("stats");

        let requests = fake.requests();
        assert_eq!(requests.len(), 1, "{requests:?}");

        let request = &requests[0];
        assert!(request.starts_with("GET "), "{request}");
        assert!(request.contains("/containers/my-geth/stats?"), "{request}");
        assert!(request.contains("stream=false"), "{request}");
        assert!(request.contains("one-shot=false"), "{request}");
    }

    #[tokio::test]
    async fn unknown_container_is_a_docker_error() {
        let fake = spawn_fake_docker(Reply::Status(404, "No such container: nope"));
        let client = DockerClient::from_docker(fake.docker.clone());

        let error = client.get_stats("nope").await.unwrap_err();

        assert!(
            matches!(
                error,
                DockerClientError::Connection(BollardError::DockerResponseServerError {
                    status_code: 404,
                    ..
                })
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("No such container"), "{error}");
    }

    #[tokio::test]
    async fn daemon_error_is_a_docker_error() {
        let fake = spawn_fake_docker(Reply::Status(500, "boom"));
        let client = DockerClient::from_docker(fake.docker.clone());

        let error = client.get_stats("abc").await.unwrap_err();

        assert!(
            matches!(error, DockerClientError::Connection(_)),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn empty_answer_means_no_stats() {
        let fake = spawn_fake_docker(Reply::Empty);
        let client = DockerClient::from_docker(fake.docker.clone());

        let error = client.get_stats("abc").await.unwrap_err();

        assert!(
            matches!(&error, DockerClientError::NoStats(id) if id == "abc"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn malformed_stats_are_a_docker_error() {
        let fake = spawn_fake_docker(Reply::Json("{\"read\": \"not stats\"}".to_string()));
        let client = DockerClient::from_docker(fake.docker.clone());

        let error = client.get_stats("abc").await.unwrap_err();

        assert!(
            matches!(error, DockerClientError::Connection(_)),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn gives_up_when_docker_never_answers() {
        let fake = spawn_fake_docker(Reply::Hang);
        let client = DockerClient::from_docker(fake.docker.clone())
            .with_stats_timeout(Duration::from_millis(150));

        let error = client.get_stats("slow").await.unwrap_err();

        match error {
            DockerClientError::Timeout {
                container_id,
                timeout,
            } => {
                assert_eq!(container_id, "slow");
                assert_eq!(timeout, Duration::from_millis(150));
            }
            other => panic!("expected a timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn client_is_reusable_after_an_error() {
        let failing = spawn_fake_docker(Reply::Status(404, "gone"));
        let client = DockerClient::from_docker(failing.docker.clone());

        assert!(client.get_stats("abc").await.is_err());
        assert!(client.get_stats("abc").await.is_err());
        assert_eq!(failing.requests().len(), 2);
    }

    #[test]
    fn default_timeout_is_bounded() {
        assert!(DEFAULT_STATS_TIMEOUT >= Duration::from_secs(2));
        assert!(DEFAULT_STATS_TIMEOUT <= Duration::from_secs(30));
    }

    #[test]
    fn error_messages_name_the_container() {
        let no_stats = DockerClientError::NoStats("abc".to_string());
        let timeout = DockerClientError::Timeout {
            container_id: "abc".to_string(),
            timeout: Duration::from_secs(10),
        };

        assert!(no_stats.to_string().contains("abc"));
        assert!(timeout.to_string().contains("abc"));
        assert!(timeout.to_string().contains("10s"));
    }
}
