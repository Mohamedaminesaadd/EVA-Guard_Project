//! Collection scheduler: drives the monitoring cycles of every node and
//! hands the observations to the storage.
//!
//! # One cycle
//!
//! ```text
//! tick (every `collection_interval`, on a fixed grid)
//!   ├─ one task per node ─────────────► NodeSource::collect  (12 collectors, in parallel)
//!   ├─ one cross-node task ───────────► PropagationCollector (heads of ALL Beacon Nodes)
//!   │      barrier: wait for all of them, at most `cycle` (90 % of the interval)
//!   ├─ merge the propagation into each node's Beacon metrics
//!   └─ 3 observations per node ───────► ObservationSink::append (JSONL)
//! ```
//!
//! # Guarantees
//!
//! - **Fixed grid.** Ticks are `start + k * interval`, whatever the duration
//!   of a cycle (`MissedTickBehavior::Skip`): a slow cycle never shifts the
//!   following ones.
//! - **Bounded cycle.** A collector is cut after 80 % of the interval (see
//!   `NodeCollector`), the whole cycle after 90 %. A node that did not
//!   finish gets an observation where every metric is `None`: the row
//!   exists, the measurement does not.
//! - **Isolation.** Nodes run in separate Tokio tasks. A node that hangs or
//!   panics cannot delay or corrupt the others.
//! - **Shared timestamp.** All the observations of a cycle carry the same
//!   *nominal* time `anchor + k * interval` (whole seconds), not the moment
//!   each task happened to finish, so the rows of a cycle can be joined and
//!   consecutive cycles are exactly one interval apart.
//! - **No stale sample.** A tick delivered more than half an interval late
//!   (process suspended, runtime blocked) is dropped instead of measured: the
//!   data would be dated to a moment that is already gone. The dataset has a
//!   gap, not a wrong point.
//! - **Graceful stop.** When the shutdown future completes, the cycle in
//!   flight is discarded (never half written) and the storage is flushed.
//!
//! Writing a cycle (15 lines of about 1 KB) is done directly in the loop:
//! it is a handful of small `write` calls, and it keeps the cycle atomic with
//! respect to shutdown, because no `.await` separates two writes.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Timelike, Utc};
use thiserror::Error;
use tokio::task::{Id, JoinSet};
use tokio::time::{interval_at, timeout, Instant, MissedTickBehavior};

use crate::clients::beacon_client::BeaconClient;
use crate::collectors::beacon::PropagationCollector;
use crate::collectors::container::DockerClient;
use crate::config::NodesConfig;
use crate::models::metrics::BlockPropagationMetrics;
use crate::models::observation::{Observation, Scenario};
use crate::monitoring::node_collector::{
    NodeCollector, NodeCollectorError, NodeSnapshot, NodeSource,
};
use crate::monitoring::node_manager::{NodeManager, NodeManagerError};
use crate::monitoring::system_collection::{
    refresh_timeout_for, stats_timeout_for, ContainerDirectory, NodeSystemCollector,
};
use crate::storage::jsonl::{JsonlStorage, JsonlStorageError};

/// Where the observations go. Implemented by [`JsonlStorage`].
pub trait ObservationSink: Send + Sync + 'static {
    /// Appends one observation.
    fn append(&self, observation: &Observation) -> Result<(), JsonlStorageError>;

    /// Makes everything appended so far durable.
    fn flush(&self) -> Result<(), io::Error>;
}

impl ObservationSink for JsonlStorage {
    fn append(&self, observation: &Observation) -> Result<(), JsonlStorageError> {
        JsonlStorage::append(self, observation)
    }

    fn flush(&self) -> Result<(), io::Error> {
        JsonlStorage::flush(self)
    }
}

/// The time limits derived from the collection interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleBudget {
    /// Maximum duration of one collector (80 % of the interval).
    pub collector: Duration,
    /// Maximum duration of the whole collection phase (90 % of the interval).
    pub cycle: Duration,
    /// A tick delivered later than this is dropped (50 % of the interval).
    pub late_tick_limit: Duration,
}

impl CycleBudget {
    pub fn for_interval(interval: Duration) -> Self {
        Self {
            collector: fraction(interval, 4, 5),
            cycle: fraction(interval, 9, 10),
            late_tick_limit: fraction(interval, 1, 2),
        }
    }
}

/// `duration * numerator / denominator`, exact (no float rounding).
fn fraction(duration: Duration, numerator: u32, denominator: u32) -> Duration {
    duration
        .checked_mul(numerator)
        .map_or(duration, |scaled| scaled / denominator)
}

/// Errors raised while building a [`Scheduler`].
#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("there is no node to monitor")]
    NoNodes,

    #[error("duplicate node id: {0}")]
    DuplicateNode(String),

    #[error("the collection interval must be at least 1 second (got {0:?})")]
    IntervalTooShort(Duration),

    #[error(transparent)]
    Nodes(#[from] NodeManagerError),

    #[error(transparent)]
    Collector(#[from] NodeCollectorError),
}

/// What happened during a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunReport {
    /// Cycles collected and written.
    pub cycles_completed: u64,
    /// Ticks dropped because they were delivered too late.
    pub cycles_skipped: u64,
    /// Node-cycles written with every metric `None` (deadline or panic).
    pub nodes_unmeasured: u64,
    /// Observations written to the sink.
    pub observations_written: u64,
    /// Observations the sink refused.
    pub write_failures: u64,
    /// The final flush succeeded.
    pub flushed: bool,
}

