//! Scenario campaign: run NORMAL, then ECLIPSE, then RPC_FLOODING, back to
//! back, applying and undoing the matching attack around each phase.
//!
//! One `--campaign` run produces the three labelled datasets
//! (`normal.jsonl`, `eclipse.jsonl`, `rpc_flooding.jsonl`) without anyone
//! changing the scenario by hand. For every phase the orchestrator:
//!
//! 1. opens the dataset of that scenario and builds a fresh [`Scheduler`]
//!    (sharing the one Docker client, so system metrics are collected during
//!    the attacks too);
//! 2. starts the phase's attack on the victim node, if any, as a background
//!    task;
//! 3. runs the scheduler for `phase_duration`;
//! 4. stops the attack and lets it restore;
//! 5. waits `cooldown` so the victim recovers before the next phase.
//!
//! The monitor watches **all** nodes every phase, so the victim shows the
//! anomaly while the others stay normal. A Ctrl+C (the `shutdown` future)
//! ends the current phase cleanly, stops the attack and ends the campaign.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{oneshot, watch};

use crate::attacks::{Attack, AttackReport, EclipseAttack, RpcFloodingAttack};
use crate::clients::geth_client::GethError;
use crate::collectors::container::DockerClient;
use crate::config::NodesConfig;
use crate::models::observation::Scenario;
use crate::monitoring::scheduler::{RunReport, Scheduler};
use crate::storage::JsonlStorage;

/// Victim node when `EVA_ATTACK_TARGET` is unset: the first configured node.
pub const ENV_TARGET: &str = "EVA_ATTACK_TARGET";
/// Duration of each scenario phase, in seconds (`EVA_PHASE_SECS`).
pub const ENV_PHASE_SECS: &str = "EVA_PHASE_SECS";
/// Pause between phases, in seconds (`EVA_COOLDOWN_SECS`).
pub const ENV_COOLDOWN_SECS: &str = "EVA_COOLDOWN_SECS";
/// Concurrent workers of the RPC flood (`EVA_FLOOD_CONCURRENCY`).
pub const ENV_FLOOD_CONCURRENCY: &str = "EVA_FLOOD_CONCURRENCY";

