//! Live attack simulation: one continuous, **unlabelled** monitoring run that
//! behaves normally for a warm-up window and then has a controlled attack
//! injected on one node — meant to test the detection pipeline (ML model +
//! agents) end to end, not to build a labelled dataset.
//!
//! Unlike [`Campaign`](crate::monitoring::campaign::Campaign), which runs one
//! scheduler per scenario and writes the three labelled datasets, this runs a
//! single [`Scheduler`] in **live** mode (`scenario = None`), so every
//! observation is written to the live dataset without a `scenario` label. The
//! model therefore sees exactly what it would see in production and has to
//! decide on its own whether an attack is happening.
//!
//! Timeline of one run:
//!
//! 1. the scheduler starts and monitors **all** nodes continuously;
//! 2. for `warmup` the system is left alone — normal behaviour;
//! 3. after the warm-up the phase's attack is started on the victim node;
//! 4. the attack keeps running until the global shutdown (Ctrl+C / SIGTERM);
//! 5. on shutdown the attack is stopped and allowed to restore the node.
//!
//! A Ctrl+C **during** the warm-up ends the run before the attack ever starts.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{oneshot, watch};

use crate::attacks::{Attack, EclipseAttack, RpcFloodingAttack};
use crate::clients::geth_client::GethError;
use crate::config::NodesConfig;
use crate::models::observation::Scenario;
use crate::monitoring::scheduler::{RunReport, Scheduler};
use crate::storage::JsonlStorage;

#[derive(Debug, Error)]
pub enum LiveAttackError {
    #[error("the configuration has no node to attack")]
    NoNodes,

    #[error("attack target `{0}` is not a configured node")]
    UnknownTarget(String),

    #[error("NORMAL is not an attack; choose ECLIPSE or RPC_FLOODING")]
    NormalIsNotAnAttack,