/// The collection loop. See the module documentation.
pub struct Scheduler<S> {
    sources: Vec<Arc<dyn NodeSource>>,
    sink: S,
    scenario: Scenario,
    interval: Duration,
    budget: CycleBudget,
}

impl<S: ObservationSink> Scheduler<S> {
    /// Builds a scheduler over already prepared nodes.
    pub fn new(
        sources: Vec<Arc<dyn NodeSource>>,
        sink: S,
        scenario: Scenario,
        interval: Duration,
    ) -> Result<Self, SchedulerError> {
        if sources.is_empty() {
            return Err(SchedulerError::NoNodes);
        }

        let mut seen = HashSet::new();
        for source in &sources {
            if !seen.insert(source.node_id()) {
                return Err(SchedulerError::DuplicateNode(source.node_id().to_string()));
            }
        }

        // The dataset stores the interval in whole seconds.
        if interval < Duration::from_secs(1) {
            return Err(SchedulerError::IntervalTooShort(interval));
        }

        Ok(Self {
            sources,
            sink,
            scenario,
            interval,
            budget: CycleBudget::for_interval(interval),
        })
    }

    /// Builds the whole pipeline from the multi-node configuration:
    /// clients, collectors and time budgets, and the container metrics
    /// through the local Docker Engine.
    ///
    /// Docker is optional: when no client can be configured, monitoring runs
    /// with `null` system metrics (and says so).
    pub fn from_config(
        config: &NodesConfig,
        scenario: Scenario,
        sink: S,
    ) -> Result<Self, SchedulerError> {
        let docker = match DockerClient::new() {
            Ok(docker) => Some(docker),
            Err(error) => {
                tracing::error!(
                    %error,
                    "Docker client unavailable: system metrics will be null"
                );
                None
            }
        };

        Self::from_config_with_docker(config, scenario, sink, docker)
    }