pub const DEFAULT_PHASE: Duration = Duration::from_secs(180);
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum CampaignError {
    #[error("attack target `{0}` is not a configured node")]
    UnknownTarget(String),

    #[error("the configuration has no node to attack")]
    NoNodes,

    #[error("invalid {var}: {value}")]
    InvalidEnv { var: &'static str, value: String },
}

/// What one phase produced.
#[derive(Debug, Clone)]
pub struct PhaseReport {
    pub scenario: Scenario,
    /// `None` when the phase could not be set up (storage or scheduler error).
    pub run: Option<RunReport>,
    /// `None` for NORMAL or when the attack could not be built.
    pub attack: Option<AttackReport>,
    /// The phase was cut short by the global shutdown.
    pub interrupted: bool,
    /// Set when the phase was skipped, with the reason.
    pub skipped: Option<String>,
}

/// What a whole campaign produced, one entry per phase attempted.
#[derive(Debug, Clone, Default)]
pub struct CampaignReport {
    pub phases: Vec<PhaseReport>,
}

/// The scenario campaign. See the module documentation.
pub struct Campaign {
    config: NodesConfig,
    data_dir: PathBuf,
    order: Vec<Scenario>,
    target: String,
    phase_duration: Duration,
    cooldown: Duration,
    flood_concurrency: usize,
    docker: Option<DockerClient>,
}

impl Campaign {
    /// Builds a campaign from the configuration and environment.
    ///
    /// Reads `EVA_ATTACK_TARGET`, `EVA_PHASE_SECS`, `EVA_COOLDOWN_SECS` and
    /// `EVA_FLOOD_CONCURRENCY`, and connects to the local Docker Engine for
    /// system metrics (optional: a failure only means `null` system metrics).
    pub fn from_env(config: &NodesConfig, data_dir: PathBuf) -> Result<Self, CampaignError> {
        let default_target = config
            .nodes
            .first()
            .map(|node| node.id.clone())
            .ok_or(CampaignError::NoNodes)?;

        let target = std::env::var(ENV_TARGET).unwrap_or(default_target);
        if config.node(&target).is_none() {
            return Err(CampaignError::UnknownTarget(target));
        }

        let phase_duration = duration_from_env(ENV_PHASE_SECS, DEFAULT_PHASE)?;
        let cooldown = duration_from_env(ENV_COOLDOWN_SECS, DEFAULT_COOLDOWN)?;
        let flood_concurrency = usize_from_env(ENV_FLOOD_CONCURRENCY, crate::attacks::rpc_flooding::DEFAULT_CONCURRENCY)?;

        let docker = match DockerClient::new() {
            Ok(docker) => Some(docker),
            Err(error) => {
                tracing::error!(%error, "Docker client unavailable: system metrics will be null");
                None
            }
        };

        Ok(Self {
            config: config.clone(),
            data_dir,
            order: Scenario::ALL.to_vec(),
            target,
            phase_duration,
            cooldown,
            flood_concurrency,
            docker,
        })
    }

    /// Builds a campaign explicitly (tests and callers that do not use the
    /// environment).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: NodesConfig,
        data_dir: PathBuf,
        target: impl Into<String>,
        phase_duration: Duration,
        cooldown: Duration,
        flood_concurrency: usize,
        docker: Option<DockerClient>,
    ) -> Result<Self, CampaignError> {
        let target = target.into();
        if config.node(&target).is_none() {
            return Err(CampaignError::UnknownTarget(target));
        }

        Ok(Self {
            config,
            data_dir,
            order: Scenario::ALL.to_vec(),
            target,
            phase_duration,
            cooldown,
            flood_concurrency,
            docker,
        })
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn phase_duration(&self) -> Duration {
        self.phase_duration
    }

    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    pub fn scenarios(&self) -> &[Scenario] {
        &self.order
    }

    /// Runs the whole campaign, ending early if `shutdown` completes.
    pub async fn run(self, shutdown: impl std::future::Future<Output = ()> + Send + 'static) -> CampaignReport {
        // A watch so every phase and cooldown can observe the global shutdown.
        let (stop_tx, stop_rx) = watch::channel(false);
        tokio::spawn(async move {
            shutdown.await;
            let _ = stop_tx.send(true);
        });

        tracing::info!(
            target = %self.target,
            phase_secs = self.phase_duration.as_secs(),
            cooldown_secs = self.cooldown.as_secs(),
            scenarios = ?self.order.iter().map(|scenario| scenario.as_str()).collect::<Vec<_>>(),
            "campaign started"
        );

        let mut report = CampaignReport::default();

        for (index, scenario) in self.order.clone().into_iter().enumerate() {
            if *stop_rx.borrow() {
                break;
            }

            if index > 0 && !self.cool_down(&stop_rx).await {
                break;
            }

            let phase = self.run_phase(scenario, &stop_rx).await;
            let interrupted = phase.interrupted;
            report.phases.push(phase);

            if interrupted {
                tracing::info!(scenario = scenario.as_str(), "campaign interrupted");
                break;
            }
        }

        tracing::info!(phases = report.phases.len(), "campaign finished");
        report
    }

    /// Waits for the cooldown. Returns `false` if the shutdown fired.
    async fn cool_down(&self, stop_rx: &watch::Receiver<bool>) -> bool {
        if self.cooldown.is_zero() {
            return !*stop_rx.borrow();
        }

        tracing::info!(secs = self.cooldown.as_secs(), "cooldown between phases");
        let mut rx = stop_rx.clone();
        tokio::select! {
            _ = tokio::time::sleep(self.cooldown) => true,
            _ = rx.wait_for(|stopped| *stopped) => false,
        }
    }

    async fn run_phase(&self, scenario: Scenario, stop_rx: &watch::Receiver<bool>) -> PhaseReport {
        let skipped = |reason: String| PhaseReport {
            scenario,
            run: None,
            attack: None,
            interrupted: false,
            skipped: Some(reason),
        };

        let storage = match JsonlStorage::new(&self.data_dir) {
            Ok(storage) => storage,
            Err(error) => {
                tracing::error!(scenario = scenario.as_str(), %error, "phase skipped: storage");
                return skipped(format!("storage: {error}"));
            }
        };

        let scheduler = match Scheduler::from_config_with_docker(
            &self.config,
            scenario,
            storage,
            self.docker.clone(),
        ) {
            Ok(scheduler) => scheduler,
            Err(error) => {
                tracing::error!(scenario = scenario.as_str(), %error, "phase skipped: scheduler");
                return skipped(format!("scheduler: {error}"));
            }
        };

        let attack = match self.attack_for(scenario) {
            Ok(attack) => attack,
            Err(error) => {
                tracing::error!(
                    scenario = scenario.as_str(),
                    %error,
                    "attack could not be built: this phase runs without it"
                );
                None
            }
        };

        tracing::info!(
            scenario = scenario.as_str(),
            dataset = %self.data_dir.join(format!("{}.jsonl", scenario.dataset_name())).display(),
            attack = attack.as_ref().map(|a| a.name()).unwrap_or("none"),
            "phase starting"
        );

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let attack_task = attack.as_ref().map(|attack| {
            tracing::warn!(
                attack = attack.name(),
                target = attack.target(),
                "ATTACK started (controlled)"
            );
            tokio::spawn(Arc::clone(attack).run(cancel_rx))
        });

        // The scheduler runs for the phase, or until the global shutdown.
        let run_report = {
            let mut rx = stop_rx.clone();
            let phase_duration = self.phase_duration;
            scheduler
                .run(async move {
                    tokio::select! {
                        _ = tokio::time::sleep(phase_duration) => {}
                        _ = rx.wait_for(|stopped| *stopped) => {}
                    }
                })
                .await
        };

        // The phase is over: stop the attack and let it restore.
        let attack_report = match attack_task {
            Some(task) => {
                let _ = cancel_tx.send(());
                match task.await {
                    Ok(report) => Some(report),
                    Err(error) => {
                        tracing::error!(%error, "attack task failed");
                        None
                    }
                }
            }
            None => None,
        };

        let interrupted = *stop_rx.borrow();
        tracing::info!(
            scenario = scenario.as_str(),
            cycles = run_report.cycles_completed,
            observations = run_report.observations_written,
            interrupted,
            "phase finished"
        );

        PhaseReport {
            scenario,
            run: Some(run_report),
            attack: attack_report,
            interrupted,
            skipped: None,
        }
    }

    /// The attack for a scenario, built against the victim node's geth.
    fn attack_for(&self, scenario: Scenario) -> Result<Option<Arc<dyn Attack>>, GethError> {
        let node = match self.config.node(&self.target) {
            Some(node) => node,
            None => return Ok(None),
        };

        let attack: Option<Arc<dyn Attack>> = match scenario {
            Scenario::Normal => None,
            Scenario::Eclipse => Some(Arc::new(EclipseAttack::new(
                &self.target,
                &node.geth.rpc_url,
                &node.geth.metrics_url,
                self.config.settings.request_timeout_secs,
            )?)),
            Scenario::RpcFlooding => Some(Arc::new(
                RpcFloodingAttack::new(
                    &self.target,
                    node.geth.rpc_url.clone(),
                    self.config.settings.request_timeout_secs,
                    self.flood_concurrency,
                )
                .map_err(GethError::Http)?,
            )),
        };

        Ok(attack)
    }
}

fn duration_from_env(var: &'static str, default: Duration) -> Result<Duration, CampaignError> {
    match std::env::var(var) {
        Ok(value) => value
            .parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| CampaignError::InvalidEnv { var, value }),
        Err(_) => Ok(default),
    }
}

