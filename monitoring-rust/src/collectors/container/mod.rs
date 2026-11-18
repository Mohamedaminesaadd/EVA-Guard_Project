pub mod connections;
pub mod cpu;
pub mod discovery;
pub mod docker_client;
pub mod memory;
pub mod network;

pub use connections::ConnectionsCollector;
pub use cpu::CpuCollector;
pub use discovery::{classify, ComponentRole, DiscoveredContainer};
pub use docker_client::{DockerClient, DockerClientError, DockerContainer};
pub use memory::MemoryCollector;
pub use network::NetworkCollector;

/// Helpers shared by the tests of the container collectors.
///
/// Needs `serde_json` and, for tokio, the `macros`, `net`, `io-util`,
/// `rt` and `time` features (all enabled by tokio's `full`).
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};

    use bollard::container::Stats;
    use bollard::{Docker, API_DEFAULT_VERSION};
    use serde_json::{json, Map, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::task::JoinHandle;

    pub(crate) const MIB: u64 = 1_048_576;

    // ---- temporary files -------------------------------------------------

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A path that no other test (or process) uses.
    pub(crate) fn unique_temp_path(label: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!("eva-guard-{label}-{}-{n}", std::process::id()))
    }

    /// A temporary directory removed on drop.
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn new(label: &str) -> Self {
            let path = unique_temp_path(label);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // ---- Docker stats fixtures --------------------------------------------

    /// `memory_stats.stats` as exposed by Docker.
    #[derive(Clone, Copy)]
    pub(crate) enum MemoryStat {
        /// No `stats` object (stopped container, old daemon).
        Missing,
        /// cgroup v1.
        V1 {
            total_inactive_file: u64,
            inactive_file: u64,
        },
        /// cgroup v2.
        V2 { inactive_file: u64 },
    }

    const V1_KEYS: &[&str] = &[
        "cache",
        "dirty",
        "mapped_file",
        "total_inactive_file",
        "pgpgout",
        "rss",
        "total_mapped_file",
        "writeback",
        "unevictable",
        "pgpgin",
        "total_unevictable",
        "pgmajfault",
        "total_rss",
        "total_rss_huge",
        "total_writeback",
        "total_inactive_anon",
        "rss_huge",
        "hierarchical_memory_limit",
        "total_pgfault",
        "total_active_file",
        "active_anon",
        "total_active_anon",
        "total_pgpgout",
        "total_cache",
        "total_dirty",
        "inactive_anon",
        "active_file",
        "pgfault",
        "inactive_file",
        "total_pgmajfault",
        "total_pgpgin",
    ];

    const V2_KEYS: &[&str] = &[
        "anon",
        "file",
        "kernel_stack",
        "slab",
        "sock",
        "shmem",
        "file_mapped",
        "file_dirty",
        "file_writeback",
        "anon_thp",
        "inactive_anon",
        "active_anon",
        "inactive_file",
        "active_file",
        "unevictable",
        "slab_reclaimable",
        "slab_unreclaimable",
        "pgfault",
        "pgmajfault",
        "workingset_refault",
        "workingset_activate",
        "workingset_nodereclaim",
        "pgrefill",
        "pgscan",
        "pgsteal",
        "pgactivate",
        "pgdeactivate",
        "pglazyfree",
        "pglazyfreed",
        "thp_fault_alloc",
        "thp_collapse_alloc",
    ];

    fn zeroed(keys: &[&str]) -> Map<String, Value> {
        keys.iter()
            .map(|key| ((*key).to_string(), json!(0)))
            .collect()
    }

    /// Builds one Docker `stats` sample.
    ///
    /// The defaults describe a healthy 2-CPU container using 100% of its
    /// CPU share and 200 MiB of a 1024 MiB limit (cgroup v2).
    #[derive(Clone)]
    pub(crate) struct StatsBuilder {
        pub(crate) cpu_total: u64,
        pub(crate) precpu_total: u64,
        pub(crate) system: Option<u64>,
        pub(crate) presystem: Option<u64>,
        pub(crate) online_cpus: Option<u64>,
        pub(crate) percpu: Option<Vec<u64>>,
        pub(crate) memory_usage: Option<u64>,
        pub(crate) memory_limit: Option<u64>,
        pub(crate) memory_stat: MemoryStat,
        /// `(interface, rx_bytes, tx_bytes)`; None omits the `networks` key.
        pub(crate) networks: Option<Vec<(&'static str, u64, u64)>>,
    }

    impl Default for StatsBuilder {
        fn default() -> Self {
            Self {
                cpu_total: 1_050,
                precpu_total: 1_000,
                system: Some(10_100),
                presystem: Some(10_000),
                online_cpus: Some(2),
                percpu: None,
                memory_usage: Some(300 * MIB),
                memory_limit: Some(1024 * MIB),
                memory_stat: MemoryStat::V2 {
                    inactive_file: 100 * MIB,
                },
                networks: Some(vec![("eth0", 1_000, 2_000)]),
            }
        }
    }

    fn cpu_stats(
        total: u64,
        system: Option<u64>,
        online_cpus: Option<u64>,
        percpu: Option<&Vec<u64>>,
    ) -> Value {
        let mut usage = json!({
            "usage_in_usermode": 0,
            "total_usage": total,
            "usage_in_kernelmode": 0,
        });

        if let Some(percpu) = percpu {
            usage["percpu_usage"] = json!(percpu);
        }

        let mut stats = json!({
            "cpu_usage": usage,
            "throttling_data": {
                "periods": 0,
                "throttled_periods": 0,
                "throttled_time": 0,
            },
        });

        if let Some(system) = system {
            stats["system_cpu_usage"] = json!(system);
        }

        if let Some(online_cpus) = online_cpus {
            stats["online_cpus"] = json!(online_cpus);
        }

        stats
    }

    impl StatsBuilder {
        fn memory_json(&self) -> Value {
            let mut memory = Map::new();

            if let Some(usage) = self.memory_usage {
                memory.insert("usage".into(), json!(usage));
            }

            if let Some(limit) = self.memory_limit {
                memory.insert("limit".into(), json!(limit));
            }

            match self.memory_stat {
                MemoryStat::Missing => {}
                MemoryStat::V1 {
                    total_inactive_file,
                    inactive_file,
                } => {
                    let mut stat = zeroed(V1_KEYS);
                    stat.insert("total_inactive_file".into(), json!(total_inactive_file));
                    stat.insert("inactive_file".into(), json!(inactive_file));
                    memory.insert("stats".into(), Value::Object(stat));
                }
                MemoryStat::V2 { inactive_file } => {
                    let mut stat = zeroed(V2_KEYS);
                    stat.insert("inactive_file".into(), json!(inactive_file));
                    memory.insert("stats".into(), Value::Object(stat));
                }
            }

            Value::Object(memory)
        }

        /// The sample as Docker sends it.
        pub(crate) fn json(&self) -> Value {
            let mut stats = json!({
                "read": "2026-11-17T18:00:01.000000000Z",
                "preread": "2026-11-17T18:00:00.000000000Z",
                "num_procs": 0,
                "pids_stats": { "current": 3 },
                "blkio_stats": {},
                "storage_stats": {},
                "cpu_stats": cpu_stats(
                    self.cpu_total,
                    self.system,
                    self.online_cpus,
                    self.percpu.as_ref(),
                ),
                "precpu_stats": cpu_stats(
                    self.precpu_total,
                    self.presystem,
                    self.online_cpus,
                    None,
                ),
                "memory_stats": self.memory_json(),
                "name": "/eva-test",
                "id": "abc123",
            });

            if let Some(networks) = &self.networks {
                let interfaces: Map<String, Value> = networks
                    .iter()
                    .map(|(name, rx_bytes, tx_bytes)| {
                        (
                            (*name).to_string(),
                            json!({
                                "rx_dropped": 0,
                                "rx_bytes": rx_bytes,
                                "rx_errors": 0,
                                "tx_packets": 0,
                                "tx_dropped": 0,
                                "rx_packets": 0,
                                "tx_errors": 0,
                                "tx_bytes": tx_bytes,
                            }),
                        )
                    })
                    .collect();

                stats["networks"] = Value::Object(interfaces);
            }

            stats
        }

        /// The sample parsed by bollard.
        pub(crate) fn build(&self) -> Stats {
            serde_json::from_value(self.json()).expect("fixture must deserialize into Stats")
        }
    }

    // ---- fake Docker daemon ------------------------------------------------

    /// What the fake daemon answers to every request.
    #[derive(Clone)]
    pub(crate) enum Reply {
        /// 200 with this JSON body.
        Json(String),
        /// An error status with a Docker-style `{"message": ...}` body.
        Status(u16, &'static str),
        /// 200 with no body at all.
        Empty,
        /// Accepts the request and never answers.
        Hang,
        /// The reply of the first route whose key is a substring of the
        /// request line (e.g. `/containers/geth1/stats`); 404 otherwise.
        Routed(Vec<(String, Reply)>),
    }

    impl Reply {
        fn for_request(&self, request_line: &str) -> Reply {
            match self {
                Reply::Routed(routes) => routes
                    .iter()
                    .find(|(key, _)| request_line.contains(key.as_str()))
                    .map(|(_, reply)| reply.for_request(request_line))
                    .unwrap_or(Reply::Status(404, "No such container")),
                other => other.clone(),
            }
        }
    }

    /// A Docker daemon listening on a temporary unix socket.
    pub(crate) struct FakeDocker {
        pub(crate) docker: Docker,
        requests: Arc<Mutex<Vec<String>>>,
        socket: PathBuf,
        server: JoinHandle<()>,
    }

    impl FakeDocker {
        /// Request lines received so far, e.g. `GET /v1.47/containers/x/stats?... HTTP/1.1`.
        pub(crate) fn requests(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl Drop for FakeDocker {
        fn drop(&mut self) {
            self.server.abort();
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    /// Starts a fake daemon. Must be called inside a tokio runtime.
    pub(crate) fn spawn_fake_docker(reply: Reply) -> FakeDocker {
        let socket = unique_temp_path("docker").with_extension("sock");
        let listener = UnixListener::bind(&socket).expect("bind fake docker socket");
        let requests = Arc::new(Mutex::new(Vec::new()));

        let server = tokio::spawn(serve(listener, reply, Arc::clone(&requests)));

        let docker = Docker::connect_with_unix(
            socket.to_str().expect("utf-8 socket path"),
            5,
            API_DEFAULT_VERSION,
        )
        .expect("connect to fake docker");

        FakeDocker {
            docker,
            requests,
            socket,
            server,
        }
    }

    async fn serve(listener: UnixListener, reply: Reply, requests: Arc<Mutex<Vec<String>>>) {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };

            let reply = reply.clone();
            let requests = Arc::clone(&requests);

            tokio::spawn(async move {
                let Some(request_line) = read_request_line(&mut socket).await else {
                    return;
                };

                let reply = reply.for_request(&request_line);

                requests
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(request_line);

                match reply {
                    Reply::Routed(_) => unreachable!("routes are resolved above"),
                    Reply::Hang => std::future::pending::<()>().await,
                    Reply::Json(body) => respond(&mut socket, 200, "OK", &body).await,
                    Reply::Empty => respond(&mut socket, 200, "OK", "").await,
                    Reply::Status(code, message) => {
                        let body = json!({ "message": message }).to_string();
                        respond(&mut socket, code, "Error", &body).await;
                    }
                }
            });
        }
    }

    /// Reads the request up to the blank line and returns its first line.
    async fn read_request_line(socket: &mut UnixStream) -> Option<String> {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];

        while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = socket.read(&mut chunk).await.ok()?;

            if read == 0 || buffer.len() > 16 * 1024 {
                return None;
            }

            buffer.extend_from_slice(&chunk[..read]);
        }

        String::from_utf8_lossy(&buffer)
            .lines()
            .next()
            .map(str::to_string)
    }

    async fn respond(socket: &mut UnixStream, status: u16, reason: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );

        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    }
}