    /// Same as [`Scheduler::from_config`] with an explicit Docker client
    /// (`None`: no system metrics). One client, hence one connection pool,
    /// is shared by every node.
    pub fn from_config_with_docker(
        config: &NodesConfig,
        scenario: Scenario,
        sink: S,
        docker: Option<DockerClient>,
    ) -> Result<Self, SchedulerError> {
        let interval = config.settings.collection_interval();
        let budget = CycleBudget::for_interval(interval);
        let request_timeout = config.settings.request_timeout();

        if request_timeout >= budget.collector {
            tracing::warn!(
                request_timeout_secs = request_timeout.as_secs(),
                collector_budget_ms = budget.collector.as_millis() as u64,
                "request_timeout_secs is not below 80% of collection_interval_secs: \
                 a hanging endpoint is cut by the time budget before its request times out"
            );
        }

        let manager = NodeManager::new(config)?;

        let directory = docker.map(|docker| {
            Arc::new(ContainerDirectory::new(
                docker.with_stats_timeout(stats_timeout_for(budget.collector)),
                &config.nodes,
                refresh_timeout_for(budget.collector),
            ))
        });

        let sources = manager
            .into_nodes()
            .into_iter()
            .map(|node| {
                let node_id = node.node_id.clone();
                NodeCollector::new(node, config.settings.request_timeout_secs, budget.collector)
                    .map(|collector| match &directory {
                        Some(directory) => collector
                            .with_system(NodeSystemCollector::new(node_id, Arc::clone(directory))),
                        None => collector,
                    })
                    .map(|collector| Arc::new(collector) as Arc<dyn NodeSource>)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Self::new(sources, sink, scenario, interval)
    }

    /// The node ids, in collection order.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().map(|source| source.node_id())
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn budget(&self) -> CycleBudget {
        self.budget
    }

    /// Runs until `shutdown` completes, then flushes the storage.
    ///
    /// ```ignore
    /// let shutdown = async {
    ///     // If the handler cannot be installed, never stop on our own.
    ///     if tokio::signal::ctrl_c().await.is_err() {
    ///         std::future::pending::<()>().await;
    ///     }
    /// };
    /// let report = scheduler.run(shutdown).await;
    /// ```
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> RunReport {
        self.run_from(Utc::now(), shutdown).await
    }

    /// Same as [`Scheduler::run`] with an explicit wall-clock anchor: the
    /// nominal time of the first cycle (truncated to whole seconds).
    pub(crate) async fn run_from(
        self,
        wall_anchor: DateTime<Utc>,
        shutdown: impl Future<Output = ()>,
    ) -> RunReport {
        tokio::pin!(shutdown);

        let wall_anchor = wall_anchor.with_nanosecond(0).unwrap_or(wall_anchor);
        let start = Instant::now();
        let mut report = RunReport::default();

        let mut ticker = interval_at(start, self.interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        tracing::info!(
            nodes = self.sources.len(),
            interval_secs = self.interval.as_secs(),
            scenario = %self.scenario,
            "scheduler started"
        );

        loop {
            let scheduled = tokio::select! {
                biased;
                () = &mut shutdown => break,
                scheduled = ticker.tick() => scheduled,
            };

            let lateness = Instant::now().saturating_duration_since(scheduled);
            if lateness > self.budget.late_tick_limit {
                report.cycles_skipped += 1;
                tracing::warn!(
                    late_ms = lateness.as_millis() as u64,
                    "tick delivered too late: cycle skipped"
                );
                continue;
            }

            let timestamp = nominal_timestamp(wall_anchor, start, scheduled);

            let outcome = tokio::select! {
                biased;
                () = &mut shutdown => break,
                outcome = collect_cycle(&self.sources, self.budget.cycle) => outcome,
            };

            self.store(outcome, timestamp, &mut report);
        }

        report.flushed = match self.sink.flush() {
            Ok(()) => true,
            Err(error) => {
                tracing::error!(%error, "final flush failed");
                false
            }
        };

        tracing::info!(
            cycles = report.cycles_completed,
            skipped = report.cycles_skipped,
            observations = report.observations_written,
            "scheduler stopped"
        );

        report
    }

    /// Writes one cycle. Synchronous on purpose (see the module docs).
    fn store(&self, outcome: CycleOutcome, timestamp: DateTime<Utc>, report: &mut RunReport) {
        let mut failures = 0_u64;
        let mut first_error: Option<JsonlStorageError> = None;

        for snapshot in outcome.snapshots {
            for observation in snapshot.into_observations(timestamp, self.scenario, self.interval) {
                match self.sink.append(&observation) {
                    Ok(()) => report.observations_written += 1,
                    Err(error) => {
                        failures += 1;
                        first_error.get_or_insert(error);
                    }
                }
            }
        }

        if let Some(error) = first_error {
            tracing::error!(failures, %error, "observations could not be written");
        }

        if !outcome.unmeasured.is_empty() {
            tracing::warn!(nodes = ?outcome.unmeasured, "nodes not collected in time");
        }

        report.write_failures += failures;
        report.nodes_unmeasured += outcome.unmeasured.len() as u64;
        report.cycles_completed += 1;
    }
}

/// The wall-clock time of a tick: the anchor plus the distance of the tick
/// from the start of the run, so ticks are exactly one interval apart.
fn nominal_timestamp(
    wall_anchor: DateTime<Utc>,
    start: Instant,
    scheduled: Instant,
) -> DateTime<Utc> {
    chrono::Duration::from_std(scheduled.saturating_duration_since(start))
        .ok()
        .and_then(|offset| wall_anchor.checked_add_signed(offset))
        .unwrap_or(wall_anchor)
}

/// The result of the collection phase of one cycle.
#[derive(Debug)]
pub(crate) struct CycleOutcome {
    /// One snapshot per node, in node order.
    pub snapshots: Vec<NodeSnapshot>,
    /// The nodes whose snapshot is empty because they missed the deadline
    /// or panicked.
    pub unmeasured: Vec<String>,
}

enum Job {
    Node(usize),
    Propagation,
}

enum Finished {
    /// Boxed: a snapshot is much larger than the propagation map.
    Node(usize, Box<NodeSnapshot>),
    Propagation(HashMap<String, BlockPropagationMetrics>),
}

/// Collects every node in parallel, plus the cross-node propagation, and
/// waits for all of them for at most `deadline`.
pub(crate) async fn collect_cycle(
    sources: &[Arc<dyn NodeSource>],
    deadline: Duration,
) -> CycleOutcome {
    let mut tasks: JoinSet<Finished> = JoinSet::new();
    let mut jobs: HashMap<Id, Job> = HashMap::new();

    for (index, source) in sources.iter().enumerate() {
        let source = Arc::clone(source);
        let handle =
            tasks.spawn(async move { Finished::Node(index, Box::new(source.collect().await)) });
        jobs.insert(handle.id(), Job::Node(index));
    }

    // Comparing heads needs at least two Beacon Nodes.
    let beacon_nodes = sources
        .iter()
        .filter(|source| source.beacon_client().is_some())
        .count();

    if beacon_nodes >= 2 {
        let sources = sources.to_vec();
        let handle =
            tasks.spawn(async move { Finished::Propagation(collect_propagation(&sources).await) });
        jobs.insert(handle.id(), Job::Propagation);
    }

    let mut slots: Vec<Option<NodeSnapshot>> = sources.iter().map(|_| None).collect();
    let mut propagation = HashMap::new();

    let barrier = async {
        while let Some(joined) = tasks.join_next_with_id().await {
            match joined {
                Ok((_, Finished::Node(index, snapshot))) => slots[index] = Some(*snapshot),
                Ok((_, Finished::Propagation(measured))) => propagation = measured,
                Err(error) => match jobs.get(&error.id()) {
                    Some(Job::Node(index)) => tracing::error!(
                        node = sources[*index].node_id(),
                        %error,
                        "node collection task failed"
                    ),
                    Some(Job::Propagation) => {
                        tracing::error!(%error, "propagation task failed")
                    }
                    None => tracing::error!(%error, "unknown collection task failed"),
                },
            }
        }
    };

    if timeout(deadline, barrier).await.is_err() {
        tracing::warn!(
            deadline_ms = deadline.as_millis() as u64,
            "cycle deadline reached: unfinished collections are abandoned"
        );
    }

    // Anything still running has missed the deadline.
    tasks.abort_all();

    let mut snapshots = Vec::with_capacity(sources.len());
    let mut unmeasured = Vec::new();

    for (source, slot) in sources.iter().zip(slots) {
        match slot {
            Some(mut snapshot) => {
                if let Some(measured) = propagation.remove(source.node_id()) {
                    snapshot.beacon.propagation = measured;
                }
                snapshots.push(snapshot);
            }
            None => {
                unmeasured.push(source.node_id().to_string());
                snapshots.push(NodeSnapshot::unmeasured(source.node_id()));
            }
        }
    }

    CycleOutcome {
        snapshots,
        unmeasured,
    }
}

/// Compares the heads of every Beacon Node of the cycle.
async fn collect_propagation(
    sources: &[Arc<dyn NodeSource>],
) -> HashMap<String, BlockPropagationMetrics> {
    let nodes: Vec<(&str, &BeaconClient)> = sources
        .iter()
        .filter_map(|source| Some((source.node_id(), source.beacon_client()?)))
        .collect();

    PropagationCollector::collect(&nodes).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::node_collector::test_support::{Alive, FakeNode};

    use futures_util::future::BoxFuture;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tokio::sync::oneshot;

    const INTERVAL: Duration = Duration::from_secs(5);

    fn anchor() -> DateTime<Utc> {
        "2026-11-16T08:00:00Z".parse().unwrap()
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        anchor() + chrono::Duration::seconds(seconds)
    }

    // ---- Test doubles ----

    #[derive(Clone, Copy)]
    enum Behavior {
        /// Answers after the given delay.
        Answer(Duration),
        Hang,
        Panic,
    }

    struct FakeSource {
        id: String,
        behavior: Behavior,
        calls: Arc<AtomicUsize>,
        started: Arc<Mutex<Vec<Instant>>>,
        /// Set when a hanging collection is dropped (cancelled).
        cancelled: Arc<AtomicBool>,
    }

    /// Sets a flag when dropped.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    impl FakeSource {
        fn new(id: &str, behavior: Behavior) -> Arc<Self> {
            Arc::new(Self {
                id: id.to_string(),
                behavior,
                calls: Arc::new(AtomicUsize::new(0)),
                started: Arc::new(Mutex::new(Vec::new())),
                cancelled: Arc::new(AtomicBool::new(false)),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl NodeSource for FakeSource {
        fn node_id(&self) -> &str {
            &self.id
        }

        fn collect(&self) -> BoxFuture<'_, NodeSnapshot> {
            Box::pin(async move {
                let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
                self.started.lock().unwrap().push(Instant::now());

                match self.behavior {
                    Behavior::Answer(delay) => tokio::time::sleep(delay).await,
                    Behavior::Hang => {
                        let _flag = DropFlag(Arc::clone(&self.cancelled));
                        std::future::pending::<()>().await
                    }
                    Behavior::Panic => panic!("simulated collector bug"),
                }

                // The call number tells which cycle the data comes from.
                let mut snapshot = NodeSnapshot::unmeasured(&self.id);
                snapshot.geth.p2p.peer_count = Some(call as u64);
                snapshot
            })
        }
    }

    fn sources(sources: &[&Arc<FakeSource>]) -> Vec<Arc<dyn NodeSource>> {
        sources
            .iter()
            .map(|source| Arc::clone(source) as Arc<dyn NodeSource>)
            .collect()
    }

    #[derive(Clone, Default)]
    struct MemorySink {
        written: Arc<Mutex<Vec<(Instant, Observation)>>>,
        flushes: Arc<AtomicUsize>,
        failures_left: Arc<AtomicUsize>,
    }

    impl MemorySink {
        fn failing(count: usize) -> Self {
            let sink = Self::default();
            sink.failures_left.store(count, Ordering::SeqCst);
            sink
        }

        fn observations(&self) -> Vec<Observation> {
            self.written
                .lock()
                .unwrap()
                .iter()
                .map(|(_, observation)| observation.clone())
                .collect()
        }

        fn first_write_at(&self) -> Instant {
            self.written.lock().unwrap()[0].0
        }

        fn flushes(&self) -> usize {
            self.flushes.load(Ordering::SeqCst)
        }
    }

    impl ObservationSink for MemorySink {
        fn append(&self, observation: &Observation) -> Result<(), JsonlStorageError> {
            let failing = self
                .failures_left
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                    left.checked_sub(1)
                })
                .is_ok();

            if failing {
                return Err(JsonlStorageError::Io(io::Error::other("disk full")));
            }

            self.written
                .lock()
                .unwrap()
                .push((Instant::now(), observation.clone()));
            Ok(())
        }

        fn flush(&self) -> Result<(), io::Error> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn scheduler(sources: Vec<Arc<dyn NodeSource>>, sink: &MemorySink) -> Scheduler<MemorySink> {
        Scheduler::new(sources, sink.clone(), Scenario::Normal, INTERVAL).unwrap()
    }

    /// Runs the scheduler for `duration` of virtual time.
    async fn run_for(scheduler: Scheduler<MemorySink>, duration: Duration) -> RunReport {
        scheduler
            .run_from(anchor(), tokio::time::sleep(duration))
            .await
    }

    fn peer_count_of(observation: &Observation) -> Option<u64> {
        serde_json::to_value(observation).unwrap()["metrics"]["peer_count"].as_u64()
    }

    fn all_null(observation: &Observation) -> bool {
        serde_json::to_value(observation).unwrap()["metrics"]
            .as_object()
            .unwrap()
            .values()
            .all(serde_json::Value::is_null)
    }

    // ---- Pure parts ----

    #[test]
    fn the_budget_is_80_90_and_50_percent_of_the_interval() {
        let budget = CycleBudget::for_interval(Duration::from_secs(5));

        assert_eq!(budget.collector, Duration::from_secs(4));
        assert_eq!(budget.cycle, Duration::from_millis(4500));
        assert_eq!(budget.late_tick_limit, Duration::from_millis(2500));
    }

    #[test]
    fn the_budget_is_exact_for_other_intervals() {
        let budget = CycleBudget::for_interval(Duration::from_secs(7));

        assert_eq!(budget.collector, Duration::from_millis(5600));
        assert_eq!(budget.cycle, Duration::from_millis(6300));
        assert_eq!(budget.late_tick_limit, Duration::from_millis(3500));
    }

    #[test]
    fn an_absurd_interval_does_not_overflow() {
        let budget = CycleBudget::for_interval(Duration::MAX);

        assert!(budget.collector > Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn nominal_timestamps_are_exactly_one_interval_apart() {
        let start = Instant::now();

        let first = nominal_timestamp(anchor(), start, start);
        let second = nominal_timestamp(anchor(), start, start + INTERVAL);
        let tenth = nominal_timestamp(anchor(), start, start + INTERVAL * 9);

        assert_eq!(first, anchor());
        assert_eq!(second, at(5));
        assert_eq!(tenth, at(45));
    }

    #[tokio::test(start_paused = true)]
    async fn the_timestamp_does_not_depend_on_when_the_task_woke_up() {
        let start = Instant::now();
        tokio::time::sleep(Duration::from_millis(1700)).await;

        // `scheduled` is the grid point, not `Instant::now()`.
        assert_eq!(nominal_timestamp(anchor(), start, start + INTERVAL), at(5));
    }

    // ---- Construction ----

    #[test]
    fn a_scheduler_needs_at_least_one_node() {
        let error = Scheduler::new(
            Vec::new(),
            MemorySink::default(),
            Scenario::Normal,
            INTERVAL,
        )
        .err()
        .unwrap();

        assert!(matches!(error, SchedulerError::NoNodes));
    }

    #[test]
    fn duplicate_node_ids_are_rejected() {
        let a = FakeSource::new("node-01", Behavior::Hang);
        let b = FakeSource::new("node-01", Behavior::Hang);

        let error = Scheduler::new(
            sources(&[&a, &b]),
            MemorySink::default(),
            Scenario::Normal,
            INTERVAL,
        )
        .err()
        .unwrap();

        assert!(matches!(&error, SchedulerError::DuplicateNode(id) if id == "node-01"));
    }

    #[test]
    fn an_interval_below_one_second_is_rejected() {
        let a = FakeSource::new("node-01", Behavior::Hang);

        let error = Scheduler::new(
            sources(&[&a]),
            MemorySink::default(),
            Scenario::Normal,
            Duration::from_millis(999),
        )
        .err()
        .unwrap();

        assert!(matches!(error, SchedulerError::IntervalTooShort(_)));
    }

    #[test]
    fn from_config_builds_one_collector_per_node_in_order() {
        let config = NodesConfig::from_toml_str(&five_nodes_toml()).unwrap();

        let scheduler =
            Scheduler::from_config(&config, Scenario::Eclipse, MemorySink::default()).unwrap();

        assert_eq!(
            scheduler.node_ids().collect::<Vec<_>>(),
            ["node-01", "node-02", "node-03", "node-04", "node-05"]
        );
        assert_eq!(scheduler.interval(), Duration::from_secs(5));
        assert_eq!(scheduler.budget(), CycleBudget::for_interval(INTERVAL));
    }

    fn five_nodes_toml() -> String {
        let mut toml =
            String::from("[settings]\nrequest_timeout_secs = 3\ncollection_interval_secs = 5\n");

        for index in 0..5_u16 {
            let base = 9000 + 100 * index;
            toml.push_str(&format!(
                r#"
[[nodes]]
id = "node-0{n}"

[nodes.geth]
rpc_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/debug/metrics/prometheus"

[nodes.beacon]
api_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/metrics"

[nodes.validator]
metrics_url = "http://127.0.0.1:{}/metrics"
"#,
                base,
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                n = index + 1,
            ));
        }

        toml
    }

    // ---- The loop (virtual time) ----

    #[tokio::test(start_paused = true)]
    async fn the_first_cycle_runs_immediately_and_writes_three_observations_per_node() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let b = FakeSource::new("node-02", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();

        let report = run_for(scheduler(sources(&[&a, &b]), &sink), Duration::from_secs(1)).await;

        let written = sink.observations();
        let order: Vec<_> = written
            .iter()
            .map(|o| (o.node_id.as_str(), o.component().as_str()))
            .collect();

        assert_eq!(
            order,
            [
                ("node-01", "geth"),
                ("node-01", "beacon"),
                ("node-01", "validator"),
                ("node-02", "geth"),
                ("node-02", "beacon"),
                ("node-02", "validator"),
            ]
        );
        assert!(written.iter().all(|o| o.timestamp == at(0)));
        assert!(written.iter().all(|o| o.scenario == Scenario::Normal));
        assert!(written.iter().all(|o| o.collection_interval == 5));
        assert_eq!(report.cycles_completed, 1);
        assert_eq!(report.observations_written, 6);
        assert_eq!(report.write_failures, 0);
        assert_eq!(report.nodes_unmeasured, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn cycles_follow_the_interval_grid() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();

        let report = run_for(
            scheduler(sources(&[&a]), &sink),
            Duration::from_millis(17_500),
        )
        .await;

        // Ticks at 0, 5, 10 and 15 seconds.
        assert_eq!(report.cycles_completed, 4);
        assert_eq!(a.calls(), 4);

        let stamps: Vec<_> = sink
            .observations()
            .iter()
            .filter(|o| o.component().as_str() == "geth")
            .map(|o| o.timestamp)
            .collect();
        assert_eq!(stamps, [at(0), at(5), at(10), at(15)]);
    }

    #[tokio::test(start_paused = true)]
    async fn data_of_each_cycle_lands_in_its_own_timestamp() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();

        run_for(
            scheduler(sources(&[&a]), &sink),
            Duration::from_millis(7_500),
        )
        .await;

        let geth: Vec<_> = sink
            .observations()
            .into_iter()
            .filter(|o| o.component().as_str() == "geth")
            .map(|o| (o.timestamp, peer_count_of(&o)))
            .collect();
        assert_eq!(geth, [(at(0), Some(1)), (at(5), Some(2))]);
    }

    #[tokio::test(start_paused = true)]
    async fn nodes_are_collected_concurrently_not_one_after_the_other() {
        let nodes: Vec<_> = (1..=5)
            .map(|n| {
                FakeSource::new(
                    &format!("node-0{n}"),
                    Behavior::Answer(Duration::from_secs(2)),
                )
            })
            .collect();
        let refs: Vec<_> = nodes.iter().collect();
        let sink = MemorySink::default();

        let report = run_for(scheduler(sources(&refs), &sink), Duration::from_secs(4)).await;

        // Sequentially, 5 x 2 s would overrun the 4.5 s cycle deadline.
        assert_eq!(report.cycles_completed, 1);
        assert_eq!(report.nodes_unmeasured, 0);
        // The fake sources only fill `peer_count`: one non-empty row per node.
        assert_eq!(
            sink.observations().iter().filter(|o| !all_null(o)).count(),
            5
        );

        let first_starts: Vec<_> = nodes.iter().map(|n| n.started.lock().unwrap()[0]).collect();
        assert!(first_starts.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[tokio::test(start_paused = true)]
    async fn a_hanging_node_is_cut_at_the_deadline_and_gets_an_empty_row() {
        let healthy = FakeSource::new("node-01", Behavior::Answer(Duration::from_secs(1)));
        let hanging = FakeSource::new("node-02", Behavior::Hang);
        let other = FakeSource::new("node-03", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();
        let start = Instant::now();

        let report = run_for(
            scheduler(sources(&[&healthy, &hanging, &other]), &sink),
            Duration::from_secs(7),
        )
        .await;

        // The cycle is written at 90 % of the interval, not later.
        assert_eq!(sink.first_write_at() - start, Duration::from_millis(4500));

        let written = sink.observations();
        assert_eq!(written.len(), 9);
        assert_eq!(peer_count_of(&written[0]), Some(1));
        assert!(written[3..6].iter().all(all_null));
        assert_eq!(written[3].node_id, "node-02");
        assert_eq!(peer_count_of(&written[6]), Some(1));

        assert_eq!(report.nodes_unmeasured, 1);
        assert_eq!(report.cycles_completed, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn an_overrunning_cycle_does_not_shift_the_next_ones() {
        let hanging = FakeSource::new("node-01", Behavior::Hang);
        let sink = MemorySink::default();

        let report = run_for(
            scheduler(sources(&[&hanging]), &sink),
            Duration::from_secs(12),
        )
        .await;

        // Cycles at 0 and 5 (each cut at +4.5 s) and one in flight at 10.
        assert_eq!(report.cycles_completed, 2);
        assert_eq!(hanging.calls(), 3);

        let stamps: Vec<_> = sink
            .observations()
            .iter()
            .filter(|o| o.component().as_str() == "geth")
            .map(|o| o.timestamp)
            .collect();
        assert_eq!(stamps, [at(0), at(5)]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_panicking_node_is_isolated_and_the_run_continues() {
        let healthy = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let broken = FakeSource::new("node-02", Behavior::Panic);
        let sink = MemorySink::default();

        let report = run_for(
            scheduler(sources(&[&healthy, &broken]), &sink),
            Duration::from_secs(7),
        )
        .await;

        assert_eq!(report.cycles_completed, 2);
        assert_eq!(report.nodes_unmeasured, 2);
        assert_eq!(healthy.calls(), 2);

        let written = sink.observations();
        assert_eq!(written.len(), 12);
        assert_eq!(peer_count_of(&written[0]), Some(1));
        assert!(written[3..6].iter().all(all_null));
        assert_eq!(peer_count_of(&written[6]), Some(2));
    }

    #[tokio::test(start_paused = true)]
    async fn a_tick_delivered_far_too_late_is_skipped_and_the_grid_is_kept() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();
        let (stop, stopped) = oneshot::channel::<()>();

        let task = tokio::spawn(
            scheduler(sources(&[&a]), &sink).run_from(anchor(), async move {
                let _ = stopped.await;
            }),
        );

        // Cycle at 0 runs; then the process "freezes" until 13.2 s.
        tokio::time::sleep(Duration::from_secs(1)).await;
        tokio::time::advance(Duration::from_millis(12_200)).await;
        tokio::time::sleep(Duration::from_secs(3)).await;
        stop.send(()).unwrap();
        let report = task.await.unwrap();

        let stamps: Vec<_> = sink
            .observations()
            .iter()
            .filter(|o| o.component().as_str() == "geth")
            .map(|o| o.timestamp)
            .collect();

        // 5 s was 8.2 s late: dropped. The next point of the grid is 15 s.
        assert_eq!(stamps, [at(0), at(15)]);
        assert_eq!(report.cycles_skipped, 1);
        assert_eq!(report.cycles_completed, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_slightly_late_tick_is_still_collected() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();
        let (stop, stopped) = oneshot::channel::<()>();

        let task = tokio::spawn(
            scheduler(sources(&[&a]), &sink).run_from(anchor(), async move {
                let _ = stopped.await;
            }),
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
        // The tick of 5 s is delivered at 6.5 s: 1.5 s late, under the 2.5 s limit.
        tokio::time::advance(Duration::from_millis(5_500)).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        stop.send(()).unwrap();
        let report = task.await.unwrap();

        let stamps: Vec<_> = sink
            .observations()
            .iter()
            .filter(|o| o.component().as_str() == "geth")
            .map(|o| o.timestamp)
            .collect();

        // Dated to the grid point, not to the moment it ran.
        assert_eq!(stamps, [at(0), at(5)]);
        assert_eq!(report.cycles_skipped, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn the_barrier_ends_as_soon_as_every_node_has_answered() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::from_secs(1)));
        let b = FakeSource::new("node-02", Behavior::Answer(Duration::from_secs(2)));
        let started = Instant::now();

        let outcome = collect_cycle(&sources(&[&a, &b]), Duration::from_millis(4500)).await;

        // Not at the 4.5 s deadline: when the slowest node is done.
        assert_eq!(started.elapsed(), Duration::from_secs(2));
        assert!(outcome.unmeasured.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn abandoned_collections_are_cancelled_and_not_leaked() {
        let hanging = FakeSource::new("node-01", Behavior::Hang);

        let outcome = collect_cycle(&sources(&[&hanging]), Duration::from_millis(4500)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert_eq!(outcome.unmeasured, ["node-01"]);
        assert!(
            hanging.cancelled.load(Ordering::SeqCst),
            "the task of a node that missed the deadline must be aborted"
        );
    }

    // ---- Shutdown ----

    #[tokio::test(start_paused = true)]
    async fn shutdown_before_the_first_tick_collects_nothing_and_flushes() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();

        let report = scheduler(sources(&[&a]), &sink)
            .run_from(anchor(), std::future::ready(()))
            .await;

        assert_eq!(a.calls(), 0);
        assert_eq!(report.cycles_completed, 0);
        assert!(sink.observations().is_empty());
        assert_eq!(sink.flushes(), 1);
        assert!(report.flushed);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_during_a_cycle_discards_it_and_flushes() {
        let slow = FakeSource::new("node-01", Behavior::Answer(Duration::from_secs(3)));
        let sink = MemorySink::default();

        let report = run_for(scheduler(sources(&[&slow]), &sink), Duration::from_secs(1)).await;

        assert_eq!(slow.calls(), 1);
        assert!(
            sink.observations().is_empty(),
            "a half cycle must not be written"
        );
        assert_eq!(report.cycles_completed, 0);
        assert_eq!(sink.flushes(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn the_run_flushes_once_at_the_end() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::default();

        run_for(scheduler(sources(&[&a]), &sink), Duration::from_secs(12)).await;

        assert_eq!(sink.flushes(), 1);
    }

    // ---- Storage failures ----

    #[tokio::test(start_paused = true)]
    async fn write_failures_are_counted_and_do_not_stop_the_run() {
        let a = FakeSource::new("node-01", Behavior::Answer(Duration::ZERO));
        let b = FakeSource::new("node-02", Behavior::Answer(Duration::ZERO));
        let sink = MemorySink::failing(4);

        let report = run_for(scheduler(sources(&[&a, &b]), &sink), Duration::from_secs(7)).await;

        // Cycle 1: 4 of 6 refused. Cycle 2: all accepted.
        assert_eq!(report.write_failures, 4);
        assert_eq!(report.observations_written, 2 + 6);
        assert_eq!(report.cycles_completed, 2);
        assert_eq!(sink.observations().len(), 8);
    }

    // ---- Real clients, fake servers ----

    #[tokio::test]
    async fn propagation_is_computed_across_nodes_and_merged_into_the_right_one() {
        let a = FakeNode::healthy("0xaaa").await;
        let b = FakeNode::healthy("0xaaa").await;
        let c = FakeNode::healthy("0xccc").await;
        let d = FakeNode::start(
            "0xaaa",
            Alive {
                geth: true,
                beacon: false,
                validator: true,
            },
        )
        .await;

        let budget = Duration::from_secs(4);
        let sources: Vec<Arc<dyn NodeSource>> = vec![
            Arc::new(a.collector("node-01", budget)),
            Arc::new(b.collector("node-02", budget)),
            Arc::new(c.collector("node-03", budget)),
            Arc::new(d.collector("node-04", budget)),
        ];

        let outcome = collect_cycle(&sources, Duration::from_millis(4500)).await;

        assert!(outcome.unmeasured.is_empty());
        let delay = |index: usize| {
            outcome.snapshots[index]
                .beacon
                .propagation
                .block_propagation_delay_ms
        };

        // node-01 and node-02 saw the same head: both get the shared spread.
        assert!(delay(0).is_some());
        assert_eq!(delay(0), delay(1));
        // node-03 is alone on its head, node-04's Beacon Node is down.
        assert_eq!(delay(2), None);
        assert_eq!(delay(3), None);

        // Each node still has its own data, in node order.
        let ids: Vec<_> = outcome
            .snapshots
            .iter()
            .map(|s| s.node_id.as_str())
            .collect();
        assert_eq!(ids, ["node-01", "node-02", "node-03", "node-04"]);
        assert!(outcome
            .snapshots
            .iter()
            .all(|s| s.geth.p2p.peer_count == Some(8)));
        assert_eq!(outcome.snapshots[3].beacon.sync.head_slot, None);
        assert_eq!(outcome.snapshots[0].beacon.sync.head_slot, Some(1200));
    }

    #[tokio::test]
    async fn a_single_node_has_no_propagation() {
        let a = FakeNode::healthy("0xaaa").await;
        let sources: Vec<Arc<dyn NodeSource>> =
            vec![Arc::new(a.collector("node-01", Duration::from_secs(4)))];

        let outcome = collect_cycle(&sources, Duration::from_millis(4500)).await;

        assert_eq!(
            outcome.snapshots[0]
                .beacon
                .propagation
                .block_propagation_delay_ms,
            None
        );
        assert_eq!(outcome.snapshots[0].beacon.sync.head_slot, Some(1200));
    }

    // ---- End to end: config -> real clients -> JSONL file ----

    /// Stops the run once `target` observations have been appended.
    struct StopAfter {
        inner: JsonlStorage,
        appended: AtomicUsize,
        target: usize,
        stop: Mutex<Option<oneshot::Sender<()>>>,
    }

    impl ObservationSink for Arc<StopAfter> {
        fn append(&self, observation: &Observation) -> Result<(), JsonlStorageError> {
            self.inner.append(observation)?;

            if self.appended.fetch_add(1, Ordering::SeqCst) + 1 == self.target {
                if let Some(stop) = self.stop.lock().unwrap().take() {
                    let _ = stop.send(());
                }
            }
            Ok(())
        }

        fn flush(&self) -> Result<(), io::Error> {
            self.inner.flush()
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eva-guard-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn read_lines(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[tokio::test]
    async fn one_cycle_end_to_end_writes_the_jsonl_dataset() {
        let first = FakeNode::healthy("0xaaa").await;
        let second = FakeNode::healthy("0xaaa").await;
        let toml = format!(
            "[settings]\nrequest_timeout_secs = 3\ncollection_interval_secs = 5\n{}{}",
            first.toml("node-01"),
            second.toml("node-02")
        );
        let config = NodesConfig::from_toml_str(&toml).unwrap();

        let dir = scratch_dir("scheduler-e2e");
        let (stop, stopped) = oneshot::channel::<()>();
        let sink = Arc::new(StopAfter {
            inner: JsonlStorage::new(&dir).unwrap(),
            appended: AtomicUsize::new(0),
            target: 6,
            stop: Mutex::new(Some(stop)),
        });

        // No Docker: the test stays independent of the host's containers.
        let scheduler =
            Scheduler::from_config_with_docker(&config, Scenario::RpcFlooding, sink, None)
                .unwrap();
        let report = scheduler
            .run_from(anchor(), async move {
                let _ = stopped.await;
            })
            .await;

        assert_eq!(report.cycles_completed, 1);
        assert_eq!(report.observations_written, 6);
        assert!(report.flushed);

        let lines = read_lines(&dir.join("rpc_flooding.jsonl"));
        assert_eq!(lines.len(), 6);

        let parsed: Vec<Observation> = lines
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        let order: Vec<_> = parsed
            .iter()
            .map(|o| (o.node_id.as_str(), o.component().as_str()))
            .collect();
        assert_eq!(
            order,
            [
                ("node-01", "geth"),
                ("node-01", "beacon"),
                ("node-01", "validator"),
                ("node-02", "geth"),
                ("node-02", "beacon"),
                ("node-02", "validator"),
            ]
        );
        assert!(parsed.iter().all(|o| o.timestamp == anchor()));
        assert!(parsed.iter().all(|o| o.scenario == Scenario::RpcFlooding));
        assert!(parsed.iter().all(|o| o.collection_interval == 5));

        let geth: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(geth["metrics"]["peer_count"], 8);
        assert_eq!(geth["metrics"]["rpc_available"], true);

        // Both nodes saw the same head: the propagation is measured.
        let beacon: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert!(beacon["metrics"]["block_propagation_delay_ms"].is_number());
        assert_eq!(beacon["metrics"]["head_slot"], 1200);

        let validator: serde_json::Value = serde_json::from_str(&lines[5]).unwrap();
        assert_eq!(validator["metrics"]["validators_total"], 4);

        // Without Docker the `system` group is present and null.
        assert!(geth["system"]["cpu_usage_percent"].is_null());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn errors_are_readable() {
        assert_eq!(
            SchedulerError::NoNodes.to_string(),
            "there is no node to monitor"
        );
        assert_eq!(
            SchedulerError::DuplicateNode("node-02".into()).to_string(),
            "duplicate node id: node-02"
        );
        assert!(SchedulerError::IntervalTooShort(Duration::from_millis(10))
            .to_string()
            .contains("at least 1 second"));
    }
}