fn usize_from_env(var: &'static str, default: usize) -> Result<usize, CampaignError> {
    match std::env::var(var) {
        Ok(value) => value
            .parse::<usize>()
            .map_err(|_| CampaignError::InvalidEnv { var, value }),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::node_collector::test_support::FakeNode;

    fn config_of(nodes: &[(&str, &FakeNode)]) -> NodesConfig {
        let mut toml =
            String::from("[settings]\nrequest_timeout_secs = 2\ncollection_interval_secs = 1\n");
        for (id, node) in nodes {
            toml.push_str(&node.toml(id));
        }
        NodesConfig::from_toml_str(&toml).unwrap()
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eva-campaign-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn lines(path: &std::path::Path) -> usize {
        std::fs::read_to_string(path)
            .map(|content| content.lines().count())
            .unwrap_or(0)
    }

    #[test]
    fn an_unknown_target_is_rejected() {
        let config = NodesConfig::from_toml_str(
            "[settings]\nrequest_timeout_secs = 2\ncollection_interval_secs = 1\n\
             [[nodes]]\nid=\"node-01\"\n[nodes.geth]\nrpc_url=\"http://127.0.0.1:1\"\n\
             metrics_url=\"http://127.0.0.1:2/m\"\n[nodes.beacon]\napi_url=\"http://127.0.0.1:3\"\n\
             metrics_url=\"http://127.0.0.1:4/m\"\n[nodes.validator]\nmetrics_url=\"http://127.0.0.1:5/m\"\n",
        )
        .unwrap();

        let result = Campaign::new(
            config,
            PathBuf::from("/tmp"),
            "node-99",
            DEFAULT_PHASE,
            DEFAULT_COOLDOWN,
            4,
            None,
        );

        assert!(matches!(
            result.err(),
            Some(CampaignError::UnknownTarget(id)) if id == "node-99"
        ));
    }

    #[tokio::test]
    async fn runs_the_three_scenarios_and_writes_their_datasets() {
        let first = FakeNode::healthy("0xaaa").await;
        let second = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &first), ("node-02", &second)]);

        let dir = scratch_dir("three-phases");
        let campaign = Campaign::new(
            config,
            dir.clone(),
            "node-01",
            Duration::from_millis(900),
            Duration::ZERO,
            2,
            None, // no Docker: the test is independent of the host
        )
        .unwrap();

        // The shutdown never fires: the campaign ends after its three phases.
        let report = campaign.run(std::future::pending()).await;

        assert_eq!(report.phases.len(), 3);
        let scenarios: Vec<_> = report.phases.iter().map(|p| p.scenario).collect();
        assert_eq!(
            scenarios,
            [Scenario::Normal, Scenario::Eclipse, Scenario::RpcFlooding]
        );
        assert!(report.phases.iter().all(|p| p.skipped.is_none()));
        assert!(report.phases.iter().all(|p| !p.interrupted));

        // NORMAL has no attack; the other two do.
        assert!(report.phases[0].attack.is_none());
        assert!(report.phases[1].attack.is_some());
        assert!(report.phases[2].attack.is_some());

        // Each scenario wrote its own dataset, 6 observations per cycle.
        for scenario in Scenario::ALL {
            let path = dir.join(format!("{}.jsonl", scenario.dataset_name()));
            let count = lines(&path);
            assert!(count >= 6 && count.is_multiple_of(6), "{scenario}: {count} lines");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn the_global_shutdown_ends_the_campaign_early() {
        let node = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &node)]);

        let dir = scratch_dir("interrupted");
        let campaign = Campaign::new(
            config,
            dir.clone(),
            "node-01",
            Duration::from_secs(30),
            Duration::ZERO,
            2,
            None,
        )
        .unwrap();

        // Stop during the first (NORMAL) phase.
        let report = campaign
            .run(async {
                tokio::time::sleep(Duration::from_millis(300)).await;
            })
            .await;

        assert_eq!(report.phases.len(), 1);
        assert_eq!(report.phases[0].scenario, Scenario::Normal);
        assert!(report.phases[0].interrupted);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