    #[error("could not build the attack: {0}")]
    Attack(#[from] GethError),

    #[error("storage: {0}")]
    Storage(String),

    #[error("scheduler: {0}")]
    Scheduler(String),
}

/// What one live-attack run produced.
pub struct LiveReport {
    /// Monitoring statistics for the whole run.
    pub run: RunReport,
    /// The unlabelled dataset the observations were written to.
    pub dataset_path: PathBuf,
    /// `true` once the attack started (i.e. the warm-up elapsed before Ctrl+C).
    pub attacked: bool,
    /// Name of the attack, set only when it started.
    pub attack_name: Option<String>,
    /// Actions the attack performed (0 when it never started).
    pub attack_actions: u64,
    /// Errors the attack hit (0 when it never started).
    pub attack_errors: u64,
}

/// Runs the live-attack simulation until `shutdown` completes.
///
/// `attack_scenario` must be an attack (`ECLIPSE` or `RPC_FLOODING`); `NORMAL`
/// is rejected. `target` is the victim node id. `warmup` is how long the run
/// stays normal before the attack is injected. `flood_concurrency` is the RPC
/// flood's worker count (`None` uses the crate default).
pub async fn run(
    config: &NodesConfig,
    data_dir: PathBuf,
    attack_scenario: Scenario,
    target: String,
    warmup: Duration,
    flood_concurrency: Option<usize>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<LiveReport, LiveAttackError> {
    // Fail fast on a bad request, before any monitoring starts.
    if config.nodes.is_empty() {
        return Err(LiveAttackError::NoNodes);
    }
    if config.node(&target).is_none() {
        return Err(LiveAttackError::UnknownTarget(target));
    }
    if attack_scenario == Scenario::Normal {
        return Err(LiveAttackError::NormalIsNotAnAttack);
    }

    let flood_concurrency =
        flood_concurrency.unwrap_or(crate::attacks::rpc_flooding::DEFAULT_CONCURRENCY);

    let attack = match build_attack(config, &target, attack_scenario, flood_concurrency)? {
        Some(attack) => attack,
        None => return Err(LiveAttackError::NormalIsNotAnAttack), // unreachable after the guard
    };

    // Live storage: the `scenario` label is omitted, so the model does real
    // inference instead of reading the ground truth from the file.
    let storage =
        JsonlStorage::new(&data_dir).map_err(|error| LiveAttackError::Storage(error.to_string()))?;
    let dataset_path = storage.live_path();
    let live: Option<Scenario> = None;
    let scheduler = Scheduler::from_config(config, live, storage)
        .map_err(|error| LiveAttackError::Scheduler(error.to_string()))?;

    // A watch channel broadcasts the global shutdown to both the scheduler and
    // the attack launcher.
    let (stop_tx, stop_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown.await;
        let _ = stop_tx.send(true);
    });

    tracing::info!(
        target = %target,
        warmup_secs = warmup.as_secs(),
        attack = attack_scenario.as_str(),
        "live attack simulation started (unlabelled)"
    );

    // Attack launcher: idle through the warm-up, then attack until cancelled.
    // A shutdown during the warm-up wins the select and the attack never runs.
    let attack_name = attack.name().to_string();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let launcher = {
        let mut rx = stop_rx.clone();
        tokio::spawn(async move {
            // Wait out the warm-up, unless the shutdown fires first. The
            // `select!` yields a plain `bool`, so no `watch` read guard is held
            // across the attack's `.await` below (which would make the spawned
            // task `!Send`).
            let warmed_up = tokio::select! {
                _ = tokio::time::sleep(warmup) => true,
                _ = rx.wait_for(|stopped| *stopped) => false,
            };

            if warmed_up {
                tracing::warn!(
                    attack = attack.name(),
                    target = attack.target(),
                    "ATTACK started (controlled)"
                );
                Some(attack.run(cancel_rx).await)
            } else {
                None
            }
        })
    };

    // The scheduler runs continuously until the global shutdown.
    let run_report = {
        let mut rx = stop_rx.clone();
        scheduler
            .run(async move {
                let _ = rx.wait_for(|stopped| *stopped).await;
            })
            .await
    };

    // The run is over: stop the attack and let it restore the node.
    let _ = cancel_tx.send(());
    let attack_report = match launcher.await {
        Ok(report) => report,
        Err(error) => {
            tracing::error!(%error, "attack task failed");
            None
        }
    };

    let (attacked, attack_actions, attack_errors) = match &attack_report {
        Some(report) => (true, report.actions as u64, report.errors as u64),
        None => (false, 0, 0),
    };

    tracing::info!(
        cycles = run_report.cycles_completed,
        observations = run_report.observations_written,
        attacked,
        "live attack simulation finished"
    );

    Ok(LiveReport {
        run: run_report,
        dataset_path,
        attacked,
        attack_name: if attacked { Some(attack_name) } else { None },
        attack_actions,
        attack_errors,
    })
}

/// Builds the attack for `scenario` against the victim node's geth.
///
/// Mirrors `Campaign::attack_for`: kept here so the live run does not depend on
/// the campaign's internals. `NORMAL` (and an unknown target) yields `None`.
fn build_attack(
    config: &NodesConfig,
    target: &str,
    scenario: Scenario,
    flood_concurrency: usize,
) -> Result<Option<Arc<dyn Attack>>, GethError> {
    let node = match config.node(target) {
        Some(node) => node,
        None => return Ok(None),
    };

    let attack: Option<Arc<dyn Attack>> = match scenario {
        Scenario::Normal => None,
        Scenario::Eclipse => Some(Arc::new(EclipseAttack::new(
            target,
            &node.geth.rpc_url,
            &node.geth.metrics_url,
            config.settings.request_timeout_secs,
        )?)),
        Scenario::RpcFlooding => Some(Arc::new(
            RpcFloodingAttack::new(
                target,
                node.geth.rpc_url.clone(),
                config.settings.request_timeout_secs,
                flood_concurrency,
            )
            .map_err(GethError::Http)?,
        )),
    };

    Ok(attack)
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
            "eva-live-attack-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn an_unknown_target_is_rejected() {
        let node = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &node)]);

        let result = run(
            &config,
            scratch_dir("unknown-target"),
            Scenario::Eclipse,
            "node-99".to_string(),
            Duration::from_millis(10),
            Some(2),
            std::future::pending(),
        )
        .await;

        assert!(matches!(
            result.err(),
            Some(LiveAttackError::UnknownTarget(id)) if id == "node-99"
        ));
    }

    #[tokio::test]
    async fn normal_is_not_an_attack() {
        let node = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &node)]);

        let result = run(
            &config,
            scratch_dir("normal"),
            Scenario::Normal,
            "node-01".to_string(),
            Duration::from_millis(10),
            Some(2),
            std::future::pending(),
        )
        .await;

        assert!(matches!(
            result.err(),
            Some(LiveAttackError::NormalIsNotAnAttack)
        ));
    }

    #[tokio::test]
    async fn warms_up_then_attacks_and_writes_unlabelled_data() {
        let first = FakeNode::healthy("0xaaa").await;
        let second = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &first), ("node-02", &second)]);

        let dir = scratch_dir("warmup-then-attack");

        // Warm-up well under the total run; stop shortly after the attack starts.
        let report = run(
            &config,
            dir.clone(),
            Scenario::Eclipse,
            "node-01".to_string(),
            Duration::from_millis(200),
            Some(2),
            async {
                tokio::time::sleep(Duration::from_millis(900)).await;
            },
        )
        .await
        .unwrap();

        assert!(report.attacked, "the attack should start after the warm-up");
        assert!(report.run.observations_written > 0);

        // The live dataset exists and carries no scenario label.
        let content = std::fs::read_to_string(&report.dataset_path).unwrap();
        assert!(!content.is_empty());
        assert!(
            !content.contains("\"scenario\""),
            "live observations must be unlabelled"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_shutdown_during_the_warmup_means_no_attack() {
        let node = FakeNode::healthy("0xaaa").await;
        let config = config_of(&[("node-01", &node)]);

        let dir = scratch_dir("stopped-in-warmup");

        // Stop (300ms) before the warm-up (5s) elapses.
        let report = run(
            &config,
            dir.clone(),
            Scenario::Eclipse,
            "node-01".to_string(),
            Duration::from_secs(5),
            Some(2),
            async {
                tokio::time::sleep(Duration::from_millis(300)).await;
            },
        )
        .await
        .unwrap();

        assert!(!report.attacked, "the attack must not start during the warm-up");
        assert!(report.attack_name.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}